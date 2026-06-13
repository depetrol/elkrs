//! Phase 3: crossing minimization (`org.eclipse.elk.alg.layered.p3order`).

pub mod barycenter_heuristic;
pub mod counting;
pub mod forster_constraint_resolver;
pub mod graph_info_holder;
pub mod greedy_port_distributor;
pub mod greedy_switch;
pub mod layer_sweep;
pub mod layer_sweep_type_decider;
pub mod model_order_barycenter_heuristic;
pub mod model_order_comparators;
pub mod port_distributor;
pub mod sweep_copy;

use crate::graph::{LGraphArena, LGraphId};
use elk_core::javacompat::JavaRandom;
use crate::options_gen::CrossingMinimizationStrategy;
use crate::phases::{IntermediateProcessorStrategy as Ips, LayeredPhases, ProcessorConfiguration};

pub fn processor_configuration(
    strategy: CrossingMinimizationStrategy,
    _a: &LGraphArena,
    _graph: LGraphId,
    config: &mut ProcessorConfiguration,
) -> Result<(), String> {
    match strategy {
        CrossingMinimizationStrategy::LAYER_SWEEP => {
            config
                .add_before(LayeredPhases::P3_NODE_ORDERING, Ips::LONG_EDGE_SPLITTER)
                .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::IN_LAYER_CONSTRAINT_PROCESSOR)
                .add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::LONG_EDGE_JOINER)
                .add_before(LayeredPhases::P3_NODE_ORDERING, Ips::PORT_LIST_SORTER);
            Ok(())
        }
        other => Err(format!("TODO: crossing minimization strategy {other:?} is not ported yet")),
    }
}

pub fn process(
    strategy: CrossingMinimizationStrategy,
    a: &mut LGraphArena,
    graph: LGraphId,
    random: &mut JavaRandom,
) -> Result<(), String> {
    match strategy {
        CrossingMinimizationStrategy::LAYER_SWEEP => layer_sweep::process(a, graph, random),
        other => Err(format!("TODO: crossing minimization strategy {other:?} is not ported yet")),
    }
}
