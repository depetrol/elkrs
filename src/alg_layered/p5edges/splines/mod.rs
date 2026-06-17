//!
//! All segments live in a
//! [`SplineSegmentStore`] that is stored as a graph property
//! (`iprops::SPLINE_SEGMENT_STORE`); edges reference segments by index.

pub mod nub_spline;
pub mod splines_math;

use std::collections::HashMap;
use std::collections::VecDeque;

use crate::core::javacompat::JavaRandom;
use crate::core::options::PortSide;
use crate::graph::math::{ElkRectangle, KVector};
use crate::graph::properties::{JavaCloneable, JavaString};
use indexmap::IndexSet;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId, LayerId, LEdgeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::SplineRoutingMode;

const MAX_VERTICAL_DIFF_FOR_STRAIGHT: f64 = 0.2;
pub const SPLINE_DIMENSION: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SideToProcess {
    Left,
    Right,
}

/// Index of a [`SplineSegment`] in the [`SplineSegmentStore`].
pub type SegIdx = usize;
/// Index of a [`Dependency`] in the [`SplineSegmentStore`].
pub type DepIdx = usize;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct EdgeInformation {
    pub start_y: f64,
    pub end_y: f64,
    pub normal_source_node: bool,
    pub normal_target_node: bool,
    pub inverted_left: bool,
    pub inverted_right: bool,
}

/// A dependency pointing from segment A
/// to segment B means that A must lay left of B.
#[derive(Clone, Debug, PartialEq)]
pub struct Dependency {
    pub source: SegIdx,
    pub target: SegIdx,
    pub weight: i32,
}

/// Data part of a spline segment; the per-layer-pair dependency lists store
/// indices into the segment store's dependency arena.
#[derive(Clone, Debug, PartialEq)]
pub struct SplineSegment {
    pub handled: bool,
    /// left ports (a set; iteration over it is order-insensitive)
    pub left_ports: Vec<LPortId>,
    /// right ports
    pub right_ports: Vec<LPortId>,
    pub outgoing: Vec<DepIdx>,
    pub incoming: Vec<DepIdx>,
    pub mark: i32,
    pub inweight: i32,
    pub outweight: i32,
    pub rank: i32,
    /// edges (a set; only the first-element iteration order matters,
    /// which is never output-relevant)
    pub edges: Vec<LEdgeId>,
    pub is_straight: bool,
    pub bounding_box: ElkRectangle,
    pub is_west_of_initial_layer: bool,
    pub x_delta: f64,
    pub source_port: Option<LPortId>,
    pub target_port: Option<LPortId>,
    pub initial_segment: bool,
    pub last_segment: bool,
    pub source_node: Option<LNodeId>,
    pub target_node: Option<LNodeId>,
    pub inverse_order: bool,
    pub hyper_edge_top_y_pos: f64,
    pub hyper_edge_bottom_y_pos: f64,
    pub center_control_point_y: f64,
    /// edge information map (keyed by edge)
    pub edge_information: Vec<(LEdgeId, EdgeInformation)>,
}

impl Default for SplineSegment {
    fn default() -> Self {
        SplineSegment {
            handled: false,
            left_ports: Vec::new(),
            right_ports: Vec::new(),
            outgoing: Vec::new(),
            incoming: Vec::new(),
            mark: 0,
            inweight: 0,
            outweight: 0,
            rank: 0,
            edges: Vec::new(),
            is_straight: false,
            bounding_box: ElkRectangle::default(),
            is_west_of_initial_layer: false,
            x_delta: 0.0,
            source_port: None,
            target_port: None,
            initial_segment: false,
            last_segment: false,
            source_node: None,
            target_node: None,
            inverse_order: false,
            hyper_edge_top_y_pos: 0.0,
            hyper_edge_bottom_y_pos: 0.0,
            center_control_point_y: 0.0,
            edge_information: Vec::new(),
        }
    }
}

// Hyper-Edge constants
const HYPEREDGE_POS_OUTER_RATE: f64 = 0.9;
const HYPEREDGE_POS_MID_RATE: f64 = 1.0 - HYPEREDGE_POS_OUTER_RATE;
const ONE_HALF: f64 = 0.5;

impl SplineSegment {
    /// Constructor for a 1:n hyper-edge.
    fn new_hyper(
        a: &LGraphArena,
        single_port: LPortId,
        edges: &[(SideToProcess, LEdgeId)],
        source_side: SideToProcess,
    ) -> SplineSegment {
        let mut seg = SplineSegment::default();

        match source_side {
            SideToProcess::Left => seg.add_left_port(single_port),
            SideToProcess::Right => seg.add_right_port(single_port),
        }

        let mut y_min_pos_of_target = f64::INFINITY;
        let mut y_max_pos_of_target = f64::NEG_INFINITY;

        for &(side, edge) in edges {
            let mut tgt_port = a.edge(edge).source.unwrap();
            if tgt_port == single_port {
                tgt_port = a.edge(edge).target.unwrap();
            }

            match side {
                SideToProcess::Left => seg.add_left_port(tgt_port),
                SideToProcess::Right => seg.add_right_port(tgt_port),
            }

            let y_pos_of_target = anchor_y(a, tgt_port);
            y_min_pos_of_target = f64::min(y_min_pos_of_target, y_pos_of_target);
            y_max_pos_of_target = f64::max(y_max_pos_of_target, y_pos_of_target);
        }

        let y_pos_of_single_side = anchor_y(a, single_port);

        // set the relevant positions
        seg.set_relevant_positions(y_pos_of_single_side, y_min_pos_of_target, y_max_pos_of_target);

        for &(_, edge) in edges {
            seg.add_edge(a, edge);
        }
        seg.is_straight = false;

        seg
    }

    /// Constructor for a hyper-edge consisting of a single edge.
    fn new_single(
        a: &LGraphArena,
        edge: LEdgeId,
        source_side: SideToProcess,
        target_side: SideToProcess,
    ) -> SplineSegment {
        let mut seg = SplineSegment::default();

        // adding left and right ports
        match source_side {
            SideToProcess::Left => seg.add_left_port(a.edge(edge).source.unwrap()),
            SideToProcess::Right => seg.add_right_port(a.edge(edge).source.unwrap()),
        }
        match target_side {
            SideToProcess::Left => seg.add_left_port(a.edge(edge).target.unwrap()),
            SideToProcess::Right => seg.add_right_port(a.edge(edge).target.unwrap()),
        }

        // adding the edges
        seg.add_edge(a, edge);

        // setting relevant positions
        let source_y = anchor_y(a, a.edge(edge).source.unwrap());
        let target_y = anchor_y(a, a.edge(edge).target.unwrap());
        seg.set_relevant_positions(source_y, target_y, target_y);

        seg.is_straight = is_straight(source_y, target_y);

        seg
    }

    /// Set-add semantics for the port sets.
    fn add_left_port(&mut self, port: LPortId) {
        if !self.left_ports.contains(&port) {
            self.left_ports.push(port);
        }
    }

    fn add_right_port(&mut self, port: LPortId) {
        if !self.right_ports.contains(&port) {
            self.right_ports.push(port);
        }
    }

    fn add_edge(&mut self, a: &LGraphArena, edge: LEdgeId) {
        if !self.edges.contains(&edge) {
            self.edges.push(edge);
        }

        let source = a.edge(edge).source.unwrap();
        let target = a.edge(edge).target.unwrap();
        let ei = EdgeInformation {
            start_y: anchor_y(a, source),
            end_y: anchor_y(a, target),
            normal_source_node: is_normal_node(a.node(a.port(source).node.unwrap()).node_type),
            normal_target_node: is_normal_node(a.node(a.port(target).node.unwrap()).node_type),
            inverted_left: a.port(source).side == PortSide::WEST,
            inverted_right: a.port(target).side == PortSide::EAST,
        };
        self.edge_information.push((edge, ei));
    }

    pub fn is_hyper_edge(&self) -> bool {
        self.edges.len() > 1
    }

    /// `getEdgeInformation` lookup (the `edgeInformation` map).
    pub fn edge_information(&self, edge: LEdgeId) -> &EdgeInformation {
        &self
            .edge_information
            .iter()
            .find(|(e, _)| *e == edge)
            .expect("edge information missing")
            .1
    }

    fn set_relevant_positions(&mut self, source_y: f64, target_y_min: f64, target_y_max: f64) {
        self.bounding_box.y = f64::min(source_y, target_y_min);
        self.bounding_box.height = f64::max(source_y, target_y_max) - self.bounding_box.y;

        if source_y < target_y_min {
            // source lays below all target ports
            self.center_control_point_y = ONE_HALF * (source_y + target_y_min);
            self.hyper_edge_top_y_pos =
                HYPEREDGE_POS_MID_RATE * self.center_control_point_y + HYPEREDGE_POS_OUTER_RATE * source_y;
            self.hyper_edge_bottom_y_pos = HYPEREDGE_POS_MID_RATE * self.center_control_point_y
                + HYPEREDGE_POS_OUTER_RATE * target_y_min;
        } else {
            // source lays above all target ports
            self.center_control_point_y = ONE_HALF * (source_y + target_y_max);
            self.hyper_edge_top_y_pos = HYPEREDGE_POS_MID_RATE * self.center_control_point_y
                + HYPEREDGE_POS_OUTER_RATE * target_y_max;
            self.hyper_edge_bottom_y_pos = HYPEREDGE_POS_MID_RATE * self.center_control_point_y
                + HYPEREDGE_POS_OUTER_RATE * source_y;
        }
    }
}

/// Arena for all spline segments and dependencies of one layout run; stored as
/// a graph property between the edge routing phase and the final bendpoints
/// calculator.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SplineSegmentStore {
    pub segments: Vec<SplineSegment>,
    pub deps: Vec<Dependency>,
}

impl JavaString for SplineSegmentStore {
    fn java_string(&self) -> String {
        format!("{self:?}")
    }
}
impl JavaCloneable for SplineSegmentStore {
    const CLONEABLE: bool = false;
}

impl SplineSegmentStore {
    /// Creates a dependency and registers it with both endpoints.
    fn create_dependency(&mut self, source: SegIdx, target: SegIdx, weight: i32) {
        let dep = self.deps.len();
        self.deps.push(Dependency { source, target, weight });
        self.segments[source].outgoing.push(dep);
        self.segments[target].incoming.push(dep);
    }
}

pub fn is_straight(first_y: f64, second_y: f64) -> bool {
    (first_y - second_y).abs() < MAX_VERTICAL_DIFF_FOR_STRAIGHT
}

pub fn is_normal_node(nt: NodeType) -> bool {
    nt == NodeType::NORMAL || nt == NodeType::BREAKING_POINT
}

pub fn is_qualified_as_starting_node(nt: NodeType) -> bool {
    nt == NodeType::NORMAL
        || nt == NodeType::NORTH_SOUTH_PORT
        || nt == NodeType::EXTERNAL_PORT
        || nt == NodeType::BREAKING_POINT
}

/// Absolute anchor position of a port.
pub(crate) fn abs_anchor(a: &LGraphArena, port: LPortId) -> KVector {
    let p = a.port(port);
    let n = a.node(p.node.unwrap());
    KVector::new(n.pos.x + p.pos.x + p.anchor.x, n.pos.y + p.pos.y + p.anchor.y)
}

/// For north/south ports the y coordinate
/// stored by the `NorthSouthPortPostprocessor` is used.
pub(crate) fn anchor_y(a: &LGraphArena, p: LPortId) -> f64 {
    let side = a.port(p).side;
    if side == PortSide::NORTH || side == PortSide::SOUTH {
        a.port(p)
            .properties
            .try_get(&iprops::SPLINE_NS_PORT_Y_COORD)
            .expect("SPLINE_NS_PORT_Y_COORD not set on north/south port")
    } else {
        abs_anchor(a, p).y
    }
}

// ---------------------------------------------------------------------------
// SplineEdgeRouter.process

pub fn process(a: &mut LGraphArena, graph: LGraphId, random: &mut JavaRandom) -> Result<(), String> {
    if a.graph(graph).layers.is_empty() {
        a.graph_mut(graph).size.x = 0.0;
        return Ok(());
    }

    // Retrieve some generic values
    let node_node_spacing: f64 =
        a.graph(graph).properties.get(&lopts::SPACING_NODE_NODE_BETWEEN_LAYERS);
    let edge_node_spacing: f64 =
        a.graph(graph).properties.get(&lopts::SPACING_EDGE_NODE_BETWEEN_LAYERS);
    let edge_edge_spacing: f64 =
        a.graph(graph).properties.get(&lopts::SPACING_EDGE_EDGE_BETWEEN_LAYERS);

    // Find out if splines should be routed thoroughly or sloppy
    let mode: SplineRoutingMode = a.graph(graph).properties.get(&lopts::EDGE_ROUTING_SPLINES_MODE);
    let sloppy_routing = mode == SplineRoutingMode::SLOPPY;
    let sloppy_layer_spacing_factor: f64 = a
        .graph(graph)
        .properties
        .get(&lopts::EDGE_ROUTING_SPLINES_SLOPPY_LAYER_SPACING_FACTOR);

    let mut store = SplineSegmentStore::default();
    let mut start_edges: Vec<LEdgeId> = Vec::new();
    let mut edge_to_segment: HashMap<LEdgeId, SegIdx> = HashMap::new();
    let mut successing_edge: HashMap<LEdgeId, LEdgeId> = HashMap::new();

    // check if the first and/or last layer are populated with external port dummies
    let layers = a.graph(graph).layers.clone();
    let first_layer = layers[0];
    let is_left_layer_external = a
        .layer(first_layer)
        .nodes
        .iter()
        .all(|&n| super::polyline::is_external_west_or_east_port(a, n));
    let last_layer = layers[layers.len() - 1];
    let is_right_layer_external = a
        .layer(last_layer)
        .nodes
        .iter()
        .all(|&n| super::polyline::is_external_west_or_east_port(a, n));

    let mut layer_iter = layers.iter();
    let mut left_layer: Option<LayerId> = None;
    let mut right_layer: Option<LayerId>;

    // initial x position
    let mut xpos = 0.0f64;
    loop {
        right_layer = layer_iter.next().copied();

        // fresh start for this pair of layers
        let (left_ports_layer, right_ports_layer, mut edges_remaining_layer) =
            clear_then_fill_mappings(
                a,
                left_layer,
                right_layer,
                &mut start_edges,
                &mut successing_edge,
            );

        // creation of the SplineSegments
        let spline_segments_layer = create_segments_and_compute_ranking(
            a,
            &mut store,
            &mut edge_to_segment,
            &left_ports_layer,
            &right_ports_layer,
            &mut edges_remaining_layer,
            random,
        )?;

        // count the number of required slots for vertical segments
        //  (edges to be drawn straight are assigned a rank but must be omitted here)
        let slot_count: i32 = spline_segments_layer
            .iter()
            .filter(|&&s| !store.segments[s].is_straight)
            .map(|&s| store.segments[s].rank + 1)
            .max()
            .unwrap_or(0);

        // the code below ensures that at least nodeNodeSpacing is preserved between a pair of
        // layers
        let mut x_segment_delta = 0.0f64;
        let mut right_layer_position = xpos;
        let is_special_left_layer =
            left_layer.is_none() || (is_left_layer_external && left_layer == Some(first_layer));
        let is_special_right_layer =
            right_layer.is_none() || (is_right_layer_external && right_layer == Some(last_layer));

        // compute horizontal positions just as for the OrthogonalEdgeRouter
        if slot_count > 0 {
            // the space between each pair of edge segments, and between nodes and edges
            let mut increment = 0.0f64;
            if left_layer.is_some() {
                increment += edge_node_spacing;
            }
            increment += (slot_count - 1) as f64 * edge_edge_spacing;
            if right_layer.is_some() {
                increment += edge_node_spacing;
            }

            // sloppy routing may want to reserve more space in-between a pair of layers
            if sloppy_routing && right_layer.is_some() {
                increment = f64::max(
                    increment,
                    compute_sloppy_spacing(
                        a,
                        right_layer.unwrap(),
                        edge_edge_spacing,
                        node_node_spacing,
                        sloppy_layer_spacing_factor,
                    ),
                );
            }

            // if we are between two layers, make sure their minimal spacing is preserved
            if increment < node_node_spacing && !is_special_left_layer && !is_special_right_layer {
                x_segment_delta = (node_node_spacing - increment) / 2.0;
                increment = node_node_spacing;
            }
            right_layer_position += increment;
        } else if !is_special_left_layer && !is_special_right_layer {
            // If all edges are straight, use the usual spacing
            right_layer_position += node_node_spacing;
        }

        // place right layer's nodes
        if let Some(right) = right_layer {
            super::orthogonal::place_nodes_horizontally(a, right, right_layer_position);
        }

        // Assign tentative start and end points to the spline segments
        for &seg in &spline_segments_layer {
            let segment = &mut store.segments[seg];
            segment.bounding_box.x = xpos;
            segment.bounding_box.width = right_layer_position - xpos;
            segment.x_delta = x_segment_delta;
            segment.is_west_of_initial_layer = left_layer.is_none();
        }

        // proceed to the next layer
        xpos = right_layer_position;
        if let Some(right) = right_layer {
            xpos += a.layer(right).size.x;
        }

        left_layer = right_layer;
        if right_layer.is_none() {
            break;
        }
    }

    // all layers have been processed, remember the spline paths for
    //  control point calculation to be done by a later intermediate processor
    for &edge in &start_edges {
        let edge_chain = get_edge_chain(&successing_edge, edge);
        a.edge(edge).properties.set(&iprops::SPLINE_EDGE_CHAIN, edge_chain.clone());

        let spline = get_spline_path(a, &mut store, &edge_to_segment, &edge_chain);
        let spline_i32: Vec<i32> = spline.iter().map(|&s| s as i32).collect();
        a.edge(edge).properties.set(&iprops::SPLINE_ROUTE_START, spline_i32);
    }

    // assign final width of the layering and thus the overall graph
    a.graph_mut(graph).size.x = xpos;

    // make the segments available to the FinalSplineBendpointsCalculator
    a.graph(graph).properties.set(&iprops::SPLINE_SEGMENT_STORE, store);

    Ok(())
}

/// Returns the segments created for the current pair of layers.
fn create_segments_and_compute_ranking(
    a: &LGraphArena,
    store: &mut SplineSegmentStore,
    edge_to_segment: &mut HashMap<LEdgeId, SegIdx>,
    left_ports_layer: &IndexSet<LPortId>,
    right_ports_layer: &IndexSet<LPortId>,
    edges_remaining_layer: &mut Vec<LEdgeId>,
    random: &mut JavaRandom,
) -> Result<Vec<SegIdx>, String> {
    let mut spline_segments_layer: Vec<SegIdx> = Vec::new();

    // create the hyperEdges having their start port on the left side.
    create_spline_segments_for_hyper_edges(
        a,
        store,
        edge_to_segment,
        left_ports_layer,
        right_ports_layer,
        SideToProcess::Left,
        true,
        edges_remaining_layer,
        &mut spline_segments_layer,
    );
    create_spline_segments_for_hyper_edges(
        a,
        store,
        edge_to_segment,
        left_ports_layer,
        right_ports_layer,
        SideToProcess::Left,
        false,
        edges_remaining_layer,
        &mut spline_segments_layer,
    );

    // create the hyperEdges having their start port on the right side.
    create_spline_segments_for_hyper_edges(
        a,
        store,
        edge_to_segment,
        left_ports_layer,
        right_ports_layer,
        SideToProcess::Right,
        true,
        edges_remaining_layer,
        &mut spline_segments_layer,
    );
    create_spline_segments_for_hyper_edges(
        a,
        store,
        edge_to_segment,
        left_ports_layer,
        right_ports_layer,
        SideToProcess::Right,
        false,
        edges_remaining_layer,
        &mut spline_segments_layer,
    );

    // remaining edges are single edges that cannot be combined with others to a hyper-edge
    create_spline_segments(
        a,
        store,
        edge_to_segment,
        edges_remaining_layer,
        left_ports_layer,
        right_ports_layer,
        &mut spline_segments_layer,
    )?;

    // Creation of the dependencies of the spline segments
    for source_idx in 0..spline_segments_layer.len() {
        for target_idx in source_idx + 1..spline_segments_layer.len() {
            create_dependency(
                a,
                store,
                spline_segments_layer[source_idx],
                spline_segments_layer[target_idx],
            );
        }
    }

    // Apply the topological numbering
    // break cycles
    break_cycles(store, &spline_segments_layer, random);

    // assign ranks to the hyper-nodes
    topological_numbering(store, &spline_segments_layer);

    Ok(spline_segments_layer)
}

/// Returns `(leftPorts, rightPorts,
/// edgesRemaining)` for the current pair of layers and updates the start edge
/// list and the successor map.
fn clear_then_fill_mappings(
    a: &LGraphArena,
    left_layer: Option<LayerId>,
    right_layer: Option<LayerId>,
    start_edges: &mut Vec<LEdgeId>,
    successing_edge: &mut HashMap<LEdgeId, LEdgeId>,
) -> (IndexSet<LPortId>, IndexSet<LPortId>, Vec<LEdgeId>) {
    let mut left_ports_layer: IndexSet<LPortId> = IndexSet::new();
    let mut right_ports_layer: IndexSet<LPortId> = IndexSet::new();
    let mut edges_remaining_layer: Vec<LEdgeId> = Vec::new();

    // iterate over all outgoing edges on the left layer.
    if let Some(left) = left_layer {
        for &node in &a.layer(left).nodes {
            for &source_port in &a.node(node).ports {
                if a.port(source_port).side != PortSide::EAST {
                    continue;
                }
                left_ports_layer.insert(source_port);

                for &edge in &a.port(source_port).outgoing_edges {
                    // Self-loops are handled in the right-layer section below.
                    if a.edge_is_self_loop(edge) {
                        continue;
                    }

                    // Add edge to set of all edges and find it's successor
                    edges_remaining_layer.push(edge);
                    find_and_add_successor(a, successing_edge, edge);

                    // Check if edge is a startingEdge
                    let source_node = a.port(a.edge(edge).source.unwrap()).node.unwrap();
                    if is_qualified_as_starting_node(a.node(source_node).node_type) {
                        start_edges.push(edge);
                    }

                    // Check port-side of target port
                    let target_port = a.edge(edge).target.unwrap();
                    let target_layer = a.node(a.port(target_port).node.unwrap()).layer;
                    if target_layer == right_layer {
                        right_ports_layer.insert(target_port);
                    } else if target_layer == left_layer {
                        left_ports_layer.insert(target_port);
                    } else {
                        // Unhandled situation. Probably there are incoming and outgoing
                        // edges on the same port. This is not supported.
                        if let Some(pos) = edges_remaining_layer.iter().position(|&e| e == edge) {
                            edges_remaining_layer.remove(pos);
                        }
                    }
                }
            }
        }
    }

    if let Some(right) = right_layer {
        for &node in &a.layer(right).nodes {
            // iterate over all outgoing edges on the right layer
            for &source_port in &a.node(node).ports {
                if a.port(source_port).side != PortSide::WEST {
                    continue;
                }
                right_ports_layer.insert(source_port);

                for &edge in &a.port(source_port).outgoing_edges {
                    // self-loops have been handled before
                    if a.edge_is_self_loop(edge) {
                        continue;
                    }

                    // Add edge to set of all edges and find it's successor
                    edges_remaining_layer.push(edge);
                    find_and_add_successor(a, successing_edge, edge);

                    // Check if edge is a startingEdge
                    let source_node = a.port(a.edge(edge).source.unwrap()).node.unwrap();
                    if is_qualified_as_starting_node(a.node(source_node).node_type) {
                        start_edges.push(edge);
                    }

                    // Check port-side of target port
                    let target_port = a.edge(edge).target.unwrap();
                    let target_layer = a.node(a.port(target_port).node.unwrap()).layer;
                    if target_layer == right_layer {
                        right_ports_layer.insert(target_port);
                    } else if target_layer == left_layer {
                        left_ports_layer.insert(target_port);
                    } else {
                        if let Some(pos) = edges_remaining_layer.iter().position(|&e| e == edge) {
                            edges_remaining_layer.remove(pos);
                        }
                    }
                }
            }
        }
    }

    (left_ports_layer, right_ports_layer, edges_remaining_layer)
}

fn compute_sloppy_spacing(
    a: &LGraphArena,
    right_layer: LayerId,
    edge_edge_spacing: f64,
    node_node_spacing: f64,
    sloppy_layer_spacing_factor: f64,
) -> f64 {
    let mut max_vert_diff = 0.0f64;
    // Iterate over the layer's nodes
    for &node in &a.layer(right_layer).nodes {
        // Calculate the maximal vertical span of output edges.
        let mut max_curr_input_y_diff = 0.0f64;
        for incoming_edge in a.node_incoming_edges(node) {
            let source_pos = abs_anchor(a, a.edge(incoming_edge).source.unwrap()).y;
            let target_pos = abs_anchor(a, a.edge(incoming_edge).target.unwrap()).y;

            max_curr_input_y_diff = f64::max(max_curr_input_y_diff, (target_pos - source_pos).abs());
        }
        max_vert_diff = f64::max(max_vert_diff, max_curr_input_y_diff);
    }

    // Determine where next layer should start based on the maximal vertical span of edges
    // between the two layers
    sloppy_layer_spacing_factor * f64::min(1.0, edge_edge_spacing / node_node_spacing) * max_vert_diff
}

fn find_and_add_successor(
    a: &LGraphArena,
    successing_edge: &mut HashMap<LEdgeId, LEdgeId>,
    edge: LEdgeId,
) {
    let target_node = a.port(a.edge(edge).target.unwrap()).node.unwrap();

    // if target node is a normal node there is no successor
    if is_normal_node(a.node(target_node).node_type) {
        return;
    }

    // otherwise take the first outgoing edge of target node
    let outgoing = a.node_outgoing_edges(target_node);
    if let Some(&succ) = outgoing.first() {
        successing_edge.insert(edge, succ);
    }
}

/// Single-edge segments for the remaining edges.
fn create_spline_segments(
    a: &LGraphArena,
    store: &mut SplineSegmentStore,
    edge_to_segment: &mut HashMap<LEdgeId, SegIdx>,
    edges: &[LEdgeId],
    left_ports: &IndexSet<LPortId>,
    right_ports: &IndexSet<LPortId>,
    hyper_edges: &mut Vec<SegIdx>,
) -> Result<(), String> {
    for &edge in edges {
        let source_port = a.edge(edge).source.unwrap();
        let source_side = if left_ports.contains(&source_port) {
            SideToProcess::Left
        } else if right_ports.contains(&source_port) {
            SideToProcess::Right
        } else {
            return Err("Source port must be in one of the port sets.".to_string());
        };

        let target_port = a.edge(edge).target.unwrap();
        let target_side = if left_ports.contains(&target_port) {
            SideToProcess::Left
        } else if right_ports.contains(&target_port) {
            SideToProcess::Right
        } else {
            return Err("Target port must be in one of the port sets.".to_string());
        };

        let seg = SplineSegment::new_single(a, edge, source_side, target_side);
        let seg_idx = store.segments.len();
        store.segments.push(seg);
        edge_to_segment.insert(edge, seg_idx);
        hyper_edges.push(seg_idx);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_spline_segments_for_hyper_edges(
    a: &LGraphArena,
    store: &mut SplineSegmentStore,
    edge_to_segment: &mut HashMap<LEdgeId, SegIdx>,
    left_ports: &IndexSet<LPortId>,
    right_ports: &IndexSet<LPortId>,
    side_to_process: SideToProcess,
    reversed: bool,
    edges_remaining: &mut Vec<LEdgeId>,
    hyper_edges: &mut Vec<SegIdx>,
) {
    let ports_to_process = match side_to_process {
        SideToProcess::Left => left_ports,
        SideToProcess::Right => right_ports,
    };

    // Iterate through all ports on the side to process.
    for &single_port in ports_to_process {
        let single_port_position = abs_anchor(a, single_port).y;
        let mut up_edges: Vec<(SideToProcess, LEdgeId)> = Vec::new();
        let mut down_edges: Vec<(SideToProcess, LEdgeId)> = Vec::new();

        // Find edges we could construct a hyper-edge from. If the edge is in the
        // edgesRemaining set, there is no hyper-edge that represents this edge.
        for edge in a.port_connected_edges(single_port) {
            if a.edge(edge).properties.get(&iprops::REVERSED) != reversed {
                continue;
            }
            if edges_remaining.contains(&edge) {
                // find the target port
                let target_port = if a.edge(edge).target == Some(single_port) {
                    a.edge(edge).source.unwrap()
                } else {
                    a.edge(edge).target.unwrap()
                };

                // check if this edge should get drawn as a straight edge
                let target_port_position = abs_anchor(a, target_port).y;
                if is_straight(target_port_position, single_port_position) {
                    continue;
                }

                // add the edge to the correct set of up/down-edges
                if target_port_position < single_port_position {
                    if left_ports.contains(&target_port) {
                        up_edges.push((SideToProcess::Left, edge));
                    } else {
                        up_edges.push((SideToProcess::Right, edge));
                    }
                } else if left_ports.contains(&target_port) {
                    down_edges.push((SideToProcess::Left, edge));
                } else {
                    down_edges.push((SideToProcess::Right, edge));
                }
            }
        }

        // Create some hyper edges.
        // We are creating only hyper-edges that have more than one real edge.
        if up_edges.len() > 1 {
            let seg = SplineSegment::new_hyper(a, single_port, &up_edges, side_to_process);
            let seg_idx = store.segments.len();
            store.segments.push(seg);
            for &(_, e) in &up_edges {
                edge_to_segment.insert(e, seg_idx);
            }
            hyper_edges.push(seg_idx);
            for &(_, e) in &up_edges {
                if let Some(pos) = edges_remaining.iter().position(|&x| x == e) {
                    edges_remaining.remove(pos);
                }
            }
        }
        if down_edges.len() > 1 {
            let seg = SplineSegment::new_hyper(a, single_port, &down_edges, side_to_process);
            let seg_idx = store.segments.len();
            store.segments.push(seg);
            for &(_, e) in &down_edges {
                edge_to_segment.insert(e, seg_idx);
            }
            hyper_edges.push(seg_idx);
            for &(_, e) in &down_edges {
                if let Some(pos) = edges_remaining.iter().position(|&x| x == e) {
                    edges_remaining.remove(pos);
                }
            }
        }
    }
}

/// Calculates the "must lay left of" dependency for two spline segments.
fn create_dependency(a: &LGraphArena, store: &mut SplineSegmentStore, edge0: SegIdx, edge1: SegIdx) {
    if store.segments[edge0].hyper_edge_top_y_pos > store.segments[edge1].hyper_edge_bottom_y_pos
        || store.segments[edge1].hyper_edge_top_y_pos > store.segments[edge0].hyper_edge_bottom_y_pos
    {
        // the two hyper-edges do not share a vertical segment
        return;
    }
    let mut edge0_counter: i32 = 0;
    let mut edge1_counter: i32 = 0;

    for &port in &store.segments[edge0].right_ports {
        if splines_math::is_between(
            abs_anchor(a, port).y,
            store.segments[edge1].hyper_edge_top_y_pos,
            store.segments[edge1].hyper_edge_bottom_y_pos,
        ) {
            edge0_counter += 1;
        }
    }
    for &port in &store.segments[edge0].left_ports {
        if splines_math::is_between(
            abs_anchor(a, port).y,
            store.segments[edge1].hyper_edge_top_y_pos,
            store.segments[edge1].hyper_edge_bottom_y_pos,
        ) {
            edge0_counter -= 1;
        }
    }
    for &port in &store.segments[edge1].right_ports {
        if splines_math::is_between(
            abs_anchor(a, port).y,
            store.segments[edge0].hyper_edge_top_y_pos,
            store.segments[edge0].hyper_edge_bottom_y_pos,
        ) {
            edge1_counter += 1;
        }
    }
    for &port in &store.segments[edge1].left_ports {
        if splines_math::is_between(
            abs_anchor(a, port).y,
            store.segments[edge0].hyper_edge_top_y_pos,
            store.segments[edge0].hyper_edge_bottom_y_pos,
        ) {
            edge1_counter -= 1;
        }
    }

    if edge0_counter < edge1_counter {
        // edge0 should lay left of edge1
        store.create_dependency(edge0, edge1, edge1_counter - edge0_counter);
    } else if edge1_counter < edge0_counter {
        // edge0 should lay right of edge1
        store.create_dependency(edge1, edge0, edge0_counter - edge1_counter);
    } else {
        // in either ordering there would be the same number of crossings
        store.create_dependency(edge1, edge0, 0);
        store.create_dependency(edge0, edge1, 0);
    }
}

// ---------------------------------------------------------------------------
// Cycle Breaking

fn break_cycles(store: &mut SplineSegmentStore, edges: &[SegIdx], random: &mut JavaRandom) {
    let mut sources: VecDeque<SegIdx> = VecDeque::new();
    let mut sinks: VecDeque<SegIdx> = VecDeque::new();

    // initialize values for the algorithm
    let mut next_mark = -1;
    for &edge in edges {
        store.segments[edge].mark = next_mark;
        next_mark -= 1;
        let outweight: i32 = store.segments[edge]
            .outgoing
            .iter()
            .map(|&d| store.deps[d].weight)
            .sum();
        let inweight: i32 = store.segments[edge]
            .incoming
            .iter()
            .map(|&d| store.deps[d].weight)
            .sum();

        store.segments[edge].inweight = inweight;
        store.segments[edge].outweight = outweight;

        if outweight == 0 {
            sinks.push_back(edge);
        } else if inweight == 0 {
            sources.push_back(edge);
        }
    }

    // assign marks to all nodes, ignore dependencies of weight zero
    let mut unprocessed: IndexSet<SegIdx> = edges.iter().copied().collect();
    let mark_base = edges.len() as i32;
    let mut next_left = mark_base + 1;
    let mut next_right = mark_base - 1;
    let mut max_edges: Vec<SegIdx> = Vec::new();

    while !unprocessed.is_empty() {
        while let Some(sink) = sinks.pop_front() {
            unprocessed.shift_remove(&sink);
            store.segments[sink].mark = next_right;
            next_right -= 1;
            update_neighbors(store, sink, &mut sources, &mut sinks);
        }

        while let Some(source) = sources.pop_front() {
            unprocessed.shift_remove(&source);
            store.segments[source].mark = next_left;
            next_left += 1;
            update_neighbors(store, source, &mut sources, &mut sinks);
        }

        let mut max_outflow = i32::MIN;
        for &edge in unprocessed.iter() {
            let outflow = store.segments[edge].outweight - store.segments[edge].inweight;
            if outflow >= max_outflow {
                if outflow > max_outflow {
                    max_edges.clear();
                    max_outflow = outflow;
                }
                max_edges.push(edge);
            }
        }

        if !max_edges.is_empty() {
            // if there are multiple SplineHyperEdges with maximal outflow, select one randomly
            let max_edge = max_edges[random.next_int_bound(max_edges.len() as i32) as usize];
            unprocessed.shift_remove(&max_edge);
            store.segments[max_edge].mark = next_left;
            next_left += 1;
            update_neighbors(store, max_edge, &mut sources, &mut sinks);
            max_edges.clear();
        }
    }

    // shift ranks that are left of the mark base
    let shift_base = edges.len() as i32 + 1;
    for &edge in edges {
        if store.segments[edge].mark < mark_base {
            store.segments[edge].mark += shift_base;
        }
    }

    // process edges that point left: remove those of zero weight, reverse the others
    for &source in edges {
        let mut i = 0;
        while i < store.segments[source].outgoing.len() {
            let dependency = store.segments[source].outgoing[i];
            let target = store.deps[dependency].target;

            if store.segments[source].mark > store.segments[target].mark {
                store.segments[source].outgoing.remove(i);
                if let Some(pos) =
                    store.segments[target].incoming.iter().position(|&d| d == dependency)
                {
                    store.segments[target].incoming.remove(pos);
                }

                if store.deps[dependency].weight > 0 {
                    store.deps[dependency].source = target;
                    store.segments[target].outgoing.push(dependency);
                    store.deps[dependency].target = source;
                    store.segments[source].incoming.push(dependency);
                }
            } else {
                i += 1;
            }
        }
    }
}

fn update_neighbors(
    store: &mut SplineSegmentStore,
    edge: SegIdx,
    sources: &mut VecDeque<SegIdx>,
    sinks: &mut VecDeque<SegIdx>,
) {
    // process following edges
    for dep in store.segments[edge].outgoing.clone() {
        let target = store.deps[dep].target;
        let weight = store.deps[dep].weight;
        if store.segments[target].mark < 0 && weight > 0 {
            store.segments[target].inweight -= weight;
            if store.segments[target].inweight <= 0 && store.segments[target].outweight > 0 {
                sources.push_back(target);
            }
        }
    }

    // process preceding edges
    for dep in store.segments[edge].incoming.clone() {
        let source = store.deps[dep].source;
        let weight = store.deps[dep].weight;
        if store.segments[source].mark < 0 && weight > 0 {
            store.segments[source].outweight -= weight;
            if store.segments[source].outweight <= 0 && store.segments[source].inweight > 0 {
                sinks.push_back(source);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Topological Ordering

fn topological_numbering(store: &mut SplineSegmentStore, edges: &[SegIdx]) {
    // determine sources, targets, incoming count and outgoing count; targets are only
    // added to the list if they only connect westward ports
    let mut sources: VecDeque<SegIdx> = VecDeque::new();
    let mut rightward_targets: VecDeque<SegIdx> = VecDeque::new();
    for &edge in edges {
        let segment = &mut store.segments[edge];
        segment.rank = 0;
        segment.inweight = segment.incoming.len() as i32;
        segment.outweight = segment.outgoing.len() as i32;

        if segment.inweight == 0 {
            sources.push_back(edge);
        }

        if segment.outweight == 0 && segment.left_ports.is_empty() {
            rightward_targets.push_back(edge);
        }
    }

    let mut max_rank = -1;

    // assign ranks using topological numbering
    while let Some(edge) = sources.pop_front() {
        for dep in store.segments[edge].outgoing.clone() {
            let target = store.deps[dep].target;
            let new_rank = i32::max(store.segments[target].rank, store.segments[edge].rank + 1);
            store.segments[target].rank = new_rank;
            max_rank = i32::max(max_rank, new_rank);

            store.segments[target].inweight -= 1;
            if store.segments[target].inweight == 0 {
                sources.push_back(target);
            }
        }
    }

    /* Move all hyper nodes with horizontal segments only pointing rightwards
     * as far right as possible. */
    if max_rank > -1 {
        // assign all target nodes with horizontal segments pointing to the right the
        // rightmost rank
        for &edge in rightward_targets.iter() {
            store.segments[edge].rank = max_rank;
        }

        // let all other segments with horizontal segments pointing rightwards move as
        // far right as possible
        while let Some(edge) = rightward_targets.pop_front() {
            // The node only has connections to western ports
            for dep in store.segments[edge].incoming.clone() {
                let source = store.deps[dep].source;
                if !store.segments[source].left_ports.is_empty() {
                    continue;
                }

                store.segments[source].rank =
                    i32::min(store.segments[source].rank, store.segments[edge].rank - 1);

                store.segments[source].outweight -= 1;
                if store.segments[source].outweight == 0 {
                    rightward_targets.push_back(source);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience

fn get_edge_chain(successing_edge: &HashMap<LEdgeId, LEdgeId>, start: LEdgeId) -> Vec<LEdgeId> {
    let mut edge_chain = Vec::new();
    let mut current = Some(start);
    while let Some(c) = current {
        edge_chain.push(c);
        current = successing_edge.get(&c).copied();
    }
    edge_chain
}

fn get_spline_path(
    a: &LGraphArena,
    store: &mut SplineSegmentStore,
    edge_to_segment: &HashMap<LEdgeId, SegIdx>,
    edge_chain: &[LEdgeId],
) -> Vec<SegIdx> {
    let mut segment_chain: Vec<SegIdx> = Vec::new();
    for &current in edge_chain {
        let seg = *edge_to_segment.get(&current).expect("edge without spline segment");
        store.segments[seg].source_port = a.edge(current).source;
        store.segments[seg].target_port = a.edge(current).target;
        segment_chain.push(seg);
    }

    let initial_segment = segment_chain[0];
    store.segments[initial_segment].initial_segment = true;
    let initial_edge = store.segments[initial_segment].edges[0];
    store.segments[initial_segment].source_node =
        a.port(a.edge(initial_edge).source.unwrap()).node;

    let last_segment = segment_chain[segment_chain.len() - 1];
    store.segments[last_segment].last_segment = true;
    let last_edge = store.segments[last_segment].edges[0];
    store.segments[last_segment].target_node = a.port(a.edge(last_edge).target.unwrap()).node;

    segment_chain
}
