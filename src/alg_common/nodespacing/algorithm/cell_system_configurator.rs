
use crate::core::adapters::AdapterGraph;
use crate::core::options::{PortConstraints, PortSide, SizeConstraint, SizeOptions};

use crate::alg_common::nodespacing::internal::{NodeContext, NodeLabelLocation};

pub fn configure_cell_system_size_contributions<G: AdapterGraph>(node_context: &mut NodeContext<G>) {
    // If the node has a fixed size, we don't need to change anything because
    // the cell system won't be used to calculate the node's size
    if node_context.size_constraints.is_empty() {
        return;
    }

    // Go through the different size constraint components
    if node_context.size_constraints.contains(SizeConstraint::PORTS) {
        // The northern and southern inside port label cells have the correct
        // width for the node
        let north = node_context.inside_port_label_cell(PortSide::NORTH);
        let south = node_context.inside_port_label_cell(PortSide::SOUTH);
        node_context.cells.set_contributes_to_minimum_width(north, true);
        node_context.cells.set_contributes_to_minimum_width(south, true);

        // For the eastern and western cells, they only give a correct height if
        // port placement is free instead of constrained (the constrained case is
        // handled separately in the node size calculator).
        let free_port_placement = node_context.port_constraints != PortConstraints::FIXED_RATIO
            && node_context.port_constraints != PortConstraints::FIXED_POS;

        let east = node_context.inside_port_label_cell(PortSide::EAST);
        let west = node_context.inside_port_label_cell(PortSide::WEST);
        node_context.cells.set_contributes_to_minimum_height(east, free_port_placement);
        node_context.cells.set_contributes_to_minimum_height(west, free_port_placement);

        // The main row needs to contribute height for the east and west port
        // label cells to be able to contribute their height
        node_context
            .cells
            .set_contributes_to_minimum_height(node_context.node_container_middle_row, free_port_placement);

        // Port labels only contribute their size if ports are accounted for as well
        if node_context.size_constraints.contains(SizeConstraint::PORT_LABELS) {
            // The port label cells contribute the space they need for inside
            // port label placement
            node_context.cells.set_contributes_to_minimum_height(north, true);
            node_context.cells.set_contributes_to_minimum_height(south, true);
            node_context.cells.set_contributes_to_minimum_width(east, true);
            node_context.cells.set_contributes_to_minimum_width(west, true);

            // The main row needs to contribute width for the east and west port
            // label cells to be able to contribute their width
            node_context
                .cells
                .set_contributes_to_minimum_width(node_context.node_container_middle_row, true);
        }
    }

    if node_context.size_constraints.contains(SizeConstraint::NODE_LABELS) {
        // The inside node label cell needs to contribute both width and height,
        // as needs the middle row
        let inside = node_context
            .inside_node_label_container
            .expect("inside node label container must exist");
        node_context.cells.set_contributes_to_minimum_height(inside, true);
        node_context.cells.set_contributes_to_minimum_width(inside, true);

        node_context
            .cells
            .set_contributes_to_minimum_height(node_context.node_container_middle_row, true);
        node_context
            .cells
            .set_contributes_to_minimum_width(node_context.node_container_middle_row, true);

        // All node label cells need to contribute height and width, but outside
        // node labels only do so unless they are configured to overhang
        let overhang = node_context
            .size_options
            .contains(SizeOptions::OUTSIDE_NODE_LABELS_OVERHANG);
        for location in NodeLabelLocation::VALUES {
            if let Some(label_cell) = node_context.node_label_cells[location.ordinal()] {
                if location.is_inside_location() {
                    node_context.cells.set_contributes_to_minimum_height(label_cell, true);
                    node_context.cells.set_contributes_to_minimum_width(label_cell, true);
                } else {
                    node_context.cells.set_contributes_to_minimum_height(label_cell, !overhang);
                    node_context.cells.set_contributes_to_minimum_width(label_cell, !overhang);
                }
            }
        }
    }

    // If the middle cell contributes to the node size, we need to set that up as well
    if node_context.size_constraints.contains(SizeConstraint::MINIMUM_SIZE)
        && node_context
            .size_options
            .contains(SizeOptions::MINIMUM_SIZE_ACCOUNTS_FOR_PADDING)
    {
        // The middle row now needs to contribute width and height, and the
        // center cell of the inside node label container needs to contribute
        // width and height as well.
        // NOTE: quirk -- setContributesToMinimumHeight(true) is called twice
        // in a row here instead of also setting the width contribution.
        node_context
            .cells
            .set_contributes_to_minimum_height(node_context.node_container_middle_row, true);
        node_context
            .cells
            .set_contributes_to_minimum_height(node_context.node_container_middle_row, true);

        // If the inside node label container is not already contributing to the
        // minimum height and width, node labels are not to be regarded. In that
        // case, turn size contributions on, but limit them to the node label
        // container's center cell
        let inside = node_context
            .inside_node_label_container
            .expect("inside node label container must exist");
        if !node_context.cells.is_contributing_to_minimum_height(inside) {
            node_context.cells.set_contributes_to_minimum_height(inside, true);
            node_context.cells.set_contributes_to_minimum_width(inside, true);
            node_context.cells.grid_set_only_center_cell_contributes(inside, true);
        }
    }
}

pub fn update_vertical_inside_port_label_cell_padding<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
) {
    // We only care for the free port placement case
    if node_context.port_constraints == PortConstraints::FIXED_RATIO
        || node_context.port_constraints == PortConstraints::FIXED_POS
    {
        return;
    }

    // Calculate where the east and west port cells will end up
    let north = node_context.inside_port_label_cell(PortSide::NORTH);
    let south = node_context.inside_port_label_cell(PortSide::SOUTH);
    let top_border_offset = node_context.cells.padding(node_context.node_container).top
        + node_context.cells.min_height(north)
        + node_context.label_cell_spacing;
    let bottom_border_offset = node_context.cells.padding(node_context.node_container).bottom
        + node_context.cells.min_height(south)
        + node_context.label_cell_spacing;

    let east_cell = node_context.inside_port_label_cell(PortSide::EAST);
    let west_cell = node_context.inside_port_label_cell(PortSide::WEST);

    // Calculate how much top padding we actually need
    let mut top_padding = 0.0f64.max(node_context.cells.padding(east_cell).top - top_border_offset);
    top_padding = top_padding.max(node_context.cells.padding(west_cell).top - top_border_offset);
    let mut bottom_padding =
        0.0f64.max(node_context.cells.padding(east_cell).bottom - bottom_border_offset);
    bottom_padding =
        bottom_padding.max(node_context.cells.padding(west_cell).bottom - bottom_border_offset);

    // Update paddings
    node_context.cells.padding_mut(east_cell).top = top_padding;
    node_context.cells.padding_mut(west_cell).top = top_padding;
    node_context.cells.padding_mut(east_cell).bottom = bottom_padding;
    node_context.cells.padding_mut(west_cell).bottom = bottom_padding;
}
