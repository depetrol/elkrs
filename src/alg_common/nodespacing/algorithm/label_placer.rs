
use crate::core::adapters::AdapterGraph;
use crate::core::options::{PortSide, SizeOptions};

use crate::alg_common::nodespacing::cellsystem::apply_label_layout;
use crate::alg_common::nodespacing::internal::{NodeContext, NodeLabelLocation};

/// Places outer node label containers as
/// well as all labels. ALL OF THEM!!!
pub fn place_labels<G: AdapterGraph>(g: &mut G, node_context: &mut NodeContext<G>) {
    // Properly place all label cells for outer node labels
    place_outer_node_label_containers(node_context);

    // Tell all node label cells to place their labels (EnumMap order ==
    // NodeLabelLocation ordinal order)
    for location in NodeLabelLocation::VALUES {
        if let Some(label_cell) = node_context.node_label_cells[location.ordinal()] {
            apply_label_layout(&node_context.cells, label_cell, g);
        }
    }

    // Tell all port label cells to place their labels
    for port_context in &node_context.port_contexts {
        if let Some(port_label_cell) = port_context.port_label_cell {
            apply_label_layout(&node_context.cells, port_label_cell, g);
        }
    }
}

fn place_outer_node_label_containers<G: AdapterGraph>(node_context: &mut NodeContext<G>) {
    let outer_node_labels_overhang = node_context
        .size_options
        .contains(SizeOptions::OUTSIDE_NODE_LABELS_OVERHANG);

    place_horizontal_outer_node_label_container(node_context, outer_node_labels_overhang, PortSide::NORTH);
    place_horizontal_outer_node_label_container(node_context, outer_node_labels_overhang, PortSide::SOUTH);
    place_vertical_outer_node_label_container(node_context, outer_node_labels_overhang, PortSide::EAST);
    place_vertical_outer_node_label_container(node_context, outer_node_labels_overhang, PortSide::WEST);
}

fn place_horizontal_outer_node_label_container<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
    outer_node_labels_overhang: bool,
    port_side: PortSide,
) {
    let node_size = node_context.node_size;
    let node_label_container = node_context.outside_node_label_container(port_side);

    // Set the container's width and height to its minimum width and height
    let min_width = node_context.cells.min_width(node_label_container);
    let min_height = node_context.cells.min_height(node_label_container);
    {
        let rect = node_context.cells.rect_mut(node_label_container);
        rect.width = min_width;
        rect.height = min_height;

        // The container must be at least as wide as the node is
        rect.width = rect.width.max(node_size.x);

        // If node labels are not allowed to overhang and if they would do so
        // right now, make the container smaller
        if rect.width > node_size.x && !outer_node_labels_overhang {
            rect.width = node_size.x;
        }

        // Container's x coordinate
        rect.x = -(rect.width - node_size.x) / 2.0;

        // Container's y coordinate depends on whether we place the thing on the
        // northern or southern side
        match port_side {
            PortSide::NORTH => {
                rect.y = -rect.height;
            }
            PortSide::SOUTH => {
                rect.y = node_size.y;
            }
            _ => {}
        }
    }

    // Layout the container's children
    node_context.cells.layout_children_horizontally(node_label_container);
    node_context.cells.layout_children_vertically(node_label_container);
}

fn place_vertical_outer_node_label_container<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
    outer_node_labels_overhang: bool,
    port_side: PortSide,
) {
    let node_size = node_context.node_size;
    let node_label_container = node_context.outside_node_label_container(port_side);

    // Set the container's width and height to its minimum width and height
    let min_width = node_context.cells.min_width(node_label_container);
    let min_height = node_context.cells.min_height(node_label_container);
    {
        let rect = node_context.cells.rect_mut(node_label_container);
        rect.width = min_width;
        rect.height = min_height;

        // The container must be at least as high as the node is
        rect.height = rect.height.max(node_size.y);

        // If node labels are not allowed to overhang and if they would do so
        // right now, make the container smaller
        if rect.height > node_size.y && !outer_node_labels_overhang {
            rect.height = node_size.y;
        }

        // Container's y coordinate
        rect.y = -(rect.height - node_size.y) / 2.0;

        // Container's x coordinate depends on whether we place the thing on the
        // eastern or western side
        match port_side {
            PortSide::WEST => {
                rect.x = -rect.width;
            }
            PortSide::EAST => {
                rect.x = node_size.x;
            }
            _ => {}
        }
    }

    // Layout the container's children
    node_context.cells.layout_children_horizontally(node_label_container);
    node_context.cells.layout_children_vertically(node_label_container);
}
