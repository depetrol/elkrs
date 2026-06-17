//!
//! Node placement that aligns long edges using linear segments (Sander 1996).

use crate::graph::properties::Property;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LayerId, NodeType};
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::spacings;

/// property for maximal priority of incoming edges.
static INPUT_PRIO: Property<i32> = Property::with_default("linearSegments.inputPrio", || 0);
/// property for maximal priority of outgoing edges.
static OUTPUT_PRIO: Property<i32> = Property::with_default("linearSegments.outputPrio", || 0);

/// factor for threshold after which balancing is aborted.
const THRESHOLD_FACTOR: f64 = 20.0;
/// the minimal number of iterations in pendulum mode.
const PENDULUM_ITERS: i32 = 4;
/// the number of additional iterations after the abort condition was met.
const FINAL_ITERS: i32 = 3;
/// Factor for threshold within which node overlapping is detected.
const OVERLAP_DETECT: f64 = 0.0001;

/// Segments are stored in a
/// `Vec` and referenced by index; after `sort_linear_segments` the index in
/// the vector equals the segment's rank (= `id` = `LNode.id`).
struct LinearSegment {
    /// Nodes of the linear segment.
    nodes: Vec<LNodeId>,
    /// Identifier value, used as index in the segments array.
    id: i32,
    /// Index in the previous layer. Used for cycle avoidance.
    index_in_last_layer: i32,
    /// The last layer where a node belonging to this segment was discovered.
    last_layer: i32,
    /// The accumulated force of the contained nodes.
    deflection: f64,
    /// The current weight of the contained nodes.
    weight: i32,
    /// The reference segment, if this has been unified with another
    /// (index into the rank-ordered segment array).
    ref_segment: Option<usize>,
}

impl LinearSegment {
    fn new(id: i32) -> Self {
        LinearSegment {
            nodes: Vec::new(),
            id,
            index_in_last_layer: -1,
            last_layer: -1,
            deflection: 0.0,
            weight: 0,
            ref_segment: None,
        }
    }
}

/// Resolves the reference chain, returning
/// the index of the top-level region segment.
fn region(segments: &[LinearSegment], idx: usize) -> usize {
    let mut seg = idx;
    while let Some(r) = segments[seg].ref_segment {
        seg = r;
    }
    seg
}

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // sort the linear segments of the layered graph
    let mut linear_segments = sort_linear_segments(a, graph);

    // create an unbalanced placement from the sorted segments
    create_unbalanced_placement(a, graph, &linear_segments);

    // balance the placement
    balance_placement(a, graph, &mut linear_segments);

    // post-process the placement for small corrections
    post_process(a, graph, &linear_segments);

    Ok(())
}

// /////////////////////////////////////////////////////////////////////////////
// Linear Segments Creation

/// Returns the
/// linear segments in rank order (`segment.id == index == LNode.id`).
fn sort_linear_segments(a: &mut LGraphArena, graph: LGraphId) -> Vec<LinearSegment> {
    // set the identifier and input / output priority for all nodes
    let mut segment_list: Vec<LinearSegment> = Vec::new();
    let layers: Vec<LayerId> = a.graph(graph).layers.clone();
    for &layer in &layers {
        for node in a.layer(layer).nodes.clone() {
            a.node_mut(node).id = -1;
            let mut inprio = i32::MIN;
            let mut outprio = i32::MIN;
            for port in a.node(node).ports.clone() {
                for edge in a.port(port).incoming_edges.clone() {
                    let prio: i32 = a.edge(edge).properties.get(&lopts::PRIORITY_STRAIGHTNESS);
                    inprio = inprio.max(prio);
                }
                for edge in a.port(port).outgoing_edges.clone() {
                    let prio: i32 = a.edge(edge).properties.get(&lopts::PRIORITY_STRAIGHTNESS);
                    outprio = outprio.max(prio);
                }
            }
            a.node(node).properties.set(&INPUT_PRIO, inprio);
            a.node(node).properties.set(&OUTPUT_PRIO, outprio);
        }
    }

    // create linear segments for the layered graph, ignoring odd port side dummies
    let mut next_linear_segment_id = 0i32;
    for &layer in &layers {
        for node in a.layer(layer).nodes.clone() {
            // calls to fillSegment may have caused the node ID to be != -1
            if a.node(node).id < 0 {
                let mut segment = LinearSegment::new(next_linear_segment_id);
                next_linear_segment_id += 1;
                fill_segment(a, node, &mut segment);
                segment_list.push(segment);
            }
        }
    }

    // create and initialize segment ordering graph
    let mut outgoing_list: Vec<Vec<i32>> = vec![Vec::new(); segment_list.len()];
    let mut incoming_count_list: Vec<i32> = vec![0; segment_list.len()];

    // create edges for the segment ordering graph
    create_dependency_graph_edges(
        a,
        graph,
        &mut segment_list,
        &mut outgoing_list,
        &mut incoming_count_list,
    );

    let num_segments = segment_list.len();
    let mut outgoing = outgoing_list;
    let mut incoming_count = incoming_count_list;

    // gather the sources of the segment ordering graph
    let mut next_rank = 0i32;
    let mut no_incoming: Vec<usize> = Vec::new();
    for i in 0..num_segments {
        if incoming_count[i] == 0 {
            no_incoming.push(i);
        }
    }

    // find a topological ordering of the segment ordering graph
    let mut new_ranks = vec![0i32; num_segments];
    while !no_incoming.is_empty() {
        let segment = no_incoming.remove(0);
        new_ranks[segment_list[segment].id as usize] = next_rank;
        next_rank += 1;

        while !outgoing[segment_list[segment].id as usize].is_empty() {
            let target = outgoing[segment_list[segment].id as usize].remove(0);
            incoming_count[target as usize] -= 1;

            if incoming_count[target as usize] == 0 {
                // the target's id equals its index in segment_list
                no_incoming.push(target as usize);
            }
        }
    }

    // apply the new ordering to the array of linear segments
    let mut ordered: Vec<Option<LinearSegment>> = (0..num_segments).map(|_| None).collect();
    for (i, mut ls) in segment_list.into_iter().enumerate() {
        let rank = new_ranks[i];
        ls.id = rank;
        for &node in &ls.nodes {
            a.node_mut(node).id = rank;
        }
        ordered[rank as usize] = Some(ls);
    }

    ordered.into_iter().map(|s| s.unwrap()).collect()
}

/// Fills the dependency graph with
/// dependencies, splitting segments that would introduce cycles.
fn create_dependency_graph_edges(
    a: &mut LGraphArena,
    graph: LGraphId,
    segment_list: &mut Vec<LinearSegment>,
    outgoing_list: &mut Vec<Vec<i32>>,
    incoming_count_list: &mut Vec<i32>,
) {
    let mut next_linear_segment_id = segment_list.len() as i32;
    let mut layer_index = 0i32;
    for &layer in &a.graph(graph).layers.clone() {
        let nodes: Vec<LNodeId> = a.layer(layer).nodes.clone();
        if nodes.is_empty() {
            // Ignore empty layers (note: the layerIndex increment is skipped)
            continue;
        }

        let mut iter_pos = 0usize;
        let mut index_in_layer = 0i32;

        // We carry the previous node with us for dependency management
        let mut previous_node: Option<LNodeId> = None;

        // Get the layer's first node
        let mut current_node: Option<LNodeId> = Some(nodes[iter_pos]);
        iter_pos += 1;

        while let Some(cur) = current_node {
            // Get the current node's segment
            let mut current_segment = a.node(cur).id as usize;

            // Check if we have a cycle
            if segment_list[current_segment].index_in_last_layer >= 0 {
                let mut cycle_segment: Option<usize> = None;
                for &cycle_node in nodes
                    .iter()
                    .skip((index_in_layer + 1) as usize)
                {
                    let cs = a.node(cycle_node).id as usize;
                    if segment_list[cs].last_layer == segment_list[current_segment].last_layer
                        && segment_list[cs].index_in_last_layer
                            < segment_list[current_segment].index_in_last_layer
                    {
                        cycle_segment = Some(cs);
                        break;
                    } else {
                        cycle_segment = None;
                    }
                }

                // If we have found a cycle segment, split the current linear segment
                if cycle_segment.is_some() {
                    // Update the current segment before it's split
                    if let Some(prev) = previous_node {
                        let cur_id = a.node(cur).id as usize;
                        incoming_count_list[cur_id] -= 1;
                        // remove the first occurrence
                        let prev_list = &mut outgoing_list[a.node(prev).id as usize];
                        if let Some(pos) =
                            prev_list.iter().position(|&s| s == current_segment as i32)
                        {
                            prev_list.remove(pos);
                        }
                    }

                    let new_id = next_linear_segment_id;
                    next_linear_segment_id += 1;
                    let new_segment = split_segment(a, segment_list, current_segment, cur, new_id);
                    current_segment = new_segment;
                    outgoing_list.push(Vec::new());

                    if previous_node.is_some() {
                        let prev = previous_node.unwrap();
                        outgoing_list[a.node(prev).id as usize].push(current_segment as i32);
                        incoming_count_list.push(1);
                    } else {
                        incoming_count_list.push(0);
                    }
                }
            }

            // Now add a dependency to the next node, if any
            let mut next_node: Option<LNodeId> = None;
            if iter_pos < nodes.len() {
                let nxt = nodes[iter_pos];
                iter_pos += 1;
                next_node = Some(nxt);
                let next_segment = a.node(nxt).id;

                outgoing_list[a.node(cur).id as usize].push(next_segment);
                incoming_count_list[next_segment as usize] += 1;
            }

            // Update segment's layer information
            segment_list[current_segment].last_layer = layer_index;
            segment_list[current_segment].index_in_last_layer = index_in_layer;
            index_in_layer += 1;

            // Cycle nodes
            previous_node = Some(cur);
            current_node = next_node;
        }

        layer_index += 1;
    }
}

/// Splits the segment before the
/// given node, moving all nodes from it onward into a new segment with the
/// given id (appended to `segment_list`; its index is returned).
fn split_segment(
    a: &mut LGraphArena,
    segment_list: &mut Vec<LinearSegment>,
    seg_idx: usize,
    node: LNodeId,
    new_id: i32,
) -> usize {
    let node_index = segment_list[seg_idx]
        .nodes
        .iter()
        .position(|&n| n == node)
        .unwrap();

    let mut new_segment = LinearSegment::new(new_id);
    new_segment.nodes = segment_list[seg_idx].nodes.split_off(node_index);
    for &moved in &new_segment.nodes {
        a.node_mut(moved).id = new_id;
    }

    segment_list.push(new_segment);
    segment_list.len() - 1
}

/// Puts a node into the given
/// linear segment and checks for following parts of a long edge.
fn fill_segment(a: &mut LGraphArena, node: LNodeId, segment: &mut LinearSegment) -> bool {
    let node_type = a.node(node).node_type;

    if a.node(node).id >= 0 {
        // The node is already part of another linear segment
        return false;
    } else {
        // Add the node to the given linear segment
        a.node_mut(node).id = segment.id;
        segment.nodes.push(node);
    }

    if node_type == NodeType::LONG_EDGE || node_type == NodeType::NORTH_SOUTH_PORT {
        // Check if any of this dummy's successors can join its segment
        for source_port in a.node(node).ports.clone() {
            // the successor ports: target ports of the outgoing edges
            for edge in a.port(source_port).outgoing_edges.clone() {
                let target_port = a.edge(edge).target.unwrap();
                let target_node = a.port(target_port).node.unwrap();
                let target_node_type = a.node(target_node).node_type;

                if a.node(node).layer != a.node(target_node).layer {
                    if target_node_type == NodeType::LONG_EDGE
                        || target_node_type == NodeType::NORTH_SOUTH_PORT
                    {
                        if fill_segment(a, target_node, segment) {
                            // We just added another node to this node's
                            // linear segment. That's quite enough.
                            return true;
                        }
                    }
                }
            }
        }
    }

    true
}

// /////////////////////////////////////////////////////////////////////////////
// Unbalanced Placement

fn create_unbalanced_placement(
    a: &mut LGraphArena,
    graph: LGraphId,
    linear_segments: &[LinearSegment],
) {
    let layers: Vec<LayerId> = a.graph(graph).layers.clone();

    // index of a node's layer in the graph's layer list
    let layer_index = |a: &LGraphArena, node: LNodeId| -> usize {
        let l = a.node(node).layer.unwrap();
        layers.iter().position(|&x| x == l).unwrap()
    };

    // How many nodes are currently placed in each layer
    let mut node_count = vec![0i32; layers.len()];
    // The node most recently placed in a given layer
    let mut recent_node: Vec<Option<LNodeId>> = vec![None; layers.len()];

    // Iterate through the linear segments (in proper order!) and place them
    for segment in linear_segments {
        // Determine the uppermost placement for the linear segment
        let mut uppermost_place = 0.0f64;
        for &node in &segment.nodes {
            let li = layer_index(a, node);
            node_count[li] += 1;

            // Calculate how much space to leave between the linear segment
            // and the last node of the given layer
            let mut spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_EDGE);
            if node_count[li] > 0 {
                if let Some(recent) = recent_node[li] {
                    spacing = spacings::vertical_spacing(a, recent, node);
                }
            }

            let layer_id = a.node(node).layer.unwrap();
            uppermost_place = f64::max(uppermost_place, a.layer(layer_id).size.y + spacing);
        }

        // Apply the uppermost placement to all elements
        for &node in &segment.nodes {
            // Set the node position
            a.node_mut(node).pos.y = uppermost_place + a.node(node).margin.top;

            // Adjust layer size
            let layer_id = a.node(node).layer.unwrap();
            let n = a.node(node);
            let new_size = uppermost_place + n.margin.top + n.size.y + n.margin.bottom;
            a.layer_mut(layer_id).size.y = new_size;

            let li = layer_index(a, node);
            recent_node[li] = Some(node);
        }
    }
}

// /////////////////////////////////////////////////////////////////////////////
// Balanced Placement

/// Definition of balancing modes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    ForwPendulum,
    BackwPendulum,
    Rubber,
}

fn balance_placement(a: &mut LGraphArena, graph: LGraphId, linear_segments: &mut [LinearSegment]) {
    let deflection_dampening: f64 = a
        .graph(graph)
        .properties
        .get(&lopts::NODE_PLACEMENT_LINEAR_SEGMENTS_DEFLECTION_DAMPENING);

    // Determine a suitable number of pendulum iterations
    let thoroughness: i32 = a.graph(graph).properties.get(&lopts::THOROUGHNESS);
    let mut pendulum_iters = PENDULUM_ITERS;
    let mut final_iters = FINAL_ITERS;
    let threshold = THRESHOLD_FACTOR / thoroughness as f64;

    // Iterate the balancing
    let mut ready = false;
    let mut mode = Mode::ForwPendulum;
    let mut last_total_deflection = i32::MAX as f64;
    loop {
        // Calculate force for every linear segment
        let incoming = mode != Mode::BackwPendulum;
        let outgoing = mode != Mode::ForwPendulum;
        let mut total_deflection = 0f64;
        for idx in 0..linear_segments.len() {
            linear_segments[idx].ref_segment = None;
            calc_deflection(
                a,
                linear_segments,
                idx,
                incoming,
                outgoing,
                deflection_dampening,
            );
            total_deflection += linear_segments[idx].deflection.abs();
        }

        // Merge linear segments to form regions
        loop {
            let merged = merge_regions(a, graph, linear_segments);
            if !merged {
                break;
            }
        }

        // Move the nodes according to the deflection value of their region
        for idx in 0..linear_segments.len() {
            let deflection = linear_segments[region(linear_segments, idx)].deflection;
            if deflection != 0.0 {
                for &node in &linear_segments[idx].nodes {
                    a.node_mut(node).pos.y += deflection;
                }
            }
        }

        // Update the balancing mode
        if mode == Mode::ForwPendulum || mode == Mode::BackwPendulum {
            pendulum_iters -= 1;
            if pendulum_iters <= 0
                && (total_deflection < last_total_deflection || -pendulum_iters > thoroughness)
            {
                mode = Mode::Rubber;
                last_total_deflection = i32::MAX as f64;
            } else if mode == Mode::ForwPendulum {
                mode = Mode::BackwPendulum;
                last_total_deflection = total_deflection;
            } else {
                mode = Mode::ForwPendulum;
                last_total_deflection = total_deflection;
            }
        } else {
            ready = total_deflection >= last_total_deflection
                || last_total_deflection - total_deflection < threshold;
            last_total_deflection = total_deflection;
            if ready {
                final_iters -= 1;
            }
        }
        if ready && final_iters <= 0 {
            break;
        }
    }
}

fn calc_deflection(
    a: &LGraphArena,
    linear_segments: &mut [LinearSegment],
    seg_idx: usize,
    incoming: bool,
    outgoing: bool,
    deflection_dampening: f64,
) {
    let mut segment_deflection = 0f64;
    let mut node_weight_sum = 0i32;
    for node_pos in 0..linear_segments[seg_idx].nodes.len() {
        let node = linear_segments[seg_idx].nodes[node_pos];
        let mut node_deflection = 0f64;
        let mut edge_weight_sum = 0i32;
        let input_prio = if incoming {
            a.node(node).properties.get(&INPUT_PRIO)
        } else {
            i32::MIN
        };
        let output_prio = if outgoing {
            a.node(node).properties.get(&OUTPUT_PRIO)
        } else {
            i32::MIN
        };
        let min_prio = input_prio.max(output_prio);

        // Calculate force for every port/edge
        for &port in &a.node(node).ports {
            let portpos = a.node(node).pos.y + a.port(port).pos.y + a.port(port).anchor.y;
            if outgoing {
                for &edge in &a.port(port).outgoing_edges {
                    let other_port = a.edge(edge).target.unwrap();
                    let other_node = a.port(other_port).node.unwrap();
                    if seg_idx as i32 != a.node(other_node).id {
                        let other_prio = i32::max(
                            a.node(other_node).properties.get(&INPUT_PRIO),
                            a.node(other_node).properties.get(&OUTPUT_PRIO),
                        );
                        let prio: i32 =
                            a.edge(edge).properties.get(&lopts::PRIORITY_STRAIGHTNESS);
                        if prio >= min_prio && prio >= other_prio {
                            node_deflection += a.node(other_node).pos.y
                                + a.port(other_port).pos.y
                                + a.port(other_port).anchor.y
                                - portpos;
                            edge_weight_sum += 1;
                        }
                    }
                }
            }

            if incoming {
                for &edge in &a.port(port).incoming_edges {
                    let other_port = a.edge(edge).source.unwrap();
                    let other_node = a.port(other_port).node.unwrap();
                    if seg_idx as i32 != a.node(other_node).id {
                        let other_prio = i32::max(
                            a.node(other_node).properties.get(&INPUT_PRIO),
                            a.node(other_node).properties.get(&OUTPUT_PRIO),
                        );
                        let prio: i32 =
                            a.edge(edge).properties.get(&lopts::PRIORITY_STRAIGHTNESS);
                        if prio >= min_prio && prio >= other_prio {
                            node_deflection += a.node(other_node).pos.y
                                + a.port(other_port).pos.y
                                + a.port(other_port).anchor.y
                                - portpos;
                            edge_weight_sum += 1;
                        }
                    }
                }
            }
        }

        // Avoid division by zero
        if edge_weight_sum > 0 {
            segment_deflection += node_deflection / edge_weight_sum as f64;
            node_weight_sum += 1;
        }
    }
    if node_weight_sum > 0 {
        linear_segments[seg_idx].deflection =
            deflection_dampening * segment_deflection / node_weight_sum as f64;
        linear_segments[seg_idx].weight = node_weight_sum;
    } else {
        linear_segments[seg_idx].deflection = 0.0;
        linear_segments[seg_idx].weight = 0;
    }
}

fn merge_regions(
    a: &LGraphArena,
    graph: LGraphId,
    linear_segments: &mut [LinearSegment],
) -> bool {
    let mut changed = false;
    let node_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_NODE_NODE);
    let threshold = OVERLAP_DETECT * node_spacing;
    for &layer in &a.graph(graph).layers {
        let nodes = &a.layer(layer).nodes;

        // Get the first node
        let mut node1 = nodes[0];
        let mut region1 = region(linear_segments, a.node(node1).id as usize);

        // While there are still nodes following the current node
        for &node2 in &nodes[1..] {
            // Test whether nodes have different regions
            let region2 = region(linear_segments, a.node(node2).id as usize);

            if region1 != region2 {
                // Calculate how much space is allowed between the nodes
                let spacing = spacings::vertical_spacing(a, node1, node2);

                let node1_extent = a.node(node1).pos.y
                    + a.node(node1).size.y
                    + a.node(node1).margin.bottom
                    + linear_segments[region1].deflection
                    + spacing;
                let node2_extent = a.node(node2).pos.y - a.node(node2).margin.top
                    + linear_segments[region2].deflection;

                // Test if the nodes are overlapping
                if node1_extent > node2_extent + threshold {
                    // Merge the first region under the second top level segment
                    let weight_sum = linear_segments[region1].weight
                        + linear_segments[region2].weight;
                    debug_assert!(weight_sum > 0);
                    linear_segments[region2].deflection = (linear_segments[region2].weight as f64
                        * linear_segments[region2].deflection
                        + linear_segments[region1].weight as f64
                            * linear_segments[region1].deflection)
                        / weight_sum as f64;
                    linear_segments[region2].weight = weight_sum;
                    linear_segments[region1].ref_segment = Some(region2);
                    changed = true;
                }
            }

            node1 = node2;
            region1 = region2;
        }
    }
    changed
}

// /////////////////////////////////////////////////////////////////////////////
// Post Processing for Correction

fn post_process(a: &mut LGraphArena, _graph: LGraphId, linear_segments: &[LinearSegment]) {
    // process each linear segment independently
    for segment in linear_segments {
        let mut min_room_above = i32::MAX as f64;
        let mut min_room_below = i32::MAX as f64;

        for &node in &segment.nodes {
            let index = a.node_index_in_layer(node);
            let layer_nodes = &a.layer(a.node(node).layer.unwrap()).nodes;

            // determine the amount by which the linear segment can be moved
            // up without overlap
            let room_above = if index > 0 {
                let neighbor = layer_nodes[(index - 1) as usize];
                let spacing = spacings::vertical_spacing(a, node, neighbor);
                a.node(node).pos.y
                    - a.node(node).margin.top
                    - (a.node(neighbor).pos.y
                        + a.node(neighbor).size.y
                        + a.node(neighbor).margin.bottom
                        + spacing)
            } else {
                a.node(node).pos.y - a.node(node).margin.top
            };
            min_room_above = f64::min(room_above, min_room_above);

            // determine the amount by which the linear segment can be moved
            // down without overlap
            let room_below = if (index as usize) < layer_nodes.len() - 1 {
                let neighbor = layer_nodes[(index + 1) as usize];
                let spacing = spacings::vertical_spacing(a, node, neighbor);
                a.node(neighbor).pos.y
                    - a.node(neighbor).margin.top
                    - (a.node(node).pos.y + a.node(node).size.y + a.node(node).margin.bottom
                        + spacing)
            } else {
                2.0 * a.node(node).pos.y
            };
            min_room_below = f64::min(room_below, min_room_below);
        }

        let mut min_displacement = i32::MAX as f64;
        let mut found_place = false;

        // determine the minimal displacement that would make one incoming
        // edge straight
        let first_node = segment.nodes[0];
        for &target in &a.node(first_node).ports {
            let pos = a.node(first_node).pos.y + a.port(target).pos.y + a.port(target).anchor.y;
            for &edge in &a.port(target).incoming_edges {
                let source = a.edge(edge).source.unwrap();
                let source_node = a.port(source).node.unwrap();
                let d = a.node(source_node).pos.y + a.port(source).pos.y + a.port(source).anchor.y
                    - pos;
                if d.abs() < min_displacement.abs()
                    && d.abs() < (if d < 0.0 { min_room_above } else { min_room_below })
                {
                    min_displacement = d;
                    found_place = true;
                }
            }
        }

        // determine the minimal displacement that would make one outgoing
        // edge straight
        let last_node = segment.nodes[segment.nodes.len() - 1];
        for &source in &a.node(last_node).ports {
            let pos = a.node(last_node).pos.y + a.port(source).pos.y + a.port(source).anchor.y;
            for &edge in &a.port(source).outgoing_edges {
                let target = a.edge(edge).target.unwrap();
                let target_node = a.port(target).node.unwrap();
                let d = a.node(target_node).pos.y + a.port(target).pos.y + a.port(target).anchor.y
                    - pos;
                if d.abs() < min_displacement.abs()
                    && d.abs() < (if d < 0.0 { min_room_above } else { min_room_below })
                {
                    min_displacement = d;
                    found_place = true;
                }
            }
        }

        // if such a displacement could be found, apply it to the whole segment
        if found_place && min_displacement != 0.0 {
            for &node in &segment.nodes {
                a.node_mut(node).pos.y += min_displacement;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alg_layered::graph::LEdgeId;

    fn make_node(a: &mut LGraphArena, layer: LayerId, height: f64) -> LNodeId {
        let g = a.layer(layer).graph.unwrap();
        let n = a.create_node(g);
        a.node_mut(n).graph = None;
        a.node_mut(n).size.y = height;
        a.node_set_layer(n, Some(layer));
        n
    }

    fn connect(a: &mut LGraphArena, source: LNodeId, target: LNodeId) -> LEdgeId {
        let sp = a.create_port();
        a.port_set_node(sp, Some(source));
        let tp = a.create_port();
        a.port_set_node(tp, Some(target));
        let e = a.create_edge();
        a.edge_set_source(e, Some(sp));
        a.edge_set_target(e, Some(tp));
        e
    }

    /// Two connected nodes in two layers must be vertically aligned (their
    /// linear segments are balanced onto the same y position).
    #[test]
    fn two_connected_nodes_align() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let l0 = a.create_layer(g);
        let l1 = a.create_layer(g);
        a.graph_mut(g).layers.push(l0);
        a.graph_mut(g).layers.push(l1);
        let n0 = make_node(&mut a, l0, 30.0);
        let n1 = make_node(&mut a, l1, 30.0);
        connect(&mut a, n0, n1);

        process(&mut a, g).unwrap();

        assert_eq!(a.node(n0).pos.y, a.node(n1).pos.y);
    }

    /// Nodes in the same layer must not overlap after placement.
    #[test]
    fn no_overlaps_in_layer() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let l0 = a.create_layer(g);
        let l1 = a.create_layer(g);
        a.graph_mut(g).layers.push(l0);
        a.graph_mut(g).layers.push(l1);
        let n0 = make_node(&mut a, l0, 30.0);
        let n1 = make_node(&mut a, l1, 20.0);
        let n2 = make_node(&mut a, l1, 20.0);
        connect(&mut a, n0, n1);
        connect(&mut a, n0, n2);

        process(&mut a, g).unwrap();

        assert!(a.node(n1).pos.y + a.node(n1).size.y <= a.node(n2).pos.y);
    }
}
