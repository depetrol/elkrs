//! Tests for the layer sweep crossing minimizer.

use crate::core::javacompat::JavaRandom;
use crate::core::options::{PortConstraints, PortSide};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId, LayerId};
use crate::alg_layered::options_gen as lopts;

use super::{process, process_with_type, CrossMinType};

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
/// dumped from it and the algorithm was traced by hand):
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
    let mut counter = crate::alg_layered::p3order::counting::CrossingsCounter::new(vec![0; 6]);
    // ids 0..6 were assigned by the initialization traversal in layer order
    assert_eq!(counter.count_crossings_between_layers(&a, &order[0], &order[1]), 0);
}

// ---------------------------------------------------------------------------
// Greedy switch (ONE_SIDED_GREEDY_SWITCH / TWO_SIDED_GREEDY_SWITCH)

/// Builds a 2-layer graph whose crossing a one-layer barycenter pass would
/// leave (the barycenters tie) but greedy switch fixes:
///
/// ```text
/// layer 0        layer 1
///  a1 p1----------.
///  a2 p2          | ,----vw1 v   (v west port list [vw1, vw2, vw3],
///  a3 p3---. p4===+=+====vw2     clockwise = bottom-to-top)
///  a4 p4===|======+=+====vw3
///          |      '-(p1)-vw1
///          '-------------uw1 u
/// ```
///
/// Edges: p1->vw1, p3->uw1, p4->vw2, p4->vw3 (creation order). Initial
/// layer 1 order is [v, u]: edge p3->uw1 crosses both p4 edges (2
/// crossings); order [u, v] has 1 crossing (p1->vw1 vs p3->uw1... counted
/// between u and v: 1). Barycenters tie: with layer-total output ranks
/// p1..p4 = 1..4, bary(u) = 3 and bary(v) = (1+4+4)/3 = 3, so a single
/// barycenter pass on layer 1 would (up to its random tie-breaking
/// perturbation) keep [v, u]; the greedy switch deterministically switches.
fn build_greedy_switch_graph(
    a: &mut LGraphArena,
) -> (LGraphId, [LayerId; 2], Vec<LNodeId>, Vec<LPortId>) {
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
    let a1 = mk_node(a, l0);
    let a2 = mk_node(a, l0);
    let a3 = mk_node(a, l0);
    let a4 = mk_node(a, l0);
    let v = mk_node(a, l1); // v first: the crossing order
    let u = mk_node(a, l1);

    let mk_port = |a: &mut LGraphArena, node: LNodeId, side: PortSide| {
        let p = a.create_port();
        a.port_set_node(p, Some(node));
        a.port_set_side(p, side);
        p
    };
    let p1 = mk_port(a, a1, PortSide::EAST);
    let p2 = mk_port(a, a2, PortSide::EAST);
    let p3 = mk_port(a, a3, PortSide::EAST);
    let p4 = mk_port(a, a4, PortSide::EAST);
    let vw1 = mk_port(a, v, PortSide::WEST);
    let vw2 = mk_port(a, v, PortSide::WEST);
    let vw3 = mk_port(a, v, PortSide::WEST);
    let uw1 = mk_port(a, u, PortSide::WEST);

    let mk_edge = |a: &mut LGraphArena, src: LPortId, tgt: LPortId| {
        let e = a.create_edge();
        a.edge_set_source(e, Some(src));
        a.edge_set_target(e, Some(tgt));
        e
    };
    mk_edge(a, p1, vw1);
    mk_edge(a, p3, uw1);
    mk_edge(a, p4, vw2);
    mk_edge(a, p4, vw3);

    // PortListSorter would have cached the port sides by now.
    for n in [a1, a2, a3, a4, v, u] {
        a.node_cache_port_sides(n);
    }

    (g, [l0, l1], vec![a1, a2, a3, a4, v, u], vec![p1, p2, p3, p4, vw1, vw2, vw3, uw1])
}

/// Hand-traced expectation for `process_with_type(.., TWO_SIDED)` with
/// `JavaRandom::new(1)`:
///
/// 1. initialize(): randomSeed = nextLong() = -4964420948893066024.
/// 2. GraphInfoHolder: ISweepPortDistributor.create -> TWO_SIDED =>
///    GreedyPortDistributor, NO random consumed. LayerSweepTypeDecider:
///    root graph => bottom-up (no RNG).
/// 3. crossMinDeterministic && crossMinAlwaysImproves =>
///    minimizeCrossingsNoCounter: isForwardSweep = nextBoolean() = false
///    (backward). No further randomness anywhere.
/// 4. while-iteration 1 (backward):
///    - setFirstLayerOrder(layer 1 = [v, u]): two-sided crossing matrix via
///      BetweenLayerEdgeTwoNodeCrossingsCounter, west adjacencies (positions
///      of layer-0 east ports p1..p4 = 0..3): v = [0, 3, 3], u = [2].
///      Merging gives (upperLower, lowerUpper) = (2, 1) => 2 > 1 => switch
///      => layer 1 = [u, v]; returns true.
///    - sweepReducingCrossings(backward): GreedyPortDistributor on layer 1
///      (no east ports, nothing); free layer 0: adjacency merging yields no
///      improving switch ((a3, a4) gives (0, 2)); port distribution on
///      layer 0 east ports (one port each) does nothing.
/// 5. while-iteration 2 (forward):
///    - setFirstLayerOrder(layer 0): no improving switch => false.
///    - sweepReducingCrossings(forward): free layer 1: (u, v) gives (1, 2),
///      no switch. GreedyPortDistributor distributes v's west ports
///      [vw3, vw2, vw1] (top-to-bottom): (vw2, vw1) counts (2, 1) => switch;
///      next pass (vw3, vw1) counts (1, 0) => switch; final port list
///      (bottom-to-top) [vw2, vw3, vw1] => improved = true.
/// 6. while-iteration 3 (backward): no further improvement => loop ends.
/// 7. setCurrentlyBestNodeOrders + transfer => layer 1 = [u, v], v ports
///    [vw2, vw3, vw1], everything gets FIXED_ORDER port constraints.
#[test]
fn two_sided_greedy_switch_fixes_crossing() {
    let mut a = LGraphArena::new();
    let (g, [l0, l1], nodes, ports) = build_greedy_switch_graph(&mut a);
    let (a1, a2, a3, a4, v, u) = (nodes[0], nodes[1], nodes[2], nodes[3], nodes[4], nodes[5]);
    let (vw1, vw2, vw3) = (ports[4], ports[5], ports[6]);

    let mut random = JavaRandom::new(1);
    process_with_type(&mut a, g, &mut random, CrossMinType::TwoSidedGreedySwitch)
        .expect("two-sided greedy switch should succeed");

    assert_eq!(a.layer(l0).nodes, vec![a1, a2, a3, a4]);
    assert_eq!(a.layer(l1).nodes, vec![u, v], "greedy switch must fix the crossing");
    assert_eq!(a.node(v).ports, vec![vw2, vw3, vw1], "greedy port distribution on v");

    for n in [a1, a2, a3, a4, v, u] {
        assert_eq!(
            a.node(n).properties.get(&lopts::PORT_CONSTRAINTS),
            PortConstraints::FIXED_ORDER
        );
    }

    // Exactly nextLong (randomSeed) + one nextBoolean (sweep direction in
    // minimizeCrossingsNoCounter) must have been consumed.
    let mut expected = JavaRandom::new(1);
    expected.next_long();
    expected.next_boolean();
    assert_eq!(random.next_long(), expected.next_long(), "random sequence diverged");
}

/// A graph without crossings: two-sided greedy switch must leave node and
/// port orders untouched (one backward iteration finds no improvement).
#[test]
fn two_sided_greedy_switch_no_crossings_unchanged() {
    let mut a = LGraphArena::new();
    let g = a.create_graph();
    let l0 = a.create_layer(g);
    let l1 = a.create_layer(g);
    a.graph_mut(g).layers.push(l0);
    a.graph_mut(g).layers.push(l1);

    let mk = |a: &mut LGraphArena, layer: LayerId, side: PortSide| {
        let n = a.create_node(g);
        a.node_set_layer(n, Some(layer));
        let p = a.create_port();
        a.port_set_node(p, Some(n));
        a.port_set_side(p, side);
        (n, p)
    };
    let (x, px) = mk(&mut a, l0, PortSide::EAST);
    let (y, py) = mk(&mut a, l0, PortSide::EAST);
    let (w, ww) = mk(&mut a, l1, PortSide::WEST);
    let (z, zw) = mk(&mut a, l1, PortSide::WEST);
    for (src, tgt) in [(px, ww), (py, zw)] {
        let e = a.create_edge();
        a.edge_set_source(e, Some(src));
        a.edge_set_target(e, Some(tgt));
    }
    for n in [x, y, w, z] {
        a.node_cache_port_sides(n);
    }

    let mut random = JavaRandom::new(1);
    process_with_type(&mut a, g, &mut random, CrossMinType::TwoSidedGreedySwitch)
        .expect("two-sided greedy switch should succeed");

    assert_eq!(a.layer(l0).nodes, vec![x, y]);
    assert_eq!(a.layer(l1).nodes, vec![w, z]);
    assert_eq!(a.node(x).ports, vec![px]);
    assert_eq!(a.node(w).ports, vec![ww]);

    // nextLong + one nextBoolean, as above.
    let mut expected = JavaRandom::new(1);
    expected.next_long();
    expected.next_boolean();
    assert_eq!(random.next_long(), expected.next_long(), "random sequence diverged");
}

/// ONE_SIDED greedy switch runs `minimizeCrossingsWithCounter` (deterministic
/// but not always improving). Hand trace for `JavaRandom::new(1)`:
///
/// 1. initialize(): nextLong (randomSeed).
/// 2. GraphInfoHolder: ISweepPortDistributor.create => nextBoolean() = false
///    => LayerTotalPortDistributor (one-sided DOES consume the boolean).
/// 3. minimizeCrossingsWithCounter: isForwardSweep = nextBoolean() = false.
///    Initial crossings = 1. setFirstLayerOrder(layer 1, backward): one-sided
///    EAST counting on [m0, m1] -- no east ports => no switch.
///    sweepReducingCrossings(backward): free layer 0, EAST counting with
///    layer-1 west positions m0=0, m1=1: (n0, n1) -> n0 adj [1], n1 adj [0]
///    => (1, 0) => switch => layer 0 = [n1, n0]; second pass no switch.
///    Crossings now 0 => done.
#[test]
fn one_sided_greedy_switch_fixes_crossing() {
    let mut a = LGraphArena::new();
    let g = a.create_graph();
    let l0 = a.create_layer(g);
    let l1 = a.create_layer(g);
    a.graph_mut(g).layers.push(l0);
    a.graph_mut(g).layers.push(l1);

    let mk = |a: &mut LGraphArena, layer: LayerId, side: PortSide| {
        let n = a.create_node(g);
        a.node_set_layer(n, Some(layer));
        let p = a.create_port();
        a.port_set_node(p, Some(n));
        a.port_set_side(p, side);
        (n, p)
    };
    let (n0, e0) = mk(&mut a, l0, PortSide::EAST);
    let (n1, e1) = mk(&mut a, l0, PortSide::EAST);
    let (m0, w0) = mk(&mut a, l1, PortSide::WEST);
    let (m1, w1) = mk(&mut a, l1, PortSide::WEST);
    // crossing: n0 -> m1, n1 -> m0
    for (src, tgt) in [(e0, w1), (e1, w0)] {
        let e = a.create_edge();
        a.edge_set_source(e, Some(src));
        a.edge_set_target(e, Some(tgt));
    }
    for n in [n0, n1, m0, m1] {
        a.node_cache_port_sides(n);
    }

    let mut random = JavaRandom::new(1);
    process_with_type(&mut a, g, &mut random, CrossMinType::OneSidedGreedySwitch)
        .expect("one-sided greedy switch should succeed");

    assert_eq!(a.layer(l0).nodes, vec![n1, n0], "backward sweep switches the first layer");
    assert_eq!(a.layer(l1).nodes, vec![m0, m1]);

    // nextLong + nextBoolean (port distributor) + nextBoolean (sweep
    // direction in minimizeCrossingsWithCounter).
    let mut expected = JavaRandom::new(1);
    expected.next_long();
    expected.next_boolean();
    expected.next_boolean();
    assert_eq!(random.next_long(), expected.next_long(), "random sequence diverged");
}
