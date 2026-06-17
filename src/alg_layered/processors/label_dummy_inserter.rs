//! Inserts dummy nodes into edges that have
//! center labels to reserve space for them.

use crate::core::options::{Direction, EdgeLabelPlacement, PortConstraints};

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LLabelId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::processors::long_edge_splitter;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // We cannot add the nodes to the graph while we're iterating over it, so
    // remember the dummy nodes we create
    let mut new_dummy_nodes: Vec<LNodeId> = Vec::new();

    let edge_label_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_LABEL);
    let label_label_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_LABEL_LABEL);
    let layout_direction: Direction = a.graph(graph).properties.get(&lopts::DIRECTION);

    let nodes = a.graph(graph).layerless_nodes.clone();
    for node in nodes {
        for edge in a.node_outgoing_edges(node) {
            if edge_needs_to_be_processed(a, edge) {
                let thickness = retrieve_thickness(a, edge);

                // Create dummy node and remember represented labels (filled below)
                let mut represented_labels: Vec<LLabelId> = Vec::new();
                let dummy_node = create_label_dummy(a, graph, edge, thickness);
                new_dummy_nodes.push(dummy_node);

                // Determine the size of the dummy node and move labels over to it
                let mut dummy_size = a.node(dummy_node).size;

                let labels = a.edge(edge).labels.clone();
                let mut remaining_labels: Vec<LLabelId> = Vec::new();
                for label in labels {
                    if a.label(label)
                        .properties
                        .get::<EdgeLabelPlacement>(&lopts::EDGE_LABELS_PLACEMENT)
                        == EdgeLabelPlacement::CENTER
                    {
                        // The way we stack labels depends on the layout direction
                        let label_size = a.label(label).size;
                        if layout_direction.is_vertical() {
                            dummy_size.x += label_size.x + label_label_spacing;
                            dummy_size.y = dummy_size.y.max(label_size.y);
                        } else {
                            dummy_size.x = dummy_size.x.max(label_size.x);
                            dummy_size.y += label_size.y + label_label_spacing;
                        }

                        // Move the label over to the dummy node's REPRESENTED_LABELS
                        represented_labels.push(label);
                    } else {
                        remaining_labels.push(label);
                    }
                }
                a.edge_mut(edge).labels = remaining_labels;

                // The dummy node now contains a superfluous label-label spacing and
                // does not include the edge-label spacing yet
                if layout_direction.is_vertical() {
                    dummy_size.x -= label_label_spacing;
                    dummy_size.y += edge_label_spacing + thickness;
                } else {
                    dummy_size.y += edge_label_spacing - label_label_spacing + thickness;
                }

                a.node_mut(dummy_node).size = dummy_size;
                a.node(dummy_node)
                    .properties
                    .set(&iprops::REPRESENTED_LABELS, represented_labels);
            }
        }
    }

    // Add created dummies to graph
    a.graph_mut(graph).layerless_nodes.extend(new_dummy_nodes);
    Ok(())
}

/// `edgeNeedsToBeProcessed`: not a self-loop and has center edge labels.
fn edge_needs_to_be_processed(a: &LGraphArena, edge: LEdgeId) -> bool {
    a.edge_source_node(edge) != a.edge_target_node(edge)
        && a.edge(edge).labels.iter().any(|&label| {
            a.label(label)
                .properties
                .get::<EdgeLabelPlacement>(&lopts::EDGE_LABELS_PLACEMENT)
                == EdgeLabelPlacement::CENTER
        })
}

/// `retrieveThickness`: the edge's thickness; negative values are
/// replaced by zero (and set on the edge).
fn retrieve_thickness(a: &mut LGraphArena, edge: LEdgeId) -> f64 {
    let mut thickness: f64 = a.edge(edge).properties.get(&lopts::EDGE_THICKNESS);
    if thickness < 0.0 {
        thickness = 0.0;
        a.edge(edge).properties.set(&lopts::EDGE_THICKNESS, thickness);
    }
    thickness
}

/// `createLabelDummy`. The `REPRESENTED_LABELS` property is set by the
/// caller once the represented labels have been collected.
fn create_label_dummy(
    a: &mut LGraphArena,
    graph: LGraphId,
    edge: LEdgeId,
    thickness: f64,
) -> LNodeId {
    let dummy_node = a.create_node(graph);
    a.node_mut(dummy_node).node_type = NodeType::LABEL;

    a.node(dummy_node)
        .properties
        .set(&iprops::ORIGIN, Origin::LEdge(edge));
    a.node(dummy_node)
        .properties
        .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
    let source_port = a.edge(edge).source.unwrap();
    let target_port = a.edge(edge).target.unwrap();
    a.node(dummy_node)
        .properties
        .set(&iprops::LONG_EDGE_SOURCE, source_port);
    a.node(dummy_node)
        .properties
        .set(&iprops::LONG_EDGE_TARGET, target_port);

    // Actually split the edge
    long_edge_splitter::split_edge(a, edge, dummy_node);

    // Place ports at the edge's center
    let port_pos = (thickness / 2.0).floor();
    for port in a.node(dummy_node).ports.clone() {
        a.port_mut(port).pos.y = port_pos;
    }

    dummy_node
}

pub fn is_inline_edge_label(a: &LGraphArena, node: LNodeId) -> bool {
    a.node(node).node_type == NodeType::LABEL
        && a.node(node)
            .properties
            .try_get(&iprops::REPRESENTED_LABELS)
            .unwrap_or_default()
            .iter()
            .all(|&label| a.label(label).properties.get(&lopts::EDGE_LABELS_INLINE))
}
