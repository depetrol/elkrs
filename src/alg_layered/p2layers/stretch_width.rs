//!
//! StretchWidth layering algorithm (Nikolov, Tarassov, Branke), designed to
//! create a layering as narrow as possible.

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // if no nodes need to be placed we can stop right here
    if a.graph(graph).layerless_nodes.is_empty() {
        return Ok(());
    }

    let mut width_current = 0f64;
    let mut width_up = 0f64;
    let mut minimum_node_size = f64::INFINITY;
    let mut maximum_node_size = f64::NEG_INFINITY;

    // initialize the dummy size with the spacing properties
    let mut dummy_size: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_EDGE);

    // --- computeSortedNodes: sort in descending order by rank ------------
    let unsorted: Vec<LNodeId> = a.graph(graph).layerless_nodes.clone();
    // the id-field is reused later on, be careful
    for &node in &unsorted {
        let rank = get_rank(a, node);
        a.node_mut(node).id = rank;
    }
    let mut sorted_layerless_nodes = unsorted.clone();
    // stable descending sort by node id / rank
    sorted_layerless_nodes.sort_by(|&o1, &o2| a.node(o2).id.cmp(&a.node(o1).id));

    // --- computeSuccessors: re-assigns node ids in sorted order ----------
    // (The successor sets themselves are never read; only the id assignment
    // has an observable effect.)
    for (i, &node) in sorted_layerless_nodes.iter().enumerate() {
        a.node_mut(node).id = i as i32;
    }

    // --- computeDegrees ---------------------------------------------------
    let n = sorted_layerless_nodes.len();
    let mut in_degree = vec![0i32; n];
    let mut out_degree = vec![0i32; n];
    for &node in &sorted_layerless_nodes {
        let id = a.node(node).id as usize;
        in_degree[id] = a.node_incoming_edges(node).len() as i32;
        out_degree[id] = a.node_outgoing_edges(node).len() as i32;
    }

    // --- minMaxNodeSize ----------------------------------------------------
    for &node in &sorted_layerless_nodes {
        if a.node(node).node_type != NodeType::NORMAL {
            continue;
        }
        let size = a.node(node).size.y;
        minimum_node_size = f64::min(minimum_node_size, size);
        maximum_node_size = f64::max(maximum_node_size, size);
    }

    // --- computeNormalizedSize (uses the *unclamped* minimum node size!) ---
    let mut norm_size = vec![0f64; n];
    for &node in &sorted_layerless_nodes {
        norm_size[a.node(node).id as usize] = a.node(node).size.y / minimum_node_size;
    }

    // make sure the values are reasonable
    minimum_node_size = f64::max(1.0, minimum_node_size);
    maximum_node_size = f64::max(1.0, maximum_node_size);

    // normalize dummy size
    dummy_size /= minimum_node_size;
    let mut max_width = maximum_node_size / minimum_node_size;

    // average out-degree; computed in float arithmetic
    let upper_layer_influence = get_average_out_degree(a, graph) as f64;

    // Layer currently worked on; add the first layer to the graph
    let mut current_layer = a.create_layer(graph);
    a.graph_mut(graph).layers.push(current_layer);

    // Copy the sorted layerless nodes so we don't overwrite it in the reset case
    let mut temp_layerless_nodes: Vec<LNodeId> = sorted_layerless_nodes.clone();
    // Copy the outDegree array
    let mut remaining_out_going = out_degree.clone();

    // number of nodes placed in the current layer
    let mut already_placed_count = 0usize;

    while !temp_layerless_nodes.is_empty() {
        // Select a node to be placed
        let selected_node = temp_layerless_nodes
            .iter()
            .copied()
            .find(|&node| remaining_out_going[a.node(node).id as usize] <= 0);

        let condition_go_up = |sel: LNodeId| -> bool {
            let id = a.node(sel).id as usize;
            let cond_a = (width_current - (out_degree[id] as f64 * dummy_size) + norm_size[id])
                > max_width;
            let cond_b = (width_up + in_degree[id] as f64 * dummy_size)
                > (max_width * upper_layer_influence * dummy_size);
            cond_a || cond_b
        };

        if selected_node.is_none()
            || (condition_go_up(selected_node.unwrap()) && already_placed_count != 0)
        {
            // go to the next layer //
            // update the remaining successors of the nodes
            update_out_going(a, current_layer, &mut remaining_out_going);
            current_layer = a.create_layer(graph);
            a.graph_mut(graph).layers.push(current_layer);
            already_placed_count = 0;
            // change width
            width_current = width_up;
            width_up = 0.0;
        } else {
            let selected = selected_node.unwrap();
            if condition_go_up(selected) {
                // reset layering //
                a.graph_mut(graph).layers.clear();
                current_layer = a.create_layer(graph);
                a.graph_mut(graph).layers.push(current_layer);
                width_current = 0.0;
                width_up = 0.0;
                already_placed_count = 0;
                // increase maxWidth
                max_width += 1.0;
                // reset layerless nodes
                temp_layerless_nodes = sorted_layerless_nodes.clone();
                // reset successors
                remaining_out_going = out_degree.clone();
            } else {
                // add node to current layer //
                a.node_set_layer(selected, Some(current_layer));
                let pos = temp_layerless_nodes
                    .iter()
                    .position(|&x| x == selected)
                    .unwrap();
                temp_layerless_nodes.remove(pos);
                already_placed_count += 1;
                // compute new widthCurrent and widthUp
                let id = a.node(selected).id as usize;
                width_current =
                    width_current - out_degree[id] as f64 * dummy_size + norm_size[id];
                width_up += in_degree[id] as f64 * dummy_size;
            }
        }
    }

    // Layering done, delete original layerless nodes
    a.graph_mut(graph).layerless_nodes.clear();
    // Algorithm is bottom-up -> reverse layers
    a.graph_mut(graph).layers.reverse();

    Ok(())
}

/// Max(d⁺(v), max(d⁺(u) : (u,v) ∈ E)).
fn get_rank(a: &LGraphArena, node: LNodeId) -> i32 {
    let mut max = a.node_outgoing_edges(node).len() as i32;
    for pre_edge in a.node_incoming_edges(node) {
        let pre = a.edge_source_node(pre_edge);
        let temp = a.node_outgoing_edges(pre).len() as i32;
        max = max.max(temp);
    }
    max
}

fn get_average_out_degree(a: &LGraphArena, graph: LGraphId) -> f32 {
    let mut all_out = 0f32;
    for &node in &a.graph(graph).layerless_nodes {
        all_out += a.node_outgoing_edges(node).len() as f32;
    }
    all_out / a.graph(graph).layerless_nodes.len() as f32
}

fn update_out_going(
    a: &LGraphArena,
    current_layer: crate::alg_layered::graph::LayerId,
    remaining_out_going: &mut [i32],
) {
    for &node in &a.layer(current_layer).nodes {
        for edge in a.node_incoming_edges(node) {
            let pos = a.node(a.edge_source_node(edge)).id as usize;
            remaining_out_going[pos] -= 1;
        }
    }
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

    /// A chain must produce a valid layering with strictly increasing indices.
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
}
