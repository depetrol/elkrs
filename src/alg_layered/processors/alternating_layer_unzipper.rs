//! Divides nodes up between sub-layers to
//! create a more compact layout (layerUnzipping.strategy = ALTERNATING).

use crate::core::options::PortConstraints;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::processors::long_edge_splitter;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mut insertion_layer_offset = 1i32;
    // (newLayer, insertionIndex)
    let mut new_layers: Vec<(LayerId, i32)> = Vec::new();

    let layer_count = a.graph(graph).layers.len();
    for i in 0..layer_count {
        let cur_layer = a.graph(graph).layers[i];
        let n = get_layer_split_property(a, cur_layer);
        let reset_on_long_edges = get_reset_on_long_edges_property(a, cur_layer);
        let minimize_edge_length = get_minimize_edge_length_property(a, cur_layer);

        if minimize_edge_length {
            let nodes = a.layer(cur_layer).nodes.clone();
            let mut max_width = 0.0_f64;
            let mut average_height = 0.0_f64;
            for &node in &nodes {
                max_width = max_width.max(a.node(node).size.x);
                average_height += a.node(node).size.y;
            }
            let count = nodes.len() as f64;
            average_height /= count;

            let edge_node_bl: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_NODE_BETWEEN_LAYERS);
            let edge_edge_bl: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_EDGE_BETWEEN_LAYERS);
            let node_node_bl: f64 = a.graph(graph).properties.get(&lopts::SPACING_NODE_NODE_BETWEEN_LAYERS);
            max_width += (2.0 * edge_node_bl).max((count * edge_edge_bl).max(node_node_bl));

            let node_node: f64 = a.graph(graph).properties.get(&lopts::SPACING_NODE_NODE);
            let edge_node: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_NODE);
            average_height += node_node.max(edge_node);

            if max_width / average_height >= count / 4.0 {
                continue;
            }
        }

        // only split if there are more nodes than the resulting sub-layers
        if a.layer(cur_layer).nodes.len() as i32 > n {
            let mut sub_layers: Vec<LayerId> = Vec::new();
            sub_layers.push(cur_layer);
            for j in 0..(n - 1) {
                let new_layer = a.create_layer(graph);
                new_layers.push((new_layer, i as i32 + j + insertion_layer_offset));
                sub_layers.push(new_layer);
            }
            insertion_layer_offset += n - 1;

            let nodes_in_layer = a.layer(sub_layers[0]).nodes.len() as i32;
            let mut j = 0i32;
            let mut node_index = 0i32;
            let mut target_layer = 0i32;
            while j < nodes_in_layer {
                let node = a.layer(sub_layers[0]).nodes[node_index as usize];
                if a.node(node).node_type != NodeType::NONSHIFTING_PLACEHOLDER {
                    node_index += shift_node(a, graph, &sub_layers, target_layer % n, node_index);
                } else {
                    j -= 1;
                    target_layer -= 1;
                }
                if reset_on_long_edges && a.node(node).node_type == NodeType::LONG_EDGE {
                    target_layer = -1;
                }
                j += 1;
                node_index += 1;
                target_layer += 1;
            }
        }
    }

    for (new_layer, index) in new_layers {
        a.graph_mut(graph).layers.insert(index as usize, new_layer);
    }

    // remove unconnected placeholder nodes
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            let t = a.node(node).node_type;
            if t == NodeType::PLACEHOLDER || t == NodeType::NONSHIFTING_PLACEHOLDER {
                a.layer_mut(layer).nodes.retain(|&x| x != node);
                a.node_mut(node).layer = None;
            }
        }
    }

    Ok(())
}

fn get_layer_split_property(a: &LGraphArena, layer: LayerId) -> i32 {
    let mut layer_split = i32::MAX;
    let mut property_unset = true;
    for &node in &a.layer(layer).nodes {
        if a.node(node).properties.has(&lopts::LAYER_UNZIPPING_LAYER_SPLIT) {
            property_unset = false;
            let node_value: i32 = a.node(node).properties.get(&lopts::LAYER_UNZIPPING_LAYER_SPLIT);
            layer_split = layer_split.min(node_value);
        }
    }
    if property_unset {
        // LayeredOptions.LAYER_UNZIPPING_LAYER_SPLIT.getDefault()
        layer_split = 2;
    }
    layer_split
}

fn get_reset_on_long_edges_property(a: &LGraphArena, layer: LayerId) -> bool {
    for &node in &a.layer(layer).nodes {
        if a.node(node).properties.has(&lopts::LAYER_UNZIPPING_RESET_ON_LONG_EDGES)
            && !a.node(node).properties.get(&lopts::LAYER_UNZIPPING_RESET_ON_LONG_EDGES)
        {
            return false;
        }
    }
    true
}

fn get_minimize_edge_length_property(a: &LGraphArena, layer: LayerId) -> bool {
    for &node in &a.layer(layer).nodes {
        if a.node(node).properties.has(&lopts::LAYER_UNZIPPING_MINIMIZE_EDGE_LENGTH)
            && a.node(node).properties.get(&lopts::LAYER_UNZIPPING_MINIMIZE_EDGE_LENGTH)
        {
            return true;
        }
    }
    false
}

fn shift_node(
    a: &mut LGraphArena,
    graph: LGraphId,
    sub_layers: &[LayerId],
    target_layer: i32,
    node_index: i32,
) -> i32 {
    let node = a.layer(sub_layers[0]).nodes[node_index as usize];
    if target_layer > 0 {
        a.node_set_layer(node, Some(sub_layers[target_layer as usize]));
    }

    let mut edge_count = 0i32;
    let mut no_incoming_edges = true;

    // Lists.reverse(Lists.newArrayList(node.getIncomingEdges()))
    let mut reversed_incoming: Vec<LEdgeId> = a.node_incoming_edges(node);
    reversed_incoming.reverse();
    for incoming_edge in reversed_incoming {
        no_incoming_edges = false;
        let mut next_edge_to_split = incoming_edge;
        for layer_index in 0..target_layer {
            let dummy_node = create_dummy_node(a, graph, next_edge_to_split);
            let sub = sub_layers[layer_index as usize];
            if node_index + edge_count > a.layer(sub).nodes.len() as i32 {
                a.node_set_layer(dummy_node, Some(sub));
            } else {
                a.node_set_layer_at(dummy_node, Some(sub), (node_index + edge_count) as usize);
            }
            next_edge_to_split = long_edge_splitter::split_edge(a, next_edge_to_split, dummy_node);
        }
        if target_layer > 0 {
            edge_count += 1;
        }
    }

    // create unconnected dummy nodes to fill the layers if there are no incoming edges
    if no_incoming_edges {
        for layer_index in 0..target_layer {
            let dummy_node = a.create_node(graph);
            a.node_mut(dummy_node).node_type = NodeType::PLACEHOLDER;
            let sub = sub_layers[layer_index as usize];
            if node_index + edge_count > a.layer(sub).nodes.len() as i32 {
                a.node_set_layer(dummy_node, Some(sub));
            } else {
                a.node_set_layer_at(dummy_node, Some(sub), (node_index + edge_count) as usize);
            }
        }
        if target_layer > 0 {
            edge_count += 1;
        }
    }

    // handle outgoing edges and following layers
    let mut extra_edge = false;
    for outgoing_edge in a.node_outgoing_edges(node) {
        let mut next_edge_to_split = outgoing_edge;
        for layer_index in (target_layer + 1)..(sub_layers.len() as i32) {
            let dummy_node = create_dummy_node(a, graph, next_edge_to_split);
            a.node_set_layer(dummy_node, Some(sub_layers[layer_index as usize]));
            next_edge_to_split = long_edge_splitter::split_edge(a, next_edge_to_split, dummy_node);
        }

        for layer_index in 0..=target_layer {
            if extra_edge {
                let placeholder = a.create_node(graph);
                a.node_mut(placeholder).node_type = NodeType::NONSHIFTING_PLACEHOLDER;
                let sub = sub_layers[layer_index as usize];
                if node_index + 1 > a.layer(sub).nodes.len() as i32 {
                    a.node_set_layer(placeholder, Some(sub));
                } else {
                    a.node_set_layer_at(placeholder, Some(sub), (node_index + 1) as usize);
                }
            }
        }

        if extra_edge {
            edge_count += 1;
        }
        extra_edge = true;
    }

    if edge_count > 0 {
        edge_count - 1
    } else {
        0
    }
}

fn create_dummy_node(a: &mut LGraphArena, graph: LGraphId, next_edge_to_split: LEdgeId) -> LNodeId {
    let dummy_node = a.create_node(graph);
    a.node_mut(dummy_node).node_type = NodeType::LONG_EDGE;
    a.node(dummy_node).properties.set(&iprops::ORIGIN, Origin::LEdge(next_edge_to_split));
    a.node(dummy_node).properties.set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
    dummy_node
}
