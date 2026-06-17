
use crate::core::adapters::AdapterGraph;
use crate::core::options::{PortLabelPlacement, PortSide};

use crate::alg_common::nodespacing::internal::{NodeContext, PortContext};

/// Creates port context
/// objects and assigns volatile IDs to all ports. Also, unless port labels
/// are fixed, the labels are added to the port context label cells.
pub fn create_port_contexts<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    ignore_inside_port_labels: bool,
) {
    // Hue hue hue... gimme color?
    let im_port_labels = !ignore_inside_port_labels
        || !node_context
            .port_labels_placement
            .contains(PortLabelPlacement::INSIDE);

    let mut volatile_id = 0usize;
    for port in g.node_ports(node_context.node) {
        if g.port_side(port) == PortSide::UNDEFINED {
            panic!(
                "Label and node size calculator can only be used with ports that have port \
                 sides assigned."
            );
        }

        create_port_context(g, node_context, port, volatile_id, im_port_labels);
        volatile_id += 1;
    }

    // Sort the contexts into their iteration order.
    node_context.sort_port_contexts();
}

fn create_port_context<G: AdapterGraph>(
    g: &G,
    node_context: &mut NodeContext<G>,
    port: G::P,
    volatile_id: usize,
    im_port_labels: bool,
) {
    let mut port_context = PortContext::new(
        g,
        node_context.port_labels_placement,
        node_context.treat_as_compound_node,
        port,
        volatile_id,
    );

    // If the port has labels and if port labels are to be placed, we need to
    // remember them
    if im_port_labels && !PortLabelPlacement::is_fixed(node_context.port_labels_placement) {
        let label_cell = node_context
            .cells
            .new_label(node_context.label_label_spacing, true);
        for label in g.port_labels(port) {
            node_context
                .cells
                .label_add_label(label_cell, label, g.label_size(label));
        }
        port_context.port_label_cell = Some(label_cell);
    }

    node_context.port_contexts.push(port_context);
}
