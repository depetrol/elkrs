//! Splits edges spanning more than one layer by
//! inserting LONG_EDGE dummy nodes.

use crate::core::options::{EdgeLabelPlacement, PortConstraints, PortSide};

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    if a.graph(graph).layers.len() <= 2 {
        return Ok(());
    }

    let layers = a.graph(graph).layers.clone();
    for i in 0..layers.len() - 1 {
        let layer = layers[i];
        let next_layer = layers[i + 1];

        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            let ports = a.node(node).ports.clone();
            for port in ports {
                let outgoing = a.port(port).outgoing_edges.clone();
                for edge in outgoing {
                    let target_layer = a.node(a.edge_target_node(edge)).layer;
                    if target_layer != Some(layer) && target_layer != Some(next_layer) {
                        let dummy = create_dummy_node(a, graph, next_layer, edge);
                        split_edge(a, edge, dummy);
                    }
                }
            }
        }
    }
    Ok(())
}

fn create_dummy_node(
    a: &mut LGraphArena,
    graph: LGraphId,
    target_layer: LayerId,
    edge_to_split: LEdgeId,
) -> LNodeId {
    let dummy = a.create_node(graph);
    // the LNode(graph) constructor doesn't add to layerless nodes
    a.node_mut(dummy).graph = Some(graph);
    a.node_mut(dummy).node_type = NodeType::LONG_EDGE;
    a.node(dummy)
        .properties
        .set(&iprops::ORIGIN, Origin::LEdge(edge_to_split));
    a.node(dummy)
        .properties
        .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
    a.node_set_layer(dummy, Some(target_layer));
    dummy
}

/// The static `LongEdgeSplitter.splitEdge` (also used by other
/// processors).
pub fn split_edge(a: &mut LGraphArena, edge: LEdgeId, dummy_node: LNodeId) -> LEdgeId {
    let old_edge_target = a.edge(edge).target;

    let mut thickness: f64 = a.edge(edge).properties.get(&lopts::EDGE_THICKNESS);
    if thickness < 0.0 {
        thickness = 0.0;
        a.edge(edge).properties.set(&lopts::EDGE_THICKNESS, thickness);
    }
    a.node_mut(dummy_node).size.y = thickness;
    let port_pos = (thickness / 2.0).floor();

    let dummy_input = a.create_port();
    a.port_set_side(dummy_input, PortSide::WEST);
    a.port_set_node(dummy_input, Some(dummy_node));
    a.port_mut(dummy_input).pos.y = port_pos;

    let dummy_output = a.create_port();
    a.port_set_side(dummy_output, PortSide::EAST);
    a.port_set_node(dummy_output, Some(dummy_node));
    a.port_mut(dummy_output).pos.y = port_pos;

    a.edge_set_target(edge, Some(dummy_input));

    let dummy_edge = a.create_edge();
    let edge_props = a.edge(edge).properties.clone();
    a.edge_mut(dummy_edge).properties.copy_from(&edge_props);
    a.edge(dummy_edge).properties.unset(&lopts::JUNCTION_POINTS);
    a.edge_set_source(dummy_edge, Some(dummy_output));
    a.edge_set_target(dummy_edge, old_edge_target);

    set_dummy_node_properties(a, dummy_node, edge, dummy_edge);
    move_head_labels(a, edge, dummy_edge);

    dummy_edge
}

fn set_dummy_node_properties(
    a: &mut LGraphArena,
    dummy_node: LNodeId,
    in_edge: LEdgeId,
    out_edge: LEdgeId,
) {
    let in_edge_source_node = a.edge_source_node(in_edge);
    let out_edge_target_node = a.edge_target_node(out_edge);

    let in_type = a.node(in_edge_source_node).node_type;
    let out_type = a.node(out_edge_target_node).node_type;

    if in_type == NodeType::LONG_EDGE {
        copy_long_edge_props(a, in_edge_source_node, dummy_node, None);
    } else if in_type == NodeType::LABEL {
        copy_long_edge_props(a, in_edge_source_node, dummy_node, Some(true));
    } else if out_type == NodeType::LABEL {
        copy_long_edge_props(a, out_edge_target_node, dummy_node, Some(true));
    } else {
        let source = a.edge(in_edge).source.unwrap();
        let target = a.edge(out_edge).target.unwrap();
        a.node(dummy_node).properties.set(&iprops::LONG_EDGE_SOURCE, source);
        a.node(dummy_node).properties.set(&iprops::LONG_EDGE_TARGET, target);
    }
}

fn copy_long_edge_props(
    a: &mut LGraphArena,
    from: LNodeId,
    to: LNodeId,
    has_label_dummies: Option<bool>,
) {
    if let Some(s) = a.node(from).properties.try_get(&iprops::LONG_EDGE_SOURCE) {
        a.node(to).properties.set(&iprops::LONG_EDGE_SOURCE, s);
    } else {
        a.node(to).properties.unset(&iprops::LONG_EDGE_SOURCE);
    }
    if let Some(t) = a.node(from).properties.try_get(&iprops::LONG_EDGE_TARGET) {
        a.node(to).properties.set(&iprops::LONG_EDGE_TARGET, t);
    } else {
        a.node(to).properties.unset(&iprops::LONG_EDGE_TARGET);
    }
    let v = match has_label_dummies {
        Some(v) => v,
        None => a
            .node(from)
            .properties
            .get(&iprops::LONG_EDGE_HAS_LABEL_DUMMIES),
    };
    a.node(to)
        .properties
        .set(&iprops::LONG_EDGE_HAS_LABEL_DUMMIES, v);
}

fn move_head_labels(a: &mut LGraphArena, old_edge: LEdgeId, new_edge: LEdgeId) {
    let labels = a.edge(old_edge).labels.clone();
    for label in labels {
        let placement: EdgeLabelPlacement =
            a.label(label).properties.get(&lopts::EDGE_LABELS_PLACEMENT);
        if placement == EdgeLabelPlacement::HEAD {
            a.edge_mut(old_edge).labels.retain(|&l| l != label);
            a.edge_mut(new_edge).labels.push(label);
            if !a.label(label).properties.has(&iprops::END_LABEL_EDGE) {
                a.label(label).properties.set(&iprops::END_LABEL_EDGE, old_edge);
            }
        }
    }
}
