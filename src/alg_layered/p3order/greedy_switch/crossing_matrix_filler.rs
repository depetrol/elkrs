//! Manages the crossing matrix and fills it
//! on demand. It needs to be reinitialized for each free layer. For each
//! layer the `node.id` fields MUST be set from 0 to `layer.size() - 1`!

use crate::alg_layered::graph::{LGraphArena, LNodeId};

use super::super::layer_sweep::CrossMinType;
use super::between_layer_edge_two_node_crossings_counter::BetweenLayerEdgeTwoNodeCrossingsCounter;
use super::switch_decider::CrossingCountSide;

pub struct CrossingMatrixFiller {
    is_crossing_matrix_filled: Vec<Vec<bool>>,
    crossing_matrix: Vec<Vec<i32>>,
    in_between_layer_crossing_counter: BetweenLayerEdgeTwoNodeCrossingsCounter,
    direction: CrossingCountSide,
    one_sided: bool,
}

impl CrossingMatrixFiller {
    pub fn new(
        a: &LGraphArena,
        greedy_switch_type: CrossMinType,
        graph: &[Vec<LNodeId>],
        free_layer_index: usize,
        direction: CrossingCountSide,
    ) -> Self {
        let one_sided = greedy_switch_type == CrossMinType::OneSidedGreedySwitch;
        let free_layer_len = graph[free_layer_index].len();
        CrossingMatrixFiller {
            is_crossing_matrix_filled: vec![vec![false; free_layer_len]; free_layer_len],
            crossing_matrix: vec![vec![0; free_layer_len]; free_layer_len],
            in_between_layer_crossing_counter: BetweenLayerEdgeTwoNodeCrossingsCounter::new(
                a,
                graph,
                free_layer_index,
            ),
            direction,
            one_sided,
        }
    }

    /// Returns entry for crossings between edges incident to two nodes,
    /// where upperNode is above lowerNode in the layer.
    pub fn get_crossing_matrix_entry(
        &mut self,
        a: &LGraphArena,
        upper_node: LNodeId,
        lower_node: LNodeId,
    ) -> i32 {
        let upper_id = a.node(upper_node).id as usize;
        let lower_id = a.node(lower_node).id as usize;
        if !self.is_crossing_matrix_filled[upper_id][lower_id] {
            self.fill_crossing_matrix(a, upper_node, lower_node);
            self.is_crossing_matrix_filled[upper_id][lower_id] = true;
            self.is_crossing_matrix_filled[lower_id][upper_id] = true;
        }
        self.crossing_matrix[upper_id][lower_id]
    }

    fn fill_crossing_matrix(&mut self, a: &LGraphArena, upper_node: LNodeId, lower_node: LNodeId) {
        if self.one_sided {
            match self.direction {
                CrossingCountSide::East => self
                    .in_between_layer_crossing_counter
                    .count_eastern_edge_crossings(a, upper_node, lower_node),
                CrossingCountSide::West => self
                    .in_between_layer_crossing_counter
                    .count_western_edge_crossings(a, upper_node, lower_node),
            }
        } else {
            self.in_between_layer_crossing_counter
                .count_both_side_crossings(a, upper_node, lower_node);
        }
        let upper_id = a.node(upper_node).id as usize;
        let lower_id = a.node(lower_node).id as usize;
        self.crossing_matrix[upper_id][lower_id] =
            self.in_between_layer_crossing_counter.get_upper_lower_crossings();
        self.crossing_matrix[lower_id][upper_id] =
            self.in_between_layer_crossing_counter.get_lower_upper_crossings();
    }
}
