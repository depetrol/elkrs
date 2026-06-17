
use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_util::edge_reverse;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let nodes: Vec<LNodeId> = a.graph(graph).layerless_nodes.clone();
    let node_count = nodes.len();

    let mut sources: Vec<LNodeId> = Vec::new();
    let mut visited = vec![false; node_count];
    let mut active = vec![false; node_count];
    let mut edges_to_be_reversed: Vec<LEdgeId> = Vec::new();

    for (index, &node) in nodes.iter().enumerate() {
        a.node_mut(node).id = index as i32;
        if a.node_incoming_edges(node).is_empty() {
            sources.push(node);
        }
    }

    fn dfs(
        a: &LGraphArena,
        n: LNodeId,
        visited: &mut [bool],
        active: &mut [bool],
        edges_to_be_reversed: &mut Vec<LEdgeId>,
    ) {
        let id = a.node(n).id as usize;
        if visited[id] {
            return;
        }
        visited[id] = true;
        active[id] = true;

        for out in a.node_outgoing_edges(n) {
            if a.edge_is_self_loop(out) {
                continue;
            }
            let target = a.edge_target_node(out);
            if active[a.node(target).id as usize] {
                edges_to_be_reversed.push(out);
            } else {
                dfs(a, target, visited, active, edges_to_be_reversed);
            }
        }
        active[id] = false;
    }

    for &source in &sources {
        dfs(a, source, &mut visited, &mut active, &mut edges_to_be_reversed);
    }
    for i in 0..node_count {
        if !visited[i] {
            let n = nodes[i];
            debug_assert_eq!(a.node(n).id as usize, i);
            dfs(a, n, &mut visited, &mut active, &mut edges_to_be_reversed);
        }
    }

    for edge in edges_to_be_reversed {
        edge_reverse(a, graph, edge, true);
        a.graph(graph).properties.set(&iprops::CYCLIC, true);
    }
    Ok(())
}
