//! Port of `LayeredLayoutProvider`: hooks ELK Layered into the core engine.

use elk_core::registry::{AlgorithmData, AlgorithmRegistry, GraphFeature, LayoutProvider};
use elk_graph::graph::{ElkGraph, NodeId};
use elk_graph::properties::EnumSet;

use crate::elk_layered;
use crate::graph::LGraphArena;
use crate::importer::ElkGraphImporter;
use crate::transferrer;

#[derive(Default)]
pub struct LayeredLayoutProvider;

impl LayoutProvider for LayeredLayoutProvider {
    fn layout(&mut self, elk: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        let mut arena = LGraphArena::new();
        let lgraph = {
            let mut importer = ElkGraphImporter::new(elk);
            importer.import_graph(layout_node, &mut arena)?
        };
        elk_layered::do_layout(&mut arena, lgraph)?;
        transferrer::apply_layout(&mut arena, elk, lgraph, layout_node)
    }
}

/// Registers the layered algorithm and its layout options.
pub fn register(
    options: &mut elk_core::data::LayoutMetaDataRegistry,
    algorithms: &mut AlgorithmRegistry,
) {
    crate::options_gen::register_layered_options(options);
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
