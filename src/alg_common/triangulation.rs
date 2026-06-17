//! Ports of `org.eclipse.elk.alg.common.TEdge`, `TTriangle`,
//! `BowyerWatsonTriangulation` and `NaiveMinST`.
//!
//! All hash sets whose iteration order leaks into results are modeled
//! with [`JavaHashSet`]; vertex identity is by exact coordinate value.

use crate::graph::math::KVector;

use crate::alg_common::elkmath::fuzzy_compare;
use crate::alg_common::jhash::{java_kvector_hash, JHashEq, JavaHashSet};
use crate::alg_common::tree::Forest;

/// `InternalProperties.FUZZINESS`.
pub const FUZZINESS: f64 = 0.0001;

/// Exact coordinate comparison of two vertices.
pub fn kv_eq(a: KVector, b: KVector) -> bool {
    a.x == b.x && a.y == b.y
}

/// An undirected edge between two vertices.
#[derive(Clone, Copy, Debug)]
pub struct TEdge {
    pub u: KVector,
    pub v: KVector,
}

impl TEdge {
    pub fn new(u: KVector, v: KVector) -> Self {
        TEdge { u, v }
    }
}

impl JHashEq for TEdge {
    fn jhash(&self) -> i32 {
        java_kvector_hash(self.u.x, self.u.y).wrapping_add(java_kvector_hash(self.v.x, self.v.y))
    }
    fn jeq(&self, other: &Self) -> bool {
        (kv_eq(self.u, other.u) && kv_eq(self.v, other.v))
            || (kv_eq(self.u, other.v) && kv_eq(self.v, other.u))
    }
}

#[derive(Clone, Copy)]
pub struct TTriangle {
    pub a: KVector,
    pub b: KVector,
    pub c: KVector,
    circumcenter: KVector,
}

impl TTriangle {
    pub fn new(a: KVector, b: KVector, c: KVector) -> Self {
        let mut t = TTriangle { a, b, c, circumcenter: KVector::default() };
        t.circumcenter = t.calculate_circumcenter();
        t
    }

    pub fn t_edges(&self) -> [TEdge; 3] {
        [
            TEdge::new(self.a, self.b),
            TEdge::new(self.b, self.c),
            TEdge::new(self.c, self.a),
        ]
    }

    fn calculate_circumcenter(&self) -> KVector {
        let (a, b, c) = (self.a, self.b, self.c);
        let mut ab = b;
        ab.sub(a);
        let mut ac = c;
        ac.sub(a);
        let mut bc = c;
        bc.sub(b);
        let e = ab.x * (a.x + b.x) + ab.y * (a.y + b.y);
        let f = ac.x * (a.x + c.x) + ac.y * (a.y + c.y);
        let g = 2.0 * (ab.x * bc.y - ab.y * bc.x);

        let px = (ac.y * e - ab.y * f) / g;
        let py = (ab.x * f - ac.x * e) / g;
        KVector::new(px, py)
    }

    pub fn in_circumcircle(&self, v: KVector) -> bool {
        fuzzy_compare(
            self.circumcenter.distance(v),
            self.circumcenter.distance(self.a),
            FUZZINESS,
        ) < 0
    }

    pub fn contains_edge(&self, e: &TEdge) -> bool {
        self.t_edges().iter().any(|te| te.jeq(e))
    }

    pub fn contains_vertex(&self, v: KVector) -> bool {
        kv_eq(v, self.a) || kv_eq(v, self.b) || kv_eq(v, self.c)
    }
}

impl JHashEq for TTriangle {
    fn jhash(&self) -> i32 {
        java_kvector_hash(self.a.x, self.a.y)
            .wrapping_add(java_kvector_hash(self.b.x, self.b.y))
            .wrapping_add(java_kvector_hash(self.c.x, self.c.y))
    }
    fn jeq(&self, other: &Self) -> bool {
        self.contains_vertex(other.a) && self.contains_vertex(other.b) && self.contains_vertex(other.c)
    }
}

pub fn bowyer_watson_triangulate(vertices: &[KVector]) -> JavaHashSet<TEdge> {
    // bounding box
    let mut topleft = KVector::new(f64::INFINITY, f64::INFINITY);
    let mut bottomright = KVector::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for v in vertices {
        topleft.x = f64::min(topleft.x, v.x);
        topleft.y = f64::min(topleft.y, v.y);
        bottomright.x = f64::max(bottomright.x, v.x);
        bottomright.y = f64::max(bottomright.y, v.y);
    }
    let size = KVector::new(bottomright.x - topleft.x, bottomright.y - topleft.y);

    // super-triangle
    let wiggleroom = 50.0;
    let sa = KVector::new(topleft.x - wiggleroom, topleft.y - size.x - wiggleroom);
    let sb = KVector::new(topleft.x - wiggleroom, bottomright.y + size.x + wiggleroom);
    let sc = KVector::new(bottomright.x + size.y / 2.0 + wiggleroom, topleft.y + size.y / 2.0);
    let super_triangle = TTriangle::new(sa, sb, sc);

    let mut triangulation: JavaHashSet<TTriangle> = JavaHashSet::new();
    let mut invalid_triangles: Vec<TTriangle> = Vec::new();
    let mut boundary: Vec<TEdge> = Vec::new();
    triangulation.add(super_triangle);

    for &vertex in vertices {
        // gather invalid triangles
        invalid_triangles.clear();
        for triangle in triangulation.iter() {
            if triangle.in_circumcircle(vertex) {
                invalid_triangles.push(*triangle);
            }
        }

        // boundary of invalid triangles
        boundary.clear();
        for triangle in &invalid_triangles {
            for t_edge in triangle.t_edges() {
                let mut on_boundary = true;
                for other in &invalid_triangles {
                    // Comparison is by object identity (other != triangle);
                    // the list never contains duplicate triangles, so value
                    // inequality is equivalent here.
                    if !std::ptr::eq(other, triangle) && other.contains_edge(&t_edge) {
                        on_boundary = false;
                    }
                }
                if on_boundary {
                    boundary.push(t_edge);
                }
            }
        }

        // remove invalid triangles
        for t in &invalid_triangles {
            triangulation.remove(t);
        }

        // triangulate boundary
        for t_edge in &boundary {
            triangulation.add(TTriangle::new(vertex, t_edge.u, t_edge.v));
        }
    }

    // collect edges
    let mut t_edges: JavaHashSet<TEdge> = JavaHashSet::new();
    for triangle in triangulation.iter() {
        for e in triangle.t_edges() {
            t_edges.add(e);
        }
    }

    // remove edges connected to the super triangle
    let to_remove: Vec<TEdge> = t_edges
        .iter()
        .filter(|e| super_triangle.contains_vertex(e.u) || super_triangle.contains_vertex(e.v))
        .copied()
        .collect();
    for e in &to_remove {
        t_edges.remove(e);
    }

    t_edges
}

fn kv_bits(v: KVector) -> (u64, u64) {
    (v.x.to_bits(), v.y.to_bits())
}

pub fn naive_min_st(
    t_edges: &JavaHashSet<TEdge>,
    root: KVector,
    mut cost: impl FnMut(&TEdge) -> f64,
) -> Forest<KVector> {
    // determine edge weights, in set iteration order
    let mut edge_list: Vec<(TEdge, f64)> = t_edges.iter().map(|e| (*e, cost(e))).collect();

    // sort edges by weight (stable, using total_cmp)
    edge_list.sort_by(|a, b| a.1.total_cmp(&b.1));

    // LinkedHashSet preserving order; entries removed as they are used.
    let mut removed = vec![false; edge_list.len()];

    let mut min_st = Forest::new(root);
    let mut tree_nodes: std::collections::HashMap<(u64, u64), usize> =
        std::collections::HashMap::new();
    tree_nodes.insert(kv_bits(root), min_st.root);

    let mut remaining = edge_list.len();
    while remaining > 0 {
        let mut next: Option<(usize, KVector, KVector)> = None; // (edge idx, nextNode, nodeInTree)
        for (i, (edge, weight)) in edge_list.iter().enumerate() {
            if removed[i] {
                continue;
            }
            if weight.is_nan() {
                continue;
            }
            if tree_nodes.contains_key(&kv_bits(edge.u)) && !tree_nodes.contains_key(&kv_bits(edge.v)) {
                next = Some((i, edge.v, edge.u));
                break;
            }
            if tree_nodes.contains_key(&kv_bits(edge.v)) && !tree_nodes.contains_key(&kv_bits(edge.u)) {
                next = Some((i, edge.u, edge.v));
                break;
            }
        }

        let Some((edge_idx, next_node, node_in_tree)) = next else {
            break;
        };

        let parent = tree_nodes[&kv_bits(node_in_tree)];
        let sub_tree = min_st.add_child(parent, next_node);
        tree_nodes.insert(kv_bits(next_node), sub_tree);
        removed[edge_idx] = true;
        remaining -= 1;
    }

    min_st
}
