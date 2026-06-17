
use std::collections::BTreeMap;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LEdgeId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_util::edge_reverse;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::GroupOrderStrategy;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let nodes: Vec<LNodeId> = a.graph(graph).layerless_nodes.clone();
    let node_count = nodes.len();
    let mut visited = vec![false; node_count];
    let mut active = vec![false; node_count];
    let mut sources: Vec<LNodeId> = Vec::new();
    let mut edges_to_be_reversed: Vec<LEdgeId> = Vec::new();

    for (index, &node) in nodes.iter().enumerate() {
        a.node_mut(node).id = index as i32;
        if a.node_incoming_edges(node).is_empty() {
            sources.push(node);
        }
    }

    let group_model_order = a
        .graph(graph)
        .properties
        .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CB_GROUP_ORDER_STRATEGY)
        == GroupOrderStrategy::ENFORCED;

    for source in sources {
        dfs(a, source, &mut visited, &mut active, &mut edges_to_be_reversed, group_model_order);
    }
    for i in 0..node_count {
        if !visited[i] {
            dfs(a, nodes[i], &mut visited, &mut active, &mut edges_to_be_reversed, group_model_order);
        }
    }

    for edge in edges_to_be_reversed {
        edge_reverse(a, graph, edge, true);
        a.graph(graph).properties.set(&iprops::CYCLIC, true);
    }

    Ok(())
}

fn dfs(
    a: &mut LGraphArena,
    node: LNodeId,
    visited: &mut [bool],
    active: &mut [bool],
    edges_to_be_reversed: &mut Vec<LEdgeId>,
    group_model_order: bool,
) {
    let nid = a.node(node).id as usize;
    if visited[nid] {
        return;
    }
    visited[nid] = true;
    active[nid] = true;

    let mut model_order_map: BTreeMap<i32, Vec<LEdgeId>> = BTreeMap::new();
    for e in a.node_outgoing_edges(node) {
        let target = a.edge_target_node(e);
        if !a.node(target).properties.has(&iprops::MODEL_ORDER) {
            let key = i32::MAX - model_order_map.len() as i32;
            model_order_map.entry(key).or_default().push(e);
        } else {
            let target_model_order = if group_model_order {
                let max_group_size: i32 =
                    a.graph(a.node_graph(node)).properties.get(&iprops::MAX_MODEL_ORDER_NODES);
                max_group_size
                    .wrapping_mul(a.node(target).properties.get(
                        &lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CYCLE_BREAKING_ID,
                    ))
                    .wrapping_add(a.node(target).properties.get(&iprops::MODEL_ORDER))
            } else {
                a.node(target).properties.get(&iprops::MODEL_ORDER)
            };
            model_order_map.entry(target_model_order).or_default().push(e);
        }
    }

    let keys: Vec<i32> = model_order_map.keys().copied().collect();
    for key in keys {
        let out = model_order_map[&key][0];
        if a.edge_is_self_loop(out) {
            continue;
        }
        let target = a.edge_target_node(out);
        let tid = a.node(target).id as usize;
        if active[tid] {
            edges_to_be_reversed.extend(model_order_map[&key].iter().copied());
        } else {
            dfs(a, target, visited, active, edges_to_be_reversed, group_model_order);
        }
    }

    active[nid] = false;
}
