//! Decides whether to sweep into a graph
//! (hierarchical handling) or process it bottom-up.

use crate::core::options::{PortConstraints, PortSide};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;

/// The nested `NodeInfo`: collects number of paths to nodes with
/// random or hierarchical influence.
#[derive(Clone, Copy, Default, Debug)]
struct NodeInfo {
    connected_edges: i32,
    hierarchical_influence: i32,
    random_influence: i32,
}

impl NodeInfo {
    fn transfer(&mut self, other: NodeInfo) {
        self.hierarchical_influence += other.hierarchical_influence;
        self.random_influence += other.random_influence;
        self.connected_edges += other.connected_edges;
    }
}

pub struct LayerSweepTypeDecider {
    node_info: Vec<Vec<NodeInfo>>,
}

impl LayerSweepTypeDecider {
    pub fn new(num_layers: usize) -> Self {
        LayerSweepTypeDecider { node_info: vec![Vec::new(); num_layers] }
    }

    pub fn init_at_layer_level(&mut self, a: &mut LGraphArena, l: usize, node_order: &[Vec<LNodeId>]) {
        // nodeOrder[l][0].getLayer().id = l (throws on empty layers)
        let layer = a.node(node_order[l][0]).layer.unwrap();
        a.layer_mut(layer).id = l as i32;
        self.node_info[l] = vec![NodeInfo::default(); node_order[l].len()];
    }

    pub fn init_at_node_level(&mut self, a: &mut LGraphArena, l: usize, n: usize, node_order: &[Vec<LNodeId>]) {
        let node = node_order[l][n];
        a.node_mut(node).id = n as i32;
        self.node_info[l][n] = NodeInfo::default();
    }

    /// Decide whether to use bottom up or
    /// cross-hierarchical sweep method.
    pub fn use_bottom_up(
        &mut self,
        a: &LGraphArena,
        lgraph: LGraphId,
        parent: Option<LNodeId>,
        cross_min_deterministic: bool,
        current_node_order: &[Vec<LNodeId>],
    ) -> bool {
        let boundary: f64 = a
            .graph(lgraph)
            .properties
            .get(&lopts::CROSSING_MINIMIZATION_HIERARCHICAL_SWEEPINESS);
        if bottom_up_forced(boundary)
            || parent.is_none() // rootNode()
            || fixed_port_order(a, parent.unwrap())
            || fewer_than_two_in_out_edges(a, parent.unwrap())
        {
            return true;
        }

        if cross_min_deterministic {
            return false;
        }

        let mut paths_to_random: i32 = 0;
        let mut paths_to_hierarchical: i32 = 0;

        let mut ns_port_dummies: Vec<LNodeId> = Vec::new();
        for layer in current_node_order {
            for &node in layer {
                // We must visit all sources of edges first, so we collect
                // north south dummies for later.
                if is_north_south_dummy(a, node) {
                    ns_port_dummies.push(node);
                    continue;
                }

                // Check for hierarchical port dummies or random influence.
                if is_external_port_dummy(a, node) {
                    self.node_info_mut(a, node).hierarchical_influence = 1;
                    if is_eastern_dummy(a, node) {
                        paths_to_hierarchical += self.node_info_of(a, node).connected_edges;
                    }
                } else if has_no_western_ports(a, node) {
                    self.node_info_mut(a, node).random_influence = 1;
                } else if has_no_eastern_ports(a, node) {
                    paths_to_random += self.node_info_of(a, node).connected_edges;
                }

                // Increase counts of paths by the number outgoing edges times
                // the influence and transfer information to targets.
                for edge in a.node_outgoing_edges(node) {
                    let current = self.node_info_of(a, node);
                    paths_to_random += current.random_influence;
                    paths_to_hierarchical += current.hierarchical_influence;
                    self.transfer_info_to(a, current, a.edge_target_node(edge));
                }

                // Do the same for north/south dummies.
                let mut north_south_ports = a.node_port_side_view(node, PortSide::NORTH);
                north_south_ports.extend(a.node_port_side_view(node, PortSide::SOUTH));
                for port in north_south_ports {
                    let ns_dummy: Option<LNodeId> =
                        a.port(port).properties.try_get(&iprops::PORT_DUMMY);
                    if let Some(ns_dummy) = ns_dummy {
                        let current = self.node_info_of(a, node);
                        paths_to_random += current.random_influence;
                        paths_to_hierarchical += current.hierarchical_influence;
                        self.transfer_info_to(a, current, ns_dummy);
                    }
                }
            }

            // Now process nsPortDummies
            for &node in &ns_port_dummies {
                for edge in a.node_outgoing_edges(node) {
                    let current = self.node_info_of(a, node);
                    paths_to_random += current.random_influence;
                    paths_to_hierarchical += current.hierarchical_influence;
                    self.transfer_info_to(a, current, a.edge_target_node(edge));
                }
            }
            ns_port_dummies.clear();
        }

        let all_paths = (paths_to_random + paths_to_hierarchical) as f64;
        let normalized = if all_paths == 0.0 {
            f64::INFINITY
        } else {
            (paths_to_random - paths_to_hierarchical) as f64 / all_paths
        };
        normalized >= boundary
    }

    fn transfer_info_to(&mut self, a: &LGraphArena, current: NodeInfo, target: LNodeId) {
        let target_node_info = self.node_info_mut(a, target);
        target_node_info.transfer(current);
        target_node_info.connected_edges += 1;
    }

    fn node_info_of(&self, a: &LGraphArena, node: LNodeId) -> NodeInfo {
        let layer = a.node(node).layer.unwrap();
        self.node_info[a.layer(layer).id as usize][a.node(node).id as usize]
    }

    fn node_info_mut(&mut self, a: &LGraphArena, node: LNodeId) -> &mut NodeInfo {
        let layer = a.node(node).layer.unwrap();
        &mut self.node_info[a.layer(layer).id as usize][a.node(node).id as usize]
    }
}

fn fixed_port_order(a: &LGraphArena, parent: LNodeId) -> bool {
    let constraints: PortConstraints = a.node(parent).properties.get(&lopts::PORT_CONSTRAINTS);
    constraints.is_order_fixed()
}

fn fewer_than_two_in_out_edges(a: &LGraphArena, parent: LNodeId) -> bool {
    a.node_port_side_view(parent, PortSide::EAST).len() < 2
        && a.node_port_side_view(parent, PortSide::WEST).len() < 2
}

fn bottom_up_forced(boundary: f64) -> bool {
    boundary < -1.0
}

fn has_no_eastern_ports(a: &LGraphArena, node: LNodeId) -> bool {
    let east_ports = a.node_port_side_view(node, PortSide::EAST);
    east_ports.is_empty()
        || !east_ports.iter().any(|&p| {
            !a.port(p).incoming_edges.is_empty() || !a.port(p).outgoing_edges.is_empty()
        })
}

fn has_no_western_ports(a: &LGraphArena, node: LNodeId) -> bool {
    let west_ports = a.node_port_side_view(node, PortSide::WEST);
    west_ports.is_empty()
        || !west_ports.iter().any(|&p| {
            !a.port(p).incoming_edges.is_empty() || !a.port(p).outgoing_edges.is_empty()
        })
}

fn is_external_port_dummy(a: &LGraphArena, node: LNodeId) -> bool {
    a.node(node).node_type == NodeType::EXTERNAL_PORT
}

fn is_north_south_dummy(a: &LGraphArena, node: LNodeId) -> bool {
    a.node(node).node_type == NodeType::NORTH_SOUTH_PORT
}

fn is_eastern_dummy(a: &LGraphArena, node: LNodeId) -> bool {
    a.port(origin_port(a, node)).side == PortSide::EAST
}

fn origin_port(a: &LGraphArena, node: LNodeId) -> crate::alg_layered::graph::LPortId {
    match a.node(node).properties.try_get(&iprops::ORIGIN) {
        Some(Origin::LPort(p)) => p,
        other => panic!("expected LPort origin on external port dummy, got {other:?}"),
    }
}
