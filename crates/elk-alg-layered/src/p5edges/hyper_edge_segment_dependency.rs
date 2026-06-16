//!
//! A dependency between two `HyperEdgeSegment`s: the source segment wants to
//! be in a lower routing slot than the target segment.

use super::hyper_edge_segment::{DependencyId, SegmentId, SegmentStore};

/// non-zero weight used for critical dependencies.
pub const CRITICAL_DEPENDENCY_WEIGHT: i32 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DependencyType {
    /// Regular dependencies are ones that, if ignored, may cause additional crossings.
    Regular,
    /// Critical dependencies are ones that, if ignored, result in edge overlaps.
    Critical,
}

pub struct HyperEdgeSegmentDependency {
    /// the dependency's type.
    pub dependency_type: DependencyType,
    /// the source segment of this dependency.
    pub source: Option<SegmentId>,
    /// the target segment of this dependency.
    pub target: Option<SegmentId>,
    /// the weight of this dependency.
    pub weight: i32,
}

/// Creates a dependency and adds it to the incident
/// dependency lists of the given segments.
fn create(
    store: &mut SegmentStore,
    dependency_type: DependencyType,
    source: SegmentId,
    target: SegmentId,
    weight: i32,
) -> DependencyId {
    let dep = store.dependencies.len();
    store.dependencies.push(HyperEdgeSegmentDependency {
        dependency_type,
        source: None,
        target: None,
        weight,
    });
    set_source(store, dep, Some(source));
    set_target(store, dep, Some(target));
    dep
}

pub fn create_and_add_regular(
    store: &mut SegmentStore,
    source: SegmentId,
    target: SegmentId,
    weight: i32,
) -> DependencyId {
    create(store, DependencyType::Regular, source, target, weight)
}

pub fn create_and_add_critical(
    store: &mut SegmentStore,
    source: SegmentId,
    target: SegmentId,
) -> DependencyId {
    create(store, DependencyType::Critical, source, target, CRITICAL_DEPENDENCY_WEIGHT)
}

pub fn remove(store: &mut SegmentStore, dep: DependencyId) {
    set_source(store, dep, None);
    set_target(store, dep, None);
}

pub fn reverse(store: &mut SegmentStore, dep: DependencyId) {
    let old_source = store.dependencies[dep].source;
    let old_target = store.dependencies[dep].target;

    set_source(store, dep, old_target);
    set_target(store, dep, old_source);
}

/// Updates the segments' outgoing dependency lists.
pub fn set_source(store: &mut SegmentStore, dep: DependencyId, new_source: Option<SegmentId>) {
    if let Some(old) = store.dependencies[dep].source {
        store.segments[old].outgoing_segment_dependencies.retain(|&d| d != dep);
    }

    store.dependencies[dep].source = new_source;

    if let Some(new) = new_source {
        store.segments[new].outgoing_segment_dependencies.push(dep);
    }
}

/// Updates the segments' incoming dependency lists.
pub fn set_target(store: &mut SegmentStore, dep: DependencyId, new_target: Option<SegmentId>) {
    if let Some(old) = store.dependencies[dep].target {
        store.segments[old].incoming_segment_dependencies.retain(|&d| d != dep);
    }

    store.dependencies[dep].target = new_target;

    if let Some(new) = new_target {
        store.segments[new].incoming_segment_dependencies.push(dep);
    }
}
