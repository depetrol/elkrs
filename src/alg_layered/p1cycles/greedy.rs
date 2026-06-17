
use std::collections::VecDeque;

use crate::core::javacompat::JavaRandom;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_util::edge_reverse;
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId, random: &mut JavaRandom) -> Result<(), String> {
    process_impl(a, graph, random, false)
}

/// Same as `GreedyCycleBreaker` but the
/// tie-break among max-outflow nodes picks the minimum (group) model order.
pub fn process_model_order(
    a: &mut LGraphArena,
    graph: LGraphId,
    random: &mut JavaRandom,
) -> Result<(), String> {
    process_impl(a, graph, random, true)
}

fn process_impl(
    a: &mut LGraphArena,
    graph: LGraphId,
    random: &mut JavaRandom,
    model_order: bool,
) -> Result<(), String> {
    let nodes: Vec<LNodeId> = a.graph(graph).layerless_nodes.clone();
    let mut unprocessed_node_count = nodes.len() as i32;
    let n = nodes.len();
    let mut indeg = vec![0i32; n];
    let mut outdeg = vec![0i32; n];
    let mut mark = vec![0i32; n];
    let mut sources: VecDeque<LNodeId> = VecDeque::new();
    let mut sinks: VecDeque<LNodeId> = VecDeque::new();

    for (index, &node) in nodes.iter().enumerate() {
        a.node_mut(node).id = index as i32;

        for &port in &a.node(node).ports {
            for &edge in &a.port(port).incoming_edges {
                if a.edge_source_node(edge) == node {
                    continue;
                }
                let priority: i32 = a.edge(edge).properties.get(&lopts::PRIORITY_DIRECTION);
                indeg[index] += if priority > 0 { priority + 1 } else { 1 };
            }
            for &edge in &a.port(port).outgoing_edges {
                if a.edge_target_node(edge) == node {
                    continue;
                }
                let priority: i32 = a.edge(edge).properties.get(&lopts::PRIORITY_DIRECTION);
                outdeg[index] += if priority > 0 { priority + 1 } else { 1 };
            }
        }

        if outdeg[index] == 0 {
            sinks.push_back(node);
        } else if indeg[index] == 0 {
            sources.push_back(node);
        }
    }

    let mut next_right = -1i32;
    let mut next_left = 1i32;
    let mut max_nodes: Vec<LNodeId> = Vec::new();

    // helper closure equivalent of updateNeighbors
    fn update_neighbors(
        a: &LGraphArena,
        node: LNodeId,
        mark: &[i32],
        indeg: &mut [i32],
        outdeg: &mut [i32],
        sources: &mut VecDeque<LNodeId>,
        sinks: &mut VecDeque<LNodeId>,
    ) {
        for &port in &a.node(node).ports {
            // connected edges: incoming first, then outgoing
            let connected: Vec<(crate::alg_layered::graph::LEdgeId, bool)> = a
                .port(port)
                .incoming_edges
                .iter()
                .map(|&e| (e, true))
                .chain(a.port(port).outgoing_edges.iter().map(|&e| (e, false)))
                .collect();
            for (edge, _incoming) in connected {
                let connected_port = if a.edge(edge).source == Some(port) {
                    a.edge(edge).target.unwrap()
                } else {
                    a.edge(edge).source.unwrap()
                };
                let endpoint = a.port(connected_port).node.unwrap();
                if node == endpoint {
                    continue;
                }
                let mut priority: i32 = a.edge(edge).properties.get(&lopts::PRIORITY_DIRECTION);
                if priority < 0 {
                    priority = 0;
                }
                let index = a.node(endpoint).id as usize;
                if mark[index] == 0 {
                    if a.edge(edge).target == Some(connected_port) {
                        indeg[index] -= priority + 1;
                        if indeg[index] <= 0 && outdeg[index] > 0 {
                            sources.push_back(endpoint);
                        }
                    } else {
                        outdeg[index] -= priority + 1;
                        if outdeg[index] <= 0 && indeg[index] > 0 {
                            sinks.push_back(endpoint);
                        }
                    }
                }
            }
        }
    }

    while unprocessed_node_count > 0 {
        while let Some(sink) = sinks.pop_front() {
            mark[a.node(sink).id as usize] = next_right;
            next_right -= 1;
            update_neighbors(a, sink, &mark, &mut indeg, &mut outdeg, &mut sources, &mut sinks);
            unprocessed_node_count -= 1;
        }
        while let Some(source) = sources.pop_front() {
            mark[a.node(source).id as usize] = next_left;
            next_left += 1;
            update_neighbors(a, source, &mark, &mut indeg, &mut outdeg, &mut sources, &mut sinks);
            unprocessed_node_count -= 1;
        }

        if unprocessed_node_count > 0 {
            let mut max_outflow = i32::MIN;
            max_nodes.clear();
            for &node in &nodes {
                let id = a.node(node).id as usize;
                if mark[id] == 0 {
                    let outflow = outdeg[id] - indeg[id];
                    if outflow >= max_outflow {
                        if outflow > max_outflow {
                            max_nodes.clear();
                            max_outflow = outflow;
                        }
                        max_nodes.push(node);
                    }
                }
            }
            debug_assert!(max_outflow > i32::MIN);

            let max_node = choose_node_with_max_outflow(a, graph, &max_nodes, random, model_order);
            mark[a.node(max_node).id as usize] = next_left;
            next_left += 1;
            update_neighbors(a, max_node, &mark, &mut indeg, &mut outdeg, &mut sources, &mut sinks);
            unprocessed_node_count -= 1;
        }
    }

    // shift negative marks to be greater than positive ones
    let shift_base = nodes.len() as i32 + 1;
    for m in mark.iter_mut() {
        if *m < 0 {
            *m += shift_base;
        }
    }

    // reverse edges that point left
    for &node in &nodes {
        let ports = a.node(node).ports.clone();
        for port in ports {
            let outgoing = a.port(port).outgoing_edges.clone();
            for edge in outgoing {
                let target_ix = a.node(a.edge_target_node(edge)).id as usize;
                if mark[a.node(node).id as usize] > mark[target_ix] {
                    edge_reverse(a, graph, edge, true);
                    a.graph(graph).properties.set(&iprops::CYCLIC, true);
                }
            }
        }
    }

    Ok(())
}

fn choose_node_with_max_outflow(
    a: &LGraphArena,
    graph: LGraphId,
    max_nodes: &[LNodeId],
    random: &mut JavaRandom,
    model_order: bool,
) -> LNodeId {
    if model_order {
        let offset = (a.graph(graph).layerless_nodes.len() as i32)
            .max(a.graph(graph).properties.get(&iprops::MAX_MODEL_ORDER_NODES));
        let big_offset =
            offset.wrapping_mul(a.graph(graph).properties.get(&iprops::CB_NUM_MODEL_ORDER_GROUPS));
        let enforce_group = a
            .graph(graph)
            .properties
            .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CB_GROUP_ORDER_STRATEGY)
            == crate::alg_layered::options_gen::GroupOrderStrategy::ENFORCED;
        let mut calc = super::group_model_order_calculator::GroupModelOrderCalculator::new();
        let mut return_node: Option<LNodeId> = None;
        let mut minimum_model_order = i32::MAX;
        for &node in max_nodes {
            if a.node(node).properties.has(&iprops::MODEL_ORDER) {
                let mo = if enforce_group {
                    calc.compute_constraint_group_model_order(a, node, big_offset, offset)
                } else {
                    calc.compute_constraint_model_order(a, node, offset)
                };
                if minimum_model_order > mo {
                    minimum_model_order = mo;
                    return_node = Some(node);
                }
            }
        }
        if let Some(n) = return_node {
            return n;
        }
    }
    max_nodes[random.next_int_bound(max_nodes.len() as i32) as usize]
}
