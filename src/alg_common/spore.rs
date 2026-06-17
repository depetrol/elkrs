//! Ports of `org.eclipse.elk.alg.common.spore`: `Node`,
//! `ScanlineOverlapCheck` (with `compaction.Scanline`) and
//! `DepthFirstCompaction`.

use crate::graph::math::{ElkRectangle, KVector};

use crate::alg_common::elkmath::{distance_segments, fuzzy_compare};
use crate::alg_common::tree::Forest;
use crate::alg_common::utils::get_rect_edges;

/// `InternalProperties.FUZZINESS`.
pub const FUZZINESS: f64 = 0.0001;

/// A rectangle plus center point. Identity is the index into the owning
/// `Vec<Node>`.
#[derive(Clone, Debug)]
pub struct Node {
    /// The original center point from the time of creation.
    pub original_vertex: KVector,
    /// A modifiable center position.
    pub vertex: KVector,
    /// The bounding box of the represented diagram element.
    pub rect: ElkRectangle,
}

impl Node {
    pub fn new(v: KVector, r: ElkRectangle) -> Self {
        Node { original_vertex: v, vertex: v, rect: r }
    }

    pub fn translate(&mut self, v: KVector) {
        self.vertex.add(v);
        self.rect.x += v.x;
        self.rect.y += v.y;
    }

    pub fn set_center_position(&mut self, p: KVector) {
        let mut d = p;
        d.sub(self.vertex);
        self.translate(d);
    }

    pub fn underlap(&self, other: &Node) -> f64 {
        let horizontal_center_distance = (self.rect.center().x - other.rect.center().x).abs();
        let vertical_center_distance = (self.rect.center().y - other.rect.center().y).abs();
        let mut h_scale = 1.0;
        let mut v_scale = 1.0;
        if horizontal_center_distance > self.rect.width / 2.0 + other.rect.width / 2.0 {
            let horizontal_underlap = f64::min(
                (self.rect.x - (other.rect.x + other.rect.width)).abs(),
                (self.rect.x + self.rect.width - other.rect.x).abs(),
            );
            h_scale = 1.0 - horizontal_underlap / horizontal_center_distance;
        }
        if vertical_center_distance > self.rect.height / 2.0 + other.rect.height / 2.0 {
            let vertical_underlap = f64::min(
                (self.rect.y - (other.rect.y + other.rect.height)).abs(),
                (self.rect.y + self.rect.height - other.rect.y).abs(),
            );
            v_scale = 1.0 - vertical_underlap / vertical_center_distance;
        }
        let scale = f64::min(h_scale, v_scale);
        (1.0 - scale)
            * (horizontal_center_distance * horizontal_center_distance
                + vertical_center_distance * vertical_center_distance)
                .sqrt()
    }

    /// How far `other` can be moved in direction `v`
    /// without colliding with this node.
    pub fn distance(&self, other: &Node, v: KVector) -> f64 {
        let mut result = f64::INFINITY;
        for (a1, a2) in get_rect_edges(&self.rect) {
            for (b1, b2) in get_rect_edges(&other.rect) {
                let distance = distance_segments(a1, a2, b1, b2, v);
                result = f64::min(result, distance);
            }
        }
        result
    }

    pub fn touches(&self, other: &Node) -> bool {
        fuzzy_compare(self.rect.x, other.rect.x + other.rect.width, FUZZINESS) <= 0
            && fuzzy_compare(other.rect.x, self.rect.x + self.rect.width, FUZZINESS) <= 0
            && fuzzy_compare(self.rect.y, other.rect.y + other.rect.height, FUZZINESS) <= 0
            && fuzzy_compare(other.rect.y, self.rect.y + self.rect.height, FUZZINESS) <= 0
    }
}

// --------------------------------------------------- ScanlineOverlapCheck

pub fn scanline_overlap_check(nodes: &[Node], mut handler: impl FnMut(usize, usize)) {
    struct Timestamp {
        node: usize,
        low: bool,
    }

    // add all nodes twice (once for the lower, once for the upper border)
    let mut points: Vec<Timestamp> = Vec::new();
    for i in 0..nodes.len() {
        points.push(Timestamp { node: i, low: true });
        points.push(Timestamp { node: i, low: false });
    }

    // Scanline.execute: stable sort by the comparator, then handle in order.
    points.sort_by(|p1, p2| {
        let mut y1 = nodes[p1.node].rect.y;
        if !p1.low {
            y1 += nodes[p1.node].rect.height;
        }
        let mut y2 = nodes[p2.node].rect.y;
        if !p2.low {
            y2 += nodes[p2.node].rect.height;
        }
        let cmp = y1.total_cmp(&y2);
        if cmp == std::cmp::Ordering::Equal {
            if !p1.low && p2.low {
                return std::cmp::Ordering::Less;
            } else if !p2.low && p1.low {
                return std::cmp::Ordering::Greater;
            }
        }
        cmp
    });

    // TreeSet sorted by (rect.x, originalVertex.x, originalVertex.y);
    // kept as a sorted Vec.
    let key = |i: usize| {
        (
            nodes[i].rect.x,
            nodes[i].original_vertex.x,
            nodes[i].original_vertex.y,
        )
    };
    let cmp_key = |a: (f64, f64, f64), b: (f64, f64, f64)| {
        a.0.total_cmp(&b.0)
            .then(a.1.total_cmp(&b.1))
            .then(a.2.total_cmp(&b.2))
    };
    let mut intervals: Vec<usize> = Vec::new();

    let overlap_x = |n1: usize, n2: usize| -> bool {
        if n1 == n2 {
            return false;
        }
        let r1 = &nodes[n1].rect;
        let r2 = &nodes[n2].rect;
        fuzzy_compare(r1.x, r2.x + r2.width, FUZZINESS) < 0
            && fuzzy_compare(r2.x, r1.x + r1.width, FUZZINESS) < 0
    };

    for p in &points {
        if p.low {
            // insert
            let k = key(p.node);
            let pos = intervals
                .binary_search_by(|&i| cmp_key(key(i), k))
                .unwrap_err();
            intervals.insert(pos, p.node);

            let mut overlaps_found = false;
            for &other in intervals.iter() {
                if overlap_x(p.node, other) {
                    handler(p.node, other);
                    overlaps_found = true;
                } else if overlaps_found {
                    break; // sorted: no more overlaps possible
                }
            }
        } else {
            // delete
            let k = key(p.node);
            if let Ok(pos) = intervals.binary_search_by(|&i| cmp_key(key(i), k)) {
                intervals.remove(pos);
            }
        }
    }
}

// --------------------------------------------------- DepthFirstCompaction

pub fn depth_first_compact(tree: &Forest<usize>, nodes: &mut [Node], orthogonal: bool) {
    compact_tree(tree, tree.root, tree.root, nodes, orthogonal);
}

fn compact_tree(
    tree: &Forest<usize>,
    subtree: usize,
    root: usize,
    nodes: &mut [Node],
    orthogonal: bool,
) {
    // first compact the children of the current node
    let children = tree.nodes[subtree].children.clone();
    for &c in &children {
        compact_tree(tree, c, root, nodes, orthogonal);
    }

    // remove underlap between root and its children
    for &child in &children {
        let tree_node = tree.nodes[subtree].value;
        let child_node = tree.nodes[child].value;

        let mut compaction_vector = nodes[tree_node].vertex;
        compaction_vector.sub(nodes[child_node].vertex);

        if orthogonal {
            let rt = nodes[tree_node].rect;
            let rc = nodes[child_node].rect;
            if compaction_vector.x.abs() >= compaction_vector.y.abs() {
                compaction_vector.y = 0.0;
                if rc.y + rc.height > rt.y && rc.y < rt.y + rt.height {
                    compaction_vector.scale_to_length(f64::max(
                        rt.x - (rc.x + rc.width),
                        rc.x - (rt.x + rt.width),
                    ));
                }
            } else {
                compaction_vector.x = 0.0;
                if rc.x + rc.width > rt.x && rc.x < rt.x + rt.width {
                    compaction_vector.scale_to_length(f64::max(
                        rt.y - (rc.y + rc.height),
                        rc.y - (rt.y + rt.height),
                    ));
                }
            }
        } else {
            let underlap = nodes[tree_node].underlap(&nodes[child_node]);
            compaction_vector.scale_to_length(underlap);
        }

        let min_underlap = compaction_vector.length();
        let min_underlap = get_min_underlap(tree, root, child, min_underlap, compaction_vector, nodes);

        compaction_vector.scale_to_length(min_underlap);
        translate_subtree(tree, child, compaction_vector, nodes);
    }
}

fn get_min_underlap(
    tree: &Forest<usize>,
    t: usize,
    child: usize,
    current_min_underlap: f64,
    compaction_vector: KVector,
    nodes: &[Node],
) -> f64 {
    let mut min_underlap = f64::min(
        current_min_underlap,
        min_underlap_with_subtree(
            tree,
            tree.nodes[t].value,
            child,
            current_min_underlap,
            compaction_vector,
            nodes,
        ),
    );

    for &c in &tree.nodes[t].children {
        if c != child {
            min_underlap = f64::min(
                min_underlap,
                get_min_underlap(tree, c, child, min_underlap, compaction_vector, nodes),
            );
        }
    }

    min_underlap
}

fn min_underlap_with_subtree(
    tree: &Forest<usize>,
    r: usize, // node index (into nodes) of the reference node
    t: usize, // tree index of the subtree
    current_min_underlap: f64,
    compaction_vector: KVector,
    nodes: &[Node],
) -> f64 {
    let mut min_underlap = current_min_underlap;
    for &child in &tree.nodes[t].children {
        let c = tree.nodes[child].value;
        let rn = &nodes[r];
        let cn = &nodes[c];
        if rn.touches(cn) {
            // if they already touch, find out if they would be moved into each other
            if (fuzzy_compare(cn.rect.x, rn.rect.x + rn.rect.width, FUZZINESS) == 0
                && compaction_vector.x < 0.0)
                || (fuzzy_compare(cn.rect.x + cn.rect.width, rn.rect.x, FUZZINESS) == 0
                    && compaction_vector.x > 0.0)
                || (fuzzy_compare(cn.rect.y, rn.rect.y + rn.rect.height, FUZZINESS) == 0
                    && compaction_vector.y < 0.0)
                || (fuzzy_compare(cn.rect.y + cn.rect.height, rn.rect.y, FUZZINESS) == 0
                    && compaction_vector.y > 0.0)
            {
                min_underlap = 0.0;
                break;
            }
        } else {
            min_underlap = f64::min(min_underlap, rn.distance(cn, compaction_vector));
        }

        min_underlap = f64::min(
            min_underlap,
            min_underlap_with_subtree(tree, r, child, min_underlap, compaction_vector, nodes),
        );
    }

    min_underlap
}

fn translate_subtree(tree: &Forest<usize>, t: usize, compaction_vector: KVector, nodes: &mut [Node]) {
    nodes[tree.nodes[t].value].translate(compaction_vector);
    for &c in &tree.nodes[t].children {
        translate_subtree(tree, c, compaction_vector, nodes);
    }
}
