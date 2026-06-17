//! Pipeline structure: `LayeredPhases`, `IntermediateProcessorStrategy`
//! (ordinal order = processing order within a slot), and the assembled
//! pipeline representation.

use crate::elk_enum;

elk_enum! {
    pub enum LayeredPhases {
        P1_CYCLE_BREAKING,
        P2_LAYERING,
        P3_NODE_ORDERING,
        P4_NODE_PLACEMENT,
        P5_EDGE_ROUTING,
    }
}

elk_enum! {
    pub enum IntermediateProcessorStrategy {
        DIRECTION_PREPROCESSOR,
        COMMENT_PREPROCESSOR,
        EDGE_AND_LAYER_CONSTRAINT_EDGE_REVERSER,
        INTERACTIVE_EXTERNAL_PORT_POSITIONER,
        PARTITION_PREPROCESSOR,
        LABEL_DUMMY_INSERTER,
        SELF_LOOP_PREPROCESSOR,
        LAYER_CONSTRAINT_PREPROCESSOR,
        PARTITION_MIDPROCESSOR,
        HIGH_DEGREE_NODE_LAYER_PROCESSOR,
        NODE_PROMOTION,
        LAYER_CONSTRAINT_POSTPROCESSOR,
        PARTITION_POSTPROCESSOR,
        HIERARCHICAL_PORT_CONSTRAINT_PROCESSOR,
        SEMI_INTERACTIVE_CROSSMIN_PROCESSOR,
        BREAKING_POINT_INSERTER,
        LONG_EDGE_SPLITTER,
        PORT_SIDE_PROCESSOR,
        INVERTED_PORT_PROCESSOR,
        PORT_LIST_SORTER,
        SORT_BY_INPUT_ORDER_OF_MODEL,
        NORTH_SOUTH_PORT_PREPROCESSOR,
        BREAKING_POINT_PROCESSOR,
        ONE_SIDED_GREEDY_SWITCH,
        TWO_SIDED_GREEDY_SWITCH,
        SELF_LOOP_PORT_RESTORER,
        ALTERNATING_LAYER_UNZIPPER,
        SINGLE_EDGE_GRAPH_WRAPPER,
        IN_LAYER_CONSTRAINT_PROCESSOR,
        END_NODE_PORT_LABEL_MANAGEMENT_PROCESSOR,
        LABEL_AND_NODE_SIZE_PROCESSOR,
        INNERMOST_NODE_MARGIN_CALCULATOR,
        SELF_LOOP_ROUTER,
        COMMENT_NODE_MARGIN_CALCULATOR,
        END_LABEL_PREPROCESSOR,
        LABEL_DUMMY_SWITCHER,
        CENTER_LABEL_MANAGEMENT_PROCESSOR,
        LABEL_SIDE_SELECTOR,
        HYPEREDGE_DUMMY_MERGER,
        HIERARCHICAL_PORT_DUMMY_SIZE_PROCESSOR,
        LAYER_SIZE_AND_GRAPH_HEIGHT_CALCULATOR,
        HIERARCHICAL_PORT_POSITION_PROCESSOR,
        CONSTRAINTS_POSTPROCESSOR,
        COMMENT_POSTPROCESSOR,
        HYPERNODE_PROCESSOR,
        HIERARCHICAL_PORT_ORTHOGONAL_EDGE_ROUTER,
        LONG_EDGE_JOINER,
        SELF_LOOP_POSTPROCESSOR,
        BREAKING_POINT_REMOVER,
        NORTH_SOUTH_PORT_POSTPROCESSOR,
        HORIZONTAL_COMPACTOR,
        LABEL_DUMMY_REMOVER,
        FINAL_SPLINE_BENDPOINTS_CALCULATOR,
        END_LABEL_SORTER,
        REVERSED_EDGE_RESTORER,
        END_LABEL_POSTPROCESSOR,
        HIERARCHICAL_NODE_RESIZER,
        DIRECTION_POSTPROCESSOR,
    }
}

/// One step of the assembled algorithm: an intermediate processor or one of
/// the five phase implementations (with its selected strategy).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PipelineStep {
    Intermediate(IntermediateProcessorStrategy),
    CycleBreaking(crate::alg_layered::options_gen::CycleBreakingStrategy),
    Layering(crate::alg_layered::options_gen::LayeringStrategy),
    CrossingMinimization(crate::alg_layered::options_gen::CrossingMinimizationStrategy),
    NodePlacement(crate::alg_layered::options_gen::NodePlacementStrategy),
    EdgeRouting(crate::core::options::EdgeRouting),
}

/// Per-slot sets of intermediate
/// processors. Slot `i` is "before phase i" for `i < 5`; slot 5 is
/// "after P5".
#[derive(Default, Clone, Debug)]
pub struct ProcessorConfiguration {
    slots: [std::collections::BTreeSet<IntermediateProcessorStrategy>; 6],
}

impl ProcessorConfiguration {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_before(
        &mut self,
        phase: LayeredPhases,
        processor: IntermediateProcessorStrategy,
    ) -> &mut Self {
        self.slots[phase as usize].insert(processor);
        self
    }

    pub fn add_after(
        &mut self,
        phase: LayeredPhases,
        processor: IntermediateProcessorStrategy,
    ) -> &mut Self {
        self.slots[phase as usize + 1].insert(processor);
        self
    }

    pub fn add_all(&mut self, other: &ProcessorConfiguration) -> &mut Self {
        for (i, slot) in other.slots.iter().enumerate() {
            self.slots[i].extend(slot.iter().copied());
        }
        self
    }

    /// Processors in slot `i`, in ordinal order (BTreeSet iteration).
    pub fn slot(&self, i: usize) -> impl Iterator<Item = IntermediateProcessorStrategy> + '_ {
        self.slots[i].iter().copied()
    }
}
