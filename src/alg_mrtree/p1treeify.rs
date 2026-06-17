//! Removes the
//! edges that destroy the tree property and stores them for reinsertion by
//! the `Untreeifyer` after node placement.

use crate::alg_mrtree::graph::{TArena, TEdgeId, TGraph, TNodeId};
use crate::alg_mrtree::options;
use crate::alg_mrtree::options::TreeifyingOrder;

pub fn process(arena: &mut TArena, graph: &mut TGraph) {
    // init: number the nodes
    let mut id = 0;
    for &node in &graph.nodes {
        arena.node_mut(node).id = id;
        id += 1;
    }
    let size = graph.nodes.len();
    let mut visited = vec![0u8; size];
    let mut eliminated: Vec<TEdgeId> = Vec::new();

    collect_edges(arena, graph, &mut visited, &mut eliminated);
}

fn collect_edges(
    arena: &mut TArena,
    graph: &mut TGraph,
    visited: &mut [u8],
    eliminated: &mut Vec<TEdgeId>,
) {
    let treeifying_order: TreeifyingOrder = graph.properties.get(&options::SEARCH_ORDER);

    // start DFS on every node in graph
    for &tnode in &graph.nodes {
        if visited[arena.node(tnode).id as usize] == 0 {
            match treeifying_order {
                TreeifyingOrder::DFS => dfs(arena, tnode, visited, eliminated),
                TreeifyingOrder::BFS => bfs(arena, tnode, visited, eliminated),
            }
            // if we come back to that node again, set the node as root
            visited[arena.node(tnode).id as usize] = 2;
        }
    }

    // remove the found edges out of the graph structure (first occurrence,
    // like LinkedList.remove(Object))
    for &tedge in eliminated.iter() {
        let (source, target) = {
            let e = arena.edge(tedge);
            (e.source, e.target)
        };
        if let Some(pos) = arena.node(source).outgoing.iter().position(|&e| e == tedge) {
            arena.node_mut(source).outgoing.remove(pos);
        }
        if let Some(pos) = arena.node(target).incoming.iter().position(|&e| e == tedge) {
            arena.node_mut(target).incoming.remove(pos);
        }
    }

    // set the list of collected edges as a graph property
    graph.removable_edges = std::mem::take(eliminated);
}

fn dfs(arena: &TArena, tnode: TNodeId, visited: &mut [u8], eliminated: &mut Vec<TEdgeId>) {
    visited[arena.node(tnode).id as usize] = 1;

    for &tedge in &arena.node(tnode).outgoing {
        let target = arena.edge(tedge).target;
        let tid = arena.node(target).id as usize;
        if visited[tid] == 1 {
            // put that edge to the list that contains the edges to remove
            eliminated.push(tedge);
        } else if visited[tid] == 2 {
            // if a previous root can be visited from another node, unmark
            // the root property
            visited[tid] = 1;
        } else {
            dfs(arena, target, visited, eliminated);
        }
    }
}

fn bfs(arena: &TArena, start_node: TNodeId, visited: &mut [u8], eliminated: &mut Vec<TEdgeId>) {
    let mut node_queue: std::collections::VecDeque<TNodeId> = std::collections::VecDeque::new();
    node_queue.push_back(start_node);

    loop {
        let node = node_queue.pop_front().expect("bfs queue empty");
        visited[arena.node(node).id as usize] = 1;

        for &tedge in &arena.node(node).outgoing {
            let target = arena.edge(tedge).target;
            let tid = arena.node(target).id as usize;
            if visited[tid] == 1 {
                eliminated.push(tedge);
            } else if visited[tid] == 2 {
                visited[tid] = 1;
            } else {
                node_queue.push_back(target);
            }
        }

        if node_queue.is_empty() {
            break;
        }
    }
}
