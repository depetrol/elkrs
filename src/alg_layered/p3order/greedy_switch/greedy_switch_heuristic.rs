//!
//! Implements the greedy switch heuristic: for two neighboring nodes, check
//! to see if by exchanging their positions ("switching" them) the number of
//! crossings is reduced. If it is, switch them, if it is not, don't.
//!
//! Configuration depends on the `CrossMinType`:
//! - `ONE_SIDED_GREEDY_SWITCH`: fixes the order of one layer and changes the
//!   order in a neighboring layer using the number of crossings to this
//!   neighboring layer.
//! - `TWO_SIDED_GREEDY_SWITCH`: sets a layer as free and counts crossings to
//!   both neighboring layers.

use std::cell::RefCell;
use std::rc::Rc;

use crate::alg_layered::graph::{LGraphArena, LNodeId};

use super::super::layer_sweep::CrossMinType;
use super::crossing_matrix_filler::CrossingMatrixFiller;
use super::switch_decider::{CrossingCountSide, SwitchDecider};

pub struct GreedySwitchHeuristic {
    greedy_switch_type: CrossMinType,
    /// shared with the per-layer `SwitchDecider`s' in-layer counters
    port_positions: Rc<RefCell<Vec<i32>>>,
    n_ports: i32,
    /// `GraphInfoHolder.hasParent()` of the graph being processed.
    pub has_parent: bool,
    /// `GraphInfoHolder.dontSweepInto()` of the graph being processed
    /// (set after the layer sweep type decision).
    pub dont_sweep_into: bool,
}

impl GreedySwitchHeuristic {
    /// `new GreedySwitchHeuristic(greedyType, graphData)`; the needed
    /// `graphData` bits (`hasParent`, `dontSweepInto`) are stored as fields.
    pub fn new(greedy_type: CrossMinType) -> Self {
        GreedySwitchHeuristic {
            greedy_switch_type: greedy_type,
            port_positions: Rc::new(RefCell::new(Vec::new())),
            n_ports: 0,
            has_parent: false,
            dont_sweep_into: false,
        }
    }

    pub fn minimize_crossings(
        &mut self,
        a: &LGraphArena,
        order: &mut [Vec<LNodeId>],
        free_layer_index: usize,
        forward_sweep: bool,
        _is_first_sweep: bool,
    ) -> Result<bool, String> {
        let mut decider = self.set_up(a, order, free_layer_index, forward_sweep)?;
        // continueSwitchingUntilNoImprovementInLayer
        let mut improved = false;
        loop {
            let continue_switching =
                self.sweep_downward_in_layer(a, order, &mut decider, free_layer_index);
            improved |= continue_switching;
            if !continue_switching {
                break;
            }
        }
        Ok(improved)
    }

    pub fn set_first_layer_order(
        &mut self,
        a: &LGraphArena,
        order: &mut [Vec<LNodeId>],
        is_forward_sweep: bool,
    ) -> Result<bool, String> {
        let start_index = start_index(is_forward_sweep, order.len());
        let mut decider = self.set_up(a, order, start_index, is_forward_sweep)?;
        Ok(self.sweep_downward_in_layer(a, order, &mut decider, start_index))
    }

    fn set_up(
        &mut self,
        a: &LGraphArena,
        order: &[Vec<LNodeId>],
        free_layer_index: usize,
        forward_sweep: bool,
    ) -> Result<SwitchDecider, String> {
        let side = if forward_sweep { CrossingCountSide::West } else { CrossingCountSide::East };
        let crossing_matrix_filler =
            CrossingMatrixFiller::new(a, self.greedy_switch_type, order, free_layer_index, side);
        SwitchDecider::new(
            a,
            free_layer_index,
            order,
            crossing_matrix_filler,
            self.port_positions.clone(),
            self.has_parent,
            self.dont_sweep_into,
            self.greedy_switch_type == CrossMinType::OneSidedGreedySwitch,
        )
    }

    fn sweep_downward_in_layer(
        &mut self,
        a: &LGraphArena,
        order: &mut [Vec<LNodeId>],
        decider: &mut SwitchDecider,
        layer_index: usize,
    ) -> bool {
        let mut continue_switching = false;
        let length_of_free_layer = order[layer_index].len();
        for upper_node_index in 0..length_of_free_layer.saturating_sub(1) {
            let lower_node_index = upper_node_index + 1;

            continue_switching |= self.switch_if_improves(
                a,
                order,
                decider,
                layer_index,
                upper_node_index,
                lower_node_index,
            );
        }
        continue_switching
    }

    #[allow(clippy::too_many_arguments)]
    fn switch_if_improves(
        &mut self,
        a: &LGraphArena,
        order: &mut [Vec<LNodeId>],
        decider: &mut SwitchDecider,
        layer_index: usize,
        upper_node_index: usize,
        lower_node_index: usize,
    ) -> bool {
        let mut continue_switching = false;

        if decider.does_switch_reduce_crossings(
            a,
            &order[layer_index],
            upper_node_index,
            lower_node_index,
        ) {
            self.exchange_nodes(a, order, decider, upper_node_index, lower_node_index, layer_index);

            continue_switching = true;
        }
        continue_switching
    }

    #[allow(clippy::too_many_arguments)]
    fn exchange_nodes(
        &mut self,
        a: &LGraphArena,
        order: &mut [Vec<LNodeId>],
        decider: &mut SwitchDecider,
        index_one: usize,
        index_two: usize,
        layer_index: usize,
    ) {
        decider.notify_of_switch(a, order[layer_index][index_one], order[layer_index][index_two]);
        order[layer_index].swap(index_one, index_two);
    }

    // ---------------------------------------------------- initialization
    // (IInitializable hooks, called by GraphInfoHolder's init traversal)

    pub fn init_at_layer_level(&mut self, a: &mut LGraphArena, l: usize, node_order: &[Vec<LNodeId>]) {
        // nodeOrder[l][0].getLayer().id = l
        let layer = a.node(node_order[l][0]).layer.unwrap();
        a.layer_mut(layer).id = l as i32;
    }

    pub fn init_at_port_level(&mut self) {
        self.n_ports += 1;
    }

    pub fn init_after_traversal(&mut self) {
        self.port_positions = Rc::new(RefCell::new(vec![0; self.n_ports as usize]));
    }
}

fn start_index(is_forward_sweep: bool, length: usize) -> usize {
    if is_forward_sweep {
        0
    } else {
        length - 1
    }
}
