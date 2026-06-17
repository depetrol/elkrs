//!
//! The two subclasses only differ in `calculatePortRanks(node, rankSum,
//! type)`, so they are merged into one struct with a `kind` discriminator.

use std::cmp::Ordering;

use crate::core::javacompat::JavaRandom;
use crate::core::options::{PortConstraints, PortSide};
use crate::graph::properties::ElkEnum;

use crate::alg_layered::graph::{LGraphArena, LNodeId, LPortId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::PortType;

use super::greedy_port_distributor::GreedyPortDistributor;
use super::layer_sweep::CrossMinType;

/// The `ISweepPortDistributor` interface: either one of the two
/// barycenter-based distributors or the greedy port distributor.
pub enum SweepPortDistributor {
    /// `AbstractBarycenterPortDistributor` subclasses
    Barycenter(PortDistributor),
    /// `GreedyPortDistributor` (used for TWO_SIDED_GREEDY_SWITCH)
    Greedy(GreedyPortDistributor),
}

impl SweepPortDistributor {
    /// Note the random consumption:
    /// for TWO_SIDED_GREEDY_SWITCH no random boolean is drawn; for all other
    /// types one boolean is drawn during `GraphInfoHolder` construction.
    pub fn create(
        cmt: CrossMinType,
        random: &mut JavaRandom,
        num_layers: usize,
    ) -> SweepPortDistributor {
        if cmt == CrossMinType::TwoSidedGreedySwitch {
            SweepPortDistributor::Greedy(GreedyPortDistributor::new())
        } else if random.next_boolean() {
            // Since both methods lead to different results, but neither is
            // clearly better, we choose randomly.
            SweepPortDistributor::Barycenter(PortDistributor::new(
                PortDistributorKind::NodeRelative,
                num_layers,
            ))
        } else {
            SweepPortDistributor::Barycenter(PortDistributor::new(
                PortDistributorKind::LayerTotal,
                num_layers,
            ))
        }
    }

    /// The barycenter heuristic casts the distributor to
    /// `AbstractBarycenterPortDistributor` (ClassCastException
    /// otherwise — unreachable, the GraphInfoHolder pairs them correctly).
    pub fn as_barycenter_mut(&mut self) -> &mut PortDistributor {
        match self {
            SweepPortDistributor::Barycenter(d) => d,
            SweepPortDistributor::Greedy(_) => {
                panic!("barycenter heuristic requires an AbstractBarycenterPortDistributor")
            }
        }
    }

    pub fn distribute_ports_while_sweeping(
        &mut self,
        a: &mut LGraphArena,
        node_order: &[Vec<LNodeId>],
        current_index: usize,
        is_forward_sweep: bool,
    ) -> bool {
        match self {
            SweepPortDistributor::Barycenter(d) => {
                d.distribute_ports_while_sweeping(a, node_order, current_index, is_forward_sweep)
            }
            SweepPortDistributor::Greedy(d) => {
                d.distribute_ports_while_sweeping(a, node_order, current_index, is_forward_sweep)
            }
        }
    }

    // ------------------------------------------- IInitializable dispatch

    pub fn init_at_layer_level(&mut self, l: usize, node_order: &[Vec<LNodeId>]) {
        if let SweepPortDistributor::Barycenter(d) = self {
            d.init_at_layer_level(l, node_order);
        }
    }

    pub fn init_at_node_level(
        &mut self,
        a: &mut LGraphArena,
        l: usize,
        n: usize,
        node_order: &[Vec<LNodeId>],
    ) {
        match self {
            SweepPortDistributor::Barycenter(d) => d.init_at_node_level(a, l, n, node_order),
            SweepPortDistributor::Greedy(d) => d.init_at_node_level(a, l, n, node_order),
        }
    }

    pub fn init_at_port_level(
        &mut self,
        a: &mut LGraphArena,
        l: usize,
        n: usize,
        p: usize,
        node_order: &[Vec<LNodeId>],
    ) {
        if let SweepPortDistributor::Barycenter(d) = self {
            d.init_at_port_level(a, l, n, p, node_order);
        }
    }

    pub fn init_after_traversal(&mut self) {
        match self {
            SweepPortDistributor::Barycenter(d) => d.init_after_traversal(),
            SweepPortDistributor::Greedy(d) => d.init_after_traversal(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PortDistributorKind {
    /// `NodeRelativePortDistributor`
    NodeRelative,
    /// `LayerTotalPortDistributor`
    LayerTotal,
}

pub struct PortDistributor {
    kind: PortDistributorKind,
    /// port ranks array in which the results of ranks calculation are stored.
    port_ranks: Vec<f32>,
    min_barycenter: f32,
    max_barycenter: f32,
    node_positions: Vec<Vec<i32>>,
    port_barycenter: Vec<f32>,
    in_layer_ports: Vec<LPortId>,
    n_ports: i32,
}

impl PortDistributor {
    /// `new NodeRelativePortDistributor(numLayers)` /
    /// `new LayerTotalPortDistributor(numLayers)`.
    pub fn new(kind: PortDistributorKind, num_layers: usize) -> PortDistributor {
        PortDistributor {
            kind,
            port_ranks: Vec::new(),
            min_barycenter: 0.0,
            max_barycenter: 0.0,
            node_positions: vec![Vec::new(); num_layers],
            port_barycenter: Vec::new(),
            in_layer_ports: Vec::new(),
            n_ports: 0,
        }
    }

    pub fn kind(&self) -> PortDistributorKind {
        self.kind
    }

    /// Returns the array of port ranks (indexed by `LPort.id`).
    pub fn port_ranks(&self) -> &[f32] {
        &self.port_ranks
    }

    // -------------------------------------------------- port rank assignment

    pub fn distribute_ports_while_sweeping(
        &mut self,
        a: &mut LGraphArena,
        node_order: &[Vec<LNodeId>],
        current_index: usize,
        is_forward_sweep: bool,
    ) -> bool {
        self.update_node_positions(a, node_order, current_index);
        let free_layer = node_order[current_index].clone();
        let side = if is_forward_sweep { PortSide::WEST } else { PortSide::EAST };
        if is_not_first_layer(node_order.len(), current_index, is_forward_sweep) {
            let fixed_layer = node_order[if is_forward_sweep {
                current_index - 1
            } else {
                current_index + 1
            }]
            .clone();
            self.calculate_port_ranks_layer(a, &fixed_layer, port_type_for(is_forward_sweep));
            for &node in &free_layer {
                self.distribute_ports(a, node, side);
            }

            self.calculate_port_ranks_layer(a, &free_layer, port_type_for(!is_forward_sweep));
            for &node in &fixed_layer {
                if !has_nested_graph(a, node) {
                    self.distribute_ports(a, node, side.opposed());
                }
            }
        } else {
            for &node in &free_layer {
                self.distribute_ports(a, node, side);
            }
        }
        // Barycenter port distributor can not be used with always improving
        // crossing minimization heuristics which do not need to count.
        false
    }

    pub fn calculate_port_ranks_layer(
        &mut self,
        a: &LGraphArena,
        layer: &[LNodeId],
        port_type: PortType,
    ) {
        let mut consumed_rank = 0.0f32;
        for &node in layer {
            consumed_rank += self.calculate_port_ranks(a, node, consumed_rank, port_type);
        }
    }

    /// The abstract `calculatePortRanks(LNode, float, PortType)`;
    /// dispatches on the distributor kind.
    fn calculate_port_ranks(
        &mut self,
        a: &LGraphArena,
        node: LNodeId,
        rank_sum: f32,
        port_type: PortType,
    ) -> f32 {
        match self.kind {
            PortDistributorKind::NodeRelative => {
                match port_type {
                    PortType::INPUT => {
                        // Count the number of input ports, and additionally
                        // the north-side input ports
                        let mut input_count = 0;
                        let mut north_input_count = 0;
                        for &port in &a.node(node).ports {
                            if !a.port(port).incoming_edges.is_empty() {
                                input_count += 1;
                                if a.port(port).side == PortSide::NORTH {
                                    north_input_count += 1;
                                }
                            }
                        }

                        // Assign port ranks in the order north - west - south - east
                        let incr = 1.0f32 / (input_count + 1) as f32;
                        let mut north_pos = rank_sum + north_input_count as f32 * incr;
                        let mut rest_pos = rank_sum + 1.0 - incr;
                        for port in a.node_input_ports(node) {
                            if a.port(port).side == PortSide::NORTH {
                                self.port_ranks[a.port(port).id as usize] = north_pos;
                                north_pos -= incr;
                            } else {
                                self.port_ranks[a.port(port).id as usize] = rest_pos;
                                rest_pos -= incr;
                            }
                        }
                    }

                    PortType::OUTPUT => {
                        // Count the number of output ports
                        let mut output_count = 0;
                        for &port in &a.node(node).ports {
                            if !a.port(port).outgoing_edges.is_empty() {
                                output_count += 1;
                            }
                        }

                        // Iterate output ports in their natural order, that
                        // is north - east - south - west
                        let incr = 1.0f32 / (output_count + 1) as f32;
                        let mut pos = rank_sum + incr;
                        for port in a.node_output_ports(node) {
                            self.port_ranks[a.port(port).id as usize] = pos;
                            pos += incr;
                        }
                    }

                    PortType::UNDEFINED => panic!("Port type is undefined"),
                }
                // the consumed rank is always 1
                1.0
            }

            PortDistributorKind::LayerTotal => match port_type {
                PortType::INPUT => {
                    // Count the number of input ports, and additionally the
                    // north-side input ports
                    let mut input_count = 0;
                    let mut north_input_count = 0;
                    for &port in &a.node(node).ports {
                        if !a.port(port).incoming_edges.is_empty() {
                            input_count += 1;
                            if a.port(port).side == PortSide::NORTH {
                                north_input_count += 1;
                            }
                        }
                    }

                    // Assign port ranks in the order north - west - south - east
                    let mut north_pos = rank_sum + north_input_count as f32;
                    let mut rest_pos = rank_sum + input_count as f32;
                    for port in a.node_input_ports(node) {
                        if a.port(port).side == PortSide::NORTH {
                            self.port_ranks[a.port(port).id as usize] = north_pos;
                            north_pos -= 1.0;
                        } else {
                            self.port_ranks[a.port(port).id as usize] = rest_pos;
                            rest_pos -= 1.0;
                        }
                    }

                    // the consumed rank corresponds to the number of input ports
                    input_count as f32
                }

                PortType::OUTPUT => {
                    // Iterate output ports in their natural order, that is
                    // north - east - south - west
                    let mut pos = 0;
                    for port in a.node_output_ports(node) {
                        pos += 1;
                        self.port_ranks[a.port(port).id as usize] = rank_sum + pos as f32;
                    }
                    pos as f32
                }

                PortType::UNDEFINED => panic!("Port type is undefined"),
            },
        }
    }

    // ------------------------------------------------------ port distribution

    fn distribute_ports(&mut self, a: &mut LGraphArena, node: LNodeId, side: PortSide) {
        let constraints: PortConstraints = a.node(node).properties.get(&lopts::PORT_CONSTRAINTS);
        if !constraints.is_order_fixed() {
            // distribute ports in sweep direction and on north south side of node.
            let ports = a.node_ports_on_side(node, side);
            self.distribute_ports_list(a, node, &ports);
            let ports = a.node_ports_on_side(node, PortSide::SOUTH);
            self.distribute_ports_list(a, node, &ports);
            let ports = a.node_ports_on_side(node, PortSide::NORTH);
            self.distribute_ports_list(a, node, &ports);
            // sort the ports by considering the side, type, and barycenter values
            self.sort_ports(a, node);
        }
    }

    fn distribute_ports_list(&mut self, a: &LGraphArena, node: LNodeId, ports: &[LPortId]) {
        self.in_layer_ports.clear();
        self.iterate_ports_and_collect_in_layer_ports(a, node, ports);

        if !self.in_layer_ports.is_empty() {
            self.calculate_in_layer_ports_barycenter_values(a, node);
        }
    }

    fn iterate_ports_and_collect_in_layer_ports(
        &mut self,
        a: &LGraphArena,
        node: LNodeId,
        ports: &[LPortId],
    ) {
        self.min_barycenter = 0.0f32;
        self.max_barycenter = 0.0f32;

        // a float value large enough to ensure that barycenters of south ports work fine
        let absurdly_large_float =
            (2 * a.layer(a.node(node).layer.unwrap()).nodes.len() + 1) as f32;
        // calculate barycenter values for the ports of the node
        'port_iteration: for &port in ports {
            let port_side = a.port(port).side;
            let north_south_port = port_side == PortSide::NORTH || port_side == PortSide::SOUTH;
            let mut sum = 0.0f32;

            if north_south_port {
                // Find the dummy node created for the port
                let port_dummy: Option<LNodeId> =
                    a.port(port).properties.try_get(&iprops::PORT_DUMMY);
                let port_dummy = match port_dummy {
                    None => continue,
                    Some(d) => d,
                };

                sum += self.deal_with_north_south_ports(a, absurdly_large_float, port, port_dummy);
            } else {
                // add up all ranks of connected ports
                for &outgoing_edge in &a.port(port).outgoing_edges {
                    let connected_port = a.edge(outgoing_edge).target.unwrap();
                    if a.node(a.port(connected_port).node.unwrap()).layer == a.node(node).layer {
                        self.in_layer_ports.push(port);
                        continue 'port_iteration;
                    } else {
                        // outgoing edges go to the subsequent layer and are seen clockwise
                        sum += self.port_ranks[a.port(connected_port).id as usize];
                    }
                }
                for &incoming_edge in &a.port(port).incoming_edges {
                    let connected_port = a.edge(incoming_edge).source.unwrap();
                    if a.node(a.port(connected_port).node.unwrap()).layer == a.node(node).layer {
                        self.in_layer_ports.push(port);
                        continue 'port_iteration;
                    } else {
                        // incoming edges go to the preceding layer and are
                        // seen counter-clockwise
                        sum -= self.port_ranks[a.port(connected_port).id as usize];
                    }
                }
            }

            let degree = a.port_degree(port);
            if degree > 0 {
                self.port_barycenter[a.port(port).id as usize] = sum / degree as f32;
                self.min_barycenter =
                    self.min_barycenter.min(self.port_barycenter[a.port(port).id as usize]);
                self.max_barycenter =
                    self.max_barycenter.max(self.port_barycenter[a.port(port).id as usize]);
            } else if north_south_port {
                // For northern and southern ports, the sum directly
                // corresponds to the barycenter value to be used.
                self.port_barycenter[a.port(port).id as usize] = sum;
            }
        }
    }

    fn calculate_in_layer_ports_barycenter_values(&mut self, a: &LGraphArena, node: LNodeId) {
        // go through the list of in-layer ports and calculate their barycenter values
        let node_index_in_layer = self.position_of(a, node) + 1;
        let layer_size = a.layer(a.node(node).layer.unwrap()).nodes.len() as i32 + 1;
        let in_layer_ports = std::mem::take(&mut self.in_layer_ports);
        for &in_layer_port in &in_layer_ports {
            // add the indices of all connected in-layer ports
            let mut sum = 0;
            let mut in_layer_connections = 0;

            for edge in a.port_connected_edges(in_layer_port) {
                let connected_port = self.other_end_of(a, edge, in_layer_port);
                if a.node(a.port(connected_port).node.unwrap()).layer == a.node(node).layer {
                    sum += self.position_of(a, a.port(connected_port).node.unwrap()) + 1;
                    in_layer_connections += 1;
                }
            }
            // The port's barycenter value is the mean index of connected nodes.
            let barycenter = sum as f32 / in_layer_connections as f32;

            let port_side = a.port(in_layer_port).side;
            let id = a.port(in_layer_port).id as usize;

            if port_side == PortSide::EAST {
                if barycenter < node_index_in_layer as f32 {
                    // take a low value in order to have the port above
                    self.port_barycenter[id] = self.min_barycenter - barycenter;
                } else {
                    // take a high value in order to have the port below
                    self.port_barycenter[id] =
                        self.max_barycenter + (layer_size as f32 - barycenter);
                }
            } else if port_side == PortSide::WEST {
                if barycenter < node_index_in_layer as f32 {
                    // take a high value in order to have the port above
                    self.port_barycenter[id] = self.max_barycenter + barycenter;
                } else {
                    // take a low value in order to have the port below
                    self.port_barycenter[id] =
                        self.min_barycenter - (layer_size as f32 - barycenter);
                }
            }
        }
        self.in_layer_ports = in_layer_ports;
    }

    fn deal_with_north_south_ports(
        &mut self,
        a: &LGraphArena,
        absurdly_large_float: f32,
        port: LPortId,
        port_dummy: LNodeId,
    ) -> f32 {
        // Find out if it's an input port, an output port, or both
        let mut input = false;
        let mut output = false;
        for &port_dummy_port in &a.node(port_dummy).ports {
            if a.port(port_dummy_port).properties.try_get(&iprops::ORIGIN)
                == Some(Origin::LPort(port))
            {
                if !a.port(port_dummy_port).outgoing_edges.is_empty() {
                    output = true;
                } else if !a.port(port_dummy_port).incoming_edges.is_empty() {
                    input = true;
                }
            }
        }
        let mut sum = 0.0f32;
        if input && (input ^ output) {
            // It's an input port; the index of its dummy node is its inverted sortkey
            sum = if a.port(port).side == PortSide::NORTH {
                -(self.position_of(a, port_dummy) as f32)
            } else {
                absurdly_large_float - self.position_of(a, port_dummy) as f32
            };
        } else if output && (input ^ output) {
            // It's an output port; the index of its dummy node is its sort key
            sum = self.position_of(a, port_dummy) as f32 + 1.0f32;
        } else if input && output {
            // It's both, an input and an output port; it must sit between
            // input and output ports
            sum = if a.port(port).side == PortSide::NORTH {
                0.0f32
            } else {
                absurdly_large_float / 2.0f32
            };
        }
        sum
    }

    fn position_of(&self, a: &LGraphArena, node: LNodeId) -> i32 {
        self.node_positions[a.layer(a.node(node).layer.unwrap()).id as usize]
            [a.node(node).id as usize]
    }

    fn update_node_positions(
        &mut self,
        a: &LGraphArena,
        node_order: &[Vec<LNodeId>],
        current_index: usize,
    ) {
        let layer = &node_order[current_index];
        for (i, &node) in layer.iter().enumerate() {
            self.node_positions[a.layer(a.node(node).layer.unwrap()).id as usize]
                [a.node(node).id as usize] = i as i32;
        }
    }

    fn other_end_of(&self, a: &LGraphArena, edge: crate::alg_layered::graph::LEdgeId, from_port: LPortId) -> LPortId {
        let e = a.edge(edge);
        if from_port == e.source.unwrap() {
            e.target.unwrap()
        } else {
            e.source.unwrap()
        }
    }

    /// Sort the ports of a node using the relative
    /// position values as a hint for the clockwise order of ports.
    fn sort_ports(&self, a: &mut LGraphArena, node: LNodeId) {
        let mut ports = a.node(node).ports.clone();
        // Vec::sort_by is stable.
        ports.sort_by(|&port1, &port2| {
            let side1 = a.port(port1).side;
            let side2 = a.port(port2).side;

            if side1 != side2 {
                // sort according to the node side
                (side1.ordinal() as i32).cmp(&(side2.ordinal() as i32))
            } else {
                let port1_bary = self.port_barycenter[a.port(port1).id as usize];
                let port2_bary = self.port_barycenter[a.port(port2).id as usize];
                if port1_bary == 0.0 && port2_bary == 0.0 {
                    Ordering::Equal
                } else if port1_bary == 0.0 {
                    Ordering::Less
                } else if port2_bary == 0.0 {
                    Ordering::Greater
                } else {
                    // sort according to the position value
                    // (Float.compare; barycenters are never NaN here)
                    port1_bary.total_cmp(&port2_bary)
                }
            }
        });
        a.node_mut(node).ports = ports;
    }

    // ---------------------------------------------------- initialization

    pub fn init_at_layer_level(&mut self, l: usize, node_order: &[Vec<LNodeId>]) {
        self.node_positions[l] = vec![0; node_order[l].len()];
    }

    pub fn init_at_node_level(
        &mut self,
        a: &mut LGraphArena,
        l: usize,
        n: usize,
        node_order: &[Vec<LNodeId>],
    ) {
        let node = node_order[l][n];
        a.node_mut(node).id = n as i32;
        self.node_positions[l][n] = n as i32;
    }

    pub fn init_at_port_level(
        &mut self,
        a: &mut LGraphArena,
        l: usize,
        n: usize,
        p: usize,
        node_order: &[Vec<LNodeId>],
    ) {
        let port = a.node(node_order[l][n]).ports[p];
        a.port_mut(port).id = self.n_ports;
        self.n_ports += 1;
    }

    pub fn init_after_traversal(&mut self) {
        self.port_ranks = vec![0.0; self.n_ports as usize];
        self.port_barycenter = vec![0.0; self.n_ports as usize];
    }
}

fn has_nested_graph(a: &LGraphArena, node: LNodeId) -> bool {
    a.node(node).nested_graph.is_some()
}

fn is_not_first_layer(length: usize, current_index: usize, is_forward_sweep: bool) -> bool {
    if is_forward_sweep {
        current_index != 0
    } else {
        current_index != length - 1
    }
}

fn port_type_for(is_forward_sweep: bool) -> PortType {
    if is_forward_sweep {
        PortType::OUTPUT
    } else {
        PortType::INPUT
    }
}
