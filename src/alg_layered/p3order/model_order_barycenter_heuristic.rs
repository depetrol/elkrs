//!
//! Extends `BarycenterHeuristic`, overriding only the sort comparator and the
//! single-layer `minimizeCrossings`. All barycenter computation, randomization
//! and sweeping is inherited (in this port, reused from
//! `barycenter_heuristic`). The override lives in
//! [`ModelOrderBarycenterState`] plus [`minimize_crossings_list`] in the
//! barycenter heuristic.

use std::collections::{HashMap, HashSet};

use crate::alg_layered::graph::{LGraphArena, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::{GroupOrderStrategy, LayerConstraint};

use super::barycenter_heuristic::BarycenterState;
use super::model_order_comparators::{calculate_model_order_or_group_model_order, Elem};

/// State of the `ModelOrderBarycenterHeuristic`: the transitive ordering maps
/// and whether `CROSSING_MINIMIZATION_FORCE_NODE_MODEL_ORDER` is active.
pub struct ModelOrderBarycenterState {
    pub force_node_model_order: bool,
    bigger_than: HashMap<LNodeId, HashSet<LNodeId>>,
    smaller_than: HashMap<LNodeId, HashSet<LNodeId>>,
}

impl ModelOrderBarycenterState {
    pub fn new(force_node_model_order: bool) -> Self {
        ModelOrderBarycenterState {
            force_node_model_order,
            bigger_than: HashMap::new(),
            smaller_than: HashMap::new(),
        }
    }

    pub fn clear_transitive_ordering(&mut self) {
        self.bigger_than = HashMap::new();
        self.smaller_than = HashMap::new();
    }

    /// The `barycenterStateComparator` lambda. `states[l][i]` is the
    /// barycenter state array (indexed by layer.id and node.id).
    pub fn compare(
        &mut self,
        a: &LGraphArena,
        states: &[Vec<BarycenterState>],
        n1: LNodeId,
        n2: LNodeId,
    ) -> i32 {
        // FIRST_SEPARATE / LAST_SEPARATE nodes are incomparable -> 0.
        if is_separate(a, n1) || is_separate(a, n2) {
            return 0;
        }
        let lgraph = a.node_graph(n1);

        let transitive = self.compare_based_on_transitive_dependencies(n1, n2);
        if transitive != 0 {
            return transitive;
        }

        if a.node(n1).properties.has(&iprops::MODEL_ORDER)
            && a.node(n2).properties.has(&iprops::MODEL_ORDER)
        {
            let offset = a.graph(lgraph).properties.get(&iprops::MAX_MODEL_ORDER_NODES);
            let mo1 = calculate_model_order_or_group_model_order(
                a,
                lgraph,
                Elem::Node(n1),
                Elem::Node(n2),
                offset,
            );
            let mo2 = calculate_model_order_or_group_model_order(
                a,
                lgraph,
                Elem::Node(n2),
                Elem::Node(n1),
                offset,
            );
            let mut value = match mo1.cmp(&mo2) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            if a.graph(lgraph)
                .properties
                .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CM_GROUP_ORDER_STRATEGY)
                == GroupOrderStrategy::ONLY_WITHIN_GROUP
            {
                let id1 = a
                    .node(n1)
                    .properties
                    .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CROSSING_MINIMIZATION_ID);
                let id2 = a
                    .node(n2)
                    .properties
                    .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CROSSING_MINIMIZATION_ID);
                if id1 != id2 {
                    value = 0;
                }
            }
            if value < 0 {
                self.update_bigger_and_smaller(n1, n2);
                return value;
            } else if value > 0 {
                self.update_bigger_and_smaller(n2, n1);
                return value;
            }
        }
        self.compare_based_on_barycenter(a, states, n1, n2)
    }

    fn compare_based_on_transitive_dependencies(&mut self, n1: LNodeId, n2: LNodeId) -> i32 {
        if !self.bigger_than.contains_key(&n1) {
            self.bigger_than.insert(n1, HashSet::new());
        } else if self.bigger_than[&n1].contains(&n2) {
            return 1;
        }
        if !self.bigger_than.contains_key(&n2) {
            self.bigger_than.insert(n2, HashSet::new());
        } else if self.bigger_than[&n2].contains(&n1) {
            return -1;
        }
        if !self.smaller_than.contains_key(&n1) {
            self.smaller_than.insert(n1, HashSet::new());
        } else if self.smaller_than[&n1].contains(&n2) {
            return -1;
        }
        if !self.smaller_than.contains_key(&n2) {
            self.smaller_than.insert(n2, HashSet::new());
        } else if self.smaller_than[&n2].contains(&n1) {
            return 1;
        }
        0
    }

    fn compare_based_on_barycenter(
        &mut self,
        a: &LGraphArena,
        states: &[Vec<BarycenterState>],
        n1: LNodeId,
        n2: LNodeId,
    ) -> i32 {
        let (l1, i1) = state_indices(a, n1);
        let (l2, i2) = state_indices(a, n2);
        let b1 = states[l1][i1].barycenter;
        let b2 = states[l2][i2].barycenter;
        match (b1, b2) {
            (Some(v1), Some(v2)) => {
                let value = match v1.total_cmp(&v2) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
                if value < 0 {
                    self.update_bigger_and_smaller(n1, n2);
                } else if value > 0 {
                    self.update_bigger_and_smaller(n2, n1);
                }
                value
            }
            (Some(_), None) => {
                self.update_bigger_and_smaller(n1, n2);
                -1
            }
            (None, Some(_)) => {
                self.update_bigger_and_smaller(n2, n1);
                1
            }
            (None, None) => 0,
        }
    }

    fn update_bigger_and_smaller(&mut self, bigger: LNodeId, smaller: LNodeId) {
        let smaller_node_bigger_than: Vec<LNodeId> =
            self.bigger_than.get(&smaller).into_iter().flatten().copied().collect();
        let bigger_node_smaller_than: Vec<LNodeId> =
            self.smaller_than.get(&bigger).into_iter().flatten().copied().collect();

        self.bigger_than.get_mut(&bigger).unwrap().insert(smaller);
        self.smaller_than.get_mut(&smaller).unwrap().insert(bigger);

        for very_small in &smaller_node_bigger_than {
            self.bigger_than.get_mut(&bigger).unwrap().insert(*very_small);
            self.smaller_than.get_mut(very_small).unwrap().insert(bigger);
            for &x in &bigger_node_smaller_than {
                self.smaller_than.get_mut(very_small).unwrap().insert(x);
            }
        }
        for very_big in &bigger_node_smaller_than {
            self.smaller_than.get_mut(&smaller).unwrap().insert(*very_big);
            self.bigger_than.get_mut(very_big).unwrap().insert(smaller);
            for &x in &smaller_node_bigger_than {
                self.bigger_than.get_mut(very_big).unwrap().insert(x);
            }
        }
    }
}

fn is_separate(a: &LGraphArena, n: LNodeId) -> bool {
    if a.node(n).properties.has(&lopts::LAYERING_LAYER_CONSTRAINT) {
        let c: LayerConstraint = a.node(n).properties.get(&lopts::LAYERING_LAYER_CONSTRAINT);
        c == LayerConstraint::FIRST_SEPARATE || c == LayerConstraint::LAST_SEPARATE
    } else {
        false
    }
}

fn state_indices(a: &LGraphArena, node: LNodeId) -> (usize, usize) {
    let layer = a.node(node).layer.unwrap();
    (a.layer(layer).id as usize, a.node(node).id as usize)
}
