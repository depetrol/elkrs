//! Port of `org.eclipse.elk.graph.properties.PropertyTest`
//! (elk/test/org.eclipse.elk.core.test).
//!
//! The Java test checks that `Property.getDefault()` returns the default
//! value itself for immutable types and an independent copy for `Cloneable`
//! types (`IDataObject`s, collections, `EnumSet`s). In the Rust port a
//! `Property`'s default is a `fn() -> T` constructor, so every call produces
//! a fresh, independent value; the tests below assert default equality and
//! copy independence. Java-identity assertions (`defaultValue != copy`) and
//! `testUnknownPropertyGetDefault` (reflective clone failure throwing
//! `IllegalStateException`) have no Rust counterpart.

use elkrs::core::options_gen::{PortSide, SizeConstraint};
use elkrs::graph::math::{KVector, KVectorChain, Spacing};
use elkrs::graph::properties::{EnumSet, Property};

/// Java `PropertyTest.testPropertyDefaultPrimitive`.
/// (Java also tests a `float` default; the Rust port has no f32 properties.)
#[test]
fn test_property_default_primitive() {
    let p: Property<i32> = Property::with_default("dummyInteger", || 43);
    assert_eq!(p.get_default(), Some(43));

    let p: Property<f64> = Property::with_default("dummyDouble", || 32.3);
    assert_eq!(p.get_default(), Some(32.3));

    let p: Property<String> = Property::with_default("dummyString", || "foo".to_string());
    assert_eq!(p.get_default(), Some("foo".to_string()));
}

/// Java `PropertyTest.testPropertyDefaultIDataObject`.
#[test]
fn test_property_default_idataobject() {
    let p: Property<KVector> = Property::with_default("dummyKVector", || KVector::new(2.0, 3.0));
    let copy = p.get_default().unwrap();
    assert_eq!(copy, KVector::new(2.0, 3.0));
    // copies are independent
    let mut mutated = p.get_default().unwrap();
    mutated.x = 99.0;
    assert_eq!(mutated.x, 99.0);
    assert_eq!(p.get_default().unwrap(), KVector::new(2.0, 3.0));

    let p: Property<KVectorChain> = Property::with_default("dummyKVectorChain", || {
        KVectorChain::of(&[KVector::new(2.0, 3.0), KVector::new(2.0, 3.0)])
    });
    let copy = p.get_default().unwrap();
    assert_eq!(copy.len(), 2);
    let mut mutated = p.get_default().unwrap();
    mutated.add(1.0, 1.0);
    assert_eq!(mutated.len(), 3);
    assert_eq!(p.get_default().unwrap().len(), 2);

    // ElkPadding / ElkMargin are both `Spacing` in the Rust port
    let p: Property<Spacing> =
        Property::with_default("dummyElkPadding", || Spacing::of_lr_tb(2.0, 3.0));
    assert_eq!(p.get_default(), Some(Spacing::of_lr_tb(2.0, 3.0)));

    let p: Property<Spacing> =
        Property::with_default("dummyElkMargin", || Spacing::of_lr_tb(3.0, 2.0));
    assert_eq!(p.get_default(), Some(Spacing::of_lr_tb(3.0, 2.0)));
}

/// Java `PropertyTest.testPropertyDefaultObject` (collections; the Rust port
/// models Java `List`s as `Vec`).
#[test]
fn test_property_default_object() {
    fn default_list() -> Vec<KVector> {
        let mut v = KVector::new(3.0, 2.0);
        let first = v;
        let second = *v.normalize();
        let third = *v.negate();
        vec![first, second, third]
    }
    let p: Property<Vec<KVector>> = Property::with_default("dummyArrayList", default_list);
    let copy = p.get_default().unwrap();
    assert_eq!(copy, default_list());
    // copies are independent
    let mut mutated = p.get_default().unwrap();
    mutated.clear();
    assert_eq!(p.get_default().unwrap(), default_list());
}

/// Java `PropertyTest.testPropertyDefaultEnum`.
#[test]
fn test_property_default_enum() {
    let p: Property<PortSide> = Property::with_default("dummyPortSide", || PortSide::EAST);
    assert_eq!(p.get_default(), Some(PortSide::EAST));
}

/// Java `PropertyTest.testPropertyDefaultEnumSet`.
#[test]
fn test_property_default_enum_set() {
    let p: Property<EnumSet<SizeConstraint>> =
        Property::with_default("dummyEnumSet", EnumSet::<SizeConstraint>::all);
    let copy = p.get_default().unwrap();
    assert_eq!(copy, EnumSet::<SizeConstraint>::all());
    let mut mutated = p.get_default().unwrap();
    mutated.remove(SizeConstraint::NODE_LABELS);
    assert_eq!(p.get_default().unwrap(), EnumSet::<SizeConstraint>::all());
}
