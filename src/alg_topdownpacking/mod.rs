//! Rust port of `org.eclipse.elk.alg.topdownpacking` — ELK's top-down packing
//! algorithm (`org.eclipse.elk.topdownpacking`).
//!
//! The algorithm places equally sized boxes in a grid and then expands them to
//! fill leftover whitespace. It is primarily meant for the engine's topdown
//! layout mode (not yet supported by elk-core), but also works standalone.

pub mod options;
pub mod provider;

use crate::core::data::LayoutMetaDataRegistry;
use crate::core::registry::{AlgorithmData, AlgorithmRegistry};
use crate::graph::properties::EnumSet;

/// Registers the topdownpacking algorithm and its layout options, mirroring
/// `TopdownpackingMetaDataProvider` (which also applies
/// `TopdownpackingOptions`).
pub fn register(options: &mut LayoutMetaDataRegistry, algorithms: &mut AlgorithmRegistry) {
    options::register_topdownpacking_options(options);

    algorithms.register(AlgorithmData {
        id: "org.eclipse.elk.topdownpacking",
        name: "ELK Top-down Packing",
        // no supportedFeatures.
        features: EnumSet::none(),
        create: || Box::new(provider::TopdownpackingLayoutProvider),
    });
}
