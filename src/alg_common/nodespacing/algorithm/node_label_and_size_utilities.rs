
use crate::core::adapters::AdapterGraph;
use crate::core::elkutil;
use crate::core::options::{self, PortLabelPlacement, PortSide, SizeConstraint, SizeOptions};
use crate::graph::math::{ElkPadding, KVector};
use crate::graph::properties::EnumSet;

use crate::alg_common::nodespacing::internal::NodeContext;

pub fn setup_minimum_client_area_size<G: AdapterGraph>(g: &G, node_context: &mut NodeContext<G>) {
    if let Some(min_size) = get_minimum_client_area_size(g, node_context) {
        let container = node_context
            .inside_node_label_container
            .expect("inside node label container must exist");
        node_context.cells.grid_set_center_cell_minimum_size(container, min_size);
    }
}

pub fn setup_node_padding_for_ports_with_offset<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
) {
    for index in 0..node_context.port_contexts.len() {
        let port = node_context.port_contexts[index].port;
        let port_side = node_context.port_contexts[index].side;

        // If the port extends into the node, ensure the inside port space is enough
        let mut port_border_offset = 0.0;
        if g.port_properties(port).has(&options::PORT_BORDER_OFFSET) {
            port_border_offset = g
                .port_properties(port)
                .try_get(&options::PORT_BORDER_OFFSET)
                .unwrap_or(0.0);

            if port_border_offset < 0.0 {
                // The port does extend into the node, by -portBorderOffset
                let node_cell_padding = node_context.cells.padding_mut(node_context.node_container);
                match port_side {
                    PortSide::NORTH => {
                        node_cell_padding.top = node_cell_padding.top.max(-port_border_offset);
                    }
                    PortSide::SOUTH => {
                        node_cell_padding.bottom = node_cell_padding.bottom.max(-port_border_offset);
                    }
                    PortSide::EAST => {
                        node_cell_padding.right = node_cell_padding.right.max(-port_border_offset);
                    }
                    PortSide::WEST => {
                        node_cell_padding.left = node_cell_padding.left.max(-port_border_offset);
                    }
                    PortSide::UNDEFINED => {}
                }
            }
        }

        if PortLabelPlacement::is_fixed(node_context.port_labels_placement) {
            // Ensure that the part of the fixed label, that is inside the node,
            // has enough space.
            let inside_part = elkutil::compute_inside_part(g, port, port_border_offset);
            let symmetry = !g
                .node_properties(node_context.node)
                .get(&options::NODE_SIZE_OPTIONS)
                .contains(SizeOptions::ASYMMETRICAL);
            let node_cell_padding = node_context.cells.padding_mut(node_context.node_container);
            match port_side {
                PortSide::NORTH => {
                    let inside_part_is_bigger = inside_part > node_cell_padding.top;
                    node_cell_padding.top = node_cell_padding.top.max(inside_part);
                    if symmetry && inside_part_is_bigger {
                        node_cell_padding.top = node_cell_padding.top.max(node_cell_padding.bottom);
                        // For symmetry, the portBorderOffset is not considered
                        // (only label + label padding)
                        node_cell_padding.bottom = node_cell_padding.top + port_border_offset;
                    }
                }
                PortSide::SOUTH => {
                    let inside_part_is_bigger = inside_part > node_cell_padding.bottom;
                    node_cell_padding.bottom = node_cell_padding.bottom.max(inside_part);
                    if symmetry && inside_part_is_bigger {
                        node_cell_padding.bottom = node_cell_padding.bottom.max(node_cell_padding.top);
                        node_cell_padding.top = node_cell_padding.bottom + port_border_offset;
                    }
                }
                PortSide::EAST => {
                    let inside_part_is_bigger = inside_part > node_cell_padding.right;
                    node_cell_padding.right = node_cell_padding.right.max(inside_part);
                    if symmetry && inside_part_is_bigger {
                        node_cell_padding.right = node_cell_padding.left.max(node_cell_padding.right);
                        node_cell_padding.left = node_cell_padding.right + port_border_offset;
                    }
                }
                PortSide::WEST => {
                    let inside_part_is_bigger = inside_part > node_cell_padding.left;
                    node_cell_padding.left = node_cell_padding.left.max(inside_part);
                    if symmetry && inside_part_is_bigger {
                        node_cell_padding.left = node_cell_padding.left.max(node_cell_padding.right);
                        node_cell_padding.right = node_cell_padding.left + port_border_offset;
                    }
                }
                PortSide::UNDEFINED => {}
            }
        }
    }
}

pub fn offset_southern_ports_by_node_size<G: AdapterGraph>(node_context: &mut NodeContext<G>) {
    let node_height = node_context.node_size.y;

    for index in node_context.ports_on_side(PortSide::SOUTH) {
        node_context.port_contexts[index].port_position.y += node_height;
    }
}

pub fn set_node_padding<G: AdapterGraph>(g: &mut G, node_context: &NodeContext<G>) {
    if !node_context.size_options.contains(SizeOptions::COMPUTE_PADDING) {
        return;
    }

    let node_rect = node_context.cells.rect(node_context.node_container);
    let client_area = node_context.cells.grid_center_cell_rectangle(
        node_context
            .inside_node_label_container
            .expect("inside node label container must exist"),
    );
    let mut node_padding = ElkPadding::default();

    // The following code assumes that the client area rectangle lies fully
    // inside the node rectangle, which should always be the case because of
    // how the client area rectangle is computed
    node_padding.left = client_area.x - node_rect.x;
    node_padding.top = client_area.y - node_rect.y;
    node_padding.right = (node_rect.x + node_rect.width) - (client_area.x + client_area.width);
    node_padding.bottom = (node_rect.y + node_rect.height) - (client_area.y + client_area.height);

    g.set_node_padding(node_context.node, node_padding);
}

pub fn apply_stuff<G: AdapterGraph>(g: &mut G, node_context: &NodeContext<G>) {
    node_context.apply_node_size(g);
    for port_context in &node_context.port_contexts {
        g.set_port_position(port_context.port, port_context.port_position);
    }
}

pub fn get_minimum_client_area_size<G: AdapterGraph>(
    g: &G,
    node_context: &NodeContext<G>,
) -> Option<KVector> {
    if node_context.size_constraints.contains(SizeConstraint::MINIMUM_SIZE)
        && node_context
            .size_options
            .contains(SizeOptions::MINIMUM_SIZE_ACCOUNTS_FOR_PADDING)
    {
        Some(get_minimum_node_or_client_area_size(g, node_context))
    } else {
        None
    }
}

pub fn get_minimum_node_size<G: AdapterGraph>(
    g: &G,
    node_context: &NodeContext<G>,
) -> Option<KVector> {
    if node_context.size_constraints.contains(SizeConstraint::MINIMUM_SIZE)
        && !node_context
            .size_options
            .contains(SizeOptions::MINIMUM_SIZE_ACCOUNTS_FOR_PADDING)
    {
        return Some(get_minimum_node_or_client_area_size(g, node_context));
    }

    None
}

pub fn get_minimum_node_or_client_area_size<G: AdapterGraph>(
    g: &G,
    node_context: &NodeContext<G>,
) -> KVector {
    // Retrieve the minimum size
    let mut min_size = g
        .node_properties(node_context.node)
        .get(&options::NODE_SIZE_MINIMUM);

    // If we are instructed to revert to a default minimum size, we check
    // whether we need to revert to that
    if node_context.size_options.contains(SizeOptions::DEFAULT_MINIMUM_SIZE) {
        if min_size.x <= 0.0 {
            min_size.x = elkutil::DEFAULT_MIN_WIDTH;
        }

        if min_size.y <= 0.0 {
            min_size.y = elkutil::DEFAULT_MIN_HEIGHT;
        }
    }

    min_size
}

/// Size
/// constraints that are empty or only contain `PORT_LABELS` should not cause
/// a node to resize.
pub fn are_size_constraints_fixed<G: AdapterGraph>(node_context: &NodeContext<G>) -> bool {
    let effectively_fixed: EnumSet<SizeConstraint> = EnumSet::of(&[SizeConstraint::PORT_LABELS]);
    node_context.size_constraints.is_empty()
        || node_context.size_constraints == effectively_fixed
}

pub fn is_first_outside_port_label_placed_differently<G: AdapterGraph>(
    node_context: &NodeContext<G>,
    port_side: PortSide,
) -> bool {
    let range = node_context.ports_on_side(port_side);
    let count = range.len();
    if count >= 2 {
        let first_port = &node_context.port_contexts[range.start];

        let always_same_side = node_context
            .port_labels_placement
            .contains(PortLabelPlacement::ALWAYS_SAME_SIDE);
        let space_efficient = node_context
            .port_labels_placement
            .contains(PortLabelPlacement::SPACE_EFFICIENT);

        !first_port.labels_next_to_port && !always_same_side && (count == 2 || space_efficient)
    } else {
        false
    }
}
