
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::math::KVector;

use crate::alg_radial::options::{RadialTranslationStrategy, POSITION};
use crate::alg_radial::p2routing::clip_vector;
use crate::alg_radial::util;

impl RadialTranslationStrategy {
    pub fn create(self) -> Option<RadialTranslationStrategy> {
        match self {
            RadialTranslationStrategy::NONE => None,
            other => Some(other),
        }
    }

    pub fn evaluate(self, g: &ElkGraph, root: NodeId) -> f64 {
        match self {
            RadialTranslationStrategy::NONE => unreachable!(),
            RadialTranslationStrategy::EDGE_LENGTH => edge_length(g, root),
            RadialTranslationStrategy::EDGE_LENGTH_BY_POSITION => edge_length_position(g, root),
            RadialTranslationStrategy::CROSSING_MINIMIZATION_BY_POSITION => {
                crossing_minimization(g, root)
            }
        }
    }
}

/// Sum of the clipped length of
/// each outgoing edge of the root.
fn edge_length(g: &ElkGraph, root: NodeId) -> f64 {
    let mut edge_length = 0.0;
    let root_shape = &g.node(root).shape;
    for edge in util::all_outgoing_edges(g, root) {
        let target = g.shape_node(g.edge(edge).targets[0]);
        let target_shape = &g.node(target).shape;

        let mut target_x = target_shape.x + target_shape.width / 2.0;
        let mut target_y = target_shape.y + target_shape.height / 2.0;

        let mut root_x = root_shape.x + root_shape.width / 2.0;
        let mut root_y = root_shape.y + root_shape.height / 2.0;

        // Clipping
        let mut vector = KVector::new(target_x - root_x, target_y - root_y);
        let mut source_clip = KVector::new(vector.x, vector.y);
        clip_vector(&mut source_clip, root_shape.width, root_shape.height);
        vector.x -= source_clip.x;
        vector.y -= source_clip.y;

        root_x = target_x - vector.x;
        root_y = target_y - vector.y;

        let mut target_clip = KVector::new(vector.x, vector.y);
        clip_vector(&mut target_clip, target_shape.width, target_shape.height);
        vector.x -= target_clip.x;
        vector.y -= target_clip.y;

        target_x = root_x + vector.x;
        target_y = root_y + vector.y;

        let vector_x = target_x - root_x;
        let vector_y = target_y - root_y;
        edge_length += (vector_x * vector_x + vector_y * vector_y).sqrt();
    }
    edge_length
}

/// When the target's `POSITION` is unset the result is `(0, 0)`.
fn edge_length_position(g: &ElkGraph, root: NodeId) -> f64 {
    let mut edge_length = 0.0;
    let root_shape = &g.node(root).shape;
    for edge in util::all_outgoing_edges(g, root) {
        let target = g.shape_node(g.edge(edge).targets[0]);
        let target_shape = &g.node(target).shape;
        let target_x = target_shape.x + target_shape.width / 2.0;
        let target_y = target_shape.y + target_shape.height / 2.0;

        let position: KVector = g.node(target).properties.get(&POSITION);
        let root_x = root_shape.x + position.x + root_shape.width / 2.0;
        // The full height is used here (not height / 2), preserved as is.
        let root_y = root_shape.y + position.y + root_shape.height;

        let vector_x = target_x - root_x;
        let vector_y = target_y - root_y;
        edge_length += (vector_x * vector_x + vector_y * vector_y).sqrt();
    }
    edge_length
}

fn crossing_minimization(g: &ElkGraph, root: NodeId) -> f64 {
    let mut crossings = 0;
    let nodes = util::get_successors(g, root);

    let mut k = 0;
    for &node1 in &nodes {
        k += 1;
        for i in k..nodes.len() {
            if is_crossing(g, root, node1, nodes[i]) {
                crossings += 1;
            }
        }
    }
    crossings as f64
}

fn is_crossing(g: &ElkGraph, root: NodeId, node1: NodeId, node2: NodeId) -> bool {
    let root_shape = &g.node(root).shape;
    let root_x = root_shape.x + root_shape.width / 2.0;
    let root_y = root_shape.x + root_shape.width / 2.0;

    // node1
    let shape1 = &g.node(node1).shape;
    let node1_vector = KVector::new(shape1.x + shape1.width / 2.0, shape1.y + shape1.height / 2.0);

    let mut position1: KVector = g.node(node1).properties.get(&POSITION);
    position1.x += root_x;
    position1.y += root_y;
    g.node(node1).properties.set(&POSITION, position1);

    let m1 = (node1_vector.y - position1.y) / (node1_vector.x - position1.x);
    let b1 = node1_vector.y - m1 * node1_vector.x;

    // node2
    let shape2 = &g.node(node2).shape;
    let node2_vector = KVector::new(shape2.x + shape2.width / 2.0, shape2.y + shape2.height / 2.0);

    let mut position2: KVector = g.node(node2).properties.get(&POSITION);
    position2.x += root_x;
    position2.y += root_y;
    g.node(node2).properties.set(&POSITION, position2);

    let m2 = (node2_vector.y - position2.y) / (node2_vector.x - position2.x);
    let b2 = node2_vector.y - m2 * node2_vector.x;

    let x_cut = (b1 - b2) / (m2 - m1);
    // check whether the cut occurs on the relevant line segment
    if (position1.x < x_cut && node1_vector.x < x_cut)
        || (x_cut < position1.x && x_cut < node1_vector.x)
    {
        false
    } else {
        !((position2.x < x_cut && node2_vector.x < x_cut)
            || (x_cut < position2.x && x_cut < node2_vector.x))
    }
}
