
use crate::core::options::PortSide;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_util::edge_reverse;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::{EdgeConstraint, LayerConstraint, PortType};

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let remaining_nodes = handle_outer_nodes(a, graph);
    handle_inner_nodes(a, graph, &remaining_nodes);
    Ok(())
}

fn edge_constraint_for(layer_constraint: LayerConstraint) -> Option<EdgeConstraint> {
    match layer_constraint {
        LayerConstraint::FIRST | LayerConstraint::FIRST_SEPARATE => {
            Some(EdgeConstraint::OUTGOING_ONLY)
        }
        LayerConstraint::LAST | LayerConstraint::LAST_SEPARATE => {
            Some(EdgeConstraint::INCOMING_ONLY)
        }
        _ => None,
    }
}

fn handle_outer_nodes(a: &mut LGraphArena, graph: LGraphId) -> Vec<LNodeId> {
    let mut remaining_nodes = Vec::new();
    let nodes = a.graph(graph).layerless_nodes.clone();
    for node in nodes {
        let layer_constraint: LayerConstraint =
            a.node(node).properties.get(&lopts::LAYERING_LAYER_CONSTRAINT);
        match edge_constraint_for(layer_constraint) {
            Some(edge_constraint) => {
                // Always stores OUTGOING_ONLY here (preserved quirk)
                a.node(node)
                    .properties
                    .set(&iprops::EDGE_CONSTRAINT, EdgeConstraint::OUTGOING_ONLY);
                if edge_constraint == EdgeConstraint::INCOMING_ONLY {
                    reverse_edges(a, graph, node, layer_constraint, PortType::INPUT);
                } else {
                    reverse_edges(a, graph, node, layer_constraint, PortType::OUTPUT);
                }
            }
            None => remaining_nodes.push(node),
        }
    }
    remaining_nodes
}

fn handle_inner_nodes(a: &mut LGraphArena, graph: LGraphId, remaining_nodes: &[LNodeId]) {
    for &node in remaining_nodes {
        let layer_constraint: LayerConstraint =
            a.node(node).properties.get(&lopts::LAYERING_LAYER_CONSTRAINT);
        match edge_constraint_for(layer_constraint) {
            Some(edge_constraint) => {
                a.node(node)
                    .properties
                    .set(&iprops::EDGE_CONSTRAINT, EdgeConstraint::OUTGOING_ONLY);
                if edge_constraint == EdgeConstraint::INCOMING_ONLY {
                    reverse_edges(a, graph, node, layer_constraint, PortType::INPUT);
                } else {
                    reverse_edges(a, graph, node, layer_constraint, PortType::OUTPUT);
                }
            }
            None => {
                let side_fixed = a
                    .node(node)
                    .properties
                    .get::<crate::core::options::PortConstraints>(&lopts::PORT_CONSTRAINTS)
                    .is_side_fixed();
                if side_fixed && !a.node(node).ports.is_empty() {
                    let mut all_ports_reversed = true;
                    'ports: for &port in &a.node(node).ports {
                        let side = a.port(port).side;
                        let net_flow = a.port_net_flow(port);
                        if !(side == PortSide::EAST && net_flow > 0
                            || side == PortSide::WEST && net_flow < 0)
                        {
                            all_ports_reversed = false;
                            break;
                        }
                        for &e in &a.port(port).outgoing_edges {
                            let target_node = a.edge_target_node(e);
                            let lc: LayerConstraint = a
                                .node(target_node)
                                .properties
                                .get(&lopts::LAYERING_LAYER_CONSTRAINT);
                            if lc == LayerConstraint::LAST || lc == LayerConstraint::LAST_SEPARATE {
                                all_ports_reversed = false;
                                break 'ports;
                            }
                        }
                        for &e in &a.port(port).incoming_edges {
                            let source_node = a.edge_source_node(e);
                            let lc: LayerConstraint = a
                                .node(source_node)
                                .properties
                                .get(&lopts::LAYERING_LAYER_CONSTRAINT);
                            if lc == LayerConstraint::FIRST
                                || lc == LayerConstraint::FIRST_SEPARATE
                            {
                                all_ports_reversed = false;
                                break 'ports;
                            }
                        }
                    }
                    if all_ports_reversed {
                        reverse_edges(a, graph, node, layer_constraint, PortType::UNDEFINED);
                    }
                }
            }
        }
    }
}

fn reverse_edges(
    a: &mut LGraphArena,
    graph: LGraphId,
    node: LNodeId,
    node_layer_constraint: LayerConstraint,
    target_port_type: PortType,
) {
    let ports = a.node(node).ports.clone();
    for port in ports {
        if target_port_type == PortType::INPUT || target_port_type == PortType::UNDEFINED {
            let outgoing: Vec<LEdgeId> = a.port(port).outgoing_edges.clone();
            for edge in outgoing {
                if can_reverse_outgoing_edge(a, node_layer_constraint, edge) {
                    edge_reverse(a, graph, edge, true);
                }
            }
        }
        if target_port_type == PortType::OUTPUT || target_port_type == PortType::UNDEFINED {
            let incoming: Vec<LEdgeId> = a.port(port).incoming_edges.clone();
            for edge in incoming {
                if can_reverse_incoming_edge(a, node_layer_constraint, edge) {
                    edge_reverse(a, graph, edge, true);
                }
            }
        }
    }
}

fn can_reverse_outgoing_edge(
    a: &LGraphArena,
    source_layer_constraint: LayerConstraint,
    edge: LEdgeId,
) -> bool {
    if a.edge(edge).properties.get(&iprops::REVERSED) {
        return false;
    }
    let target_node = a.edge_target_node(edge);
    if source_layer_constraint == LayerConstraint::LAST
        && a.node(target_node).node_type == NodeType::LABEL
    {
        return false;
    }
    let target_layer_constraint: LayerConstraint = a
        .node(target_node)
        .properties
        .get(&lopts::LAYERING_LAYER_CONSTRAINT);
    target_layer_constraint != LayerConstraint::LAST_SEPARATE
}

fn can_reverse_incoming_edge(
    a: &LGraphArena,
    target_layer_constraint: LayerConstraint,
    edge: LEdgeId,
) -> bool {
    if a.edge(edge).properties.get(&iprops::REVERSED) {
        return false;
    }
    let source_node = a.edge_source_node(edge);
    if target_layer_constraint == LayerConstraint::FIRST
        && a.node(source_node).node_type == NodeType::LABEL
    {
        return false;
    }
    let source_layer_constraint: LayerConstraint = a
        .node(source_node)
        .properties
        .get(&lopts::LAYERING_LAYER_CONSTRAINT);
    source_layer_constraint != LayerConstraint::FIRST_SEPARATE
}
