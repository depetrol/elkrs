//! Orders the nodes
//! of each level. (The alternative `OrderBalance` phase is unreachable —
//! `TreeLayoutPhases.create()` always instantiates `NodeOrderer` — and
//! is not ported.)

use crate::alg_mrtree::graph::{TArena, TEdgeId, TGraph, TNodeId};
use crate::alg_mrtree::options;
use crate::alg_mrtree::options::OrderWeighting;

pub fn process(arena: &mut TArena, graph: &TGraph) {
    // find the root node, assuming that: 1. there is a root, 2. only one
    let root = graph
        .nodes
        .iter()
        .copied()
        .find(|&x| arena.node(x).root)
        .expect("NodeOrderer: no root");

    let weighting: OrderWeighting = graph.properties.get(&options::WEIGHTING);
    if weighting == OrderWeighting::FAN || weighting == OrderWeighting::DESCENDANTS {
        order_level_fan_descendants(arena, &mut vec![root], weighting);
    } else if weighting == OrderWeighting::CONSTRAINT {
        order_level_constraint(arena, vec![root]);
    }
    // MODEL_ORDER: no reordering
}

/// The sort property accessor for FAN / DESCENDANTS weighting.
fn sort_property(arena: &TArena, n: TNodeId, weighting: OrderWeighting) -> i32 {
    if weighting == OrderWeighting::DESCENDANTS {
        arena.node(n).descendants
    } else {
        arena.node(n).fan
    }
}

/// The level list is sorted
/// in place (the caller's `children` list is passed, whose sorted state is
/// visible to the caller's subsequent position sort).
fn order_level_fan_descendants(
    arena: &mut TArena,
    current_level: &mut Vec<TNodeId>,
    weighting: OrderWeighting,
) {
    let mut pos = 0;

    // sort all nodes in this level by their fan out
    // (PropertyHolderComparator: ascending, stable)
    current_level.sort_by_key(|&n| sort_property(arena, n, weighting));

    // find the first occurrence of a leaf in the list, searching backwards
    let mut first_occ = current_level.len();
    for &tnode in current_level.iter().rev() {
        if sort_property(arena, tnode, weighting) == 0 {
            first_occ -= 1;
        } else {
            break;
        }
    }

    // separate the level into leaves and inner nodes
    let inners: Vec<TNodeId> = current_level[..first_occ].to_vec();
    let mut leaves: Vec<TNodeId> = current_level[first_occ..].to_vec();

    if inners.is_empty() {
        // leave the leaves in their order
        for &tnode in &leaves {
            arena.node_mut(tnode).position = pos;
            pos += 1;
        }
    } else {
        // order each level of descendants of the inner nodes
        for &tpnode in &inners {
            arena.node_mut(tpnode).position = pos;
            pos += 1;

            // set the position of the children and set them in order
            let mut children = arena.children(tpnode);
            order_level_fan_descendants(arena, &mut children, weighting);

            // order the children by their reverse position (stable)
            children.sort_by(|&a, &b| arena.node(b).position.cmp(&arena.node(a).position));

            // reset the list of children with the new order
            let mut sorted_out_edges: Vec<TEdgeId> = Vec::new();
            for &tnode in &children {
                for &tedge in &arena.node(tpnode).outgoing {
                    if arena.edge(tedge).target == tnode {
                        sorted_out_edges.push(tedge);
                    }
                }
            }
            arena.node_mut(tpnode).outgoing = sorted_out_edges;

            // fill gaps with leaves, taken from the end of the leaves list
            let mut fill_gap = arena.node(tpnode).outgoing.len() as i32;
            while fill_gap > 0 && !leaves.is_empty() {
                let tnode = *leaves.last().unwrap();
                if sort_property(arena, tnode, weighting) == 0 {
                    arena.node_mut(tnode).position = pos;
                    pos += 1;
                    fill_gap -= 1;
                    leaves.pop();
                } else {
                    break;
                }
            }
        }
    }
}

fn order_level_constraint(arena: &mut TArena, current_level: Vec<TNodeId>) {
    let constraint =
        |arena: &TArena, n: TNodeId| -> i32 { arena.node(n).properties.get(&options::POSITION_CONSTRAINT) };

    let level_size = current_level.len() as i32;
    let mut undefined_nodes: Vec<TNodeId> = current_level
        .iter()
        .copied()
        .filter(|&x| constraint(arena, x) < 0)
        .collect();
    let mut in_bound_nodes: Vec<TNodeId> = current_level
        .iter()
        .copied()
        .filter(|&x| constraint(arena, x) < level_size && constraint(arena, x) >= 0)
        .collect();
    let mut out_of_bound_nodes: Vec<TNodeId> = current_level
        .iter()
        .copied()
        .filter(|&x| constraint(arena, x) >= level_size)
        .collect();

    let mut sorted_nodes: Vec<Option<TNodeId>> = vec![None; current_level.len()];

    // Priority 1: set non duplicate constraints (note: the upper bound is
    // the *shrinking* inBoundNodes list size)
    let mut i = 0usize;
    while i < in_bound_nodes.len() {
        let cur_node = in_bound_nodes[i];
        let target_pos = constraint(arena, cur_node);
        if target_pos >= 0
            && (target_pos as usize) < in_bound_nodes.len()
            && sorted_nodes[target_pos as usize].is_none()
        {
            sorted_nodes[target_pos as usize] = Some(cur_node);
            in_bound_nodes.remove(i);
        } else {
            i += 1;
        }
    }
    // Priority 2: set duplicate constraints (the `i--` after each
    // placement keeps the index at 0 while the list shrinks)
    let i = 0usize;
    while i < in_bound_nodes.len() {
        let cur_node = in_bound_nodes[i];
        let target_pos = constraint(arena, cur_node) as i64;
        let mut j: i64 = 0;
        loop {
            let new_target_pos = target_pos + j;
            if new_target_pos < sorted_nodes.len() as i64
                && new_target_pos >= 0
                && sorted_nodes[new_target_pos as usize].is_none()
            {
                sorted_nodes[new_target_pos as usize] = Some(cur_node);
                in_bound_nodes.remove(i);
                break;
            }
            let new_target_pos = target_pos - j;
            if new_target_pos < sorted_nodes.len() as i64
                && new_target_pos >= 0
                && sorted_nodes[new_target_pos as usize].is_none()
            {
                sorted_nodes[new_target_pos as usize] = Some(cur_node);
                in_bound_nodes.remove(i);
                break;
            }
            j += 1;
        }
    }
    // Priority 3: set out of bounds constraints (descending, stable)
    out_of_bound_nodes.sort_by(|&x, &y| constraint(arena, y).cmp(&constraint(arena, x)));
    for i in (0..sorted_nodes.len()).rev() {
        if sorted_nodes[i].is_none() && !out_of_bound_nodes.is_empty() {
            sorted_nodes[i] = Some(out_of_bound_nodes.remove(0));
        }
    }
    // Priority 4: set no constraint nodes
    for slot in sorted_nodes.iter_mut() {
        if slot.is_none() && !undefined_nodes.is_empty() {
            *slot = Some(undefined_nodes.remove(0));
        }
    }

    // set final node positions
    for (i, slot) in sorted_nodes.iter().enumerate() {
        let n = slot.expect("orderLevelConstraint: unassigned slot");
        arena.node_mut(n).position = i as i32;
    }

    // recursive calls, apply node positions
    let inners: Vec<TNodeId> = current_level
        .iter()
        .copied()
        .filter(|&x| arena.node(x).fan != 0)
        .collect();
    for &tpnode in &inners {
        // recursive call
        let mut children = arena.children(tpnode);
        order_level_constraint(arena, children.clone());

        // order the children by their position (ascending, stable)
        children.sort_by(|&a, &b| arena.node(a).position.cmp(&arena.node(b).position));

        // reset the list of children with the new order
        let mut sorted_out_edges: Vec<TEdgeId> = Vec::new();
        for &tnode in &children {
            for &tedge in &arena.node(tpnode).outgoing {
                if arena.edge(tedge).target == tnode {
                    sorted_out_edges.push(tedge);
                }
            }
        }
        arena.node_mut(tpnode).outgoing = sorted_out_edges;
    }
}
