//!
//! Element references are stored as arena ids. Properties whose type is
//! a mutable shared object require read-modify-write at the call sites.

use crate::core::adapters::LabelSide;
use crate::core::options::PortSide;
use crate::graph::math::{KVector, KVectorChain};
use crate::graph::properties::{EnumSet, JavaCloneable, JavaString, Property};

use crate::alg_layered::graph::{LEdgeId, LGraphId, LLabelId, LNodeId, LPortId};
use crate::alg_layered::options_gen::{EdgeConstraint, GraphProperties, InLayerConstraint};
use crate::alg_layered::processors::end_label_preprocessor::EndLabelCells;

/// Reference back to the original `ElkGraph` element
/// (`InternalProperties.ORIGIN`, typed `Object`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    Node(crate::graph::graph::NodeId),
    Port(crate::graph::graph::PortId),
    Edge(crate::graph::graph::EdgeId),
    Label(crate::graph::graph::LabelId),
    /// internal: a dummy node's originating LEdge
    LEdge(LEdgeId),
    /// internal: a dummy node's originating LGraph element set
    LNode(LNodeId),
    /// internal: a dummy node's / dummy port's originating LPort (e.g. for
    /// north/south port dummies and external port dummies)
    LPort(LPortId),
}

macro_rules! internal_value {
    ($T:ty) => {
        impl JavaString for $T {
            fn java_string(&self) -> String {
                format!("{:?}", self)
            }
        }
        impl JavaCloneable for $T {
            const CLONEABLE: bool = false;
        }
    };
}

internal_value!(Origin);
internal_value!(LNodeId);
internal_value!(LPortId);
internal_value!(LEdgeId);
internal_value!(LGraphId);
internal_value!(LLabelId);
internal_value!(EndLabelCells);

pub static ORIGIN: Property<Origin> = Property::new("origin");
pub static COORDINATE_SYSTEM_ORIGIN: Property<LGraphId> = Property::new("coordinateOrigin");
pub static COMPOUND_NODE: Property<bool> = Property::with_default("compoundNode", || false);
pub static INSIDE_CONNECTIONS: Property<bool> =
    Property::with_default("insideConnections", || false);
pub static ORIGINAL_BENDPOINTS: Property<KVectorChain> = Property::new("originalBendpoints");
pub static ORIGINAL_DUMMY_NODE_POSITION: Property<f64> =
    Property::new("originalDummyNodePosition");
pub static ORIGINAL_LABEL_EDGE: Property<LEdgeId> = Property::new("originalLabelEdge");
pub static MAX_EDGE_THICKNESS: Property<f64> = Property::with_default("maxEdgeThickness", || 0.0);
pub static REVERSED: Property<bool> = Property::with_default("reversed", || false);
pub static LONG_EDGE_SOURCE: Property<LPortId> = Property::new("longEdgeSource");
pub static LONG_EDGE_TARGET: Property<LPortId> = Property::new("longEdgeTarget");
pub static LONG_EDGE_HAS_LABEL_DUMMIES: Property<bool> =
    Property::with_default("longEdgeHasLabelDummies", || false);
pub static LONG_EDGE_BEFORE_LABEL_DUMMY: Property<bool> =
    Property::with_default("longEdgeBeforeLabelDummy", || false);
pub static EDGE_CONSTRAINT: Property<EdgeConstraint> =
    Property::with_default("edgeConstraint", || EdgeConstraint::NONE);
pub static IN_LAYER_LAYOUT_UNIT: Property<LNodeId> = Property::new("inLayerLayoutUnit");
pub static IN_LAYER_CONSTRAINT: Property<InLayerConstraint> =
    Property::with_default("inLayerConstraint", || InLayerConstraint::NONE);
pub static IN_LAYER_SUCCESSOR_CONSTRAINTS: Property<Vec<LNodeId>> =
    Property::with_default("inLayerSuccessorConstraint", Vec::new);
pub static IN_LAYER_SUCCESSOR_CONSTRAINTS_BETWEEN_NON_DUMMIES: Property<bool> =
    Property::with_default("inLayerSuccessorConstraintBetweenNonDummies", || false);
pub static PORT_DUMMY: Property<LNodeId> = Property::new("portDummy");
pub static CROSSING_HINT: Property<i32> = Property::with_default("crossingHint", || 0);
pub static GRAPH_PROPERTIES: Property<EnumSet<GraphProperties>> =
    Property::with_default("graphProperties", EnumSet::none);
pub static EXT_PORT_SIDE: Property<PortSide> =
    Property::with_default("externalPortSide", || PortSide::UNDEFINED);
pub static EXT_PORT_SIZE: Property<KVector> =
    Property::with_default("externalPortSize", KVector::default);
pub static EXT_PORT_REPLACED_DUMMIES: Property<Vec<LNodeId>> =
    Property::new("externalPortReplacedDummies");
pub static EXT_PORT_REPLACED_DUMMY: Property<LNodeId> =
    Property::new("externalPortReplacedDummy");
pub static EXT_PORT_CONNECTIONS: Property<EnumSet<PortSide>> =
    Property::with_default("externalPortConnections", EnumSet::none);
pub static PORT_RATIO_OR_POSITION: Property<f64> =
    Property::with_default("portRatioOrPosition", || 0.0);
pub static BARYCENTER_ASSOCIATES: Property<Vec<LNodeId>> =
    Property::new("barycenterAssociates");
pub static TOP_COMMENTS: Property<Vec<LNodeId>> = Property::new("TopSideComments");
pub static BOTTOM_COMMENTS: Property<Vec<LNodeId>> = Property::new("BottomSideComments");
pub static COMMENT_CONN_PORT: Property<LPortId> = Property::new("CommentConnectionPort");
pub static INPUT_COLLECT: Property<bool> = Property::with_default("inputCollect", || false);
pub static OUTPUT_COLLECT: Property<bool> = Property::with_default("outputCollect", || false);
pub static CYCLIC: Property<bool> = Property::with_default("cyclic", || false);
pub static TARGET_OFFSET: Property<KVector> = Property::new("targetOffset");
pub static PARTITION_DUMMY: Property<bool> =
    Property::with_default("partitionConstraint", || false);
pub static MODEL_ORDER: Property<i32> = Property::new("modelOrder");
pub static MAX_MODEL_ORDER_NODES: Property<i32> = Property::new("modelOrder.maximum");
pub static CB_NUM_MODEL_ORDER_GROUPS: Property<i32> =
    Property::new("modelOrderGroups.cb.number");
pub static LONG_EDGE_TARGET_NODE: Property<LNodeId> = Property::new("longEdgeTargetNode");
pub static FIRST_TRY_WITH_INITIAL_ORDER: Property<bool> =
    Property::with_default("firstTryWithInitialOrder", || false);
pub static SECOND_TRY_WITH_INITIAL_ORDER: Property<bool> =
    Property::with_default("firstTryWithInitialOrder", || false);
pub static TARJAN_LOWLINK: Property<i32> =
    Property::with_default("tarjan.lowlink", || i32::MAX);
pub static TARJAN_ID: Property<i32> = Property::with_default("tarjan.id", || -1);
pub static TARJAN_ON_STACK: Property<bool> = Property::with_default("tarjan.onstack", || false);
pub static IS_PART_OF_CYCLE: Property<bool> = Property::with_default("partOfCycle", || false);
pub static WEIGHT: Property<f64> = Property::new("medianHeuristic.weight");
pub static HIDDEN_NODES: Property<Vec<LNodeId>> = Property::new("hiddenNodes");
pub static ORIGINAL_OPPOSITE_PORT: Property<LPortId> = Property::new("originalOppositePort");
pub static END_LABEL_EDGE: Property<LEdgeId> = Property::new("endLabelEdge");
/// `InternalProperties.REPRESENTED_LABELS` (`List<LLabel>` on a label
/// dummy node).
pub static REPRESENTED_LABELS: Property<Vec<LLabelId>> = Property::new("representedLabels");
/// `InternalProperties.END_LABELS` (`Map<LPort, LabelCell>` on a node);
/// stored as an ordered list of (port, cell) pairs.
pub static END_LABELS: Property<EndLabelCells> = Property::new("endLabels");
/// `InternalProperties.LABEL_SIDE` (set on label dummy nodes and on
/// edge labels; distinct from `LabelSide.LABEL_SIDE`, see lgraph_adapters).
pub static LABEL_SIDE: Property<LabelSide> =
    Property::with_default("labelSide", || LabelSide::UNKNOWN);
pub static ORIGINAL_PORT_CONSTRAINTS: Property<crate::core::options::PortConstraints> =
    Property::new("originalPortConstraints");
pub static SPLINE_NS_PORT_Y_COORD: Property<f64> = Property::new("splines.nsPortY");
/// `InternalProperties.SPLINE_SURVIVING_EDGE` (only set by the wrapping
/// `BreakingPointRemover`, which is not ported yet).
pub static SPLINE_SURVIVING_EDGE: Property<LEdgeId> = Property::new("splines.survivingEdge");
/// `InternalProperties.SPLINE_ROUTE_START` (`List<SplineSegment>`); here
/// indices into the graph's `SPLINE_SEGMENT_STORE`.
pub static SPLINE_ROUTE_START: Property<Vec<i32>> = Property::new("splines.route.start");
/// `InternalProperties.SPLINE_EDGE_CHAIN` (`List<LEdge>`).
pub static SPLINE_EDGE_CHAIN: Property<Vec<LEdgeId>> = Property::new("splines.edgeChain");

/// Rust-only: the arena of `SplineSegment`s shared between the
/// `SplineEdgeRouter` and the `FinalSplineBendpointsCalculator`.
pub static SPLINE_SEGMENT_STORE: Property<crate::alg_layered::p5edges::splines::SplineSegmentStore> =
    Property::new("splines.segmentStore.rs");

/// `InternalProperties.CROSS_HIERARCHY_MAP`
/// (`Multimap<LEdge, CrossHierarchyEdge>`), attached to the top-level graph by
/// the `CompoundGraphPreprocessor` and consumed by the postprocessor.
pub static CROSS_HIERARCHY_MAP: Property<crate::alg_layered::compound::CrossHierarchyMap> =
    Property::new("crossHierarchyMap");

/// `InternalProperties.TARGET_NODE_MODEL_ORDER` (`Map<LNode, Integer>`),
/// cached on a node by `SortByInputModelProcessor.longEdgeTargetNodePreprocessing`.
/// Insertion order is irrelevant (only keyed lookups happen), but kept
/// deterministic via `IndexMap`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TargetNodeModelOrder(pub indexmap::IndexMap<LNodeId, i32>);
impl JavaString for TargetNodeModelOrder {
    fn java_string(&self) -> String {
        format!("{:?}", self.0)
    }
}
impl JavaCloneable for TargetNodeModelOrder {
    const CLONEABLE: bool = false;
}
pub static TARGET_NODE_MODEL_ORDER: Property<TargetNodeModelOrder> =
    Property::new("targetNodeModelOrder");

// ---------------------------------------------------------------------------
// Breaking-point wrapping (multi-edge) storage.
// ---------------------------------------------------------------------------

/// Index of a [`BPInfo`] inside the per-graph [`BPInfoStore`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BPInfoId(pub usize);

internal_value!(BPInfoId);

/// Information attached to a single
/// breaking point. The objects live in a per-graph
/// [`BPInfoStore`] (held as a graph property) and nodes reference them by
/// [`BPInfoId`]. Linked-list `prev`/`next` are also ids.
#[derive(Clone, Debug, PartialEq)]
pub struct BPInfo {
    pub start: LNodeId,
    pub end: LNodeId,
    pub node_start_edge: LEdgeId,
    pub start_end_edge: LEdgeId,
    pub original_edge: LEdgeId,

    pub start_in_layer_dummy: Option<LNodeId>,
    pub start_in_layer_edge: Option<LEdgeId>,
    pub end_in_layer_dummy: Option<LNodeId>,
    pub end_in_layer_edge: Option<LEdgeId>,

    pub prev: Option<BPInfoId>,
    pub next: Option<BPInfoId>,
}

impl BPInfo {
    pub fn new(
        start: LNodeId,
        end: LNodeId,
        node_start_edge: LEdgeId,
        start_end_edge: LEdgeId,
        original_edge: LEdgeId,
    ) -> BPInfo {
        BPInfo {
            start,
            end,
            node_start_edge,
            start_end_edge,
            original_edge,
            start_in_layer_dummy: None,
            start_in_layer_edge: None,
            end_in_layer_dummy: None,
            end_in_layer_edge: None,
            prev: None,
            next: None,
        }
    }
}

/// Per-graph arena of [`BPInfo`] objects (pooled and referenced by
/// [`BPInfoId`]).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct BPInfoStore {
    pub infos: Vec<BPInfo>,
}

impl BPInfoStore {
    pub fn push(&mut self, info: BPInfo) -> BPInfoId {
        let id = BPInfoId(self.infos.len());
        self.infos.push(info);
        id
    }
    pub fn get(&self, id: BPInfoId) -> &BPInfo {
        &self.infos[id.0]
    }
    pub fn get_mut(&mut self, id: BPInfoId) -> &mut BPInfo {
        &mut self.infos[id.0]
    }
}

impl crate::graph::properties::JavaString for BPInfoStore {
    fn java_string(&self) -> String {
        format!("{self:?}")
    }
}
impl crate::graph::properties::JavaCloneable for BPInfoStore {
    const CLONEABLE: bool = false;
}

/// `InternalProperties.BREAKING_POINT_INFO` (a `BPInfo` on each breaking
/// point dummy node). Stored here as a [`BPInfoId`] into the graph's
/// [`BPInfoStore`].
pub static BREAKING_POINT_INFO: Property<BPInfoId> = Property::new("breakingPoint.info");

/// Rust-only: the per-graph arena of [`BPInfo`] objects, attached to the graph
/// for the lifetime of the breaking-point wrapping phase.
pub static BP_INFO_STORE: Property<BPInfoStore> = Property::new("breakingPoint.store.rs");
