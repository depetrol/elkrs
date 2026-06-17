
use crate::core::options::{Direction, EdgeRouting, HierarchyHandling, PortConstraints};
use crate::graph::properties::EnumSet;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_util;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::{
    CrossingMinimizationStrategy, GraphCompactionStrategy, GraphProperties, GreedySwitchType,
    LayerUnzippingStrategy, NodePromotionStrategy, OrderingStrategy, WrappingStrategy,
};
use crate::alg_layered::phases::{
    IntermediateProcessorStrategy as Ips, LayeredPhases, PipelineStep, ProcessorConfiguration,
};

const MIN_EDGE_SPACING: f64 = 2.0;

/// Configures graph properties and returns
/// the assembled pipeline (stored by the caller).
pub fn prepare_graph_for_layout(
    a: &mut LGraphArena,
    graph: LGraphId,
) -> Result<Vec<PipelineStep>, String> {
    configure_graph_properties(a, graph)?;

    // phases
    let p1 = a.graph(graph).properties.get(&lopts::CYCLE_BREAKING_STRATEGY);
    let p2 = a.graph(graph).properties.get(&lopts::LAYERING_STRATEGY);
    let p3 = a
        .graph(graph)
        .properties
        .get(&lopts::CROSSING_MINIMIZATION_STRATEGY);
    let p4 = a.graph(graph).properties.get(&lopts::NODE_PLACEMENT_STRATEGY);
    let p5 = a.graph(graph).properties.get(&lopts::EDGE_ROUTING);

    // gather the processor configuration: phase contributions first, then
    // the phase-independent configuration
    let mut config = ProcessorConfiguration::new();
    crate::alg_layered::p1cycles::processor_configuration(p1, a, graph, &mut config)?;
    crate::alg_layered::p2layers::processor_configuration(p2, a, graph, &mut config)?;
    crate::alg_layered::p3order::processor_configuration(p3, a, graph, &mut config)?;
    crate::alg_layered::p4nodes::processor_configuration(p4, a, graph, &mut config)?;
    crate::alg_layered::p5edges::processor_configuration(p5, a, graph, &mut config)?;
    let independent = phase_independent_configuration(a, graph)?;
    config.add_all(&independent);

    // assemble: slot 0, P1, slot 1, P2, ..., P5, slot 5
    let mut pipeline: Vec<PipelineStep> = Vec::new();
    for (i, phase_step) in [
        PipelineStep::CycleBreaking(p1),
        PipelineStep::Layering(p2),
        PipelineStep::CrossingMinimization(p3),
        PipelineStep::NodePlacement(p4),
        PipelineStep::EdgeRouting(p5),
    ]
    .into_iter()
    .enumerate()
    {
        pipeline.extend(config.slot(i).map(PipelineStep::Intermediate));
        pipeline.push(phase_step);
    }
    pipeline.extend(config.slot(5).map(PipelineStep::Intermediate));

    Ok(pipeline)
}

fn configure_graph_properties(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let edge_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_EDGE);
    if edge_spacing < MIN_EDGE_SPACING {
        a.graph(graph)
            .properties
            .set(&lopts::SPACING_EDGE_EDGE, MIN_EDGE_SPACING);
    }

    let direction: Direction = a.graph(graph).properties.get(&lopts::DIRECTION);
    if direction == Direction::UNDEFINED {
        let dir = lgraph_util::get_direction(a, graph);
        a.graph(graph).properties.set(&lopts::DIRECTION, dir);
    }

    // The random number generator is created by the driver from RANDOM_SEED
    // (kept in the arena).

    if !a
        .graph(graph)
        .properties
        .has(&lopts::NODE_PLACEMENT_FAVOR_STRAIGHT_EDGES)
    {
        let favor = a.graph(graph).properties.get::<EdgeRouting>(&lopts::EDGE_ROUTING)
            == EdgeRouting::ORTHOGONAL;
        a.graph(graph)
            .properties
            .set(&lopts::NODE_PLACEMENT_FAVOR_STRAIGHT_EDGES, favor);
    }

    copy_port_constraints_graph(a, graph);

    // Spacings are computed on demand in Rust (see crate::alg_layered::spacings); no
    // SPACINGS property needs to be attached.
    Ok(())
}

fn copy_port_constraints_graph(a: &mut LGraphArena, graph: LGraphId) {
    let mut nodes: Vec<LNodeId> = a.graph(graph).layerless_nodes.clone();
    for &layer in &a.graph(graph).layers {
        nodes.extend(a.layer(layer).nodes.iter().copied());
    }
    for node in nodes {
        copy_port_constraints_node(a, node);
    }
}

fn copy_port_constraints_node(a: &mut LGraphArena, node: LNodeId) {
    let original: PortConstraints = a.node(node).properties.get(&lopts::PORT_CONSTRAINTS);
    a.node(node)
        .properties
        .set(&iprops::ORIGINAL_PORT_CONSTRAINTS, original);
    if let Some(nested) = a.node(node).nested_graph {
        copy_port_constraints_graph(a, nested);
    }
}

fn phase_independent_configuration(
    a: &LGraphArena,
    graph: LGraphId,
) -> Result<ProcessorConfiguration, String> {
    let props = &a.graph(graph).properties;
    let graph_properties: EnumSet<GraphProperties> = props.get(&iprops::GRAPH_PROPERTIES);

    // Baseline
    let mut configuration = ProcessorConfiguration::new();
    configuration
        .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::INNERMOST_NODE_MARGIN_CALCULATOR)
        .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::LABEL_AND_NODE_SIZE_PROCESSOR)
        .add_before(
            LayeredPhases::P5_EDGE_ROUTING,
            Ips::LAYER_SIZE_AND_GRAPH_HEIGHT_CALCULATOR,
        )
        .add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::END_LABEL_SORTER);

    // Hierarchical layout
    if props.get::<HierarchyHandling>(&lopts::HIERARCHY_HANDLING)
        == HierarchyHandling::INCLUDE_CHILDREN
    {
        configuration.add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::HIERARCHICAL_NODE_RESIZER);
    }

    // Port side processor
    if props.get(&lopts::FEEDBACK_EDGES) {
        configuration.add_before(LayeredPhases::P1_CYCLE_BREAKING, Ips::PORT_SIDE_PROCESSOR);
    } else {
        configuration.add_before(LayeredPhases::P3_NODE_ORDERING, Ips::PORT_SIDE_PROCESSOR);
    }

    // (Label management is not supported; LABEL_MANAGER is never set.)

    if props.get(&lopts::INTERACTIVE_LAYOUT) || props.get(&lopts::GENERATE_POSITION_AND_LAYER_IDS) {
        configuration.add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::CONSTRAINTS_POSTPROCESSOR);
    }

    match props.get::<Direction>(&lopts::DIRECTION) {
        Direction::LEFT | Direction::DOWN | Direction::UP => {
            configuration
                .add_before(LayeredPhases::P1_CYCLE_BREAKING, Ips::DIRECTION_PREPROCESSOR)
                .add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::DIRECTION_POSTPROCESSOR);
        }
        _ => {}
    }

    if graph_properties.contains(GraphProperties::COMMENTS) {
        configuration
            .add_before(LayeredPhases::P1_CYCLE_BREAKING, Ips::COMMENT_PREPROCESSOR)
            .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::COMMENT_NODE_MARGIN_CALCULATOR)
            .add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::COMMENT_POSTPROCESSOR);
    }

    if props.get::<NodePromotionStrategy>(&lopts::LAYERING_NODE_PROMOTION_STRATEGY)
        != NodePromotionStrategy::NONE
    {
        configuration.add_before(LayeredPhases::P3_NODE_ORDERING, Ips::NODE_PROMOTION);
    }

    if graph_properties.contains(GraphProperties::PARTITIONS) {
        configuration
            .add_before(LayeredPhases::P1_CYCLE_BREAKING, Ips::PARTITION_PREPROCESSOR)
            .add_before(LayeredPhases::P2_LAYERING, Ips::PARTITION_MIDPROCESSOR)
            .add_before(LayeredPhases::P3_NODE_ORDERING, Ips::PARTITION_POSTPROCESSOR);
    }

    if props.get::<GraphCompactionStrategy>(&lopts::COMPACTION_POST_COMPACTION_STRATEGY)
        != GraphCompactionStrategy::NONE
        && props.get::<EdgeRouting>(&lopts::EDGE_ROUTING) != EdgeRouting::POLYLINE
    {
        configuration.add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::HORIZONTAL_COMPACTOR);
    }

    if props.get(&lopts::HIGH_DEGREE_NODES_TREATMENT) {
        configuration
            .add_before(LayeredPhases::P3_NODE_ORDERING, Ips::HIGH_DEGREE_NODE_LAYER_PROCESSOR);
    }

    if props.get(&lopts::CROSSING_MINIMIZATION_SEMI_INTERACTIVE) {
        configuration
            .add_before(LayeredPhases::P3_NODE_ORDERING, Ips::SEMI_INTERACTIVE_CROSSMIN_PROCESSOR);
    }

    if activate_greedy_switch_for(a, graph) {
        let greedy_switch_type: GreedySwitchType = if is_hierarchical_layout(a, graph) {
            props.get(&lopts::CROSSING_MINIMIZATION_GREEDY_SWITCH_HIERARCHICAL_TYPE)
        } else {
            props.get(&lopts::CROSSING_MINIMIZATION_GREEDY_SWITCH_TYPE)
        };
        let internal_greedy_type = if greedy_switch_type == GreedySwitchType::ONE_SIDED {
            Ips::ONE_SIDED_GREEDY_SWITCH
        } else {
            Ips::TWO_SIDED_GREEDY_SWITCH
        };
        configuration.add_before(LayeredPhases::P4_NODE_PLACEMENT, internal_greedy_type);
    }

    if props.get::<LayerUnzippingStrategy>(&lopts::LAYER_UNZIPPING_STRATEGY)
        == LayerUnzippingStrategy::ALTERNATING
    {
        configuration.add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::ALTERNATING_LAYER_UNZIPPER);
    }

    match props.get::<WrappingStrategy>(&lopts::WRAPPING_STRATEGY) {
        WrappingStrategy::SINGLE_EDGE => {
            configuration
                .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::SINGLE_EDGE_GRAPH_WRAPPER);
        }
        WrappingStrategy::MULTI_EDGE => {
            configuration
                .add_before(LayeredPhases::P3_NODE_ORDERING, Ips::BREAKING_POINT_INSERTER)
                .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::BREAKING_POINT_PROCESSOR)
                .add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::BREAKING_POINT_REMOVER);
        }
        WrappingStrategy::OFF => {}
    }

    if props.get::<OrderingStrategy>(&lopts::CONSIDER_MODEL_ORDER_STRATEGY) != OrderingStrategy::NONE
    {
        configuration
            .add_before(LayeredPhases::P3_NODE_ORDERING, Ips::SORT_BY_INPUT_ORDER_OF_MODEL);
    }

    Ok(configuration)
}

pub fn activate_greedy_switch_for(a: &LGraphArena, graph: LGraphId) -> bool {
    let props = &a.graph(graph).properties;
    if is_hierarchical_layout(a, graph) {
        return a.graph(graph).parent_node.is_none()
            && props.get::<GreedySwitchType>(
                &lopts::CROSSING_MINIMIZATION_GREEDY_SWITCH_HIERARCHICAL_TYPE,
            ) != GreedySwitchType::OFF;
    }

    let greedy_switch_type: GreedySwitchType =
        props.get(&lopts::CROSSING_MINIMIZATION_GREEDY_SWITCH_TYPE);
    let interactive_cross_min = props.get(&lopts::CROSSING_MINIMIZATION_SEMI_INTERACTIVE)
        || props.get::<CrossingMinimizationStrategy>(&lopts::CROSSING_MINIMIZATION_STRATEGY)
            == CrossingMinimizationStrategy::INTERACTIVE;
    let activation_threshold: i32 =
        props.get(&lopts::CROSSING_MINIMIZATION_GREEDY_SWITCH_ACTIVATION_THRESHOLD);
    let graph_size = a.graph(graph).layerless_nodes.len() as i32;

    !interactive_cross_min
        && greedy_switch_type != GreedySwitchType::OFF
        && (activation_threshold == 0 || activation_threshold > graph_size)
}

fn is_hierarchical_layout(a: &LGraphArena, graph: LGraphId) -> bool {
    a.graph(graph)
        .properties
        .get::<HierarchyHandling>(&lopts::HIERARCHY_HANDLING)
        == HierarchyHandling::INCLUDE_CHILDREN
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alg_layered::phases::IntermediateProcessorStrategy as Ips;
    use crate::alg_layered::phases::PipelineStep as Step;

    /// The default pipeline for a simple connected graph (no ports, labels,
    /// self loops, hierarchy; direction RIGHT) — derived by hand from the
    /// configurator and phase configurations.
    #[test]
    fn default_pipeline_for_simple_graph() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let pipeline = prepare_graph_for_layout(&mut a, g).unwrap();

        use crate::alg_layered::options_gen::{
            CrossingMinimizationStrategy, CycleBreakingStrategy, LayeringStrategy,
            NodePlacementStrategy,
        };
        let expected = vec![
            Step::Intermediate(Ips::EDGE_AND_LAYER_CONSTRAINT_EDGE_REVERSER),
            Step::CycleBreaking(CycleBreakingStrategy::GREEDY),
            Step::Intermediate(Ips::LAYER_CONSTRAINT_PREPROCESSOR),
            Step::Layering(LayeringStrategy::NETWORK_SIMPLEX),
            Step::Intermediate(Ips::LAYER_CONSTRAINT_POSTPROCESSOR),
            Step::Intermediate(Ips::LONG_EDGE_SPLITTER),
            Step::Intermediate(Ips::PORT_SIDE_PROCESSOR),
            Step::Intermediate(Ips::PORT_LIST_SORTER),
            Step::CrossingMinimization(CrossingMinimizationStrategy::LAYER_SWEEP),
            // default greedy switch type is TWO_SIDED with activation
            // threshold 40 (> small graph sizes)
            Step::Intermediate(Ips::TWO_SIDED_GREEDY_SWITCH),
            Step::Intermediate(Ips::IN_LAYER_CONSTRAINT_PROCESSOR),
            Step::Intermediate(Ips::LABEL_AND_NODE_SIZE_PROCESSOR),
            Step::Intermediate(Ips::INNERMOST_NODE_MARGIN_CALCULATOR),
            Step::NodePlacement(NodePlacementStrategy::BRANDES_KOEPF),
            Step::Intermediate(Ips::LAYER_SIZE_AND_GRAPH_HEIGHT_CALCULATOR),
            Step::EdgeRouting(crate::core::options::EdgeRouting::ORTHOGONAL),
            Step::Intermediate(Ips::LONG_EDGE_JOINER),
            Step::Intermediate(Ips::END_LABEL_SORTER),
            Step::Intermediate(Ips::REVERSED_EDGE_RESTORER),
        ];
        assert_eq!(pipeline, expected);
    }
}
