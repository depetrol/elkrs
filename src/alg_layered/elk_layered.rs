//! The algorithm driver (flat layout only for now).

use crate::core::javacompat::JavaRandom;
use crate::core::options::{ContentAlignment, PortSide, SizeConstraint, SizeOptions};
use crate::graph::math::KVector;
use crate::graph::properties::EnumSet;

use crate::alg_layered::components::ComponentsProcessor;
use crate::alg_layered::configurator;
use crate::alg_layered::graph::{LGraphArena, LGraphId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::GraphProperties;
use crate::alg_layered::phases::PipelineStep;
use crate::alg_layered::processors;

pub fn do_layout(a: &mut LGraphArena, lgraph: LGraphId) -> Result<(), String> {
    // the random number generator
    let mut random = make_random(a, lgraph);

    let pipeline = configurator::prepare_graph_for_layout(a, lgraph)?;

    let mut components = ComponentsProcessor::split(a, lgraph)?;
    for &component in &components {
        layout(a, component, &pipeline, &mut random)?;
    }
    ComponentsProcessor::combine(a, &mut components, lgraph)?;

    resize_graph(a, lgraph);
    Ok(())
}

/// The random number generator created from `RANDOM_SEED`.
fn make_random(a: &LGraphArena, lgraph: LGraphId) -> JavaRandom {
    let random_seed: i32 = a.graph(lgraph).properties.get(&lopts::RANDOM_SEED);
    if random_seed == 0 {
        JavaRandom::new(1) // time-based seed would not be reproducible
    } else {
        JavaRandom::new(random_seed as i64)
    }
}

pub fn do_compound_layout(a: &mut LGraphArena, lgraph: LGraphId) -> Result<(), String> {
    // Preprocess the compound graph by splitting cross-hierarchy edges.
    crate::alg_layered::compound::preprocess(a, lgraph)?;

    hierarchical_layout(a, lgraph)?;

    // Postprocess the compound graph by combining split cross-hierarchy edges.
    crate::alg_layered::compound::postprocess(a, lgraph)?;

    Ok(())
}

fn hierarchical_layout(a: &mut LGraphArena, lgraph: LGraphId) -> Result<(), String> {
    // Perform a reversed breadth first search: the graphs in the lowest
    // hierarchy come first.
    let graphs = collect_all_graphs_bottom_up(a, lgraph);

    // Make sure hierarchical processors don't break control flow (#228).
    review_and_correct_hierarchical_processors(a, lgraph, &graphs)?;

    // Random number generator is created from the root graph.
    let mut random = make_random(a, lgraph);

    // Get list of processors for each graph, since they can be different.
    let mut graphs_and_algorithms: Vec<(LGraphId, Vec<PipelineStep>, usize)> = Vec::new();
    for &g in &graphs {
        let pipeline = configurator::prepare_graph_for_layout(a, g)?;
        graphs_and_algorithms.push((g, pipeline, 0));
    }

    // The root graph is the last one in the bottom-up list.
    let root_index = graphs_and_algorithms.len() - 1;

    // When the root graph has finished layout, the layout is complete.
    loop {
        if graphs_and_algorithms[root_index].2 >= graphs_and_algorithms[root_index].1.len() {
            break;
        }
        // Layout from bottom up.
        for gi in 0..graphs_and_algorithms.len() {
            loop {
                let (graph, ref pipeline, step) = graphs_and_algorithms[gi];
                if step >= pipeline.len() {
                    break;
                }
                let processor = pipeline[step];
                let is_hierarchical = is_hierarchy_aware(processor);
                let is_root = a.graph(graph).parent_node.is_none();

                if !is_hierarchical {
                    run_step(a, graph, processor, &mut random)?;
                    graphs_and_algorithms[gi].2 += 1;
                } else if is_root {
                    // Hierarchy-aware processor runs once on the root.
                    run_step(a, graph, processor, &mut random)?;
                    graphs_and_algorithms[gi].2 += 1;
                    // Continue with the graph at the bottom of the hierarchy.
                    break;
                } else {
                    // Operates on full hierarchy and is not root. The non-root
                    // graph SKIPS the hierarchical processor (which the root runs
                    // on its behalf) and resumes at the next processor on the
                    // following visit.
                    graphs_and_algorithms[gi].2 += 1;
                    break;
                }
            }
        }
    }

    Ok(())
}

/// A processor is hierarchy-aware iff it is the LAYER_SWEEP crossing
/// minimization phase or the one/two-sided greedy switch intermediate
/// processors.
fn is_hierarchy_aware(step: PipelineStep) -> bool {
    use crate::alg_layered::options_gen::CrossingMinimizationStrategy;
    use crate::alg_layered::phases::IntermediateProcessorStrategy as Ips;
    match step {
        PipelineStep::CrossingMinimization(CrossingMinimizationStrategy::LAYER_SWEEP) => true,
        PipelineStep::Intermediate(Ips::ONE_SIDED_GREEDY_SWITCH)
        | PipelineStep::Intermediate(Ips::TWO_SIDED_GREEDY_SWITCH) => true,
        _ => false,
    }
}

/// Runs a single pipeline step on a graph (does not move nodes out of layers,
/// unlike the flat `layout`, since the hierarchical resizer / final phases
/// handle that).
fn run_step(
    a: &mut LGraphArena,
    graph: LGraphId,
    step: PipelineStep,
    random: &mut JavaRandom,
) -> Result<(), String> {
    match step {
        PipelineStep::Intermediate(strategy) => processors::process(strategy, a, graph, random),
        PipelineStep::CycleBreaking(s) => crate::alg_layered::p1cycles::process(s, a, graph, random),
        PipelineStep::Layering(s) => crate::alg_layered::p2layers::process(s, a, graph, random),
        PipelineStep::CrossingMinimization(s) => crate::alg_layered::p3order::process(s, a, graph, random),
        PipelineStep::NodePlacement(s) => crate::alg_layered::p4nodes::process(s, a, graph, random),
        PipelineStep::EdgeRouting(s) => crate::alg_layered::p5edges::process(s, a, graph, random),
    }
}

/// Breadth-first search in the
/// compound graph with reversed order (innermost graphs first).
fn collect_all_graphs_bottom_up(a: &LGraphArena, root: LGraphId) -> Vec<LGraphId> {
    // both deques are used as stacks (push = push_front, pop = pop_front).
    let mut collected: std::collections::VecDeque<LGraphId> = std::collections::VecDeque::new();
    let mut to_search: std::collections::VecDeque<LGraphId> = std::collections::VecDeque::new();
    collected.push_front(root);
    to_search.push_front(root);

    while let Some(next_graph) = to_search.pop_front() {
        for &node in &a.graph(next_graph).layerless_nodes {
            if let Some(nested) = a.node(node).nested_graph {
                collected.push_front(nested);
                to_search.push_front(nested);
            }
        }
    }
    collected.into_iter().collect()
}

fn review_and_correct_hierarchical_processors(
    a: &mut LGraphArena,
    root: LGraphId,
    graphs: &[LGraphId],
) -> Result<(), String> {
    use crate::alg_layered::options_gen::{CrossingMinimizationStrategy, GreedySwitchType};

    let parent_cms: CrossingMinimizationStrategy =
        a.graph(root).properties.get(&lopts::CROSSING_MINIMIZATION_STRATEGY);
    for &child in graphs {
        let child_cms: CrossingMinimizationStrategy =
            a.graph(child).properties.get(&lopts::CROSSING_MINIMIZATION_STRATEGY);
        if child_cms != parent_cms {
            return Err(format!(
                "The hierarchy aware processor {child_cms:?} in a child node is only allowed if \
                 the root node specifies the same hierarchical processor."
            ));
        }
    }

    // Greedy switch (copy the root behaviour to all children).
    let root_type: GreedySwitchType = a
        .graph(root)
        .properties
        .get(&lopts::CROSSING_MINIMIZATION_GREEDY_SWITCH_HIERARCHICAL_TYPE);
    for &g in graphs {
        a.graph(g)
            .properties
            .set(&lopts::CROSSING_MINIMIZATION_GREEDY_SWITCH_HIERARCHICAL_TYPE, root_type);
    }
    Ok(())
}

fn layout(
    a: &mut LGraphArena,
    lgraph: LGraphId,
    pipeline: &[PipelineStep],
    random: &mut JavaRandom,
) -> Result<(), String> {
    for &step in pipeline {
        match step {
            PipelineStep::Intermediate(strategy) => {
                processors::process(strategy, a, lgraph, random)?
            }
            PipelineStep::CycleBreaking(s) => crate::alg_layered::p1cycles::process(s, a, lgraph, random)?,
            PipelineStep::Layering(s) => crate::alg_layered::p2layers::process(s, a, lgraph, random)?,
            PipelineStep::CrossingMinimization(s) => {
                crate::alg_layered::p3order::process(s, a, lgraph, random)?
            }
            PipelineStep::NodePlacement(s) => crate::alg_layered::p4nodes::process(s, a, lgraph, random)?,
            PipelineStep::EdgeRouting(s) => crate::alg_layered::p5edges::process(s, a, lgraph, random)?,
        }
    }

    // move all nodes away from the layers
    let layers = a.graph(lgraph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        a.graph_mut(lgraph).layerless_nodes.extend(nodes.iter().copied());
        a.layer_mut(layer).nodes.clear();
        for node in nodes {
            a.node_mut(node).layer = None;
        }
    }
    a.graph_mut(lgraph).layers.clear();
    Ok(())
}

fn resize_graph(a: &mut LGraphArena, lgraph: LGraphId) {
    let size_constraint: EnumSet<SizeConstraint> =
        a.graph(lgraph).properties.get(&lopts::NODE_SIZE_CONSTRAINTS);
    let size_options: EnumSet<SizeOptions> =
        a.graph(lgraph).properties.get(&lopts::NODE_SIZE_OPTIONS);

    let calculated_size = a.graph_actual_size(lgraph);
    let mut adjusted_size = calculated_size;

    if size_constraint.contains(SizeConstraint::MINIMUM_SIZE) {
        let mut min_size: KVector = a.graph(lgraph).properties.get(&lopts::NODE_SIZE_MINIMUM);
        if size_options.contains(SizeOptions::DEFAULT_MINIMUM_SIZE) {
            if min_size.x <= 0.0 {
                min_size.x = crate::core::elkutil::DEFAULT_MIN_WIDTH;
            }
            if min_size.y <= 0.0 {
                min_size.y = crate::core::elkutil::DEFAULT_MIN_HEIGHT;
            }
        }
        adjusted_size.x = f64::max(calculated_size.x, min_size.x);
        adjusted_size.y = f64::max(calculated_size.y, min_size.y);
    }

    if !a.graph(lgraph).properties.get(&lopts::NODE_SIZE_FIXED_GRAPH_SIZE) {
        resize_graph_no_really_i_mean_it(a, lgraph, calculated_size, adjusted_size);
    }
}

fn resize_graph_no_really_i_mean_it(
    a: &mut LGraphArena,
    lgraph: LGraphId,
    old_size: KVector,
    new_size: KVector,
) {
    let content_alignment: EnumSet<ContentAlignment> =
        a.graph(lgraph).properties.get(&lopts::CONTENT_ALIGNMENT);

    if new_size.x > old_size.x {
        if content_alignment.contains(ContentAlignment::H_CENTER) {
            a.graph_mut(lgraph).offset.x += (new_size.x - old_size.x) / 2.0;
        } else if content_alignment.contains(ContentAlignment::H_RIGHT) {
            a.graph_mut(lgraph).offset.x += new_size.x - old_size.x;
        }
    }
    if new_size.y > old_size.y {
        if content_alignment.contains(ContentAlignment::V_CENTER) {
            a.graph_mut(lgraph).offset.y += (new_size.y - old_size.y) / 2.0;
        } else if content_alignment.contains(ContentAlignment::V_BOTTOM) {
            a.graph_mut(lgraph).offset.y += new_size.y - old_size.y;
        }
    }

    let graph_properties: EnumSet<GraphProperties> =
        a.graph(lgraph).properties.get(&iprops::GRAPH_PROPERTIES);
    if graph_properties.contains(GraphProperties::EXTERNAL_PORTS)
        && (new_size.x > old_size.x || new_size.y > old_size.y)
    {
        let nodes = a.graph(lgraph).layerless_nodes.clone();
        for node in nodes {
            if a.node(node).node_type == NodeType::EXTERNAL_PORT {
                let ext_port_side: PortSide =
                    a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
                if ext_port_side == PortSide::EAST {
                    a.node_mut(node).pos.x += new_size.x - old_size.x;
                } else if ext_port_side == PortSide::SOUTH {
                    a.node_mut(node).pos.y += new_size.y - old_size.y;
                }
            }
        }
    }

    let padding = a.graph(lgraph).padding;
    a.graph_mut(lgraph).size.x = new_size.x - padding.left - padding.right;
    a.graph_mut(lgraph).size.y = new_size.y - padding.top - padding.bottom;
}
