//! Rust port of `org.eclipse.elk.alg.spore` — the SPOrE compaction
//! (`org.eclipse.elk.sporeCompaction`) and overlap removal
//! (`org.eclipse.elk.sporeOverlap`) algorithms.

pub mod graph;
pub mod importer;
pub mod options;
pub mod phases;

use crate::alg_common::jhash::JavaHashSet;
use crate::alg_common::triangulation::TEdge;
use crate::core::data::LayoutMetaDataRegistry;
use crate::core::registry::{AlgorithmData, AlgorithmRegistry, LayoutProvider};
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::properties::EnumSet;

use importer::ElkGraphImporter;

/// Registers the spore algorithms and their layout options, mirroring
/// `SporeMetaDataProvider`.
pub fn register(options: &mut LayoutMetaDataRegistry, algorithms: &mut AlgorithmRegistry) {
    options::register_spore_options(options);

    algorithms.register(AlgorithmData {
        id: "org.eclipse.elk.sporeCompaction",
        name: "ELK SPOrE Compaction",
        features: EnumSet::none(),
        create: || Box::new(ShrinkTreeLayoutProvider),
    });
    algorithms.register(AlgorithmData {
        id: "org.eclipse.elk.sporeOverlap",
        name: "ELK SPOrE Overlap Removal",
        features: EnumSet::none(),
        create: || Box::new(OverlapRemovalLayoutProvider),
    });
}

fn check_underlying_layout_algorithm(g: &ElkGraph, layout_node: NodeId) -> Result<(), String> {
    if g.node(layout_node)
        .properties
        .has(&options::UNDERLYING_LAYOUT_ALGORITHM)
    {
        // This port has no global registry to draw providers from.
        return Err(
            "org.eclipse.elk.underlyingLayoutAlgorithm is not supported by this port".to_string(),
        );
    }
    Ok(())
}

#[derive(Default)]
pub struct ShrinkTreeLayoutProvider;

impl LayoutProvider for ShrinkTreeLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        check_underlying_layout_algorithm(g, layout_node)?;

        let (importer, mut graph) = ElkGraphImporter::import_graph(g, layout_node)?;

        // ShrinkTree.shrink: only compact if there's more than one element.
        if graph.vertices.len() > 1 {
            // P1_STRUCTURE: DELAUNAY_TRIANGULATION
            phases::delaunay_triangulation_phase(&mut graph);
            // P2_PROCESSING_ORDER: min/max spanning tree
            phases::spanning_tree_phase(&mut graph, |gr, e| importer.cost(gr, e));
            // P3_EXECUTION: DEPTH_FIRST shrink tree compaction
            phases::shrink_tree_compaction_phase(&mut graph);
        }

        importer.apply_positions(g, &graph);
        Ok(())
    }
}

#[derive(Default)]
pub struct OverlapRemovalLayoutProvider;

impl LayoutProvider for OverlapRemovalLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        check_underlying_layout_algorithm(g, layout_node)?;

        // set algorithm properties
        g.node(layout_node).properties.set(
            &options::PROCESSING_ORDER_ROOT_SELECTION,
            options::RootSelection::CENTER_NODE,
        );
        g.node(layout_node).properties.set(
            &options::PROCESSING_ORDER_SPANNING_TREE_COST_FUNCTION,
            options::SpanningTreeCostFunction::INVERTED_OVERLAP,
        );
        g.node(layout_node).properties.set(
            &options::PROCESSING_ORDER_TREE_CONSTRUCTION,
            options::TreeConstructionStrategy::MINIMUM_SPANNING_TREE,
        );
        let max_iterations: i32 = g
            .node(layout_node)
            .properties
            .get(&options::OVERLAP_REMOVAL_MAX_ITERATIONS);

        // set overlap handler and import ElkGraph
        let mut overlap_edges: JavaHashSet<TEdge> = JavaHashSet::new();
        let (mut importer, mut graph) = ElkGraphImporter::import_graph(g, layout_node)?;

        let mut overlaps_existed = true;
        let mut iteration = 0;

        // repeat overlap removal
        while iteration < max_iterations && overlaps_existed {
            // scanline overlap check
            if g.node(layout_node)
                .properties
                .get(&options::OVERLAP_REMOVAL_RUN_SCANLINE)
            {
                overlap_edges.clear();
                crate::alg_common::spore::scanline_overlap_check(&graph.vertices, |n1, n2| {
                    overlap_edges.add(TEdge::new(
                        graph.vertices[n1].original_vertex,
                        graph.vertices[n2].original_vertex,
                    ));
                });
                if overlap_edges.is_empty() {
                    break; // don't bother if nothing overlaps
                }
                graph.t_edges = Some(std::mem::take(&mut overlap_edges));
            }

            // assembling and executing the algorithm
            phases::delaunay_triangulation_phase(&mut graph);
            phases::spanning_tree_phase(&mut graph, |gr, e| importer.cost(gr, e));
            phases::grow_tree_phase(&mut graph);

            // update node positions (clears tree and tEdges; keeps the
            // overlapEdges set object — and thus its table capacity — alive)
            if let Some(set) = graph.t_edges.take() {
                overlap_edges = set;
            }
            importer.update_graph(&mut graph);

            overlaps_existed = graph.overlaps_existed;
            iteration += 1;
        }

        // apply node positions to ElkGraph
        importer.apply_positions(g, &graph);
        Ok(())
    }
}
