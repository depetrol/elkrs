//! Shared support code for the graph-wrapping processors. Ports
//! `org.eclipse.elk.alg.layered.intermediate.wrapping.{GraphStats,
//! CuttingUtils, ICutIndexCalculator, MSDCutIndexHeuristic,
//! ARDCutIndexHeuristic}`.

use crate::core::options::{Direction, PortConstraints, PortSide};
use crate::core::util::IndividualSpacings;
use crate::graph::properties::PropertyHolder;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::CuttingStrategy;

/// Lazily computed values are eagerly computed here
/// (the cost is negligible and avoids interior mutability).
pub struct GraphStats {
    pub dar: f64,
    pub longest_path: usize,
    spacing: f64,
    in_layer_spacing: f64,
    widths: Vec<f64>,
    heights: Vec<f64>,
    cuts_allowed: Vec<bool>,
    max_width: f64,
    sum_width: f64,
    max_height: f64,
}

impl GraphStats {
    pub fn new(a: &LGraphArena, graph: LGraphId) -> GraphStats {
        let g = a.graph(graph);
        let dir: Direction = g.properties.get(&lopts::DIRECTION);
        let aspect_ratio: f64 = g.properties.get(&lopts::ASPECT_RATIO);
        let correction: f64 = g.properties.get(&lopts::WRAPPING_CORRECTION_FACTOR);
        let dar = if dir == Direction::LEFT || dir == Direction::RIGHT || dir == Direction::UNDEFINED
        {
            aspect_ratio * correction
        } else {
            1.0 / (aspect_ratio * correction)
        };

        let spacing: f64 = g.properties.get(&lopts::SPACING_NODE_NODE_BETWEEN_LAYERS);
        let in_layer_spacing: f64 = g.properties.get(&lopts::SPACING_NODE_NODE);
        let longest_path = g.layers.len();

        let mut gs = GraphStats {
            dar,
            longest_path,
            spacing,
            in_layer_spacing,
            widths: Vec::new(),
            heights: Vec::new(),
            cuts_allowed: Vec::new(),
            max_width: 0.0,
            sum_width: 0.0,
            max_height: 0.0,
        };

        // widths/heights
        let layers = a.graph(graph).layers.clone();
        gs.widths = layers.iter().map(|&l| gs.determine_layer_width(a, l)).collect();
        gs.heights = layers.iter().map(|&l| gs.determine_layer_height(a, l)).collect();

        // max and sum reductions over all layers (longest_path >= 1 guaranteed
        // at call sites that use them).
        gs.max_width = gs.widths.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        gs.sum_width = gs.widths.iter().copied().sum();
        gs.max_height = gs.heights.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        gs.init_cut_allowed(a, graph);
        gs
    }

    pub fn get_max_width(&self) -> f64 {
        self.max_width
    }
    pub fn get_sum_width(&self) -> f64 {
        self.sum_width
    }
    pub fn get_max_height(&self) -> f64 {
        self.max_height
    }
    pub fn get_widths(&self) -> &[f64] {
        &self.widths
    }
    pub fn get_heights(&self) -> &[f64] {
        &self.heights
    }

    fn determine_layer_width(&self, a: &LGraphArena, l: LayerId) -> f64 {
        let mut max_w: f64 = 0.0;
        for &n in &a.layer(l).nodes {
            let node = a.node(n);
            let nw = node.size.x + node.margin.right + node.margin.left + self.spacing;
            max_w = max_w.max(nw);
        }
        max_w
    }

    fn determine_layer_height(&self, a: &LGraphArena, l: LayerId) -> f64 {
        let mut l_h: f64 = 0.0;
        let nodes = a.layer(l).nodes.clone();
        for n in nodes {
            let node = a.node(n);
            l_h += node.size.y + node.margin.bottom + node.margin.top + self.in_layer_spacing;

            for inc in a.node_incoming_edges(n) {
                let src = a.edge_source_node(inc);
                if a.node(src).node_type == NodeType::NORTH_SOUTH_PORT {
                    if let Some(Origin::LNode(origin)) =
                        a.node(src).properties.try_get(&iprops::ORIGIN)
                    {
                        let o = a.node(origin);
                        l_h += o.size.y + o.margin.bottom + o.margin.top;
                    }
                }
            }
        }
        l_h
    }

    pub fn is_cut_allowed(&self, layer_index: usize) -> bool {
        self.cuts_allowed[layer_index]
    }

    fn init_cut_allowed(&mut self, a: &LGraphArena, graph: LGraphId) {
        let layers = a.graph(graph).layers.clone();
        let mut cuts_allowed = vec![false; layers.len()];

        if a.graph(graph).properties.has(&lopts::WRAPPING_VALIDIFY_FORBIDDEN_INDICES) {
            // user-specified forbidden indices: everything else stays at the
            // boolean default `false`. Only forbidden entries are set to false
            // on an all-false array, so the array stays all-false.
            let forbidden: Vec<i32> =
                a.graph(graph).properties.get(&lopts::WRAPPING_VALIDIFY_FORBIDDEN_INDICES);
            for f in forbidden {
                if f > 0 && (f as usize) < cuts_allowed.len() {
                    cuts_allowed[f as usize] = false;
                }
            }
        } else {
            // default behavior: cuts_allowed[0] = false, others computed
            for (i, &layer) in layers.iter().enumerate().skip(1) {
                cuts_allowed[i] = Self::is_cut_allowed_layer(a, layer);
            }
        }
        self.cuts_allowed = cuts_allowed;
    }

    fn is_cut_allowed_layer(a: &LGraphArena, layer: LayerId) -> bool {
        let mut n1: Option<LNodeId> = None;
        let mut n2: Option<LNodeId> = None;
        for &tgt in &a.layer(layer).nodes {
            for e in a.node_incoming_edges(tgt) {
                if n1.is_some() && n1 != Some(tgt) {
                    return false;
                }
                n1 = Some(tgt);
                let src = a.edge_source_node(e);
                if n2.is_some() && n2 != Some(src) {
                    return false;
                }
                n2 = Some(src);
            }
        }
        true
    }
}

pub trait CutIndexCalculator {
    fn get_cut_indexes(&self, a: &LGraphArena, graph: LGraphId, gs: &GraphStats) -> Vec<i32>;
    fn guarantee_valid(&self) -> bool;
}

pub struct ManualCutIndexCalculator;
impl CutIndexCalculator for ManualCutIndexCalculator {
    fn get_cut_indexes(&self, a: &LGraphArena, graph: LGraphId, _gs: &GraphStats) -> Vec<i32> {
        a.graph(graph).properties.try_get(&lopts::WRAPPING_CUTTING_CUTS).unwrap_or_default()
    }
    fn guarantee_valid(&self) -> bool {
        false
    }
}

pub struct ArdCutIndexHeuristic;
impl ArdCutIndexHeuristic {
    pub fn get_chunk_count(gs: &GraphStats) -> i32 {
        let rowsd = (gs.get_sum_width() / (gs.dar * gs.get_max_height())).sqrt();
        let mut rows = java_round(rowsd) as i32;
        rows = rows.min(gs.longest_path as i32);
        rows
    }
}
impl CutIndexCalculator for ArdCutIndexHeuristic {
    fn get_cut_indexes(&self, _a: &LGraphArena, _graph: LGraphId, gs: &GraphStats) -> Vec<i32> {
        let rows = Self::get_chunk_count(gs);
        let mut cuts = Vec::new();
        let step = gs.longest_path as f64 / rows as f64;
        for idx in 1..rows {
            cuts.push(java_round(idx as f64 * step) as i32);
        }
        cuts
    }
    fn guarantee_valid(&self) -> bool {
        false
    }
}

pub struct MsdCutIndexHeuristic;
impl CutIndexCalculator for MsdCutIndexHeuristic {
    fn get_cut_indexes(&self, a: &LGraphArena, graph: LGraphId, gs: &GraphStats) -> Vec<i32> {
        let widths = gs.get_widths();
        let heights = gs.get_heights();

        let mut width_at_index = vec![0.0_f64; widths.len()];
        width_at_index[0] = widths[0];
        let mut total = widths[0];
        for i in 1..widths.len() {
            width_at_index[i] = width_at_index[i - 1] + widths[i];
            total += widths[i];
        }

        let cut_cnt = ArdCutIndexHeuristic::get_chunk_count(gs) - 1;
        let freedom: i32 = a.graph(graph).properties.get(&lopts::WRAPPING_CUTTING_MSD_FREEDOM);

        let mut best_max_scale = f64::NEG_INFINITY;
        let mut best_cuts: Vec<i32> = Vec::new();

        let m_lo = 0.max(cut_cnt - freedom);
        let m_hi = (gs.longest_path as i32 - 1).min(cut_cnt + freedom);
        let mut m = m_lo;
        while m <= m_hi {
            let row_sum = total / (m as f64 + 1.0);
            let mut sum_so_far = 0.0_f64;
            let mut index = 1usize;
            let mut cuts: Vec<i32> = Vec::new();

            let mut width = f64::NEG_INFINITY;
            let mut last_cut_width = 0.0_f64;
            let mut height = 0.0_f64;
            let mut row_height_max = heights[0];

            if m == 0 {
                width = total;
                height = gs.get_max_height();
            } else {
                while index < gs.longest_path {
                    if width_at_index[index - 1] - sum_so_far >= row_sum {
                        cuts.push(index as i32);
                        width = width.max(width_at_index[index - 1] - last_cut_width);
                        height += row_height_max;
                        sum_so_far += width_at_index[index - 1] - sum_so_far;
                        last_cut_width = width_at_index[index - 1];
                        row_height_max = heights[index];
                    }
                    row_height_max = row_height_max.max(heights[index]);
                    index += 1;
                }
                height += row_height_max;
            }

            let max_scale = (1.0 / width).min((1.0 / gs.dar) / height);
            if max_scale > best_max_scale {
                best_max_scale = max_scale;
                best_cuts = cuts;
            }
            m += 1;
        }

        best_cuts
    }
    fn guarantee_valid(&self) -> bool {
        false
    }
}

/// Returns the configured cut-index calculator.
pub fn cut_index_calculator(a: &LGraphArena, graph: LGraphId) -> Box<dyn CutIndexCalculator> {
    match a.graph(graph).properties.get::<CuttingStrategy>(&lopts::WRAPPING_CUTTING_STRATEGY) {
        CuttingStrategy::MANUAL => Box::new(ManualCutIndexCalculator),
        CuttingStrategy::ARD => Box::new(ArdCutIndexHeuristic),
        CuttingStrategy::MSD => Box::new(MsdCutIndexHeuristic),
    }
}

/// `Math.round(double)` returns `floor(x + 0.5)` as a long.
pub fn java_round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

pub fn validify_indexes_greedily(gs: &GraphStats, cuts: &[i32]) -> Vec<i32> {
    let mut valid_cuts = Vec::new();
    let mut offset = 0i32;
    let longest = gs.longest_path as i32;
    for &c in cuts {
        let mut cut = c + offset;
        while cut < longest && !gs.is_cut_allowed(cut as usize) {
            cut += 1;
            offset += 1;
        }
        if cut >= longest {
            break;
        }
        valid_cuts.push(cut);
    }
    valid_cuts
}

pub fn validify_indexes_looking_back(gs: &GraphStats, desired_cuts: &[i32]) -> Vec<i32> {
    if desired_cuts.is_empty() {
        return Vec::new();
    }
    let mut valid_cuts: Vec<i32> = Vec::new();
    valid_cuts.push(i32::MIN);
    for i in 1..gs.longest_path {
        if gs.is_cut_allowed(i) {
            valid_cuts.push(i as i32);
        }
    }
    if valid_cuts.len() == 1 {
        return Vec::new();
    }
    valid_cuts.push(i32::MAX);
    validify_indexes_looking_back_inner(desired_cuts, &valid_cuts)
}

fn validify_indexes_looking_back_inner(desired_cuts: &[i32], valid_cuts: &[i32]) -> Vec<i32> {
    let mut final_cuts: Vec<i32> = Vec::new();
    let mut i_idx = 0usize;
    let mut c_idx = 0usize;
    let mut offset = 0i32;

    while i_idx < valid_cuts.len() - 1 && c_idx < desired_cuts.len() {
        let current = desired_cuts[c_idx] + offset;
        while valid_cuts[i_idx + 1] < current {
            i_idx += 1;
        }
        let mut select = 0usize;
        let dist_lower = current - valid_cuts[i_idx];
        let dist_higher = valid_cuts[i_idx + 1] - current;
        if dist_lower > dist_higher {
            select += 1;
        }
        final_cuts.push(valid_cuts[i_idx + select]);
        offset += valid_cuts[i_idx + select] - current;
        c_idx += 1;
        while c_idx < desired_cuts.len()
            && desired_cuts[c_idx] + offset <= valid_cuts[i_idx + select]
        {
            c_idx += 1;
        }
        i_idx += 1 + select;
    }
    final_cuts
}

/// Returns the chain of created edges.
pub fn insert_dummies(
    a: &mut LGraphArena,
    graph: LGraphId,
    original_edge: LEdgeId,
    offset_first_in_layer_dummy: usize,
) -> Vec<LEdgeId> {
    let edge_node_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_NODE);
    let additional_spacing: f64 =
        a.graph(graph).properties.get(&lopts::WRAPPING_ADDITIONAL_EDGE_SPACING);
    let mut is = IndividualSpacings::default();
    is.properties_mut()
        .set(&lopts::SPACING_EDGE_NODE, edge_node_spacing + additional_spacing);

    let mut edge = original_edge;
    let target_port = a.edge(edge).target;
    let src = a.edge_source_node(edge);
    let tgt = a.edge_target_node(edge);

    let layers = a.graph(graph).layers.clone();
    let src_index = layers.iter().position(|&l| Some(l) == a.node(src).layer).unwrap();
    let tgt_index = layers.iter().position(|&l| Some(l) == a.node(tgt).layer).unwrap();

    let mut created_edges = Vec::new();

    for i in src_index..=tgt_index {
        // create dummy node (not added to the layerless list)
        let dummy_node = a.create_node(graph);
        a.node_mut(dummy_node).node_type = NodeType::LONG_EDGE;
        a.node(dummy_node).properties.set(&iprops::ORIGIN, Origin::LEdge(edge));
        a.node(dummy_node).properties.set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
        a.node(dummy_node).properties.set(&lopts::SPACING_INDIVIDUAL, is.clone());

        let next_layer = layers[i];
        if i == src_index {
            let pos = a.layer(next_layer).nodes.len() - offset_first_in_layer_dummy;
            a.node_set_layer_at(dummy_node, Some(next_layer), pos);
        } else {
            a.node_set_layer(dummy_node, Some(next_layer));
        }

        let mut thickness: f64 = a.edge(edge).properties.get(&lopts::EDGE_THICKNESS);
        if thickness < 0.0 {
            thickness = 0.0;
            a.edge(edge).properties.set(&lopts::EDGE_THICKNESS, thickness);
        }
        a.node_mut(dummy_node).size.y = thickness;
        let port_pos = (thickness / 2.0).floor();

        let dummy_input = a.create_port();
        a.port_set_side(dummy_input, PortSide::WEST);
        a.port_set_node(dummy_input, Some(dummy_node));
        a.port_mut(dummy_input).pos.y = port_pos;

        let dummy_output = a.create_port();
        a.port_set_side(dummy_output, PortSide::EAST);
        a.port_set_node(dummy_output, Some(dummy_node));
        a.port_mut(dummy_output).pos.y = port_pos;

        a.edge_set_target(edge, Some(dummy_input));

        let dummy_edge = a.create_edge();
        let edge_props = a.edge(edge).properties.clone();
        a.edge_mut(dummy_edge).properties.copy_from(&edge_props);
        a.edge(dummy_edge).properties.unset(&lopts::JUNCTION_POINTS);
        a.edge_set_source(dummy_edge, Some(dummy_output));
        a.edge_set_target(dummy_edge, target_port);

        set_dummy_properties(a, dummy_node, edge, dummy_edge);
        created_edges.push(dummy_edge);
        edge = dummy_edge;
    }

    created_edges
}

fn set_dummy_properties(a: &mut LGraphArena, dummy: LNodeId, in_edge: LEdgeId, out_edge: LEdgeId) {
    let in_edge_source_node = a.edge_source_node(in_edge);
    if a.node(in_edge_source_node).node_type == NodeType::LONG_EDGE {
        let s = a.node(in_edge_source_node).properties.try_get(&iprops::LONG_EDGE_SOURCE);
        let t = a.node(in_edge_source_node).properties.try_get(&iprops::LONG_EDGE_TARGET);
        match s {
            Some(v) => a.node(dummy).properties.set(&iprops::LONG_EDGE_SOURCE, v),
            None => a.node(dummy).properties.unset(&iprops::LONG_EDGE_SOURCE),
        };
        match t {
            Some(v) => a.node(dummy).properties.set(&iprops::LONG_EDGE_TARGET, v),
            None => a.node(dummy).properties.unset(&iprops::LONG_EDGE_TARGET),
        };
    } else {
        let source = a.edge(in_edge).source.unwrap();
        let target = a.edge(out_edge).target.unwrap();
        a.node(dummy).properties.set(&iprops::LONG_EDGE_SOURCE, source);
        a.node(dummy).properties.set(&iprops::LONG_EDGE_TARGET, target);
    }
}
