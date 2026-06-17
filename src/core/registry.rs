//! Layout algorithm registry, port of the algorithm side of
//! `LayoutMetaDataService` plus `LayoutAlgorithmData`.

use crate::elk_enum;
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::properties::EnumSet;

elk_enum! {
    pub enum GraphFeature {
        SELF_LOOPS,
        INSIDE_SELF_LOOPS,
        MULTI_EDGES,
        EDGE_LABELS,
        PORTS,
        COMPOUND,
        CLUSTERS,
        DISCONNECTED,
    }
}

/// A layout algorithm implementation.
pub trait LayoutProvider {
    /// Lays out the children of `node` within `node`.
    fn layout(&mut self, g: &mut ElkGraph, node: NodeId) -> Result<(), String>;
}

pub struct AlgorithmData {
    pub id: &'static str,
    pub name: &'static str,
    pub features: EnumSet<GraphFeature>,
    pub create: fn() -> Box<dyn LayoutProvider>,
}

/// Algorithm registry (suffix resolution mirrors `getAlgorithmDataBySuffix`).
#[derive(Default)]
pub struct AlgorithmRegistry {
    algorithms: Vec<AlgorithmData>,
}

impl AlgorithmRegistry {
    pub fn register(&mut self, data: AlgorithmData) {
        self.algorithms.push(data);
    }

    pub fn by_id(&self, id: &str) -> Option<&AlgorithmData> {
        self.algorithms.iter().find(|a| a.id == id)
    }

    pub fn by_suffix(&self, suffix: &str) -> Option<&AlgorithmData> {
        if suffix.is_empty() {
            return None;
        }
        if let Some(d) = self.by_id(suffix) {
            return Some(d);
        }
        let matches_suffix = |id: &str| {
            id.ends_with(suffix)
                && (suffix.len() == id.len()
                    || id.as_bytes()[id.len() - suffix.len() - 1] == b'.')
        };
        let mut found: Option<&AlgorithmData> = None;
        for d in &self.algorithms {
            if matches_suffix(d.id) {
                if found.is_some() {
                    return None; // ambiguous
                }
                found = Some(d);
            }
        }
        found
    }
}
