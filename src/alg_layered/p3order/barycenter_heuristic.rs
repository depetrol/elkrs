use std::cmp::Ordering;

use crate::core::javacompat::JavaRandom;

use crate::alg_layered::graph::{LGraphArena, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;

use super::forster_constraint_resolver::ForsterConstraintResolver;
use super::model_order_barycenter_heuristic::ModelOrderBarycenterState;
use super::port_distributor::PortDistributor;

/// the amount of random value to add to each calculated barycenter.
const RANDOM_AMOUNT: f32 = 0.07f32;

/// The current barycenter
/// state of a node.
#[derive(Clone, Debug)]
pub struct BarycenterState {
    /// The node this state holds data for.
    pub node: LNodeId,
    /// The sum of the node weights. Each node weight is the sum of the
    /// weights of the ports the node's ports are connected to.
    pub summed_weight: f64,
    /// The number of ports relevant to the barycenter calculation.
    pub degree: i32,
    /// This vertex' barycenter value. (summedWeight / degree)
    pub barycenter: Option<f64>,
    /// Whether the node group has been visited in some traversing algorithm.
    pub visited: bool,
}

impl BarycenterState {
    pub fn new(node: LNodeId) -> Self {
        BarycenterState {
            node,
            summed_weight: 0.0,
            degree: 0,
            barycenter: None,
            visited: false,
        }
    }
}

/// Looks up the state of a node: `barycenterState[node.getLayer().id][node.id]`.
fn state_indices(a: &LGraphArena, node: LNodeId) -> (usize, usize) {
    let layer = a.node(node).layer.unwrap();
    (a.layer(layer).id as usize, a.node(node).id as usize)
}

#[allow(clippy::too_many_arguments)]
pub fn minimize_crossings_in_sweep(
    a: &LGraphArena,
    order: &mut [Vec<LNodeId>],
    resolver: &mut ForsterConstraintResolver,
    model_order: Option<&mut ModelOrderBarycenterState>,
    distributor: &mut PortDistributor,
    random: &mut JavaRandom,
    free_layer_index: usize,
    forward_sweep: bool,
    is_first_sweep: bool,
) -> bool {
    if !is_first_layer(order, free_layer_index, forward_sweep) {
        let fixed_layer = &order[(free_layer_index as i32 - change_index(forward_sweep)) as usize];
        distributor.calculate_port_ranks_layer(a, fixed_layer, port_type_for(forward_sweep));
    }

    let first_node_in_layer = order[free_layer_index][0];
    let pre_ordered = !is_first_sweep || is_external_port_dummy(a, first_node_in_layer);

    let mut nodes: Vec<LNodeId> = order[free_layer_index].clone();
    minimize_crossings_list(a, &mut nodes, resolver, model_order, distributor, random, pre_ordered, false, forward_sweep);
    // apply the new ordering
    order[free_layer_index].clone_from(&nodes);

    false // Does not always improve.
}

#[allow(clippy::too_many_arguments)]
pub fn set_first_layer_order(
    a: &LGraphArena,
    order: &mut [Vec<LNodeId>],
    resolver: &mut ForsterConstraintResolver,
    model_order: Option<&mut ModelOrderBarycenterState>,
    distributor: &mut PortDistributor,
    random: &mut JavaRandom,
    is_forward_sweep: bool,
) -> bool {
    let start_index = start_index(is_forward_sweep, order.len());
    let mut nodes: Vec<LNodeId> = order[start_index].clone();
    // randomize nodes' barycenters
    minimize_crossings_list(a, &mut nodes, resolver, model_order, distributor, random, false, true, is_forward_sweep);
    // fill first layer with nodes
    order[start_index].clone_from(&nodes);

    false // Does not always improve
}

/// The package-visible `minimizeCrossings(List<LNode>, boolean,
/// boolean, boolean)`.
#[allow(clippy::too_many_arguments)]
pub fn minimize_crossings_list(
    a: &LGraphArena,
    layer: &mut Vec<LNodeId>,
    resolver: &mut ForsterConstraintResolver,
    model_order: Option<&mut ModelOrderBarycenterState>,
    distributor: &mut PortDistributor,
    random: &mut JavaRandom,
    pre_ordered: bool,
    randomize: bool,
    forward: bool,
) {
    if randomize {
        // Randomize barycenters (we don't need to update the edge count in
        // this case; there are no edges of interest since we're only
        // concerned with one layer); simply a permutation of nodes in layer
        randomize_barycenters(a, layer, resolver, random);
    } else {
        // Calculate barycenters and assign barycenters to barycenterless node groups
        calculate_barycenters(a, layer, resolver, distributor, random, forward);
        fill_in_unknown_barycenters(a, layer, resolver, random, pre_ordered);
    }

    if layer.len() > 1 {
        match model_order {
            // ModelOrderBarycenterHeuristic.minimizeCrossings
            Some(mo) => {
                let states = &resolver.barycenter_states;
                if mo.force_node_model_order {
                    // insertionSort with the model-order comparator; NO
                    // processConstraints afterwards.
                    insertion_sort_model_order(a, layer, states, mo);
                    mo.clear_transitive_ordering();
                } else {
                    // Collections.sort (TimSort) with the model-order
                    // comparator, then processConstraints.
                    crate::core::javacompat::tim_sort(layer, |&n1, &n2| {
                        mo.compare(a, states, n1, n2)
                    });
                    resolver.process_constraints(a, layer);
                }
            }
            // Plain BarycenterHeuristic.
            None => {
                let states = &resolver.barycenter_states;
                layer.sort_by(|&n1, &n2| {
                    let (l1, i1) = state_indices(a, n1);
                    let (l2, i2) = state_indices(a, n2);
                    let b1 = states[l1][i1].barycenter;
                    let b2 = states[l2][i2].barycenter;
                    match (b1, b2) {
                        (Some(v1), Some(v2)) => v1.total_cmp(&v2),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => Ordering::Equal,
                    }
                });

                // Resolve ordering constraints
                resolver.process_constraints(a, layer);
            }
        }
    }
}

fn insertion_sort_model_order(
    a: &LGraphArena,
    layer: &mut [LNodeId],
    states: &[Vec<BarycenterState>],
    mo: &mut ModelOrderBarycenterState,
) {
    for i in 1..layer.len() {
        let temp = layer[i];
        let mut j = i;
        while j > 0 && mo.compare(a, states, layer[j - 1], temp) > 0 {
            layer[j] = layer[j - 1];
            j -= 1;
        }
        layer[j] = temp;
    }
}

fn randomize_barycenters(
    a: &LGraphArena,
    nodes: &[LNodeId],
    resolver: &mut ForsterConstraintResolver,
    random: &mut JavaRandom,
) {
    for &node in nodes {
        // Set barycenters only for nodeGroups containing a single node.
        let value = random.next_double();
        let (l, n) = state_indices(a, node);
        let state = &mut resolver.barycenter_states[l][n];
        state.barycenter = Some(value);
        state.summed_weight = value;
        state.degree = 1;
    }
}

fn fill_in_unknown_barycenters(
    a: &LGraphArena,
    nodes: &[LNodeId],
    resolver: &mut ForsterConstraintResolver,
    random: &mut JavaRandom,
    pre_ordered: bool,
) {
    // Determine placements for nodes with undefined barycenter value
    if pre_ordered {
        let mut last_value = -1.0f64;

        for (index, &node) in nodes.iter().enumerate() {
            let (l, n) = state_indices(a, node);
            let value = resolver.barycenter_states[l][n].barycenter;

            let value = match value {
                Some(v) => v,
                None => {
                    // The barycenter is undefined - take the center of the
                    // previous value and the next defined value in the list
                    let mut next_value = last_value + 1.0;

                    for &next_node in &nodes[index + 1..] {
                        let (nl, nn) = state_indices(a, next_node);
                        if let Some(x) = resolver.barycenter_states[nl][nn].barycenter {
                            next_value = x;
                            break;
                        }
                    }

                    let value = (last_value + next_value) / 2.0;
                    let state = &mut resolver.barycenter_states[l][n];
                    state.barycenter = Some(value);
                    state.summed_weight = value;
                    state.degree = 1;
                    value
                }
            };

            last_value = value;
        }
    } else {
        // No previous ordering - determine random placement for new nodes
        let mut max_bary = 0.0f64;
        for &node in nodes {
            let (l, n) = state_indices(a, node);
            if let Some(b) = resolver.barycenter_states[l][n].barycenter {
                max_bary = max_bary.max(b);
            }
        }

        max_bary += 2.0;
        for &node in nodes {
            let (l, n) = state_indices(a, node);
            if resolver.barycenter_states[l][n].barycenter.is_none() {
                // float promoted to double before the multiplication
                let value = random.next_float() as f64 * max_bary - 1.0;
                let state = &mut resolver.barycenter_states[l][n];
                state.barycenter = Some(value);
                state.summed_weight = value;
                state.degree = 1;
            }
        }
    }
}

fn calculate_barycenters(
    a: &LGraphArena,
    nodes: &[LNodeId],
    resolver: &mut ForsterConstraintResolver,
    distributor: &PortDistributor,
    random: &mut JavaRandom,
    forward: bool,
) {
    // Set all visited flags to false
    for &node in nodes {
        let (l, n) = state_indices(a, node);
        resolver.barycenter_states[l][n].visited = false;
    }

    for &node in nodes {
        // Calculate the node groups's new barycenter (may be None)
        calculate_barycenter(a, node, resolver, distributor, random, forward);
    }
}

/// The recursive `calculateBarycenter`. Handles in-layer edges; may
/// give incorrect results if the in-layer edges form a cycle.
fn calculate_barycenter(
    a: &LGraphArena,
    node: LNodeId,
    resolver: &mut ForsterConstraintResolver,
    distributor: &PortDistributor,
    random: &mut JavaRandom,
    forward: bool,
) {
    let (l, n) = state_indices(a, node);

    // Check if the node group's barycenter was already computed
    if resolver.barycenter_states[l][n].visited {
        return;
    } else {
        resolver.barycenter_states[l][n].visited = true;
    }

    {
        let state = &mut resolver.barycenter_states[l][n];
        state.degree = 0;
        state.summed_weight = 0.0;
        state.barycenter = None;
    }

    for free_port in a.node(node).ports.clone() {
        // forward: predecessor ports (sources of incoming edges);
        // backward: successor ports (targets of outgoing edges)
        let fixed_ports: Vec<crate::alg_layered::graph::LPortId> = if forward {
            a.port(free_port).incoming_edges.iter().map(|&e| a.edge(e).source.unwrap()).collect()
        } else {
            a.port(free_port).outgoing_edges.iter().map(|&e| a.edge(e).target.unwrap()).collect()
        };
        for fixed_port in fixed_ports {
            // If the node the fixed port belongs to is part of the free
            // layer (thus, if we have an in-layer edge), use that node's
            // barycenter calculation instead
            let fixed_node = a.port(fixed_port).node.unwrap();

            if a.node(fixed_node).layer == a.node(node).layer {
                // Self-loops are ignored
                if fixed_node != node {
                    // Find the fixed node's node group and calculate its barycenter
                    calculate_barycenter(a, fixed_node, resolver, distributor, random, forward);

                    // Update this node group's values
                    let (fl, fn_) = state_indices(a, fixed_node);
                    let fixed_degree = resolver.barycenter_states[fl][fn_].degree;
                    let fixed_summed = resolver.barycenter_states[fl][fn_].summed_weight;
                    let state = &mut resolver.barycenter_states[l][n];
                    state.degree += fixed_degree;
                    state.summed_weight += fixed_summed;
                }
            } else {
                let rank = distributor.port_ranks()[a.port(fixed_port).id as usize];
                let state = &mut resolver.barycenter_states[l][n];
                state.summed_weight += rank as f64; // float promoted to double
                state.degree += 1;
            }
        }
    }

    // Iterate over the node's barycenter associates
    let barycenter_associates: Option<Vec<LNodeId>> =
        a.node(node).properties.try_get(&iprops::BARYCENTER_ASSOCIATES);
    if let Some(associates) = barycenter_associates {
        for associate in associates {
            // Make sure the associate is in the same layer as this node
            if a.node(node).layer == a.node(associate).layer {
                // Find the associate's node group and calculate its barycenter
                calculate_barycenter(a, associate, resolver, distributor, random, forward);

                // Update this vertex's values
                let (al, an) = state_indices(a, associate);
                let assoc_degree = resolver.barycenter_states[al][an].degree;
                let assoc_summed = resolver.barycenter_states[al][an].summed_weight;
                let state = &mut resolver.barycenter_states[l][n];
                state.degree += assoc_degree;
                state.summed_weight += assoc_summed;
            }
        }
    }

    if resolver.barycenter_states[l][n].degree > 0 {
        // add a small random perturbation in order to increase diversity of solutions
        // (computed in float, then promoted to double)
        let perturbation = random.next_float() * RANDOM_AMOUNT - RANDOM_AMOUNT / 2.0;
        let state = &mut resolver.barycenter_states[l][n];
        state.summed_weight += perturbation as f64;
        state.barycenter = Some(state.summed_weight / state.degree as f64);
    }
}

// ----------------------------------------------------------------- helpers

fn is_external_port_dummy(a: &LGraphArena, first_node: LNodeId) -> bool {
    a.node(first_node).node_type == NodeType::EXTERNAL_PORT
}

fn change_index(dir: bool) -> i32 {
    if dir {
        1
    } else {
        -1
    }
}

fn port_type_for(direction: bool) -> crate::alg_layered::options_gen::PortType {
    if direction {
        crate::alg_layered::options_gen::PortType::OUTPUT
    } else {
        crate::alg_layered::options_gen::PortType::INPUT
    }
}

fn start_index(dir: bool, length: usize) -> usize {
    if dir {
        0
    } else {
        length.saturating_sub(1) // Math.max(0, length - 1)
    }
}

fn is_first_layer(node_order: &[Vec<LNodeId>], current_index: usize, forward_sweep: bool) -> bool {
    current_index == start_index(forward_sweep, node_order.len())
}
