//! Rust port of `org.eclipse.elk.alg.mrtree` — ELK's tree layout algorithm
//! ("ELK Mr. Tree", `org.eclipse.elk.mrtree`).

pub mod components;
pub mod graph;
pub mod importer;
pub mod intermediate;
pub mod options;
pub mod p1treeify;
pub mod p2order;
pub mod p3place;
pub mod p4route;
pub mod provider;
pub mod tree_util;

use elk_core::data::LayoutMetaDataRegistry;
use elk_core::registry::{AlgorithmData, AlgorithmRegistry, GraphFeature};
use elk_graph::properties::EnumSet;

/// Registers the mrtree algorithm and its layout options, mirroring
/// `MrTreeMetaDataProvider` and `MrTreeOptions`.
pub fn register(options: &mut LayoutMetaDataRegistry, algorithms: &mut AlgorithmRegistry) {
    options::register_mrtree_options(options);

    algorithms.register(AlgorithmData {
        id: "org.eclipse.elk.mrtree",
        name: "ELK Mr. Tree",
        // supportedFeatures: EnumSet.of(GraphFeature.DISCONNECTED)
        features: EnumSet::of(&[GraphFeature::DISCONNECTED]),
        create: || Box::new(provider::TreeLayoutProvider),
    });
}
