//! Core layout providers (`org.eclipse.elk.fixed`, `.box`, `.random`).

pub mod box_layouter;
pub mod fixed;
pub mod random;

use crate::graph::properties::EnumSet;

use crate::core::registry::{AlgorithmData, AlgorithmRegistry};

/// Registers the algorithms shipped with `org.eclipse.elk.core`.
pub fn register_core_algorithms(reg: &mut AlgorithmRegistry) {
    reg.register(AlgorithmData {
        id: "org.eclipse.elk.fixed",
        name: "ELK Fixed",
        features: EnumSet::none(),
        create: || Box::new(fixed::FixedLayoutProvider),
    });
    reg.register(AlgorithmData {
        id: "org.eclipse.elk.box",
        name: "ELK Box",
        features: EnumSet::none(),
        create: || Box::new(box_layouter::BoxLayoutProvider),
    });
    reg.register(AlgorithmData {
        id: "org.eclipse.elk.random",
        name: "Randomizer",
        features: EnumSet::none(),
        create: || Box::new(random::RandomLayoutProvider),
    });
}
