//! Port of `org.eclipse.elk.core.math.KVectorTest`
//! (elk/test/org.eclipse.elk.core.test).

use elkrs::graph::math::KVector;

/// Java `KVectorTest.testEquals`.
#[test]
fn test_equals() {
    // init 2 KVectors
    let kvector1 = KVector::new(10.0, 10.0);
    let mut kvector2 = KVector::default();
    kvector2.x = 10.0;
    kvector2.y = 10.0;

    // test if kvector1 equals to kvector2
    assert!(kvector1 == kvector2);

    // The Java test also asserts `!kvector1.equals(new Object())`; comparing
    // a KVector with an arbitrary object is not expressible in Rust's type
    // system, so that assertion has no Rust counterpart.
}

/// Java `KVectorTest.testAddAndSub`.
#[test]
fn test_add_and_sub() {
    // init 2 KVectors
    let mut kvector1 = KVector::new(12.0, 70.0);
    let kvector2 = KVector::new(15.0, 17.0);

    // adding and subtracting kvector2 to/from kvector1 = kvector1
    // (Java compares the mutated vector against itself; the intended
    // assertion is that the value is unchanged)
    let original = kvector1;
    kvector1.add(kvector2).sub(kvector2);
    assert!(kvector1 == original);
}

/// Java `KVectorTest.testScale`.
#[test]
fn test_scale() {
    let mut a = KVector::new(12.0, 70.0);
    let mut b = KVector::new(12.0, 70.0);

    let a_temp = KVector::new(12.0, 70.0);

    a.add(a_temp).add(a_temp);
    b.scale(3.0);

    assert!(a == b);
}

/// Java `KVectorTest.testTranslate`.
#[test]
fn test_translate() {
    let mut v = KVector::new(10.0, 30.0);
    let b = KVector::new(50.0, 50.0);
    assert!(b == *v.add_xy(40.0, 20.0));
}

/// Java `KVectorTest.testNormalize`.
#[test]
fn test_normalize() {
    let mut v = KVector::new(2.0, 0.0);
    let n = KVector::new(1.0, 0.0);
    assert!(n == *v.normalize());
    let mut v = KVector::new(0.0, 2.0);
    let n = KVector::new(0.0, 1.0);
    assert!(n == *v.normalize());
}

/// Java `KVectorTest.testToDegrees`.
#[test]
fn test_to_degrees() {
    let cases = [
        (KVector::new(10.0, 0.0), 0.0),
        (KVector::new(10.0, 10.0), 45.0),
        (KVector::new(0.0, 10.0), 90.0),
        (KVector::new(-10.0, 10.0), 135.0),
        (KVector::new(-10.0, 0.0), 180.0),
        (KVector::new(-10.0, -10.0), 225.0),
        (KVector::new(0.0, -10.0), 270.0),
        (KVector::new(10.0, -10.0), 315.0),
    ];
    for (v, expected) in cases {
        assert!(
            (v.to_degrees() - expected).abs() <= 0.00001,
            "expected {expected} degrees, got {}",
            v.to_degrees()
        );
    }
}

/// Java `KVectorTest.testDistance`.
#[test]
fn test_distance() {
    let v1 = KVector::new(5.0, 50.0);
    let v2 = KVector::new(5.0, 50.0);
    assert_eq!(0.0, v1.distance(v2));
    let v1 = KVector::new(0.0, 20.0);
    let v2 = KVector::new(0.0, 50.0);
    assert_eq!(30.0, v1.distance(v2));
}

/// Java `KVectorTest.testParse`.
#[test]
fn test_parse() {
    let v1 = KVector::new(5.0, 50.0);
    assert!(v1 == KVector::parse("(5,50)").unwrap());
    assert!(v1 == KVector::parse("{5,50}").unwrap());
    assert!(v1 == KVector::parse("[5,50]").unwrap());
    assert!(v1 == KVector::parse("{(5,50)}").unwrap());
    assert!(v1 == KVector::parse("[(5,50)]").unwrap());
    assert!(v1 == KVector::parse("[{5,50}]").unwrap());
}

/// Java `KVectorTest.testApplyBounds`.
#[test]
fn test_apply_bounds() {
    // test if vt.x > lowx and vt.y > lowy (the result must be the same vt)
    let v = KVector::new(30.0, 30.0);
    let mut vt = v;
    let v_lower_bound = KVector::new(10.0, 10.0);
    let v_upper_bound = KVector::new(40.0, 40.0);
    vt.bound(v_lower_bound.x, v_lower_bound.y, v_upper_bound.x, v_upper_bound.y);
    assert!(vt == v);

    // test if vt.x < lowx and vt.y < lowy (the result must be vt(lowx,lowy))
    let mut vt = v;
    let v_lower_bound = KVector::new(40.0, 40.0);
    let v_upper_bound = KVector::new(60.0, 60.0);
    vt.bound(v_lower_bound.x, v_lower_bound.y, v_upper_bound.x, v_upper_bound.y);
    assert!(vt == v_lower_bound);

    // test if vt.x > highx and vt.y > highy (the result must be vt(highx,highy))
    let mut vt = v;
    let v_lower_bound = KVector::new(20.0, 20.0);
    let v_upper_bound = KVector::new(30.0, 30.0);
    vt.bound(v_lower_bound.x, v_lower_bound.y, v_upper_bound.x, v_upper_bound.y);
    assert!(vt == v_upper_bound);
}
