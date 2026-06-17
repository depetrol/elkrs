//! Distributes ports greedily on a single
//! node. Used as the sweep port distributor for
//! `CrossMinType.TWO_SIDED_GREEDY_SWITCH`.

use crate::core::options::{PortConstraints, PortSide};

use crate::alg_layered::graph::{LGraphArena, LNodeId, LPortId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;

use super::counting::CrossingsCounter;
use super::greedy_switch::between_layer_edge_two_node_crossings_counter::BetweenLayerEdgeTwoNodeCrossingsCounter;

pub struct GreedyPortDistributor {
    crossings_counter: Option<CrossingsCounter>,
    n_ports: i32,
    hierarchical_crossings_counter: Option<BetweenLayerEdgeTwoNodeCrossingsCounter>,
}

impl GreedyPortDistributor {
    pub fn new() -> Self {
        GreedyPortDistributor {
            crossings_counter: None,
            n_ports: 0,
            hierarchical_crossings_counter: None,
        }
    }

    pub fn distribute_ports_while_sweeping(
        &mut self,
        a: &mut LGraphArena,
        node_order: &[Vec<LNodeId>],
        current_index: usize,
        is_forward_sweep: bool,
    ) -> bool {
        self.initialize(a, node_order, current_index, is_forward_sweep);

        self.distribute_ports_in_layer(a, node_order, current_index, is_forward_sweep)
    }

    fn distribute_ports_in_layer(
        &mut self,
        a: &mut LGraphArena,
        node_order: &[Vec<LNodeId>],
        current_index: usize,
        is_forward_sweep: bool,
    ) -> bool {
        let side = if is_forward_sweep { PortSide::WEST } else { PortSide::EAST };
        let mut improved = false;
        for &node in &node_order[current_index] {
            let constraints: PortConstraints =
                a.node(node).properties.get(&lopts::PORT_CONSTRAINTS);
            if constraints.is_order_fixed() {
                continue;
            }
            let nested_graph = a.node(node).nested_graph;
            let use_hierarchical_cross_counter =
                !a.node_port_side_view(node, side).is_empty() && nested_graph.is_some();
            if use_hierarchical_cross_counter {
                let nested = nested_graph.unwrap();
                let inner_graph: Vec<Vec<LNodeId>> = {
                    let layers = a.graph(nested).layers.clone();
                    layers.iter().map(|&l| a.layer(l).nodes.clone()).collect()
                };
                let free_layer_index =
                    if is_forward_sweep { 0 } else { inner_graph.len() - 1 };
                self.hierarchical_crossings_counter = Some(
                    BetweenLayerEdgeTwoNodeCrossingsCounter::new(a, &inner_graph, free_layer_index),
                );
            }
            improved |= self.distribute_ports_on_node(a, node, side, use_hierarchical_cross_counter);
        }
        improved
    }

    /// Distribute ports greedily on a single node.
    fn distribute_ports_on_node(
        &mut self,
        a: &mut LGraphArena,
        node: LNodeId,
        side: PortSide,
        use_hierarchical_crosscounter: bool,
    ) -> bool {
        // Works on the live port side (sub-)list view, reversed for
        // SOUTH/WEST; switches write through into the node's port list.
        let view = a.node_port_side_view(node, side);
        let mut ports: Vec<LPortId> = if side == PortSide::SOUTH || side == PortSide::WEST {
            view.into_iter().rev().collect()
        } else {
            view
        };
        let mut improved = false;
        loop {
            let mut continue_switching = false;
            for i in 0..ports.len().saturating_sub(1) {
                let upper_port = ports[i];
                let lower_port = ports[i + 1];
                if self.switching_decreases_crossings(
                    a,
                    upper_port,
                    lower_port,
                    use_hierarchical_crosscounter,
                ) {
                    improved = true;
                    self.switch_ports(a, &mut ports, node, i, i + 1);
                    continue_switching = true;
                }
            }
            if !continue_switching {
                break;
            }
        }
        improved
    }

    /// Initialize crossings counter for given layers (`initForLayers`).
    fn init_for_layers(&mut self, a: &LGraphArena, left_layer: &[LNodeId], right_layer: &[LNodeId]) {
        self.crossings_counter
            .as_mut()
            .unwrap()
            .init_for_counting_between(a, left_layer, right_layer);
    }

    fn switching_decreases_crossings(
        &mut self,
        a: &LGraphArena,
        upper_port: LPortId,
        lower_port: LPortId,
        use_hierarchical_crosscounter: bool,
    ) -> bool {
        let (mut upper_lower_crossings, mut lower_upper_crossings) = self
            .crossings_counter
            .as_mut()
            .unwrap()
            .count_crossings_between_ports_in_both_orders(a, upper_port, lower_port);
        if use_hierarchical_crosscounter {
            let upper_node: Option<LNodeId> =
                a.port(upper_port).properties.try_get(&iprops::PORT_DUMMY);
            let lower_node: Option<LNodeId> =
                a.port(lower_port).properties.try_get(&iprops::PORT_DUMMY);
            if let (Some(upper_node), Some(lower_node)) = (upper_node, lower_node) {
                let counter = self.hierarchical_crossings_counter.as_mut().unwrap();
                counter.count_both_side_crossings(a, upper_node, lower_node);
                upper_lower_crossings += counter.get_upper_lower_crossings();
                lower_upper_crossings += counter.get_lower_upper_crossings();
            }
        }
        upper_lower_crossings > lower_upper_crossings
    }

    fn switch_ports(
        &mut self,
        a: &mut LGraphArena,
        ports: &mut [LPortId],
        node: LNodeId,
        top_port: usize,
        bottom_port: usize,
    ) {
        self.crossings_counter
            .as_mut()
            .unwrap()
            .switch_ports(a, ports[top_port], ports[bottom_port]);
        // write through to the node's real port list (the port side
        // sublist view is mutated)
        let node_ports = &mut a.node_mut(node).ports;
        let i1 = node_ports.iter().position(|&p| p == ports[top_port]).unwrap();
        let i2 = node_ports.iter().position(|&p| p == ports[bottom_port]).unwrap();
        node_ports.swap(i1, i2);
        ports.swap(top_port, bottom_port);
    }

    fn initialize(
        &mut self,
        a: &LGraphArena,
        node_order: &[Vec<LNodeId>],
        current_index: usize,
        is_forward_sweep: bool,
    ) {
        if is_forward_sweep && current_index > 0 {
            self.init_for_layers(a, &node_order[current_index - 1], &node_order[current_index]);
        } else if !is_forward_sweep && current_index < node_order.len() - 1 {
            self.init_for_layers(a, &node_order[current_index], &node_order[current_index + 1]);
        } else {
            self.crossings_counter
                .as_mut()
                .unwrap()
                .init_port_positions_for_in_layer_crossings(
                    a,
                    &node_order[current_index],
                    if is_forward_sweep { PortSide::WEST } else { PortSide::EAST },
                );
        }
    }

    // ---------------------------------------------------- initialization
    // (IInitializable hooks)

    pub fn init_at_node_level(&mut self, a: &LGraphArena, l: usize, n: usize, node_order: &[Vec<LNodeId>]) {
        self.n_ports += a.node(node_order[l][n]).ports.len() as i32;
    }

    pub fn init_after_traversal(&mut self) {
        self.crossings_counter = Some(CrossingsCounter::new(vec![0; self.n_ports as usize]));
    }
}

impl Default for GreedyPortDistributor {
    fn default() -> Self {
        Self::new()
    }
}
