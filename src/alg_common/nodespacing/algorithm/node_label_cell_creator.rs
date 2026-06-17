
use crate::core::adapters::AdapterGraph;
use crate::core::options::{self, PortSide, SizeOptions};

use crate::alg_common::nodespacing::cellsystem::{CellId, ContainerArea, Strip};
use crate::alg_common::nodespacing::internal::{NodeContext, NodeLabelLocation};

/// Iterates over all of
/// the node's labels and creates all required cell containers and label cells.
pub fn create_node_label_cells<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    only_inside: bool,
    horizontal_layout_mode: bool,
) {
    // Make sure all the relevant containers exist
    create_node_label_cell_containers(node_context, only_inside);

    // Handle each of the node's labels
    for label in g.node_labels(node_context.node) {
        handle_node_label(g, node_context, label, only_inside, horizontal_layout_mode);
    }
}

fn handle_node_label<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    label: G::L,
    only_inside: bool,
    horizontal_layout_mode: bool,
) {
    // Find the effective label location
    let label_placement = if g.label_properties(label).has(&options::NODE_LABELS_PLACEMENT) {
        g.label_properties(label).get(&options::NODE_LABELS_PLACEMENT)
    } else {
        node_context.node_label_placement
    };
    let label_location = NodeLabelLocation::from_node_label_placement(label_placement);

    // If the label has its location fixed, we will ignore it
    if label_location == NodeLabelLocation::UNDEFINED {
        return;
    }

    // If the label's location is on the node's outside but we only want inside
    // node labels, we will ignore it
    if only_inside && !label_location.is_inside_location() {
        return;
    }

    let cell = retrieve_node_label_cell(node_context, label_location, horizontal_layout_mode);
    node_context.cells.label_add_label(cell, label, g.label_size(label));
}

fn create_node_label_cell_containers<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
    only_inside: bool,
) {
    let symmetry = !node_context.size_options.contains(SizeOptions::ASYMMETRICAL);
    let tabular_node_labels = node_context
        .size_options
        .contains(SizeOptions::FORCE_TABULAR_NODE_LABELS);

    // Inside container
    let inside = node_context.cells.new_grid(
        tabular_node_labels,
        symmetry,
        node_context.label_cell_spacing,
    );
    node_context.inside_node_label_container = Some(inside);

    // (The node labels padding is never absent, since the property has a
    // default value.)
    *node_context.cells.padding_mut(inside) = node_context.node_labels_padding;
    node_context
        .cells
        .strip_set_cell(node_context.node_container_middle_row, ContainerArea::Center, inside);

    // Outside containers, if requested
    if !only_inside {
        let label_cell_spacing = node_context.label_cell_spacing;
        let node_label_spacing = node_context.node_label_spacing;

        let north_container = node_context.cells.new_strip(Strip::Horizontal, symmetry, label_cell_spacing);
        node_context.cells.padding_mut(north_container).bottom = node_label_spacing;
        node_context.outside_node_label_containers[PortSide::NORTH as usize] = Some(north_container);

        let south_container = node_context.cells.new_strip(Strip::Horizontal, symmetry, label_cell_spacing);
        node_context.cells.padding_mut(south_container).top = node_label_spacing;
        node_context.outside_node_label_containers[PortSide::SOUTH as usize] = Some(south_container);

        let west_container = node_context.cells.new_strip(Strip::Vertical, symmetry, label_cell_spacing);
        node_context.cells.padding_mut(west_container).right = node_label_spacing;
        node_context.outside_node_label_containers[PortSide::WEST as usize] = Some(west_container);

        let east_container = node_context.cells.new_strip(Strip::Vertical, symmetry, label_cell_spacing);
        node_context.cells.padding_mut(east_container).left = node_label_spacing;
        node_context.outside_node_label_containers[PortSide::EAST as usize] = Some(east_container);
    }
}

fn retrieve_node_label_cell<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
    node_label_location: NodeLabelLocation,
    horizontal_layout_mode: bool,
) -> CellId {
    let location_index = node_label_location.ordinal();

    if let Some(existing) = node_context.node_label_cells[location_index] {
        return existing;
    }

    // The node label cell doesn't exist yet, so create one and add it to the
    // relevant container
    let node_label_cell = node_context
        .cells
        .new_label(node_context.label_label_spacing, horizontal_layout_mode);
    {
        let data = node_context.cells.label_mut(node_label_cell);
        data.horizontal_alignment = node_label_location.horizontal_alignment();
        data.vertical_alignment = node_label_location.vertical_alignment();
    }
    node_context.node_label_cells[location_index] = Some(node_label_cell);

    // Find the correct container and add the cell to it
    if node_label_location.is_inside_location() {
        let container = node_context
            .inside_node_label_container
            .expect("inside node label container must exist");
        node_context.cells.grid_set_cell(
            container,
            node_label_location.container_row(),
            node_label_location.container_column(),
            node_label_cell,
        );
    } else {
        let outside_side = node_label_location.outside_side();
        let container_cell = node_context.outside_node_label_container(outside_side);

        match outside_side {
            PortSide::NORTH | PortSide::SOUTH => {
                node_context
                    .cells
                    .set_contributes_to_minimum_height(node_label_cell, true);
                node_context.cells.strip_set_cell(
                    container_cell,
                    node_label_location.container_column(),
                    node_label_cell,
                );
            }
            PortSide::WEST | PortSide::EAST => {
                node_context
                    .cells
                    .set_contributes_to_minimum_width(node_label_cell, true);
                node_context.cells.strip_set_cell(
                    container_cell,
                    node_label_location.container_row(),
                    node_label_cell,
                );
            }
            PortSide::UNDEFINED => {}
        }
    }

    node_label_cell
}
