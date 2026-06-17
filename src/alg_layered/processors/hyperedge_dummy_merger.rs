//! Merges adjacent long edge dummy nodes that
//! belong to the same hyperedge (sharing a port) so that edges originating from
//! or going into the same port are joined. Runs after crossing minimization,
//! before phase 4.

use crate::core::options::PortSide;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    identify_hyperedges(a, graph);

    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        // empty layers are handled by the loop anyway.
        // a signed index is used that can decrement to -1 then re-increment.
        let mut node_index: i64 = 0;
        let mut last_node: Option<LNodeId> = None;
        let mut last_node_type: Option<NodeType> = None;

        while (node_index as usize) < a.layer(layer).nodes.len() {
            let mut curr_node = a.layer(layer).nodes[node_index as usize];
            let mut curr_node_type = a.node(curr_node).node_type;

            if curr_node_type == NodeType::LONG_EDGE
                && last_node_type == Some(NodeType::LONG_EDGE)
            {
                let last = last_node.unwrap();
                let state = check_merge_allowed(a, curr_node, last);
                if state.allow_merge {
                    merge_nodes(a, curr_node, last, state.same_source, state.same_target);

                    a.layer_mut(layer).nodes.remove(node_index as usize);
                    node_index -= 1;
                    curr_node = last;
                    curr_node_type = last_node_type.unwrap();
                }
            }

            last_node = Some(curr_node);
            last_node_type = Some(curr_node_type);
            node_index += 1;
        }
    }

    Ok(())
}

struct MergeState {
    allow_merge: bool,
    same_source: bool,
    same_target: bool,
}

fn check_merge_allowed(a: &LGraphArena, curr_node: LNodeId, last_node: LNodeId) -> MergeState {
    let curr_has_label_dummies =
        a.node(curr_node).properties.get(&iprops::LONG_EDGE_HAS_LABEL_DUMMIES);
    let last_has_label_dummies =
        a.node(last_node).properties.get(&iprops::LONG_EDGE_HAS_LABEL_DUMMIES);

    let curr_node_source = a.node(curr_node).properties.try_get(&iprops::LONG_EDGE_SOURCE);
    let last_node_source = a.node(last_node).properties.try_get(&iprops::LONG_EDGE_SOURCE);
    let curr_node_target = a.node(curr_node).properties.try_get(&iprops::LONG_EDGE_TARGET);
    let last_node_target = a.node(last_node).properties.try_get(&iprops::LONG_EDGE_TARGET);

    // same source/target (non-null!)
    let same_source = curr_node_source.is_some() && curr_node_source == last_node_source;
    let same_target = curr_node_target.is_some() && curr_node_target == last_node_target;

    if !curr_has_label_dummies && !last_has_label_dummies {
        // assumption: long edge dummies always have two ports, both have same id
        let curr_first_port_id = a.node(curr_node).ports[0];
        let last_first_port_id = a.node(last_node).ports[0];
        let allow = a.port(curr_first_port_id).id == a.port(last_first_port_id).id;
        return MergeState { allow_merge: allow, same_source, same_target };
    }

    let curr_before = a.node(curr_node).properties.get(&iprops::LONG_EDGE_BEFORE_LABEL_DUMMY);
    let last_before = a.node(last_node).properties.get(&iprops::LONG_EDGE_BEFORE_LABEL_DUMMY);

    let eligible_for_source_merging =
        (!curr_has_label_dummies || curr_before) && (!last_has_label_dummies || last_before);

    let eligible_for_target_merging =
        (!curr_has_label_dummies || !curr_before) && (!last_has_label_dummies || !last_before);

    MergeState {
        allow_merge: (same_source && eligible_for_source_merging)
            || (same_target && eligible_for_target_merging),
        same_source,
        same_target,
    }
}

fn merge_nodes(
    a: &mut LGraphArena,
    merge_source: LNodeId,
    merge_target: LNodeId,
    keep_source_port: bool,
    keep_target_port: bool,
) {
    // input port is west, output port east
    let merge_target_input_port = a
        .node_ports_on_side(merge_target, PortSide::WEST)
        .into_iter()
        .next()
        .expect("merge target without west port");
    let merge_target_output_port = a
        .node_ports_on_side(merge_target, PortSide::EAST)
        .into_iter()
        .next()
        .expect("merge target without east port");

    let source_ports = a.node(merge_source).ports.clone();
    for port in source_ports {
        while !a.port(port).incoming_edges.is_empty() {
            let edge = a.port(port).incoming_edges[0];
            a.edge_set_target(edge, Some(merge_target_input_port));
        }
        while !a.port(port).outgoing_edges.is_empty() {
            let edge = a.port(port).outgoing_edges[0];
            a.edge_set_source(edge, Some(merge_target_output_port));
        }
    }

    if !keep_source_port {
        a.node(merge_target).properties.unset(&iprops::LONG_EDGE_SOURCE);
    }
    if !keep_target_port {
        a.node(merge_target).properties.unset(&iprops::LONG_EDGE_TARGET);
    }
}

fn identify_hyperedges(a: &mut LGraphArena, graph: LGraphId) {
    // collect ports in layer/node/port order
    let mut ports: Vec<LPortId> = Vec::new();
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            for &port in &a.node(node).ports {
                ports.push(port);
            }
        }
    }

    for &p in &ports {
        a.port_mut(p).id = -1;
    }

    let mut index = 0i32;
    for &p in &ports {
        if a.port(p).id == -1 {
            dfs(a, p, index);
            index += 1;
        }
    }
}

fn dfs(a: &mut LGraphArena, p: LPortId, index: i32) {
    a.port_mut(p).id = index;
    // follow edges connected to the same port (predecessor/successor ports)
    for p2 in port_connected_ports(a, p) {
        if a.port(p2).id == -1 {
            dfs(a, p2, index);
        }
    }
    // follow edges connected to the same long edge dummy
    let node = a.port(p).node.unwrap();
    if a.node(node).node_type == NodeType::LONG_EDGE {
        let node_ports = a.node(node).ports.clone();
        for p2 in node_ports {
            if p2 != p && a.port(p2).id == -1 {
                dfs(a, p2, index);
            }
        }
    }
}

/// `LPort.getConnectedPorts`: source ports of incoming edges followed by
/// target ports of outgoing edges.
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
