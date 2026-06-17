//! Hand-ported option constants for ELK Rectangle Packing, mirroring
//! `org.eclipse.elk.alg.rectpacking.options` (`RectPackingOptions`,
//! `RectPackingMetaDataProvider`, `InternalProperties`, `OptimizationGoal`)
//! plus the phase strategy enums.

use crate::core::data::{parse_enum, LayoutMetaDataRegistry, OptionData, OptionKind, Targets};
use crate::elk_enum;
use crate::graph::math::{ElkPadding, Spacing};
use crate::graph::properties::Property;

pub const ALGORITHM_ID: &str = "org.eclipse.elk.rectpacking";

elk_enum! {
    pub enum OptimizationGoal {
        ASPECT_RATIO_DRIVEN,
        MAX_SCALE_DRIVEN,
        AREA_DRIVEN,
    }
}

elk_enum! {
    pub enum WidthApproximationStrategy {
        GREEDY,
        TARGET_WIDTH,
    }
}

elk_enum! {
    pub enum PackingStrategy {
        COMPACTION,
        SIMPLE,
        NONE,
    }
}

elk_enum! {
    pub enum WhiteSpaceEliminationStrategy {
        EQUAL_BETWEEN_STRUCTURES,
        TO_ASPECT_RATIO,
        NONE,
    }
}

// ---------------------------------------------------------- RectPackingOptions
// Core option ids with the algorithm-specific defaults from RectPackingOptions.

pub static ASPECT_RATIO: Property<f64> =
    Property::with_default("org.eclipse.elk.aspectRatio", || 1.3);
pub static NODE_SIZE_FIXED_GRAPH_SIZE: Property<bool> =
    Property::with_default("org.eclipse.elk.nodeSize.fixedGraphSize", || false);
pub static PADDING: Property<ElkPadding> =
    Property::with_default("org.eclipse.elk.padding", || Spacing::uniform(15.0));
pub static SPACING_NODE_NODE: Property<f64> =
    Property::with_default("org.eclipse.elk.spacing.nodeNode", || 15.0);

// Options delegated to CoreOptions without a default override.
pub use crate::core::options::{INTERACTIVE, OMIT_NODE_MICRO_LAYOUT, PRIORITY};

// ----------------------------------------------- RectPackingMetaDataProvider

pub static TRYBOX: Property<bool> =
    Property::with_default("org.eclipse.elk.rectpacking.trybox", || false);
pub static CURRENT_POSITION: Property<i32> = Property::with_bounds(
    "org.eclipse.elk.rectpacking.currentPosition",
    || -1,
    Some(|| -1),
    None,
);
pub static DESIRED_POSITION: Property<i32> = Property::with_bounds(
    "org.eclipse.elk.rectpacking.desiredPosition",
    || -1,
    Some(|| -1),
    None,
);
pub static IN_NEW_ROW: Property<bool> =
    Property::with_default("org.eclipse.elk.rectpacking.inNewRow", || false);
pub static ORDER_BY_SIZE: Property<bool> =
    Property::with_default("org.eclipse.elk.rectpacking.orderBySize", || false);
pub static WIDTH_APPROXIMATION_STRATEGY: Property<WidthApproximationStrategy> =
    Property::with_default("org.eclipse.elk.rectpacking.widthApproximation.strategy", || {
        WidthApproximationStrategy::GREEDY
    });
pub static WIDTH_APPROXIMATION_TARGET_WIDTH: Property<f64> =
    Property::with_default("org.eclipse.elk.rectpacking.widthApproximation.targetWidth", || -1.0);
pub static WIDTH_APPROXIMATION_OPTIMIZATION_GOAL: Property<OptimizationGoal> =
    Property::with_default(
        "org.eclipse.elk.rectpacking.widthApproximation.optimizationGoal",
        || OptimizationGoal::MAX_SCALE_DRIVEN,
    );
pub static WIDTH_APPROXIMATION_LAST_PLACE_SHIFT: Property<bool> = Property::with_default(
    "org.eclipse.elk.rectpacking.widthApproximation.lastPlaceShift",
    || true,
);
pub static PACKING_STRATEGY: Property<PackingStrategy> =
    Property::with_default("org.eclipse.elk.rectpacking.packing.strategy", || {
        PackingStrategy::COMPACTION
    });
pub static PACKING_COMPACTION_ROW_HEIGHT_REEVALUATION: Property<bool> = Property::with_default(
    "org.eclipse.elk.rectpacking.packing.compaction.rowHeightReevaluation",
    || false,
);
pub static PACKING_COMPACTION_ITERATIONS: Property<i32> = Property::with_bounds(
    "org.eclipse.elk.rectpacking.packing.compaction.iterations",
    || 1,
    Some(|| 1),
    None,
);
pub static WHITE_SPACE_ELIMINATION_STRATEGY: Property<WhiteSpaceEliminationStrategy> =
    Property::with_default("org.eclipse.elk.rectpacking.whiteSpaceElimination.strategy", || {
        WhiteSpaceEliminationStrategy::NONE
    });

// -------------------------------------------------------- InternalProperties
// The ROWS property holds the row structure in a context struct passed between
// the phases (it is never serialized since its id is not registered with the
// metadata service).

pub static ADDITIONAL_HEIGHT: Property<f64> = Property::new("additionalHeight");
pub static DRAWING_HEIGHT: Property<f64> = Property::new("drawingHeight");
pub static DRAWING_WIDTH: Property<f64> = Property::new("drawingWidth");
pub static MIN_HEIGHT: Property<f64> = Property::new("minHeight");
pub static MIN_WIDTH: Property<f64> = Property::new("minWidth");
pub static TARGET_WIDTH: Property<f64> = Property::new("targetWidth");
pub static MIN_ROW_INCREASE: Property<f64> = Property::with_default("minRowIncrease", || 0.0);
pub static MAX_ROW_INCREASE: Property<f64> = Property::with_default("maxRowIncrease", || 0.0);
pub static MIN_ROW_DECREASE: Property<f64> = Property::with_default("minRowDecrease", || 0.0);
pub static MAX_ROW_DECREASE: Property<f64> = Property::with_default("maxRowDecrease", || 0.0);

// ------------------------------------------------------------------ metadata

/// Option metadata from `RectPackingMetaDataProvider.apply` (the core options
/// referenced by `RectPackingOptions.apply` are registered by elk-core).
pub fn register_rectpacking_options(reg: &mut LayoutMetaDataRegistry) {
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.trybox", group: "", kind: OptionKind::Bool, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.currentPosition", group: "", kind: OptionKind::Int, targets: Targets::NODES, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.desiredPosition", group: "", kind: OptionKind::Int, targets: Targets::NODES, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.inNewRow", group: "", kind: OptionKind::Bool, targets: Targets::NODES, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.orderBySize", group: "", kind: OptionKind::Bool, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.widthApproximation.strategy", group: "widthApproximation", kind: OptionKind::Enum(parse_enum::<WidthApproximationStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.widthApproximation.targetWidth", group: "widthApproximation", kind: OptionKind::Double, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.widthApproximation.optimizationGoal", group: "widthApproximation", kind: OptionKind::Enum(parse_enum::<OptimizationGoal>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.widthApproximation.lastPlaceShift", group: "widthApproximation", kind: OptionKind::Bool, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.packing.strategy", group: "packing", kind: OptionKind::Enum(parse_enum::<PackingStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.packing.compaction.rowHeightReevaluation", group: "packing.compaction", kind: OptionKind::Bool, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.packing.compaction.iterations", group: "packing.compaction", kind: OptionKind::Int, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.rectpacking.whiteSpaceElimination.strategy", group: "whiteSpaceElimination", kind: OptionKind::Enum(parse_enum::<WhiteSpaceEliminationStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
}
