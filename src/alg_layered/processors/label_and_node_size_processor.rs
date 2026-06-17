
use crate::core::options::{PortLabelPlacement, PortSide};
use crate::graph::properties::EnumSet;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_adapters::LGraphAdapter;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::GraphProperties;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    {
        let mut adapter = LGraphAdapter::new(a, graph, true, true, |arena, n| {
            arena.node(n).node_type == NodeType::NORMAL
        });
        crate::alg_common::nodespacing::calculate_label_and_node_sizes(&mut adapter, |_, _| true);
    }

    let graph_properties: EnumSet<GraphProperties> =
        a.graph(graph).properties.get(&iprops::GRAPH_PROPERTIES);
    if graph_properties.contains(GraphProperties::EXTERNAL_PORTS) {
        let port_label_placement: EnumSet<PortLabelPlacement> =
            a.graph(graph).properties.get(&lopts::PORT_LABELS_PLACEMENT);
        let place_next_to_port =
            port_label_placement.contains(PortLabelPlacement::NEXT_TO_PORT_IF_POSSIBLE);
        let treat_as_group: bool = a.graph(graph).properties.get(&lopts::PORT_LABELS_TREAT_AS_GROUP);

        let layers = a.graph(graph).layers.clone();
        let mut dummies: Vec<LNodeId> = Vec::new();
        for layer in &layers {
            for &node in &a.layer(*layer).nodes {
                if a.node(node).node_type == NodeType::EXTERNAL_PORT {
                    dummies.push(node);
                }
            }
        }
        // Dummies of east/west ports live in the layerless list before layering;
        // but at this processor's slot they are in layers. Also handle any in the
        // layerless list (defensive).
        for &node in &a.graph(graph).layerless_nodes.clone() {
            if a.node(node).node_type == NodeType::EXTERNAL_PORT && !dummies.contains(&node) {
                dummies.push(node);
            }
        }
        for dummy in dummies {
            place_external_port_dummy_labels(
                a,
                dummy,
                port_label_placement,
                place_next_to_port,
                treat_as_group,
            );
        }
    }
    Ok(())
}

fn place_external_port_dummy_labels(
    a: &mut LGraphArena,
    dummy: LNodeId,
    graph_port_label_placement: EnumSet<PortLabelPlacement>,
    place_next_to_port_if_possible: bool,
    treat_as_group: bool,
) {
    let label_port_spacing_h: f64 = a.node(dummy).properties.get(&lopts::SPACING_LABEL_PORT_HORIZONTAL);
    let label_port_spacing_v: f64 = a.node(dummy).properties.get(&lopts::SPACING_LABEL_PORT_VERTICAL);
    let label_label_spacing: f64 = a.node(dummy).properties.get(&lopts::SPACING_LABEL_LABEL);

    let dummy_size = a.node(dummy).size;
    let dummy_port = a.node(dummy).ports[0];
    let dummy_port_pos = a.port(dummy_port).pos;

    // compute label box
    let labels = a.port(dummy_port).labels.clone();
    if labels.is_empty() {
        return;
    }
    let mut box_w = 0.0f64;
    let mut box_h = 0.0f64;
    for &label in &labels {
        let lsize = a.label(label).size;
        box_w = box_w.max(lsize.x);
        box_h += lsize.y;
    }
    box_h += (labels.len() as f64 - 1.0) * label_label_spacing;

    let box_x;
    let mut box_y = 0.0f64;
    let ext_port_side: PortSide = a.node(dummy).properties.get(&iprops::EXT_PORT_SIDE);

    if graph_port_label_placement.contains(PortLabelPlacement::INSIDE) {
        match ext_port_side {
            PortSide::NORTH => {
                box_x = (dummy_size.x - box_w) / 2.0 - dummy_port_pos.x;
                box_y = label_port_spacing_v;
            }
            PortSide::SOUTH => {
                box_x = (dummy_size.x - box_w) / 2.0 - dummy_port_pos.x;
                box_y = -label_port_spacing_v - box_h;
            }
            PortSide::EAST => {
                if label_next_to_port(a, dummy_port, true, place_next_to_port_if_possible) {
                    let label_height = if treat_as_group {
                        box_h
                    } else {
                        a.label(labels[0]).size.y
                    };
                    box_y = (dummy_size.y - label_height) / 2.0 - dummy_port_pos.y;
                } else {
                    box_y = dummy_size.y + label_port_spacing_v - dummy_port_pos.y;
                }
                box_x = -label_port_spacing_h - box_w;
            }
            PortSide::WEST => {
                if label_next_to_port(a, dummy_port, true, place_next_to_port_if_possible) {
                    let label_height = if treat_as_group {
                        box_h
                    } else {
                        a.label(labels[0]).size.y
                    };
                    box_y = (dummy_size.y - label_height) / 2.0 - dummy_port_pos.y;
                } else {
                    box_y = dummy_size.y + label_port_spacing_v - dummy_port_pos.y;
                }
                box_x = label_port_spacing_h;
            }
            PortSide::UNDEFINED => {
                box_x = 0.0;
            }
        }
    } else if graph_port_label_placement.contains(PortLabelPlacement::OUTSIDE) {
        match ext_port_side {
            PortSide::NORTH | PortSide::SOUTH => {
                box_x = dummy_port_pos.x + label_port_spacing_h;
            }
            PortSide::EAST | PortSide::WEST => {
                if label_next_to_port(a, dummy_port, false, place_next_to_port_if_possible) {
                    let label_height = if treat_as_group {
                        box_h
                    } else {
                        a.label(labels[0]).size.y
                    };
                    box_y = (dummy_size.y - label_height) / 2.0 - dummy_port_pos.y;
                } else {
                    box_y = dummy_port_pos.y + label_port_spacing_v;
                }
                box_x = 0.0;
            }
            PortSide::UNDEFINED => {
                box_x = 0.0;
            }
        }
    } else {
        box_x = 0.0;
    }

    // place the labels
    let mut current_y = box_y;
    for &label in &labels {
        a.label_mut(label).pos.x = box_x;
        a.label_mut(label).pos.y = current_y;
        current_y += a.label(label).size.y + label_label_spacing;
    }
}

fn label_next_to_port(
    a: &LGraphArena,
    dummy_port: crate::alg_layered::graph::LPortId,
    inside_labels: bool,
    place_next_to_port_if_possible: bool,
) -> bool {
    if !place_next_to_port_if_possible {
        false
    } else if inside_labels {
        a.port(dummy_port).incoming_edges.is_empty() && a.port(dummy_port).outgoing_edges.is_empty()
    } else {
        !a.port(dummy_port).connected_to_external_nodes
    }
}
