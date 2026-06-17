//! Port of `org.eclipse.elk.core.math.KVectorChainTest`
//! (elk/test/org.eclipse.elk.core.test).

use elkrs::graph::math::{KVector, KVectorChain};

/// Java `KVectorChainTest.testParse`.
#[test]
fn test_parse() {
    let v0 = KVector::new(5.0, 50.0);
    let v1 = KVector::new(10.0, 50.0);
    let v2 = KVector::new(30.0, 50.0);
    let kv = KVectorChain::parse("{(5,50),(10,50),(30,50)}").unwrap();

    assert!(v0 == kv.0[0]);
    assert!(v1 == kv.0[1]);
    assert!(v2 == kv.0[2]);

    // ignore a dangling element
    let kv = KVectorChain::parse("{(5,50),(10,50),(30,)}").unwrap();

    assert!(v0 == kv.0[0]);
    assert!(v1 == kv.0[1]);
    assert!(kv.len() == 2);

    // some weird syntax
    let kv = KVectorChain::parse("{(5; 50 ], [10 , 50 ),(30,,,)}").unwrap();

    assert!(v0 == kv.0[0]);
    assert!(v1 == kv.0[1]);
    assert!(kv.len() == 2);
}

/// Java `KVectorChainTest.testParseIllegalArgumentException`
/// (Java expects `IllegalArgumentException`; the Rust port returns `Err`).
#[test]
fn test_parse_illegal_argument_exception() {
    assert!(KVectorChain::parse("{(5,a),(10,50),(30,50)}").is_err());
}

/// Java `KVectorChainTest.testGetLength`.
#[test]
fn test_get_length() {
    // 3 overlapping KVectors
    let kv = KVectorChain::parse("{(10,50),(10,50),(10,50)}").unwrap();
    assert_eq!(0.0, kv.total_length());

    // 3 differing KVectors
    let kv = KVectorChain::parse("{(10,0),(10,20),(10,30)}").unwrap();
    assert_eq!(30.0, kv.total_length());
}

/// Java `KVectorChainTest.testGetPointOnLine`.
#[test]
fn test_get_point_on_line() {
    let v0 = KVector::new(5.0, 50.0);
    let v1 = KVector::new(10.0, 50.0);
    let v2 = KVector::new(30.0, 50.0);
    let kv = KVectorChain::of(&[v0, v1, v2]);

    // test if returns v0 for distance = 0
    assert!(v0 == kv.point_on_line(0.0));

    // test if returns v1 for distance = 5
    assert!(v1 == kv.point_on_line(5.0));

    // test if returns endpoint for distance > KVectorChain's length
    assert!(v2 == kv.point_on_line(40.0));
}
