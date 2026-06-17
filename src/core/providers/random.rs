//!
//! Note: seed 0 (the default) would mean a time-seeded `Random`, which is not
//! reproducible. We use seed 1 in that case; pixel parity is only expected for
//! explicit non-zero seeds.

use crate::graph::graph::{EdgeId, ElkGraph, NodeId, ShapeId};

use crate::core::elkutil;
use crate::core::javacompat::JavaRandom;
use crate::core::options::*;
use crate::core::providers::fixed::all_outgoing_edges;
use crate::core::registry::LayoutProvider;

const MAX_BENDS: i32 = 5;
const RAND_FACT: f64 = 0.2f32 as f64;

#[derive(Default)]
pub struct RandomLayoutProvider;

impl LayoutProvider for RandomLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, parent: NodeId) -> Result<(), String> {
        if g.node(parent).children.is_empty() {
            return Ok(());
        }
        let seed: i32 = g.node(parent).properties.get(&random::RANDOM_SEED);
        let mut rng = if seed != 0 {
            JavaRandom::new(seed as i64)
        } else {
            JavaRandom::new(1)
        };

        let aspect_ratio = g.node(parent).properties.get(&random::ASPECT_RATIO) as f32;
        let spacing = g.node(parent).properties.get(&random::SPACING_NODE_NODE) as f32;
        let padding = g.node(parent).properties.get(&random::PADDING);

        randomize_node(g, parent, &mut rng, aspect_ratio as f64, spacing as f64, padding);
        Ok(())
    }
}

fn randomize_node(
    g: &mut ElkGraph,
    parent: NodeId,
    rng: &mut JavaRandom,
    aspect_ratio: f64,
    spacing: f64,
    padding: crate::graph::math::Spacing,
) {
    let mut nodes_area = 0.0f64;
    let mut max_width = 0.0f64;
    let mut max_height = 0.0f64;
    let mut m = 1i32;
    let children = g.node(parent).children.clone();
    for &node in &children {
        m += all_outgoing_edges(g, node).len() as i32;
        let s = &g.node(node).shape;
        max_width = f64::max(max_width, s.width);
        max_height = f64::max(max_height, s.height);
        nodes_area += s.width * s.height;
    }
    let n = children.len() as i32;

    let draw_area = nodes_area + 2.0 * spacing * spacing * m as f64 * n as f64;
    let area_sqrt = draw_area.sqrt();
    let draw_width = f64::max(area_sqrt * aspect_ratio, max_width);
    let draw_height = f64::max(area_sqrt / aspect_ratio, max_height);

    for &node in &children {
        let (w, h) = {
            let s = &g.node(node).shape;
            (s.width, s.height)
        };
        let x = padding.left + rng.next_double() * (draw_width - w);
        let y = padding.left + rng.next_double() * (draw_height - h);
        g.node_mut(node).shape.set_location(x, y);
    }

    let total_width = draw_width + padding.horizontal();
    let total_height = draw_height + padding.vertical();
    for &source in &children {
        for edge in all_outgoing_edges(g, source) {
            if !g.is_hierarchical(edge) {
                randomize_edge(g, edge, rng, total_width, total_height);
            }
        }
    }

    let total_width = total_width + padding.left + padding.right;
    let total_height = total_height + padding.top + padding.bottom;
    elkutil::resize_node(g, parent, total_width, total_height, false, true);
}

fn shape_geometry(g: &ElkGraph, shape: ShapeId) -> (f64, f64, f64, f64) {
    match shape {
        ShapeId::Node(n) => {
            let s = &g.node(n).shape;
            (s.x, s.y, s.width, s.height)
        }
        ShapeId::Port(p) => {
            let s = &g.port(p).shape;
            (s.x, s.y, s.width, s.height)
        }
    }
}

fn randomize_edge(
    g: &mut ElkGraph,
    edge: EdgeId,
    rng: &mut JavaRandom,
    draw_width: f64,
    draw_height: f64,
) {
    let source_shape = g.edge(edge).sources[0];
    let (mut source_x, mut source_y, sw, sh) = shape_geometry(g, source_shape);
    let source_width = sw / 2.0;
    let source_height = sh / 2.0;
    if let ShapeId::Port(p) = source_shape {
        // The parent's X is added twice (bug preserved for parity)
        let px = g.node(g.port(p).parent.unwrap()).shape.x;
        source_x += px;
        source_x += px;
    }
    source_x += source_width;
    source_y += source_height;

    // The target reuses edge.getSources().get(0) again (bug preserved)
    let target_shape = g.edge(edge).sources[0];
    let (mut target_x, mut target_y, tw, th) = shape_geometry(g, target_shape);
    let target_width = tw / 2.0;
    let target_height = th / 2.0;
    if let ShapeId::Port(p) = target_shape {
        let px = g.node(g.port(p).parent.unwrap()).shape.x;
        target_x += px;
        target_x += px;
    }
    target_x += target_width;
    target_y += target_height;

    if g.edge(edge).sections.is_empty() {
        g.create_section(edge);
    } else if g.edge(edge).sections.len() > 1 {
        // see FixedLayoutProvider: invalid state
        panic!("RandomLayoutProvider: edge with multiple sections");
    }
    let section = g.edge(edge).sections[0];

    let mut source_px = target_x;
    if target_x > source_x + source_width {
        source_px = source_x + source_width;
    } else if target_x < source_x - source_width {
        source_px = source_x - source_width;
    }
    let mut source_py = target_y;
    if target_y > source_y + source_height {
        source_py = source_y + source_height;
    } else if target_y < source_y - source_height {
        source_py = source_y - source_height;
    }
    if source_px > source_x - source_width
        && source_px < source_x + source_width
        && source_py > source_y - source_height
        && source_py < source_y + source_height
    {
        source_px = source_x + source_width;
    }

    let mut target_px = source_x;
    if source_x > target_x + target_width {
        target_px = target_x + target_width;
    } else if source_x < target_x - target_width {
        target_px = target_x - target_width;
    }
    let mut target_py = source_y;
    if source_y > target_y + target_height {
        target_py = target_y + target_height;
    } else if source_y < target_y - target_height {
        target_py = target_y - target_height;
    }
    if target_px > target_x - target_width
        && target_px < target_x + target_width
        && target_py > target_y - target_height
        && target_py < target_y + target_height
    {
        target_py = target_y + target_height;
    }

    g.section_mut(section).set_start_location(source_px, source_py);
    g.section_mut(section).set_end_location(target_px, target_py);

    let mut bend_points = Vec::new();
    let mut bends_num = rng.next_int_bound(MAX_BENDS);
    if source_shape == target_shape {
        bends_num += 1;
    }
    let xdiff = target_px - source_px;
    let ydiff = target_py - source_py;
    let total_dist = (xdiff * xdiff + ydiff * ydiff).sqrt();
    let max_rand = total_dist * RAND_FACT;
    let xincr = xdiff / (bends_num + 1) as f64;
    let yincr = ydiff / (bends_num + 1) as f64;
    let mut x = source_px;
    let mut y = source_py;
    for _ in 0..bends_num {
        x += xincr;
        y += yincr;
        let mut randx = x + rng.next_float() as f64 * max_rand - max_rand / 2.0;
        if randx < 0.0 {
            randx = 1.0;
        } else if randx > draw_width {
            randx = draw_width - 1.0;
        }
        let mut randy = y + rng.next_float() as f64 * max_rand - max_rand / 2.0;
        if randy < 0.0 {
            randy = 1.0;
        } else if randy > draw_height {
            randy = draw_height - 1.0;
        }
        bend_points.push((randx, randy));
    }
    g.section_mut(section).bend_points = bend_points;
}
