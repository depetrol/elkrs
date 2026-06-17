//! Calculates cut points and inserts
//! BREAKING_POINT dummies into the layering (wrappingStrategy = MULTI_EDGE).

use crate::core::options::{PortConstraints, PortSide};
use std::collections::HashSet;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::{BPInfo, BPInfoStore};
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::ValidifyStrategy;
use crate::alg_layered::processors::wrapping_support as ws;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let gs = ws::GraphStats::new(a, graph);

    // #1 determine the cut points
    let icic = ws::cut_index_calculator(a, graph);
    let mut cuts = icic.get_cut_indexes(a, graph, &gs);

    // #2 improve cuts
    if a.graph(graph).properties.get(&lopts::WRAPPING_MULTI_EDGE_IMPROVE_CUTS) {
        cuts = improve_cuts(a, graph, &cuts);
    }

    // #3 if not guaranteed valid, validify
    if !icic.guarantee_valid() && a.graph(graph).properties.has(&lopts::WRAPPING_VALIDIFY_STRATEGY) {
        match a.graph(graph).properties.get::<ValidifyStrategy>(&lopts::WRAPPING_VALIDIFY_STRATEGY) {
            ValidifyStrategy::LOOK_BACK => cuts = ws::validify_indexes_looking_back(&gs, &cuts),
            ValidifyStrategy::GREEDY => cuts = ws::validify_indexes_greedily(&gs, &cuts),
            ValidifyStrategy::NO => {}
        }
    }

    if cuts.is_empty() {
        return Ok(());
    }

    // #4 insert the breaking points
    apply_cuts(a, graph, &cuts);
    Ok(())
}

fn apply_cuts(a: &mut LGraphArena, graph: LGraphId, cuts: &[i32]) {
    let mut store = BPInfoStore::default();

    let mut cut_it = cuts.iter().copied();
    let mut idx = 0i32;
    let mut cut = cut_it.next().unwrap();

    let mut already_split: HashSet<LEdgeId> = HashSet::new();
    // 'open' edges, insertion-ordered
    let mut open_edges: Vec<LEdgeId> = Vec::new();

    // We iterate over a snapshot of layer ids while inserting new layers, with
    // `idx` decoupled from list positions. We do this by walking the original
    // layer list and tracking insertion.
    let mut layer_list = a.graph(graph).layers.clone();
    let mut li = 0usize;
    while li < layer_list.len() {
        let layer = layer_list[li];
        // number of extra layers inserted at this position (skipped, since the
        // cursor sits past the inserted layers).
        let mut inserted = 0usize;

        // book keeping of 'open' edges
        let nodes = a.layer(layer).nodes.clone();
        for n in &nodes {
            for e in a.node_outgoing_edges(*n) {
                if !open_edges.contains(&e) {
                    open_edges.push(e);
                }
            }
            for e in a.node_incoming_edges(*n) {
                open_edges.retain(|&x| x != e);
            }
        }

        if idx + 1 == cut {
            // insert two new layers right after the current one
            let bp_layer1 = a.create_layer(graph);
            let bp_layer2 = a.create_layer(graph);
            // both inserts go after the current element and advance the cursor,
            // so they land at li+1 and li+2.
            layer_list.insert(li + 1, bp_layer1);
            layer_list.insert(li + 2, bp_layer2);
            a.graph_mut(graph).layers = layer_list.clone();
            inserted = 2;

            for &original_edge in &open_edges.clone() {
                if !already_split.contains(&original_edge) {
                    already_split.insert(original_edge);
                }

                // start dummy
                let bp_start = a.create_node(graph);
                a.node(bp_start)
                    .properties
                    .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_SIDE);
                a.node_set_layer(bp_start, Some(bp_layer1));
                a.node_mut(bp_start).node_type = NodeType::BREAKING_POINT;
                let in_port_bp1 = a.create_port();
                a.port_set_node(in_port_bp1, Some(bp_start));
                a.port_set_side(in_port_bp1, PortSide::WEST);
                let out_port_bp1 = a.create_port();
                a.port_set_node(out_port_bp1, Some(bp_start));
                a.port_set_side(out_port_bp1, PortSide::EAST);

                // end dummy
                let bp_end = a.create_node(graph);
                a.node(bp_end)
                    .properties
                    .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_SIDE);
                a.node_set_layer(bp_end, Some(bp_layer2));
                a.node_mut(bp_end).node_type = NodeType::BREAKING_POINT;
                let in_port_bp2 = a.create_port();
                a.port_set_node(in_port_bp2, Some(bp_end));
                a.port_set_side(in_port_bp2, PortSide::WEST);
                let out_port_bp2 = a.create_port();
                a.port_set_node(out_port_bp2, Some(bp_end));
                a.port_set_side(out_port_bp2, PortSide::EAST);

                let orig_source = a.edge(original_edge).source;
                let orig_model_order = a.edge(original_edge).properties.try_get(&iprops::MODEL_ORDER);

                // first dummy edge: source -> inPortBp1
                let node_start_edge = a.create_edge();
                a.edge_set_source(node_start_edge, orig_source);
                a.edge_set_target(node_start_edge, Some(in_port_bp1));
                if let Some(mo) = orig_model_order {
                    a.edge(node_start_edge).properties.set(&iprops::MODEL_ORDER, mo);
                }

                // second dummy edge: outPortBp1 -> inPortBp2
                let start_end_edge = a.create_edge();
                a.edge_set_source(start_end_edge, Some(out_port_bp1));
                a.edge_set_target(start_end_edge, Some(in_port_bp2));
                if let Some(mo) = orig_model_order {
                    a.edge(start_end_edge).properties.set(&iprops::MODEL_ORDER, mo);
                }

                // reroute the original edge: source -> outPortBp2
                a.edge_set_source(original_edge, Some(out_port_bp2));

                // attach BPInfo to both dummies
                let bpi = store.push(BPInfo::new(
                    bp_start,
                    bp_end,
                    node_start_edge,
                    start_end_edge,
                    original_edge,
                ));
                a.node(bp_start).properties.set(&iprops::BREAKING_POINT_INFO, bpi);
                a.node(bp_end).properties.set(&iprops::BREAKING_POINT_INFO, bpi);

                // possibly chain to a previous breaking point dummy
                let prev_node = a.edge_source_node(node_start_edge);
                if a.node(prev_node).node_type == NodeType::BREAKING_POINT {
                    if let Some(bpi_prev) =
                        a.node(prev_node).properties.try_get(&iprops::BREAKING_POINT_INFO)
                    {
                        store.get_mut(bpi_prev).next = Some(bpi);
                        store.get_mut(bpi).prev = Some(bpi_prev);
                    }
                }
            }

            match cut_it.next() {
                Some(c) => cut = c,
                None => break,
            }
        }

        idx += 1;
        li += 1 + inserted;
    }

    a.graph(graph).properties.set(&iprops::BP_INFO_STORE, store);
}

// ---------------------------------------------------------------------------
// improveCuts (the `Cut` helper logic).
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Cut {
    index: i32,
    new_index: i32,
    prev: Option<usize>,
    suc: Option<usize>,
    assigned: bool,
}

struct Cuts {
    cuts: Vec<Cut>,
}

impl Cuts {
    fn self_or_next(&self, i: usize) -> Option<usize> {
        if !self.cuts[i].assigned {
            Some(i)
        } else if let Some(s) = self.cuts[i].suc {
            self.self_or_next(s)
        } else {
            None
        }
    }
    fn next(&self, i: usize) -> Option<usize> {
        if let Some(s) = self.cuts[i].suc {
            self.self_or_next(s)
        } else {
            None
        }
    }
    fn offset(&mut self, i: usize) {
        let offset = self.cuts[i].new_index - self.cuts[i].index;
        self.cuts[i].index += offset;
        self.offset_prev(i, offset);
        self.offset_suc(i, offset);
    }
    fn offset_prev(&mut self, i: usize, offset: i32) {
        if let Some(p) = self.cuts[i].prev {
            if !self.cuts[p].assigned {
                self.cuts[p].index += offset;
                self.offset_prev(p, offset);
            }
        }
    }
    fn offset_suc(&mut self, i: usize, offset: i32) {
        if let Some(s) = self.cuts[i].suc {
            if !self.cuts[s].assigned {
                self.cuts[s].index += offset;
                self.offset_suc(s, offset);
            }
        }
    }
}

fn improve_cuts(a: &LGraphArena, graph: LGraphId, cuts: &[i32]) -> Vec<i32> {
    let mut improved_cuts: Vec<i32> = Vec::new();

    // build Cut chain
    let mut ccuts: Vec<Cut> = Vec::new();
    let mut last_cut: Option<usize> = None;
    for &cut_idx in cuts {
        let i = ccuts.len();
        ccuts.push(Cut {
            index: cut_idx,
            new_index: 0,
            prev: None,
            suc: None,
            assigned: false,
        });
        if let Some(lc) = last_cut {
            ccuts[i].prev = Some(lc);
            ccuts[lc].suc = Some(i);
        }
        last_cut = Some(i);
    }
    let mut cuts_struct = Cuts { cuts: ccuts };

    let spans = compute_edge_spans(a, graph);
    let distance_penalty: f64 =
        a.graph(graph).properties.get(&lopts::WRAPPING_MULTI_EDGE_DISTANCE_PENALTY);
    let num_layers = a.graph(graph).layers.len() as i32;
    let n_cuts = cuts_struct.cuts.len();

    for _ in 0..n_cuts {
        let mut l_cut: Option<usize> = None;
        let mut r_cut: Option<usize> =
            if n_cuts > 0 { cuts_struct.self_or_next(0) } else { None };

        let mut best_cut: Option<usize> = None;
        let mut best_score = f64::INFINITY;
        let mut best_new_index = 0i32;

        let mut idx = 1i32;
        while idx < num_layers {
            let r_dist = match r_cut {
                Some(rc) => (cuts_struct.cuts[rc].index - idx).abs(),
                None => (idx - cuts_struct.cuts[l_cut.unwrap()].index).abs() + 1,
            };
            let l_dist = match l_cut {
                Some(lc) => (idx - cuts_struct.cuts[lc].index).abs(),
                None => r_dist + 1,
            };
            let (hit, dist) = if l_dist < r_dist {
                (l_cut, l_dist)
            } else {
                (r_cut, r_dist)
            };

            let score = spans[idx as usize] as f64 + (dist as f64).powf(distance_penalty);
            if score < best_score {
                best_score = score;
                best_cut = hit;
                best_new_index = idx;
            }

            if let Some(rc) = r_cut {
                if idx == cuts_struct.cuts[rc].index {
                    l_cut = r_cut;
                    r_cut = cuts_struct.next(rc);
                }
            }

            idx += 1;
        }

        if let Some(bc) = best_cut {
            cuts_struct.cuts[bc].new_index = best_new_index;
            improved_cuts.push(best_new_index);
            cuts_struct.cuts[bc].assigned = true;
            cuts_struct.offset(bc);
        }
    }

    improved_cuts.sort();
    improved_cuts
}

fn compute_edge_spans(a: &LGraphArena, graph: LGraphId) -> Vec<i32> {
    let layers = a.graph(graph).layers.clone();
    let mut spans = vec![0i32; layers.len() + 1];
    let mut open: HashSet<LEdgeId> = HashSet::new();
    for (i, &l) in layers.iter().enumerate() {
        spans[i] = open.len() as i32;
        let nodes = a.layer(l).nodes.clone();
        for n in &nodes {
            for e in a.node_outgoing_edges(*n) {
                open.insert(e);
            }
        }
        for n in &nodes {
            for e in a.node_incoming_edges(*n) {
                open.remove(&e);
            }
        }
    }
    spans
}
