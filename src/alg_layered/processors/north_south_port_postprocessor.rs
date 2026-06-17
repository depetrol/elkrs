//! Removes the dummy nodes created by
//! the `NorthSouthPortPreprocessor` and routes the edges properly.

use crate::core::options::{EdgeRouting, PortSide};
use crate::graph::math::{KVector, KVectorChain};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // Spline edge routing manages bend points in the edge router. Differentiate
    // behaviour depending on edge router.
    let routing: EdgeRouting = a.graph(graph).properties.get(&lopts::EDGE_ROUTING);

    // Iterate through the layers
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        // Iterate through the nodes (use an array to avoid concurrent modification)
        let node_array = a.layer(layer).nodes.clone();
        for node in node_array {
            // We only care for North/South Port dummy nodes
            if a.node(node).node_type != NodeType::NORTH_SOUTH_PORT {
                continue;
            }

            if routing == EdgeRouting::SPLINES {
                // Iterate through ports
                for port in a.node(node).ports.clone() {
                    if !a.port(port).incoming_edges.is_empty() {
                        process_spline_input_port(a, port)?;
                    }
                    if !a.port(port).outgoing_edges.is_empty() {
                        process_spline_output_port(a, port)?;
                    }
                }
            } else if matches!(
                a.node(node).properties.try_get(&iprops::ORIGIN),
                Some(Origin::LEdge(_))
            ) {
                // It's a self-loop
                process_self_loop(a, node)?;
            } else {
                // Check if all ports were created for the same origin port
                let ports = a.node(node).ports.clone();
                let same_origin_port = if ports.len() >= 2 {
                    // Iterate over the dummy's ports to find out whether the origin is
                    // always the same
                    ports.windows(2).all(|pair| {
                        a.port(pair[0]).properties.try_get(&iprops::ORIGIN)
                            == a.port(pair[1]).properties.try_get(&iprops::ORIGIN)
                    })
                } else {
                    false
                };

                // Iterate through the ports
                for port in ports {
                    if !a.port(port).incoming_edges.is_empty() {
                        process_input_port(a, port, same_origin_port)?;
                    }

                    if !a.port(port).outgoing_edges.is_empty() {
                        process_output_port(a, port, same_origin_port)?;
                    }
                }
            }

            // Remove the node
            a.node_set_layer(node, None);
        }
    }

    Ok(())
}

/// The origin port a dummy port was created for.
fn origin_port(a: &LGraphArena, dummy_port: LPortId) -> Result<LPortId, String> {
    match a.port(dummy_port).properties.try_get(&iprops::ORIGIN) {
        Some(Origin::LPort(p)) => Ok(p),
        other => Err(format!(
            "north/south port dummy port without LPort origin: {other:?}"
        )),
    }
}

/// `LPort.getAbsoluteAnchor().x`.
fn absolute_anchor_x(a: &LGraphArena, port: LPortId) -> f64 {
    let p = a.port(port);
    let n = a.node(p.node.unwrap());
    n.pos.x + p.pos.x + p.anchor.x
}

/// Adds a junction point at `(x, y)` to the edge, materializing the
/// `JUNCTION_POINTS` default if necessary and appending.
fn add_junction_point(a: &LGraphArena, edge: crate::alg_layered::graph::LEdgeId, x: f64, y: f64) {
    let mut junction_points: KVectorChain = a.edge(edge).properties.get(&lopts::JUNCTION_POINTS);
    junction_points.add_last(KVector::new(x, y));
    a.edge(edge).properties.set(&lopts::JUNCTION_POINTS, junction_points);
}

/// Reroutes the edges connected to the given input port back to the port it
/// was created for. If `add_junction_points` is set, adds a junction point to
/// the edge that equals the bend point computed for the edge.
fn process_input_port(
    a: &mut LGraphArena,
    input_port: LPortId,
    add_junction_points: bool,
) -> Result<(), String> {
    // Retrieve the port the dummy node was created from
    let origin = origin_port(a, input_port)?;

    // Calculate the bend point
    let x = absolute_anchor_x(a, origin);
    let y = a.node(a.port(input_port).node.unwrap()).pos.y;

    // Reroute the edges, inserting a new bend point at the position of the dummy node
    let edge_array = a.port(input_port).incoming_edges.clone();
    for in_edge in edge_array {
        a.edge_set_target(in_edge, Some(origin));
        a.edge_mut(in_edge).bend_points.add_last(KVector::new(x, y));

        // Check if a junction point should be added
        if add_junction_points {
            add_junction_point(a, in_edge, x, y);
        }
    }
    Ok(())
}

/// Reroutes the edges connected to the given output port back to the port it
/// was created for.
fn process_output_port(
    a: &mut LGraphArena,
    output_port: LPortId,
    add_junction_points: bool,
) -> Result<(), String> {
    // Retrieve the port the dummy node was created from
    let origin = origin_port(a, output_port)?;

    // Calculate the bend point
    let x = absolute_anchor_x(a, origin);
    let y = a.node(a.port(output_port).node.unwrap()).pos.y;

    // Reroute the edges, inserting a new bend point at the position of the dummy node
    let edge_array = a.port(output_port).outgoing_edges.clone();
    for out_edge in edge_array {
        a.edge_set_source(out_edge, Some(origin));
        a.edge_mut(out_edge).bend_points.add_first(KVector::new(x, y));

        // Check if a junction point should be added
        if add_junction_points {
            add_junction_point(a, out_edge, x, y);
        }
    }
    Ok(())
}

/// Reroutes and reconnects the self-loop edge represented by the given dummy.
fn process_self_loop(a: &mut LGraphArena, dummy: LNodeId) -> Result<(), String> {
    // Get the edge and the ports it was originally connected to
    let self_loop = match a.node(dummy).properties.try_get(&iprops::ORIGIN) {
        Some(Origin::LEdge(e)) => e,
        other => {
            return Err(format!(
                "north/south self-loop dummy without LEdge origin: {other:?}"
            ))
        }
    };
    let input_port = *a
        .node(dummy)
        .ports
        .iter()
        .find(|&&p| a.port(p).side == PortSide::WEST)
        .ok_or("north/south self-loop dummy without western port")?;
    let output_port = *a
        .node(dummy)
        .ports
        .iter()
        .find(|&&p| a.port(p).side == PortSide::EAST)
        .ok_or("north/south self-loop dummy without eastern port")?;
    let origin_input_port = origin_port(a, input_port)?;
    let origin_output_port = origin_port(a, output_port)?;

    // Reconnect the edge
    a.edge_set_source(self_loop, Some(origin_output_port));
    a.edge_set_target(self_loop, Some(origin_input_port));

    // Add two bend points
    let mut bend_point = a.node(a.port(output_port).node.unwrap()).pos;
    bend_point.x = absolute_anchor_x(a, origin_output_port);
    a.edge_mut(self_loop).bend_points.add_last(bend_point);

    let mut bend_point = a.node(a.port(input_port).node.unwrap()).pos;
    bend_point.x = absolute_anchor_x(a, origin_input_port);
    a.edge_mut(self_loop).bend_points.add_last(bend_point);

    Ok(())
}

/// Reroutes the edges connected to the given input port back to the port it
/// was created for (spline routing variant).
fn process_spline_input_port(a: &mut LGraphArena, input_port: LPortId) -> Result<(), String> {
    // Retrieve the port the dummy node was created from
    let origin = origin_port(a, input_port)?;
    let y = a.node(a.port(input_port).node.unwrap()).pos.y;
    a.port(origin).properties.set(&iprops::SPLINE_NS_PORT_Y_COORD, y);

    // Reroute the edges
    let edge_array = a.port(input_port).incoming_edges.clone();
    for in_edge in edge_array {
        a.edge_set_target(in_edge, Some(origin));
    }
    Ok(())
}

/// Reroutes the edges connected to the given output port back to the port it
/// was created for (spline routing variant).
fn process_spline_output_port(a: &mut LGraphArena, output_port: LPortId) -> Result<(), String> {
    // Retrieve the port the dummy node was created from
    let origin = origin_port(a, output_port)?;
    let y = a.node(a.port(output_port).node.unwrap()).pos.y;
    a.port(origin).properties.set(&iprops::SPLINE_NS_PORT_Y_COORD, y);

    // Reroute the edges
    let edge_array = a.port(output_port).outgoing_edges.clone();
    for out_edge in edge_array {
        a.edge_set_source(out_edge, Some(origin));
    }
    Ok(())
}
