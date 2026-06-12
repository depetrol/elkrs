//! Port of `org.eclipse.elk.alg.layered.graph.transform.ElkGraphImporter`.
//!
//! Currently supports flat graphs (`SEPARATE_CHILDREN`); hierarchical import
//! (`INCLUDE_CHILDREN`) and external-port handling are ported on demand and
//! fail loudly until then.

use elk_core::options as copts;
use elk_core::options::{
    Direction, EdgeLabelPlacement, HierarchyHandling, PortConstraints, PortSide,
};
use elk_graph::graph::{EdgeId, ElkGraph, NodeId, PortId, ShapeId};
use elk_graph::math::KVector;
use elk_graph::properties::EnumSet;
use indexmap::IndexMap;

use crate::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LPortId};
use crate::internal_properties as iprops;
use crate::internal_properties::Origin;
use crate::lgraph_util;
use crate::options_gen as lopts;
use crate::options_gen::{
    ComponentOrderingStrategy, CrossingMinimizationStrategy, CycleBreakingStrategy,
    GraphProperties, LayeringStrategy, NodePlacementStrategy, NodePromotionStrategy,
    OrderingStrategy, PortType,
};

/// Maps original graph elements to their imported counterparts.
#[derive(Default)]
pub struct ImportMaps {
    pub node_map: IndexMap<NodeId, LNodeId>,
    pub port_map: IndexMap<PortId, LPortId>,
    /// external ports represented by dummy nodes
    pub external_port_dummies: IndexMap<PortId, LNodeId>,
}

pub struct ElkGraphImporter<'g> {
    pub elk: &'g mut ElkGraph,
    pub maps: ImportMaps,
}

impl<'g> ElkGraphImporter<'g> {
    pub fn new(elk: &'g mut ElkGraph) -> Self {
        ElkGraphImporter { elk, maps: ImportMaps::default() }
    }

    /// Port of `importGraph`.
    pub fn import_graph(
        &mut self,
        elkgraph: NodeId,
        a: &mut LGraphArena,
    ) -> Result<LGraphId, String> {
        let top_level_graph = self.create_lgraph(elkgraph, a)?;

        // Assign defined port sides to all external ports
        let ports = self.elk.node(elkgraph).ports.clone();
        for elkport in &ports {
            self.ensure_defined_port_side(a, top_level_graph, *elkport);
        }

        // Transform external ports, if any
        let mut graph_properties: EnumSet<GraphProperties> =
            a.graph(top_level_graph).properties.get(&iprops::GRAPH_PROPERTIES);
        self.check_external_ports(elkgraph, &mut graph_properties);
        a.graph(top_level_graph)
            .properties
            .set(&iprops::GRAPH_PROPERTIES, graph_properties);
        if graph_properties.contains(GraphProperties::EXTERNAL_PORTS) {
            return Err("TODO: external port import is not ported yet".to_string());
        }

        // Calculate the graph's minimum size
        if self.should_calculate_minimum_graph_size(elkgraph) {
            self.calculate_minimum_graph_size(elkgraph, a, top_level_graph)?;
        }

        if a.graph(top_level_graph).properties.get(&lopts::PARTITIONING_ACTIVATE) {
            let mut gp: EnumSet<GraphProperties> =
                a.graph(top_level_graph).properties.get(&iprops::GRAPH_PROPERTIES);
            gp.add(GraphProperties::PARTITIONS);
            a.graph(top_level_graph).properties.set(&iprops::GRAPH_PROPERTIES, gp);
        }

        if a.graph(top_level_graph).properties.has(&lopts::SPACING_BASE_VALUE) {
            return Err("TODO: LayeredSpacings.withBaseValue is not ported yet".to_string());
        }

        if self.elk.node(elkgraph).properties.get(&lopts::HIERARCHY_HANDLING)
            == HierarchyHandling::INCLUDE_CHILDREN
        {
            return Err("TODO: hierarchical (INCLUDE_CHILDREN) import is not ported yet".to_string());
        } else {
            self.import_flat_graph(elkgraph, a, top_level_graph)?;
        }

        Ok(top_level_graph)
    }

    /// Port of `ensureDefinedPortSide`.
    fn ensure_defined_port_side(&mut self, a: &LGraphArena, lgraph: LGraphId, elkport: PortId) {
        let layout_direction: Direction = a.graph(lgraph).properties.get(&lopts::DIRECTION);
        let mut port_side: PortSide = self.elk.port(elkport).properties.get(&copts::PORT_SIDE);
        let port_constraints: PortConstraints =
            a.graph(lgraph).properties.get(&lopts::PORT_CONSTRAINTS);

        if !port_constraints.is_side_fixed() {
            let net_flow = self.calculate_net_flow(elkport);
            if net_flow > 0 {
                port_side = PortSide::from_direction(layout_direction);
            } else {
                port_side = PortSide::from_direction(layout_direction).opposed();
            }
        } else if port_side == PortSide::UNDEFINED {
            port_side = elk_core::elkutil::calc_port_side(self.elk, elkport, layout_direction);
            if port_side == PortSide::UNDEFINED {
                port_side = PortSide::from_direction(layout_direction);
            }
        }

        self.elk.port_mut(elkport).properties.set(&copts::PORT_SIDE, port_side);
    }

    /// Port of `shouldCalculateMinimumGraphSize`.
    fn should_calculate_minimum_graph_size(&self, elkgraph: NodeId) -> bool {
        !self
            .elk
            .node(elkgraph)
            .properties
            .get(&lopts::NODE_SIZE_CONSTRAINTS)
            .is_empty()
    }

    /// Port of `calculateMinimumGraphSize`.
    fn calculate_minimum_graph_size(
        &mut self,
        elkgraph: NodeId,
        _a: &mut LGraphArena,
        _lgraph: LGraphId,
    ) -> Result<(), String> {
        if self.elk.node(elkgraph).parent.is_none() {
            return Ok(());
        }
        Err("TODO: NodeLabelAndSizeCalculator minimum graph size is not ported yet".to_string())
    }

    /// Port of `importFlatGraph`.
    fn import_flat_graph(
        &mut self,
        elkgraph: NodeId,
        a: &mut LGraphArena,
        lgraph: LGraphId,
    ) -> Result<(), String> {
        let mut index = 0i32;
        let mut cb_group_model_orders: std::collections::HashSet<i32> =
            std::collections::HashSet::new();
        let children = self.elk.node(elkgraph).children.clone();
        for child in children {
            if !self.elk.node(child).properties.get(&copts::NO_LAYOUT) {
                if self.needs_model_order(child) {
                    self.elk
                        .node_mut(child)
                        .properties
                        .set(&iprops::MODEL_ORDER, index);
                    index += 1;
                    if self
                        .elk
                        .node(child)
                        .properties
                        .has(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CYCLE_BREAKING_ID)
                    {
                        cb_group_model_orders.insert(
                            self.elk.node(child).properties.get(
                                &lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CYCLE_BREAKING_ID,
                            ),
                        );
                    }
                }
                self.transform_node(child, a, lgraph)?;
            }
        }
        a.graph(lgraph).properties.set(&iprops::MAX_MODEL_ORDER_NODES, index);
        a.graph(lgraph)
            .properties
            .set(&iprops::CB_NUM_MODEL_ORDER_GROUPS, cb_group_model_orders.len() as i32);

        // edges in input order
        let mut index = 0i32;
        let contained_edges = self.elk.node(elkgraph).contained_edges.clone();
        for elkedge in contained_edges {
            if self.needs_model_order_based_on_parent(elkgraph) {
                self.elk
                    .edge_mut(elkedge)
                    .properties
                    .set(&iprops::MODEL_ORDER, index);
                index += 1;
            }
            let source = self.elk.shape_node(self.elk.edge(elkedge).sources[0]);
            let target = self.elk.shape_node(self.elk.edge(elkedge).targets[0]);

            let enable_inside_self_loops =
                self.elk.node(source).properties.get(&copts::INSIDE_SELF_LOOPS_ACTIVATE);
            let is_to_be_laid_out = !self.elk.edge(elkedge).properties.get(&copts::NO_LAYOUT);
            let is_inside_self_loop = enable_inside_self_loops
                && is_elk_self_loop(self.elk, elkedge)
                && self.elk.edge(elkedge).properties.get(&copts::INSIDE_SELF_LOOPS_YO);
            let connects_siblings = self.elk.node(source).parent == Some(elkgraph)
                && self.elk.node(source).parent == self.elk.node(target).parent;
            let connects_to_graph = (self.elk.node(source).parent == Some(elkgraph)
                && target == elkgraph)
                ^ (self.elk.node(target).parent == Some(elkgraph) && source == elkgraph);

            if is_to_be_laid_out && !is_inside_self_loop && (connects_to_graph || connects_siblings)
            {
                self.transform_edge(elkedge, elkgraph, a, lgraph)?;
            }
        }

        // collect inside self loops of `elkgraph` itself
        if let Some(parent) = self.elk.node(elkgraph).parent {
            let parent_edges = self.elk.node(parent).contained_edges.clone();
            for elkedge in parent_edges {
                let source = self.elk.shape_node(self.elk.edge(elkedge).sources[0]);
                if source == elkgraph && is_elk_self_loop(self.elk, elkedge) {
                    let is_inside_self_loop = self
                        .elk
                        .node(source)
                        .properties
                        .get(&copts::INSIDE_SELF_LOOPS_ACTIVATE)
                        && self.elk.edge(elkedge).properties.get(&copts::INSIDE_SELF_LOOPS_YO);
                    if is_inside_self_loop {
                        self.transform_edge(elkedge, elkgraph, a, lgraph)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Port of `needsModelOrder`.
    fn needs_model_order(&self, child: NodeId) -> bool {
        match self.elk.node(child).parent {
            None => false,
            Some(elkgraph) => {
                self.needs_model_order_based_on_parent(elkgraph)
                    && !self
                        .elk
                        .node(child)
                        .properties
                        .get(&lopts::CONSIDER_MODEL_ORDER_NO_MODEL_ORDER)
            }
        }
    }

    /// Port of `needsModelOrderBasedOnParent`.
    fn needs_model_order_based_on_parent(&self, elkgraph: NodeId) -> bool {
        let props = &self.elk.node(elkgraph).properties;
        let cbs: CycleBreakingStrategy = props.get(&lopts::CYCLE_BREAKING_STRATEGY);
        let model_order_cycle_breaking = matches!(
            cbs,
            CycleBreakingStrategy::MODEL_ORDER
                | CycleBreakingStrategy::BFS_NODE_ORDER
                | CycleBreakingStrategy::DFS_NODE_ORDER
                | CycleBreakingStrategy::GREEDY_MODEL_ORDER
                | CycleBreakingStrategy::SCC_CONNECTIVITY
                | CycleBreakingStrategy::SCC_NODE_TYPE
        );
        let ls: LayeringStrategy = props.get(&lopts::LAYERING_STRATEGY);
        let nps: NodePromotionStrategy = props.get(&lopts::LAYERING_NODE_PROMOTION_STRATEGY);
        let model_order_layering = matches!(
            ls,
            LayeringStrategy::BF_MODEL_ORDER | LayeringStrategy::DF_MODEL_ORDER
        ) || matches!(
            nps,
            NodePromotionStrategy::MODEL_ORDER_LEFT_TO_RIGHT
                | NodePromotionStrategy::MODEL_ORDER_RIGHT_TO_LEFT
        );
        let model_order_crossing_minimization =
            props.get::<OrderingStrategy>(&lopts::CONSIDER_MODEL_ORDER_STRATEGY)
                != OrderingStrategy::NONE
                || props.get(&lopts::CROSSING_MINIMIZATION_FORCE_NODE_MODEL_ORDER)
                || props.get::<ComponentOrderingStrategy>(&lopts::CONSIDER_MODEL_ORDER_COMPONENTS)
                    != ComponentOrderingStrategy::NONE
                || props.get::<f64>(&lopts::CONSIDER_MODEL_ORDER_CROSSING_COUNTER_NODE_INFLUENCE)
                    != 0.0
                || props.get::<f64>(&lopts::CONSIDER_MODEL_ORDER_CROSSING_COUNTER_PORT_INFLUENCE)
                    != 0.0;
        model_order_cycle_breaking || model_order_layering || model_order_crossing_minimization
    }

    /// Port of `createLGraph`.
    fn create_lgraph(&mut self, elkgraph: NodeId, a: &mut LGraphArena) -> Result<LGraphId, String> {
        let lgraph = a.create_graph();

        // Copy the properties of the ElkGraph to the layered graph
        a.graph_mut(lgraph)
            .properties
            .copy_from(&self.elk.node(elkgraph).properties);
        if a.graph(lgraph).properties.get::<Direction>(&lopts::DIRECTION) == Direction::UNDEFINED {
            let dir = lgraph_util::get_direction(a, lgraph);
            a.graph(lgraph).properties.set(&lopts::DIRECTION, dir);
        }

        // (Label management not supported: LABEL_MANAGER is not ported.)

        // Initialize the graph properties discovered during the transformations
        a.graph(lgraph)
            .properties
            .set(&iprops::GRAPH_PROPERTIES, EnumSet::<GraphProperties>::none());

        // Adjust the padding to respect inside node labels
        let node_label_padding = self.compute_inside_node_label_padding(elkgraph)?;
        let node_padding: elk_graph::math::Spacing =
            a.graph(lgraph).properties.get(&lopts::PADDING);

        let p = &mut a.graph_mut(lgraph).padding;
        p.top += node_padding.top + node_label_padding.top;
        p.bottom += node_padding.bottom + node_label_padding.bottom;
        p.left += node_padding.left + node_label_padding.left;
        p.right += node_padding.right + node_label_padding.right;

        Ok(lgraph)
    }

    /// Minimal port of `NodeLabelAndSizeCalculator.computeInsideNodeLabelPadding`:
    /// no inside node labels means zero padding. Full support arrives with
    /// the alg.common port.
    fn compute_inside_node_label_padding(
        &self,
        elkgraph: NodeId,
    ) -> Result<elk_graph::math::Spacing, String> {
        if self.elk.node(elkgraph).labels.is_empty() {
            Ok(elk_graph::math::Spacing::default())
        } else {
            Err("TODO: inside node label padding is not ported yet (graph has labels)".to_string())
        }
    }

    /// Port of `checkExternalPorts`.
    fn check_external_ports(
        &self,
        elkgraph: NodeId,
        graph_properties: &mut EnumSet<GraphProperties>,
    ) {
        let enable_self_loops = self
            .elk
            .node(elkgraph)
            .properties
            .get(&copts::INSIDE_SELF_LOOPS_ACTIVATE);
        let port_label_placement = self
            .elk
            .node(elkgraph)
            .properties
            .get(&copts::PORT_LABELS_PLACEMENT);

        let mut has_external_ports = false;
        let mut has_hyperedges = false;

        for &elkport in &self.elk.node(elkgraph).ports {
            if has_external_ports && has_hyperedges {
                break;
            }
            let mut external_port_edges = 0;
            let incident: Vec<EdgeId> = self
                .elk
                .port(elkport)
                .outgoing_edges
                .iter()
                .chain(self.elk.port(elkport).incoming_edges.iter())
                .copied()
                .collect();
            for elkedge in incident {
                let is_inside_self_loop = enable_self_loops
                    && is_elk_self_loop(self.elk, elkedge)
                    && self.elk.edge(elkedge).properties.get(&copts::INSIDE_SELF_LOOPS_YO);
                let connects_to_child = if self
                    .elk
                    .edge(elkedge)
                    .sources
                    .contains(&ShapeId::Port(elkport))
                {
                    self.elk
                        .node(self.elk.shape_node(self.elk.edge(elkedge).targets[0]))
                        .parent
                        == Some(elkgraph)
                } else {
                    self.elk
                        .node(self.elk.shape_node(self.elk.edge(elkedge).sources[0]))
                        .parent
                        == Some(elkgraph)
                };
                if is_inside_self_loop || connects_to_child {
                    external_port_edges += 1;
                    if external_port_edges > 1 {
                        break;
                    }
                }
            }

            if external_port_edges > 0 {
                has_external_ports = true;
            } else if port_label_placement.contains(copts::PortLabelPlacement::INSIDE)
                && !self.elk.port(elkport).labels.is_empty()
            {
                has_external_ports = true;
            }
            if external_port_edges > 1 {
                has_hyperedges = true;
            }
        }

        if has_external_ports {
            graph_properties.add(GraphProperties::EXTERNAL_PORTS);
        }
        if has_hyperedges {
            graph_properties.add(GraphProperties::HYPEREDGES);
        }
    }

    /// Port of `calculateNetFlow`.
    fn calculate_net_flow(&self, elkport: PortId) -> i32 {
        let elkgraph = self.elk.port(elkport).parent.unwrap();
        let inside_self_loops_enabled = self
            .elk
            .node(elkgraph)
            .properties
            .get(&copts::INSIDE_SELF_LOOPS_ACTIVATE);

        let mut output_port_vote = 0;
        let mut input_port_vote = 0;

        for &outgoing in &self.elk.port(elkport).outgoing_edges {
            let is_self_loop = is_elk_self_loop(self.elk, outgoing);
            let is_inside_self_loop = is_self_loop
                && inside_self_loops_enabled
                && self.elk.edge(outgoing).properties.get(&copts::INSIDE_SELF_LOOPS_YO);
            let target_node = self.elk.shape_node(self.elk.edge(outgoing).targets[0]);
            if is_self_loop && is_inside_self_loop {
                input_port_vote += 1;
            } else if is_self_loop {
                output_port_vote += 1;
            } else if self.elk.node(target_node).parent == Some(elkgraph)
                || target_node == elkgraph
            {
                input_port_vote += 1;
            } else {
                output_port_vote += 1;
            }
        }
        for &incoming in &self.elk.port(elkport).incoming_edges {
            let is_self_loop = is_elk_self_loop(self.elk, incoming);
            let is_inside_self_loop = is_self_loop
                && inside_self_loops_enabled
                && self.elk.edge(incoming).properties.get(&copts::INSIDE_SELF_LOOPS_YO);
            let source_node = self.elk.shape_node(self.elk.edge(incoming).sources[0]);
            if is_self_loop && is_inside_self_loop {
                output_port_vote += 1;
            } else if is_self_loop {
                input_port_vote += 1;
            } else if self.elk.node(source_node).parent == Some(elkgraph)
                || source_node == elkgraph
            {
                output_port_vote += 1;
            } else {
                input_port_vote += 1;
            }
        }

        output_port_vote - input_port_vote
    }

    /// Port of `transformNode`.
    fn transform_node(
        &mut self,
        elknode: NodeId,
        a: &mut LGraphArena,
        lgraph: LGraphId,
    ) -> Result<LNodeId, String> {
        let lnode = a.create_node(lgraph);
        a.node_mut(lnode)
            .properties
            .copy_from(&self.elk.node(elknode).properties);
        a.node(lnode).properties.set(&iprops::ORIGIN, Origin::Node(elknode));

        {
            let shape = &self.elk.node(elknode).shape;
            let n = a.node_mut(lnode);
            n.size.x = shape.width;
            n.size.y = shape.height;
            n.pos.x = shape.x;
            n.pos.y = shape.y;
        }

        a.graph_mut(lgraph).layerless_nodes.push(lnode);
        self.maps.node_map.insert(elknode, lnode);

        if !self.elk.node(elknode).children.is_empty()
            || self
                .elk
                .node(elknode)
                .properties
                .get(&copts::INSIDE_SELF_LOOPS_ACTIVATE)
        {
            a.node(lnode).properties.set(&iprops::COMPOUND_NODE, true);
        }

        let mut graph_properties: EnumSet<GraphProperties> =
            a.graph(lgraph).properties.get(&iprops::GRAPH_PROPERTIES);

        // port constraints and sides cannot be undefined
        let port_constraints: PortConstraints =
            a.node(lnode).properties.get(&lopts::PORT_CONSTRAINTS);
        if port_constraints == PortConstraints::UNDEFINED {
            a.node(lnode)
                .properties
                .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FREE);
        } else if port_constraints != PortConstraints::FREE {
            graph_properties.add(GraphProperties::NON_FREE_PORTS);
        }

        let mut port_model_order = 0i32;
        let direction: Direction = a.graph(lgraph).properties.get(&lopts::DIRECTION);
        let ports = self.elk.node(elknode).ports.clone();
        for elkport in ports {
            if self.needs_model_order(elknode) {
                self.elk
                    .port_mut(elkport)
                    .properties
                    .set(&iprops::MODEL_ORDER, port_model_order);
                port_model_order += 1;
            }
            if !self.elk.port(elkport).properties.get(&copts::NO_LAYOUT) {
                self.transform_port(
                    elkport,
                    a,
                    lnode,
                    &mut graph_properties,
                    direction,
                    port_constraints,
                )?;
            }
        }

        // node labels
        let labels = self.elk.node(elknode).labels.clone();
        for elklabel in labels {
            if !self.elk.label(elklabel).properties.get(&copts::NO_LAYOUT)
                && !self.elk.label(elklabel).text.is_empty()
            {
                let llabel = self.transform_label(elklabel, a);
                a.node_mut(lnode).labels.push(llabel);
            }
        }

        if a.node(lnode).properties.get(&lopts::COMMENT_BOX) {
            graph_properties.add(GraphProperties::COMMENTS);
        }

        if a.node(lnode).properties.get(&lopts::HYPERNODE) {
            graph_properties.add(GraphProperties::HYPERNODES);
            graph_properties.add(GraphProperties::HYPEREDGES);
            a.node(lnode)
                .properties
                .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FREE);
        }

        a.graph(lgraph)
            .properties
            .set(&iprops::GRAPH_PROPERTIES, graph_properties);

        Ok(lnode)
    }

    /// Port of `transformPort`.
    #[allow(clippy::too_many_arguments)]
    fn transform_port(
        &mut self,
        elkport: PortId,
        a: &mut LGraphArena,
        parent_lnode: LNodeId,
        graph_properties: &mut EnumSet<GraphProperties>,
        layout_direction: Direction,
        port_constraints: PortConstraints,
    ) -> Result<LPortId, String> {
        let lport = a.create_port();
        a.port_mut(lport)
            .properties
            .copy_from(&self.elk.port(elkport).properties);
        let side = self.elk.port(elkport).properties.get(&copts::PORT_SIDE);
        a.port_set_side(lport, side);
        a.port(lport).properties.set(&iprops::ORIGIN, Origin::Port(elkport));
        a.port_set_node(lport, Some(parent_lnode));

        {
            let shape = &self.elk.port(elkport).shape;
            let p = a.port_mut(lport);
            p.size.x = shape.width;
            p.size.y = shape.height;
            p.pos.x = shape.x;
            p.pos.y = shape.y;
        }

        self.maps.port_map.insert(elkport, lport);

        // connections to descendants?
        let parent_node = self.elk.port(elkport).parent.unwrap();
        let mut connections_to_descendants = self
            .elk
            .port(elkport)
            .outgoing_edges
            .iter()
            .flat_map(|&e| self.elk.edge(e).targets.iter())
            .any(|&t| self.elk.is_descendant(self.elk.shape_node(t), parent_node));
        if !connections_to_descendants {
            connections_to_descendants = self
                .elk
                .port(elkport)
                .incoming_edges
                .iter()
                .flat_map(|&e| self.elk.edge(e).sources.iter())
                .any(|&s| self.elk.is_descendant(self.elk.shape_node(s), parent_node));
        }
        if !connections_to_descendants {
            connections_to_descendants = self.elk.port(elkport).outgoing_edges.iter().any(|&e| {
                is_elk_self_loop(self.elk, e)
                    && self.elk.edge(e).properties.get(&copts::INSIDE_SELF_LOOPS_YO)
            });
        }
        a.port(lport)
            .properties
            .set(&iprops::INSIDE_CONNECTIONS, connections_to_descendants);

        // initialize the port's side, offset and anchor
        let anchor = self.elk.port(elkport).properties.try_get(&copts::PORT_ANCHOR);
        lgraph_util::initialize_port(a, lport, port_constraints, layout_direction, anchor);

        // port labels
        let labels = self.elk.port(elkport).labels.clone();
        for elklabel in labels {
            if !self.elk.label(elklabel).properties.get(&copts::NO_LAYOUT)
                && !self.elk.label(elklabel).text.is_empty()
            {
                let llabel = self.transform_label(elklabel, a);
                a.port_mut(lport).labels.push(llabel);
            }
        }

        match layout_direction {
            Direction::LEFT | Direction::RIGHT => {
                if matches!(a.port(lport).side, PortSide::NORTH | PortSide::SOUTH) {
                    graph_properties.add(GraphProperties::NORTH_SOUTH_PORTS);
                }
            }
            Direction::UP | Direction::DOWN => {
                if matches!(a.port(lport).side, PortSide::EAST | PortSide::WEST) {
                    graph_properties.add(GraphProperties::NORTH_SOUTH_PORTS);
                }
            }
            Direction::UNDEFINED => {}
        }

        Ok(lport)
    }

    /// Port of `transformEdge`.
    fn transform_edge(
        &mut self,
        elkedge: EdgeId,
        _elkparent: NodeId,
        a: &mut LGraphArena,
        lgraph: LGraphId,
    ) -> Result<LEdgeId, String> {
        check_edge_validity(self.elk, elkedge)?;

        let elk_source_shape = self.elk.edge(elkedge).sources[0];
        let elk_target_shape = self.elk.edge(elkedge).targets[0];
        let elk_source_node = self.elk.shape_node(elk_source_shape);
        let elk_target_node = self.elk.shape_node(elk_target_shape);

        let edge_section = self.elk.edge(elkedge).sections.first().copied();

        let mut source_lnode = self.maps.node_map.get(&elk_source_node).copied();
        let mut target_lnode = self.maps.node_map.get(&elk_target_node).copied();
        let mut source_lport: Option<LPortId> = None;
        let mut target_lport: Option<LPortId> = None;

        if let ShapeId::Port(p) = elk_source_shape {
            if let Some(&lp) = self.maps.port_map.get(&p) {
                source_lport = Some(lp);
            } else if let Some(&ln) = self.maps.external_port_dummies.get(&p) {
                source_lnode = Some(ln);
                source_lport = Some(a.node(ln).ports[0]);
            }
        }
        if let ShapeId::Port(p) = elk_target_shape {
            if let Some(&lp) = self.maps.port_map.get(&p) {
                target_lport = Some(lp);
            } else if let Some(&ln) = self.maps.external_port_dummies.get(&p) {
                target_lnode = Some(ln);
                target_lport = Some(a.node(ln).ports[0]);
            }
        }

        let source_lnode = source_lnode.ok_or_else(|| edge_endpoint_error())?;
        let target_lnode = target_lnode.ok_or_else(|| edge_endpoint_error())?;

        let ledge = a.create_edge();
        a.edge_mut(ledge)
            .properties
            .copy_from(&self.elk.edge(elkedge).properties);
        a.edge(ledge).properties.set(&iprops::ORIGIN, Origin::Edge(elkedge));

        // Clear junction points, since they are recomputed from scratch
        a.edge(ledge).properties.unset(&lopts::JUNCTION_POINTS);

        let mut graph_properties: EnumSet<GraphProperties> =
            a.graph(lgraph).properties.get(&iprops::GRAPH_PROPERTIES);
        if source_lnode == target_lnode {
            graph_properties.add(GraphProperties::SELF_LOOPS);
        }

        // create source and target ports if they do not exist yet
        let source_lport = match source_lport {
            Some(p) => p,
            None => {
                let mut port_type = PortType::OUTPUT;
                let mut source_point = None;
                if let Some(section) = edge_section {
                    if a.node(source_lnode)
                        .properties
                        .get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
                        .is_side_fixed()
                    {
                        let s = self.elk.section(section);
                        let mut sp = KVector::new(s.start_x, s.start_y);
                        // coordinates relative to elkparent
                        to_absolute(self.elk, &mut sp, self.elk.edge(elkedge).containing_node);
                        to_relative(self.elk, &mut sp, Some(_elkparent));
                        if self.elk.is_descendant(elk_target_node, elk_source_node) {
                            port_type = PortType::INPUT;
                            let node_pos = a.node(source_lnode).pos;
                            sp.add(node_pos);
                        }
                        source_point = Some(sp);
                    }
                }
                lgraph_util::create_port(a, source_lnode, source_point, port_type, lgraph)
            }
        };

        let target_lport = match target_lport {
            Some(p) => p,
            None => {
                let port_type = PortType::INPUT;
                let mut target_point = None;
                if let Some(section) = edge_section {
                    if a.node(target_lnode)
                        .properties
                        .get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
                        .is_side_fixed()
                    {
                        let s = self.elk.section(section);
                        let mut tp = KVector::new(s.end_x, s.end_y);
                        to_absolute(self.elk, &mut tp, self.elk.edge(elkedge).containing_node);
                        to_relative(self.elk, &mut tp, Some(_elkparent));
                        target_point = Some(tp);
                    }
                }
                // Java: targetLNode.getGraph() — the graph the target node is in
                let target_graph = a.node_graph(target_lnode);
                lgraph_util::create_port(a, target_lnode, target_point, port_type, target_graph)
            }
        };

        a.edge_set_source(ledge, Some(source_lport));
        a.edge_set_target(ledge, Some(target_lport));

        if a.port(source_lport).incoming_edges.len() > 1
            || a.port(source_lport).outgoing_edges.len() > 1
            || a.port(target_lport).incoming_edges.len() > 1
            || a.port(target_lport).outgoing_edges.len() > 1
        {
            graph_properties.add(GraphProperties::HYPEREDGES);
        }

        // edge labels
        let labels = self.elk.edge(elkedge).labels.clone();
        for elklabel in labels {
            if !self.elk.label(elklabel).properties.get(&copts::NO_LAYOUT)
                && !self.elk.label(elklabel).text.is_empty()
            {
                let llabel = self.transform_label(elklabel, a);
                a.edge_mut(ledge).labels.push(llabel);

                match a
                    .label(llabel)
                    .properties
                    .get::<EdgeLabelPlacement>(&lopts::EDGE_LABELS_PLACEMENT)
                {
                    EdgeLabelPlacement::HEAD | EdgeLabelPlacement::TAIL => {
                        graph_properties.add(GraphProperties::END_LABELS);
                    }
                    EdgeLabelPlacement::CENTER => {
                        graph_properties.add(GraphProperties::CENTER_LABELS);
                        a.label(llabel)
                            .properties
                            .set(&lopts::EDGE_LABELS_PLACEMENT, EdgeLabelPlacement::CENTER);
                    }
                }
            }
        }

        a.graph(lgraph)
            .properties
            .set(&iprops::GRAPH_PROPERTIES, graph_properties);

        // copy original bend points if required
        let cross_min: CrossingMinimizationStrategy =
            a.graph(lgraph).properties.get(&lopts::CROSSING_MINIMIZATION_STRATEGY);
        let node_place: NodePlacementStrategy =
            a.graph(lgraph).properties.get(&lopts::NODE_PLACEMENT_STRATEGY);
        let bend_points_required = cross_min == CrossingMinimizationStrategy::INTERACTIVE
            || node_place == NodePlacementStrategy::INTERACTIVE;

        if let Some(section) = edge_section {
            if !self.elk.section(section).bend_points.is_empty() && bend_points_required {
                let imported = self.elk.section_chain(section);
                a.edge(ledge)
                    .properties
                    .set(&iprops::ORIGINAL_BENDPOINTS, imported);
            }
        }

        Ok(ledge)
    }

    /// Port of `transformLabel`.
    fn transform_label(&mut self, elklabel: elk_graph::graph::LabelId, a: &mut LGraphArena) -> crate::graph::LLabelId {
        let text = self.elk.label(elklabel).text.clone();
        let llabel = a.create_label(&text);
        a.label_mut(llabel)
            .properties
            .copy_from(&self.elk.label(elklabel).properties);
        a.label(llabel).properties.set(&iprops::ORIGIN, Origin::Label(elklabel));
        {
            let shape = &self.elk.label(elklabel).shape;
            let l = a.label_mut(llabel);
            l.size.x = shape.width;
            l.size.y = shape.height;
            l.pos.x = shape.x;
            l.pos.y = shape.y;
        }
        llabel
    }
}

fn edge_endpoint_error() -> String {
    "The source or the target of an edge could not be found. This usually happens when an edge \
     connects a node laid out by ELK Layered to a node in another level of hierarchy laid out by \
     either another instance of ELK Layered or another layout algorithm alltogether. The former \
     can be solved by setting the hierarchyHandling option to INCLUDE_CHILDREN."
        .to_string()
}

/// Java `ElkEdge.isSelfloop` on the original graph.
pub fn is_elk_self_loop(elk: &ElkGraph, edge: EdgeId) -> bool {
    let e = elk.edge(edge);
    let mut nodes = e
        .sources
        .iter()
        .chain(e.targets.iter())
        .map(|&s| elk.shape_node(s));
    match nodes.next() {
        None => false,
        Some(first) => nodes.all(|n| n == first),
    }
}

/// Java `ElkEdge.isHyperedge`.
pub fn is_elk_hyperedge(elk: &ElkGraph, edge: EdgeId) -> bool {
    let e = elk.edge(edge);
    e.sources.len() + e.targets.len() > 2
}

/// Port of `checkEdgeValidity`.
fn check_edge_validity(elk: &ElkGraph, edge: EdgeId) -> Result<(), String> {
    if elk.edge(edge).sources.is_empty() {
        Err("Edges must have a source.".to_string())
    } else if elk.edge(edge).targets.is_empty() {
        Err("Edges must have a target.".to_string())
    } else if is_elk_hyperedge(elk, edge) {
        Err("Hyperedges are not supported.".to_string())
    } else {
        Ok(())
    }
}

/// Port of `ElkUtil.toAbsolute` (point relative to `parent` -> absolute).
pub fn to_absolute(elk: &ElkGraph, point: &mut KVector, parent: Option<NodeId>) {
    let mut current = parent;
    while let Some(node) = current {
        point.x += elk.node(node).shape.x;
        point.y += elk.node(node).shape.y;
        current = elk.node(node).parent;
    }
}

/// Port of `ElkUtil.toRelative` (absolute point -> relative to `parent`).
pub fn to_relative(elk: &ElkGraph, point: &mut KVector, parent: Option<NodeId>) {
    let mut current = parent;
    while let Some(node) = current {
        point.x -= elk.node(node).shape.x;
        point.y -= elk.node(node).shape.y;
        current = elk.node(node).parent;
    }
}
