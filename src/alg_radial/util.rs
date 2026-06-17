
use std::cmp::Ordering;

use crate::graph::graph::{EdgeId, ElkGraph, NodeId, ShapeId};

use crate::alg_radial::options::POSITION;

/// Constant for 2*PI.
pub const TWO_PI: f64 = 2.0 * std::f64::consts::PI;
/// Constant for fuzzy compare.
const EPSILON: f64 = 1e-10;

/// Guava `DoubleMath.fuzzyEquals`.
pub fn fuzzy_equals(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance || a == b || (a.is_nan() && b.is_nan())
}

/// Guava `DoubleMath.fuzzyCompare`.
pub fn fuzzy_compare(a: f64, b: f64, tolerance: f64) -> Ordering {
    if fuzzy_equals(a, b, tolerance) {
        Ordering::Equal
    } else if a < b {
        Ordering::Less
    } else if a > b {
        Ordering::Greater
    } else {
        // Booleans.compare(Double.isNaN(a), Double.isNaN(b))
        a.is_nan().cmp(&b.is_nan())
    }
}

/// `ElkGraphUtil.allOutgoingEdges`: the node's own outgoing edges
/// followed by those of its ports.
pub fn all_outgoing_edges(g: &ElkGraph, node: NodeId) -> Vec<EdgeId> {
    let n = g.node(node);
    let mut edges = n.outgoing_edges.clone();
    for &port in &n.ports {
        edges.extend(g.port(port).outgoing_edges.iter().copied());
    }
    edges
}

/// `ElkGraphUtil.allIncomingEdges`.
pub fn all_incoming_edges(g: &ElkGraph, node: NodeId) -> Vec<EdgeId> {
    let n = g.node(node);
    let mut edges = n.incoming_edges.clone();
    for &port in &n.ports {
        edges.extend(g.port(port).incoming_edges.iter().copied());
    }
    edges
}

pub fn get_successors(g: &ElkGraph, node: NodeId) -> Vec<NodeId> {
    let mut successors = Vec::new();
    let children = &g.node(node).children;
    for edge in all_outgoing_edges(g, node) {
        let e = g.edge(edge);
        if !matches!(e.sources[0], ShapeId::Port(_)) {
            let target = g.shape_node(e.targets[0]);
            if !children.contains(&target) {
                successors.push(target);
            }
        }
    }
    successors
}

/// First child of the graph without incoming
/// edges.
pub fn find_root(g: &ElkGraph, graph: NodeId) -> Option<NodeId> {
    g.node(graph)
        .children
        .iter()
        .copied()
        .find(|&child| all_incoming_edges(g, child).is_empty())
}

pub fn find_root_of_node(g: &ElkGraph, node: NodeId) -> NodeId {
    match get_tree_parent(g, node) {
        Some(parent) => find_root_of_node(g, parent),
        None => node,
    }
}

pub fn get_number_of_leaves(g: &ElkGraph, node: NodeId) -> i32 {
    let successors = get_successors(g, node);
    if successors.is_empty() {
        1
    } else {
        successors.iter().map(|&c| get_number_of_leaves(g, c)).sum()
    }
}

/// Compares two nodes by their
/// polar angle derived from the `CoreOptions.POSITION` property.
///
/// When `POSITION` is unset the result is `(0, 0)`.
pub fn polar_compare(
    g: &ElkGraph,
    radial_offset: f64,
    node_offset_y: f64,
    node1: NodeId,
    node2: NodeId,
) -> Ordering {
    let arc_of = |node: NodeId| {
        let position = g.node(node).properties.get(&POSITION);
        let mut arc = (position.y + node_offset_y).atan2(position.x);
        if arc < 0.0 {
            arc += TWO_PI;
        }
        arc += radial_offset;
        if arc > TWO_PI {
            arc -= TWO_PI;
        }
        arc
    };
    fuzzy_compare(arc_of(node1), arc_of(node2), EPSILON)
}

pub fn find_largest_node_in_graph(g: &ElkGraph, graph: NodeId) -> f64 {
    let mut largest_child_size: f64 = 0.0;
    for &child in &g.node(graph).children {
        let shape = &g.node(child).shape;
        let diameter = (shape.width * shape.width + shape.height * shape.height).sqrt();
        largest_child_size = largest_child_size.max(diameter);
        largest_child_size = largest_child_size.max(find_largest_node_in_graph(g, child));
    }
    largest_child_size
}

pub fn get_next_level_nodes(g: &ElkGraph, nodes: &[NodeId]) -> Vec<NodeId> {
    let mut successors = Vec::new();
    for &node in nodes {
        successors.extend(get_successors(g, node));
    }
    successors
}

/// Deduplicated successors in insertion order, which is deterministic.
pub fn get_next_level_node_set(g: &ElkGraph, nodes: &[NodeId]) -> Vec<NodeId> {
    let mut successors = Vec::new();
    for &node in nodes {
        for s in get_successors(g, node) {
            if !successors.contains(&s) {
                successors.push(s);
            }
        }
    }
    successors
}

pub fn center_nodes_on_radi(g: &mut ElkGraph, node: NodeId, x_pos: f64, y_pos: f64) {
    let shape = &mut g.node_mut(node).shape;
    shape.x = x_pos - shape.width / 2.0;
    shape.y = y_pos - shape.height / 2.0;
}

#[allow(dead_code)]
pub fn shift_closest_edge_to_radi(g: &mut ElkGraph, node: NodeId, x_pos: f64, y_pos: f64) {
    let shape = &mut g.node_mut(node).shape;
    if fuzzy_equals(x_pos, 0.0, EPSILON) && fuzzy_equals(y_pos, 0.0, EPSILON) {
        // center root
        shape.x = x_pos - shape.width / 2.0;
        shape.y = y_pos - shape.height / 2.0;
    } else if x_pos < 0.0 {
        shape.x = x_pos - shape.width;
        shape.y = if y_pos < 0.0 { y_pos } else { y_pos + shape.height };
    } else {
        shape.x = x_pos;
        shape.y = if y_pos < 0.0 { y_pos } else { y_pos + shape.height };
    }
}

/// Source of the first incoming edge.
pub fn get_tree_parent(g: &ElkGraph, node: NodeId) -> Option<NodeId> {
    all_incoming_edges(g, node)
        .first()
        .map(|&edge| g.shape_node(g.edge(edge).sources[0]))
}
