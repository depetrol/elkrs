//!
//! Determines an optimal layering of all nodes in the graph concerning a
//! minimal length of all edges using the network simplex algorithm (Gansner
//! et al.). Each connected component is layered separately; the component
//! with the most nodes is layered first.

use std::collections::{HashMap, VecDeque};

use crate::alg_common::networksimplex::{NGraph, NNodeId, NetworkSimplex};

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LPortId};
use crate::alg_layered::options_gen as lopts;

/// Factor by which the maximal number of iterations is multiplied.
const ITER_LIMIT_FACTOR: i32 = 4;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let thoroughness: i32 = a
        .graph(graph)
        .properties
        .get(&lopts::THOROUGHNESS)
        .wrapping_mul(ITER_LIMIT_FACTOR);

    let the_nodes: Vec<LNodeId> = a.graph(graph).layerless_nodes.clone();
    if the_nodes.is_empty() {
        return Ok(());
    }

    // layer graph, each connected component separately
    let connected_components = connected_components(a, &the_nodes);
    let num_components = connected_components.len();
    let mut previous_layering_node_counts: Option<Vec<i32>> = None;
    for conn_comp in &connected_components {
        // determine a limit on the number of iterations
        let iter_limit = thoroughness.wrapping_mul((conn_comp.len() as f64).sqrt() as i32);

        let mut ngraph = initialize(a, conn_comp);

        // execute the network simplex algorithm on the (sub-)graph
        NetworkSimplex::for_graph(&mut ngraph)
            .with_iteration_limit(iter_limit)
            .with_previous_layering(previous_layering_node_counts.clone())
            .with_balancing(true)
            .execute();

        // the layers are stored in the NNode's layer field.
        for &nnode in &ngraph.nodes {
            // add additional layers to match required number
            while a.graph(graph).layers.len() <= ngraph.node(nnode).layer as usize {
                let layer = a.create_layer(graph);
                a.graph_mut(graph).layers.push(layer);
            }
            let lnode = LNodeId(ngraph.node(nnode).origin as u32);
            let layer = a.graph(graph).layers[ngraph.node(nnode).layer as usize];
            a.node_set_layer(lnode, Some(layer));
        }

        if num_components > 1 {
            let mut counts = vec![0i32; a.graph(graph).layers.len()];
            for (layer_idx, &l) in a.graph(graph).layers.iter().enumerate() {
                counts[layer_idx] = a.layer(l).nodes.len() as i32;
            }
            previous_layering_node_counts = Some(counts);
        }
    }

    // empty the list of unlayered nodes
    a.graph_mut(graph).layerless_nodes.clear();

    Ok(())
}

/// Determines all connected
/// components of the graph. The component with the most nodes is kept at the
/// front of the list.
fn connected_components(a: &mut LGraphArena, the_nodes: &[LNodeId]) -> VecDeque<Vec<LNodeId>> {
    // initialize required attributes
    let mut node_visited = vec![false; the_nodes.len()];

    // re-index nodes
    let mut counter = 0;
    for &node in the_nodes {
        a.node_mut(node).id = counter;
        counter += 1;
    }
    // determine connected components
    let mut components: VecDeque<Vec<LNodeId>> = VecDeque::new();
    for &node in the_nodes {
        if !node_visited[a.node(node).id as usize] {
            let mut component_nodes = Vec::new();
            connected_components_dfs(a, node, &mut node_visited, &mut component_nodes);
            // connected component with the most nodes should be layered first to guarantee
            // reusability of attribute instances
            if components.is_empty() || components.front().unwrap().len() < component_nodes.len() {
                components.push_front(component_nodes);
            } else {
                components.push_back(component_nodes);
            }
        }
    }
    components
}

/// Adds all nodes connected to
/// `node` to `component_nodes`.
fn connected_components_dfs(
    a: &LGraphArena,
    node: LNodeId,
    node_visited: &mut [bool],
    component_nodes: &mut Vec<LNodeId>,
) {
    node_visited[a.node(node).id as usize] = true;

    // node is part of the current connected component
    component_nodes.push(node);

    // continue with next nodes, if not already visited
    for &port in &a.node(node).ports {
        for edge in a.port_connected_edges(port) {
            let opposite = a.port(get_opposite(a, port, edge)).node.unwrap();
            if !node_visited[a.node(opposite).id as usize] {
                connected_components_dfs(a, opposite, node_visited, component_nodes);
            }
        }
    }
}

/// Builds the `NGraph` for one connected
/// component. Each `NNode`'s `origin` holds the raw id of its `LNodeId`.
fn initialize(a: &LGraphArena, the_nodes: &[LNodeId]) -> NGraph {
    let mut node_map: HashMap<LNodeId, NNodeId> = HashMap::new();

    // transform nodes
    let mut graph = NGraph::new();
    for &lnode in the_nodes {
        let nnode = graph.add_node();
        graph.node_mut(nnode).origin = lnode.0 as i32;
        node_map.insert(lnode, nnode);
    }

    // transform edges
    for &lnode in the_nodes {
        for ledge in a.node_outgoing_edges(lnode) {
            // ignore self-loops
            if a.edge_is_self_loop(ledge) {
                continue;
            }

            let shortness: i32 = a.edge(ledge).properties.get(&lopts::PRIORITY_SHORTNESS);
            graph.add_edge(
                node_map[&a.edge_source_node(ledge)],
                node_map[&a.edge_target_node(ledge)],
                (1 * shortness.max(1)) as f64,
                1,
            );
        }
    }

    graph
}

/// The port connected to the opposite
/// side of the edge from the viewpoint of the input port.
fn get_opposite(a: &LGraphArena, port: LPortId, edge: LEdgeId) -> LPortId {
    let e = a.edge(edge);
    if e.source == Some(port) {
        return e.target.unwrap();
    } else if e.target == Some(port) {
        return e.source.unwrap();
    }
    panic!("Input edge is not connected to the input port.");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a node with `graph` membership, registered as layerless.
    fn make_node(a: &mut LGraphArena, graph: LGraphId) -> LNodeId {
        let n = a.create_node(graph);
        a.graph_mut(graph).layerless_nodes.push(n);
        n
    }

    /// Connects two nodes with a fresh edge through fresh ports.
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

    /// Two components: a chain n1->n2->n3 and a single node n4. The chain is
    /// layered first (larger component); the single node is then balanced
    /// into the (least filled) layer 0.
    #[test]
    fn chain_and_isolated_node() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let n1 = make_node(&mut a, g);
        let n2 = make_node(&mut a, g);
        let n3 = make_node(&mut a, g);
        let n4 = make_node(&mut a, g);
        connect(&mut a, n1, n2);
        connect(&mut a, n2, n3);

        process(&mut a, g).unwrap();

        assert_eq!(layer_nodes(&a, g), vec![vec![n1, n4], vec![n2], vec![n3]]);
        assert!(a.graph(g).layerless_nodes.is_empty());
    }

    /// Diamond plus chain tail: n0->n1, n0->n2, n1->n3, n2->n3, n3->n4.
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

    /// Self-loops must be ignored.
    #[test]
    fn self_loop_is_ignored() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let n1 = make_node(&mut a, g);
        let n2 = make_node(&mut a, g);
        connect(&mut a, n1, n1);
        connect(&mut a, n1, n2);

        process(&mut a, g).unwrap();

        assert_eq!(layer_nodes(&a, g), vec![vec![n1], vec![n2]]);
    }

    /// PRIORITY_SHORTNESS weights the edge: heavy edge m->z is kept short,
    /// so m moves towards z (and the balancer moves it back into the
    /// emptier layer 1 afterwards).
    #[test]
    fn priority_shortness_weights_edges() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let na = make_node(&mut a, g);
        let nb = make_node(&mut a, g);
        let nc = make_node(&mut a, g);
        let nz = make_node(&mut a, g);
        let nm = make_node(&mut a, g);
        connect(&mut a, na, nb);
        connect(&mut a, na, nm);
        connect(&mut a, nb, nc);
        connect(&mut a, nc, nz);
        let heavy = connect(&mut a, nm, nz);
        a.edge(heavy).properties.set(&lopts::PRIORITY_SHORTNESS, 10);

        process(&mut a, g).unwrap();

        assert_eq!(
            layer_nodes(&a, g),
            vec![vec![na], vec![nb, nm], vec![nc], vec![nz]]
        );
    }
}
