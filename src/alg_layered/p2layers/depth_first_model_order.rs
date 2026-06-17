
use crate::core::javacompat::tim_sort;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;

struct Df {
    current_layer: LayerId,
    current_layer_id: i32,
    current_dummy_layer: Option<LayerId>,
    nodes_to_place: Vec<LNodeId>,
    max_to_place: i32,
}

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mut real_nodes: Vec<LNodeId> = a
        .graph(graph)
        .layerless_nodes
        .iter()
        .copied()
        .filter(|&n| a.node(n).node_type == NodeType::NORMAL)
        .collect();

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
                "The DF model order layer assigner requires all real nodes to have a model order."
                    .to_string(),
            );
            0
        }
    });
    if let Some(e) = error {
        return Err(e);
    }

    let mut first_node = true;
    let current_layer = a.create_layer(graph);
    a.graph_mut(graph).layers.push(current_layer);
    a.layer_mut(current_layer).id = 0;
    let mut st = Df {
        current_layer,
        current_layer_id: 0,
        current_dummy_layer: None,
        nodes_to_place: Vec::new(),
        max_to_place: 0,
    };

    for node in real_nodes {
        if first_node {
            a.node_set_layer(node, Some(st.current_layer));
            first_node = false;
        } else if is_connected_to_current_layer(a, &st, node) {
            let mut max_layer = st.current_layer_id;
            max_layer = get_max_connected_layer(a, max_layer, node);
            let desired_layer = max_layer + 2;
            let layer_diff = max_layer - st.current_layer_id;
            if !st.nodes_to_place.is_empty() {
                if layer_diff > 0 {
                    for &to_place in &st.nodes_to_place.clone() {
                        let id = a.node(to_place).id;
                        a.node_mut(to_place).id = id + (max_layer - st.max_to_place);
                    }
                    place_nodes_to_place(a, graph, &mut st);
                    st.nodes_to_place.clear();
                    add_node_to_layer(a, graph, &mut st, desired_layer, node);
                } else {
                    st.nodes_to_place.push(node);
                    a.node_mut(node).id = desired_layer;
                    st.max_to_place = st.max_to_place.max(desired_layer);
                    for edge in a.node_incoming_edges(node) {
                        let src = a.edge_source_node(edge);
                        if a.node(src).layer.is_none() && a.node(src).node_type == NodeType::LABEL {
                            st.nodes_to_place.push(src);
                            a.node_mut(src).id = desired_layer - 1;
                        }
                    }
                    st.current_layer_id = desired_layer;
                }
            } else {
                add_node_to_layer(a, graph, &mut st, desired_layer, node);
            }
        } else {
            // A new strip has to begin.
            place_nodes_to_place(a, graph, &mut st);
            st.nodes_to_place.clear();

            if a.node_incoming_edges(node).is_empty() {
                st.nodes_to_place.push(node);
                a.node_mut(node).id = 0;
                st.max_to_place = st.max_to_place.max(0);
                st.current_layer = a.graph(graph).layers[0];
                st.current_layer_id = 0;
            } else {
                let mut max_layer = 0;
                max_layer = get_max_connected_layer(a, max_layer, node);
                let desired_layer = max_layer + 2;
                add_node_to_layer(a, graph, &mut st, desired_layer, node);
            }
        }
    }

    if !st.nodes_to_place.is_empty() {
        place_nodes_to_place(a, graph, &mut st);
    }

    a.graph_mut(graph).layerless_nodes.clear();

    // Delete empty layers and reassign ids.
    let layers = a.graph(graph).layers.clone();
    let kept: Vec<LayerId> = layers.into_iter().filter(|&l| !a.layer(l).nodes.is_empty()).collect();
    a.graph_mut(graph).layers = kept.clone();
    for (i, &layer) in kept.iter().enumerate() {
        a.layer_mut(layer).id = i as i32;
    }

    Ok(())
}

fn is_connected_to_current_layer(a: &LGraphArena, st: &Df, node: LNodeId) -> bool {
    for edge in a.node_incoming_edges(node) {
        let src = a.edge_source_node(edge);
        let src_type = a.node(src).node_type;
        let (directly, via_dummy) = if st.nodes_to_place.is_empty() {
            let directly = src_type == NodeType::NORMAL
                && a.node(src).layer.is_some()
                && a.layer(a.node(src).layer.unwrap()).id == st.current_layer_id;
            let inc = a.node_incoming_edges(src);
            let via_dummy = if let Some(&first) = inc.first() {
                let ss = a.edge_source_node(first);
                src_type == NodeType::LABEL
                    && a.node(ss).layer.is_some()
                    && a.layer(a.node(ss).layer.unwrap()).id == st.current_layer_id
            } else {
                false
            };
            (directly, via_dummy)
        } else {
            let directly = src_type == NodeType::NORMAL && a.node(src).id == st.current_layer_id;
            // unconditionally takes the first incoming edge here.
            let inc = a.node_incoming_edges(src);
            let via_dummy = src_type == NodeType::LABEL
                && inc.first().map_or(false, |&first| {
                    a.node(a.edge_source_node(first)).id == st.current_layer_id
                });
            (directly, via_dummy)
        };
        if directly || via_dummy {
            return true;
        }
    }
    false
}

fn add_node_to_layer(a: &mut LGraphArena, graph: LGraphId, st: &mut Df, layer_id: i32, node: LNodeId) {
    if (layer_id as usize) < a.graph(graph).layers.len() {
        st.current_layer = a.graph(graph).layers[layer_id as usize];
        st.current_dummy_layer = Some(a.graph(graph).layers[(layer_id - 1) as usize]);
        st.current_layer_id = layer_id;
    } else {
        let dummy = a.create_layer(graph);
        a.layer_mut(dummy).id = layer_id - 1;
        a.graph_mut(graph).layers.push(dummy);
        st.current_dummy_layer = Some(dummy);
        let new_layer = a.create_layer(graph);
        a.layer_mut(new_layer).id = layer_id;
        a.graph_mut(graph).layers.push(new_layer);
        st.current_layer = new_layer;
        st.current_layer_id = layer_id;
    }
    a.node_set_layer(node, Some(st.current_layer));
    for edge in a.node_incoming_edges(node) {
        let src = a.edge_source_node(edge);
        if a.node(src).layer.is_none() && a.node(src).node_type == NodeType::LABEL {
            a.node_set_layer(src, st.current_dummy_layer);
        }
    }
}

fn get_max_connected_layer(a: &LGraphArena, layer_id: i32, node: LNodeId) -> i32 {
    let mut max_layer = layer_id;
    for edge in a.node_incoming_edges(node) {
        let src = a.edge_source_node(edge);
        if let Some(l) = a.node(src).layer {
            max_layer = max_layer.max(a.layer(l).id);
        }
    }
    max_layer
}

fn place_nodes_to_place(a: &mut LGraphArena, graph: LGraphId, st: &mut Df) {
    st.max_to_place = 0;
    for &node_to_place in &st.nodes_to_place.clone() {
        let id = a.node(node_to_place).id;
        if id as usize >= a.graph(graph).layers.len() {
            let dummy = a.create_layer(graph);
            a.layer_mut(dummy).id = id - 1;
            a.graph_mut(graph).layers.push(dummy);
            let new_layer = a.create_layer(graph);
            a.layer_mut(new_layer).id = id;
            a.graph_mut(graph).layers.push(new_layer);
        }
        let target = a.graph(graph).layers[id as usize];
        a.node_set_layer(node_to_place, Some(target));
    }
}
