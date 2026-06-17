//! Removes LONG_EDGE dummies, joining the edge
//! fragments back together.

use crate::core::options::PortSide;
use crate::graph::math::KVector;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let add_unnecessary_bendpoints: bool =
        a.graph(graph).properties.get(&lopts::UNNECESSARY_BENDPOINTS);

    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            if a.node(node).node_type == NodeType::LONG_EDGE {
                join_at(a, node, add_unnecessary_bendpoints);
                // remove from layer
                a.layer_mut(layer).nodes.retain(|&n| n != node);
                a.node_mut(node).layer = None;
            }
        }
    }
    Ok(())
}

/// The static `LongEdgeJoiner.joinAt` (also used by other processors).
pub fn join_at(a: &mut LGraphArena, long_edge_dummy: LNodeId, add_unnecessary_bendpoints: bool) {
    let west_port = a
        .node_ports_on_side(long_edge_dummy, PortSide::WEST)
        .into_iter()
        .next()
        .expect("long edge dummy without west port");
    let east_port = a
        .node_ports_on_side(long_edge_dummy, PortSide::EAST)
        .into_iter()
        .next()
        .expect("long edge dummy without east port");

    let mut edge_count = a.port(west_port).incoming_edges.len();

    // absolute anchor of the dummy's first port
    let first_port = a.node(long_edge_dummy).ports[0];
    let node_pos = a.node(long_edge_dummy).pos;
    let unnecessary_bendpoint = KVector::new(
        node_pos.x + a.port(first_port).pos.x + a.port(first_port).anchor.x,
        node_pos.y + a.port(first_port).pos.y + a.port(first_port).anchor.y,
    );

    while edge_count > 0 {
        edge_count -= 1;
        let surviving_edge = a.port(west_port).incoming_edges[0];
        let dropped_edge = a.port(east_port).outgoing_edges[0];

        let dropped_target = a.edge(dropped_edge).target.unwrap();
        let dropped_edge_list_index = a
            .port(dropped_target)
            .incoming_edges
            .iter()
            .position(|&e| e == dropped_edge)
            .unwrap();
        a.edge_set_target_at_index(surviving_edge, Some(dropped_target), dropped_edge_list_index);

        a.edge_set_source(dropped_edge, None);
        a.edge_set_target(dropped_edge, None);

        if add_unnecessary_bendpoints {
            a.edge_mut(surviving_edge).bend_points.add_last(unnecessary_bendpoint);
        }
        let dropped_bends = a.edge(dropped_edge).bend_points.clone();
        a.edge_mut(surviving_edge).bend_points.0.extend(dropped_bends.0);

        let dropped_labels = a.edge(dropped_edge).labels.clone();
        a.edge_mut(surviving_edge).labels.extend(dropped_labels);

        // junction points: materialize the default (empty chain) on both
        // edges, then append in place
        let mut surviving_jps = a
            .edge(surviving_edge)
            .properties
            .get(&lopts::JUNCTION_POINTS);
        let dropped_jps = a.edge(dropped_edge).properties.get(&lopts::JUNCTION_POINTS);
        surviving_jps.0.extend(dropped_jps.0);
        a.edge(surviving_edge)
            .properties
            .set(&lopts::JUNCTION_POINTS, surviving_jps);
    }
}
