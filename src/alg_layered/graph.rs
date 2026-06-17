//! The layered algorithm's
//! internal graph model (LGraph, LNode, LPort, LEdge, LLabel, Layer).
//!
//! All elements live in arenas inside [`LGraphArena`] and reference each other
//! through typed indices. One arena holds every element of a layout run,
//! including nested graphs (compound layout).

use crate::core::options::PortSide;
use crate::graph::math::{KVector, KVectorChain, Spacing};
use crate::graph::properties::PropertyMap;

macro_rules! id_type {
    ($Name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
        pub struct $Name(pub u32);

        impl $Name {
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

id_type!(LGraphId);
id_type!(LNodeId);
id_type!(LPortId);
id_type!(LEdgeId);
id_type!(LLabelId);
id_type!(LayerId);

crate::elk_enum! {
    pub enum NodeType {
        NORMAL,
        LONG_EDGE,
        EXTERNAL_PORT,
        NORTH_SOUTH_PORT,
        LABEL,
        BREAKING_POINT,
        PLACEHOLDER,
        NONSHIFTING_PLACEHOLDER,
    }
}

pub use crate::core::options::PortSide as Side;

#[derive(Default, Debug)]
pub struct LGraph {
    /// scratch id (`LGraphElement.id`)
    pub id: i32,
    pub size: KVector,
    pub padding: Spacing,
    pub offset: KVector,
    pub layerless_nodes: Vec<LNodeId>,
    pub layers: Vec<LayerId>,
    pub parent_node: Option<LNodeId>,
    pub properties: PropertyMap,
}

#[derive(Default, Debug)]
pub struct Layer {
    pub id: i32,
    pub graph: Option<LGraphId>,
    pub nodes: Vec<LNodeId>,
    pub size: KVector,
    pub properties: PropertyMap,
}

#[derive(Debug)]
pub struct LNode {
    pub id: i32,
    pub graph: Option<LGraphId>,
    pub layer: Option<LayerId>,
    pub node_type: NodeType,
    pub ports: Vec<LPortId>,
    pub labels: Vec<LLabelId>,
    pub nested_graph: Option<LGraphId>,
    pub margin: Spacing,
    pub padding: Spacing,
    pub pos: KVector,
    pub size: KVector,
    pub properties: PropertyMap,
    /// The `InternalProperties.SELF_LOOP_HOLDER` property: a mutable
    /// `SelfLoopHolder` object on the node, stored here as a
    /// dedicated field so mutations stay by-reference.
    pub self_loop_holder: Option<Box<crate::alg_layered::loops::SelfLoopHolder>>,
    /// cached port side index ranges (after PortListSorter)
    port_side_indices: Option<[(usize, usize); 5]>,
    port_sides_cached: bool,
}

impl Default for LNode {
    fn default() -> Self {
        LNode {
            id: 0,
            graph: None,
            layer: None,
            node_type: NodeType::NORMAL,
            ports: Vec::new(),
            labels: Vec::new(),
            nested_graph: None,
            margin: Spacing::default(),
            padding: Spacing::default(),
            pos: KVector::default(),
            size: KVector::default(),
            properties: PropertyMap::new(),
            self_loop_holder: None,
            port_side_indices: None,
            port_sides_cached: false,
        }
    }
}

#[derive(Debug)]
pub struct LPort {
    pub id: i32,
    pub node: Option<LNodeId>,
    pub side: PortSide,
    pub anchor: KVector,
    pub explicit_anchor: bool,
    pub margin: Spacing,
    pub labels: Vec<LLabelId>,
    pub incoming_edges: Vec<LEdgeId>,
    pub outgoing_edges: Vec<LEdgeId>,
    pub connected_to_external_nodes: bool,
    pub pos: KVector,
    pub size: KVector,
    pub properties: PropertyMap,
}

impl Default for LPort {
    fn default() -> Self {
        LPort {
            id: 0,
            node: None,
            side: PortSide::UNDEFINED,
            anchor: KVector::default(),
            explicit_anchor: false,
            margin: Spacing::default(),
            labels: Vec::new(),
            incoming_edges: Vec::new(),
            outgoing_edges: Vec::new(),
            connected_to_external_nodes: true,
            pos: KVector::default(),
            size: KVector::default(),
            properties: PropertyMap::new(),
        }
    }
}

#[derive(Default, Debug)]
pub struct LEdge {
    pub id: i32,
    pub bend_points: KVectorChain,
    pub source: Option<LPortId>,
    pub target: Option<LPortId>,
    pub labels: Vec<LLabelId>,
    pub properties: PropertyMap,
}

#[derive(Default, Debug)]
pub struct LLabel {
    pub id: i32,
    pub text: String,
    pub pos: KVector,
    pub size: KVector,
    pub properties: PropertyMap,
}

/// Arena owning all layered-graph elements of one layout run.
#[derive(Default, Debug)]
pub struct LGraphArena {
    pub graphs: Vec<LGraph>,
    pub nodes: Vec<LNode>,
    pub ports: Vec<LPort>,
    pub edges: Vec<LEdge>,
    pub labels: Vec<LLabel>,
    pub layers: Vec<Layer>,
}

impl LGraphArena {
    pub fn new() -> Self {
        Self::default()
    }

    // ------------------------------------------------------------ accessors

    pub fn graph(&self, id: LGraphId) -> &LGraph {
        &self.graphs[id.index()]
    }
    pub fn graph_mut(&mut self, id: LGraphId) -> &mut LGraph {
        &mut self.graphs[id.index()]
    }
    pub fn node(&self, id: LNodeId) -> &LNode {
        &self.nodes[id.index()]
    }
    pub fn node_mut(&mut self, id: LNodeId) -> &mut LNode {
        &mut self.nodes[id.index()]
    }
    pub fn port(&self, id: LPortId) -> &LPort {
        &self.ports[id.index()]
    }
    pub fn port_mut(&mut self, id: LPortId) -> &mut LPort {
        &mut self.ports[id.index()]
    }
    pub fn edge(&self, id: LEdgeId) -> &LEdge {
        &self.edges[id.index()]
    }
    pub fn edge_mut(&mut self, id: LEdgeId) -> &mut LEdge {
        &mut self.edges[id.index()]
    }
    pub fn label(&self, id: LLabelId) -> &LLabel {
        &self.labels[id.index()]
    }
    pub fn label_mut(&mut self, id: LLabelId) -> &mut LLabel {
        &mut self.labels[id.index()]
    }
    pub fn layer(&self, id: LayerId) -> &Layer {
        &self.layers[id.index()]
    }
    pub fn layer_mut(&mut self, id: LayerId) -> &mut Layer {
        &mut self.layers[id.index()]
    }

    // ------------------------------------------------------------- creation

    pub fn create_graph(&mut self) -> LGraphId {
        let id = LGraphId(self.graphs.len() as u32);
        self.graphs.push(LGraph::default());
        id
    }

    pub fn create_node(&mut self, graph: LGraphId) -> LNodeId {
        let id = LNodeId(self.nodes.len() as u32);
        self.nodes.push(LNode { graph: Some(graph), ..Default::default() });
        id
    }

    pub fn create_port(&mut self) -> LPortId {
        let id = LPortId(self.ports.len() as u32);
        self.ports.push(LPort::default());
        id
    }

    pub fn create_edge(&mut self) -> LEdgeId {
        let id = LEdgeId(self.edges.len() as u32);
        self.edges.push(LEdge::default());
        id
    }

    pub fn create_label(&mut self, text: &str) -> LLabelId {
        let id = LLabelId(self.labels.len() as u32);
        self.labels.push(LLabel { text: text.to_string(), ..Default::default() });
        id
    }

    /// `new Layer(graph)` — does NOT add the layer to the graph's list.
    pub fn create_layer(&mut self, graph: LGraphId) -> LayerId {
        let id = LayerId(self.layers.len() as u32);
        self.layers.push(Layer { graph: Some(graph), ..Default::default() });
        id
    }

    // ------------------------------------------------- structural mutators

    pub fn port_set_node(&mut self, port: LPortId, node: Option<LNodeId>) {
        if let Some(old) = self.port(port).node {
            self.node_mut(old).ports.retain(|&p| p != port);
        }
        self.port_mut(port).node = node;
        if let Some(new) = node {
            self.node_mut(new).ports.push(port);
        }
    }

    pub fn port_set_side(&mut self, port: LPortId, side: PortSide) {
        let p = self.port_mut(port);
        p.side = side;
        if !p.explicit_anchor {
            match side {
                PortSide::NORTH => {
                    p.anchor.x = p.size.x / 2.0;
                    p.anchor.y = 0.0;
                }
                PortSide::EAST => {
                    p.anchor.x = p.size.x;
                    p.anchor.y = p.size.y / 2.0;
                }
                PortSide::SOUTH => {
                    p.anchor.x = p.size.x / 2.0;
                    p.anchor.y = p.size.y;
                }
                PortSide::WEST => {
                    p.anchor.x = 0.0;
                    p.anchor.y = p.size.y / 2.0;
                }
                PortSide::UNDEFINED => {}
            }
        }
    }

    pub fn edge_set_source(&mut self, edge: LEdgeId, source: Option<LPortId>) {
        if let Some(old) = self.edge(edge).source {
            self.port_mut(old).outgoing_edges.retain(|&e| e != edge);
        }
        self.edge_mut(edge).source = source;
        if let Some(new) = source {
            self.port_mut(new).outgoing_edges.push(edge);
        }
    }

    pub fn edge_set_target(&mut self, edge: LEdgeId, target: Option<LPortId>) {
        if let Some(old) = self.edge(edge).target {
            self.port_mut(old).incoming_edges.retain(|&e| e != edge);
        }
        self.edge_mut(edge).target = target;
        if let Some(new) = target {
            self.port_mut(new).incoming_edges.push(edge);
        }
    }

    pub fn edge_set_target_at_index(&mut self, edge: LEdgeId, target: Option<LPortId>, index: usize) {
        if let Some(old) = self.edge(edge).target {
            self.port_mut(old).incoming_edges.retain(|&e| e != edge);
        }
        self.edge_mut(edge).target = target;
        if let Some(new) = target {
            self.port_mut(new).incoming_edges.insert(index, edge);
        }
    }

    pub fn node_set_layer(&mut self, node: LNodeId, layer: Option<LayerId>) {
        if let Some(old) = self.node(node).layer {
            self.layer_mut(old).nodes.retain(|&n| n != node);
        }
        self.node_mut(node).layer = layer;
        if let Some(new) = layer {
            self.layer_mut(new).nodes.push(node);
        }
    }

    pub fn node_set_layer_at(&mut self, node: LNodeId, layer: Option<LayerId>, index: usize) {
        if let Some(old) = self.node(node).layer {
            self.layer_mut(old).nodes.retain(|&n| n != node);
        }
        self.node_mut(node).layer = layer;
        if let Some(new) = layer {
            self.layer_mut(new).nodes.insert(index, node);
        }
    }

    // ----------------------------------------------------------- navigation

    /// The graph containing the node (via the layer if needed).
    pub fn node_graph(&self, node: LNodeId) -> LGraphId {
        let n = self.node(node);
        if let Some(g) = n.graph {
            return g;
        }
        n.layer
            .and_then(|l| self.layer(l).graph)
            .expect("node neither in graph nor in layer")
    }

    /// All edges entering the node through any of its ports.
    pub fn node_incoming_edges(&self, node: LNodeId) -> Vec<LEdgeId> {
        let mut result = Vec::new();
        for &port in &self.node(node).ports {
            result.extend(self.port(port).incoming_edges.iter().copied());
        }
        result
    }

    /// All edges leaving the node through any of its ports.
    pub fn node_outgoing_edges(&self, node: LNodeId) -> Vec<LEdgeId> {
        let mut result = Vec::new();
        for &port in &self.node(node).ports {
            result.extend(self.port(port).outgoing_edges.iter().copied());
        }
        result
    }

    /// All connected edges (per port: incoming first, then outgoing).
    pub fn node_connected_edges(&self, node: LNodeId) -> Vec<LEdgeId> {
        let mut result = Vec::new();
        for &port in &self.node(node).ports {
            result.extend(self.port(port).incoming_edges.iter().copied());
            result.extend(self.port(port).outgoing_edges.iter().copied());
        }
        result
    }

    pub fn port_connected_edges(&self, port: LPortId) -> Vec<LEdgeId> {
        let p = self.port(port);
        let mut result = p.incoming_edges.clone();
        result.extend(p.outgoing_edges.iter().copied());
        result
    }

    pub fn port_degree(&self, port: LPortId) -> usize {
        let p = self.port(port);
        p.incoming_edges.len() + p.outgoing_edges.len()
    }

    pub fn port_net_flow(&self, port: LPortId) -> i32 {
        let p = self.port(port);
        p.incoming_edges.len() as i32 - p.outgoing_edges.len() as i32
    }

    /// Ports of the node with the given side, in port list order.
    pub fn node_ports_on_side(&self, node: LNodeId, side: PortSide) -> Vec<LPortId> {
        self.node(node)
            .ports
            .iter()
            .copied()
            .filter(|&p| self.port(p).side == side)
            .collect()
    }

    /// Input ports (with incoming edges), `getPorts(PortType.INPUT)`.
    pub fn node_input_ports(&self, node: LNodeId) -> Vec<LPortId> {
        self.node(node)
            .ports
            .iter()
            .copied()
            .filter(|&p| !self.port(p).incoming_edges.is_empty())
            .collect()
    }

    /// Output ports (with outgoing edges), `getPorts(PortType.OUTPUT)`.
    pub fn node_output_ports(&self, node: LNodeId) -> Vec<LPortId> {
        self.node(node)
            .ports
            .iter()
            .copied()
            .filter(|&p| !self.port(p).outgoing_edges.is_empty())
            .collect()
    }

    pub fn node_index_in_layer(&self, node: LNodeId) -> i32 {
        match self.node(node).layer {
            None => -1,
            Some(layer) => self
                .layer(layer)
                .nodes
                .iter()
                .position(|&n| n == node)
                .map(|i| i as i32)
                .unwrap_or(-1),
        }
    }

    pub fn node_port_side_view(&self, node: LNodeId, side: PortSide) -> Vec<LPortId> {
        let n = self.node(node);
        if n.port_sides_cached {
            if let Some(indices) = &n.port_side_indices {
                let (start, end) = indices[side as usize];
                return n.ports[start..end].to_vec();
            }
        }
        // not cached: compute the side ranges on the fly
        let indices = Self::find_port_indices(self, node);
        let (start, end) = indices[side as usize];
        n.ports[start..end].to_vec()
    }

    pub fn node_cache_port_sides(&mut self, node: LNodeId) {
        let indices = Self::find_port_indices(self, node);
        let n = self.node_mut(node);
        n.port_side_indices = Some(indices);
        n.port_sides_cached = true;
    }

    /// Invalidate the cache (the port list sorter re-caches).
    pub fn node_invalidate_port_side_cache(&mut self, node: LNodeId) {
        let n = self.node_mut(node);
        n.port_side_indices = None;
        n.port_sides_cached = false;
    }

    fn find_port_indices(&self, node: LNodeId) -> [(usize, usize); 5] {
        let n = self.node(node);
        let mut result = [(0usize, 0usize); 5];
        let mut first_index_for_current = 0;
        let mut current_side = PortSide::NORTH;
        let mut current_index = 0;
        for (i, &port) in n.ports.iter().enumerate() {
            current_index = i;
            let side = self.port(port).side;
            if side != current_side {
                if first_index_for_current != i {
                    result[current_side as usize] = (first_index_for_current, i);
                }
                current_side = side;
                first_index_for_current = i;
            }
        }
        if !n.ports.is_empty() {
            current_index += 1;
        }
        result[current_side as usize] = (first_index_for_current, current_index);
        result
    }

    pub fn edge_is_self_loop(&self, edge: LEdgeId) -> bool {
        let e = self.edge(edge);
        match (e.source, e.target) {
            (Some(s), Some(t)) => {
                let sn = self.port(s).node;
                sn.is_some() && sn == self.port(t).node
            }
            _ => false,
        }
    }

    pub fn edge_is_in_layer(&self, edge: LEdgeId) -> bool {
        if self.edge_is_self_loop(edge) {
            return false;
        }
        let e = self.edge(edge);
        let source_layer = self.port(e.source.unwrap()).node.and_then(|n| self.node(n).layer);
        let target_layer = self.port(e.target.unwrap()).node.and_then(|n| self.node(n).layer);
        source_layer == target_layer
    }

    /// Source node of an edge.
    pub fn edge_source_node(&self, edge: LEdgeId) -> LNodeId {
        self.port(self.edge(edge).source.unwrap()).node.unwrap()
    }

    /// Target node of an edge.
    pub fn edge_target_node(&self, edge: LEdgeId) -> LNodeId {
        self.port(self.edge(edge).target.unwrap()).node.unwrap()
    }

    pub fn graph_actual_size(&self, graph: LGraphId) -> KVector {
        let g = self.graph(graph);
        KVector::new(
            g.size.x + g.padding.left + g.padding.right,
            g.size.y + g.padding.top + g.padding.bottom,
        )
    }

    /// All nodes of the graph: layerless nodes plus nodes in layers.
    pub fn graph_all_nodes(&self, graph: LGraphId) -> Vec<LNodeId> {
        let g = self.graph(graph);
        let mut result = g.layerless_nodes.clone();
        for &layer in &g.layers {
            result.extend(self.layer(layer).nodes.iter().copied());
        }
        result
    }

    pub fn node_border_to_content_area_coordinates(
        &mut self,
        node: LNodeId,
        horizontal: bool,
        vertical: bool,
    ) {
        let graph = self.node_graph(node);
        let g = self.graph(graph);
        let (padding, offset) = (g.padding, g.offset);
        let n = self.node_mut(node);
        if horizontal {
            n.pos.x = n.pos.x - padding.left - offset.x;
        }
        if vertical {
            n.pos.y = n.pos.y - padding.top - offset.y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_navigate() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let n1 = a.create_node(g);
        let n2 = a.create_node(g);
        a.graph_mut(g).layerless_nodes.push(n1);
        a.graph_mut(g).layerless_nodes.push(n2);
        let p1 = a.create_port();
        a.port_set_node(p1, Some(n1));
        let p2 = a.create_port();
        a.port_set_node(p2, Some(n2));
        let e = a.create_edge();
        a.edge_set_source(e, Some(p1));
        a.edge_set_target(e, Some(p2));

        assert_eq!(a.node_outgoing_edges(n1), vec![e]);
        assert_eq!(a.node_incoming_edges(n2), vec![e]);
        assert_eq!(a.edge_source_node(e), n1);
        assert_eq!(a.edge_target_node(e), n2);
        assert!(!a.edge_is_self_loop(e));

        // re-target the edge
        a.edge_set_target(e, Some(p1));
        assert!(a.edge_is_self_loop(e));
        assert!(a.node_incoming_edges(n2).is_empty());
    }

    #[test]
    fn layers_and_indices() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let l1 = a.create_layer(g);
        a.graph_mut(g).layers.push(l1);
        let n1 = a.create_node(g);
        let n2 = a.create_node(g);
        a.node_set_layer(n1, Some(l1));
        a.node_set_layer_at(n2, Some(l1), 0);
        assert_eq!(a.layer(l1).nodes, vec![n2, n1]);
        assert_eq!(a.node_index_in_layer(n1), 1);
        assert_eq!(a.node_index_in_layer(n2), 0);
    }

    #[test]
    fn port_side_views() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let n = a.create_node(g);
        let pn = a.create_port();
        let pe1 = a.create_port();
        let pe2 = a.create_port();
        let pw = a.create_port();
        for (p, side) in [
            (pn, PortSide::NORTH),
            (pe1, PortSide::EAST),
            (pe2, PortSide::EAST),
            (pw, PortSide::WEST),
        ] {
            a.port_set_node(p, Some(n));
            a.port_set_side(p, side);
        }
        assert_eq!(a.node_port_side_view(n, PortSide::EAST), vec![pe1, pe2]);
        assert_eq!(a.node_port_side_view(n, PortSide::NORTH), vec![pn]);
        assert_eq!(a.node_port_side_view(n, PortSide::WEST), vec![pw]);
        assert!(a.node_port_side_view(n, PortSide::SOUTH).is_empty());
        a.node_cache_port_sides(n);
        assert_eq!(a.node_port_side_view(n, PortSide::EAST), vec![pe1, pe2]);
    }

    #[test]
    fn port_anchor_follows_side() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let n = a.create_node(g);
        let p = a.create_port();
        a.port_set_node(p, Some(n));
        a.port_mut(p).size = KVector::new(10.0, 6.0);
        a.port_set_side(p, PortSide::EAST);
        assert_eq!(a.port(p).anchor, KVector::new(10.0, 3.0));
        a.port_set_side(p, PortSide::SOUTH);
        assert_eq!(a.port(p).anchor, KVector::new(5.0, 6.0));
    }
}
