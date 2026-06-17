//! Splits a force
//! graph into connected components and packs them back together after layout.

use crate::graph::math::KVector;

use crate::alg_force::graph::{FArena, FEdgeId, FGraph, FNodeId};
use crate::alg_force::options;

pub fn split(arena: &mut FArena, graph: FGraph) -> Vec<FGraph> {
    let separate: bool = graph.properties.get(&options::SEPARATE_CONNECTED_COMPONENTS);
    if separate {
        let mut visited = vec![false; graph.nodes.len()];
        let incidence = build_incidence_lists(arena, &graph);

        // perform DFS starting on each node, collecting connected components
        let mut components: Vec<FGraph> = Vec::new();
        for &node in &graph.nodes {
            if !visited[arena.node(node).id as usize] {
                let mut comp = FGraph::default();
                dfs(arena, node, None, &mut comp, &mut visited, &incidence);
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

fn build_incidence_lists(arena: &FArena, graph: &FGraph) -> Vec<Vec<FEdgeId>> {
    let n = graph.nodes.len();
    let mut incidence: Vec<Vec<FEdgeId>> = vec![Vec::new(); n];
    for &edge in &graph.edges {
        let e = arena.edge(edge);
        incidence[arena.node(e.source).id as usize].push(edge);
        incidence[arena.node(e.target).id as usize].push(edge);
    }
    incidence
}

/// This adds an edge to the
/// component once per traversal in which it is not skipped, which can add the
/// same edge (and its labels) more than once in cyclic graphs.
fn dfs(
    arena: &FArena,
    node: FNodeId,
    last: Option<FNodeId>,
    component: &mut FGraph,
    visited: &mut [bool],
    incidence: &[Vec<FEdgeId>],
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
            if Some(target) == last || Some(source) == last {
                // Do not handle again the edge we just arrived from
                continue;
            }
            if source != node {
                dfs(arena, source, Some(node), component, visited, incidence);
            }
            if target != node {
                dfs(arena, target, Some(node), component, visited, incidence);
            }
            component.edges.push(edge);
            component.labels.extend(arena.edge(edge).labels.iter().copied());
        }
    }
}

pub fn recombine(arena: &mut FArena, mut components: Vec<FGraph>) -> FGraph {
    if components.len() == 1 {
        return components.pop().unwrap();
    } else if components.is_empty() {
        return FGraph::default();
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
            // careful: the (x,y) of an FNode refers to its center
            minx = f64::min(minx, n.position.x - n.size.x / 2.0);
            miny = f64::min(miny, n.position.y - n.size.y / 2.0);
            maxx = f64::max(maxx, n.position.x + n.size.x / 2.0);
            maxy = f64::max(maxy, n.position.y + n.size.y / 2.0);
        }
        graph.properties.set(&options::PRIORITY, priority);
        graph.properties.set(&options::BB_UPLEFT, KVector::new(minx, miny));
        graph.properties.set(&options::BB_LOWRIGHT, KVector::new(maxx, maxy));
    }

    // sort the components by their priority and size (Collections.sort is
    // stable, as is Vec::sort_by)
    components.sort_by(|graph1, graph2| {
        let prio1: i32 = graph1.properties.get(&options::PRIORITY);
        let prio2: i32 = graph2.properties.get(&options::PRIORITY);
        let prio = prio2.wrapping_sub(prio1);
        if prio == 0 {
            let size1 = bb_size(graph1);
            let size2 = bb_size(graph2);
            // Double.compare
            (size1.x * size1.y).total_cmp(&(size2.x * size2.y))
        } else {
            prio.cmp(&0)
        }
    });

    let mut result = FGraph::default();
    result.properties.copy_from(&components[0].properties);
    result.origin = components[0].origin;

    // determine the maximal row width by the maximal box width and the total area
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
        move_graph(arena, &mut result, graph, xpos, ypos);
        broadest_row = f64::max(broadest_row, xpos + size.x);
        highest_box = f64::max(highest_box, size.y);
        xpos += size.x + spacing;
    }
    let _ = broadest_row; // dead

    result
}

fn bb_size(graph: &FGraph) -> KVector {
    let mut size: KVector = graph
        .properties
        .try_get(&options::BB_LOWRIGHT)
        .expect("bounding box not computed");
    size.sub(graph.properties.try_get(&options::BB_UPLEFT).unwrap());
    size
}

fn move_graph(
    arena: &mut FArena,
    dest_graph: &mut FGraph,
    source_graph: &FGraph,
    offsetx: f64,
    offsety: f64,
) {
    let mut graph_offset = KVector::new(offsetx, offsety);
    graph_offset.sub(source_graph.properties.try_get(&options::BB_UPLEFT).unwrap());

    for &node in &source_graph.nodes {
        arena.node_mut(node).position.add(graph_offset);
        dest_graph.nodes.push(node);
    }

    for &edge in &source_graph.edges {
        let bendpoints = arena.edge(edge).bendpoints.clone();
        for bendpoint in bendpoints {
            arena.bendpoint_mut(bendpoint).position.add(graph_offset);
        }
        dest_graph.edges.push(edge);
    }

    for &label in &source_graph.labels {
        arena.label_mut(label).position.add(graph_offset);
        dest_graph.labels.push(label);
    }
}
