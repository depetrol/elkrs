
use crate::core::adapters::AdapterGraph;
use crate::core::options::{PortLabelPlacement, PortSide};

use crate::alg_common::nodespacing::cellsystem::{CellId, ContainerArea};
use crate::alg_common::nodespacing::internal::NodeContext;

/// These are
/// set up even when there are no inside port labels since they also determine
/// how much space we need to place ports along the node borders.
pub fn create_inside_port_label_cells<G: AdapterGraph>(node_context: &mut NodeContext<G>) {
    // Create all inside port label cells
    create_inside_port_label_cell(node_context, node_context.node_container, ContainerArea::Begin, PortSide::NORTH);
    create_inside_port_label_cell(node_context, node_context.node_container, ContainerArea::End, PortSide::SOUTH);

    create_inside_port_label_cell(
        node_context,
        node_context.node_container_middle_row,
        ContainerArea::Begin,
        PortSide::WEST,
    );
    create_inside_port_label_cell(
        node_context,
        node_context.node_container_middle_row,
        ContainerArea::End,
        PortSide::EAST,
    );

    setup_north_or_south_port_label_cell(node_context, PortSide::NORTH);
    setup_north_or_south_port_label_cell(node_context, PortSide::SOUTH);
    setup_east_or_west_port_label_cell(node_context, PortSide::EAST);
    setup_east_or_west_port_label_cell(node_context, PortSide::WEST);
}

fn create_inside_port_label_cell<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
    container: CellId,
    container_area: ContainerArea,
    port_side: PortSide,
) {
    let port_label_cell = node_context.cells.new_atomic();
    node_context.cells.strip_set_cell(container, container_area, port_label_cell);
    node_context.inside_port_label_cells[port_side as usize] = Some(port_label_cell);
}

fn setup_north_or_south_port_label_cell<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    let cell = node_context.inside_port_label_cell(port_side);
    let port_label_spacing_vertical = node_context.port_label_spacing_vertical;
    let surrounding = node_context.surrounding_port_margins;
    let padding = node_context.cells.padding_mut(cell);

    match port_side {
        PortSide::NORTH => {
            // In case of negative port spacing, do not use it as a padding since
            // this would increase the node size. Needed to ensure that negative
            // label port spacing behaves the same as positive.
            if port_label_spacing_vertical >= 0.0 {
                padding.top = port_label_spacing_vertical;
            }
        }
        PortSide::SOUTH => {
            if port_label_spacing_vertical >= 0.0 {
                padding.bottom = port_label_spacing_vertical;
            }
        }
        _ => {}
    }

    // (The surrounding port margins are never absent, since the property has
    // a default.)
    padding.left = surrounding.left;
    padding.right = surrounding.right;
}

fn setup_east_or_west_port_label_cell<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    if node_context
        .port_labels_placement
        .contains(PortLabelPlacement::INSIDE)
    {
        calculate_width_due_to_labels(node_context, port_side);
    }
    setup_top_and_bottom_padding(node_context, port_side);
}

fn calculate_width_due_to_labels<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    // Retrieve the appropriate cell
    let the_appropriate_cell = node_context.inside_port_label_cell(port_side);

    let mut min_x = node_context
        .cells
        .atomic_min_content_area_size(the_appropriate_cell)
        .x;
    for index in node_context.ports_on_side(port_side) {
        // Update the maximum label width
        if let Some(port_label_cell) = node_context.port_contexts[index].port_label_cell {
            min_x = min_x.max(node_context.cells.min_width(port_label_cell));
        }
    }
    node_context
        .cells
        .atomic_min_content_area_size_mut(the_appropriate_cell)
        .x = min_x;

    // If the cell has a minimum width by now, that means we actually have
    // labels in there. Which, in turn, means that we need to add a padding to
    // the cell to ensure enough space between ports and their inside labels
    if min_x > 0.0 {
        match port_side {
            PortSide::EAST => {
                node_context.cells.padding_mut(the_appropriate_cell).right =
                    node_context.port_label_spacing_horizontal;
            }
            PortSide::WEST => {
                node_context.cells.padding_mut(the_appropriate_cell).left =
                    node_context.port_label_spacing_horizontal;
            }
            _ => {}
        }
    }
}

fn setup_top_and_bottom_padding<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    // (surroundingPortMargins is never null; see NodeContext.)
    let cell = node_context.inside_port_label_cell(port_side);
    let surrounding = node_context.surrounding_port_margins;
    let padding = node_context.cells.padding_mut(cell);
    padding.top = surrounding.top;
    padding.bottom = surrounding.bottom;
}
