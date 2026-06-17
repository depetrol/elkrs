//!
//! A cycle breaker that responds to user interaction by respecting the
//! direction of edges as given in the original drawing.

use crate::graph::math::KVector;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId};
use crate::alg_layered::lgraph_util::edge_reverse;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::InteractiveReferencePoint;

/// Returns the node's anchor
/// point position, depending on the graph's `INTERACTIVE_REFERENCE_POINT`
/// property (CENTER vs TOP_LEFT).
pub fn interactive_reference_point(a: &LGraphArena, node: LNodeId) -> KVector {
    let graph = a.node(node).graph.unwrap();
    let mode: InteractiveReferencePoint =
        a.graph(graph).properties.get(&lopts::INTERACTIVE_REFERENCE_POINT);
    let pos = a.node(node).pos;
    let size = a.node(node).size;
    match mode {
        InteractiveReferencePoint::CENTER => {
            KVector::new(pos.x + size.x / 2.0, pos.y + size.y / 2.0)
        }
        InteractiveReferencePoint::TOP_LEFT => KVector::new(pos.x, pos.y),
    }
}

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // gather edges that point to the wrong direction
    let mut rev_edges: Vec<LEdgeId> = Vec::new();
    let nodes = a.graph(graph).layerless_nodes.clone();
    for &source in &nodes {
        a.node_mut(source).id = 1;
        let sourcex = interactive_reference_point(a, source).x;
        for port in a.node_output_ports(source) {
            for edge in a.port(port).outgoing_edges.clone() {
                let target = a.edge_target_node(edge);
                if target != source {
                    let targetx = interactive_reference_point(a, target).x;
                    if targetx < sourcex {
                        rev_edges.push(edge);
                    }
                }
            }
        }
    }
    // reverse the gathered edges
    for &edge in &rev_edges {
        edge_reverse(a, graph, edge, true);
    }

    // perform an additional check for cycles - maybe we missed something
    // (could happen if some nodes have the same horizontal position)
    rev_edges.clear();
    for &node in &nodes {
        // unvisited nodes have id = 1
        if a.node(node).id > 0 {
            find_cycles(a, node, &mut rev_edges);
        }
    }
    // again, reverse the edges that were marked
    for &edge in &rev_edges {
        edge_reverse(a, graph, edge, true);
    }

    Ok(())
}

/// Perform a DFS starting on the given node and mark back edges in order to
/// break cycles.
fn find_cycles(a: &mut LGraphArena, node1: LNodeId, rev_edges: &mut Vec<LEdgeId>) {
    // nodes with negative id are part of the currently inspected path
    a.node_mut(node1).id = -1;
    for port in a.node_output_ports(node1) {
        for edge in a.port(port).outgoing_edges.clone() {
            let node2 = a.edge_target_node(edge);
            if node1 != node2 {
                if a.node(node2).id < 0 {
                    // a node of the current path is found --> cycle
                    rev_edges.push(edge);
                } else if a.node(node2).id > 0 {
                    // the node has not been visited yet --> expand the current path
                    find_cycles(a, node2, rev_edges);
                }
            }
        }
    }
    // nodes with id = 0 have been already visited and are ignored if
    // encountered again
    a.node_mut(node1).id = 0;
}
