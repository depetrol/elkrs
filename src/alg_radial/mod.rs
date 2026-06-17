//! Rust port of `org.eclipse.elk.alg.radial` — ELK's radial tree layout
//! algorithm (`org.eclipse.elk.radial`).

pub mod compaction;
pub mod intermediate;
pub mod optimization;
pub mod options;
pub mod overlaps;
pub mod p1position;
pub mod p2routing;
pub mod provider;
pub mod rotation;
pub mod sorting;
pub mod util;

use crate::core::data::LayoutMetaDataRegistry;
use crate::core::registry::{AlgorithmData, AlgorithmRegistry};
use crate::graph::properties::EnumSet;

/// Registers the radial algorithm and its layout options, mirroring
/// `RadialMetaDataProvider` and `RadialOptions`.
pub fn register(options: &mut LayoutMetaDataRegistry, algorithms: &mut AlgorithmRegistry) {
    options::register_radial_options(options);

    algorithms.register(AlgorithmData {
        id: "org.eclipse.elk.radial",
        name: "ELK Radial",
        // no supportedFeatures
        features: EnumSet::none(),
        create: || Box::new(provider::RadialLayoutProvider),
    });
}
