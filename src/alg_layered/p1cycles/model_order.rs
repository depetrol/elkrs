//!
//! Reverses all edges that go against the (group) model order, i.e. edges from
//! high model order to low model order.

use crate::alg_layered::graph::{LGraphArena, LGraphId, LEdgeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_util::edge_reverse;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::GroupOrderStrategy;

use super::group_model_order_calculator::GroupModelOrderCalculator;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mut rev_edges: Vec<LEdgeId> = Vec::new();

    let layerless = a.graph(graph).layerless_nodes.clone();
    let offset = (layerless.len() as i32)
        .max(a.graph(graph).properties.get(&iprops::MAX_MODEL_ORDER_NODES));
    let big_offset = offset.wrapping_mul(a.graph(graph).properties.get(&iprops::CB_NUM_MODEL_ORDER_GROUPS));
    let enforce_group_model_order = a
        .graph(graph)
        .properties
        .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CB_GROUP_ORDER_STRATEGY)
        == GroupOrderStrategy::ENFORCED;

    for source in layerless {
        let mut calculator = GroupModelOrderCalculator::new();
        let model_order_source = if enforce_group_model_order {
            calculator.compute_constraint_group_model_order(a, source, big_offset, offset)
        } else {
            calculator.compute_constraint_model_order(a, source, offset)
        };

        // source.getPorts(PortType.OUTPUT) -> ports with outgoing edges.
        let out_ports: Vec<_> = a
            .node(source)
            .ports
            .iter()
            .copied()
            .filter(|&p| !a.port(p).outgoing_edges.is_empty())
            .collect();
        for port in out_ports {
            for edge in a.port(port).outgoing_edges.clone() {
                let target = a.edge_target_node(edge);
                let model_order_target = if enforce_group_model_order {
                    calculator.compute_constraint_group_model_order(a, target, big_offset, offset)
                } else {
                    calculator.compute_constraint_model_order(a, target, offset)
                };
                if model_order_target < model_order_source {
                    rev_edges.push(edge);
                }
            }
        }
    }

    for edge in rev_edges {
        edge_reverse(a, graph, edge, true);
        a.graph(graph).properties.set(&iprops::CYCLIC, true);
    }

    Ok(())
}
