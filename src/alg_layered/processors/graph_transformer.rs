//! A layout processor that is able to perform
//! transformations on the coordinates of a graph. Used as both the
//! DIRECTION_PREPROCESSOR (`Mode::ToInternalLtr`) and the
//! DIRECTION_POSTPROCESSOR (`Mode::ToInputDirection`).

use crate::core::options::{Alignment, Direction, NodeLabelPlacement, PortSide};
use crate::graph::math::{KVector, Spacing};
use crate::graph::properties::{EnumSet, PropertyMap};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::{
    DirectionCongruency, EdgeLabelSideSelection, InLayerConstraint, LayerConstraint,
};

/// Definition of transformation modes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// the input graph's direction to internal direction (left-to-right).
    ToInternalLtr,
    /// internal direction back to input graph's direction.
    ToInputDirection,
}

impl EdgeLabelSideSelection {
    fn transpose(self) -> EdgeLabelSideSelection {
        match self {
            EdgeLabelSideSelection::ALWAYS_UP => EdgeLabelSideSelection::ALWAYS_DOWN,
            EdgeLabelSideSelection::ALWAYS_DOWN => EdgeLabelSideSelection::ALWAYS_UP,
            EdgeLabelSideSelection::DIRECTION_UP => EdgeLabelSideSelection::DIRECTION_DOWN,
            EdgeLabelSideSelection::DIRECTION_DOWN => EdgeLabelSideSelection::DIRECTION_UP,
            EdgeLabelSideSelection::SMART_UP => EdgeLabelSideSelection::SMART_DOWN,
            EdgeLabelSideSelection::SMART_DOWN => EdgeLabelSideSelection::SMART_UP,
        }
    }
}

pub fn process(a: &mut LGraphArena, graph: LGraphId, mode: Mode) -> Result<(), String> {
    // We need to add all layerless nodes as well as all nodes in layers since this processor
    // is run twice -- once before layering, and once afterwards
    let nodes = a.graph_all_nodes(graph);

    // graph transformations for unusual layout directions
    let congruency: DirectionCongruency =
        a.graph(graph).properties.get(&lopts::DIRECTION_CONGRUENCY);
    let direction: Direction = a.graph(graph).properties.get(&lopts::DIRECTION);
    if congruency == DirectionCongruency::READING_DIRECTION {
        // --------------------------------------------------------------
        //          variant that preserves reading direction
        // --------------------------------------------------------------
        match direction {
            Direction::LEFT => mirror_all_x(a, graph, &nodes),
            Direction::DOWN => transpose_all(a, graph, &nodes),
            Direction::UP => {
                if mode == Mode::ToInternalLtr {
                    transpose_all(a, graph, &nodes);
                    mirror_all_y(a, graph, &nodes);
                } else {
                    mirror_all_y(a, graph, &nodes);
                    transpose_all(a, graph, &nodes);
                }
            }
            _ => {}
        }
    } else if mode == Mode::ToInternalLtr {
        // --------------------------------------------------------------
        //          to internally used left-to-right direction
        // --------------------------------------------------------------
        match direction {
            Direction::LEFT => {
                mirror_all_x(a, graph, &nodes);
                mirror_all_y(a, graph, &nodes);
            }
            Direction::DOWN => rotate90_clockwise(a, graph, &nodes),
            Direction::UP => rotate90_counter_clockwise(a, graph, &nodes),
            _ => {}
        }
    } else {
        // --------------------------------------------------------------
        //                 back to original direction
        // --------------------------------------------------------------
        match direction {
            Direction::LEFT => {
                mirror_all_x(a, graph, &nodes);
                mirror_all_y(a, graph, &nodes);
            }
            Direction::DOWN => rotate90_counter_clockwise(a, graph, &nodes),
            Direction::UP => rotate90_clockwise(a, graph, &nodes),
            _ => {}
        }
    }
    Ok(())
}

///////////////////////////////////////////////////////////////////////////////
// Convenience

fn rotate90_clockwise(a: &mut LGraphArena, graph: LGraphId, nodes: &[LNodeId]) {
    transpose_all(a, graph, nodes);
    mirror_all_x(a, graph, nodes);
}

fn rotate90_counter_clockwise(a: &mut LGraphArena, graph: LGraphId, nodes: &[LNodeId]) {
    mirror_all_x(a, graph, nodes);
    transpose_all(a, graph, nodes);
}

fn mirror_all_x(a: &mut LGraphArena, graph: LGraphId, nodes: &[LNodeId]) {
    mirror_x_nodes(a, nodes, graph);
    mirror_spacing_x(&mut a.graph_mut(graph).padding);
    // materializes the cloneable default and mutates it in place.
    let mut padding = a.graph(graph).properties.get(&lopts::NODE_LABELS_PADDING);
    mirror_spacing_x(&mut padding);
    a.graph(graph).properties.set(&lopts::NODE_LABELS_PADDING, padding);
}

fn mirror_all_y(a: &mut LGraphArena, graph: LGraphId, nodes: &[LNodeId]) {
    mirror_y_nodes(a, nodes, graph);
    mirror_spacing_y(&mut a.graph_mut(graph).padding);
    let mut padding = a.graph(graph).properties.get(&lopts::NODE_LABELS_PADDING);
    mirror_spacing_y(&mut padding);
    a.graph(graph).properties.set(&lopts::NODE_LABELS_PADDING, padding);
}

fn transpose_all(a: &mut LGraphArena, graph: LGraphId, nodes: &[LNodeId]) {
    transpose_nodes(a, nodes);
    transpose_edge_label_placement(a, graph);
    {
        let g = a.graph_mut(graph);
        transpose_vec(&mut g.offset);
        transpose_vec(&mut g.size);
        transpose_spacing(&mut g.padding);
    }
    let mut padding = a.graph(graph).properties.get(&lopts::NODE_LABELS_PADDING);
    transpose_spacing(&mut padding);
    a.graph(graph).properties.set(&lopts::NODE_LABELS_PADDING, padding);
}

///////////////////////////////////////////////////////////////////////////////
// Mirror Horizontally

fn mirror_x_nodes(a: &mut LGraphArena, nodes: &[LNodeId], graph: LGraphId) {
    /* Assuming that no nodes extend into negative x coordinates, mirroring a node means that the
     * space left to its left border equals the space right to its right border when mirrored. In
     * mathematical terms:
     *     oldPosition.x = graphWidth - newPosition.x - nodeWidth
     * We use the offset variable to store graphWidth, since that's the constant offset against
     * which we calculate the new node positions. Once nodes are allowed to extend into negative
     * coordinates, we have to subtract from the graphWidth the amount of space the graph extends
     * into negative coordinates, which is saved in the graph's graphOffset. */
    let (graph_size_x, graph_offset_x) = {
        let g = a.graph(graph);
        (g.size.x, g.offset.x)
    };

    // If the graph already had its size calculated, use that; if not, find its width by iterating
    // over its nodes
    let mut offset = 0.0;
    if graph_size_x == 0.0 {
        for &node in nodes {
            let n = a.node(node);
            offset = f64::max(offset, n.pos.x + n.size.x + n.margin.right);
        }
    } else {
        offset = graph_size_x - graph_offset_x;
    }
    offset -= graph_offset_x;

    // mirror all nodes, ports, edges, and labels
    for &node in nodes {
        let node_size = a.node(node).size;
        {
            let n = a.node_mut(node);
            n.pos.x = (offset - node_size.x) - n.pos.x;
            mirror_spacing_x(&mut n.padding);
        }
        mirror_node_label_placement_x(&a.node(node).properties);

        // mirror position
        if a.node(node).properties.has(&lopts::POSITION) {
            let mut pos: KVector = a.node(node).properties.try_get(&lopts::POSITION).unwrap();
            pos.x = (offset - node_size.x) - pos.x;
            a.node(node).properties.set(&lopts::POSITION, pos);
        }

        // mirror the alignment
        match a.node(node).properties.get(&lopts::ALIGNMENT) {
            Alignment::LEFT => {
                a.node(node).properties.set(&lopts::ALIGNMENT, Alignment::RIGHT);
            }
            Alignment::RIGHT => {
                a.node(node).properties.set(&lopts::ALIGNMENT, Alignment::LEFT);
            }
            _ => {}
        }

        let ports = a.node(node).ports.clone();
        for port in ports {
            {
                let p = a.port_mut(port);
                p.pos.x = (node_size.x - p.size.x) - p.pos.x;
                p.anchor.x = p.size.x - p.anchor.x;
            }
            // setting the side recomputes the anchor unless it was explicitly supplied
            let mirrored_side = mirrored_port_side_x(a.port(port).side);
            a.port_set_side(port, mirrored_side);
            reverse_index(a, port);

            let outgoing = a.port(port).outgoing_edges.clone();
            for edge in outgoing {
                // Mirror bend points
                for bend_point in a.edge_mut(edge).bend_points.0.iter_mut() {
                    bend_point.x = offset - bend_point.x;
                }

                // Mirror junction points (materializes the empty default)
                if let Some(mut junction_points) =
                    a.edge(edge).properties.get_opt(&lopts::JUNCTION_POINTS)
                {
                    for jp in junction_points.0.iter_mut() {
                        jp.x = offset - jp.x;
                    }
                    a.edge(edge).properties.set(&lopts::JUNCTION_POINTS, junction_points);
                }

                // Mirror edge label positions
                let labels = a.edge(edge).labels.clone();
                for label in labels {
                    let l = a.label_mut(label);
                    l.pos.x = (offset - l.size.x) - l.pos.x;
                }
            }

            // Mirror port label positions
            let port_size_x = a.port(port).size.x;
            let labels = a.port(port).labels.clone();
            for label in labels {
                let l = a.label_mut(label);
                l.pos.x = (port_size_x - l.size.x) - l.pos.x;
            }
        }

        // External port dummy?
        if a.node(node).node_type == NodeType::EXTERNAL_PORT {
            let ext_side = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
            a.node(node)
                .properties
                .set(&iprops::EXT_PORT_SIDE, mirrored_port_side_x(ext_side));
            mirror_layer_constraint_x(&a.node(node).properties);
        }

        // Mirror node labels
        let labels = a.node(node).labels.clone();
        for label in labels {
            mirror_node_label_placement_x(&a.label(label).properties);
            let l = a.label_mut(label);
            l.pos.x = (node_size.x - l.size.x) - l.pos.x;
        }
    }
}

/// Mirrors the given spacing in X direction.
fn mirror_spacing_x(spacing: &mut Spacing) {
    std::mem::swap(&mut spacing.left, &mut spacing.right);
}

/// Horizontally mirrors the node label
/// placement options, if any are set. (`shape` is a node or label.)
fn mirror_node_label_placement_x(props: &PropertyMap) {
    if !props.has(&lopts::NODE_LABELS_PLACEMENT) {
        return;
    }

    let mut placement: EnumSet<NodeLabelPlacement> =
        props.try_get(&lopts::NODE_LABELS_PLACEMENT).unwrap();
    if placement.contains(NodeLabelPlacement::H_LEFT) {
        placement.remove(NodeLabelPlacement::H_LEFT);
        placement.add(NodeLabelPlacement::H_RIGHT);
    } else if placement.contains(NodeLabelPlacement::H_RIGHT) {
        placement.remove(NodeLabelPlacement::H_RIGHT);
        placement.add(NodeLabelPlacement::H_LEFT);
    }
    props.set(&lopts::NODE_LABELS_PLACEMENT, placement);
}

/// The port side that is horizontally
/// mirrored from the given side.
fn mirrored_port_side_x(side: PortSide) -> PortSide {
    match side {
        PortSide::EAST => PortSide::WEST,
        PortSide::WEST => PortSide::EAST,
        _ => side,
    }
}

/// Horizontally mirrors the layer
/// constraint set on a node (only meant for external port dummy nodes).
fn mirror_layer_constraint_x(props: &PropertyMap) {
    match props.get(&lopts::LAYERING_LAYER_CONSTRAINT) {
        LayerConstraint::FIRST => {
            props.set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::LAST);
        }
        LayerConstraint::FIRST_SEPARATE => {
            props.set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::LAST_SEPARATE);
        }
        LayerConstraint::LAST => {
            props.set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::FIRST);
        }
        LayerConstraint::LAST_SEPARATE => {
            props.set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::FIRST_SEPARATE);
        }
        _ => {}
    }
}

///////////////////////////////////////////////////////////////////////////////
// Mirror Vertically

fn mirror_y_nodes(a: &mut LGraphArena, nodes: &[LNodeId], graph: LGraphId) {
    // See mirror_x_nodes for an explanation of how the offset is calculated
    let (graph_size_y, graph_offset_y) = {
        let g = a.graph(graph);
        (g.size.y, g.offset.y)
    };

    let mut offset = 0.0;
    if graph_size_y == 0.0 {
        for &node in nodes {
            let n = a.node(node);
            offset = f64::max(offset, n.pos.y + n.size.y + n.margin.bottom);
        }
    } else {
        offset = graph_size_y - graph_offset_y;
    }
    offset -= graph_offset_y;

    // mirror all nodes, ports, edges, and labels
    for &node in nodes {
        let node_size = a.node(node).size;
        {
            let n = a.node_mut(node);
            n.pos.y = (offset - node_size.y) - n.pos.y;
            mirror_spacing_y(&mut n.padding);
        }
        mirror_node_label_placement_y(&a.node(node).properties);

        // mirror position
        if a.node(node).properties.has(&lopts::POSITION) {
            let mut pos: KVector = a.node(node).properties.try_get(&lopts::POSITION).unwrap();
            pos.y = (offset - node_size.y) - pos.y;
            a.node(node).properties.set(&lopts::POSITION, pos);
        }

        // mirror the alignment
        match a.node(node).properties.get(&lopts::ALIGNMENT) {
            Alignment::TOP => {
                a.node(node).properties.set(&lopts::ALIGNMENT, Alignment::BOTTOM);
            }
            Alignment::BOTTOM => {
                a.node(node).properties.set(&lopts::ALIGNMENT, Alignment::TOP);
            }
            _ => {}
        }

        let ports = a.node(node).ports.clone();
        for port in ports {
            {
                let p = a.port_mut(port);
                p.pos.y = (node_size.y - p.size.y) - p.pos.y;
                p.anchor.y = p.size.y - p.anchor.y;
            }
            let mirrored_side = mirrored_port_side_y(a.port(port).side);
            a.port_set_side(port, mirrored_side);
            reverse_index(a, port);

            let outgoing = a.port(port).outgoing_edges.clone();
            for edge in outgoing {
                // Mirror bend points
                for bend_point in a.edge_mut(edge).bend_points.0.iter_mut() {
                    bend_point.y = offset - bend_point.y;
                }

                // Mirror junction points
                if let Some(mut junction_points) =
                    a.edge(edge).properties.get_opt(&lopts::JUNCTION_POINTS)
                {
                    for jp in junction_points.0.iter_mut() {
                        jp.y = offset - jp.y;
                    }
                    a.edge(edge).properties.set(&lopts::JUNCTION_POINTS, junction_points);
                }

                // Mirror edge label positions
                let labels = a.edge(edge).labels.clone();
                for label in labels {
                    let l = a.label_mut(label);
                    l.pos.y = (offset - l.size.y) - l.pos.y;
                }
            }

            // Mirror port label positions
            let port_size_y = a.port(port).size.y;
            let labels = a.port(port).labels.clone();
            for label in labels {
                let l = a.label_mut(label);
                l.pos.y = (port_size_y - l.size.y) - l.pos.y;
            }
        }

        // External port dummy?
        if a.node(node).node_type == NodeType::EXTERNAL_PORT {
            let ext_side = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
            a.node(node)
                .properties
                .set(&iprops::EXT_PORT_SIDE, mirrored_port_side_y(ext_side));
            mirror_in_layer_constraint_y(&a.node(node).properties);
        }

        // Mirror node labels
        let labels = a.node(node).labels.clone();
        for label in labels {
            mirror_node_label_placement_y(&a.label(label).properties);
            let l = a.label_mut(label);
            l.pos.y = (node_size.y - l.size.y) - l.pos.y;
        }
    }
}

/// Mirrors the given spacing in Y direction.
fn mirror_spacing_y(spacing: &mut Spacing) {
    std::mem::swap(&mut spacing.top, &mut spacing.bottom);
}

/// Vertically mirrors the node label
/// placement options, if any are set.
fn mirror_node_label_placement_y(props: &PropertyMap) {
    if !props.has(&lopts::NODE_LABELS_PLACEMENT) {
        return;
    }

    let mut placement: EnumSet<NodeLabelPlacement> =
        props.try_get(&lopts::NODE_LABELS_PLACEMENT).unwrap();
    if placement.contains(NodeLabelPlacement::V_TOP) {
        placement.remove(NodeLabelPlacement::V_TOP);
        placement.add(NodeLabelPlacement::V_BOTTOM);
    } else if placement.contains(NodeLabelPlacement::V_BOTTOM) {
        placement.remove(NodeLabelPlacement::V_BOTTOM);
        placement.add(NodeLabelPlacement::V_TOP);
    }
    props.set(&lopts::NODE_LABELS_PLACEMENT, placement);
}

/// The port side that is vertically mirrored
/// from the given side.
fn mirrored_port_side_y(side: PortSide) -> PortSide {
    match side {
        PortSide::NORTH => PortSide::SOUTH,
        PortSide::SOUTH => PortSide::NORTH,
        _ => side,
    }
}

/// Vertically mirrors the in-layer
/// constraint set on a node (only meant for external port dummy nodes).
fn mirror_in_layer_constraint_y(props: &PropertyMap) {
    match props.get(&iprops::IN_LAYER_CONSTRAINT) {
        InLayerConstraint::TOP => {
            props.set(&iprops::IN_LAYER_CONSTRAINT, InLayerConstraint::BOTTOM);
        }
        InLayerConstraint::BOTTOM => {
            props.set(&iprops::IN_LAYER_CONSTRAINT, InLayerConstraint::TOP);
        }
        _ => {}
    }
}

///////////////////////////////////////////////////////////////////////////////
// Transpose

fn transpose_nodes(a: &mut LGraphArena, nodes: &[LNodeId]) {
    // Transpose nodes
    for &node in nodes {
        {
            let n = a.node_mut(node);
            transpose_vec(&mut n.pos);
            transpose_vec(&mut n.size);
            transpose_spacing(&mut n.padding);
        }
        transpose_node_label_placement(&a.node(node).properties);
        transpose_properties(&a.node(node).properties);

        // Transpose ports
        let ports = a.node(node).ports.clone();
        for port in ports {
            {
                let p = a.port_mut(port);
                transpose_vec(&mut p.pos);
                transpose_vec(&mut p.anchor);
                transpose_vec(&mut p.size);
            }
            let transposed_side = transposed_port_side(a.port(port).side);
            a.port_set_side(port, transposed_side);
            reverse_index(a, port);

            // Transpose edges
            let outgoing = a.port(port).outgoing_edges.clone();
            for edge in outgoing {
                // Transpose bend points
                for bend_point in a.edge_mut(edge).bend_points.0.iter_mut() {
                    transpose_vec(bend_point);
                }

                // Transpose junction points
                if let Some(mut junction_points) =
                    a.edge(edge).properties.get_opt(&lopts::JUNCTION_POINTS)
                {
                    for jp in junction_points.0.iter_mut() {
                        transpose_vec(jp);
                    }
                    a.edge(edge).properties.set(&lopts::JUNCTION_POINTS, junction_points);
                }

                // Transpose edge labels
                let labels = a.edge(edge).labels.clone();
                for label in labels {
                    let l = a.label_mut(label);
                    transpose_vec(&mut l.pos);
                    transpose_vec(&mut l.size);
                }
            }

            // Transpose port labels
            let labels = a.port(port).labels.clone();
            for label in labels {
                let l = a.label_mut(label);
                transpose_vec(&mut l.pos);
                transpose_vec(&mut l.size);
            }
        }

        // External port dummy?
        if a.node(node).node_type == NodeType::EXTERNAL_PORT {
            let ext_side = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
            a.node(node)
                .properties
                .set(&iprops::EXT_PORT_SIDE, transposed_port_side(ext_side));
            transpose_layer_constraint(&a.node(node).properties);
        }

        // Transpose node labels
        let labels = a.node(node).labels.clone();
        for label in labels {
            transpose_node_label_placement(&a.label(label).properties);
            let l = a.label_mut(label);
            transpose_vec(&mut l.size);
            transpose_vec(&mut l.pos);
        }
    }
}

fn transpose_vec(v: &mut KVector) {
    std::mem::swap(&mut v.x, &mut v.y);
}

fn transpose_spacing(spacing: &mut Spacing) {
    let old = *spacing;
    spacing.top = old.left;
    spacing.bottom = old.right;
    spacing.left = old.top;
    spacing.right = old.bottom;
}

/// Transposes the node label placement
/// options, if any are set.
fn transpose_node_label_placement(props: &PropertyMap) {
    if !props.has(&lopts::NODE_LABELS_PLACEMENT) {
        return;
    }
    let old_placement: EnumSet<NodeLabelPlacement> =
        props.try_get(&lopts::NODE_LABELS_PLACEMENT).unwrap();
    if old_placement.is_empty() {
        return;
    }

    // Build up a new node label placement enumeration
    let mut new_placement: EnumSet<NodeLabelPlacement> = EnumSet::none();

    // Inside or outside
    if old_placement.contains(NodeLabelPlacement::INSIDE) {
        new_placement.add(NodeLabelPlacement::INSIDE);
    } else {
        new_placement.add(NodeLabelPlacement::OUTSIDE);
    }

    // Horizontal priority
    if !old_placement.contains(NodeLabelPlacement::H_PRIORITY) {
        new_placement.add(NodeLabelPlacement::H_PRIORITY);
    }

    // Horizontal alignment
    if old_placement.contains(NodeLabelPlacement::H_LEFT) {
        new_placement.add(NodeLabelPlacement::V_TOP);
    } else if old_placement.contains(NodeLabelPlacement::H_CENTER) {
        new_placement.add(NodeLabelPlacement::V_CENTER);
    } else if old_placement.contains(NodeLabelPlacement::H_RIGHT) {
        new_placement.add(NodeLabelPlacement::V_BOTTOM);
    }

    // Vertical alignment
    if old_placement.contains(NodeLabelPlacement::V_TOP) {
        new_placement.add(NodeLabelPlacement::H_LEFT);
    } else if old_placement.contains(NodeLabelPlacement::V_CENTER) {
        new_placement.add(NodeLabelPlacement::H_CENTER);
    } else if old_placement.contains(NodeLabelPlacement::V_BOTTOM) {
        new_placement.add(NodeLabelPlacement::H_RIGHT);
    }

    // Apply new placement
    props.set(&lopts::NODE_LABELS_PLACEMENT, new_placement);
}

/// The transposed side of the given
/// port side.
fn transposed_port_side(side: PortSide) -> PortSide {
    match side {
        PortSide::NORTH => PortSide::WEST,
        PortSide::WEST => PortSide::NORTH,
        PortSide::SOUTH => PortSide::EAST,
        PortSide::EAST => PortSide::SOUTH,
        _ => PortSide::UNDEFINED,
    }
}

/// Transposes the placement of edge
/// labels in the graph.
fn transpose_edge_label_placement(a: &mut LGraphArena, graph: LGraphId) {
    // the option has a non-null default, so the transposed value is always set.
    let old_side: EdgeLabelSideSelection =
        a.graph(graph).properties.get(&lopts::EDGE_LABELS_SIDE_SELECTION);
    a.graph(graph)
        .properties
        .set(&lopts::EDGE_LABELS_SIDE_SELECTION, old_side.transpose());
}

/// Transposes the layer constraint and
/// in-layer constraint set on a node. Only meant for external port dummy
/// nodes and only supports the cases that can occur with them.
fn transpose_layer_constraint(props: &PropertyMap) {
    let layer_constraint: LayerConstraint = props.get(&lopts::LAYERING_LAYER_CONSTRAINT);
    let in_layer_constraint: InLayerConstraint = props.get(&iprops::IN_LAYER_CONSTRAINT);

    if layer_constraint == LayerConstraint::FIRST_SEPARATE {
        props.set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::NONE);
        props.set(&iprops::IN_LAYER_CONSTRAINT, InLayerConstraint::TOP);
    } else if layer_constraint == LayerConstraint::LAST_SEPARATE {
        props.set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::NONE);
        props.set(&iprops::IN_LAYER_CONSTRAINT, InLayerConstraint::BOTTOM);
    } else if in_layer_constraint == InLayerConstraint::TOP {
        props.set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::FIRST_SEPARATE);
        props.set(&iprops::IN_LAYER_CONSTRAINT, InLayerConstraint::NONE);
    } else if in_layer_constraint == InLayerConstraint::BOTTOM {
        props.set(&lopts::LAYERING_LAYER_CONSTRAINT, LayerConstraint::LAST_SEPARATE);
        props.set(&iprops::IN_LAYER_CONSTRAINT, InLayerConstraint::NONE);
    }
}

/// Checks a node's properties for ones that
/// need to be transposed (NODE_SIZE_MINIMUM, ALIGNMENT, POSITION).
fn transpose_properties(props: &PropertyMap) {
    // Transpose MIN_HEIGHT and MIN_WIDTH
    let min_size: KVector = props.get(&lopts::NODE_SIZE_MINIMUM);
    props.set(&lopts::NODE_SIZE_MINIMUM, KVector::new(min_size.y, min_size.x));

    // Transpose ALIGNMENT
    match props.get(&lopts::ALIGNMENT) {
        Alignment::LEFT => {
            props.set(&lopts::ALIGNMENT, Alignment::TOP);
        }
        Alignment::RIGHT => {
            props.set(&lopts::ALIGNMENT, Alignment::BOTTOM);
        }
        Alignment::TOP => {
            props.set(&lopts::ALIGNMENT, Alignment::LEFT);
        }
        Alignment::BOTTOM => {
            props.set(&lopts::ALIGNMENT, Alignment::RIGHT);
        }
        _ => {}
    }

    // POSITION
    if props.has(&lopts::POSITION) {
        let mut pos: KVector = props.try_get(&lopts::POSITION).unwrap();
        transpose_vec(&mut pos);
        props.set(&lopts::POSITION, pos);
    }
}

/// Reverses the port index.
fn reverse_index(a: &LGraphArena, port: LPortId) {
    if let Some(index) = a.port(port).properties.try_get(&lopts::PORT_INDEX) {
        a.port(port).properties.set(&lopts::PORT_INDEX, -index);
    }
}
