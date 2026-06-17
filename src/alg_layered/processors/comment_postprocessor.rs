//! Reinserts comment boxes removed by the
//! `CommentPreprocessor` and places them above or below their corresponding
//! connected node.

use crate::graph::math::KVector;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::spacings;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let mut boxes: Vec<LNodeId> = Vec::new();
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            let top_boxes = a.node(node).properties.try_get(&iprops::TOP_COMMENTS);
            let bottom_boxes = a.node(node).properties.try_get(&iprops::BOTTOM_COMMENTS);

            if top_boxes.is_some() || bottom_boxes.is_some() {
                process_node(a, node, top_boxes.as_deref(), bottom_boxes.as_deref());

                if let Some(top_boxes) = top_boxes {
                    boxes.extend(top_boxes);
                }

                if let Some(bottom_boxes) = bottom_boxes {
                    boxes.extend(bottom_boxes);
                }
            }
        }

        a.layer_mut(layer).nodes.extend(boxes);
    }
    Ok(())
}

/// `process(LNode, List<LNode>, List<LNode>)`: process a node with its
/// connected comment boxes.
fn process_node(
    a: &mut LGraphArena,
    node: LNodeId,
    top_boxes: Option<&[LNodeId]>,
    bottom_boxes: Option<&[LNodeId]>,
) {
    let node_pos = a.node(node).pos;
    let node_size = a.node(node).size;
    let margin = a.node(node).margin;

    let comment_comment_spacing =
        spacings::get_individual_or_default(a, node, &lopts::SPACING_COMMENT_COMMENT).unwrap();

    if let Some(top_boxes) = top_boxes {
        // determine the total width and maximal height of the top boxes
        let mut boxes_width = comment_comment_spacing * (top_boxes.len() as f64 - 1.0);
        let mut max_height = 0.0f64;
        for &boxx in top_boxes {
            boxes_width += a.node(boxx).size.x;
            max_height = max_height.max(a.node(boxx).size.y);
        }

        // place the boxes on top of the node, horizontally centered around the node itself
        let mut x = node_pos.x - (boxes_width - node_size.x) / 2.0;
        let base_line = node_pos.y - margin.top + max_height;
        let anchor_inc = node_size.x / (top_boxes.len() as f64 + 1.0);
        let mut anchor_x = anchor_inc;
        for &boxx in top_boxes {
            let box_size = a.node(boxx).size;
            a.node_mut(boxx).pos.x = x;
            a.node_mut(boxx).pos.y = base_line - box_size.y;
            x += box_size.x + comment_comment_spacing;
            // set source and target point for the connecting edge
            let box_port = get_box_port(a, boxx).expect("comment box without connecting port");
            let box_port_anchor = a.port(box_port).anchor;
            a.port_mut(box_port).pos.x = box_size.x / 2.0 - box_port_anchor.x;
            a.port_mut(box_port).pos.y = box_size.y;
            let node_port: LPortId = a
                .node(boxx)
                .properties
                .try_get(&iprops::COMMENT_CONN_PORT)
                .expect("comment box without COMMENT_CONN_PORT");
            if a.port_degree(node_port) == 1 {
                let node_port_anchor = a.port(node_port).anchor;
                a.port_mut(node_port).pos = KVector::new(anchor_x - node_port_anchor.x, 0.0);
                a.port_set_node(node_port, Some(node));
            }
            anchor_x += anchor_inc;
        }
    }

    if let Some(bottom_boxes) = bottom_boxes {
        // determine the total width and maximal height of the bottom boxes
        let mut boxes_width = comment_comment_spacing * (bottom_boxes.len() as f64 - 1.0);
        let mut max_height = 0.0f64;
        for &boxx in bottom_boxes {
            boxes_width += a.node(boxx).size.x;
            max_height = max_height.max(a.node(boxx).size.y);
        }

        // place the boxes in the bottom of the node, horizontally centered around the node
        let mut x = node_pos.x - (boxes_width - node_size.x) / 2.0;
        let base_line = node_pos.y + node_size.y + margin.bottom - max_height;
        let anchor_inc = node_size.x / (bottom_boxes.len() as f64 + 1.0);
        let mut anchor_x = anchor_inc;
        for &boxx in bottom_boxes {
            let box_size = a.node(boxx).size;
            a.node_mut(boxx).pos.x = x;
            a.node_mut(boxx).pos.y = base_line;
            x += box_size.x + comment_comment_spacing;
            // set source and target point for the connecting edge
            let box_port = get_box_port(a, boxx).expect("comment box without connecting port");
            let box_port_anchor = a.port(box_port).anchor;
            a.port_mut(box_port).pos.x = box_size.x / 2.0 - box_port_anchor.x;
            a.port_mut(box_port).pos.y = 0.0;
            let node_port: LPortId = a
                .node(boxx)
                .properties
                .try_get(&iprops::COMMENT_CONN_PORT)
                .expect("comment box without COMMENT_CONN_PORT");
            if a.port_degree(node_port) == 1 {
                let node_port_anchor = a.port(node_port).anchor;
                a.port_mut(node_port).pos =
                    KVector::new(anchor_x - node_port_anchor.x, node_size.y);
                a.port_set_node(node_port, Some(node));
            }
            anchor_x += anchor_inc;
        }
    }
}

/// `getBoxPort`: retrieves the port of the given comment box that
/// connects it with the corresponding node, reconnecting the edge that the
/// pre-processor disconnected.
fn get_box_port(a: &mut LGraphArena, comment_box: LNodeId) -> Option<LPortId> {
    let node_port: LPortId = a
        .node(comment_box)
        .properties
        .try_get(&iprops::COMMENT_CONN_PORT)
        .expect("comment box without COMMENT_CONN_PORT");
    for port in a.node(comment_box).ports.clone() {
        if let Some(&edge) = a.port(port).outgoing_edges.first() {
            // reconnect the edge (has been disconnected by pre-processor)
            a.edge_set_target(edge, Some(node_port));
            return Some(port);
        }
        if let Some(&edge) = a.port(port).incoming_edges.first() {
            // reconnect the edge (has been disconnected by pre-processor)
            a.edge_set_source(edge, Some(node_port));
            return Some(port);
        }
    }
    None
}
