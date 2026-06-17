//! The tree algorithm's internal
//! graph model (TGraph, TNode, TEdge).
//!
//! All elements live in arenas inside
//! [`TArena`] and reference each other through typed indices. [`TGraph`]
//! instances (the full graph and its connected components) hold id lists
//! into the shared arena; components are formed by moving node ids between
//! `TGraph` objects.
//!
//! The `InternalProperties` entries are plain fields here. None of those
//! property ids ("ROOT", "FAN", "PRELIM", ...) is a registered layout option,
//! so although `ElkGraphImporter.applyLayout` copies them onto the output
//! nodes, the JSON exporter filters them out — making fields
//! behaviorally equivalent. The one exception is
//! `MrTreeOptions.TREE_LEVEL` (`org.eclipse.elk.mrtree.treeLevel`), which is
//! a registered option visible in the output; it is kept in the node's
//! [`PropertyMap`].

use crate::graph::graph::{EdgeId, NodeId};
use crate::graph::math::{KVector, KVectorChain};
use crate::graph::properties::PropertyMap;

use crate::alg_mrtree::options;

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

id_type!(TNodeId);
id_type!(TEdgeId);

#[derive(Default, Debug)]
pub struct TNode {
    /// Public `id` field (reassigned by several processors; the
    /// SUPER_ROOT dummy shares id 0 with a real node, faithfully kept).
    pub id: i32,
    pub label: String,
    pub pos: KVector,
    pub size: KVector,
    pub properties: PropertyMap,
    /// `InternalProperties.ORIGIN`.
    pub origin: Option<NodeId>,
    pub outgoing: Vec<TEdgeId>,
    pub incoming: Vec<TEdgeId>,

    // ----- InternalProperties as fields -----
    /// `ROOT`.
    pub root: bool,
    /// `DUMMY`.
    pub dummy: bool,
    /// `FAN`.
    pub fan: i32,
    /// `DESCENDANTS`.
    pub descendants: i32,
    /// `ID` (block id string built by `FanProcessor`).
    pub id_string: String,
    /// `POSITION`.
    pub position: i32,
    /// `PRELIM`.
    pub prelim: f64,
    /// `MODIFIER`.
    pub modifier: f64,
    /// `LEFTNEIGHBOR` / `RIGHTNEIGHBOR` / `LEFTSIBLING` / `RIGHTSIBLING`.
    pub left_neighbor: Option<TNodeId>,
    pub right_neighbor: Option<TNodeId>,
    pub left_sibling: Option<TNodeId>,
    pub right_sibling: Option<TNodeId>,
    /// `XCOOR` / `YCOOR`.
    pub xcoor: i32,
    pub ycoor: i32,
    /// `LEVELHEIGHT`.
    pub level_height: f64,
    /// `LEVELMIN` / `LEVELMAX`.
    pub level_min: f64,
    pub level_max: f64,
    /// `COMPACT_LEVEL_ASCENSION`.
    pub compact_level_ascension: bool,
    /// `COMPACT_CONSTRAINTS`.
    pub compact_constraints: Vec<TNodeId>,
}

#[derive(Debug)]
pub struct TEdge {
    pub source: TNodeId,
    pub target: TNodeId,
    /// During edge routing this chain accumulates *all* points including the
    /// eventual start and end point.
    pub bend_points: KVectorChain,
    pub properties: PropertyMap,
    /// `InternalProperties.ORIGIN`.
    pub origin: Option<EdgeId>,
    /// `InternalProperties.DUMMY`.
    pub dummy: bool,
}

/// Arena owning every tree-graph element of a layout run.
#[derive(Default, Debug)]
pub struct TArena {
    pub nodes: Vec<TNode>,
    pub edges: Vec<TEdge>,
}

impl TArena {
    pub fn node(&self, id: TNodeId) -> &TNode {
        &self.nodes[id.index()]
    }
    pub fn node_mut(&mut self, id: TNodeId) -> &mut TNode {
        &mut self.nodes[id.index()]
    }
    pub fn edge(&self, id: TEdgeId) -> &TEdge {
        &self.edges[id.index()]
    }
    pub fn edge_mut(&mut self, id: TEdgeId) -> &mut TEdge {
        &mut self.edges[id.index()]
    }

    pub fn create_node(&mut self, id: i32, label: String) -> TNodeId {
        let nid = TNodeId(self.nodes.len() as u32);
        self.nodes.push(TNode { id, label, ..Default::default() });
        nid
    }

    pub fn create_edge(&mut self, source: TNodeId, target: TNodeId) -> TEdgeId {
        let eid = TEdgeId(self.edges.len() as u32);
        self.edges.push(TEdge {
            source,
            target,
            bend_points: KVectorChain::new(),
            properties: PropertyMap::new(),
            origin: None,
            dummy: false,
        });
        eid
    }

    pub fn node_string(&self, n: TNodeId) -> String {
        let node = self.node(n);
        if node.label.is_empty() {
            format!("n_{}", node.id)
        } else {
            format!("n_{}", node.label)
        }
    }

    pub fn edge_string(&self, e: TEdgeId) -> String {
        let edge = self.edge(e);
        format!("{}->{}", self.node_string(edge.source), self.node_string(edge.target))
    }

    /// Source of the first incoming edge.
    pub fn parent(&self, n: TNodeId) -> Option<TNodeId> {
        self.node(n).incoming.first().map(|&e| self.edge(e).source)
    }

    pub fn children(&self, n: TNodeId) -> Vec<TNodeId> {
        self.node(n).outgoing.iter().map(|&e| self.edge(e).target).collect()
    }

    pub fn is_leaf(&self, n: TNodeId) -> bool {
        self.node(n).outgoing.is_empty()
    }

    pub fn tree_level(&self, n: TNodeId) -> i32 {
        self.node(n).properties.get(&options::TREE_LEVEL)
    }

    pub fn set_tree_level(&self, n: TNodeId, level: i32) {
        self.nodes[n.index()].properties.set(&options::TREE_LEVEL, level);
    }
}

/// Element id lists into a shared [`TArena`] plus the
/// graph-level internal properties as fields.
#[derive(Default, Debug)]
pub struct TGraph {
    pub properties: PropertyMap,
    pub nodes: Vec<TNodeId>,
    pub edges: Vec<TEdgeId>,
    /// `InternalProperties.ORIGIN` of the graph.
    pub origin: Option<NodeId>,
    /// `InternalProperties.REMOVABLE_EDGES`.
    pub removable_edges: Vec<TEdgeId>,
    /// `InternalProperties.GRAPH_XMIN` / `GRAPH_YMIN` / `GRAPH_XMAX` / `GRAPH_YMAX`.
    pub graph_xmin: f64,
    pub graph_ymin: f64,
    pub graph_xmax: f64,
    pub graph_ymax: f64,
    /// `InternalProperties.BB_UPLEFT` / `BB_LOWRIGHT`.
    pub bb_upleft: KVector,
    pub bb_lowright: KVector,
    /// `MrTreeOptions.PRIORITY` as set on the graph by `ComponentsProcessor`.
    pub priority: i32,
}

/// Creates a dummy edge in the graph.
pub fn add_child(arena: &mut TArena, graph: &mut TGraph, parent: TNodeId, child: TNodeId) {
    let new_edge = arena.create_edge(parent, child);
    arena.edge_mut(new_edge).dummy = true;
    graph.edges.push(new_edge);
    arena.node_mut(parent).outgoing.push(new_edge);
    arena.node_mut(child).incoming.push(new_edge);
}

/// Reference comparison of two boxed `Integer` property
/// values (`getProperty(TREE_LEVEL) != getProperty(TREE_LEVEL)` in
/// `TreeUtil`): equal values are `==` only while inside the Integer cache
/// range [-128, 127]; outside it every boxing creates a fresh object.
pub fn integer_ref_neq(a: i32, b: i32) -> bool {
    a != b || !(-128..=127).contains(&a)
}
