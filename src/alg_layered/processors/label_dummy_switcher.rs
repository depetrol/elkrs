//! Moves label dummy nodes into an "optimal"
//! layer their long edges cross by switching the order of long edge dummies
//! and label dummies.

use crate::core::options::Alignment;
use crate::graph::properties::{ElkEnum, Property};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::CenterEdgeLabelPlacementStrategy;

/// A property to communicate with
/// the analyses that can be run on a graph.
pub static INCLUDE_LABEL: Property<bool> =
    Property::with_default("edgelabelcenterednessanalysis.includelabel", || false);

/// `CenterEdgeLabelPlacementStrategy.usesLabelSizeInformation`.
fn uses_label_size_information(strategy: CenterEdgeLabelPlacementStrategy) -> bool {
    matches!(
        strategy,
        CenterEdgeLabelPlacementStrategy::WIDEST_LAYER
            | CenterEdgeLabelPlacementStrategy::CENTER_LAYER
            | CenterEdgeLabelPlacementStrategy::SPACE_EFFICIENT_LAYER
    )
}

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // The default placement strategy to be assumed if no more specific strategy is set
    let default_placement_strategy: CenterEdgeLabelPlacementStrategy = a
        .graph(graph)
        .properties
        .get(&lopts::EDGE_LABELS_CENTER_LABEL_PLACEMENT_STRATEGY);

    // Assign layer IDs
    assign_ids_to_layers(a, graph);

    // Collect label dummy infos by placement strategy
    let label_dummy_infos = gather_label_dummy_infos(a, graph, default_placement_strategy);

    // If at least one width-based strategy is active, calculate layer widths. After this
    // point, layer widths are either all zero or equal to each layer's widest non-dummy node
    let mut layer_widths = vec![0.0f64; a.graph(graph).layers.len()];

    for &strategy in CenterEdgeLabelPlacementStrategy::VALUES {
        if uses_label_size_information(strategy)
            && !label_dummy_infos[strategy.ordinal()].is_empty()
        {
            // We have found an active width-based strategy
            calculate_layer_widths(a, graph, &mut layer_widths);
            break;
        }
    }

    // Work through the non-width-based strategies first. They might change the width of
    // layers, which might influence size-based strategies later on
    for &strategy in CenterEdgeLabelPlacementStrategy::VALUES {
        if !uses_label_size_information(strategy) {
            process_strategy(a, &label_dummy_infos[strategy.ordinal()], &mut layer_widths);
        }
    }

    // Now execute size-based strategies
    for &strategy in CenterEdgeLabelPlacementStrategy::VALUES {
        if uses_label_size_information(strategy) {
            process_strategy(a, &label_dummy_infos[strategy.ordinal()], &mut layer_widths);
        }
    }

    Ok(())
}

/// `assignIdsToLayers`: zero-based IDs in layer order.
fn assign_ids_to_layers(a: &mut LGraphArena, graph: LGraphId) {
    let layers = a.graph(graph).layers.clone();
    for (layer_index, layer) in layers.into_iter().enumerate() {
        a.layer_mut(layer).id = layer_index as i32;
    }
}

/// `gatherLabelDummyInfos`: the graph's label dummies as
/// `LabelDummyInfo`s, indexed by placement strategy ordinal.
fn gather_label_dummy_infos(
    a: &LGraphArena,
    graph: LGraphId,
    default_placement_strategy: CenterEdgeLabelPlacementStrategy,
) -> Vec<Vec<LabelDummyInfo>> {
    let mut infos: Vec<Vec<LabelDummyInfo>> = (0..CenterEdgeLabelPlacementStrategy::VALUES
        .len())
        .map(|_| Vec::new())
        .collect();

    for &layer in &a.graph(graph).layers {
        for &node in &a.layer(layer).nodes {
            if a.node(node).node_type == NodeType::LABEL {
                let info = LabelDummyInfo::new(a, node, default_placement_strategy);
                infos[info.placement_strategy.ordinal()].push(info);
            }
        }
    }

    infos
}

/// `calculateLayerWidths`.
fn calculate_layer_widths(a: &LGraphArena, graph: LGraphId, layer_widths: &mut [f64]) {
    for &layer in &a.graph(graph).layers {
        layer_widths[a.layer(layer).id as usize] = find_max_non_dummy_node_width(a, layer);
    }
}

fn find_max_non_dummy_node_width(a: &LGraphArena, layer: LayerId) -> f64 {
    let graph = a.layer(layer).graph.unwrap();
    if a.graph(graph)
        .properties
        .get::<crate::core::options::Direction>(&lopts::DIRECTION)
        .is_vertical()
    {
        return 0.0;
    }

    let mut max_width = 0.0f64;
    for &node in &a.layer(layer).nodes {
        if a.node(node).node_type == NodeType::NORMAL {
            max_width = max_width.max(a.node(node).size.x);
        }
    }
    max_width
}

/// `processStrategy`: executes label dummy switching on the given list
/// of label dummy infos.
fn process_strategy(
    a: &mut LGraphArena,
    label_dummy_infos: &[LabelDummyInfo],
    layer_widths: &mut [f64],
) {
    // There might not be anything to do for this strategy
    if label_dummy_infos.is_empty() {
        return;
    }

    // Check if the strategy has a special processing method
    if label_dummy_infos[0].placement_strategy
        == CenterEdgeLabelPlacementStrategy::SPACE_EFFICIENT_LAYER
    {
        compute_space_efficient_assignment(a, label_dummy_infos, layer_widths);
    } else {
        // Execute the strategy for each label dummy
        for info in label_dummy_infos {
            match info.placement_strategy {
                CenterEdgeLabelPlacementStrategy::CENTER_LAYER => {
                    let target = find_center_layer_target_id(a, info, layer_widths);
                    assign_layer(a, info, target, layer_widths);
                }
                CenterEdgeLabelPlacementStrategy::MEDIAN_LAYER => {
                    let target = find_median_layer_target_id(info);
                    assign_layer(a, info, target, layer_widths);
                }
                CenterEdgeLabelPlacementStrategy::WIDEST_LAYER => {
                    let target = find_widest_layer_target_id(info, layer_widths);
                    assign_layer(a, info, target, layer_widths);
                }
                CenterEdgeLabelPlacementStrategy::HEAD_LAYER => {
                    set_end_layer_node_alignment(a, info);
                    let target = find_end_layer_target_id(a, info, true);
                    assign_layer(a, info, target, layer_widths);
                }
                CenterEdgeLabelPlacementStrategy::TAIL_LAYER => {
                    set_end_layer_node_alignment(a, info);
                    let target = find_end_layer_target_id(a, info, false);
                    assign_layer(a, info, target, layer_widths);
                }
                CenterEdgeLabelPlacementStrategy::SPACE_EFFICIENT_LAYER => unreachable!(),
            }

            update_long_edge_source_label_dummy_info(a, info);
        }
    }
}

//////////////////////////////////////////////////////////////////////////////
// Widest Layer

/// `findWidestLayerTargetId`.
fn find_widest_layer_target_id(info: &LabelDummyInfo, layer_widths: &[f64]) -> i32 {
    // Find the widest layer among those the long edge dummies are placed in
    let mut widest_layer_index = info.leftmost_layer_id;

    for index in (widest_layer_index + 1)..=info.rightmost_layer_id {
        if layer_widths[index as usize] > layer_widths[widest_layer_index as usize] {
            widest_layer_index = index;
        }
    }

    widest_layer_index
}

//////////////////////////////////////////////////////////////////////////////
// Center Layer

/// `findCenterLayerTargetId`.
fn find_center_layer_target_id(
    a: &LGraphArena,
    info: &LabelDummyInfo,
    layer_widths: &[f64],
) -> i32 {
    // Sum up the widths of all the layers this thing spans
    let layer_width_sums = compute_layer_width_sums(a, info, layer_widths);

    // Find the first layer that exceeds half the width
    let threshold = layer_width_sums[layer_width_sums.len() - 1] / 2.0;

    for (i, &sum) in layer_width_sums.iter().enumerate() {
        if sum >= threshold {
            return info.leftmost_layer_id + i as i32;
        }
    }

    // This should actually not happen
    debug_assert!(false);
    info.leftmost_layer_id + info.left_long_edge_dummies.len() as i32
}

/// `computeLayerWidthSums`.
fn compute_layer_width_sums(a: &LGraphArena, info: &LabelDummyInfo, layer_widths: &[f64]) -> Vec<f64> {
    // The minimum space that we think will be left between layers
    let lgraph = a.node_graph(info.label_dummy);
    let edge_node_spacing: f64 = a
        .graph(lgraph)
        .properties
        .get::<f64>(&lopts::SPACING_EDGE_NODE_BETWEEN_LAYERS)
        * 2.0;
    let node_node_spacing: f64 = a
        .graph(lgraph)
        .properties
        .get(&lopts::SPACING_NODE_NODE_BETWEEN_LAYERS);
    let min_space_between_layers = edge_node_spacing.max(node_node_spacing);

    // The array that will hold the accumulated widths
    let mut layer_width_sums = vec![0.0f64; info.total_dummy_count() as usize];

    let mut current_width_sum = -min_space_between_layers;
    let mut current_index = 0;

    for &left_dummy in &info.left_long_edge_dummies {
        current_width_sum += layer_widths[a.layer(a.node(left_dummy).layer.unwrap()).id as usize]
            + min_space_between_layers;
        layer_width_sums[current_index] = current_width_sum;
        current_index += 1;
    }

    current_width_sum += layer_widths
        [a.layer(a.node(info.label_dummy).layer.unwrap()).id as usize]
        + min_space_between_layers;
    layer_width_sums[current_index] = current_width_sum;
    current_index += 1;

    for &right_dummy in &info.right_long_edge_dummies {
        current_width_sum += layer_widths[a.layer(a.node(right_dummy).layer.unwrap()).id as usize]
            + min_space_between_layers;
        layer_width_sums[current_index] = current_width_sum;
        current_index += 1;
    }

    layer_width_sums
}

//////////////////////////////////////////////////////////////////////////////
// Median Layer

/// `findMedianLayerTargetId`.
fn find_median_layer_target_id(info: &LabelDummyInfo) -> i32 {
    // Find the median of the layers spanned by the long edge this label dummy is part of
    let layers = info.total_dummy_count();
    let lower_median = (layers - 1) / 2;

    info.leftmost_layer_id + lower_median
}

//////////////////////////////////////////////////////////////////////////////
// End Layer

/// `findEndLayerTargetId`.
fn find_end_layer_target_id(a: &LGraphArena, info: &LabelDummyInfo, head_layer: bool) -> i32 {
    let reversed = is_part_of_reversed_edge(a, info);

    if (head_layer && !reversed) || (!head_layer && reversed) {
        info.rightmost_layer_id
    } else {
        info.leftmost_layer_id
    }
}

/// `setEndLayerNodeAlignment`.
fn set_end_layer_node_alignment(a: &mut LGraphArena, info: &LabelDummyInfo) {
    let is_head_label =
        info.placement_strategy == CenterEdgeLabelPlacementStrategy::HEAD_LAYER;
    let is_part_of_reversed_edge = is_part_of_reversed_edge(a, info);

    if (is_head_label && !is_part_of_reversed_edge)
        || (!is_head_label && is_part_of_reversed_edge)
    {
        a.node(info.label_dummy)
            .properties
            .set(&lopts::ALIGNMENT, Alignment::RIGHT);
    } else {
        a.node(info.label_dummy)
            .properties
            .set(&lopts::ALIGNMENT, Alignment::LEFT);
    }
}

/// `isPartOfReversedEdge`.
fn is_part_of_reversed_edge(a: &LGraphArena, info: &LabelDummyInfo) -> bool {
    debug_assert!(a.node(info.label_dummy).node_type == NodeType::LABEL);

    // Find incoming and outgoing edge
    let incoming = a.node_incoming_edges(info.label_dummy)[0];
    let outgoing = a.node_outgoing_edges(info.label_dummy)[0];

    a.edge(incoming).properties.get(&iprops::REVERSED)
        || a.edge(outgoing).properties.get(&iprops::REVERSED)
}

//////////////////////////////////////////////////////////////////////////////
// Space Efficient

/// `computeSpaceEfficientAssignment`.
fn compute_space_efficient_assignment(
    a: &mut LGraphArena,
    label_dummy_infos: &[LabelDummyInfo],
    layer_widths: &mut [f64],
) {
    // We start by assigning all label dummies that only have a single layer to choose from
    // or that can be assigned to a layer large enough for them
    let mut non_trivial_labels = perform_trivial_assignments(a, label_dummy_infos, layer_widths);
    if non_trivial_labels.is_empty() {
        return;
    }

    // The remaining labels are not as easy to assign. Sort descendingly by size.
    non_trivial_labels.sort_by(|&i1, &i2| {
        a.node(label_dummy_infos[i2].label_dummy)
            .size
            .x
            .total_cmp(&a.node(label_dummy_infos[i1].label_dummy).size.x)
    });

    let label_count = non_trivial_labels.len();
    for label_index in 0..label_count {
        let target = find_potentially_widest_layer(
            a,
            label_dummy_infos,
            &non_trivial_labels,
            label_index,
            layer_widths,
        );
        assign_layer(
            a,
            &label_dummy_infos[non_trivial_labels[label_index]],
            target,
            layer_widths,
        );
    }
}

/// `performTrivialAssignments`. Returns indices into `label_dummy_infos`
/// of the labels that remain unassigned.
fn perform_trivial_assignments(
    a: &mut LGraphArena,
    label_dummy_infos: &[LabelDummyInfo],
    layer_widths: &mut [f64],
) -> Vec<usize> {
    let mut remaining_labels: Vec<usize> = Vec::new();

    for (index, info) in label_dummy_infos.iter().enumerate() {
        if info.leftmost_layer_id == info.rightmost_layer_id {
            // Assign to only available layer and remove from list
            assign_layer(a, info, info.leftmost_layer_id, layer_widths);
        } else if !assign_to_wider_layer(a, info, layer_widths) {
            // Ending up here means that we didn't find a layer wide enough for the node
            remaining_labels.push(index);
        }
    }

    remaining_labels
}

/// `assignToWiderLayer`: assigns the given label dummy to the first
/// layer wide enough to house it, returning whether that succeeded.
fn assign_to_wider_layer(
    a: &mut LGraphArena,
    info: &LabelDummyInfo,
    layer_widths: &mut [f64],
) -> bool {
    // Check if the label dummy can be assigned a layer that already is at least as wide
    let dummy_width = a.node(info.label_dummy).size.x;
    let graph = a.node_graph(info.label_dummy);
    let valid_layers: Vec<LayerId> = a.graph(graph).layers
        [info.leftmost_layer_id as usize..=info.rightmost_layer_id as usize]
        .to_vec();

    for layer in valid_layers {
        if a.layer(layer).size.x >= dummy_width {
            let layer_id = a.layer(layer).id;
            assign_layer(a, info, layer_id, layer_widths);
            return true;
        }
    }

    // Ending up here means that we didn't find a layer wide enough for our label
    false
}

/// `findPotentiallyWidestLayer`. `sorted_infos` holds indices into
/// `label_dummy_infos`, sorted descendingly by label width; `label_index`
/// indexes into `sorted_infos`.
fn find_potentially_widest_layer(
    a: &LGraphArena,
    label_dummy_infos: &[LabelDummyInfo],
    sorted_infos: &[usize],
    label_index: usize,
    layer_widths: &[f64],
) -> i32 {
    let label_count = sorted_infos.len();
    let info = &label_dummy_infos[sorted_infos[label_index]];
    let label_dummy_width = a.node(info.label_dummy).size.x;

    // Iterate over the label's valid layers
    let mut widest_layer_index = info.leftmost_layer_id;
    let mut widest_layer_width = 0.0f64;

    for layer in info.leftmost_layer_id..=info.rightmost_layer_id {
        // If the layer is already at least as large as the current label, simply return it
        if label_dummy_width <= layer_widths[layer as usize] {
            return layer;
        }

        // The initial potential width is less wide than the label
        let mut potential_width = layer_widths[layer as usize];

        // Find the largest unassigned label that is part of this layer
        let mut largest_unassigned_label: Option<&LabelDummyInfo> = None;
        for label in (label_index + 1)..label_count {
            // Check if the label can be placed in the current layer
            let curr_label_info = &label_dummy_infos[sorted_infos[label]];
            if curr_label_info.leftmost_layer_id <= layer
                && curr_label_info.rightmost_layer_id >= layer
            {
                largest_unassigned_label = Some(curr_label_info);
            }
        }

        // Update layer's potential size
        if let Some(largest) = largest_unassigned_label {
            potential_width = potential_width.max(a.node(largest.label_dummy).size.x);
        }

        // Update widest layer (if there are multiple widest layers, we use the leftmost one)
        if potential_width > widest_layer_width {
            widest_layer_index = layer;
            widest_layer_width = potential_width;
        }
    }

    widest_layer_index
}

//////////////////////////////////////////////////////////////////////////////
// Swapping Utilities

/// `assignLayer`: assigns the label dummy to the layer with the given
/// index, updating layer width information.
fn assign_layer(
    a: &mut LGraphArena,
    info: &LabelDummyInfo,
    target_layer_index: i32,
    layer_widths: &mut [f64],
) {
    // If the label dummy is not in the target layer yet, swap it with the long edge
    // dummy that is
    if target_layer_index
        != info.leftmost_layer_id + info.left_long_edge_dummies.len() as i32
    {
        swap_nodes(
            a,
            info.label_dummy,
            info.ith_dummy_node((target_layer_index - info.leftmost_layer_id) as usize),
        );
    }

    // Update the size information of the label dummy's new layer
    let new_layer_id = a.layer(a.node(info.label_dummy).layer.unwrap()).id as usize;
    layer_widths[new_layer_id] =
        layer_widths[new_layer_id].max(a.node(info.label_dummy).size.x);

    let represented = a
        .node(info.label_dummy)
        .properties
        .try_get(&iprops::REPRESENTED_LABELS)
        .unwrap_or_default();
    for label in represented {
        a.label(label).properties.set(&INCLUDE_LABEL, true);
    }
}

/// `swapNodes`: swaps the label dummy with the given long edge dummy.
fn swap_nodes(a: &mut LGraphArena, label_dummy: LNodeId, long_edge_dummy: LNodeId) {
    // Find the layers and the positions inside the layers of the dummy nodes
    let layer1 = a.node(label_dummy).layer.unwrap();
    let layer2 = a.node(long_edge_dummy).layer.unwrap();

    let dummy1_layer_position = a.node_index_in_layer(label_dummy) as usize;
    let dummy2_layer_position = a.node_index_in_layer(long_edge_dummy) as usize;

    // Detect incoming and outgoing ports of the nodes (assumes there's just one of each
    // kind, which should be true for long edge and label dummy nodes)
    let input_port1 = a.node_input_ports(label_dummy)[0];
    let output_port1 = a.node_output_ports(label_dummy)[0];
    let input_port2 = a.node_input_ports(long_edge_dummy)[0];
    let output_port2 = a.node_output_ports(long_edge_dummy)[0];

    // Store incoming and outgoing edges
    let incoming_edges1 = a.port(input_port1).incoming_edges.clone();
    let outgoing_edges1 = a.port(output_port1).outgoing_edges.clone();
    let incoming_edges2 = a.port(input_port2).incoming_edges.clone();
    let outgoing_edges2 = a.port(output_port2).outgoing_edges.clone();

    // Put first dummy into second dummy's layer and reroute second dummy's edges to it
    a.node_set_layer_at(label_dummy, Some(layer2), dummy2_layer_position);
    for edge in incoming_edges2 {
        a.edge_set_target(edge, Some(input_port1));
    }
    for edge in outgoing_edges2 {
        a.edge_set_source(edge, Some(output_port1));
    }

    // Put second dummy into first dummy's layer and reroute first dummy's edges to it
    a.node_set_layer_at(long_edge_dummy, Some(layer1), dummy1_layer_position);
    for edge in incoming_edges1 {
        a.edge_set_target(edge, Some(input_port2));
    }
    for edge in outgoing_edges1 {
        a.edge_set_source(edge, Some(output_port2));
    }
}

/// `updateLongEdgeSourceLabelDummyInfo`: updates the
/// `LONG_EDGE_BEFORE_LABEL_DUMMY` property of long edge dummies preceding the
/// given label dummy node.
fn update_long_edge_source_label_dummy_info(a: &mut LGraphArena, info: &LabelDummyInfo) {
    // Predecessors
    let mut long_edge_dummy = a.edge_source_node(a.node_incoming_edges(info.label_dummy)[0]);
    while a.node(long_edge_dummy).node_type == NodeType::LONG_EDGE {
        a.node(long_edge_dummy)
            .properties
            .set(&iprops::LONG_EDGE_BEFORE_LABEL_DUMMY, true);
        long_edge_dummy = a.edge_source_node(a.node_incoming_edges(long_edge_dummy)[0]);
    }

    // We may want to do things to the successors as well at some point
}

//////////////////////////////////////////////////////////////////////////////
// Label Dummy Info Class

/// `LabelDummyInfo`: a label dummy along with the long edge dummies to
/// its left and right.
struct LabelDummyInfo {
    /// The label dummy node.
    label_dummy: LNodeId,
    /// The label placement strategy to be used with this label.
    placement_strategy: CenterEdgeLabelPlacementStrategy,
    /// The long edge dummies to the left of the label dummy. May well be empty.
    left_long_edge_dummies: Vec<LNodeId>,
    /// The long edge dummies to the right of the label dummy. May well be empty.
    right_long_edge_dummies: Vec<LNodeId>,
    /// ID of the leftmost layer the label dummy's long edge has a dummy node in.
    leftmost_layer_id: i32,
    /// ID of the rightmost layer the label dummy's long edge has a dummy node in.
    rightmost_layer_id: i32,
}

impl LabelDummyInfo {
    fn new(
        a: &LGraphArena,
        label_dummy: LNodeId,
        default_placement_strategy: CenterEdgeLabelPlacementStrategy,
    ) -> Self {
        let mut info = LabelDummyInfo {
            label_dummy,
            placement_strategy: default_placement_strategy,
            left_long_edge_dummies: Vec::new(),
            right_long_edge_dummies: Vec::new(),
            leftmost_layer_id: 0,
            rightmost_layer_id: 0,
        };

        // Gather long edge dummies that are part of the label dummy's edge
        info.gather_left_long_edge_dummies(a);
        info.gather_right_long_edge_dummies(a);

        info.leftmost_layer_id = match info.left_long_edge_dummies.first() {
            None => a.layer(a.node(label_dummy).layer.unwrap()).id,
            Some(&first) => a.layer(a.node(first).layer.unwrap()).id,
        };

        info.rightmost_layer_id = match info.right_long_edge_dummies.last() {
            None => a.layer(a.node(label_dummy).layer.unwrap()).id,
            Some(&last) => a.layer(a.node(last).layer.unwrap()).id,
        };

        // Check if the label wants to deviate from the default placement strategy
        let represented = a
            .node(label_dummy)
            .properties
            .try_get(&iprops::REPRESENTED_LABELS)
            .unwrap_or_default();
        for label in represented {
            // Take the first override we can find
            if a.label(label)
                .properties
                .has(&lopts::EDGE_LABELS_CENTER_LABEL_PLACEMENT_STRATEGY)
            {
                info.placement_strategy = a
                    .label(label)
                    .properties
                    .get(&lopts::EDGE_LABELS_CENTER_LABEL_PLACEMENT_STRATEGY);
                break;
            }
        }

        info
    }

    /// `gatherLeftLongEdgeDummies`.
    fn gather_left_long_edge_dummies(&mut self, a: &LGraphArena) {
        let mut source = self.label_dummy;
        loop {
            source = a.edge_source_node(a.node_incoming_edges(source)[0]);
            if a.node(source).node_type == NodeType::LONG_EDGE {
                self.left_long_edge_dummies.push(source);
            } else {
                break;
            }
        }

        // The list is currently not in the order we would expect, so reverse it
        self.left_long_edge_dummies.reverse();
    }

    /// `gatherRightLongEdgeDummies`.
    fn gather_right_long_edge_dummies(&mut self, a: &LGraphArena) {
        let mut target = self.label_dummy;
        loop {
            target = a.edge_target_node(a.node_outgoing_edges(target)[0]);
            if a.node(target).node_type == NodeType::LONG_EDGE {
                self.right_long_edge_dummies.push(target);
            } else {
                break;
            }
        }
    }

    /// `totalDummyCount`.
    fn total_dummy_count(&self) -> i32 {
        self.rightmost_layer_id - self.leftmost_layer_id + 1
    }

    /// `ithDummyNode`.
    fn ith_dummy_node(&self, i: usize) -> LNodeId {
        if i < self.left_long_edge_dummies.len() {
            // The i-th dummy is a long edge dummy to the label dummy's left
            self.left_long_edge_dummies[i]
        } else if i == self.left_long_edge_dummies.len() {
            // The i-th dummy is the label dummy itself
            self.label_dummy
        } else {
            // The i-th dummy is a long edge dummy to the label dummy's right
            self.right_long_edge_dummies[i - self.left_long_edge_dummies.len() - 1]
        }
    }
}
