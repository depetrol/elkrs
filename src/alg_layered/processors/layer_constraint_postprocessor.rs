//! Moves FIRST/LAST nodes to
//! dedicated layers and restores hidden FIRST_SEPARATE/LAST_SEPARATE nodes.

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::LayerConstraint;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    if !a.graph(graph).layers.is_empty() {
        let first_layer = a.graph(graph).layers[0];
        let last_layer = *a.graph(graph).layers.last().unwrap();

        let first_label_layer = a.create_layer(graph);
        let last_label_layer = a.create_layer(graph);

        move_first_and_last_nodes(a, graph, first_layer, last_layer, first_label_layer, last_label_layer)?;

        if !a.layer(first_label_layer).nodes.is_empty() {
            a.graph_mut(graph).layers.insert(0, first_label_layer);
        }
        if !a.layer(last_label_layer).nodes.is_empty() {
            a.graph_mut(graph).layers.push(last_label_layer);
        }
    }

    if a.graph(graph).properties.has(&iprops::HIDDEN_NODES) {
        let first_separate_layer = a.create_layer(graph);
        let last_separate_layer = a.create_layer(graph);

        restore_hidden_nodes(a, graph, first_separate_layer, last_separate_layer);

        if !a.layer(first_separate_layer).nodes.is_empty() {
            a.graph_mut(graph).layers.insert(0, first_separate_layer);
        }
        if !a.layer(last_separate_layer).nodes.is_empty() {
            a.graph_mut(graph).layers.push(last_separate_layer);
        }
    }
    Ok(())
}

fn move_first_and_last_nodes(
    a: &mut LGraphArena,
    graph: LGraphId,
    first_layer: LayerId,
    last_layer: LayerId,
    first_label_layer: LayerId,
    last_label_layer: LayerId,
) -> Result<(), String> {
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            match a
                .node(node)
                .properties
                .get::<LayerConstraint>(&lopts::LAYERING_LAYER_CONSTRAINT)
            {
                LayerConstraint::FIRST => {
                    throw_up_unless_no_incoming_edges(a, node)?;
                    a.node_set_layer(node, Some(first_layer));
                    move_labels_to_label_layer(a, node, true, first_label_layer);
                }
                LayerConstraint::LAST => {
                    throw_up_unless_no_outgoing_edges(a, node)?;
                    a.node_set_layer(node, Some(last_layer));
                    move_labels_to_label_layer(a, node, false, last_label_layer);
                }
                _ => {}
            }
        }
    }

    // remove empty layers
    let layers = a.graph(graph).layers.clone();
    let non_empty: Vec<LayerId> = layers
        .into_iter()
        .filter(|&l| !a.layer(l).nodes.is_empty())
        .collect();
    a.graph_mut(graph).layers = non_empty;
    Ok(())
}

fn move_labels_to_label_layer(
    a: &mut LGraphArena,
    node: LNodeId,
    incoming: bool,
    label_layer: LayerId,
) {
    let edges = if incoming {
        a.node_incoming_edges(node)
    } else {
        a.node_outgoing_edges(node)
    };
    for edge in edges {
        let possible_label_dummy = if incoming {
            a.edge_source_node(edge)
        } else {
            a.edge_target_node(edge)
        };
        if a.node(possible_label_dummy).node_type == NodeType::LABEL {
            a.node_set_layer(possible_label_dummy, Some(label_layer));
        }
    }
}

fn restore_hidden_nodes(
    a: &mut LGraphArena,
    graph: LGraphId,
    first_separate_layer: LayerId,
    last_separate_layer: LayerId,
) {
    let hidden_nodes: Vec<LNodeId> = a
        .graph(graph)
        .properties
        .try_get(&iprops::HIDDEN_NODES)
        .unwrap_or_default();
    for hidden_node in hidden_nodes {
        match a
            .node(hidden_node)
            .properties
            .get::<LayerConstraint>(&lopts::LAYERING_LAYER_CONSTRAINT)
        {
            LayerConstraint::FIRST_SEPARATE => {
                a.node_set_layer(hidden_node, Some(first_separate_layer));
            }
            LayerConstraint::LAST_SEPARATE => {
                a.node_set_layer(hidden_node, Some(last_separate_layer));
            }
            _ => debug_assert!(false),
        }

        let edges = a.node_connected_edges(hidden_node);
        for hidden_edge in edges {
            if a.edge(hidden_edge).source.is_some() && a.edge(hidden_edge).target.is_some() {
                continue;
            }
            let is_outgoing = a.edge(hidden_edge).target.is_none();
            let original_opposite_port = a
                .edge(hidden_edge)
                .properties
                .try_get(&iprops::ORIGINAL_OPPOSITE_PORT);
            if is_outgoing {
                a.edge_set_target(hidden_edge, original_opposite_port);
            } else {
                a.edge_set_source(hidden_edge, original_opposite_port);
            }
        }
    }
}

fn throw_up_unless_no_incoming_edges(a: &LGraphArena, node: LNodeId) -> Result<(), String> {
    for incoming in a.node_incoming_edges(node) {
        if a.node(a.edge_source_node(incoming)).node_type != NodeType::LABEL {
            return Err("Node has its layer constraint set to FIRST, but has at least one \
                        incoming edge that does not come from a FIRST_SEPARATE node."
                .to_string());
        }
    }
    Ok(())
}

fn throw_up_unless_no_outgoing_edges(a: &LGraphArena, node: LNodeId) -> Result<(), String> {
    for outgoing in a.node_outgoing_edges(node) {
        if a.node(a.edge_target_node(outgoing)).node_type != NodeType::LABEL {
            return Err("Node has its layer constraint set to LAST, but has at least one \
                        outgoing edge that does not go to a LAST_SEPARATE node."
                .to_string());
        }
    }
    Ok(())
}
