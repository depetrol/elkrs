//! Port of `org.eclipse.elk.alg.radial.intermediate.overlaps.RadiusExtensionOverlapRemoval`.

use elk_graph::graph::{ElkGraph, NodeId};

use crate::compaction::RadiusExtension;
use crate::options;
use crate::sorting::RadialSorter;
use crate::util;

/// Port of `RadiusExtensionOverlapRemoval.process` / `removeOverlaps`.
pub fn process(g: &mut ElkGraph, graph: NodeId, root: NodeId) {
    let props = &g.node(graph).properties;
    let mut sorter = props.get(&options::SORTER).create();
    let spacing: f64 = props.get(&options::SPACING_NODE_NODE);
    // Overlap removal keeps the default compaction step of 1 (Java never
    // calls setCompactionStep here).
    let ext = RadiusExtension { compaction_step: 1, spacing, root };

    let successors = util::get_successors(g, root);
    extend(g, &ext, &mut sorter, successors);
}

/// Port of `extend`: extend the radii until the nodes are non-overlapping.
///
/// Java iterates a `HashSet` for the next level (undefined order); here the
/// deterministic insertion order from `get_next_level_node_set` is used.
fn extend(
    g: &mut ElkGraph,
    ext: &RadiusExtension,
    sorter: &mut Option<Box<dyn RadialSorter>>,
    nodes: Vec<NodeId>,
) {
    if nodes.is_empty() {
        return;
    }
    // Save old positions (Java stores all, but only the first is used)
    let first_old = (g.node(nodes[0]).shape.x, g.node(nodes[0]).shape.y);
    while ext.overlap_layer(g, &nodes) {
        ext.contract_layer(g, &nodes, false);
    }

    let moved_x = g.node(nodes[0]).shape.x - first_old.0;
    let moved_y = g.node(nodes[0]).shape.y - first_old.1;
    let moved_distance = (moved_x * moved_x + moved_y * moved_y).sqrt();
    let next_level_nodes = util::get_next_level_node_set(g, &nodes);
    // Move all children and grandchildren by the moved distance.
    for &next_level_node in &next_level_nodes {
        ext.move_node(g, next_level_node, moved_distance);
    }

    if let Some(sorter) = sorter.as_mut() {
        // Java sorts a copy that is then discarded; only the sorter's side
        // effects (e.g. PolarCoordinateSorter assigning order ids) remain.
        let mut copy = next_level_nodes.clone();
        sorter.sort(g, &mut copy);
    }
    extend(g, ext, sorter, next_level_nodes);
}
