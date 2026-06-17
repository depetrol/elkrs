//! Phase 1: cycle breaking (`org.eclipse.elk.alg.layered.p1cycles`).

pub mod bfs_node_order;
pub mod depth_first;
pub mod dfs_node_order;
pub mod greedy;
pub mod group_model_order_calculator;
pub mod interactive;
pub mod model_order;
pub mod scc;

use crate::core::javacompat::JavaRandom;

use crate::alg_layered::graph::{LGraphArena, LGraphId};
use crate::alg_layered::options_gen::CycleBreakingStrategy;
use crate::alg_layered::phases::{IntermediateProcessorStrategy as Ips, LayeredPhases, ProcessorConfiguration};

/// The processor configuration contributed by the selected cycle breaker.
pub fn processor_configuration(
    strategy: CycleBreakingStrategy,
    _a: &LGraphArena,
    _graph: LGraphId,
    config: &mut ProcessorConfiguration,
) -> Result<(), String> {
    match strategy {
        CycleBreakingStrategy::GREEDY
        | CycleBreakingStrategy::DEPTH_FIRST
        | CycleBreakingStrategy::MODEL_ORDER
        | CycleBreakingStrategy::GREEDY_MODEL_ORDER
        | CycleBreakingStrategy::BFS_NODE_ORDER
        | CycleBreakingStrategy::DFS_NODE_ORDER
        | CycleBreakingStrategy::SCC_CONNECTIVITY
        | CycleBreakingStrategy::SCC_NODE_TYPE => {
            config.add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::REVERSED_EDGE_RESTORER);
            Ok(())
        }
        CycleBreakingStrategy::INTERACTIVE => {
            config
                .add_before(
                    LayeredPhases::P1_CYCLE_BREAKING,
                    Ips::INTERACTIVE_EXTERNAL_PORT_POSITIONER,
                )
                .add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::REVERSED_EDGE_RESTORER);
            Ok(())
        }
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
        CycleBreakingStrategy::DEPTH_FIRST => depth_first::process(a, graph),
        CycleBreakingStrategy::MODEL_ORDER => model_order::process(a, graph),
        CycleBreakingStrategy::GREEDY_MODEL_ORDER => greedy::process_model_order(a, graph, random),
        CycleBreakingStrategy::BFS_NODE_ORDER => bfs_node_order::process(a, graph),
        CycleBreakingStrategy::DFS_NODE_ORDER => dfs_node_order::process(a, graph),
        CycleBreakingStrategy::SCC_CONNECTIVITY => scc::process_connectivity(a, graph),
        CycleBreakingStrategy::SCC_NODE_TYPE => scc::process_node_type(a, graph),
        CycleBreakingStrategy::INTERACTIVE => interactive::process(a, graph),
    }
}
