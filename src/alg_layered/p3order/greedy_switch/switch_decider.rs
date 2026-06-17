//! Decides whether two neighboring nodes should be
//! switched. There are two variants:
//!
//! - OneSided: checks if a switch would reduce crossings on the given side
//!   of the layer whose nodes are to be switched.
//! - TwoSided: checks if a switch would reduce crossings on both sides of
//!   the layer whose nodes are to be switched.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::options::PortSide;

use crate::alg_layered::graph::{LGraphArena, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;

use super::super::counting::CrossingsCounter;
use super::crossing_matrix_filler::CrossingMatrixFiller;
use super::north_south_edge_neighbouring_node_crossings_counter::NorthSouthEdgeNeighbouringNodeCrossingsCounter;

/// The side on which to count crossings for the one-sided SwitchDecider
/// (port of the nested enum `CrossingCountSide`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CrossingCountSide {
    /// Consider crossings to the west of the free layer.
    West,
    /// Consider crossings to the east of the free layer.
    East,
}

pub struct SwitchDecider {
    left_in_layer_counter: CrossingsCounter,
    right_in_layer_counter: CrossingsCounter,
    north_south_counter: NorthSouthEdgeNeighbouringNodeCrossingsCounter,
    crossing_matrix_filler: CrossingMatrixFiller,
}

impl SwitchDecider {
    /// Creates a SwitchDecider for the given free layer. The
    /// `port_positions` array is the `GreedySwitchHeuristic`'s array, shared
    /// between the two in-layer counters.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        a: &LGraphArena,
        free_layer_index: usize,
        graph: &[Vec<LNodeId>],
        crossing_matrix_filler: CrossingMatrixFiller,
        port_positions: Rc<RefCell<Vec<i32>>>,
        has_parent: bool,
        dont_sweep_into: bool,
        one_sided: bool,
    ) -> Result<Self, String> {
        if free_layer_index >= graph.len() {
            return Err("Greedy SwitchDecider: Free layer not in graph.".to_string());
        }
        let free_layer = &graph[free_layer_index];

        let mut left_in_layer_counter = CrossingsCounter::new_shared(port_positions.clone());
        left_in_layer_counter.init_port_positions_for_in_layer_crossings(
            a,
            free_layer,
            PortSide::WEST,
        );
        let mut right_in_layer_counter = CrossingsCounter::new_shared(port_positions);
        right_in_layer_counter.init_port_positions_for_in_layer_crossings(
            a,
            free_layer,
            PortSide::EAST,
        );
        let north_south_counter = NorthSouthEdgeNeighbouringNodeCrossingsCounter::new(a, free_layer);
        let count_crossings_caused_by_port_switch = !one_sided
            && has_parent
            && !dont_sweep_into
            && a.node(free_layer[0]).node_type == NodeType::EXTERNAL_PORT;
        if count_crossings_caused_by_port_switch {
            // Requires the parent graph's crossings counter
            // (initParentCrossingsCounters); only reachable with hierarchical
            // (INCLUDE_CHILDREN) crossing minimization, which is rejected
            // before any sweep starts.
            return Err(
                "TODO: hierarchical greedy switch (countCrossingsCausedByPortSwitch) not ported yet"
                    .to_string(),
            );
        }

        Ok(SwitchDecider {
            left_in_layer_counter,
            right_in_layer_counter,
            north_south_counter,
            crossing_matrix_filler,
        })
    }

    /// Notifies in-layer counters of node switch for efficiency reasons.
    pub fn notify_of_switch(&mut self, a: &LGraphArena, upper_node: LNodeId, lower_node: LNodeId) {
        self.left_in_layer_counter
            .switch_nodes(a, upper_node, lower_node, PortSide::WEST);
        self.right_in_layer_counter
            .switch_nodes(a, upper_node, lower_node, PortSide::EAST);
        // countCrossingsCausedByPortSwitch is always false in this port
        // (rejected in the constructor), so no parent counter to notify.
    }

    /// Whether switching the nodes represented by the indices would reduce
    /// the number of crossings. `free_layer` must be the live free layer
    /// (the mutated `LNode[]` is read in place).
    pub fn does_switch_reduce_crossings(
        &mut self,
        a: &LGraphArena,
        free_layer: &[LNodeId],
        upper_node_index: usize,
        lower_node_index: usize,
    ) -> bool {
        if self.constraints_prevent_switch(a, free_layer, upper_node_index, lower_node_index) {
            return false;
        }

        let upper_node = free_layer[upper_node_index];
        let lower_node = free_layer[lower_node_index];

        let left_inlayer = self.left_in_layer_counter.count_in_layer_crossings_between_nodes_in_both_orders(
            a,
            upper_node,
            lower_node,
            PortSide::WEST,
        );
        let right_inlayer = self.right_in_layer_counter.count_in_layer_crossings_between_nodes_in_both_orders(
            a,
            upper_node,
            lower_node,
            PortSide::EAST,
        );
        self.north_south_counter.count_crossings(a, upper_node, lower_node);
        let upper_lower_crossings = self
            .crossing_matrix_filler
            .get_crossing_matrix_entry(a, upper_node, lower_node)
            + left_inlayer.0
            + right_inlayer.0
            + self.north_south_counter.get_upper_lower_crossings();
        let lower_upper_crossings = self
            .crossing_matrix_filler
            .get_crossing_matrix_entry(a, lower_node, upper_node)
            + left_inlayer.1
            + right_inlayer.1
            + self.north_south_counter.get_lower_upper_crossings();

        // countCrossingsCausedByPortSwitch is always false in this port.

        upper_lower_crossings > lower_upper_crossings
    }

    /// Check if in-layer `IN_LAYER_SUCCESSOR_CONSTRAINTS` or
    /// `IN_LAYER_LAYOUT_UNIT` constraints prevent a possible switch or if
    /// the nodes are a normal node and a north south port dummy.
    fn constraints_prevent_switch(
        &self,
        a: &LGraphArena,
        free_layer: &[LNodeId],
        node_index: usize,
        lower_node_index: usize,
    ) -> bool {
        let upper_node = free_layer[node_index];
        let lower_node = free_layer[lower_node_index];

        have_successor_constraints(a, upper_node, lower_node)
            || have_layout_unit_constraints(a, upper_node, lower_node)
            || are_normal_and_north_south_port_dummy(a, upper_node, lower_node)
    }
}

fn have_successor_constraints(a: &LGraphArena, upper_node: LNodeId, lower_node: LNodeId) -> bool {
    // getProperty materializes the default (empty, Cloneable) list.
    let constraints: Vec<LNodeId> = a
        .node(upper_node)
        .properties
        .get(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS);
    !constraints.is_empty() && constraints.contains(&lower_node)
}

fn have_layout_unit_constraints(a: &LGraphArena, upper_node: LNodeId, lower_node: LNodeId) -> bool {
    let neither_node_is_long_edge_dummy = a.node(upper_node).node_type != NodeType::LONG_EDGE
        && a.node(lower_node).node_type != NodeType::LONG_EDGE;

    // If upperNode and lowerNode are part of a layout unit not only
    // containing themselves, then the layout units must be equal for a
    // switch to be allowed.
    let upper_layout_unit: Option<LNodeId> =
        a.node(upper_node).properties.try_get(&iprops::IN_LAYER_LAYOUT_UNIT);
    let lower_layout_unit: Option<LNodeId> =
        a.node(lower_node).properties.try_get(&iprops::IN_LAYER_LAYOUT_UNIT);

    let are_in_different_layout_units = upper_layout_unit != lower_layout_unit;

    // FIXME the following predicate is problematic, layout units
    // are represented by a regular node, thus 'upperNode' can be
    // 'upperLayoutUnit' and still have more nodes in the layout unit
    let mut nodes_have_layout_units = part_of_multi_node_layout_unit(upper_node, upper_layout_unit)
        || part_of_multi_node_layout_unit(lower_node, lower_layout_unit);

    let upper_node_has_northern_edges = has_edges_on_side(a, upper_node, PortSide::NORTH);
    let lower_node_has_southern_edges = has_edges_on_side(a, lower_node, PortSide::SOUTH);

    // hotfix for #162, if north or south edges are present, there must be a layout unit
    nodes_have_layout_units |= has_edges_on_side(a, upper_node, PortSide::SOUTH)
        || has_edges_on_side(a, lower_node, PortSide::NORTH);

    let has_layout_unit_constraint = (nodes_have_layout_units && are_in_different_layout_units)
        || (upper_node_has_northern_edges || lower_node_has_southern_edges);

    neither_node_is_long_edge_dummy && has_layout_unit_constraint
}

fn has_edges_on_side(a: &LGraphArena, node: LNodeId, side: PortSide) -> bool {
    for port in a.node_port_side_view(node, side) {
        if a.port(port).properties.try_get(&iprops::PORT_DUMMY).is_some()
            || a.port_degree(port) > 0
        {
            return true;
        }
    }
    false
}

fn part_of_multi_node_layout_unit(node: LNodeId, layout_unit: Option<LNodeId>) -> bool {
    layout_unit.is_some_and(|unit| unit != node)
}

fn are_normal_and_north_south_port_dummy(
    a: &LGraphArena,
    upper_node: LNodeId,
    lower_node: LNodeId,
) -> bool {
    (is_north_south_port_node(a, upper_node) && is_normal_node(a, lower_node))
        || (is_north_south_port_node(a, lower_node) && is_normal_node(a, upper_node))
}

fn is_normal_node(a: &LGraphArena, node: LNodeId) -> bool {
    a.node(node).node_type == NodeType::NORMAL
}

fn is_north_south_port_node(a: &LGraphArena, node: LNodeId) -> bool {
    a.node(node).node_type == NodeType::NORTH_SOUTH_PORT
}
