//! The force algorithm's internal
//! graph model (FGraph, FNode, FEdge, FLabel, FBendpoint, FParticle).
//!
//! All elements live in arenas inside
//! [`FArena`] and reference each other through typed indices. [`FGraph`]
//! instances (the full graph and its connected components) hold id lists
//! into the shared arena; components are formed by moving node ids between
//! `FGraph` objects.

use crate::core::javacompat::JavaRandom;
use crate::graph::graph::{EdgeId, LabelId, NodeId};
use crate::graph::math::KVector;
use crate::graph::properties::PropertyMap;

use crate::alg_force::options;

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

id_type!(FNodeId);
id_type!(FEdgeId);
id_type!(FLabelId);
id_type!(FBendpointId);

/// Any `FParticle` (node, label or bend point). Particles are compared by
/// id.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FParticleId {
    Node(FNodeId),
    Label(FLabelId),
    Bend(FBendpointId),
}

#[derive(Default, Debug)]
pub struct FNode {
    pub properties: PropertyMap,
    pub position: KVector,
    pub size: KVector,
    pub displacement: KVector,
    /// Public `id` field (component-local index).
    pub id: i32,
    pub label: String,
    /// `InternalProperties.ORIGIN`.
    pub origin: Option<NodeId>,
}

#[derive(Debug)]
pub struct FEdge {
    pub properties: PropertyMap,
    pub bendpoints: Vec<FBendpointId>,
    pub labels: Vec<FLabelId>,
    pub source: FNodeId,
    pub target: FNodeId,
    /// `InternalProperties.ORIGIN`.
    pub origin: Option<EdgeId>,
}

#[derive(Debug)]
pub struct FLabel {
    pub properties: PropertyMap,
    pub position: KVector,
    pub size: KVector,
    pub displacement: KVector,
    pub edge: FEdgeId,
    pub text: String,
    /// `InternalProperties.ORIGIN`.
    pub origin: Option<LabelId>,
}

#[derive(Debug)]
pub struct FBendpoint {
    pub properties: PropertyMap,
    pub position: KVector,
    pub size: KVector,
    pub displacement: KVector,
    pub edge: FEdgeId,
}

/// Arena owning every force-graph element of a layout run.
#[derive(Default, Debug)]
pub struct FArena {
    pub nodes: Vec<FNode>,
    pub edges: Vec<FEdge>,
    pub labels: Vec<FLabel>,
    pub bendpoints: Vec<FBendpoint>,
}

impl FArena {
    pub fn node(&self, id: FNodeId) -> &FNode {
        &self.nodes[id.index()]
    }
    pub fn node_mut(&mut self, id: FNodeId) -> &mut FNode {
        &mut self.nodes[id.index()]
    }
    pub fn edge(&self, id: FEdgeId) -> &FEdge {
        &self.edges[id.index()]
    }
    pub fn edge_mut(&mut self, id: FEdgeId) -> &mut FEdge {
        &mut self.edges[id.index()]
    }
    pub fn label(&self, id: FLabelId) -> &FLabel {
        &self.labels[id.index()]
    }
    pub fn label_mut(&mut self, id: FLabelId) -> &mut FLabel {
        &mut self.labels[id.index()]
    }
    pub fn bendpoint(&self, id: FBendpointId) -> &FBendpoint {
        &self.bendpoints[id.index()]
    }
    pub fn bendpoint_mut(&mut self, id: FBendpointId) -> &mut FBendpoint {
        &mut self.bendpoints[id.index()]
    }

    pub fn create_node(&mut self, label: String) -> FNodeId {
        let id = FNodeId(self.nodes.len() as u32);
        self.nodes.push(FNode { label, ..Default::default() });
        id
    }

    pub fn create_edge(&mut self, source: FNodeId, target: FNodeId) -> FEdgeId {
        let id = FEdgeId(self.edges.len() as u32);
        self.edges.push(FEdge {
            properties: PropertyMap::new(),
            bendpoints: Vec::new(),
            labels: Vec::new(),
            source,
            target,
            origin: None,
        });
        id
    }

    /// Also adds the label to the edge.
    pub fn create_label(&mut self, edge: FEdgeId, text: String) -> FLabelId {
        let id = FLabelId(self.labels.len() as u32);
        self.labels.push(FLabel {
            properties: PropertyMap::new(),
            position: KVector::default(),
            size: KVector::default(),
            displacement: KVector::default(),
            edge,
            text,
            origin: None,
        });
        self.edge_mut(edge).labels.push(id);
        id
    }

    /// Also adds the bend point to the edge.
    pub fn create_bendpoint(&mut self, edge: FEdgeId) -> FBendpointId {
        let id = FBendpointId(self.bendpoints.len() as u32);
        self.bendpoints.push(FBendpoint {
            properties: PropertyMap::new(),
            position: KVector::default(),
            size: KVector::default(),
            displacement: KVector::default(),
            edge,
        });
        self.edge_mut(edge).bendpoints.push(id);
        id
    }

    // ----------------------------------------------------- particle access

    pub fn position(&self, p: FParticleId) -> KVector {
        match p {
            FParticleId::Node(n) => self.node(n).position,
            FParticleId::Label(l) => self.label(l).position,
            FParticleId::Bend(b) => self.bendpoint(b).position,
        }
    }

    pub fn position_mut(&mut self, p: FParticleId) -> &mut KVector {
        match p {
            FParticleId::Node(n) => &mut self.node_mut(n).position,
            FParticleId::Label(l) => &mut self.label_mut(l).position,
            FParticleId::Bend(b) => &mut self.bendpoint_mut(b).position,
        }
    }

    pub fn size(&self, p: FParticleId) -> KVector {
        match p {
            FParticleId::Node(n) => self.node(n).size,
            FParticleId::Label(l) => self.label(l).size,
            FParticleId::Bend(b) => self.bendpoint(b).size,
        }
    }

    pub fn radius(&self, p: FParticleId) -> f64 {
        self.size(p).length() / 2.0
    }

    pub fn displacement(&self, p: FParticleId) -> KVector {
        match p {
            FParticleId::Node(n) => self.node(n).displacement,
            FParticleId::Label(l) => self.label(l).displacement,
            FParticleId::Bend(b) => self.bendpoint(b).displacement,
        }
    }

    pub fn displacement_mut(&mut self, p: FParticleId) -> &mut KVector {
        match p {
            FParticleId::Node(n) => &mut self.node_mut(n).displacement,
            FParticleId::Label(l) => &mut self.label_mut(l).displacement,
            FParticleId::Bend(b) => &mut self.bendpoint_mut(b).displacement,
        }
    }

    pub fn properties(&self, p: FParticleId) -> &PropertyMap {
        match p {
            FParticleId::Node(n) => &self.node(n).properties,
            FParticleId::Label(l) => &self.label(l).properties,
            FParticleId::Bend(b) => &self.bendpoint(b).properties,
        }
    }

    // ------------------------------------------------------- edge geometry

    pub fn edge_source_point(&self, e: FEdgeId) -> KVector {
        let edge = self.edge(e);
        let source = self.node(edge.source);
        let target = self.node(edge.target);
        let mut v = target.position;
        v.sub(source.position);
        clip_vector(&mut v, source.size.x, source.size.y);
        v.add(source.position);
        v
    }

    pub fn edge_target_point(&self, e: FEdgeId) -> KVector {
        let edge = self.edge(e);
        let source = self.node(edge.source);
        let target = self.node(edge.target);
        let mut v = source.position;
        v.sub(target.position);
        clip_vector(&mut v, target.size.x, target.size.y);
        v.add(target.position);
        v
    }

    pub fn distribute_bendpoints(&mut self, e: FEdgeId) {
        let count = self.edge(e).bendpoints.len();
        if count > 0 {
            let source_pos = self.node(self.edge(e).source).position;
            let target_pos = self.node(self.edge(e).target).position;
            let mut incr = target_pos;
            incr.sub(source_pos);
            incr.scale(1.0 / (count as f64 + 1.0));
            let mut pos = source_pos;
            let bendpoints = self.edge(e).bendpoints.clone();
            for bp in bendpoints {
                let b = self.bendpoint_mut(bp);
                b.position.x = pos.x + incr.x;
                b.position.y = pos.y + incr.y;
                pos.add(incr);
            }
        }
    }

    pub fn refresh_label_position(&mut self, l: FLabelId) {
        let label = self.label(l);
        let place_inline = label.properties.get(&options::EDGE_LABELS_INLINE);
        let edge = self.edge(label.edge);
        let src = self.node(edge.source).position;
        let tgt = self.node(edge.target).position;

        if place_inline {
            let mut src_to_tgt = tgt;
            src_to_tgt.sub(src);
            src_to_tgt.scale(0.5);
            let mut to_label_center = label.size;
            to_label_center.scale(0.5);
            let mut new_label_position = src;
            new_label_position.add(src_to_tgt);
            new_label_position.sub(to_label_center);
            let pos = &mut self.label_mut(l).position;
            pos.set(new_label_position.x, new_label_position.y);
        } else {
            let spacing: f64 = edge.properties.get(&options::SPACING_EDGE_LABEL);
            let size_y = label.size.y;
            let pos = &mut self.label_mut(l).position;
            if src.x >= tgt.x {
                if src.y >= tgt.y {
                    // CASE1, src top left, tgt bottom right
                    pos.x = tgt.x + ((src.x - tgt.x) / 2.0) + spacing;
                    pos.y = tgt.y + ((src.y - tgt.y) / 2.0) - spacing - size_y;
                } else {
                    // CASE2, src bottom left, tgt top right
                    pos.x = tgt.x + ((src.x - tgt.x) / 2.0) + spacing;
                    pos.y = src.y + ((tgt.y - src.y) / 2.0) + spacing;
                }
            } else if src.y >= tgt.y {
                // CASE2, src top right, tgt bottom left
                pos.x = src.x + ((tgt.x - src.x) / 2.0) + spacing;
                pos.y = tgt.y + ((src.y - tgt.y) / 2.0) + spacing;
            } else {
                // CASE1, src bottom right, tgt top left
                pos.x = src.x + ((tgt.x - src.x) / 2.0) + spacing;
                pos.y = src.y + ((tgt.y - src.y) / 2.0) - spacing - size_y;
            }
        }
    }
}

/// Element id lists into a shared [`FArena`].
#[derive(Default, Debug)]
pub struct FGraph {
    pub properties: PropertyMap,
    pub nodes: Vec<FNodeId>,
    pub edges: Vec<FEdgeId>,
    pub labels: Vec<FLabelId>,
    pub bendpoints: Vec<FBendpointId>,
    /// Adjacency matrix (`calcAdjacency`), indexed by `FNode.id`.
    adjacency: Vec<Vec<i32>>,
    /// `InternalProperties.ORIGIN` of the graph.
    pub origin: Option<NodeId>,
}

impl FGraph {
    /// Nodes, then labels, then bend points.
    pub fn particles(&self) -> Vec<FParticleId> {
        let mut result = Vec::with_capacity(
            self.nodes.len() + self.labels.len() + self.bendpoints.len(),
        );
        result.extend(self.nodes.iter().map(|&n| FParticleId::Node(n)));
        result.extend(self.labels.iter().map(|&l| FParticleId::Label(l)));
        result.extend(self.bendpoints.iter().map(|&b| FParticleId::Bend(b)));
        result
    }

    pub fn calc_adjacency(&mut self, arena: &FArena) {
        let n = self.nodes.len();
        self.adjacency = vec![vec![0; n]; n];
        for &e in &self.edges {
            let edge = arena.edge(e);
            let s = arena.node(edge.source).id as usize;
            let t = arena.node(edge.target).id as usize;
            let priority: i32 = edge.properties.get(&options::PRIORITY);
            self.adjacency[s][t] = self.adjacency[s][t].wrapping_add(priority);
        }
    }

    pub fn connection(&self, arena: &FArena, p1: FParticleId, p2: FParticleId) -> i32 {
        match (p1, p2) {
            (FParticleId::Node(n1), FParticleId::Node(n2)) => {
                let id1 = arena.node(n1).id as usize;
                let id2 = arena.node(n2).id as usize;
                self.adjacency[id1][id2].wrapping_add(self.adjacency[id2][id1])
            }
            (FParticleId::Bend(b1), FParticleId::Bend(b2)) => {
                let e1 = arena.bendpoint(b1).edge;
                let e2 = arena.bendpoint(b2).edge;
                if e1 == e2 {
                    arena.edge(e2).properties.get(&options::PRIORITY)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }
}

pub fn clip_vector(v: &mut KVector, width: f64, height: f64) {
    let wh = width / 2.0;
    let hh = height / 2.0;
    let absx = v.x.abs();
    let absy = v.y.abs();
    let mut xscale = 1.0;
    let mut yscale = 1.0;
    if absx > wh {
        xscale = wh / absx;
    }
    if absy > hh {
        yscale = hh / absy;
    }
    v.scale(f64::min(xscale, yscale));
}

pub fn wiggle(v: &mut KVector, random: &mut JavaRandom, amount: f64) {
    v.x += random.next_double() * amount - amount / 2.0;
    v.y += random.next_double() * amount - amount / 2.0;
}
