//! Rust port of `org.eclipse.elk.alg.common.spore.UtilsTest.overlapTest`.
//! For a battery of rectangle pairs, the `overlap` factor must place a moved
//! copy of `r2` so it just touches `r1` (shortest distance fuzzily >= 0).

use elk_alg_common::elkmath::{fuzzy_compare, shortest_distance};
use elk_alg_common::utils::overlap;
use elk_graph::math::ElkRectangle;

const TOLERANCE: f64 = 0.0001; // CompareFuzzy.TOLERANCE

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
