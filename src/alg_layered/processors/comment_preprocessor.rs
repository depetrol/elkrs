//! Removes comment boxes that have exactly
//! one connection to a normal node from the graph and stores them on the
//! connected node for later processing by the `CommentPostprocessor`.

use crate::core::options::{PortConstraints, PortSide};

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LPortId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_util;
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let nodes = a.graph(graph).layerless_nodes.clone();
    for node in nodes {
        if a.node(node).properties.get(&lopts::COMMENT_BOX) {
            let mut edge_count = 0;
            let mut edge: Option<LEdgeId> = None;
            let mut opposite_port: Option<LPortId> = None;
            for &port in &a.node(node).ports {
                edge_count += a.port_degree(port);
                if a.port(port).incoming_edges.len() == 1 {
                    let e = a.port(port).incoming_edges[0];
                    edge = Some(e);
                    opposite_port = a.edge(e).source;
                }
                if a.port(port).outgoing_edges.len() == 1 {
                    let e = a.port(port).outgoing_edges[0];
                    edge = Some(e);
                    opposite_port = a.edge(e).target;
                }
            }

            let single_connection = edge_count == 1 && {
                let opp = opposite_port.unwrap();
                a.port_degree(opp) == 1
                    && !a
                        .node(a.port(opp).node.unwrap())
                        .properties
                        .get::<bool>(&lopts::COMMENT_BOX)
            };

            if single_connection {
                // found a comment that has exactly one connection
                let opp = opposite_port.unwrap();
                let real_node = a.port(opp).node.unwrap();
                process_box(a, node, edge.unwrap(), opp, real_node);
                a.graph_mut(graph).layerless_nodes.retain(|&n| n != node);
            } else {
                // reverse edges that are oddly connected
                let mut rev_edges: Vec<LEdgeId> = Vec::new();
                for &port in &a.node(node).ports {
                    for &outedge in &a.port(port).outgoing_edges {
                        let target = a.edge(outedge).target.unwrap();
                        if !a.port(target).outgoing_edges.is_empty() {
                            rev_edges.push(outedge);
                        }
                    }

                    for &inedge in &a.port(port).incoming_edges {
                        let source = a.edge(inedge).source.unwrap();
                        if !a.port(source).incoming_edges.is_empty() {
                            rev_edges.push(inedge);
                        }
                    }
                }

                for re in rev_edges {
                    lgraph_util::edge_reverse(a, graph, re, true);
                }
            }
        }
    }
    Ok(())
}

/// `processBox`: process a comment box by putting it into a property of
/// the corresponding node.
fn process_box(
    a: &mut LGraphArena,
    boxx: LNodeId,
    edge: LEdgeId,
    opposite_port: LPortId,
    real_node: LNodeId,
) {
    let top_first: bool;
    let mut only_top = false;
    let mut only_bottom = false;
    if a.node(real_node)
        .properties
        .get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
        .is_side_fixed()
    {
        let mut has_north = false;
        let mut has_south = false;
        'port_loop: for &port1 in &a.node(real_node).ports {
            for port2 in port_connected_ports(a, port1) {
                let port2_node = a.port(port2).node.unwrap();
                if !a.node(port2_node).properties.get::<bool>(&lopts::COMMENT_BOX) {
                    if a.port(port1).side == PortSide::NORTH {
                        has_north = true;
                        break 'port_loop;
                    }
                    if a.port(port1).side == PortSide::SOUTH {
                        has_south = true;
                        break 'port_loop;
                    }
                }
            }
        }
        only_top = has_south && !has_north;
        only_bottom = has_north && !has_south;
    }

    if !only_top && !only_bottom && !a.node(real_node).labels.is_empty() {
        let mut label_pos = 0.0;
        for &label in &a.node(real_node).labels {
            label_pos += a.label(label).pos.y + a.label(label).size.y / 2.0;
        }
        label_pos /= a.node(real_node).labels.len() as f64;
        top_first = label_pos >= a.node(real_node).size.y / 2.0;
    } else {
        top_first = !only_bottom;
    }

    // Determine the list (property) the comment box is added to. We remember
    // which property won.
    let use_top: bool;
    if top_first {
        // determine the position to use, favoring the top position
        let top_boxes = a.node(real_node).properties.try_get(&iprops::TOP_COMMENTS);
        match top_boxes {
            None => {
                a.node(real_node)
                    .properties
                    .set(&iprops::TOP_COMMENTS, Vec::<LNodeId>::new());
                use_top = true;
            }
            Some(top_boxes) => {
                if only_top {
                    use_top = true;
                } else {
                    let bottom_boxes =
                        a.node(real_node).properties.try_get(&iprops::BOTTOM_COMMENTS);
                    match bottom_boxes {
                        None => {
                            a.node(real_node)
                                .properties
                                .set(&iprops::BOTTOM_COMMENTS, Vec::<LNodeId>::new());
                            use_top = false;
                        }
                        Some(bottom_boxes) => {
                            use_top = top_boxes.len() <= bottom_boxes.len();
                        }
                    }
                }
            }
        }
    } else {
        // determine the position to use, favoring the bottom position
        let bottom_boxes = a.node(real_node).properties.try_get(&iprops::BOTTOM_COMMENTS);
        match bottom_boxes {
            None => {
                a.node(real_node)
                    .properties
                    .set(&iprops::BOTTOM_COMMENTS, Vec::<LNodeId>::new());
                use_top = false;
            }
            Some(bottom_boxes) => {
                if only_bottom {
                    use_top = false;
                } else {
                    let top_boxes = a.node(real_node).properties.try_get(&iprops::TOP_COMMENTS);
                    match top_boxes {
                        None => {
                            a.node(real_node)
                                .properties
                                .set(&iprops::TOP_COMMENTS, Vec::<LNodeId>::new());
                            use_top = true;
                        }
                        Some(top_boxes) => {
                            use_top = !(bottom_boxes.len() <= top_boxes.len());
                        }
                    }
                }
            }
        }
    }

    // add the comment box to one of the two possible lists
    let list_property = if use_top { &iprops::TOP_COMMENTS } else { &iprops::BOTTOM_COMMENTS };
    let mut box_list = a
        .node(real_node)
        .properties
        .try_get(list_property)
        .unwrap_or_default();
    box_list.push(boxx);
    a.node(real_node).properties.set(list_property, box_list);

    // set the opposite port as property for the comment box
    a.node(boxx)
        .properties
        .set(&iprops::COMMENT_CONN_PORT, opposite_port);
    // detach the edge and the opposite port
    if a.edge(edge).target == Some(opposite_port) {
        a.edge_set_target(edge, None);
        if a.port_degree(opposite_port) == 0 {
            a.port_set_node(opposite_port, None);
        }
        remove_hierarchical_port_dummy_node(a, opposite_port);
    } else {
        a.edge_set_source(edge, None);
        if a.port_degree(opposite_port) == 0 {
            a.port_set_node(opposite_port, None);
        }
    }
    a.edge_mut(edge).bend_points.0.clear();
}

/// `LPort.getConnectedPorts`: the source ports of all incoming edges
/// followed by the target ports of all outgoing edges.
fn port_connected_ports(a: &LGraphArena, port: LPortId) -> Vec<LPortId> {
    let p = a.port(port);
    let mut result: Vec<LPortId> = Vec::new();
    for &edge in &p.incoming_edges {
        result.push(a.edge(edge).source.unwrap());
    }
    for &edge in &p.outgoing_edges {
        result.push(a.edge(edge).target.unwrap());
    }
    result
}

/// `removeHierarchicalPortDummyNode`.
fn remove_hierarchical_port_dummy_node(a: &mut LGraphArena, opposite_port: LPortId) {
    if let Some(dummy) = a
        .port(opposite_port)
        .properties
        .try_get(&iprops::PORT_DUMMY)
    {
        let layer = a.node(dummy).layer.expect("port dummy without layer");
        let graph = a.layer(layer).graph.expect("layer without graph");
        a.layer_mut(layer).nodes.retain(|&n| n != dummy);
        a.node_mut(dummy).layer = None;
        if a.layer(layer).nodes.is_empty() {
            a.graph_mut(graph).layers.retain(|&l| l != layer);
        }
    }
}
