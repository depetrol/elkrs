
use crate::core::adapters::AdapterGraph;
use crate::core::elkutil;
use crate::core::options::{PortAlignment, PortConstraints, PortLabelPlacement, PortSide, SizeConstraint, SizeOptions};

use crate::alg_common::nodespacing::algorithm::horizontal_port_placement_size_calculator::min_size_required_to_respect_spacing;
use crate::alg_common::nodespacing::algorithm::port_placement_calculator::PORT_RATIO_OR_POSITION;
use crate::alg_common::nodespacing::internal::NodeContext;

pub fn calculate_vertical_port_placement_size<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
) {
    // We need to calculate the space required by the ports even if ports are
    // not part of the size constraints, since we will use that later to place them
    match node_context.port_constraints {
        PortConstraints::FIXED_POS => {
            // We don't have any freedom at all, so simply calculate where the
            // bottommost port is on each side
            calculate_vertical_node_size_required_by_fixed_pos_ports(g, node_context, PortSide::EAST);
            calculate_vertical_node_size_required_by_fixed_pos_ports(g, node_context, PortSide::WEST);
        }
        PortConstraints::FIXED_RATIO => {
            // We can require the node to be large enough to avoid spacing
            // violations with fixed ratio ports
            calculate_vertical_node_size_required_by_fixed_ratio_ports(g, node_context, PortSide::EAST);
            calculate_vertical_node_size_required_by_fixed_ratio_ports(g, node_context, PortSide::WEST);
        }
        _ => {
            // If we are free to place things, make the node large enough to
            // place everything properly
            calculate_vertical_node_size_required_by_free_ports(g, node_context, PortSide::EAST);
            calculate_vertical_node_size_required_by_free_ports(g, node_context, PortSide::WEST);
        }
    }
}

fn calculate_vertical_node_size_required_by_fixed_pos_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    let mut bottommost_port_border = 0.0f64;

    // Check all ports on the correct side
    for index in node_context.ports_on_side(port_side) {
        let pc = &node_context.port_contexts[index];
        bottommost_port_border =
            bottommost_port_border.max(pc.port_position.y + g.port_size(pc.port).y);
    }

    // Set the cell size and remove top padding since the cell size itself
    // already includes all the space we need
    let cell = node_context.inside_port_label_cell(port_side);
    node_context.cells.padding_mut(cell).top = 0.0;
    node_context.cells.atomic_min_content_area_size_mut(cell).y = bottommost_port_border;
}

fn calculate_vertical_node_size_required_by_fixed_ratio_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    let cell = node_context.inside_port_label_cell(port_side);

    // Fetch the port contexts on the given side and abort if there are none
    let range = node_context.ports_on_side(port_side);
    if range.is_empty() {
        node_context.cells.padding_mut(cell).top = 0.0;
        node_context.cells.padding_mut(cell).bottom = 0.0;
        return;
    }

    let port_labels_inside = node_context
        .port_labels_placement
        .contains(PortLabelPlacement::INSIDE);
    let mut min_height = 0.0f64;

    // If port labels are to be respected, we need to calculate the port's margins to do so
    if node_context.size_constraints.contains(SizeConstraint::PORT_LABELS) {
        setup_port_margins(g, node_context, port_side);
    }

    // Go over all pairs of consecutive ports
    let mut previous_port_index: Option<usize> = None;
    let mut previous_port_ratio = 0.0;
    let mut previous_port_height = 0.0;

    for current in range.clone() {
        // Get the next port and find out things about it
        let current_port = node_context.port_contexts[current].port;
        let current_port_ratio = g
            .port_properties(current_port)
            .get(&PORT_RATIO_OR_POSITION);
        let current_port_height = g.port_size(current_port).y;

        match previous_port_index {
            None => {
                // This is the first port, so find out how high the node needs to
                // be to respect the top surrounding port margins, if any
                if node_context.surrounding_port_margins.top > 0.0 {
                    min_height = min_height.max(min_size_required_to_respect_spacing(
                        node_context.surrounding_port_margins.top
                            + node_context.port_contexts[current].port_margin.top,
                        0.0,
                        current_port_ratio,
                    ));
                }
            }
            Some(previous) => {
                let required_space = previous_port_height
                    + node_context.port_contexts[previous].port_margin.bottom
                    + node_context.port_port_spacing
                    + node_context.port_contexts[current].port_margin.top;
                min_height = min_height.max(min_size_required_to_respect_spacing(
                    required_space,
                    previous_port_ratio,
                    current_port_ratio,
                ));
            }
        }

        // Our current port is going to be the previous port during the next iteration
        previous_port_index = Some(current);
        previous_port_ratio = current_port_ratio;
        previous_port_height = current_port_height;
    }

    // If there are bottom surrounding port margins, apply those as well
    if node_context.surrounding_port_margins.bottom > 0.0 {
        // We're using the port's bare height here because we don't care about
        // label sizes on the bottommost port
        let mut required_space = previous_port_height + node_context.surrounding_port_margins.bottom;

        // We're only interested in the port's bottom margin if its label is placed inside
        if port_labels_inside {
            required_space += node_context.port_contexts[previous_port_index.unwrap()]
                .port_margin
                .bottom;
        }

        min_height = min_height.max(min_size_required_to_respect_spacing(
            required_space,
            previous_port_ratio,
            1.0,
        ));
    }

    // Set the cell size and remove top padding since the cell size itself
    // already includes all the space we need
    node_context.cells.padding_mut(cell).top = 0.0;
    node_context.cells.atomic_min_content_area_size_mut(cell).y = min_height;
}

fn calculate_vertical_node_size_required_by_free_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    let cell = node_context.inside_port_label_cell(port_side);

    // Handle the common case first: if there are no ports, set everything to zero
    if node_context.ports_on_side(port_side).is_empty() {
        node_context.cells.padding_mut(cell).top = 0.0;
        node_context.cells.padding_mut(cell).bottom = 0.0;
        return;
    }

    // Set the padding to match the surrounding port space
    node_context.cells.padding_mut(cell).top = node_context.surrounding_port_margins.top;
    node_context.cells.padding_mut(cell).bottom = node_context.surrounding_port_margins.bottom;

    // If we are to take labels into account, we need to setup the port margins
    // such that they include the space required for their labels
    if node_context.size_constraints.contains(SizeConstraint::PORT_LABELS) {
        setup_port_margins(g, node_context, port_side);
    }

    let mut height = port_height_plus_port_port_spacing(g, node_context, port_side);

    // For distributed port alignment, we need to surround the ports by a
    // port-port spacing on each side
    if node_context.get_port_alignment(g, port_side) == PortAlignment::DISTRIBUTED {
        height += 2.0 * node_context.port_port_spacing;
    }

    // Set the cell size
    node_context.cells.atomic_min_content_area_size_mut(cell).y = height;
}

fn setup_port_margins<G: AdapterGraph>(g: &G, node_context: &mut NodeContext<G>, port_side: PortSide) {
    let range = node_context.ports_on_side(port_side);

    let port_labels_outside = node_context
        .port_labels_placement
        .contains(PortLabelPlacement::OUTSIDE);
    let always_same_side = node_context
        .port_labels_placement
        .contains(PortLabelPlacement::ALWAYS_SAME_SIDE);
    let always_same_side_above = node_context
        .port_labels_placement
        .contains(PortLabelPlacement::ALWAYS_OTHER_SAME_SIDE);
    let space_efficient = node_context
        .port_labels_placement
        .contains(PortLabelPlacement::SPACE_EFFICIENT);
    let uniform_port_spacing = node_context
        .size_options
        .contains(SizeOptions::UNIFORM_PORT_SPACING);

    let space_efficient_port_labels = !always_same_side
        && !always_same_side_above
        && (space_efficient || range.len() == 2);

    // Set the vertical port margins of all ports according to how their labels
    // will be placed. We'll be modifying the margins soon enough.
    compute_vertical_port_margins(g, node_context, port_side, port_labels_outside);

    // The topmost and bottommost ports are possibly required
    let topmost = range.start;
    let bottommost = range.end - 1;

    // If port labels are placed outside, there's stuff we can do
    if port_labels_outside {
        // The topmost and bottommost ports don't need their top and bottom
        // margin, respectively
        node_context.port_contexts[topmost].port_margin.top = 0.0;
        node_context.port_contexts[bottommost].port_margin.bottom = 0.0;

        // If we place port labels space-efficiently and the topmost port
        // doesn't have its label placed right next to it, it doesn't need its
        // bottom margin either since its label will be placed above
        if space_efficient_port_labels && !node_context.port_contexts[topmost].labels_next_to_port {
            node_context.port_contexts[topmost].port_margin.bottom = 0.0;
        }
    }

    // If ports are placed uniformly, we reflect that here by equalizing all port margins
    if uniform_port_spacing {
        unify_port_margins(node_context, range);

        // Uniforming may have reset the topmost port's top and bottommost
        // port's bottom margins
        if port_labels_outside {
            node_context.port_contexts[topmost].port_margin.top = 0.0;
            node_context.port_contexts[bottommost].port_margin.bottom = 0.0;
        }
    }
}

fn compute_vertical_port_margins<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
    _port_labels_outside: bool,
) {
    for index in node_context.ports_on_side(port_side) {
        let label_height = match node_context.port_contexts[index].port_label_cell {
            Some(cell) => node_context.cells.min_height(cell),
            None => 0.0,
        };

        if label_height > 0.0 {
            if node_context.port_contexts[index].labels_next_to_port {
                // The label is placed next to the port
                let port_height = g.port_size(node_context.port_contexts[index].port).y;
                if label_height > port_height {
                    let label_cell = node_context.port_contexts[index].port_label_cell.unwrap();
                    if node_context.port_labels_treat_as_group
                        || node_context.cells.label(label_cell).labels.len() == 1
                    {
                        // We are to center all of the labels
                        let overhang = (label_height - port_height) / 2.0;
                        node_context.port_contexts[index].port_margin.top = overhang;
                        node_context.port_contexts[index].port_margin.bottom = overhang;
                    } else {
                        // Simulate centering the first port label
                        let first_label = node_context.cells.label(label_cell).labels[0];
                        let first_label_height = g.label_size(first_label).y;
                        let first_label_overhang = (first_label_height - port_height) / 2.0;

                        node_context.port_contexts[index].port_margin.top =
                            first_label_overhang.max(0.0);
                        node_context.port_contexts[index].port_margin.bottom =
                            label_height - first_label_overhang - port_height;
                    }
                }
            } else {
                // The label is either placed outside (below the port) or possibly
                // inside, but for a compound node, which means that it is placed
                // below the port as well to keep it from overlapping with inside edges
                node_context.port_contexts[index].port_margin.bottom =
                    node_context.port_label_spacing_vertical + label_height;
            }
        } else if PortLabelPlacement::is_fixed(node_context.port_labels_placement) {
            // The fixed port label is not considered with portContext.portLabelCell.
            // Nevertheless, a port margin must be added if necessary.
            let port = node_context.port_contexts[index].port;
            let labels_bounds = elkutil::get_labels_bounds(g, port);
            if labels_bounds.y < 0.0 {
                // Add the part of the label that is above the port to the top margin
                node_context.port_contexts[index].port_margin.top = -labels_bounds.y;
            }
            if labels_bounds.y + labels_bounds.height > g.port_size(port).y {
                // Add the part of the label that is below the port to the bottom margin
                node_context.port_contexts[index].port_margin.bottom =
                    labels_bounds.y + labels_bounds.height - g.port_size(port).y;
            }
        }
    }
}

fn unify_port_margins<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
    range: std::ops::Range<usize>,
) {
    let mut max_top = 0.0f64;
    let mut max_bottom = 0.0f64;

    // Find maximum
    for index in range.clone() {
        max_top = max_top.max(node_context.port_contexts[index].port_margin.top);
        max_bottom = max_bottom.max(node_context.port_contexts[index].port_margin.bottom);
    }

    // Apply maximum
    for index in range {
        node_context.port_contexts[index].port_margin.top = max_top;
        node_context.port_contexts[index].port_margin.bottom = max_bottom;
    }
}

fn port_height_plus_port_port_spacing<G: AdapterGraph>(
    g: &G,
    node_context: &NodeContext<G>,
    port_side: PortSide,
) -> f64 {
    let mut result = 0.0;

    let range = node_context.ports_on_side(port_side);
    let last = range.end;
    for index in range {
        let pc = &node_context.port_contexts[index];

        result += pc.port_margin.top + g.port_size(pc.port).y + pc.port_margin.bottom;

        if index + 1 < last {
            result += node_context.port_port_spacing;
        }
    }

    result
}
