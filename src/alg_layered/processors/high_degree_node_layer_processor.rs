//! Moves small trees connected to
//! high-degree nodes into newly introduced layers next to the high-degree
//! node's layer, reducing drawing height. Runs after phase 2.

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LayerId};
use crate::alg_layered::options_gen as lopts;

#[derive(Clone, Copy, PartialEq)]
enum EdgeDir {
    Incoming,
    Outgoing,
}

fn edges_of(a: &LGraphArena, node: LNodeId, dir: EdgeDir) -> Vec<LEdgeId> {
    match dir {
        EdgeDir::Incoming => a.node_incoming_edges(node),
        EdgeDir::Outgoing => a.node_outgoing_edges(node),
    }
}

struct Ctx {
    degree_threshold: i32,
    tree_height_threshold: i32,
}

#[derive(Default)]
struct HighDegreeNodeInformation {
    inc_trees_max_height: i32,
    inc_tree_roots: Option<Vec<LNodeId>>,
    out_trees_max_height: i32,
    out_tree_roots: Option<Vec<LNodeId>>,
}

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let degree_threshold = a.graph(graph).properties.get(&lopts::HIGH_DEGREE_NODES_THRESHOLD);
    let mut tree_height_threshold =
        a.graph(graph).properties.get(&lopts::HIGH_DEGREE_NODES_TREE_HEIGHT);
    if tree_height_threshold == 0 {
        tree_height_threshold = i32::MAX;
    }
    let ctx = Ctx { degree_threshold, tree_height_threshold };

    // iterate through all layers, mirroring a ListIterator cursor.
    // `cursor` is the index of the *next* layer to be returned (i.e. one past
    // the current layer once we've called next()).
    let mut cursor = 0usize;
    while cursor < a.graph(graph).layers.len() {
        let lay = a.graph(graph).layers[cursor];
        cursor += 1; // we've now "consumed" this layer; cursor points after it

        // #1 find high degree nodes and their incoming/outgoing trees
        let mut high_degree_nodes: Vec<(LNodeId, HighDegreeNodeInformation)> = Vec::new();
        let mut inc_max = -1i32;
        let mut out_max = -1i32;
        let lay_nodes = a.layer(lay).nodes.clone();
        for n in lay_nodes {
            if is_high_degree_node(a, &ctx, n) {
                let hdni = calculate_information(a, &ctx, n);
                inc_max = inc_max.max(hdni.inc_trees_max_height);
                out_max = out_max.max(hdni.out_trees_max_height);
                high_degree_nodes.push((n, hdni));
            }
        }

        // #2 insert layers before the current layer and move the trees
        // prependLayer inserts a layer right before the current layer; the
        // cursor stays positioned right after the current layer.
        let mut pre_layers: Vec<LayerId> = Vec::new();
        for _ in 0..inc_max {
            let l = a.create_layer(graph);
            // insert at the position of the current layer (cursor-1)
            let insert_at = cursor - 1;
            a.graph_mut(graph).layers.insert(insert_at, l);
            cursor += 1; // current layer shifted right, keep cursor after it
            pre_layers.insert(0, l);
        }
        for (_, hdni) in &high_degree_nodes {
            if let Some(inc_roots) = &hdni.inc_tree_roots {
                for &inc_root in inc_roots {
                    move_tree(a, inc_root, EdgeDir::Incoming, &pre_layers);
                }
            }
        }

        // #2 insert layers after the current layer and move the trees
        // appendLayer inserts a layer at the cursor (after current layer); the
        // cursor moves past it.
        let mut after_layers: Vec<LayerId> = Vec::new();
        for _ in 0..out_max {
            let l = a.create_layer(graph);
            a.graph_mut(graph).layers.insert(cursor, l);
            cursor += 1;
            after_layers.push(l);
        }
        for (_, hdni) in &high_degree_nodes {
            if let Some(out_roots) = &hdni.out_tree_roots {
                for &out_root in out_roots {
                    move_tree(a, out_root, EdgeDir::Outgoing, &after_layers);
                }
            }
        }
    }

    // remove layers that became empty
    let layers = a.graph(graph).layers.clone();
    let mut kept: Vec<LayerId> = Vec::new();
    for l in layers {
        if !a.layer(l).nodes.is_empty() {
            kept.push(l);
        }
    }
    a.graph_mut(graph).layers = kept;

    Ok(())
}

fn is_high_degree_node(a: &LGraphArena, ctx: &Ctx, node: LNodeId) -> bool {
    degree(a, node) >= ctx.degree_threshold
}

fn degree(a: &LGraphArena, node: LNodeId) -> i32 {
    a.node_connected_edges(node).len() as i32
}

fn degree_dir(a: &LGraphArena, node: LNodeId, dir: EdgeDir) -> i32 {
    edges_of(a, node, dir).len() as i32
}

fn calculate_information(a: &LGraphArena, ctx: &Ctx, hdn: LNodeId) -> HighDegreeNodeInformation {
    let mut hdni = HighDegreeNodeInformation {
        inc_trees_max_height: -1,
        out_trees_max_height: -1,
        ..Default::default()
    };

    // incoming trees
    for inc_edge in a.node_incoming_edges(hdn) {
        if a.edge_is_self_loop(inc_edge) {
            continue;
        }
        let src = a.edge_source_node(inc_edge);
        if has_single_connection(a, src, EdgeDir::Outgoing) {
            let tree_height = is_tree_root(a, ctx, src, EdgeDir::Outgoing, EdgeDir::Incoming);
            if tree_height == -1 {
                continue;
            }
            hdni.inc_trees_max_height = hdni.inc_trees_max_height.max(tree_height);
            hdni.inc_tree_roots.get_or_insert_with(Vec::new).push(src);
        }
    }

    // outgoing trees
    for out_edge in a.node_outgoing_edges(hdn) {
        if a.edge_is_self_loop(out_edge) {
            continue;
        }
        let tgt = a.edge_target_node(out_edge);
        if has_single_connection(a, tgt, EdgeDir::Incoming) {
            let tree_height = is_tree_root(a, ctx, tgt, EdgeDir::Incoming, EdgeDir::Outgoing);
            if tree_height == -1 {
                continue;
            }
            hdni.out_trees_max_height = hdni.out_trees_max_height.max(tree_height);
            hdni.out_tree_roots.get_or_insert_with(Vec::new).push(tgt);
        }
    }

    hdni
}

fn move_tree(a: &mut LGraphArena, root: LNodeId, edges_fun: EdgeDir, layers: &[LayerId]) {
    debug_assert!(!layers.is_empty());
    a.node_set_layer(root, Some(layers[0]));

    let sub = &layers[1..];
    for e in edges_of(a, root, edges_fun) {
        let other_node = other(a, e, root);
        move_tree(a, other_node, edges_fun, sub);
    }
}

/// Whether the passed edges connect `node` to a single other node. Allows
/// multiple edges between the two involved nodes.
fn has_single_connection(a: &LGraphArena, node: LNodeId, dir: EdgeDir) -> bool {
    let mut connection: Option<LNodeId> = None;
    for e in edges_of(a, node, dir) {
        let o = other(a, e, node);
        match connection {
            None => connection = Some(o),
            Some(c) => {
                if o != c {
                    return false;
                }
            }
        }
    }
    true
}

/// For an edge (u, v) returns u if node == v and v if node == u.
fn other(a: &LGraphArena, edge: LEdgeId, node: LNodeId) -> LNodeId {
    if a.edge_source_node(edge) == node {
        a.edge_target_node(edge)
    } else {
        a.edge_source_node(edge)
    }
}

fn is_tree_root(
    a: &LGraphArena,
    ctx: &Ctx,
    root: LNodeId,
    ancestor_edges: EdgeDir,
    descendant_edges: EdgeDir,
) -> i32 {
    // exclude high degree nodes themselves
    if is_high_degree_node(a, ctx, root) {
        return -1;
    }
    // exactly one parent?
    if !has_single_connection(a, root, ancestor_edges) {
        return -1;
    }
    // is it a leaf?
    if degree_dir(a, root, descendant_edges) == 0 {
        return 1;
    }

    // recursively check subtrees
    let mut current_height = 0i32;
    for e in edges_of(a, root, descendant_edges) {
        let other_node = other(a, e, root);
        let height = is_tree_root(a, ctx, other_node, ancestor_edges, descendant_edges);
        if height == -1 {
            return -1;
        }
        current_height = current_height.max(height);
        if current_height > ctx.tree_height_threshold - 1 {
            return -1;
        }
    }

    current_height + 1
}
