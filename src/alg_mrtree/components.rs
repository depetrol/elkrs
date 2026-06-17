//! Splits a tree
//! graph into connected components and packs them after layout.

use crate::graph::math::KVector;

use crate::alg_mrtree::graph::{TArena, TEdgeId, TGraph, TNodeId};
use crate::alg_mrtree::intermediate;
use crate::alg_mrtree::options;

pub fn split(arena: &mut TArena, graph: TGraph) -> Vec<TGraph> {
    let separate: bool = graph.properties.get(&options::SEPARATE_CONNECTED_COMPONENTS);
    if separate {
        // initialize adjacency lists (indexed by the import-time node id)
        let n = graph.nodes.len();
        let mut incidence: Vec<Vec<TEdgeId>> = vec![Vec::new(); n];
        for &edge in &graph.edges {
            let e = arena.edge(edge);
            incidence[arena.node(e.source).id as usize].push(edge);
            incidence[arena.node(e.target).id as usize].push(edge);
        }
        let mut visited = vec![false; n];

        // perform DFS starting on each node, collecting connected components
        let mut components: Vec<TGraph> = Vec::new();
        for &node in &graph.nodes {
            if !visited[arena.node(node).id as usize] {
                let mut comp = TGraph::default();
                dfs(arena, node, &mut comp, &mut visited, &incidence);
                comp.properties.copy_from(&graph.properties);
                comp.origin = graph.origin;
                components.push(comp);
            }
        }

        // redistribute identifier numbers to each component
        if components.len() > 1 {
            for comp in &components {
                let mut id = 0;
                for &node in &comp.nodes {
                    arena.node_mut(node).id = id;
                    id += 1;
                }
            }
        }
        return components;
    }
    vec![graph]
}

/// Every edge ends up in the
/// component's edge list **twice** (once per endpoint visit); downstream code
/// dedupes with `distinct()` where it matters.
fn dfs(
    arena: &TArena,
    node: TNodeId,
    component: &mut TGraph,
    visited: &mut [bool],
    incidence: &[Vec<TEdgeId>],
) {
    let id = arena.node(node).id as usize;
    if !visited[id] {
        visited[id] = true;
        component.nodes.push(node);
        for &edge in &incidence[id] {
            let (source, target) = {
                let e = arena.edge(edge);
                (e.source, e.target)
            };
            if source != node {
                dfs(arena, source, component, visited, incidence);
            }
            if target != node {
                dfs(arena, target, component, visited, incidence);
            }
            component.edges.push(edge);
        }
    }
}

pub fn pack(arena: &mut TArena, mut components: Vec<TGraph>) -> TGraph {
    if components.len() == 1 {
        let mut g = components.pop().unwrap();
        apply_padding_and_normalize_positions(arena, &mut g);
        return g;
    } else if components.is_empty() {
        return TGraph::default();
    }

    // assign priorities and sizes
    for graph in &mut components {
        let mut priority: i32 = 0;
        let mut minx = 2147483647.0f64; // Integer.MAX_VALUE
        let mut miny = 2147483647.0f64;
        let mut maxx = -2147483648.0f64; // Integer.MIN_VALUE
        let mut maxy = -2147483648.0f64;
        for &node in &graph.nodes {
            let n = arena.node(node);
            priority = priority.wrapping_add(n.properties.get(&options::PRIORITY));
            minx = f64::min(minx, n.pos.x);
            miny = f64::min(miny, n.pos.y);
            maxx = f64::max(maxx, n.pos.x + n.size.x);
            maxy = f64::max(maxy, n.pos.y + n.size.y);
        }
        graph.priority = priority;
        graph.bb_upleft = KVector::new(minx, miny);
        graph.bb_lowright = KVector::new(maxx, maxy);
    }

    // sort the components by their priority and size (stable, like
    // Collections.sort)
    components.sort_by(|graph1, graph2| {
        let prio = graph2.priority.wrapping_sub(graph1.priority);
        if prio == 0 {
            let size1 = bb_size(graph1);
            let size2 = bb_size(graph2);
            (size1.x * size1.y).total_cmp(&(size2.x * size2.y))
        } else {
            prio.cmp(&0)
        }
    });

    let mut result = TGraph::default();
    result.properties.copy_from(&components[0].properties);
    result.origin = components[0].origin;

    // determine the maximal row width by the maximal box width and the total
    // area
    let mut max_row_width = 0.0f64;
    let mut total_area = 0.0f64;
    for graph in &components {
        let size = bb_size(graph);
        max_row_width = f64::max(max_row_width, size.x);
        total_area += size.x * size.y;
    }
    // note the float cast
    let aspect_ratio: f64 = result.properties.get(&options::ASPECT_RATIO);
    max_row_width = f64::max(max_row_width, (total_area.sqrt() as f32 as f64) * aspect_ratio);
    let spacing: f64 = result.properties.get(&options::SPACING_NODE_NODE);

    // place nodes iteratively into rows
    let mut xpos = 0.0f64;
    let mut ypos = 0.0f64;
    let mut highest_box = 0.0f64;
    let mut broadest_row = spacing;
    for graph in &components {
        let size = bb_size(graph);
        if xpos + size.x > max_row_width {
            // place the graph into the next row
            xpos = 0.0;
            ypos += highest_box + spacing;
            highest_box = 0.0;
        }
        move_graph(arena, Some(&mut result), graph, xpos, ypos);
        broadest_row = f64::max(broadest_row, xpos + size.x);
        highest_box = f64::max(highest_box, size.y);
        xpos += size.x + spacing;
    }
    let _ = broadest_row; // dead

    // Property merge across components. Since the merged values are
    // all copies of the same input-graph properties, plain overwrite is
    // observably equivalent here and the merged map is never copied back to
    // the output graph anyway.
    for tgraph in &components {
        result.properties.copy_from(&tgraph.properties);
    }

    // We need to recompute the graph's bounds, since each component only
    // knows its own bounds and the prop merge cannot catch this.
    intermediate::graph_bounds_processor(arena, &mut result);
    // Move the resulting graph to 0,0 and apply padding
    apply_padding_and_normalize_positions(arena, &mut result);

    result
}

fn bb_size(graph: &TGraph) -> KVector {
    let mut size = graph.bb_lowright;
    size.sub(graph.bb_upleft);
    size
}

fn apply_padding_and_normalize_positions(arena: &mut TArena, g: &mut TGraph) {
    let padding = g.properties.get(&options::PADDING);
    g.bb_upleft = KVector::new(0.0, 0.0);
    let offsetx = padding.left - g.graph_xmin;
    let offsety = padding.top - g.graph_ymin;
    // The graph is moved into a throwaway destination graph here.
    move_graph(arena, None, g, offsetx, offsety);
}

fn move_graph(
    arena: &mut TArena,
    mut dest_graph: Option<&mut TGraph>,
    source_graph: &TGraph,
    offsetx: f64,
    offsety: f64,
) {
    let mut graph_offset = KVector::new(offsetx, offsety);
    graph_offset.sub(source_graph.bb_upleft);

    for &node in &source_graph.nodes {
        arena.node_mut(node).pos.add(graph_offset);
        if let Some(dest) = dest_graph.as_deref_mut() {
            dest.nodes.push(node);
        }
    }

    let mut seen: Vec<TEdgeId> = Vec::new();
    for &edge in &source_graph.edges {
        if seen.contains(&edge) {
            continue;
        }
        seen.push(edge);
        for bendpoint in arena.edge_mut(edge).bend_points.0.iter_mut() {
            bendpoint.add(graph_offset);
        }
        if let Some(dest) = dest_graph.as_deref_mut() {
            dest.edges.push(edge);
        }
    }
}
