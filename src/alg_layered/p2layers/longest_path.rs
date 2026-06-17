//!
//! The most basic layering algorithm, which assigns layers according to the
//! longest path to a sink.

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId};

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let nodes: Vec<LNodeId> = a.graph(graph).layerless_nodes.clone();

    // initialize values required for the computation
    let mut node_heights = vec![0i32; nodes.len()];
    for (index, &node) in nodes.iter().enumerate() {
        // the node id is used as index for the nodeHeights array
        a.node_mut(node).id = index as i32;
        node_heights[index] = -1;
    }

    // process all nodes
    for &node in &nodes {
        visit(a, graph, &mut node_heights, node);
    }

    // empty the list of unlayered nodes
    a.graph_mut(graph).layerless_nodes.clear();

    Ok(())
}

/// If not already visited, find the longest path to a
/// sink. Returns the height of the given node in the layered graph.
fn visit(a: &mut LGraphArena, graph: LGraphId, node_heights: &mut [i32], node: LNodeId) -> i32 {
    let height = node_heights[a.node(node).id as usize];
    if height >= 0 {
        // the node was already visited (the case height == 0 should never occur)
        height
    } else {
        let mut max_height = 1;
        for port in a.node(node).ports.clone() {
            for edge in a.port(port).outgoing_edges.clone() {
                let target_node = a.edge_target_node(edge);

                // ignore self-loops
                if node != target_node {
                    let target_height = visit(a, graph, node_heights, target_node);
                    max_height = max_height.max(target_height + 1);
                }
            }
        }
        put_node(a, graph, node_heights, node, max_height);
        max_height
    }
}

/// Puts the given node into the layered graph,
/// adding new layers at the front as necessary (height = number of layers -
/// layer index).
fn put_node(a: &mut LGraphArena, graph: LGraphId, node_heights: &mut [i32], node: LNodeId, height: i32) {
    // add layers so as to guarantee that number of layers >= height
    let mut i = a.graph(graph).layers.len() as i32;
    while i < height {
        let layer = a.create_layer(graph);
        a.graph_mut(graph).layers.insert(0, layer);
        i += 1;
    }

    // layer index = number of layers - height
    let num_layers = a.graph(graph).layers.len();
    let layer = a.graph(graph).layers[num_layers - height as usize];
    a.node_set_layer(node, Some(layer));
    node_heights[a.node(node).id as usize] = height;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alg_layered::graph::LEdgeId;

    fn make_node(a: &mut LGraphArena, graph: LGraphId) -> LNodeId {
        let n = a.create_node(graph);
        a.graph_mut(graph).layerless_nodes.push(n);
        n
    }

    fn connect(a: &mut LGraphArena, source: LNodeId, target: LNodeId) -> LEdgeId {
        let sp = a.create_port();
        a.port_set_node(sp, Some(source));
        let tp = a.create_port();
        a.port_set_node(tp, Some(target));
        let e = a.create_edge();
        a.edge_set_source(e, Some(sp));
        a.edge_set_target(e, Some(tp));
        e
    }

    fn layer_nodes(a: &LGraphArena, graph: LGraphId) -> Vec<Vec<LNodeId>> {
        a.graph(graph)
            .layers
            .iter()
            .map(|&l| a.layer(l).nodes.clone())
            .collect()
    }

    /// Diamond plus chain tail: n0->n1, n0->n2, n1->n3, n2->n3, n3->n4.
    /// Heights: n4=1, n3=2, n1=n2=3, n0=4 -> four layers, n1 placed before
    /// n2 in layer 1 (DFS visits n1's branch first).
    #[test]
    fn diamond_with_chain() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let n: Vec<LNodeId> = (0..5).map(|_| make_node(&mut a, g)).collect();
        connect(&mut a, n[0], n[1]);
        connect(&mut a, n[0], n[2]);
        connect(&mut a, n[1], n[3]);
        connect(&mut a, n[2], n[3]);
        connect(&mut a, n[3], n[4]);

        process(&mut a, g).unwrap();

        assert_eq!(
            layer_nodes(&a, g),
            vec![vec![n[0]], vec![n[1], n[2]], vec![n[3]], vec![n[4]]]
        );
        assert!(a.graph(g).layerless_nodes.is_empty());
    }

    /// A node whose only outgoing edge is a self-loop is a sink (height 1);
    /// the self-loop must be ignored during traversal.
    #[test]
    fn self_loop_is_ignored() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let n1 = make_node(&mut a, g);
        let n2 = make_node(&mut a, g);
        connect(&mut a, n1, n2);
        connect(&mut a, n2, n2);

        process(&mut a, g).unwrap();

        assert_eq!(layer_nodes(&a, g), vec![vec![n1], vec![n2]]);
    }
}
