
use crate::graph::graph::{ElkGraph, NodeId, ShapeId};
use crate::graph::math::KVector;

use crate::alg_radial::util;

pub fn clip_vector(v: &mut KVector, width: f64, height: f64) {
    let wh = width / 2.0;
    let hh = height / 2.0;
    let absx = v.x.abs();
    let absy = v.y.abs();
    let mut xscale = 1.0;
    let mut yscale = 1.0;
    if absx > wh {
        xscale = wh / absx;
    }
    if absy > hh {
        yscale = hh / absy;
    }
    v.scale(f64::min(xscale, yscale));
}

pub fn process(g: &mut ElkGraph, _graph: NodeId, root: NodeId) {
    route_edges(g, root);
}

/// Route edges from node center to node center, then
/// clip them to not cross the nodes.
fn route_edges(g: &mut ElkGraph, node: NodeId) {
    for edge in util::all_outgoing_edges(g, node) {
        if !matches!(g.edge(edge).sources[0], ShapeId::Port(_)) {
            let target = g.shape_node(g.edge(edge).targets[0]);
            if !g.is_hierarchical(edge) {
                let node_shape = &g.node(node).shape;
                let target_shape = &g.node(target).shape;

                let mut source_x = node_shape.x + node_shape.width / 2.0;
                let mut source_y = node_shape.y + node_shape.height / 2.0;

                let mut target_x = target_shape.x + target_shape.width / 2.0;
                let mut target_y = target_shape.y + target_shape.height / 2.0;

                // Clipping
                let mut vector = KVector::new(target_x - source_x, target_y - source_y);
                let mut source_clip = KVector::new(vector.x, vector.y);
                clip_vector(&mut source_clip, node_shape.width, node_shape.height);
                vector.x -= source_clip.x;
                vector.y -= source_clip.y;

                source_x = target_x - vector.x;
                source_y = target_y - vector.y;

                let mut target_clip = KVector::new(vector.x, vector.y);
                clip_vector(&mut target_clip, target_shape.width, target_shape.height);
                vector.x -= target_clip.x;
                vector.y -= target_clip.y;

                target_x = source_x + vector.x;
                target_y = source_y + vector.y;

                // reset the first section and remove all others.
                g.edge_mut(edge).sections.truncate(1);
                let section = g.first_edge_section(edge, true);
                g.section_mut(section).set_start_location(source_x, source_y);
                g.section_mut(section).set_end_location(target_x, target_y);
                route_edges(g, target);
            }
        }
    }
}
