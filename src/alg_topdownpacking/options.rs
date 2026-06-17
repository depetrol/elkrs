//! Hand-ported option constants for ELK Top-down Packing, mirroring
//! `org.eclipse.elk.alg.topdownpacking.options` (`TopdownpackingOptions`,
//! `TopdownpackingMetaDataProvider`) plus the phase strategy enums.

use crate::core::data::{parse_enum, LayoutMetaDataRegistry, OptionData, OptionKind, Targets};
use crate::elk_enum;
use crate::graph::properties::Property;

pub const ALGORITHM_ID: &str = "org.eclipse.elk.topdownpacking";

elk_enum! {
    pub enum NodeArrangementStrategy {
        LEFT_RIGHT_TOP_DOWN_NODE_PLACER,
    }
}

elk_enum! {
    pub enum WhitespaceEliminationStrategy {
        BOTTOM_ROW_EQUAL_WHITESPACE_ELIMINATOR,
    }
}

// ------------------------------------------------------ TopdownpackingOptions
// All core options are supported without a default override (the algorithm
// also supports `topdownLayout` and declares the algorithm-specific default
// `topdown.nodeType = PARALLEL_NODE`; both are only read by the engine's
// topdown layout mode, which elk-core does not implement yet).

pub use crate::core::options::{
    PADDING, SPACING_NODE_NODE, TOPDOWN_HIERARCHICAL_NODE_ASPECT_RATIO,
    TOPDOWN_HIERARCHICAL_NODE_WIDTH,
};

// -------------------------------------------- TopdownpackingMetaDataProvider

pub static NODE_ARRANGEMENT_STRATEGY: Property<NodeArrangementStrategy> = Property::with_default(
    "org.eclipse.elk.topdownpacking.nodeArrangement.strategy",
    || NodeArrangementStrategy::LEFT_RIGHT_TOP_DOWN_NODE_PLACER,
);
pub static WHITESPACE_ELIMINATION_STRATEGY: Property<WhitespaceEliminationStrategy> =
    Property::with_default(
        "org.eclipse.elk.topdownpacking.whitespaceElimination.strategy",
        || WhitespaceEliminationStrategy::BOTTOM_ROW_EQUAL_WHITESPACE_ELIMINATOR,
    );

// ------------------------------------------------------------------ metadata

/// Option metadata from `TopdownpackingMetaDataProvider.apply` (the core
/// options referenced by `TopdownpackingOptions.apply` are registered by
/// elk-core).
pub fn register_topdownpacking_options(reg: &mut LayoutMetaDataRegistry) {
    reg.register_option(OptionData { id: "org.eclipse.elk.topdownpacking.nodeArrangement.strategy", group: "nodeArrangement", kind: OptionKind::Enum(parse_enum::<NodeArrangementStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.topdownpacking.whitespaceElimination.strategy", group: "whitespaceElimination", kind: OptionKind::Enum(parse_enum::<WhitespaceEliminationStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
}
