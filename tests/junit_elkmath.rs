//! Port of `org.eclipse.elk.core.math.ElkMathTest`
//! (elk/test/org.eclipse.elk.core.test). `ElkMath` itself lives in
//! `elkrs::alg_common::elkmath` in the Rust port.

use elkrs::alg_common::elkmath::{
    approximate_bezier_segment, approximate_bezier_spline, averaged, averagef, averagel,
    binomiald, binomiall, distance_from_bezier_segment, factd, factl, maxd, maxf, maxi, mind,
    minf, mini, powd, powf, rect_contains_line, rect_contains_path, rect_contains_point,
    rect_intersects_path, segments_intersect,
};
use elkrs::graph::math::{ElkRectangle, KVector, KVectorChain};

/// Java `ElkMathTest.testPointContains`.
#[test]
fn test_point_contains() {
    let rect = ElkRectangle::new(23.0, 14.0, 20.0, 20.0);
    let contained = KVector::new(26.0, 19.0);
    assert!(rect_contains_point(&rect, contained));

    let not_contained = KVector::new(10.0, 9.0);
    assert!(!rect_contains_point(&rect, not_contained));

    let on_border = KVector::new(23.0, 20.0);
    assert!(!rect_contains_point(&rect, on_border));

    let on_corner = KVector::new(23.0, 14.0);
    assert!(!rect_contains_point(&rect, on_corner));
}

/// Java `ElkMathTest.testLineContains`.
#[test]
fn test_line_contains() {
    let rect = ElkRectangle::new(23.0, 14.0, 20.0, 20.0);
    let line11 = KVector::new(24.0, 20.0);
    let line12 = KVector::new(40.0, 32.0);
    assert!(rect_contains_line(&rect, line11, line12));

    let line21 = KVector::new(10.0, 10.0); // outside
    let line22 = KVector::new(40.0, 32.0);
    assert!(!rect_contains_line(&rect, line21, line22));
}

/// Java `ElkMathTest.testPathContains`.
#[test]
fn test_path_contains() {
    let rect = ElkRectangle::new(23.0, 14.0, 20.0, 20.0);

    let mut path = KVectorChain::new();
    path.add(24.0, 15.0);
    path.add(27.0, 20.0);
    path.add(39.0, 30.0);
    path.add(29.0, 19.0);
    assert!(rect_contains_path(&rect, &path));

    // on border
    let mut path2 = path.clone();
    path2.add(23.0, 14.0);
    assert!(!rect_contains_path(&rect, &path2));

    // outside
    path.add(10.0, 10.0);
    assert!(!rect_contains_path(&rect, &path));
}

/// Java `ElkMathTest.testLineLineIntersect`.
#[test]
fn test_line_line_intersect() {
    let l11 = KVector::new(10.0, 10.0);
    let l12 = KVector::new(20.0, 20.0);

    // cross
    let l21 = KVector::new(11.0, 21.0);
    let l22 = KVector::new(21.0, 11.0);
    assert!(segments_intersect(l11, l12, l21, l22));

    // touch
    let l21 = KVector::new(10.0, 10.0);
    let l22 = KVector::new(15.0, 10.0);
    assert!(!segments_intersect(l11, l12, l21, l22));

    // no cross
    let l21 = KVector::new(1.0, 2.0);
    let l22 = KVector::new(2.0, 1.0);
    assert!(!segments_intersect(l11, l12, l21, l22));

    // same line
    assert!(!segments_intersect(l11, l12, l11, l12));
    assert!(!segments_intersect(l11, l12, l12, l11));

    // parallel
    let l21 = KVector::new(11.0, 1.0);
    let l22 = KVector::new(21.0, 21.0);
    assert!(!segments_intersect(l11, l12, l21, l22));
}

/// Java `ElkMathTest.testPathIntersects`.
#[test]
fn test_path_intersects() {
    let rect = ElkRectangle::new(23.0, 14.0, 20.0, 20.0);

    let mut path = KVectorChain::new();
    path.add(24.0, 15.0);
    path.add(27.0, 20.0);
    path.add(39.0, 30.0);
    path.add(29.0, 19.0);
    assert!(!rect_intersects_path(&rect, &path));

    // on border
    let mut path2 = path.clone();
    path2.add(23.0, 14.0);
    assert!(!rect_intersects_path(&rect, &path2));

    // cross
    path.add(10.0, 10.0);
    assert!(rect_intersects_path(&rect, &path));
}

/// Java `ElkMathTest.testFactl`.
#[test]
fn test_factl() {
    assert_eq!(1, factl(0));
    assert_eq!(1, factl(1));
    assert_eq!(2432902008176640000i64, factl(20));
}

/// Java `ElkMathTest.testFactlLittleIllegalArgumentException`.
#[test]
#[should_panic(expected = "The input must be between 0 and")]
fn test_factl_little_illegal_argument_exception() {
    factl(-50);
}

/// Java `ElkMathTest.testFactlBigIllegalArgumentException`.
#[test]
#[should_panic(expected = "The input must be between 0 and")]
fn test_factl_big_illegal_argument_exception() {
    factl(21);
}

/// Java `ElkMathTest.testFactd`.
#[test]
fn test_factd() {
    assert_eq!(1.0, factd(0));
    assert_eq!(1.0, factd(1));
}

/// Java `ElkMathTest.testFacdlLittleIllegalArgumentException`.
#[test]
#[should_panic(expected = "The input must be positive")]
fn test_factd_little_illegal_argument_exception() {
    factd(-1);
}

/// Java `ElkMathTest.testBinomiall`.
#[test]
fn test_binomiall() {
    assert_eq!(1, binomiall(2, 0));
    assert_eq!(1, binomiall(20, 20));
    assert_eq!(2, binomiall(2, 1));
}

/// Java `ElkMathTest.testBinomialllLittleIllegalArgumentException`.
#[test]
#[should_panic(expected = "k and n must be positive")]
fn test_binomiall_little_illegal_argument_exception() {
    binomiall(-1, 1);
}

/// Java `ElkMathTest.testBinomiald`.
#[test]
fn test_binomiald() {
    assert_eq!(1.0, binomiald(2, 0));
    assert_eq!(1.0, binomiald(20, 20));
    assert_eq!(2.0, binomiald(2, 1));
}

/// Java `ElkMathTest.testBinomialdLittleIllegalArgumentException`.
#[test]
#[should_panic(expected = "k and n must be positive")]
fn test_binomiald_little_illegal_argument_exception() {
    binomiald(-1, 1);
}

/// Java `ElkMathTest.testPow`.
#[test]
fn test_pow() {
    let ad: f64 = 10.0;
    let af: f32 = 10.0;
    assert_eq!(1.0, powd(ad, 0));
    assert_eq!(1.0, powf(af, 0));
    assert_eq!(100.0, powd(ad, 2));
    assert_eq!(100.0, powf(af, 2));
}

/// Java `ElkMathTest.testCalcBezierPoints`.
#[test]
fn test_calc_bezier_points() {
    // some KVectors
    let kvector1 = KVector::new(10.0, 10.0);
    let kvector2 = KVector::new(20.0, 20.0);
    let kvector3 = KVector::new(30.0, 30.0);
    let kvector4 = KVector::new(50.0, 50.0);

    // test if the last KVector of the result similar to kvector4
    let result = approximate_bezier_segment(20, &[kvector1, kvector2, kvector3, kvector4]);
    assert!((kvector4.x - result[result.len() - 1].x).abs() <= 0.000000001);
    assert!((kvector4.y - result[result.len() - 1].y).abs() <= 0.000000001);

    // some KVectors with y=10
    let kvector1 = KVector::new(50.0, 10.0);
    let kvector2 = KVector::new(70.0, 10.0);
    let kvector3 = KVector::new(80.0, 10.0);
    let kvector4 = KVector::new(100.0, 10.0);

    // test if all result-KVectors have y=10
    let result = approximate_bezier_segment(20, &[kvector1, kvector2, kvector3, kvector4]);
    for k in &result {
        assert!((10.0 - k.y).abs() <= 0.000000001);
    }
}

/// Java `ElkMathTest.testAppoximateSpline`.
#[test]
fn test_approximate_spline() {
    // some KVectors
    let kvector1 = KVector::new(10.0, 10.0);
    let kvector2 = KVector::new(20.0, 20.0);
    let kvector3 = KVector::new(30.0, 30.0);
    let kvector4 = KVector::new(50.0, 50.0);

    // test if the last KVector of the result similar to kvector4
    let vectors = approximate_bezier_segment(20, &[kvector1, kvector2, kvector3, kvector4]);
    let control_points = KVectorChain::of(&vectors);
    let result = approximate_bezier_spline(&control_points);
    let k = result.0[result.len() - 1];
    assert!((kvector4.x - k.x).abs() <= 0.000000001);
    assert!((kvector4.y - k.y).abs() <= 0.000000001);

    // some KVectors with y=10
    let kvector1 = KVector::new(50.0, 10.0);
    let kvector2 = KVector::new(70.0, 10.0);
    let kvector3 = KVector::new(80.0, 10.0);
    let kvector4 = KVector::new(100.0, 10.0);

    // test if all result-KVectors have y=10
    let vectors = approximate_bezier_segment(20, &[kvector1, kvector2, kvector3, kvector4]);
    let control_points = KVectorChain::of(&vectors);
    let result = approximate_bezier_spline(&control_points);

    for kv in &result {
        assert!((10.0 - kv.y).abs() <= 0.000000001);
    }
}

/// Java `ElkMathTest.testDistanceFromSpline`.
#[test]
fn test_distance_from_spline() {
    // some KVectors
    let kvector1 = KVector::new(10.0, 10.0);
    let kvector2 = KVector::new(20.0, 20.0);
    let kvector3 = KVector::new(30.0, 30.0);
    let kvector4 = KVector::new(50.0, 50.0);

    // test if the result is 0 when kvector4 = needle
    let result = distance_from_bezier_segment(kvector1, kvector2, kvector3, kvector4, kvector4);
    assert!(result.abs() <= 0.01);

    // test if the result is 0 when kvector3 = needle
    let result = distance_from_bezier_segment(kvector1, kvector2, kvector3, kvector4, kvector3);
    assert!(result.abs() <= 0.01);

    // test if the result is 0 when kvector2 = needle
    let result = distance_from_bezier_segment(kvector1, kvector2, kvector3, kvector4, kvector2);
    assert!(result.abs() <= 0.01);

    // test if the result is 0 when kvector1 = needle
    let result = distance_from_bezier_segment(kvector1, kvector2, kvector3, kvector4, kvector1);
    assert!(result.abs() <= 0.01);
}

/// Java `ElkMathTest.testMax`.
#[test]
fn test_max() {
    // test if the max is 7
    assert_eq!(7, maxi(&[1, 7, 5, 6]));
    assert_eq!(7.0, maxf(&[1.0, 7.0, 5.0, 6.0]));
    assert_eq!(7.0, maxd(&[1.0, 7.0, 5.0, 6.0]));
}

/// Java `ElkMathTest.testMin`.
#[test]
fn test_min() {
    // test if the mini is 1
    assert_eq!(1, mini(&[1, 7, 5, 6]));
    // test if the mini is 0
    assert_eq!(0, mini(&[8, 1, 9, 0]));
    // test if the mini is 8
    assert_eq!(8, mini(&[8, 8, 8, 8]));
    // test if the minf is 1
    assert_eq!(1.0, minf(&[1.0, 7.0, 5.0, 6.0]));
    // test if the minf is 0
    assert_eq!(0.0, minf(&[8.0, 1.0, 9.0, 0.0]));
    // test if the minf is 8
    assert_eq!(8.0, minf(&[8.0, 8.0, 8.0, 8.0]));
    // test if the mind is 1
    assert_eq!(1.0, mind(&[1.0, 7.0, 5.0, 6.0]));
    // test if the mind is 0
    assert_eq!(0.0, mind(&[8.0, 1.0, 9.0, 0.0]));
    // test if the mind is 8
    assert_eq!(8.0, mind(&[8.0, 8.0, 8.0, 8.0]));
}

/// Java `ElkMathTest.testAverage`.
#[test]
fn test_average() {
    // test if the averagel is 4
    assert_eq!(4, averagel(&[5, 8, 2, 1]));
    // test if the averagel is 2
    assert_eq!(2, averagel(&[5, 0, 2, 1]));
    // test if the averagef is 4
    assert_eq!(4.0, averagef(&[5.0, 8.0, 2.0, 1.0]));
    // test if the averagef is 2
    assert_eq!(2.0, averagef(&[5.0, 0.0, 2.0, 1.0]));
    // test if the averaged is 4
    assert_eq!(4.0, averaged(&[5.0, 8.0, 2.0, 1.0]));
    // test if the averaged is 2
    assert_eq!(2.0, averaged(&[5.0, 0.0, 2.0, 1.0]));
}
