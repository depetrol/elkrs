//! Splits path-like graphs into multiple
//! rows to improve the aspect ratio (wrappingStrategy = SINGLE_EDGE).

use crate::alg_layered::graph::{LGraphArena, LGraphId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::ValidifyStrategy;
use crate::alg_layered::processors::wrapping_support as ws;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    if a.graph(graph).layers.is_empty() {
        return Ok(());
    }

    let gs = ws::GraphStats::new(a, graph);

    // stop early if there's nothing we can do
    let sum_width = gs.get_max_width() * gs.longest_path as f64;
    let current_ar = sum_width / gs.get_max_width();
    if gs.dar > current_ar {
        return Ok(());
    }

    let icic = ws::cut_index_calculator(a, graph);
    let mut cuts = icic.get_cut_indexes(a, graph, &gs);

    if !icic.guarantee_valid() {
        match a.graph(graph).properties.get::<ValidifyStrategy>(&lopts::WRAPPING_VALIDIFY_STRATEGY) {
            ValidifyStrategy::LOOK_BACK => cuts = ws::validify_indexes_looking_back(&gs, &cuts),
            ValidifyStrategy::GREEDY => cuts = ws::validify_indexes_greedily(&gs, &cuts),
            ValidifyStrategy::NO => {}
        }
    }

    perform_cuts(a, graph, &gs, &cuts);
    Ok(())
}

fn perform_cuts(a: &mut LGraphArena, graph: LGraphId, gs: &ws::GraphStats, cuts: &[i32]) {
    if cuts.is_empty() {
        return;
    }

    let mut index = 0i32;
    let mut new_index = 0i32;

    let mut cut_it = cuts.iter().copied();
    let mut next_cut = cut_it.next().unwrap();

    let longest = gs.longest_path as i32;
    while index < longest {
        if index == next_cut {
            new_index = 0;
            match cut_it.next() {
                Some(c) => next_cut = c,
                None => next_cut = longest + 1,
            }
        }

        if index != new_index {
            let layers = a.graph(graph).layers.clone();
            let old_layer = layers[index as usize];
            let new_layer = layers[new_index as usize];

            let nodes_to_move = a.layer(old_layer).nodes.clone();
            for n in nodes_to_move {
                // first move the original node
                let pos = a.layer(new_layer).nodes.len();
                a.node_set_layer_at(n, Some(new_layer), pos);

                if new_index == 0 {
                    let inc_edges = a.node_incoming_edges(n);
                    for e in inc_edges {
                        crate::alg_layered::lgraph_util::edge_reverse(a, graph, e, true);
                        a.graph(graph).properties.set(&iprops::CYCLIC, true);
                        ws::insert_dummies(a, graph, e, 1);
                    }
                }
            }
        }

        new_index += 1;
        index += 1;
    }

    // remove old layers that are now empty
    let layers = a.graph(graph).layers.clone();
    let mut kept = Vec::new();
    for l in layers {
        if a.layer(l).nodes.is_empty() {
            // leave it detached
        } else {
            kept.push(l);
        }
    }
    a.graph_mut(graph).layers = kept;
}
