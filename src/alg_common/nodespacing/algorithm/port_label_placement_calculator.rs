
use crate::core::adapters::AdapterGraph;
use crate::core::options::{self, PortConstraints, PortLabelPlacement, PortSide, SizeConstraint};
use crate::graph::math::{ElkRectangle, KVector};

use crate::alg_common::nodespacing::algorithm::node_label_and_size_utilities as utilities;
use crate::alg_common::nodespacing::cellsystem::{HorizontalLabelAlignment, VerticalLabelAlignment};
use crate::alg_common::nodespacing::internal::NodeContext;
use crate::alg_common::overlaps::{OverlapRemovalDirection, RectangleStripOverlapRemover};

/// Places port labels for northern and
/// southern ports.
pub fn place_horizontal_port_labels<G: AdapterGraph>(g: &G, node_context: &mut NodeContext<G>) {
    place_port_labels(g, node_context, PortSide::NORTH);
    place_port_labels(g, node_context, PortSide::SOUTH);
}

/// Places port labels for eastern and
/// western ports.
pub fn place_vertical_port_labels<G: AdapterGraph>(g: &G, node_context: &mut NodeContext<G>) {
    place_port_labels(g, node_context, PortSide::EAST);
    place_port_labels(g, node_context, PortSide::WEST);
}

fn place_port_labels<G: AdapterGraph>(g: &G, node_context: &mut NodeContext<G>, port_side: PortSide) {
    // If port labels were not taken into account when calculating the node
    // size or if port placement was set to fixed positions, we don't have an
    // arbitrary amount of freedom to place our labels
    let constrained_placement = !node_context
        .size_constraints
        .contains(SizeConstraint::PORT_LABELS)
        || node_context.port_constraints == PortConstraints::FIXED_POS;

    if node_context
        .port_labels_placement
        .contains(PortLabelPlacement::INSIDE)
    {
        if constrained_placement {
            constrained_inside_port_label_placement(g, node_context, port_side);
        } else {
            simple_inside_port_label_placement(g, node_context, port_side);
        }
    } else if node_context
        .port_labels_placement
        .contains(PortLabelPlacement::OUTSIDE)
    {
        if constrained_placement {
            constrained_outside_port_label_placement(g, node_context, port_side);
        } else {
            simple_outside_port_label_placement(g, node_context, port_side);
        }
    }
}

/// Whether the given port context has a label cell with labels.
fn has_port_labels<G: AdapterGraph>(node_context: &NodeContext<G>, index: usize) -> bool {
    match node_context.port_contexts[index].port_label_cell {
        Some(cell) => node_context.cells.label_has_labels(cell),
        None => false,
    }
}

fn simple_inside_port_label_placement<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    // For northern and southern port labels, we need to set the inside port
    // label cell's client height later
    let mut inside_north_or_south_port_label_area_height = 0.0f64;

    // Some spacings we may need later
    let label_border_offset = port_label_border_offset_for_port_side(node_context, port_side);
    let port_label_spacing_horizontal = node_context.port_label_spacing_horizontal;
    let port_label_spacing_vertical = node_context.port_label_spacing_vertical;

    for index in node_context.ports_on_side(port_side) {
        // If the port doesn't have labels, skip
        if !has_port_labels(node_context, index) {
            continue;
        }

        // Retrieve information about the port itself
        let port = node_context.port_contexts[index].port;
        let port_size = g.port_size(port);
        let port_border_offset = g
            .port_properties(port)
            .try_get(&options::PORT_BORDER_OFFSET)
            .unwrap_or(0.0);

        // Retrieve the label cell and its rectangle and set the rectangle's
        // size (we will use the rectangle to place the cell relative to the
        // port below)
        let port_label_cell = node_context.port_contexts[index].port_label_cell.unwrap();
        let labels_next_to_port = node_context.port_contexts[index].labels_next_to_port;
        let min_width = node_context.cells.min_width(port_label_cell);
        let min_height = node_context.cells.min_height(port_label_cell);
        {
            let rect = node_context.cells.rect_mut(port_label_cell);
            rect.width = min_width;
            rect.height = min_height;
        }

        // Calculate the position of the port's label cell. If the node is a
        // compound node, we make an effort to place port labels such that
        // edges won't cross them
        match port_side {
            PortSide::NORTH => {
                let x = if labels_next_to_port {
                    (port_size.x - min_width) / 2.0
                } else {
                    port_size.x + port_label_spacing_horizontal
                };
                let rect = node_context.cells.rect_mut(port_label_cell);
                rect.x = x;
                rect.y = port_size.y + port_border_offset + label_border_offset;
                let data = node_context.cells.label_mut(port_label_cell);
                data.horizontal_alignment = HorizontalLabelAlignment::Center;
                data.vertical_alignment = VerticalLabelAlignment::Top;
            }
            PortSide::SOUTH => {
                let x = if labels_next_to_port {
                    (port_size.x - min_width) / 2.0
                } else {
                    port_size.x + port_label_spacing_horizontal
                };
                let rect = node_context.cells.rect_mut(port_label_cell);
                rect.x = x;
                rect.y = -port_border_offset - label_border_offset - min_height;
                let data = node_context.cells.label_mut(port_label_cell);
                data.horizontal_alignment = HorizontalLabelAlignment::Center;
                data.vertical_alignment = VerticalLabelAlignment::Bottom;
            }
            PortSide::EAST => {
                let y = if labels_next_to_port {
                    let label_height = if node_context.port_labels_treat_as_group {
                        min_height
                    } else {
                        let first_label = node_context.cells.label(port_label_cell).labels[0];
                        g.label_size(first_label).y
                    };
                    (port_size.y - label_height) / 2.0
                } else {
                    port_size.y + port_label_spacing_vertical
                };
                let rect = node_context.cells.rect_mut(port_label_cell);
                rect.x = -port_border_offset - label_border_offset - min_width;
                rect.y = y;
                let data = node_context.cells.label_mut(port_label_cell);
                data.horizontal_alignment = HorizontalLabelAlignment::Right;
                data.vertical_alignment = VerticalLabelAlignment::Center;
            }
            PortSide::WEST => {
                let y = if labels_next_to_port {
                    let label_height = if node_context.port_labels_treat_as_group {
                        min_height
                    } else {
                        let first_label = node_context.cells.label(port_label_cell).labels[0];
                        g.label_size(first_label).y
                    };
                    (port_size.y - label_height) / 2.0
                } else {
                    port_size.y + port_label_spacing_vertical
                };
                let rect = node_context.cells.rect_mut(port_label_cell);
                rect.x = port_size.x + port_border_offset + label_border_offset;
                rect.y = y;
                let data = node_context.cells.label_mut(port_label_cell);
                data.horizontal_alignment = HorizontalLabelAlignment::Left;
                data.vertical_alignment = VerticalLabelAlignment::Center;
            }
            PortSide::UNDEFINED => {}
        }

        // If we have a north or south port, update our port label area height
        if port_side == PortSide::NORTH || port_side == PortSide::SOUTH {
            inside_north_or_south_port_label_area_height =
                inside_north_or_south_port_label_area_height
                    .max(node_context.cells.rect(port_label_cell).height);
        }
    }

    // If we have a northern or southern label area height, apply it
    if inside_north_or_south_port_label_area_height > 0.0 {
        let cell = node_context.inside_port_label_cell(port_side);
        node_context.cells.atomic_min_content_area_size_mut(cell).y =
            inside_north_or_south_port_label_area_height;
    }
}

fn port_label_border_offset_for_port_side<G: AdapterGraph>(
    node_context: &NodeContext<G>,
    port_side: PortSide,
) -> f64 {
    let padding = node_context.cells.padding(node_context.node_container);
    match port_side {
        PortSide::NORTH => padding.top + node_context.port_label_spacing_vertical,
        PortSide::SOUTH => padding.bottom + node_context.port_label_spacing_vertical,
        PortSide::EAST => padding.right + node_context.port_label_spacing_horizontal,
        PortSide::WEST => padding.left + node_context.port_label_spacing_horizontal,
        PortSide::UNDEFINED => 0.0,
    }
}

fn constrained_inside_port_label_placement<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    // If it's neither the northern nor the southern side, simply revert to
    // simple port label placement
    if port_side == PortSide::EAST || port_side == PortSide::WEST {
        simple_inside_port_label_placement(g, node_context, port_side);
        return;
    }

    // Prepare things
    let overlap_removal_direction = if port_side == PortSide::NORTH {
        OverlapRemovalDirection::Down
    } else {
        OverlapRemovalDirection::Up
    };
    let vertical_label_alignment = if port_side == PortSide::NORTH {
        VerticalLabelAlignment::Top
    } else {
        VerticalLabelAlignment::Bottom
    };

    // To keep labels from extending over the content area of the inside port
    // label container, we need to know where its content area's left and right
    // boundaries are. We also make sure to always keep a bit of space to the
    // node border
    let inside_port_label_container = node_context.inside_port_label_cell(port_side);
    let label_container_rect = node_context.cells.rect(inside_port_label_container);
    let container_padding = node_context.cells.padding(inside_port_label_container);
    // ElkMath.maxd over several values
    let left_border = label_container_rect.x
        + container_padding
            .left
            .max(node_context.surrounding_port_margins.left)
            .max(node_context.node_label_spacing);
    let right_border = label_container_rect.x + label_container_rect.width
        - container_padding
            .right
            .max(node_context.surrounding_port_margins.right)
            .max(node_context.node_label_spacing);

    // Obtain a rectangle strip overlap remover, which will actually do most of the work
    let mut overlap_remover =
        RectangleStripOverlapRemover::create_for_direction(overlap_removal_direction).with_gap(
            node_context.port_label_spacing_horizontal,
            node_context.port_label_spacing_vertical,
        );

    // Iterate over our ports and add rectangles to the overlap remover. Also,
    // calculate the start coordinate
    let mut start_coordinate = if port_side == PortSide::NORTH {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    };

    let range = node_context.ports_on_side(port_side);
    let mut handles: Vec<(usize, usize)> = Vec::new();

    for index in range.clone() {
        if !has_port_labels(node_context, index) {
            continue;
        }

        let port = node_context.port_contexts[index].port;
        let port_size = g.port_size(port);
        let port_position = node_context.port_contexts[index].port_position;
        let port_label_cell = node_context.port_contexts[index].port_label_cell.unwrap();

        // Setup the less interesting cell properties
        let min_width = node_context.cells.min_width(port_label_cell);
        let min_height = node_context.cells.min_height(port_label_cell);
        {
            let rect = node_context.cells.rect_mut(port_label_cell);
            rect.width = min_width;
            rect.height = min_height;
        }
        {
            let data = node_context.cells.label_mut(port_label_cell);
            data.vertical_alignment = vertical_label_alignment;
            data.horizontal_alignment = HorizontalLabelAlignment::Right;
        }

        // Center the label, but make sure it doesn't hang over the node boundaries
        {
            let rect = node_context.cells.rect_mut(port_label_cell);
            center_port_label(rect, port_position, port_size, left_border, right_border);
        }

        // Add the rectangle to the overlap remover
        let handle = overlap_remover.add_rectangle(node_context.cells.rect(port_label_cell));
        handles.push((index, handle));

        // Update start coordinate
        start_coordinate = if port_side == PortSide::NORTH {
            start_coordinate.max(port_position.y + port_size.y)
        } else {
            start_coordinate.min(port_position.y)
        };
    }

    // The start coordinate needs to be offset by the port-label space
    start_coordinate += if port_side == PortSide::NORTH {
        node_context.port_label_spacing_vertical
    } else {
        -node_context.port_label_spacing_vertical
    };

    // Invoke the overlap remover
    let strip_height = {
        overlap_remover = overlap_remover.with_start_coordinate(start_coordinate);
        overlap_remover.remove_overlaps()
    };

    // Write the moved rectangles back into the cell system
    for &(index, handle) in &handles {
        let port_label_cell = node_context.port_contexts[index].port_label_cell.unwrap();
        *node_context.cells.rect_mut(port_label_cell) = overlap_remover.rectangle(handle);
    }

    if strip_height > 0.0 {
        let cell = node_context.inside_port_label_cell(port_side);
        node_context.cells.atomic_min_content_area_size_mut(cell).y = strip_height;
    }

    // We need to update the label cell's coordinates to be relative to the ports
    for index in range {
        if !has_port_labels(node_context, index) {
            continue;
        }

        let port_position = node_context.port_contexts[index].port_position;
        let port_label_cell = node_context.port_contexts[index].port_label_cell.unwrap();
        let rect = node_context.cells.rect_mut(port_label_cell);

        // Setup the label cell's cell rectangle
        rect.x -= port_position.x;
        rect.y -= port_position.y;
    }
}

/// Centers the given label under its port, but
/// makes an effort to keep it from hanging over the given minimum and maximum
/// coordinates. The label position is absolute, not relative to the port.
fn center_port_label(
    port_label_cell_rect: &mut ElkRectangle,
    port_position: KVector,
    port_size: KVector,
    min_x: f64,
    max_x: f64,
) {
    // Center the label
    port_label_cell_rect.x = port_position.x - (port_label_cell_rect.width - port_size.x) / 2.0;

    // Make sure that the label won't slide past the port
    let actual_min_x = min_x.min(port_position.x);
    let actual_max_x = max_x.max(port_position.x + port_size.x);

    // Make sure that the label stays inside the boundaries, but only correct
    // in one of the two possible directions
    if port_label_cell_rect.x < actual_min_x {
        port_label_cell_rect.x = actual_min_x;
    } else if port_label_cell_rect.x + port_label_cell_rect.width > actual_max_x {
        port_label_cell_rect.x = actual_max_x - port_label_cell_rect.width;
    }
}

fn simple_outside_port_label_placement<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    // If there are only two ports on a side, we place the first port's label
    // on its other side to make it especially clear which port it belongs to.
    // The same applies if the user requested space-efficient mode
    let mut place_first_port_differently =
        utilities::is_first_outside_port_label_placed_differently(node_context, port_side);

    let always_above = node_context
        .port_labels_placement
        .contains(PortLabelPlacement::ALWAYS_OTHER_SAME_SIDE);

    for index in node_context.ports_on_side(port_side) {
        // If the port doesn't have labels, skip
        if !has_port_labels(node_context, index) {
            continue;
        }

        // Retrieve information about the port itself
        let port = node_context.port_contexts[index].port;
        let port_size = g.port_size(port);
        let labels_next_to_port = node_context.port_contexts[index].labels_next_to_port;

        // Retrieve the label cell and its rectangle and set the rectangle's size
        let port_label_cell = node_context.port_contexts[index].port_label_cell.unwrap();
        let min_width = node_context.cells.min_width(port_label_cell);
        let min_height = node_context.cells.min_height(port_label_cell);
        {
            let rect = node_context.cells.rect_mut(port_label_cell);
            rect.width = min_width;
            rect.height = min_height;
        }

        // Calculate the position of the port's label space
        match port_side {
            PortSide::NORTH | PortSide::SOUTH => {
                let (x, h_align) = if labels_next_to_port {
                    ((port_size.x - min_width) / 2.0, HorizontalLabelAlignment::Center)
                } else if place_first_port_differently || always_above {
                    (
                        -min_width - node_context.port_label_spacing_horizontal,
                        HorizontalLabelAlignment::Right,
                    )
                } else {
                    (
                        port_size.x + node_context.port_label_spacing_horizontal,
                        HorizontalLabelAlignment::Left,
                    )
                };
                let y = if port_side == PortSide::NORTH {
                    -min_height - node_context.port_label_spacing_vertical
                } else {
                    port_size.y + node_context.port_label_spacing_vertical
                };
                let v_align = if port_side == PortSide::NORTH {
                    VerticalLabelAlignment::Bottom
                } else {
                    VerticalLabelAlignment::Top
                };
                let rect = node_context.cells.rect_mut(port_label_cell);
                rect.x = x;
                rect.y = y;
                let data = node_context.cells.label_mut(port_label_cell);
                data.horizontal_alignment = h_align;
                data.vertical_alignment = v_align;
            }
            PortSide::EAST | PortSide::WEST => {
                let (y, v_align) = if labels_next_to_port {
                    let label_height = if node_context.port_labels_treat_as_group {
                        min_height
                    } else {
                        let first_label = node_context.cells.label(port_label_cell).labels[0];
                        g.label_size(first_label).y
                    };
                    ((port_size.y - label_height) / 2.0, VerticalLabelAlignment::Center)
                } else if place_first_port_differently || always_above {
                    (
                        -min_height - node_context.port_label_spacing_vertical,
                        VerticalLabelAlignment::Bottom,
                    )
                } else {
                    (
                        port_size.y + node_context.port_label_spacing_vertical,
                        VerticalLabelAlignment::Top,
                    )
                };
                let (x, h_align) = if port_side == PortSide::EAST {
                    (
                        port_size.x + node_context.port_label_spacing_horizontal,
                        HorizontalLabelAlignment::Left,
                    )
                } else {
                    (
                        -min_width - node_context.port_label_spacing_horizontal,
                        HorizontalLabelAlignment::Right,
                    )
                };
                let rect = node_context.cells.rect_mut(port_label_cell);
                rect.x = x;
                rect.y = y;
                let data = node_context.cells.label_mut(port_label_cell);
                data.horizontal_alignment = h_align;
                data.vertical_alignment = v_align;
            }
            PortSide::UNDEFINED => {}
        }

        // The next port definitely doesn't have special needs anymore
        place_first_port_differently = false;
    }
}

fn constrained_outside_port_label_placement<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    let range = node_context.ports_on_side(port_side);

    // If there are at most two ports on this port side, or if it's neither
    // the northern nor the southern side, simply revert to simple port label
    // placement
    if range.len() <= 2 || port_side == PortSide::EAST || port_side == PortSide::WEST {
        simple_outside_port_label_placement(g, node_context, port_side);
        return;
    }

    // If space-efficient port labels are active, the leftmost / topmost
    // port's label must be placed to its left / above it
    let mut port_with_special_needs = node_context
        .port_labels_placement
        .contains(PortLabelPlacement::SPACE_EFFICIENT);

    // Prepare things
    let overlap_removal_direction = if port_side == PortSide::NORTH {
        OverlapRemovalDirection::Up
    } else {
        OverlapRemovalDirection::Down
    };
    let vertical_label_alignment = if port_side == PortSide::NORTH {
        VerticalLabelAlignment::Bottom
    } else {
        VerticalLabelAlignment::Top
    };

    // Obtain a rectangle strip overlap remover, which will actually do most of the work
    let mut overlap_remover =
        RectangleStripOverlapRemover::create_for_direction(overlap_removal_direction).with_gap(
            node_context.port_label_spacing_vertical,
            node_context.port_label_spacing_horizontal,
        );

    // Iterate over our ports and add rectangles to the overlap remover. Also,
    // calculate the start coordinate
    let mut start_coordinate = if port_side == PortSide::NORTH {
        f64::INFINITY
    } else {
        f64::NEG_INFINITY
    };

    let mut handles: Vec<(usize, usize)> = Vec::new();

    for index in range.clone() {
        if !has_port_labels(node_context, index) {
            continue;
        }

        let port = node_context.port_contexts[index].port;
        let port_size = g.port_size(port);
        let port_position = node_context.port_contexts[index].port_position;
        let port_label_cell = node_context.port_contexts[index].port_label_cell.unwrap();

        // Setup the label cell's cell rectangle
        let min_width = node_context.cells.min_width(port_label_cell);
        let min_height = node_context.cells.min_height(port_label_cell);
        {
            let rect = node_context.cells.rect_mut(port_label_cell);
            rect.width = min_width;
            rect.height = min_height;
            if port_with_special_needs {
                rect.x =
                    port_position.x - min_width - node_context.port_label_spacing_horizontal;
                port_with_special_needs = false;
            } else {
                rect.x = port_position.x + port_size.x + node_context.port_label_spacing_horizontal;
            }
        }

        {
            let data = node_context.cells.label_mut(port_label_cell);
            data.vertical_alignment = vertical_label_alignment;
            data.horizontal_alignment = HorizontalLabelAlignment::Right;
        }

        // Add the rectangle to the overlap remover
        let handle = overlap_remover.add_rectangle(node_context.cells.rect(port_label_cell));
        handles.push((index, handle));

        // Update start coordinate
        start_coordinate = if port_side == PortSide::NORTH {
            start_coordinate.min(port_position.y)
        } else {
            start_coordinate.max(port_position.y + port_size.y)
        };
    }

    // The start coordinate needs to be offset by the port-label space
    start_coordinate += if port_side == PortSide::NORTH {
        -node_context.port_label_spacing_vertical
    } else {
        node_context.port_label_spacing_vertical
    };

    // Invoke the overlap remover
    overlap_remover = overlap_remover.with_start_coordinate(start_coordinate);
    overlap_remover.remove_overlaps();

    // Write the moved rectangles back into the cell system
    for &(index, handle) in &handles {
        let port_label_cell = node_context.port_contexts[index].port_label_cell.unwrap();
        *node_context.cells.rect_mut(port_label_cell) = overlap_remover.rectangle(handle);
    }

    // We need to update the label cell's coordinates to be relative to the ports
    for index in range {
        if !has_port_labels(node_context, index) {
            continue;
        }

        let port_position = node_context.port_contexts[index].port_position;
        let port_label_cell = node_context.port_contexts[index].port_label_cell.unwrap();
        let rect = node_context.cells.rect_mut(port_label_cell);

        // Setup the label cell's cell rectangle
        rect.x -= port_position.x;
        rect.y -= port_position.y;
    }
}
