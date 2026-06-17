//!
//! Counts the crossings caused by the order of north south port dummies when
//! their respective normal node in the same layer has a fixed port order.
//! Also counts crossings between north south edges and long edge dummies.

use std::collections::HashMap;

use crate::core::options::PortSide;

use crate::alg_layered::graph::{LGraphArena, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;

use super::super::counting::in_north_south_east_west_order;

pub struct NorthSouthEdgeNeighbouringNodeCrossingsCounter {
    upper_lower_crossings: i32,
    lower_upper_crossings: i32,
    port_positions: HashMap<LPortId, i32>,
}

impl NorthSouthEdgeNeighbouringNodeCrossingsCounter {
    /// Creates a counter for north south port crossings for the given layer.
    pub fn new(a: &LGraphArena, nodes: &[LNodeId]) -> Self {
        let mut counter = NorthSouthEdgeNeighbouringNodeCrossingsCounter {
            upper_lower_crossings: 0,
            lower_upper_crossings: 0,
            port_positions: HashMap::new(),
        };
        // initializePortPositions: ports are numbered as they are in the
        // list returned by getPorts().
        for &node in nodes {
            counter.set_port_ids_on(a, node, PortSide::SOUTH);
            counter.set_port_ids_on(a, node, PortSide::NORTH);
        }
        counter
    }

    fn set_port_ids_on(&mut self, a: &LGraphArena, node: LNodeId, side: PortSide) {
        let mut port_id = 0;
        for port in in_north_south_east_west_order(a, node, side) {
            self.port_positions.insert(port, port_id);
            port_id += 1;
        }
    }

    /// Counts north south port crossings and crossings between north south
    /// ports and dummy nodes, for upperNode and lowerNode.
    pub fn count_crossings(&mut self, a: &LGraphArena, upper_node: LNodeId, lower_node: LNodeId) {
        self.upper_lower_crossings = 0;
        self.lower_upper_crossings = 0;

        self.process_if_two_north_south_nodes(a, upper_node, lower_node);

        self.process_if_north_south_long_edge_dummy_crossing(a, upper_node, lower_node);

        self.process_if_normal_node_with_ns_ports_and_long_edge_dummy(a, upper_node, lower_node);
    }

    fn process_if_two_north_south_nodes(
        &mut self,
        a: &LGraphArena,
        upper_node: LNodeId,
        lower_node: LNodeId,
    ) {
        if is_north_south(a, upper_node)
            && is_north_south(a, lower_node)
            && !have_different_origins(a, upper_node, lower_node)
        {
            if is_north_of_normal_node(a, upper_node) {
                self.count_crossings_of_two_north_south_dummies(a, upper_node, lower_node);
            } else {
                self.count_crossings_of_two_north_south_dummies(a, lower_node, upper_node);
            }
        }
    }

    fn count_crossings_of_two_north_south_dummies(
        &mut self,
        a: &LGraphArena,
        further_from_normal_node: LNodeId,
        closer_to_normal_node: LNodeId,
    ) {
        if self.origin_port_position_of(a, further_from_normal_node)
            > self.origin_port_position_of(a, closer_to_normal_node)
        {
            let closer_east_ports = a.node_port_side_view(closer_to_normal_node, PortSide::EAST);
            self.upper_lower_crossings = match closer_east_ports.first() {
                None => 0,
                Some(&p) => a.port_degree(p) as i32,
            };
            let further_west_ports =
                a.node_port_side_view(further_from_normal_node, PortSide::WEST);
            self.lower_upper_crossings = match further_west_ports.first() {
                None => 0,
                Some(&p) => a.port_degree(p) as i32,
            };
        } else {
            let closer_west_ports = a.node_port_side_view(closer_to_normal_node, PortSide::WEST);
            self.upper_lower_crossings = match closer_west_ports.first() {
                None => 0,
                Some(&p) => a.port_degree(p) as i32,
            };
            let further_east_ports =
                a.node_port_side_view(further_from_normal_node, PortSide::EAST);
            self.lower_upper_crossings = match further_east_ports.first() {
                None => 0,
                Some(&p) => a.port_degree(p) as i32,
            };
        }
    }

    fn process_if_north_south_long_edge_dummy_crossing(
        &mut self,
        a: &LGraphArena,
        upper_node: LNodeId,
        lower_node: LNodeId,
    ) {
        if is_north_south(a, upper_node) && is_long_edge_dummy(a, lower_node) {
            if is_north_of_normal_node(a, upper_node) {
                self.upper_lower_crossings = 1;
            } else {
                self.lower_upper_crossings = 1;
            }
        } else if is_north_south(a, lower_node) && is_long_edge_dummy(a, upper_node) {
            if is_north_of_normal_node(a, lower_node) {
                self.lower_upper_crossings = 1;
            } else {
                self.upper_lower_crossings = 1;
            }
        }
    }

    fn process_if_normal_node_with_ns_ports_and_long_edge_dummy(
        &mut self,
        a: &LGraphArena,
        upper_node: LNodeId,
        lower_node: LNodeId,
    ) {
        if is_normal(a, upper_node) && is_long_edge_dummy(a, lower_node) {
            self.upper_lower_crossings = number_of_north_south_edges(a, upper_node, PortSide::SOUTH);
            self.lower_upper_crossings = number_of_north_south_edges(a, upper_node, PortSide::NORTH);
        }
        if is_normal(a, lower_node) && is_long_edge_dummy(a, upper_node) {
            self.upper_lower_crossings = number_of_north_south_edges(a, lower_node, PortSide::NORTH);
            self.lower_upper_crossings = number_of_north_south_edges(a, lower_node, PortSide::SOUTH);
        }
    }

    fn origin_port_position_of(&self, a: &LGraphArena, node: LNodeId) -> i32 {
        let origin = origin_port_of(a, node);
        *self
            .port_positions
            .get(&origin)
            .expect("origin port must have a position")
    }

    /// Get crossing count for the order upper - lower.
    pub fn get_upper_lower_crossings(&self) -> i32 {
        self.upper_lower_crossings
    }

    /// Get crossing count for the order lower - upper.
    pub fn get_lower_upper_crossings(&self) -> i32 {
        self.lower_upper_crossings
    }
}

fn number_of_north_south_edges(a: &LGraphArena, node: LNodeId, side: PortSide) -> i32 {
    let mut number_of_edges = 0;
    for port in a.node_port_side_view(node, side) {
        number_of_edges += if has_connected_north_south_edge(a, port) { 1 } else { 0 };
    }
    number_of_edges
}

fn has_connected_north_south_edge(a: &LGraphArena, port: LPortId) -> bool {
    a.port(port).properties.try_get(&iprops::PORT_DUMMY).is_some()
}

fn have_different_origins(a: &LGraphArena, upper_node: LNodeId, lower_node: LNodeId) -> bool {
    origin_of(a, upper_node) != origin_of(a, lower_node)
}

fn origin_port_of(a: &LGraphArena, node: LNodeId) -> LPortId {
    let port = a.node(node).ports[0];
    match a.port(port).properties.try_get(&iprops::ORIGIN) {
        Some(Origin::LPort(p)) => p,
        other => panic!("expected LPort origin on north/south port dummy, got {other:?}"),
    }
}

fn is_north_of_normal_node(a: &LGraphArena, upper_node: LNodeId) -> bool {
    a.port(origin_port_of(a, upper_node)).side == PortSide::NORTH
}

/// `(LNode) node.getProperty(InternalProperties.ORIGIN)` (reference
/// comparison; `None` corresponds to null).
fn origin_of(a: &LGraphArena, node: LNodeId) -> Option<Origin> {
    a.node(node).properties.try_get(&iprops::ORIGIN)
}

fn is_long_edge_dummy(a: &LGraphArena, node: LNodeId) -> bool {
    a.node(node).node_type == NodeType::LONG_EDGE
}

fn is_north_south(a: &LGraphArena, node: LNodeId) -> bool {
    a.node(node).node_type == NodeType::NORTH_SOUTH_PORT
}

fn is_normal(a: &LGraphArena, node: LNodeId) -> bool {
    a.node(node).node_type == NodeType::NORMAL
}
