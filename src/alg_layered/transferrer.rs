//! Applies the computed LGraph layout
//! back to the original ElkGraph.

use crate::core::options::{EdgeRouting, PortConstraints, SizeConstraint, SizeOptions};

use crate::alg_layered::options_gen::NodePlacementStrategy;
use crate::graph::graph::ElkGraph;
use crate::graph::math::{KVector, KVectorChain};
use crate::graph::properties::EnumSet;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::GraphProperties;

pub fn apply_layout(
    a: &mut LGraphArena,
    elk: &mut ElkGraph,
    lgraph: LGraphId,
    elk_parent: crate::graph::graph::NodeId,
) -> Result<(), String> {
    let parent_lnode = a.graph(lgraph).parent_node;

    // offset incl. padding
    let mut offset = a.graph(lgraph).offset;
    let lpadding = a.graph(lgraph).padding;
    offset.x += lpadding.left;
    offset.y += lpadding.top;

    // computed padding
    let size_options: EnumSet<SizeOptions> = elk
        .node(elk_parent)
        .properties
        .get(&lopts::NODE_SIZE_OPTIONS);
    if size_options.contains(SizeOptions::COMPUTE_PADDING) {
        let mut padding = elk.node(elk_parent).properties.get(&lopts::PADDING);
        padding.bottom = lpadding.bottom;
        padding.top = lpadding.top;
        padding.left = lpadding.left;
        padding.right = lpadding.right;
        elk.node(elk_parent).properties.set(&lopts::PADDING, padding);
    }

    let mut edge_list: Vec<LEdgeId> = Vec::new();

    // process the nodes
    let nodes = a.graph(lgraph).layerless_nodes.clone();
    for lnode in nodes {
        match a.node(lnode).properties.try_get(&iprops::ORIGIN) {
            Some(Origin::Node(elknode)) => {
                apply_node_layout(a, elk, lnode, elknode, offset);
            }
            Some(Origin::Port(elkport)) if parent_lnode.is_none() => {
                // External port on the top-most hierarchy level of the current
                // layout run; set its position. (Reachable only for top-level
                // external ports, which ELK Layered does not otherwise support.)
                let (w, h) = {
                    let p = elk.port(elkport);
                    (p.shape.width, p.shape.height)
                };
                let port_position =
                    crate::alg_layered::lgraph_util::get_external_port_position(a, lgraph, lnode, w, h);
                elk.port_mut(elkport)
                    .shape
                    .set_location(port_position.x, port_position.y);
            }
            _ => {}
        }

        // collect edges except those going into nested subgraphs
        for &port in &a.node(lnode).ports {
            for &edge in &a.port(port).outgoing_edges {
                let target_node = a.edge_target_node(edge);
                if !lnode_is_descendant(a, target_node, lnode) {
                    edge_list.push(edge);
                }
            }
        }
    }

    // edges from the representing node down into descendants
    if let Some(parent) = parent_lnode {
        for &port in &a.node(parent).ports {
            for &edge in &a.port(port).outgoing_edges {
                let target_node = a.edge_target_node(edge);
                if lnode_is_descendant(a, target_node, parent) {
                    edge_list.push(edge);
                }
            }
        }
    }

    let routing: EdgeRouting = elk.node(elk_parent).properties.get(&lopts::EDGE_ROUTING);
    for ledge in edge_list {
        apply_edge_layout(a, elk, ledge, routing, offset)?;
    }

    apply_parent_node_layout(a, elk, lgraph, elk_parent);

    // nested subgraphs
    let nodes = a.graph(lgraph).layerless_nodes.clone();
    for lnode in nodes {
        if let Some(nested) = a.node(lnode).nested_graph {
            // The nested graph's ElkNode parent is the origin of the LNode
            // that represents it on this level.
            if let Some(Origin::Node(child_elk)) = a.node(lnode).properties.try_get(&iprops::ORIGIN)
            {
                apply_layout(a, elk, nested, child_elk)?;
            }
        }
    }
    Ok(())
}

/// `LGraphUtil.isDescendant` (LGraph hierarchy).
fn lnode_is_descendant(a: &LGraphArena, child: LNodeId, parent: LNodeId) -> bool {
    let mut current_graph = a.node_graph(child);
    loop {
        match a.graph(current_graph).parent_node {
            None => return false,
            Some(rep) => {
                if rep == parent {
                    return true;
                }
                current_graph = a.node_graph(rep);
            }
        }
    }
}

fn apply_node_layout(
    a: &mut LGraphArena,
    elk: &mut ElkGraph,
    lnode: LNodeId,
    elknode: crate::graph::graph::NodeId,
    offset: KVector,
) {
    // position/layer ids
    let node_id: i32 = a
        .node(lnode)
        .properties
        .get_opt(&lopts::CROSSING_MINIMIZATION_POSITION_ID)
        .unwrap_or(-1);
    let layer_id: i32 = a
        .node(lnode)
        .properties
        .get_opt(&lopts::LAYERING_LAYER_ID)
        .unwrap_or(-1);
    elk.node(elknode)
        .properties
        .set(&lopts::CROSSING_MINIMIZATION_POSITION_ID, node_id);
    elk.node(elknode)
        .properties
        .set(&lopts::LAYERING_LAYER_ID, layer_id);

    // position
    let pos = a.node(lnode).pos;
    elk.node_mut(elknode).shape.x = pos.x + offset.x;
    elk.node_mut(elknode).shape.y = pos.y + offset.y;

    // size, if necessary
    let size_constraints: EnumSet<SizeConstraint> = elk
        .node(elknode)
        .properties
        .get(&lopts::NODE_SIZE_CONSTRAINTS);
    let node_place: NodePlacementStrategy = {
        let g = a.node_graph(lnode);
        a.graph(g).properties.get(&lopts::NODE_PLACEMENT_STRATEGY)
    };
    let flexible_ns = node_place == NodePlacementStrategy::NETWORK_SIMPLEX
        && node_flexibility_is_flexible_size_where_space_permits(a, lnode);
    if !size_constraints.is_empty() || a.node(lnode).nested_graph.is_some() || flexible_ns {
        let size = a.node(lnode).size;
        elk.node_mut(elknode).shape.width = size.x;
        elk.node_mut(elknode).shape.height = size.y;
    }

    // port positions
    for &lport in &a.node(lnode).ports {
        if let Some(Origin::Port(elkport)) = a.port(lport).properties.try_get(&iprops::ORIGIN) {
            let p = a.port(lport);
            elk.port_mut(elkport).shape.set_location(p.pos.x, p.pos.y);
            elk.port(elkport).properties.set(&lopts::PORT_SIDE, p.side);
        }
    }

    // node labels, if placement was computed
    let node_has_label_placement = !a
        .node(lnode)
        .properties
        .get(&lopts::NODE_LABELS_PLACEMENT)
        .is_empty();
    for &llabel in &a.node(lnode).labels {
        let label_has_placement = !a
            .label(llabel)
            .properties
            .get(&lopts::NODE_LABELS_PLACEMENT)
            .is_empty();
        if node_has_label_placement || label_has_placement {
            if let Some(Origin::Label(elklabel)) =
                a.label(llabel).properties.try_get(&iprops::ORIGIN)
            {
                let l = a.label(llabel);
                elk.label_mut(elklabel).shape.set_dimensions(l.size.x, l.size.y);
                elk.label_mut(elklabel).shape.set_location(l.pos.x, l.pos.y);
            }
        }
    }

    // port labels, if not fixed
    let port_labels = a.node(lnode).properties.get(&lopts::PORT_LABELS_PLACEMENT);
    if !port_label_placement_is_fixed(port_labels) {
        for &lport in &a.node(lnode).ports {
            for &llabel in &a.port(lport).labels {
                if let Some(Origin::Label(elklabel)) =
                    a.label(llabel).properties.try_get(&iprops::ORIGIN)
                {
                    let l = a.label(llabel);
                    elk.label_mut(elklabel).shape.set_dimensions(l.size.x, l.size.y);
                    elk.label_mut(elklabel).shape.set_location(l.pos.x, l.pos.y);
                }
            }
        }
    }
}

/// `PortLabelPlacement.isFixed(Set)`.
fn port_label_placement_is_fixed(
    placement: EnumSet<crate::core::options::PortLabelPlacement>,
) -> bool {
    !placement.contains(crate::core::options::PortLabelPlacement::INSIDE)
        && !placement.contains(crate::core::options::PortLabelPlacement::OUTSIDE)
}

/// `NodeFlexibility.getNodeFlexibility(node).isFlexibleSizeWhereSpacePermits()`.
fn node_flexibility_is_flexible_size_where_space_permits(a: &LGraphArena, lnode: LNodeId) -> bool {
    use crate::alg_layered::options_gen::NodeFlexibility;
    let nf: Option<NodeFlexibility> = a
        .node(lnode)
        .properties
        .try_get(&lopts::NODE_PLACEMENT_NETWORK_SIMPLEX_NODE_FLEXIBILITY)
        .or_else(|| {
            let g = a.node_graph(lnode);
            a.graph(g)
                .properties
                .get_opt(&lopts::NODE_PLACEMENT_NETWORK_SIMPLEX_NODE_FLEXIBILITY_DEFAULT)
        });
    matches!(
        nf,
        Some(NodeFlexibility::NODE_SIZE_WHERE_SPACE_PERMITS) | Some(NodeFlexibility::NODE_SIZE)
    )
}

fn apply_edge_layout(
    a: &mut LGraphArena,
    elk: &mut ElkGraph,
    ledge: LEdgeId,
    routing: EdgeRouting,
    offset: KVector,
) -> Result<(), String> {
    let elkedge = match a.edge(ledge).properties.try_get(&iprops::ORIGIN) {
        Some(Origin::Edge(e)) => e,
        _ => return Ok(()), // self-loops under other routers have no origin
    };

    let mut bend_points = a.edge(ledge).bend_points.clone();

    // hierarchical offset
    let mut edge_offset = offset;
    edge_offset.add(calculate_hierarchical_offset(a, ledge));

    // source point
    let source_port = a.edge(ledge).source.unwrap();
    let source_node = a.edge_source_node(ledge);
    let target_node = a.edge_target_node(ledge);
    let source_point = if lnode_is_descendant(a, target_node, source_node) {
        let p = a.port(source_port);
        let mut sp = KVector::new(p.pos.x + p.anchor.x, p.pos.y + p.anchor.y);
        sp.sub(offset);
        sp
    } else {
        port_absolute_anchor(a, source_port)
    };
    bend_points.add_first(source_point);

    // target point
    let target_port = a.edge(ledge).target.unwrap();
    let mut target_point = port_absolute_anchor(a, target_port);
    if let Some(target_offset) = a.edge(ledge).properties.try_get(&iprops::TARGET_OFFSET) {
        target_point.add(target_offset);
    }
    bend_points.add_last(target_point);

    bend_points.offset(edge_offset);

    // apply to the first edge section (reset)
    let section = elk.first_edge_section(elkedge, true);
    let incoming = elk.edge(elkedge).sources[0];
    let outgoing = elk.edge(elkedge).targets[0];
    elk.section_mut(section).incoming_shape = Some(incoming);
    elk.section_mut(section).outgoing_shape = Some(outgoing);
    crate::core::elkutil::apply_vector_chain(elk, &bend_points, section);

    // labels
    let labels = a.edge(ledge).labels.clone();
    for llabel in labels {
        if let Some(Origin::Label(elklabel)) = a.label(llabel).properties.try_get(&iprops::ORIGIN)
        {
            let l = a.label(llabel);
            let (w, h) = (l.size.x, l.size.y);
            let (x, y) = (l.pos.x + edge_offset.x, l.pos.y + edge_offset.y);
            elk.label_mut(elklabel).shape.set_dimensions(w, h);
            elk.label_mut(elklabel).shape.set_location(x, y);

            let include_label: bool = a
                .label(llabel)
                .properties
                .get(&crate::alg_layered::processors::label_dummy_switcher::INCLUDE_LABEL);
            elk.label(elklabel)
                .properties
                .set(&crate::alg_layered::processors::label_dummy_switcher::INCLUDE_LABEL, include_label);
        }
    }

    // junction points
    let junction_points: Option<KVectorChain> =
        a.edge(ledge).properties.get_opt(&lopts::JUNCTION_POINTS);
    match junction_points {
        Some(mut jps) => {
            jps.offset(edge_offset);
            elk.edge(elkedge).properties.set(&lopts::JUNCTION_POINTS, jps);
        }
        None => {
            elk.edge(elkedge).properties.unset(&lopts::JUNCTION_POINTS);
        }
    }

    // routing marker
    if routing == EdgeRouting::SPLINES {
        elk.edge(elkedge)
            .properties
            .set(&lopts::EDGE_ROUTING, EdgeRouting::SPLINES);
    } else {
        elk.edge(elkedge).properties.unset(&lopts::EDGE_ROUTING);
    }
    Ok(())
}

fn port_absolute_anchor(a: &LGraphArena, port: crate::alg_layered::graph::LPortId) -> KVector {
    let p = a.port(port);
    let node = p.node.unwrap();
    let n = a.node(node);
    KVector::new(
        n.pos.x + p.pos.x + p.anchor.x,
        n.pos.y + p.pos.y + p.anchor.y,
    )
}

fn calculate_hierarchical_offset(a: &LGraphArena, ledge: LEdgeId) -> KVector {
    if let Some(target_coordinate_system) = a
        .edge(ledge)
        .properties
        .try_get(&iprops::COORDINATE_SYSTEM_ORIGIN)
    {
        let mut result = KVector::default();
        let mut current_graph = a.node_graph(a.edge_source_node(ledge));
        while current_graph != target_coordinate_system {
            let representing_node = a
                .graph(current_graph)
                .parent_node
                .expect("no upper level before reaching target coordinate system");
            current_graph = a.node_graph(representing_node);
            let g = a.graph(current_graph);
            result.add(a.node(representing_node).pos);
            result.add(g.offset);
            result.add_xy(g.padding.left, g.padding.top);
        }
        return result;
    }
    KVector::default()
}

fn apply_parent_node_layout(
    a: &mut LGraphArena,
    elk: &mut ElkGraph,
    lgraph: LGraphId,
    elknode: crate::graph::graph::NodeId,
) {
    let size_constraints_included_port_labels = elk
        .node(elknode)
        .properties
        .get::<EnumSet<SizeConstraint>>(&lopts::NODE_SIZE_CONSTRAINTS)
        .contains(SizeConstraint::PORT_LABELS);

    if a.graph(lgraph).parent_node.is_none() {
        let graph_props: EnumSet<GraphProperties> =
            a.graph(lgraph).properties.get(&iprops::GRAPH_PROPERTIES);
        let actual_graph_size = a.graph_actual_size(lgraph);

        if graph_props.contains(GraphProperties::EXTERNAL_PORTS) {
            elk.node(elknode)
                .properties
                .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
            crate::core::elkutil::resize_node(
                elk,
                elknode,
                actual_graph_size.x,
                actual_graph_size.y,
                false,
                true,
            );
        } else if !elk
            .node(elknode)
            .properties
            .get(&lopts::NODE_SIZE_FIXED_GRAPH_SIZE)
        {
            crate::core::elkutil::resize_node(
                elk,
                elknode,
                actual_graph_size.x,
                actual_graph_size.y,
                true,
                true,
            );
        }
    }

    if size_constraints_included_port_labels {
        elk.node(elknode).properties.set(
            &lopts::NODE_SIZE_CONSTRAINTS,
            EnumSet::of(&[SizeConstraint::PORT_LABELS]),
        );
    } else {
        elk.node(elknode)
            .properties
            .set(&lopts::NODE_SIZE_CONSTRAINTS, EnumSet::<SizeConstraint>::none());
    }
}
