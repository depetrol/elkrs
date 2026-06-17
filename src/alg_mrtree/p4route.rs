
use indexmap::IndexMap;

use crate::core::options_gen::Direction;
use crate::graph::math::KVector;

use crate::alg_mrtree::graph::{TArena, TEdgeId, TGraph, TNodeId};
use crate::alg_mrtree::options;
use crate::alg_mrtree::options::EdgeRoutingMode;
use crate::alg_mrtree::tree_util;

const ONE_HALF: f64 = 0.5;
const STEEP_END_EDGE_THERESHOLD_DISTANCE: f64 = 50.0;
const STEEP_END_EDGE_RATIO: f64 = 5.3;
const STEEP_END_EDGE_SAMPLE_HEIGHT: f64 = 40.0;

fn java_stream_average(values: impl Iterator<Item = f64>) -> f64 {
    let mut sum = 0.0f64;
    let mut compensation = 0.0f64;
    let mut count: u64 = 0;
    let mut simple_sum = 0.0f64;
    for v in values {
        count += 1;
        simple_sum += v;
        let tmp = v - compensation;
        let velvel = sum + tmp;
        compensation = (velvel - sum) - tmp;
        sum = velvel;
    }
    let tmp = sum + compensation;
    let final_sum = if tmp.is_nan() && simple_sum.is_infinite() { simple_sum } else { tmp };
    final_sum / count as f64
}

pub fn process(arena: &mut TArena, graph: &TGraph) {
    let mode: EdgeRoutingMode = graph.properties.get(&options::EDGE_ROUTING_MODE);
    if mode == EdgeRoutingMode::MIDDLE_TO_MIDDLE {
        for &tedge in &graph.edges {
            middle_to_middle_edge_route(arena, tedge);
        }
    } else if mode == EdgeRoutingMode::AVOID_OVERLAP {
        avoid_overlap(arena, graph);

        // fallback
        for &e in &graph.edges {
            if arena.edge(e).bend_points.len() < 2 {
                middle_to_middle_edge_route(arena, e);
            }
        }
    }
}

fn middle_to_middle_edge_route(arena: &mut TArena, tedge: TEdgeId) {
    let (source, target) = {
        let e = arena.edge(tedge);
        (e.source, e.target)
    };
    let source_node = arena.node(source);
    let source_point = KVector::new(
        source_node.pos.x + source_node.size.x / 2.0,
        source_node.pos.y + source_node.size.y / 2.0,
    );
    let target_node = arena.node(target);
    let target_point = KVector::new(
        target_node.pos.x + target_node.size.x / 2.0,
        target_node.pos.y + target_node.size.y / 2.0,
    );
    let source_size = source_node.size;
    let target_size = target_node.size;

    let chain = &mut arena.edge_mut(tedge).bend_points;
    chain.0.insert(0, source_point);
    chain.0.push(target_point);

    // correct the source and target points (these are the aliased
    // first/last chain elements, mutated in order)
    let next = chain.0[1];
    let mut first = chain.0[0];
    tree_util::to_node_border(&mut first, next, source_size);
    chain.0[0] = first;

    let next = chain.0[chain.0.len() - 2];
    let last_idx = chain.0.len() - 1;
    let mut last = chain.0[last_idx];
    tree_util::to_node_border(&mut last, next, target_size);
    chain.0[last_idx] = last;
}

fn avoid_overlap(arena: &mut TArena, graph: &TGraph) {
    let _root = tree_util::get_root(arena, graph);
    let node_bendpoint_padding: f64 = graph.properties.get(&options::SPACING_EDGE_NODE);
    let edge_end_texture_padding: f64 = graph.properties.get(&options::EDGE_END_TEXTURE_LENGTH);
    let d: Direction = graph.properties.get(&options::DIRECTION);

    avoid_overlap_set_start_points(arena, graph, d, node_bendpoint_padding);

    avoid_overlap_special_edges(arena, graph, d, node_bendpoint_padding, edge_end_texture_padding);

    avoid_overlap_set_end_points(arena, graph, d, node_bendpoint_padding, edge_end_texture_padding);
}

// --------------------------------------------- MultiLevelEdgeNodeNodeGap

/// The bend
/// point `KVector` objects are registered by reference; this stores `(edge, index)`
/// of the first of the two bend points instead (the index stays valid since
/// later additions only append to the chain).
struct MultiLevelEdgeNodeNodeGap {
    neighbor_one: Option<TNodeId>,
    neighbor_two: Option<TNodeId>,
    bend_points: Vec<(TEdgeId, usize)>,
    d: Direction,
    node_bendpoint_padding: f64,
    on_first_node_side: bool,
    on_last_node_side: bool,
}

impl MultiLevelEdgeNodeNodeGap {
    fn new(
        arena: &mut TArena,
        graph: &TGraph,
        neighbor_one: Option<TNodeId>,
        neighbor_two: Option<TNodeId>,
        bend_triple: (TEdgeId, usize),
    ) -> Self {
        let mut gap = MultiLevelEdgeNodeNodeGap {
            neighbor_one,
            neighbor_two,
            bend_points: vec![bend_triple],
            d: graph.properties.get(&options::DIRECTION),
            node_bendpoint_padding: graph.properties.get(&options::SPACING_EDGE_NODE),
            on_first_node_side: false,
            on_last_node_side: false,
        };
        gap.update_bend_points(arena);
        gap
    }

    fn add_bend_points(&mut self, arena: &mut TArena, new_bends: (TEdgeId, usize)) {
        self.bend_points.push(new_bends);

        if self.d.is_horizontal() {
            self.bend_points.sort_by(|&(xe, _), &(ye, _)| {
                let xp = arena.node(arena.edge(xe).target).pos.y;
                let yp = arena.node(arena.edge(ye).target).pos.y;
                xp.total_cmp(&yp)
            });
        } else {
            self.bend_points.sort_by(|&(xe, _), &(ye, _)| {
                let xp = arena.node(arena.edge(xe).target).pos.x;
                let yp = arena.node(arena.edge(ye).target).pos.x;
                xp.total_cmp(&yp)
            });
        }

        self.update_bend_points(arena);
    }

    fn update_bend_points(&mut self, arena: &mut TArena) {
        let count = self.bend_points.len();
        let d = self.d;
        let pad = self.node_bendpoint_padding;
        for (i, &(e, idx)) in self.bend_points.iter().enumerate() {
            let i = i as f64;
            let interpolation = (i + 1.0) / (count as f64 + 1.0);
            let (bend1, bend2): (KVector, KVector);
            match (self.neighbor_one, self.neighbor_two) {
                (None, None) => return,
                (Some(n1), None) => {
                    // bend point should be placed next to the last node
                    self.on_last_node_side = true;
                    let n1n = arena.node(n1);
                    if d == Direction::LEFT {
                        let bend_tmp = n1n.pos.y + n1n.size.y + pad * (i + 1.0);
                        bend1 = KVector::new(n1n.level_max + pad, bend_tmp);
                        bend2 = KVector::new(n1n.level_min - pad, bend_tmp);
                    } else if d == Direction::RIGHT {
                        let bend_tmp = n1n.pos.y + n1n.size.y + pad * (i + 1.0);
                        bend1 = KVector::new(n1n.level_min - pad, bend_tmp);
                        bend2 = KVector::new(n1n.level_max + pad, bend_tmp);
                    } else if d == Direction::UP {
                        let bend_tmp = n1n.pos.x + n1n.size.x + pad * (i + 1.0);
                        bend1 = KVector::new(bend_tmp, n1n.level_max + pad);
                        bend2 = KVector::new(bend_tmp, n1n.level_min - pad);
                    } else {
                        let bend_tmp = n1n.pos.x + n1n.size.x + pad * (i + 1.0);
                        bend1 = KVector::new(bend_tmp, n1n.level_min - pad);
                        bend2 = KVector::new(bend_tmp, n1n.level_max + pad);
                    }
                }
                (Some(n1), Some(n2)) => {
                    // bend point should be placed in between two nodes
                    let n1n = arena.node(n1);
                    let n2n = arena.node(n2);
                    if d == Direction::LEFT {
                        let bend_tmp = n2n.pos.y * interpolation
                            + (n1n.pos.y + n1n.size.y) * (1.0 - interpolation);
                        bend1 = KVector::new(n1n.level_max + pad, bend_tmp);
                        bend2 = KVector::new(n1n.level_min - pad, bend_tmp);
                    } else if d == Direction::RIGHT {
                        let bend_tmp = n2n.pos.y * interpolation
                            + (n1n.pos.y + n1n.size.y) * (1.0 - interpolation);
                        bend1 = KVector::new(n1n.level_min - pad, bend_tmp);
                        bend2 = KVector::new(n1n.level_max + pad, bend_tmp);
                    } else if d == Direction::UP {
                        let bend_tmp = n2n.pos.x * interpolation
                            + (n1n.pos.x + n1n.size.x) * (1.0 - interpolation);
                        bend1 = KVector::new(bend_tmp, n1n.level_max + pad);
                        bend2 = KVector::new(bend_tmp, n1n.level_min - pad);
                    } else {
                        let bend_tmp = n2n.pos.x * interpolation
                            + (n1n.pos.x + n1n.size.x) * (1.0 - interpolation);
                        bend1 = KVector::new(bend_tmp, n1n.level_min - pad);
                        bend2 = KVector::new(bend_tmp, n1n.level_max + pad);
                    }
                }
                (None, Some(n2)) => {
                    // bend point should be placed next to the first node
                    self.on_first_node_side = true;
                    let n2n = arena.node(n2);
                    if d == Direction::LEFT {
                        let bend_tmp = n2n.pos.y - pad * (i + 1.0);
                        bend1 = KVector::new(n2n.level_max + pad, bend_tmp);
                        bend2 = KVector::new(n2n.level_min - pad, bend_tmp);
                    } else if d == Direction::RIGHT {
                        let bend_tmp = n2n.pos.y - pad * (i + 1.0);
                        bend1 = KVector::new(n2n.level_min - pad, bend_tmp);
                        bend2 = KVector::new(n2n.level_max + pad, bend_tmp);
                    } else if d == Direction::UP {
                        let bend_tmp = n2n.pos.x - pad * (i + 1.0);
                        bend1 = KVector::new(bend_tmp, n2n.level_max + pad);
                        bend2 = KVector::new(bend_tmp, n2n.level_min - pad);
                    } else {
                        let bend_tmp = n2n.pos.x - pad * (i + 1.0);
                        bend1 = KVector::new(bend_tmp, n2n.level_min - pad);
                        bend2 = KVector::new(bend_tmp, n2n.level_max + pad);
                    }
                }
            }

            // commit new values
            let chain = &mut arena.edge_mut(e).bend_points;
            chain.0[idx] = bend1;
            chain.0[idx + 1] = bend2;
        }
    }
}

// ------------------------------------------------------------ special edges

fn avoid_overlap_special_edges(
    arena: &mut TArena,
    graph: &TGraph,
    d: Direction,
    node_bendpoint_padding: f64,
    edge_end_texture_padding: f64,
) {
    // counts how many edges are routed along the sides of the tree
    let mut side_one_edges: i32 = 0;
    let mut side_two_edges: i32 = 0;
    let mut node_gaps: IndexMap<i64, MultiLevelEdgeNodeNodeGap> = IndexMap::new();

    let max_level = graph
        .nodes
        .iter()
        .map(|&x| arena.tree_level(x))
        .max()
        .expect("avoidOverlapSpecialEdges: empty graph")
        + 1;
    let mut outs_per_level = vec![0i32; max_level as usize];
    let mut ins_per_level = vec![0i32; max_level as usize];

    // distinct edges, preserving first-occurrence order
    let mut distinct_edges: Vec<TEdgeId> = Vec::new();
    for &e in &graph.edges {
        if !distinct_edges.contains(&e) {
            distinct_edges.push(e);
        }
    }

    for &e in &distinct_edges {
        let (e_source, e_target) = {
            let edge = arena.edge(e);
            (edge.source, edge.target)
        };
        let source_level = arena.tree_level(e_source);
        let target_level = arena.tree_level(e_target);
        let level_diff = target_level - source_level;
        if level_diff > 1 {
            // multi level edges
            'levels: for cur_level in (source_level + 1)..target_level {
                let mut next_level_nodes: Vec<TNodeId> = graph
                    .nodes
                    .iter()
                    .copied()
                    .filter(|&x| arena.tree_level(x) == cur_level)
                    .collect();
                // find the node gap in the next level through which we can
                // route our multi level edge
                let mut i: usize = 0;
                if d.is_horizontal() {
                    next_level_nodes
                        .sort_by(|&x, &y| arena.node(x).pos.y.total_cmp(&arena.node(y).pos.y));
                    while i < next_level_nodes.len() {
                        let interpolation = (cur_level - source_level) as f64
                            / (target_level - source_level) as f64;
                        if arena.node(next_level_nodes[i]).pos.y
                            > (arena.node(e_source).pos.y * (1.0 - interpolation)
                                + arena.node(e_target).pos.y * interpolation)
                        {
                            break;
                        }
                        i += 1;
                    }

                    // skip unnecessary level side bend points
                    if !next_level_nodes.is_empty() {
                        let start = if arena.edge(e).bend_points.is_empty() {
                            arena.node(e_source).pos
                        } else {
                            arena.edge(e).bend_points.last()
                        };
                        let last_node = arena.node(next_level_nodes[next_level_nodes.len() - 1]);
                        let mut last = last_node.pos;
                        last.add(last_node.size);
                        let first_node = arena.node(next_level_nodes[0]);
                        let mut first = first_node.pos;
                        first.add(first_node.size);
                        if i >= next_level_nodes.len() - 1
                            && start.y > last.y
                            && arena.node(e_target).pos.y > last.y
                        {
                            continue 'levels;
                        }
                        // (the `first.x` in the second comparison replicates
                        // a typo in the original)
                        if i == 0 && start.y < first.x && arena.node(e_target).pos.y < first.y {
                            continue 'levels;
                        }
                    }
                } else {
                    next_level_nodes
                        .sort_by(|&x, &y| arena.node(x).pos.x.total_cmp(&arena.node(y).pos.x));
                    while i < next_level_nodes.len() {
                        let interpolation = (cur_level - source_level) as f64
                            / (target_level - source_level) as f64;
                        if arena.node(next_level_nodes[i]).pos.x
                            > (arena.node(e_source).pos.x * (1.0 - interpolation)
                                + arena.node(e_target).pos.x * interpolation)
                        {
                            break;
                        }
                        i += 1;
                    }

                    // skip unnecessary level side bend points
                    if !next_level_nodes.is_empty() {
                        let start = if arena.edge(e).bend_points.is_empty() {
                            arena.node(e_source).pos
                        } else {
                            arena.edge(e).bend_points.last()
                        };
                        let last_node = arena.node(next_level_nodes[next_level_nodes.len() - 1]);
                        let mut last = last_node.pos;
                        last.add(last_node.size);
                        let first_node = arena.node(next_level_nodes[0]);
                        let mut first = first_node.pos;
                        first.add(first_node.size);
                        if i >= next_level_nodes.len() - 1
                            && start.x > last.x
                            && arena.node(e_target).pos.x > last.x
                        {
                            continue 'levels;
                        }
                        if i == 0 && start.x < first.x && arena.node(e_target).pos.x < first.x {
                            continue 'levels;
                        }
                    }
                }

                // add multi level edge gap to nodeGaps
                {
                    let chain = &mut arena.edge_mut(e).bend_points;
                    chain.0.push(KVector::default());
                    chain.0.push(KVector::default());
                }
                let idx = arena.edge(e).bend_points.len() - 2;
                let bend_triple = (e, idx);
                let key = tree_util::get_unique_long(cur_level, i as i32);
                if !node_gaps.contains_key(&key) {
                    let neighbor_one =
                        if i == 0 { None } else { Some(next_level_nodes[i - 1]) };
                    let neighbor_two = if i == next_level_nodes.len() {
                        None
                    } else {
                        Some(next_level_nodes[i])
                    };
                    let gap = MultiLevelEdgeNodeNodeGap::new(
                        arena,
                        graph,
                        neighbor_one,
                        neighbor_two,
                        bend_triple,
                    );
                    node_gaps.insert(key, gap);
                } else {
                    node_gaps.get_mut(&key).unwrap().add_bend_points(arena, bend_triple);
                }
                let gap = &node_gaps[&key];

                // increment end of level edge counters
                if !d.is_horizontal() {
                    if gap.on_first_node_side {
                        let n2 = arena.node(gap.neighbor_two.unwrap());
                        if n2.pos.x <= graph.graph_xmin {
                            side_one_edges += 1;
                        }
                    }
                    if gap.on_last_node_side {
                        let n1 = arena.node(gap.neighbor_one.unwrap());
                        if n1.pos.x + n1.size.x >= graph.graph_xmax {
                            side_two_edges += 1;
                        }
                    }
                } else {
                    if gap.on_first_node_side {
                        let n2 = arena.node(gap.neighbor_two.unwrap());
                        if n2.pos.y <= graph.graph_ymin {
                            side_one_edges += 1;
                        }
                    }
                    if gap.on_last_node_side {
                        let n1 = arena.node(gap.neighbor_one.unwrap());
                        if n1.pos.y + n1.size.y >= graph.graph_ymax {
                            side_two_edges += 1;
                        }
                    }
                }
            }
        } else if level_diff == 0 {
            // fallback for edges that come from and go to the same level
            middle_to_middle_edge_route(arena, e);
        } else if level_diff < 0 {
            outs_per_level[source_level as usize] += 1;
            ins_per_level[target_level as usize] += 1;
            let sides = avoid_overlap_handle_cycle_inducing_edges(
                arena,
                e,
                d,
                graph,
                (side_one_edges, side_two_edges),
                node_bendpoint_padding,
                edge_end_texture_padding,
                (ins_per_level[target_level as usize], outs_per_level[source_level as usize]),
            );
            side_one_edges = sides.0;
            side_two_edges = sides.1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn avoid_overlap_handle_cycle_inducing_edges(
    arena: &mut TArena,
    e: TEdgeId,
    d: Direction,
    graph: &TGraph,
    side_edges: (i32, i32),
    node_bendpoint_padding: f64,
    edge_end_texture_padding: f64,
    in_outs: (i32, i32),
) -> (i32, i32) {
    let mut side_one_edges = side_edges.0;
    let mut side_two_edges = side_edges.1;

    let (s, t) = {
        let edge = arena.edge(e);
        (edge.source, edge.target)
    };
    // compute bendTmp, the coordinate at the end of the graph the edge will
    // be routed along
    let bend_tmp: f64;
    if d.is_horizontal() {
        let middle_tree = java_stream_average(
            graph.nodes.iter().map(|&x| arena.node(x).pos.y + arena.node(x).size.y / 2.0),
        );
        if arena.node(s).pos.y + arena.node(s).size.y / 2.0 > middle_tree {
            side_two_edges += 1;
            let k = side_two_edges as f64;
            bend_tmp = graph
                .nodes
                .iter()
                .map(|&x| arena.node(x).pos.y + arena.node(x).size.y + node_bendpoint_padding * k)
                .fold(f64::NEG_INFINITY, f64::max);
        } else {
            side_one_edges += 1;
            let k = side_one_edges as f64;
            bend_tmp = graph
                .nodes
                .iter()
                .map(|&x| arena.node(x).pos.y - node_bendpoint_padding * k)
                .fold(f64::INFINITY, f64::min);
        }
    } else {
        let middle_tree = java_stream_average(
            graph.nodes.iter().map(|&x| arena.node(x).pos.x + arena.node(x).size.x / 2.0),
        );
        if arena.node(s).pos.x + arena.node(s).size.x / 2.0 > middle_tree {
            side_two_edges += 1;
            let k = side_two_edges as f64;
            bend_tmp = graph
                .nodes
                .iter()
                .map(|&x| arena.node(x).pos.x + arena.node(x).size.x + node_bendpoint_padding * k)
                .fold(f64::NEG_INFINITY, f64::max);
        } else {
            side_one_edges += 1;
            let k = side_one_edges as f64;
            bend_tmp = graph
                .nodes
                .iter()
                .map(|&x| arena.node(x).pos.x - node_bendpoint_padding * k)
                .fold(f64::INFINITY, f64::min);
        }
    }

    // set bend points
    let s_level_min = arena.node(s).level_min;
    let s_level_max = arena.node(s).level_max;
    let s_pos = arena.node(s).pos;
    let s_size = arena.node(s).size;
    let t_pos = arena.node(t).pos;
    let t_size = arena.node(t).size;
    let chain = &mut arena.edge_mut(e).bend_points;
    if d == Direction::LEFT {
        chain.add(s_level_min - node_bendpoint_padding, bend_tmp);
        chain.add(
            t_pos.x + t_size.x + node_bendpoint_padding + edge_end_texture_padding,
            bend_tmp,
        );
        chain.add(
            t_pos.x + t_size.x + node_bendpoint_padding + edge_end_texture_padding,
            t_pos.y + t_size.y / 2.0,
        );
        chain.add(t_pos.x + t_size.x, t_pos.y + t_size.y / 2.0);
    } else if d == Direction::RIGHT {
        chain.add(s_level_max + node_bendpoint_padding, s_pos.y + s_size.y / 2.0);
        chain.add(s_pos.x + s_size.x + node_bendpoint_padding, bend_tmp);
        chain.add(t_pos.x - node_bendpoint_padding - edge_end_texture_padding, bend_tmp);
        chain.add(
            t_pos.x - node_bendpoint_padding - edge_end_texture_padding,
            t_pos.y + t_size.y / 2.0,
        );
        chain.add(t_pos.x, t_pos.y + t_size.y / 2.0);
    } else if d == Direction::UP {
        chain.add(bend_tmp, s_level_min - node_bendpoint_padding);
        chain.add(
            bend_tmp,
            t_pos.y + t_size.y + node_bendpoint_padding + edge_end_texture_padding,
        );
        chain.add(
            t_pos.x + t_size.x / 2.0,
            t_pos.y + t_size.y + node_bendpoint_padding + edge_end_texture_padding,
        );
        chain.add(t_pos.x + t_size.x / 2.0, t_pos.y + t_size.y + node_bendpoint_padding);
    } else {
        if !chain.is_empty() {
            let last = chain.0.len() - 1;
            chain.0[last].y = s_level_max + node_bendpoint_padding * in_outs.1 as f64;
        }
        chain.add(bend_tmp, s_level_max + node_bendpoint_padding * in_outs.1 as f64);
        chain.add(
            bend_tmp,
            t_pos.y - node_bendpoint_padding * in_outs.0 as f64 - edge_end_texture_padding,
        );
    }

    (side_one_edges, side_two_edges)
}

// ------------------------------------------------------------- start points

fn avoid_overlap_set_start_points(
    arena: &mut TArena,
    graph: &TGraph,
    d: Direction,
    node_bendpoint_padding: f64,
) {
    for &n in &graph.nodes {
        if arena.node(n).label == "SUPER_ROOT" {
            continue;
        }

        // get a list of all outgoing edges from n (not n.getOutgoingEdges(),
        // which may be outdated in certain scenarios)
        let mut outs = tree_util::get_all_outgoing_edges(arena, n, graph);
        if d.is_horizontal() {
            outs.sort_by(|&x, &y| {
                tree_util::get_first_point(arena, x)
                    .y
                    .total_cmp(&tree_util::get_first_point(arena, y).y)
            });
        } else {
            outs.sort_by(|&x, &y| {
                tree_util::get_first_point(arena, x)
                    .x
                    .total_cmp(&tree_util::get_first_point(arena, y).x)
            });
        }

        // set the bend points for all outs
        let num = outs.len();
        for (i, &out) in outs.iter().enumerate() {
            if arena.node(n).compact_level_ascension
                && !tree_util::is_cycle_inducing(arena, out, graph)
            {
                continue;
            }

            let interpolation =
                if num == 1 { ONE_HALF } else { (i + 1) as f64 / (num + 1) as f64 };
            let node = arena.node(n);
            let (p_node, p_level): (KVector, KVector);
            if d == Direction::LEFT {
                let level_end_coord = node.level_min;
                let y = node.pos.y + node.size.y * interpolation;
                p_level =
                    KVector::new(f64::min(level_end_coord, node.pos.x - node_bendpoint_padding), y);
                p_node = KVector::new(node.pos.x, y);
            } else if d == Direction::RIGHT {
                let level_end_coord = node.level_max + node_bendpoint_padding;
                let y = node.pos.y + node.size.y * interpolation;
                p_level = KVector::new(level_end_coord, y);
                p_node = KVector::new(node.pos.x + node.size.x, y);
            } else if d == Direction::UP {
                let level_end_coord = node.level_min;
                let x = node.pos.x + node.size.x * interpolation;
                p_level =
                    KVector::new(x, f64::min(node.pos.y - node_bendpoint_padding, level_end_coord));
                p_node = KVector::new(x, node.pos.y);
            } else {
                let level_end_coord = node.level_max + node_bendpoint_padding;
                let x = node.pos.x + node.size.x * interpolation;
                p_level = KVector::new(x, level_end_coord);
                p_node = KVector::new(x, node.pos.y + node.size.y);
            }
            let chain = &mut arena.edge_mut(out).bend_points;
            chain.0.insert(0, p_level);
            chain.0.insert(0, p_node);
        }
    }
}

// --------------------------------------------------------------- end points

fn avoid_overlap_set_end_points(
    arena: &mut TArena,
    graph: &TGraph,
    d: Direction,
    node_bendpoint_padding: f64,
    edge_end_texture_padding: f64,
) {
    for &n in &graph.nodes {
        if arena.node(n).label == "SUPER_ROOT" {
            continue;
        }

        // get the incoming edges and sort them by their current bend points
        let mut ins = tree_util::get_all_incoming_edges(arena, n, graph);
        if d.is_horizontal() {
            ins.sort_by(|&x, &y| {
                tree_util::get_last_point(arena, x)
                    .y
                    .total_cmp(&tree_util::get_last_point(arena, y).y)
            });
        } else {
            ins.sort_by(|&x, &y| {
                tree_util::get_last_point(arena, x)
                    .x
                    .total_cmp(&tree_util::get_last_point(arena, y).x)
            });
        }

        // set the bend points
        let num = ins.len();
        for (i, &in_edge) in ins.iter().enumerate() {
            let interpolation =
                if num == 1 { ONE_HALF } else { (1 + i) as f64 / (num + 1) as f64 };
            let node_pos = arena.node(n).pos;
            let node_size = arena.node(n).size;
            if d == Direction::LEFT {
                let level_start_coord = arena.node(n).level_max;
                // only add a level bend point if the distance is great enough
                if node_pos.x + node_size.x + edge_end_texture_padding < level_start_coord {
                    arena.edge_mut(in_edge).bend_points.add(
                        level_start_coord + node_bendpoint_padding,
                        node_pos.y + node_size.y * interpolation,
                    );
                // if the angle of the end piece is too steep, add another
                // bend point
                } else if !arena.edge(in_edge).bend_points.is_empty() {
                    let last = arena.edge(in_edge).bend_points.last();
                    let last_x = last.x;
                    let next_x = node_pos.x + node_size.x / 2.0;
                    let last_y = last.y;
                    let next_y = node_pos.y + node_size.y / 2.0;
                    if edge_end_texture_padding > 0.0
                        && (last_y - next_y).abs()
                            / ((last_x - next_x).abs() / STEEP_END_EDGE_SAMPLE_HEIGHT)
                            > STEEP_END_EDGE_THERESHOLD_DISTANCE
                    {
                        if next_y > last_y {
                            // place it to the left
                            arena.edge_mut(in_edge).bend_points.add(
                                node_pos.x + node_size.x
                                    + edge_end_texture_padding / STEEP_END_EDGE_RATIO,
                                node_pos.y + node_size.y * interpolation
                                    - edge_end_texture_padding / 2.0,
                            );
                        } else {
                            // place it to the right
                            arena.edge_mut(in_edge).bend_points.add(
                                node_pos.x + node_size.x
                                    + edge_end_texture_padding / STEEP_END_EDGE_RATIO,
                                node_pos.y + node_size.y * interpolation
                                    + edge_end_texture_padding / 2.0,
                            );
                        }
                    }
                }
                arena
                    .edge_mut(in_edge)
                    .bend_points
                    .add(node_pos.x + node_size.x, node_pos.y + node_size.y * interpolation);
            } else if d == Direction::RIGHT {
                let level_start_coord = arena.node(n).level_min;
                if node_pos.x - edge_end_texture_padding > level_start_coord {
                    arena.edge_mut(in_edge).bend_points.add(
                        level_start_coord - node_bendpoint_padding,
                        node_pos.y + node_size.y * interpolation,
                    );
                } else if !arena.edge(in_edge).bend_points.is_empty() {
                    let last = arena.edge(in_edge).bend_points.last();
                    let last_x = last.x;
                    let next_x = node_pos.x + node_size.x / 2.0;
                    let last_y = last.y;
                    let next_y = node_pos.y + node_size.y / 2.0;
                    if edge_end_texture_padding > 0.0
                        && (last_y - next_y).abs()
                            / ((last_x - next_x).abs() / STEEP_END_EDGE_SAMPLE_HEIGHT)
                            > STEEP_END_EDGE_THERESHOLD_DISTANCE
                    {
                        if next_y > last_y {
                            arena.edge_mut(in_edge).bend_points.add(
                                node_pos.x - edge_end_texture_padding / STEEP_END_EDGE_RATIO,
                                node_pos.y + node_size.y * interpolation
                                    - edge_end_texture_padding / 2.0,
                            );
                        } else {
                            arena.edge_mut(in_edge).bend_points.add(
                                node_pos.x - edge_end_texture_padding / STEEP_END_EDGE_RATIO,
                                node_pos.y + node_size.y * interpolation
                                    + edge_end_texture_padding / 2.0,
                            );
                        }
                    }
                }
                arena
                    .edge_mut(in_edge)
                    .bend_points
                    .add(node_pos.x, node_pos.y + node_size.y * interpolation);
            } else if d == Direction::UP {
                let level_start_coord = arena.node(n).level_max;
                if node_pos.y + node_size.y + edge_end_texture_padding < level_start_coord {
                    arena.edge_mut(in_edge).bend_points.add(
                        node_pos.x + node_size.x * interpolation,
                        level_start_coord + node_bendpoint_padding,
                    );
                } else if !arena.edge(in_edge).bend_points.is_empty() {
                    let last = arena.edge(in_edge).bend_points.last();
                    let last_x = last.x;
                    let next_x = node_pos.x + node_size.x / 2.0;
                    let last_y = last.y;
                    let next_y = node_pos.y + node_size.y / 2.0;
                    if edge_end_texture_padding > 0.0
                        && (last_x - next_x).abs()
                            / ((last_y - next_y).abs() / STEEP_END_EDGE_SAMPLE_HEIGHT)
                            > STEEP_END_EDGE_THERESHOLD_DISTANCE
                    {
                        if next_x > last_x {
                            arena.edge_mut(in_edge).bend_points.add(
                                node_pos.x + node_size.x * interpolation
                                    - edge_end_texture_padding / 2.0,
                                node_pos.y
                                    + edge_end_texture_padding / STEEP_END_EDGE_RATIO
                                    + node_size.y,
                            );
                        } else {
                            arena.edge_mut(in_edge).bend_points.add(
                                node_pos.x + node_size.x * interpolation
                                    + edge_end_texture_padding / 2.0,
                                node_pos.y
                                    + edge_end_texture_padding / STEEP_END_EDGE_RATIO
                                    + node_size.y,
                            );
                        }
                    }
                }
                arena
                    .edge_mut(in_edge)
                    .bend_points
                    .add(node_pos.x + node_size.x * interpolation, node_pos.y + node_size.y);
            } else {
                let level_start_coord = arena.node(n).level_min;
                // for cycle inducing edges add the end piece
                if tree_util::is_cycle_inducing(arena, in_edge, graph) {
                    let last_y = arena.edge(in_edge).bend_points.last().y;
                    arena
                        .edge_mut(in_edge)
                        .bend_points
                        .add(node_pos.x + node_size.x * interpolation, last_y);
                // only add a level bend point if the distance is great enough
                } else if node_pos.y - edge_end_texture_padding > level_start_coord {
                    arena.edge_mut(in_edge).bend_points.add(
                        node_pos.x + node_size.x * interpolation,
                        level_start_coord - node_bendpoint_padding,
                    );
                // if the angle of the end piece is too steep, add another
                // bend point
                } else if !arena.edge(in_edge).bend_points.is_empty() {
                    let last = arena.edge(in_edge).bend_points.last();
                    let last_x = last.x;
                    let next_x = node_pos.x + node_size.x / 2.0;
                    let last_y = last.y;
                    let next_y = node_pos.y + node_size.y / 2.0;
                    if edge_end_texture_padding > 0.0
                        && (last_x - next_x).abs()
                            / ((last_y - next_y).abs() / STEEP_END_EDGE_SAMPLE_HEIGHT)
                            > STEEP_END_EDGE_THERESHOLD_DISTANCE
                    {
                        if next_x > last_x {
                            arena.edge_mut(in_edge).bend_points.add(
                                node_pos.x + node_size.x * interpolation
                                    - edge_end_texture_padding / 2.0,
                                node_pos.y - edge_end_texture_padding / STEEP_END_EDGE_RATIO,
                            );
                        } else {
                            arena.edge_mut(in_edge).bend_points.add(
                                node_pos.x + node_size.x * interpolation
                                    + edge_end_texture_padding / 2.0,
                                node_pos.y - edge_end_texture_padding / STEEP_END_EDGE_RATIO,
                            );
                        }
                    }
                }

                arena
                    .edge_mut(in_edge)
                    .bend_points
                    .add(node_pos.x + node_size.x * interpolation, node_pos.y);
            }
        }
    }
}
