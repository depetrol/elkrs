//! Inserts LONG_EDGE dummy nodes for edges
//! connected to input ports on the EAST side or output ports on the WEST
//! side (inverted ports), creating in-layer connections.

use crate::core::options::{EdgeLabelPlacement, PortConstraints, PortSide};

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let layers = a.graph(graph).layers.clone();

    // Iterate through the layers and for each layer create a list of dummy
    // nodes that were created, but not yet assigned to the layer
    let mut current_layer = None;
    let mut unassigned_nodes: Vec<LNodeId> = Vec::new();

    for layer in layers {
        // Update previous and current layers
        let previous_layer = current_layer;
        current_layer = Some(layer);

        // If the previous layer had unassigned nodes, assign them now and clear the list
        for node in unassigned_nodes.drain(..).collect::<Vec<_>>() {
            a.node_set_layer(node, previous_layer);
        }

        // Iterate through the layer's nodes
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            // Skip dummy nodes
            if a.node(node).node_type != NodeType::NORMAL {
                continue;
            }

            // Skip nodes whose port sides are not fixed
            if !a
                .node(node)
                .properties
                .get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
                .is_side_fixed()
            {
                continue;
            }

            // Look for input ports on the right side. The port list itself is
            // not modified here, so evaluating the predicates per port suffices.
            let ports = a.node(node).ports.clone();
            for &port in &ports {
                if a.port(port).side != PortSide::EAST || a.port(port).incoming_edges.is_empty() {
                    continue;
                }
                // Copy of the current list of edges
                let edge_array = a.port(port).incoming_edges.clone();
                for edge in edge_array {
                    create_east_port_side_dummies(a, graph, port, edge, &mut unassigned_nodes);
                }
            }

            // Look for output ports on the left side
            for &port in &ports {
                if a.port(port).side != PortSide::WEST || a.port(port).outgoing_edges.is_empty() {
                    continue;
                }
                let edge_array = a.port(port).outgoing_edges.clone();
                for edge in edge_array {
                    create_west_port_side_dummies(a, graph, port, edge, &mut unassigned_nodes);
                }
            }
        }
    }

    // There may be unassigned nodes left
    for node in unassigned_nodes {
        a.node_set_layer(node, current_layer);
    }

    Ok(())
}

/// Creates the necessary dummy nodes for an input port on the east side of a
/// node, provided that the edge connects two different nodes.
fn create_east_port_side_dummies(
    a: &mut LGraphArena,
    graph: LGraphId,
    eastward_port: LPortId,
    edge: LEdgeId,
    layer_node_list: &mut Vec<LNodeId>,
) {
    debug_assert_eq!(a.edge(edge).target, Some(eastward_port));

    // Ignore self loops
    if a.edge_source_node(edge) == a.port(eastward_port).node.unwrap() {
        return;
    }

    // Dummy node in the same layer
    let dummy = a.create_node(graph);
    a.node_mut(dummy).node_type = NodeType::LONG_EDGE;
    a.node(dummy).properties.set(&iprops::ORIGIN, Origin::LEdge(edge));
    a.node(dummy)
        .properties
        .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
    layer_node_list.push(dummy);

    let dummy_input = a.create_port();
    a.port_set_node(dummy_input, Some(dummy));
    a.port_set_side(dummy_input, PortSide::WEST);

    let dummy_output = a.create_port();
    a.port_set_node(dummy_output, Some(dummy));
    a.port_set_side(dummy_output, PortSide::EAST);

    // Reroute the original edge
    a.edge_set_target(edge, Some(dummy_input));

    // Connect the dummy with the original port
    let dummy_edge = a.create_edge();
    let edge_props = a.edge(edge).properties.clone();
    a.edge(dummy_edge).properties.copy_from(&edge_props);
    a.edge(dummy_edge).properties.unset(&lopts::JUNCTION_POINTS);
    a.edge_set_source(dummy_edge, Some(dummy_output));
    a.edge_set_target(dummy_edge, Some(eastward_port));

    // Set LONG_EDGE_SOURCE and LONG_EDGE_TARGET properties on the LONG_EDGE dummy
    set_long_edge_source_and_target(a, dummy, dummy_input, dummy_output);

    // Move head labels from the old edge over to the new one
    let labels = a.edge(edge).labels.clone();
    for label in labels {
        let label_placement: EdgeLabelPlacement =
            a.label(label).properties.get(&lopts::EDGE_LABELS_PLACEMENT);

        if label_placement == EdgeLabelPlacement::HEAD {
            // Remember which edge the label originally belonged to, unless it already knows
            if !a.label(label).properties.has(&iprops::END_LABEL_EDGE) {
                a.label(label).properties.set(&iprops::END_LABEL_EDGE, edge);
            }

            a.edge_mut(edge).labels.retain(|&l| l != label);
            a.edge_mut(dummy_edge).labels.push(label);
        }
    }
}

/// Creates the necessary dummy nodes for an output port on the west side of a
/// node, provided that the edge connects two different nodes.
fn create_west_port_side_dummies(
    a: &mut LGraphArena,
    graph: LGraphId,
    westward_port: LPortId,
    edge: LEdgeId,
    layer_node_list: &mut Vec<LNodeId>,
) {
    debug_assert_eq!(a.edge(edge).source, Some(westward_port));

    // Ignore self loops
    if a.edge_target_node(edge) == a.port(westward_port).node.unwrap() {
        return;
    }

    // Dummy node in the same layer
    let dummy = a.create_node(graph);
    a.node_mut(dummy).node_type = NodeType::LONG_EDGE;
    a.node(dummy).properties.set(&iprops::ORIGIN, Origin::LEdge(edge));
    a.node(dummy)
        .properties
        .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
    layer_node_list.push(dummy);

    let dummy_input = a.create_port();
    a.port_set_node(dummy_input, Some(dummy));
    a.port_set_side(dummy_input, PortSide::WEST);

    let dummy_output = a.create_port();
    a.port_set_node(dummy_output, Some(dummy));
    a.port_set_side(dummy_output, PortSide::EAST);

    // Reroute the original edge
    let original_target = a.edge(edge).target;
    a.edge_set_target(edge, Some(dummy_input));

    // Connect the dummy with the original port
    let dummy_edge = a.create_edge();
    let edge_props = a.edge(edge).properties.clone();
    a.edge(dummy_edge).properties.copy_from(&edge_props);
    a.edge(dummy_edge).properties.unset(&lopts::JUNCTION_POINTS);
    a.edge_set_source(dummy_edge, Some(dummy_output));
    a.edge_set_target(dummy_edge, original_target);

    // Move any head labels over to the new dummy edge
    let labels = a.edge(edge).labels.clone();
    for label in labels {
        let label_placement: EdgeLabelPlacement =
            a.label(label).properties.get(&lopts::EDGE_LABELS_PLACEMENT);

        if label_placement == EdgeLabelPlacement::HEAD {
            // Remember which edge the label belonged to originally
            debug_assert!(!a.label(label).properties.has(&iprops::END_LABEL_EDGE));
            a.label(label).properties.set(&iprops::END_LABEL_EDGE, edge);

            a.edge_mut(edge).labels.retain(|&l| l != label);
            a.edge_mut(dummy_edge).labels.push(label);
        }
    }

    // Set LONG_EDGE_SOURCE and LONG_EDGE_TARGET properties on the LONG_EDGE dummy
    set_long_edge_source_and_target(a, dummy, dummy_input, dummy_output);
}

/// Properly sets the LONG_EDGE_SOURCE and LONG_EDGE_TARGET properties for the
/// given long edge dummy (required for the HyperedgeDummyMerger).
fn set_long_edge_source_and_target(
    a: &mut LGraphArena,
    long_edge_dummy: LNodeId,
    dummy_input_port: LPortId,
    dummy_output_port: LPortId,
) {
    // There's exactly one edge connected to the input and output port
    let source_port = a
        .edge(a.port(dummy_input_port).incoming_edges[0])
        .source
        .unwrap();
    let source_node = a.port(source_port).node.unwrap();
    let source_node_type = a.node(source_node).node_type;

    let target_port = a
        .edge(a.port(dummy_output_port).outgoing_edges[0])
        .target
        .unwrap();
    let target_node = a.port(target_port).node.unwrap();
    let target_node_type = a.node(target_node).node_type;

    // Set the LONG_EDGE_SOURCE property
    if source_node_type == NodeType::LONG_EDGE {
        // The source is a LONG_EDGE node; use its LONG_EDGE_SOURCE
        // (an absent source removes the entry when unset)
        match a.node(source_node).properties.try_get(&iprops::LONG_EDGE_SOURCE) {
            Some(s) => {
                a.node(long_edge_dummy).properties.set(&iprops::LONG_EDGE_SOURCE, s);
            }
            None => {
                a.node(long_edge_dummy).properties.unset(&iprops::LONG_EDGE_SOURCE);
            }
        }
    } else {
        // The target is the original node; use it
        a.node(long_edge_dummy)
            .properties
            .set(&iprops::LONG_EDGE_SOURCE, source_port);
    }

    // Set the LONG_EDGE_TARGET property
    if target_node_type == NodeType::LONG_EDGE {
        // The target is a LONG_EDGE node; use its LONG_EDGE_TARGET
        match a.node(target_node).properties.try_get(&iprops::LONG_EDGE_TARGET) {
            Some(t) => {
                a.node(long_edge_dummy).properties.set(&iprops::LONG_EDGE_TARGET, t);
            }
            None => {
                a.node(long_edge_dummy).properties.unset(&iprops::LONG_EDGE_TARGET);
            }
        }
    } else {
        // The target is the original node; use it
        a.node(long_edge_dummy)
            .properties
            .set(&iprops::LONG_EDGE_TARGET, target_port);
    }
}
