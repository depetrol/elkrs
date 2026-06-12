//! Phase 1: cycle breaking (`org.eclipse.elk.alg.layered.p1cycles`).

pub mod greedy;

use elk_core::javacompat::JavaRandom;

use crate::graph::{LGraphArena, LGraphId};
use crate::options_gen::CycleBreakingStrategy;
use crate::phases::{IntermediateProcessorStrategy as Ips, LayeredPhases, ProcessorConfiguration};

/// The processor configuration contributed by the selected cycle breaker.
pub fn processor_configuration(
    strategy: CycleBreakingStrategy,
    _a: &LGraphArena,
    _graph: LGraphId,
    config: &mut ProcessorConfiguration,
) -> Result<(), String> {
    match strategy {
        CycleBreakingStrategy::GREEDY | CycleBreakingStrategy::DEPTH_FIRST => {
            config.add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::REVERSED_EDGE_RESTORER);
            Ok(())
        }
        other => Err(format!("TODO: cycle breaking strategy {other:?} is not ported yet")),
    }
}

/// Runs the selected cycle breaker.
pub fn process(
    strategy: CycleBreakingStrategy,
    a: &mut LGraphArena,
    graph: LGraphId,
    random: &mut JavaRandom,
) -> Result<(), String> {
    match strategy {
        CycleBreakingStrategy::GREEDY => greedy::process(a, graph, random),
        other => Err(format!("TODO: cycle breaking strategy {other:?} is not ported yet")),
    }
}
