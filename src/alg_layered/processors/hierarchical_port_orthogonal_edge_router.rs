//! Routes edges connected to
//! hierarchical ports and fixes external port dummy coordinates.

use crate::core::javacompat::JavaRandom;
use crate::core::options::{PortConstraints, PortSide, SizeConstraint};
use crate::graph::math::{KVector, KVectorChain};
use crate::graph::properties::EnumSet;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::p5edges::direction::RoutingDirection;
use crate::alg_layered::p5edges::orthogonal_routing_generator::OrthogonalRoutingGenerator;

pub fn process(
    a: &mut LGraphArena,
    graph: LGraphId,
    random: &mut JavaRandom,
) -> Result<(), String> {
    // Step 1: restore north/south dummies.
    let north_south_dummies = restore_north_south_dummies(a, graph)?;

    // Step 2: north/south dummy coordinates.
    set_north_south_dummy_coordinates(a, graph, &north_south_dummies);

    // Step 3: orthogonal edge routing.
    route_edges(a, graph, &north_south_dummies, random);

    // Step 4: remove temporary north/south dummies.
    remove_temporary_north_south_dummies(a, graph);

    // Step 5: fix east/west and north/south dummy coordinates.
    fix_coordinates(a, graph);

    // Step 6: correct slanted edge segments.
    correct_slanted_edge_segments(a, graph);

    Ok(())
}

// ===========================================================================
// STEP 1
// ===========================================================================

fn restore_north_south_dummies(
    a: &mut LGraphArena,
    graph: LGraphId,
) -> Result<Vec<LNodeId>, String> {
    let mut restored: Vec<LNodeId> = Vec::new();

    if !a.graph(graph).properties.has(&iprops::EXT_PORT_REPLACED_DUMMIES) {
        return Ok(restored);
    }

    let dummies: Vec<LNodeId> = a.graph(graph).properties.get(&iprops::EXT_PORT_REPLACED_DUMMIES);
    for dummy in dummies {
        restore_dummy(a, dummy, graph)?;
        restored.push(dummy);
    }

    // Look for temporary dummies (replaced the restored dummies) and connect
    // them to the restored ones.
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            if a.node(node).node_type != NodeType::EXTERNAL_PORT {
                continue;
            }
            if let Some(replaced) = a
                .node(node)
                .properties
                .get_opt::<LNodeId>(&iprops::EXT_PORT_REPLACED_DUMMY)
            {
                connect_node_to_dummy(a, node, replaced);
            }
        }
    }

    // Assign restored dummies to the last layer.
    let last_layer = *a.graph(graph).layers.last().unwrap();
    for &dummy in &restored {
        a.node_set_layer(dummy, Some(last_layer));
    }

    Ok(restored)
}

fn restore_dummy(a: &mut LGraphArena, dummy: LNodeId, graph: LGraphId) -> Result<(), String> {
    let port_side: PortSide = a.node(dummy).properties.get(&iprops::EXT_PORT_SIDE);
    let dummy_port = a.node(dummy).ports[0];
    if port_side == PortSide::NORTH {
        a.port_set_side(dummy_port, PortSide::SOUTH);
    } else if port_side == PortSide::SOUTH {
        a.port_set_side(dummy_port, PortSide::NORTH);
    }

    let size_constraints: EnumSet<SizeConstraint> =
        a.graph(graph).properties.get(&lopts::NODE_SIZE_CONSTRAINTS);
    if size_constraints.contains(SizeConstraint::PORT_LABELS) {
        return Err(
            "TODO: hierarchical north/south port dummy label margins (PORT_LABELS) not ported yet"
                .to_string(),
        );
    }
    Ok(())
}

fn connect_node_to_dummy(a: &mut LGraphArena, node: LNodeId, dummy: LNodeId) {
    let out_port = a.create_port();
    a.port_set_node(out_port, Some(node));
    let ext_port_side: PortSide = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
    a.port_set_side(out_port, ext_port_side);

    let in_port = a.node(dummy).ports[0];

    let edge = a.create_edge();
    a.edge_set_source(edge, Some(out_port));
    a.edge_set_target(edge, Some(in_port));
}

// ===========================================================================
// STEP 2
// ===========================================================================

fn set_north_south_dummy_coordinates(
    a: &mut LGraphArena,
    graph: LGraphId,
    north_south_dummies: &[LNodeId],
) {
    let constraints: PortConstraints = a.graph(graph).properties.get(&lopts::PORT_CONSTRAINTS);
    let graph_size = a.graph(graph).size;
    let padding = a.graph(graph).padding;
    let offset = a.graph(graph).offset;
    let graph_width = graph_size.x + padding.left + padding.right;
    let north_y = 0.0 - padding.top - offset.y;
    let south_y = graph_size.y + padding.top + padding.bottom - offset.y;

    let mut northern: Vec<LNodeId> = Vec::new();
    let mut southern: Vec<LNodeId> = Vec::new();

    for &dummy in north_south_dummies {
        match constraints {
            PortConstraints::FREE | PortConstraints::FIXED_SIDE | PortConstraints::FIXED_ORDER => {
                calculate_north_south_dummy_positions(a, dummy);
            }
            PortConstraints::FIXED_RATIO => {
                apply_north_south_dummy_ratio(a, dummy, graph_width);
                a.node_border_to_content_area_coordinates(dummy, true, false);
            }
            PortConstraints::FIXED_POS => {
                apply_north_south_dummy_position(a, dummy);
                a.node_border_to_content_area_coordinates(dummy, true, false);
                let new_x = a.node(dummy).pos.x + a.node(dummy).size.x / 2.0;
                a.graph_mut(graph).size.x = graph_size.x.max(new_x);
            }
            PortConstraints::UNDEFINED => {}
        }

        let side: PortSide = a.node(dummy).properties.get(&iprops::EXT_PORT_SIDE);
        match side {
            PortSide::NORTH => {
                a.node_mut(dummy).pos.y = north_y;
                northern.push(dummy);
            }
            PortSide::SOUTH => {
                a.node_mut(dummy).pos.y = south_y;
                southern.push(dummy);
            }
            _ => {}
        }
    }

    match constraints {
        PortConstraints::FREE | PortConstraints::FIXED_SIDE => {
            ensure_unique_positions(a, &northern, graph);
            ensure_unique_positions(a, &southern, graph);
        }
        PortConstraints::FIXED_ORDER => {
            restore_proper_order(a, &northern, graph);
            restore_proper_order(a, &southern, graph);
        }
        _ => {}
    }
}

fn calculate_north_south_dummy_positions(a: &mut LGraphArena, dummy: LNodeId) {
    let dummy_in_port = a.node(dummy).ports[0];
    let connected = a.port_connected_edges(dummy_in_port);
    if connected.is_empty() {
        a.node_mut(dummy).pos.x = 0.0;
        return;
    }
    let mut pos_sum = 0.0;
    let degree = connected.len();
    for edge in &connected {
        // the connected port (the other end)
        let src = a.edge(*edge).source.unwrap();
        let tgt = a.edge(*edge).target.unwrap();
        let connected_port = if a.port(src).node == Some(dummy) { tgt } else { src };
        let node = a.port(connected_port).node.unwrap();
        pos_sum += a.node(node).pos.x + a.port(connected_port).pos.x + a.port(connected_port).anchor.x;
    }
    let anchor = a.node(dummy).properties.get::<KVector>(&lopts::PORT_ANCHOR);
    a.node_mut(dummy).pos.x = pos_sum / degree as f64 - anchor.x;
}

fn apply_north_south_dummy_ratio(a: &mut LGraphArena, dummy: LNodeId, width: f64) {
    let anchor = a.node(dummy).properties.get::<KVector>(&lopts::PORT_ANCHOR);
    let ratio: f64 = a.node(dummy).properties.get(&iprops::PORT_RATIO_OR_POSITION);
    a.node_mut(dummy).pos.x = width * ratio - anchor.x;
}

fn apply_north_south_dummy_position(a: &mut LGraphArena, dummy: LNodeId) {
    let anchor = a.node(dummy).properties.get::<KVector>(&lopts::PORT_ANCHOR);
    let pos: f64 = a.node(dummy).properties.get(&iprops::PORT_RATIO_OR_POSITION);
    a.node_mut(dummy).pos.x = pos - anchor.x;
}

fn ensure_unique_positions(a: &mut LGraphArena, dummies: &[LNodeId], graph: LGraphId) {
    if dummies.is_empty() {
        return;
    }
    let mut arr = dummies.to_vec();
    arr.sort_by(|&x, &y| {
        a.node(x)
            .pos
            .x
            .partial_cmp(&a.node(y).pos.x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    assign_ascending_coordinates(a, &arr, graph);
}

fn restore_proper_order(a: &mut LGraphArena, dummies: &[LNodeId], graph: LGraphId) {
    if dummies.is_empty() {
        return;
    }
    let mut arr = dummies.to_vec();
    arr.sort_by(|&x, &y| {
        let px: f64 = a.node(x).properties.get(&iprops::PORT_RATIO_OR_POSITION);
        let py: f64 = a.node(y).properties.get(&iprops::PORT_RATIO_OR_POSITION);
        px.partial_cmp(&py).unwrap_or(std::cmp::Ordering::Equal)
    });
    assign_ascending_coordinates(a, &arr, graph);
}

fn assign_ascending_coordinates(a: &mut LGraphArena, dummies: &[LNodeId], graph: LGraphId) {
    let spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_PORT_PORT);
    let mut next_valid = a.node(dummies[0]).pos.x
        + a.node(dummies[0]).size.x
        + a.node(dummies[0]).margin.right
        + spacing;

    for &dummy in &dummies[1..] {
        let pos_x = a.node(dummy).pos.x;
        let size_x = a.node(dummy).size.x;
        let margin_left = a.node(dummy).margin.left;
        let margin_right = a.node(dummy).margin.right;

        let delta = pos_x - margin_left - next_valid;
        let new_pos_x = if delta < 0.0 { pos_x - delta } else { pos_x };
        a.node_mut(dummy).pos.x = new_pos_x;

        let graph_size_x = a.graph(graph).size.x;
        a.graph_mut(graph).size.x = graph_size_x.max(new_pos_x + size_x);

        next_valid = new_pos_x + size_x + margin_right + spacing;
    }
}

// ===========================================================================
// STEP 3
// ===========================================================================

fn route_edges(
    a: &mut LGraphArena,
    graph: LGraphId,
    north_south_dummies: &[LNodeId],
    random: &mut JavaRandom,
) {
    let mut northern_source: Vec<LNodeId> = Vec::new();
    let mut northern_target: Vec<LNodeId> = Vec::new();
    let mut southern_source: Vec<LNodeId> = Vec::new();
    let mut southern_target: Vec<LNodeId> = Vec::new();

    let node_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_NODE_NODE);
    let edge_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_EDGE);

    for &dummy in north_south_dummies {
        let side: PortSide = a.node(dummy).properties.get(&iprops::EXT_PORT_SIDE);
        if side == PortSide::NORTH {
            push_unique(&mut northern_target, dummy);
            for edge in a.node_incoming_edges(dummy) {
                push_unique(&mut northern_source, a.edge_source_node(edge));
            }
        } else if side == PortSide::SOUTH {
            push_unique(&mut southern_target, dummy);
            for edge in a.node_incoming_edges(dummy) {
                push_unique(&mut southern_source, a.edge_source_node(edge));
            }
        }
    }

    if !northern_source.is_empty() {
        let mut generator = OrthogonalRoutingGenerator::new(
            RoutingDirection::SouthToNorth,
            edge_spacing,
            "extnorth",
        );
        let offset_y = a.graph(graph).offset.y;
        let slots = generator.route_edges(
            a,
            Some(&northern_source),
            0,
            Some(&northern_target),
            -node_spacing - offset_y,
            random,
        );
        if slots > 0 {
            let height = node_spacing + (slots as f64 - 1.0) * edge_spacing;
            a.graph_mut(graph).offset.y += height;
            a.graph_mut(graph).size.y += height;
        }
    }

    if !southern_source.is_empty() {
        let mut generator = OrthogonalRoutingGenerator::new(
            RoutingDirection::NorthToSouth,
            edge_spacing,
            "extsouth",
        );
        let start = a.graph(graph).size.y + node_spacing - a.graph(graph).offset.y;
        let slots = generator.route_edges(
            a,
            Some(&southern_source),
            0,
            Some(&southern_target),
            start,
            random,
        );
        if slots > 0 {
            a.graph_mut(graph).size.y += node_spacing + (slots as f64 - 1.0) * edge_spacing;
        }
    }
}

fn push_unique(v: &mut Vec<LNodeId>, n: LNodeId) {
    if !v.contains(&n) {
        v.push(n);
    }
}

// ===========================================================================
// STEP 4
// ===========================================================================

fn remove_temporary_north_south_dummies(a: &mut LGraphArena, graph: LGraphId) {
    let mut to_remove: Vec<LNodeId> = Vec::new();

    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            if a.node(node).node_type != NodeType::EXTERNAL_PORT {
                continue;
            }
            if !a.node(node).properties.has(&iprops::EXT_PORT_REPLACED_DUMMY) {
                continue;
            }

            let mut node_in_port: Option<LPortId> = None;
            let mut node_out_port: Option<LPortId> = None;
            let mut node_origin_port: Option<LPortId> = None;
            for &port in &a.node(node).ports {
                match a.port(port).side {
                    PortSide::WEST => node_in_port = Some(port),
                    PortSide::EAST => node_out_port = Some(port),
                    _ => node_origin_port = Some(port),
                }
            }
            let node_in_port = node_in_port.unwrap();
            let node_out_port = node_out_port.unwrap();
            let node_origin_port = node_origin_port.unwrap();

            let node_to_origin_edge = a.port(node_origin_port).outgoing_edges[0];

            // incoming edge bend points
            let mut incoming_bps = a.edge(node_to_origin_edge).bend_points.clone();
            let mut first_bp = a.port(node_origin_port).pos;
            first_bp.add(a.node(node).pos);
            incoming_bps.0.insert(0, first_bp);

            // outgoing edge bend points
            let mut outgoing_bps = KVectorChain::reverse(&a.edge(node_to_origin_edge).bend_points);
            let mut last_bp = a.port(node_origin_port).pos;
            last_bp.add(a.node(node).pos);
            outgoing_bps.0.push(last_bp);

            let replaced_dummy: LNodeId =
                a.node(node).properties.try_get(&iprops::EXT_PORT_REPLACED_DUMMY).unwrap();
            let replaced_dummy_port = a.node(replaced_dummy).ports[0];

            // reroute the input port's edges
            let in_edges = a.port(node_in_port).incoming_edges.clone();
            for edge in in_edges {
                a.edge_set_target(edge, Some(replaced_dummy_port));
                a.edge_mut(edge).bend_points.0.extend(incoming_bps.0.iter().copied());
            }

            // reroute the output port's edges
            let out_edges = a.port(node_out_port).outgoing_edges.clone();
            for edge in out_edges {
                a.edge_set_source(edge, Some(replaced_dummy_port));
                let mut new_bps = outgoing_bps.clone();
                new_bps.0.extend(a.edge(edge).bend_points.0.iter().copied());
                a.edge_mut(edge).bend_points = new_bps;
            }

            // disconnect node-to-origin edge
            a.edge_set_source(node_to_origin_edge, None);
            a.edge_set_target(node_to_origin_edge, None);

            to_remove.push(node);
        }
    }

    for node in to_remove {
        a.node_set_layer(node, None);
    }
}

// ===========================================================================
// STEP 5
// ===========================================================================

fn fix_coordinates(a: &mut LGraphArena, graph: LGraphId) {
    let constraints: PortConstraints = a.graph(graph).properties.get(&lopts::PORT_CONSTRAINTS);
    let layers = a.graph(graph).layers.clone();
    fix_coordinates_layer(a, layers[0], constraints, graph);
    fix_coordinates_layer(a, layers[layers.len() - 1], constraints, graph);
}

fn fix_coordinates_layer(
    a: &mut LGraphArena,
    layer: crate::alg_layered::graph::LayerId,
    constraints: PortConstraints,
    graph: LGraphId,
) {
    let padding = a.graph(graph).padding;
    let offset = a.graph(graph).offset;
    let graph_actual_size = a.graph_actual_size(graph);

    let mut new_actual_height = graph_actual_size.y;

    let nodes = a.layer(layer).nodes.clone();
    for node in &nodes {
        let node = *node;
        if a.node(node).node_type != NodeType::EXTERNAL_PORT {
            continue;
        }
        let ext_port_side: PortSide = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
        let ext_port_size: KVector = a.node(node).properties.get(&iprops::EXT_PORT_SIZE);

        match ext_port_side {
            PortSide::EAST => {
                a.node_mut(node).pos.x = a.graph(graph).size.x + padding.right - offset.x;
            }
            PortSide::WEST => {
                a.node_mut(node).pos.x = -offset.x - padding.left;
            }
            _ => {}
        }

        let mut required_height = 0.0;
        if ext_port_side == PortSide::EAST || ext_port_side == PortSide::WEST {
            if constraints == PortConstraints::FIXED_RATIO {
                let ratio: f64 = a.node(node).properties.get(&iprops::PORT_RATIO_OR_POSITION);
                let anchor_y = a.node(node).properties.get::<KVector>(&lopts::PORT_ANCHOR).y;
                a.node_mut(node).pos.y = graph_actual_size.y * ratio - anchor_y;
                required_height = a.node(node).pos.y + ext_port_size.y;
                a.node_border_to_content_area_coordinates(node, false, true);
            } else if constraints == PortConstraints::FIXED_POS {
                let pos: f64 = a.node(node).properties.get(&iprops::PORT_RATIO_OR_POSITION);
                let anchor_y = a.node(node).properties.get::<KVector>(&lopts::PORT_ANCHOR).y;
                a.node_mut(node).pos.y = pos - anchor_y;
                required_height = a.node(node).pos.y + ext_port_size.y;
                a.node_border_to_content_area_coordinates(node, false, true);
            }
        }

        new_actual_height = new_actual_height.max(required_height);
    }

    let grow = new_actual_height - graph_actual_size.y;
    a.graph_mut(graph).size.y += grow;

    // second pass: north/south after height fixed
    for node in &nodes {
        let node = *node;
        if a.node(node).node_type != NodeType::EXTERNAL_PORT {
            continue;
        }
        let ext_port_side: PortSide = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
        match ext_port_side {
            PortSide::NORTH => {
                a.node_mut(node).pos.y = -offset.y - padding.top;
            }
            PortSide::SOUTH => {
                a.node_mut(node).pos.y = a.graph(graph).size.y + padding.bottom - offset.y;
            }
            _ => {}
        }
    }
}

// ===========================================================================
// STEP 6
// ===========================================================================

fn correct_slanted_edge_segments(a: &mut LGraphArena, graph: LGraphId) {
    let layers = a.graph(graph).layers.clone();
    correct_slanted_layer(a, layers[0]);
    correct_slanted_layer(a, layers[layers.len() - 1]);
}

fn correct_slanted_layer(a: &mut LGraphArena, layer: crate::alg_layered::graph::LayerId) {
    let nodes = a.layer(layer).nodes.clone();
    for node in nodes {
        if a.node(node).node_type != NodeType::EXTERNAL_PORT {
            continue;
        }
        let ext_port_side: PortSide = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
        if ext_port_side != PortSide::EAST && ext_port_side != PortSide::WEST {
            continue;
        }
        let edges = a.node_connected_edges(node);
        for edge in edges {
            if a.edge(edge).bend_points.is_empty() {
                continue;
            }
            let source_port = a.edge(edge).source.unwrap();
            if a.port(source_port).node == Some(node) {
                let y = port_absolute_anchor(a, source_port).y;
                a.edge_mut(edge).bend_points.0[0].y = y;
            }
            let target_port = a.edge(edge).target.unwrap();
            if a.port(target_port).node == Some(node) {
                let y = port_absolute_anchor(a, target_port).y;
                let last = a.edge(edge).bend_points.0.len() - 1;
                a.edge_mut(edge).bend_points.0[last].y = y;
            }
        }
    }
}

fn port_absolute_anchor(a: &LGraphArena, port: LPortId) -> KVector {
    let p = a.port(port);
    let node = p.node.unwrap();
    let n = a.node(node);
    KVector::new(n.pos.x + p.pos.x + p.anchor.x, n.pos.y + p.pos.y + p.anchor.y)
}
