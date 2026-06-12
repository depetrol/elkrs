//! Port of `ElkLayered`: the algorithm driver (flat layout only for now).

use elk_core::javacompat::JavaRandom;
use elk_core::options::{ContentAlignment, HierarchyHandling, PortSide, SizeConstraint, SizeOptions};
use elk_graph::math::KVector;
use elk_graph::properties::EnumSet;

use crate::components::ComponentsProcessor;
use crate::configurator;
use crate::graph::{LGraphArena, LGraphId, NodeType};
use crate::internal_properties as iprops;
use crate::options_gen as lopts;
use crate::options_gen::GraphProperties;
use crate::phases::PipelineStep;
use crate::processors;

/// Port of `ElkLayered.doLayout`.
pub fn do_layout(a: &mut LGraphArena, lgraph: LGraphId) -> Result<(), String> {
    if a.graph(lgraph)
        .properties
        .get::<HierarchyHandling>(&lopts::HIERARCHY_HANDLING)
        == HierarchyHandling::INCLUDE_CHILDREN
    {
        return Err("TODO: compound (INCLUDE_CHILDREN) layout is not ported yet".to_string());
    }

    // the random number generator (Java: stored in the RANDOM property)
    let random_seed: i32 = a.graph(lgraph).properties.get(&lopts::RANDOM_SEED);
    let mut random = if random_seed == 0 {
        JavaRandom::new(1) // Java uses time-based here; not reproducible
    } else {
        JavaRandom::new(random_seed as i64)
    };

    let pipeline = configurator::prepare_graph_for_layout(a, lgraph)?;

    let mut components = ComponentsProcessor::split(a, lgraph)?;
    for &component in &components {
        layout(a, component, &pipeline, &mut random)?;
    }
    ComponentsProcessor::combine(a, &mut components, lgraph)?;

    resize_graph(a, lgraph);
    Ok(())
}

/// Port of `ElkLayered.layout` (single component).
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
            PipelineStep::CycleBreaking(s) => crate::p1cycles::process(s, a, lgraph, random)?,
            PipelineStep::Layering(s) => crate::p2layers::process(s, a, lgraph, random)?,
            PipelineStep::CrossingMinimization(s) => {
                crate::p3order::process(s, a, lgraph, random)?
            }
            PipelineStep::NodePlacement(s) => crate::p4nodes::process(s, a, lgraph, random)?,
            PipelineStep::EdgeRouting(s) => crate::p5edges::process(s, a, lgraph, random)?,
        }
    }

    // move all nodes away from the layers (Java: end of ElkLayered#layout)
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

/// Port of `ElkLayered.resizeGraph`.
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
                min_size.x = elk_core::elkutil::DEFAULT_MIN_WIDTH;
            }
            if min_size.y <= 0.0 {
                min_size.y = elk_core::elkutil::DEFAULT_MIN_HEIGHT;
            }
        }
        adjusted_size.x = f64::max(calculated_size.x, min_size.x);
        adjusted_size.y = f64::max(calculated_size.y, min_size.y);
    }

    if !a.graph(lgraph).properties.get(&lopts::NODE_SIZE_FIXED_GRAPH_SIZE) {
        resize_graph_no_really_i_mean_it(a, lgraph, calculated_size, adjusted_size);
    }
}

/// Port of `resizeGraphNoReallyIMeanIt`.
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
