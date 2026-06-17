//! Hand-ported option constants for ELK Force and ELK Stress, mirroring
//! `org.eclipse.elk.alg.force.options` (`ForceOptions`, `StressOptions`,
//! `ForceMetaDataProvider`, `StressMetaDataProvider`, `InternalProperties`,
//! `ForceModelStrategy` and `StressMajorization.Dimension`).

use crate::core::data::{parse_enum, LayoutMetaDataRegistry, OptionData, OptionKind, Targets};
use crate::elk_enum;
use crate::graph::math::{ElkPadding, KVector, Spacing};
use crate::graph::properties::Property;

elk_enum! {
    pub enum ForceModelStrategy {
        EADES,
        FRUCHTERMAN_REINGOLD,
    }
}

elk_enum! {
    pub enum Dimension {
        XY,
        X,
        Y,
    }
}

// ------------------------------------------------------------- ForceOptions
// Core option ids with the algorithm-specific defaults.

pub static PRIORITY: Property<i32> = Property::with_default("org.eclipse.elk.priority", || 1);
pub static SPACING_NODE_NODE: Property<f64> =
    Property::with_default("org.eclipse.elk.spacing.nodeNode", || 80.0);
pub static SPACING_EDGE_LABEL: Property<f64> =
    Property::with_default("org.eclipse.elk.spacing.edgeLabel", || 5.0);
pub static ASPECT_RATIO: Property<f64> =
    Property::with_default("org.eclipse.elk.aspectRatio", || 1.6f32 as f64);
pub static RANDOM_SEED: Property<i32> =
    Property::with_default("org.eclipse.elk.randomSeed", || 1);
pub static SEPARATE_CONNECTED_COMPONENTS: Property<bool> =
    Property::with_default("org.eclipse.elk.separateConnectedComponents", || true);
pub static PADDING: Property<ElkPadding> =
    Property::with_default("org.eclipse.elk.padding", || Spacing::uniform(50.0));
pub static EDGE_LABELS_INLINE: Property<bool> =
    Property::with_default("org.eclipse.elk.edgeLabels.inline", || false);

// Options delegated to CoreOptions without a default override.
pub use crate::core::options::{
    CHILD_AREA_HEIGHT, CHILD_AREA_WIDTH, INTERACTIVE, NODE_LABELS_PLACEMENT,
    NODE_SIZE_CONSTRAINTS, NODE_SIZE_FIXED_GRAPH_SIZE, OMIT_NODE_MICRO_LAYOUT,
};

// ----------------------------------------------------- ForceMetaDataProvider

pub static MODEL: Property<ForceModelStrategy> = Property::with_default(
    "org.eclipse.elk.force.model",
    || ForceModelStrategy::FRUCHTERMAN_REINGOLD,
);
pub static ITERATIONS: Property<i32> =
    Property::with_bounds("org.eclipse.elk.force.iterations", || 300, Some(|| 1), None);
pub static REPULSIVE_POWER: Property<i32> =
    Property::with_bounds("org.eclipse.elk.force.repulsivePower", || 0, Some(|| 0), None);
// Lower bound is ExclusiveBounds.greaterThan(0); bounds are only used
// by `checkProperties`, which the force importer has commented out.
pub static TEMPERATURE: Property<f64> =
    Property::with_default("org.eclipse.elk.force.temperature", || 0.001);
pub static REPULSION: Property<f64> =
    Property::with_default("org.eclipse.elk.force.repulsion", || 5.0);

// ---------------------------------------------------- StressMetaDataProvider

pub static FIXED: Property<bool> =
    Property::with_default("org.eclipse.elk.stress.fixed", || false);
pub static DESIRED_EDGE_LENGTH: Property<f64> =
    Property::with_default("org.eclipse.elk.stress.desiredEdgeLength", || 100.0);
pub static DIMENSION: Property<Dimension> =
    Property::with_default("org.eclipse.elk.stress.dimension", || Dimension::XY);
pub static EPSILON: Property<f64> =
    Property::with_default("org.eclipse.elk.stress.epsilon", || 10e-4);
pub static ITERATION_LIMIT: Property<i32> =
    Property::with_default("org.eclipse.elk.stress.iterationLimit", || i32::MAX);

// -------------------------------------------------------- InternalProperties
// ORIGIN and RANDOM are plain struct fields / function parameters in this
// port; only the bounding box corners live in the property map.

pub static BB_UPLEFT: Property<KVector> = Property::new("boundingBox.upLeft");
pub static BB_LOWRIGHT: Property<KVector> = Property::new("boundingBox.lowRight");

// --------------------------------------------------------------- metadata

/// Option metadata from `ForceMetaDataProvider.apply` (the core options it
/// references are registered by `crate::core::options_gen::register_core_options`).
pub fn register_force_options(reg: &mut LayoutMetaDataRegistry) {
    reg.register_option(OptionData { id: "org.eclipse.elk.force.model", group: "", kind: OptionKind::Enum(parse_enum::<ForceModelStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.force.iterations", group: "", kind: OptionKind::Int, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.force.repulsivePower", group: "", kind: OptionKind::Int, targets: Targets::EDGES, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.force.temperature", group: "", kind: OptionKind::Double, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.force.repulsion", group: "", kind: OptionKind::Double, targets: Targets::PARENTS, legacy_ids: &[] });
}

/// Option metadata from `StressMetaDataProvider.apply`.
pub fn register_stress_options(reg: &mut LayoutMetaDataRegistry) {
    reg.register_option(OptionData { id: "org.eclipse.elk.stress.fixed", group: "", kind: OptionKind::Bool, targets: Targets::NODES, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.stress.desiredEdgeLength", group: "", kind: OptionKind::Double, targets: Targets::PARENTS | Targets::EDGES, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.stress.dimension", group: "", kind: OptionKind::Enum(parse_enum::<Dimension>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.stress.epsilon", group: "", kind: OptionKind::Double, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.stress.iterationLimit", group: "", kind: OptionKind::Int, targets: Targets::PARENTS, legacy_ids: &[] });
}
