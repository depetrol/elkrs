//! Decides for each edge label whether to place
//! it above or below its respective edge.

use crate::core::adapters::LabelSide;
use crate::core::options::PortSide;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LLabelId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::EdgeLabelSideSelection;
use crate::alg_layered::processors::end_label_preprocessor;
use crate::alg_layered::processors::label_dummy_inserter::is_inline_edge_label;

/// `LabelSide.opposite()` (UNKNOWN maps to itself).
fn opposite(side: LabelSide) -> LabelSide {
    match side {
        LabelSide::ABOVE => LabelSide::BELOW,
        LabelSide::BELOW => LabelSide::ABOVE,
        LabelSide::INLINE => LabelSide::INLINE,
        LabelSide::UNKNOWN => LabelSide::UNKNOWN,
    }
}

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mode: EdgeLabelSideSelection = a
        .graph(graph)
        .properties
        .get(&lopts::EDGE_LABELS_SIDE_SELECTION);

    // Calculate all label sides depending on the given strategy
    match mode {
        EdgeLabelSideSelection::ALWAYS_UP => same_side(a, graph, LabelSide::ABOVE),
        EdgeLabelSideSelection::ALWAYS_DOWN => same_side(a, graph, LabelSide::BELOW),
        EdgeLabelSideSelection::DIRECTION_UP => based_on_direction(a, graph, LabelSide::ABOVE),
        EdgeLabelSideSelection::DIRECTION_DOWN => based_on_direction(a, graph, LabelSide::BELOW),
        EdgeLabelSideSelection::SMART_UP => smart(a, graph, LabelSide::ABOVE),
        EdgeLabelSideSelection::SMART_DOWN => smart(a, graph, LabelSide::BELOW),
    }
    Ok(())
}

//////////////////////////////////////////////////////////////////////////////
// Simple Placement Strategies

/// `sameSide`: configures all labels to be placed on the given side.
fn same_side(a: &mut LGraphArena, graph: LGraphId, label_side: LabelSide) {
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            if a.node(node).node_type == NodeType::LABEL {
                apply_label_side_to_node(a, node, label_side);
            }

            for edge in a.node_outgoing_edges(node) {
                apply_label_side_to_edge(a, edge, label_side);
            }
        }
    }
}

/// `basedOnDirection`: configures all labels to be placed according to
/// their edge's direction.
fn based_on_direction(a: &mut LGraphArena, graph: LGraphId, side_for_rightward_edges: LabelSide) {
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            if a.node(node).node_type == NodeType::LABEL {
                let side = if does_label_dummy_point_right(a, node) {
                    side_for_rightward_edges
                } else {
                    opposite(side_for_rightward_edges)
                };
                apply_label_side_to_node(a, node, side);
            }

            for edge in a.node_outgoing_edges(node) {
                let side = if does_edge_point_right(a, edge) {
                    side_for_rightward_edges
                } else {
                    opposite(side_for_rightward_edges)
                };
                apply_label_side_to_edge(a, edge, side);
            }
        }
    }
}

//////////////////////////////////////////////////////////////////////////////
// Smart Placement Strategy

/// `smart`: chooses label sides depending on certain patterns.
fn smart(a: &mut LGraphArena, graph: LGraphId, default_side: LabelSide) {
    // We will collect consecutive runs of certain dummy nodes while we iterate
    let mut dummy_node_queue: Vec<LNodeId> = Vec::new();

    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let mut top_group = true;
        let mut label_dummies_in_queue = 0;

        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            match a.node(node).node_type {
                NodeType::LABEL => {
                    label_dummies_in_queue += 1;
                    dummy_node_queue.push(node);
                }
                NodeType::LONG_EDGE => {
                    dummy_node_queue.push(node);
                }
                node_type => {
                    if node_type == NodeType::NORMAL {
                        smart_for_regular_node(a, node, default_side);
                    }

                    // Empty dummy node queue
                    if !dummy_node_queue.is_empty() {
                        smart_for_consecutive_dummy_node_run(
                            a,
                            &mut dummy_node_queue,
                            label_dummies_in_queue,
                            top_group,
                            false,
                            default_side,
                        );
                    }

                    // Reset things
                    top_group = false;
                    label_dummies_in_queue = 0;
                }
            }
        }

        // Do stuff with the nodes in the queue
        if !dummy_node_queue.is_empty() {
            smart_for_consecutive_dummy_node_run(
                a,
                &mut dummy_node_queue,
                label_dummies_in_queue,
                top_group,
                true,
                default_side,
            );
        }
    }
}

/// `smartForConsecutiveDummyNodeRun`: assigns label sides to all label
/// dummies in the given queue and empties the queue afterwards.
fn smart_for_consecutive_dummy_node_run(
    a: &mut LGraphArena,
    dummy_nodes: &mut Vec<LNodeId>,
    label_dummy_count: usize,
    top_group: bool,
    bottom_group: bool,
    default_side: LabelSide,
) {
    debug_assert!(!dummy_nodes.is_empty());

    // We distinguish a number of special cases whose rules seem rather complicated.
    if top_group
        && (!bottom_group || dummy_nodes.len() > 1)
        && label_dummy_count == 1
        && a.node(dummy_nodes[0]).node_type == NodeType::LABEL
    {
        // The current run of dummy nodes is at the top of the layer, has only a single
        // label dummy, and that label dummy is at the top of the run; select the ABOVE
        // side to ensure that its edge doesn't get too long
        apply_label_side_to_node(a, dummy_nodes[0], LabelSide::ABOVE);
    } else if bottom_group
        && (!top_group || dummy_nodes.len() > 1)
        && label_dummy_count == 1
        && a.node(*dummy_nodes.last().unwrap()).node_type == NodeType::LABEL
    {
        // Symmetric to the previous case, only at the bottom of the layer
        apply_label_side_to_node(a, *dummy_nodes.last().unwrap(), LabelSide::BELOW);
    } else if dummy_nodes.len() == 2 {
        // There's only a run of two edges, so place the label of the first above its
        // edge, and the label of the second below its edge
        apply_label_side_to_node(a, dummy_nodes[0], LabelSide::ABOVE);
        apply_label_side_to_node(a, dummy_nodes[1], LabelSide::BELOW);
    } else {
        // Not one of the special cases above. Iterate over the dummy nodes and assign
        // the default label side, except if we find two consecutive label dummies that
        // connect the same nodes (tight loops in control flow diagrams)
        apply_for_dummy_node_run_with_simple_loops(a, dummy_nodes, default_side);
    }

    // Ensure the list is cleared
    dummy_nodes.clear();
}

/// `applyForDummyNodeRunWithSimpleLoops`.
fn apply_for_dummy_node_run_with_simple_loops(
    a: &mut LGraphArena,
    dummy_nodes: &[LNodeId],
    default_side: LabelSide,
) {
    // We keep track of runs of consecutive label dummy nodes that connect the same nodes
    let mut label_dummy_run: Vec<LNodeId> = Vec::with_capacity(dummy_nodes.len());
    let mut prev_long_edge_source: Option<LNodeId> = None;
    let mut prev_long_edge_target: Option<LNodeId> = None;

    for &current_dummy in dummy_nodes {
        debug_assert!(matches!(
            a.node(current_dummy).node_type,
            NodeType::LABEL | NodeType::LONG_EDGE
        ));

        // Check if we are continuing a previous run
        let curr_long_edge_source = get_long_edge_end_node(a, current_dummy, true);
        let curr_long_edge_target = get_long_edge_end_node(a, current_dummy, false);

        if prev_long_edge_source != curr_long_edge_source
            || prev_long_edge_target != curr_long_edge_target
        {
            // We're starting a new run
            apply_label_sides_to_label_dummy_run(a, &mut label_dummy_run, default_side);

            prev_long_edge_source = curr_long_edge_source;
            prev_long_edge_target = curr_long_edge_target;
        }

        label_dummy_run.push(current_dummy);
    }

    // Assign label sides to whatever dummy nodes are left
    apply_label_sides_to_label_dummy_run(a, &mut label_dummy_run, default_side);
}

/// `getLongEdgeEndNode`: the long edge source or target node of the
/// given dummy. May be, but shouldn't be, `None`.
fn get_long_edge_end_node(a: &LGraphArena, label_dummy: LNodeId, source: bool) -> Option<LNodeId> {
    let end_port = if source {
        a.node(label_dummy).properties.try_get(&iprops::LONG_EDGE_SOURCE)
    } else {
        a.node(label_dummy).properties.try_get(&iprops::LONG_EDGE_TARGET)
    };

    end_port.and_then(|port| a.port(port).node)
}

/// `applyLabelSidesToLabelDummyRun`: applies label sides to the given
/// list of consecutive dummy nodes and empties that list afterwards.
fn apply_label_sides_to_label_dummy_run(
    a: &mut LGraphArena,
    label_dummy_run: &mut Vec<LNodeId>,
    default_side: LabelSide,
) {
    if !label_dummy_run.is_empty() {
        // If the list contains exactly two label dummies, we place labels differently
        if label_dummy_run.len() == 2 {
            apply_label_side_to_node(a, label_dummy_run[0], LabelSide::ABOVE);
            apply_label_side_to_node(a, label_dummy_run[1], LabelSide::BELOW);
        } else {
            for &dummy_node in label_dummy_run.iter() {
                apply_label_side_to_node(a, dummy_node, default_side);
            }
        }

        label_dummy_run.clear();
    }
}

/// `smartForRegularNode`: assigns label sides to all end labels incident
/// to this node, depending on how many ports there are on any given side.
fn smart_for_regular_node(a: &mut LGraphArena, node: LNodeId, default_side: LabelSide) {
    // Iterate over the node's list of ports on each side. Remember the ones that have
    // edges connected to them and decide based on how many such ports there are
    let mut end_label_queue: Vec<Vec<LLabelId>> = Vec::new();
    let mut current_port_side: Option<PortSide> = None;

    // This is where we assume that the list of ports is properly sorted
    let ports = a.node(node).ports.clone();
    for port in ports {
        let side = a.port(port).side;
        if Some(side) != current_port_side {
            if !end_label_queue.is_empty() {
                smart_for_regular_node_port_end_labels(
                    a,
                    &end_label_queue,
                    current_port_side.unwrap(),
                    default_side,
                );
            }

            end_label_queue.clear();
            current_port_side = Some(side);
        }

        // Possibly add the port's end labels to our queue, if it has any
        if let Some(port_end_labels) = end_label_preprocessor::gather_labels(a, port) {
            end_label_queue.push(port_end_labels);
        }
    }

    // Clear remaining ports
    if !end_label_queue.is_empty() {
        smart_for_regular_node_port_end_labels(
            a,
            &end_label_queue,
            current_port_side.unwrap(),
            default_side,
        );
    }
}

/// `smartForRegularNodePortEndLabels`.
fn smart_for_regular_node_port_end_labels(
    a: &mut LGraphArena,
    end_label_queue: &[Vec<LLabelId>],
    port_side: PortSide,
    default_side: LabelSide,
) {
    debug_assert!(!end_label_queue.is_empty());

    if end_label_queue.len() == 2 {
        // What we're going to do depends on which port side we are traversing...
        if port_side == PortSide::NORTH || port_side == PortSide::EAST {
            apply_label_side_to_labels(a, &end_label_queue[0], LabelSide::ABOVE);
            apply_label_side_to_labels(a, &end_label_queue[1], LabelSide::BELOW);
        } else {
            apply_label_side_to_labels(a, &end_label_queue[0], LabelSide::BELOW);
            apply_label_side_to_labels(a, &end_label_queue[1], LabelSide::ABOVE);
        }
    } else {
        for label_list in end_label_queue {
            apply_label_side_to_labels(a, label_list, default_side);
        }
    }
}

//////////////////////////////////////////////////////////////////////////////
// Helper Methods

/// `applyLabelSide(LNode, LabelSide)`: applies the given label side to
/// the given label dummy node, moving its ports if necessary.
fn apply_label_side_to_node(a: &mut LGraphArena, label_dummy: LNodeId, side: LabelSide) {
    // This method only does things to label dummy nodes
    if a.node(label_dummy).node_type == NodeType::LABEL {
        let effective_side = if is_inline_edge_label(a, label_dummy) {
            LabelSide::INLINE
        } else {
            side
        };

        a.node(label_dummy)
            .properties
            .set(&iprops::LABEL_SIDE, effective_side);

        // If the label is not below the edge, the ports need to be moved
        if effective_side != LabelSide::BELOW {
            let origin_edge = match a.node(label_dummy).properties.try_get(&iprops::ORIGIN) {
                Some(Origin::LEdge(e)) => e,
                _ => panic!("label dummy without LEdge origin"),
            };
            let thickness: f64 = a.edge(origin_edge).properties.get(&lopts::EDGE_THICKNESS);

            // The new port position depends on the new placement
            let mut port_pos = 0.0;
            if effective_side == LabelSide::ABOVE {
                port_pos = a.node(label_dummy).size.y - (thickness / 2.0).ceil();
            } else if effective_side == LabelSide::INLINE {
                // The label dummy has a superfluous label-edge spacing
                let graph = a.node_graph(label_dummy);
                let edge_label_spacing: f64 =
                    a.graph(graph).properties.get(&lopts::SPACING_EDGE_LABEL);
                port_pos = (a.node(label_dummy).size.y - edge_label_spacing - thickness).ceil()
                    / 2.0;
                a.node_mut(label_dummy).size.y -= edge_label_spacing;
                a.node_mut(label_dummy).size.y -= thickness;
            }

            for port in a.node(label_dummy).ports.clone() {
                a.port_mut(port).pos.y = port_pos;
            }
        }
    }
}

/// `applyLabelSide(LEdge, LabelSide)`: applies the given label side to
/// all labels of the given edge.
fn apply_label_side_to_edge(a: &mut LGraphArena, edge: LEdgeId, side: LabelSide) {
    for label in a.edge(edge).labels.clone() {
        a.label(label).properties.set(&iprops::LABEL_SIDE, side);
    }
}

/// `applyLabelSide(List<LLabel>, LabelSide)`.
fn apply_label_side_to_labels(a: &mut LGraphArena, labels: &[LLabelId], side: LabelSide) {
    for &label in labels {
        a.label(label).properties.set(&iprops::LABEL_SIDE, side);
    }
}

/// `doesEdgePointRight(LEdge)`.
fn does_edge_point_right(a: &LGraphArena, edge: LEdgeId) -> bool {
    !a.edge(edge).properties.get::<bool>(&iprops::REVERSED)
}

/// `doesEdgePointRight(LNode)`: checks if the given label dummy node is
/// part of an edge segment that will point right in the final drawing.
fn does_label_dummy_point_right(a: &LGraphArena, label_dummy: LNodeId) -> bool {
    debug_assert!(a.node(label_dummy).node_type == NodeType::LABEL);

    // Find incoming and outgoing edge
    let incoming = a.node_incoming_edges(label_dummy)[0];
    let outgoing = a.node_outgoing_edges(label_dummy)[0];

    does_edge_point_right(a, incoming) || does_edge_point_right(a, outgoing)
}
