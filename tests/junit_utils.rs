//! Port of `org.eclipse.elk.alg.spore.test.UtilsTest`
//! (elk/test/org.eclipse.elk.alg.spore.test). The classes under test —
//! `org.eclipse.elk.alg.common.utils.Utils` and
//! `org.eclipse.elk.alg.common.spore.Node` — live in elk-alg-common, so the
//! test does too. `CompareFuzzy` is `fuzzy_equals`/`fuzzy_compare` with
//! tolerance 0.0001 in the Rust port.

use elkrs::alg_common::elkmath::{fuzzy_compare, fuzzy_equals, shortest_distance};
use elkrs::alg_common::spore::Node;
use elkrs::alg_common::utils;
use elkrs::graph::math::{ElkRectangle, KVector};

/// `CompareFuzzy.TOLERANCE`.
const TOLERANCE: f64 = 0.0001;

/// Java `UtilsTest.testOverlapComputation`: compute position for r2 to not
/// overlap r1, move it accordingly, and check that the distance becomes >= 0.
fn test_overlap_computation(r1: &ElkRectangle, r2: &ElkRectangle) -> bool {
    let o = utils::overlap(r1, r2);
    let c1 = r1.center();
    let c2 = r2.center();
    let mut d = c2;
    d.sub(c1);
    let mut c3 = c1;
    c3.add(*d.scale(o));
    let mut r3 = *r2;
    let mut offset = c3;
    offset.sub(c2);
    r3.move_by(offset);

    fuzzy_compare(shortest_distance(r1, &r3), 0.0, TOLERANCE) >= 0
}

/// Java `UtilsTest.overlapTest`.
#[test]
fn overlap_test() {
    let r1 = ElkRectangle::new(0.0, 0.0, 40.0, 80.0);
    let rs = [
        ElkRectangle::new(0.0, 0.0, 70.0, 20.0),
        ElkRectangle::new(10.0, 50.0, 70.0, 20.0),
        ElkRectangle::new(-40.0, 30.0, 70.0, 20.0),
        ElkRectangle::new(-60.0, 70.0, 70.0, 20.0),
        ElkRectangle::new(-10.0, 70.0, 70.0, 20.0),
        ElkRectangle::new(-20.0, -10.0, 70.0, 20.0),
        ElkRectangle::new(10.0, 20.0, 20.0, 20.0),
        ElkRectangle::new(-20.0, -20.0, 100.0, 120.0),
        ElkRectangle::new(0.0, -0.001, 40.0, 80.0),
    ];
    for r in &rs {
        assert!(test_overlap_computation(&r1, r));
    }
}

/// Java `UtilsTest.init`: the rectangle fixture for the underlap and
/// distance tests.
fn fixture() -> (ElkRectangle, Vec<ElkRectangle>) {
    let r1 = ElkRectangle::new(0.0, 0.0, 20.0, 60.0);
    let rectangles = vec![
        ElkRectangle::new(40.0, 20.0, 20.0, 20.0),
        ElkRectangle::new(40.0, 40.0, 20.0, 20.0),
        ElkRectangle::new(30.0, 70.0, 20.0, 20.0),
        ElkRectangle::new(20.0, 80.0, 20.0, 20.0),
        ElkRectangle::new(10.0, 80.0, 20.0, 20.0),
        ElkRectangle::new(0.0, 80.0, 20.0, 20.0),
        ElkRectangle::new(-30.0, 70.0, 20.0, 20.0),
        ElkRectangle::new(-40.0, 40.0, 20.0, 20.0),
        ElkRectangle::new(-40.0, 20.0, 20.0, 20.0),
        ElkRectangle::new(-30.0, -20.0, 20.0, 20.0),
        ElkRectangle::new(-20.0, -40.0, 20.0, 20.0),
        ElkRectangle::new(0.0, -30.0, 20.0, 20.0),
        ElkRectangle::new(30.0, -30.0, 20.0, 20.0),
        ElkRectangle::new(20.0, 0.0, 20.0, 20.0),
        ElkRectangle::new(0.0, 60.0, 20.0, 20.0),
    ];
    (r1, rectangles)
}

/// Java `UtilsTest.testUnderlapComputation`.
fn test_underlap_computation(r1: &ElkRectangle, r2: &ElkRectangle) -> bool {
    let n1 = Node::new(r1.center(), *r1);
    let mut n2 = Node::new(r2.center(), *r2);

    let underlap = n1.underlap(&n2);
    // Additionally, the underlap should be equal to the distance in the
    // direction given by the nodes' centers.
    let mut dir = n1.vertex;
    dir.sub(n2.vertex);
    assert!(fuzzy_equals(underlap, n1.distance(&n2, dir), TOLERANCE));

    // move + check shortest distance
    let mut translation = n1.vertex;
    translation.sub(n2.vertex).scale_to_length(underlap);
    n2.translate(translation);

    fuzzy_equals(shortest_distance(&n1.rect, &n2.rect), 0.0, TOLERANCE)
}

/// Java `UtilsTest.underlapTest`.
#[test]
fn underlap_test() {
    let (r1, rectangles) = fixture();
    for r in &rectangles {
        assert!(test_underlap_computation(&r1, r));
    }
}

/// Java `UtilsTest.distanceTest`: distance test using different directions.
#[test]
fn distance_test() {
    let (r1, rectangles) = fixture();
    // (direction, true if the nodes should collide)
    let vectors = [
        (KVector::new(-20.0, 20.0), true),
        (KVector::new(-80.0, 0.0), false),
        (KVector::new(-20.0, 9.0), false),
        (KVector::new(0.0, 50.0), false),
        (KVector::new(-9.99, 50.0), false),
        (KVector::new(60.0, 60.0), false),
        (KVector::new(-30.0, 50.0), true),
        (KVector::new(-20.0, 130.0), true),
        (KVector::new(-20.0, -21.0), false),
    ];

    let n1 = Node::new(r1.center(), r1);
    for (v, should_collide) in vectors {
        let mut n2 = Node::new(rectangles[12].center(), rectangles[12]);

        let distance = n1.distance(&n2, v);

        // if they should collide, move them, otherwise check whether the
        // returned distance is infinite
        if should_collide {
            // move + check shortest distance
            let mut translation = v;
            translation.scale_to_length(distance);
            n2.translate(translation);
            assert!(fuzzy_equals(shortest_distance(&n1.rect, &n2.rect), 0.0, TOLERANCE));
        } else {
            assert!(distance.is_infinite());
        }
    }
}
