//! Hand-ported option constants for ELK DisCo, mirroring
//! `org.eclipse.elk.alg.disco.options` (`DisCoMetaDataProvider`,
//! `DisCoOptions`) plus the pieces of
//! `org.eclipse.elk.alg.common.compaction.options.PolyominoOptions` that
//! DisCo consults.
//!
//! Note: the Java oracle never registers `PolyominoOptions` with its
//! `LayoutMetaDataService` (only `DisCoMetaDataProvider`), so the four
//! `org.eclipse.elk.polyomino.*` options are unknown to it and silently
//! dropped from inputs/outputs; this port mirrors that by not registering
//! them either.

use elk_core::data::{parse_enum, LayoutMetaDataRegistry, OptionData, OptionKind, Targets};
use elk_graph::elk_enum;
use elk_graph::math::{ElkPadding, Spacing};
use elk_graph::properties::Property;

elk_enum! {
    pub enum CompactionStrategy {
        POLYOMINO,
    }
}

// ----------------------------------------------------- DisCoMetaDataProvider

pub static COMPONENT_COMPACTION_STRATEGY: Property<CompactionStrategy> = Property::with_default(
    "org.eclipse.elk.disco.componentCompaction.strategy",
    || CompactionStrategy::POLYOMINO,
);

pub static COMPONENT_COMPACTION_COMPONENT_LAYOUT_ALGORITHM: Property<String> =
    Property::new("org.eclipse.elk.disco.componentCompaction.componentLayoutAlgorithm");

/// Java holds the `DCGraph` object here (printed as `DCGraph@<identityhash>`
/// by the JSON exporter); this port stores a string stand-in.
pub static DEBUG_DISCO_GRAPH: Property<String> =
    Property::new("org.eclipse.elk.disco.debug.discoGraph");

/// Java holds the `List<DCPolyomino>` here; this port stores its exact
/// `toString()` rendition.
pub static DEBUG_DISCO_POLYS: Property<String> =
    Property::new("org.eclipse.elk.disco.debug.discoPolys");

// --------------------------------------------- core options used by DisCo

pub use elk_core::options::{EDGE_THICKNESS, SPACING_COMPONENT_COMPONENT};

/// `CoreOptions.ASPECT_RATIO` (no default; null in Java).
pub static ASPECT_RATIO: Property<f64> = Property::new("org.eclipse.elk.aspectRatio");

/// `CoreOptions.PADDING` (default `new ElkPadding(12)`).
pub static PADDING: Property<ElkPadding> =
    Property::with_default("org.eclipse.elk.padding", || Spacing::uniform(12.0));

// ------------------------------------------------- PolyominoOptions pieces

pub static POLYOMINO_FILL: Property<bool> =
    Property::with_default("org.eclipse.elk.polyomino.fill", || true);

// --------------------------------------------------------------- metadata

/// Option metadata from `DisCoMetaDataProvider.apply`.
pub fn register_disco_options(reg: &mut LayoutMetaDataRegistry) {
    reg.register_option(OptionData { id: "org.eclipse.elk.disco.componentCompaction.strategy", group: "componentCompaction", kind: OptionKind::Enum(parse_enum::<CompactionStrategy>), targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.disco.componentCompaction.componentLayoutAlgorithm", group: "componentCompaction", kind: OptionKind::Str, targets: Targets::PARENTS, legacy_ids: &[] });
    // Java registers these as Type.OBJECT (hidden, debug only); they never
    // appear in inputs, but registering them keeps the JSON exporter from
    // dropping them.
    reg.register_option(OptionData { id: "org.eclipse.elk.disco.debug.discoGraph", group: "debug", kind: OptionKind::Str, targets: Targets::PARENTS, legacy_ids: &[] });
    reg.register_option(OptionData { id: "org.eclipse.elk.disco.debug.discoPolys", group: "debug", kind: OptionKind::Str, targets: Targets::PARENTS, legacy_ids: &[] });
}
