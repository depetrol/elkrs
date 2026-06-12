//! Port of `org.eclipse.elk.alg.layered.graph.LGraphUtil` (subset; grows as
//! more of the algorithm is ported).

use elk_core::options::{Direction, PortConstraints, PortSide};
use elk_graph::math::KVector;
use elk_graph::properties::EnumSet;

use crate::graph::{LGraphArena, LGraphId, LNodeId, LPortId};
use crate::internal_properties as iprops;
use crate::options_gen::{GraphProperties, PortType};
use crate::options_gen as lopts;

/// Port of `LGraphUtil.getDirection`.
pub fn get_direction(a: &LGraphArena, graph: LGraphId) -> Direction {
    let direction = a.graph(graph).properties.get(&lopts::DIRECTION);
    if direction == Direction::UNDEFINED {
        let aspect_ratio: f64 = a.graph(graph).properties.get(&lopts::ASPECT_RATIO);
        if aspect_ratio >= 1.0 {
            Direction::RIGHT
        } else {
            Direction::DOWN
        }
    } else {
        direction
    }
}

/// Port of `LGraphUtil.calcPortSide` (operates on LPort geometry).
pub fn calc_port_side(a: &LGraphArena, port: LPortId, direction: Direction) -> PortSide {
    let node = a.port(port).node.expect("port without node");
    let node_width = a.node(node).size.x;
    let node_height = a.node(node).size.y;
    if node_width <= 0.0 && node_height <= 0.0 {
        return PortSide::UNDEFINED;
    }

    let p = a.port(port);
    let (xpos, ypos) = (p.pos.x, p.pos.y);
    let (width, height) = (p.size.x, p.size.y);
    match direction {
        Direction::LEFT | Direction::RIGHT => {
            if xpos < 0.0 {
                return PortSide::WEST;
            } else if xpos + width > node_width {
                return PortSide::EAST;
            }
        }
        Direction::UP | Direction::DOWN => {
            if ypos < 0.0 {
                return PortSide::NORTH;
            } else if ypos + height > node_height {
                return PortSide::SOUTH;
            }
        }
        Direction::UNDEFINED => {}
    }

    let width_percent = (xpos + width / 2.0) / node_width;
    let height_percent = (ypos + height / 2.0) / node_height;
    if width_percent + height_percent <= 1.0 && width_percent - height_percent <= 0.0 {
        PortSide::WEST
    } else if width_percent + height_percent >= 1.0 && width_percent - height_percent >= 0.0 {
        PortSide::EAST
    } else if height_percent < 0.5 {
        PortSide::NORTH
    } else {
        PortSide::SOUTH
    }
}

/// Port of `LGraphUtil.calcPortOffset`.
pub fn calc_port_offset(a: &LGraphArena, port: LPortId, side: PortSide) -> f64 {
    let node = a.port(port).node.expect("port without node");
    let p = a.port(port);
    let n = a.node(node);
    match side {
        PortSide::NORTH => -(p.pos.y + p.size.y),
        PortSide::EAST => p.pos.x - n.size.x,
        PortSide::SOUTH => p.pos.y - n.size.y,
        PortSide::WEST => -(p.pos.x + p.size.x),
        PortSide::UNDEFINED => 0.0,
    }
}

/// Port of `LGraphUtil.centerPoint`.
pub fn center_point(point: &mut KVector, boundary: KVector, side: PortSide) {
    match side {
        PortSide::NORTH => {
            point.x = boundary.x / 2.0;
            point.y = 0.0;
        }
        PortSide::EAST => {
            point.x = boundary.x;
            point.y = boundary.y / 2.0;
        }
        PortSide::SOUTH => {
            point.x = boundary.x / 2.0;
            point.y = boundary.y;
        }
        PortSide::WEST => {
            point.x = 0.0;
            point.y = boundary.y / 2.0;
        }
        PortSide::UNDEFINED => {}
    }
}

/// Port of `LGraphUtil.provideCollectorPort`.
pub fn provide_collector_port(
    a: &mut LGraphArena,
    _graph: LGraphId,
    node: LNodeId,
    port_type: PortType,
    side: PortSide,
) -> LPortId {
    match port_type {
        PortType::INPUT => {
            for &inport in &a.node(node).ports {
                if a.port(inport).properties.get(&iprops::INPUT_COLLECT) {
                    return inport;
                }
            }
        }
        PortType::OUTPUT => {
            for &outport in &a.node(node).ports {
                if a.port(outport).properties.get(&iprops::OUTPUT_COLLECT) {
                    return outport;
                }
            }
        }
        PortType::UNDEFINED => panic!("collector port type must be INPUT or OUTPUT"),
    }
    let port = a.create_port();
    match port_type {
        PortType::INPUT => a.port(port).properties.set(&iprops::INPUT_COLLECT, true),
        _ => a.port(port).properties.set(&iprops::OUTPUT_COLLECT, true),
    };
    a.port_set_node(port, Some(node));
    a.port_set_side(port, side);
    let node_size = a.node(node).size;
    let mut pos = a.port(port).pos;
    center_point(&mut pos, node_size, side);
    a.port_mut(port).pos = pos;
    port
}

/// Port of `LGraphUtil.createPort` (used by the importer when an edge end
/// has no explicit port).
pub fn create_port(
    a: &mut LGraphArena,
    node: LNodeId,
    end_point: Option<KVector>,
    port_type: PortType,
    graph: LGraphId,
) -> LPortId {
    let direction = get_direction(a, graph);
    let merge_ports: bool = a.graph(graph).properties.get(&lopts::MERGE_EDGES);
    let hypernode: bool = a.node(node).properties.get(&lopts::HYPERNODE);
    let side_fixed = a
        .node(node)
        .properties
        .get(&lopts::PORT_CONSTRAINTS)
        .is_side_fixed();

    if (merge_ports || hypernode) && !side_fixed {
        let default_side = PortSide::from_direction(direction);
        let side = if port_type == PortType::OUTPUT {
            default_side
        } else {
            default_side.opposed()
        };
        provide_collector_port(a, graph, node, port_type, side)
    } else {
        let port = a.create_port();
        a.port_set_node(port, Some(node));

        if let Some(end_point) = end_point {
            let node_pos = a.node(node).pos;
            let node_size = a.node(node).size;
            let mut pos = a.port(port).pos;
            pos.x = end_point.x - node_pos.x;
            pos.y = end_point.y - node_pos.y;
            pos.bound(0.0, 0.0, node_size.x, node_size.y);
            a.port_mut(port).pos = pos;
            let side = calc_port_side(a, port, direction);
            a.port_set_side(port, side);
        } else {
            let default_side = PortSide::from_direction(direction);
            let side = if port_type == PortType::OUTPUT {
                default_side
            } else {
                default_side.opposed()
            };
            a.port_set_side(port, side);
        }

        let port_side = a.port(port).side;
        let mut graph_properties: EnumSet<GraphProperties> =
            a.graph(graph).properties.get(&iprops::GRAPH_PROPERTIES);
        match direction {
            Direction::LEFT | Direction::RIGHT => {
                if port_side == PortSide::NORTH || port_side == PortSide::SOUTH {
                    graph_properties.add(GraphProperties::NORTH_SOUTH_PORTS);
                    a.graph(graph).properties.set(&iprops::GRAPH_PROPERTIES, graph_properties);
                }
            }
            Direction::UP | Direction::DOWN => {
                if port_side == PortSide::EAST || port_side == PortSide::WEST {
                    graph_properties.add(GraphProperties::NORTH_SOUTH_PORTS);
                    a.graph(graph).properties.set(&iprops::GRAPH_PROPERTIES, graph_properties);
                }
            }
            Direction::UNDEFINED => {}
        }
        port
    }
}

/// Port of `LGraphUtil.initializePort`.
pub fn initialize_port(
    a: &mut LGraphArena,
    port: LPortId,
    port_constraints: PortConstraints,
    direction: Direction,
    anchor_pos: Option<KVector>,
) {
    let mut port_side = a.port(port).side;

    if port_side == PortSide::UNDEFINED && port_constraints.is_side_fixed() {
        port_side = calc_port_side(a, port, direction);
        a.port_set_side(port, port_side);

        let pos = a.port(port).pos;
        if !a.port(port).properties.has(&lopts::PORT_BORDER_OFFSET)
            && port_side != PortSide::UNDEFINED
            && (pos.x != 0.0 || pos.y != 0.0)
        {
            let offset = calc_port_offset(a, port, port_side);
            a.port(port).properties.set(&lopts::PORT_BORDER_OFFSET, offset);
        }
    }

    if port_constraints.is_ratio_fixed() {
        let mut ratio = 0.0;
        let node = a.port(port).node.unwrap();
        match port_side {
            PortSide::NORTH | PortSide::SOUTH => {
                let node_width = a.node(node).size.x;
                if node_width > 0.0 {
                    ratio = a.port(port).pos.x / node_width;
                }
            }
            PortSide::EAST | PortSide::WEST => {
                let node_height = a.node(node).size.y;
                if node_height > 0.0 {
                    ratio = a.port(port).pos.y / node_height;
                }
            }
            PortSide::UNDEFINED => {}
        }
        a.port(port).properties.set(&iprops::PORT_RATIO_OR_POSITION, ratio);
    }

    let port_size = a.port(port).size;
    let mut port_anchor = a.port(port).anchor;

    if let Some(anchor_pos) = anchor_pos {
        port_anchor.x = anchor_pos.x;
        port_anchor.y = anchor_pos.y;
        a.port_mut(port).explicit_anchor = true;
    } else if port_constraints.is_side_fixed() && port_side != PortSide::UNDEFINED {
        match port_side {
            PortSide::NORTH => {
                port_anchor.x = port_size.x / 2.0;
            }
            PortSide::EAST => {
                port_anchor.x = port_size.x;
                port_anchor.y = port_size.y / 2.0;
            }
            PortSide::SOUTH => {
                port_anchor.x = port_size.x / 2.0;
                port_anchor.y = port_size.y;
            }
            PortSide::WEST => {
                port_anchor.y = port_size.y / 2.0;
            }
            PortSide::UNDEFINED => {}
        }
    } else {
        port_anchor.x = port_size.x / 2.0;
        port_anchor.y = port_size.y / 2.0;
    }
    a.port_mut(port).anchor = port_anchor;
}

/// Port of `LEdge.reverse`.
pub fn edge_reverse(
    a: &mut LGraphArena,
    graph: LGraphId,
    edge: crate::graph::LEdgeId,
    adapt_ports: bool,
) {
    let old_source = a.edge(edge).source;
    let old_target = a.edge(edge).target;

    a.edge_set_source(edge, None);
    a.edge_set_target(edge, None);

    let old_target_port = old_target.expect("edge without target");
    if adapt_ports && a.port(old_target_port).properties.get(&iprops::INPUT_COLLECT) {
        let node = a.port(old_target_port).node.unwrap();
        let collector =
            provide_collector_port(a, graph, node, PortType::OUTPUT, PortSide::EAST);
        a.edge_set_source(edge, Some(collector));
    } else {
        a.edge_set_source(edge, old_target);
    }

    let old_source_port = old_source.expect("edge without source");
    if adapt_ports && a.port(old_source_port).properties.get(&iprops::OUTPUT_COLLECT) {
        let node = a.port(old_source_port).node.unwrap();
        let collector =
            provide_collector_port(a, graph, node, PortType::INPUT, PortSide::WEST);
        a.edge_set_target(edge, Some(collector));
    } else {
        a.edge_set_target(edge, old_source);
    }

    let labels = a.edge(edge).labels.clone();
    for label in labels {
        let placement: elk_core::options::EdgeLabelPlacement =
            a.label(label).properties.get(&lopts::EDGE_LABELS_PLACEMENT);
        match placement {
            elk_core::options::EdgeLabelPlacement::TAIL => {
                a.label(label)
                    .properties
                    .set(&lopts::EDGE_LABELS_PLACEMENT, elk_core::options::EdgeLabelPlacement::HEAD);
            }
            elk_core::options::EdgeLabelPlacement::HEAD => {
                a.label(label)
                    .properties
                    .set(&lopts::EDGE_LABELS_PLACEMENT, elk_core::options::EdgeLabelPlacement::TAIL);
            }
            _ => {}
        }
    }

    let reversed: bool = a.edge(edge).properties.get(&iprops::REVERSED);
    a.edge(edge).properties.set(&iprops::REVERSED, !reversed);

    let reversed_bps = elk_graph::math::KVectorChain::reverse(&a.edge(edge).bend_points);
    a.edge_mut(edge).bend_points = reversed_bps;
}
