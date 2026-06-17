//!
//! A node layerer that allows user interaction by respecting previous node
//! positions. These positions could be contrary to edge directions, so the
//! resulting layering must be checked for consistency.

use indexmap::IndexSet;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LayerId};

/// Utility class for marking horizontal regions that are already covered by
/// some nodes.
struct LayerSpan {
    start: f64,
    end: f64,
    nodes: Vec<LNodeId>,
}

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // create layers with a start and an end position, merging when they
    // overlap with others
    let mut current_spans: Vec<LayerSpan> = Vec::new();
    let nodes = a.graph(graph).layerless_nodes.clone();
    for node in nodes.iter().copied() {
        let minx = a.node(node).pos.x;
        let mut maxx = minx + a.node(node).size.x;
        // for the following code we have to guarantee that every node has a
        // width, which, for instance, might not be the case for external
        // dummy nodes
        maxx = (minx + 1.0).max(maxx);

        // look for a position in the sorted list where the node can be
        // inserted. We model a ListIterator: `cursor` is the index of
        // the element that would be returned by the next `next()` call.
        let mut cursor = 0usize;
        let mut found_span: Option<usize> = None;
        while cursor < current_spans.len() {
            let idx = cursor;
            cursor += 1; // listIterator.next()
            if current_spans[idx].start >= maxx {
                // the next layer span is further right, so insert the node here
                cursor -= 1; // spanIter.previous()
                break;
            } else if current_spans[idx].end > minx {
                // the layer span has an intersection with the node
                if found_span.is_none() {
                    // add the node to the current layer span
                    current_spans[idx].nodes.push(node);
                    current_spans[idx].start = current_spans[idx].start.min(minx);
                    current_spans[idx].end = current_spans[idx].end.max(maxx);
                    found_span = Some(idx);
                } else {
                    // merge the previously found layer span with the current one
                    let fs = found_span.unwrap();
                    let span_nodes = std::mem::take(&mut current_spans[idx].nodes);
                    let span_end = current_spans[idx].end;
                    current_spans[fs].nodes.extend(span_nodes);
                    current_spans[fs].end = current_spans[fs].end.max(span_end);
                    // remove the last element returned by next() (index `idx`).
                    current_spans.remove(idx);
                    cursor -= 1; // iterator cursor shifts back after removal
                    if fs > idx {
                        // found_span index moved due to the removal
                        found_span = Some(fs - 1);
                    }
                }
            }
        }
        if found_span.is_none() {
            // no intersecting span was found, so create a new one. Insert
            // before the element that would be returned by next() (index
            // `cursor`).
            current_spans.insert(
                cursor,
                LayerSpan { start: minx, end: maxx, nodes: vec![node] },
            );
        }
    }

    // create real layers from the layer spans
    let mut next_index = 0i32;
    for span in &current_spans {
        let layer = a.create_layer(graph);
        a.layer_mut(layer).id = next_index;
        next_index += 1;
        a.graph_mut(graph).layers.push(layer);
        for &node in &span.nodes {
            a.node_set_layer(node, Some(layer));
            a.node_mut(node).id = 0;
        }
    }

    // correct the layering respecting the graph topology, so edges point from
    // left to right
    for &node in &nodes {
        if a.node(node).id == 0 {
            let mut shifted_nodes = check_node(a, node, graph);
            // Since shiftedNodes might require other nodes to be shifted, do it
            // again until all shiftedNodes do no longer require another shift.
            while !shifted_nodes.is_empty() {
                let node_to_check = *shifted_nodes.iter().next().unwrap();
                shifted_nodes.shift_remove(&node_to_check);
                let more = check_node(a, node_to_check, graph);
                for n in more {
                    shifted_nodes.insert(n);
                }
            }
        }
    }

    // remove empty layers, which can happen when the layering has to be
    // corrected
    let layers = a.graph(graph).layers.clone();
    let mut remaining: Vec<LayerId> = Vec::new();
    for layer in layers {
        if a.layer(layer).nodes.is_empty() {
            // dropped
        } else {
            remaining.push(layer);
        }
    }
    a.graph_mut(graph).layers = remaining;

    // clear the list of nodes that have no layer, since now they all have one
    a.graph_mut(graph).layerless_nodes.clear();

    Ok(())
}

/// Check the layering of the given node by comparing the layer index of all
/// successors.
fn check_node(a: &mut LGraphArena, node1: LNodeId, graph: LGraphId) -> IndexSet<LNodeId> {
    a.node_mut(node1).id = 1;
    let layer1 = a.node(node1).layer.unwrap();
    let layer1_id = a.layer(layer1).id;
    let mut shift_nodes: IndexSet<LNodeId> = IndexSet::new();
    for port in a.node_output_ports(node1) {
        for edge in a.port(port).outgoing_edges.clone() {
            let node2 = a.edge_target_node(edge);
            if node1 != node2 {
                let layer2 = a.node(node2).layer.unwrap();
                if a.layer(layer2).id <= layer1_id {
                    // a violation was detected - move the target node to the
                    // next layer
                    let new_index = layer1_id + 1;
                    if new_index as usize == a.graph(graph).layers.len() {
                        let new_layer = a.create_layer(graph);
                        a.layer_mut(new_layer).id = new_index;
                        a.graph_mut(graph).layers.push(new_layer);
                        a.node_set_layer(node2, Some(new_layer));
                    } else {
                        let new_layer = a.graph(graph).layers[new_index as usize];
                        a.node_set_layer(node2, Some(new_layer));
                    }
                    shift_nodes.insert(node2);
                }
            }
        }
    }
    shift_nodes
}
