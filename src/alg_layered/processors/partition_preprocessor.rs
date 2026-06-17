//! Reverses edges that connect
//! higher-index to lower-index partitions.

use std::collections::HashSet;
use std::collections::VecDeque;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId};
use crate::alg_layered::lgraph_util::edge_reverse;
use crate::alg_layered::options_gen as lopts;

/// The priority to set on added constraint edges (arbitrary, large value).
const PARTITION_CONSTRAINT_EDGE_PRIORITY: i32 = 1_000;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // Collect partitioned layerless nodes (in declaration order).
    let layerless = a.graph(graph).layerless_nodes.clone();
    let partitioned_nodes: Vec<LNodeId> = layerless
        .iter()
        .copied()
        .filter(|&n| a.node(n).properties.has(&lopts::PARTITIONING_PARTITION))
        .collect();

    // Find all edges that must be reversed (two-step to avoid concurrent mutation).
    let mut edges_to_be_reversed: Vec<LEdgeId> = Vec::new();
    for &node in &partitioned_nodes {
        for edge in a.node_outgoing_edges(node) {
            if must_be_reversed(a, edge, &partitioned_nodes) {
                edges_to_be_reversed.push(edge);
            }
        }
    }

    for edge in edges_to_be_reversed {
        reverse(a, graph, edge);
    }

    Ok(())
}

fn must_be_reversed(a: &LGraphArena, edge: LEdgeId, partitioned_nodes: &[LNodeId]) -> bool {
    let source_node = a.edge_source_node(edge);
    let target_node = a.edge_target_node(edge);

    if a.node(target_node).properties.has(&lopts::PARTITIONING_PARTITION) {
        let source_partition: i32 = a.node(source_node).properties.get(&lopts::PARTITIONING_PARTITION);
        let target_partition: i32 = a.node(target_node).properties.get(&lopts::PARTITIONING_PARTITION);
        source_partition > target_partition
    } else {
        let source_partition: i32 =
            a.node(source_node).properties.get(&lopts::PARTITIONING_PARTITION);
        let partitioned_with_lower: HashSet<LNodeId> = partitioned_nodes
            .iter()
            .copied()
            .filter(|&n| {
                a.node(n).properties.get::<i32>(&lopts::PARTITIONING_PARTITION) < source_partition
            })
            .collect();

        // BFS from the source node.
        let mut queue: VecDeque<LNodeId> = VecDeque::new();
        let mut visited: HashSet<LNodeId> = HashSet::new();
        queue.push_back(source_node);
        visited.insert(source_node);
        while let Some(current) = queue.pop_front() {
            if partitioned_with_lower.contains(&current) {
                return true;
            }
            for out in a.node_outgoing_edges(current) {
                let target = a.edge_target_node(out);
                if !visited.contains(&target) {
                    visited.insert(target);
                    queue.push_back(target);
                }
            }
        }
        false
    }
}

fn reverse(a: &mut LGraphArena, graph: LGraphId, edge: LEdgeId) {
    edge_reverse(a, graph, edge, true);

    // Add base priority on top of any user priority.
    let mut priority = PARTITION_CONSTRAINT_EDGE_PRIORITY;
    if a.edge(edge).properties.has(&lopts::PRIORITY_DIRECTION) {
        priority += a.edge(edge).properties.get::<i32>(&lopts::PRIORITY_DIRECTION);
    }
    a.edge(edge).properties.set(&lopts::PRIORITY_DIRECTION, priority);
}
