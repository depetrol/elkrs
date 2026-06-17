//! Hand-ported option constants for ELK Radial, mirroring
//! `org.eclipse.elk.alg.radial.options` (`RadialOptions`,
//! `RadialMetaDataProvider` and the strategy enums).

use crate::core::data::{parse_enum, LayoutMetaDataRegistry, OptionData, OptionKind, Targets};
use crate::elk_enum;
use crate::graph::properties::Property;

elk_enum! {
    pub enum SortingStrategy {
        NONE,
        POLAR_COORDINATE,
        ID,
    }
}

elk_enum! {
    pub enum AnnulusWedgeCriteria {
        LEAF_NUMBER,
        NODE_SIZE,
    }
}

elk_enum! {
    pub enum CompactionStrategy {
        NONE,
        RADIAL_COMPACTION,
        WEDGE_COMPACTION,
    }
}

elk_enum! {
    pub enum RadialTranslationStrategy {
        NONE,
        EDGE_LENGTH,
        EDGE_LENGTH_BY_POSITION,
        CROSSING_MINIMIZATION_BY_POSITION,
    }
}

elk_enum! {
    /// Not a registered layout option; kept for completeness.
    pub enum OverlapRemovalStrategy {
        EXTENT_RADII,
    }
}

// ---------------------------------------------------- RadialMetaDataProvider

pub static CENTER_ON_ROOT: Property<bool> =
    Property::with_default("org.eclipse.elk.radial.centerOnRoot", || false);
pub static ORDER_ID: Property<i32> =
    Property::with_default("org.eclipse.elk.radial.orderId", || 0);
pub static RADIUS: Property<f64> =
    Property::with_default("org.eclipse.elk.radial.radius", || 0.0);
pub static ROTATE: Property<bool> =
    Property::with_default("org.eclipse.elk.radial.rotate", || false);
pub static COMPACTOR: Property<CompactionStrategy> =
    Property::with_default("org.eclipse.elk.radial.compactor", || CompactionStrategy::NONE);
pub static COMPACTION_STEP_SIZE: Property<i32> = Property::with_bounds(
    "org.eclipse.elk.radial.compactionStepSize",
    || 1,
    Some(|| 0),
    None,
);
pub static SORTER: Property<SortingStrategy> =
    Property::with_default("org.eclipse.elk.radial.sorter", || SortingStrategy::NONE);
pub static WEDGE_CRITERIA: Property<AnnulusWedgeCriteria> = Property::with_default(
    "org.eclipse.elk.radial.wedgeCriteria",
    || AnnulusWedgeCriteria::NODE_SIZE,
);
pub static OPTIMIZATION_CRITERIA: Property<RadialTranslationStrategy> = Property::with_default(
    "org.eclipse.elk.radial.optimizationCriteria",
    || RadialTranslationStrategy::NONE,
);
pub static ROTATION_TARGET_ANGLE: Property<f64> =
    Property::with_default("org.eclipse.elk.radial.rotation.targetAngle", || 0.0);
pub static ROTATION_COMPUTE_ADDITIONAL_WEDGE_SPACE: Property<bool> = Property::with_default(
    "org.eclipse.elk.radial.rotation.computeAdditionalWedgeSpace",
    || false,
);
pub static ROTATION_OUTGOING_EDGE_ANGLES: Property<bool> = Property::with_default(
    "org.eclipse.elk.radial.rotation.outgoingEdgeAngles",
    || false,
);

// Core options used via RadialOptions without a default override.
pub use crate::core::options::{
    CHILD_AREA_HEIGHT, CHILD_AREA_WIDTH, MARGINS, NODE_SIZE_FIXED_GRAPH_SIZE,
    OMIT_NODE_MICRO_LAYOUT, PADDING, POSITION, SPACING_NODE_NODE,
};

// ------------------------------------------------------------------ metadata

/// Option metadata from `RadialMetaDataProvider.apply` (the core options the
/// algorithm supports are registered by `register_core_options`).
pub fn register_radial_options(reg: &mut LayoutMetaDataRegistry) {
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.centerOnRoot", group: "", kind: OptionKind::Bool, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.orderId", group: "", kind: OptionKind::Int, targets: Targets::NODES, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.radius", group: "", kind: OptionKind::Double, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.rotate", group: "", kind: OptionKind::Bool, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.compactor", group: "", kind: OptionKind::Enum(parse_enum::<CompactionStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.compactionStepSize", group: "", kind: OptionKind::Int, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.sorter", group: "", kind: OptionKind::Enum(parse_enum::<SortingStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.wedgeCriteria", group: "", kind: OptionKind::Enum(parse_enum::<AnnulusWedgeCriteria>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.optimizationCriteria", group: "", kind: OptionKind::Enum(parse_enum::<RadialTranslationStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.rotation.targetAngle", group: "rotation", kind: OptionKind::Double, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.rotation.computeAdditionalWedgeSpace", group: "rotation", kind: OptionKind::Bool, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.radial.rotation.outgoingEdgeAngles", group: "rotation", kind: OptionKind::Bool, targets: Targets::PARENTS, legacy_ids: &[] });
}
