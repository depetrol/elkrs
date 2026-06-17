//! Port of `org.eclipse.elk.alg.common.networksimplex.NetworkSimplexTest`
//! (elk/test/org.eclipse.elk.alg.common.test).

use elkrs::alg_common::networksimplex::{NGraph, NetworkSimplex};
use elkrs::core::javacompat::JavaRandom;

/// Java `NetworkSimplexTest.generateRandomGraph`.
fn generate_random_graph(random: &mut JavaRandom) -> NGraph {
    let mut graph = NGraph::new();

    const N: i32 = 4000;
    const E: i32 = 8000;

    // create nodes
    for i in 0..N {
        let node = graph.add_node();
        graph.node_mut(node).id = i;
    }

    // create edges
    for _ in 0..E {
        let src = random.next_int_bound(N);
        let mut tgt = random.next_int_bound(N);
        // no self loops
        while src == tgt {
            tgt = random.next_int_bound(N);
        }

        let delta = random.next_int_bound(50);
        let weight = random.next_double() * 50.0;
        graph.add_edge(graph.nodes[src as usize], graph.nodes[tgt as usize], weight, delta);
    }

    // assert connectedness
    for i in 0..(N - 1) as usize {
        let delta = random.next_int_bound(50);
        let weight = random.next_double() * 50.0;
        graph.add_edge(graph.nodes[i], graph.nodes[i + 1], weight, delta);
    }

    // assert acyclic
    for &node in &graph.nodes.clone() {
        for edge in graph.node(node).outgoing_edges.clone() {
            let (src_id, tgt_id) = {
                let e = graph.edge(edge);
                (graph.node(e.source).id, graph.node(e.target).id)
            };
            if src_id > tgt_id {
                graph.reverse_edge(edge);
            }
        }
    }

    graph
}

/// Java `NetworkSimplexTest.testDeltas`.
#[test]
fn test_deltas() {
    // Random with a fixed seed for determinism.
    let mut random = JavaRandom::new(1);

    for _j in 0..5 {
        let n = 5; // Integer.MAX_VALUE
        for _i in 0..n {
            let mut graph = generate_random_graph(&mut random);

            assert!(graph.is_acyclic());

            NetworkSimplex::for_graph(&mut graph).execute();

            for &node in &graph.nodes {
                for &e in &graph.node(node).outgoing_edges {
                    let edge = graph.edge(e);
                    assert!(
                        graph.node(edge.target).layer - graph.node(edge.source).layer
                            >= edge.delta,
                        "Valid delta"
                    );
                }
            }
        }
    }
}
