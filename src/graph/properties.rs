//! Typed property system mirroring `org.eclipse.elk.graph.properties`.
//!
//! A [`PropertyMap`] stores `String -> Box<dyn PropValue>`
//! and a [`Property<T>`] is a typed handle (id + default) used for access.

use std::any::Any;
use std::fmt;
use std::marker::PhantomData;

use indexmap::IndexMap;

/// Serializes property values to their string representation.
pub trait JavaString {
    fn java_string(&self) -> String;
}

/// Object-safe trait for values stored in a [`PropertyMap`].
pub trait PropValue: Any + fmt::Debug {
    fn clone_box(&self) -> Box<dyn PropValue>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn to_java_string(&self) -> String;
    fn eq_value(&self, other: &dyn PropValue) -> bool;
}

impl<T: Any + Clone + fmt::Debug + JavaString + PartialEq> PropValue for T {
    fn clone_box(&self) -> Box<dyn PropValue> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn to_java_string(&self) -> String {
        JavaString::java_string(self)
    }
    fn eq_value(&self, other: &dyn PropValue) -> bool {
        other.as_any().downcast_ref::<T>().is_some_and(|o| o == self)
    }
}

impl Clone for Box<dyn PropValue> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// ---------------------------------------------------------------- JavaString

impl JavaString for bool {
    fn java_string(&self) -> String {
        self.to_string()
    }
}
impl JavaString for i32 {
    fn java_string(&self) -> String {
        self.to_string()
    }
}
impl JavaString for f64 {
    fn java_string(&self) -> String {
        crate::graph::math::fmt_java_double(*self)
    }
}
impl JavaString for String {
    fn java_string(&self) -> String {
        self.clone()
    }
}
impl JavaString for crate::graph::math::KVector {
    fn java_string(&self) -> String {
        self.to_string()
    }
}
impl JavaString for crate::graph::math::KVectorChain {
    fn java_string(&self) -> String {
        self.to_string()
    }
}
impl JavaString for crate::graph::math::Spacing {
    fn java_string(&self) -> String {
        self.to_string()
    }
}
impl<T: JavaString> JavaString for Vec<T> {
    fn java_string(&self) -> String {
        // List format: "[a, b, c]"
        let items: Vec<String> = self.iter().map(JavaString::java_string).collect();
        format!("[{}]", items.join(", "))
    }
}

/// Whether a property read materializes (clones and stores) the default
/// for this type.
pub trait JavaCloneable {
    const CLONEABLE: bool;
}

impl JavaCloneable for bool {
    const CLONEABLE: bool = false;
}
impl JavaCloneable for i32 {
    const CLONEABLE: bool = false;
}
impl JavaCloneable for f64 {
    const CLONEABLE: bool = false;
}
impl JavaCloneable for String {
    const CLONEABLE: bool = false;
}
impl JavaCloneable for crate::graph::math::KVector {
    const CLONEABLE: bool = true;
}
impl JavaCloneable for crate::graph::math::KVectorChain {
    const CLONEABLE: bool = true;
}
impl JavaCloneable for crate::graph::math::Spacing {
    const CLONEABLE: bool = true;
}
impl<T: JavaCloneable> JavaCloneable for Vec<T> {
    const CLONEABLE: bool = true;
}

// ----------------------------------------------------------------- ElkEnum

/// Implemented by `elk_enum!`-generated enums.
pub trait ElkEnum: Copy + Eq + fmt::Debug + 'static {
    const VALUES: &'static [Self];
    fn name(&self) -> &'static str;
    fn from_name(s: &str) -> Option<Self>;
    fn ordinal(&self) -> usize;
}

/// Defines a Rust enum with `name()`/`valueOf` semantics,
/// `Display` printing the variant name, and `JavaString` for serialization.
#[macro_export]
macro_rules! elk_enum {
    ($(#[$meta:meta])* pub enum $Name:ident { $($Variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
        #[allow(non_camel_case_types)]
        pub enum $Name { $($Variant),+ }

        impl $crate::graph::properties::ElkEnum for $Name {
            const VALUES: &'static [$Name] = &[$($Name::$Variant),+];
            fn name(&self) -> &'static str {
                match self { $($Name::$Variant => stringify!($Variant)),+ }
            }
            fn from_name(s: &str) -> Option<$Name> {
                match s { $(stringify!($Variant) => Some($Name::$Variant),)+ _ => None }
            }
            fn ordinal(&self) -> usize { *self as usize }
        }

        impl std::fmt::Display for $Name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str($crate::graph::properties::ElkEnum::name(self))
            }
        }

        // Fallback only: property lookups use the option's default;
        // this is required to satisfy `PropertyMap::get`'s bound.
        impl Default for $Name {
            fn default() -> Self {
                <$Name as $crate::graph::properties::ElkEnum>::VALUES[0]
            }
        }

        impl $crate::graph::properties::JavaString for $Name {
            fn java_string(&self) -> String {
                $crate::graph::properties::ElkEnum::name(self).to_string()
            }
        }

        impl $crate::graph::properties::JavaCloneable for $Name {
            const CLONEABLE: bool = false;
        }
    };
}

/// A bitset over an [`ElkEnum`].
pub struct EnumSet<T: ElkEnum> {
    bits: u64,
    _pd: PhantomData<T>,
}

impl<T: ElkEnum> EnumSet<T> {
    pub fn none() -> Self {
        EnumSet { bits: 0, _pd: PhantomData }
    }

    pub fn all() -> Self {
        let mut s = Self::none();
        for &v in T::VALUES {
            s.add(v);
        }
        s
    }

    pub fn of(values: &[T]) -> Self {
        let mut s = Self::none();
        for &v in values {
            s.add(v);
        }
        s
    }

    pub fn add(&mut self, v: T) {
        self.bits |= 1 << v.ordinal();
    }

    pub fn remove(&mut self, v: T) {
        self.bits &= !(1 << v.ordinal());
    }

    pub fn contains(&self, v: T) -> bool {
        self.bits & (1 << v.ordinal()) != 0
    }

    pub fn contains_all(&self, other: &EnumSet<T>) -> bool {
        self.bits & other.bits == other.bits
    }

    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    pub fn len(&self) -> usize {
        self.bits.count_ones() as usize
    }

    /// Iterates in ordinal order.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        T::VALUES.iter().copied().filter(|v| self.contains(*v))
    }
}

impl<T: ElkEnum> Clone for EnumSet<T> {
    fn clone(&self) -> Self {
        EnumSet { bits: self.bits, _pd: PhantomData }
    }
}
impl<T: ElkEnum> Copy for EnumSet<T> {}
impl<T: ElkEnum> PartialEq for EnumSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.bits == other.bits
    }
}
impl<T: ElkEnum> Eq for EnumSet<T> {}
impl<T: ElkEnum> Default for EnumSet<T> {
    fn default() -> Self {
        Self::none()
    }
}

impl<T: ElkEnum> fmt::Debug for EnumSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.java_string_impl())
    }
}

impl<T: ElkEnum> EnumSet<T> {
    fn java_string_impl(&self) -> String {
        let names: Vec<&str> = self.iter().map(|v| {
            // SAFETY of lifetime: name() returns &'static str
            T::VALUES[v.ordinal()].name()
        }).collect();
        format!("[{}]", names.join(", "))
    }
}

impl<T: ElkEnum> JavaCloneable for EnumSet<T> {
    const CLONEABLE: bool = true;
}

impl<T: ElkEnum> JavaString for EnumSet<T> {
    fn java_string(&self) -> String {
        self.java_string_impl()
    }
}

impl<T: ElkEnum> FromIterator<T> for EnumSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut s = Self::none();
        for v in iter {
            s.add(v);
        }
        s
    }
}

// ----------------------------------------------------------------- Property

/// Typed property handle, port of `Property<T>`. Equality/identity is by id.
pub struct Property<T: 'static> {
    pub id: &'static str,
    pub default: Option<fn() -> T>,
    pub lower_bound: Option<fn() -> T>,
    pub upper_bound: Option<fn() -> T>,
}

impl<T: 'static> Property<T> {
    pub const fn new(id: &'static str) -> Self {
        Property { id, default: None, lower_bound: None, upper_bound: None }
    }

    pub const fn with_default(id: &'static str, default: fn() -> T) -> Self {
        Property { id, default: Some(default), lower_bound: None, upper_bound: None }
    }

    pub const fn with_bounds(
        id: &'static str,
        default: fn() -> T,
        lower: Option<fn() -> T>,
        upper: Option<fn() -> T>,
    ) -> Self {
        Property { id, default: Some(default), lower_bound: lower, upper_bound: upper }
    }

    pub fn get_default(&self) -> Option<T> {
        self.default.map(|f| f())
    }
}

// ------------------------------------------------------------- PropertyMap

/// Per-element property storage, port of `MapPropertyHolder`.
///
/// Uses an `IndexMap` (insertion order) so that serialization output is
/// deterministic. The map is `RefCell`-backed because reading has
/// write-through semantics: reading an unset property
/// whose default is `Cloneable` stores the cloned default in the map.
#[derive(Default, Debug)]
pub struct PropertyMap {
    map: std::cell::RefCell<IndexMap<String, Box<dyn PropValue>>>,
}

impl Clone for PropertyMap {
    fn clone(&self) -> Self {
        PropertyMap { map: std::cell::RefCell::new(self.map.borrow().clone()) }
    }
}

impl PartialEq for PropertyMap {
    fn eq(&self, other: &Self) -> bool {
        let a = self.map.borrow();
        let b = other.map.borrow();
        a.len() == b.len()
            && a.iter().all(|(k, v)| b.get(k).is_some_and(|o| v.eq_value(o.as_ref())))
    }
}

impl PropertyMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stored value, else the property default (cloned
    /// and stored if the type is "Cloneable"), else `T::default()`.
    pub fn get<T: PropValue + Clone + Default + JavaCloneable>(&self, p: &Property<T>) -> T {
        self.get_opt(p).unwrap_or_default()
    }

    /// For call sites that handle absence: stored value or
    /// property default; `None` when neither is present.
    pub fn get_opt<T: PropValue + Clone + JavaCloneable>(&self, p: &Property<T>) -> Option<T> {
        if let Some(v) = self.try_get(p) {
            return Some(v);
        }
        let default = p.get_default()?;
        if T::CLONEABLE {
            self.map
                .borrow_mut()
                .insert(p.id.to_string(), Box::new(default.clone()));
        }
        Some(default)
    }

    /// The stored value only (no default, no materialization); clone of the
    /// stored value. Used for "value if present, else none" patterns
    /// and for read-modify-write of in-place mutations.
    pub fn try_get<T: PropValue + Clone>(&self, p: &Property<T>) -> Option<T> {
        self.map
            .borrow()
            .get(p.id)
            .and_then(|v| v.as_any().downcast_ref::<T>())
            .cloned()
    }

    pub fn set<T: PropValue>(&self, p: &Property<T>, value: T) -> &Self {
        self.map.borrow_mut().insert(p.id.to_string(), Box::new(value));
        self
    }

    /// Removes the property's stored value.
    pub fn unset<T>(&self, p: &Property<T>) -> &Self {
        self.map.borrow_mut().shift_remove(p.id);
        self
    }

    pub fn has<T>(&self, p: &Property<T>) -> bool {
        self.map.borrow().contains_key(p.id)
    }

    pub fn has_id(&self, id: &str) -> bool {
        self.map.borrow().contains_key(id)
    }

    /// Raw clone of a value by option id (serialization, option resolution).
    pub fn get_by_id(&self, id: &str) -> Option<Box<dyn PropValue>> {
        self.map.borrow().get(id).cloned()
    }

    pub fn set_by_id(&self, id: &str, value: Box<dyn PropValue>) {
        self.map.borrow_mut().insert(id.to_string(), value);
    }

    /// Other's entries overwrite ours.
    pub fn copy_from(&self, other: &PropertyMap) {
        let other_map = other.map.borrow();
        let mut own = self.map.borrow_mut();
        for (k, v) in other_map.iter() {
            own.insert(k.clone(), v.clone());
        }
    }

    /// Snapshot of all entries in insertion order.
    pub fn entries(&self) -> Vec<(String, Box<dyn PropValue>)> {
        self.map
            .borrow()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.map.borrow().is_empty()
    }

    pub fn len(&self) -> usize {
        self.map.borrow().len()
    }
}

pub trait PropertyHolder {
    fn properties(&self) -> &PropertyMap;
    fn properties_mut(&mut self) -> &mut PropertyMap;

    fn get_property<T: PropValue + Clone + Default + JavaCloneable>(&self, p: &Property<T>) -> T {
        self.properties().get(p)
    }
    fn set_property<T: PropValue>(&mut self, p: &Property<T>, value: T) {
        self.properties_mut().set(p, value);
    }
    fn has_property<T>(&self, p: &Property<T>) -> bool {
        self.properties().has(p)
    }
    fn copy_properties_from(&mut self, other: &PropertyMap) {
        self.properties_mut().copy_from(other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::math::KVector;

    static SPACING: Property<f64> = Property::with_default("test.spacing", || 20.0);
    static OFFSET: Property<KVector> = Property::new("test.offset");
    static FLAG: Property<bool> = Property::with_default("test.flag", || true);

    elk_enum! {
        pub enum TestSide { NORTH, EAST, SOUTH, WEST }
    }

    #[test]
    fn defaults_and_overrides() {
        let m = PropertyMap::new();
        assert_eq!(m.get(&SPACING), 20.0);
        assert!(!m.has(&SPACING));
        m.set(&SPACING, 5.0);
        assert_eq!(m.get(&SPACING), 5.0);
        m.unset(&SPACING);
        assert_eq!(m.get(&SPACING), 20.0);
        assert!(m.get(&FLAG));
    }

    #[test]
    fn cloneable_defaults_materialize_on_read() {
        let m = PropertyMap::new();
        let v: KVector = m.get(&OFFSET);
        assert_eq!(v, KVector::default());
        // KVector is "Cloneable", so reading stores... only when a
        // default exists; OFFSET has none, so nothing is stored.
        assert!(!m.has(&OFFSET));

        static MARGIN: Property<crate::graph::math::Spacing> =
            Property::with_default("test.margin", || crate::graph::math::Spacing::uniform(3.0));
        let _ = m.get(&MARGIN);
        assert!(m.has(&MARGIN));

        // non-Cloneable defaults are not materialized
        let _ = m.get(&SPACING);
        assert!(!m.has(&SPACING));
    }

    #[test]
    fn copy_overwrites() {
        let a = PropertyMap::new();
        let b = PropertyMap::new();
        a.set(&SPACING, 1.0);
        b.set(&SPACING, 2.0);
        b.set(&FLAG, false);
        a.copy_from(&b);
        assert_eq!(a.get(&SPACING), 2.0);
        assert!(!a.get(&FLAG));
    }

    #[test]
    fn enum_and_enumset_java_strings() {
        assert_eq!(TestSide::NORTH.java_string(), "NORTH");
        assert_eq!(TestSide::from_name("EAST"), Some(TestSide::EAST));
        let set = EnumSet::of(&[TestSide::WEST, TestSide::NORTH]);
        assert_eq!(set.java_string(), "[NORTH, WEST]");
        assert_eq!(EnumSet::<TestSide>::none().java_string(), "[]");
    }
}
