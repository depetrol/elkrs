
use crate::core::javacompat::JavaRandom;
use crate::core::registry::LayoutProvider;
use crate::graph::graph::{ElkGraph, NodeId};

use crate::alg_force::model::{self, EadesModel, ForceModel, FruchtermanReingoldModel};
use crate::alg_force::options::{self, ForceModelStrategy};
use crate::alg_force::{components, importer};

#[derive(Default)]
pub struct ForceLayoutProvider;

impl LayoutProvider for ForceLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        force_layout(g, layout_node)
    }
}

/// `ForceLayoutProvider.layout` as a free function so the stress provider can
/// reuse it.
pub(crate) fn force_layout(g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
    // if requested, compute nodes's dimensions, place node labels, ports,
    // port labels, etc.
    if !g
        .node(layout_node)
        .properties
        .get(&options::OMIT_NODE_MICRO_LAYOUT)
    {
        execute_node_micro_layout(g, layout_node);
    }

    // transform the input graph
    let (mut arena, fgraph) = importer::import_graph(g, layout_node)?;

    // set special properties for the layered graph (ForceLayoutProvider.setOptions):
    // create the random number generator based on the random seed option.
    let random_seed: i32 = fgraph.properties.get(&options::RANDOM_SEED);
    let mut random = if random_seed == 0 {
        // seeded from the system clock, not reproducible
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        JavaRandom::new(nanos)
    } else {
        JavaRandom::new(random_seed as i64)
    };

    // update the force model depending on user selection
    let strategy: ForceModelStrategy = fgraph.properties.get(&options::MODEL);
    let mut force_model: Box<dyn ForceModel> = match strategy {
        ForceModelStrategy::EADES => Box::new(EadesModel::default()),
        ForceModelStrategy::FRUCHTERMAN_REINGOLD => Box::new(FruchtermanReingoldModel::default()),
    };

    // split the input graph into components
    let mut comps = components::split(&mut arena, fgraph);

    // perform the actual layout; all components share the single Random
    // instance.
    for comp in &mut comps {
        model::layout(force_model.as_mut(), &mut arena, comp, &mut random);
    }

    // pack the components back into one graph
    let fgraph = components::recombine(&mut arena, comps);

    // apply the layout results to the original graph
    importer::apply_layout(&arena, &fgraph, g, layout_node);

    Ok(())
}

pub(crate) fn execute_node_micro_layout(g: &mut ElkGraph, layout_node: NodeId) {
    let mut adapter = crate::core::adapters::ElkGraphAdapter::new(g, layout_node);
    crate::alg_common::nodespacing::sort_port_lists(&mut adapter);
    crate::alg_common::nodespacing::calculate_label_and_node_sizes(&mut adapter, |_, _| true);
    crate::alg_common::nodespacing::calculate_node_margins(&mut adapter, false);
}
