
use crate::core::adapters::AdapterGraph;
use crate::core::options::{self, PortConstraints, PortSide, SizeConstraint, SizeOptions};

use crate::alg_common::nodespacing::algorithm::node_label_and_size_utilities as utilities;
use crate::alg_common::nodespacing::internal::NodeContext;

/// Sets the node's width according
/// to the active node size constraints. Also sets that width on the cell
/// system and tells it to compute a horizontal layout.
pub fn set_node_width<G: AdapterGraph>(g: &G, node_context: &mut NodeContext<G>) {
    let width;

    if utilities::are_size_constraints_fixed(node_context) {
        // Simply use the node's current width
        width = node_context.node_size.x;
    } else {
        // Ask the cell system how wide it would like to be or take the node's
        // width if it has already been set to a greater value
        let mut w = if node_context.topdown_layout {
            node_context
                .node_size
                .x
                .max(node_context.cells.min_width(node_context.node_container))
        } else {
            node_context.cells.min_width(node_context.node_container)
        };

        // If we include node labels and outside node labels are not to
        // overhang, we need to include those as well
        if node_context.size_constraints.contains(SizeConstraint::NODE_LABELS)
            && !node_context
                .size_options
                .contains(SizeOptions::OUTSIDE_NODE_LABELS_OVERHANG)
        {
            w = w.max(
                node_context
                    .cells
                    .min_width(node_context.outside_node_label_container(PortSide::NORTH)),
            );
            w = w.max(
                node_context
                    .cells
                    .min_width(node_context.outside_node_label_container(PortSide::SOUTH)),
            );
        }

        // The node might have a minimum size set...
        if let Some(min_node_size) = utilities::get_minimum_node_size(g, node_context) {
            w = w.max(min_node_size.x);
        }

        width = w;
    }

    // Set the node's width
    if g.graph_properties().get(&options::NODE_SIZE_FIXED_GRAPH_SIZE) {
        node_context.node_size.x = node_context.node_size.x.max(width);
    } else {
        node_context.node_size.x = width;
    }

    // Set the cell system's width and tell it to compute horizontal
    // coordinates and widths. (This uses the local `width`, not the
    // potentially larger node size in the fixed-graph-size case.)
    let node_cell_rectangle = node_context.cells.rect_mut(node_context.node_container);
    node_cell_rectangle.x = 0.0;
    node_cell_rectangle.width = width;

    node_context.cells.layout_children_horizontally(node_context.node_container);
}

pub fn set_node_height<G: AdapterGraph>(g: &G, node_context: &mut NodeContext<G>) {
    let height;

    if utilities::are_size_constraints_fixed(node_context) {
        // Simply use the node's current height
        height = node_context.node_size.y;
    } else {
        // Ask the cell system how high it would like to be or take the node's
        // height if it has already been set to a greater value
        let mut h = if node_context.topdown_layout {
            node_context
                .node_size
                .y
                .max(node_context.cells.min_height(node_context.node_container))
        } else {
            node_context.cells.min_height(node_context.node_container)
        };

        // If we include node labels and outside node labels are not to
        // overhang, we need to include those as well
        if node_context.size_constraints.contains(SizeConstraint::NODE_LABELS)
            && !node_context
                .size_options
                .contains(SizeOptions::OUTSIDE_NODE_LABELS_OVERHANG)
        {
            h = h.max(
                node_context
                    .cells
                    .min_height(node_context.outside_node_label_container(PortSide::EAST)),
            );
            h = h.max(
                node_context
                    .cells
                    .min_height(node_context.outside_node_label_container(PortSide::WEST)),
            );
        }

        // The node might have a minimum size set...
        if let Some(min_node_size) = utilities::get_minimum_node_size(g, node_context) {
            h = h.max(min_node_size.y);
        }

        // If size constraints are set to include ports, but port constraints
        // are FIXED_POS or FIXED_RATIO, we need to manually apply the height
        // required to place eastern and western ports because those heights
        // don't come out of the cell system
        if node_context.size_constraints.contains(SizeConstraint::PORTS)
            && (node_context.port_constraints == PortConstraints::FIXED_RATIO
                || node_context.port_constraints == PortConstraints::FIXED_POS)
        {
            h = h.max(
                node_context
                    .cells
                    .min_height(node_context.inside_port_label_cell(PortSide::EAST)),
            );
            h = h.max(
                node_context
                    .cells
                    .min_height(node_context.inside_port_label_cell(PortSide::WEST)),
            );
        }

        height = h;
    }

    // Set the node's height
    if g.graph_properties().get(&options::NODE_SIZE_FIXED_GRAPH_SIZE) {
        node_context.node_size.y = node_context.node_size.y.max(height);
    } else {
        node_context.node_size.y = height;
    }

    // Set the cell system's height and tell it to compute vertical coordinates
    // and heights
    let node_cell_rectangle = node_context.cells.rect_mut(node_context.node_container);
    node_cell_rectangle.y = 0.0;
    node_cell_rectangle.height = height;

    node_context.cells.layout_children_vertically(node_context.node_container);
}
