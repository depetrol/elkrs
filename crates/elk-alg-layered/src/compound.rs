//! Port of `org.eclipse.elk.alg.layered.compound`: cross-hierarchy edge
//! splitting for compound (INCLUDE_CHILDREN) layout.
//!
//! Not yet ported. The compound driver scaffolding in [`crate::elk_layered`]
//! is in place, but `CompoundGraphPreprocessor` / `CompoundGraphPostprocessor`
//! (which split cross-hierarchy edges into per-level segments with external
//! port dummies) and the hierarchical-port intermediate processors still need
//! porting before compound layout produces correct results. Until then this
//! fails loudly rather than silently diverging.

use elk_graph::properties::{JavaCloneable, JavaString};

use crate::graph::{LEdgeId, LGraphArena, LGraphId, LPortId};

/// Port of `CrossHierarchyEdge`: one segment of a cross-hierarchy edge in a
/// single graph of the hierarchy.
#[derive(Clone, Debug, PartialEq)]
pub struct CrossHierarchyEdge {
    pub edge: LEdgeId,
    pub graph: LGraphId,
    pub source_port: Option<LPortId>,
    pub target_port: Option<LPortId>,
}

/// Port of `InternalProperties.CROSS_HIERARCHY_MAP`
/// (`Multimap<LEdge, CrossHierarchyEdge>`): for each original edge, its
/// per-level segments in hierarchy order.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct CrossHierarchyMap(pub indexmap::IndexMap<LEdgeId, Vec<CrossHierarchyEdge>>);

impl JavaString for CrossHierarchyMap {
    fn java_string(&self) -> String {
        format!("{:?}", self)
    }
}
impl JavaCloneable for CrossHierarchyMap {
    const CLONEABLE: bool = false;
}

/// Port of `CompoundGraphPreprocessor.process`.
pub fn preprocess(_a: &mut LGraphArena, _lgraph: LGraphId) -> Result<(), String> {
    Err("TODO: CompoundGraphPreprocessor (cross-hierarchy edges) is not ported yet".to_string())
}

/// Port of `CompoundGraphPostprocessor.process`.
pub fn postprocess(_a: &mut LGraphArena, _lgraph: LGraphId) -> Result<(), String> {
    Err("TODO: CompoundGraphPostprocessor is not ported yet".to_string())
}
