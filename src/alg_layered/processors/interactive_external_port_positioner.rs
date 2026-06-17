//!
//! Interactive layout relies on previously specified positions to determine a
//! layout of the graph. For dummy nodes such as external port dummies no
//! positions can be specified up front. This processor assigns reasonable
//! positions to such dummy nodes.

use crate::graph::properties::EnumSet;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::{GraphProperties, InLayerConstraint, LayerConstraint};

/// An arbitrarily chosen spacing value to separate external port dummies from
/// other nodes.
const ARBITRARY_SPACING: f64 = 10.0;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // if the graph does not contain any external ports ...
    let graph_properties: EnumSet<GraphProperties> =
        a.graph(graph).properties.get(&iprops::GRAPH_PROPERTIES);
    if !graph_properties.contains(GraphProperties::EXTERNAL_PORTS) {
        // ... nothing we can do about it
        return Ok(());
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    let nodes = a.graph(graph).layerless_nodes.clone();

    // find the minimum and maximum x coordinates of the graph
    for &node in &nodes {
        if a.node(node).node_type == NodeType::NORMAL {
            let margins = a.node(node).margin;
            let pos = a.node(node).pos;
            let size = a.node(node).size;
            min_x = min_x.min(pos.x - margins.left);
            max_x = max_x.max(pos.x + size.x + margins.right);
            min_y = min_y.min(pos.y - margins.top);
            max_y = max_y.max(pos.y + size.y + margins.bottom);
        }
    }

    // assign reasonable coordinates to external port dummies
    for &node in &nodes {
        if a.node(node).node_type != NodeType::NORMAL {
            match a.node(node).node_type {
                NodeType::EXTERNAL_PORT => {
                    let lc: LayerConstraint =
                        a.node(node).properties.get(&lopts::LAYERING_LAYER_CONSTRAINT);
                    if lc == LayerConstraint::FIRST_SEPARATE {
                        // it's a WEST port
                        a.node_mut(node).pos.x = min_x - ARBITRARY_SPACING;
                        if let Some(d) = find_y_coordinate(a, node, true) {
                            a.node_mut(node).pos.y = d;
                        }
                        continue;
                    }

                    if lc == LayerConstraint::LAST_SEPARATE {
                        // it's a EAST port
                        a.node_mut(node).pos.x = max_x + ARBITRARY_SPACING;
                        if let Some(d) = find_y_coordinate(a, node, false) {
                            a.node_mut(node).pos.y = d;
                        }
                        continue;
                    }

                    let ilc: InLayerConstraint =
                        a.node(node).properties.get(&iprops::IN_LAYER_CONSTRAINT);
                    if ilc == InLayerConstraint::TOP {
                        if let Some(x) = find_north_south_port_x_coordinate(a, node)? {
                            a.node_mut(node).pos.x = x + ARBITRARY_SPACING;
                        }
                        a.node_mut(node).pos.y = min_y - ARBITRARY_SPACING;
                        continue;
                    }

                    if ilc == InLayerConstraint::BOTTOM {
                        if let Some(x) = find_north_south_port_x_coordinate(a, node)? {
                            a.node_mut(node).pos.x = x + ARBITRARY_SPACING;
                        }
                        a.node_mut(node).pos.y = max_y + ARBITRARY_SPACING;
                        continue;
                    }
                }
                other => {
                    return Err(format!(
                        "The node type {other:?} is not supported by the \
                         InteractiveExternalPortPositioner"
                    ));
                }
            }
        }
    }

    Ok(())
}

/// `funGetOtherNode`: when `target_node` is true, returns `e.getTarget().getNode()`
/// (WEST case), otherwise `e.getSource().getNode()` (EAST case).
fn find_y_coordinate(a: &LGraphArena, dummy: LNodeId, target_node: bool) -> Option<f64> {
    for e in a.node_connected_edges(dummy) {
        let other = if target_node {
            a.edge_target_node(e)
        } else {
            a.edge_source_node(e)
        };
        return Some(a.node(other).pos.y + a.node(other).size.y / 2.0);
    }
    None
}

fn find_north_south_port_x_coordinate(
    a: &LGraphArena,
    dummy: LNodeId,
) -> Result<Option<f64>, String> {
    // external port dummies must have exactly one port
    debug_assert_eq!(a.node(dummy).ports.len(), 1);

    let port = a.node(dummy).ports[0];

    let has_out = !a.port(port).outgoing_edges.is_empty();
    let has_in = !a.port(port).incoming_edges.is_empty();

    if has_out && has_in {
        return Err("Interactive layout does not support NORTH/SOUTH ports with \
                    incoming _and_ outgoing edges."
            .to_string());
    }

    if has_out {
        // find the minimum position
        let mut min = f64::INFINITY;
        for e in a.port(port).outgoing_edges.clone() {
            let n = a.edge_target_node(e);
            let margins = a.node(n).margin;
            min = min.min(a.node(n).pos.x - margins.left);
        }
        return Ok(Some(min));
    }

    if has_in {
        // find the maximum value
        let mut max = f64::NEG_INFINITY;
        for e in a.port(port).incoming_edges.clone() {
            let n = a.edge_source_node(e);
            let margins = a.node(n).margin;
            max = max.max(a.node(n).pos.x + a.node(n).size.x + margins.right);
        }
        return Ok(Some(max));
    }

    // we should never reach here
    Ok(None)
}
