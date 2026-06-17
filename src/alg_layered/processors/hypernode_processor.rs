//! Improves the placement of hypernodes by
//! moving them such that they replace the join points of connected edges.
//! Runs after phase 5.

use crate::core::options::PortSide;
use crate::graph::math::KVector;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId};
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            if a.node(node).properties.get(&lopts::HYPERNODE) && a.node(node).ports.len() <= 2 {
                let (mut top, mut right, mut bottom, mut left) = (0, 0, 0, 0);
                for &port in &a.node(node).ports {
                    match a.port(port).side {
                        PortSide::NORTH => top += 1,
                        PortSide::EAST => right += 1,
                        PortSide::SOUTH => bottom += 1,
                        PortSide::WEST => left += 1,
                        PortSide::UNDEFINED => {}
                    }
                }
                // don't move the node if there are any edges to the top or bottom
                if top == 0 && bottom == 0 {
                    move_hypernode(a, graph, node, left <= right);
                }
            }
        }
    }
    Ok(())
}

fn move_hypernode(a: &mut LGraphArena, graph: LGraphId, hypernode: LNodeId, right: bool) {
    const MAX: f64 = i32::MAX as f64; // Integer.MAX_VALUE as a double

    let mut bend_edges: Vec<crate::alg_layered::graph::LEdgeId> = Vec::new();
    let mut bendx: f64 = MAX;
    let mut diffx: f64 = MAX;
    let mut diffy: f64 = MAX;

    if right {
        bendx = a.graph(graph).size.x;
        let ports = a.node(hypernode).ports.clone();
        for port in ports {
            let edges = a.port(port).outgoing_edges.clone();
            for edge in edges {
                if !a.edge(edge).bend_points.is_empty() {
                    let first_point = a.edge(edge).bend_points.first();
                    if first_point.x < bendx {
                        diffx = bendx - first_point.x;
                        diffy = MAX;
                        bend_edges.clear();
                        bendx = first_point.x;
                    }
                    if first_point.x <= bendx {
                        bend_edges.push(edge);
                        if a.edge(edge).bend_points.len() > 1 {
                            let second_y = a.edge(edge).bend_points.0[1].y;
                            diffy = diffy.min((second_y - first_point.y).abs());
                        }
                    }
                }
            }
        }
    } else {
        let ports = a.node(hypernode).ports.clone();
        for port in ports {
            let edges = a.port(port).incoming_edges.clone();
            for edge in edges {
                if !a.edge(edge).bend_points.is_empty() {
                    let last_point = a.edge(edge).bend_points.last();
                    if last_point.x > bendx {
                        diffx = last_point.x - bendx;
                        diffy = MAX;
                        bend_edges.clear();
                        bendx = last_point.x;
                    }
                    if last_point.x >= bendx {
                        bend_edges.push(edge);
                        let size = a.edge(edge).bend_points.len();
                        if size > 1 {
                            let pen_y = a.edge(edge).bend_points.0[size - 2].y;
                            diffy = diffy.min((pen_y - last_point.y).abs());
                        }
                    }
                }
            }
        }
    }

    let hn_size = a.node(hypernode).size;
    if !bend_edges.is_empty() && diffx > hn_size.x / 2.0 && diffy > hn_size.y / 2.0 {
        // create new ports for the edges
        let north_port = a.create_port();
        a.port_set_node(north_port, Some(hypernode));
        a.port_set_side(north_port, PortSide::NORTH);
        a.port_mut(north_port).pos.x = hn_size.x / 2.0;

        let south_port = a.create_port();
        a.port_set_node(south_port, Some(hypernode));
        a.port_set_side(south_port, PortSide::SOUTH);
        a.port_mut(south_port).pos.x = hn_size.x / 2.0;
        a.port_mut(south_port).pos.y = hn_size.y;

        for edge in bend_edges {
            let first: KVector;
            let second: KVector;
            if right {
                first = a.edge_mut(edge).bend_points.0.remove(0);
                second = if a.edge(edge).bend_points.is_empty() {
                    let tgt = a.edge(edge).target.unwrap();
                    port_absolute_anchor(a, tgt)
                } else {
                    a.edge(edge).bend_points.first()
                };
                if second.y >= first.y {
                    a.edge_set_source(edge, Some(south_port));
                } else {
                    a.edge_set_source(edge, Some(north_port));
                }
            } else {
                first = a.edge_mut(edge).bend_points.0.pop().unwrap();
                second = if a.edge(edge).bend_points.is_empty() {
                    let src = a.edge(edge).source.unwrap();
                    port_absolute_anchor(a, src)
                } else {
                    a.edge(edge).bend_points.last()
                };
                if second.y >= first.y {
                    a.edge_set_target(edge, Some(south_port));
                } else {
                    a.edge_set_target(edge, Some(north_port));
                }
            }
            // remove junction points that collide with the eliminated bend point
            if let Some(mut junction_points) =
                a.edge(edge).properties.try_get(&lopts::JUNCTION_POINTS)
            {
                if let Some(pos) = junction_points.0.iter().position(|&jp| jp == first) {
                    junction_points.0.remove(pos);
                    a.edge(edge).properties.set(&lopts::JUNCTION_POINTS, junction_points);
                }
            }
        }
        // move the node to new position
        a.node_mut(hypernode).pos.x = bendx - hn_size.x / 2.0;
    }
}

fn port_absolute_anchor(a: &LGraphArena, port: LPortId) -> KVector {
    let p = a.port(port);
    let node = p.node.unwrap();
    let n = a.node(node);
    KVector::new(n.pos.x + p.pos.x + p.anchor.x, n.pos.y + p.pos.y + p.anchor.y)
}
