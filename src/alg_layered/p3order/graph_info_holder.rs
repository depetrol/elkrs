//! Collects data needed for cross minimization
//! and port distribution for one graph of the hierarchy.
//!
//! The `IInitializable.init(...)` traversal is flattened into the
//! constructor below; the per-level hooks of all participating objects are
//! invoked in exactly the same order as the initializable list
//! `[this, crossingsCounter, layerSweepTypeDecider, portDistributor,
//! (constraintResolver,) crossMinimizer]` (the constraint resolver only
//! participates for the barycenter heuristic).

use crate::core::javacompat::JavaRandom;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::GraphProperties;

use super::counting::AllCrossingsCounter;
use super::forster_constraint_resolver::ForsterConstraintResolver;
use super::greedy_switch::GreedySwitchHeuristic;
use super::layer_sweep::CrossMinType;
use super::layer_sweep_type_decider::LayerSweepTypeDecider;
use super::port_distributor::SweepPortDistributor;
use super::sweep_copy::SweepCopy;

/// The `ICrossingMinimizationHeuristic` field (`crossMinimizer`).
pub enum CrossMinimizer {
    /// `BarycenterHeuristic` (which owns the `ForsterConstraintResolver`
    /// in this port).
    /// `model_order` is `Some` for `ModelOrderBarycenterHeuristic`
    /// (considerModelOrder.strategy != NONE or force-node-model-order).
    Barycenter {
        constraint_resolver: ForsterConstraintResolver,
        model_order: Option<super::model_order_barycenter_heuristic::ModelOrderBarycenterState>,
    },
    /// `GreedySwitchHeuristic` (one- or two-sided).
    GreedySwitch(GreedySwitchHeuristic),
}

pub struct GraphInfoHolder {
    /// Raw graph data.
    pub lgraph: LGraphId,

    /// Saved node orders.
    pub current_node_order: Vec<Vec<LNodeId>>,
    pub currently_best_node_and_port_order: Option<SweepCopy>,
    pub best_node_and_port_order: Option<SweepCopy>,
    /// Port position array (`portPositions()`, used by greedy switch).
    pub port_positions: Vec<i32>,

    /// Processing type information.
    pub use_bottom_up: bool,

    /// Hierarchy access.
    pub child_graphs: Vec<LGraphId>,
    pub has_external_ports: bool,
    pub has_parent: bool,
    pub parent: Option<LNodeId>,
    /// index of the parent's graph in the `graph_info_holders` list
    pub parent_graph_index: Option<usize>,

    /// Pre-initialized auxiliary objects.
    pub cross_min_type: CrossMinType,
    pub cross_minimizer: CrossMinimizer,
    pub port_distributor: SweepPortDistributor,
    pub crossings_counter: AllCrossingsCounter,
}

impl GraphInfoHolder {
    pub fn new(
        a: &mut LGraphArena,
        graph: LGraphId,
        cross_min_type: CrossMinType,
        graphs: &[GraphInfoHolder],
        random: &mut JavaRandom,
    ) -> Result<Self, String> {
        // currentNodeOrder = graph.toNodeArray()
        let current_node_order: Vec<Vec<LNodeId>> = {
            let layers = a.graph(graph).layers.clone();
            layers.iter().map(|&l| a.layer(l).nodes.clone()).collect()
        };
        let num_layers = current_node_order.len();

        // Hierarchy information.
        let parent = a.graph(graph).parent_node;
        let has_parent = parent.is_some();
        let parent_graph_index =
            parent.map(|p| a.graph(a.node_graph(p)).id as usize);
        if let Some(idx) = parent_graph_index {
            debug_assert!(idx < graphs.len());
        }
        let graph_properties = a.graph(graph).properties.get(&iprops::GRAPH_PROPERTIES);
        let has_external_ports = graph_properties.contains(GraphProperties::EXTERNAL_PORTS);
        let mut child_graphs: Vec<LGraphId> = Vec::new();

        // Init all objects needing initialization by graph traversal.
        let mut crossings_counter = AllCrossingsCounter::new(num_layers);
        // (the RANDOM graph property is the `random` parameter here)
        let mut port_distributor = SweepPortDistributor::create(cross_min_type, random, num_layers);
        let mut decider = LayerSweepTypeDecider::new(num_layers);

        let mut cross_minimizer = match cross_min_type {
            CrossMinType::Barycenter => {
                // Use ModelOrderBarycenterHeuristic ONLY when
                // CROSSING_MINIMIZATION_FORCE_NODE_MODEL_ORDER is set.
                // For considerModelOrder.strategy != NONE the *plain*
                // BarycenterHeuristic is used; the model order is preserved by
                // SortByInputModelProcessor + FIRST_TRY_WITH_INITIAL_ORDER.
                let force = a
                    .graph(graph)
                    .properties
                    .get(&lopts::CROSSING_MINIMIZATION_FORCE_NODE_MODEL_ORDER);
                let model_order = if force {
                    Some(super::model_order_barycenter_heuristic::ModelOrderBarycenterState::new(
                        force,
                    ))
                } else {
                    None
                };
                CrossMinimizer::Barycenter {
                    constraint_resolver: ForsterConstraintResolver::new(a, &current_node_order),
                    model_order,
                }
            }
            CrossMinType::Median => {
                return Err("TODO: MedianHeuristic not ported yet".to_string());
            }
            CrossMinType::OneSidedGreedySwitch | CrossMinType::TwoSidedGreedySwitch => {
                CrossMinimizer::GreedySwitch(GreedySwitchHeuristic::new(cross_min_type))
            }
        };

        // Apply Initializer (IInitializable.init), in the order
        // [this, crossingsCounter, layerSweepTypeDecider, portDistributor,
        //  (constraintResolver,) crossMinimizer].
        let mut n_ports_holder: i32 = 0;
        for l in 0..current_node_order.len() {
            // --- initAtLayerLevel
            // this: (nothing)
            // crossingsCounter: (nothing)
            decider.init_at_layer_level(a, l, &current_node_order);
            port_distributor.init_at_layer_level(l, &current_node_order);
            match &mut cross_minimizer {
                CrossMinimizer::Barycenter { constraint_resolver, .. } => {
                    constraint_resolver.init_at_layer_level(l, &current_node_order);
                    // crossMinimizer (BarycenterHeuristic):
                    // nodeOrder[l][0].getLayer().id = l
                    let layer = a.node(current_node_order[l][0]).layer.unwrap();
                    a.layer_mut(layer).id = l as i32;
                }
                CrossMinimizer::GreedySwitch(heuristic) => {
                    // crossMinimizer (GreedySwitchHeuristic): layer.id = l
                    heuristic.init_at_layer_level(a, l, &current_node_order);
                }
            }

            for n in 0..current_node_order[l].len() {
                // --- initAtNodeLevel
                // this: collect child graphs
                let node = current_node_order[l][n];
                if let Some(nested) = a.node(node).nested_graph {
                    child_graphs.push(nested);
                }
                crossings_counter.init_at_node_level(a, l, n, &current_node_order);
                decider.init_at_node_level(a, l, n, &current_node_order);
                port_distributor.init_at_node_level(a, l, n, &current_node_order);
                if let CrossMinimizer::Barycenter { constraint_resolver, .. } = &mut cross_minimizer {
                    constraint_resolver.init_at_node_level(a, l, n, &current_node_order);
                }
                // crossMinimizer: (nothing at node level)

                let num_ports = a.node(node).ports.len();
                for p in 0..num_ports {
                    // --- initAtPortLevel
                    // this: nPorts++
                    n_ports_holder += 1;
                    crossings_counter.init_at_port_level(a, l, n, p, &current_node_order);
                    port_distributor.init_at_port_level(a, l, n, p, &current_node_order);
                    if let CrossMinimizer::GreedySwitch(heuristic) = &mut cross_minimizer {
                        // crossMinimizer (GreedySwitchHeuristic): nPorts++
                        heuristic.init_at_port_level();
                    }

                    // --- initAtEdgeLevel (only the crossings counter uses it)
                    let port = a.node(node).ports[p];
                    for (e, edge) in a.port_connected_edges(port).into_iter().enumerate() {
                        crossings_counter.init_at_edge_level(a, l, n, p, e, edge, &current_node_order);
                    }
                }
            }
        }
        // --- initAfterTraversal
        let port_positions = vec![0; n_ports_holder as usize];
        crossings_counter.init_after_traversal();
        port_distributor.init_after_traversal();
        match &mut cross_minimizer {
            // BarycenterHeuristic's initAfterTraversal only captures
            // references to the resolver's states and the distributor's port
            // ranks, which this port passes explicitly at the call sites.
            CrossMinimizer::Barycenter { .. } => {}
            CrossMinimizer::GreedySwitch(heuristic) => heuristic.init_after_traversal(),
        }

        // calculate whether we need to use bottom up or sweep into this graph.
        let cross_min_deterministic = cross_min_deterministic(cross_min_type);
        let use_bottom_up =
            decider.use_bottom_up(a, graph, parent, cross_min_deterministic, &current_node_order);

        // Make the graph data the greedy switch heuristic needs available
        // (GreedySwitchHeuristic holds a reference to this holder).
        if let CrossMinimizer::GreedySwitch(heuristic) = &mut cross_minimizer {
            heuristic.has_parent = has_parent;
            heuristic.dont_sweep_into = use_bottom_up;
        }

        Ok(GraphInfoHolder {
            lgraph: graph,
            current_node_order,
            currently_best_node_and_port_order: None,
            best_node_and_port_order: None,
            port_positions,
            use_bottom_up,
            child_graphs,
            has_external_ports,
            has_parent,
            parent,
            parent_graph_index,
            cross_min_type,
            cross_minimizer,
            port_distributor,
            crossings_counter,
        })
    }

    /// `dontSweepInto()`.
    pub fn dont_sweep_into(&self) -> bool {
        self.use_bottom_up
    }

    /// `crossMinDeterministic()`.
    pub fn cross_min_deterministic(&self) -> bool {
        cross_min_deterministic(self.cross_min_type)
    }

    /// `crossMinAlwaysImproves()`.
    pub fn cross_min_always_improves(&self) -> bool {
        cross_min_always_improves(self.cross_min_type)
    }

    /// `getBestSweep()`.
    pub fn get_best_sweep(&self) -> Option<&SweepCopy> {
        if self.cross_min_deterministic() {
            self.currently_best_node_and_port_order.as_ref()
        } else {
            self.best_node_and_port_order.as_ref()
        }
    }
}

/// `ICrossingMinimizationHeuristic.isDeterministic()` per heuristic type.
fn cross_min_deterministic(cross_min_type: CrossMinType) -> bool {
    match cross_min_type {
        // BarycenterHeuristic.isDeterministic() == false
        CrossMinType::Barycenter => false,
        // GreedySwitchHeuristic.isDeterministic() == true
        CrossMinType::OneSidedGreedySwitch | CrossMinType::TwoSidedGreedySwitch => true,
        // MedianHeuristic.isDeterministic() == true (not ported)
        CrossMinType::Median => true,
    }
}

/// `ICrossingMinimizationHeuristic.alwaysImproves()` per heuristic type.
fn cross_min_always_improves(cross_min_type: CrossMinType) -> bool {
    match cross_min_type {
        // BarycenterHeuristic.alwaysImproves() == false
        CrossMinType::Barycenter => false,
        // GreedySwitchHeuristic.alwaysImproves() ==
        //   !(greedySwitchType == ONE_SIDED_GREEDY_SWITCH)
        CrossMinType::OneSidedGreedySwitch => false,
        CrossMinType::TwoSidedGreedySwitch => true,
        // MedianHeuristic.alwaysImproves() == false (not ported)
        CrossMinType::Median => false,
    }
}
