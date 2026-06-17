//! The parts of `org.eclipse.elk.core.util.ElkUtil` used by the
//! engine and the basic layout providers.

use crate::graph::graph::{EdgeId, ElkGraph, NodeId, PortId, SectionId};
use crate::graph::math::{KVector, KVectorChain};
use crate::graph::properties::EnumSet;

use crate::core::options::*;

pub const DEFAULT_MIN_WIDTH: f64 = 20.0;
pub const DEFAULT_MIN_HEIGHT: f64 = 20.0;

pub fn calc_port_offset(g: &ElkGraph, port: PortId, side: PortSide) -> f64 {
    let node = g.port(port).parent.expect("port must have a parent node");
    let p = &g.port(port).shape;
    let n = &g.node(node).shape;
    match side {
        PortSide::NORTH => -(p.y + p.height),
        PortSide::EAST => p.x - n.width,
        PortSide::SOUTH => p.y - n.height,
        PortSide::WEST => -(p.x + p.width),
        PortSide::UNDEFINED => 0.0,
    }
}

pub fn calc_port_side(g: &ElkGraph, port: PortId, direction: Direction) -> PortSide {
    let node = g.port(port).parent.expect("port must have a parent node");
    let n = &g.node(node).shape;
    let (node_width, node_height) = (n.width, n.height);
    if node_width <= 0.0 && node_height <= 0.0 {
        return PortSide::UNDEFINED;
    }
    let p = &g.port(port).shape;
    let (xpos, ypos) = (p.x, p.y);
    match direction {
        Direction::LEFT | Direction::RIGHT => {
            if xpos < 0.0 {
                return PortSide::WEST;
            } else if xpos + p.width > node_width {
                return PortSide::EAST;
            }
        }
        Direction::UP | Direction::DOWN => {
            if ypos < 0.0 {
                return PortSide::NORTH;
            } else if ypos + p.height > node_height {
                return PortSide::SOUTH;
            }
        }
        Direction::UNDEFINED => {}
    }
    let width_percent = (xpos + p.width / 2.0) / node_width;
    let height_percent = (ypos + p.height / 2.0) / node_height;
    if width_percent + height_percent <= 1.0 && width_percent - height_percent <= 0.0 {
        PortSide::WEST
    } else if width_percent + height_percent >= 1.0 && width_percent - height_percent >= 0.0 {
        PortSide::EAST
    } else if height_percent < 0.5 {
        PortSide::NORTH
    } else {
        PortSide::SOUTH
    }
}

/// The `DIRECTION` that applies to a node: its parent's, or its own at root.
fn applicable_direction(g: &ElkGraph, node: NodeId) -> Direction {
    match g.node(node).parent {
        None => g.node(node).properties.get(&DIRECTION),
        Some(parent) => g.node(parent).properties.get(&DIRECTION),
    }
}

pub fn effective_min_size_constraint_for(g: &ElkGraph, node: NodeId) -> KVector {
    let size_constraint: EnumSet<SizeConstraint> =
        g.node(node).properties.get(&NODE_SIZE_CONSTRAINTS);
    if size_constraint.contains(SizeConstraint::MINIMUM_SIZE) {
        let size_options: EnumSet<SizeOptions> = g.node(node).properties.get(&NODE_SIZE_OPTIONS);
        let mut min_size = g.node(node).properties.get(&NODE_SIZE_MINIMUM);
        if size_options.contains(SizeOptions::DEFAULT_MINIMUM_SIZE) {
            if min_size.x <= 0.0 {
                min_size.x = DEFAULT_MIN_WIDTH;
            }
            if min_size.y <= 0.0 {
                min_size.y = DEFAULT_MIN_HEIGHT;
            }
        }
        min_size
    } else {
        KVector::default()
    }
}

/// Returns `None` when the size constraints are empty.
pub fn resize_node_constraints(g: &mut ElkGraph, node: NodeId) -> Option<KVector> {
    let size_constraint: EnumSet<SizeConstraint> =
        g.node(node).properties.get(&NODE_SIZE_CONSTRAINTS);
    if size_constraint.is_empty() {
        return None;
    }

    let mut new_width = 0.0;
    let mut new_height = 0.0;

    if size_constraint.contains(SizeConstraint::PORTS) {
        let port_constraints: PortConstraints = g.node(node).properties.get(&PORT_CONSTRAINTS);
        let (mut min_north, mut min_east, mut min_south, mut min_west) = (2.0, 2.0, 2.0, 2.0);
        let direction = applicable_direction(g, node);
        let ports: Vec<PortId> = g.node(node).ports.clone();
        for port in ports {
            let mut port_side: PortSide = g.port(port).properties.get(&PORT_SIDE);
            if port_side == PortSide::UNDEFINED {
                port_side = calc_port_side(g, port, direction);
                g.port_mut(port).properties.set(&PORT_SIDE, port_side);
            }
            let p = &g.port(port).shape;
            if port_constraints == PortConstraints::FIXED_POS {
                match port_side {
                    PortSide::NORTH => min_north = f64::max(min_north, p.x + p.width),
                    PortSide::EAST => min_east = f64::max(min_east, p.y + p.height),
                    PortSide::SOUTH => min_south = f64::max(min_south, p.x + p.width),
                    PortSide::WEST => min_west = f64::max(min_west, p.y + p.height),
                    PortSide::UNDEFINED => {}
                }
            } else {
                match port_side {
                    PortSide::NORTH => min_north += p.width + 2.0,
                    PortSide::EAST => min_east += p.height + 2.0,
                    PortSide::SOUTH => min_south += p.width + 2.0,
                    PortSide::WEST => min_west += p.height + 2.0,
                    PortSide::UNDEFINED => {}
                }
            }
        }
        new_width = f64::max(min_north, min_south);
        new_height = f64::max(min_east, min_west);
    }

    Some(resize_node(g, node, new_width, new_height, true, true))
}

pub fn resize_node(
    g: &mut ElkGraph,
    node: NodeId,
    new_width: f64,
    new_height: f64,
    move_ports: bool,
    move_labels: bool,
) -> KVector {
    let old_size = KVector::new(g.node(node).shape.width, g.node(node).shape.height);

    let mut new_size = effective_min_size_constraint_for(g, node);
    new_size.x = f64::max(new_size.x, new_width);
    new_size.y = f64::max(new_size.y, new_height);

    let width_ratio = new_size.x / old_size.x;
    let height_ratio = new_size.y / old_size.y;
    let width_diff = new_size.x - old_size.x;
    let height_diff = new_size.y - old_size.y;

    if move_ports {
        let direction = applicable_direction(g, node);
        let fixed_ports =
            g.node(node).properties.get(&PORT_CONSTRAINTS) == PortConstraints::FIXED_POS;
        let ports: Vec<PortId> = g.node(node).ports.clone();
        for port in ports {
            let mut port_side: PortSide = g.port(port).properties.get(&PORT_SIDE);
            if port_side == PortSide::UNDEFINED {
                port_side = calc_port_side(g, port, direction);
                g.port_mut(port).properties.set(&PORT_SIDE, port_side);
            }
            let p = &mut g.port_mut(port).shape;
            match port_side {
                PortSide::NORTH => {
                    if !fixed_ports {
                        p.x *= width_ratio;
                    }
                }
                PortSide::EAST => {
                    p.x += width_diff;
                    if !fixed_ports {
                        p.y *= height_ratio;
                    }
                }
                PortSide::SOUTH => {
                    if !fixed_ports {
                        p.x *= width_ratio;
                    }
                    p.y += height_diff;
                }
                PortSide::WEST => {
                    if !fixed_ports {
                        p.y *= height_ratio;
                    }
                }
                PortSide::UNDEFINED => {}
            }
        }
    }

    // resize the node AFTER ports have been placed
    g.node_mut(node).shape.set_dimensions(new_size.x, new_size.y);

    if move_labels {
        let labels = g.node(node).labels.clone();
        for label in labels {
            let l = &mut g.label_mut(label).shape;
            let midx = l.x + l.width / 2.0;
            let midy = l.y + l.height / 2.0;
            let width_percent = midx / old_size.x;
            let height_percent = midy / old_size.y;
            if width_percent + height_percent >= 1.0 {
                if width_percent - height_percent > 0.0 && midy >= 0.0 {
                    l.x += width_diff;
                    l.y += height_diff * height_percent;
                } else if width_percent - height_percent < 0.0 && midx >= 0.0 {
                    l.x += width_diff * width_percent;
                    l.y += height_diff;
                }
            }
        }
    }

    g.node_mut(node)
        .properties
        .set(&NODE_SIZE_CONSTRAINTS, EnumSet::<SizeConstraint>::none());

    KVector::new(width_ratio, height_ratio)
}

/// Translates children
/// and contained edges.
pub fn translate(g: &mut ElkGraph, parent: NodeId, xoffset: f64, yoffset: f64) {
    let children = g.node(parent).children.clone();
    for child in children {
        let s = &mut g.node_mut(child).shape;
        s.x += xoffset;
        s.y += yoffset;
    }
    let edges = g.node(parent).contained_edges.clone();
    for edge in edges {
        translate_edge(g, edge, xoffset, yoffset);
    }
}

pub fn translate_edge(g: &mut ElkGraph, edge: EdgeId, xoffset: f64, yoffset: f64) {
    let sections = g.edge(edge).sections.clone();
    for section in sections {
        translate_section(g, section, xoffset, yoffset);
    }
    let labels = g.edge(edge).labels.clone();
    for label in labels {
        let s = &mut g.label_mut(label).shape;
        s.x += xoffset;
        s.y += yoffset;
    }
    // Reading JUNCTION_POINTS materializes the empty Cloneable default into
    // the edge, so even edges without junction points gain an (empty) chain
    // that the exporter then emits as "()". Use `get` (materializing), not
    // `try_get`.
    let mut jps = g.edge(edge).properties.get(&JUNCTION_POINTS);
    jps.offset_xy(xoffset, yoffset);
    g.edge(edge).properties.set(&JUNCTION_POINTS, jps);
}

pub fn translate_section(g: &mut ElkGraph, section: SectionId, xoffset: f64, yoffset: f64) {
    let s = g.section_mut(section);
    s.start_x += xoffset;
    s.start_y += yoffset;
    for bp in &mut s.bend_points {
        bp.0 += xoffset;
        bp.1 += yoffset;
    }
    s.end_x += xoffset;
    s.end_y += yoffset;
}

/// Content alignment
/// shift after a node grew.
pub fn translate_aligned(g: &mut ElkGraph, parent: NodeId, new_size: KVector, old_size: KVector) {
    let content_alignment: EnumSet<ContentAlignment> =
        g.node(parent).properties.get(&CONTENT_ALIGNMENT);
    let mut x_translate = 0.0;
    let mut y_translate = 0.0;

    if new_size.x > old_size.x {
        if content_alignment.contains(ContentAlignment::H_CENTER) {
            x_translate = (new_size.x - old_size.x) / 2.0;
        } else if content_alignment.contains(ContentAlignment::H_RIGHT) {
            x_translate = new_size.x - old_size.x;
        }
    }
    if new_size.y > old_size.y {
        if content_alignment.contains(ContentAlignment::V_CENTER) {
            y_translate = (new_size.y - old_size.y) / 2.0;
        } else if content_alignment.contains(ContentAlignment::V_BOTTOM) {
            y_translate = new_size.y - old_size.y;
        }
    }
    translate(g, parent, x_translate, y_translate);
}

pub fn apply_configured_node_scaling(g: &mut ElkGraph, node: NodeId) {
    let scaling_factor: f64 = g.node(node).properties.get(&SCALE_FACTOR);
    if scaling_factor == 1.0 {
        return;
    }
    {
        let s = &mut g.node_mut(node).shape;
        let (w, h) = (s.width, s.height);
        s.set_dimensions(scaling_factor * w, scaling_factor * h);
    }
    let mut shapes: Vec<ScaledShape> = Vec::new();
    for &label in &g.node(node).labels {
        shapes.push(ScaledShape::Label(label));
    }
    for &port in &g.node(node).ports {
        shapes.push(ScaledShape::Port(port));
        for &label in &g.port(port).labels {
            shapes.push(ScaledShape::Label(label));
        }
    }
    for shape in shapes {
        match shape {
            ScaledShape::Label(l) => {
                let s = &mut g.label_mut(l).shape;
                s.set_location(scaling_factor * s.x, scaling_factor * s.y);
                let (w, h) = (s.width, s.height);
                s.set_dimensions(scaling_factor * w, scaling_factor * h);
            }
            ScaledShape::Port(p) => {
                {
                    let s = &mut g.port_mut(p).shape;
                    s.set_location(scaling_factor * s.x, scaling_factor * s.y);
                    let (w, h) = (s.width, s.height);
                    s.set_dimensions(scaling_factor * w, scaling_factor * h);
                }
                if let Some(mut anchor) = g.port(p).properties.try_get(&PORT_ANCHOR) {
                    anchor.x *= scaling_factor;
                    anchor.y *= scaling_factor;
                    g.port_mut(p).properties.set(&PORT_ANCHOR, anchor);
                }
            }
        }
    }
}

enum ScaledShape {
    Label(crate::graph::graph::LabelId),
    Port(PortId),
}

pub fn apply_vector_chain(g: &mut ElkGraph, chain: &KVectorChain, section: SectionId) {
    assert!(
        chain.len() >= 2,
        "The vector chain must contain at least a source and a target point."
    );
    let s = g.section_mut(section);
    let first = chain.first();
    s.start_x = first.x;
    s.start_y = first.y;
    s.bend_points = chain.0[1..chain.len() - 1].iter().map(|v| (v.x, v.y)).collect();
    let last = chain.last();
    s.end_x = last.x;
    s.end_y = last.y;
}

pub fn create_vector_chain(g: &ElkGraph, section: SectionId) -> KVectorChain {
    g.section_chain(section)
}

pub fn determine_junction_points(g: &ElkGraph, edge: EdgeId) -> KVectorChain {
    assert_eq!(
        g.edge(edge).sections.len(),
        1,
        "The edge needs to have exactly one edge section."
    );
    let mut junction_points = KVectorChain::new();
    if let Some(port) = g.shape_port(g.edge(edge).sources[0]) {
        junction_points
            .0
            .extend(determine_junction_points_at_port(g, edge, port, false).0);
    }
    if let Some(port) = g.shape_port(g.edge(edge).targets[0]) {
        junction_points
            .0
            .extend(determine_junction_points_at_port(g, edge, port, true).0);
    }
    junction_points
}

fn section_points(g: &ElkGraph, section: SectionId) -> Vec<KVector> {
    g.section_chain(section).0
}

fn determine_junction_points_at_port(
    g: &ElkGraph,
    edge: EdgeId,
    port: PortId,
    reverse: bool,
) -> KVectorChain {
    let section = g.edge(edge).sections[0];
    let mut junction_points = KVectorChain::new();
    let points = section_points(g, section);

    // All sections connected to the port, except the main edge's.
    let mut all_connected: Vec<(Vec<KVector>, KVector)> = Vec::new();
    let incident: Vec<EdgeId> = g
        .port(port)
        .outgoing_edges
        .iter()
        .chain(g.port(port).incoming_edges.iter())
        .copied()
        .collect();
    for other_edge in incident {
        if other_edge != edge {
            let other_section = g.edge(other_edge).sections[0];
            let other_points = section_points(g, other_section);
            let offset = if reverse {
                KVector::diff(points[points.len() - 1], other_points[other_points.len() - 1])
            } else {
                KVector::diff(points[0], other_points[0])
            };
            all_connected.push((other_points, offset));
        }
    }

    if !all_connected.is_empty() {
        let idx = |i: usize, len: usize| if reverse { len - 1 - i } else { i };
        let mut p1 = points[idx(0, points.len())];
        for i in 1..points.len() {
            let p2 = points[idx(i, points.len())];
            let mut remaining = Vec::new();
            for (other_points, offset) in all_connected.drain(..) {
                if other_points.len() <= i {
                    continue; // drop this section
                }
                let mut p3 = other_points[idx(i, other_points.len())];
                p3.add(offset);
                if p2.x != p3.x || p2.y != p3.y {
                    let dx2 = p2.x - p1.x;
                    let dy2 = p2.y - p1.y;
                    let dx3 = p3.x - p1.x;
                    let dy3 = p3.y - p1.y;
                    if (dx3 * dy2) == (dy3 * dx2)
                        && java_signum(dx2) == java_signum(dx3)
                        && java_signum(dy2) == java_signum(dy3)
                    {
                        if dx2.abs() < dx3.abs() || dy2.abs() < dy3.abs() {
                            junction_points.add_last(p2);
                        }
                    } else if i > 1 {
                        junction_points.add_last(p1);
                    }
                    // do not consider this section in following iterations
                } else {
                    remaining.push((other_points, offset));
                }
            }
            all_connected = remaining;
            p1 = p2;
        }
    }

    junction_points
}

/// Bounding box of the
/// port's labels, in coordinates relative to the port. Returns an empty
/// rectangle if the port has no labels.
pub fn get_labels_bounds<G: crate::core::adapters::AdapterGraph>(
    g: &G,
    port: G::P,
) -> crate::graph::math::ElkRectangle {
    use crate::graph::math::ElkRectangle;
    let mut bounds: Option<ElkRectangle> = None;
    for label in g.port_labels(port) {
        let pos = g.label_position(label);
        let size = g.label_size(label);
        let current = ElkRectangle::new(pos.x, pos.y, size.x, size.y);
        match &mut bounds {
            None => bounds = Some(current),
            Some(b) => b.union(&current),
        }
    }
    bounds.unwrap_or_default()
}

/// The part of the
/// port's (fixed-placement) labels that lies inside the node.
pub fn compute_inside_part<G: crate::core::adapters::AdapterGraph>(
    g: &G,
    port: G::P,
    port_border_offset: f64,
) -> f64 {
    let label_bounds = get_labels_bounds(g, port);
    compute_inside_part_values(
        KVector::new(label_bounds.x, label_bounds.y),
        KVector::new(label_bounds.width, label_bounds.height),
        g.port_size(port),
        port_border_offset,
        g.port_side(port),
    )
}

pub fn compute_inside_part_values(
    label_position: KVector,
    label_size: KVector,
    port_size: KVector,
    port_border_offset: f64,
    port_side: PortSide,
) -> f64 {
    match port_side {
        PortSide::NORTH => {
            (label_size.y + label_position.y - (port_size.y + port_border_offset)).max(0.0)
        }
        PortSide::SOUTH => (-label_position.y - port_border_offset).max(0.0),
        PortSide::EAST => (-label_position.x - port_border_offset).max(0.0),
        PortSide::WEST => {
            (label_size.x + label_position.x - (port_size.x + port_border_offset)).max(0.0)
        }
        PortSide::UNDEFINED => 0.0,
    }
}

/// `Math.signum` (returns 0.0 for ±0.0, NaN for NaN).
fn java_signum(v: f64) -> f64 {
    if v == 0.0 || v.is_nan() {
        v
    } else if v > 0.0 {
        1.0
    } else {
        -1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::graph::ElkGraph;

    #[test]
    fn resize_node_respects_min_size() {
        let mut g = ElkGraph::new();
        let n = g.create_node(Some(g.root));
        g.node_mut(n).shape.set_dimensions(10.0, 10.0);
        g.node_mut(n)
            .properties
            .set(&NODE_SIZE_CONSTRAINTS, EnumSet::of(&[SizeConstraint::MINIMUM_SIZE]));
        g.node_mut(n)
            .properties
            .set(&NODE_SIZE_OPTIONS, EnumSet::of(&[SizeOptions::DEFAULT_MINIMUM_SIZE]));
        resize_node(&mut g, n, 5.0, 5.0, true, true);
        // default min size (20, 20) wins over the requested (5, 5)
        assert_eq!(g.node(n).shape.width, 20.0);
        assert_eq!(g.node(n).shape.height, 20.0);
        // constraints reset to fixed afterwards
        let sc: EnumSet<SizeConstraint> = g.node(n).properties.get(&NODE_SIZE_CONSTRAINTS);
        assert!(sc.is_empty());
    }

    #[test]
    fn calc_port_side_quadrants() {
        let mut g = ElkGraph::new();
        let n = g.create_node(Some(g.root));
        g.node_mut(n).shape.set_dimensions(100.0, 100.0);
        let p = g.create_port(n);
        g.port_mut(p).shape.set_location(-5.0, 50.0);
        assert_eq!(calc_port_side(&g, p, Direction::RIGHT), PortSide::WEST);
        g.port_mut(p).shape.set_location(98.0, 50.0);
        g.port_mut(p).shape.set_dimensions(10.0, 10.0);
        assert_eq!(calc_port_side(&g, p, Direction::RIGHT), PortSide::EAST);
        g.port_mut(p).shape.set_location(50.0, 2.0);
        assert_eq!(calc_port_side(&g, p, Direction::DOWN), PortSide::NORTH);
    }
}
