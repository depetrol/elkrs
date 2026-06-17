
use std::collections::HashMap;

use crate::core::elkutil;
use crate::core::providers::fixed::all_outgoing_edges;
use crate::graph::graph::{ElkGraph, NodeId};

use crate::alg_mrtree::graph::{TArena, TGraph, TNodeId};
use crate::alg_mrtree::options;

pub fn import_graph(g: &ElkGraph, elkgraph: NodeId) -> (TArena, TGraph) {
    let mut arena = TArena::default();
    let mut tgraph = TGraph::default();

    // copy the properties of the KGraph to the t-graph
    tgraph.properties.copy_from(&g.node(elkgraph).properties);
    tgraph.origin = Some(elkgraph);

    // keep a list of created nodes in the t-graph
    let mut elem_map: HashMap<NodeId, TNodeId> = HashMap::new();

    transform_nodes(g, elkgraph, &mut arena, &mut tgraph, &mut elem_map);
    transform_edges(g, elkgraph, &mut arena, &mut tgraph, &elem_map);

    (arena, tgraph)
}

fn transform_nodes(
    g: &ElkGraph,
    parent_node: NodeId,
    arena: &mut TArena,
    tgraph: &mut TGraph,
    elem_map: &mut HashMap<NodeId, TNodeId>,
) {
    let mut index = 0;
    for &elknode in &g.node(parent_node).children {
        // copy label
        let label = match g.node(elknode).labels.first() {
            Some(&l) => g.label(l).text.clone(),
            None => String::new(),
        };

        // create new tNode
        let new_node = arena.create_node(index, label);
        index += 1;
        {
            let shape = &g.node(elknode).shape;
            let n = arena.node_mut(new_node);
            n.properties.copy_from(&g.node(elknode).properties);
            n.origin = Some(elknode);

            n.pos.y = shape.y + shape.height / 2.0;
            n.size.x = f64::max(shape.width, 1.0);
            n.pos.x = shape.x + shape.width / 2.0;
            n.size.y = f64::max(shape.height, 1.0);
        }

        tgraph.nodes.push(new_node);
        elem_map.insert(elknode, new_node);
    }
}

/// Every source and target connects to the
/// same node.
fn is_selfloop(g: &ElkGraph, edge: crate::graph::graph::EdgeId) -> bool {
    let e = g.edge(edge);
    let mut nodes = e.sources.iter().chain(e.targets.iter()).map(|&s| g.shape_node(s));
    match nodes.next() {
        Some(first) => nodes.all(|n| n == first),
        None => true,
    }
}

fn transform_edges(
    g: &ElkGraph,
    parent_node: NodeId,
    arena: &mut TArena,
    tgraph: &mut TGraph,
    elem_map: &HashMap<NodeId, TNodeId>,
) {
    for &elknode in &g.node(parent_node).children {
        for elkedge in all_outgoing_edges(g, elknode) {
            // exclude edges that pass hierarchy bounds and self-loops
            // (isHierarchical is checked twice instead of isHyperedge,
            // so hyperedges are NOT excluded; targets[0] is used)
            if !g.is_hierarchical(elkedge) && !is_selfloop(g, elkedge) {
                // find the corresponding source and target tNode of edge
                let source = elem_map.get(&elknode).copied();
                let target =
                    elem_map.get(&g.shape_node(g.edge(elkedge).targets[0])).copied();

                if let (Some(source), Some(target)) = (source, target) {
                    // create an edge and add edge to tGraph
                    let new_edge = arena.create_edge(source, target);
                    arena.edge_mut(new_edge).origin = Some(elkedge);

                    // set properties of the new edge
                    arena
                        .edge_mut(new_edge)
                        .properties
                        .copy_from(&g.edge(elkedge).properties);

                    // update tNode accordingly
                    arena.node_mut(source).outgoing.push(new_edge);
                    arena.node_mut(target).incoming.push(new_edge);

                    tgraph.edges.push(new_edge);
                }
            }
        }
    }
}

pub fn apply_layout(arena: &TArena, tgraph: &TGraph, g: &mut ElkGraph) {
    // get the corresponding kGraph
    let elkgraph = tgraph.origin.expect("t-graph without origin");

    // calculate the offset from border spacing and node distribution
    let mut min_x_pos = 2147483647.0f64; // Integer.MAX_VALUE
    let mut min_y_pos = 2147483647.0f64;
    let mut max_x_pos = -2147483648.0f64; // Integer.MIN_VALUE
    let mut max_y_pos = -2147483648.0f64;
    for &tnode in &tgraph.nodes {
        let pos = arena.node(tnode).pos;
        let size = arena.node(tnode).size;
        min_x_pos = f64::min(min_x_pos, pos.x - size.x / 2.0);
        min_y_pos = f64::min(min_y_pos, pos.y - size.y / 2.0);
        max_x_pos = f64::max(max_x_pos, pos.x + size.x / 2.0);
        max_y_pos = f64::max(max_y_pos, pos.y + size.y / 2.0);
    }

    let padding = g.node(elkgraph).properties.get(&options::PADDING);

    // apply tNode positions to elkNodes
    for &tnode in &tgraph.nodes {
        if let Some(elknode) = arena.node(tnode).origin {
            let pos = arena.node(tnode).pos;
            g.node_mut(elknode).shape.set_location(pos.x, pos.y);
            // The entire tNode property map is copied back; internal
            // (unregistered) entries are invisible in JSON output, so only
            // the PropertyMap (which includes the registered treeLevel) is
            // copied here.
            let props = arena.node(tnode).properties.clone();
            g.node_mut(elknode).properties.copy_from(&props);
        }
    }

    // copy tEdge bendpoints to elkEdges
    for &tedge in &tgraph.edges {
        if let Some(elkedge) = arena.edge(tedge).origin {
            let bend_points = &arena.edge(tedge).bend_points;
            let edge_section = g.first_edge_section(elkedge, true);
            g.edge_mut(elkedge).sections.truncate(1);
            elkutil::apply_vector_chain(g, bend_points, edge_section);
        }
    }

    // set up the graph
    let width = max_x_pos - min_x_pos + padding.horizontal();
    let height = max_y_pos - min_y_pos + padding.vertical();
    if !g.node(elkgraph).properties.get(&options::NODE_SIZE_FIXED_GRAPH_SIZE) {
        elkutil::resize_node(g, elkgraph, width, height, false, false);
    }
    g.node(elkgraph)
        .properties
        .set(&options::CHILD_AREA_WIDTH, width - padding.horizontal());
    g.node(elkgraph)
        .properties
        .set(&options::CHILD_AREA_HEIGHT, height - padding.vertical());
}
