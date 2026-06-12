//! Phase 2: layering (`org.eclipse.elk.alg.layered.p2layers`).

pub mod longest_path;
pub mod network_simplex;

use elk_core::javacompat::JavaRandom;

use crate::graph::{LGraphArena, LGraphId};
use crate::options_gen::LayeringStrategy;
use crate::phases::{IntermediateProcessorStrategy as Ips, LayeredPhases, ProcessorConfiguration};

pub fn processor_configuration(
    strategy: LayeringStrategy,
    _a: &LGraphArena,
    _graph: LGraphId,
    config: &mut ProcessorConfiguration,
) -> Result<(), String> {
    match strategy {
        LayeringStrategy::NETWORK_SIMPLEX
        | LayeringStrategy::LONGEST_PATH
        | LayeringStrategy::LONGEST_PATH_SOURCE => {
            config
                .add_before(
                    LayeredPhases::P1_CYCLE_BREAKING,
                    Ips::EDGE_AND_LAYER_CONSTRAINT_EDGE_REVERSER,
                )
                .add_before(LayeredPhases::P2_LAYERING, Ips::LAYER_CONSTRAINT_PREPROCESSOR)
                .add_before(LayeredPhases::P3_NODE_ORDERING, Ips::LAYER_CONSTRAINT_POSTPROCESSOR);
            Ok(())
        }
        other => Err(format!("TODO: layering strategy {other:?} is not ported yet")),
    }
}

pub fn process(
    strategy: LayeringStrategy,
    a: &mut LGraphArena,
    graph: LGraphId,
    _random: &mut JavaRandom,
) -> Result<(), String> {
    match strategy {
        LayeringStrategy::NETWORK_SIMPLEX => network_simplex::process(a, graph),
        LayeringStrategy::LONGEST_PATH => longest_path::process(a, graph),
        other => Err(format!("TODO: layering strategy {other:?} is not ported yet")),
    }
}
