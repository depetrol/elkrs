
use std::collections::HashSet;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LEdgeId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_util::edge_reverse;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::GroupOrderStrategy;

use super::group_model_order_calculator::GroupModelOrderCalculator;

/// Which SCC variant: determines `findNodes`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SccVariant {
    /// The base model-order `findNodes` variant; the MODEL_ORDER path is the
    /// GreedyModelOrder, so this base is used by neither strategy directly.
    /// Kept for completeness/reuse.
    ModelOrder,
    Connectivity,
    NodeType,
}

pub fn process_connectivity(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    process(a, graph, SccVariant::Connectivity)
}

pub fn process_node_type(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    process(a, graph, SccVariant::NodeType)
}

fn process(a: &mut LGraphArena, graph: LGraphId, variant: SccVariant) -> Result<(), String> {
    let offset = (a.graph(graph).layerless_nodes.len() as i32)
        .max(a.graph(graph).properties.get(&iprops::MAX_MODEL_ORDER_NODES));
    let big_offset =
        offset.wrapping_mul(a.graph(graph).properties.get(&iprops::CB_NUM_MODEL_ORDER_GROUPS));

    loop {
        let mut tarjan = Tarjan::new();
        tarjan.reset(a, graph);
        tarjan.run(a, graph);

        if tarjan.strongly_connected_components.is_empty() {
            break;
        }

        let mut rev_edges: Vec<LEdgeId> = Vec::new();
        find_nodes(a, graph, variant, &tarjan.strongly_connected_components, offset, big_offset, &mut rev_edges);

        for edge in rev_edges {
            edge_reverse(a, graph, edge, false);
            let src_node = a.edge_source_node(edge);
            let lid: i32 = a.node(src_node).properties.get(&lopts::LAYERING_LAYER_ID);
            a.node(src_node).properties.set(&lopts::LAYERING_LAYER_ID, lid + 1);
            a.graph(graph).properties.set(&iprops::CYCLIC, true);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn find_nodes(
    a: &LGraphArena,
    graph: LGraphId,
    variant: SccVariant,
    sccs: &[Vec<LNodeId>],
    offset: i32,
    big_offset: i32,
    rev_edges: &mut Vec<LEdgeId>,
) {
    let enforce_group_model_order = a
        .graph(graph)
        .properties
        .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CB_GROUP_ORDER_STRATEGY)
        == GroupOrderStrategy::ENFORCED;

    let mo = |calc: &mut GroupModelOrderCalculator, n: LNodeId| -> i32 {
        if enforce_group_model_order {
            calc.compute_constraint_group_model_order(a, n, big_offset, offset)
        } else {
            calc.compute_constraint_model_order(a, n, offset)
        }
    };

    for scc in sccs {
        if variant != SccVariant::ModelOrder && scc.len() <= 1 {
            continue;
        }
        let scc_set: HashSet<LNodeId> = scc.iter().copied().collect();
        let mut calc = GroupModelOrderCalculator::new();

        match variant {
            SccVariant::ModelOrder => {
                // Reverse outgoing edges of the maximum model-order node.
                let mut max: Option<LNodeId> = None;
                let mut max_model_order = i32::MIN;
                for &n in scc {
                    let cur = mo(&mut calc, n);
                    if max.is_none() {
                        max = Some(n);
                        max_model_order = cur;
                    } else if max_model_order < cur {
                        max = Some(n);
                        max_model_order = cur;
                    }
                }
                let max = max.unwrap();
                for edge in a.node_outgoing_edges(max) {
                    if scc_set.contains(&a.edge_target_node(edge)) {
                        rev_edges.push(edge);
                    }
                }
            }
            SccVariant::Connectivity | SccVariant::NodeType => {
                let mut min: Option<LNodeId> = None;
                let mut max: Option<LNodeId> = None;
                let mut model_order_min = i32::MAX;
                let mut model_order_max = i32::MIN;
                for &n in scc {
                    if min.is_none() || max.is_none() {
                        let cur = mo(&mut calc, n);
                        min = Some(n);
                        model_order_min = cur;
                        max = Some(n);
                        model_order_max = cur;
                    } else {
                        let cur = mo(&mut calc, n);
                        if model_order_min > cur {
                            min = Some(n);
                            model_order_min = cur;
                        }
                        if model_order_max < cur {
                            max = Some(n);
                            model_order_max = cur;
                        }
                    }
                }
                let min = min.unwrap();
                let max = max.unwrap();

                if variant == SccVariant::NodeType {
                    let min_group: i32 = a
                        .node(min)
                        .properties
                        .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CYCLE_BREAKING_ID);
                    let max_group: i32 = a
                        .node(max)
                        .properties
                        .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CYCLE_BREAKING_ID);
                    let preferred_source = a
                        .graph(graph)
                        .properties
                        .try_get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CB_PREFERRED_SOURCE_ID);
                    let preferred_target = a
                        .graph(graph)
                        .properties
                        .try_get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CB_PREFERRED_TARGET_ID);
                    if Some(min_group) == preferred_source {
                        for edge in a.node_incoming_edges(min) {
                            if scc_set.contains(&a.edge_source_node(edge)) {
                                rev_edges.push(edge);
                            }
                        }
                        continue;
                    } else if Some(max_group) == preferred_target {
                        for edge in a.node_outgoing_edges(max) {
                            if scc_set.contains(&a.edge_source_node(edge)) {
                                rev_edges.push(edge);
                            }
                        }
                        continue;
                    }
                    // else fall through to connectivity decision below.
                }

                // Connectivity decision.
                if a.node_incoming_edges(min).len() > a.node_outgoing_edges(max).len() {
                    for edge in a.node_incoming_edges(min) {
                        if scc_set.contains(&a.edge_source_node(edge)) {
                            rev_edges.push(edge);
                        }
                    }
                } else {
                    for edge in a.node_outgoing_edges(max) {
                        if scc_set.contains(&a.edge_target_node(edge)) {
                            rev_edges.push(edge);
                        }
                    }
                }
            }
        }
    }
}

struct Tarjan {
    index: i32,
    stack: Vec<LNodeId>,
    strongly_connected_components: Vec<Vec<LNodeId>>,
}

impl Tarjan {
    fn new() -> Self {
        Tarjan { index: 0, stack: Vec::new(), strongly_connected_components: Vec::new() }
    }

    fn reset(&mut self, a: &mut LGraphArena, graph: LGraphId) {
        let nodes = a.graph(graph).layerless_nodes.clone();
        for n in nodes {
            a.node(n).properties.set(&iprops::TARJAN_ON_STACK, false);
            a.node(n).properties.set(&iprops::TARJAN_LOWLINK, -1);
            a.node(n).properties.set(&iprops::TARJAN_ID, -1);
            self.stack.clear();
            for e in a.node_connected_edges(n) {
                a.edge(e).properties.set(&iprops::IS_PART_OF_CYCLE, false);
            }
        }
    }

    fn run(&mut self, a: &mut LGraphArena, graph: LGraphId) {
        self.index = 0;
        self.stack.clear();
        let nodes = a.graph(graph).layerless_nodes.clone();
        for node in nodes {
            if a.node(node).properties.get::<i32>(&iprops::TARJAN_ID) == -1 {
                self.strongly_connected(a, node);
                self.stack.clear();
            }
        }
    }

    fn strongly_connected(&mut self, a: &mut LGraphArena, v: LNodeId) {
        a.node(v).properties.set(&iprops::TARJAN_ID, self.index);
        a.node(v).properties.set(&iprops::TARJAN_LOWLINK, self.index);
        self.index += 1;
        self.stack.push(v);
        a.node(v).properties.set(&iprops::TARJAN_ON_STACK, true);

        for edge in a.node_connected_edges(v) {
            // `edgesToBeReversed` is always empty here, so the contains()
            // checks reduce to: skip incoming edges of v.
            let source_is_v = a.edge_source_node(edge) == v;
            if !source_is_v {
                continue;
            }
            let target = if a.edge_target_node(edge) == v {
                a.edge_source_node(edge)
            } else {
                a.edge_target_node(edge)
            };
            if a.node(target).properties.get::<i32>(&iprops::TARJAN_ID) == -1 {
                self.strongly_connected(a, target);
                let vl: i32 = a.node(v).properties.get(&iprops::TARJAN_LOWLINK);
                let tl: i32 = a.node(target).properties.get(&iprops::TARJAN_LOWLINK);
                a.node(v).properties.set(&iprops::TARJAN_LOWLINK, vl.min(tl));
            } else if a.node(target).properties.get(&iprops::TARJAN_ON_STACK) {
                let vl: i32 = a.node(v).properties.get(&iprops::TARJAN_LOWLINK);
                let ti: i32 = a.node(target).properties.get(&iprops::TARJAN_ID);
                a.node(v).properties.set(&iprops::TARJAN_LOWLINK, vl.min(ti));
            }
        }

        let vl: i32 = a.node(v).properties.get(&iprops::TARJAN_LOWLINK);
        let vid: i32 = a.node(v).properties.get(&iprops::TARJAN_ID);
        if vl == vid {
            let mut scc: Vec<LNodeId> = Vec::new();
            loop {
                let n = self.stack.pop().unwrap();
                a.node(n).properties.set(&iprops::TARJAN_ON_STACK, false);
                scc.push(n);
                if v == n {
                    break;
                }
            }
            if scc.len() > 1 {
                self.strongly_connected_components.push(scc);
            }
        }
    }
}
