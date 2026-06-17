//!
//! A crossing minimizer that allows user interaction by respecting previous
//! node positions.

use std::cmp::Ordering;

use crate::graph::math::{KVector, KVectorChain};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::p1cycles::interactive::interactive_reference_point;
use crate::alg_layered::p3order::port_distributor::{PortDistributor, PortDistributorKind};

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // Set ID's for each layer since they will be used by the port distribution
    // code to index into arrays
    let layers = a.graph(graph).layers.clone();
    for (layer_index, &layer) in layers.iter().enumerate() {
        a.layer_mut(layer).id = layer_index as i32;
    }

    // nodeOrder = graph.toNodeArray()
    let node_order: Vec<Vec<LNodeId>> =
        layers.iter().map(|&l| a.layer(l).nodes.clone()).collect();

    let mut port_distributor =
        PortDistributor::new(PortDistributorKind::NodeRelative, node_order.len());

    // IInitializable.init(Arrays.asList(portDistributor), nodeOrder)
    for l in 0..node_order.len() {
        port_distributor.init_at_layer_level(l, &node_order);
        for n in 0..node_order[l].len() {
            port_distributor.init_at_node_level(a, l, n, &node_order);
            let num_ports = a.node(node_order[l][n]).ports.len();
            for p in 0..num_ports {
                port_distributor.init_at_port_level(a, l, n, p, &node_order);
            }
        }
    }
    port_distributor.init_after_traversal();

    let mut port_count = 0i32;
    for (layer_index, &layer) in layers.iter().enumerate() {
        // determine a horizontal position for edge bend points comparison
        let mut horiz_pos = 0.0f64;
        let mut node_count = 0i32;
        let layer_nodes = a.layer(layer).nodes.clone();
        for &node in &layer_nodes {
            if a.node(node).pos.x > 0.0 {
                horiz_pos += a.node(node).pos.x + a.node(node).size.x / 2.0;
                node_count += 1;
            }
            for p in 0..a.node(node).ports.len() {
                let port = a.node(node).ports[p];
                a.port_mut(port).id = port_count;
                port_count += 1;
            }
        }

        if node_count > 0 {
            horiz_pos /= node_count as f64;
        }

        // create an array of vertical node positions
        let mut pos = vec![0.0f64; layer_nodes.len()];
        let mut next_index = 0i32;
        for &node in &layer_nodes {
            a.node_mut(node).id = next_index;
            next_index += 1;
            let p = get_pos(a, node, horiz_pos);
            pos[a.node(node).id as usize] = p;

            // if we have a long edge dummy node, save the calculated position
            // in a property to be used by the interactive node placer
            if a.node(node).node_type == NodeType::LONG_EDGE {
                a.node_mut(node)
                    .properties
                    .set(&iprops::ORIGINAL_DUMMY_NODE_POSITION, p);
            }
        }

        // sort the nodes using the position array
        let mut sorted = layer_nodes.clone();
        sorted.sort_by(|&node1, &node2| {
            let compare = total_cmp_double(pos[a.node(node1).id as usize], pos[a.node(node2).id as usize]);
            if compare == Ordering::Equal {
                // The two nodes have the same y coordinate. Check for node
                // successor constraints
                let node1_succ: Vec<LNodeId> =
                    a.node(node1).properties.get(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS);
                let node2_succ: Vec<LNodeId> =
                    a.node(node2).properties.get(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS);

                if node1_succ.contains(&node2) {
                    return Ordering::Less;
                } else if node2_succ.contains(&node1) {
                    return Ordering::Greater;
                }
            }
            compare
        });
        a.layer_mut(layer).nodes = sorted;

        port_distributor.distribute_ports_while_sweeping(a, &node_order, layer_index, true);
    }

    Ok(())
}

/// `Double.compare`. Distinguishes -0.0/0.0 and orders NaN greatest;
/// `f64::total_cmp` matches `Double.compare` exactly.
fn total_cmp_double(a: f64, b: f64) -> Ordering {
    a.total_cmp(&b)
}

/// Determine a vertical position for the given node.
fn get_pos(a: &LGraphArena, node: LNodeId, horiz_pos: f64) -> f64 {
    match a.node(node).node_type {
        NodeType::LONG_EDGE => {
            let edge = match a.node(node).properties.try_get(&iprops::ORIGIN) {
                Some(iprops::Origin::LEdge(e)) => e,
                _ => panic!("LONG_EDGE dummy without LEdge origin"),
            };

            // reconstruct the original bend points from the node annotations
            let mut bendpoints: KVectorChain = match a
                .edge(edge)
                .properties
                .try_get(&iprops::ORIGINAL_BENDPOINTS)
            {
                None => KVectorChain::new(),
                Some(bp) => {
                    if a.edge(edge).properties.get(&iprops::REVERSED) {
                        KVectorChain::reverse(&bp)
                    } else {
                        bp
                    }
                }
            };

            // Check if we can determine the position just by using the source
            // point, if we can determine it
            let source: Option<LPortId> =
                a.node(node).properties.try_get(&iprops::LONG_EDGE_SOURCE);
            if let Some(source) = source {
                let source_point = port_absolute_anchor(a, source);
                if horiz_pos <= source_point.x {
                    return source_point.y;
                }
                bendpoints.add_first(source_point);
            }

            // Check if we can determine the position just by using the target
            // point
            let target: Option<LPortId> =
                a.node(node).properties.try_get(&iprops::LONG_EDGE_TARGET);
            if let Some(target) = target {
                let target_point = port_absolute_anchor(a, target);
                if target_point.x <= horiz_pos {
                    return target_point.y;
                }
                bendpoints.add_last(target_point);
            }

            // Find the two points along the edge that the horizontal point lies
            // between
            if bendpoints.len() >= 2 {
                let pts = &bendpoints.0;
                let mut i = 0usize;
                let mut point1 = pts[i];
                i += 1;
                let mut point2 = pts[i];
                while point2.x < horiz_pos && i + 1 < pts.len() {
                    point1 = point2;
                    i += 1;
                    point2 = pts[i];
                }
                return point1.y
                    + (horiz_pos - point1.x) / (point2.x - point1.x) * (point2.y - point1.y);
            }
        }

        NodeType::NORTH_SOUTH_PORT => {
            // Get one of the ports the dummy node was created for, and its
            // original node
            let dummy_port = a.node(node).ports[0];
            let origin_port = match a.port(dummy_port).properties.try_get(&iprops::ORIGIN) {
                Some(iprops::Origin::LPort(p)) => p,
                _ => panic!("NORTH_SOUTH_PORT dummy port without LPort origin"),
            };
            let origin_node = a.port(origin_port).node.unwrap();

            match a.port(origin_port).side {
                crate::core::options::PortSide::NORTH => {
                    // Use the position of the node's northern side.
                    return a.node(origin_node).pos.y;
                }
                crate::core::options::PortSide::SOUTH => {
                    // Use the position of the node's southern side
                    return a.node(origin_node).pos.y + a.node(origin_node).size.y;
                }
                _ => {}
            }
        }

        _ => {}
    }

    // the fallback solution is to take the previous position of the node's
    // anchor point
    interactive_reference_point(a, node).y
}

fn port_absolute_anchor(a: &LGraphArena, port: LPortId) -> KVector {
    let p = a.port(port);
    let node = p.node.unwrap();
    let n = a.node(node);
    KVector::new(n.pos.x + p.pos.x + p.anchor.x, n.pos.y + p.pos.y + p.anchor.y)
}
