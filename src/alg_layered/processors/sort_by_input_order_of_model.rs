//!
//! Sorts each layer's nodes and ports by their `MODEL_ORDER`, using the
//! `ModelOrderNodeComparator` / `ModelOrderPortComparator`.

use crate::core::javacompat::tim_sort;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::TargetNodeModelOrder;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::p3order::model_order_comparators::{
    has_fixed_port_order, ModelOrderNodeComparator, ModelOrderPortComparator,
};

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let strategy = a.graph(graph).properties.get(&lopts::CONSIDER_MODEL_ORDER_STRATEGY);
    let long_edge_strategy =
        a.graph(graph).properties.get(&lopts::CONSIDER_MODEL_ORDER_LONG_EDGE_STRATEGY);
    let group_strategy = a
        .graph(graph)
        .properties
        .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CM_GROUP_ORDER_STRATEGY);
    let port_model_order = a.graph(graph).properties.get(&lopts::CONSIDER_MODEL_ORDER_PORT_MODEL_ORDER);

    let layers = a.graph(graph).layers.clone();

    for (layer_index, &layer) in layers.iter().enumerate() {
        // layer.id is necessary to check whether nodes really connect to the
        // previous layer.
        a.layer_mut(layer).id = layer_index as i32;
        let previous_layer_index = if layer_index == 0 { 0 } else { layer_index - 1 };
        let previous_layer_nodes = a.layer(layers[previous_layer_index]).nodes.clone();

        // Sort nodes before port sorting (for in-layer feedback edge dummies).
        {
            let mut comparator = ModelOrderNodeComparator::new(
                a,
                graph,
                previous_layer_nodes.clone(),
                strategy,
                long_edge_strategy,
                group_strategy,
                true,
            );
            let mut nodes = a.layer(layer).nodes.clone();
            insertion_sort_nodes(&mut nodes, &mut comparator);
            comparator.clear_transitive_ordering();
            set_layer_nodes(a, layer, nodes);
        }

        // Sort ports of each node.
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            if !has_fixed_port_order(a, node) {
                let tnmo = long_edge_target_node_preprocessing(a, node);
                let mut comparator = ModelOrderPortComparator::new(
                    a,
                    graph,
                    previous_layer_nodes.clone(),
                    strategy,
                    Some(tnmo),
                    port_model_order,
                );
                let mut ports: Vec<LPortId> = a.node(node).ports.clone();
                tim_sort(&mut ports, |&p1, &p2| comparator.compare(p1, p2));
                a.node_mut(node).ports = ports;
            }
        }

        // Sort nodes again (also sorts dummy feedback nodes correctly).
        {
            let mut comparator = ModelOrderNodeComparator::new(
                a,
                graph,
                previous_layer_nodes.clone(),
                strategy,
                long_edge_strategy,
                group_strategy,
                false,
            );
            let mut nodes = a.layer(layer).nodes.clone();
            insertion_sort_nodes(&mut nodes, &mut comparator);
            comparator.clear_transitive_ordering();
            set_layer_nodes(a, layer, nodes);
        }
    }

    Ok(())
}

/// Replace the node list of a layer (and fix each node's back-reference).
fn set_layer_nodes(a: &mut LGraphArena, layer: crate::alg_layered::graph::LayerId, nodes: Vec<LNodeId>) {
    a.layer_mut(layer).nodes = nodes;
}

fn insertion_sort_nodes(layer: &mut Vec<LNodeId>, comparator: &mut ModelOrderNodeComparator) {
    for i in 1..layer.len() {
        let temp = layer[i];
        let mut j = i;
        while j > 0 && comparator.compare(layer[j - 1], temp) > 0 {
            layer[j] = layer[j - 1];
            j -= 1;
        }
        layer[j] = temp;
    }
}

fn long_edge_target_node_preprocessing(a: &mut LGraphArena, node: LNodeId) -> TargetNodeModelOrder {
    if let Some(existing) = a.node(node).properties.try_get(&iprops::TARGET_NODE_MODEL_ORDER) {
        return existing;
    }
    let mut target_node_model_order: indexmap::IndexMap<LNodeId, i32> = indexmap::IndexMap::new();
    let ports: Vec<LPortId> = a
        .node(node)
        .ports
        .iter()
        .copied()
        .filter(|&p| !a.port(p).outgoing_edges.is_empty())
        .collect();
    for p in ports {
        let target_node = get_target_node(a, p);
        if let Some(tn) = target_node {
            a.port(p).properties.set(&iprops::LONG_EDGE_TARGET_NODE, tn);
        } else {
            a.port(p).properties.unset(&iprops::LONG_EDGE_TARGET_NODE);
        }
        if let Some(target_node) = target_node {
            let previous_order =
                *target_node_model_order.get(&target_node).unwrap_or(&i32::MAX);
            let edge = a.port(p).outgoing_edges[0];
            if !a.edge(edge).properties.get(&iprops::REVERSED) {
                let mo: i32 = a.edge(edge).properties.get(&iprops::MODEL_ORDER);
                target_node_model_order.insert(target_node, mo.min(previous_order));
            }
        }
    }
    let result = TargetNodeModelOrder(target_node_model_order);
    a.node(node).properties.set(&iprops::TARGET_NODE_MODEL_ORDER, result.clone());
    result
}

/// The target node of a port considering long edges.
fn get_target_node(a: &LGraphArena, port: LPortId) -> Option<LNodeId> {
    let mut edge = a.port(port).outgoing_edges[0];
    let mut node;
    loop {
        node = a.edge(edge).target.and_then(|t| a.port(t).node).unwrap();
        if let Some(let_) = a.node(node).properties.try_get(&iprops::LONG_EDGE_TARGET) {
            return a.port(let_).node;
        }
        if a.node(node).node_type != NodeType::NORMAL {
            if let Some(&next) = a.node_outgoing_edges(node).first() {
                edge = next;
            } else {
                return None;
            }
        }
        if a.node(node).node_type == NodeType::NORMAL {
            break;
        }
    }
    Some(node)
}
