//! The
//! greedy switch crossing minimization heuristic and its crossing counters.

pub mod between_layer_edge_two_node_crossings_counter;
pub mod crossing_matrix_filler;
pub mod greedy_switch_heuristic;
pub mod north_south_edge_neighbouring_node_crossings_counter;
pub mod switch_decider;

pub use greedy_switch_heuristic::GreedySwitchHeuristic;
