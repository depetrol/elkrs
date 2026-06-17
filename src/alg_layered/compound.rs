//! Cross-hierarchy edge
//! splitting for compound (INCLUDE_CHILDREN) layout.
//!
//! `CompoundGraphPreprocessor` splits cross-hierarchy edges into per-level
//! segments connected by external-port dummies, storing the segments in the
//! `CROSS_HIERARCHY_MAP` attached to the top-level graph;
//! `CompoundGraphPostprocessor` restores the original edges from those
//! segments after layout.

use crate::core::options::{Direction, PortConstraints, PortLabelPlacement, PortSide};
use crate::graph::math::{KVector, KVectorChain};
use crate::graph::properties::{EnumSet, JavaCloneable, JavaString, PropertyMap};

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LLabelId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_util::{self, PortPropertyHolder};
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::{GraphProperties, PortType};

/// One segment of a cross-hierarchy edge in a
/// single graph of the hierarchy.
#[derive(Clone, Debug, PartialEq)]
pub struct CrossHierarchyEdge {
    /// the dummy edge used in the layered graph to compute a layout.
    pub edge: LEdgeId,
    /// the layered graph in which the layout was computed.
    pub graph: LGraphId,
    /// the flow direction: input or output.
    pub port_type: PortType,
}

impl CrossHierarchyEdge {
    /// `CrossHierarchyEdge.getActualSource`.
    fn actual_source(&self, a: &LGraphArena) -> LPortId {
        let src = a.edge(self.edge).source.unwrap();
        let node = a.port(src).node.unwrap();
        if a.node(node).node_type == NodeType::EXTERNAL_PORT {
            if let Some(crate::alg_layered::internal_properties::Origin::LPort(p)) =
                a.node(node).properties.try_get(&iprops::ORIGIN)
            {
                return p;
            }
        }
        src
    }

    /// `CrossHierarchyEdge.getActualTarget`.
    fn actual_target(&self, a: &LGraphArena) -> LPortId {
        let tgt = a.edge(self.edge).target.unwrap();
        let node = a.port(tgt).node.unwrap();
        if a.node(node).node_type == NodeType::EXTERNAL_PORT {
            if let Some(crate::alg_layered::internal_properties::Origin::LPort(p)) =
                a.node(node).properties.try_get(&iprops::ORIGIN)
            {
                return p;
            }
        }
        tgt
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct CrossHierarchyMap(pub indexmap::IndexMap<LEdgeId, Vec<CrossHierarchyEdge>>);

impl CrossHierarchyMap {
    fn put(&mut self, orig: LEdgeId, seg: CrossHierarchyEdge) {
        self.0.entry(orig).or_default().push(seg);
    }
}

impl JavaString for CrossHierarchyMap {
    fn java_string(&self) -> String {
        format!("{:?}", self)
    }
}
impl JavaCloneable for CrossHierarchyMap {
    const CLONEABLE: bool = false;
}

/// Internal representation of an external port.
struct ExternalPort {
    orig_edges: Vec<LEdgeId>,
    new_edge: LEdgeId,
    dummy_node: LNodeId,
    dummy_port: LPortId,
    port_type: PortType,
    exported: bool,
}

/// Mutable state of the preprocessor.
struct Preprocessor {
    cross_hierarchy_map: CrossHierarchyMap,
    /// map of ports to their assigned dummy nodes in the nested graphs.
    dummy_node_map: indexmap::IndexMap<LPortId, LNodeId>,
}

pub fn preprocess(a: &mut LGraphArena, lgraph: LGraphId) -> Result<(), String> {
    let mut pre = Preprocessor {
        cross_hierarchy_map: CrossHierarchyMap::default(),
        dummy_node_map: indexmap::IndexMap::new(),
    };

    pre.transform_hierarchy_edges(a, lgraph, None);
    pre.move_labels_and_remove_original_edges(a, lgraph);
    pre.set_sides_of_ports_to_sides_of_dummy_nodes(a);

    a.graph(lgraph)
        .properties
        .set(&iprops::CROSS_HIERARCHY_MAP, pre.cross_hierarchy_map);
    Ok(())
}

impl Preprocessor {
    fn set_sides_of_ports_to_sides_of_dummy_nodes(&self, a: &mut LGraphArena) {
        for (&external_port, &dummy_node) in &self.dummy_node_map {
            a.node(dummy_node)
                .properties
                .set(&iprops::ORIGIN, crate::alg_layered::internal_properties::Origin::LPort(external_port));
            a.port(external_port)
                .properties
                .set(&iprops::PORT_DUMMY, dummy_node);
            a.port(external_port)
                .properties
                .set(&iprops::INSIDE_CONNECTIONS, true);
            let side: PortSide = a.node(dummy_node).properties.get(&iprops::EXT_PORT_SIDE);
            a.port_set_side(external_port, side);

            let port_node = a.port(external_port).node.unwrap();
            a.node(port_node)
                .properties
                .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_SIDE);
            let g = a.node_graph(port_node);
            let mut gp: EnumSet<GraphProperties> =
                a.graph(g).properties.get(&iprops::GRAPH_PROPERTIES);
            gp.add(GraphProperties::NON_FREE_PORTS);
            a.graph(g).properties.set(&iprops::GRAPH_PROPERTIES, gp);
        }
    }

    fn transform_hierarchy_edges(
        &mut self,
        a: &mut LGraphArena,
        graph: LGraphId,
        parent_node: Option<LNodeId>,
    ) -> Vec<ExternalPort> {
        let mut contained_external_ports: Vec<ExternalPort> = Vec::new();

        let nodes = a.graph(graph).layerless_nodes.clone();
        for node in nodes {
            if let Some(nested_graph) = a.node(node).nested_graph {
                let child_ports = self.transform_hierarchy_edges(a, nested_graph, Some(node));
                contained_external_ports.extend(child_ports);

                self.process_inside_self_loops(a, nested_graph, node);

                // make sure all hierarchical ports have dummy nodes
                let gp: EnumSet<GraphProperties> =
                    a.graph(nested_graph).properties.get(&iprops::GRAPH_PROPERTIES);
                if gp.contains(GraphProperties::EXTERNAL_PORTS) {
                    let port_constraints: PortConstraints =
                        a.node(node).properties.get(&lopts::PORT_CONSTRAINTS);
                    let inside_port_labels: bool = a
                        .node(node)
                        .properties
                        .get::<EnumSet<PortLabelPlacement>>(&lopts::PORT_LABELS_PLACEMENT)
                        .contains(PortLabelPlacement::INSIDE);

                    let ports = a.node(node).ports.clone();
                    for port in ports {
                        let dummy_node = match self.dummy_node_map.get(&port) {
                            Some(&d) => d,
                            None => {
                                let side = a.port(port).side;
                                let net_flow = a.port_net_flow(port);
                                let port_size = a.port(port).size;
                                let direction: Direction =
                                    a.graph(nested_graph).properties.get(&lopts::DIRECTION);
                                let d = lgraph_util::create_external_port_dummy(
                                    a,
                                    PortPropertyHolder::LPort(port),
                                    port_constraints,
                                    side,
                                    -net_flow,
                                    None,
                                    Some(KVector::default()),
                                    port_size,
                                    direction,
                                    nested_graph,
                                );
                                a.node(d).properties.set(
                                    &iprops::ORIGIN,
                                    crate::alg_layered::internal_properties::Origin::LPort(port),
                                );
                                self.dummy_node_map.insert(port, d);
                                a.graph_mut(nested_graph).layerless_nodes.push(d);
                                d
                            }
                        };

                        // reserve space for external port labels on the dummy's port
                        let dummy_node_port = a.node(dummy_node).ports[0];
                        let port_labels = a.port(port).labels.clone();
                        let port_side = a.port(port).side;
                        let port_size = a.port(port).size;
                        let labels_fixed = port_label_placement_is_fixed(
                            a.node(node).properties.get(&lopts::PORT_LABELS_PLACEMENT),
                        );
                        for ext_port_label in port_labels {
                            let lsize = a.label(ext_port_label).size;
                            let lpos = a.label(ext_port_label).pos;
                            let dummy_label = a.create_label("");
                            a.label_mut(dummy_label).size.x = lsize.x;
                            a.label_mut(dummy_label).size.y = lsize.y;
                            a.port_mut(dummy_node_port).labels.push(dummy_label);

                            if !inside_port_labels {
                                let mut inside_part = 0.0;
                                if labels_fixed {
                                    inside_part = crate::core::elkutil::compute_inside_part_values(
                                        lpos, lsize, port_size, 0.0, port_side,
                                    );
                                }
                                if port_constraints == PortConstraints::FREE
                                    || port_side == PortSide::EAST
                                    || port_side == PortSide::WEST
                                {
                                    a.label_mut(dummy_label).size.x = inside_part;
                                } else {
                                    a.label_mut(dummy_label).size.y = inside_part;
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut exported_external_ports: Vec<ExternalPort> = Vec::new();

        self.process_inner_hierarchical_edge_segments(
            a,
            graph,
            parent_node,
            &contained_external_ports,
            &mut exported_external_ports,
        );

        if let Some(parent) = parent_node {
            self.process_outer_hierarchical_edge_segments(
                a,
                graph,
                parent,
                &mut exported_external_ports,
            );
        }

        exported_external_ports
    }

    fn move_labels_and_remove_original_edges(&mut self, a: &mut LGraphArena, graph: LGraphId) {
        let orig_edges: Vec<LEdgeId> = self.cross_hierarchy_map.0.keys().copied().collect();
        for orig_edge in orig_edges {
            let labels = a.edge(orig_edge).labels.clone();
            if !labels.is_empty() {
                let mut edge_segments = self.cross_hierarchy_map.0[&orig_edge].clone();
                sort_segments(a, &mut edge_segments, graph);

                let mut remaining: Vec<LLabelId> = Vec::new();
                for curr_label in labels {
                    let placement: crate::core::options::EdgeLabelPlacement =
                        a.label(curr_label).properties.get(&lopts::EDGE_LABELS_PLACEMENT);
                    let target_index: i64 = match placement {
                        crate::core::options::EdgeLabelPlacement::HEAD => {
                            edge_segments.len() as i64 - 1
                        }
                        crate::core::options::EdgeLabelPlacement::CENTER => {
                            get_shallowest_edge_segment(&edge_segments)
                        }
                        crate::core::options::EdgeLabelPlacement::TAIL => 0,
                    };

                    if target_index != -1 {
                        let target_segment = &edge_segments[target_index as usize];
                        let seg_edge = target_segment.edge;
                        a.edge_mut(seg_edge).labels.push(curr_label);
                        let src_node = a.edge_source_node(seg_edge);
                        let g = a.node_graph(src_node);
                        let mut gp: EnumSet<GraphProperties> =
                            a.graph(g).properties.get(&iprops::GRAPH_PROPERTIES);
                        gp.add(GraphProperties::END_LABELS);
                        gp.add(GraphProperties::CENTER_LABELS);
                        a.graph(g).properties.set(&iprops::GRAPH_PROPERTIES, gp);

                        a.label(curr_label)
                            .properties
                            .set(&iprops::ORIGINAL_LABEL_EDGE, orig_edge);
                    } else {
                        remaining.push(curr_label);
                    }
                }
                a.edge_mut(orig_edge).labels = remaining;
            }

            // remove original edge
            a.edge_set_source(orig_edge, None);
            a.edge_set_target(orig_edge, None);
        }
    }

    fn process_inner_hierarchical_edge_segments(
        &mut self,
        a: &mut LGraphArena,
        graph: LGraphId,
        parent_node: Option<LNodeId>,
        contained_external_ports: &[ExternalPort],
        exported_external_ports: &mut Vec<ExternalPort>,
    ) {
        let mut created_external_ports: Vec<ExternalPort> = Vec::new();

        for (ep_index, external_port) in contained_external_ports.iter().enumerate() {
            let mut current_external_port: Option<usize> = None; // index into created_external_ports

            if external_port.port_type == PortType::OUTPUT {
                for &out_edge in &external_port.orig_edges.clone() {
                    let target_node = a.edge_target_node(out_edge);
                    if a.node_graph(target_node) == graph {
                        // case 1: connects to a direct child
                        let target = a.edge(out_edge).target.unwrap();
                        self.connect_child(
                            a,
                            graph,
                            external_port.port_type,
                            out_edge,
                            external_port.dummy_port,
                            target,
                        );
                    } else if parent_node.is_none()
                        || lgraph_util::is_descendant(a, target_node, parent_node.unwrap())
                    {
                        // case 2: connects two direct children
                        self.connect_siblings(
                            a,
                            graph,
                            external_port,
                            contained_external_ports,
                            ep_index,
                            out_edge,
                        );
                    } else {
                        // case 3: connects to parent or outside
                        self.introduce_hierarchical_edge_segment(
                            a,
                            graph,
                            parent_node.unwrap(),
                            out_edge,
                            external_port.dummy_port,
                            PortType::OUTPUT,
                            &mut created_external_ports,
                            &mut current_external_port,
                        );
                    }
                }
            } else {
                for &in_edge in &external_port.orig_edges.clone() {
                    let source_node = a.edge_source_node(in_edge);
                    if a.node_graph(source_node) == graph {
                        // case 1: comes from a direct child
                        let source = a.edge(in_edge).source.unwrap();
                        self.connect_child(
                            a,
                            graph,
                            external_port.port_type,
                            in_edge,
                            source,
                            external_port.dummy_port,
                        );
                    } else if parent_node.is_none()
                        || lgraph_util::is_descendant(a, source_node, parent_node.unwrap())
                    {
                        // case 2: handled by output port code; nothing to do
                        continue;
                    } else {
                        // case 3: comes from parent or outside
                        self.introduce_hierarchical_edge_segment(
                            a,
                            graph,
                            parent_node.unwrap(),
                            in_edge,
                            external_port.dummy_port,
                            PortType::INPUT,
                            &mut created_external_ports,
                            &mut current_external_port,
                        );
                    }
                }
            }
        }

        for external_port in created_external_ports {
            if !a
                .graph(graph)
                .layerless_nodes
                .contains(&external_port.dummy_node)
            {
                a.graph_mut(graph).layerless_nodes.push(external_port.dummy_node);
            }
            if external_port.exported {
                exported_external_ports.push(external_port);
            }
        }
    }

    fn connect_child(
        &mut self,
        a: &mut LGraphArena,
        graph: LGraphId,
        port_type: PortType,
        orig_edge: LEdgeId,
        source_port: LPortId,
        target_port: LPortId,
    ) {
        let dummy_edge = create_dummy_edge(a, orig_edge);
        a.edge_set_source(dummy_edge, Some(source_port));
        a.edge_set_target(dummy_edge, Some(target_port));
        self.cross_hierarchy_map.put(
            orig_edge,
            CrossHierarchyEdge {
                edge: dummy_edge,
                graph,
                port_type,
            },
        );
    }

    fn connect_siblings(
        &mut self,
        a: &mut LGraphArena,
        graph: LGraphId,
        external_output_port: &ExternalPort,
        contained_external_ports: &[ExternalPort],
        output_index: usize,
        orig_edge: LEdgeId,
    ) {
        // find the opposite external port
        let mut target_dummy_port = None;
        for (i, ep2) in contained_external_ports.iter().enumerate() {
            if i != output_index && ep2.orig_edges.contains(&orig_edge) {
                debug_assert!(ep2.port_type == PortType::INPUT);
                target_dummy_port = Some(ep2.dummy_port);
                break;
            }
        }
        let target_dummy_port = target_dummy_port.expect("sibling external port not found");

        let dummy_edge = create_dummy_edge(a, orig_edge);
        a.edge_set_source(dummy_edge, Some(external_output_port.dummy_port));
        a.edge_set_target(dummy_edge, Some(target_dummy_port));
        self.cross_hierarchy_map.put(
            orig_edge,
            CrossHierarchyEdge {
                edge: dummy_edge,
                graph,
                port_type: external_output_port.port_type,
            },
        );
    }

    fn process_outer_hierarchical_edge_segments(
        &mut self,
        a: &mut LGraphArena,
        graph: LGraphId,
        parent_node: LNodeId,
        exported_external_ports: &mut Vec<ExternalPort>,
    ) {
        let mut created_external_ports: Vec<ExternalPort> = Vec::new();

        let child_nodes = a.graph(graph).layerless_nodes.clone();
        for child_node in child_nodes {
            let ports = a.node(child_node).ports.clone();
            for child_port in ports {
                let mut current_output: Option<usize> = None;
                let out_edges = a.port(child_port).outgoing_edges.clone();
                for out_edge in out_edges {
                    let target_node = a.edge_target_node(out_edge);
                    if !lgraph_util::is_descendant(a, target_node, parent_node) {
                        let source = a.edge(out_edge).source.unwrap();
                        self.introduce_hierarchical_edge_segment(
                            a,
                            graph,
                            parent_node,
                            out_edge,
                            source,
                            PortType::OUTPUT,
                            &mut created_external_ports,
                            &mut current_output,
                        );
                    }
                }

                let mut current_input: Option<usize> = None;
                let in_edges = a.port(child_port).incoming_edges.clone();
                for in_edge in in_edges {
                    let source_node = a.edge_source_node(in_edge);
                    if !lgraph_util::is_descendant(a, source_node, parent_node) {
                        let target = a.edge(in_edge).target.unwrap();
                        self.introduce_hierarchical_edge_segment(
                            a,
                            graph,
                            parent_node,
                            in_edge,
                            target,
                            PortType::INPUT,
                            &mut created_external_ports,
                            &mut current_input,
                        );
                    }
                }
            }
        }

        for external_port in created_external_ports {
            if !a
                .graph(graph)
                .layerless_nodes
                .contains(&external_port.dummy_node)
            {
                a.graph_mut(graph).layerless_nodes.push(external_port.dummy_node);
            }
            if external_port.exported {
                exported_external_ports.push(external_port);
            }
        }
    }

    fn process_inside_self_loops(
        &mut self,
        a: &mut LGraphArena,
        nested_graph: LGraphId,
        node: LNodeId,
    ) {
        if !a.node(node).properties.get(&lopts::INSIDE_SELF_LOOPS_ACTIVATE) {
            return;
        }

        let ports = a.node(node).ports.clone();
        for lport in ports {
            let out_edges = a.port(lport).outgoing_edges.clone();
            for out_edge in out_edges {
                let is_self_loop = a.edge_target_node(out_edge) == node;
                let is_inside_self_loop = is_self_loop
                    && a.edge(out_edge).properties.get(&lopts::INSIDE_SELF_LOOPS_YO);

                if is_inside_self_loop {
                    let source_port = a.edge(out_edge).source.unwrap();
                    let source_dummy = self.ensure_self_loop_dummy(a, nested_graph, source_port, -1);
                    let target_port = a.edge(out_edge).target.unwrap();
                    let target_dummy = self.ensure_self_loop_dummy(a, nested_graph, target_port, 1);

                    let dummy_edge = create_dummy_edge(a, out_edge);
                    let sp = a.node(source_dummy).ports[0];
                    let tp = a.node(target_dummy).ports[0];
                    a.edge_set_source(dummy_edge, Some(sp));
                    a.edge_set_target(dummy_edge, Some(tp));

                    self.cross_hierarchy_map.put(
                        out_edge,
                        CrossHierarchyEdge {
                            edge: dummy_edge,
                            graph: nested_graph,
                            port_type: PortType::OUTPUT,
                        },
                    );

                    let mut gp: EnumSet<GraphProperties> =
                        a.graph(nested_graph).properties.get(&iprops::GRAPH_PROPERTIES);
                    gp.add(GraphProperties::EXTERNAL_PORTS);
                    a.graph(nested_graph).properties.set(&iprops::GRAPH_PROPERTIES, gp);
                }
            }
        }
    }

    fn ensure_self_loop_dummy(
        &mut self,
        a: &mut LGraphArena,
        nested_graph: LGraphId,
        port: LPortId,
        net_flow: i32,
    ) -> LNodeId {
        if let Some(&d) = self.dummy_node_map.get(&port) {
            return d;
        }
        let side = a.port(port).side;
        let port_size = a.port(port).size;
        let direction: Direction = a.graph(nested_graph).properties.get(&lopts::DIRECTION);
        let d = lgraph_util::create_external_port_dummy(
            a,
            PortPropertyHolder::LPort(port),
            PortConstraints::FREE,
            side,
            net_flow,
            None,
            None,
            port_size,
            direction,
            nested_graph,
        );
        a.node(d)
            .properties
            .set(&iprops::ORIGIN, crate::alg_layered::internal_properties::Origin::LPort(port));
        self.dummy_node_map.insert(port, d);
        a.graph_mut(nested_graph).layerless_nodes.push(d);
        d
    }

    /// The created/reused external
    /// port is recorded in `created` and `current`.
    #[allow(clippy::too_many_arguments)]
    fn introduce_hierarchical_edge_segment(
        &mut self,
        a: &mut LGraphArena,
        graph: LGraphId,
        parent_node: LNodeId,
        orig_edge: LEdgeId,
        opposite_port: LPortId,
        port_type: PortType,
        created: &mut Vec<ExternalPort>,
        current: &mut Option<usize>,
    ) {
        let merge_external_ports: bool = a.graph(graph).properties.get(&lopts::MERGE_HIERARCHY_EDGES);

        // does the edge connect to the parent node?
        let mut parent_end_port: Option<LPortId> = None;
        if port_type == PortType::INPUT && a.edge_source_node(orig_edge) == parent_node {
            parent_end_port = a.edge(orig_edge).source;
        } else if port_type == PortType::OUTPUT && a.edge_target_node(orig_edge) == parent_node {
            parent_end_port = a.edge(orig_edge).target;
        }

        let default_external_port = current.map(|i| &created[i]);
        if default_external_port.is_none() || !merge_external_ports || parent_end_port.is_some() {
            // create a new external port
            let mut external_port_side = PortSide::UNDEFINED;
            if let Some(pep) = parent_end_port {
                external_port_side = a.port(pep).side;
            } else if a
                .node(parent_node)
                .properties
                .get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
                .is_side_fixed()
            {
                external_port_side = if port_type == PortType::INPUT {
                    PortSide::WEST
                } else {
                    PortSide::EAST
                };
            }

            let dummy_node = self.create_external_port_dummy(
                a,
                graph,
                parent_node,
                port_type,
                external_port_side,
                orig_edge,
            );

            let parent_graph = a.node_graph(parent_node);
            let dummy_edge = create_dummy_edge(a, orig_edge);
            let dummy_port = a.node(dummy_node).ports[0];
            // ensure edge lives in the parent graph conceptually — the arena
            // does not track per-edge graph membership; connectivity is what
            // matters. We simply connect.
            let _ = parent_graph;
            if port_type == PortType::INPUT {
                a.edge_set_source(dummy_edge, Some(dummy_port));
                a.edge_set_target(dummy_edge, Some(opposite_port));
            } else {
                a.edge_set_source(dummy_edge, Some(opposite_port));
                a.edge_set_target(dummy_edge, Some(dummy_port));
            }

            let dummy_origin_port = match a.node(dummy_node).properties.try_get(&iprops::ORIGIN) {
                Some(crate::alg_layered::internal_properties::Origin::LPort(p)) => p,
                _ => panic!("external port dummy origin not an LPort"),
            };

            self.cross_hierarchy_map.put(
                orig_edge,
                CrossHierarchyEdge {
                    edge: dummy_edge,
                    graph,
                    port_type,
                },
            );

            let exported = parent_end_port.is_none();
            created.push(ExternalPort {
                orig_edges: vec![orig_edge],
                new_edge: dummy_edge,
                dummy_node,
                dummy_port: dummy_origin_port,
                port_type,
                exported,
            });
            if exported {
                *current = Some(created.len() - 1);
            }
        } else {
            // reuse the existing external port
            let idx = current.unwrap();
            let new_edge = created[idx].new_edge;
            created[idx].orig_edges.push(orig_edge);

            let thickness = a
                .edge(new_edge)
                .properties
                .get::<f64>(&lopts::EDGE_THICKNESS)
                .max(a.edge(orig_edge).properties.get::<f64>(&lopts::EDGE_THICKNESS));
            a.edge(new_edge).properties.set(&lopts::EDGE_THICKNESS, thickness);

            self.cross_hierarchy_map.put(
                orig_edge,
                CrossHierarchyEdge {
                    edge: new_edge,
                    graph,
                    port_type,
                },
            );
        }
    }

    fn create_external_port_dummy(
        &mut self,
        a: &mut LGraphArena,
        graph: LGraphId,
        parent_node: LNodeId,
        port_type: PortType,
        port_side: PortSide,
        edge: LEdgeId,
    ) -> LNodeId {
        // outside port the edge connects to
        let outside_port = if port_type == PortType::INPUT {
            a.edge(edge).source.unwrap()
        } else {
            a.edge(edge).target.unwrap()
        };
        let layout_direction = lgraph_util::get_direction(a, graph);

        let dummy_node;
        if a.port(outside_port).node == Some(parent_node) {
            if let Some(&d) = self.dummy_node_map.get(&outside_port) {
                dummy_node = d;
            } else {
                let pc: PortConstraints =
                    a.node(parent_node).properties.get(&lopts::PORT_CONSTRAINTS);
                let net_flow = self.calculate_net_flow(a, outside_port);
                let position = a.port(outside_port).pos;
                let size = a.port(outside_port).size;
                let d = lgraph_util::create_external_port_dummy(
                    a,
                    PortPropertyHolder::LPort(outside_port),
                    pc,
                    port_side,
                    net_flow,
                    None,
                    Some(position),
                    size,
                    layout_direction,
                    graph,
                );
                a.node(d).properties.set(
                    &iprops::ORIGIN,
                    crate::alg_layered::internal_properties::Origin::LPort(outside_port),
                );
                self.dummy_node_map.insert(outside_port, d);
                dummy_node = d;
            }
        } else {
            let pc: PortConstraints =
                a.node(parent_node).properties.get(&lopts::PORT_CONSTRAINTS);
            let holder = create_external_port_properties(a, graph);
            let net_flow = if port_type == PortType::INPUT { -1 } else { 1 };
            let d = lgraph_util::create_external_port_dummy(
                a,
                PortPropertyHolder::Map(&holder),
                pc,
                port_side,
                net_flow,
                None,
                Some(KVector::default()),
                KVector::new(0.0, 0.0),
                layout_direction,
                graph,
            );
            let dummy_port = self.create_port_for_dummy(a, d, parent_node, port_type);
            a.node(d)
                .properties
                .set(&iprops::ORIGIN, crate::alg_layered::internal_properties::Origin::LPort(dummy_port));
            self.dummy_node_map.insert(dummy_port, d);
            dummy_node = d;
        }

        // graph properties
        let mut gp: EnumSet<GraphProperties> =
            a.graph(graph).properties.get(&iprops::GRAPH_PROPERTIES);
        gp.add(GraphProperties::EXTERNAL_PORTS);
        a.graph(graph).properties.set(&iprops::GRAPH_PROPERTIES, gp);
        if a.graph(graph)
            .properties
            .get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
            .is_side_fixed()
        {
            a.graph(graph)
                .properties
                .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_SIDE);
        } else {
            a.graph(graph)
                .properties
                .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FREE);
        }

        dummy_node
    }

    fn create_port_for_dummy(
        &self,
        a: &mut LGraphArena,
        dummy_node: LNodeId,
        parent_node: LNodeId,
        port_type: PortType,
    ) -> LPortId {
        let graph = a.node_graph(parent_node);
        let layout_direction = lgraph_util::get_direction(a, graph);
        let port = a.create_port();
        a.port_set_node(port, Some(parent_node));
        match port_type {
            PortType::INPUT => {
                a.port_set_side(port, PortSide::from_direction(layout_direction).opposed());
            }
            PortType::OUTPUT => {
                a.port_set_side(port, PortSide::from_direction(layout_direction));
            }
            PortType::UNDEFINED => {}
        }
        let offset: f64 = a.node(dummy_node).properties.get(&lopts::PORT_BORDER_OFFSET);
        a.port(port).properties.set(&lopts::PORT_BORDER_OFFSET, offset);
        port
    }

    fn calculate_net_flow(&self, a: &LGraphArena, port: LPortId) -> i32 {
        let node = a.port(port).node.unwrap();
        let inside_self_loops_enabled: bool =
            a.node(node).properties.get(&lopts::INSIDE_SELF_LOOPS_ACTIVATE);

        let mut output_vote = 0;
        let mut input_vote = 0;

        for &out in &a.port(port).outgoing_edges {
            let is_self_loop = a.edge_is_self_loop(out);
            let is_inside_self_loop = is_self_loop
                && inside_self_loops_enabled
                && a.edge(out).properties.get(&lopts::INSIDE_SELF_LOOPS_YO);
            let target_node = a.edge_target_node(out);

            if is_self_loop && is_inside_self_loop {
                input_vote += 1;
            } else if is_self_loop && !is_inside_self_loop {
                output_vote += 1;
            } else if a.graph(a.node_graph(target_node)).parent_node == Some(node) {
                input_vote += 1;
            } else {
                output_vote += 1;
            }
        }

        for &inc in &a.port(port).incoming_edges {
            let is_self_loop = a.edge_is_self_loop(inc);
            let is_inside_self_loop = is_self_loop
                && inside_self_loops_enabled
                && a.edge(inc).properties.get(&lopts::INSIDE_SELF_LOOPS_YO);
            let source_node = a.edge_source_node(inc);

            if is_self_loop && is_inside_self_loop {
                output_vote += 1;
            } else if is_self_loop && !is_inside_self_loop {
                input_vote += 1;
            } else if a.graph(a.node_graph(source_node)).parent_node == Some(node) {
                output_vote += 1;
            } else {
                input_vote += 1;
            }
        }

        output_vote - input_vote
    }
}

fn create_dummy_edge(a: &mut LGraphArena, orig_edge: LEdgeId) -> LEdgeId {
    let dummy_edge = a.create_edge();
    let props = a.edge(orig_edge).properties.clone();
    a.edge(dummy_edge).properties.copy_from(&props);
    a.edge(dummy_edge).properties.unset(&lopts::JUNCTION_POINTS);
    dummy_edge
}

fn create_external_port_properties(a: &LGraphArena, graph: LGraphId) -> PropertyMap {
    let holder = PropertyMap::new();
    let offset = a.graph(graph).properties.get::<f64>(&lopts::SPACING_EDGE_EDGE) / 2.0;
    holder.set(&lopts::PORT_BORDER_OFFSET, offset);
    holder
}

/// `PortLabelPlacement.isFixed(Set)`.
fn port_label_placement_is_fixed(placement: EnumSet<PortLabelPlacement>) -> bool {
    !placement.contains(PortLabelPlacement::INSIDE)
        && !placement.contains(PortLabelPlacement::OUTSIDE)
}

fn get_shallowest_edge_segment(edge_segments: &[CrossHierarchyEdge]) -> i64 {
    let mut result = -1;
    let mut index = 0;
    for che in edge_segments {
        if che.port_type == PortType::INPUT {
            result = if index == 0 { 0 } else { index - 1 };
            break;
        } else if index as usize == edge_segments.len() - 1 {
            result = index;
        }
        index += 1;
    }
    result
}

/// Sort cross-hierarchy edge segments from source to target
/// (via a stable sort).
fn sort_segments(a: &LGraphArena, segments: &mut [CrossHierarchyEdge], top: LGraphId) {
    segments.sort_by(|e1, e2| compare_segments(a, e1, e2, top));
}

fn compare_segments(
    a: &LGraphArena,
    edge1: &CrossHierarchyEdge,
    edge2: &CrossHierarchyEdge,
    top: LGraphId,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if edge1.port_type == PortType::OUTPUT && edge2.port_type == PortType::INPUT {
        return Ordering::Less;
    } else if edge1.port_type == PortType::INPUT && edge2.port_type == PortType::OUTPUT {
        return Ordering::Greater;
    }
    let level1 = hierarchy_level(a, edge1.graph, top);
    let level2 = hierarchy_level(a, edge2.graph, top);
    let cmp = if edge1.port_type == PortType::OUTPUT {
        level2 - level1
    } else {
        level1 - level2
    };
    cmp.cmp(&0)
}

fn hierarchy_level(a: &LGraphArena, nested: LGraphId, top: LGraphId) -> i32 {
    let mut current = nested;
    let mut level = 0;
    loop {
        if current == top {
            return level;
        }
        let node = a
            .graph(current)
            .parent_node
            .expect("graph is not an ancestor in the hierarchy");
        current = a.node_graph(node);
        level += 1;
    }
}

// ============================================================================
// Postprocessor
// ============================================================================

const TOLERANCE: f64 = 0.000_01;

pub fn postprocess(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let add_unnecessary_bendpoints: bool =
        a.graph(graph).properties.get(&lopts::UNNECESSARY_BENDPOINTS);

    let cross_hierarchy_map: CrossHierarchyMap =
        a.graph(graph).properties.get(&iprops::CROSS_HIERARCHY_MAP);

    let mut dummy_edges: Vec<LEdgeId> = Vec::new();

    for (&orig_edge, segments) in &cross_hierarchy_map.0 {
        let mut cross_edges = segments.clone();
        sort_segments(a, &mut cross_edges, graph);

        let source_port = cross_edges[0].actual_source(a);
        let target_port = cross_edges[cross_edges.len() - 1].actual_target(a);

        // reference graph for bend points
        let reference_node = a.port(source_port).node.unwrap();
        let target_node = a.port(target_port).node.unwrap();
        let reference_graph = if lgraph_util::is_descendant(a, target_node, reference_node) {
            a.node(reference_node).nested_graph.unwrap()
        } else {
            a.node_graph(reference_node)
        };

        // junction points
        let mut junction_points = clear_junction_points(a, orig_edge, &cross_edges);

        // reset bend points
        a.edge_mut(orig_edge).bend_points.0.clear();

        let mut last_point: Option<KVector> = None;
        for ch_edge in &cross_edges {
            let mut offset = KVector::default();
            lgraph_util::change_coord_system(a, &mut offset, ch_edge.graph, reference_graph);

            let ledge = ch_edge.edge;
            let mut bend_points = a.edge(ledge).bend_points.clone();
            bend_points.offset(offset);

            let src_port = a.edge(ledge).source.unwrap();
            let tgt_port = a.edge(ledge).target.unwrap();
            let mut source_point = port_absolute_anchor(a, src_port);
            let mut target_point = port_absolute_anchor(a, tgt_port);
            source_point.add(offset);
            target_point.add(offset);

            if let Some(lp) = last_point {
                let next_point = if bend_points.is_empty() {
                    target_point
                } else {
                    bend_points.first()
                };
                let x_diff = (lp.x - next_point.x).abs() > TOLERANCE;
                let y_diff = (lp.y - next_point.y).abs() > TOLERANCE;
                if (!add_unnecessary_bendpoints && x_diff && y_diff)
                    || (add_unnecessary_bendpoints && (x_diff || y_diff))
                {
                    a.edge_mut(orig_edge).bend_points.0.push(source_point);
                }
            }

            a.edge_mut(orig_edge)
                .bend_points
                .0
                .extend(bend_points.0.iter().copied());

            last_point = if bend_points.is_empty() {
                Some(source_point)
            } else {
                Some(bend_points.last())
            };

            // junction points
            copy_junction_points(a, ledge, &mut junction_points, offset);

            // target offset
            if ch_edge.actual_target(a) == target_port {
                let mut t_offset = offset;
                let tp_node = a.port(target_port).node.unwrap();
                let tp_graph = a.node_graph(tp_node);
                if tp_graph != ch_edge.graph {
                    t_offset = KVector::default();
                    lgraph_util::change_coord_system(a, &mut t_offset, tp_graph, reference_graph);
                }
                a.edge(orig_edge).properties.set(&iprops::TARGET_OFFSET, t_offset);
            }

            // labels
            copy_labels_back(a, ledge, orig_edge, reference_graph);

            dummy_edges.push(ledge);
        }

        // write back junction points
        if let Some(jps) = junction_points {
            a.edge(orig_edge).properties.set(&lopts::JUNCTION_POINTS, jps);
        }

        // restore the original source/target ports
        a.edge_set_source(orig_edge, Some(source_port));
        a.edge_set_target(orig_edge, Some(target_port));
    }

    // remove dummy edges (dedup since they may appear for several edges)
    let mut seen = std::collections::HashSet::new();
    for dummy_edge in dummy_edges {
        if seen.insert(dummy_edge) {
            a.edge_set_source(dummy_edge, None);
            a.edge_set_target(dummy_edge, None);
        }
    }

    Ok(())
}

/// `LPort.getAbsoluteAnchor`.
fn port_absolute_anchor(a: &LGraphArena, port: LPortId) -> KVector {
    let p = a.port(port);
    let node = p.node.unwrap();
    let n = a.node(node);
    KVector::new(
        n.pos.x + p.pos.x + p.anchor.x,
        n.pos.y + p.pos.y + p.anchor.y,
    )
}

/// Returns the (possibly new, possibly cleared)
/// junction-point chain to be filled and written back by the caller.
fn clear_junction_points(
    a: &mut LGraphArena,
    orig_edge: LEdgeId,
    cross_edges: &[CrossHierarchyEdge],
) -> Option<KVectorChain> {
    let mut junction_points: Option<KVectorChain> =
        a.edge(orig_edge).properties.get_opt(&lopts::JUNCTION_POINTS);
    let any_has_jp = cross_edges.iter().any(|che| {
        a.edge(che.edge)
            .properties
            .get_opt::<KVectorChain>(&lopts::JUNCTION_POINTS)
            .map(|jp| !jp.is_empty())
            .unwrap_or(false)
    });
    if any_has_jp {
        match &mut junction_points {
            None => {
                junction_points = Some(KVectorChain::new());
            }
            Some(jp) => jp.0.clear(),
        }
    } else if junction_points.is_some() {
        a.edge(orig_edge).properties.unset(&lopts::JUNCTION_POINTS);
        junction_points = None;
    }
    junction_points
}

fn copy_junction_points(
    a: &LGraphArena,
    source: LEdgeId,
    target: &mut Option<KVectorChain>,
    offset: KVector,
) {
    if let Some(ledge_jps) = a.edge(source).properties.get_opt::<KVectorChain>(&lopts::JUNCTION_POINTS)
    {
        if let Some(target) = target {
            let mut copies = ledge_jps.clone();
            copies.offset(offset);
            target.0.extend(copies.0);
        }
    }
}

fn copy_labels_back(
    a: &mut LGraphArena,
    hierarchy_segment: LEdgeId,
    orig_edge: LEdgeId,
    reference_graph: LGraphId,
) {
    let labels = a.edge(hierarchy_segment).labels.clone();
    let mut remaining: Vec<LLabelId> = Vec::new();
    for curr_label in labels {
        let belongs = a
            .label(curr_label)
            .properties
            .get_opt::<LEdgeId>(&iprops::ORIGINAL_LABEL_EDGE)
            == Some(orig_edge);
        if !belongs {
            remaining.push(curr_label);
            continue;
        }
        let src_node = a.edge_source_node(hierarchy_segment);
        let src_graph = a.node_graph(src_node);
        let mut pos = a.label(curr_label).pos;
        lgraph_util::change_coord_system(a, &mut pos, src_graph, reference_graph);
        a.label_mut(curr_label).pos = pos;
        a.edge_mut(orig_edge).labels.push(curr_label);
    }
    a.edge_mut(hierarchy_segment).labels = remaining;
}
