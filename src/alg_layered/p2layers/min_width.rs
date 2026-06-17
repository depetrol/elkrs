//!
//! Heuristic for the NP-hard minimum-width layering problem with
//! consideration of dummy nodes (Nikolov, Tarassov, Branke 2005), extended to
//! consider actual node sizes.

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::options_gen as lopts;

/// Recommended value ranges suggested by Nikolov et al.
const UPPERBOUND_ON_WIDTH_RANGE: (i32, i32) = (1, 4);
const COMPENSATOR_RANGE: (i32, i32) = (1, 2);

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let not_inserted: Vec<LNodeId> = a.graph(graph).layerless_nodes.clone();

    let upper_bound_on_width: i32 = a
        .graph(graph)
        .properties
        .get(&lopts::LAYERING_MIN_WIDTH_UPPER_BOUND_ON_WIDTH);
    let compensator: i32 = a
        .graph(graph)
        .properties
        .get(&lopts::LAYERING_MIN_WIDTH_UPPER_LAYER_ESTIMATION_SCALING_FACTOR);

    // First step to consider the real size of nodes: initialize the dummy
    // size with the spacing properties
    let mut dummy_size: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_EDGE);

    // Compute the minimum node size (of the real nodes).
    let mut minimum_node_size = f64::INFINITY;
    for &node in &not_inserted {
        if a.node(node).node_type != NodeType::NORMAL {
            continue;
        }
        let size = a.node(node).size.y;
        minimum_node_size = f64::min(minimum_node_size, size);
    }
    // The minimum node size might be zero; in that case don't normalize.
    minimum_node_size = f64::max(1.0, minimum_node_size);

    // Initialize the nodes' id (index into the degree and size arrays) and
    // compute normalized sizes.
    let num_of_nodes = not_inserted.len();
    let mut in_degree = vec![0i32; num_of_nodes];
    let mut out_degree = vec![0i32; num_of_nodes];
    let mut norm_size = vec![0f64; num_of_nodes];
    let mut i = 0i32;
    let mut avg_size = 0f64;
    for &node in &not_inserted {
        // Warning: LNode.id is being redefined here!
        a.node_mut(node).id = i;
        i += 1;
        let id = a.node(node).id as usize;
        in_degree[id] = count_edges_except_self_loops(a, &a.node_incoming_edges(node));
        out_degree[id] = count_edges_except_self_loops(a, &a.node_outgoing_edges(node));
        norm_size[id] = a.node(node).size.y / minimum_node_size;
        avg_size += norm_size[id];
    }
    // normalize dummy size, too:
    dummy_size /= minimum_node_size;
    // Divide sum of normalized node sizes by the number of nodes.
    avg_size /= num_of_nodes as f64;

    // Precalculate the successors of all nodes (sets of node ids, indexed by
    // node id; only membership is queried).
    let node_successors: Vec<Vec<usize>> = precalc_successors(a, &not_inserted);

    // Guarantee ConditionSelect from the paper: order the nodes by descending
    // maximum out-degree in advance (stable sort by descending out-degree).
    let mut sorted: Vec<LNodeId> = not_inserted.clone();
    sorted.sort_by(|&o1, &o2| {
        // reverse order: compare(o2, o1) of the ascending comparator
        let outs1 = out_degree[a.node(o2).id as usize];
        let outs2 = out_degree[a.node(o1).id as usize];
        outs1.cmp(&outs2)
    });

    let mut min_width = f64::INFINITY;
    let mut min_num_of_layers = i32::MAX;
    let mut candidate_layering: Option<Vec<Vec<LNodeId>>> = None;

    // At first blindly set the parameters to the configured exact values …
    let mut ubw_start = upper_bound_on_width;
    let mut ubw_end = upper_bound_on_width;
    let mut c_start = compensator;
    let mut c_end = compensator;

    // … then check whether special (negative) values have been used; in that
    // case use the recommended ranges.
    if upper_bound_on_width < 0 {
        ubw_start = UPPERBOUND_ON_WIDTH_RANGE.0;
        ubw_end = UPPERBOUND_ON_WIDTH_RANGE.1;
    }
    if compensator < 0 {
        c_start = COMPENSATOR_RANGE.0;
        c_end = COMPENSATOR_RANGE.1;
    }

    // Up to 8 iterations resulting in one, two, four or eight layerings.
    let mut ubw = ubw_start;
    while ubw <= ubw_end {
        let mut c = c_start;
        while c <= c_end {
            let (new_width, layering) = compute_min_width_layering(
                a,
                ubw,
                c,
                &sorted,
                &node_successors,
                avg_size,
                dummy_size,
                &in_degree,
                &out_degree,
                &norm_size,
            );

            // Replace the current candidate layering with a newly computed
            // one, if it is narrower or has the same maximum width but fewer
            // layers.
            let new_num_of_layers = layering.len() as i32;
            if new_width < min_width
                || (new_width == min_width && new_num_of_layers < min_num_of_layers)
            {
                min_width = new_width;
                min_num_of_layers = new_num_of_layers;
                candidate_layering = Some(layering);
            }
            c += 1;
        }
        ubw += 1;
    }

    // Finally, add the winning layering to the layered graph data structures.
    if let Some(candidate) = candidate_layering {
        for layer_list in candidate {
            let current_layer = a.create_layer(graph);
            for node in layer_list {
                a.node_set_layer(node, Some(current_layer));
            }
            a.graph_mut(graph).layers.push(current_layer);
        }
    }

    // The algorithm constructs the layering bottom up, but ElkLayered expects
    // the list of layers to be ordered top down.
    a.graph_mut(graph).layers.reverse();
    a.graph_mut(graph).layerless_nodes.clear();

    Ok(())
}

/// Per-node sets of successor
/// node ids without self-loops (deduplicated; only
/// membership is queried, so the order is irrelevant).
fn precalc_successors(a: &LGraphArena, nodes: &[LNodeId]) -> Vec<Vec<usize>> {
    let mut successors = Vec::with_capacity(nodes.len());
    for &node in nodes {
        let mut out_nodes: Vec<usize> = Vec::new();
        for edge in a.node_outgoing_edges(node) {
            if !is_self_loop(a, edge) {
                let id = a.node(a.edge_target_node(edge)).id as usize;
                if !out_nodes.contains(&id) {
                    out_nodes.push(id);
                }
            }
        }
        successors.push(out_nodes);
    }
    successors
}

#[allow(clippy::too_many_arguments)]
fn compute_min_width_layering(
    a: &LGraphArena,
    upper_bound_on_width: i32,
    compensator: i32,
    nodes: &[LNodeId],
    node_successors: &[Vec<usize>],
    avg_size: f64,
    dummy_size: f64,
    in_degree: &[i32],
    out_degree: &[i32],
    norm_size: &[f64],
) -> (f64, Vec<Vec<LNodeId>>) {
    let mut layers: Vec<Vec<LNodeId>> = Vec::new();
    // LinkedHashSet over the (sorted) nodes: iteration in insertion order
    let mut unplaced_nodes: Vec<LNodeId> = nodes.to_vec();

    // Our upper bound takes node sizes into account:
    let ubw_consider_size = upper_bound_on_width as f64 * avg_size;

    // in- and out-degree of the currently considered node
    let mut out_deg = 0i32;

    // nodes already placed in layers determined before the current layer,
    // indexed by node id
    let mut already_placed_in_other_layers = vec![false; norm_size.len()];

    let mut current_layer: Vec<LNodeId> = Vec::new();

    let mut width_current = 0f64;
    let mut width_up = 0f64;

    let mut max_width = 0f64;
    let mut real_width = 0f64;
    let mut current_spanning_edges = 0f64;
    let mut going_out_from_this_layer = 0f64;

    while !unplaced_nodes.is_empty() {
        // Find a node whose edges only point to nodes in the set
        // alreadyPlacedInOtherLayers; `None` if no such node exists.
        let current_node = select_node(
            a,
            &unplaced_nodes,
            node_successors,
            &already_placed_in_other_layers,
        );

        if let Some(cn) = current_node {
            let pos = unplaced_nodes.iter().position(|&n| n == cn).unwrap();
            unplaced_nodes.remove(pos);
            current_layer.push(cn);

            let id = a.node(cn).id as usize;
            out_deg = out_degree[id];
            width_current += norm_size[id] - out_deg as f64 * dummy_size;

            let in_deg = in_degree[id];
            width_up += in_deg as f64 * dummy_size;

            going_out_from_this_layer += out_deg as f64 * dummy_size;

            real_width += norm_size[id];
        }

        // Go to the next layer if 1) no node was selected, 2) no unplaced
        // nodes are left, or 3) conditionGoUp from the paper is satisfied.
        let go_up = match current_node {
            None => true,
            Some(cn) => {
                unplaced_nodes.is_empty()
                    || (width_current >= ubw_consider_size
                        && norm_size[a.node(cn).id as usize] > out_deg as f64 * dummy_size)
                    || width_up >= compensator as f64 * ubw_consider_size
            }
        };
        if go_up {
            for &n in &current_layer {
                already_placed_in_other_layers[a.node(n).id as usize] = true;
            }
            layers.push(std::mem::take(&mut current_layer));

            // Remove all edges starting at a node placed in this layer from
            // the dummy node count …
            current_spanning_edges -= going_out_from_this_layer;
            // … now compare the width including dummy node widths.
            max_width = f64::max(max_width, current_spanning_edges * dummy_size + real_width);
            // Consider new dummy nodes from edges coming into this layer.
            current_spanning_edges += width_up;

            width_current = width_up;
            width_up = 0.0;
            going_out_from_this_layer = 0.0;
            real_width = 0.0;
        }
    }

    (max_width, layers)
}

/// The first
/// node in `nodes` whose successors are all contained in `targets`.
fn select_node(
    a: &LGraphArena,
    nodes: &[LNodeId],
    successors: &[Vec<usize>],
    targets: &[bool],
) -> Option<LNodeId> {
    for &node in nodes {
        let succ = &successors[a.node(node).id as usize];
        if succ.iter().all(|&s| targets[s]) {
            return Some(node);
        }
    }
    None
}

fn count_edges_except_self_loops(a: &LGraphArena, edges: &[crate::alg_layered::graph::LEdgeId]) -> i32 {
    let mut i = 0;
    for &edge in edges {
        if !is_self_loop(a, edge) {
            i += 1;
        }
    }
    i
}

/// Source node == target node.
fn is_self_loop(a: &LGraphArena, edge: crate::alg_layered::graph::LEdgeId) -> bool {
    a.edge_source_node(edge) == a.edge_target_node(edge)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alg_layered::graph::LEdgeId;

    fn make_node(a: &mut LGraphArena, graph: LGraphId, height: f64) -> LNodeId {
        let n = a.create_node(graph);
        a.node_mut(n).size.y = height;
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

    fn layer_of(a: &LGraphArena, graph: LGraphId, node: LNodeId) -> usize {
        a.graph(graph)
            .layers
            .iter()
            .position(|&l| Some(l) == a.node(node).layer)
            .unwrap()
    }

    /// A simple chain must produce a valid layering with increasing indices.
    #[test]
    fn chain_layering_is_valid() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let n: Vec<LNodeId> = (0..4).map(|_| make_node(&mut a, g, 20.0)).collect();
        connect(&mut a, n[0], n[1]);
        connect(&mut a, n[1], n[2]);
        connect(&mut a, n[2], n[3]);

        process(&mut a, g).unwrap();

        for w in n.windows(2) {
            assert!(layer_of(&a, g, w[0]) < layer_of(&a, g, w[1]));
        }
        assert!(a.graph(g).layerless_nodes.is_empty());
    }

    /// Self loops must not break the layering.
    #[test]
    fn self_loops_are_ignored() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let n1 = make_node(&mut a, g, 20.0);
        let n2 = make_node(&mut a, g, 20.0);
        connect(&mut a, n1, n1);
        connect(&mut a, n1, n2);

        process(&mut a, g).unwrap();

        assert!(layer_of(&a, g, n1) < layer_of(&a, g, n2));
    }
}
