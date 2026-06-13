//! Port of `org.eclipse.elk.alg.layered.options.InternalProperties` (the
//! subset needed so far; extended as more processors are ported).
//!
//! Element references are stored as arena ids. Properties whose Java type is
//! a mutable shared object require read-modify-write at the call sites.

use elk_core::adapters::LabelSide;
use elk_core::options::PortSide;
use elk_graph::math::{KVector, KVectorChain};
use elk_graph::properties::{EnumSet, JavaCloneable, JavaString, Property};

use crate::graph::{LEdgeId, LGraphId, LLabelId, LNodeId, LPortId};
use crate::options_gen::{EdgeConstraint, GraphProperties, InLayerConstraint};
use crate::processors::end_label_preprocessor::EndLabelCells;

/// Reference back to the original `ElkGraph` element (Java
/// `InternalProperties.ORIGIN`, typed `Object`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    Node(elk_graph::graph::NodeId),
    Port(elk_graph::graph::PortId),
    Edge(elk_graph::graph::EdgeId),
    Label(elk_graph::graph::LabelId),
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
/// Java `InternalProperties.REPRESENTED_LABELS` (`List<LLabel>` on a label
/// dummy node).
pub static REPRESENTED_LABELS: Property<Vec<LLabelId>> = Property::new("representedLabels");
/// Java `InternalProperties.END_LABELS` (`Map<LPort, LabelCell>` on a node);
/// stored as an ordered list of (port, cell) pairs.
pub static END_LABELS: Property<EndLabelCells> = Property::new("endLabels");
/// Java `InternalProperties.LABEL_SIDE` (set on label dummy nodes and on
/// edge labels; distinct from `LabelSide.LABEL_SIDE`, see lgraph_adapters).
pub static LABEL_SIDE: Property<LabelSide> =
    Property::with_default("labelSide", || LabelSide::UNKNOWN);
pub static ORIGINAL_PORT_CONSTRAINTS: Property<elk_core::options::PortConstraints> =
    Property::new("originalPortConstraints");
pub static SPLINE_NS_PORT_Y_COORD: Property<f64> = Property::new("splines.nsPortY");
/// Java `InternalProperties.SPLINE_SURVIVING_EDGE` (only set by the wrapping
/// `BreakingPointRemover`, which is not ported yet).
pub static SPLINE_SURVIVING_EDGE: Property<LEdgeId> = Property::new("splines.survivingEdge");
/// Java `InternalProperties.SPLINE_ROUTE_START` (`List<SplineSegment>`); here
/// indices into the graph's `SPLINE_SEGMENT_STORE`.
pub static SPLINE_ROUTE_START: Property<Vec<i32>> = Property::new("splines.route.start");
/// Java `InternalProperties.SPLINE_EDGE_CHAIN` (`List<LEdge>`).
pub static SPLINE_EDGE_CHAIN: Property<Vec<LEdgeId>> = Property::new("splines.edgeChain");

/// Rust-only: the arena of `SplineSegment`s shared between the
/// `SplineEdgeRouter` and the `FinalSplineBendpointsCalculator` (Java shares
/// the segment objects directly via `SPLINE_ROUTE_START`).
pub static SPLINE_SEGMENT_STORE: Property<crate::p5edges::splines::SplineSegmentStore> =
    Property::new("splines.segmentStore.rs");
