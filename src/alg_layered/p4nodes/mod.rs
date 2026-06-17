//! Phase 4: node placement (`org.eclipse.elk.alg.layered.p4nodes`).

pub mod bk;
pub mod interactive;
pub mod linear_segments;
pub mod network_simplex_placer;
pub mod simple;
pub mod bk_aligned_layout;
pub mod bk_aligner;
pub mod bk_compactor;
pub mod neighborhood_information;
pub mod threshold_strategy;

use crate::graph::properties::EnumSet;

use crate::core::javacompat::JavaRandom;

use crate::alg_layered::graph::{LGraphArena, LGraphId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen::{GraphProperties, NodePlacementStrategy};
use crate::alg_layered::phases::{IntermediateProcessorStrategy as Ips, LayeredPhases, ProcessorConfiguration};

pub fn processor_configuration(
    strategy: NodePlacementStrategy,
    a: &LGraphArena,
    graph: LGraphId,
    config: &mut ProcessorConfiguration,
) -> Result<(), String> {
    match strategy {
        // BKNodePlacer, SimpleNodePlacer, LinearSegmentsNodePlacer and
        // NetworkSimplexPlacer all add the HIERARCHICAL_PORT_POSITION_PROCESSOR
        // for graphs with external ports (and nothing otherwise).
        NodePlacementStrategy::BRANDES_KOEPF
        | NodePlacementStrategy::SIMPLE
        | NodePlacementStrategy::LINEAR_SEGMENTS
        | NodePlacementStrategy::NETWORK_SIMPLEX
        | NodePlacementStrategy::INTERACTIVE => {
            let graph_properties: EnumSet<GraphProperties> =
                a.graph(graph).properties.get(&iprops::GRAPH_PROPERTIES);
            if graph_properties.contains(GraphProperties::EXTERNAL_PORTS) {
                config.add_before(
                    LayeredPhases::P5_EDGE_ROUTING,
                    Ips::HIERARCHICAL_PORT_POSITION_PROCESSOR,
                );
            }
            Ok(())
        }
    }
}

pub fn process(
    strategy: NodePlacementStrategy,
    a: &mut LGraphArena,
    graph: LGraphId,
    _random: &mut JavaRandom,
) -> Result<(), String> {
    match strategy {
        NodePlacementStrategy::BRANDES_KOEPF => bk::process(a, graph),
        NodePlacementStrategy::SIMPLE => simple::process(a, graph),
        NodePlacementStrategy::LINEAR_SEGMENTS => linear_segments::process(a, graph),
        NodePlacementStrategy::NETWORK_SIMPLEX => network_simplex_placer::process(a, graph),
        NodePlacementStrategy::INTERACTIVE => interactive::process(a, graph),
    }
}
