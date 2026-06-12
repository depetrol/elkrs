//! Arena-based ELK graph model mirroring `org.eclipse.elk.graph`.
//!
//! Java uses an EMF object graph with bidirectional containment references.
//! Here all elements live in arenas inside [`ElkGraph`] and reference each
//! other through typed indices, which keeps ownership simple and iteration
//! deterministic.

use crate::math::KVectorChain;
use crate::properties::{PropertyHolder, PropertyMap};

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

id_type!(NodeId);
id_type!(PortId);
id_type!(EdgeId);
id_type!(LabelId);
id_type!(SectionId);

/// A node or a port — anything an edge can connect to
/// (Java `ElkConnectableShape`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ShapeId {
    Node(NodeId),
    Port(PortId),
}

/// Any graph element that can own labels (Java `ElkGraphElement`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ElementId {
    Node(NodeId),
    Port(PortId),
    Edge(EdgeId),
    Label(LabelId),
}

/// Shape geometry shared by nodes, ports and labels (Java `ElkShape`).
#[derive(Clone, Default, Debug)]
pub struct Shape {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Shape {
    pub fn set_location(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
    }
    pub fn set_dimensions(&mut self, width: f64, height: f64) {
        self.width = width;
        self.height = height;
    }
}

#[derive(Default, Debug)]
pub struct ElkNode {
    pub identifier: Option<String>,
    pub shape: Shape,
    pub properties: PropertyMap,
    pub labels: Vec<LabelId>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub ports: Vec<PortId>,
    /// Edges contained in this node (Java `containedEdges`); for edges, the
    /// containing node is the lowest common ancestor rule's assigned parent.
    pub contained_edges: Vec<EdgeId>,
    /// Edges that have this node among their sources/targets.
    pub outgoing_edges: Vec<EdgeId>,
    pub incoming_edges: Vec<EdgeId>,
}

#[derive(Default, Debug)]
pub struct ElkPort {
    pub identifier: Option<String>,
    pub shape: Shape,
    pub properties: PropertyMap,
    pub labels: Vec<LabelId>,
    pub parent: Option<NodeId>,
    pub outgoing_edges: Vec<EdgeId>,
    pub incoming_edges: Vec<EdgeId>,
}

#[derive(Default, Debug)]
pub struct ElkLabel {
    pub identifier: Option<String>,
    pub shape: Shape,
    pub properties: PropertyMap,
    pub labels: Vec<LabelId>,
    pub text: String,
    pub parent: Option<ElementId>,
}

#[derive(Default, Debug)]
pub struct ElkEdge {
    pub identifier: Option<String>,
    pub properties: PropertyMap,
    pub labels: Vec<LabelId>,
    pub containing_node: Option<NodeId>,
    pub sources: Vec<ShapeId>,
    pub targets: Vec<ShapeId>,
    pub sections: Vec<SectionId>,
}

#[derive(Default, Debug)]
pub struct ElkEdgeSection {
    pub identifier: Option<String>,
    pub properties: PropertyMap,
    pub parent: Option<EdgeId>,
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub bend_points: Vec<(f64, f64)>,
    pub outgoing_shape: Option<ShapeId>,
    pub incoming_shape: Option<ShapeId>,
    pub outgoing_sections: Vec<SectionId>,
    pub incoming_sections: Vec<SectionId>,
}

impl ElkEdgeSection {
    pub fn set_start_location(&mut self, x: f64, y: f64) {
        self.start_x = x;
        self.start_y = y;
    }
    pub fn set_end_location(&mut self, x: f64, y: f64) {
        self.end_x = x;
        self.end_y = y;
    }
}

/// The arena owning all graph elements. `root` is the graph's root node.
#[derive(Debug)]
pub struct ElkGraph {
    pub nodes: Vec<ElkNode>,
    pub ports: Vec<ElkPort>,
    pub edges: Vec<ElkEdge>,
    pub labels: Vec<ElkLabel>,
    pub sections: Vec<ElkEdgeSection>,
    pub root: NodeId,
}

impl Default for ElkGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ElkGraph {
    /// Creates a graph containing only a root node.
    pub fn new() -> Self {
        ElkGraph {
            nodes: vec![ElkNode::default()],
            ports: Vec::new(),
            edges: Vec::new(),
            labels: Vec::new(),
            sections: Vec::new(),
            root: NodeId(0),
        }
    }

    // ------------------------------------------------------------ accessors

    pub fn node(&self, id: NodeId) -> &ElkNode {
        &self.nodes[id.index()]
    }
    pub fn node_mut(&mut self, id: NodeId) -> &mut ElkNode {
        &mut self.nodes[id.index()]
    }
    pub fn port(&self, id: PortId) -> &ElkPort {
        &self.ports[id.index()]
    }
    pub fn port_mut(&mut self, id: PortId) -> &mut ElkPort {
        &mut self.ports[id.index()]
    }
    pub fn edge(&self, id: EdgeId) -> &ElkEdge {
        &self.edges[id.index()]
    }
    pub fn edge_mut(&mut self, id: EdgeId) -> &mut ElkEdge {
        &mut self.edges[id.index()]
    }
    pub fn label(&self, id: LabelId) -> &ElkLabel {
        &self.labels[id.index()]
    }
    pub fn label_mut(&mut self, id: LabelId) -> &mut ElkLabel {
        &mut self.labels[id.index()]
    }
    pub fn section(&self, id: SectionId) -> &ElkEdgeSection {
        &self.sections[id.index()]
    }
    pub fn section_mut(&mut self, id: SectionId) -> &mut ElkEdgeSection {
        &mut self.sections[id.index()]
    }

    // ------------------------------------------------------------- creation

    pub fn create_node(&mut self, parent: Option<NodeId>) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(ElkNode { parent, ..Default::default() });
        if let Some(p) = parent {
            self.node_mut(p).children.push(id);
        }
        id
    }

    pub fn create_port(&mut self, node: NodeId) -> PortId {
        let id = PortId(self.ports.len() as u32);
        self.ports.push(ElkPort { parent: Some(node), ..Default::default() });
        self.node_mut(node).ports.push(id);
        id
    }

    pub fn create_label(&mut self, text: &str, owner: ElementId) -> LabelId {
        let id = LabelId(self.labels.len() as u32);
        self.labels.push(ElkLabel {
            text: text.to_string(),
            parent: Some(owner),
            ..Default::default()
        });
        match owner {
            ElementId::Node(n) => self.node_mut(n).labels.push(id),
            ElementId::Port(p) => self.port_mut(p).labels.push(id),
            ElementId::Edge(e) => self.edge_mut(e).labels.push(id),
            ElementId::Label(l) => self.label_mut(l).labels.push(id),
        }
        id
    }

    /// Creates an edge contained in `containing_node` (Java
    /// `ElkGraphUtil.createEdge`). Use [`ElkGraph::connect`] to hook up
    /// endpoints, or [`ElkGraph::create_simple_edge`] for both at once.
    pub fn create_edge(&mut self, containing_node: Option<NodeId>) -> EdgeId {
        let id = EdgeId(self.edges.len() as u32);
        self.edges.push(ElkEdge { containing_node, ..Default::default() });
        if let Some(c) = containing_node {
            self.node_mut(c).contained_edges.push(id);
        }
        id
    }

    pub fn add_edge_source(&mut self, edge: EdgeId, source: ShapeId) {
        self.edge_mut(edge).sources.push(source);
        match source {
            ShapeId::Node(n) => self.node_mut(n).outgoing_edges.push(edge),
            ShapeId::Port(p) => self.port_mut(p).outgoing_edges.push(edge),
        }
    }

    pub fn add_edge_target(&mut self, edge: EdgeId, target: ShapeId) {
        self.edge_mut(edge).targets.push(target);
        match target {
            ShapeId::Node(n) => self.node_mut(n).incoming_edges.push(edge),
            ShapeId::Port(p) => self.port_mut(p).incoming_edges.push(edge),
        }
    }

    /// Java `ElkGraphUtil.createSimpleEdge`: connects source and target and
    /// sets the containing node to their lowest common ancestor ("best
    /// containment").
    pub fn create_simple_edge(&mut self, source: ShapeId, target: ShapeId) -> EdgeId {
        let edge = self.create_edge(None);
        self.add_edge_source(edge, source);
        self.add_edge_target(edge, target);
        self.update_containment(edge);
        edge
    }

    pub fn create_section(&mut self, edge: EdgeId) -> SectionId {
        let id = SectionId(self.sections.len() as u32);
        self.sections.push(ElkEdgeSection { parent: Some(edge), ..Default::default() });
        self.edge_mut(edge).sections.push(id);
        id
    }

    // ----------------------------------------------------------- navigation

    /// The node a connectable shape belongs to (Java
    /// `ElkGraphUtil.connectableShapeToNode`).
    pub fn shape_node(&self, shape: ShapeId) -> NodeId {
        match shape {
            ShapeId::Node(n) => n,
            ShapeId::Port(p) => self.port(p).parent.expect("port without parent node"),
        }
    }

    /// The port if the shape is a port (Java `connectableShapeToPort`).
    pub fn shape_port(&self, shape: ShapeId) -> Option<PortId> {
        match shape {
            ShapeId::Port(p) => Some(p),
            ShapeId::Node(_) => None,
        }
    }

    pub fn is_descendant(&self, child: NodeId, ancestor: NodeId) -> bool {
        let mut current = child;
        while let Some(parent) = self.node(current).parent {
            if parent == ancestor {
                return true;
            }
            current = parent;
        }
        false
    }

    /// Containment depth; the root node has depth 0.
    pub fn depth(&self, node: NodeId) -> usize {
        let mut depth = 0;
        let mut current = node;
        while let Some(parent) = self.node(current).parent {
            depth += 1;
            current = parent;
        }
        depth
    }

    /// Recomputes the edge's containing node as the lowest common ancestor of
    /// all endpoints (Java `ElkGraphUtil.updateContainment` semantics).
    pub fn update_containment(&mut self, edge: EdgeId) {
        let e = self.edge(edge);
        let mut endpoints = e.sources.iter().chain(e.targets.iter());
        let best = if let Some(&first) = endpoints.next() {
            let mut anc: Vec<NodeId> = self.ancestors_inclusive(self.shape_node(first));
            // For a source node itself being a hierarchical parent, Java uses
            // the node itself as candidate; ancestors of the *node* work
            // because findBestContainment uses findLowestCommonAncestor of
            // the endpoints' parents — replicated here via ancestor chains.
            for &other in endpoints {
                let other_anc = self.ancestors_inclusive(self.shape_node(other));
                anc.retain(|a| other_anc.contains(a));
            }
            anc.first().copied()
        } else {
            None
        };

        let old = self.edge(edge).containing_node;
        if old != best {
            if let Some(o) = old {
                self.node_mut(o).contained_edges.retain(|&x| x != edge);
            }
            self.edge_mut(edge).containing_node = best;
            if let Some(b) = best {
                self.node_mut(b).contained_edges.push(edge);
            }
        }
    }

    /// Chain of ancestors starting at the node's parent... actually starting
    /// at the node's parent and walking to the root, deepest first.
    fn ancestors_inclusive(&self, node: NodeId) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut current = self.node(node).parent;
        while let Some(n) = current {
            result.push(n);
            current = self.node(n).parent;
        }
        result
    }

    /// All nodes in the subtree below `node` in pre-order, excluding `node`.
    pub fn descendants(&self, node: NodeId) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut stack: Vec<NodeId> = self.node(node).children.iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            result.push(n);
            stack.extend(self.node(n).children.iter().rev());
        }
        result
    }

    /// First section of the edge, creating one if absent
    /// (Java `ElkGraphUtil.firstEdgeSection`).
    pub fn first_edge_section(&mut self, edge: EdgeId, reset: bool) -> SectionId {
        if let Some(&s) = self.edge(edge).sections.first() {
            if reset {
                let sec = self.section_mut(s);
                let (id, parent) = (sec.identifier.take(), sec.parent);
                *sec = ElkEdgeSection { identifier: id, parent, ..Default::default() };
            }
            s
        } else {
            self.create_section(edge)
        }
    }

    /// Bend points of a section as a vector chain.
    pub fn section_chain(&self, section: SectionId) -> KVectorChain {
        let s = self.section(section);
        let mut chain = KVectorChain::new();
        chain.add(s.start_x, s.start_y);
        for &(x, y) in &s.bend_points {
            chain.add(x, y);
        }
        chain.add(s.end_x, s.end_y);
        chain
    }

    /// True if the edge connects two elements with the same parent node
    /// or a node to one of its descendants — i.e. no hierarchy crossing
    /// (mirrors Java `ElkGraphUtil.isInsideSelfLoop` & co. usage patterns;
    /// hierarchy checks are refined where algorithms need them).
    pub fn is_hierarchical(&self, edge: EdgeId) -> bool {
        let e = self.edge(edge);
        let mut parents = e
            .sources
            .iter()
            .chain(e.targets.iter())
            .map(|&s| self.node(self.shape_node(s)).parent);
        let first = match parents.next() {
            Some(p) => p,
            None => return false,
        };
        parents.any(|p| p != first)
    }
}

impl PropertyHolder for ElkNode {
    fn properties(&self) -> &PropertyMap {
        &self.properties
    }
    fn properties_mut(&mut self) -> &mut PropertyMap {
        &mut self.properties
    }
}
impl PropertyHolder for ElkPort {
    fn properties(&self) -> &PropertyMap {
        &self.properties
    }
    fn properties_mut(&mut self) -> &mut PropertyMap {
        &mut self.properties
    }
}
impl PropertyHolder for ElkEdge {
    fn properties(&self) -> &PropertyMap {
        &self.properties
    }
    fn properties_mut(&mut self) -> &mut PropertyMap {
        &mut self.properties
    }
}
impl PropertyHolder for ElkLabel {
    fn properties(&self) -> &PropertyMap {
        &self.properties
    }
    fn properties_mut(&mut self) -> &mut PropertyMap {
        &mut self.properties
    }
}
impl PropertyHolder for ElkEdgeSection {
    fn properties(&self) -> &PropertyMap {
        &self.properties
    }
    fn properties_mut(&mut self) -> &mut PropertyMap {
        &mut self.properties
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_graph() {
        let mut g = ElkGraph::new();
        let n1 = g.create_node(Some(g.root));
        let n2 = g.create_node(Some(g.root));
        let e = g.create_simple_edge(ShapeId::Node(n1), ShapeId::Node(n2));
        assert_eq!(g.edge(e).containing_node, Some(g.root));
        assert_eq!(g.node(g.root).children.len(), 2);
        assert_eq!(g.node(n1).outgoing_edges, vec![e]);
        assert_eq!(g.node(n2).incoming_edges, vec![e]);
    }

    #[test]
    fn containment_lowest_common_ancestor() {
        let mut g = ElkGraph::new();
        let a = g.create_node(Some(g.root));
        let a1 = g.create_node(Some(a));
        let a2 = g.create_node(Some(a));
        let e = g.create_simple_edge(ShapeId::Node(a1), ShapeId::Node(a2));
        assert_eq!(g.edge(e).containing_node, Some(a));
        let b = g.create_node(Some(g.root));
        let e2 = g.create_simple_edge(ShapeId::Node(a1), ShapeId::Node(b));
        assert_eq!(g.edge(e2).containing_node, Some(g.root));
    }

    #[test]
    fn ports_and_sections() {
        let mut g = ElkGraph::new();
        let n1 = g.create_node(Some(g.root));
        let n2 = g.create_node(Some(g.root));
        let p1 = g.create_port(n1);
        let e = g.create_simple_edge(ShapeId::Port(p1), ShapeId::Node(n2));
        assert_eq!(g.shape_node(ShapeId::Port(p1)), n1);
        assert_eq!(g.edge(e).containing_node, Some(g.root));
        let s = g.first_edge_section(e, false);
        g.section_mut(s).set_start_location(1.0, 2.0);
        g.section_mut(s).bend_points.push((3.0, 4.0));
        g.section_mut(s).set_end_location(5.0, 6.0);
        let chain = g.section_chain(s);
        assert_eq!(chain.len(), 3);
        assert_eq!(g.first_edge_section(e, false), s);
    }

    #[test]
    fn descendants_preorder() {
        let mut g = ElkGraph::new();
        let a = g.create_node(Some(g.root));
        let b = g.create_node(Some(g.root));
        let a1 = g.create_node(Some(a));
        let a2 = g.create_node(Some(a));
        assert_eq!(g.descendants(g.root), vec![a, a1, a2, b]);
        assert!(g.is_descendant(a1, g.root));
        assert!(!g.is_descendant(b, a));
        assert_eq!(g.depth(a1), 2);
    }
}
