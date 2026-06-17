//! Rust port of `org.eclipse.elk.alg.disco` — the DisCo disconnected
//! components layouter (`org.eclipse.elk.disco`).

pub mod compactor;
pub mod graph;
pub mod options;
pub mod transform;

use crate::core::data::LayoutMetaDataRegistry;
use crate::core::registry::{AlgorithmData, AlgorithmRegistry, LayoutProvider};
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::properties::EnumSet;

/// Registers the disco algorithm and its layout options, mirroring
/// `DisCoMetaDataProvider`.
pub fn register(options: &mut LayoutMetaDataRegistry, algorithms: &mut AlgorithmRegistry) {
    options::register_disco_options(options);

    algorithms.register(AlgorithmData {
        id: "org.eclipse.elk.disco",
        name: "ELK DisCo",
        features: EnumSet::none(),
        create: || Box::new(DisCoLayoutProvider),
    });
}

#[derive(Default)]
pub struct DisCoLayoutProvider;

impl LayoutProvider for DisCoLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        let component_spacing: f64 = g
            .node(layout_node)
            .properties
            .get(&options::SPACING_COMPONENT_COMPONENT);

        // If desired, apply a layout algorithm to the connected components
        // themselves. (This port has no global registry to draw providers from.)
        if g.node(layout_node)
            .properties
            .has(&options::COMPONENT_COMPACTION_COMPONENT_LAYOUT_ALGORITHM)
        {
            return Err(
                "org.eclipse.elk.disco.componentCompaction.componentLayoutAlgorithm is not \
                 supported by this port"
                    .to_string(),
            );
        }

        // 1.) Transform the graph into a DCGraph.
        let mut transformer = transform::ElkGraphTransformer::new(component_spacing);
        let mut result = transformer.import_graph(g, layout_node);

        // 2.) Compact the DCGraph (only polyomino compaction at the moment).
        match g
            .node(layout_node)
            .properties
            .get(&options::COMPONENT_COMPACTION_STRATEGY)
        {
            options::CompactionStrategy::POLYOMINO => {
                let polys_debug = compactor::compact(&mut result);
                g.node(layout_node)
                    .properties
                    .set(&options::DEBUG_DISCO_POLYS, polys_debug);
            }
        }

        // 3.) Apply the new layout to the input graph.
        transformer.apply_layout(g, &result);

        // Stores the DCGraph object itself here; the JSON oracle prints
        // it as "...DCGraph@<identityhash>". The identity hash cannot be
        // reproduced, so the expected goldens strip the "@..." suffix.
        g.node(layout_node).properties.set(
            &options::DEBUG_DISCO_GRAPH,
            "org.eclipse.elk.alg.disco.graph.DCGraph".to_string(),
        );

        Ok(())
    }
}
