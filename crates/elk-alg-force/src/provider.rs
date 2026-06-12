//! Port of `org.eclipse.elk.alg.force.ForceLayoutProvider`.

use elk_core::javacompat::JavaRandom;
use elk_core::registry::LayoutProvider;
use elk_graph::graph::{ElkGraph, NodeId};
use elk_graph::properties::EnumSet;

use crate::model::{self, EadesModel, ForceModel, FruchtermanReingoldModel};
use crate::options::{self, ForceModelStrategy};
use crate::{components, importer};

/// Port of `ForceLayoutProvider`.
#[derive(Default)]
pub struct ForceLayoutProvider;

impl LayoutProvider for ForceLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        force_layout(g, layout_node)
    }
}

/// `ForceLayoutProvider.layout` as a free function so the stress provider can
/// reuse it (Java instantiates a `ForceLayoutProvider` there).
pub(crate) fn force_layout(g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
    // if requested, compute nodes's dimensions, place node labels, ports,
    // port labels, etc.
    if !g
        .node(layout_node)
        .properties
        .get(&options::OMIT_NODE_MICRO_LAYOUT)
    {
        check_node_micro_layout(g, layout_node)?;
    }

    // transform the input graph
    let (mut arena, fgraph) = importer::import_graph(g, layout_node)?;

    // set special properties for the layered graph (ForceLayoutProvider.setOptions):
    // create the random number generator based on the random seed option.
    // ForceOptions.RANDOM_SEED has default 1, so Java's null check never fires.
    let random_seed: i32 = fgraph.properties.get(&options::RANDOM_SEED);
    let mut random = if random_seed == 0 {
        // Java: new Random() — seeded from the system clock, not reproducible
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
    // instance, like Java (it is stored in the copied property maps there)
    for comp in &mut comps {
        model::layout(force_model.as_mut(), &mut arena, comp, &mut random);
    }

    // pack the components back into one graph
    let fgraph = components::recombine(&mut arena, comps);

    // apply the layout results to the original graph
    importer::apply_layout(&arena, &fgraph, g, layout_node);

    Ok(())
}

// TODO(nodespacing): Java runs `NodeMicroLayout.forGraph(elkGraph).execute()`
// here (alg.common nodespacing: sortPortLists, calculateLabelAndNodeSizes,
// calculateNodeMargins), which is not ported yet. For graphs where it would
// be a no-op with respect to the force result (no ports, no node size
// constraints, no node label placement) we proceed without it; otherwise we
// fail loudly instead of silently diverging from Java. Node margins, which
// Java always computes, are not read by the force/stress algorithms.
pub(crate) fn check_node_micro_layout(g: &ElkGraph, layout_node: NodeId) -> Result<(), String> {
    for &child in &g.node(layout_node).children {
        let node = g.node(child);
        if !node.ports.is_empty() {
            return Err(
                "TODO(nodespacing): node micro layout (port placement) is not ported yet; \
                 set org.eclipse.elk.omitNodeMicroLayout=true or remove ports"
                    .to_string(),
            );
        }
        let constraints: EnumSet<elk_core::options::SizeConstraint> =
            node.properties.get(&options::NODE_SIZE_CONSTRAINTS);
        if !constraints.is_empty() {
            return Err(
                "TODO(nodespacing): node micro layout (node size calculation) is not ported \
                 yet; set org.eclipse.elk.omitNodeMicroLayout=true or remove nodeSize.constraints"
                    .to_string(),
            );
        }
        if !node.labels.is_empty() {
            let placement: EnumSet<elk_core::options::NodeLabelPlacement> =
                node.properties.get(&options::NODE_LABELS_PLACEMENT);
            if !placement.is_empty() {
                return Err(
                    "TODO(nodespacing): node micro layout (node label placement) is not \
                     ported yet; set org.eclipse.elk.omitNodeMicroLayout=true or remove \
                     nodeLabels.placement"
                        .to_string(),
                );
            }
        }
    }
    Ok(())
}
