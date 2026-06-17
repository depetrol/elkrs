//! Exposes the LGraph through the
//! `crate::core::adapters::AdapterGraph` trait for the node-sizing code.

use crate::core::adapters::{AdapterGraph, LabelSide};
use crate::core::options::{PortConstraints, PortSide};
use crate::graph::math::{KVector, Spacing};
use crate::graph::properties::{Property, PropertyMap};

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LLabelId, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;

pub static LABEL_SIDE: Property<LabelSide> =
    Property::with_default("org.eclipse.elk.labelSide", || LabelSide::UNKNOWN);

/// The node filter mirrors the predicate parameter.
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

    /// `LGraphAdapter.getNodes`: nodes from the LAYERS (not layerless),
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
    /// `LNodeAdapter.getIncomingEdges` returns an empty list.
    fn node_incoming_edges(&self, _n: LNodeId) -> Vec<LEdgeId> {
        Vec::new()
    }
    fn node_outgoing_edges(&self, _n: LNodeId) -> Vec<LEdgeId> {
        Vec::new()
    }
    /// `LNodeAdapter.sortPortList`: only sorts when port order is fixed,
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
                crate::alg_layered::processors::port_list_sorter::cmp_combined_pub(a, p1, p2)
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
    /// `LPortAdapter.getIncomingEdges` incl. transparent north/south
    /// handling and the self loop holder's hidden incoming edges.
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
        // Add the incoming edges from the self loop holder if asked for one
        // (SELF_LOOP_HOLDER property)
        if let Some(slh) = &a.node(node).self_loop_holder {
            if let Some(slp) = slh.sl_port_idx(p) {
                for &sle in &slh.sl_ports[slp].incoming_sl_edges {
                    edges.push(slh.sl_edges[sle].l_edge);
                }
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
        // Add the outgoing edges from the self loop holder if asked for one
        // (SELF_LOOP_HOLDER property)
        if let Some(slh) = &a.node(node).self_loop_holder {
            if let Some(slp) = slh.sl_port_idx(p) {
                for &sle in &slh.sl_ports[slp].outgoing_sl_edges {
                    edges.push(slh.sl_edges[sle].l_edge);
                }
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
