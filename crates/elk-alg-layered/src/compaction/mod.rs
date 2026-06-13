//! Port of `org.eclipse.elk.alg.layered.intermediate.compaction`: the LGraph
//! side of horizontal post-compaction. Transforms an already-routed [`LGraph`]
//! into a constraint graph ([`elk_alg_common::compaction::CGraph`]), runs the
//! one-dimensional compactor, and transfers the compacted positions back.
//!
//! [`LGraph`]: crate::graph::LGraph

pub mod edge_aware_scanline;
pub mod network_simplex_compaction;
pub mod transformer;
pub mod vertical_segment;

pub use transformer::LGraphToCGraphTransformer;
pub use vertical_segment::VerticalSegment;

/// Re-export of the common compaction tolerance helpers under the name used by
/// the Java `org.eclipse.elk.alg.common.compaction.oned.CompareFuzzy`.
pub use elk_alg_common::compaction::compare_fuzzy;
