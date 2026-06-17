//! Removes the inserted center label dummies
//! and places the labels on their position.

use crate::core::adapters::LabelSide;
use crate::core::options::{Direction, EdgeRouting};
use crate::graph::math::KVector;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LLabelId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::processors::label_dummy_inserter::is_inline_edge_label;
use crate::alg_layered::processors::long_edge_joiner;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let edge_label_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_LABEL);
    let label_label_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_LABEL_LABEL);
    let layout_direction: Direction = a.graph(graph).properties.get(&lopts::DIRECTION);

    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        // An iterator is necessary for traversing nodes, since dummy nodes
        // might be removed
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            if a.node(node).node_type == NodeType::LABEL {
                // First, place labels on position of dummy node
                let origin_edge = match a.node(node).properties.try_get(&iprops::ORIGIN) {
                    Some(Origin::LEdge(e)) => e,
                    other => {
                        return Err(format!(
                            "label dummy without LEdge origin: {other:?}"
                        ))
                    }
                };
                let thickness: f64 =
                    a.edge(origin_edge).properties.get(&lopts::EDGE_THICKNESS);
                let labels_below_edge = a
                    .node(node)
                    .properties
                    .get::<LabelSide>(&iprops::LABEL_SIDE)
                    == LabelSide::BELOW;

                let mut curr_label_pos = a.node(node).pos;

                // If the labels are to be placed below their edge, we need to move the
                // first label's position down a bit to respect the label spacing
                if labels_below_edge {
                    curr_label_pos.y += thickness + edge_label_spacing;
                }

                // Calculate the space available for the placement of labels
                let node_size = a.node(node).size;
                let label_space = KVector::new(
                    node_size.x,
                    node_size.y
                        + (if is_inline_edge_label(a, node) {
                            0.0
                        } else {
                            -thickness - edge_label_spacing
                        }),
                );

                // Place labels
                let represented_labels: Vec<LLabelId> = a
                    .node(node)
                    .properties
                    .try_get(&iprops::REPRESENTED_LABELS)
                    .unwrap_or_default();

                if layout_direction.is_vertical() {
                    place_labels_for_vertical_layout(
                        a,
                        &represented_labels,
                        curr_label_pos,
                        label_label_spacing,
                        label_space,
                        labels_below_edge,
                        layout_direction,
                    );
                } else {
                    place_labels_for_horizontal_layout(
                        a,
                        &represented_labels,
                        curr_label_pos,
                        label_label_spacing,
                        label_space,
                    );
                }

                // Add represented labels back to the original edge
                a.edge_mut(origin_edge).labels.extend(represented_labels);

                // Whether we need to add unnecessary bend points around the label dummy
                // depends on the edge router. For orthogonal edge routing, they are not
                // necessary. For splines, they may even be harmful. For polylines, they
                // are necessary to keep the routes edges take close to their labels
                let polyline = a
                    .graph(graph)
                    .properties
                    .get::<EdgeRouting>(&lopts::EDGE_ROUTING)
                    == EdgeRouting::POLYLINE;
                long_edge_joiner::join_at(a, node, polyline);

                // Remove the node
                a.layer_mut(layer).nodes.retain(|&n| n != node);
                a.node_mut(node).layer = None;
            }
        }
    }
    Ok(())
}

/// `placeLabelsForHorizontalLayout`.
fn place_labels_for_horizontal_layout(
    a: &mut LGraphArena,
    labels: &[LLabelId],
    mut label_pos: KVector,
    label_spacing: f64,
    label_space: KVector,
) {
    for &label in labels {
        let label_size = a.label(label).size;
        a.label_mut(label).pos.x = label_pos.x + (label_space.x - label_size.x) / 2.0;
        a.label_mut(label).pos.y = label_pos.y;

        label_pos.y += label_size.y + label_spacing;
    }
}

/// `placeLabelsForVerticalLayout`.
fn place_labels_for_vertical_layout(
    a: &mut LGraphArena,
    labels: &[LLabelId],
    mut label_pos: KVector,
    label_spacing: f64,
    label_space: KVector,
    left_aligned: bool,
    layout_direction: Direction,
) {
    // We may have to override the alignment if all labels here are inline labels
    let inline = labels
        .iter()
        .all(|&label| a.label(label).properties.get(&lopts::EDGE_LABELS_INLINE));

    // Due to the way layout directions work, we need to pay attention to the order in
    // which we place labels. While we can simply place them as they come for the DOWN
    // direction, doing the same for the UP direction would reverse the label order in
    // the final result; in that case we iterate over the reversed label list
    let mut effective_labels: Vec<LLabelId> = labels.to_vec();
    if layout_direction == Direction::UP {
        effective_labels.reverse();
    }

    for label in effective_labels {
        let label_size = a.label(label).size;
        a.label_mut(label).pos.x = label_pos.x;

        if inline {
            a.label_mut(label).pos.y = label_pos.y + (label_space.y - label_size.y) / 2.0;
        } else if left_aligned {
            a.label_mut(label).pos.y = label_pos.y;
        } else {
            a.label_mut(label).pos.y = label_pos.y + label_space.y - label_size.y;
        }

        label_pos.x += label_size.x + label_spacing;
    }
}
