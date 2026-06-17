//!
//! Edge router module that draws edges with non-orthogonal line segments.

use crate::core::options::PortSide;
use crate::graph::math::{KVector, KVectorChain};

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LPortId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;

/// the minimal vertical difference for creating bend points.
const MIN_VERT_DIFF: f64 = 1.0;
/// factor for spacing apart layers between which edges are routed.
const LAYER_SPACE_FAC: f64 = 0.4;

pub(crate) fn is_external_west_or_east_port(a: &LGraphArena, node: LNodeId) -> bool {
    let ext_port_side: PortSide = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
    a.node(node).node_type == NodeType::EXTERNAL_PORT
        && (ext_port_side == PortSide::WEST || ext_port_side == PortSide::EAST)
}

/// Absolute anchor position of a port.
fn abs_anchor(a: &LGraphArena, port: LPortId) -> KVector {
    let p = a.port(port);
    let n = a.node(p.node.unwrap());
    KVector::new(n.pos.x + p.pos.x + p.anchor.x, n.pos.y + p.pos.y + p.anchor.y)
}

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let sloped_edge_zone_width: f64 = a
        .graph(graph)
        .properties
        .get(&lopts::EDGE_ROUTING_POLYLINE_SLOPED_EDGE_ZONE_WIDTH);
    let node_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_NODE_NODE_BETWEEN_LAYERS);
    let edge_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_EDGE_BETWEEN_LAYERS);
    let edge_space_fac = f64::min(1.0, edge_spacing / node_spacing);

    // Set of already created junction points, to avoid multiple points at the
    // same position (queried by value equality).
    let mut created_junction_points: Vec<KVector> = Vec::new();

    let mut xpos = 0.0f64;
    let mut layer_spacing;

    let layers = a.graph(graph).layers.clone();

    // Determine the horizontal spacing required to route west-side in-layer edges of the
    // first layer
    if !layers.is_empty() {
        let y_diff = calculate_west_in_layer_edge_y_diff(a, layers[0]);
        xpos = LAYER_SPACE_FAC * edge_space_fac * y_diff;
    }

    // Iterate over the layers
    for (layer_index, &layer) in layers.iter().enumerate() {
        let has_next = layer_index + 1 < layers.len();
        let external_layer = a
            .layer(layer)
            .nodes
            .iter()
            .all(|&node| is_external_west_or_east_port(a, node));

        // The rightmost layer is not given any node spacing if it's an external port layer
        if external_layer && xpos > 0.0 {
            xpos -= node_spacing;
        }

        // Set horizontal coordinates for all nodes of the layer
        super::orthogonal::place_nodes_horizontally(a, layer, xpos);

        // While routing edges, we remember the maximum vertical span of any edge between
        // this and the next layer
        let mut max_vert_diff = 0.0f64;

        // Iterate over the layer's nodes
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            // Calculate the maximal vertical span of output edges. In-layer edges will
            // also be routed at this point
            let mut max_curr_output_y_diff = 0.0f64;
            for outgoing_edge in a.node_outgoing_edges(node) {
                let mut source_pos = abs_anchor(a, a.edge(outgoing_edge).source.unwrap()).y;
                let mut target_pos = abs_anchor(a, a.edge(outgoing_edge).target.unwrap()).y;

                let target_node = a.port(a.edge(outgoing_edge).target.unwrap()).node.unwrap();
                if Some(layer) == a.node(target_node).layer && !a.edge_is_self_loop(outgoing_edge) {
                    // In-layer edges require an extra bend point to make them look nice
                    process_in_layer_edge(
                        a,
                        outgoing_edge,
                        xpos,
                        LAYER_SPACE_FAC * edge_space_fac * (source_pos - target_pos).abs(),
                    );

                    if a.port(a.edge(outgoing_edge).source.unwrap()).side == PortSide::WEST {
                        // The spacing required for routing in-layer edges on the west side
                        // doesn't contribute anything to the spacing required between this
                        // and the next layer and was already taken into account previously
                        source_pos = 0.0;
                        target_pos = 0.0;
                    }
                }

                max_curr_output_y_diff =
                    f64::max(max_curr_output_y_diff, (target_pos - source_pos).abs());
            }

            // We currently only handle certain node types
            match a.node(node).node_type {
                NodeType::NORMAL
                | NodeType::LABEL
                | NodeType::LONG_EDGE
                | NodeType::NORTH_SOUTH_PORT
                | NodeType::BREAKING_POINT => {
                    process_node(a, node, xpos, sloped_edge_zone_width, &mut created_junction_points);
                }
                _ => {}
            }

            max_vert_diff = f64::max(max_vert_diff, max_curr_output_y_diff);
        }

        // Consider the span of west-side in-layer edges of the next layer
        if has_next {
            let y_diff = calculate_west_in_layer_edge_y_diff(a, layers[layer_index + 1]);
            max_vert_diff = f64::max(max_vert_diff, y_diff);
        }

        // Determine where next layer should start based on the maximal vertical span of
        // edges between the two layers
        layer_spacing = LAYER_SPACE_FAC * edge_space_fac * max_vert_diff;
        if !external_layer && has_next {
            layer_spacing += node_spacing;
        }

        xpos += a.layer(layer).size.x + layer_spacing;
    }

    // Set the graph's horizontal size
    a.graph_mut(graph).size.x = xpos;

    Ok(())
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Actual Edge Routing Code

/// Inserts bend points for edges incident to this node.
fn process_node(
    a: &mut LGraphArena,
    node: LNodeId,
    layer_left_x_pos: f64,
    max_acceptable_x_diff: f64,
    created_junction_points: &mut Vec<KVector>,
) {
    // The right side of the layer
    let layer_right_x_pos =
        layer_left_x_pos + a.layer(a.node(node).layer.unwrap()).size.x;

    let ports = a.node(node).ports.clone();
    for port in ports {
        let mut absolute_port_anchor = abs_anchor(a, port);

        if a.node(node).node_type == NodeType::NORTH_SOUTH_PORT {
            // North/south ports require special handling (also see #515): use the
            // north/south port's x-coordinate instead of the dummy node's one.
            let corresponding_port = match a.port(port).properties.try_get(&iprops::ORIGIN) {
                Some(Origin::LPort(p)) => p,
                other => panic!("north/south port dummy port without LPort origin: {other:?}"),
            };
            absolute_port_anchor.x = abs_anchor(a, corresponding_port).x;
            // It is important to move the dummy node to the correct location as well.
            a.node_mut(node).pos.x = absolute_port_anchor.x;
        }

        let mut bend_point = KVector::new(0.0, absolute_port_anchor.y);

        match a.port(port).side {
            PortSide::EAST => bend_point.x = layer_right_x_pos,
            PortSide::WEST => bend_point.x = layer_left_x_pos,
            // We only know what to do with eastern and western ports
            _ => continue,
        }

        // If the port's absolute anchor equals the bend point, we don't want to insert
        // anything (unless the node represents an in-layer dummy)
        let x_distance = (absolute_port_anchor.x - bend_point.x).abs();
        if x_distance <= max_acceptable_x_diff && !is_in_layer_dummy(a, node) {
            continue;
        }

        // Whether to add a junction point or not
        let add_junction_point =
            a.port(port).outgoing_edges.len() + a.port(port).incoming_edges.len() > 1;

        // Iterate over the edges and add bend (and possibly junction) points
        for e in a.port_connected_edges(port) {
            let other_port = if a.edge(e).source == Some(port) {
                a.edge(e).target.unwrap()
            } else {
                a.edge(e).source.unwrap()
            };
            if (abs_anchor(a, other_port).y - bend_point.y).abs() > MIN_VERT_DIFF {
                // Insert bend point
                add_bend_point(a, e, bend_point, add_junction_point, port, created_junction_points);
            }
        }
    }
}

/// In-layer edges get an extra bend point halfway
/// between the edge's upper and lower end.
fn process_in_layer_edge(a: &mut LGraphArena, edge: LEdgeId, layer_x_pos: f64, edge_spacing: f64) {
    let source_port = a.edge(edge).source.unwrap();
    let target_port = a.edge(edge).target.unwrap();

    let source_anchor_y = abs_anchor(a, source_port).y;
    let mid_y = (source_anchor_y + abs_anchor(a, target_port).y) / 2.0;

    let bend_point = if a.port(source_port).side == PortSide::EAST {
        let source_node = a.port(source_port).node.unwrap();
        let layer_size_x = a.layer(a.node(source_node).layer.unwrap()).size.x;
        KVector::new(layer_x_pos + layer_size_x + edge_spacing, mid_y)
    } else {
        KVector::new(layer_x_pos - edge_spacing, mid_y)
    };

    a.edge_mut(edge).bend_points.0.insert(0, bend_point);
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Utility Methods

fn calculate_west_in_layer_edge_y_diff(a: &LGraphArena, layer: LayerId) -> f64 {
    let mut max_y_diff = 0.0f64;

    for &node in &a.layer(layer).nodes {
        for outgoing_edge in a.node_outgoing_edges(node) {
            let target_node = a.port(a.edge(outgoing_edge).target.unwrap()).node.unwrap();
            if Some(layer) == a.node(target_node).layer
                && a.port(a.edge(outgoing_edge).source.unwrap()).side == PortSide::WEST
            {
                let source_pos = abs_anchor(a, a.edge(outgoing_edge).source.unwrap()).y;
                let target_pos = abs_anchor(a, a.edge(outgoing_edge).target.unwrap()).y;
                max_y_diff = f64::max(max_y_diff, (target_pos - source_pos).abs());
            }
        }
    }

    max_y_diff
}

fn add_bend_point(
    a: &mut LGraphArena,
    edge: LEdgeId,
    bend_point: KVector,
    add_junction_point: bool,
    curr_port: LPortId,
    created_junction_points: &mut Vec<KVector>,
) {
    // Only insert the bend point if necessary; for in-layer edges we are extra save and
    // add the bend point in any case
    if (a.edge_is_in_layer(edge) || abs_anchor(a, curr_port) != bend_point)
        && !a.edge_is_self_loop(edge)
    {
        if a.edge(edge).source == Some(curr_port) {
            a.edge_mut(edge).bend_points.0.insert(0, bend_point);
        } else {
            a.edge_mut(edge).bend_points.0.push(bend_point);
        }

        if add_junction_point && !created_junction_points.contains(&bend_point) {
            // create a new junction point for the edge at the bend point's position
            // (the JUNCTION_POINTS default is materialized on access)
            let mut junction_points: KVectorChain =
                a.edge(edge).properties.get(&lopts::JUNCTION_POINTS);
            junction_points.add_last(bend_point);
            a.edge(edge).properties.set(&lopts::JUNCTION_POINTS, junction_points);
            created_junction_points.push(bend_point);
        }
    }
}

/// A node is considered an in-layer dummy if it is of
/// type `LONG_EDGE` and has an incident in-layer edge.
fn is_in_layer_dummy(a: &LGraphArena, node: LNodeId) -> bool {
    if a.node(node).node_type == NodeType::LONG_EDGE {
        for e in a.node_connected_edges(node) {
            if a.edge_is_in_layer(e) {
                return true;
            }
        }
    }
    false
}
