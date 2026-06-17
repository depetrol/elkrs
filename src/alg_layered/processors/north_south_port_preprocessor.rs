//! Inserts NORTH_SOUTH_PORT dummy
//! nodes for ports on the northern and southern node sides, including the
//! special self-loop cases, and sets layout units, in-layer successor
//! constraints and barycenter associates.

use std::cmp::Ordering;

use crate::core::options::{PortConstraints, PortSide};

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::OrderingStrategy;

/// `USE_NEW_APPROACH = true`: the dummy nodes' order is not fixed at this
/// point; only the relation between dummies and their regular node is
/// constrained. (The old approach's code paths are dead and are not ported.)
const USE_NEW_APPROACH: bool = true;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let _ = USE_NEW_APPROACH;

    // Iterate through the layers
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        // The pointer indicates the index of the current node while northern ports are
        // processed, and the index of the most recently inserted dummy while south
        // ports are processed
        let mut pointer: i32 = -1;

        // Iterate through the nodes (use an array to avoid concurrent modification)
        let node_array = a.layer(layer).nodes.clone();
        for node in node_array {
            pointer += 1;

            // We only care about non-dummy nodes with fixed port sides
            if !(a.node(node).node_type == NodeType::NORMAL
                && a
                    .node(node)
                    .properties
                    .get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
                    .is_side_fixed())
            {
                continue;
            }

            let node_graph = a.node_graph(node);
            let ordering_strategy: OrderingStrategy = a
                .graph(node_graph)
                .properties
                .get(&lopts::CONSIDER_MODEL_ORDER_STRATEGY);

            // Sort the port list if we have control over the port order
            if !a
                .node(node)
                .properties
                .get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
                .is_order_fixed()
                && ordering_strategy == OrderingStrategy::NONE
            {
                sort_port_list(a, node);
            }

            // Nodes form their own layout unit
            a.node(node).properties.set(&iprops::IN_LAYER_LAYOUT_UNIT, node);

            // The lists of northern and southern dummy nodes
            let mut north_dummy_nodes: Vec<LNodeId> = Vec::new();
            let mut south_dummy_nodes: Vec<LNodeId> = Vec::new();

            // Create a list of barycenter associates for the node
            let mut barycenter_associates: Vec<LNodeId> = Vec::new();

            // Prepare a list of ports on the northern side, sorted from left to right
            // (when viewed in the diagram); create the appropriate dummy nodes and
            // assign them to the layer
            let mut port_list: Vec<LPortId> = a
                .node(node)
                .ports
                .iter()
                .copied()
                .filter(|&p| a.port(p).side == PortSide::NORTH)
                .collect();

            if ordering_strategy != OrderingStrategy::NONE {
                port_list = model_order_north_south_input_reversing(a, port_list);
            }

            create_dummy_nodes(
                a,
                graph,
                &port_list,
                &mut north_dummy_nodes,
                Some(&mut south_dummy_nodes),
                &mut barycenter_associates,
            );

            // Insert the northern dummies into the layer and set up constraints.
            // Each dummy on the northern side has its regular node as a successor.
            let insert_point = pointer;
            let successor = node;
            for dummy in north_dummy_nodes {
                a.node_set_layer_at(dummy, Some(layer), insert_point as usize);
                pointer += 1;

                // The dummy nodes form a layout unit identified by the node they were
                // created from. In addition, northern dummy nodes must appear before
                // the regular node
                a.node(dummy).properties.set(&iprops::IN_LAYER_LAYOUT_UNIT, node);

                // Each dummy node has at least one port (there may be two if an odd
                // port has both an incoming and an outgoing edge, however the origin
                // is the same)
                debug_assert!(!a.node(dummy).ports.is_empty());
                let dummy_port = a.node(dummy).ports[0];
                // The port the dummy node was created for
                let origin_port = port_origin(a, dummy_port)?;

                // If originPort has ALLOW_NON_FLOW_PORTS_TO_SWITCH_SIDES, do not apply
                // successor constraints to the dummy node
                if !a
                    .port(origin_port)
                    .properties
                    .get(&lopts::ALLOW_NON_FLOW_PORTS_TO_SWITCH_SIDES)
                {
                    let mut constraints: Vec<LNodeId> = a
                        .node(dummy)
                        .properties
                        .get(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS);
                    constraints.push(successor);
                    a.node(dummy)
                        .properties
                        .set(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS, constraints);
                }
            }

            // Do the same for ports on the southern side; the list of ports must be
            // built in reversed order, since ports on the southern side are listed
            // from right to left
            let mut port_list: Vec<LPortId> = a
                .node(node)
                .ports
                .iter()
                .copied()
                .filter(|&p| a.port(p).side == PortSide::SOUTH)
                .collect();
            port_list.reverse();

            if ordering_strategy != OrderingStrategy::NONE {
                port_list = model_order_north_south_input_reversing(a, port_list);
            }

            create_dummy_nodes(
                a,
                graph,
                &port_list,
                &mut south_dummy_nodes,
                None,
                &mut barycenter_associates,
            );

            let predecessor = node;
            for dummy in south_dummy_nodes {
                pointer += 1;
                a.node_set_layer_at(dummy, Some(layer), pointer as usize);

                // The dummy nodes form a layout unit identified by the node they were
                // created from. In addition, southern dummy nodes must appear after
                // the regular node
                a.node(dummy).properties.set(&iprops::IN_LAYER_LAYOUT_UNIT, node);

                debug_assert!(!a.node(dummy).ports.is_empty());
                let dummy_port = a.node(dummy).ports[0];
                let origin_port = port_origin(a, dummy_port)?;

                if !a
                    .port(origin_port)
                    .properties
                    .get(&lopts::ALLOW_NON_FLOW_PORTS_TO_SWITCH_SIDES)
                {
                    let mut constraints: Vec<LNodeId> = a
                        .node(predecessor)
                        .properties
                        .get(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS);
                    constraints.push(dummy);
                    a.node(predecessor)
                        .properties
                        .set(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS, constraints);
                }
            }

            // If the list of barycenter associates contains nodes, set the property
            if !barycenter_associates.is_empty() {
                a.node(node)
                    .properties
                    .set(&iprops::BARYCENTER_ASSOCIATES, barycenter_associates);
            }
        }
    }

    Ok(())
}

/// The `ORIGIN` of a dummy port, which is always the `LPort` it was created for.
fn port_origin(a: &LGraphArena, dummy_port: LPortId) -> Result<LPortId, String> {
    match a.port(dummy_port).properties.try_get(&iprops::ORIGIN) {
        Some(Origin::LPort(p)) => Ok(p),
        other => Err(format!(
            "north/south port dummy port without LPort origin: {other:?}"
        )),
    }
}

// /////////////////////////////////////////////////////////////////////////////
// PORT LIST SORTING

/// Sorts the list of northern and southern ports such that ports with only
/// incoming edges end up left, ports with only outgoing edges end up right,
/// and ports with both end up in between. Ports on the eastern and western
/// sides are left untouched.
fn sort_port_list(a: &mut LGraphArena, node: LNodeId) {
    let ports = a.node(node).ports.len() as i32;

    // Next IDs for ports with a given configuration of input and output edges. The
    // choice of initial IDs ensures that port IDs will be unique
    let mut in_ports = 0;
    let mut in_out_ports = ports;
    let mut out_ports = 2 * ports;

    // Iterate over the list of ports and set their IDs
    let port_list = a.node(node).ports.clone();
    for port in port_list {
        match a.port(port).side {
            PortSide::EAST | PortSide::WEST => {
                a.port_mut(port).id = -1;
            }
            PortSide::NORTH | PortSide::SOUTH => {
                let incoming = a.port(port).incoming_edges.len();
                let outgoing = a.port(port).outgoing_edges.len();

                let id = if incoming > 0 && outgoing > 0 {
                    let v = in_out_ports;
                    in_out_ports += 1;
                    v
                } else if incoming > 0 {
                    let v = in_ports;
                    in_ports += 1;
                    v
                } else if outgoing > 0 {
                    let v = out_ports;
                    out_ports += 1;
                    v
                } else {
                    // Unconnected ports are placed between input ports...
                    let v = in_ports;
                    in_ports += 1;
                    v
                };
                a.port_mut(port).id = id;
            }
            PortSide::UNDEFINED => {}
        }
    }

    // With all IDs assigned, sort the port list (stable)
    let mut sorted = a.node(node).ports.clone();
    sorted.sort_by(|&port1, &port2| {
        let side1 = a.port(port1).side;
        let side2 = a.port(port2).side;

        if side1 != side2 {
            // sort according to the node side
            (side1 as i32).cmp(&(side2 as i32))
        } else {
            let id1 = a.port(port1).id;
            let id2 = a.port(port2).id;
            if id1 == id2 {
                // Eastern and western ports have the same ID and have to retain their order
                Ordering::Equal
            } else if side1 == PortSide::NORTH {
                id1.cmp(&id2)
            } else {
                id2.cmp(&id1)
            }
        }
    });
    a.node_mut(node).ports = sorted;
}

fn model_order_north_south_input_reversing(
    a: &LGraphArena,
    port_list: Vec<LPortId>,
) -> Vec<LPortId> {
    let mut incoming: Vec<LPortId> = Vec::new();
    let mut outgoing: Vec<LPortId> = Vec::new();
    for port in port_list {
        if !a.port(port).incoming_edges.is_empty() {
            // Incoming edge
            incoming.push(port);
        } else {
            outgoing.push(port);
        }
    }
    let mut result: Vec<LPortId> = outgoing.into_iter().rev().collect();
    result.extend(incoming);
    result
}

// /////////////////////////////////////////////////////////////////////////////
// DUMMY NODE CREATION

/// Creates dummy nodes for the given ports (which must be sorted by position
/// from left to right) and adds them to `dummy_nodes`. Dummy nodes created
/// for the southern side due to north-south self-loops are placed in
/// `opposing_side_dummy_nodes` (may be `None` when called for southern
/// ports). Dummy nodes created for anything other than self-loops are added
/// to `barycenter_associates`.
fn create_dummy_nodes(
    a: &mut LGraphArena,
    graph: LGraphId,
    ports: &[LPortId],
    dummy_nodes: &mut Vec<LNodeId>,
    mut opposing_side_dummy_nodes: Option<&mut Vec<LNodeId>>,
    barycenter_associates: &mut Vec<LNodeId>,
) {
    // We'll assemble lists of ports with only incoming, ports with only outgoing
    // and ports with both, incoming and outgoing edges
    let mut in_ports: Vec<LPortId> = Vec::new();
    let mut out_ports: Vec<LPortId> = Vec::new();
    let mut in_out_ports: Vec<LPortId> = Vec::new();
    let mut same_side_self_loop_edges: Vec<LEdgeId> = Vec::new();
    let mut north_south_self_loop_edges: Vec<LEdgeId> = Vec::new();

    for &port in ports {
        // Go through the port's outgoing edges, looking for self-loops that need
        // special handling
        for edge in a.port(port).outgoing_edges.clone() {
            // Check for self loops we'd be interested in
            if a.edge_source_node(edge) == a.edge_target_node(edge) {
                // Check which sides the ports are on
                let target_side = a.port(a.edge(edge).target.unwrap()).side;
                if a.port(port).side == target_side {
                    // Same side
                    same_side_self_loop_edges.push(edge);
                } else if a.port(port).side == PortSide::NORTH
                    && target_side == PortSide::SOUTH
                {
                    // North->South self-loop. Due to the SelfLoopProcessor, a
                    // South->North self-loop cannot happen
                    north_south_self_loop_edges.push(edge);
                }
            }
        }
    }

    // First, create the dummy nodes that handle north->south self-loops. For now,
    // we always route north->south self-loops east to the node.
    for edge in north_south_self_loop_edges {
        create_north_south_self_loop_dummy_nodes(
            a,
            graph,
            edge,
            dummy_nodes,
            opposing_side_dummy_nodes.as_deref_mut(),
            PortSide::EAST,
        );
    }

    // Second, create the dummy nodes that handle same-side self-loops
    for edge in same_side_self_loop_edges {
        create_same_side_self_loop_dummy_node(a, graph, edge, dummy_nodes);
    }

    // Now we iterate over the ports again, with certain self-loop edges already
    // removed, and check if they are input ports, output ports, or both
    for &port in ports {
        // Find out if the port has incoming or outgoing edges
        let has_in = !a.port(port).incoming_edges.is_empty();
        let has_out = !a.port(port).outgoing_edges.is_empty();

        if has_in && has_out {
            in_out_ports.push(port);
        } else if has_in {
            in_ports.push(port);
        } else if has_out {
            out_ports.push(port);
        }
    }

    // New approach: give every input and output port its own dummy node
    for in_port in in_ports {
        barycenter_associates.push(create_dummy_node(a, graph, Some(in_port), None, dummy_nodes));
    }

    for out_port in out_ports {
        barycenter_associates.push(create_dummy_node(a, graph, None, Some(out_port), dummy_nodes));
    }

    // in / out ports get their own dummy nodes
    for in_out_port in in_out_ports {
        barycenter_associates.push(create_dummy_node(
            a,
            graph,
            Some(in_out_port),
            Some(in_out_port),
            dummy_nodes,
        ));
    }
}

/// Creates a dummy node for the given ports. Edges going into `in_port` are
/// rerouted to the dummy node's input port. Edges leaving the `out_port` are
/// rerouted to the dummy node's output port. Both arguments may refer to the
/// same port.
fn create_dummy_node(
    a: &mut LGraphArena,
    graph: LGraphId,
    in_port: Option<LPortId>,
    out_port: Option<LPortId>,
    dummy_nodes: &mut Vec<LNodeId>,
) -> LNodeId {
    let dummy = a.create_node(graph);
    a.node_mut(dummy).node_type = NodeType::NORTH_SOUTH_PORT;
    a.node(dummy)
        .properties
        .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);

    let mut crossing_hint = 0;

    // Input port
    if let Some(in_port) = in_port {
        // The port is expected to have edges connected to it
        debug_assert!(
            !(a.port(in_port).incoming_edges.is_empty()
                && a.port(in_port).outgoing_edges.is_empty())
        );

        let dummy_input_port = a.create_port();
        a.port(dummy_input_port)
            .properties
            .set(&iprops::ORIGIN, Origin::LPort(in_port));
        a.node(dummy)
            .properties
            .set(&iprops::ORIGIN, Origin::LNode(a.port(in_port).node.unwrap()));
        a.port_set_side(dummy_input_port, PortSide::WEST);
        a.port_set_node(dummy_input_port, Some(dummy));

        // Reroute edges
        let edge_array = a.port(in_port).incoming_edges.clone();
        for edge in edge_array {
            a.edge_set_target(edge, Some(dummy_input_port));
        }

        // Make sure the inPort knows about the dummy node
        a.port(in_port).properties.set(&iprops::PORT_DUMMY, dummy);

        crossing_hint += 1;
    }

    // Output port
    if let Some(out_port) = out_port {
        // The port is expected to have edges connected to it
        debug_assert!(
            !(a.port(out_port).incoming_edges.is_empty()
                && a.port(out_port).outgoing_edges.is_empty())
        );

        let dummy_output_port = a.create_port();
        a.node(dummy)
            .properties
            .set(&iprops::ORIGIN, Origin::LNode(a.port(out_port).node.unwrap()));
        a.port(dummy_output_port)
            .properties
            .set(&iprops::ORIGIN, Origin::LPort(out_port));
        a.port_set_side(dummy_output_port, PortSide::EAST);
        a.port_set_node(dummy_output_port, Some(dummy));

        // Reroute edges
        let edge_array = a.port(out_port).outgoing_edges.clone();
        for edge in edge_array {
            a.edge_set_source(edge, Some(dummy_output_port));
        }

        // Make sure the outPort knows about the dummy node
        a.port(out_port).properties.set(&iprops::PORT_DUMMY, dummy);

        crossing_hint += 1;
    }

    // Set the crossing hint used for cross counting later
    a.node(dummy).properties.set(&iprops::CROSSING_HINT, crossing_hint);

    dummy_nodes.push(dummy);

    dummy
}

/// Creates a dummy node for the given non-north-south self-loop edge. The
/// dummy node's `ORIGIN` property is set to the edge. The dummy node has two
/// ports, one for each port the node was connected to.
fn create_same_side_self_loop_dummy_node(
    a: &mut LGraphArena,
    graph: LGraphId,
    self_loop: LEdgeId,
    dummy_nodes: &mut Vec<LNodeId>,
) {
    let dummy = a.create_node(graph);
    a.node_mut(dummy).node_type = NodeType::NORTH_SOUTH_PORT;
    a.node(dummy)
        .properties
        .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
    a.node(dummy).properties.set(&iprops::ORIGIN, Origin::LEdge(self_loop));

    let source_port = a.edge(self_loop).source.unwrap();
    let target_port = a.edge(self_loop).target.unwrap();

    // Input port
    let dummy_input_port = a.create_port();
    a.port(dummy_input_port)
        .properties
        .set(&iprops::ORIGIN, Origin::LPort(target_port));
    a.port_set_side(dummy_input_port, PortSide::WEST);
    a.port_set_node(dummy_input_port, Some(dummy));

    // Output port
    let dummy_output_port = a.create_port();
    a.port(dummy_output_port)
        .properties
        .set(&iprops::ORIGIN, Origin::LPort(source_port));
    a.port_set_side(dummy_output_port, PortSide::EAST);
    a.port_set_node(dummy_output_port, Some(dummy));

    // Make sure the ports know about the dummy node
    a.port(source_port).properties.set(&iprops::PORT_DUMMY, dummy);
    a.port(target_port).properties.set(&iprops::PORT_DUMMY, dummy);

    // Disconnect the edge
    a.edge_set_source(self_loop, None);
    a.edge_set_target(self_loop, None);

    dummy_nodes.push(dummy);

    // Set the crossing hint used for cross counting later
    a.node(dummy).properties.set(&iprops::CROSSING_HINT, 2);
}

/// Creates two dummy nodes for the given north-south self-loop edge. Each
/// dummy node has only one port, on the specified side of the node.
fn create_north_south_self_loop_dummy_nodes(
    a: &mut LGraphArena,
    graph: LGraphId,
    self_loop: LEdgeId,
    north_dummy_nodes: &mut Vec<LNodeId>,
    south_dummy_nodes: Option<&mut Vec<LNodeId>>,
    port_side: PortSide,
) {
    let source_port = a.edge(self_loop).source.unwrap();
    let target_port = a.edge(self_loop).target.unwrap();

    // North dummy
    let north_dummy = a.create_node(graph);
    a.node_mut(north_dummy).node_type = NodeType::NORTH_SOUTH_PORT;
    a.node(north_dummy)
        .properties
        .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
    a.node(north_dummy)
        .properties
        .set(&iprops::ORIGIN, Origin::LNode(a.port(source_port).node.unwrap()));

    let north_dummy_output_port = a.create_port();
    a.port(north_dummy_output_port)
        .properties
        .set(&iprops::ORIGIN, Origin::LPort(source_port));
    a.port_set_side(north_dummy_output_port, port_side);
    a.port_set_node(north_dummy_output_port, Some(north_dummy));

    a.port(source_port).properties.set(&iprops::PORT_DUMMY, north_dummy);

    // South dummy
    let south_dummy = a.create_node(graph);
    a.node_mut(south_dummy).node_type = NodeType::NORTH_SOUTH_PORT;
    a.node(south_dummy)
        .properties
        .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
    a.node(south_dummy)
        .properties
        .set(&iprops::ORIGIN, Origin::LNode(a.port(target_port).node.unwrap()));

    let south_dummy_input_port = a.create_port();
    a.port(south_dummy_input_port)
        .properties
        .set(&iprops::ORIGIN, Origin::LPort(target_port));
    a.port_set_side(south_dummy_input_port, port_side);
    a.port_set_node(south_dummy_input_port, Some(south_dummy));

    a.port(target_port).properties.set(&iprops::PORT_DUMMY, south_dummy);

    // Reroute the edge
    a.edge_set_source(self_loop, Some(north_dummy_output_port));
    a.edge_set_target(self_loop, Some(south_dummy_input_port));

    north_dummy_nodes.insert(0, north_dummy);
    // North-south self-loops can only be encountered while processing the
    // northern ports, where the southern list is always present.
    south_dummy_nodes
        .expect("north-south self-loop encountered while processing southern ports")
        .push(south_dummy);

    // Set the crossing hints used for cross counting later
    a.node(north_dummy).properties.set(&iprops::CROSSING_HINT, 1);
    a.node(south_dummy).properties.set(&iprops::CROSSING_HINT, 1);
}
