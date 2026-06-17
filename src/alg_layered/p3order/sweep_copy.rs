//! Stores node and port order for a sweep, and writes
//! a saved order back into the graph.

use std::cmp::Ordering;

use crate::core::options::{PortConstraints, PortSide};
use crate::graph::properties::ElkEnum;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;

/// Saves a copy of the node order and of the port order on each node.
#[derive(Clone, Debug)]
pub struct SweepCopy {
    /// Saves a copy of the node order.
    node_order: Vec<Vec<LNodeId>>,
    /// Saves a copy of the orders of the ports on each node, because they
    /// are reordered in each sweep.
    port_orders: Vec<Vec<Vec<LPortId>>>,
}

impl SweepCopy {
    /// Copies on construction (`SweepCopy(LNode[][])`).
    pub fn new(a: &LGraphArena, node_order_in: &[Vec<LNodeId>]) -> Self {
        let node_order = node_order_in.to_vec();
        let mut port_orders = Vec::with_capacity(node_order_in.len());
        for layer in node_order_in {
            let mut layer_ports = Vec::with_capacity(layer.len());
            for &node in layer {
                layer_ports.push(a.node(node).ports.clone());
            }
            port_orders.push(layer_ports);
        }
        SweepCopy { node_order, port_orders }
    }

    /// Returns the copy of the node orders.
    pub fn nodes(&self) -> &[Vec<LNodeId>] {
        &self.node_order
    }

    pub fn transfer_node_and_port_orders_to_graph(
        &self,
        a: &mut LGraphArena,
        lgraph: LGraphId,
        set_port_constraints: bool,
    ) {
        // the ALLOW_NON_FLOW_PORTS_TO_SWITCH_SIDES option allows the crossing
        // minimizer to decide the side a corresponding dummy node is placed
        // on; as a consequence the configured port side may not be valid
        // anymore and has to be corrected
        let mut north_south_port_dummies: Vec<LNodeId> = Vec::new();
        let mut update_port_order: Vec<LNodeId> = Vec::new();

        // iterate the layers
        let layers = a.graph(lgraph).layers.clone();
        for i in 0..layers.len() {
            let num_nodes = a.layer(layers[i]).nodes.len();
            north_south_port_dummies.clear();

            // iterate and order the nodes within the layer
            for j in 0..num_nodes {
                let node = self.node_order[i][j];
                // use the id field to remember the order within the layer
                a.node_mut(node).id = j as i32;
                if a.node(node).node_type == NodeType::NORTH_SOUTH_PORT {
                    north_south_port_dummies.push(node);
                }

                a.layer_mut(layers[i]).nodes[j] = node;
                // order ports as computed
                a.node_mut(node).ports = self.port_orders[i][j].clone();
                if set_port_constraints {
                    let constraints: PortConstraints =
                        a.node(node).properties.get(&lopts::PORT_CONSTRAINTS);
                    if !constraints.is_order_fixed() {
                        a.node(node)
                            .properties
                            .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_ORDER);
                    }
                }
            }

            // assert that the port side is set properly
            for &dummy in &north_south_port_dummies {
                let origin = assert_correct_port_sides(a, dummy);
                for n in [origin, dummy] {
                    if !update_port_order.contains(&n) {
                        update_port_order.push(n);
                    }
                }
            }
        }

        // since the side of certain ports may have changed at this point, the
        // list of ports must be re-sorted (see PortListSorter) and the port
        // list views must be re-cached.
        for &node in &update_port_order {
            let mut ports = a.node(node).ports.clone();
            ports.sort_by(|&p1, &p2| cmp_combined(a, p1, p2));
            a.node_mut(node).ports = ports;
            a.node_cache_port_sides(node);
        }
    }
}

/// Corrects the `PortSide` of the dummy's
/// origin. Returns the `LNode` ('origin') whose port `dummy` represents.
fn assert_correct_port_sides(a: &mut LGraphArena, dummy: LNodeId) -> LNodeId {
    debug_assert_eq!(a.node(dummy).node_type, NodeType::NORTH_SOUTH_PORT);

    let origin: LNodeId = a
        .node(dummy)
        .properties
        .try_get(&iprops::IN_LAYER_LAYOUT_UNIT)
        .expect("north/south port dummy without layout unit");

    // a north south port dummy has exactly one port
    let dummy_port = a.node(dummy).ports[0];
    let dummy_port_origin = match a.port(dummy_port).properties.try_get(&iprops::ORIGIN) {
        Some(Origin::LPort(p)) => Some(p),
        _ => None,
    };

    // find the corresponding port on the regular node
    for port in a.node(origin).ports.clone() {
        if Some(port) == dummy_port_origin {
            // switch the port's side if necessary
            let side = a.port(port).side;
            let dummy_id = a.node(dummy).id;
            let origin_id = a.node(origin).id;
            if side == PortSide::NORTH && dummy_id > origin_id {
                a.port_set_side(port, PortSide::SOUTH);
                if a.port(port).explicit_anchor {
                    // Set new coordinates for port anchor since it was
                    // switched from NORTH to SOUTH: mirror the y coordinate
                    let port_height = a.port(port).size.y;
                    let anchor_y = a.port(port).anchor.y;
                    a.port_mut(port).anchor.y = port_height - anchor_y;
                }
            } else if side == PortSide::SOUTH && origin_id > dummy_id {
                a.port_set_side(port, PortSide::NORTH);
                if a.port(port).explicit_anchor {
                    let port_height = a.port(port).size.y;
                    let anchor_y = a.port(port).anchor.y;
                    a.port_mut(port).anchor.y = -(port_height - anchor_y);
                }
            }
            break;
        }
    }
    origin
}

// ---------------------------------------------------------------------------
// PortListSorter.CMP_COMBINED (duplicated here because the processor module's
// comparators are private; semantics identical to
// processors/port_list_sorter.rs)

fn cmp_combined(a: &LGraphArena, p1: LPortId, p2: LPortId) -> Ordering {
    cmp_port_side(a, p1, p2).then_with(|| cmp_fixed_order_and_fixed_pos(a, p1, p2))
}

fn cmp_port_side(a: &LGraphArena, p1: LPortId, p2: LPortId) -> Ordering {
    let o1 = a.port(p1).side.ordinal() as i32;
    let o2 = a.port(p2).side.ordinal() as i32;
    o1.cmp(&o2)
}

fn cmp_fixed_order_and_fixed_pos(a: &LGraphArena, p1: LPortId, p2: LPortId) -> Ordering {
    let node = a.port(p1).node.unwrap();
    let port_constraints: PortConstraints = a.node(node).properties.get(&lopts::PORT_CONSTRAINTS);

    let ordinal_difference = a.port(p1).side.ordinal() as i32 - a.port(p2).side.ordinal() as i32;
    if ordinal_difference != 0 || !port_constraints.is_order_fixed() {
        return Ordering::Equal;
    }

    if port_constraints == PortConstraints::FIXED_ORDER {
        let index1 = a.port(p1).properties.try_get(&lopts::PORT_INDEX);
        let index2 = a.port(p2).properties.try_get(&lopts::PORT_INDEX);
        if let (Some(i1), Some(i2)) = (index1, index2) {
            if i1 != i2 {
                return i1.cmp(&i2);
            }
        }
    }

    match a.port(p1).side {
        PortSide::NORTH => a.port(p1).pos.x.total_cmp(&a.port(p2).pos.x),
        PortSide::EAST => a.port(p1).pos.y.total_cmp(&a.port(p2).pos.y),
        PortSide::SOUTH => a.port(p2).pos.x.total_cmp(&a.port(p1).pos.x),
        PortSide::WEST => a.port(p2).pos.y.total_cmp(&a.port(p1).pos.y),
        PortSide::UNDEFINED => panic!("Port side is undefined"),
    }
}
