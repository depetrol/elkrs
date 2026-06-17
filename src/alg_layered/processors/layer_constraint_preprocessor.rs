//! Hides FIRST_SEPARATE and
//! LAST_SEPARATE nodes before layering.

use crate::graph::properties::Property;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::LayerConstraint;

crate::elk_enum! {
    pub enum HiddenNodeConnections {
        NONE,
        FIRST_SEPARATE,
        LAST_SEPARATE,
        BOTH,
    }
}

impl HiddenNodeConnections {
    fn combine(self, layer_constraint: LayerConstraint) -> Self {
        match self {
            HiddenNodeConnections::NONE => {
                if layer_constraint == LayerConstraint::FIRST_SEPARATE {
                    HiddenNodeConnections::FIRST_SEPARATE
                } else {
                    HiddenNodeConnections::LAST_SEPARATE
                }
            }
            HiddenNodeConnections::FIRST_SEPARATE => {
                if layer_constraint == LayerConstraint::FIRST_SEPARATE {
                    HiddenNodeConnections::FIRST_SEPARATE
                } else {
                    HiddenNodeConnections::BOTH
                }
            }
            HiddenNodeConnections::LAST_SEPARATE => {
                if layer_constraint == LayerConstraint::FIRST_SEPARATE {
                    HiddenNodeConnections::BOTH
                } else {
                    HiddenNodeConnections::LAST_SEPARATE
                }
            }
            HiddenNodeConnections::BOTH => HiddenNodeConnections::BOTH,
        }
    }
}

pub static HIDDEN_NODE_CONNECTIONS: Property<HiddenNodeConnections> =
    Property::with_default("separateLayerConnections", || HiddenNodeConnections::NONE);

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mut hidden_nodes: Vec<LNodeId> = Vec::new();

    let nodes = a.graph(graph).layerless_nodes.clone();
    for node in nodes {
        if is_relevant_node(a, node) {
            hide(a, node)?;
            hidden_nodes.push(node);
            a.graph_mut(graph).layerless_nodes.retain(|&n| n != node);
        }
    }

    if !hidden_nodes.is_empty() {
        a.graph(graph).properties.set(&iprops::HIDDEN_NODES, hidden_nodes);
    }
    Ok(())
}

fn is_relevant_node(a: &LGraphArena, node: LNodeId) -> bool {
    matches!(
        a.node(node)
            .properties
            .get::<LayerConstraint>(&lopts::LAYERING_LAYER_CONSTRAINT),
        LayerConstraint::FIRST_SEPARATE | LayerConstraint::LAST_SEPARATE
    )
}

fn hide(a: &mut LGraphArena, node: LNodeId) -> Result<(), String> {
    ensure_no_inacceptable_edges(a, node)?;
    let edges = a.node_connected_edges(node);
    for edge in edges {
        hide_edge(a, node, edge);
    }
    Ok(())
}

fn hide_edge(a: &mut LGraphArena, node: LNodeId, edge: LEdgeId) {
    let is_outgoing = a.edge_source_node(edge) == node;
    let opposite_port = if is_outgoing {
        a.edge(edge).target.unwrap()
    } else {
        a.edge(edge).source.unwrap()
    };

    if is_outgoing {
        a.edge_set_target(edge, None);
    } else {
        a.edge_set_source(edge, None);
    }

    a.edge(edge)
        .properties
        .set(&iprops::ORIGINAL_OPPOSITE_PORT, opposite_port);

    let opposite_node = a.port(opposite_port).node.unwrap();
    update_opposite_node_layer_constraints(a, node, opposite_node);
}

fn update_opposite_node_layer_constraints(
    a: &mut LGraphArena,
    hidden_node: LNodeId,
    opposite_node: LNodeId,
) {
    if a.node(opposite_node)
        .properties
        .has(&lopts::LAYERING_LAYER_CONSTRAINT)
    {
        return;
    }

    let connections = a
        .node(opposite_node)
        .properties
        .get(&HIDDEN_NODE_CONNECTIONS)
        .combine(
            a.node(hidden_node)
                .properties
                .get(&lopts::LAYERING_LAYER_CONSTRAINT),
        );
    a.node(opposite_node)
        .properties
        .set(&HIDDEN_NODE_CONNECTIONS, connections);

    if !a.node_connected_edges(opposite_node).is_empty() {
        return;
    }

    match connections {
        HiddenNodeConnections::FIRST_SEPARATE => {
            a.node(opposite_node)
                .properties
                .set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::FIRST);
        }
        HiddenNodeConnections::LAST_SEPARATE => {
            a.node(opposite_node)
                .properties
                .set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::LAST);
        }
        _ => {}
    }
}

fn ensure_no_inacceptable_edges(a: &LGraphArena, node: LNodeId) -> Result<(), String> {
    let layer_constraint: LayerConstraint = a
        .node(node)
        .properties
        .get(&lopts::LAYERING_LAYER_CONSTRAINT);
    if layer_constraint == LayerConstraint::FIRST_SEPARATE {
        for edge in a.node_incoming_edges(node) {
            if !is_acceptable_incident_edge(a, edge) {
                return Err(
                    "Node has its layer constraint set to FIRST_SEPARATE, but has at least one \
                     incoming edge. FIRST_SEPARATE nodes must not have incoming edges."
                        .to_string(),
                );
            }
        }
    } else if layer_constraint == LayerConstraint::LAST_SEPARATE {
        for edge in a.node_outgoing_edges(node) {
            if !is_acceptable_incident_edge(a, edge) {
                return Err(
                    "Node has its layer constraint set to LAST_SEPARATE, but has at least one \
                     outgoing edge. LAST_SEPARATE nodes must not have outgoing edges."
                        .to_string(),
                );
            }
        }
    }
    Ok(())
}

fn is_acceptable_incident_edge(a: &LGraphArena, edge: LEdgeId) -> bool {
    let source_node = a.edge_source_node(edge);
    let target_node = a.edge_target_node(edge);
    a.node(source_node).node_type == NodeType::EXTERNAL_PORT
        && a.node(target_node).node_type == NodeType::EXTERNAL_PORT
}
