//! Phase 3: crossing minimization (`org.eclipse.elk.alg.layered.p3order`).

pub mod barycenter_heuristic;
pub mod counting;
pub mod forster_constraint_resolver;
pub mod graph_info_holder;
pub mod greedy_port_distributor;
pub mod interactive;
pub mod greedy_switch;
pub mod layer_sweep;
pub mod layer_sweep_type_decider;
pub mod model_order_barycenter_heuristic;
pub mod model_order_comparators;
pub mod port_distributor;
pub mod sweep_copy;

use crate::graph::properties::EnumSet;

use crate::alg_layered::graph::{LGraphArena, LGraphId};
use crate::core::javacompat::JavaRandom;
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen::{CrossingMinimizationStrategy, GraphProperties};
use crate::alg_layered::phases::{IntermediateProcessorStrategy as Ips, LayeredPhases, ProcessorConfiguration};

pub fn processor_configuration(
    strategy: CrossingMinimizationStrategy,
    a: &LGraphArena,
    graph: LGraphId,
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
        CrossingMinimizationStrategy::INTERACTIVE => {
            config
                .add_before(LayeredPhases::P3_NODE_ORDERING, Ips::LONG_EDGE_SPLITTER)
                .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::IN_LAYER_CONSTRAINT_PROCESSOR)
                .add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::LONG_EDGE_JOINER);
            let graph_properties: EnumSet<GraphProperties> =
                a.graph(graph).properties.get(&iprops::GRAPH_PROPERTIES);
            if graph_properties.contains(GraphProperties::NON_FREE_PORTS) {
                config.add_before(LayeredPhases::P3_NODE_ORDERING, Ips::PORT_LIST_SORTER);
            }
            Ok(())
        }
        // `NoCrossingMinimizer`: same intermediate processors as LAYER_SWEEP
        // (its INTERMEDIATE_PROCESSING_CONFIGURATION plus an unconditional
        // PORT_LIST_SORTER), but the phase itself is a no-op.
        CrossingMinimizationStrategy::NONE => {
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
        CrossingMinimizationStrategy::INTERACTIVE => interactive::process(a, graph),
        // `NoCrossingMinimizer.process` is empty: keep the layer order as is.
        CrossingMinimizationStrategy::NONE => Ok(()),
        other => Err(format!("TODO: crossing minimization strategy {other:?} is not ported yet")),
    }
}
