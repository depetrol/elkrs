//!
//! Calculates the number of crossings for edges incident to two nodes. In
//! the case where there is free port order and two edges go into one port,
//! this crossing counter can in some cases count too few crossings.

use std::collections::HashMap;

use crate::core::options::PortSide;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LNodeId, LPortId};

use super::super::counting::in_north_south_east_west_order;

/// Naming assumes a left-right layer ordering.
pub struct BetweenLayerEdgeTwoNodeCrossingsCounter {
    upper_lower_crossings: i32,
    lower_upper_crossings: i32,
    /// the free layer (only `currentNodeOrder[freeLayerIndex]` is read after
    /// construction)
    free_layer: Vec<LNodeId>,
    port_positions: HashMap<LPortId, i32>,
    eastern_adjacencies: HashMap<LNodeId, AdjacencyList>,
    western_adjacencies: HashMap<LNodeId, AdjacencyList>,
}

impl BetweenLayerEdgeTwoNodeCrossingsCounter {
    /// The constructor: sets the port positions of the neighbouring
    /// layers.
    pub fn new(
        a: &LGraphArena,
        current_node_order: &[Vec<LNodeId>],
        free_layer_index: usize,
    ) -> Self {
        let mut counter = BetweenLayerEdgeTwoNodeCrossingsCounter {
            upper_lower_crossings: 0,
            lower_upper_crossings: 0,
            free_layer: current_node_order[free_layer_index].clone(),
            port_positions: HashMap::new(),
            eastern_adjacencies: HashMap::new(),
            western_adjacencies: HashMap::new(),
        };
        // setPortPositionsForNeighbouringLayers
        if free_layer_index > 0 {
            // freeLayerIsNotFirstLayer
            counter.set_port_positions_for_layer(
                a,
                &current_node_order[free_layer_index - 1],
                PortSide::EAST,
            );
        }
        if free_layer_index < current_node_order.len() - 1 {
            // freeLayerIsNotLastLayer
            counter.set_port_positions_for_layer(
                a,
                &current_node_order[free_layer_index + 1],
                PortSide::WEST,
            );
        }
        counter
    }

    fn set_port_positions_for_layer(
        &mut self,
        a: &LGraphArena,
        layer: &[LNodeId],
        port_side: PortSide,
    ) {
        let mut port_id = 0;
        for &node in layer {
            for port in in_north_south_east_west_order(a, node, port_side) {
                self.port_positions.insert(port, port_id);
                port_id += 1;
            }
        }
    }

    /// Calculates the number of crossings for incident edges coming from the
    /// east to the nodes (port of `countEasternEdgeCrossings`).
    pub fn count_eastern_edge_crossings(
        &mut self,
        a: &LGraphArena,
        upper_node: LNodeId,
        lower_node: LNodeId,
    ) {
        self.reset_crossing_count();
        if upper_node == lower_node {
            return;
        }
        self.add_eastern_crossings(a, upper_node, lower_node);
    }

    /// Calculates the number of crossings for incident edges coming from the
    /// west to the nodes (port of `countWesternEdgeCrossings`).
    pub fn count_western_edge_crossings(
        &mut self,
        a: &LGraphArena,
        upper_node: LNodeId,
        lower_node: LNodeId,
    ) {
        self.reset_crossing_count();
        if upper_node == lower_node {
            return;
        }
        self.add_western_crossings(a, upper_node, lower_node);
    }

    /// Calculates the number of crossings for incident edges coming from
    /// both sides to the nodes (port of `countBothSideCrossings`).
    pub fn count_both_side_crossings(
        &mut self,
        a: &LGraphArena,
        upper_node: LNodeId,
        lower_node: LNodeId,
    ) {
        self.reset_crossing_count();
        if upper_node == lower_node {
            return;
        }
        self.add_western_crossings(a, upper_node, lower_node);
        self.add_eastern_crossings(a, upper_node, lower_node);
    }

    fn reset_crossing_count(&mut self) {
        self.upper_lower_crossings = 0;
        self.lower_upper_crossings = 0;
    }

    fn add_eastern_crossings(&mut self, a: &LGraphArena, upper_node: LNodeId, lower_node: LNodeId) {
        self.ensure_adjacencies(a, PortSide::EAST);
        // (each fetch resets the saved list to its original state)
        let mut upper = self.eastern_adjacencies.remove(&upper_node).unwrap();
        upper.reset();
        let mut lower = self.eastern_adjacencies.remove(&lower_node).unwrap();
        lower.reset();
        if upper.size() != 0 && lower.size() != 0 {
            self.count_crossings_by_merging_adjacency_lists(&mut upper, &mut lower);
        }
        self.eastern_adjacencies.insert(upper_node, upper);
        self.eastern_adjacencies.insert(lower_node, lower);
    }

    fn add_western_crossings(&mut self, a: &LGraphArena, upper_node: LNodeId, lower_node: LNodeId) {
        self.ensure_adjacencies(a, PortSide::WEST);
        let mut upper = self.western_adjacencies.remove(&upper_node).unwrap();
        upper.reset();
        let mut lower = self.western_adjacencies.remove(&lower_node).unwrap();
        lower.reset();
        if upper.size() != 0 && lower.size() != 0 {
            self.count_crossings_by_merging_adjacency_lists(&mut upper, &mut lower);
        }
        self.western_adjacencies.insert(upper_node, upper);
        self.western_adjacencies.insert(lower_node, lower);
    }

    /// Since calculating adjacencies is a little expensive, it is only done
    /// once for each configuration and the sorted adjacencies saved
    /// (`getAdjacencyFor`'s lazy fill of the map).
    fn ensure_adjacencies(&mut self, a: &LGraphArena, side: PortSide) {
        let port_positions = &self.port_positions;
        let adjacencies = match side {
            PortSide::EAST => &mut self.eastern_adjacencies,
            _ => &mut self.western_adjacencies,
        };
        if adjacencies.is_empty() {
            for &n in &self.free_layer {
                adjacencies.insert(n, AdjacencyList::new(a, n, side, port_positions));
            }
        }
    }

    /// The main algorithm: by merging the two sorted adjacency lists, both
    /// the number of between-layer crossings for the order upper - lower and
    /// for the opposite order can be found.
    fn count_crossings_by_merging_adjacency_lists(
        &mut self,
        upper_adjacencies: &mut AdjacencyList,
        lower_adjacencies: &mut AdjacencyList,
    ) {
        while !upper_adjacencies.is_empty() && !lower_adjacencies.is_empty() {
            if is_below(upper_adjacencies.first(), lower_adjacencies.first()) {
                self.upper_lower_crossings += upper_adjacencies.size();
                lower_adjacencies.remove_first();
            } else if is_below(lower_adjacencies.first(), upper_adjacencies.first()) {
                self.lower_upper_crossings += lower_adjacencies.size();
                upper_adjacencies.remove_first();
            } else {
                self.upper_lower_crossings +=
                    upper_adjacencies.count_adjacencies_below_node_of_first_port();
                self.lower_upper_crossings +=
                    lower_adjacencies.count_adjacencies_below_node_of_first_port();
                upper_adjacencies.remove_first();
                lower_adjacencies.remove_first();
            }
        }
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

fn is_below(first_port: i32, second_port: i32) -> bool {
    first_port > second_port
}

/// The inner class `AdjacencyList`: the adjacency list of a node
/// holds the positions of connected ports in a neighbouring layer on the
/// given side. The remove operation does not actually delete entries;
/// `current_index` / `current_size` / `current_cardinality` track the
/// current state and `reset()` restores the original state.
struct AdjacencyList {
    adjacency_list: Vec<Adjacency>,
    size: i32,
    current_size: i32,
    current_index: usize,
}

/// Adjacency containing only the position and number of ports with the same
/// position.
struct Adjacency {
    /// The position of the port.
    position: i32,
    /// The number of adjacencies with the same position.
    cardinality: i32,
    /// The current number of adjacencies with the same position.
    current_cardinality: i32,
}

impl Adjacency {
    fn reset(&mut self) {
        self.current_cardinality = self.cardinality;
    }
}

impl AdjacencyList {
    fn new(
        a: &LGraphArena,
        node: LNodeId,
        side: PortSide,
        port_positions: &HashMap<LPortId, i32>,
    ) -> Self {
        let mut list = AdjacencyList {
            adjacency_list: Vec::new(),
            size: 0,
            current_size: 0,
            current_index: 0,
        };
        // getAdjacenciesSortedByPosition / iterateTroughEdgesCollectingAdjacencies
        for port in in_north_south_east_west_order(a, node, side) {
            let edges: Vec<LEdgeId> = if side == PortSide::WEST {
                a.port(port).incoming_edges.clone()
            } else {
                a.port(port).outgoing_edges.clone()
            };
            for edge in edges {
                if !a.edge_is_self_loop(edge) && is_not_in_layer(a, edge) {
                    list.add_adjacency_of(a, edge, side, port_positions);
                    list.size += 1;
                    list.current_size += 1;
                }
            }
        }
        // stable sort, compares positions only.
        list.adjacency_list.sort_by(|x, y| x.position.cmp(&y.position));
        list
    }

    fn add_adjacency_of(
        &mut self,
        a: &LGraphArena,
        edge: LEdgeId,
        side: PortSide,
        port_positions: &HashMap<LPortId, i32>,
    ) {
        let adjacent_port = if side == PortSide::WEST {
            a.edge(edge).source.unwrap()
        } else {
            a.edge(edge).target.unwrap()
        };
        let adjacent_port_position = *port_positions
            .get(&adjacent_port)
            .expect("adjacent port must be in a neighbouring layer");
        match self.adjacency_list.last_mut() {
            Some(last) if last.position == adjacent_port_position => {
                last.cardinality += 1;
                last.current_cardinality += 1;
            }
            _ => self.adjacency_list.push(Adjacency {
                position: adjacent_port_position,
                cardinality: 1,
                current_cardinality: 1,
            }),
        }
    }

    fn reset(&mut self) {
        self.current_index = 0;
        self.current_size = self.size;
        if !self.is_empty() {
            self.adjacency_list[self.current_index].reset();
        }
    }

    fn count_adjacencies_below_node_of_first_port(&self) -> i32 {
        self.current_size - self.adjacency_list[self.current_index].current_cardinality
    }

    fn remove_first(&mut self) {
        if self.is_empty() {
            return;
        }
        if self.adjacency_list[self.current_index].current_cardinality == 1 {
            self.increment_current_index();
        } else {
            self.adjacency_list[self.current_index].current_cardinality -= 1;
        }
        self.current_size -= 1;
    }

    fn increment_current_index(&mut self) {
        self.current_index += 1;
        // reset Adjacency for reuse
        if self.current_index < self.adjacency_list.len() {
            self.adjacency_list[self.current_index].reset();
        }
    }

    fn is_empty(&self) -> bool {
        self.current_size == 0
    }

    fn first(&self) -> i32 {
        self.adjacency_list[self.current_index].position
    }

    fn size(&self) -> i32 {
        self.current_size
    }
}

fn is_not_in_layer(a: &LGraphArena, edge: LEdgeId) -> bool {
    a.node(a.edge_source_node(edge)).layer != a.node(a.edge_target_node(edge)).layer
}
