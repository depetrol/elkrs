//! Ports of the SPOrE phases: `DelaunayTriangulationPhase`, `MinSTPhase`,
//! `MaxSTPhase`, `GrowTreePhase` and `ShrinkTreeCompactionPhase`.

use std::collections::HashMap;

use crate::alg_common::spore::depth_first_compact;
use crate::alg_common::tree::Forest;
use crate::alg_common::triangulation::{bowyer_watson_triangulate, naive_min_st, TEdge};
use crate::graph::math::KVector;

use crate::alg_spore::graph::Graph;
use crate::alg_spore::options::TreeConstructionStrategy;

fn kv_bits(v: KVector) -> (u64, u64) {
    (v.x.to_bits(), v.y.to_bits())
}

pub fn delaunay_triangulation_phase(graph: &mut Graph) {
    let vertices: Vec<KVector> = graph.vertices.iter().map(|v| v.original_vertex).collect();

    let triangulation = bowyer_watson_triangulate(&vertices);
    match &mut graph.t_edges {
        None => graph.t_edges = Some(triangulation),
        Some(existing) => {
            for e in triangulation.iter() {
                existing.add(*e);
            }
        }
    }
}

pub fn spanning_tree_phase(graph: &mut Graph, cost: impl Fn(&Graph, &TEdge) -> f64) {
    let gr: &Graph = graph;
    let t_tree = match gr.tree_construction_strategy {
        TreeConstructionStrategy::MINIMUM_SPANNING_TREE => {
            let root = match gr.preferred_root {
                Some(r) => gr.vertices[r].original_vertex,
                None => gr.vertices[0].original_vertex,
            };
            naive_min_st(gr.t_edges.as_ref().expect("tEdges"), root, |e| cost(gr, e))
        }
        TreeConstructionStrategy::MAXIMUM_SPANNING_TREE => {
            // inverted cost function; root uses the (current) vertex
            let root = match gr.preferred_root {
                Some(r) => gr.vertices[r].vertex,
                None => gr.vertices[0].vertex,
            };
            naive_min_st(gr.t_edges.as_ref().expect("tEdges"), root, |e| -cost(gr, e))
        }
    };

    // convert the Tree<KVector> to a Tree<Node>
    let mut node_map: HashMap<(u64, u64), usize> = HashMap::new();
    for (idx, n) in graph.vertices.iter().enumerate() {
        node_map.insert(kv_bits(n.original_vertex), idx);
    }
    let mut converted = Forest::new(node_map[&kv_bits(t_tree.nodes[t_tree.root].value)]);
    let converted_root = converted.root;
    convert_add(&t_tree, t_tree.root, &mut converted, converted_root, &node_map);
    graph.tree = Some(converted);
}

fn convert_add(
    t_tree: &Forest<KVector>,
    t_idx: usize,
    converted: &mut Forest<usize>,
    c_idx: usize,
    node_map: &HashMap<(u64, u64), usize>,
) {
    for &t_child in &t_tree.nodes[t_idx].children {
        let child_value = node_map[&kv_bits(t_tree.nodes[t_child].value)];
        let c_child = converted.add_child(c_idx, child_value);
        convert_add(t_tree, t_child, converted, c_child, node_map);
    }
}

/// The GTree algorithm of Nachmanson et al.
pub fn grow_tree_phase(graph: &mut Graph) {
    let tree = graph.tree.take().expect("tree");
    let mut overlaps_existed = false;
    grow_at(&tree, tree.root, &mut graph.vertices, &mut overlaps_existed);
    graph.tree = Some(tree);
    graph.overlaps_existed = overlaps_existed;
}

fn grow_at(
    tree: &Forest<usize>,
    r: usize,
    nodes: &mut [crate::alg_common::spore::Node],
    overlaps_existed: &mut bool,
) {
    let r_node = tree.nodes[r].value;
    for &c in &tree.nodes[r].children {
        let c_node = tree.nodes[c].value;

        // update position of the child
        let mut delta = nodes[r_node].vertex;
        delta.sub(nodes[r_node].original_vertex);
        nodes[c_node].translate(delta);

        // the elongation factor for an edge required to remove overlap
        let t = crate::alg_common::utils::overlap(&nodes[r_node].rect, &nodes[c_node].rect);
        if t > 1.0 {
            *overlaps_existed = true;
        }

        // elongate the edge by factor t to remove overlap
        let mut dir = nodes[c_node].original_vertex;
        dir.sub(nodes[r_node].original_vertex);
        dir.scale(t);
        let mut new_center = nodes[r_node].vertex;
        new_center.add(dir);
        nodes[c_node].set_center_position(new_center);

        grow_at(tree, c, nodes, overlaps_existed);
    }
}

pub fn shrink_tree_compaction_phase(graph: &mut Graph) {
    let tree = graph.tree.take().expect("tree");
    depth_first_compact(&tree, &mut graph.vertices, graph.orthogonal_compaction);
    graph.tree = Some(tree);
}
