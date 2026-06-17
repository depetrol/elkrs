//!
//! A layering algorithm that places nodes in layers subject to a bound on the
//! maximum number of (original) nodes per layer (Coffman & Graham 1972).

use std::cmp::Ordering;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LayerId};
use crate::alg_layered::options_gen as lopts;

/// Replica of `java.util.PriorityQueue` with its exact sift semantics and
/// comparison call pattern. The comparator is passed per operation because it
/// closes over state that is mutated between operations (the comparator object
/// reads mutable fields).
///
/// The comparator-using siftDown compares `(child, right)` and
/// `(inserted, child)`; this matters here because CoffmanGraham's comparator
/// is not consistent (see `compare_nodes_in_topo`).
struct JavaPq {
    heap: Vec<LNodeId>,
}

impl JavaPq {
    fn new() -> Self {
        JavaPq { heap: Vec::new() }
    }

    fn add(&mut self, x: LNodeId, cmp: &mut dyn FnMut(LNodeId, LNodeId) -> Ordering) {
        let mut k = self.heap.len();
        self.heap.push(x);
        while k > 0 {
            let parent = (k - 1) >> 1;
            if cmp(x, self.heap[parent]) != Ordering::Less {
                break;
            }
            self.heap[k] = self.heap[parent];
            k = parent;
        }
        self.heap[k] = x;
    }

    fn poll(&mut self, cmp: &mut dyn FnMut(LNodeId, LNodeId) -> Ordering) -> Option<LNodeId> {
        if self.heap.is_empty() {
            return None;
        }
        let result = self.heap[0];
        let x = self.heap.pop().unwrap();
        if !self.heap.is_empty() {
            // siftDown(0, x)
            let n = self.heap.len();
            let half = n >> 1;
            let mut k = 0;
            while k < half {
                let mut child = 2 * k + 1;
                let right = child + 1;
                if right < n && cmp(self.heap[child], self.heap[right]) == Ordering::Greater {
                    child = right;
                }
                if cmp(x, self.heap[child]) != Ordering::Greater {
                    break;
                }
                self.heap[k] = self.heap[child];
                k = child;
            }
            self.heap[k] = x;
        }
        Some(result)
    }
}

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    if a.graph(graph).layerless_nodes.is_empty() {
        return Ok(());
    }

    // the maximum number of allowed nodes per layer
    let w: i32 = a
        .graph(graph)
        .properties
        .get(&lopts::LAYERING_COFFMAN_GRAHAM_LAYER_BOUND);

    let nodes: Vec<LNodeId> = a.graph(graph).layerless_nodes.clone();

    // initialization, assign indexes and initialize arrays
    let mut index = 0i32;
    let mut edge_index = 0i32;
    for &n in &nodes {
        a.node_mut(n).id = index;
        index += 1;
        for e in a.node_outgoing_edges(n) {
            a.edge_mut(e).id = edge_index;
            edge_index += 1;
        }
    }
    let mut node_mark = vec![false; index as usize];
    let mut edge_mark = vec![false; edge_index as usize];
    let mut in_deg = vec![0i32; index as usize];
    let mut out_deg = vec![0i32; index as usize];
    let mut topo_ord = vec![0i32; index as usize];
    // for each node, the positions of incoming nodes in the topological
    // ordering (the lists are sorted by construction)
    let mut in_topo: Vec<Vec<i32>> = vec![Vec::new(); index as usize];

    // --------------------------
    // #1 Remove transitive edges
    // --------------------------
    for &start in &nodes {
        node_mark.fill(false);
        for out in a.node_outgoing_edges(start) {
            let target = a.edge_target_node(out);
            dfs(a, start, target, &mut node_mark, &mut edge_mark);
        }
    }

    // -------------------------------
    // #2 Compute topological ordering
    // -------------------------------
    let mut sources = JavaPq::new();

    // for each node, determine its current in-degree and remember initial sources
    for &v in &nodes {
        for e in a.node_incoming_edges(v) {
            if !edge_mark[a.edge(e).id as usize] {
                in_deg[a.node(v).id as usize] += 1;
            }
        }
        if in_deg[a.node(v).id as usize] == 0 {
            sources.add(v, &mut |u, vv| compare_nodes_in_topo(a, &in_topo, u, vv));
        }
    }

    // compute topological ordering
    let mut i = 0i32;
    loop {
        let v = match sources.poll(&mut |u, vv| compare_nodes_in_topo(a, &in_topo, u, vv)) {
            Some(v) => v,
            None => break,
        };
        // assign number of topological order
        topo_ord[a.node(v).id as usize] = i;
        i += 1;

        // update the rest of the graph
        for e in a.node_outgoing_edges(v) {
            if edge_mark[a.edge(e).id as usize] {
                continue;
            }
            let tgt = a.edge_target_node(e);
            in_deg[a.node(tgt).id as usize] -= 1;
            in_topo[a.node(tgt).id as usize].push(topo_ord[a.node(v).id as usize]);
            if in_deg[a.node(tgt).id as usize] == 0 {
                // 'tgt' is added according to its priority
                sources.add(tgt, &mut |u, vv| compare_nodes_in_topo(a, &in_topo, u, vv));
            }
        }
    }

    // --------------------------
    // #3 Actual layer assignment
    // --------------------------
    // note that this time we start with sinks and work our way to the original
    // graph's sources; highest priority (max topological order) first
    let mut sinks = JavaPq::new();
    let mut sinks_cmp = |n1: LNodeId, n2: LNodeId| -> Ordering {
        // -Integer.compare(topoOrd[n1.id], topoOrd[n2.id])
        topo_ord[a.node(n1).id as usize]
            .cmp(&topo_ord[a.node(n2).id as usize])
            .reverse()
    };
    for &v in &nodes {
        for e in a.node_outgoing_edges(v) {
            if !edge_mark[a.edge(e).id as usize] {
                out_deg[a.node(v).id as usize] += 1;
            }
        }
        if out_deg[a.node(v).id as usize] == 0 {
            sinks.add(v, &mut sinks_cmp);
        }
    }

    // assign the layers
    let mut layers: Vec<LayerId> = Vec::new();
    let mut current_layer = create_layer(a, graph, &mut layers);
    loop {
        // select a node for which all outgoing nodes have been placed,
        // and with maximum value in the topological sort
        let u = {
            let mut cmp = |n1: LNodeId, n2: LNodeId| -> Ordering {
                topo_ord[a.node(n1).id as usize]
                    .cmp(&topo_ord[a.node(n2).id as usize])
                    .reverse()
            };
            match sinks.poll(&mut cmp) {
                Some(u) => u,
                None => break,
            }
        };

        // start a new layer if the current one is already full,
        // or an in-layer edge would be introduced
        if is_layer_full(a, current_layer, w) || !can_add(a, u, current_layer) {
            current_layer = create_layer(a, graph, &mut layers);
        }

        // place the node in the layer
        a.node_set_layer(u, Some(current_layer));

        // update out-degrees and collect the new sinks
        for e in a.node_incoming_edges(u) {
            if edge_mark[a.edge(e).id as usize] {
                continue;
            }
            let src = a.edge_source_node(e);
            out_deg[a.node(src).id as usize] -= 1;
            if out_deg[a.node(src).id as usize] == 0 {
                let mut cmp = |n1: LNodeId, n2: LNodeId| -> Ordering {
                    topo_ord[a.node(n1).id as usize]
                        .cmp(&topo_ord[a.node(n2).id as usize])
                        .reverse()
                };
                sinks.add(src, &mut cmp);
            }
        }
    }

    // the layers were created in inverse order
    for j in (0..layers.len()).rev() {
        let l = layers[j];
        a.graph_mut(graph).layers.push(l);
    }

    // clear layerless nodes
    a.graph_mut(graph).layerless_nodes.clear();

    Ok(())
}

fn is_layer_full(a: &LGraphArena, layer: LayerId, w: i32) -> bool {
    a.layer(layer).nodes.len() as i64 >= w as i64
}

/// `true` if node `n` can be added to layer `l` without introducing in-layer edges.
fn can_add(a: &LGraphArena, n: LNodeId, l: LayerId) -> bool {
    for e in a.node_outgoing_edges(n) {
        let v = a.edge_target_node(e);
        if a.node(v).layer == Some(l) {
            return false;
        }
    }
    true
}

/// Creates a layer, appends it to `layers` (but not to the graph's layer list).
fn create_layer(a: &mut LGraphArena, graph: LGraphId, layers: &mut Vec<LayerId>) -> LayerId {
    let layer = a.create_layer(graph);
    layers.push(layer);
    layer
}

///    are treated as identical and the loop continues; equal values >= 128 are
///    treated as distinct, so the branch returns 0 immediately.
/// 2. The post-loop check uses `has_next` (cursor < size), not
///    `has_previous`. With backwards iteration from the end, `has_next` is
///    false exactly when the iterator never moved (or the list is empty).
fn compare_nodes_in_topo(
    a: &LGraphArena,
    in_topo: &[Vec<i32>],
    u: LNodeId,
    v: LNodeId,
) -> Ordering {
    let u_id = a.node(u).id;
    let v_id = a.node(v).id;
    let list_u = &in_topo[u_id as usize];
    let list_v = &in_topo[v_id as usize];
    let mut cur_u = list_u.len(); // ListIterator cursor, starts at the end
    let mut cur_v = list_v.len();

    // find the node with the lower associated maximum value in 'inTopo';
    // break ties by ignoring all equal maxima
    while cur_u > 0 && cur_v > 0 {
        cur_u -= 1;
        cur_v -= 1;
        let iu = list_u[cur_u];
        let iv = list_v[cur_v];
        if iu != iv {
            return iu.cmp(&iv);
        } else if !(-128..=127).contains(&iu) {
            return Ordering::Equal;
        }
        // equal cached boxes: same object, continue with earlier values
    }

    let u_has_next = cur_u < list_u.len();
    let v_has_next = cur_v < list_v.len();
    if !u_has_next && !v_has_next {
        // If the two nodes are the same, a secondary criterion is needed.
        // Else use the node id as a last resort.
        u_id.cmp(&v_id)
    } else if !u_has_next {
        Ordering::Less // u < v
    } else {
        Ordering::Greater // u > v
    }
}

fn dfs(
    a: &LGraphArena,
    start: LNodeId,
    v: LNodeId,
    node_mark: &mut [bool],
    edge_mark: &mut [bool],
) {
    if node_mark[a.node(v).id as usize] {
        return;
    }

    for out in a.node_outgoing_edges(v) {
        let w = a.edge_target_node(out);
        for transitive in a.node_incoming_edges(w) {
            if a.edge_source_node(transitive) == start {
                edge_mark[a.edge(transitive).id as usize] = true;
            }
        }
        dfs(a, start, w, node_mark, edge_mark);
    }

    node_mark[a.node(v).id as usize] = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alg_layered::graph::LEdgeId;

    fn make_node(a: &mut LGraphArena, graph: LGraphId) -> LNodeId {
        let n = a.create_node(graph);
        a.graph_mut(graph).layerless_nodes.push(n);
        n
    }

    fn connect(a: &mut LGraphArena, source: LNodeId, target: LNodeId) -> LEdgeId {
        let sp = a.create_port();
        a.port_set_node(sp, Some(source));
        let tp = a.create_port();
        a.port_set_node(tp, Some(target));
        let e = a.create_edge();
        a.edge_set_source(e, Some(sp));
        a.edge_set_target(e, Some(tp));
        e
    }

    fn layer_nodes(a: &LGraphArena, graph: LGraphId) -> Vec<Vec<LNodeId>> {
        a.graph(graph)
            .layers
            .iter()
            .map(|&l| a.layer(l).nodes.clone())
            .collect()
    }

    /// Chain n0->n1->n2 with transitive edge n0->n2: the transitive edge is
    /// reduced; result is three layers.
    #[test]
    fn chain_with_transitive_edge() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let n: Vec<LNodeId> = (0..3).map(|_| make_node(&mut a, g)).collect();
        connect(&mut a, n[0], n[1]);
        connect(&mut a, n[1], n[2]);
        connect(&mut a, n[0], n[2]);

        process(&mut a, g).unwrap();

        assert_eq!(
            layer_nodes(&a, g),
            vec![vec![n[0]], vec![n[1]], vec![n[2]]]
        );
        assert!(a.graph(g).layerless_nodes.is_empty());
    }

    /// Four independent nodes with layer bound 2: two layers of two nodes.
    #[test]
    fn width_bound_respected() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let _n: Vec<LNodeId> = (0..4).map(|_| make_node(&mut a, g)).collect();
        a.graph(g)
            .properties
            .set(&lopts::LAYERING_COFFMAN_GRAHAM_LAYER_BOUND, 2);

        process(&mut a, g).unwrap();

        let layers = layer_nodes(&a, g);
        assert_eq!(layers.len(), 2);
        assert!(layers.iter().all(|l| l.len() == 2));
    }
}
