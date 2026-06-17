//! Hand-ported option constants for ELK Mr. Tree, mirroring
//! `org.eclipse.elk.alg.mrtree.options` (`MrTreeOptions`,
//! `MrTreeMetaDataProvider` and the enums `OrderWeighting`,
//! `TreeifyingOrder`, `EdgeRoutingMode`, `CompactionMode`).
//!
//! The members of `InternalProperties` are plain struct fields on the
//! TGraph model (see `graph.rs`); none of their ids collide with
//! registered layout options, so they are never visible in JSON output.

use crate::core::data::{parse_enum, LayoutMetaDataRegistry, OptionData, OptionKind, Targets};
use crate::elk_enum;
use crate::graph::math::{ElkPadding, Spacing};
use crate::graph::properties::Property;

elk_enum! {
    pub enum OrderWeighting {
        MODEL_ORDER,
        DESCENDANTS,
        FAN,
        CONSTRAINT,
    }
}

elk_enum! {
    pub enum TreeifyingOrder {
        DFS,
        BFS,
    }
}

elk_enum! {
    pub enum EdgeRoutingMode {
        NONE,
        MIDDLE_TO_MIDDLE,
        AVOID_OVERLAP,
    }
}

elk_enum! {
    /// (Declared but not referenced by the algorithm; the
    /// `compaction` option is a plain boolean.)
    pub enum CompactionMode {
        NONE,
        LEVEL_PRESERVING,
        AGGRESSIVE,
    }
}

// -------------------------------------------------------------- MrTreeOptions
// Core option ids with the algorithm-specific defaults.

pub static PADDING: Property<ElkPadding> =
    Property::with_default("org.eclipse.elk.padding", || Spacing::uniform(20.0));
pub static SPACING_NODE_NODE: Property<f64> =
    Property::with_default("org.eclipse.elk.spacing.nodeNode", || 20.0);
pub static SPACING_EDGE_NODE: Property<f64> =
    Property::with_default("org.eclipse.elk.spacing.edgeNode", || 3.0);
pub static ASPECT_RATIO: Property<f64> =
    Property::with_default("org.eclipse.elk.aspectRatio", || 1.6f32 as f64);
pub static PRIORITY: Property<i32> = Property::with_default("org.eclipse.elk.priority", || 1);
pub static SEPARATE_CONNECTED_COMPONENTS: Property<bool> =
    Property::with_default("org.eclipse.elk.separateConnectedComponents", || true);

// Options delegated to CoreOptions without a default override.
pub use crate::core::options::{
    CHILD_AREA_HEIGHT, CHILD_AREA_WIDTH, DEBUG_MODE, DIRECTION, INTERACTIVE,
    NODE_SIZE_FIXED_GRAPH_SIZE, OMIT_NODE_MICRO_LAYOUT,
};

// ----------------------------------------------------- MrTreeMetaDataProvider

/// Turns on tree compaction.
pub static COMPACTION: Property<bool> =
    Property::with_default("org.eclipse.elk.mrtree.compaction", || false);
/// Length of the texture at the end of an edge.
pub static EDGE_END_TEXTURE_LENGTH: Property<f64> =
    Property::with_default("org.eclipse.elk.mrtree.edgeEndTextureLength", || 7.0);
/// The index of the tree level the node is in.
pub static TREE_LEVEL: Property<i32> =
    Property::with_bounds("org.eclipse.elk.mrtree.treeLevel", || 0, Some(|| 0), None);
/// Position constraint for `OrderWeighting::CONSTRAINT`.
pub static POSITION_CONSTRAINT: Property<i32> =
    Property::with_default("org.eclipse.elk.mrtree.positionConstraint", || -1);
/// Which weighting to use when computing a node order.
pub static WEIGHTING: Property<OrderWeighting> =
    Property::with_default("org.eclipse.elk.mrtree.weighting", || OrderWeighting::MODEL_ORDER);
/// Chooses an edge routing algorithm.
pub static EDGE_ROUTING_MODE: Property<EdgeRoutingMode> = Property::with_default(
    "org.eclipse.elk.mrtree.edgeRoutingMode",
    || EdgeRoutingMode::AVOID_OVERLAP,
);
/// Which search order to use when computing a spanning tree.
pub static SEARCH_ORDER: Property<TreeifyingOrder> =
    Property::with_default("org.eclipse.elk.mrtree.searchOrder", || TreeifyingOrder::DFS);

// ------------------------------------------------------------------ metadata

/// Option metadata from `MrTreeMetaDataProvider.apply` (the core options it
/// references are registered by `crate::core::options_gen::register_core_options`).
pub fn register_mrtree_options(reg: &mut LayoutMetaDataRegistry) {
    reg.register_option(OptionData { id: "org.eclipse.elk.mrtree.compaction", group: "", kind: OptionKind::Bool, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.mrtree.edgeEndTextureLength", group: "", kind: OptionKind::Double, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.mrtree.treeLevel", group: "", kind: OptionKind::Int, targets: Targets::NODES, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.mrtree.positionConstraint", group: "", kind: OptionKind::Int, targets: Targets::NODES, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.mrtree.weighting", group: "", kind: OptionKind::Enum(parse_enum::<OrderWeighting>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.mrtree.edgeRoutingMode", group: "", kind: OptionKind::Enum(parse_enum::<EdgeRoutingMode>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.mrtree.searchOrder", group: "", kind: OptionKind::Enum(parse_enum::<TreeifyingOrder>), targets: Targets::PARENTS, legacy_ids: &[] });
}
