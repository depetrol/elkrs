
use std::collections::HashMap;

use crate::alg_common::elkmath;
use crate::alg_common::spore::Node;
use crate::alg_common::triangulation::TEdge;
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::math::{ElkRectangle, KVector};

use crate::alg_spore::graph::Graph;
use crate::alg_spore::options::{self, RootSelection, SpanningTreeCostFunction};

fn kv_bits(v: KVector) -> (u64, u64) {
    (v.x.to_bits(), v.y.to_bits())
}

/// Provides the conversions between `ElkGraph` and [`Graph`].
pub struct ElkGraphImporter {
    /// Associates SPOrE vertices with (vertex index, original ElkNode).
    node_map: HashMap<(u64, u64), (usize, NodeId)>,
    /// The original graph to be resized after the execution phase.
    elk_graph: NodeId,
    /// Spacing between nodes.
    #[allow(dead_code)]
    spacing_node_node: f64,
    /// The cost function selected during import.
    cost_function_id: SpanningTreeCostFunction,
}

impl ElkGraphImporter {
    pub fn import_graph(g: &mut ElkGraph, input_graph: NodeId) -> Result<(Self, Graph), String> {
        // calculate margins
        {
            let mut adapter = crate::core::adapters::ElkGraphAdapter::new(g, input_graph);
            crate::alg_common::nodespacing::calculate_node_margins(&mut adapter, false);
        }

        // retrieve layout options
        let preferred_root_id: Option<String> = g
            .node(input_graph)
            .properties
            .try_get(&options::PROCESSING_ORDER_PREFERRED_ROOT);
        let cost_function_id: SpanningTreeCostFunction = g
            .node(input_graph)
            .properties
            .get(&options::PROCESSING_ORDER_SPANNING_TREE_COST_FUNCTION);
        let tree_construction_strategy = g
            .node(input_graph)
            .properties
            .get(&options::PROCESSING_ORDER_TREE_CONSTRUCTION);
        let compaction_strategy = g
            .node(input_graph)
            .properties
            .get(&options::COMPACTION_COMPACTION_STRATEGY);
        let root_selection: RootSelection = g
            .node(input_graph)
            .properties
            .get(&options::PROCESSING_ORDER_ROOT_SELECTION);
        let spacing_node_node: f64 =
            g.node(input_graph).properties.get(&options::SPACING_NODE_NODE);

        let mut graph = Graph::new(tree_construction_strategy, compaction_strategy);
        graph.orthogonal_compaction =
            g.node(input_graph).properties.get(&options::COMPACTION_ORTHOGONAL);

        let mut importer = ElkGraphImporter {
            node_map: HashMap::new(),
            elk_graph: input_graph,
            spacing_node_node,
            cost_function_id,
        };

        if g.node(input_graph).children.is_empty() {
            // don't bother
            return Ok((importer, graph));
        }

        // create Nodes representing the ElkNodes
        // Perturbs coinciding center points with a fixed-seed java.util.Random.
        let mut random: Option<crate::core::javacompat::JavaRandom> = None;
        let children = g.node(input_graph).children.clone();
        for elk_node in children {
            let shape = &g.node(elk_node).shape;
            let half_width = shape.width / 2.0;
            let half_height = shape.height / 2.0;
            let mut vertex = KVector::new(shape.x + half_width, shape.y + half_height);

            // randomly shift identical points a tiny bit to make them unique
            while importer.node_map.contains_key(&kv_bits(vertex)) {
                let r = random.get_or_insert_with(|| crate::core::javacompat::JavaRandom::new(1));
                vertex.add_xy(
                    (r.next_double() - 0.5) * 0.001,
                    (r.next_double() - 0.5) * 0.001,
                );
            }

            let margin: crate::graph::math::ElkMargin =
                g.node(elk_node).properties.get(&crate::core::options::MARGINS);

            let (width, height) = (g.node(elk_node).shape.width, g.node(elk_node).shape.height);
            let node = Node::new(
                vertex,
                ElkRectangle {
                    x: vertex.x - half_width - spacing_node_node / 2.0 - margin.left,
                    y: vertex.y - half_height - spacing_node_node / 2.0 - margin.top,
                    width: width + spacing_node_node + margin.horizontal(),
                    height: height + spacing_node_node + margin.vertical(),
                },
            );

            let idx = graph.vertices.len();
            graph.vertices.push(node);
            importer.node_map.insert(kv_bits(vertex), (idx, elk_node));
        }

        // spanning tree root selection method
        match root_selection {
            RootSelection::FIXED => {
                match preferred_root_id {
                    None => {
                        // get first Node in list if no ID specified
                        graph.preferred_root = Some(0);
                    }
                    Some(ref preferred) => {
                        for (idx, node) in graph.vertices.iter().enumerate() {
                            let (_, elk_node) =
                                importer.node_map[&kv_bits(node.original_vertex)];
                            if let Some(id) = &g.node(elk_node).identifier {
                                if id == preferred {
                                    graph.preferred_root = Some(idx);
                                }
                            }
                        }
                    }
                }
            }
            RootSelection::CENTER_NODE => {
                // find node that is most central in the drawing
                let shape = &g.node(input_graph).shape;
                let mut center = KVector::new(shape.width, shape.height);
                center.scale(0.5);
                center.add_xy(shape.x, shape.y);
                let mut closest = f64::INFINITY;
                for (idx, node) in graph.vertices.iter().enumerate() {
                    let distance = node.original_vertex.distance(center);
                    if distance < closest {
                        closest = distance;
                        graph.preferred_root = Some(idx);
                    }
                }
            }
        }

        Ok((importer, graph))
    }

    /// The cost function callbacks defined in `ElkGraphImporter`.
    pub fn cost(&self, graph: &Graph, e: &TEdge) -> f64 {
        match self.cost_function_id {
            SpanningTreeCostFunction::CENTER_DISTANCE => e.u.distance(e.v),
            SpanningTreeCostFunction::MINIMUM_ROOT_DISTANCE => {
                let root = &graph.vertices[graph.preferred_root.expect("preferredRoot")];
                f64::min(e.u.distance(root.vertex), e.v.distance(root.vertex))
            }
            SpanningTreeCostFunction::CIRCLE_UNDERLAP => {
                let n1 = &graph.vertices[self.node_map[&kv_bits(e.u)].0];
                let n2 = &graph.vertices[self.node_map[&kv_bits(e.v)].0];
                e.u.distance(e.v)
                    - e.u.distance(n1.rect.position())
                    - e.v.distance(n2.rect.position())
            }
            SpanningTreeCostFunction::RECTANGLE_UNDERLAP => {
                let n1 = &graph.vertices[self.node_map[&kv_bits(e.u)].0];
                let n2 = &graph.vertices[self.node_map[&kv_bits(e.v)].0];
                n1.underlap(n2)
            }
            SpanningTreeCostFunction::INVERTED_OVERLAP => {
                let n1 = &graph.vertices[self.node_map[&kv_bits(e.u)].0];
                let n2 = &graph.vertices[self.node_map[&kv_bits(e.v)].0];
                let r1 = &n1.rect;
                let r2 = &n2.rect;
                let dist = elkmath::shortest_distance(r1, r2);
                if dist >= 0.0 {
                    return dist;
                }
                let mut s = r2.center();
                s.sub(r1.center());
                let s = s.length();
                -(crate::alg_common::utils::overlap(r1, r2) - 1.0) * s
            }
        }
    }

    pub fn update_graph(&mut self, graph: &mut Graph) {
        let mut updated_node_map = HashMap::new();
        // reset graph
        graph.t_edges = None;
        graph.tree = None;

        // update nodes
        for n in graph.vertices.iter_mut() {
            let original = self.node_map[&kv_bits(n.original_vertex)];
            n.original_vertex = n.rect.center();
            updated_node_map.insert(kv_bits(n.original_vertex), original);
        }
        self.node_map = updated_node_map;
    }

    pub fn apply_positions(&self, g: &mut ElkGraph, graph: &Graph) {
        // set new node positions
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for node in &graph.vertices {
            let (_, elk_node) = self.node_map[&kv_bits(node.original_vertex)];
            g.node_mut(elk_node)
                .shape
                .set_location(node.rect.x, node.rect.y);
            let shape = &g.node(elk_node).shape;
            min_x = f64::min(min_x, shape.x);
            min_y = f64::min(min_y, shape.y);
            max_x = f64::max(max_x, shape.x + shape.width);
            max_y = f64::max(max_y, shape.y + shape.height);
        }

        // set new dimensions of parent node
        let padding: crate::graph::math::ElkPadding =
            g.node(self.elk_graph).properties.get(&options::PADDING);
        crate::core::elkutil::resize_node(
            g,
            self.elk_graph,
            max_x - min_x + padding.horizontal(),
            max_y - min_y + padding.vertical(),
            true,
            true,
        );
        // ElkUtil.translate materializes the junction points property on
        // every contained edge (getProperty with a Cloneable default).
        let contained = g.node(self.elk_graph).contained_edges.clone();
        for e in &contained {
            let _ = g
                .edge(*e)
                .properties
                .get(&crate::core::options::JUNCTION_POINTS);
        }
        crate::core::elkutil::translate(
            g,
            self.elk_graph,
            -min_x + padding.left,
            -min_y + padding.top,
        );

        // update edges and route them as straight lines
        for e in contained {
            // ElkGraphUtil.firstEdgeSection(e, true, true): reset the first
            // section (keeping incident shape references) and drop others.
            let section = if let Some(&s) = g.edge(e).sections.first() {
                let sec = g.section_mut(s);
                sec.bend_points.clear();
                sec.set_start_location(0.0, 0.0);
                sec.set_end_location(0.0, 0.0);
                let extra: Vec<_> = g.edge(e).sections[1..].to_vec();
                if !extra.is_empty() {
                    g.edge_mut(e).sections.truncate(1);
                }
                s
            } else {
                g.create_section(e)
            };

            let source = g.shape_node(g.edge(e).sources[0]);
            let target = g.shape_node(g.edge(e).targets[0]);
            let s_shape = &g.node(source).shape;
            let mut start_location =
                KVector::new(s_shape.x + s_shape.width / 2.0, s_shape.y + s_shape.height / 2.0);
            let t_shape = &g.node(target).shape;
            let mut end_location =
                KVector::new(t_shape.x + t_shape.width / 2.0, t_shape.y + t_shape.height / 2.0);

            let mut uv = end_location;
            uv.sub(start_location);
            let (sw, sh) = (g.node(source).shape.width, g.node(source).shape.height);
            elkmath::clip_vector(&mut uv, sw, sh);
            start_location.add(uv);
            let mut vu = start_location;
            vu.sub(end_location);
            let (tw, th) = (g.node(target).shape.width, g.node(target).shape.height);
            elkmath::clip_vector(&mut vu, tw, th);
            end_location.add(vu);

            let sec = g.section_mut(section);
            sec.set_start_location(start_location.x, start_location.y);
            sec.set_end_location(end_location.x, end_location.y);
        }
    }
}
