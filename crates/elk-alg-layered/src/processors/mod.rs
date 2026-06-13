//! Intermediate processors (`org.eclipse.elk.alg.layered.intermediate`).
//! Each ported processor lives in its own module; unported ones fail loudly.

pub mod comment_node_margin_calculator;
pub mod comment_postprocessor;
pub mod comment_preprocessor;
pub mod edge_and_layer_constraint_edge_reverser;
pub mod end_label_postprocessor;
pub mod end_label_preprocessor;
pub mod end_label_sorter;
pub mod final_spline_bendpoints_calculator;
pub mod graph_transformer;
pub mod high_degree_node_layer_processor;
pub mod horizontal_compactor;
pub mod hyperedge_dummy_merger;
pub mod hypernode_processor;
pub mod label_dummy_inserter;
pub mod label_dummy_remover;
pub mod label_dummy_switcher;
pub mod label_side_selector;
pub mod in_layer_constraint_processor;
pub mod innermost_node_margin_calculator;
pub mod inverted_port_processor;
pub mod label_and_node_size_processor;
pub mod layer_constraint_postprocessor;
pub mod layer_constraint_preprocessor;
pub mod layer_size_and_graph_height_calculator;
pub mod long_edge_joiner;
pub mod long_edge_splitter;
pub mod node_promotion;
pub mod north_south_port_postprocessor;
pub mod partition_midprocessor;
pub mod partition_postprocessor;
pub mod partition_preprocessor;
pub mod north_south_port_preprocessor;
pub mod port_list_sorter;
pub mod port_side_processor;
pub mod reversed_edge_restorer;
pub mod self_loop_port_restorer;
pub mod self_loop_post_processor;
pub mod self_loop_pre_processor;
pub mod self_loop_router;

use elk_core::javacompat::JavaRandom;

use crate::graph::{LGraphArena, LGraphId};
use crate::p3order::layer_sweep::{self, CrossMinType};
use crate::phases::IntermediateProcessorStrategy as Ips;

/// Dispatches an intermediate processor (Java: `IntermediateProcessorStrategy.create()` + run).
pub fn process(
    strategy: Ips,
    a: &mut LGraphArena,
    graph: LGraphId,
    random: &mut JavaRandom,
) -> Result<(), String> {
    match strategy {
        Ips::EDGE_AND_LAYER_CONSTRAINT_EDGE_REVERSER => {
            edge_and_layer_constraint_edge_reverser::process(a, graph)
        }
        // Java quirk: IntermediateProcessorStrategy (ELK 0.11.0) maps the
        // modes opposite to their names — DIRECTION_PREPROCESSOR creates
        // GraphTransformer(TO_INPUT_DIRECTION) and DIRECTION_POSTPROCESSOR
        // creates GraphTransformer(TO_INTERNAL_LTR). Only the UP +
        // READING_DIRECTION case is sensitive to the mode.
        Ips::DIRECTION_PREPROCESSOR => {
            graph_transformer::process(a, graph, graph_transformer::Mode::ToInputDirection)
        }
        Ips::DIRECTION_POSTPROCESSOR => {
            graph_transformer::process(a, graph, graph_transformer::Mode::ToInternalLtr)
        }
        Ips::LAYER_CONSTRAINT_PREPROCESSOR => layer_constraint_preprocessor::process(a, graph),
        Ips::NODE_PROMOTION => node_promotion::process(a, graph),
        Ips::LAYER_CONSTRAINT_POSTPROCESSOR => layer_constraint_postprocessor::process(a, graph),
        Ips::LONG_EDGE_SPLITTER => long_edge_splitter::process(a, graph),
        Ips::PORT_SIDE_PROCESSOR => port_side_processor::process(a, graph),
        Ips::INVERTED_PORT_PROCESSOR => inverted_port_processor::process(a, graph),
        Ips::PORT_LIST_SORTER => port_list_sorter::process(a, graph),
        Ips::NORTH_SOUTH_PORT_PREPROCESSOR => north_south_port_preprocessor::process(a, graph),
        Ips::NORTH_SOUTH_PORT_POSTPROCESSOR => north_south_port_postprocessor::process(a, graph),
        Ips::IN_LAYER_CONSTRAINT_PROCESSOR => in_layer_constraint_processor::process(a, graph),
        Ips::LABEL_AND_NODE_SIZE_PROCESSOR => label_and_node_size_processor::process(a, graph),
        Ips::INNERMOST_NODE_MARGIN_CALCULATOR => {
            innermost_node_margin_calculator::process(a, graph)
        }
        Ips::LAYER_SIZE_AND_GRAPH_HEIGHT_CALCULATOR => {
            layer_size_and_graph_height_calculator::process(a, graph)
        }
        Ips::LONG_EDGE_JOINER => long_edge_joiner::process(a, graph),
        Ips::FINAL_SPLINE_BENDPOINTS_CALCULATOR => {
            final_spline_bendpoints_calculator::process(a, graph)
        }
        Ips::COMMENT_PREPROCESSOR => comment_preprocessor::process(a, graph),
        Ips::COMMENT_NODE_MARGIN_CALCULATOR => comment_node_margin_calculator::process(a, graph),
        Ips::COMMENT_POSTPROCESSOR => comment_postprocessor::process(a, graph),
        Ips::LABEL_DUMMY_INSERTER => label_dummy_inserter::process(a, graph),
        Ips::LABEL_DUMMY_SWITCHER => label_dummy_switcher::process(a, graph),
        Ips::LABEL_SIDE_SELECTOR => label_side_selector::process(a, graph),
        Ips::LABEL_DUMMY_REMOVER => label_dummy_remover::process(a, graph),
        Ips::END_LABEL_PREPROCESSOR => end_label_preprocessor::process(a, graph),
        Ips::END_LABEL_SORTER => end_label_sorter::process(a, graph),
        Ips::END_LABEL_POSTPROCESSOR => end_label_postprocessor::process(a, graph),
        Ips::REVERSED_EDGE_RESTORER => reversed_edge_restorer::process(a, graph),
        Ips::SELF_LOOP_PREPROCESSOR => self_loop_pre_processor::process(a, graph),
        Ips::SELF_LOOP_PORT_RESTORER => self_loop_port_restorer::process(a, graph),
        Ips::SELF_LOOP_ROUTER => self_loop_router::process(a, graph, random),
        Ips::SELF_LOOP_POSTPROCESSOR => self_loop_post_processor::process(a, graph),
        // Java: new LayerSweepCrossingMinimizer(CrossMinType.ONE_SIDED_GREEDY_SWITCH)
        Ips::ONE_SIDED_GREEDY_SWITCH => {
            layer_sweep::process_with_type(a, graph, random, CrossMinType::OneSidedGreedySwitch)
        }
        // Java: new LayerSweepCrossingMinimizer(CrossMinType.TWO_SIDED_GREEDY_SWITCH)
        Ips::TWO_SIDED_GREEDY_SWITCH => {
            layer_sweep::process_with_type(a, graph, random, CrossMinType::TwoSidedGreedySwitch)
        }
        Ips::PARTITION_PREPROCESSOR => partition_preprocessor::process(a, graph),
        Ips::PARTITION_MIDPROCESSOR => partition_midprocessor::process(a, graph),
        Ips::PARTITION_POSTPROCESSOR => partition_postprocessor::process(a, graph),
        Ips::HYPEREDGE_DUMMY_MERGER => hyperedge_dummy_merger::process(a, graph),
        Ips::HYPERNODE_PROCESSOR => hypernode_processor::process(a, graph),
        Ips::HIGH_DEGREE_NODE_LAYER_PROCESSOR => {
            high_degree_node_layer_processor::process(a, graph)
        }
        Ips::HORIZONTAL_COMPACTOR => horizontal_compactor::process(a, graph),
        other => Err(format!("TODO: intermediate processor {other:?} is not ported yet")),
    }
}
