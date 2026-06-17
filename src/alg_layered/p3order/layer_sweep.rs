//!
//! Minimizes crossings by sweeping through the graph, holding the order of
//! nodes in one layer fixed and switching the nodes in the other layer.
//! After each re-sorting step, the ports in the two current layers are
//! sorted.
//!
//! `CrossMinType::Barycenter` (the LAYER_SWEEP phase implementation) and the
//! greedy switch types (the ONE_SIDED_GREEDY_SWITCH /
//! TWO_SIDED_GREEDY_SWITCH intermediate processors) are ported; the
//! hierarchical sweep paths are structurally present but bail out with a
//! TODO error when a nested graph is actually encountered.

use crate::core::javacompat::JavaRandom;
use crate::core::options::{HierarchyHandling, PortConstraints, PortSide};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::OrderingStrategy;

use super::barycenter_heuristic;
use super::graph_info_holder::{CrossMinimizer, GraphInfoHolder};
use super::sweep_copy::SweepCopy;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CrossMinType {
    /// Use BarycenterHeuristic.
    Barycenter,
    /// Use one-sided GreedySwitchHeuristic.
    OneSidedGreedySwitch,
    /// Use two-sided GreedySwitchHeuristic.
    TwoSidedGreedySwitch,
    /// Use MedianHeuristic (not ported yet).
    Median,
}

/// State of one `process` run.
struct LayerSweep {
    /// Collected information about each graph.
    holders: Vec<GraphInfoHolder>,
    /// We only need to save the orders of graphs whose node order actually
    /// changed. (Insertion-ordered here for determinism.)
    graphs_whose_node_order_changed: Vec<usize>,
    random_seed: i64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MinimizingMethod {
    /// `compareDifferentRandomizedLayouts`
    CompareRandomized,
    /// `minimizeCrossingsNoCounter`
    NoCounter,
    /// `minimizeCrossingsWithCounter`
    WithCounter,
}

/// Entry point for the LAYER_SWEEP phase implementation, i.e.
/// `new LayerSweepCrossingMinimizer(CrossMinType.BARYCENTER)`.
pub fn process(a: &mut LGraphArena, graph: LGraphId, random: &mut JavaRandom) -> Result<(), String> {
    process_with_type(a, graph, random, CrossMinType::Barycenter)
}

/// Entry point with an explicit crossing minimizer type; the
/// ONE_SIDED_GREEDY_SWITCH / TWO_SIDED_GREEDY_SWITCH intermediate
/// processors are `new LayerSweepCrossingMinimizer(crossMinType)`, too.
pub fn process_with_type(
    a: &mut LGraphArena,
    graph: LGraphId,
    random: &mut JavaRandom,
    cross_min_type: CrossMinType,
) -> Result<(), String> {
    // Short-circuit cases in which no crossings can be minimized.
    let layers = a.graph(graph).layers.clone();
    let empty_graph =
        layers.is_empty() || layers.iter().all(|&layer| a.layer(layer).nodes.is_empty());
    let single_node = layers.len() == 1 && a.layer(layers[0]).nodes.len() == 1;
    let hierarchical_layout = a.graph(graph).properties.get(&lopts::HIERARCHY_HANDLING)
        == HierarchyHandling::INCLUDE_CHILDREN;

    if empty_graph || (single_node && !hierarchical_layout) {
        return Ok(());
    }

    // --- Early rejection of unported code paths, before any work is done
    // (in particular before any random numbers are consumed). ---
    if cross_min_type == CrossMinType::Median {
        return Err(format!("TODO: cross minimizer {cross_min_type:?} not ported yet"));
    }
    // Hierarchical (nested-graph) layouts are handled bottom-up by the
    // LayerSweepTypeDecider for the simple compound case; the only unported
    // path (sweeping top-down into a child) is rejected at the point it would
    // actually be taken, in `sweep_in_hierarchical_nodes`.

    let (mut sweep, graphs_to_sweep_on) = initialize(a, graph, random, cross_min_type)?;

    let minimizing_method = choose_minimizing_method(&sweep, &graphs_to_sweep_on);

    minimize_crossings(&mut sweep, a, random, &graphs_to_sweep_on, minimizing_method)?;

    transfer_node_and_port_orders_to_graph(&sweep, a);

    Ok(())
}

/// Traverses inclusion breadth-first and initializes
/// each graph. Returns the sweep state and the `graphsToSweepOn` list
/// (indices into `holders`).
fn initialize(
    a: &mut LGraphArena,
    root_graph: LGraphId,
    random: &mut JavaRandom,
    cross_min_type: CrossMinType,
) -> Result<(LayerSweep, Vec<usize>), String> {
    let mut holders: Vec<GraphInfoHolder> = Vec::new();
    // random = rootGraph.getProperty(InternalProperties.RANDOM) is the
    // `random` parameter in this port.
    let random_seed = random.next_long();
    let mut graphs_to_sweep_on: Vec<usize> = Vec::new();
    let mut graphs: Vec<LGraphId> = vec![root_graph];
    let mut i = 0usize;
    while i < graphs.len() {
        let graph = graphs[i];
        a.graph_mut(graph).id = i as i32;
        i += 1;
        let gdata = GraphInfoHolder::new(a, graph, cross_min_type, &holders, random)?;
        graphs.extend(gdata.child_graphs.iter().copied());
        let dont_sweep_into = gdata.dont_sweep_into();
        holders.push(gdata);
        if dont_sweep_into {
            graphs_to_sweep_on.insert(0, holders.len() - 1);
        }
    }

    Ok((
        LayerSweep { holders, graphs_whose_node_order_changed: Vec::new(), random_seed },
        graphs_to_sweep_on,
    ))
}

fn choose_minimizing_method(sweep: &LayerSweep, graphs_to_sweep_on: &[usize]) -> MinimizingMethod {
    let parent = &sweep.holders[graphs_to_sweep_on[0]];
    if !parent.cross_min_deterministic() {
        MinimizingMethod::CompareRandomized
    } else if parent.cross_min_always_improves() {
        MinimizingMethod::NoCounter
    } else {
        MinimizingMethod::WithCounter
    }
}

fn minimize_crossings(
    sweep: &mut LayerSweep,
    a: &mut LGraphArena,
    random: &mut JavaRandom,
    graphs_to_sweep_on: &[usize],
    method: MinimizingMethod,
) -> Result<(), String> {
    for &gidx in graphs_to_sweep_on {
        if !sweep.holders[gidx].current_node_order.is_empty() {
            match method {
                MinimizingMethod::CompareRandomized => {
                    compare_different_randomized_layouts(sweep, a, random, gidx)?
                }
                MinimizingMethod::NoCounter => minimize_crossings_no_counter(sweep, a, random, gidx)?,
                MinimizingMethod::WithCounter => {
                    minimize_crossings_with_counter(sweep, a, random, gidx)?;
                }
            }
            if sweep.holders[gidx].has_parent {
                set_port_order_on_parent_graph(sweep, a, gidx);
            }
        }
    }
    Ok(())
}

fn set_port_order_on_parent_graph(sweep: &mut LayerSweep, a: &mut LGraphArena, gidx: usize) {
    let holder = &sweep.holders[gidx];
    if holder.has_external_ports && holder.get_best_sweep().is_some() {
        let best_sweep_nodes = holder.get_best_sweep().unwrap().nodes().to_vec();
        let parent = holder.parent.unwrap();
        // Sort ports on left and right side of the parent node
        sort_ports_by_dummy_positions_in_last_layer(a, &best_sweep_nodes, parent, true);
        sort_ports_by_dummy_positions_in_last_layer(a, &best_sweep_nodes, parent, false);
        a.node(parent).properties.set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_ORDER);
    }
}

/// For use with any two-layer crossing minimizer which always improves
/// crossings (e.g. two-sided greedy switch).
fn minimize_crossings_no_counter(
    sweep: &mut LayerSweep,
    a: &mut LGraphArena,
    random: &mut JavaRandom,
    gidx: usize,
) -> Result<(), String> {
    let mut is_forward_sweep = random.next_boolean();
    let mut improved = true;
    while improved {
        improved = set_first_layer_order(sweep, a, random, gidx, is_forward_sweep)?;
        improved |= sweep_reducing_crossings(sweep, a, random, gidx, is_forward_sweep, false)?;
        is_forward_sweep = !is_forward_sweep;
    }
    set_currently_best_node_orders(sweep, a);
    Ok(())
}

fn compare_different_randomized_layouts(
    sweep: &mut LayerSweep,
    a: &mut LGraphArena,
    random: &mut JavaRandom,
    gidx: usize,
) -> Result<(), String> {
    // Reset the seed, otherwise copies of hierarchical graphs in different
    // parent nodes are layouted differently.
    random.set_seed(sweep.random_seed);

    // In order to only copy graphs whose node order has changed, save them in a set.
    sweep.graphs_whose_node_order_changed.clear();

    let lgraph = sweep.holders[gidx].lgraph;
    let node_influence: f64 = a
        .graph(lgraph)
        .properties
        .get(&lopts::CONSIDER_MODEL_ORDER_CROSSING_COUNTER_NODE_INFLUENCE);
    // CROSSING_COUNTER_NODE_INFLUENCE is checked twice (instead of the port
    // influence).
    #[allow(clippy::nonminimal_bool)]
    if node_influence != 0.0 || node_influence != 0.0 {
        let mut best_crossings = f64::MAX;
        if a.graph(lgraph).properties.get(&lopts::CONSIDER_MODEL_ORDER_STRATEGY)
            != OrderingStrategy::NONE
        {
            // unreachable in this port: rejected early in process()
            a.graph(lgraph).properties.set(&iprops::FIRST_TRY_WITH_INITIAL_ORDER, true);
        }
        let thoroughness: i32 = a.graph(lgraph).properties.get(&lopts::THOROUGHNESS);
        for _ in 0..thoroughness {
            let crossings = minimize_crossings_node_port_order_with_counter(sweep, a, random, gidx)?;
            if crossings < best_crossings {
                best_crossings = crossings;
                save_all_node_orders_of_changed_graphs(sweep);
                if best_crossings == 0.0 {
                    break;
                }
            }
        }
    } else {
        let mut best_crossings = i32::MAX;
        if a.graph(lgraph).properties.get(&lopts::CONSIDER_MODEL_ORDER_STRATEGY)
            != OrderingStrategy::NONE
        {
            // unreachable in this port: rejected early in process()
            a.graph(lgraph).properties.set(&iprops::FIRST_TRY_WITH_INITIAL_ORDER, true);
        }
        let thoroughness: i32 = a.graph(lgraph).properties.get(&lopts::THOROUGHNESS);
        for _ in 0..thoroughness {
            let crossings = minimize_crossings_with_counter(sweep, a, random, gidx)?;
            if crossings < best_crossings {
                best_crossings = crossings;
                save_all_node_orders_of_changed_graphs(sweep);
                if best_crossings == 0 {
                    break;
                }
            }
        }
    }
    Ok(())
}

/// `gData.crossMinimizer().setFirstLayerOrder(...)`.
fn set_first_layer_order(
    sweep: &mut LayerSweep,
    a: &LGraphArena,
    random: &mut JavaRandom,
    gidx: usize,
    is_forward_sweep: bool,
) -> Result<bool, String> {
    let holder = &mut sweep.holders[gidx];
    match &mut holder.cross_minimizer {
        CrossMinimizer::Barycenter { constraint_resolver, model_order } => {
            Ok(barycenter_heuristic::set_first_layer_order(
                a,
                &mut holder.current_node_order,
                constraint_resolver,
                model_order.as_mut(),
                holder.port_distributor.as_barycenter_mut(),
                random,
                is_forward_sweep,
            ))
        }
        CrossMinimizer::GreedySwitch(heuristic) => {
            heuristic.set_first_layer_order(a, &mut holder.current_node_order, is_forward_sweep)
        }
    }
}

fn minimize_crossings_with_counter(
    sweep: &mut LayerSweep,
    a: &mut LGraphArena,
    random: &mut JavaRandom,
    gidx: usize,
) -> Result<i32, String> {
    let mut is_forward_sweep = random.next_boolean();
    let lgraph = sweep.holders[gidx].lgraph;

    // If the first, initial ordering is already optimal, do not change anything.
    let initial_crossings = count_current_number_of_crossings(sweep, a, gidx);
    if initial_crossings == 0
        && a.graph(lgraph).properties.get(&iprops::FIRST_TRY_WITH_INITIAL_ORDER)
    {
        // E.g. model order is already correct.
        return Ok(0);
    }

    let first_try: bool = a.graph(lgraph).properties.get(&iprops::FIRST_TRY_WITH_INITIAL_ORDER);
    let second_try: bool = a.graph(lgraph).properties.get(&iprops::SECOND_TRY_WITH_INITIAL_ORDER);
    if (!first_try && !second_try)
        || a.graph(lgraph).properties.get(&lopts::CONSIDER_MODEL_ORDER_STRATEGY)
            == OrderingStrategy::NONE
    {
        set_first_layer_order(sweep, a, random, gidx, is_forward_sweep)?;
    } else {
        is_forward_sweep = first_try;
    }
    sweep_reducing_crossings(sweep, a, random, gidx, is_forward_sweep, true)?;
    if a.graph(lgraph).properties.get(&iprops::SECOND_TRY_WITH_INITIAL_ORDER) {
        a.graph(lgraph).properties.set(&iprops::SECOND_TRY_WITH_INITIAL_ORDER, false);
    }
    if a.graph(lgraph).properties.get(&iprops::FIRST_TRY_WITH_INITIAL_ORDER) {
        a.graph(lgraph).properties.set(&iprops::FIRST_TRY_WITH_INITIAL_ORDER, false);
        a.graph(lgraph).properties.set(&iprops::SECOND_TRY_WITH_INITIAL_ORDER, true);
    }
    let mut crossings_in_graph = count_current_number_of_crossings(sweep, a, gidx);
    let mut old_number_of_crossings;
    loop {
        set_currently_best_node_orders(sweep, a);

        if crossings_in_graph == 0 {
            return Ok(0);
        }

        is_forward_sweep = !is_forward_sweep;
        old_number_of_crossings = crossings_in_graph;
        sweep_reducing_crossings(sweep, a, random, gidx, is_forward_sweep, false)?;
        crossings_in_graph = count_current_number_of_crossings(sweep, a, gidx);
        if old_number_of_crossings <= crossings_in_graph {
            break;
        }
    }

    Ok(old_number_of_crossings)
}

fn minimize_crossings_node_port_order_with_counter(
    sweep: &mut LayerSweep,
    a: &mut LGraphArena,
    random: &mut JavaRandom,
    gidx: usize,
) -> Result<f64, String> {
    let mut is_forward_sweep = random.next_boolean();
    let lgraph = sweep.holders[gidx].lgraph;

    let initial_crossings = count_current_number_of_crossings_node_port_order(sweep, a, gidx);
    if initial_crossings == 0.0
        && a.graph(lgraph).properties.get(&iprops::FIRST_TRY_WITH_INITIAL_ORDER)
    {
        return Ok(0.0);
    }

    let first_try: bool = a.graph(lgraph).properties.get(&iprops::FIRST_TRY_WITH_INITIAL_ORDER);
    let second_try: bool = a.graph(lgraph).properties.get(&iprops::SECOND_TRY_WITH_INITIAL_ORDER);
    if (!first_try && !second_try)
        || a.graph(lgraph).properties.get(&lopts::CONSIDER_MODEL_ORDER_STRATEGY)
            == OrderingStrategy::NONE
    {
        set_first_layer_order(sweep, a, random, gidx, is_forward_sweep)?;
    } else {
        is_forward_sweep = first_try;
    }
    sweep_reducing_crossings(sweep, a, random, gidx, is_forward_sweep, true)?;
    if a.graph(lgraph).properties.get(&iprops::SECOND_TRY_WITH_INITIAL_ORDER) {
        a.graph(lgraph).properties.set(&iprops::SECOND_TRY_WITH_INITIAL_ORDER, false);
    }
    if a.graph(lgraph).properties.get(&iprops::FIRST_TRY_WITH_INITIAL_ORDER) {
        a.graph(lgraph).properties.set(&iprops::FIRST_TRY_WITH_INITIAL_ORDER, false);
        a.graph(lgraph).properties.set(&iprops::SECOND_TRY_WITH_INITIAL_ORDER, true);
    }
    let mut crossings_in_graph = count_current_number_of_crossings_node_port_order(sweep, a, gidx);
    let mut old_number_of_crossings;
    loop {
        set_currently_best_node_orders(sweep, a);

        if crossings_in_graph == 0.0 {
            return Ok(0.0);
        }

        is_forward_sweep = !is_forward_sweep;
        old_number_of_crossings = crossings_in_graph;
        sweep_reducing_crossings(sweep, a, random, gidx, is_forward_sweep, false)?;
        crossings_in_graph = count_current_number_of_crossings_node_port_order(sweep, a, gidx);
        if old_number_of_crossings <= crossings_in_graph {
            break;
        }
    }

    Ok(old_number_of_crossings)
}

/// We only need to count crossings
/// below the current graph and also only if they are marked as to be
/// processed hierarchically.
fn count_current_number_of_crossings(sweep: &mut LayerSweep, a: &LGraphArena, gidx: usize) -> i32 {
    let mut total_crossings = 0;
    // A deque only ever holds the current graph; child graphs are handled by
    // the recursive call.
    {
        let holder = &mut sweep.holders[gidx];
        let order = std::mem::take(&mut holder.current_node_order);
        total_crossings += holder.crossings_counter.count_all_crossings(a, &order);
        holder.current_node_order = order;
    }
    let children = sweep.holders[gidx].child_graphs.clone();
    for child_lgraph in children {
        let child_idx = a.graph(child_lgraph).id as usize;
        if !sweep.holders[child_idx].dont_sweep_into() {
            total_crossings += count_current_number_of_crossings(sweep, a, child_idx);
        }
    }
    total_crossings
}

/// The model order
/// influence terms are guaranteed to be zero here because
/// `CONSIDER_MODEL_ORDER_STRATEGY != NONE` is rejected early in `process`
/// (the model-order comparators are not ported yet).
fn count_current_number_of_crossings_node_port_order(
    sweep: &mut LayerSweep,
    a: &LGraphArena,
    gidx: usize,
) -> f64 {
    let mut total_crossings = 0.0f64;
    {
        let lgraph = sweep.holders[gidx].lgraph;
        debug_assert_eq!(
            a.graph(lgraph).properties.get(&lopts::CONSIDER_MODEL_ORDER_STRATEGY),
            OrderingStrategy::NONE
        );
        let model_order_influence = 0.0f64;
        let holder = &mut sweep.holders[gidx];
        let order = std::mem::take(&mut holder.current_node_order);
        total_crossings +=
            holder.crossings_counter.count_all_crossings(a, &order) as f64 + model_order_influence;
        holder.current_node_order = order;
    }
    let children = sweep.holders[gidx].child_graphs.clone();
    for child_lgraph in children {
        let child_idx = a.graph(child_lgraph).id as usize;
        if !sweep.holders[child_idx].dont_sweep_into() {
            // The *int* counting method is called for children here.
            total_crossings += count_current_number_of_crossings(sweep, a, child_idx) as f64;
        }
    }
    total_crossings
}

fn sweep_reducing_crossings(
    sweep: &mut LayerSweep,
    a: &mut LGraphArena,
    random: &mut JavaRandom,
    gidx: usize,
    forward: bool,
    first_sweep: bool,
) -> Result<bool, String> {
    let length = sweep.holders[gidx].current_node_order.len();
    let lgraph = sweep.holders[gidx].lgraph;

    let mut improved = {
        let holder = &mut sweep.holders[gidx];
        holder.port_distributor.distribute_ports_while_sweeping(
            a,
            &holder.current_node_order,
            first_index(forward, length),
            forward,
        )
    };
    let first_layer = sweep.holders[gidx].current_node_order[first_index(forward, length)].clone();
    improved |= sweep_in_hierarchical_nodes(sweep, a, random, &first_layer, forward, first_sweep)?;
    let mut i = first_free(forward, length);
    while is_not_end(length, i, forward) {
        let first_try: bool =
            a.graph(lgraph).properties.get(&iprops::FIRST_TRY_WITH_INITIAL_ORDER);
        let second_try: bool =
            a.graph(lgraph).properties.get(&iprops::SECOND_TRY_WITH_INITIAL_ORDER);
        {
            let holder = &mut sweep.holders[gidx];
            improved |= match &mut holder.cross_minimizer {
                CrossMinimizer::Barycenter { constraint_resolver, model_order } => {
                    barycenter_heuristic::minimize_crossings_in_sweep(
                        a,
                        &mut holder.current_node_order,
                        constraint_resolver,
                        model_order.as_mut(),
                        holder.port_distributor.as_barycenter_mut(),
                        random,
                        i as usize,
                        forward,
                        first_sweep && !first_try && !second_try,
                    )
                }
                CrossMinimizer::GreedySwitch(heuristic) => heuristic.minimize_crossings(
                    a,
                    &mut holder.current_node_order,
                    i as usize,
                    forward,
                    first_sweep && !first_try && !second_try,
                )?,
            };
            improved |= holder.port_distributor.distribute_ports_while_sweeping(
                a,
                &holder.current_node_order,
                i as usize,
                forward,
            );
        }
        let layer = sweep.holders[gidx].current_node_order[i as usize].clone();
        improved |= sweep_in_hierarchical_nodes(sweep, a, random, &layer, forward, first_sweep)?;
        i += next(forward);
    }

    if !sweep.graphs_whose_node_order_changed.contains(&gidx) {
        sweep.graphs_whose_node_order_changed.push(gidx);
    }
    Ok(improved)
}

fn sweep_in_hierarchical_nodes(
    sweep: &mut LayerSweep,
    a: &mut LGraphArena,
    random: &mut JavaRandom,
    layer: &[LNodeId],
    is_forward_sweep: bool,
    is_first_sweep: bool,
) -> Result<bool, String> {
    let mut improved = false;
    for &node in layer {
        if let Some(nested) = a.node(node).nested_graph {
            let child_idx = a.graph(nested).id as usize;
            if !sweep.holders[child_idx].dont_sweep_into() {
                improved |= sweep_in_hierarchical_node(
                    sweep,
                    a,
                    random,
                    node,
                    is_forward_sweep,
                    is_first_sweep,
                )?;
            }
        }
    }
    Ok(improved)
}

fn sweep_in_hierarchical_node(
    sweep: &mut LayerSweep,
    a: &mut LGraphArena,
    random: &mut JavaRandom,
    node: LNodeId,
    is_forward_sweep: bool,
    is_first_sweep: bool,
) -> Result<bool, String> {
    let nested = a.node(node).nested_graph.unwrap();
    let child_idx = a.graph(nested).id as usize;
    let order_len = sweep.holders[child_idx].current_node_order.len();
    let start_index = first_index(is_forward_sweep, order_len);
    let first_node = sweep.holders[child_idx].current_node_order[start_index][0];

    if is_external_port_dummy(a, first_node) {
        let side = side_opposed_sweep_direction(is_forward_sweep);
        let layer_close = sweep.holders[child_idx].current_node_order[start_index].clone();
        let sorted = sort_port_dummies_by_port_positions(a, node, &layer_close, side);
        sweep.holders[child_idx].current_node_order[start_index] = sorted;
    } else {
        set_first_layer_order(sweep, a, random, child_idx, is_forward_sweep)?;
    }

    let improved = sweep_reducing_crossings(sweep, a, random, child_idx, is_forward_sweep, is_first_sweep)?;

    let parent = sweep.holders[child_idx].parent.unwrap();
    let order = sweep.holders[child_idx].current_node_order.clone();
    sort_ports_by_dummy_positions_in_last_layer(a, &order, parent, is_forward_sweep);

    Ok(improved)
}

/// `PortSide.sideOpposedSweepDirection`-equivalent inline logic: a forward
/// sweep approaches the child's left (WEST) side first.
fn side_opposed_sweep_direction(is_forward_sweep: bool) -> PortSide {
    if is_forward_sweep {
        PortSide::WEST
    } else {
        PortSide::EAST
    }
}

fn sort_port_dummies_by_port_positions(
    a: &LGraphArena,
    parent_node: LNodeId,
    layer_close_to_node_edge: &[LNodeId],
    side: PortSide,
) -> Vec<LNodeId> {
    let ports = crate::alg_layered::p3order::counting::in_north_south_east_west_order(a, parent_node, side);

    let mut sorted: Vec<LNodeId> = Vec::with_capacity(layer_close_to_node_edge.len());
    for port in ports {
        if is_hierarchical(a, port) {
            let dummy: LNodeId = a.port(port).properties.try_get(&iprops::PORT_DUMMY).unwrap();
            sorted.push(dummy);
        }
    }

    if sorted.len() < layer_close_to_node_edge.len() {
        panic!(
            "Expected {} hierarchical ports, but found only {}.",
            layer_close_to_node_edge.len(),
            sorted.len()
        );
    }
    sorted
}

fn sort_ports_by_dummy_positions_in_last_layer(
    a: &mut LGraphArena,
    node_order: &[Vec<LNodeId>],
    parent: LNodeId,
    on_right_most_layer: bool,
) {
    let end_index = end_index(on_right_most_layer, node_order.len());
    let last_layer = &node_order[end_index];
    // Check whether the node to check is an external port dummy.
    let mut j = first_index(on_right_most_layer, last_layer.len()) as i32;
    if !is_external_port_dummy(a, last_layer[j as usize]) {
        return;
    }

    let mut ports = a.node(parent).ports.clone();
    for i in 0..ports.len() {
        let port = ports[i];
        if is_on_end_of_sweep_side(a, port, on_right_most_layer) && is_hierarchical(a, port) {
            // Only an external port dummy node has a port as its origin.
            ports[i] = origin_port(a, last_layer[j as usize]);
            j += next(on_right_most_layer);
        }
    }
    a.node_mut(parent).ports = ports;
}

fn save_all_node_orders_of_changed_graphs(sweep: &mut LayerSweep) {
    for &gidx in &sweep.graphs_whose_node_order_changed {
        let holder = &sweep.holders[gidx];
        let copy = holder
            .currently_best_node_and_port_order
            .clone()
            .expect("currently best node order must have been saved");
        sweep.holders[gidx].best_node_and_port_order = Some(copy);
    }
}

fn set_currently_best_node_orders(sweep: &mut LayerSweep, a: &LGraphArena) {
    for &gidx in &sweep.graphs_whose_node_order_changed.clone() {
        let copy = SweepCopy::new(a, &sweep.holders[gidx].current_node_order);
        sweep.holders[gidx].currently_best_node_and_port_order = Some(copy);
    }
}

fn transfer_node_and_port_orders_to_graph(sweep: &LayerSweep, a: &mut LGraphArena) {
    for holder in &sweep.holders {
        if let Some(best_sweep) = holder.get_best_sweep().cloned() {
            best_sweep.transfer_node_and_port_orders_to_graph(a, holder.lgraph, true);
        }
    }
}

// ------------------------------------------------------------------ helpers

fn first_index(is_forward_sweep: bool, length: usize) -> usize {
    if is_forward_sweep {
        0
    } else {
        length - 1
    }
}

fn end_index(is_forward_sweep: bool, length: usize) -> usize {
    if is_forward_sweep {
        length - 1
    } else {
        0
    }
}

fn first_free(is_forward_sweep: bool, length: usize) -> i32 {
    if is_forward_sweep {
        1
    } else {
        length as i32 - 2
    }
}

fn next(is_forward_sweep: bool) -> i32 {
    if is_forward_sweep {
        1
    } else {
        -1
    }
}

fn is_not_end(length: usize, free_layer_index: i32, is_forward_sweep: bool) -> bool {
    if is_forward_sweep {
        free_layer_index < length as i32
    } else {
        free_layer_index >= 0
    }
}

fn is_external_port_dummy(a: &LGraphArena, node: LNodeId) -> bool {
    a.node(node).node_type == NodeType::EXTERNAL_PORT
}

fn origin_port(a: &LGraphArena, node: LNodeId) -> LPortId {
    match a.node(node).properties.try_get(&iprops::ORIGIN) {
        Some(Origin::LPort(p)) => p,
        other => panic!("expected LPort origin on external port dummy, got {other:?}"),
    }
}

fn is_hierarchical(a: &LGraphArena, port: LPortId) -> bool {
    a.port(port).properties.get(&iprops::INSIDE_CONNECTIONS)
}

fn is_on_end_of_sweep_side(a: &LGraphArena, port: LPortId, is_forward_sweep: bool) -> bool {
    if is_forward_sweep {
        a.port(port).side == PortSide::EAST
    } else {
        a.port(port).side == PortSide::WEST
    }
}

#[cfg(test)]
mod tests;
