
use std::collections::HashMap;

use crate::core::elkutil;
use crate::core::providers::fixed::all_outgoing_edges;
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::math::KVector;

use crate::alg_force::graph::{FArena, FGraph};
use crate::alg_force::options;

pub fn import_graph(g: &ElkGraph, kgraph: NodeId) -> Result<(FArena, FGraph), String> {
    let mut arena = FArena::default();
    let mut fgraph = FGraph::default();

    // copy the properties of the KGraph to the force graph
    fgraph.properties.copy_from(&g.node(kgraph).properties);
    fgraph.origin = Some(kgraph);

    let mut elem_map: HashMap<NodeId, crate::alg_force::graph::FNodeId> = HashMap::new();

    transform_nodes(g, kgraph, &mut arena, &mut fgraph, &mut elem_map);
    transform_edges(g, kgraph, &mut arena, &mut fgraph, &elem_map)?;

    Ok((arena, fgraph))
}

fn transform_nodes(
    g: &ElkGraph,
    parent_node: NodeId,
    arena: &mut FArena,
    fgraph: &mut FGraph,
    elem_map: &mut HashMap<NodeId, crate::alg_force::graph::FNodeId>,
) {
    let mut index = 0;
    for &knode in &g.node(parent_node).children {
        let label = match g.node(knode).labels.first() {
            Some(&l) => g.label(l).text.clone(),
            None => String::new(),
        };
        let new_node = arena.create_node(label);
        {
            let n = arena.node_mut(new_node);
            n.properties.copy_from(&g.node(knode).properties);
            n.origin = Some(knode);

            n.id = index;
            index += 1;
            let shape = &g.node(knode).shape;
            n.position.x = shape.x + shape.width / 2.0;
            n.position.y = shape.y + shape.height / 2.0;
            n.size.x = f64::max(shape.width, 1.0);
            n.size.y = f64::max(shape.height, 1.0);
        }

        fgraph.nodes.push(new_node);
        elem_map.insert(knode, new_node);

        // UNDEFINED port constraints would be normalized here, but the result
        // is never used (ports are not yet considered).
    }
}

fn transform_edges(
    g: &ElkGraph,
    parent_node: NodeId,
    arena: &mut FArena,
    fgraph: &mut FGraph,
    elem_map: &HashMap<NodeId, crate::alg_force::graph::FNodeId>,
) -> Result<(), String> {
    for &knode in &g.node(parent_node).children {
        for kedge in all_outgoing_edges(g, knode) {
            let e = g.edge(kedge);

            // We don't support hyperedges
            if e.sources.len() > 1 || e.targets.len() > 1 {
                return Err("Graph must not contain hyperedges.".to_string());
            }

            // exclude edges that pass hierarchy bounds as well as self-loops
            let target_node = g.shape_node(e.targets[0]);
            if !g.is_hierarchical(kedge) && knode != target_node {
                let source = *elem_map.get(&knode).expect("source not imported");
                let target = *elem_map.get(&target_node).expect("target not imported");
                let new_edge = arena.create_edge(source, target);
                arena
                    .edge_mut(new_edge)
                    .properties
                    .copy_from(&g.edge(kedge).properties);
                arena.edge_mut(new_edge).origin = Some(kedge);

                fgraph.edges.push(new_edge);

                // transform the edge's labels
                for &klabel in &g.edge(kedge).labels {
                    let new_label = arena.create_label(new_edge, g.label(klabel).text.clone());
                    {
                        let l = arena.label_mut(new_label);
                        l.properties.copy_from(&g.label(klabel).properties);
                        l.origin = Some(klabel);
                        let shape = &g.label(klabel).shape;
                        l.size.x = f64::max(shape.width, 1.0);
                        l.size.y = f64::max(shape.height, 1.0);
                    }
                    arena.refresh_label_position(new_label);

                    fgraph.labels.push(new_label);
                }
            }
        }
    }
    Ok(())
}

pub fn apply_layout(arena: &FArena, fgraph: &FGraph, g: &mut ElkGraph, _layout_node: NodeId) {
    let kgraph = fgraph.origin.expect("force graph without origin");

    // calculate the offset from border spacing and node distribution
    let mut min_x_pos = 2147483647.0f64; // Integer.MAX_VALUE
    let mut min_y_pos = 2147483647.0f64;
    let mut max_x_pos = -2147483648.0f64; // Integer.MIN_VALUE
    let mut max_y_pos = -2147483648.0f64;

    for &node in &fgraph.nodes {
        let pos = arena.node(node).position;
        let size = arena.node(node).size;
        min_x_pos = f64::min(min_x_pos, pos.x - size.x / 2.0);
        min_y_pos = f64::min(min_y_pos, pos.y - size.y / 2.0);
        max_x_pos = f64::max(max_x_pos, pos.x + size.x / 2.0);
        max_y_pos = f64::max(max_y_pos, pos.y + size.y / 2.0);
    }
    for &bendpoint in &fgraph.bendpoints {
        let pos = arena.bendpoint(bendpoint).position;
        let size = arena.bendpoint(bendpoint).size;
        min_x_pos = f64::min(min_x_pos, pos.x - size.x / 2.0);
        min_y_pos = f64::min(min_y_pos, pos.y - size.y / 2.0);
        max_x_pos = f64::max(max_x_pos, pos.x + size.x / 2.0);
        max_y_pos = f64::max(max_y_pos, pos.y + size.y / 2.0);
    }

    let padding = g.node(kgraph).properties.get(&options::PADDING);
    let offset = KVector::new(padding.left - min_x_pos, padding.top - min_y_pos);

    // process the nodes
    for &fnode in &fgraph.nodes {
        if let Some(knode) = arena.node(fnode).origin {
            let mut node_pos = arena.node(fnode).position;
            node_pos.add(offset);
            let (w, h) = (g.node(knode).shape.width, g.node(knode).shape.height);
            g.node_mut(knode)
                .shape
                .set_location(node_pos.x - w / 2.0, node_pos.y - h / 2.0);
        }
    }

    // process the edges
    for &fedge in &fgraph.edges {
        let kedge = arena.edge(fedge).origin.expect("force edge without origin");
        // reset the first section and remove all others.
        let kedge_section = g.first_edge_section(kedge, true);
        g.edge_mut(kedge).sections.truncate(1);

        let mut start_location = arena.edge_source_point(fedge);
        start_location.add(offset);
        g.section_mut(kedge_section)
            .set_start_location(start_location.x, start_location.y);

        for &bp in &arena.edge(fedge).bendpoints {
            let mut position = arena.bendpoint(bp).position;
            position.add(offset);
            g.section_mut(kedge_section)
                .bend_points
                .push((position.x, position.y));
        }

        let mut end_location = arena.edge_target_point(fedge);
        end_location.add(offset);
        g.section_mut(kedge_section)
            .set_end_location(end_location.x, end_location.y);
    }

    // process the labels
    for &flabel in &fgraph.labels {
        let klabel = arena.label(flabel).origin.expect("force label without origin");
        let mut label_pos = arena.label(flabel).position;
        label_pos.add(offset);
        g.label_mut(klabel).shape.set_location(label_pos.x, label_pos.y);
    }

    // set up the parent node
    let width = (max_x_pos - min_x_pos) + padding.horizontal();
    let height = (max_y_pos - min_y_pos) + padding.vertical();
    if !g.node(kgraph).properties.get(&options::NODE_SIZE_FIXED_GRAPH_SIZE) {
        elkutil::resize_node(g, kgraph, width, height, false, true);
    }
    g.node(kgraph)
        .properties
        .set(&options::CHILD_AREA_WIDTH, width - padding.horizontal());
    g.node(kgraph)
        .properties
        .set(&options::CHILD_AREA_HEIGHT, height - padding.vertical());
}
