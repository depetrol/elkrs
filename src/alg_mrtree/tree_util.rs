
use crate::core::options_gen::Direction;
use crate::graph::math::KVector;

use crate::alg_mrtree::graph::{integer_ref_neq, TArena, TEdgeId, TGraph, TNodeId};
use crate::alg_mrtree::options;

pub fn get_root(arena: &TArena, graph: &TGraph) -> TNodeId {
    *graph
        .nodes
        .iter()
        .find(|&&n| arena.node(n).root)
        .expect("TreeUtil.getRoot: no root in graph")
}

/// Outgoing edge targets, `distinct()`.
pub fn get_children(arena: &TArena, n: TNodeId) -> Vec<TNodeId> {
    let mut re: Vec<TNodeId> = Vec::new();
    for &out in &arena.node(n).outgoing {
        let t = arena.edge(out).target;
        if !re.contains(&t) {
            re.push(t);
        }
    }
    re
}

/// All graph edges into `n` (matched
/// by the `id` field), excluding same-level edges (with boxed
/// `Integer !=` semantics) and edges whose `toString` duplicates a previous
/// match; sorted by source x position.
pub fn get_all_incoming_edges(arena: &TArena, n: TNodeId, graph: &TGraph) -> Vec<TEdgeId> {
    let mut re: Vec<TEdgeId> = Vec::new();
    let n_id = arena.node(n).id;
    for &e in &graph.edges {
        let edge = arena.edge(e);
        if arena.node(edge.target).id == n_id
            && integer_ref_neq(arena.tree_level(edge.source), arena.tree_level(edge.target))
            && !re.iter().any(|&x| arena.edge_string(x) == arena.edge_string(e))
        {
            re.push(e);
        }
    }
    re.sort_by(|&x, &y| {
        let xs = arena.node(arena.edge(x).source).pos.x;
        let ys = arena.node(arena.edge(y).source).pos.x;
        xs.total_cmp(&ys)
    });
    re
}

/// All graph edges out of `n`
/// (matched by the `id` field, excluding the SUPER_ROOT by label),
/// excluding same-level edges and `toString` duplicates; sorted by target x.
pub fn get_all_outgoing_edges(arena: &TArena, n: TNodeId, graph: &TGraph) -> Vec<TEdgeId> {
    let mut re: Vec<TEdgeId> = Vec::new();
    let n_id = arena.node(n).id;
    for &e in &graph.edges {
        let edge = arena.edge(e);
        if arena.node(edge.source).id == n_id
            && arena.node(edge.source).label != "SUPER_ROOT"
            && integer_ref_neq(arena.tree_level(edge.source), arena.tree_level(edge.target))
            && !re.iter().any(|&x| arena.edge_string(x) == arena.edge_string(e))
        {
            re.push(e);
        }
    }
    re.sort_by(|&x, &y| {
        let xt = arena.node(arena.edge(x).target).pos.x;
        let yt = arena.node(arena.edge(y).target).pos.x;
        xt.total_cmp(&yt)
    });
    re
}

pub fn get_first_point(arena: &TArena, e: TEdgeId) -> KVector {
    let edge = arena.edge(e);
    if edge.bend_points.is_empty() {
        arena.node(edge.target).pos
    } else {
        edge.bend_points.first()
    }
}

pub fn get_last_point(arena: &TArena, e: TEdgeId) -> KVector {
    let edge = arena.edge(e);
    if edge.bend_points.is_empty() {
        arena.node(edge.source).pos
    } else {
        edge.bend_points.last()
    }
}

pub fn get_direction(graph: &TGraph) -> Direction {
    graph.properties.get(&options::DIRECTION)
}

pub fn get_direction_vector(d: Direction) -> KVector {
    match d {
        Direction::UP => KVector::new(0.0, -1.0),
        Direction::RIGHT => KVector::new(1.0, 0.0),
        Direction::LEFT => KVector::new(-1.0, 0.0),
        _ => KVector::new(0.0, 1.0),
    }
}

pub fn to_node_border(center: &mut KVector, next: KVector, size: KVector) {
    let wh = size.x / 2.0;
    let hh = size.y / 2.0;
    let absx = (next.x - center.x).abs();
    let absy = (next.y - center.y).abs();
    let mut xscale = 1.0;
    let mut yscale = 1.0;
    if absx > wh {
        xscale = wh / absx;
    }
    if absy > hh {
        yscale = hh / absy;
    }
    let scale = f64::min(xscale, yscale);
    center.x += scale * (next.x - center.x);
    center.y += scale * (next.y - center.y);
}

pub fn is_cycle_inducing(arena: &TArena, e: TEdgeId, graph: &TGraph) -> bool {
    let edge = arena.edge(e);
    let delta = KVector::new(
        arena.node(edge.target).pos.x - arena.node(edge.source).pos.x,
        arena.node(edge.target).pos.y - arena.node(edge.source).pos.y,
    );
    get_direction_vector(get_direction(graph)).dot_product(delta) <= 0.0
}

pub fn get_unique_long(a: i32, b: i32) -> i64 {
    ((a as i64) << 32) | (b as i64 & 0xFFFF_FFFF)
}

pub fn get_lowest_parent(arena: &TArena, n: TNodeId, graph: &TGraph) -> Option<TNodeId> {
    let dir_vec = get_direction_vector(get_direction(graph));
    if arena.node(n).incoming.is_empty() {
        return None;
    }
    let sources: Vec<TNodeId> =
        arena.node(n).incoming.iter().map(|&e| arena.edge(e).source).collect();
    let parents: Vec<TNodeId> =
        graph.nodes.iter().copied().filter(|x| sources.contains(x)).collect();
    let key = |x: TNodeId| {
        let node = arena.node(x);
        KVector::new(node.pos.x + node.size.x / 2.0, node.pos.y + node.size.y / 2.0)
            .dot_product(dir_vec)
    };
    let lowest_parent_pos = parents
        .iter()
        .map(|&x| key(x))
        .max_by(|a, b| a.total_cmp(b))
        .expect("getLowestParent: no parents");
    parents.iter().copied().find(|&x| key(x) == lowest_parent_pos)
}

pub fn get_left_most(arena: &TArena, currentlevel: &[TNodeId]) -> Option<TNodeId> {
    get_left_most_depth(arena, currentlevel, -1)
}

pub fn get_left_most_depth(
    arena: &TArena,
    currentlevel: &[TNodeId],
    depth: i32,
) -> Option<TNodeId> {
    if !currentlevel.is_empty() {
        let d = depth;

        // the leftmost descendant at depth levels down
        if 1 < d {
            let next_level: Vec<TNodeId> =
                currentlevel.iter().flat_map(|&c| arena.children(c)).collect();
            return get_left_most_depth(arena, &next_level, d - 1);
        }

        // the leftmost node at the deepest level
        if d < 0 {
            let next_level: Vec<TNodeId> =
                currentlevel.iter().flat_map(|&c| arena.children(c)).collect();
            if !next_level.is_empty() {
                return get_left_most_depth(arena, &next_level, d);
            }
        }
    }
    // return the leftmost node at the current level
    currentlevel.first().copied()
}
