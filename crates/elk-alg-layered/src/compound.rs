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
///
/// The full Java processor splits cross-hierarchy edges into per-level segments
/// with external-port dummies. For the current milestone we only support
/// compound graphs in which every edge stays within a single hierarchy level
/// (no edge crosses a node boundary). In that case the preprocessor is a
/// genuine no-op: `crossHierarchyMap` is built empty, no labels move and no
/// edges are removed. We detect any cross-hierarchy edge and bail out loudly
/// rather than silently producing a wrong layout.
pub fn preprocess(a: &mut LGraphArena, lgraph: LGraphId) -> Result<(), String> {
    if has_cross_hierarchy_edge(a, lgraph) {
        return Err(
            "TODO: cross-hierarchy edges (CompoundGraphPreprocessor edge splitting) are not \
             ported yet"
                .to_string(),
        );
    }
    // Attach an (empty) cross-hierarchy map, as the Java processor always does;
    // the postprocessor reads it back.
    a.graph(lgraph)
        .properties
        .set(&crate::internal_properties::CROSS_HIERARCHY_MAP, CrossHierarchyMap::default());
    Ok(())
}

/// Port of `CompoundGraphPostprocessor.process`.
///
/// With no cross-hierarchy edges, the cross-hierarchy map is empty, so the
/// Java postprocessor iterates over nothing and removes no dummy edges: a
/// no-op. (The preprocessor would already have errored out on any
/// cross-hierarchy edge.)
pub fn postprocess(_a: &mut LGraphArena, _lgraph: LGraphId) -> Result<(), String> {
    Ok(())
}

/// Returns `true` if any edge in the hierarchy rooted at `lgraph` connects two
/// nodes that live in different hierarchy levels, i.e. the edge crosses a node
/// boundary. The importer places each edge in the LGraph of the deepest common
/// ancestor, so a cross-hierarchy edge is one whose source-node graph or
/// target-node graph differs from the graph the edge lives in.
fn has_cross_hierarchy_edge(a: &LGraphArena, lgraph: LGraphId) -> bool {
    let mut stack = vec![lgraph];
    while let Some(g) = stack.pop() {
        let nodes = a.graph(g).layerless_nodes.clone();
        for node in nodes {
            for &port in &a.node(node).ports {
                for &edge in &a.port(port).outgoing_edges {
                    let src_graph = a.node_graph(a.edge_source_node(edge));
                    let tgt_graph = a.node_graph(a.edge_target_node(edge));
                    if src_graph != tgt_graph {
                        return true;
                    }
                }
            }
            if let Some(nested) = a.node(node).nested_graph {
                stack.push(nested);
            }
        }
    }
    false
}
