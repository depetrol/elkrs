
use crate::core::registry::LayoutProvider;
use crate::graph::graph::{ElkGraph, NodeId};

use crate::alg_radial::options::{self, CompactionStrategy};
use crate::alg_radial::{compaction, intermediate, overlaps, p1position, p2routing, rotation, util};

/// The `AlgorithmAssembler` pipeline is
/// inlined: phases P1 (node placement) and P2 (edge routing) with the
/// intermediate processors in `IntermediateProcessorStrategy` ordinal order.
#[derive(Default)]
pub struct RadialLayoutProvider;

impl LayoutProvider for RadialLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        // if requested, compute nodes's dimensions, place node labels, ports,
        // port labels, etc.
        if !g
            .node(layout_node)
            .properties
            .get(&options::OMIT_NODE_MICRO_LAYOUT)
        {
            execute_node_micro_layout(g, layout_node);
        }

        // pre calculate the root node (passed to each processor instead of
        // being stored in InternalProperties.ROOT_NODE)
        let root = util::find_root(g, layout_node)
            .ok_or_else(|| "The given graph is not a tree!".to_string())?;

        // Calculate the radius or take the one given by the user.
        let mut layout_radius: f64 = g.node(layout_node).properties.get(&options::RADIUS);
        if layout_radius == 0.0 {
            layout_radius = util::find_largest_node_in_graph(g, layout_node);
        }
        g.node(layout_node).properties.set(&options::RADIUS, layout_radius);

        // execute the different phases (assembleAlgorithm order)
        p1position::process(g, layout_node, root);
        overlaps::process(g, layout_node, root);
        if g.node(layout_node).properties.get(&options::COMPACTOR) != CompactionStrategy::NONE {
            compaction::process(g, layout_node, root);
        }
        if g.node(layout_node).properties.get(&options::ROTATE) {
            rotation::process(g, layout_node, root);
        }
        intermediate::calculate_graph_size(g, layout_node, root);
        p2routing::process(g, layout_node, root);
        if g
            .node(layout_node)
            .properties
            .get(&options::ROTATION_OUTGOING_EDGE_ANGLES)
        {
            intermediate::edge_angle_calculator(g, layout_node, root);
        }
        Ok(())
    }
}

fn execute_node_micro_layout(g: &mut ElkGraph, layout_node: NodeId) {
    let mut adapter = crate::core::adapters::ElkGraphAdapter::new(g, layout_node);
    crate::alg_common::nodespacing::sort_port_lists(&mut adapter);
    crate::alg_common::nodespacing::calculate_label_and_node_sizes(&mut adapter, |_, _| true);
    crate::alg_common::nodespacing::calculate_node_margins(&mut adapter, false);
}
