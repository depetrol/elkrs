//! Layout option metadata service, port of `org.eclipse.elk.core.data`.
//!
//! Holds metadata for every known layout option (id, value kind, targets,
//! legacy ids) and parses string values from their string form.

use std::collections::HashMap;

use crate::graph::math::{KVector, KVectorChain, Spacing};
use crate::graph::properties::{ElkEnum, EnumSet, JavaString, PropValue};

use crate::core::util::IndividualSpacings;

pub type ParseFn = fn(&str) -> Option<Box<dyn PropValue>>;

/// How an option value is parsed from its string form.
#[derive(Clone, Copy)]
pub enum OptionKind {
    Str,
    Bool,
    Int,
    Double,
    Enum(ParseFn),
    EnumSet(ParseFn),
    Object(ParseFn),
    Unparseable,
}

/// Option targets bitset.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Targets(pub u8);

impl Targets {
    pub const PARENTS: Targets = Targets(1);
    pub const NODES: Targets = Targets(2);
    pub const EDGES: Targets = Targets(4);
    pub const PORTS: Targets = Targets(8);
    pub const LABELS: Targets = Targets(16);

    pub const fn empty() -> Targets {
        Targets(0)
    }

    pub fn contains(&self, other: Targets) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for Targets {
    type Output = Targets;
    fn bitor(self, rhs: Targets) -> Targets {
        Targets(self.0 | rhs.0)
    }
}

/// Metadata for one layout option.
pub struct OptionData {
    pub id: &'static str,
    pub group: &'static str,
    pub kind: OptionKind,
    pub targets: Targets,
    pub legacy_ids: &'static [&'static str],
}

impl OptionData {
    pub fn parse_value(&self, value: &str) -> Option<Box<dyn PropValue>> {
        if value == "null" {
            return None;
        }
        if value.is_empty() && !matches!(self.kind, OptionKind::EnumSet(_)) {
            return None;
        }
        match self.kind {
            OptionKind::Str => Some(Box::new(value.to_string())),
            OptionKind::Bool => {
                if value.eq_ignore_ascii_case("true") {
                    Some(Box::new(true))
                } else if value.eq_ignore_ascii_case("false") {
                    Some(Box::new(false))
                } else {
                    None
                }
            }
            OptionKind::Int => value.parse::<i32>().ok().map(|v| Box::new(v) as _),
            OptionKind::Double => parse_java_double(value).map(|v| Box::new(v) as _),
            OptionKind::Enum(f) | OptionKind::EnumSet(f) | OptionKind::Object(f) => f(value),
            OptionKind::Unparseable => None,
        }
    }
}

/// `Double.valueOf` accepts trailing whitespace and `d`/`f` suffixes.
fn parse_java_double(s: &str) -> Option<f64> {
    let t = s.trim();
    let t = t.strip_suffix(['d', 'D', 'f', 'F']).unwrap_or(t);
    t.parse::<f64>().ok()
}

/// Resolves an enum from a string: name first, then ordinal index.
pub fn enum_for_string<T: ElkEnum>(s: &str) -> Option<T> {
    T::from_name(s).or_else(|| {
        s.parse::<usize>().ok().and_then(|i| T::VALUES.get(i).copied())
    })
}

pub fn parse_enum<T: ElkEnum + JavaString + std::fmt::Debug + 'static>(
    s: &str,
) -> Option<Box<dyn PropValue>> {
    enum_for_string::<T>(s).map(|v| Box::new(v) as _)
}

pub fn parse_enumset<T: ElkEnum + JavaString + std::fmt::Debug + 'static>(
    s: &str,
) -> Option<Box<dyn PropValue>> {
    let mut set = EnumSet::<T>::none();
    for component in s.split(|c: char| "[] ,".contains(c)) {
        if component.trim().is_empty() {
            continue;
        }
        match enum_for_string::<T>(component) {
            Some(v) => set.add(v),
            None => return None,
        }
    }
    Some(Box::new(set))
}

pub fn parse_kvector(s: &str) -> Option<Box<dyn PropValue>> {
    KVector::parse(s).ok().map(|v| Box::new(v) as _)
}

pub fn parse_kvectorchain(s: &str) -> Option<Box<dyn PropValue>> {
    KVectorChain::parse(s).ok().map(|v| Box::new(v) as _)
}

pub fn parse_padding(s: &str) -> Option<Box<dyn PropValue>> {
    Spacing::parse(s).ok().map(|v| Box::new(v) as _)
}

pub fn parse_margin(s: &str) -> Option<Box<dyn PropValue>> {
    Spacing::parse(s).ok().map(|v| Box::new(v) as _)
}

pub fn parse_individual_spacings(_s: &str) -> Option<Box<dyn PropValue>> {
    // IndividualSpacings cannot be parsed from a string; the JSON
    // importer handles the "individualSpacings" object specially.
    Some(Box::new(IndividualSpacings::default()))
}

#[derive(Default)]
pub struct LayoutMetaDataRegistry {
    options: Vec<OptionData>,
    by_id: HashMap<&'static str, usize>,
}

impl LayoutMetaDataRegistry {
    pub fn register_option(&mut self, data: OptionData) {
        let idx = self.options.len();
        self.by_id.insert(data.id, idx);
        for &legacy in data.legacy_ids {
            self.by_id.insert(legacy, idx);
        }
        self.options.push(data);
    }

    pub fn option_by_id(&self, id: &str) -> Option<&OptionData> {
        self.by_id.get(id).map(|&i| &self.options[i])
    }

    /// Exact id first, then unique
    /// dot-boundary suffix of an id, then unique suffix of a legacy id.
    pub fn option_by_suffix(&self, suffix: &str) -> Option<&OptionData> {
        if suffix.is_empty() {
            return None;
        }
        if let Some(d) = self.option_by_id(suffix) {
            return Some(d);
        }
        let matches_suffix = |id: &str| {
            id.ends_with(suffix)
                && (suffix.len() == id.len()
                    || id.as_bytes()[id.len() - suffix.len() - 1] == b'.')
        };
        let mut found: Option<&OptionData> = None;
        for d in &self.options {
            if matches_suffix(d.id) {
                if found.is_some() {
                    return None; // ambiguous suffix
                }
                found = Some(d);
            }
        }
        if found.is_none() {
            for d in &self.options {
                for legacy in d.legacy_ids {
                    if matches_suffix(legacy) {
                        if found.is_some() {
                            return None;
                        }
                        found = Some(d);
                    }
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> LayoutMetaDataRegistry {
        let mut reg = LayoutMetaDataRegistry::default();
        crate::core::options_gen::register_core_options(&mut reg);
        reg
    }

    #[test]
    fn suffix_resolution() {
        let reg = registry();
        assert_eq!(
            reg.option_by_suffix("org.eclipse.elk.algorithm").unwrap().id,
            "org.eclipse.elk.algorithm"
        );
        assert_eq!(reg.option_by_suffix("algorithm").unwrap().id, "org.eclipse.elk.algorithm");
        assert_eq!(
            reg.option_by_suffix("spacing.nodeNode").unwrap().id,
            "org.eclipse.elk.spacing.nodeNode"
        );
        // "spacing" alone is not a dot-boundary suffix of any option id
        assert!(reg.option_by_suffix("spacing").is_none());
    }

    #[test]
    fn parse_values() {
        let reg = registry();
        let algo = reg.option_by_suffix("algorithm").unwrap();
        assert_eq!(algo.parse_value("layered").unwrap().to_java_string(), "layered");

        let dir = reg.option_by_suffix("elk.direction").unwrap();
        assert_eq!(dir.parse_value("DOWN").unwrap().to_java_string(), "DOWN");
        assert_eq!(dir.parse_value("2").unwrap().to_java_string(), "LEFT");
        assert!(dir.parse_value("NOPE").is_none());

        let spacing = reg.option_by_suffix("spacing.nodeNode").unwrap();
        assert_eq!(spacing.parse_value("25.5").unwrap().to_java_string(), "25.5");

        let debug = reg.option_by_suffix("debugMode").unwrap();
        assert_eq!(debug.parse_value("TRUE").unwrap().to_java_string(), "true");

        let sc = reg.option_by_suffix("nodeSize.constraints").unwrap();
        assert_eq!(
            sc.parse_value("[NODE_LABELS, MINIMUM_SIZE]").unwrap().to_java_string(),
            "[NODE_LABELS, MINIMUM_SIZE]"
        );
        assert_eq!(sc.parse_value("").unwrap().to_java_string(), "[]");
    }
}
