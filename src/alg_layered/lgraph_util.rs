
use crate::core::options::{Direction, PortConstraints, PortSide, SizeConstraint};
use crate::graph::math::KVector;
use crate::graph::properties::{EnumSet, PropertyMap};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen::{EdgeConstraint, GraphProperties, InLayerConstraint, LayerConstraint, PortType};
use crate::alg_layered::options_gen as lopts;

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

/// The `IPropertyHolder` parameter of `LGraphUtil.createExternalPortDummy`:
/// either an arena `LPort` or an external (Elk port / scratch) property map.
pub enum PortPropertyHolder<'h> {
    LPort(LPortId),
    Map(&'h PropertyMap),
}

impl<'h> PortPropertyHolder<'h> {
    fn with_props<R>(&self, a: &LGraphArena, f: impl FnOnce(&PropertyMap) -> R) -> R {
        match self {
            PortPropertyHolder::LPort(p) => f(&a.port(*p).properties),
            PortPropertyHolder::Map(m) => f(m),
        }
    }
}

/// Creates a dummy node (with
/// one port) for an external port. The dummy is NOT added to the graph's node list.
#[allow(clippy::too_many_arguments)]
pub fn create_external_port_dummy(
    a: &mut LGraphArena,
    property_holder: PortPropertyHolder,
    port_constraints: PortConstraints,
    port_side: PortSide,
    net_flow: i32,
    port_node_size: Option<KVector>,
    port_position: Option<KVector>,
    port_size: KVector,
    layout_direction: Direction,
    layered_graph: LGraphId,
) -> LNodeId {
    let mut final_external_port_side = port_side;

    // Create the dummy with one port
    let dummy = a.create_node(layered_graph);
    a.node_mut(dummy).node_type = NodeType::EXTERNAL_PORT;
    a.node(dummy).properties.set(&iprops::EXT_PORT_SIZE, port_size);
    a.node(dummy)
        .properties
        .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
    let port_border_offset: f64 =
        property_holder.with_props(a, |p| p.get(&lopts::PORT_BORDER_OFFSET));
    a.node(dummy)
        .properties
        .set(&lopts::PORT_BORDER_OFFSET, port_border_offset);

    let dummy_port = a.create_port();
    a.port_set_node(dummy_port, Some(dummy));

    // If the port constraints are free, we need to determine where to put the
    // dummy (and its port)
    if !port_constraints.is_side_fixed() {
        debug_assert!(layout_direction != Direction::UNDEFINED);
        if net_flow >= 0 {
            final_external_port_side = PortSide::from_direction(layout_direction);
        } else {
            final_external_port_side = PortSide::from_direction(layout_direction).opposed();
        }
        property_holder.with_props(a, |p| {
            p.set(&lopts::PORT_SIDE, final_external_port_side);
        });
    }

    // Retrieve the anchor point, possibly to be modified later
    let mut anchor = KVector::default();
    let mut explicit_anchor = false;
    if property_holder.with_props(a, |p| p.has(&lopts::PORT_ANCHOR)) {
        let v: KVector =
            property_holder.with_props(a, |p| p.try_get(&lopts::PORT_ANCHOR).unwrap());
        anchor.set(v.x, v.y);
        explicit_anchor = true;
    } else {
        anchor.set(port_size.x / 2.0, port_size.y / 2.0);
    }

    // With the port side at hand, set the necessary properties and place the
    // dummy's port at the dummy's center
    match final_external_port_side {
        PortSide::WEST => {
            a.node(dummy)
                .properties
                .set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::FIRST_SEPARATE);
            a.node(dummy)
                .properties
                .set(&iprops::EDGE_CONSTRAINT, EdgeConstraint::OUTGOING_ONLY);
            a.node_mut(dummy).size.y = port_size.y;
            if port_border_offset < 0.0 {
                a.node_mut(dummy).size.x = -port_border_offset;
            }
            a.port_set_side(dummy_port, PortSide::EAST);
            if !explicit_anchor {
                anchor.x = port_size.x;
            }
            // The port anchors think that there is a difference between the
            // port's left and right border coordinates, which makes sense if
            // the port has a non-zero width. The port dummy, however, will
            // have a width of zero. Thus, the anchor must be relative to
            // -portWidth. This fixes #546.
            anchor.x -= port_size.x;
        }
        PortSide::EAST => {
            a.node(dummy)
                .properties
                .set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::LAST_SEPARATE);
            a.node(dummy)
                .properties
                .set(&iprops::EDGE_CONSTRAINT, EdgeConstraint::INCOMING_ONLY);
            a.node_mut(dummy).size.y = port_size.y;
            if port_border_offset < 0.0 {
                a.node_mut(dummy).size.x = -port_border_offset;
            }
            a.port_set_side(dummy_port, PortSide::WEST);
            if !explicit_anchor {
                anchor.x = 0.0;
            }
        }
        PortSide::NORTH => {
            a.node(dummy)
                .properties
                .set(&iprops::IN_LAYER_CONSTRAINT, InLayerConstraint::TOP);
            a.node_mut(dummy).size.x = port_size.x;
            if port_border_offset < 0.0 {
                a.node_mut(dummy).size.y = -port_border_offset;
            }
            a.port_set_side(dummy_port, PortSide::SOUTH);
            if !explicit_anchor {
                anchor.y = port_size.y;
            }
            // See comments in case WEST. This partly fixes #680.
            anchor.y -= port_size.y;
        }
        PortSide::SOUTH => {
            a.node(dummy)
                .properties
                .set(&iprops::IN_LAYER_CONSTRAINT, InLayerConstraint::BOTTOM);
            a.node_mut(dummy).size.x = port_size.x;
            if port_border_offset < 0.0 {
                a.node_mut(dummy).size.y = -port_border_offset;
            }
            a.port_set_side(dummy_port, PortSide::NORTH);
            if !explicit_anchor {
                anchor.y = 0.0;
            }
        }
        PortSide::UNDEFINED => {
            debug_assert!(false, "external port side is UNDEFINED");
        }
    }

    // Finally apply the anchor by setting the dummy port position accordingly.
    // Also, remember the anchor on the dummy itself since the hierarchical
    // port processors depend on that
    a.port_mut(dummy_port).pos = anchor;
    a.node(dummy).properties.set(&lopts::PORT_ANCHOR, anchor);

    if port_constraints.is_order_fixed() {
        // The order of ports is fixed in some way, so what we will have to do
        // is to remember information about it
        let mut information_about_it = 0.0f64;

        // If only the order is fixed _and_ the port has an explicit index set
        // on it, remember that
        if port_constraints == PortConstraints::FIXED_ORDER
            && property_holder.with_props(a, |p| p.has(&lopts::PORT_INDEX))
        {
            // We will have to be careful: on the SOUTH and WEST sides, the
            // index is in reverse to what we would later expect in the code,
            // so we'll use the index * -1 there
            let index: i32 =
                property_holder.with_props(a, |p| p.try_get(&lopts::PORT_INDEX).unwrap());
            match final_external_port_side {
                PortSide::NORTH | PortSide::EAST => {
                    information_about_it = index as f64;
                }
                PortSide::SOUTH | PortSide::WEST => {
                    information_about_it = -1.0 * index as f64;
                }
                PortSide::UNDEFINED => {}
            }
        } else {
            // Otherwise, we will just go with the position itself
            match final_external_port_side {
                PortSide::WEST | PortSide::EAST => {
                    information_about_it = port_position.unwrap().y;
                    if port_constraints.is_ratio_fixed() {
                        information_about_it /= port_node_size.unwrap().y;
                    }
                }
                PortSide::NORTH | PortSide::SOUTH => {
                    information_about_it = port_position.unwrap().x;
                    if port_constraints.is_ratio_fixed() {
                        information_about_it /= port_node_size.unwrap().x;
                    }
                }
                PortSide::UNDEFINED => {}
            }
        }

        a.node(dummy)
            .properties
            .set(&iprops::PORT_RATIO_OR_POSITION, information_about_it);
    }

    // Set the port side of the dummy
    a.node(dummy)
        .properties
        .set(&iprops::EXT_PORT_SIDE, final_external_port_side);

    dummy
}

/// Calculates the position of
/// the external port's top left corner from the position of the given dummy
/// node that represents the port. Also adjusts the dummy node's position.
pub fn get_external_port_position(
    a: &mut LGraphArena,
    graph: LGraphId,
    port_dummy: LNodeId,
    port_width: f64,
    port_height: f64,
) -> KVector {
    let mut port_position = a.node(port_dummy).pos;
    port_position.x += a.node(port_dummy).size.x / 2.0;
    port_position.y += a.node(port_dummy).size.y / 2.0;
    let port_offset: f64 = a.node(port_dummy).properties.get(&lopts::PORT_BORDER_OFFSET);

    // Get some properties of the graph
    let graph_size = a.graph(graph).size;
    let padding = a.graph(graph).padding;
    let graph_offset = a.graph(graph).offset;

    // The exact coordinates depend on the port's side...
    match a.node(port_dummy).properties.get::<PortSide>(&iprops::EXT_PORT_SIDE) {
        PortSide::NORTH => {
            port_position.x += padding.left + graph_offset.x - (port_width / 2.0);
            port_position.y = -port_height - port_offset;
            a.node_mut(port_dummy).pos.y = -(padding.top + port_offset + graph_offset.y);
        }
        PortSide::EAST => {
            port_position.x = graph_size.x + padding.left + padding.right + port_offset;
            port_position.y += padding.top + graph_offset.y - (port_height / 2.0);
            a.node_mut(port_dummy).pos.x =
                graph_size.x + padding.right + port_offset - graph_offset.x;
        }
        PortSide::SOUTH => {
            port_position.x += padding.left + graph_offset.x - (port_width / 2.0);
            port_position.y = graph_size.y + padding.top + padding.bottom + port_offset;
            a.node_mut(port_dummy).pos.y =
                graph_size.y + padding.bottom + port_offset - graph_offset.y;
        }
        PortSide::WEST => {
            port_position.x = -port_width - port_offset;
            port_position.y += padding.top + graph_offset.y - (port_height / 2.0);
            a.node_mut(port_dummy).pos.x = -(padding.left + port_offset + graph_offset.x);
        }
        PortSide::UNDEFINED => {}
    }

    port_position
}

/// Resizes a node to the
/// given width and height, adjusting port and label positions if needed.
pub fn resize_node(
    a: &mut LGraphArena,
    node: LNodeId,
    new_size: KVector,
    move_ports: bool,
    move_labels: bool,
) {
    let old_size = a.node(node).size;

    // These calculations are performed in float!
    let width_ratio = (new_size.x / old_size.x) as f32;
    let height_ratio = (new_size.y / old_size.y) as f32;
    let width_diff = (new_size.x - old_size.x) as f32;
    let height_diff = (new_size.y - old_size.y) as f32;

    // Update port positions
    if move_ports {
        let fixed_ports = a.node(node).properties.get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
            == PortConstraints::FIXED_POS;

        for port in a.node(node).ports.clone() {
            match a.port(port).side {
                PortSide::NORTH => {
                    if !fixed_ports {
                        a.port_mut(port).pos.x *= width_ratio as f64;
                    }
                }
                PortSide::EAST => {
                    a.port_mut(port).pos.x += width_diff as f64;
                    if !fixed_ports {
                        a.port_mut(port).pos.y *= height_ratio as f64;
                    }
                }
                PortSide::SOUTH => {
                    if !fixed_ports {
                        a.port_mut(port).pos.x *= width_ratio as f64;
                    }
                    a.port_mut(port).pos.y += height_diff as f64;
                }
                PortSide::WEST => {
                    if !fixed_ports {
                        a.port_mut(port).pos.y *= height_ratio as f64;
                    }
                }
                PortSide::UNDEFINED => {}
            }
        }
    }

    // Update label positions
    if move_labels {
        for label in a.node(node).labels.clone() {
            let l = a.label(label);
            let midx = l.pos.x + l.size.x / 2.0;
            let midy = l.pos.y + l.size.y / 2.0;
            let width_percent = midx / old_size.x;
            let height_percent = midy / old_size.y;

            if width_percent + height_percent >= 1.0 {
                if width_percent - height_percent > 0.0 && midy >= 0.0 {
                    // label is on the right
                    a.label_mut(label).pos.x += width_diff as f64;
                    a.label_mut(label).pos.y += height_diff as f64 * height_percent;
                } else if width_percent - height_percent < 0.0 && midx >= 0.0 {
                    // label is on the bottom
                    a.label_mut(label).pos.x += width_diff as f64 * width_percent;
                    a.label_mut(label).pos.y += height_diff as f64;
                }
            }
        }
    }

    // Set the new node size
    a.node_mut(node).size = new_size;

    // Set fixed size option for the node: now the size is assumed to stay as
    // determined here. `SizeConstraint.fixed()` == an EMPTY set, which
    // means "fixed size" — the label/node-size processor will not resize it.
    a.node(node)
        .properties
        .set(&lopts::NODE_SIZE_CONSTRAINTS, EnumSet::<SizeConstraint>::none());
}

pub fn is_descendant(a: &LGraphArena, child: LNodeId, parent: LNodeId) -> bool {
    let mut current = child;
    let mut next = a.graph(a.node_graph(current)).parent_node;
    while let Some(n) = next {
        current = n;
        if current == parent {
            return true;
        }
        next = a.graph(a.node_graph(current)).parent_node;
    }
    false
}

/// Converts the given point from the
/// coordinate system of `old_graph` to that of `new_graph`.
pub fn change_coord_system(
    a: &LGraphArena,
    point: &mut KVector,
    old_graph: LGraphId,
    new_graph: LGraphId,
) {
    if old_graph == new_graph {
        // nothing has to be done
        return;
    }

    // transform to absolute coordinates
    let mut graph = old_graph;
    loop {
        point.add(a.graph(graph).offset);
        match a.graph(graph).parent_node {
            Some(node) => {
                let padding = a.graph(graph).padding;
                point.add_xy(padding.left, padding.top);
                point.add(a.node(node).pos);
                graph = a.node_graph(node);
            }
            None => break,
        }
    }

    // transform to relative coordinates (to newGraph)
    let mut graph = new_graph;
    loop {
        point.sub(a.graph(graph).offset);
        match a.graph(graph).parent_node {
            Some(node) => {
                let padding = a.graph(graph).padding;
                point.sub_xy(padding.left, padding.top);
                point.sub(a.node(node).pos);
                graph = a.node_graph(node);
            }
            None => break,
        }
    }
}

pub fn edge_reverse(
    a: &mut LGraphArena,
    graph: LGraphId,
    edge: crate::alg_layered::graph::LEdgeId,
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
        let placement: crate::core::options::EdgeLabelPlacement =
            a.label(label).properties.get(&lopts::EDGE_LABELS_PLACEMENT);
        match placement {
            crate::core::options::EdgeLabelPlacement::TAIL => {
                a.label(label)
                    .properties
                    .set(&lopts::EDGE_LABELS_PLACEMENT, crate::core::options::EdgeLabelPlacement::HEAD);
            }
            crate::core::options::EdgeLabelPlacement::HEAD => {
                a.label(label)
                    .properties
                    .set(&lopts::EDGE_LABELS_PLACEMENT, crate::core::options::EdgeLabelPlacement::TAIL);
            }
            _ => {}
        }
    }

    let reversed: bool = a.edge(edge).properties.get(&iprops::REVERSED);
    a.edge(edge).properties.set(&iprops::REVERSED, !reversed);

    let reversed_bps = crate::graph::math::KVectorChain::reverse(&a.edge(edge).bend_points);
    a.edge_mut(edge).bend_points = reversed_bps;
}
