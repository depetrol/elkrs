//! Finds regular nodes with self loops and
//! postprocesses those loops (restores edges and places labels).

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::loops::SelfLoopHolder;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for l_node in nodes {
            if a.node(l_node).node_type == NodeType::NORMAL
                && a.node(l_node).self_loop_holder.is_some()
            {
                let sl_holder = a.node_mut(l_node).self_loop_holder.take().unwrap();
                process_node(a, l_node, &sl_holder);
                a.node_mut(l_node).self_loop_holder = Some(sl_holder);
            }
        }
    }
    Ok(())
}

fn process_node(a: &mut LGraphArena, l_node: LNodeId, sl_holder: &SelfLoopHolder) {
    for sl_loop in &sl_holder.sl_hyper_loops {
        for &sl_edge in &sl_loop.sl_edges {
            restore_edge(a, l_node, sl_holder, sl_edge);
        }
    }

    for sl_loop in &sl_holder.sl_hyper_loops {
        if let Some(sl_labels) = &sl_loop.sl_labels {
            sl_labels.apply_placement(a, a.node(l_node).pos);
        }
    }
}

/// `restoreEdge`.
fn restore_edge(
    a: &mut LGraphArena,
    l_node: LNodeId,
    sl_holder: &SelfLoopHolder,
    sl_edge: crate::alg_layered::loops::SlEdgeIdx,
) {
    let edge = &sl_holder.sl_edges[sl_edge];
    let l_edge = edge.l_edge;
    a.edge_set_source(l_edge, Some(sl_holder.sl_ports[edge.sl_source].l_port));
    a.edge_set_target(l_edge, Some(sl_holder.sl_ports[edge.sl_target].l_port));

    let node_pos = a.node(l_node).pos;
    a.edge_mut(l_edge).bend_points.offset(node_pos);
}
