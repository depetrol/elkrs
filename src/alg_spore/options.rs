//! Hand-ported option constants for ELK SPOrE, mirroring
//! `org.eclipse.elk.alg.spore.options` (`SporeMetaDataProvider`,
//! `SporeCompactionOptions`, `SporeOverlapRemovalOptions`).

use crate::core::data::{parse_enum, LayoutMetaDataRegistry, OptionData, OptionKind, Targets};
use crate::elk_enum;
use crate::graph::math::{ElkPadding, Spacing};
use crate::graph::properties::Property;

elk_enum! {
    pub enum StructureExtractionStrategy {
        DELAUNAY_TRIANGULATION,
    }
}

elk_enum! {
    pub enum TreeConstructionStrategy {
        MINIMUM_SPANNING_TREE,
        MAXIMUM_SPANNING_TREE,
    }
}

elk_enum! {
    pub enum SpanningTreeCostFunction {
        CENTER_DISTANCE,
        CIRCLE_UNDERLAP,
        RECTANGLE_UNDERLAP,
        INVERTED_OVERLAP,
        MINIMUM_ROOT_DISTANCE,
    }
}

elk_enum! {
    pub enum RootSelection {
        FIXED,
        CENTER_NODE,
    }
}

elk_enum! {
    pub enum CompactionStrategy {
        DEPTH_FIRST,
    }
}

// ----------------------------------------------------- SporeMetaDataProvider

pub static UNDERLYING_LAYOUT_ALGORITHM: Property<String> =
    Property::new("org.eclipse.elk.underlyingLayoutAlgorithm");

pub static STRUCTURE_STRUCTURE_EXTRACTION_STRATEGY: Property<StructureExtractionStrategy> =
    Property::with_default("org.eclipse.elk.structure.structureExtractionStrategy", || {
        StructureExtractionStrategy::DELAUNAY_TRIANGULATION
    });

pub static PROCESSING_ORDER_TREE_CONSTRUCTION: Property<TreeConstructionStrategy> =
    Property::with_default("org.eclipse.elk.processingOrder.treeConstruction", || {
        TreeConstructionStrategy::MINIMUM_SPANNING_TREE
    });

pub static PROCESSING_ORDER_SPANNING_TREE_COST_FUNCTION: Property<SpanningTreeCostFunction> =
    Property::with_default("org.eclipse.elk.processingOrder.spanningTreeCostFunction", || {
        SpanningTreeCostFunction::CIRCLE_UNDERLAP
    });

pub static PROCESSING_ORDER_PREFERRED_ROOT: Property<String> =
    Property::new("org.eclipse.elk.processingOrder.preferredRoot");

pub static PROCESSING_ORDER_ROOT_SELECTION: Property<RootSelection> =
    Property::with_default("org.eclipse.elk.processingOrder.rootSelection", || {
        RootSelection::CENTER_NODE
    });

pub static COMPACTION_COMPACTION_STRATEGY: Property<CompactionStrategy> =
    Property::with_default("org.eclipse.elk.compaction.compactionStrategy", || {
        CompactionStrategy::DEPTH_FIRST
    });

pub static COMPACTION_ORTHOGONAL: Property<bool> =
    Property::with_default("org.eclipse.elk.compaction.orthogonal", || false);

pub static OVERLAP_REMOVAL_MAX_ITERATIONS: Property<i32> =
    Property::with_default("org.eclipse.elk.overlapRemoval.maxIterations", || 64);

pub static OVERLAP_REMOVAL_RUN_SCANLINE: Property<bool> =
    Property::with_default("org.eclipse.elk.overlapRemoval.runScanline", || true);

// -------------------------- algorithm-specific defaults of core options
// (identical for SporeCompactionOptions and SporeOverlapRemovalOptions)

pub static PADDING: Property<ElkPadding> =
    Property::with_default("org.eclipse.elk.padding", || Spacing::uniform(8.0));

pub static SPACING_NODE_NODE: Property<f64> =
    Property::with_default("org.eclipse.elk.spacing.nodeNode", || 8.0);

pub use crate::core::options::DEBUG_MODE;

// --------------------------------------------------------------- metadata

/// Option metadata from `SporeMetaDataProvider.apply`.
pub fn register_spore_options(reg: &mut LayoutMetaDataRegistry) {
    reg.register_option(OptionData { id: "org.eclipse.elk.underlyingLayoutAlgorithm", group: "", kind: OptionKind::Str, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.structure.structureExtractionStrategy", group: "structure", kind: OptionKind::Enum(parse_enum::<StructureExtractionStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.processingOrder.treeConstruction", group: "processingOrder", kind: OptionKind::Enum(parse_enum::<TreeConstructionStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.processingOrder.spanningTreeCostFunction", group: "processingOrder", kind: OptionKind::Enum(parse_enum::<SpanningTreeCostFunction>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.processingOrder.preferredRoot", group: "processingOrder", kind: OptionKind::Str, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.processingOrder.rootSelection", group: "processingOrder", kind: OptionKind::Enum(parse_enum::<RootSelection>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.compaction.compactionStrategy", group: "compaction", kind: OptionKind::Enum(parse_enum::<CompactionStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.compaction.orthogonal", group: "compaction", kind: OptionKind::Bool, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.overlapRemoval.maxIterations", group: "overlapRemoval", kind: OptionKind::Int, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.overlapRemoval.runScanline", group: "overlapRemoval", kind: OptionKind::Bool, targets: Targets::PARENTS, legacy_ids: &[] });
}
