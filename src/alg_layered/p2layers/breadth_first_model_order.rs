
use crate::core::javacompat::tim_sort;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // Gather the real (NORMAL) nodes.
    let mut real_nodes: Vec<LNodeId> = a
        .graph(graph)
        .layerless_nodes
        .iter()
        .copied()
        .filter(|&n| a.node(n).node_type == NodeType::NORMAL)
        .collect();

    // Sort by model order (Collections.sort = TimSort).
    let mut error: Option<String> = None;
    tim_sort(&mut real_nodes, |&n1, &n2| {
        if a.node(n1).properties.has(&iprops::MODEL_ORDER)
            && a.node(n2).properties.has(&iprops::MODEL_ORDER)
        {
            let m1: i32 = a.node(n1).properties.get(&iprops::MODEL_ORDER);
            let m2: i32 = a.node(n2).properties.get(&iprops::MODEL_ORDER);
            match m1.cmp(&m2) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            }
        } else {
            error = Some(
                "The BF model order layer assigner requires all real nodes to have a model order."
                    .to_string(),
            );
            0
        }
    });
    if let Some(e) = error {
        return Err(e);
    }

    let mut first_node = true;
    let mut current_layer = a.create_layer(graph);
    let mut current_dummy_layer: Option<LayerId> = None;
    a.graph_mut(graph).layers.push(current_layer);

    for node in real_nodes {
        if first_node {
            a.node_set_layer(node, Some(current_layer));
            first_node = false;
        } else {
            // Check whether any incoming edge connects to the current layer.
            for edge in a.node_incoming_edges(node) {
                let source_node = a.edge_source_node(edge);
                let source_type = a.node(source_node).node_type;
                let connects_to_current = if source_type == NodeType::NORMAL {
                    a.node(source_node).layer == Some(current_layer)
                } else if source_type == NodeType::LABEL {
                    // Case dummy label in-between: source-of-source's layer.
                    let inc = a.node_incoming_edges(source_node);
                    if let Some(&first_inc) = inc.first() {
                        let ss = a.edge_source_node(first_inc);
                        a.node(ss).layer == Some(current_layer)
                    } else {
                        false
                    }
                } else {
                    false
                };
                if connects_to_current {
                    let dummy = a.create_layer(graph);
                    a.graph_mut(graph).layers.push(dummy);
                    current_dummy_layer = Some(dummy);
                    current_layer = a.create_layer(graph);
                    a.graph_mut(graph).layers.push(current_layer);
                }
            }
            // Add all unlayered label dummies to the in-between dummy layer.
            for edge in a.node_incoming_edges(node) {
                let source_node = a.edge_source_node(edge);
                if a.node(source_node).node_type == NodeType::LABEL
                    && a.node(source_node).layer.is_none()
                {
                    a.node_set_layer(source_node, current_dummy_layer);
                }
            }
            a.node_set_layer(node, Some(current_layer));
        }
    }

    a.graph_mut(graph).layerless_nodes.clear();

    delete_empty_layers_and_reid(a, graph);

    Ok(())
}

/// Remove empty layers and reassign `layer.id`.
fn delete_empty_layers_and_reid(a: &mut LGraphArena, graph: LGraphId) {
    let layers = a.graph(graph).layers.clone();
    let kept: Vec<LayerId> = layers.into_iter().filter(|&l| !a.layer(l).nodes.is_empty()).collect();
    a.graph_mut(graph).layers = kept.clone();
    for (i, &layer) in kept.iter().enumerate() {
        a.layer_mut(layer).id = i as i32;
    }
}
