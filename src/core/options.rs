//! Layout options: re-exports the generated core options plus manual pieces
//! (per-algorithm default overrides and the resolved-algorithm marker).

pub use crate::core::options_gen::*;

use crate::graph::math::{KVector, Spacing};
use crate::graph::properties::{JavaString, Property};

/// Value stored under `org.eclipse.elk.resolvedAlgorithm`
/// (only the id matters for layout).
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedAlgorithm(pub String);

impl JavaString for ResolvedAlgorithm {
    fn java_string(&self) -> String {
        format!("Layout Algorithm: {}", self.0)
    }
}

pub static RESOLVED_ALGORITHM_TYPED: Property<ResolvedAlgorithm> =
    Property::new("org.eclipse.elk.resolvedAlgorithm");

/// `FixedLayouterOptions`: core ids with algorithm-specific defaults.
pub mod fixed {
    use super::*;

    pub static PADDING: Property<Spacing> =
        Property::with_default("org.eclipse.elk.padding", || Spacing::uniform(15.0));
    pub static POSITION: Property<KVector> = Property::new("org.eclipse.elk.position");
}

/// `BoxLayouterOptions`.
pub mod boxl {
    use super::*;

    pub static PADDING: Property<Spacing> =
        Property::with_default("org.eclipse.elk.padding", || Spacing::uniform(15.0));
    pub static SPACING_NODE_NODE: Property<f64> =
        Property::with_default("org.eclipse.elk.spacing.nodeNode", || 15.0);
    pub static PRIORITY: Property<i32> =
        Property::with_default("org.eclipse.elk.priority", || 0);
    pub static ASPECT_RATIO: Property<f64> =
        Property::with_default("org.eclipse.elk.aspectRatio", || 1.3f32 as f64);
}

/// `RandomLayouterOptions`.
pub mod random {
    use super::*;

    pub static PADDING: Property<Spacing> =
        Property::with_default("org.eclipse.elk.padding", || Spacing::uniform(15.0));
    pub static SPACING_NODE_NODE: Property<f64> =
        Property::with_default("org.eclipse.elk.spacing.nodeNode", || 15.0);
    pub static RANDOM_SEED: Property<i32> =
        Property::with_default("org.eclipse.elk.randomSeed", || 0);
    pub static ASPECT_RATIO: Property<f64> =
        Property::with_default("org.eclipse.elk.aspectRatio", || 1.6f32 as f64);
}
