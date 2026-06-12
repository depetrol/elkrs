//! Typed property system mirroring `org.eclipse.elk.graph.properties`.
//!
//! Java stores `IProperty<T> -> Object` in a `HashMap` per element and casts
//! on access. Here a [`PropertyMap`] stores `String -> Box<dyn PropValue>`
//! and a [`Property<T>`] is a typed handle (id + default) used for access.

use std::any::Any;
use std::fmt;
use std::marker::PhantomData;

use indexmap::IndexMap;

/// Java `toString()` equivalent used when serializing property values.
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
        crate::math::fmt_java_double(*self)
    }
}
impl JavaString for String {
    fn java_string(&self) -> String {
        self.clone()
    }
}
impl JavaString for crate::math::KVector {
    fn java_string(&self) -> String {
        self.to_string()
    }
}
impl JavaString for crate::math::KVectorChain {
    fn java_string(&self) -> String {
        self.to_string()
    }
}
impl JavaString for crate::math::Spacing {
    fn java_string(&self) -> String {
        self.to_string()
    }
}
impl<T: JavaString> JavaString for Vec<T> {
    fn java_string(&self) -> String {
        // Java List.toString: "[a, b, c]"
        let items: Vec<String> = self.iter().map(JavaString::java_string).collect();
        format!("[{}]", items.join(", "))
    }
}

// ----------------------------------------------------------------- ElkEnum

/// Implemented by `elk_enum!`-generated enums; mirrors Java `Enum`.
pub trait ElkEnum: Copy + Eq + fmt::Debug + 'static {
    const VALUES: &'static [Self];
    fn name(&self) -> &'static str;
    fn from_name(s: &str) -> Option<Self>;
    fn ordinal(&self) -> usize;
}

/// Defines a Rust enum mirroring a Java enum: `name()`/`valueOf` semantics,
/// `Display` printing the variant name, and `JavaString` for serialization.
#[macro_export]
macro_rules! elk_enum {
    ($(#[$meta:meta])* pub enum $Name:ident { $($Variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
        pub enum $Name { $($Variant),+ }

        impl $crate::properties::ElkEnum for $Name {
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
                f.write_str($crate::properties::ElkEnum::name(self))
            }
        }

        impl $crate::properties::JavaString for $Name {
            fn java_string(&self) -> String {
                $crate::properties::ElkEnum::name(self).to_string()
            }
        }
    };
}

/// Port of Java `EnumSet`, a bitset over an [`ElkEnum`].
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

    /// Iterates in ordinal order, like Java's `EnumSet`.
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
/// deterministic; Java uses `HashMap` and never relies on its order for
/// layout decisions.
#[derive(Default, Clone, Debug)]
pub struct PropertyMap {
    map: IndexMap<String, Box<dyn PropValue>>,
}

impl PartialEq for PropertyMap {
    fn eq(&self, other: &Self) -> bool {
        self.map.len() == other.map.len()
            && self
                .map
                .iter()
                .all(|(k, v)| other.map.get(k).is_some_and(|o| v.eq_value(o.as_ref())))
    }
}

impl PropertyMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Java `getProperty`: stored value, else the property default, else the
    /// type's `Default`. Unlike Java this does not write the default back
    /// into the map; use [`PropertyMap::get_or_insert_default`] when the
    /// caller mutates the returned value in place.
    pub fn get<T: PropValue + Clone + Default>(&self, p: &Property<T>) -> T {
        self.try_get(p)
            .cloned()
            .or_else(|| p.get_default())
            .unwrap_or_default()
    }

    /// Stored value or property default, without falling back to `T::Default`.
    pub fn get_opt<T: PropValue + Clone>(&self, p: &Property<T>) -> Option<T> {
        self.try_get(p).cloned().or_else(|| p.get_default())
    }

    /// Reference to the stored value, if set.
    pub fn try_get<T: PropValue>(&self, p: &Property<T>) -> Option<&T> {
        self.map.get(p.id).and_then(|v| v.as_any().downcast_ref::<T>())
    }

    /// Mutable reference, inserting the default first if unset. This mirrors
    /// Java's behavior where `getProperty` stores cloned mutable defaults.
    pub fn get_or_insert_default<T: PropValue + Clone + Default>(
        &mut self,
        p: &Property<T>,
    ) -> &mut T {
        if !self.map.contains_key(p.id) {
            let v: T = p.get_default().unwrap_or_default();
            self.map.insert(p.id.to_string(), Box::new(v));
        }
        self.map
            .get_mut(p.id)
            .and_then(|v| v.as_any_mut().downcast_mut::<T>())
            .expect("property type mismatch")
    }

    pub fn set<T: PropValue>(&mut self, p: &Property<T>, value: T) -> &mut Self {
        self.map.insert(p.id.to_string(), Box::new(value));
        self
    }

    /// Java `setProperty(p, null)`.
    pub fn unset<T>(&mut self, p: &Property<T>) -> &mut Self {
        self.map.shift_remove(p.id);
        self
    }

    pub fn has<T>(&self, p: &Property<T>) -> bool {
        self.map.contains_key(p.id)
    }

    pub fn has_id(&self, id: &str) -> bool {
        self.map.contains_key(id)
    }

    /// Raw access by option id (used by serialization and option resolution).
    pub fn get_by_id(&self, id: &str) -> Option<&dyn PropValue> {
        self.map.get(id).map(|b| b.as_ref())
    }

    pub fn set_by_id(&mut self, id: &str, value: Box<dyn PropValue>) {
        self.map.insert(id.to_string(), value);
    }

    /// Java `copyProperties`: other's entries overwrite ours.
    pub fn copy_from(&mut self, other: &PropertyMap) {
        for (k, v) in &other.map {
            self.map.insert(k.clone(), v.clone());
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &dyn PropValue)> {
        self.map.iter().map(|(k, v)| (k.as_str(), v.as_ref()))
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

/// Port of `IPropertyHolder`; implemented by all graph elements.
pub trait PropertyHolder {
    fn properties(&self) -> &PropertyMap;
    fn properties_mut(&mut self) -> &mut PropertyMap;

    fn get_property<T: PropValue + Clone + Default>(&self, p: &Property<T>) -> T {
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
    use crate::math::KVector;

    static SPACING: Property<f64> = Property::with_default("test.spacing", || 20.0);
    static OFFSET: Property<KVector> = Property::new("test.offset");
    static FLAG: Property<bool> = Property::with_default("test.flag", || true);

    elk_enum! {
        pub enum TestSide { NORTH, EAST, SOUTH, WEST }
    }

    #[test]
    fn defaults_and_overrides() {
        let mut m = PropertyMap::new();
        assert_eq!(m.get(&SPACING), 20.0);
        assert!(!m.has(&SPACING));
        m.set(&SPACING, 5.0);
        assert_eq!(m.get(&SPACING), 5.0);
        m.unset(&SPACING);
        assert_eq!(m.get(&SPACING), 20.0);
        assert!(m.get(&FLAG));
    }

    #[test]
    fn get_or_insert_default_mutates_in_place() {
        let mut m = PropertyMap::new();
        m.get_or_insert_default(&OFFSET).x = 7.0;
        assert_eq!(m.get(&OFFSET), KVector::new(7.0, 0.0));
    }

    #[test]
    fn copy_overwrites() {
        let mut a = PropertyMap::new();
        let mut b = PropertyMap::new();
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
