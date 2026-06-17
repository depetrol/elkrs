
use crate::graph::graph::{EdgeId, ElkGraph, NodeId};
use crate::graph::math::KVector;

use crate::core::elkutil;
use crate::core::options::*;
use crate::core::registry::LayoutProvider;

#[derive(Default)]
pub struct FixedLayoutProvider;

impl LayoutProvider for FixedLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        let edge_routing: EdgeRouting = g.node(layout_node).properties.get(&EDGE_ROUTING);

        let mut maxx = 0.0f64;
        let mut maxy = 0.0f64;

        let children = g.node(layout_node).children.clone();
        for node in children.iter().copied() {
            // set the fixed position of the node, or leave it as it is
            let pos = g.node(node).properties.try_get(&fixed::POSITION);
            if let Some(pos) = pos {
                g.node_mut(node).shape.set_location(pos.x, pos.y);
                let constraints = g.node(node).properties.get(&NODE_SIZE_CONSTRAINTS);
                if constraints.contains(SizeConstraint::MINIMUM_SIZE) {
                    let min_size: KVector = g.node(node).properties.get(&NODE_SIZE_MINIMUM);
                    if min_size.x > 0.0 && min_size.y > 0.0 {
                        elkutil::resize_node(g, node, min_size.x, min_size.y, true, true);
                    }
                }
            }
            let n = &g.node(node).shape;
            maxx = f64::max(maxx, n.x + n.width);
            maxy = f64::max(maxy, n.y + n.height);
            let (node_x, node_y) = (n.x, n.y);

            // node labels
            let labels = g.node(node).labels.clone();
            for label in labels {
                let pos = g.label(label).properties.try_get(&fixed::POSITION);
                if let Some(pos) = pos {
                    g.label_mut(label).shape.set_location(pos.x, pos.y);
                }
                let l = &g.label(label).shape;
                maxx = f64::max(maxx, node_x + l.x + l.width);
                maxy = f64::max(maxy, node_y + l.y + l.height);
            }

            // ports
            let ports = g.node(node).ports.clone();
            for port in ports {
                let pos = g.port(port).properties.try_get(&fixed::POSITION);
                if let Some(pos) = pos {
                    g.port_mut(port).shape.set_location(pos.x, pos.y);
                }
                let p = &g.port(port).shape;
                let portx = node_x + p.x;
                let porty = node_y + p.y;
                maxx = f64::max(maxx, portx + p.width);
                maxy = f64::max(maxy, porty + p.height);

                let labels = g.port(port).labels.clone();
                for label in labels {
                    let pos = g.label(label).properties.try_get(&fixed::POSITION);
                    if let Some(pos) = pos {
                        g.label_mut(label).shape.set_location(pos.x, pos.y);
                    }
                    let l = &g.label(label).shape;
                    maxx = f64::max(maxx, portx + l.x + l.width);
                    maxy = f64::max(maxy, porty + l.y + l.height);
                }
            }

            // outgoing edges (from the node and its ports)
            for edge in all_outgoing_edges(g, node) {
                let maxv = process_edge(g, edge, edge_routing)?;
                maxx = f64::max(maxx, maxv.x);
                maxy = f64::max(maxy, maxv.y);
            }

            // incoming hierarchical edges
            for edge in all_incoming_edges(g, node) {
                let source_node = g.shape_node(g.edge(edge).sources[0]);
                if g.node(source_node).parent != Some(layout_node) {
                    let maxv = process_edge(g, edge, edge_routing)?;
                    maxx = f64::max(maxx, maxv.x);
                    maxy = f64::max(maxy, maxv.y);
                }
            }
        }

        // junction points for orthogonal routing
        if edge_routing == EdgeRouting::ORTHOGONAL {
            for node in children.iter().copied() {
                for edge in all_outgoing_edges(g, node) {
                    let junction_points = elkutil::determine_junction_points(g, edge);
                    if junction_points.is_empty() {
                        g.edge_mut(edge).properties.unset(&JUNCTION_POINTS);
                    } else {
                        g.edge_mut(edge).properties.set(&JUNCTION_POINTS, junction_points);
                    }
                }
            }
        }

        // set size of the parent node unless fixed
        if !g.node(layout_node).properties.get(&NODE_SIZE_FIXED_GRAPH_SIZE) {
            let padding = g.node(layout_node).properties.get(&fixed::PADDING);
            let new_width = maxx + padding.left + padding.right;
            let new_height = maxy + padding.top + padding.bottom;
            elkutil::resize_node(g, layout_node, new_width, new_height, true, true);
        }
        Ok(())
    }
}

/// Edges with the node or one of its ports as source.
pub fn all_outgoing_edges(g: &ElkGraph, node: NodeId) -> Vec<EdgeId> {
    let mut edges = g.node(node).outgoing_edges.clone();
    for &port in &g.node(node).ports {
        edges.extend(g.port(port).outgoing_edges.iter().copied());
    }
    edges
}

/// Edges with the node or one of its ports as target.
pub fn all_incoming_edges(g: &ElkGraph, node: NodeId) -> Vec<EdgeId> {
    let mut edges = g.node(node).incoming_edges.clone();
    for &port in &g.node(node).ports {
        edges.extend(g.port(port).incoming_edges.iter().copied());
    }
    edges
}

fn process_edge(g: &mut ElkGraph, edge: EdgeId, _edge_routing: EdgeRouting) -> Result<KVector, String> {
    let source_parent = g.node(g.shape_node(g.edge(edge).sources[0])).parent;
    let target_parent = g.node(g.shape_node(g.edge(edge).targets[0])).parent;
    let same_hierarchy = source_parent == target_parent;

    let mut maxv = KVector::default();
    let bend_points = g.edge(edge).properties.try_get(&BEND_POINTS);

    if let Some(bend_points) = bend_points {
        if bend_points.len() >= 2 {
            if g.edge(edge).sections.is_empty() {
                g.create_section(edge);
            } else if g.edge(edge).sections.len() > 1 {
                // An edge with multiple sections is an invalid state here;
                // fail loudly.
                return Err("FixedLayoutProvider: edge with multiple sections".to_string());
            }
            let section = g.edge(edge).sections[0];
            elkutil::apply_vector_chain(g, &bend_points, section);
        }
    }

    if same_hierarchy {
        for &section in &g.edge(edge).sections {
            for &(x, y) in &g.section(section).bend_points {
                maxv.x = f64::max(maxv.x, x);
                maxv.y = f64::max(maxv.y, y);
            }
        }
    }

    let labels = g.edge(edge).labels.clone();
    for label in labels {
        let pos = g.label(label).properties.try_get(&fixed::POSITION);
        if let Some(pos) = pos {
            g.label_mut(label).shape.set_location(pos.x, pos.y);
        }
        if same_hierarchy {
            let l = &g.label(label).shape;
            maxv.x = f64::max(maxv.x, l.x + l.width);
            maxv.y = f64::max(maxv.y, l.y + l.height);
        }
    }

    Ok(maxv)
}
