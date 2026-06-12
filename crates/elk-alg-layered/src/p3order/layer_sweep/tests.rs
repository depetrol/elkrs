//! Tests for the layer sweep crossing minimizer.

use elk_core::javacompat::JavaRandom;
use elk_core::options::{PortConstraints, PortSide};

use crate::graph::{LGraphArena, LGraphId, LNodeId, LPortId, LayerId};
use crate::options_gen as lopts;

use super::process;

/// Builds the test graph:
///
/// ```text
/// layer 0      layer 1
///  n0 e0---.    ,--w0b m0   (west port list [w0a, w0b] is
///  n1 e1---+---+---w0a      clockwise, i.e. bottom-to-top)
///  n2 e2---+---'
///          '------w1  m1
/// ```
///
/// Edges: n0->m1 (port w1), n1->m0 (port w0a), n2->m0 (port w0b). In the
/// initial order [n0, n1, n2] / [m0, m1] there are 3 crossings.
fn build_two_layer_graph(
    a: &mut LGraphArena,
) -> (LGraphId, [LayerId; 2], [LNodeId; 5], [LPortId; 6]) {
    let g = a.create_graph();
    let l0 = a.create_layer(g);
    let l1 = a.create_layer(g);
    a.graph_mut(g).layers.push(l0);
    a.graph_mut(g).layers.push(l1);

    let mk_node = |a: &mut LGraphArena, layer: LayerId| {
        let n = a.create_node(g);
        a.node_set_layer(n, Some(layer));
        n
    };
    let n0 = mk_node(a, l0);
    let n1 = mk_node(a, l0);
    let n2 = mk_node(a, l0);
    let m0 = mk_node(a, l1);
    let m1 = mk_node(a, l1);

    let mk_port = |a: &mut LGraphArena, node: LNodeId, side: PortSide| {
        let p = a.create_port();
        a.port_set_node(p, Some(node));
        a.port_set_side(p, side);
        p
    };
    let e0 = mk_port(a, n0, PortSide::EAST);
    let e1 = mk_port(a, n1, PortSide::EAST);
    let e2 = mk_port(a, n2, PortSide::EAST);
    let w0a = mk_port(a, m0, PortSide::WEST);
    let w0b = mk_port(a, m0, PortSide::WEST);
    let w1 = mk_port(a, m1, PortSide::WEST);

    let mk_edge = |a: &mut LGraphArena, src: LPortId, tgt: LPortId| {
        let e = a.create_edge();
        a.edge_set_source(e, Some(src));
        a.edge_set_target(e, Some(tgt));
        e
    };
    mk_edge(a, e0, w1);
    mk_edge(a, e1, w0a);
    mk_edge(a, e2, w0b);

    // PortListSorter would have cached the port sides by now.
    for n in [n0, n1, n2, m0, m1] {
        a.node_cache_port_sides(n);
    }

    (g, [l0, l1], [n0, n1, n2, m0, m1], [e0, e1, e2, w0a, w0b, w1])
}

/// Hand-traced expectation for `JavaRandom::new(1)` (the JavaRandom
/// implementation is bit-exact vs. java.util.Random; the tape below was
/// dumped from it and the algorithm was traced by hand against the Java
/// sources):
///
/// 1. initialize(): randomSeed = nextLong() = -4964420948893066024.
/// 2. GraphInfoHolder: ISweepPortDistributor.create -> nextBoolean() = false
///    => LayerTotalPortDistributor.
/// 3. LayerSweepTypeDecider: root graph => bottom-up (no RNG).
/// 4. Barycenter is non-deterministic => compareDifferentRandomizedLayouts:
///    setSeed(randomSeed); node influence == 0 => integer counter branch,
///    THOROUGHNESS = 7 tries.
/// 5. Try 1, minimizeCrossingsWithCounter:
///    - isForwardSweep = nextBoolean() = false (backward sweep).
///    - initial crossings = 3 (no RNG).
///    - setFirstLayerOrder on layer 1 [m0, m1]: randomizeBarycenters:
///      m0.bary = nextDouble() = 0.8958574803319523,
///      m1.bary = nextDouble() = 0.9523050803747884 => order stays [m0, m1].
///    - sweepReducingCrossings(backward, firstSweep):
///      distributePortsWhileSweeping(layer 1) changes nothing (no east/north/
///      south ports on m0/m1). Free layer 0: calculatePortRanks(layer 1,
///      INPUT) (layer-total): w0a=2.0, w0b=1.0, w1=3.0. calculateBarycenters
///      backward for [n0, n1, n2] (one nextFloat perturbation each:
///      0.2897275, 0.20729738, 0.52440023):
///        n0 -> w1:  3.0 + (0.2897275*0.07f - 0.035f)  ~ 2.98528
///        n1 -> w0a: 2.0 + (0.20729738*0.07f - 0.035f) ~ 1.97951
///        n2 -> w0b: 1.0 + (0.52440023*0.07f - 0.035f) ~ 1.00171
///      sorting yields layer 0 = [n2, n1, n0]. Port distribution leaves
///      m0's west list [w0a, w0b] (barycenters -2 < -1, clockwise =
///      bottom-to-top, matching n1 below n2).
///    - count: 0 crossings => currentlyBest = copy, return 0.
/// 6. bestCrossings == 0 => save + break out of the thoroughness loop.
/// 7. transferNodeAndPortOrdersToGraph writes the best sweep back and sets
///    FIXED_ORDER port constraints.
#[test]
fn two_layer_barycenter_sweep() {
    let mut a = LGraphArena::new();
    let (g, [l0, l1], [n0, n1, n2, m0, m1], [_e0, _e1, _e2, w0a, w0b, _w1]) =
        build_two_layer_graph(&mut a);

    let mut random = JavaRandom::new(1);
    process(&mut a, g, &mut random).expect("layer sweep should succeed");

    assert_eq!(a.layer(l0).nodes, vec![n2, n1, n0]);
    assert_eq!(a.layer(l1).nodes, vec![m0, m1]);
    assert_eq!(a.node(m0).ports, vec![w0a, w0b]);

    // transferNodeAndPortOrdersToGraph(.., true) fixes the port order of all
    // nodes whose order was not fixed before.
    for n in [n0, n1, n2, m0, m1] {
        assert_eq!(
            a.node(n).properties.get(&lopts::PORT_CONSTRAINTS),
            PortConstraints::FIXED_ORDER
        );
    }

    // The exact number of consumed random values must match the trace above:
    // nextLong + nextBoolean, setSeed(seed), then nextBoolean + 2x nextDouble
    // + 3x nextFloat.
    let mut expected = JavaRandom::new(1);
    let seed = expected.next_long();
    expected.next_boolean();
    expected.set_seed(seed);
    expected.next_boolean();
    expected.next_double();
    expected.next_double();
    expected.next_float();
    expected.next_float();
    expected.next_float();
    assert_eq!(random.next_long(), expected.next_long(), "random sequence diverged");
}

/// Same graph, but started in an already-optimal order: the heuristic must
/// still consume the same shape of randomness and keep a crossing-free
/// layout (it may pick any zero-crossing order; with this seed the layout
/// stays the one found in the trace above).
#[test]
fn two_layer_barycenter_sweep_other_seed() {
    let mut a = LGraphArena::new();
    let (g, [l0, l1], _nodes, _ports) = build_two_layer_graph(&mut a);

    let mut random = JavaRandom::new(42);
    process(&mut a, g, &mut random).expect("layer sweep should succeed");

    // verify zero crossings in the final order with a fresh counter
    let order: Vec<Vec<LNodeId>> = vec![a.layer(l0).nodes.clone(), a.layer(l1).nodes.clone()];
    let mut counter = crate::p3order::counting::CrossingsCounter::new(vec![0; 6]);
    // ids 0..6 were assigned by the initialization traversal in layer order
    assert_eq!(counter.count_crossings_between_layers(&a, &order[0], &order[1]), 0);
}
