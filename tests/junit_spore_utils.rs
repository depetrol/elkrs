//! Rust port of `org.eclipse.elk.alg.common.spore.UtilsTest.overlapTest`.
//! For a battery of rectangle pairs, the `overlap` factor must place a moved
//! copy of `r2` so it just touches `r1` (shortest distance fuzzily >= 0).

use elkrs::alg_common::elkmath::{fuzzy_compare, shortest_distance};
use elkrs::alg_common::spore::Node;
use elkrs::alg_common::utils::overlap;
use elkrs::graph::math::{ElkRectangle, KVector};

const TOLERANCE: f64 = 0.0001; // CompareFuzzy.TOLERANCE

/// CompareFuzzy.eq
fn feq(a: f64, b: f64) -> bool {
    fuzzy_compare(a, b, TOLERANCE) == 0
}

/// The 15 rectangles built by the test's `init()` (`r2`..`r16`).
fn rectangles() -> Vec<ElkRectangle> {
    let r = |x, y, w, h| ElkRectangle::new(x, y, w, h);
    vec![
        r(40., 20., 20., 20.),   // r2
        r(40., 40., 20., 20.),   // r3
        r(30., 70., 20., 20.),   // r4
        r(20., 80., 20., 20.),   // r5
        r(10., 80., 20., 20.),   // r6
        r(0., 80., 20., 20.),    // r7
        r(-30., 70., 20., 20.),  // r8
        r(-40., 40., 20., 20.),  // r9
        r(-40., 20., 20., 20.),  // r10
        r(-30., -20., 20., 20.), // r11
        r(-20., -40., 20., 20.), // r12
        r(0., -30., 20., 20.),   // r13
        r(30., -30., 20., 20.),  // r14
        r(20., 0., 20., 20.),    // r15
        r(0., 60., 20., 20.),    // r16
    ]
}

const R1: ElkRectangle = ElkRectangle::new(0., 0., 20., 60.);

/// Mirror of the Java `testOverlapComputation` helper.
fn test_overlap_computation(r1: &ElkRectangle, r2: &ElkRectangle) -> bool {
    let o = overlap(r1, r2);
    let c1 = r1.center();
    let c2 = r2.center();
    // d = (c2 - c1) * o
    let mut d = c2.clone();
    d.sub(c1.clone());
    d.scale(o);
    // c3 = c1 + d
    let mut c3 = c1.clone();
    c3.add(d);
    // r3 = copy of r2, moved by (c3 - c2)
    let mut r3 = *r2;
    let mut delta = c3;
    delta.sub(c2);
    r3.move_by(delta);
    // CompareFuzzy.ge(shortestDistance(r1, r3), 0.0)
    fuzzy_compare(shortest_distance(r1, &r3), 0.0, TOLERANCE) >= 0
}

#[test]
fn overlap_test() {
    let r = |x, y, w, h| ElkRectangle::new(x, y, w, h);
    let r1 = r(0.0, 0.0, 40.0, 80.0);
    let rs = [
        r(0.0, 0.0, 70.0, 20.0),     // r2
        r(10.0, 50.0, 70.0, 20.0),   // r3
        r(-40.0, 30.0, 70.0, 20.0),  // r4
        r(-60.0, 70.0, 70.0, 20.0),  // r5
        r(-10.0, 70.0, 70.0, 20.0),  // r6
        r(-20.0, -10.0, 70.0, 20.0), // r7
        r(10.0, 20.0, 20.0, 20.0),   // r8
        r(-20.0, -20.0, 100.0, 120.0), // r9
        r(0.0, -0.001, 40.0, 80.0),  // r10
    ];
    for (i, r2) in rs.iter().enumerate() {
        assert!(
            test_overlap_computation(&r1, r2),
            "overlap computation failed for rectangle index {i}"
        );
    }
}

/// Mirror of the Java `testUnderlapComputation` helper.
fn test_underlap_computation(r1: &ElkRectangle, r2: &ElkRectangle) -> bool {
    let n1 = Node::new(r1.center(), *r1);
    let mut n2 = Node::new(r2.center(), *r2);
    let underlap = n1.underlap(&n2);
    // underlap equals the distance along the line between the two centers
    let mut dir = n1.vertex;
    dir.sub(n2.vertex);
    if !feq(underlap, n1.distance(&n2, dir)) {
        return false;
    }
    // move n2 toward n1 by `underlap`; they should then just touch
    let mut step = n1.vertex;
    step.sub(n2.vertex);
    step.scale_to_length(underlap);
    n2.translate(step);
    feq(shortest_distance(&n1.rect, &n2.rect), 0.0)
}

#[test]
fn underlap_test() {
    for (i, r2) in rectangles().iter().enumerate() {
        assert!(
            test_underlap_computation(&R1, r2),
            "underlap computation failed for rectangle index {i}"
        );
    }
}

#[test]
fn distance_test() {
    // (direction, should-collide)
    let vectors: [(KVector, bool); 9] = [
        (KVector::new(-20., 20.), true),
        (KVector::new(-80., 0.), false),
        (KVector::new(-20., 9.), false),
        (KVector::new(0., 50.), false),
        (KVector::new(-9.99, 50.), false),
        (KVector::new(60., 60.), false),
        (KVector::new(-30., 50.), true),
        (KVector::new(-20., 130.), true),
        (KVector::new(-20., -21.), false),
    ];
    let r14 = rectangles()[12]; // rectangles.get(12)
    let n1 = Node::new(R1.center(), R1);
    for (v, collide) in vectors {
        let mut n2 = Node::new(r14.center(), r14);
        let distance = n1.distance(&n2, v);
        if collide {
            let mut step = v;
            step.scale_to_length(distance);
            n2.translate(step);
            assert!(
                feq(shortest_distance(&n1.rect, &n2.rect), 0.0),
                "expected collision (dist 0) for direction {v:?}"
            );
        } else {
            assert!(distance.is_infinite(), "expected infinite distance for direction {v:?}");
        }
    }
}
