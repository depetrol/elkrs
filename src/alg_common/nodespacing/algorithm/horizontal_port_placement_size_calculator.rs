
use crate::core::adapters::AdapterGraph;
use crate::core::elkutil;
use crate::core::options::{PortAlignment, PortConstraints, PortLabelPlacement, PortSide, SizeConstraint, SizeOptions};

use crate::alg_common::nodespacing::algorithm::port_placement_calculator::PORT_RATIO_OR_POSITION;
use crate::alg_common::nodespacing::internal::NodeContext;

/// Calculates the space
/// required for horizontal port placements. If the port placement is not
/// fixed, this will also setup the left and right padding of the inside port
/// label placement cells.
pub fn calculate_horizontal_port_placement_size<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
) {
    // We need to calculate the space required by the ports even if ports are
    // not part of the size constraints, since we will use that later to place them
    match node_context.port_constraints {
        PortConstraints::FIXED_POS => {
            // We don't have any freedom at all, so simply calculate where the
            // rightmost port is on each side
            calculate_horizontal_node_size_required_by_fixed_pos_ports(g, node_context, PortSide::NORTH);
            calculate_horizontal_node_size_required_by_fixed_pos_ports(g, node_context, PortSide::SOUTH);
        }
        PortConstraints::FIXED_RATIO => {
            // We can require the node to be large enough to avoid spacing
            // violations with fixed ratio ports
            calculate_horizontal_node_size_required_by_fixed_ratio_ports(g, node_context, PortSide::NORTH);
            calculate_horizontal_node_size_required_by_fixed_ratio_ports(g, node_context, PortSide::SOUTH);
        }
        _ => {
            // If we are free to place things, make the node large enough to
            // place everything properly
            calculate_horizontal_node_size_required_by_free_ports(g, node_context, PortSide::NORTH);
            calculate_horizontal_node_size_required_by_free_ports(g, node_context, PortSide::SOUTH);
        }
    }
}

fn calculate_horizontal_node_size_required_by_fixed_pos_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    let mut rightmost_port_border = 0.0f64;

    // Check all ports on the correct side
    for index in node_context.ports_on_side(port_side) {
        let pc = &node_context.port_contexts[index];
        rightmost_port_border =
            rightmost_port_border.max(pc.port_position.x + g.port_size(pc.port).x);
    }

    // Set the cell size and remove left padding since the cell size itself
    // already includes all the space we need
    let cell = node_context.inside_port_label_cell(port_side);
    node_context.cells.padding_mut(cell).left = 0.0;
    node_context.cells.atomic_min_content_area_size_mut(cell).x = rightmost_port_border;
}

fn calculate_horizontal_node_size_required_by_fixed_ratio_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    let cell = node_context.inside_port_label_cell(port_side);

    // Fetch the port contexts on the given side and abort if there are none
    let range = node_context.ports_on_side(port_side);
    if range.is_empty() {
        node_context.cells.padding_mut(cell).left = 0.0;
        node_context.cells.padding_mut(cell).right = 0.0;
        return;
    }

    let port_labels_inside = node_context
        .port_labels_placement
        .contains(PortLabelPlacement::INSIDE);
    let mut min_width = 0.0f64;

    // Go over all pairs of consecutive ports
    let mut previous_port_index: Option<usize> = None;
    let mut previous_port_ratio = 0.0;
    let mut previous_port_width = 0.0;

    for current in range.clone() {
        // Get the next port and find out things about it
        let current_port = node_context.port_contexts[current].port;
        let current_port_ratio = g
            .port_properties(current_port)
            .get(&PORT_RATIO_OR_POSITION);
        let current_port_width = g.port_size(current_port).x;

        // If port labels are to be respected, we need to calculate the port's
        // margins to do so. (This call inside the loop is idempotent.)
        if node_context.size_constraints.contains(SizeConstraint::PORT_LABELS) {
            setup_port_margins(g, node_context, port_side);
        }

        match previous_port_index {
            None => {
                // This is the first port, so find out how wide the node needs to
                // be to respect the left surrounding port margins, if any
                if node_context.surrounding_port_margins.left > 0.0 {
                    min_width = min_width.max(min_size_required_to_respect_spacing(
                        node_context.surrounding_port_margins.left
                            + node_context.port_contexts[current].port_margin.left,
                        0.0,
                        current_port_ratio,
                    ));
                }
            }
            Some(previous) => {
                let required_space = previous_port_width
                    + node_context.port_contexts[previous].port_margin.right
                    + node_context.port_port_spacing
                    + node_context.port_contexts[current].port_margin.left;
                min_width = min_width.max(min_size_required_to_respect_spacing(
                    required_space,
                    previous_port_ratio,
                    current_port_ratio,
                ));
            }
        }

        // Our current port is going to be the previous port during the next iteration
        previous_port_index = Some(current);
        previous_port_ratio = current_port_ratio;
        previous_port_width = current_port_width;
    }

    // If there are right surrounding port margins, apply those as well
    if node_context.surrounding_port_margins.right > 0.0 {
        let mut required_space = previous_port_width + node_context.surrounding_port_margins.right;

        // We're only interested in the port's right margin if its label is placed inside
        if port_labels_inside {
            required_space += node_context.port_contexts[previous_port_index.unwrap()]
                .port_margin
                .right;
        }

        min_width = min_width.max(min_size_required_to_respect_spacing(
            required_space,
            previous_port_ratio,
            1.0,
        ));
    }

    // Set the cell size and remove left padding since the cell size itself
    // already includes all the space we need
    node_context.cells.padding_mut(cell).left = 0.0;
    node_context.cells.atomic_min_content_area_size_mut(cell).x = min_width;
}

/// Fuzzyness allowed to still consider two double values to be equal.
const EQUALITY_TOLERANCE: f64 = 0.01;

/// Guava's `DoubleMath.fuzzyEquals`.
fn fuzzy_equals(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance || a == b || (a.is_nan() && b.is_nan())
}

pub fn min_size_required_to_respect_spacing(spacing: f64, first_ratio: f64, second_ratio: f64) -> f64 {
    // Some failsafing
    if fuzzy_equals(first_ratio, second_ratio, EQUALITY_TOLERANCE) {
        0.0
    } else {
        spacing / (second_ratio - first_ratio)
    }
}

fn calculate_horizontal_node_size_required_by_free_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    let cell = node_context.inside_port_label_cell(port_side);

    // Handle the common case first: if there are no ports, set everything to zero
    if node_context.ports_on_side(port_side).is_empty() {
        node_context.cells.padding_mut(cell).left = 0.0;
        node_context.cells.padding_mut(cell).right = 0.0;
        return;
    }

    // Set the padding to match the surrounding port space
    node_context.cells.padding_mut(cell).left = node_context.surrounding_port_margins.left;
    node_context.cells.padding_mut(cell).right = node_context.surrounding_port_margins.right;

    // If we are to take labels into account, we need to setup the port margins
    // such that they include the space required for their labels
    if node_context.size_constraints.contains(SizeConstraint::PORT_LABELS) {
        setup_port_margins(g, node_context, port_side);
    }

    let mut width = port_width_plus_port_port_spacing(g, node_context, port_side);

    // For distributed port alignment, we need to surround the ports by a
    // port-port spacing on each side
    if node_context.get_port_alignment(g, port_side) == PortAlignment::DISTRIBUTED {
        width += 2.0 * node_context.port_port_spacing;
    }

    // Set the cell size
    node_context.cells.atomic_min_content_area_size_mut(cell).x = width;
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

    // Set the horizontal port margins of all ports according to how their
    // labels will be placed. We'll be modifying the margins soon enough.
    compute_horizontal_port_margins(g, node_context, port_side, port_labels_outside);

    // The leftmost and rightmost ports are possibly required
    let leftmost = range.start;
    let rightmost = range.end - 1;

    // If port labels are placed outside, there's stuff we can do
    if port_labels_outside {
        // The leftmost and rightmost ports don't need their left and right
        // margin, respectively
        node_context.port_contexts[leftmost].port_margin.left = 0.0;
        node_context.port_contexts[rightmost].port_margin.right = 0.0;

        // If we place port labels space-efficiently and the leftmost port
        // doesn't have its label placed right next to it, it doesn't need its
        // right margin either since its label will be placed to its left
        if space_efficient_port_labels && !node_context.port_contexts[leftmost].labels_next_to_port {
            node_context.port_contexts[leftmost].port_margin.right = 0.0;
        }
    }

    // If ports are placed uniformly, we reflect that here by equalizing all port margins
    if uniform_port_spacing {
        unify_port_margins(node_context, range);

        // Uniforming may have reset the leftmost port's left and rightmost
        // port's right margins
        if port_labels_outside {
            node_context.port_contexts[leftmost].port_margin.left = 0.0;
            node_context.port_contexts[rightmost].port_margin.right = 0.0;
        }
    }
}

fn compute_horizontal_port_margins<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
    _port_labels_outside: bool,
) {
    for index in node_context.ports_on_side(port_side) {
        let label_width = match node_context.port_contexts[index].port_label_cell {
            Some(cell) => node_context.cells.min_width(cell),
            None => 0.0,
        };

        if label_width > 0.0 {
            if node_context.port_contexts[index].labels_next_to_port {
                // The label is placed next to the port
                let port_width = g.port_size(node_context.port_contexts[index].port).x;
                if label_width > port_width {
                    let overhang = (label_width - port_width) / 2.0;
                    node_context.port_contexts[index].port_margin.left = overhang;
                    node_context.port_contexts[index].port_margin.right = overhang;
                }
            } else {
                // The label is either placed outside (right to the port) or
                // possibly inside, but for a compound node, which means that it is
                // placed right of the port as well to keep it from overlapping
                // with inside edges
                node_context.port_contexts[index].port_margin.right =
                    node_context.port_label_spacing_horizontal + label_width;
            }
        } else if PortLabelPlacement::is_fixed(node_context.port_labels_placement) {
            // The fixed port label is not considered with portContext.portLabelCell.
            // Nevertheless, a port margin must be added if necessary.
            let port = node_context.port_contexts[index].port;
            let labels_bounds = elkutil::get_labels_bounds(g, port);
            if labels_bounds.x < 0.0 {
                // Add the part of the label that is on the left of the port to the left margin
                node_context.port_contexts[index].port_margin.left = -labels_bounds.x;
            }
            if labels_bounds.x + labels_bounds.width > g.port_size(port).x {
                // Add the part of the label that is on the right of the port to the right margin
                node_context.port_contexts[index].port_margin.right =
                    labels_bounds.x + labels_bounds.width - g.port_size(port).x;
            }
        }
    }
}

/// Sets all port margins to the maximum margins.
fn unify_port_margins<G: AdapterGraph>(
    node_context: &mut NodeContext<G>,
    range: std::ops::Range<usize>,
) {
    let mut max_left = 0.0f64;
    let mut max_right = 0.0f64;

    // Find maximum
    for index in range.clone() {
        max_left = max_left.max(node_context.port_contexts[index].port_margin.left);
        max_right = max_right.max(node_context.port_contexts[index].port_margin.right);
    }

    // Apply maximum
    for index in range {
        node_context.port_contexts[index].port_margin.left = max_left;
        node_context.port_contexts[index].port_margin.right = max_right;
    }
}

fn port_width_plus_port_port_spacing<G: AdapterGraph>(
    g: &G,
    node_context: &NodeContext<G>,
    port_side: PortSide,
) -> f64 {
    let mut result = 0.0;

    let range = node_context.ports_on_side(port_side);
    let last = range.end;
    for index in range {
        let pc = &node_context.port_contexts[index];

        // Add the current port's width to our result
        result += pc.port_margin.left + g.port_size(pc.port).x + pc.port_margin.right;

        // If there is another port after this one, include the required spacing
        if index + 1 < last {
            result += node_context.port_port_spacing;
        }
    }

    result
}
