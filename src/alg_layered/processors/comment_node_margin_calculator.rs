//! Computes and sets the node margins
//! required to place comment boxes.

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::spacings;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // Iterate through the layers to additionally handle comments
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            process_comments(a, node);
        }
    }
    Ok(())
}

/// `processComments`: make some extra space for comment boxes that are
/// placed near the given node.
fn process_comments(a: &mut LGraphArena, node: LNodeId) {
    let top_boxes: Option<Vec<LNodeId>> = a.node(node).properties.try_get(&iprops::TOP_COMMENTS);
    let bottom_boxes: Option<Vec<LNodeId>> =
        a.node(node).properties.try_get(&iprops::BOTTOM_COMMENTS);

    if top_boxes.is_none() && bottom_boxes.is_none() {
        // Shortcut if there are no attached comments
        return;
    }

    // Retrieve the spacings that apply to this node
    let comment_comment_spacing =
        spacings::get_individual_or_default(a, node, &lopts::SPACING_COMMENT_COMMENT).unwrap();
    let comment_node_spacing =
        spacings::get_individual_or_default(a, node, &lopts::SPACING_COMMENT_NODE).unwrap();

    let mut margin = a.node(node).margin;

    // Consider comment boxes that are put on top of the node
    let mut top_width = 0.0f64;
    if let Some(top_boxes) = &top_boxes {
        let mut max_height = 0.0f64;
        for &comment_box in top_boxes {
            max_height = max_height.max(a.node(comment_box).size.y);
            top_width += a.node(comment_box).size.x;
        }
        top_width += comment_comment_spacing * (top_boxes.len() as f64 - 1.0);
        margin.top += max_height + comment_node_spacing;
    }

    // Consider comment boxes that are put in the bottom of the node
    let mut bottom_width = 0.0f64;
    if let Some(bottom_boxes) = &bottom_boxes {
        let mut max_height = 0.0f64;
        for &comment_box in bottom_boxes {
            max_height = max_height.max(a.node(comment_box).size.y);
            bottom_width += a.node(comment_box).size.x;
        }
        bottom_width += comment_comment_spacing * (bottom_boxes.len() as f64 - 1.0);
        margin.bottom += max_height + comment_node_spacing;
    }

    // Check if the maximum width of the comments is wider than the node itself, which
    // the comments are centered on
    let max_comment_width = top_width.max(bottom_width);
    if max_comment_width > a.node(node).size.x {
        let protrusion = (max_comment_width - a.node(node).size.x) / 2.0;
        margin.left = margin.left.max(protrusion);
        margin.right = margin.right.max(protrusion);
    }

    a.node_mut(node).margin = margin;
}
