//! Performs the actual 'wrapping' of the
//! graph by relocating layers following a breaking point start dummy.

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::{BPInfo, BPInfoId, BPInfoStore};
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::processors::long_edge_joiner;
use crate::alg_layered::processors::wrapping_support as ws;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mut store = a.graph(graph).properties.try_get(&iprops::BP_INFO_STORE).unwrap_or_default();

    // #1 perform initial wrapping
    perform_wrapping(a, graph, &mut store);

    // #2 if desired, improve edge lengths
    if a.graph(graph).properties.get(&lopts::WRAPPING_MULTI_EDGE_IMPROVE_WRAPPED_EDGES) {
        // assign indexes to nodes (node.id == position within its layer)
        let layers = a.graph(graph).layers.clone();
        for l in layers {
            let nodes = a.layer(l).nodes.clone();
            for (index, n) in nodes.into_iter().enumerate() {
                a.node_mut(n).id = index as i32;
            }
        }

        improve_multi_cut_index_edges(a, graph, &mut store);
        improve_unnecessarily_long_edges(a, graph, &mut store, true);
        improve_unnecessarily_long_edges(a, graph, &mut store, false);
    }

    a.graph(graph).properties.set(&iprops::BP_INFO_STORE, store);
    Ok(())
}

/// Returns the BPInfo id attached to a node, if any.
fn bpi_of(a: &LGraphArena, n: LNodeId) -> Option<BPInfoId> {
    a.node(n).properties.try_get(&iprops::BREAKING_POINT_INFO)
}

fn is_start(a: &LGraphArena, store: &BPInfoStore, n: LNodeId) -> bool {
    bpi_of(a, n).map(|id| store.get(id).start == n).unwrap_or(false)
}

fn is_end(a: &LGraphArena, store: &BPInfoStore, n: LNodeId) -> bool {
    bpi_of(a, n).map(|id| store.get(id).end == n).unwrap_or(false)
}

fn perform_wrapping(a: &mut LGraphArena, graph: LGraphId, store: &mut BPInfoStore) {
    // add initial empty layer to account for break point start dummies
    let initial = a.create_layer(graph);
    a.graph_mut(graph).layers.insert(0, initial);

    let mut reverse = false;
    let mut idx: i32 = 1;

    // Iterate starting just after the inserted layer.
    let mut li = 1usize;
    loop {
        let layers = a.graph(graph).layers.clone();
        if li >= layers.len() {
            break;
        }
        let layer = layers[li];
        let new_layer = layers[idx as usize];
        let nodes_to_move = a.layer(layer).nodes.clone();

        let offset = nodes_to_move.len();

        // move the nodes to their new layer
        for &n in &nodes_to_move {
            a.node_set_layer(n, Some(new_layer));
        }

        if reverse {
            // process nodes in reversed order
            for &n in nodes_to_move.iter().rev() {
                for e in a.node_incoming_edges(n) {
                    crate::alg_layered::lgraph_util::edge_reverse(a, graph, e, true);
                    a.graph(graph).properties.set(&iprops::CYCLIC, true);

                    let dummy_edges = ws::insert_dummies(a, graph, e, offset);

                    // ameliorate breaking point info
                    let bpi = bpi_of(a, n).expect("wrapped node without BPInfo");
                    let start_in_layer_edge = *dummy_edges.last().unwrap();
                    let start_in_layer_dummy = a.edge_source_node(start_in_layer_edge);
                    let end_in_layer_dummy = a.edge_target_node(e);
                    let info = store.get_mut(bpi);
                    info.start_in_layer_dummy = Some(start_in_layer_dummy);
                    info.start_in_layer_edge = Some(start_in_layer_edge);
                    info.end_in_layer_dummy = Some(end_in_layer_dummy);
                    info.end_in_layer_edge = Some(e);
                }
            }
            reverse = false;
        } else if let Some(&a_node) = nodes_to_move.first() {
            if a.node(a_node).node_type == NodeType::BREAKING_POINT {
                reverse = true;
                idx = -1;
            }
        }

        idx += 1;
        li += 1;
    }

    // remove old layers that are now empty
    let layers = a.graph(graph).layers.clone();
    let kept: Vec<_> = layers.into_iter().filter(|&l| !a.layer(l).nodes.is_empty()).collect();
    a.graph_mut(graph).layers = kept;
}

fn improve_multi_cut_index_edges(a: &mut LGraphArena, graph: LGraphId, store: &mut BPInfoStore) {
    let layers = a.graph(graph).layers.clone();
    for l in layers {
        let nodes = a.layer(l).nodes.clone();
        for n in nodes {
            if is_start(a, store, n) {
                let info_id = bpi_of(a, n).unwrap();
                let info = store.get(info_id).clone();
                if info.prev.is_none() && info.next.is_some() {
                    let mut current_id = info_id;
                    let mut next_id = info.next;

                    while let Some(nid) = next_id {
                        let current = store.get(current_id).clone();
                        let next = store.get(nid).clone();

                        // drop the dummy chain
                        drop_dummies(a, store, next.start, next.start_in_layer_dummy.unwrap(), false, true);

                        // update in-layer indexes of subsequent nodes
                        update_indexes_after(a, current.end);
                        update_indexes_after(a, next.start);
                        update_indexes_after(a, next.start_in_layer_dummy.unwrap());
                        update_indexes_after(a, next.end_in_layer_dummy.unwrap());

                        // reconnect the edge
                        let cur_end_target = a.edge(current.end_in_layer_edge.unwrap()).target;
                        a.edge_set_target(next.end_in_layer_edge.unwrap(), cur_end_target);
                        a.edge_set_target(current.end_in_layer_edge.unwrap(), None);

                        // throw out unnecessary stuff
                        a.node_set_layer(current.end, None);
                        a.node_set_layer(next.start, None);
                        a.node_set_layer(next.start_in_layer_dummy.unwrap(), None);
                        a.node_set_layer(next.end_in_layer_dummy.unwrap(), None);

                        // assemble new BPInfo
                        let mut new_info = BPInfo::new(
                            current.start,
                            next.end,
                            current.node_start_edge,
                            next.start_end_edge,
                            next.original_edge,
                        );
                        new_info.start_in_layer_dummy = current.start_in_layer_dummy;
                        new_info.start_in_layer_edge = current.start_in_layer_edge;
                        new_info.end_in_layer_dummy = current.end_in_layer_dummy;
                        new_info.end_in_layer_edge = next.end_in_layer_edge;
                        new_info.prev = current.prev;
                        new_info.next = next.next;

                        let new_id = store.push(new_info);
                        a.node(current.start).properties.set(&iprops::BREAKING_POINT_INFO, new_id);
                        a.node(next.end).properties.set(&iprops::BREAKING_POINT_INFO, new_id);

                        next_id = next.next;
                        current_id = new_id;
                    }
                }
            }
        }
    }
}

fn improve_unnecessarily_long_edges(
    a: &mut LGraphArena,
    graph: LGraphId,
    store: &mut BPInfoStore,
    forwards: bool,
) {
    loop {
        let mut didsome = false;
        let mut layers = a.graph(graph).layers.clone();
        if forwards {
            layers.reverse();
        }
        for layer in layers {
            let mut nodes = a.layer(layer).nodes.clone();
            if !forwards {
                nodes.reverse();
            }
            for n in nodes {
                let matches = if forwards {
                    is_end(a, store, n)
                } else {
                    is_start(a, store, n)
                };
                if matches {
                    let bp_info_id = bpi_of(a, n).unwrap();
                    let dummy = if forwards {
                        store.get(bp_info_id).end_in_layer_dummy.unwrap()
                    } else {
                        store.get(bp_info_id).start_in_layer_dummy.unwrap()
                    };
                    didsome = drop_dummies(a, store, n, dummy, forwards, false);
                }
            }
        }
        if !didsome {
            break;
        }
    }
}

fn drop_dummies(
    a: &mut LGraphArena,
    _store: &mut BPInfoStore,
    bp_node: LNodeId,
    in_layer_dummy: LNodeId,
    forwards: bool,
    force: bool,
) -> bool {
    let mut pred_one = next_long_edge_dummy(a, bp_node, forwards);
    let mut pred_two = next_long_edge_dummy(a, in_layer_dummy, forwards);

    let mut didsome = false;
    while let (Some(p1), Some(p2)) = (pred_one, pred_two) {
        if force || is_adjacent_or_separated_by_breakingpoints(a, p1, p2, forwards) {
            let next_one = next_long_edge_dummy(a, p1, forwards);
            let next_two = next_long_edge_dummy(a, p2, forwards);

            update_indexes_after(a, in_layer_dummy);
            update_indexes_after(a, bp_node);

            // the two dummies were in the same layer
            let new_layer = a.node(p1).layer;

            long_edge_joiner::join_at(a, p1, false);
            long_edge_joiner::join_at(a, p2, false);

            let p1_id = a.node(p1).id;
            let p2_id = a.node(p2).id;

            if forwards {
                a.node_set_layer_at(in_layer_dummy, new_layer, p2_id as usize);
                a.node_mut(in_layer_dummy).id = p2_id;
                a.node_set_layer_at(bp_node, new_layer, (p1_id + 1) as usize);
                a.node_mut(bp_node).id = p1_id;
            } else {
                a.node_set_layer_at(bp_node, new_layer, p1_id as usize);
                a.node_mut(bp_node).id = p1_id;
                a.node_set_layer_at(in_layer_dummy, new_layer, (p2_id + 1) as usize);
                a.node_mut(in_layer_dummy).id = p2_id;
            }

            a.node_set_layer(p1, None);
            a.node_set_layer(p2, None);

            pred_one = next_one;
            pred_two = next_two;
            didsome = true;
        } else {
            break;
        }
    }
    didsome
}

fn is_adjacent_or_separated_by_breakingpoints(
    a: &LGraphArena,
    dummy1: LNodeId,
    dummy2: LNodeId,
    forwards: bool,
) -> bool {
    let layer = a.node(dummy1).layer.unwrap();
    let start = if forwards { dummy2 } else { dummy1 };
    let end = if forwards { dummy1 } else { dummy2 };

    let start_id = a.node(start).id;
    let end_id = a.node(end).id;
    let mut i = start_id + 1;
    while i < end_id {
        let node = a.layer(layer).nodes[i as usize];
        if !(a.node(node).node_type == NodeType::BREAKING_POINT || is_in_layer_dummy(a, node)) {
            return false;
        }
        i += 1;
    }
    true
}

fn next_long_edge_dummy(a: &LGraphArena, start: LNodeId, forwards: bool) -> Option<LNodeId> {
    let edges = if forwards {
        a.node_outgoing_edges(start)
    } else {
        a.node_incoming_edges(start)
    };
    let start_layer = a.node(start).layer;
    for e in edges {
        let other = edge_other(a, e, start);
        if a.node(other).node_type == NodeType::LONG_EDGE && a.node(other).layer != start_layer {
            return Some(other);
        }
    }
    None
}

fn edge_other(a: &LGraphArena, e: LEdgeId, node: LNodeId) -> LNodeId {
    let src = a.edge_source_node(e);
    if src == node {
        a.edge_target_node(e)
    } else {
        src
    }
}

fn is_in_layer_dummy(a: &LGraphArena, node: LNodeId) -> bool {
    if a.node(node).node_type == NodeType::LONG_EDGE {
        for e in a.node_connected_edges(node) {
            if !a.edge_is_self_loop(e) && a.node(node).layer == a.node(edge_other(a, e, node)).layer
            {
                return true;
            }
        }
    }
    false
}

fn update_indexes_after(a: &mut LGraphArena, node: LNodeId) {
    let layer = a.node(node).layer.unwrap();
    let start = a.node(node).id + 1;
    let nodes = a.layer(layer).nodes.clone();
    let mut i = start;
    while (i as usize) < nodes.len() {
        let other = nodes[i as usize];
        a.node_mut(other).id -= 1;
        i += 1;
    }
}
