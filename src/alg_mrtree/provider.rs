//!
//! `MrTree` assembles its processor pipeline with `AlgorithmAssembler`. All
//! four phases are fixed (`TreeLayoutPhases` is its own factory), so the
//! assembled algorithm is always the same list; it is materialized below
//! with the slot/ordinal reasoning spelled out.

use crate::core::registry::LayoutProvider;
use crate::graph::graph::{ElkGraph, NodeId};

use crate::alg_mrtree::graph::{TArena, TGraph};
use crate::alg_mrtree::{components, importer, intermediate, options, p1treeify, p2order, p3place, p4route};

#[derive(Default)]
pub struct TreeLayoutProvider;

impl LayoutProvider for TreeLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        // if requested, compute node dimensions, place node labels, ports,
        // port labels, etc.
        if !g
            .node(layout_node)
            .properties
            .get(&options::OMIT_NODE_MICRO_LAYOUT)
        {
            execute_node_micro_layout(g, layout_node);
        }

        // build tGraph
        let (mut arena, tgraph) = importer::import_graph(g, layout_node);

        // split the input graph into components
        let comps = components::split(&mut arena, tgraph);

        // perform the actual layout on the components
        let mut laid_out: Vec<TGraph> = Vec::with_capacity(comps.len());
        for mut comp in comps {
            do_layout(&mut arena, &mut comp);
            laid_out.push(comp);
        }

        // pack the components back into one graph
        let tgraph = components::pack(&mut arena, laid_out);

        // apply the layout results to the original graph
        importer::apply_layout(&arena, &tgraph, g);

        Ok(())
    }
}

/// Runs the assembled algorithm on one component.
///
/// `AlgorithmAssembler.build` produces, per slot, the union of the processors
/// requested by the phases' `LayoutProcessorConfiguration`s, sorted by their
/// `IntermediateProcessorStrategy` ordinal:
///
/// * before P1: —
/// * before P2 (NodeOrderer: ROOT, FAN, LEVEL; NodePlacer: ROOT):
///   `ROOT_PROC(0)`, `FAN_PROC(1)`, `LEVEL_PROC(2)`
/// * before P3 (NodePlacer: LEVEL_HEIGHT, NEIGHBORS):
///   `NEIGHBORS_PROC(3)`, `LEVEL_HEIGHT(4)`
/// * before P4 (NodePlacer: DIRECTION, NODE_POSITION; EdgeRouter:
///   LEVEL_COORDS, COMPACTION, GRAPH_BOUNDS; DFSTreeifyer: DETREEIFYING
///   after P3): `DIRECTION_PROC(5)`, `NODE_POSITION_PROC(6)`,
///   `COMPACTION_PROC(7)`, `LEVEL_COORDS(8)`, `GRAPH_BOUNDS_PROC(9)`,
///   `DETREEIFYING_PROC(10)`
/// * after P4: —
fn do_layout(arena: &mut TArena, graph: &mut TGraph) {
    // P1_TREEIFICATION
    p1treeify::process(arena, graph);
    // slot before P2
    intermediate::root_processor(arena, graph);
    intermediate::fan_processor(arena, graph);
    intermediate::level_processor(arena, graph);
    // P2_NODE_ORDERING
    p2order::process(arena, graph);
    // slot before P3
    intermediate::neighbors_processor(arena, graph);
    intermediate::level_height_processor(arena, graph);
    // P3_NODE_PLACEMENT
    p3place::process(arena, graph);
    // slot before P4
    intermediate::direction_processor(arena, graph);
    intermediate::node_position_processor(arena, graph);
    intermediate::compaction_processor(arena, graph);
    intermediate::level_coordinates_processor(arena, graph);
    intermediate::graph_bounds_processor(arena, graph);
    intermediate::untreeifyer(arena, graph);
    // P4_EDGE_ROUTING
    p4route::process(arena, graph);
}

fn execute_node_micro_layout(g: &mut ElkGraph, layout_node: NodeId) {
    let mut adapter = crate::core::adapters::ElkGraphAdapter::new(g, layout_node);
    crate::alg_common::nodespacing::sort_port_lists(&mut adapter);
    crate::alg_common::nodespacing::calculate_label_and_node_sizes(&mut adapter, |_, _| true);
    crate::alg_common::nodespacing::calculate_node_margins(&mut adapter, false);
}
