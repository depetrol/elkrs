//!
//! Node placement strategy of Gansner et al.: the problem is converted into
//! an auxiliary graph which is layered using the network simplex algorithm
//! (reusing `crate::alg_common::networksimplex`). Includes the `NodeFlexibility`
//! handling (flexible ports / node sizes) and the "favor straight edges"
//! pre-/post-processing.

use std::collections::HashMap;
use std::collections::VecDeque;

use crate::alg_common::networksimplex::{NEdgeId, NGraph, NNodeId, NetworkSimplex};
use crate::core::options::{NodeLabelPlacement, PortConstraints, PortSide};
use crate::graph::math::Spacing;
use crate::graph::properties::EnumSet;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::NodeFlexibility;
use crate::alg_layered::spacings;

// - - - - - - edge weights used in the auxiliary network simplex graph - - - - - -
/// Basis for the weight of edges.
const EDGE_WEIGHT_BASE: f64 = 4.0;
/// Smaller weight than default, since horizontal edges are more important.
const SMALL_EDGE_WEIGHT: f64 = 0.1;
/// If this factor is smaller than one straight long edges are deemed more
/// important than straight (node) paths identified during
/// `prefer_straight_edges`.
const LONG_EDGE_VS_PATH_FACTOR: f64 = 2.0;
/// Large weight to be applied if nodes must not change in size.
const NODE_SIZE_WEIGHT_STATIC: f64 = 10000.0;
/// Weight to be applied if nodes may change in size.
const NODE_SIZE_WEIGHT_FLEXIBLE: f64 = 1.0;
/// Epsilon for double equality testing.
const EPSILON: f64 = 0.00001;

/// Indicates that the node has been visited (despite its name, the constant
/// is used as the visited marker).
const VISITED: i32 = -1;
/// Indicates that a node is not a junction.
const OTHER: i32 = 0;
/// A junction has in-degree > 1, out-degree > 1, or exactly one incident edge.
const JUNCTION: i32 = 2;

#[derive(Clone, Copy)]
struct NodeRep {
    origin: LNodeId,
    /// True if origin's NodeFlexibility doesn't equal NONE (and applies).
    is_flexible: bool,
    /// The 'head' of the node (the border with the lower y coordinate).
    head: NNodeId,
    /// The 'tail' of the node (the border with the larger y coordinate).
    tail: NNodeId,
}

#[derive(Clone, Copy)]
struct EdgeRep {
    left: NEdgeId,
    right: NEdgeId,
}

impl EdgeRep {
    fn is_straight(&self, ng: &NGraph) -> bool {
        self.not_straight_by(ng) == 0
    }

    fn not_straight_by(&self, ng: &NGraph) -> i32 {
        let left = ng.edge(self.left);
        let right = ng.edge(self.right);
        (ng.node(left.target).layer - left.delta) - (ng.node(right.target).layer - right.delta)
    }
}

/// All state of one placer run.
struct Placer {
    ngraph: NGraph,
    /// indexed by LNode.id
    node_reps: Vec<Option<NodeRep>>,
    /// indexed by LEdge.id
    edge_reps: Vec<Option<EdgeRep>>,
    /// `portMap` (HashMap<LGraphElement, NNode>); only get/put.
    port_map: HashMap<LPortId, NNodeId>,
    /// LNode origins of NNodes (`NNode.origin`), for the `instanceof LNode`
    /// check in `improveTwoPath`.
    nnode_lnode_origin: HashMap<NNodeId, LNodeId>,
    node_count: usize,
    edge_count: usize,
    /// indexed by LNode.id; used for edge straightening
    node_state: Vec<i32>,
    two_paths: Vec<Vec<LEdgeId>>,
    /// indexed by LEdge.id
    crossing: Vec<bool>,
    /// edges representing the size of NODE_SIZE_WHERE_SPACE_PERMITS nodes
    flexible_where_space_permits_edges: Vec<NEdgeId>,
}

/// `java.lang.Math.round(double)` as used for coordinate "integerification".
fn java_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn fuzzy_equals(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance || a == b
}

pub fn get_node_flexibility(a: &LGraphArena, node: LNodeId) -> NodeFlexibility {
    if a.node(node)
        .properties
        .has(&lopts::NODE_PLACEMENT_NETWORK_SIMPLEX_NODE_FLEXIBILITY)
    {
        a.node(node)
            .properties
            .get(&lopts::NODE_PLACEMENT_NETWORK_SIMPLEX_NODE_FLEXIBILITY)
    } else {
        let graph = a.node_graph(node);
        a.graph(graph)
            .properties
            .get(&lopts::NODE_PLACEMENT_NETWORK_SIMPLEX_NODE_FLEXIBILITY_DEFAULT)
    }
}

fn is_flexible_size(nf: NodeFlexibility) -> bool {
    nf == NodeFlexibility::NODE_SIZE
}

fn is_flexible_size_where_space_permits(nf: NodeFlexibility) -> bool {
    nf == NodeFlexibility::NODE_SIZE_WHERE_SPACE_PERMITS || nf == NodeFlexibility::NODE_SIZE
}

fn is_flexible_ports(nf: NodeFlexibility) -> bool {
    nf == NodeFlexibility::PORT_POSITION
        || nf == NodeFlexibility::NODE_SIZE_WHERE_SPACE_PERMITS
        || nf == NodeFlexibility::NODE_SIZE
}

pub fn is_flexible_node(a: &LGraphArena, node: LNodeId) -> bool {
    // dummies are not flexible!
    if a.node(node).node_type != NodeType::NORMAL {
        return false;
    }

    // at least two ports are required ...
    if a.node(node).ports.len() <= 1 {
        return false;
    }

    // if ports may not be moved there's no use in enlarging the node
    let pc: PortConstraints = a.node(node).properties.get(&lopts::PORT_CONSTRAINTS);
    if pc == PortConstraints::FIXED_POS {
        return false;
    }

    let nf = get_node_flexibility(a, node);
    if nf == NodeFlexibility::NONE {
        return false;
    }

    // if we cannot resize the node, and the given height is not enough to
    // properly accommodate all ports, reuse the existing port positions
    if !is_flexible_size_where_space_permits(nf) {
        let port_spacing =
            spacings::get_individual_or_default(a, node, &lopts::SPACING_PORT_PORT).unwrap_or(0.0);
        // LNode.getProperty: node-level value or the option's default
        let additional_port_spacing: Spacing = a
            .node(node)
            .properties
            .get_opt(&lopts::SPACING_PORTS_SURROUNDING)
            .unwrap_or_else(|| Spacing::uniform(port_spacing));

        // check west side
        let west_ports = a.node_port_side_view(node, PortSide::WEST);
        let required_west_height = additional_port_spacing.top
            + additional_port_spacing.bottom
            + (west_ports.len() as i32 - 1) as f64 * port_spacing;
        if required_west_height > a.node(node).size.y {
            return false;
        }

        // check east side
        let east_ports = a.node_port_side_view(node, PortSide::EAST);
        let required_east_height = additional_port_spacing.top
            + additional_port_spacing.bottom
            + (east_ports.len() as i32 - 1) as f64 * port_spacing;
        if required_east_height > a.node(node).size.y {
            return false;
        }
    }
    true
}

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mut p = Placer {
        ngraph: NGraph::new(),
        node_reps: Vec::new(),
        edge_reps: Vec::new(),
        port_map: HashMap::new(),
        nnode_lnode_origin: HashMap::new(),
        node_count: 0,
        edge_count: 0,
        node_state: Vec::new(),
        two_paths: Vec::new(),
        crossing: Vec::new(),
        flexible_where_space_permits_edges: Vec::new(),
    };

    // -------------------------------
    // #1 build the auxiliary graph
    // -------------------------------
    prepare(a, graph, &mut p);

    build_initial_auxiliary_graph(a, graph, &mut p);

    insert_north_south_auxiliary_edges(a, graph, &mut p);
    insert_in_layer_edge_auxiliary_edges(a, graph, &mut p);

    let favor_straight_edges: bool = a
        .graph(graph)
        .properties
        .get(&lopts::NODE_PLACEMENT_FAVOR_STRAIGHT_EDGES);
    if favor_straight_edges {
        prefer_straight_edges(a, graph, &mut p);
    }

    // make sure the ngraph is connected
    p.ngraph.make_connected();

    // --------------------------------
    // #2 execute the network simplex
    // --------------------------------
    let thoroughness: i32 = a.graph(graph).properties.get(&lopts::THOROUGHNESS);
    let iter_limit = thoroughness.wrapping_mul(p.ngraph.nodes.len() as i32);

    NetworkSimplex::for_graph(&mut p.ngraph)
        .with_iteration_limit(iter_limit)
        .with_balancing(false)
        .execute();

    // every individual node can be 'flexible where space permits'
    if !p.flexible_where_space_permits_edges.is_empty() {
        insert_flexible_where_space_auxiliary_edges(a, &mut p);
        // now the nodes may resize -> alter the weights
        for &edge in &p.flexible_where_space_permits_edges {
            p.ngraph.edge_mut(edge).weight = NODE_SIZE_WEIGHT_FLEXIBLE;
        }

        // run network simplex a second time
        NetworkSimplex::for_graph(&mut p.ngraph)
            .with_iteration_limit(iter_limit)
            .with_balancing(false)
            .execute();
    }

    // post process 'two paths' identified during #preferStraightEdges()
    if favor_straight_edges {
        post_process_two_paths(a, &mut p);
    }

    // --------------------------------
    // #3 apply positions
    // --------------------------------
    apply_positions(a, graph, &mut p);

    Ok(())
}

// ------------------------------------------------------------------------------------------------
//                                         Preparation
// ------------------------------------------------------------------------------------------------

fn prepare(a: &mut LGraphArena, graph: LGraphId, p: &mut Placer) {
    // "integerify" port anchor and port positions
    // ... while we're at it, we assign ids to the nodes and edges
    let mut node_idx = 0i32;
    let mut edge_idx = 0i32;
    for &layer in &a.graph(graph).layers.clone() {
        for lnode in a.layer(layer).nodes.clone() {
            a.node_mut(lnode).id = node_idx;
            node_idx += 1;
            for e in a.node_outgoing_edges(lnode) {
                a.edge_mut(e).id = edge_idx;
                edge_idx += 1;
            }

            // if a node is flexible, an edge attaches to the port itself
            // within the auxiliary graph, thus the anchor must be integer
            let anchor_must_be_integer = is_flexible_node(a, lnode);
            for port in a.node(lnode).ports.clone() {
                if anchor_must_be_integer {
                    // anchor
                    let y = a.port(port).anchor.y;
                    if y != y.floor() {
                        let offset = y - java_round(y);
                        a.port_mut(port).anchor.y -= offset;
                    }
                }

                // port + anchor
                let y = a.port(port).pos.y + a.port(port).anchor.y;
                if y != y.floor() {
                    let offset = y - java_round(y);
                    a.port_mut(port).pos.y -= offset;
                }
            }
        }
    }

    p.node_count = node_idx as usize;
    p.edge_count = edge_idx as usize;
    p.node_reps = vec![None; p.node_count];
    p.edge_reps = vec![None; p.edge_count];
    p.flexible_where_space_permits_edges.clear();
}

// ------------------------------------------------------------------------------------------------
//                                      Auxiliary Graph
// ------------------------------------------------------------------------------------------------

fn build_initial_auxiliary_graph(a: &mut LGraphArena, graph: LGraphId, p: &mut Placer) {
    for &layer in &a.graph(graph).layers.clone() {
        transform_layer(a, layer, p);
    }
    transform_edges(a, graph, p);
}

fn transform_layer(a: &mut LGraphArena, layer: crate::alg_layered::graph::LayerId, p: &mut Placer) {
    let mut last_rep: Option<NodeRep> = None;
    for lnode in a.layer(layer).nodes.clone() {
        let node_rep = if is_flexible_node(a, lnode) {
            transform_fixed_order_node(a, lnode, p)
        } else {
            transform_fixed_pos_node(a, lnode, p)
        };
        p.node_reps[a.node(lnode).id as usize] = Some(node_rep);

        // if there is a previous node in the layer, create a separation edge
        if let Some(last) = last_rep {
            let mut spacing = a.node(last.origin).margin.bottom
                + spacings::vertical_spacing(a, last.origin, lnode)
                + a.node(lnode).margin.top;

            if !last.is_flexible {
                // for non-flexible nodes their height must be included
                // in the minimal length of the separation edge
                spacing += a.node(last.origin).size.y;
            }

            p.ngraph
                .add_edge(last.tail, node_rep.head, 0.0, spacing.ceil() as i32);
        }

        last_rep = Some(node_rep);
    }
}

fn transform_fixed_pos_node(a: &LGraphArena, lnode: LNodeId, p: &mut Placer) -> NodeRep {
    let single_node = p.ngraph.add_node();
    p.nnode_lnode_origin.insert(single_node, lnode);

    // register the ports with the node
    for &port in &a.node(lnode).ports {
        let side = a.port(port).side;
        if side == PortSide::EAST || side == PortSide::WEST {
            p.port_map.insert(port, single_node);
        }
    }

    NodeRep { origin: lnode, is_flexible: false, head: single_node, tail: single_node }
}

fn transform_fixed_order_node(a: &LGraphArena, lnode: LNodeId, p: &mut Placer) -> NodeRep {
    // corner creation
    let top_left = p.ngraph.add_node();
    p.nnode_lnode_origin.insert(top_left, lnode);
    let bottom_left = p.ngraph.add_node();
    p.nnode_lnode_origin.insert(bottom_left, lnode);
    let corners = NodeRep { origin: lnode, is_flexible: true, head: top_left, tail: bottom_left };

    // weight & minimum length
    let min_height = a.node(lnode).size.y;

    let nf = get_node_flexibility(a, lnode);
    let mut size_weight = NODE_SIZE_WEIGHT_STATIC;
    if is_flexible_size(nf) {
        // we are allowed to enlarge the node; nevertheless, a little weight
        // is good, otherwise the node can become arbitrarily tall
        size_weight = NODE_SIZE_WEIGHT_FLEXIBLE;
    }

    let node_size_edge =
        p.ngraph
            .add_edge(top_left, bottom_left, size_weight, min_height.ceil() as i32);

    if nf == NodeFlexibility::NODE_SIZE_WHERE_SPACE_PERMITS {
        p.flexible_where_space_permits_edges.push(node_size_edge);
    }

    // port transformation: the list of westward ports must be reversed since
    // their original order is from bottom to top
    let mut west_ports = a.node_port_side_view(lnode, PortSide::WEST);
    west_ports.reverse();
    transform_ports(a, &west_ports, &corners, p);
    let east_ports = a.node_port_side_view(lnode, PortSide::EAST);
    transform_ports(a, &east_ports, &corners, p);

    corners
}

fn transform_ports(a: &LGraphArena, ports: &[LPortId], corners: &NodeRep, p: &mut Placer) {
    if ports.is_empty() {
        // nothing to do ... the top and bottom border of the node are
        // already safely spaced apart
        return;
    }

    let port_spacing =
        spacings::get_individual_or_default(a, corners.origin, &lopts::SPACING_PORT_PORT)
            .unwrap_or(0.0);
    let port_surrounding: Spacing = spacings::get_individual_or_default(
        a,
        corners.origin,
        &lopts::SPACING_PORTS_SURROUNDING,
    )
    // No additional port spacing set
    .unwrap_or_default();

    let mut last_nnode = corners.head;
    let mut last_port: Option<LPortId> = None;
    for &port in ports {
        // spacing between the current pair of ports (or to the top border)
        let spacing = match last_port {
            None => port_surrounding.top,
            Some(lp) => port_spacing + a.port(lp).size.y,
        };

        // create NNode for the port
        let nnode = p.ngraph.add_node();
        p.port_map.insert(port, nnode);

        // connect with previous NNode
        p.ngraph.add_edge(last_nnode, nnode, 0.0, spacing.ceil() as i32);

        last_port = Some(port);
        last_nnode = nnode;
    }

    // and connect to the bottom border
    p.ngraph.add_edge(
        last_nnode,
        corners.tail,
        0.0,
        (port_surrounding.bottom + a.port(last_port.unwrap()).size.y).ceil() as i32,
    );
}

fn transform_edges(a: &LGraphArena, graph: LGraphId, p: &mut Placer) {
    for &layer in &a.graph(graph).layers {
        for &node in &a.layer(layer).nodes {
            for edge in a.node_outgoing_edges(node) {
                if is_handled_edge(a, edge) {
                    transform_edge(a, edge, p);
                }
            }
        }
    }
}

fn transform_edge(a: &LGraphArena, ledge: LEdgeId, p: &mut Placer) {
    // a dummy node
    let dummy = p.ngraph.add_node();

    // calculate port offsets
    let src_port = a.edge(ledge).source.unwrap();
    let tgt_port = a.edge(ledge).target.unwrap();
    let src_rep = p.node_reps[a.node(a.port(src_port).node.unwrap()).id as usize].unwrap();
    let tgt_rep = p.node_reps[a.node(a.port(tgt_port).node.unwrap()).id as usize].unwrap();

    let mut src_offset = a.port(src_port).anchor.y;
    let mut tgt_offset = a.port(tgt_port).anchor.y;
    // for non-flexible nodes, ports are relative to node positions
    if !src_rep.is_flexible {
        src_offset += a.port(src_port).pos.y;
    }
    if !tgt_rep.is_flexible {
        tgt_offset += a.port(tgt_port).pos.y;
    }

    let tgt_delta = f64::max(0.0, src_offset - tgt_offset) as i32;
    let src_delta = f64::max(0.0, tgt_offset - src_offset) as i32;

    let weight = get_edge_weight(a, ledge);

    // an edge to the source
    let left = p
        .ngraph
        .add_edge(dummy, p.port_map[&src_port], weight, src_delta);

    // an edge to the target
    let right = p
        .ngraph
        .add_edge(dummy, p.port_map[&tgt_port], weight, tgt_delta);

    p.edge_reps[a.edge(ledge).id as usize] = Some(EdgeRep { left, right });
}

/// Keep edges connected to
/// inverted ports short.
fn insert_in_layer_edge_auxiliary_edges(a: &LGraphArena, graph: LGraphId, p: &mut Placer) {
    for &layer in &a.graph(graph).layers {
        for &node in &a.layer(layer).nodes {
            if a.node(node).node_type != NodeType::NORMAL {
                continue;
            }
            for edge in a.node_connected_edges(node) {
                if !a.edge_is_in_layer(edge) {
                    continue;
                }

                let src_node = a.edge_source_node(edge);
                let src_is_dummy = a.node(src_node).node_type != NodeType::NORMAL;

                let the_port = if src_is_dummy {
                    a.edge(edge).target.unwrap()
                } else {
                    a.edge(edge).source.unwrap()
                };
                // LEdge.getOther(port).getNode()
                let dummy_node = if a.edge(edge).source == Some(the_port) {
                    a.edge_target_node(edge)
                } else {
                    a.edge_source_node(edge)
                };

                let port_rep = p.port_map[&the_port];
                // head/tail doesn't matter since it's a dummy node
                let dummy_rep = p.node_reps[a.node(dummy_node).id as usize].unwrap().head;

                let the_port_node = a.port(the_port).node.unwrap();
                let (src, tgt) =
                    if a.node_index_in_layer(the_port_node) < a.node_index_in_layer(dummy_node) {
                        // port --> dummy
                        (port_rep, dummy_rep)
                    } else {
                        // dummy --> port
                        (dummy_rep, port_rep)
                    };

                p.ngraph.add_edge(src, tgt, EDGE_WEIGHT_BASE, 0);
            }
        }
    }
}

/// Keep north and south port
/// edges short.
fn insert_north_south_auxiliary_edges(a: &LGraphArena, graph: LGraphId, p: &mut Placer) {
    for &layer in &a.graph(graph).layers {
        for &n in &a.layer(layer).nodes {
            for sp in a.node_port_side_view(n, PortSide::SOUTH) {
                let other: Option<LNodeId> =
                    a.port(sp).properties.try_get(&crate::alg_layered::internal_properties::PORT_DUMMY);
                // if no edge was attached to the port, no dummy was created ...
                if let Some(other) = other {
                    p.ngraph.add_edge(
                        p.node_reps[a.node(n).id as usize].unwrap().tail,
                        p.node_reps[a.node(other).id as usize].unwrap().head,
                        SMALL_EDGE_WEIGHT,
                        0, // doesn't matter, separation is already taken care of
                    );
                }
            }

            for sp in a.node_port_side_view(n, PortSide::NORTH) {
                let other: Option<LNodeId> =
                    a.port(sp).properties.try_get(&crate::alg_layered::internal_properties::PORT_DUMMY);
                if let Some(other) = other {
                    p.ngraph.add_edge(
                        p.node_reps[a.node(other).id as usize].unwrap().tail,
                        p.node_reps[a.node(n).id as usize].unwrap().head,
                        SMALL_EDGE_WEIGHT,
                        0,
                    );
                }
            }
        }
    }
}

fn insert_flexible_where_space_auxiliary_edges(a: &LGraphArena, p: &mut Placer) {
    let min_layer = p
        .ngraph
        .nodes
        .iter()
        .map(|&n| p.ngraph.node(n).layer)
        .min()
        .unwrap();
    let max_layer = p
        .ngraph
        .nodes
        .iter()
        .map(|&n| p.ngraph.node(n).layer)
        .max()
        .unwrap();
    let used_layers = max_layer - min_layer;

    let global_source = p.ngraph.add_node();
    let global_sink = p.ngraph.add_node();

    // make sure the distance between source and sink is preserved
    p.ngraph
        .add_edge(global_source, global_sink, NODE_SIZE_WEIGHT_STATIC * 2.0, used_layers);

    // fix the position of most non-flexible nodes and make sure the flexible
    // nodes can only increase in size
    for i in 0..p.node_reps.len() {
        let nr = p.node_reps[i].unwrap();
        if a.node(nr.origin).node_type != NodeType::NORMAL {
            continue;
        }
        // allow leaves to move
        if a.node(nr.origin).ports.len() <= 1 {
            continue;
        }
        let tail_layer = p.ngraph.node(nr.tail).layer;
        p.ngraph
            .add_edge(global_source, nr.tail, 0.0, tail_layer - min_layer);

        let head_layer = p.ngraph.node(nr.head).layer;
        p.ngraph
            .add_edge(nr.head, global_sink, 0.0, used_layers - head_layer);
    }
}

// ------------------------------------------------------------------------------------------------
//                                       Apply Layout
// ------------------------------------------------------------------------------------------------

fn apply_positions(a: &mut LGraphArena, graph: LGraphId, p: &mut Placer) {
    for &layer in &a.graph(graph).layers.clone() {
        for lnode in a.layer(layer).nodes.clone() {
            // find the node's corners
            let node_rep = p.node_reps[a.node(lnode).id as usize].unwrap();
            let min_y = p.ngraph.node(node_rep.head).layer as f64;
            let max_y = p.ngraph.node(node_rep.tail).layer as f64;

            // set new position and size
            a.node_mut(lnode).pos.y = min_y;

            let size_delta = (max_y - min_y) - a.node(lnode).size.y;

            let flexible_node = is_flexible_node(a, lnode);
            let nf = get_node_flexibility(a, lnode);

            // modify the size?
            if flexible_node && is_flexible_size_where_space_permits(nf) {
                a.node_mut(lnode).size.y += size_delta;
            }

            // reposition ports if allowed
            if flexible_node && is_flexible_ports(nf) {
                for port in a.node(lnode).ports.clone() {
                    let side = a.port(port).side;
                    if side == PortSide::EAST || side == PortSide::WEST {
                        let nnode = p.port_map[&port];
                        a.port_mut(port).pos.y = p.ngraph.node(nnode).layer as f64 - min_y;
                    }
                }
                // when the node got resized, the positions of labels and
                // south ports have to be adjusted
                for label in a.node(lnode).labels.clone() {
                    adjust_label_position(a, lnode, label, size_delta);
                }
                if is_flexible_size_where_space_permits(nf) {
                    for port in a.node_port_side_view(lnode, PortSide::SOUTH) {
                        a.port_mut(port).pos.y += size_delta;
                    }
                }
            }
        }
    }
}

fn adjust_label_position(
    a: &mut LGraphArena,
    node: LNodeId,
    label: crate::alg_layered::graph::LLabelId,
    size_delta: f64,
) {
    let placement: EnumSet<NodeLabelPlacement> =
        a.node(node).properties.get(&lopts::NODE_LABELS_PLACEMENT);
    if placement.contains(NodeLabelPlacement::V_BOTTOM) {
        a.label_mut(label).pos.y += size_delta;
    } else if placement.contains(NodeLabelPlacement::V_CENTER) {
        a.label_mut(label).pos.y += size_delta / 2.0;
    }
    // V_TOP placement does not require adjustment
}

// ------------------------------------------------------------------------------------------------
//                                        Convenience
// ------------------------------------------------------------------------------------------------

fn get_edge_weight(a: &LGraphArena, edge: LEdgeId) -> f64 {
    let priority = i32::max(
        1,
        a.edge(edge).properties.get(&lopts::PRIORITY_STRAIGHTNESS),
    );
    let edge_type_weight = get_edge_weight_by_types(
        a.node(a.edge_source_node(edge)).node_type,
        a.node(a.edge_target_node(edge)).node_type,
    );
    priority as f64 * edge_type_weight
}

fn get_edge_weight_by_types(node_type1: NodeType, node_type2: NodeType) -> f64 {
    if node_type1 == NodeType::NORMAL && node_type2 == NodeType::NORMAL {
        1.0 * EDGE_WEIGHT_BASE
    } else if node_type1 == NodeType::NORMAL || node_type2 == NodeType::NORMAL {
        2.0 * EDGE_WEIGHT_BASE
    } else {
        8.0 * EDGE_WEIGHT_BASE
    }
}

/// Neither a self loop nor an in-layer edge.
fn is_handled_edge(a: &LGraphArena, edge: LEdgeId) -> bool {
    !a.edge_is_self_loop(edge) && !a.edge_is_in_layer(edge)
}

// ------------------------------------------------------------------------------------------------
//                                      Edge Straightening
// ------------------------------------------------------------------------------------------------

fn path_contains_long_edge_dummy(a: &LGraphArena, path: &[LEdgeId]) -> bool {
    if path.is_empty() {
        return false;
    }
    if a.node(a.edge_source_node(path[0])).node_type == NodeType::LONG_EDGE {
        return true;
    }
    path.iter()
        .any(|&e| a.node(a.edge_target_node(e)).node_type == NodeType::LONG_EDGE)
}

fn path_contains_flexible_node(a: &LGraphArena, path: &[LEdgeId]) -> bool {
    if path.is_empty() {
        return false;
    }
    let nf = get_node_flexibility(a, a.edge_source_node(path[0]));
    if is_flexible_size_where_space_permits(nf) {
        return true;
    }
    path.iter().any(|&e| {
        is_flexible_size_where_space_permits(get_node_flexibility(a, a.edge_target_node(e)))
    })
}

fn order_two_path(a: &LGraphArena, path: &mut Vec<LEdgeId>) {
    assert!(path.len() == 2, "Order only allowed for two paths.");
    let first = path[0];
    let second = path[1];
    if a.edge_target_node(first) != a.edge_source_node(second) {
        path.clear();
        path.push(second);
        path.push(first);
    }
}

fn is_two_path_center_node_flexible(a: &LGraphArena, path: &[LEdgeId]) -> bool {
    is_flexible_node(a, a.edge_target_node(path[0]))
}

fn prefer_straight_edges(a: &LGraphArena, graph: LGraphId, p: &mut Placer) {
    // the nodes were counted and indexed during #prepare
    p.node_state = vec![0; p.node_count];
    p.two_paths = Vec::new();

    // record node states
    for &layer in &a.graph(graph).layers {
        for &n in &a.layer(layer).nodes {
            p.node_state[a.node(n).id as usize] = get_node_state(a, n);
        }
    }

    mark_edge_crossings(a, graph, p);
    let identified_paths = identify_paths(a, graph, p);

    // essentially 'long paths' are treated like 'long edges'
    for mut path in identified_paths {
        if path.len() <= 1 {
            continue;
        }

        // remember 'two paths' for processing after network simplex
        if path.len() == 2 {
            order_two_path(a, &mut path);
            if !is_two_path_center_node_flexible(a, &path) {
                p.two_paths.push(path);
            }
            continue;
        }

        // ignore paths that contain long edge dummies, and paths that
        // contain flexible nodes that allow resizing
        if path_contains_long_edge_dummy(a, &path) || path_contains_flexible_node(a, &path) {
            continue;
        }

        let path_len = path.len();
        let mut last: Option<LEdgeId> = None;
        for (i, &cur) in path.iter().enumerate() {
            let cur_rep = p.edge_reps[a.edge(cur).id as usize].unwrap();

            let mut weight = if last.is_none() || i + 1 == path_len {
                // first or last segment
                get_edge_weight_by_types(NodeType::NORMAL, NodeType::LONG_EDGE)
            } else {
                get_edge_weight_by_types(NodeType::LONG_EDGE, NodeType::LONG_EDGE)
            };

            // at this point one can decide whether long edges are more
            // important than "paths"
            weight *= LONG_EDGE_VS_PATH_FACTOR;

            let old_left_weight = p.ngraph.edge(cur_rep.left).weight;
            p.ngraph.edge_mut(cur_rep.left).weight =
                f64::max(old_left_weight, old_left_weight + (weight - old_left_weight));
            let old_right_weight = p.ngraph.edge(cur_rep.right).weight;
            p.ngraph.edge_mut(cur_rep.right).weight =
                f64::max(old_right_weight, old_right_weight + (weight - old_right_weight));

            last = Some(cur);
        }
    }
}

fn post_process_two_paths(a: &LGraphArena, p: &mut Placer) {
    let mut q: VecDeque<Vec<LEdgeId>> = p.two_paths.iter().cloned().collect();

    let mut s: Vec<Vec<LEdgeId>> = Vec::new();
    while let Some(path) = q.pop_front() {
        let try_again = improve_two_path(a, &path, true, p);
        if try_again {
            s.push(path);
        }
    }

    while let Some(path) = s.pop() {
        improve_two_path(a, &path, false, p);
    }
}

fn improve_two_path(a: &LGraphArena, path: &[LEdgeId], probe: bool, p: &mut Placer) -> bool {
    let left_edge = p.edge_reps[a.edge(path[0]).id as usize].unwrap();
    let right_edge = p.edge_reps[a.edge(path[1]).id as usize].unwrap();

    // is the edge already straight?
    if left_edge.is_straight(&p.ngraph) && right_edge.is_straight(&p.ngraph) {
        return false;
    }

    // get center node; can be a node or a port
    let center_target = p.ngraph.edge(left_edge.right).target;
    let center_node = match p.nnode_lnode_origin.get(&center_target) {
        Some(&n) => n,
        None => return false,
    };
    let n_node = p.node_reps[a.node(center_node).id as usize].unwrap();

    // identify on which side there is more space
    let node_index = a.node_index_in_layer(center_node);
    let layer_nodes = &a.layer(a.node(center_node).layer.unwrap()).nodes;
    let mut above_dist = f64::INFINITY;
    if node_index > 0 {
        let above = layer_nodes[(node_index - 1) as usize];
        let above_rep = p.node_reps[a.node(above).id as usize].unwrap();
        let spacing = spacings::vertical_spacing(a, above, center_node).ceil();
        above_dist = (p.ngraph.node(n_node.head).layer as f64
            - a.node(center_node).margin.top)
            - (p.ngraph.node(above_rep.head).layer as f64
                + a.node(above).size.y
                + a.node(above).margin.bottom)
            - spacing;
    }
    let mut below_dist = f64::INFINITY;
    if (node_index as usize) < layer_nodes.len() - 1 {
        let below = layer_nodes[(node_index + 1) as usize];
        let below_rep = p.node_reps[a.node(below).id as usize].unwrap();
        let spacing = spacings::vertical_spacing(a, below, center_node).ceil();
        below_dist = (p.ngraph.node(below_rep.head).layer as f64 - a.node(below).margin.top)
            - (p.ngraph.node(n_node.head).layer as f64
                + a.node(center_node).size.y
                + a.node(center_node).margin.bottom)
            - spacing;
    }

    // same space on both sides, check again later
    if probe && fuzzy_equals(above_dist, below_dist, EPSILON) {
        return true;
    }

    // the following variables represent the length of each of the four edges
    // O--a--o--b--O--c--o--d--O
    let av = length(&p.ngraph, left_edge.left);
    let bv = -length(&p.ngraph, left_edge.right);
    let cv = -length(&p.ngraph, right_edge.left);
    let dv = length(&p.ngraph, right_edge.right);

    let case_d =
        left_edge.not_straight_by(&p.ngraph) > 0 && right_edge.not_straight_by(&p.ngraph) < 0;
    let case_c =
        left_edge.not_straight_by(&p.ngraph) < 0 && right_edge.not_straight_by(&p.ngraph) > 0;
    let case_b = p.ngraph.node(p.ngraph.edge(left_edge.left).target).layer
        + p.ngraph.edge(left_edge.right).delta
        < p.ngraph.node(p.ngraph.edge(right_edge.right).target).layer
            + p.ngraph.edge(right_edge.left).delta;
    let case_a = p.ngraph.node(p.ngraph.edge(left_edge.left).target).layer
        + p.ngraph.edge(left_edge.right).delta
        > p.ngraph.node(p.ngraph.edge(right_edge.right).target).layer
            + p.ngraph.edge(right_edge.left).delta;

    let mut mv = 0i32;
    if !case_d && !case_c {
        if case_a {
            if above_dist + cv as f64 > 0.0 {
                mv = cv;
            } else if below_dist - av as f64 > 0.0 {
                mv = av;
            }
        } else if case_b {
            if above_dist + bv as f64 > 0.0 {
                mv = bv;
            } else if below_dist - dv as f64 > 0.0 {
                mv = dv;
            }
        }
    }

    // move the center node
    p.ngraph.node_mut(n_node.head).layer += mv;
    if n_node.is_flexible {
        p.ngraph.node_mut(n_node.tail).layer += mv;
    }

    false
}

fn length(ng: &NGraph, edge: NEdgeId) -> i32 {
    let e = ng.edge(edge);
    (ng.node(e.source).layer - ng.node(e.target).layer).abs() - e.delta
}

// ------------------------------------------------------------------------------------------------
//                                      Path identification
// ------------------------------------------------------------------------------------------------

fn identify_paths(a: &LGraphArena, graph: LGraphId, p: &mut Placer) -> Vec<Vec<LEdgeId>> {
    let mut paths: Vec<Vec<LEdgeId>> = Vec::new();
    for &layer in &a.graph(graph).layers {
        for &junction in &a.layer(layer).nodes {
            if p.node_state[a.node(junction).id as usize] != JUNCTION {
                continue;
            }
            for e in a.node_connected_edges(junction) {
                if !is_handled_edge(a, e) {
                    continue;
                }
                let mut path: Vec<LEdgeId> = Vec::new();
                follow(a, e, junction, &mut path, p);
                if path.len() > 1 {
                    paths.push(path);
                }
            }
        }
    }
    paths
}

fn follow(a: &LGraphArena, edge: LEdgeId, current: LNodeId, path: &mut Vec<LEdgeId>, p: &mut Placer) {
    // LEdge.getOther(LNode)
    let other = if a.edge_source_node(edge) == current {
        a.edge_target_node(edge)
    } else {
        a.edge_source_node(edge)
    };
    path.push(edge);

    // stop criteria
    let other_id = a.node(other).id as usize;
    if p.node_state[other_id] == VISITED
        || p.node_state[other_id] == JUNCTION
        || p.crossing[a.edge(edge).id as usize]
    {
        return;
    }

    // recurse
    p.node_state[other_id] = VISITED;
    for incident in a.node_connected_edges(other) {
        if !is_handled_edge(a, incident) || incident == edge {
            continue;
        }
        follow(a, incident, other, path, p);
        return;
    }
}

fn get_node_state(a: &LGraphArena, node: LNodeId) -> i32 {
    let mut inco = 0i64;
    let mut ouco = 0i64;
    for &port in &a.node(node).ports {
        inco += a
            .port(port)
            .incoming_edges
            .iter()
            .filter(|&&e| !a.edge_is_self_loop(e))
            .count() as i64;
        ouco += a
            .port(port)
            .outgoing_edges
            .iter()
            .filter(|&&e| !a.edge_is_self_loop(e))
            .count() as i64;
        if inco > 1 || ouco > 1 {
            return JUNCTION;
        }
    }
    if inco + ouco == 1 {
        return JUNCTION;
    }
    OTHER
}

// ------------------------------------------------------------------------------------------------
//                                      Mark Crossings
// ------------------------------------------------------------------------------------------------

fn mark_edge_crossings(a: &LGraphArena, graph: LGraphId, p: &mut Placer) {
    p.crossing = vec![false; p.edge_count];
    let layers = &a.graph(graph).layers;
    for pair in layers.windows(2) {
        mark_crossing_edges(a, pair[0], pair[1], p);
    }
}

fn mark_crossing_edges(
    a: &LGraphArena,
    left: crate::alg_layered::graph::LayerId,
    right: crate::alg_layered::graph::LayerId,
    p: &mut Placer,
) {
    let mut open_edges: Vec<LEdgeId> = Vec::new();

    // add all edges in the order they occur in the left layer
    for &node in &a.layer(left).nodes {
        for port in a.node_port_side_view(node, PortSide::EAST) {
            for &edge in &a.port(port).outgoing_edges {
                if a.edge_is_in_layer(edge)
                    || a.edge_is_self_loop(edge)
                    || a.node(a.edge_target_node(edge)).layer != Some(right)
                {
                    continue;
                }
                open_edges.push(edge);
            }
        }
    }

    // close the edges one after another, recording edge crossings
    for &node in a.layer(right).nodes.iter().rev() {
        for port in a.node_port_side_view(node, PortSide::WEST) {
            // don't reverse, bottom up is correct
            for &edge in &a.port(port).incoming_edges {
                if a.edge_is_in_layer(edge)
                    || a.edge_is_self_loop(edge)
                    || a.node(a.edge_source_node(edge)).layer != Some(left)
                {
                    continue;
                }
                if !open_edges.is_empty() {
                    // ListIterator from the end; previous() decrements the
                    // cursor and returns the element at the new position
                    let mut cursor = open_edges.len() - 1;
                    let mut last = open_edges[cursor];
                    while last != edge && cursor > 0 {
                        // mark both edges as being part of an edge crossing
                        p.crossing[a.edge(last).id as usize] = true;
                        p.crossing[a.edge(edge).id as usize] = true;
                        cursor -= 1;
                        last = open_edges[cursor];
                    }
                    // remove the element last returned by previous() — but
                    // only if the cursor is not at the very beginning
                    if cursor > 0 {
                        open_edges.remove(cursor);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alg_layered::graph::LayerId;

    fn make_node(a: &mut LGraphArena, layer: LayerId, height: f64) -> LNodeId {
        let g = a.layer(layer).graph.unwrap();
        let n = a.create_node(g);
        a.node_mut(n).size.y = height;
        a.node_set_layer(n, Some(layer));
        n
    }

    fn connect(a: &mut LGraphArena, source: LNodeId, target: LNodeId) -> LEdgeId {
        let sp = a.create_port();
        a.port_set_node(sp, Some(source));
        a.port_mut(sp).side = PortSide::EAST;
        let tp = a.create_port();
        a.port_set_node(tp, Some(target));
        a.port_mut(tp).side = PortSide::WEST;
        let e = a.create_edge();
        a.edge_set_source(e, Some(sp));
        a.edge_set_target(e, Some(tp));
        e
    }

    /// Two connected nodes of equal height end up vertically aligned.
    #[test]
    fn two_connected_nodes_align() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        a.graph(g)
            .properties
            .set(&lopts::NODE_PLACEMENT_FAVOR_STRAIGHT_EDGES, false);
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

    /// Two nodes in one layer must be separated by at least their spacing.
    #[test]
    fn in_layer_separation() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        a.graph(g)
            .properties
            .set(&lopts::NODE_PLACEMENT_FAVOR_STRAIGHT_EDGES, false);
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
