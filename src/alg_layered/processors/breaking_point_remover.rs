//! Removes BREAKING_POINT dummies and
//! transfers the split edge route back to the original edge.

use crate::core::options::EdgeRouting;
use crate::graph::math::KVectorChain;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::{BPInfo, BPInfoId, BPInfoStore};
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let store = a.graph(graph).properties.try_get(&iprops::BP_INFO_STORE).unwrap_or_default();
    let edge_routing: EdgeRouting = a.graph(graph).properties.get(&lopts::EDGE_ROUTING);

    let layers = a.graph(graph).layers.clone();
    for l in layers {
        let nodes = a.layer(l).nodes.clone();
        for node in nodes {
            if is_end(a, &store, node) {
                let bpi = bpi_of(a, node).unwrap();
                if store.get(bpi).next.is_none() {
                    remove(a, &store, bpi, edge_routing);
                }
            }
        }
    }
    Ok(())
}

fn bpi_of(a: &LGraphArena, n: LNodeId) -> Option<BPInfoId> {
    a.node(n).properties.try_get(&iprops::BREAKING_POINT_INFO)
}

fn is_end(a: &LGraphArena, store: &BPInfoStore, n: LNodeId) -> bool {
    bpi_of(a, n).map(|id| store.get(id).end == n).unwrap_or(false)
}

fn remove(a: &mut LGraphArena, store: &BPInfoStore, bpi_id: BPInfoId, edge_routing: EdgeRouting) {
    let bpi: BPInfo = store.get(bpi_id).clone();

    let mut new_bends = KVectorChain::default();

    match edge_routing {
        EdgeRouting::SPLINES => {
            // gather spline segment indices (into SPLINE_SEGMENT_STORE) and chains
            let s1: Vec<i32> =
                a.edge(bpi.node_start_edge).properties.try_get(&iprops::SPLINE_ROUTE_START).unwrap_or_default();
            let s2: Vec<i32> =
                a.edge(bpi.start_end_edge).properties.try_get(&iprops::SPLINE_ROUTE_START).unwrap_or_default();
            let s3: Vec<i32> =
                a.edge(bpi.original_edge).properties.try_get(&iprops::SPLINE_ROUTE_START).unwrap_or_default();
            let e1: Vec<_> =
                a.edge(bpi.node_start_edge).properties.try_get(&iprops::SPLINE_EDGE_CHAIN).unwrap_or_default();
            let e2: Vec<_> =
                a.edge(bpi.start_end_edge).properties.try_get(&iprops::SPLINE_EDGE_CHAIN).unwrap_or_default();
            let e3: Vec<_> =
                a.edge(bpi.original_edge).properties.try_get(&iprops::SPLINE_EDGE_CHAIN).unwrap_or_default();

            // mark s2 segments as inverse-order in the shared segment store
            if let Some(mut seg_store) = a.graph(graph_of(a, bpi.original_edge)).properties.try_get(&iprops::SPLINE_SEGMENT_STORE) {
                for &si in &s2 {
                    seg_store.segments[si as usize].inverse_order = true;
                }
                a.graph(graph_of(a, bpi.original_edge))
                    .properties
                    .set(&iprops::SPLINE_SEGMENT_STORE, seg_store);
            }

            let mut joined_segments: Vec<i32> = Vec::new();
            joined_segments.extend(s1);
            joined_segments.extend(s2.iter().rev().copied());
            joined_segments.extend(s3);

            let mut joined_edges = Vec::new();
            joined_edges.extend(e1);
            joined_edges.extend(e2.iter().rev().copied());
            joined_edges.extend(e3);

            a.edge(bpi.original_edge).properties.set(&iprops::SPLINE_ROUTE_START, joined_segments);
            a.edge(bpi.original_edge).properties.set(&iprops::SPLINE_EDGE_CHAIN, joined_edges);
            a.edge(bpi.original_edge).properties.set(&iprops::SPLINE_SURVIVING_EDGE, bpi.original_edge);

            a.edge(bpi.node_start_edge).properties.unset(&iprops::SPLINE_ROUTE_START);
            a.edge(bpi.node_start_edge).properties.unset(&iprops::SPLINE_EDGE_CHAIN);
            a.edge(bpi.start_end_edge).properties.unset(&iprops::SPLINE_ROUTE_START);
            a.edge(bpi.start_end_edge).properties.unset(&iprops::SPLINE_EDGE_CHAIN);
        }
        EdgeRouting::POLYLINE => {
            new_bends.0.extend(a.edge(bpi.node_start_edge).bend_points.0.iter().copied());
            new_bends.add_last(a.node(bpi.start).pos);
            new_bends.0.extend(a.edge(bpi.start_end_edge).bend_points.0.iter().rev().copied());
            new_bends.add_last(a.node(bpi.end).pos);
            new_bends.0.extend(a.edge(bpi.original_edge).bend_points.0.iter().copied());
        }
        _ => {
            // ORTHOGONAL
            new_bends.0.extend(a.edge(bpi.node_start_edge).bend_points.0.iter().copied());
            new_bends.0.extend(a.edge(bpi.start_end_edge).bend_points.0.iter().rev().copied());
            new_bends.0.extend(a.edge(bpi.original_edge).bend_points.0.iter().copied());
        }
    }

    // restore original edge
    a.edge_mut(bpi.original_edge).bend_points.0.clear();
    a.edge_mut(bpi.original_edge).bend_points.0.extend(new_bends.0);
    let node_start_source = a.edge(bpi.node_start_edge).source;
    a.edge_set_source(bpi.original_edge, node_start_source);

    // collect junction points (order can be arbitrary)
    let jp1 = a.edge(bpi.node_start_edge).properties.try_get(&lopts::JUNCTION_POINTS);
    let jp2 = a.edge(bpi.start_end_edge).properties.try_get(&lopts::JUNCTION_POINTS);
    let jp3 = a.edge(bpi.original_edge).properties.try_get(&lopts::JUNCTION_POINTS);
    if jp1.is_some() || jp2.is_some() || jp3.is_some() {
        let mut new_jps = KVectorChain::default();
        if let Some(c) = jp3 {
            new_jps.0.extend(c.0);
        }
        if let Some(c) = jp2 {
            new_jps.0.extend(c.0);
        }
        if let Some(c) = jp1 {
            new_jps.0.extend(c.0);
        }
        a.edge(bpi.original_edge).properties.set(&lopts::JUNCTION_POINTS, new_jps);
    }

    // remove all the dummy stuff
    a.edge_set_source(bpi.start_end_edge, None);
    a.edge_set_target(bpi.start_end_edge, None);
    a.edge_set_source(bpi.node_start_edge, None);
    a.edge_set_target(bpi.node_start_edge, None);
    a.node_set_layer(bpi.end, None);
    a.node_set_layer(bpi.start, None);

    if let Some(prev) = bpi.prev {
        remove(a, store, prev, edge_routing);
    }
}

fn graph_of(a: &LGraphArena, edge: crate::alg_layered::graph::LEdgeId) -> LGraphId {
    let node = a.edge_source_node(edge);
    a.node_graph(node)
}
