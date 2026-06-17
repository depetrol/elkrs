
use crate::graph::graph::{ElkGraph, NodeId, ShapeId};
use crate::graph::math::KVector;

use crate::alg_radial::options;
use crate::alg_radial::util;

pub fn process(g: &mut ElkGraph, graph: NodeId, root: NodeId) {
    let mut target_angle: f64 = g.node(graph).properties.get(&options::ROTATION_TARGET_ANGLE);

    if g.node(graph)
        .properties
        .get(&options::ROTATION_COMPUTE_ADDITIONAL_WEDGE_SPACE)
    {
        // Using the target angle as our base alignment we want to further
        // rotate the layout such that a line following the target angle runs
        // directly through the middle of the wedge between the first and last
        // node. (The targets are cast to ElkNode; a port here would be a
        // ClassCastException.)
        let outgoing = &g.node(root).outgoing_edges;
        let as_node = |shape: ShapeId| match shape {
            ShapeId::Node(n) => n,
            ShapeId::Port(_) => panic!("expected an ElkNode edge target"),
        };
        let last_node = as_node(g.edge(*outgoing.last().expect("root has no outgoing edges")).targets[0]);
        let first_node = as_node(g.edge(outgoing[0]).targets[0]);
        let last_shape = &g.node(last_node).shape;
        let first_shape = &g.node(first_node).shape;
        let last_vector = KVector::new(
            last_shape.x + last_shape.width / 2.0,
            last_shape.y + last_shape.height / 2.0,
        );
        let first_vector = KVector::new(
            first_shape.x + first_shape.width / 2.0,
            first_shape.y + first_shape.height / 2.0,
        );

        // we shift all angles into the range (0,pi] to avoid dealing with
        // negative angles.
        let mut alpha = target_angle;
        if alpha <= 0.0 {
            alpha += util::TWO_PI;
        }

        let mut wedge_angle = last_vector.angle(first_vector);
        if wedge_angle <= 0.0 {
            wedge_angle += util::TWO_PI;
        }

        let mut alignment_angle = last_vector.y.atan2(last_vector.x);
        if alignment_angle <= 0.0 {
            alignment_angle += util::TWO_PI;
        }

        // rotate the entire layout by subtracting the incoming angle alpha
        // and add half the wedge angle back to align through the center of
        // the wedge; invert for the downward facing coordinate system.
        target_angle = std::f64::consts::PI - (alignment_angle - alpha + wedge_angle / 2.0);
    }

    // rotate all nodes around the origin, because the root node is positioned
    // at the origin; nodes are positioned with their center on the radius
    let children = g.node(graph).children.clone();
    for node in children {
        let shape = &g.node(node).shape;
        let mut pos = KVector::new(shape.x + shape.width / 2.0, shape.y + shape.height / 2.0);
        pos.rotate(target_angle);
        let shape = &mut g.node_mut(node).shape;
        shape.set_location(pos.x - shape.width / 2.0, pos.y - shape.height / 2.0);
    }
}
