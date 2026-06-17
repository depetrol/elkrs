//! The
//! node-positioning algorithm for general trees by John Q. Walker II.

use crate::core::options_gen::Direction;

use crate::alg_mrtree::graph::{TArena, TGraph, TNodeId};
use crate::alg_mrtree::options;
use crate::alg_mrtree::tree_util;

/// Bit-exact port of `java.lang.Math.round(double)` (returns `long`).
pub fn java_math_round(a: f64) -> i64 {
    const SIGNIFICAND_WIDTH: i64 = 53;
    const EXP_BIAS: i64 = 1023;
    const EXP_BIT_MASK: i64 = 0x7FF0_0000_0000_0000;
    const SIGNIF_BIT_MASK: i64 = 0x000F_FFFF_FFFF_FFFF;

    let long_bits = a.to_bits() as i64;
    let biased_exp = (long_bits & EXP_BIT_MASK) >> (SIGNIFICAND_WIDTH - 1);
    let shift = (SIGNIFICAND_WIDTH - 2 + EXP_BIAS) - biased_exp;
    if (shift & -64) == 0 {
        // shift >= 0 && shift < 64: a is a finite number such that pow(2,-64) <= ulp(a) < 1
        let mut r = (long_bits & SIGNIF_BIT_MASK) | (SIGNIF_BIT_MASK + 1);
        if long_bits < 0 {
            r = -r;
        }
        ((r >> shift) + 1) >> 1
    } else {
        // a is either:
        // - a finite number with abs(a) < exp(2,SIGNIFICAND_WIDTH-64) < 1/2
        // - a finite number with ulp(a) >= 1 and hence is a mathematical integer
        // - an infinity or NaN
        a as i64
    }
}

struct NodePlacer {
    spacing: f64,
    direction: Direction,
    /// `xTopAdjustment` / `yTopAdjustment` (always 0).
    x_top_adjustment: f64,
    y_top_adjustment: f64,
}

pub fn process(arena: &mut TArena, graph: &mut TGraph) {
    // set the settings according to the user inputs
    let spacing: f64 = graph.properties.get(&options::SPACING_NODE_NODE);
    let mut direction: Direction = graph.properties.get(&options::DIRECTION);

    // set direction to DOWN if it is UNDEFINED
    if direction == Direction::UNDEFINED {
        direction = Direction::DOWN;
        graph.properties.set(&options::DIRECTION, direction);
    }

    // find the root node of this component
    let roots: Vec<TNodeId> =
        graph.nodes.iter().copied().filter(|&n| arena.node(n).root).collect();
    let root = *roots.first().expect("NodePlacer: no root");

    let placer =
        NodePlacer { spacing, direction, x_top_adjustment: 0.0, y_top_adjustment: 0.0 };

    // do the preliminary positioning with a postorder walk
    placer.first_walk(arena, root, 0);

    // do the final positioning with a preorder walk
    placer.second_walk(
        arena,
        Some(root),
        placer.y_top_adjustment - (arena.node(root).level_height / 2.0),
        placer.x_top_adjustment,
    );
}

impl NodePlacer {
    fn first_walk(&self, arena: &mut TArena, c_n: TNodeId, level: i32) {
        arena.node_mut(c_n).modifier = 0.0;
        let l_s = arena.node(c_n).left_sibling;

        if arena.is_leaf(c_n) {
            if let Some(l_s) = l_s {
                // preliminary x-coordinate based on the left sibling, the
                // separation, and the mean size of both nodes
                let p = arena.node(l_s).prelim + self.spacing + self.mean_node_width(arena, Some(l_s), Some(c_n));
                arena.node_mut(c_n).prelim = p;
            } else {
                // no sibling on the left to worry about
                arena.node_mut(c_n).prelim = 0.0;
            }
        } else {
            // this node is not a leaf, so recurse for each of its offspring
            for child in arena.children(c_n) {
                self.first_walk(arena, child, level + 1);
            }

            let children = arena.children(c_n);
            let l_m = *children.first().unwrap();
            let r_m = *children.last().unwrap();
            let mid_point = (arena.node(r_m).prelim + arena.node(l_m).prelim) / 2.0;

            if let Some(l_s) = l_s {
                // this node has a left sibling, so its offspring must be
                // shifted to the right
                let p = arena.node(l_s).prelim + self.spacing + self.mean_node_width(arena, Some(l_s), Some(c_n));
                arena.node_mut(c_n).prelim = p;
                arena.node_mut(c_n).modifier = arena.node(c_n).prelim - mid_point;
                // shift the offspring of this node to the right
                self.apportion(arena, c_n, level);
            } else {
                // no sibling on the left to worry about
                arena.node_mut(c_n).prelim = mid_point;
            }
        }
    }

    fn apportion(&self, arena: &mut TArena, c_n: TNodeId, _level: i32) {
        // initialize the leftmost and neighbor corresponding to the root of
        // the subtree
        let mut leftmost: Option<TNodeId> = arena.children(c_n).first().copied();
        let mut neighbor: Option<TNodeId> =
            leftmost.and_then(|l| arena.node(l).left_neighbor);
        let mut compare_depth = 1;

        while let (Some(lm), Some(nb)) = (leftmost, neighbor) {
            // compute the location of leftmost and where it should be with
            // respect to neighbor
            let mut left_mod_sum = 0.0f64;
            let mut right_mod_sum = 0.0f64;
            let mut ancestor_leftmost = lm;
            let mut ancestor_neighbor = nb;
            for _ in 0..compare_depth {
                ancestor_leftmost =
                    arena.parent(ancestor_leftmost).expect("apportion: missing ancestor");
                ancestor_neighbor =
                    arena.parent(ancestor_neighbor).expect("apportion: missing ancestor");
                right_mod_sum += arena.node(ancestor_leftmost).modifier;
                left_mod_sum += arena.node(ancestor_neighbor).modifier;
            }

            // find the move distance and apply it to the node's subtree
            let pr_n = arena.node(nb).prelim;
            let pr_l = arena.node(lm).prelim;
            let mean = self.mean_node_width(arena, Some(lm), Some(nb));
            let mut move_distance = pr_n + left_mod_sum + self.spacing + mean - pr_l - right_mod_sum;

            if 0.0 < move_distance {
                // count interior sibling subtrees in leftSiblings
                let mut left_sibling = Some(c_n);
                let mut left_siblings = 0i32;
                while let Some(ls) = left_sibling {
                    if ls == ancestor_neighbor {
                        break;
                    }
                    left_siblings += 1;
                    left_sibling = arena.node(ls).left_sibling;
                }
                // apply portions to appropriate left sibling subtrees
                if left_sibling.is_some() {
                    let portion = move_distance / left_siblings as f64;
                    let mut left_sibling = c_n;
                    while left_sibling != ancestor_neighbor {
                        let new_pr = arena.node(left_sibling).prelim + move_distance;
                        arena.node_mut(left_sibling).prelim = new_pr;
                        let new_mod = arena.node(left_sibling).modifier + move_distance;
                        arena.node_mut(left_sibling).modifier = new_mod;
                        move_distance -= portion;
                        left_sibling = match arena.node(left_sibling).left_sibling {
                            Some(ls) => ls,
                            None => break, // unreachable since the loop above terminated at ancestorNeighbor
                        };
                    }
                } else {
                    // it needs to be done by an ancestor because
                    // ancestorNeighbor and ancestorLeftmost are not siblings
                    return;
                }
            }

            // determine the leftmost descendant of the node at the next
            // lower level to compare its positioning against its neighbor
            compare_depth += 1;
            if arena.is_leaf(lm) {
                let children = arena.children(c_n);
                leftmost = tree_util::get_left_most_depth(arena, &children, compare_depth);
            } else {
                leftmost = arena.children(lm).first().copied();
            }
            neighbor = leftmost.and_then(|l| arena.node(l).left_neighbor);
        }
    }

    fn mean_node_width(
        &self,
        arena: &TArena,
        left_node: Option<TNodeId>,
        right_node: Option<TNodeId>,
    ) -> f64 {
        let mut node_width = 0.0f64;
        if let Some(left) = left_node {
            node_width += if self.direction.is_vertical() {
                arena.node(left).size.x / 2.0
            } else {
                arena.node(left).size.y / 2.0
            };
        }
        if let Some(right) = right_node {
            node_width += if self.direction.is_vertical() {
                arena.node(right).size.x / 2.0
            } else {
                arena.node(right).size.y / 2.0
            };
        }
        node_width
    }

    fn second_walk(&self, arena: &mut TArena, t_node: Option<TNodeId>, y_coor: f64, modsum: f64) {
        if let Some(t_node) = t_node {
            // the x-position is the sum of the preliminary coordinate and
            // the modifiers of all the node's ancestors
            let x_temp = arena.node(t_node).prelim + modsum;
            let y_temp = y_coor + (arena.node(t_node).level_height / 2.0);
            arena.node_mut(t_node).xcoor = java_math_round(x_temp) as i32;
            arena.node_mut(t_node).ycoor = java_math_round(y_temp) as i32;

            // apply the modifier value for this node to all its offspring
            if !arena.is_leaf(t_node) {
                self.second_walk(
                    arena,
                    arena.children(t_node).first().copied(),
                    y_coor + arena.node(t_node).level_height + self.spacing,
                    modsum + arena.node(t_node).modifier,
                );
            }
            // go ahead with the sibling to the right
            if arena.node(t_node).right_sibling.is_some() {
                self.second_walk(arena, arena.node(t_node).right_sibling, y_coor, modsum);
            }
        }
    }
}
