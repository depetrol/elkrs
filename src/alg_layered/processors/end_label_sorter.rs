//! Sorts end labels according to the order of
//! nodes their respective edges come from or head to.

use crate::core::options::EdgeLabelPlacement;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LLabelId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::processors::end_label_preprocessor::LabelCell;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            if a.node(node).node_type == NodeType::NORMAL {
                process_node(a, graph, node);
            }
        }
    }
    Ok(())
}

//////////////////////////////////////////////////////////////////////////////
// Node Processing and Initialization

fn process_node(a: &mut LGraphArena, graph: LGraphId, node: LNodeId) {
    let mut initialize_method_called = false;

    if a.node(node).properties.has(&iprops::END_LABELS) {
        let mut label_cell_map = a
            .node(node)
            .properties
            .try_get(&iprops::END_LABELS)
            .unwrap();
        let mut modified = false;

        // Iterate over all ports and check for each port if it requires its labels
        // to be sorted
        for port in a.node(node).ports.clone() {
            if needs_sorting(a, port) {
                // Check if we need to initialize
                if !initialize_method_called {
                    initialize(a, graph);
                    initialize_method_called = true;
                }

                let cell = label_cell_map
                    .get_mut(port)
                    .expect("port needing sorting without a label cell");
                sort(a, cell);
                modified = true;
            }
        }

        if modified {
            a.node(node).properties.set(&iprops::END_LABELS, label_cell_map);
        }
    }
}

/// `needsSorting`: a port requires its end labels to be sorted if there
/// are end labels of at least two edges there.
fn needs_sorting(a: &LGraphArena, port: LPortId) -> bool {
    let mut edges_with_end_labels = 0;

    for &in_edge in &a.port(port).incoming_edges {
        let head_labels = a.edge(in_edge).labels.iter().any(|&label| {
            a.label(label)
                .properties
                .get::<EdgeLabelPlacement>(&lopts::EDGE_LABELS_PLACEMENT)
                == EdgeLabelPlacement::HEAD
        });
        if head_labels {
            edges_with_end_labels += 1;
        }
    }

    for &out_edge in &a.port(port).outgoing_edges {
        let tail_labels = a.edge(out_edge).labels.iter().any(|&label| {
            a.label(label)
                .properties
                .get::<EdgeLabelPlacement>(&lopts::EDGE_LABELS_PLACEMENT)
                == EdgeLabelPlacement::TAIL
        });
        if tail_labels {
            edges_with_end_labels += 1;
        }
    }

    edges_with_end_labels >= 2
}

/// `initialize`: gives nodes and ports ascending IDs.
fn initialize(a: &mut LGraphArena, graph: LGraphId) {
    let mut next_element_id = 0;
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            a.node_mut(node).id = next_element_id;
            next_element_id += 1;

            for port in a.node(node).ports.clone() {
                a.port_mut(port).id = next_element_id;
                next_element_id += 1;
            }
        }
    }
}

//////////////////////////////////////////////////////////////////////////////
// Sorting

/// `sort`: sorts the labels contained in the given label cell.
fn sort(a: &LGraphArena, port_label_cell: &mut LabelCell) {
    let mut label_groups = create_label_groups(a, port_label_cell);
    label_groups.sort_by(|group1, group2| compare_label_groups(a, group1, group2));

    // Re-add the label cell's labels in the proper order. We directly access the
    // cell's label list here, which won't cause the cell to recompute its size
    port_label_cell.labels.clear();
    for group in label_groups {
        port_label_cell.labels.extend(group.labels);
    }
}

/// `createLabelGroups`: groups labels from the same edge. The groups are
/// collected in insertion order — order of first label occurrence.
fn create_label_groups(a: &LGraphArena, port_label_cell: &LabelCell) -> Vec<LabelGroup> {
    let mut groups: Vec<LabelGroup> = Vec::new();

    // Make sure every label is contained in a label group
    for &label in &port_label_cell.labels {
        let edge = a
            .label(label)
            .properties
            .try_get(&iprops::END_LABEL_EDGE)
            .expect("end label without END_LABEL_EDGE");

        match groups.iter_mut().find(|group| group.edge == edge) {
            Some(group) => group.labels.push(label),
            None => groups.push(LabelGroup { edge, labels: vec![label] }),
        }
    }

    groups
}

/// `LabelGroup`: a group of labels belonging to a single edge.
struct LabelGroup {
    /// The edge the labels belong to. This can be a dummy edge if the original
    /// edge was broken by a label dummy.
    edge: LEdgeId,
    /// Labels that belong to this group, in their original order.
    labels: Vec<LLabelId>,
}

/// `LABEL_GROUP_COMPARATOR`.
fn compare_label_groups(
    a: &LGraphArena,
    group1: &LabelGroup,
    group2: &LabelGroup,
) -> std::cmp::Ordering {
    let source_id = |edge: LEdgeId| a.port(a.edge(edge).source.unwrap()).id;
    let target_node_id = |edge: LEdgeId| a.node(a.edge_target_node(edge)).id;
    let target_id = |edge: LEdgeId| a.port(a.edge(edge).target.unwrap()).id;

    // If they are not connected to the same source port, use the difference
    let source_port_diff = source_id(group1.edge).cmp(&source_id(group2.edge));
    if source_port_diff != std::cmp::Ordering::Equal {
        return source_port_diff;
    }

    // They are connected to the same source port. Sort by target node.
    let target_node_diff = target_node_id(group1.edge).cmp(&target_node_id(group2.edge));
    if target_node_diff != std::cmp::Ordering::Equal {
        return target_node_diff;
    }

    // They are connected to the same source port and to the same target node.
    // Compare target ports, but backwards: since western ports are ordered from
    // bottom to top, not sorting backwards would yield the opposite of our desires.
    target_id(group2.edge).cmp(&target_id(group1.edge))
}
