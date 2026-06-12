//! Port of `LGraphAdapters`: exposes the LGraph through the
//! `elk_core::adapters::AdapterGraph` trait for the node-sizing code.

use elk_core::adapters::{AdapterGraph, LabelSide};
use elk_core::options::{PortConstraints, PortSide};
use elk_graph::math::{KVector, Spacing};
use elk_graph::properties::{Property, PropertyMap};

use crate::graph::{LEdgeId, LGraphArena, LGraphId, LLabelId, LNodeId, LPortId, NodeType};
use crate::internal_properties as iprops;
use crate::options_gen as lopts;

/// Port of `LabelSide.LABEL_SIDE` (property on labels).
pub static LABEL_SIDE: Property<LabelSide> =
    Property::with_default("org.eclipse.elk.labelSide", || LabelSide::UNKNOWN);

/// Port of `LGraphAdapters.adapt`. The node filter mirrors the Java
/// predicate parameter.
pub struct LGraphAdapter<'a> {
    pub arena: &'a mut LGraphArena,
    pub graph: LGraphId,
    pub transparent_north_south_edges: bool,
    pub transparent_comment_nodes: bool,
    pub node_filter: fn(&LGraphArena, LNodeId) -> bool,
}

impl<'a> LGraphAdapter<'a> {
    pub fn new(
        arena: &'a mut LGraphArena,
        graph: LGraphId,
        transparent_north_south_edges: bool,
        transparent_comment_nodes: bool,
        node_filter: fn(&LGraphArena, LNodeId) -> bool,
    ) -> Self {
        LGraphAdapter {
            arena,
            graph,
            transparent_north_south_edges,
            transparent_comment_nodes,
            node_filter,
        }
    }
}

impl<'a> AdapterGraph for LGraphAdapter<'a> {
    type N = LNodeId;
    type P = LPortId;
    type L = LLabelId;
    type E = LEdgeId;

    fn graph_properties(&self) -> &PropertyMap {
        &self.arena.graph(self.graph).properties
    }

    /// Java `LGraphAdapter.getNodes`: nodes from the LAYERS (not layerless),
    /// filtered, plus comment nodes when transparent.
    fn nodes(&self) -> Vec<LNodeId> {
        let a = &*self.arena;
        let mut result = Vec::new();
        for &layer in &a.graph(self.graph).layers {
            for &n in &a.layer(layer).nodes {
                if (self.node_filter)(a, n) {
                    result.push(n);
                    if self.transparent_comment_nodes {
                        if let Some(comments) = a.node(n).properties.try_get(&iprops::TOP_COMMENTS)
                        {
                            result.extend(comments);
                        }
                        if let Some(comments) =
                            a.node(n).properties.try_get(&iprops::BOTTOM_COMMENTS)
                        {
                            result.extend(comments);
                        }
                    }
                }
            }
        }
        result
    }

    fn node_size(&self, n: LNodeId) -> KVector {
        self.arena.node(n).size
    }
    fn set_node_size(&mut self, n: LNodeId, size: KVector) {
        self.arena.node_mut(n).size = size;
    }
    fn node_position(&self, n: LNodeId) -> KVector {
        self.arena.node(n).pos
    }
    fn set_node_position(&mut self, n: LNodeId, pos: KVector) {
        self.arena.node_mut(n).pos = pos;
    }
    fn node_properties(&self, n: LNodeId) -> &PropertyMap {
        &self.arena.node(n).properties
    }
    fn node_labels(&self, n: LNodeId) -> Vec<LLabelId> {
        self.arena.node(n).labels.clone()
    }
    fn node_ports(&self, n: LNodeId) -> Vec<LPortId> {
        self.arena.node(n).ports.clone()
    }
    /// Java `LNodeAdapter.getIncomingEdges` returns an empty list.
    fn node_incoming_edges(&self, _n: LNodeId) -> Vec<LEdgeId> {
        Vec::new()
    }
    fn node_outgoing_edges(&self, _n: LNodeId) -> Vec<LEdgeId> {
        Vec::new()
    }
    /// Java `LNodeAdapter.sortPortList`: only sorts when port order is fixed,
    /// using the PortListSorter comparator (side, then index/position).
    fn sort_port_list(&mut self, n: LNodeId) {
        let order_fixed = self
            .arena
            .node(n)
            .properties
            .get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
            .is_order_fixed();
        if order_fixed {
            let a = &*self.arena;
            let mut ports = a.node(n).ports.clone();
            ports.sort_by(|&p1, &p2| {
                crate::processors::port_list_sorter::cmp_combined_pub(a, p1, p2)
            });
            self.arena.node_mut(n).ports = ports;
        }
    }
    fn is_compound_node(&self, n: LNodeId) -> bool {
        self.arena.node(n).properties.get(&iprops::COMPOUND_NODE)
    }
    fn node_padding(&self, n: LNodeId) -> Spacing {
        self.arena.node(n).padding
    }
    fn set_node_padding(&mut self, n: LNodeId, padding: Spacing) {
        self.arena.node_mut(n).padding = padding;
    }
    fn node_margin(&self, n: LNodeId) -> Spacing {
        self.arena.node(n).margin
    }
    fn set_node_margin(&mut self, n: LNodeId, margin: Spacing) {
        self.arena.node_mut(n).margin = margin;
    }

    fn port_side(&self, p: LPortId) -> PortSide {
        self.arena.port(p).side
    }
    fn port_size(&self, p: LPortId) -> KVector {
        self.arena.port(p).size
    }
    fn set_port_size(&mut self, p: LPortId, size: KVector) {
        self.arena.port_mut(p).size = size;
    }
    fn port_position(&self, p: LPortId) -> KVector {
        self.arena.port(p).pos
    }
    fn set_port_position(&mut self, p: LPortId, pos: KVector) {
        self.arena.port_mut(p).pos = pos;
    }
    fn port_properties(&self, p: LPortId) -> &PropertyMap {
        &self.arena.port(p).properties
    }
    fn port_labels(&self, p: LPortId) -> Vec<LLabelId> {
        self.arena.port(p).labels.clone()
    }
    fn port_margin(&self, p: LPortId) -> Spacing {
        self.arena.port(p).margin
    }
    fn set_port_margin(&mut self, p: LPortId, margin: Spacing) {
        self.arena.port_mut(p).margin = margin;
    }
    /// Java `LPortAdapter.getIncomingEdges` incl. transparent north/south
    /// handling (self-loop holder handling arrives with self-loop support).
    fn port_incoming_edges(&self, p: LPortId) -> Vec<LEdgeId> {
        let a = &*self.arena;
        let node = a.port(p).node.unwrap();
        if self.transparent_north_south_edges
            && a.node(node).node_type == NodeType::NORTH_SOUTH_PORT
        {
            return Vec::new();
        }
        let mut edges = a.port(p).incoming_edges.clone();
        if self.transparent_north_south_edges {
            if let Some(port_dummy) = a.port(p).properties.try_get(&iprops::PORT_DUMMY) {
                edges.extend(a.node_incoming_edges(port_dummy));
            }
        }
        edges
    }
    fn port_outgoing_edges(&self, p: LPortId) -> Vec<LEdgeId> {
        let a = &*self.arena;
        let node = a.port(p).node.unwrap();
        if self.transparent_north_south_edges
            && a.node(node).node_type == NodeType::NORTH_SOUTH_PORT
        {
            return Vec::new();
        }
        let mut edges = a.port(p).outgoing_edges.clone();
        if self.transparent_north_south_edges {
            if let Some(port_dummy) = a.port(p).properties.try_get(&iprops::PORT_DUMMY) {
                edges.extend(a.node_outgoing_edges(port_dummy));
            }
        }
        edges
    }
    fn port_has_compound_connections(&self, p: LPortId) -> bool {
        self.arena.port(p).properties.get(&iprops::INSIDE_CONNECTIONS)
    }

    fn label_size(&self, l: LLabelId) -> KVector {
        self.arena.label(l).size
    }
    fn set_label_size(&mut self, l: LLabelId, size: KVector) {
        self.arena.label_mut(l).size = size;
    }
    fn label_position(&self, l: LLabelId) -> KVector {
        self.arena.label(l).pos
    }
    fn set_label_position(&mut self, l: LLabelId, pos: KVector) {
        self.arena.label_mut(l).pos = pos;
    }
    fn label_properties(&self, l: LLabelId) -> &PropertyMap {
        &self.arena.label(l).properties
    }
    fn label_side(&self, l: LLabelId) -> LabelSide {
        self.arena.label(l).properties.get(&LABEL_SIDE)
    }
    fn label_text(&self, l: LLabelId) -> String {
        self.arena.label(l).text.clone()
    }

    fn edge_labels(&self, e: LEdgeId) -> Vec<LLabelId> {
        self.arena.edge(e).labels.clone()
    }
}
