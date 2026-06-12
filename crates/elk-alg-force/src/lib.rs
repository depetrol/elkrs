//! Rust port of `org.eclipse.elk.alg.force` — ELK's force-based layout
//! algorithms (`org.eclipse.elk.force`) and stress-minimizing layout
//! (`org.eclipse.elk.stress`).

pub mod components;
pub mod graph;
pub mod importer;
pub mod model;
pub mod options;
pub mod provider;
pub mod stress;

use elk_core::data::LayoutMetaDataRegistry;
use elk_core::registry::{AlgorithmData, AlgorithmRegistry, GraphFeature};
use elk_graph::properties::EnumSet;

/// Registers the force and stress algorithms and their layout options,
/// mirroring `ForceMetaDataProvider` and `StressMetaDataProvider`.
pub fn register(options: &mut LayoutMetaDataRegistry, algorithms: &mut AlgorithmRegistry) {
    options::register_force_options(options);
    options::register_stress_options(options);

    algorithms.register(AlgorithmData {
        id: "org.eclipse.elk.force",
        name: "ELK Force",
        features: EnumSet::of(&[GraphFeature::MULTI_EDGES, GraphFeature::EDGE_LABELS]),
        create: || Box::new(provider::ForceLayoutProvider),
    });
    algorithms.register(AlgorithmData {
        id: "org.eclipse.elk.stress",
        name: "ELK Stress",
        // StressOptions.java registers no supportedFeatures
        features: EnumSet::none(),
        create: || Box::new(stress::StressLayoutProvider),
    });
}
