//! A uniform,
//! mutable view over a graph model, used by the node-sizing code in
//! elk-alg-common so it can operate on both `ElkGraph` and the layered
//! algorithm's `LGraph`.
//!
//! A single trait exposes everything through copyable element ids.

use crate::graph::graph::{EdgeId, ElkGraph, LabelId, NodeId, PortId, ShapeId};
use crate::graph::math::{KVector, Spacing};
use crate::graph::properties::PropertyMap;

use crate::core::options::PortSide;

crate::elk_enum! {
    pub enum LabelSide {
        UNKNOWN,
        ABOVE,
        BELOW,
        INLINE,
    }
}

/// The adapter trait. `N`/`P`/`L`/`E` identify nodes, ports, labels, edges.
pub trait AdapterGraph {
    type N: Copy + Eq + std::fmt::Debug;
    type P: Copy + Eq + std::fmt::Debug;
    type L: Copy + Eq + std::fmt::Debug;
    type E: Copy + Eq + std::fmt::Debug;

    // ------------------------------------------------------------ graph
    fn graph_properties(&self) -> &PropertyMap;
    fn nodes(&self) -> Vec<Self::N>;

    // ------------------------------------------------------------- node
    fn node_size(&self, n: Self::N) -> KVector;
    fn set_node_size(&mut self, n: Self::N, size: KVector);
    fn node_position(&self, n: Self::N) -> KVector;
    fn set_node_position(&mut self, n: Self::N, pos: KVector);
    fn node_properties(&self, n: Self::N) -> &PropertyMap;
    fn node_labels(&self, n: Self::N) -> Vec<Self::L>;
    fn node_ports(&self, n: Self::N) -> Vec<Self::P>;
    fn node_incoming_edges(&self, n: Self::N) -> Vec<Self::E>;
    fn node_outgoing_edges(&self, n: Self::N) -> Vec<Self::E>;
    /// Clockwise order (used by ElkGraph adapter; the LGraph adapter sorts by
    /// the PortListSorter comparator).
    fn sort_port_list(&mut self, n: Self::N);
    fn is_compound_node(&self, n: Self::N) -> bool;
    fn node_padding(&self, n: Self::N) -> Spacing;
    fn set_node_padding(&mut self, n: Self::N, padding: Spacing);
    fn node_margin(&self, n: Self::N) -> Spacing;
    fn set_node_margin(&mut self, n: Self::N, margin: Spacing);

    // ------------------------------------------------------------- port
    fn port_side(&self, p: Self::P) -> PortSide;
    fn port_size(&self, p: Self::P) -> KVector;
    fn set_port_size(&mut self, p: Self::P, size: KVector);
    fn port_position(&self, p: Self::P) -> KVector;
    fn set_port_position(&mut self, p: Self::P, pos: KVector);
    fn port_properties(&self, p: Self::P) -> &PropertyMap;
    fn port_labels(&self, p: Self::P) -> Vec<Self::L>;
    fn port_margin(&self, p: Self::P) -> Spacing;
    fn set_port_margin(&mut self, p: Self::P, margin: Spacing);
    fn port_incoming_edges(&self, p: Self::P) -> Vec<Self::E>;
    fn port_outgoing_edges(&self, p: Self::P) -> Vec<Self::E>;
    fn port_has_compound_connections(&self, p: Self::P) -> bool;

    // ------------------------------------------------------------ label
    fn label_size(&self, l: Self::L) -> KVector;
    fn set_label_size(&mut self, l: Self::L, size: KVector);
    fn label_position(&self, l: Self::L) -> KVector;
    fn set_label_position(&mut self, l: Self::L, pos: KVector);
    fn label_properties(&self, l: Self::L) -> &PropertyMap;
    fn label_side(&self, l: Self::L) -> LabelSide;
    fn label_text(&self, l: Self::L) -> String;

    // ------------------------------------------------------------- edge
    fn edge_labels(&self, e: Self::E) -> Vec<Self::L>;
}

/// Adapter over the original `ElkGraph`, viewing
/// the children of `parent` as the graph's nodes.
pub struct ElkGraphAdapter<'g> {
    pub elk: &'g mut ElkGraph,
    pub parent: NodeId,
    /// Node adapters can have a *null* parent graph adapter
    /// (`adaptSingleNode` of a node without a parent);
    /// graph-level property lookups then fall back to the property defaults
    /// without materializing them on any real element. This scratch map
    /// absorbs those lookups.
    null_graph_properties: Option<PropertyMap>,
}

impl<'g> ElkGraphAdapter<'g> {
    pub fn new(elk: &'g mut ElkGraph, parent: NodeId) -> Self {
        ElkGraphAdapter { elk, parent, null_graph_properties: None }
    }

    pub fn adapt_single_node(elk: &'g mut ElkGraph, node: NodeId) -> Self {
        match elk.node(node).parent {
            Some(parent) => Self::new(elk, parent),
            None => ElkGraphAdapter {
                elk,
                parent: node,
                null_graph_properties: Some(PropertyMap::new()),
            },
        }
    }
}

impl<'g> AdapterGraph for ElkGraphAdapter<'g> {
    type N = NodeId;
    type P = PortId;
    type L = LabelId;
    type E = EdgeId;

    fn graph_properties(&self) -> &PropertyMap {
        match &self.null_graph_properties {
            Some(map) => map,
            None => &self.elk.node(self.parent).properties,
        }
    }
    fn nodes(&self) -> Vec<NodeId> {
        self.elk.node(self.parent).children.clone()
    }

    fn node_size(&self, n: NodeId) -> KVector {
        let s = &self.elk.node(n).shape;
        KVector::new(s.width, s.height)
    }
    fn set_node_size(&mut self, n: NodeId, size: KVector) {
        self.elk.node_mut(n).shape.set_dimensions(size.x, size.y);
    }
    fn node_position(&self, n: NodeId) -> KVector {
        let s = &self.elk.node(n).shape;
        KVector::new(s.x, s.y)
    }
    fn set_node_position(&mut self, n: NodeId, pos: KVector) {
        self.elk.node_mut(n).shape.set_location(pos.x, pos.y);
    }
    fn node_properties(&self, n: NodeId) -> &PropertyMap {
        &self.elk.node(n).properties
    }
    fn node_labels(&self, n: NodeId) -> Vec<LabelId> {
        self.elk.node(n).labels.clone()
    }
    fn node_ports(&self, n: NodeId) -> Vec<PortId> {
        self.elk.node(n).ports.clone()
    }
    fn node_incoming_edges(&self, n: NodeId) -> Vec<EdgeId> {
        // node + its ports
        let mut edges = self.elk.node(n).incoming_edges.clone();
        for &port in &self.elk.node(n).ports {
            edges.extend(self.elk.port(port).incoming_edges.iter().copied());
        }
        edges
    }
    fn node_outgoing_edges(&self, n: NodeId) -> Vec<EdgeId> {
        let mut edges = self.elk.node(n).outgoing_edges.clone();
        for &port in &self.elk.node(n).ports {
            edges.extend(self.elk.port(port).outgoing_edges.iter().copied());
        }
        edges
    }
    fn sort_port_list(&mut self, n: NodeId) {
        // ElkGraphAdapters.ElkNodeAdapter.sortPortList: sorts by
        // side (N, E, S, W) and position, with the DEFAULT_PORT_COMPARATOR.
        let elk: &ElkGraph = self.elk;
        let mut ports = elk.node(n).ports.clone();
        ports.sort_by(|&a, &b| default_port_comparator(elk, a, b));
        self.elk.node_mut(n).ports = ports;
    }
    fn is_compound_node(&self, n: NodeId) -> bool {
        !self.elk.node(n).children.is_empty()
    }
    fn node_padding(&self, n: NodeId) -> Spacing {
        self.elk
            .node(n)
            .properties
            .get(&crate::core::options::PADDING)
    }
    fn set_node_padding(&mut self, n: NodeId, padding: Spacing) {
        self.elk
            .node(n)
            .properties
            .set(&crate::core::options::PADDING, padding);
    }
    fn node_margin(&self, n: NodeId) -> Spacing {
        self.elk.node(n).properties.get(&crate::core::options::MARGINS)
    }
    fn set_node_margin(&mut self, n: NodeId, margin: Spacing) {
        self.elk.node(n).properties.set(&crate::core::options::MARGINS, margin);
    }

    fn port_side(&self, p: PortId) -> PortSide {
        self.elk.port(p).properties.get(&crate::core::options::PORT_SIDE)
    }
    fn port_size(&self, p: PortId) -> KVector {
        let s = &self.elk.port(p).shape;
        KVector::new(s.width, s.height)
    }
    fn set_port_size(&mut self, p: PortId, size: KVector) {
        self.elk.port_mut(p).shape.set_dimensions(size.x, size.y);
    }
    fn port_position(&self, p: PortId) -> KVector {
        let s = &self.elk.port(p).shape;
        KVector::new(s.x, s.y)
    }
    fn set_port_position(&mut self, p: PortId, pos: KVector) {
        self.elk.port_mut(p).shape.set_location(pos.x, pos.y);
    }
    fn port_properties(&self, p: PortId) -> &PropertyMap {
        &self.elk.port(p).properties
    }
    fn port_labels(&self, p: PortId) -> Vec<LabelId> {
        self.elk.port(p).labels.clone()
    }
    fn port_margin(&self, p: PortId) -> Spacing {
        self.elk.port(p).properties.get(&crate::core::options::MARGINS)
    }
    fn set_port_margin(&mut self, p: PortId, margin: Spacing) {
        self.elk.port(p).properties.set(&crate::core::options::MARGINS, margin);
    }
    fn port_incoming_edges(&self, p: PortId) -> Vec<EdgeId> {
        self.elk.port(p).incoming_edges.clone()
    }
    fn port_outgoing_edges(&self, p: PortId) -> Vec<EdgeId> {
        self.elk.port(p).outgoing_edges.clone()
    }
    fn port_has_compound_connections(&self, p: PortId) -> bool {
        // ElkGraphAdapters.ElkPortAdapter.hasCompoundConnections
        let port_parent = self.elk.port(p).parent.unwrap();
        for &edge in &self.elk.port(p).outgoing_edges {
            for &target in &self.elk.edge(edge).targets {
                let target_node = self.elk.shape_node(target);
                if self.elk.is_descendant(target_node, port_parent) {
                    return true;
                }
            }
        }
        for &edge in &self.elk.port(p).incoming_edges {
            for &source in &self.elk.edge(edge).sources {
                let source_node = self.elk.shape_node(source);
                if self.elk.is_descendant(source_node, port_parent) {
                    return true;
                }
            }
        }
        false
    }

    fn label_size(&self, l: LabelId) -> KVector {
        let s = &self.elk.label(l).shape;
        KVector::new(s.width, s.height)
    }
    fn set_label_size(&mut self, l: LabelId, size: KVector) {
        self.elk.label_mut(l).shape.set_dimensions(size.x, size.y);
    }
    fn label_position(&self, l: LabelId) -> KVector {
        let s = &self.elk.label(l).shape;
        KVector::new(s.x, s.y)
    }
    fn set_label_position(&mut self, l: LabelId, pos: KVector) {
        self.elk.label_mut(l).shape.set_location(pos.x, pos.y);
    }
    fn label_properties(&self, l: LabelId) -> &PropertyMap {
        &self.elk.label(l).properties
    }
    fn label_side(&self, _l: LabelId) -> LabelSide {
        // ElkGraph labels have no label side information
        LabelSide::UNKNOWN
    }
    fn label_text(&self, l: LabelId) -> String {
        self.elk.label(l).text.clone()
    }

    fn edge_labels(&self, e: EdgeId) -> Vec<LabelId> {
        self.elk.edge(e).labels.clone()
    }
}

fn default_port_comparator(elk: &ElkGraph, p1: PortId, p2: PortId) -> std::cmp::Ordering {
    use crate::graph::properties::ElkEnum;
    let side1: PortSide = elk.port(p1).properties.get(&crate::core::options::PORT_SIDE);
    let side2: PortSide = elk.port(p2).properties.get(&crate::core::options::PORT_SIDE);
    let ordinal_difference = side1.ordinal() as i32 - side2.ordinal() as i32;
    if ordinal_difference != 0 {
        return ordinal_difference.cmp(&0);
    }
    // In case of equal sides, sort by port index (if set on both)
    let index1 = elk.port(p1).properties.try_get(&crate::core::options::PORT_INDEX);
    let index2 = elk.port(p2).properties.try_get(&crate::core::options::PORT_INDEX);
    if let (Some(i1), Some(i2)) = (index1, index2) {
        if i1 != i2 {
            return i1.cmp(&i2);
        }
    }
    // In case of equal index, sort by position
    let s1 = &elk.port(p1).shape;
    let s2 = &elk.port(p2).shape;
    match side1 {
        PortSide::NORTH => s1.x.total_cmp(&s2.x),
        PortSide::EAST => s1.y.total_cmp(&s2.y),
        PortSide::SOUTH => s2.x.total_cmp(&s1.x),
        PortSide::WEST => s2.y.total_cmp(&s1.y),
        PortSide::UNDEFINED => std::cmp::Ordering::Equal,
    }
}

/// Edge incident to a node or its ports; free function shared by users of the
/// adapter.
pub fn all_incident_shapes(_elk: &ElkGraph, _shape: ShapeId) -> Vec<ShapeId> {
    unimplemented!("extend when needed")
}
