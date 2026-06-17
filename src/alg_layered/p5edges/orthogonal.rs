//!
//! Edge routing implementation that creates orthogonal bend points.
//!
//! Precondition: the graph has a proper layering with assigned node and port
//! positions; the size of each layer is correctly set. Postcondition: each
//! node is assigned a horizontal coordinate; the bend points of each edge are
//! set; the width of the whole graph is set.

use crate::core::javacompat::JavaRandom;
use crate::core::options::{Alignment, PortSide};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;

use super::direction::RoutingDirection;
use super::orthogonal_routing_generator::OrthogonalRoutingGenerator;

/// `OrthogonalEdgeRouter.process`. The random number generator stands in
/// for the `InternalProperties.RANDOM` graph property (used by the
/// hyperedge cycle detector).
pub fn process(a: &mut LGraphArena, graph: LGraphId, random: &mut JavaRandom) -> Result<(), String> {
    // Retrieve some generic values
    let node_node_spacing: f64 =
        a.graph(graph).properties.get(&lopts::SPACING_NODE_NODE_BETWEEN_LAYERS);
    let edge_edge_spacing: f64 =
        a.graph(graph).properties.get(&lopts::SPACING_EDGE_EDGE_BETWEEN_LAYERS);
    let edge_node_spacing: f64 =
        a.graph(graph).properties.get(&lopts::SPACING_EDGE_NODE_BETWEEN_LAYERS);

    // Prepare for iteration!
    let mut routing_generator =
        OrthogonalRoutingGenerator::new(RoutingDirection::WestToEast, edge_edge_spacing, "phase5");
    // the x position is accumulated in a float!
    let mut xpos: f32 = 0.0;
    let layers = a.graph(graph).layers.clone();
    let mut layer_iter = layers.iter();
    let mut next_index: i32 = 0;
    let mut left_layer: Option<LayerId> = None;
    let mut left_layer_nodes: Option<Vec<LNodeId>> = None;
    let mut left_layer_index: i32 = -1;

    // Iterate!
    loop {
        // Fetch the next layer, if any
        let right_layer: Option<LayerId> = layer_iter.next().copied();
        let right_layer_nodes: Option<Vec<LNodeId>> =
            right_layer.map(|layer| a.layer(layer).nodes.clone());
        let right_layer_index = if right_layer.is_some() {
            let index = next_index;
            next_index += 1;
            index
        } else {
            next_index - 1
        };

        // Place the left layer's nodes, if any
        if let Some(left) = left_layer {
            place_nodes_horizontally(a, left, xpos as f64);
            xpos = (xpos as f64 + a.layer(left).size.x) as f32;
        }

        // Route edges between the two layers
        let start_pos: f64 =
            if left_layer.is_none() { xpos as f64 } else { xpos as f64 + edge_node_spacing };
        let slots_count = routing_generator.route_edges(
            a,
            left_layer_nodes.as_deref(),
            left_layer_index,
            right_layer_nodes.as_deref(),
            start_pos,
            random,
        );

        let is_left_layer_external = left_layer.is_none()
            || left_layer_nodes
                .as_ref()
                .unwrap()
                .iter()
                .all(|&node| is_external_west_or_east_port(a, node));
        let is_right_layer_external = right_layer.is_none()
            || right_layer_nodes
                .as_ref()
                .unwrap()
                .iter()
                .all(|&node| is_external_west_or_east_port(a, node));

        if slots_count > 0 {
            // Compute routing area's width
            let mut routing_width = (slots_count - 1) as f64 * edge_edge_spacing;

            if left_layer.is_some() {
                routing_width += edge_node_spacing;
            }

            if right_layer.is_some() {
                routing_width += edge_node_spacing;
            }

            // If we are between two layers, make sure their minimal spacing is preserved
            if routing_width < node_node_spacing
                && !is_left_layer_external
                && !is_right_layer_external
            {
                routing_width = node_node_spacing;
            }
            xpos = (xpos as f64 + routing_width) as f32;
        } else if !is_left_layer_external && !is_right_layer_external {
            // If all edges are straight, use the usual spacing
            xpos = (xpos as f64 + node_node_spacing) as f32;
        }

        left_layer = right_layer;
        left_layer_nodes = right_layer_nodes;
        left_layer_index = right_layer_index;

        if right_layer.is_none() {
            break;
        }
    }

    a.graph_mut(graph).size.x = xpos as f64;

    Ok(())
}

/// `PolylineEdgeRouter.PRED_EXTERNAL_WEST_OR_EAST_PORT`.
fn is_external_west_or_east_port(a: &LGraphArena, node: LNodeId) -> bool {
    let ext_port_side: PortSide = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
    a.node(node).node_type == NodeType::EXTERNAL_PORT
        && (ext_port_side == PortSide::WEST || ext_port_side == PortSide::EAST)
}

/// Places the nodes of the given
/// layer, aligning them based on their alignment options or port counts.
/// (Also used by the polyline and spline edge routers.)
pub(crate) fn place_nodes_horizontally(a: &mut LGraphArena, layer: LayerId, xoffset: f64) {
    // determine maximal left and right margin
    let mut max_left_margin: f64 = 0.0;
    let mut max_right_margin: f64 = 0.0;
    for &node in &a.layer(layer).nodes {
        let margin = a.node(node).margin;
        max_left_margin = if max_left_margin >= margin.left { max_left_margin } else { margin.left };
        max_right_margin =
            if max_right_margin >= margin.right { max_right_margin } else { margin.right };
    }

    let layer_size_x = a.layer(layer).size.x;
    let nodes = a.layer(layer).nodes.clone();
    for node in nodes {
        let alignment: Alignment = a.node(node).properties.get(&lopts::ALIGNMENT);
        let ratio: f64 = match alignment {
            Alignment::LEFT => 0.0,
            Alignment::RIGHT => 1.0,
            Alignment::CENTER => 0.5,
            _ => {
                // determine the number of input and output ports for the node
                let mut inports: i32 = 0;
                let mut outports: i32 = 0;
                for &port in &a.node(node).ports {
                    if !a.port(port).incoming_edges.is_empty() {
                        inports += 1;
                    }

                    if !a.port(port).outgoing_edges.is_empty() {
                        outports += 1;
                    }
                }

                // calculate node placement based on the port numbers
                if inports + outports == 0 {
                    0.5
                } else {
                    outports as f64 / (inports + outports) as f64
                }
            }
        };

        // align nodes to the layer's maximal margin
        let node_size = a.node(node).size.x;
        let mut xpos = (layer_size_x - node_size) * ratio;
        if ratio > 0.5 {
            xpos -= max_right_margin * 2.0 * (ratio - 0.5);
        } else if ratio < 0.5 {
            xpos += max_left_margin * 2.0 * (0.5 - ratio);
        }

        // consider the node's individual margin
        let left_margin = a.node(node).margin.left;
        if xpos < left_margin {
            xpos = left_margin;
        }
        let right_margin = a.node(node).margin.right;
        if xpos > layer_size_x - right_margin - node_size {
            xpos = layer_size_x - right_margin - node_size;
        }

        a.node_mut(node).pos.x = xoffset + xpos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alg_layered::graph::{LEdgeId, LNodeId, LPortId};
    use crate::graph::math::KVector;

    /// Builds a two-layer graph with two 30x30 nodes per layer and three
    /// edges, two of which cross:
    ///
    /// - layer 0: node a at y=0, node b at y=100
    /// - layer 1: node c at y=0, node d at y=100
    /// - e1: a (port at y=10) -> d (port at y=105)
    /// - e2: b (port at y=125) -> c (port at y=5)    (crosses e1)
    /// - e3: a (port at y=20) -> c (port at y=20)    (straight line)
    fn build_two_layer_graph(
        a: &mut crate::alg_layered::graph::LGraphArena,
    ) -> (LGraphId, [LEdgeId; 3], [LNodeId; 4]) {
        let g = a.create_graph();
        let layer0 = a.create_layer(g);
        let layer1 = a.create_layer(g);
        a.graph_mut(g).layers.push(layer0);
        a.graph_mut(g).layers.push(layer1);

        let make_node = |a: &mut crate::alg_layered::graph::LGraphArena, layer, y: f64| {
            let n = a.create_node(g);
            a.node_mut(n).graph = Some(g);
            a.node_mut(n).size = KVector::new(30.0, 30.0);
            a.node_mut(n).pos.y = y;
            a.node_set_layer(n, Some(layer));
            n
        };
        let na = make_node(a, layer0, 0.0);
        let nb = make_node(a, layer0, 100.0);
        let nc = make_node(a, layer1, 0.0);
        let nd = make_node(a, layer1, 100.0);

        a.layer_mut(layer0).size = KVector::new(30.0, 130.0);
        a.layer_mut(layer1).size = KVector::new(30.0, 130.0);

        let make_port = |a: &mut crate::alg_layered::graph::LGraphArena,
                         node: LNodeId,
                         side: PortSide,
                         x: f64,
                         y: f64|
         -> LPortId {
            let p = a.create_port();
            a.port_set_node(p, Some(node));
            a.port_set_side(p, side);
            a.port_mut(p).pos = KVector::new(x, y);
            p
        };
        // sources (EAST side of left layer nodes)
        let pa1 = make_port(a, na, PortSide::EAST, 30.0, 10.0);
        let pa2 = make_port(a, na, PortSide::EAST, 30.0, 20.0);
        let pb1 = make_port(a, nb, PortSide::EAST, 30.0, 25.0);
        // targets (WEST side of right layer nodes)
        let pd1 = make_port(a, nd, PortSide::WEST, 0.0, 5.0);
        let pc1 = make_port(a, nc, PortSide::WEST, 0.0, 5.0);
        let pc2 = make_port(a, nc, PortSide::WEST, 0.0, 20.0);

        let make_edge = |a: &mut crate::alg_layered::graph::LGraphArena, src: LPortId, tgt: LPortId| {
            let e = a.create_edge();
            a.edge_set_source(e, Some(src));
            a.edge_set_target(e, Some(tgt));
            e
        };
        let e1 = make_edge(a, pa1, pd1); // a@10 -> d@105
        let e2 = make_edge(a, pb1, pc1); // b@125 -> c@5
        let e3 = make_edge(a, pa2, pc2); // a@20 -> c@20 (straight)

        (g, [e1, e2, e3], [na, nb, nc, nd])
    }

    /// Hand-traced (ELK 0.11.0) with default
    /// spacings (nodeNodeBetweenLayers=20, edgeEdgeBetweenLayers=10,
    /// edgeNodeBetweenLayers=10):
    ///
    /// Segments between the layers (source side EAST, target side WEST):
    /// - s0 for e1: incoming=[10], outgoing=[105]
    /// - s1 for e3: incoming=[20], outgoing=[20]  (straight, no slot)
    /// - s2 for e2: incoming=[125], outgoing=[5]
    ///
    /// minimum segment distance = 10 => criticalConflictThreshold = 2,
    /// conflictThreshold = 5. s0/s2 produce no (critical) conflicts but one
    /// crossing each way => two zero-weight dependencies s0<->s2. Cycle
    /// breaking marks (no random draw needed): s0=6, s1=5, s2=4, so the
    /// s0->s2 dependency is removed. Topological numbering: s2 slot 0,
    /// s0 slot 1, slotsCount=2.
    ///
    /// Layer x positions: layer 0 at 0, routing area width
    /// 10 (slots) + 2*10 (edge-node) = 30, layer 1 at 60, graph width 90.
    /// startPos between the layers = 30 + 10 = 40:
    /// - e1 trunk at x = 40 + 1*10 = 50 => bends (50,10), (50,105)
    /// - e2 trunk at x = 40 + 0*10 = 40 => bends (40,125), (40,5)
    /// - e3 straight => no bends
    #[test]
    fn two_layers_with_crossing_pair() {
        let mut a = crate::alg_layered::graph::LGraphArena::new();
        let (g, [e1, e2, e3], [na, nb, nc, nd]) = build_two_layer_graph(&mut a);

        let mut random = JavaRandom::new(1);
        process(&mut a, g, &mut random).unwrap();

        let bends = |e: LEdgeId| -> Vec<(f64, f64)> {
            a.edge(e).bend_points.iter().map(|v| (v.x, v.y)).collect()
        };
        assert_eq!(bends(e1), vec![(50.0, 10.0), (50.0, 105.0)]);
        assert_eq!(bends(e2), vec![(40.0, 125.0), (40.0, 5.0)]);
        assert_eq!(bends(e3), Vec::<(f64, f64)>::new());

        // no hyperedges involved => no junction points were created
        for e in [e1, e2, e3] {
            let junction_points = a.edge(e).properties.try_get(&lopts::JUNCTION_POINTS);
            assert!(junction_points.map_or(true, |jps| jps.is_empty()));
        }

        // node x coordinates and graph width
        assert_eq!(a.node(na).pos.x, 0.0);
        assert_eq!(a.node(nb).pos.x, 0.0);
        assert_eq!(a.node(nc).pos.x, 60.0);
        assert_eq!(a.node(nd).pos.x, 60.0);
        assert_eq!(a.graph(g).size.x, 90.0);
    }

    /// A hyperedge (two edges sharing one source port) must produce a
    /// junction point where the second edge branches off the vertical trunk.
    ///
    /// Hand-trace: a single EAST port at y=15 with edges to two WEST ports at
    /// y=5 and y=115 yields one segment with incoming=[15], outgoing=[5,115].
    /// One slot (slot 0), trunk at x = 30 + 10 = 40 (no other segments).
    /// Edge 1 gets bends (40,15), (40,5); edge 2 gets bends (40,15), (40,115).
    /// For edge 1's bend (40,15): p=15 is inside (5,115) => junction point.
    /// For edge 2's first bend (40,15): already created => skipped; (40,115)
    /// is the segment end => no junction point.
    #[test]
    fn hyperedge_produces_junction_point() {
        let mut a = crate::alg_layered::graph::LGraphArena::new();
        let g = a.create_graph();
        let layer0 = a.create_layer(g);
        let layer1 = a.create_layer(g);
        a.graph_mut(g).layers.push(layer0);
        a.graph_mut(g).layers.push(layer1);

        let make_node = |a: &mut crate::alg_layered::graph::LGraphArena, layer, y: f64| {
            let n = a.create_node(g);
            a.node_mut(n).graph = Some(g);
            a.node_mut(n).size = KVector::new(30.0, 30.0);
            a.node_mut(n).pos.y = y;
            a.node_set_layer(n, Some(layer));
            n
        };
        let na = make_node(&mut a, layer0, 0.0);
        let nc = make_node(&mut a, layer1, 0.0);
        let nd = make_node(&mut a, layer1, 100.0);
        a.layer_mut(layer0).size = KVector::new(30.0, 130.0);
        a.layer_mut(layer1).size = KVector::new(30.0, 130.0);

        let pa = a.create_port();
        a.port_set_node(pa, Some(na));
        a.port_set_side(pa, PortSide::EAST);
        a.port_mut(pa).pos = KVector::new(30.0, 15.0);

        let pc = a.create_port();
        a.port_set_node(pc, Some(nc));
        a.port_set_side(pc, PortSide::WEST);
        a.port_mut(pc).pos = KVector::new(0.0, 5.0);

        let pd = a.create_port();
        a.port_set_node(pd, Some(nd));
        a.port_set_side(pd, PortSide::WEST);
        a.port_mut(pd).pos = KVector::new(0.0, 15.0);

        let e1 = a.create_edge();
        a.edge_set_source(e1, Some(pa));
        a.edge_set_target(e1, Some(pc));
        let e2 = a.create_edge();
        a.edge_set_source(e2, Some(pa));
        a.edge_set_target(e2, Some(pd));

        let mut random = JavaRandom::new(1);
        process(&mut a, g, &mut random).unwrap();

        let bends = |e: LEdgeId| -> Vec<(f64, f64)> {
            a.edge(e).bend_points.iter().map(|v| (v.x, v.y)).collect()
        };
        assert_eq!(bends(e1), vec![(40.0, 15.0), (40.0, 5.0)]);
        assert_eq!(bends(e2), vec![(40.0, 15.0), (40.0, 115.0)]);

        // e1 received the junction point at the branch position
        let jps1 = a.edge(e1).properties.get(&lopts::JUNCTION_POINTS);
        let jps1: Vec<(f64, f64)> = jps1.iter().map(|v| (v.x, v.y)).collect();
        assert_eq!(jps1, vec![(40.0, 15.0)]);
        let jps2 = a.edge(e2).properties.try_get(&lopts::JUNCTION_POINTS);
        assert!(jps2.map_or(true, |jps| jps.is_empty()));
    }
}
