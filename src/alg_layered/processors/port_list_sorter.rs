//! Sorts each node's port list clockwise (north,
//! east, south, west) and caches the side index ranges.

use std::cmp::Ordering;

use crate::core::options::{PortConstraints, PortSide};
use crate::graph::properties::ElkEnum;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LPortId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::PortSortingStrategy;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let pss: PortSortingStrategy = a.graph(graph).properties.get(&lopts::PORT_SORTING_STRATEGY);

    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            let port_constraints: PortConstraints =
                a.node(node).properties.get(&lopts::PORT_CONSTRAINTS);
            let mut ports = a.node(node).ports.clone();

            if port_constraints.is_order_fixed() {
                // CMP_COMBINED = side, then fixed order / fixed pos
                stable_sort_by(&mut ports, |&p1, &p2| cmp_combined(a, p1, p2));
                a.node_mut(node).ports = ports;
            } else if port_constraints.is_side_fixed() {
                stable_sort_by(&mut ports, |&p1, &p2| cmp_port_side(a, p1, p2));
                reverse_west_and_south_side(a, &mut ports);
                if pss == PortSortingStrategy::PORT_DEGREE {
                    stable_sort_by(&mut ports, |&p1, &p2| cmp_port_degree_east_west(a, p1, p2));
                }
                a.node_mut(node).ports = ports;
            }
            a.node_cache_port_sides(node);
        }
    }
    Ok(())
}

fn stable_sort_by<F: FnMut(&LPortId, &LPortId) -> Ordering>(ports: &mut [LPortId], cmp: F) {
    ports.sort_by(cmp); // Rust sort_by is stable
}

fn cmp_port_side(a: &LGraphArena, p1: LPortId, p2: LPortId) -> Ordering {
    let o1 = a.port(p1).side.ordinal() as i32;
    let o2 = a.port(p2).side.ordinal() as i32;
    o1.cmp(&o2)
}

fn cmp_fixed_order_and_fixed_pos(a: &LGraphArena, p1: LPortId, p2: LPortId) -> Ordering {
    let node = a.port(p1).node.unwrap();
    let port_constraints: PortConstraints = a.node(node).properties.get(&lopts::PORT_CONSTRAINTS);

    let ordinal_difference =
        a.port(p1).side.ordinal() as i32 - a.port(p2).side.ordinal() as i32;
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

fn cmp_combined(a: &LGraphArena, p1: LPortId, p2: LPortId) -> Ordering {
    cmp_port_side(a, p1, p2).then_with(|| cmp_fixed_order_and_fixed_pos(a, p1, p2))
}

/// Public version of CMP_COMBINED for use by the LGraph adapter.
pub fn cmp_combined_pub(a: &LGraphArena, p1: LPortId, p2: LPortId) -> Ordering {
    cmp_combined(a, p1, p2)
}

fn cmp_port_degree_east_west(a: &LGraphArena, p1: LPortId, p2: LPortId) -> Ordering {
    let ordinal_difference =
        a.port(p1).side.ordinal() as i32 - a.port(p2).side.ordinal() as i32;
    if ordinal_difference != 0 {
        return Ordering::Equal; // ports on different sides -- not our job
    }
    match a.port(p1).side {
        PortSide::EAST => {
            (real_degree(a, p2, false)).cmp(&real_degree(a, p1, false))
        }
        PortSide::WEST => (real_degree(a, p1, true)).cmp(&real_degree(a, p2, true)),
        _ => Ordering::Equal,
    }
}

fn real_degree(a: &LGraphArena, p: LPortId, incoming: bool) -> i32 {
    let edges = if incoming {
        &a.port(p).incoming_edges
    } else {
        &a.port(p).outgoing_edges
    };
    edges
        .iter()
        .filter(|&&e| !a.edge(e).properties.get(&iprops::REVERSED))
        .count() as i32
}

fn reverse_west_and_south_side(a: &LGraphArena, ports: &mut [LPortId]) {
    if ports.len() <= 1 {
        return;
    }
    let (lo, hi) = find_port_side_range(a, ports, PortSide::SOUTH);
    reverse(ports, lo, hi);
    let (lo, hi) = find_port_side_range(a, ports, PortSide::WEST);
    reverse(ports, lo, hi);
}

fn find_port_side_range(a: &LGraphArena, ports: &[LPortId], side: PortSide) -> (usize, usize) {
    if ports.is_empty() {
        return (0, 0);
    }
    let mut current_side = a.port(ports[0]).side;
    let mut low_idx = 0usize;
    let lb = side.ordinal();
    let hb = side.ordinal() + 1;

    while low_idx < ports.len() - 1 && current_side.ordinal() < lb {
        low_idx += 1;
        current_side = a.port(ports[low_idx]).side;
    }
    let mut high_idx = low_idx;
    while high_idx < ports.len() - 1 && current_side.ordinal() < hb {
        high_idx += 1;
        // Bug preserved: reads low_idx, not high_idx
        current_side = a.port(ports[low_idx]).side;
    }
    (low_idx, high_idx)
}

fn reverse(ports: &mut [LPortId], low_idx: usize, high_idx: usize) {
    if high_idx <= low_idx + 2 {
        return;
    }
    let n = (high_idx - low_idx) / 2;
    for i in 0..n {
        ports.swap(low_idx + i, high_idx - i - 1);
    }
}
