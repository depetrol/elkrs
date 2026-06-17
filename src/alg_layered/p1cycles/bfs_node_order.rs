
use std::collections::{BTreeMap, VecDeque};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LEdgeId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_util::edge_reverse;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::GroupOrderStrategy;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let nodes: Vec<LNodeId> = a.graph(graph).layerless_nodes.clone();
    let n = nodes.len();
    let mut visited = vec![false; n];
    let mut is_source = vec![false; n];
    let mut is_sink = vec![false; n];
    let mut sources: Vec<LNodeId> = Vec::new();
    let mut bfs_queue: VecDeque<LNodeId> = VecDeque::new();
    let mut edges_to_be_reversed: Vec<LEdgeId> = Vec::new();

    for (index, &node) in nodes.iter().enumerate() {
        a.node_mut(node).id = index as i32;
        if a.node_incoming_edges(node).is_empty() {
            sources.push(node);
            is_source[index] = true;
        }
        if a.node_outgoing_edges(node).is_empty() {
            is_sink[index] = true;
        }
    }

    let group_model_order = a
        .graph(graph)
        .properties
        .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CB_GROUP_ORDER_STRATEGY)
        == GroupOrderStrategy::ENFORCED;

    for source in sources {
        bfs_queue.push_back(source);
        bfs_loop(a, &mut bfs_queue, &mut visited, &is_source, &is_sink, &mut edges_to_be_reversed, group_model_order);
    }
    bfs_loop(a, &mut bfs_queue, &mut visited, &is_source, &is_sink, &mut edges_to_be_reversed, group_model_order);

    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..n {
            if !visited[i] {
                bfs_queue.push_back(nodes[i]);
                changed = true;
                break;
            }
        }
        bfs_loop(a, &mut bfs_queue, &mut visited, &is_source, &is_sink, &mut edges_to_be_reversed, group_model_order);
    }

    for edge in edges_to_be_reversed {
        edge_reverse(a, graph, edge, true);
        a.graph(graph).properties.set(&iprops::CYCLIC, true);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn bfs_loop(
    a: &mut LGraphArena,
    bfs_queue: &mut VecDeque<LNodeId>,
    visited: &mut [bool],
    is_source: &[bool],
    is_sink: &[bool],
    edges_to_be_reversed: &mut Vec<LEdgeId>,
    group_model_order: bool,
) {
    while let Some(node) = bfs_queue.pop_front() {
        bfs(a, node, bfs_queue, visited, is_source, is_sink, edges_to_be_reversed, group_model_order);
    }
}

#[allow(clippy::too_many_arguments)]
fn bfs(
    a: &LGraphArena,
    node: LNodeId,
    bfs_queue: &mut VecDeque<LNodeId>,
    visited: &mut [bool],
    is_source: &[bool],
    is_sink: &[bool],
    edges_to_be_reversed: &mut Vec<LEdgeId>,
    group_model_order: bool,
) {
    let nid = a.node(node).id as usize;
    if visited[nid] {
        return;
    }
    visited[nid] = true;

    // BTreeMap mirrors the TreeSet<Integer> iteration order over the keys.
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
        if visited[tid] && !is_source[nid] && !is_sink[tid] {
            edges_to_be_reversed.extend(model_order_map[&key].iter().copied());
        } else {
            bfs_queue.push_back(target);
        }
    }
}
