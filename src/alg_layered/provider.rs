//! Hooks ELK Layered into the core engine.

use crate::core::options::HierarchyHandling;
use crate::core::registry::{AlgorithmData, AlgorithmRegistry, GraphFeature, LayoutProvider};
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::properties::EnumSet;

use crate::alg_layered::elk_layered;
use crate::alg_layered::graph::LGraphArena;
use crate::alg_layered::importer::ElkGraphImporter;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::transferrer;

#[derive(Default)]
pub struct LayeredLayoutProvider;

impl LayoutProvider for LayeredLayoutProvider {
    fn layout(&mut self, elk: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        let mut arena = LGraphArena::new();
        let lgraph = {
            let mut importer = ElkGraphImporter::new(elk);
            importer.import_graph(layout_node, &mut arena)?
        };
        // LayeredLayoutProvider.layout: doCompoundLayout when the graph
        // (or any of its children) wants its children included, else doLayout.
        if elk.node(layout_node).properties.get::<HierarchyHandling>(&lopts::HIERARCHY_HANDLING)
            == HierarchyHandling::INCLUDE_CHILDREN
        {
            elk_layered::do_compound_layout(&mut arena, lgraph)?;
        } else {
            elk_layered::do_layout(&mut arena, lgraph)?;
        }
        transferrer::apply_layout(&mut arena, elk, lgraph, layout_node)
    }
}

/// Registers the layered algorithm and its layout options.
pub fn register(
    options: &mut crate::core::data::LayoutMetaDataRegistry,
    algorithms: &mut AlgorithmRegistry,
) {
    crate::alg_layered::options_gen::register_layered_options(options);
    algorithms.register(AlgorithmData {
        id: "org.eclipse.elk.layered",
        name: "ELK Layered",
        features: EnumSet::of(&[
            GraphFeature::SELF_LOOPS,
            GraphFeature::INSIDE_SELF_LOOPS,
            GraphFeature::MULTI_EDGES,
            GraphFeature::EDGE_LABELS,
            GraphFeature::PORTS,
            GraphFeature::COMPOUND,
            GraphFeature::CLUSTERS,
        ]),
        create: || Box::new(LayeredLayoutProvider),
    });
}
