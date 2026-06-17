//! Turns
//! tentative spline routes (computed by the `SplineEdgeRouter`) into concrete
//! bezier control points that become the bend points of the edges.

use crate::core::options::PortSide;
use crate::graph::math::{ElkRectangle, KVector};

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::{GraphCompactionStrategy, SplineRoutingMode};
use crate::alg_layered::p5edges::splines::nub_spline::NubSpline;
use crate::alg_layered::p5edges::splines::splines_math;
use crate::alg_layered::p5edges::splines::{
    abs_anchor, is_qualified_as_starting_node, SegIdx, SplineSegmentStore, SPLINE_DIMENSION,
};

/// Avoiding magic number problems.
const ONE_HALF: f64 = 0.5;
/// `NODE_TO_STRAIGHTENING_CP_GAP`.
pub const NODE_TO_STRAIGHTENING_CP_GAP: f64 = 5.0;
/// `SLOPPY_CENTER_CP_MULTIPLIER`.
const SLOPPY_CENTER_CP_MULTIPLIER: f64 = 0.4;

struct Ctx {
    edge_edge_spacing: f64,
    edge_node_spacing: f64,
    spline_routing_mode: SplineRoutingMode,
    compaction_strategy: GraphCompactionStrategy,
}

/// `FinalSplineBendpointsCalculator.process`.
pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let ctx = Ctx {
        edge_edge_spacing: a.graph(graph).properties.get(&lopts::SPACING_EDGE_EDGE_BETWEEN_LAYERS),
        edge_node_spacing: a.graph(graph).properties.get(&lopts::SPACING_EDGE_NODE_BETWEEN_LAYERS),
        spline_routing_mode: a.graph(graph).properties.get(&lopts::EDGE_ROUTING_SPLINES_MODE),
        compaction_strategy: a
            .graph(graph)
            .properties
            .get(&lopts::COMPACTION_POST_COMPACTION_STRATEGY),
    };

    // assign indices to nodes to efficiently query neighbors within the same layer
    index_nodes_per_layer(a, graph);

    // the spline segments computed by the SplineEdgeRouter (absent if the
    // graph has no layers, in which case no SPLINE_ROUTE_START edges are found)
    let mut store: SplineSegmentStore = a
        .graph(graph)
        .properties
        .try_get(&iprops::SPLINE_SEGMENT_STORE)
        .unwrap_or_default();
    a.graph(graph).properties.unset(&iprops::SPLINE_SEGMENT_STORE);

    // collect all edges that represent the first segment of a spline
    let mut start_edges: Vec<LEdgeId> = Vec::new();
    for &layer in &a.graph(graph).layers.clone() {
        for &node in &a.layer(layer).nodes.clone() {
            for e in a.node_outgoing_edges(node) {
                if !a.edge_is_self_loop(e)
                    && a.edge(e).properties.has(&iprops::SPLINE_ROUTE_START)
                {
                    start_edges.push(e);
                }
            }
        }
    }

    // first determine the NUB control points
    for &e in &start_edges {
        let spline: Vec<i32> = a
            .edge(e)
            .properties
            .try_get(&iprops::SPLINE_ROUTE_START)
            .unwrap();
        for s in spline {
            calculate_control_points(a, &mut store, s as SegIdx, &ctx);
        }
        a.edge(e).properties.unset(&iprops::SPLINE_ROUTE_START);
    }

    // ... then convert them to bezier splines
    for &e in &start_edges {
        // may be null
        let surviving_edge = a.edge(e).properties.try_get(&iprops::SPLINE_SURVIVING_EDGE);
        let edge_chain: Vec<LEdgeId> =
            a.edge(e).properties.try_get(&iprops::SPLINE_EDGE_CHAIN).unwrap();
        calculate_bezier_bend_points(a, &edge_chain, surviving_edge, &ctx)?;
        // clear property
        a.edge(e).properties.unset(&iprops::SPLINE_EDGE_CHAIN);
    }

    Ok(())
}

/// `indexNodesPerLayer`.
fn index_nodes_per_layer(a: &mut LGraphArena, graph: LGraphId) {
    for &layer in &a.graph(graph).layers.clone() {
        let nodes = a.layer(layer).nodes.clone();
        for (index, node) in nodes.into_iter().enumerate() {
            a.node_mut(node).id = index as i32;
        }
    }
}

/// `calculateControlPoints`: dispatches the calculation of NUB control
/// points for the passed segment.
fn calculate_control_points(a: &mut LGraphArena, store: &mut SplineSegmentStore, seg: SegIdx, ctx: &Ctx) {
    // with hyperedges it can happen that this method is called multiple times for the
    // same segment
    if store.segments[seg].handled {
        return;
    }
    store.segments[seg].handled = true;

    let edges = store.segments[seg].edges.clone();
    let is_hyper_edge = store.segments[seg].is_hyper_edge();
    for edge in edges {
        if store.segments[seg].is_straight && !is_hyper_edge {
            calculate_control_points_straight(a, store, seg);
            continue;
        }

        // Remember that the edge itself is not necessarily valid at this point
        //  (it may have been removed by the long edge joiner, for instance)
        let ei = store.segments[seg].edge_information(edge).clone();
        // inverted ports are handled in the same way
        if ei.inverted_left || ei.inverted_right {
            calculate_control_points_inverted_edge(a, store, seg, edge, ctx);
            continue;
        }

        // to compute sloppy control points at least one of the two nodes connected by
        // the segment must be a 'normal' node; for hyperedges sloppy routing is not
        // possible
        let sloppy = ctx.spline_routing_mode == SplineRoutingMode::SLOPPY
            && (ei.normal_source_node || ei.normal_target_node)
            && segment_allows_sloppy_routing(a, store, seg, ctx)
            && !is_hyper_edge;
        if sloppy {
            calculate_control_points_sloppy(a, store, seg, edge);
        } else {
            calculate_control_points_conservative(a, store, seg, edge, ctx);
        }
    }

    if store.segments[seg].inverse_order {
        for edge in store.segments[seg].edges.clone() {
            a.edge_mut(edge).bend_points.0.reverse();
        }
    }
}

/// `calculateControlPointsStraight`: adds a single control point halfway
/// between the source and target layer.
fn calculate_control_points_straight(a: &mut LGraphArena, store: &SplineSegmentStore, seg: SegIdx) {
    let segment = &store.segments[seg];
    let x_start_pos = segment.bounding_box.x;
    let x_end_pos = segment.bounding_box.x + segment.bounding_box.width;
    let halfway = KVector::new(
        x_start_pos + (x_end_pos - x_start_pos) / 2.0,
        segment.center_control_point_y,
    );
    let edge = segment.edges[0];
    a.edge_mut(edge).bend_points.0.push(halfway);
}

/// `calculateControlPointsInvertedEdge`.
fn calculate_control_points_inverted_edge(
    a: &mut LGraphArena,
    store: &SplineSegmentStore,
    seg: SegIdx,
    edge: LEdgeId,
    ctx: &Ctx,
) {
    let containing_segment = &store.segments[seg];
    let start_x_pos = containing_segment.bounding_box.x;
    let end_x_pos = containing_segment.bounding_box.x + containing_segment.bounding_box.width;

    let ei = containing_segment.edge_information(edge);
    let y_source_anchor = ei.start_y;
    let y_target_anchor = ei.end_y;

    // compute the desired control points
    let source_straight_cp = if ei.inverted_left {
        KVector::new(end_x_pos, y_source_anchor)
    } else {
        KVector::new(start_x_pos, y_source_anchor)
    };
    let target_straight_cp = if ei.inverted_right {
        KVector::new(start_x_pos, y_target_anchor)
    } else {
        KVector::new(end_x_pos, y_target_anchor)
    };

    // the center position is the same for all edges but depends on sloppiness of the
    // routing
    let mut center_x_pos = start_x_pos;
    if !containing_segment.is_west_of_initial_layer {
        center_x_pos += ctx.edge_node_spacing;
    }
    center_x_pos +=
        containing_segment.x_delta + containing_segment.rank as f64 * ctx.edge_edge_spacing;

    let source_vertical_cp = KVector::new(center_x_pos, y_source_anchor);
    let target_vertical_cp = KVector::new(center_x_pos, y_target_anchor);

    // add control points to the edge's bendpoints
    let is_hyperedge = containing_segment.edges.len() > 1;
    let center = KVector::new(center_x_pos, containing_segment.center_control_point_y);

    let bps = &mut a.edge_mut(edge).bend_points.0;
    bps.push(source_straight_cp);
    bps.push(source_vertical_cp);
    if is_hyperedge {
        // add an additional center control point to assert that the hyperedge segments
        // share a part of their route
        bps.push(center);
    }
    bps.push(target_vertical_cp);
    bps.push(target_straight_cp);
}

/// `calculateControlPointsConservative`.
fn calculate_control_points_conservative(
    a: &mut LGraphArena,
    store: &SplineSegmentStore,
    seg: SegIdx,
    edge: LEdgeId,
    ctx: &Ctx,
) {
    let containing_segment = &store.segments[seg];
    let start_x_pos = containing_segment.bounding_box.x;
    let end_x_pos = containing_segment.bounding_box.x + containing_segment.bounding_box.width;

    let ei = containing_segment.edge_information(edge);
    let y_source_anchor = ei.start_y;
    let y_target_anchor = ei.end_y;

    // Calculate bend points to draw inner layer segments straight to prevent
    // intersections with big nodes
    let source_straight_cp = KVector::new(start_x_pos, y_source_anchor);
    let target_straight_cp = KVector::new(end_x_pos, y_target_anchor);

    let mut center_x_pos = start_x_pos;
    if !containing_segment.is_west_of_initial_layer {
        center_x_pos += ctx.edge_node_spacing;
    }
    center_x_pos +=
        containing_segment.x_delta + containing_segment.rank as f64 * ctx.edge_edge_spacing;
    let source_vertical_cp = KVector::new(center_x_pos, y_source_anchor);
    let target_vertical_cp = KVector::new(center_x_pos, y_target_anchor);

    // Traditional four control points (plus an extra center control point for
    // hyperedges)
    let is_hyperedge = containing_segment.edges.len() > 1;
    let center = KVector::new(center_x_pos, containing_segment.center_control_point_y);

    let bps = &mut a.edge_mut(edge).bend_points.0;
    bps.push(source_straight_cp);
    bps.push(source_vertical_cp);
    if is_hyperedge {
        bps.push(center);
    }
    bps.push(target_vertical_cp);
    bps.push(target_straight_cp);
}

/// `calculateControlPointsSloppy`.
fn calculate_control_points_sloppy(
    a: &mut LGraphArena,
    store: &SplineSegmentStore,
    seg: SegIdx,
    edge: LEdgeId,
) {
    let containing_segment = &store.segments[seg];
    let ei = containing_segment.edge_information(edge);
    debug_assert!(ei.normal_source_node || ei.normal_target_node);

    let start_x_pos = containing_segment.bounding_box.x;
    let end_x_pos = containing_segment.bounding_box.x + containing_segment.bounding_box.width;

    let y_source_anchor = ei.start_y;
    let y_target_anchor = ei.end_y;
    let edge_points_downwards = y_source_anchor < y_target_anchor;

    // pre-compute a number of coordinates that we might use as control points
    let source_straight_cp = KVector::new(start_x_pos, y_source_anchor);
    let target_straight_cp = KVector::new(end_x_pos, y_target_anchor);
    let center_x_pos = (start_x_pos + end_x_pos) / 2.0;
    let source_vertical_cp = KVector::new(center_x_pos, y_source_anchor);
    let target_vertical_cp = KVector::new(center_x_pos, y_target_anchor);

    // evaluate if a rather direct curve is possible
    let center_y_pos = compute_sloppy_center_y(a, edge, y_source_anchor, y_target_anchor);
    let v1 = abs_anchor(a, containing_segment.source_port.unwrap());
    let v2 = KVector::new(center_x_pos, center_y_pos);
    let v3 = abs_anchor(a, containing_segment.target_port.unwrap());
    // approx will be an array holding two values, the zeroth of which represents the
    // center of the curve
    let approx = splines_math::approximate_bezier_segment(2, &[v1, v2, v3]);

    let mut short_cut_source = false;
    let src = a.port(containing_segment.source_port.unwrap()).node;
    // when graph wrapping is activated, it can happen that the 'src' node doesn't exist
    // anymore (the same goes for the node's layer)
    if let Some(src) = src {
        if a.node(src).layer.is_some() && ei.normal_source_node {
            // possible intersections must only be checked if there are nodes with which
            // the edge could potentially intersect
            let src_layer_nodes = &a.layer(a.node(src).layer.unwrap()).nodes;
            let src_id = a.node(src).id;
            let need_to_check_src = (edge_points_downwards
                && src_id < src_layer_nodes.len() as i32 - 1)
                || (!edge_points_downwards && src_id > 0);

            if !need_to_check_src {
                short_cut_source = true;
            } else {
                // check within src's layer
                let mut neighbor_index = src_id;
                if edge_points_downwards {
                    neighbor_index += 1;
                } else {
                    neighbor_index -= 1;
                }
                let neighbor = src_layer_nodes[neighbor_index as usize];
                let bx = node_to_bounding_box(a, neighbor);
                short_cut_source = !(splines_math::rect_intersects_line(&bx, v1, approx[0])
                    || splines_math::rect_contains_line(&bx, v1, approx[0]));
            }
        }
    }

    let mut short_cut_target = false;
    let tgt = a.port(containing_segment.target_port.unwrap()).node;
    // see comment above
    if let Some(tgt) = tgt {
        if a.node(tgt).layer.is_some() && ei.normal_target_node {
            let tgt_layer_nodes = &a.layer(a.node(tgt).layer.unwrap()).nodes;
            let tgt_id = a.node(tgt).id;
            let need_to_check_tgt = (edge_points_downwards && tgt_id > 0)
                || (!edge_points_downwards && tgt_id < tgt_layer_nodes.len() as i32 - 1);

            // tgt's layer
            if !need_to_check_tgt {
                short_cut_target = true;
            } else {
                let mut neighbor_index = tgt_id;
                if edge_points_downwards {
                    neighbor_index -= 1;
                } else {
                    neighbor_index += 1;
                }
                let neighbor = tgt_layer_nodes[neighbor_index as usize];
                let bx = node_to_bounding_box(a, neighbor);
                short_cut_target = !(splines_math::rect_intersects_line(&bx, approx[0], v3)
                    || splines_math::rect_contains_line(&bx, approx[0], v3));
            }
        }
    }

    // now add the control points
    let bps = &mut a.edge_mut(edge).bend_points.0;
    if short_cut_source && short_cut_target {
        bps.push(v2);
    }
    if !short_cut_source {
        bps.push(source_straight_cp);
        bps.push(source_vertical_cp);
    }
    if !short_cut_target {
        bps.push(target_vertical_cp);
        bps.push(target_straight_cp);
    }
}

/// `nodeToBoundingBox`.
fn node_to_bounding_box(a: &LGraphArena, node: LNodeId) -> ElkRectangle {
    let n = a.node(node);
    let pos = n.pos;
    let size = n.size;
    let m = n.margin;
    ElkRectangle::new(
        pos.x - m.left,
        pos.y - m.top,
        size.x + m.horizontal(),
        size.y + m.vertical(),
    )
}

/// `Math.signum(double)`.
fn java_signum(x: f64) -> f64 {
    if x == 0.0 || x.is_nan() {
        x
    } else if x > 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// Computes the sloppy center Y coordinate. Note the quirks: the *source* is
/// checked for null while the *target*'s node is inspected, and vice versa.
fn compute_sloppy_center_y(
    a: &LGraphArena,
    edge: LEdgeId,
    y_source_anchor: f64,
    y_target_anchor: f64,
) -> f64 {
    let mut indegree: i32 = 0;
    let mut outdegree: i32 = 0;
    // Count all the incoming and outgoing edges; the edge's source and target may have
    // been set to null by now (in case they were connected to a dummy node beforehand),
    // in which case the degree is assumed to be one
    if a.edge(edge).source.is_some() {
        let target_node = a.port(a.edge(edge).target.unwrap()).node.unwrap();
        for &port in &a.node(target_node).ports {
            indegree += a.port(port).incoming_edges.len() as i32;
        }
    } else {
        indegree = 1;
    }
    if a.edge(edge).target.is_some() {
        let source_node = a.port(a.edge(edge).source.unwrap()).node.unwrap();
        for &port in &a.node(source_node).ports {
            outdegree += a.port(port).outgoing_edges.len() as i32;
        }
    } else {
        outdegree = 1;
    }
    let degree_diff = java_signum((outdegree - indegree) as f64) as i32;

    ((y_target_anchor + y_source_anchor) / 2.0)
        + (y_target_anchor - y_source_anchor) * (SLOPPY_CENTER_CP_MULTIPLIER * degree_diff as f64)
}

/// `calculateBezierBendPoints`: collects all NUB control points computed
/// for a spline segment chain and converts them to bezier control points.
fn calculate_bezier_bend_points(
    a: &mut LGraphArena,
    edge_chain: &[LEdgeId],
    surviving_edge: Option<LEdgeId>,
    ctx: &Ctx,
) -> Result<(), String> {
    if edge_chain.is_empty() {
        return Ok(());
    }

    // in this chain we will put all NURBS control points.
    let mut all_cp: Vec<KVector> = Vec::new();
    // add the computed bendpoints to the specified edge (default to the first edge in
    // the edge chain)
    let edge = surviving_edge.unwrap_or(edge_chain[0]);
    // Process the source end of the edge-chain.
    let source_port = a.edge(edge).source.unwrap();

    // edge must be the first edge of a chain of edges
    let source_node_type = a.node(a.port(source_port).node.unwrap()).node_type;
    if !is_qualified_as_starting_node(source_node_type) {
        return Err(
            "The target node of the edge must be a normal node or a northSouthPort.".to_string()
        );
    }

    // add the source as the very first control point.
    all_cp.push(abs_anchor(a, source_port));

    // add an additional control point if the source port is a north or south port
    let source_side = a.port(source_port).side;
    if source_side == PortSide::NORTH || source_side == PortSide::SOUTH {
        let y: f64 = a
            .port(source_port)
            .properties
            .try_get(&iprops::SPLINE_NS_PORT_Y_COORD)
            .expect("SPLINE_NS_PORT_Y_COORD not set on north/south port");
        let north_south_cp = KVector::new(abs_anchor(a, source_port).x, y);
        all_cp.push(north_south_cp);
    }

    // copy the calculated control points for all spline segments, possibly adding
    // additional control points halfway between computed ones
    let mut last_cp: Option<KVector> = None;
    let mut add_mid_point = false;
    for &current_edge in edge_chain {
        // read the stored bend-points for vertical segments
        let current_bend_points = a.edge(current_edge).bend_points.clone();

        if !current_bend_points.is_empty() {
            // add a CP in the middle of the straight segment between two vertical
            // segments to get a more straight horizontal segment
            if add_mid_point {
                let mut halfway = last_cp.unwrap();
                halfway.add(current_bend_points.first());
                halfway.scale(ONE_HALF);
                all_cp.push(halfway);
                add_mid_point = false;
            } else {
                add_mid_point = true;
            }
            last_cp = Some(current_bend_points.last());
            all_cp.extend(current_bend_points.iter());
            a.edge_mut(current_edge).bend_points.0.clear();
        }
    }

    // finalize the spline
    let target_port = a.edge(edge).target.unwrap();

    // again, add an additional control point if the target port is a north or south port
    let target_side = a.port(target_port).side;
    if target_side == PortSide::NORTH || target_side == PortSide::SOUTH {
        let y: f64 = a
            .port(target_port)
            .properties
            .try_get(&iprops::SPLINE_NS_PORT_Y_COORD)
            .expect("SPLINE_NS_PORT_Y_COORD not set on north/south port");
        let north_south_cp = KVector::new(abs_anchor(a, target_port).x, y);
        all_cp.push(north_south_cp);
    }

    // finish with the target as last control point.
    all_cp.push(abs_anchor(a, target_port));

    // insert straightening control points (if desired)
    if ctx.spline_routing_mode == SplineRoutingMode::CONSERVATIVE {
        insert_straightening_control_points(&mut all_cp, source_side, target_side);
    }

    // convert list of NUB control points to bezier control points
    let mut nub_spline = NubSpline::new_clamped(SPLINE_DIMENSION, all_cp);
    // ... and set them as bendpoints of the edge
    let bezier_cp = nub_spline.get_bezier_cp();
    a.edge_mut(edge).bend_points.0.extend(bezier_cp);

    Ok(())
}

/// `insertStraighteningControlPoints`.
fn insert_straightening_control_points(
    all_cps: &mut Vec<KVector>,
    src_port_side: PortSide,
    tgt_port_side: PortSide,
) {
    // beginning
    let first = all_cps[0];
    let second = all_cps[1];

    let mut v = KVector::from_angle(splines_math::port_side_to_direction(src_port_side));
    v.scale(NODE_TO_STRAIGHTENING_CP_GAP);
    let mut v2 = second;
    v2.sub(first);
    let mut straighten_beginning = KVector::new(abs_min(v.x, v2.x), abs_min(v.y, v2.y));
    straighten_beginning.add(first);

    all_cps.insert(1, straighten_beginning);

    // ending
    let last = all_cps[all_cps.len() - 1];
    let second_last = all_cps[all_cps.len() - 2];

    let mut v = KVector::from_angle(splines_math::port_side_to_direction(tgt_port_side));
    v.scale(NODE_TO_STRAIGHTENING_CP_GAP);
    let mut v2 = second_last;
    v2.sub(last);
    let mut straighten_ending = KVector::new(abs_min(v.x, v2.x), abs_min(v.y, v2.y));
    straighten_ending.add(last);

    let pos = all_cps.len() - 1;
    all_cps.insert(pos, straighten_ending);
}

/// `absMin`.
fn abs_min(d1: f64, d2: f64) -> f64 {
    if d1.abs() < d2.abs() {
        d1
    } else {
        d2
    }
}

/// `segmentAllowsSloppyRouting`.
fn segment_allows_sloppy_routing(
    a: &LGraphArena,
    store: &SplineSegmentStore,
    seg: SegIdx,
    ctx: &Ctx,
) -> bool {
    // only check this if one dimensional compaction is applied
    if ctx.compaction_strategy == GraphCompactionStrategy::NONE {
        return true;
    }

    let segment = &store.segments[seg];
    let start_x_pos = segment.bounding_box.x;
    let end_x_pos = segment.bounding_box.x + segment.bounding_box.width;

    if segment.initial_segment {
        let n = segment.source_node.unwrap();
        let t = segment_node_distance_threshold(a, n);
        let node_segment_distance = start_x_pos - (a.node(n).pos.x + a.node(n).size.x);
        if node_segment_distance > t {
            return false;
        }
    }
    if segment.last_segment {
        let n = segment.target_node.unwrap();
        let t = segment_node_distance_threshold(a, n);
        let node_segment_distance = a.node(n).pos.x - end_x_pos;
        if node_segment_distance > t {
            return false;
        }
    }

    true
}

/// `segmentNodeDistanceThreshold`.
fn segment_node_distance_threshold(a: &LGraphArena, n: LNodeId) -> f64 {
    a.layer(a.node(n).layer.unwrap()).size.x - a.node(n).size.x / 2.0
}
