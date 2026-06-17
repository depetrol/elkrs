//! Phase 2: layering (`org.eclipse.elk.alg.layered.p2layers`).

pub mod breadth_first_model_order;
pub mod coffman_graham;
pub mod depth_first_model_order;
pub mod interactive;
pub mod longest_path;
pub mod longest_path_source;
pub mod min_width;
pub mod network_simplex;
pub mod stretch_width;

use crate::core::javacompat::JavaRandom;

use crate::alg_layered::graph::{LGraphArena, LGraphId};
use crate::alg_layered::options_gen::LayeringStrategy;
use crate::alg_layered::phases::{IntermediateProcessorStrategy as Ips, LayeredPhases, ProcessorConfiguration};

pub fn processor_configuration(
    strategy: LayeringStrategy,
    _a: &LGraphArena,
    _graph: LGraphId,
    config: &mut ProcessorConfiguration,
) -> Result<(), String> {
    match strategy {
        // NetworkSimplexLayerer, LongestPathLayerer, LongestPathSourceLayerer,
        // CoffmanGrahamLayerer, MinWidthLayerer and StretchWidthLayerer all
        // declare the same baseline intermediate processing configuration.
        LayeringStrategy::NETWORK_SIMPLEX
        | LayeringStrategy::LONGEST_PATH
        | LayeringStrategy::LONGEST_PATH_SOURCE
        | LayeringStrategy::COFFMAN_GRAHAM
        | LayeringStrategy::MIN_WIDTH
        | LayeringStrategy::STRETCH_WIDTH
        | LayeringStrategy::BF_MODEL_ORDER
        | LayeringStrategy::DF_MODEL_ORDER => {
            config
                .add_before(
                    LayeredPhases::P1_CYCLE_BREAKING,
                    Ips::EDGE_AND_LAYER_CONSTRAINT_EDGE_REVERSER,
                )
                .add_before(LayeredPhases::P2_LAYERING, Ips::LAYER_CONSTRAINT_PREPROCESSOR)
                .add_before(LayeredPhases::P3_NODE_ORDERING, Ips::LAYER_CONSTRAINT_POSTPROCESSOR);
            Ok(())
        }
        LayeringStrategy::INTERACTIVE => {
            config
                .add_before(
                    LayeredPhases::P1_CYCLE_BREAKING,
                    Ips::INTERACTIVE_EXTERNAL_PORT_POSITIONER,
                )
                .add_before(LayeredPhases::P2_LAYERING, Ips::LAYER_CONSTRAINT_PREPROCESSOR)
                .add_before(LayeredPhases::P3_NODE_ORDERING, Ips::LAYER_CONSTRAINT_POSTPROCESSOR);
            Ok(())
        }
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
        LayeringStrategy::LONGEST_PATH_SOURCE => longest_path_source::process(a, graph),
        LayeringStrategy::COFFMAN_GRAHAM => coffman_graham::process(a, graph),
        LayeringStrategy::MIN_WIDTH => min_width::process(a, graph),
        LayeringStrategy::STRETCH_WIDTH => stretch_width::process(a, graph),
        LayeringStrategy::BF_MODEL_ORDER => breadth_first_model_order::process(a, graph),
        LayeringStrategy::DF_MODEL_ORDER => depth_first_model_order::process(a, graph),
        LayeringStrategy::INTERACTIVE => interactive::process(a, graph),
    }
}
