
use crate::core::adapters::AdapterGraph;
use crate::core::options::{self, PortAlignment, PortConstraints, PortSide, SizeOptions};
use crate::graph::properties::Property;

use crate::alg_common::nodespacing::internal::NodeContext;

/// Copy of the layered algorithm's `PORT_RATIO_OR_POSITION` internal option.
pub static PORT_RATIO_OR_POSITION: Property<f64> =
    Property::with_default("portRatioOrPosition", || 0.0);

pub fn place_horizontal_ports<G: AdapterGraph>(g: &G, node_context: &mut NodeContext<G>) {
    // How we are going to place the ports depends on their constraints
    match node_context.port_constraints {
        PortConstraints::FIXED_POS => {
            place_horizontal_fixed_pos_ports(g, node_context, PortSide::NORTH);
            place_horizontal_fixed_pos_ports(g, node_context, PortSide::SOUTH);
        }
        PortConstraints::FIXED_RATIO => {
            place_horizontal_fixed_ratio_ports(g, node_context, PortSide::NORTH);
            place_horizontal_fixed_ratio_ports(g, node_context, PortSide::SOUTH);
        }
        _ => {
            place_horizontal_free_ports(g, node_context, PortSide::NORTH);
            place_horizontal_free_ports(g, node_context, PortSide::SOUTH);
        }
    }
}

fn place_horizontal_fixed_pos_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    for index in node_context.ports_on_side(port_side) {
        // The port's x coordinate is already fixed anyway, so simply adjust its
        // y coordinate according to its offset, if any
        node_context.port_contexts[index].port_position.y =
            calculate_horizontal_port_y_coordinate(g, node_context, index);
    }
}

fn place_horizontal_fixed_ratio_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    let node_width = node_context.node_size.x;

    for index in node_context.ports_on_side(port_side) {
        // The x coordinate is a function of the node's width and the port's position ratio
        let port = node_context.port_contexts[index].port;
        node_context.port_contexts[index].port_position.x =
            node_width * g.port_properties(port).get(&PORT_RATIO_OR_POSITION);
        node_context.port_contexts[index].port_position.y =
            calculate_horizontal_port_y_coordinate(g, node_context, index);
    }
}

fn place_horizontal_free_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    // If there are no ports on the given side, abort
    let range = node_context.ports_on_side(port_side);
    if range.is_empty() {
        return;
    }
    let port_count = range.len();

    // Retrieve the proper inside port label cell, which will give us hints as
    // to where to place our ports
    let inside_port_label_cell = node_context.inside_port_label_cell(port_side);
    let inside_port_label_cell_rectangle = node_context.cells.rect(inside_port_label_cell);
    let inside_port_label_cell_padding = node_context.cells.padding(inside_port_label_cell);

    // Note that we don't have to distinguish any cases here because the port
    // margins already include space required for labels, if such space is to
    // be reserved. Yay!
    let mut port_alignment = node_context.get_port_alignment(g, port_side);
    let available_space = inside_port_label_cell_rectangle.width
        - inside_port_label_cell_padding.left
        - inside_port_label_cell_padding.right;
    let mut calculated_port_placement_width = node_context
        .cells
        .atomic_min_content_area_size(inside_port_label_cell)
        .x;
    let mut current_x_pos = inside_port_label_cell_rectangle.x + inside_port_label_cell_padding.left;
    let mut space_between_ports = node_context.port_port_spacing;

    // If the port alignment is distributed or justified, but there's only a
    // single port, we change the alignment to center to keep things from
    // looking stupid
    if (port_alignment == PortAlignment::DISTRIBUTED || port_alignment == PortAlignment::JUSTIFIED)
        && port_count == 1
    {
        calculated_port_placement_width =
            modified_port_placement_size(node_context, port_alignment, calculated_port_placement_width);
        port_alignment = PortAlignment::CENTER;
    }

    if available_space < calculated_port_placement_width
        && !node_context.size_options.contains(SizeOptions::PORTS_OVERHANG)
    {
        // There is not enough space available for the ports, but they are not
        // allowed to overhang either. Reduce the space between them to cram
        // them into the available space.
        if port_alignment == PortAlignment::DISTRIBUTED {
            space_between_ports +=
                (available_space - calculated_port_placement_width) / (port_count + 1) as f64;
            current_x_pos += space_between_ports;
        } else {
            space_between_ports +=
                (available_space - calculated_port_placement_width) / (port_count - 1) as f64;
        }
    } else {
        // We are allowed to overhang. Yay. However, if we use distributed or
        // justified alignment, this is another case where we should fall back
        // to centered alignment
        if available_space < calculated_port_placement_width {
            calculated_port_placement_width = modified_port_placement_size(
                node_context,
                port_alignment,
                calculated_port_placement_width,
            );
            port_alignment = PortAlignment::CENTER;
        }

        // Calculate where we need to start placing ports (note that the node
        // size required by the port placement includes left and right
        // surrounding port margins, which changes the formulas a bit from what
        // you'd otherwise expect)
        match port_alignment {
            PortAlignment::BEGIN => {
                // There's nothing to do here
            }
            PortAlignment::CENTER => {
                current_x_pos += (available_space - calculated_port_placement_width) / 2.0;
            }
            PortAlignment::END => {
                current_x_pos += available_space - calculated_port_placement_width;
            }
            PortAlignment::DISTRIBUTED => {
                // In this case, if there is not enough space available to place
                // the ports, we are allowed to overhang. We thus need to ensure
                // that we're only ever increasing the port spacing here
                let additional_space_between_ports =
                    (available_space - calculated_port_placement_width) / (port_count + 1) as f64;
                space_between_ports += additional_space_between_ports.max(0.0);
                current_x_pos += space_between_ports;
            }
            PortAlignment::JUSTIFIED => {
                let additional_space_between_ports =
                    (available_space - calculated_port_placement_width) / (port_count - 1) as f64;
                space_between_ports += additional_space_between_ports.max(0.0);
            }
        }
    }

    // Iterate over all ports and place them
    for index in range {
        let y = calculate_horizontal_port_y_coordinate(g, node_context, index);
        let port_size_x = g.port_size(node_context.port_contexts[index].port).x;
        let pc = &mut node_context.port_contexts[index];
        pc.port_position.x = current_x_pos + pc.port_margin.left;
        pc.port_position.y = y;

        // Update the x coordinate for the next port
        current_x_pos += pc.port_margin.left + port_size_x + pc.port_margin.right + space_between_ports;
    }
}

fn calculate_horizontal_port_y_coordinate<G: AdapterGraph>(
    g: &G,
    node_context: &NodeContext<G>,
    index: usize,
) -> f64 {
    let pc = &node_context.port_contexts[index];
    let port = pc.port;

    // The y coordinate is set according to the port's offset, if any
    if g.port_properties(port).has(&options::PORT_BORDER_OFFSET) {
        let offset = g
            .port_properties(port)
            .try_get(&options::PORT_BORDER_OFFSET)
            .unwrap_or(0.0);
        if pc.side == PortSide::NORTH {
            -g.port_size(port).y - offset
        } else {
            offset
        }
    } else if pc.side == PortSide::NORTH {
        -g.port_size(port).y
    } else {
        0.0
    }
}

pub fn place_vertical_ports<G: AdapterGraph>(g: &G, node_context: &mut NodeContext<G>) {
    // How we are going to place the ports depends on their constraints
    match node_context.port_constraints {
        PortConstraints::FIXED_POS => {
            place_vertical_fixed_pos_ports(g, node_context, PortSide::EAST);
            place_vertical_fixed_pos_ports(g, node_context, PortSide::WEST);
        }
        PortConstraints::FIXED_RATIO => {
            place_vertical_fixed_ratio_ports(g, node_context, PortSide::EAST);
            place_vertical_fixed_ratio_ports(g, node_context, PortSide::WEST);
        }
        _ => {
            place_vertical_free_ports(g, node_context, PortSide::EAST);
            place_vertical_free_ports(g, node_context, PortSide::WEST);
        }
    }
}

fn place_vertical_fixed_pos_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    let node_width = node_context.node_size.x;

    for index in node_context.ports_on_side(port_side) {
        // The port's y coordinate is already fixed anyway, so simply adjust its
        // x coordinate according to its offset, if any
        node_context.port_contexts[index].port_position.x =
            calculate_vertical_port_x_coordinate(g, node_context, index, node_width);
    }
}

fn place_vertical_fixed_ratio_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    let node_size = node_context.node_size;

    for index in node_context.ports_on_side(port_side) {
        // The y coordinate is a function of the node's height and the port's position ratio
        let port = node_context.port_contexts[index].port;
        node_context.port_contexts[index].port_position.x =
            calculate_vertical_port_x_coordinate(g, node_context, index, node_size.x);
        node_context.port_contexts[index].port_position.y =
            node_size.y * g.port_properties(port).get(&PORT_RATIO_OR_POSITION);
    }
}

fn place_vertical_free_ports<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port_side: PortSide,
) {
    // If there are no ports on the given side, abort
    let range = node_context.ports_on_side(port_side);
    if range.is_empty() {
        return;
    }
    let port_count = range.len();

    // Retrieve the proper inside port label cell, which will give us hints as
    // to where to place our ports
    let inside_port_label_cell = node_context.inside_port_label_cell(port_side);
    let inside_port_label_cell_rectangle = node_context.cells.rect(inside_port_label_cell);
    let inside_port_label_cell_padding = node_context.cells.padding(inside_port_label_cell);

    // Note that we don't have to distinguish any cases here because the port
    // margins already include space required for labels, if such space is to
    // be reserved. Yay!
    let mut port_alignment = node_context.get_port_alignment(g, port_side);
    let available_space = inside_port_label_cell_rectangle.height
        - inside_port_label_cell_padding.top
        - inside_port_label_cell_padding.bottom;
    let mut calculated_port_placement_height = node_context
        .cells
        .atomic_min_content_area_size(inside_port_label_cell)
        .y;
    let mut current_y_pos = inside_port_label_cell_rectangle.y + inside_port_label_cell_padding.top;
    let mut space_between_ports = node_context.port_port_spacing;
    let node_width = node_context.node_size.x;

    // If the port alignment is distributed or justified, but there's only a
    // single port, we change the alignment to center to keep things from
    // looking stupid
    if (port_alignment == PortAlignment::DISTRIBUTED || port_alignment == PortAlignment::JUSTIFIED)
        && port_count == 1
    {
        calculated_port_placement_height = modified_port_placement_size(
            node_context,
            port_alignment,
            calculated_port_placement_height,
        );
        port_alignment = PortAlignment::CENTER;
    }

    if available_space < calculated_port_placement_height
        && !node_context.size_options.contains(SizeOptions::PORTS_OVERHANG)
    {
        // There is not enough space available for the ports, but they are not
        // allowed to overhang either. Reduce the space between them to cram
        // them into the available space.
        if port_alignment == PortAlignment::DISTRIBUTED {
            space_between_ports +=
                (available_space - calculated_port_placement_height) / (port_count + 1) as f64;
            current_y_pos += space_between_ports;
        } else {
            space_between_ports +=
                (available_space - calculated_port_placement_height) / (port_count - 1) as f64;
        }
    } else {
        // We are allowed to overhang. Yay. However, if we use distributed or
        // justified alignment, this is another case where we should fall back
        // to centered alignment
        if available_space < calculated_port_placement_height {
            calculated_port_placement_height = modified_port_placement_size(
                node_context,
                port_alignment,
                calculated_port_placement_height,
            );
            port_alignment = PortAlignment::CENTER;
        }

        // Calculate where we need to start placing ports
        match port_alignment {
            PortAlignment::BEGIN => {
                // There's nothing to do here
            }
            PortAlignment::CENTER => {
                current_y_pos += (available_space - calculated_port_placement_height) / 2.0;
            }
            PortAlignment::END => {
                current_y_pos += available_space - calculated_port_placement_height;
            }
            PortAlignment::DISTRIBUTED => {
                let additional_space_between_ports =
                    (available_space - calculated_port_placement_height) / (port_count + 1) as f64;
                space_between_ports += additional_space_between_ports.max(0.0);
                current_y_pos += space_between_ports;
            }
            PortAlignment::JUSTIFIED => {
                let additional_space_between_ports =
                    (available_space - calculated_port_placement_height) / (port_count - 1) as f64;
                space_between_ports += additional_space_between_ports.max(0.0);
            }
        }
    }

    // Iterate over all ports and place them
    for index in range {
        let x = calculate_vertical_port_x_coordinate(g, node_context, index, node_width);
        let port_size_y = g.port_size(node_context.port_contexts[index].port).y;
        let pc = &mut node_context.port_contexts[index];
        pc.port_position.x = x;
        pc.port_position.y = current_y_pos + pc.port_margin.top;

        // Update the y coordinate for the next port
        current_y_pos += pc.port_margin.top + port_size_y + pc.port_margin.bottom + space_between_ports;
    }
}

fn calculate_vertical_port_x_coordinate<G: AdapterGraph>(
    g: &G,
    node_context: &NodeContext<G>,
    index: usize,
    node_width: f64,
) -> f64 {
    let pc = &node_context.port_contexts[index];
    let port = pc.port;

    // The x coordinate is set according to the port's offset, if any
    if g.port_properties(port).has(&options::PORT_BORDER_OFFSET) {
        let offset = g
            .port_properties(port)
            .try_get(&options::PORT_BORDER_OFFSET)
            .unwrap_or(0.0);
        if pc.side == PortSide::WEST {
            -g.port_size(port).x - offset
        } else {
            node_width + offset
        }
    } else if pc.side == PortSide::WEST {
        -g.port_size(port).x
    } else {
        node_width
    }
}

/// If we switch from distributed or
/// justified alignment back to centered alignment, this may require us to
/// modify the required port placement size calculated in a previous phase.
fn modified_port_placement_size<G: AdapterGraph>(
    node_context: &NodeContext<G>,
    old_port_alignment: PortAlignment,
    current_port_placement_size: f64,
) -> f64 {
    if old_port_alignment == PortAlignment::DISTRIBUTED {
        current_port_placement_size - 2.0 * node_context.port_port_spacing
    } else {
        current_port_placement_size
    }
}
