//! Rust port of `org.eclipse.elk.alg.rectpacking` — ELK's rectangle packing
//! algorithm (`org.eclipse.elk.rectpacking`).

pub mod options;
pub mod p1widthapproximation;
pub mod p2packing;
pub mod p3whitespaceelimination;
pub mod provider;
pub mod util;

use crate::core::data::LayoutMetaDataRegistry;
use crate::core::registry::{AlgorithmData, AlgorithmRegistry};
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::properties::EnumSet;

/// Registers the rectpacking algorithm and its layout options, mirroring
/// `RectPackingMetaDataProvider` (which also applies `RectPackingOptions`).
pub fn register(options: &mut LayoutMetaDataRegistry, algorithms: &mut AlgorithmRegistry) {
    options::register_rectpacking_options(options);

    algorithms.register(AlgorithmData {
        id: "org.eclipse.elk.rectpacking",
        name: "ELK Rectangle Packing",
        // no supportedFeatures.
        features: EnumSet::none(),
        create: || Box::new(provider::RectPackingLayoutProvider),
    });
}

/// Sets
/// `interactive` on a root node configured for the rectpacking algorithm.
pub fn set_interactive_options(g: &mut ElkGraph, root: NodeId) {
    let algorithm: Option<String> = g
        .node(root)
        .properties
        .try_get(&crate::core::options::ALGORITHM);
    if let Some(algorithm) = algorithm {
        if options::ALGORITHM_ID.ends_with(&algorithm) && !g.node(root).children.is_empty() {
            g.node(root).properties.set(&options::INTERACTIVE, true);
        }
    }
}
