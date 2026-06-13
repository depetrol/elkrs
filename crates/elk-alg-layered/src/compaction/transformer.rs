//! Port of `LGraphToCGraphTransformer`.

use elk_core::options::{Direction, EdgeRouting, PortSide};
use elk_graph::math::{ElkRectangle, KVector};

use elk_alg_common::compaction::{
    CGraph, CNode, CNodeId, CNodeOrigin, Quadruplet,
};

use crate::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, NodeType};
use crate::internal_properties as iprops;
use crate::options_gen as lopts;

use super::compare_fuzzy;
use super::vertical_segment::{BendRef, JpRef, VerticalSegment};

/// Manages the transformation of an [`LGraph`](crate::graph::LGraph) into a
/// compactable [`CGraph`].
pub struct LGraphToCGraphTransformer {
    /// the layered graph.
    pub graph: LGraphId,
    /// current style of edge routing.
    pub edge_routing: EdgeRouting,
    /// comment box -> (other node, offset). Insertion-ordered like Java's
    /// `HashMap` iteration is here forced to be deterministic.
    comment_offsets: Vec<(LNodeId, LNodeId, KVector)>,

    /// LNodeId -> CNodeId.
    pub nodes_map: Vec<Option<CNodeId>>,
    /// segment index -> CNodeId.
    vertical_segments_map: Vec<Option<CNodeId>>,
    /// All vertical segments (origin index targets these).
    pub segments: Vec<VerticalSegment>,
    /// CNodeId -> Quadruplet (the lock map, used by connection locking).
    pub lock_map: Vec<Quadruplet>,
}

impl LGraphToCGraphTransformer {
    pub fn new() -> Self {
        LGraphToCGraphTransformer {
            graph: LGraphId(0),
            edge_routing: EdgeRouting::ORTHOGONAL,
            comment_offsets: Vec::new(),
            nodes_map: Vec::new(),
            vertical_segments_map: Vec::new(),
            segments: Vec::new(),
            lock_map: Vec::new(),
        }
    }

    /// Java `transform(LGraph)`.
    pub fn transform(&mut self, a: &mut LGraphArena, graph: LGraphId) -> CGraph {
        self.graph = graph;
        self.edge_routing = a.graph(graph).properties.get(&lopts::EDGE_ROUTING);

        let mut cgraph = self.init(a, graph);
        self.transform_nodes(a, graph, &mut cgraph);
        self.transform_edges(a, graph, &mut cgraph);

        cgraph
    }

    fn init(&mut self, a: &mut LGraphArena, graph: LGraphId) -> CGraph {
        // assign layer ids and check whether the graph has edges
        let mut has_edges = false;
        let layers = a.graph(graph).layers.clone();
        for (index, &l) in layers.iter().enumerate() {
            a.layer_mut(l).id = index as i32;
            if !has_edges {
                let nodes = a.layer(l).nodes.clone();
                for n in nodes {
                    if !a.node_connected_edges(n).is_empty() {
                        has_edges = true;
                        break;
                    }
                }
            }
        }

        let mut supported = vec![Direction::UNDEFINED, Direction::LEFT, Direction::RIGHT];
        if !has_edges {
            supported.push(Direction::UP);
            supported.push(Direction::DOWN);
        }

        self.comment_offsets.clear();
        self.nodes_map.clear();
        self.vertical_segments_map.clear();
        self.segments.clear();
        self.lock_map.clear();

        CGraph::new(supported)
    }

    fn transform_nodes(&mut self, a: &mut LGraphArena, graph: LGraphId, cgraph: &mut CGraph) {
        // grow nodes_map to cover all node ids
        self.nodes_map = vec![None; a.nodes.len()];

        let layers = a.graph(graph).layers.clone();
        for layer in layers {
            let nodes = a.layer(layer).nodes.clone();
            for node in nodes {
                // comment boxes are part of a node's margins; neglect here
                if a.node(node).properties.get(&lopts::COMMENT_BOX) {
                    let connected = a.node_connected_edges(node);
                    if !connected.is_empty() {
                        let e = connected[0];
                        let mut other = a.port(a.edge(e).source.unwrap()).node.unwrap();
                        if other == node {
                            other = a.port(a.edge(e).target.unwrap()).node.unwrap();
                        }
                        let offset = {
                            let mut p = a.node(node).pos;
                            p.sub(a.node(other).pos);
                            p
                        };
                        self.comment_offsets.push((node, other, offset));
                        continue;
                    }
                }

                let pos = a.node(node).pos;
                let size = a.node(node).size;
                let m = a.node(node).margin;
                let hitbox = ElkRectangle::new(
                    pos.x - m.left,
                    pos.y - m.top,
                    size.x + m.left + m.right,
                    size.y + m.top + m.bottom,
                );

                // create the CNode living in its own group
                let cnode = cgraph.add_cnode(CNode {
                    origin: CNodeOrigin::LNode(node.0),
                    hitbox,
                    ..Default::default()
                });
                cgraph.add_cgroup_with(&[cnode], Some(cnode));

                let mut node_lock = Quadruplet::new();
                // lock the node for the direction with fewer connected edges
                let difference = a.node_incoming_edges(node).len() as i32
                    - a.node_outgoing_edges(node).len() as i32;
                if difference < 0 {
                    node_lock.set_dir(true, Direction::LEFT);
                } else if difference > 0 {
                    node_lock.set_dir(true, Direction::RIGHT);
                }

                if a.node(node).node_type == NodeType::EXTERNAL_PORT {
                    node_lock.set_all(false, false, false, false);
                }

                self.set_lock(cnode, node_lock);
                self.nodes_map[node.index()] = Some(cnode);
            }
        }
    }

    fn transform_edges(&mut self, a: &mut LGraphArena, graph: LGraphId, cgraph: &mut CGraph) {
        let style = a.graph(graph).properties.get(&lopts::EDGE_ROUTING);
        let segments = match style {
            EdgeRouting::ORTHOGONAL => self.collect_vertical_segments_orthogonal(a, graph, cgraph),
            EdgeRouting::SPLINES => {
                // Splines compaction is not exercised by the configurator (the
                // post-compaction processor is only inserted for non-polyline
                // routings, and splines route is unsupported in this port path).
                panic!("Spline compaction not supported.")
            }
            other => panic!("Compaction not supported for {other:?} edges."),
        };

        // merge them
        self.merge_vertical_segments(a, cgraph, segments);

        // create precomputed constraints
        for vs_idx in 0..self.segments.len() {
            let vs_node = match self.vertical_segments_map.get(vs_idx).copied().flatten() {
                Some(n) => n,
                None => continue,
            };
            // only iterate the segments that are keys in the map (i.e. survivors)
            // Java iterates `verticalSegmentsMap.keySet()` which includes both
            // survivors and joined entries — all mapped to a CNode. We mirror
            // that by iterating every mapped segment.
            let constraints = self.segments[vs_idx].constraints.clone();
            for other in constraints {
                if let Some(other_node) =
                    self.vertical_segments_map.get(other).copied().flatten()
                {
                    cgraph
                        .predefined_horizontal_constraints
                        .push((vs_node, other_node));
                }
            }
        }
    }

    /// Java `collectVerticalSegmentsOrthogonal`. Returns the list of created
    /// segments (stored in `self.segments`, indices returned in order).
    fn collect_vertical_segments_orthogonal(
        &mut self,
        a: &LGraphArena,
        graph: LGraphId,
        cgraph: &CGraph,
    ) -> Vec<usize> {
        let mut result: Vec<usize> = Vec::new();

        let layers = a.graph(graph).layers.clone();
        for layer in layers {
            let nodes = a.layer(layer).nodes.clone();
            for node in nodes {
                let c_node = match self.nodes_map.get(node.index()).copied().flatten() {
                    Some(c) => c,
                    None => continue, // comment boxes have no CNode
                };
                let c_hitbox = cgraph.cnodes[c_node].hitbox;

                // outgoing edges
                for edge in a.node_outgoing_edges(node) {
                    let bends = &a.edge(edge).bend_points.0;
                    if bends.is_empty() {
                        continue;
                    }
                    let jps = self.junction_points_of(a, edge);

                    let mut first = true;
                    let mut last_segment: Option<usize> = None;

                    let mut bend1 = bends[0];
                    let mut bend1_index = 0usize;

                    let src_side = a.port(a.edge(edge).source.unwrap()).side;
                    if src_side == PortSide::NORTH {
                        let mut vs = VerticalSegment::new(
                            bend1,
                            KVector::new(bend1.x, c_hitbox.y),
                            Some(BendRef { edge, index: 0 }),
                            None,
                            Some(c_node),
                            &jps,
                        );
                        vs.ignore_spacing.down = true;
                        vs.a_port = a.edge(edge).source;
                        vs.represented_ledges.push(edge);
                        result.push(self.push_segment(vs));
                    }
                    if src_side == PortSide::SOUTH {
                        let mut vs = VerticalSegment::new(
                            bend1,
                            KVector::new(bend1.x, c_hitbox.y + c_hitbox.height),
                            Some(BendRef { edge, index: 0 }),
                            None,
                            Some(c_node),
                            &jps,
                        );
                        vs.ignore_spacing.up = true;
                        vs.a_port = a.edge(edge).source;
                        vs.represented_ledges.push(edge);
                        result.push(self.push_segment(vs));
                    }

                    // regular segments
                    let mut i = 1usize;
                    while i < bends.len() {
                        let bend2 = bends[i];
                        let bend2_index = i;
                        if !compare_fuzzy::eq(bend1.y, bend2.y) {
                            let mut vs = VerticalSegment::new(
                                bend1,
                                bend2,
                                Some(BendRef { edge, index: bend1_index }),
                                Some(BendRef { edge, index: bend2_index }),
                                None,
                                &jps,
                            );
                            vs.represented_ledges.push(edge);
                            let seg_idx = self.push_segment(vs);
                            result.push(seg_idx);
                            last_segment = Some(seg_idx);

                            if first {
                                first = false;
                                let s = &mut self.segments[seg_idx];
                                if bend2.y < c_hitbox.y {
                                    s.ignore_spacing.down = true;
                                } else if bend2.y > c_hitbox.y + c_hitbox.height {
                                    s.ignore_spacing.up = true;
                                } else {
                                    s.ignore_spacing.up = true;
                                    s.ignore_spacing.down = true;
                                }
                            }
                        }

                        if i + 1 < bends.len() {
                            bend1 = bend2;
                            bend1_index = bend2_index;
                        }
                        i += 1;
                    }

                    // handle last vertical segment (uses final bend1)
                    if let Some(seg_idx) = last_segment {
                        let target_node = a.port(a.edge(edge).target.unwrap()).node.unwrap();
                        let c_target = self.nodes_map[target_node.index()].unwrap();
                        let t_hitbox = cgraph.cnodes[c_target].hitbox;
                        let s = &mut self.segments[seg_idx];
                        if bend1.y < t_hitbox.y {
                            s.ignore_spacing.down = true;
                        } else if bend1.y > t_hitbox.y + t_hitbox.height {
                            s.ignore_spacing.up = true;
                        } else {
                            s.ignore_spacing.up = true;
                            s.ignore_spacing.down = true;
                        }
                    }
                }

                // incoming edges -> n/s segments on target side
                for edge in a.node_incoming_edges(node) {
                    let bends = &a.edge(edge).bend_points.0;
                    if bends.is_empty() {
                        continue;
                    }
                    let jps = self.junction_points_of(a, edge);
                    let last_index = bends.len() - 1;
                    let bend1 = bends[last_index];
                    let tgt_side = a.port(a.edge(edge).target.unwrap()).side;
                    if tgt_side == PortSide::NORTH {
                        let mut vs = VerticalSegment::new(
                            bend1,
                            KVector::new(bend1.x, c_hitbox.y),
                            Some(BendRef { edge, index: last_index }),
                            None,
                            Some(c_node),
                            &jps,
                        );
                        vs.ignore_spacing.down = true;
                        vs.a_port = a.edge(edge).target;
                        vs.represented_ledges.push(edge);
                        result.push(self.push_segment(vs));
                    }
                    if tgt_side == PortSide::SOUTH {
                        let mut vs = VerticalSegment::new(
                            bend1,
                            KVector::new(bend1.x, c_hitbox.y + c_hitbox.height),
                            Some(BendRef { edge, index: last_index }),
                            None,
                            Some(c_node),
                            &jps,
                        );
                        vs.ignore_spacing.up = true;
                        vs.a_port = a.edge(edge).target;
                        vs.represented_ledges.push(edge);
                        result.push(self.push_segment(vs));
                    }
                }
            }
        }

        result
    }

    /// Junction points of an edge as `(JpRef, value)` pairs.
    fn junction_points_of(&self, a: &LGraphArena, edge: LEdgeId) -> Vec<(JpRef, KVector)> {
        let mut out = Vec::new();
        if let Some(jps) = a.edge(edge).properties.try_get(&lopts::JUNCTION_POINTS) {
            for (index, jp) in jps.iter().enumerate() {
                out.push((JpRef { edge, index }, *jp));
            }
        }
        out
    }

    fn push_segment(&mut self, vs: VerticalSegment) -> usize {
        let idx = self.segments.len();
        self.segments.push(vs);
        idx
    }

    fn merge_vertical_segments(
        &mut self,
        a: &LGraphArena,
        cgraph: &mut CGraph,
        mut segments: Vec<usize>,
    ) {
        if segments.is_empty() {
            return;
        }

        // sort by VerticalSegment.compareTo (stable)
        segments.sort_by(|&x, &y| self.segments[x].compare_to(&self.segments[y]));

        let mut it = segments.into_iter();
        let mut survivor = it.next().unwrap();

        for next in it {
            if self.segments[survivor].intersects(&self.segments[next]) {
                let other = self.segments[next].clone();
                self.segments[survivor].join_with(&other, next);
            } else {
                self.vertical_segment_to_cnode(a, cgraph, survivor);
                survivor = next;
            }
        }
        self.vertical_segment_to_cnode(a, cgraph, survivor);
    }

    fn vertical_segment_to_cnode(&mut self, a: &LGraphArena, cgraph: &mut CGraph, vs_idx: usize) {
        let hitbox = self.segments[vs_idx].hitbox;
        let cnode = cgraph.add_cnode(CNode {
            origin: CNodeOrigin::VerticalSegment(vs_idx as u32),
            type_: Some("vs".to_string()),
            hitbox,
            ..Default::default()
        });

        // group the node with the (first) potential group parent if any
        if let Some(&parent) = self.segments[vs_idx].potential_group_parents.first() {
            let parent_group = cgraph.cnodes[parent].cgroup.unwrap();
            cgraph.group_add_cnode(parent_group, cnode);
        }

        let mut vs_lock = Quadruplet::new();
        // lock in the direction with fewer distinct ports connected.
        // Java: distinct source ports vs distinct target ports of representedLEdges.
        let mut inc: Vec<crate::graph::LPortId> = Vec::new();
        let mut out: Vec<crate::graph::LPortId> = Vec::new();
        for &e in &self.segments[vs_idx].represented_ledges {
            if let Some(s) = a.edge(e).source {
                if !inc.contains(&s) {
                    inc.push(s);
                }
            }
            if let Some(t) = a.edge(e).target {
                if !out.contains(&t) {
                    out.push(t);
                }
            }
        }
        let difference = inc.len() as i32 - out.len() as i32;
        if difference < 0 {
            vs_lock.set_dir(true, Direction::LEFT);
            vs_lock.set_dir(false, Direction::RIGHT);
        } else if difference > 0 {
            vs_lock.set_dir(false, Direction::LEFT);
            vs_lock.set_dir(true, Direction::RIGHT);
        }
        self.set_lock(cnode, vs_lock);

        // map this segment and all joined ones to the cnode
        let joined = self.segments[vs_idx].joined.clone();
        self.ensure_vsmap_len();
        for other in joined {
            self.vertical_segments_map[other] = Some(cnode);
        }
        self.vertical_segments_map[vs_idx] = Some(cnode);
    }

    fn ensure_vsmap_len(&mut self) {
        if self.vertical_segments_map.len() < self.segments.len() {
            self.vertical_segments_map.resize(self.segments.len(), None);
        }
    }

    fn set_lock(&mut self, cnode: CNodeId, q: Quadruplet) {
        if self.lock_map.len() <= cnode {
            self.lock_map.resize(cnode + 1, Quadruplet::new());
        }
        self.lock_map[cnode] = q;
    }

    // ---------------------------------------------------------- applyLayout

    /// Java `applyLayout`. Applies compacted positions back to the LGraph and
    /// updates its size and offset.
    pub fn apply_layout(&mut self, a: &mut LGraphArena, cgraph: &CGraph) {
        // apply compacted positions to LNodes
        for cnode in &cgraph.cnodes {
            if let CNodeOrigin::LNode(n) = cnode.origin {
                let node = LNodeId(n);
                let left = a.node(node).margin.left;
                a.node_mut(node).pos.x = cnode.hitbox.x + left;
            }
        }

        // adjust comment boxes
        self.apply_comment_positions(a);

        // apply new positions to vertical segments
        for cnode in &cgraph.cnodes {
            if let CNodeOrigin::VerticalSegment(vs) = cnode.origin {
                let delta_x = cnode.hitbox.x - cnode.hitbox_pre_compaction.x;
                let vs = vs as usize;
                let bends = self.segments[vs].affected_bends.clone();
                for b in bends {
                    a.edge_mut(b.edge).bend_points.0[b.index].x += delta_x;
                }
                let jps = self.segments[vs].junction_points.clone();
                for jp in jps {
                    if let Some(mut chain) =
                        a.edge(jp.edge).properties.try_get(&lopts::JUNCTION_POINTS)
                    {
                        chain.0[jp.index].x += delta_x;
                        a.edge(jp.edge).properties.set(&lopts::JUNCTION_POINTS, chain);
                    }
                }
            }
        }

        // (spline handling omitted: spline routing reaches a panic earlier)

        // offset selfloop labels
        let node_ids: Vec<LNodeId> = (0..self.nodes_map.len())
            .filter(|&i| self.nodes_map[i].is_some())
            .map(|i| LNodeId(i as u32))
            .collect();
        for n in node_ids {
            for sl in a.node_outgoing_edges(n) {
                if a.edge_is_self_loop(sl) {
                    let l_node = a.port(a.edge(sl).source.unwrap()).node.unwrap();
                    let cnode = self.nodes_map[l_node.index()].unwrap();
                    let delta_x =
                        cgraph.cnodes[cnode].hitbox.x - cgraph.cnodes[cnode].hitbox_pre_compaction.x;
                    let labels = a.edge(sl).labels.clone();
                    for l in labels {
                        a.label_mut(l).pos.x += delta_x;
                    }
                }
            }
        }

        // calculate new graph size and offset
        let mut top_left = KVector::new(f64::INFINITY, f64::INFINITY);
        let mut bottom_right = KVector::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for cnode in &cgraph.cnodes {
            top_left.x = top_left.x.min(cnode.hitbox.x);
            top_left.y = top_left.y.min(cnode.hitbox.y);
            bottom_right.x = bottom_right.x.max(cnode.hitbox.x + cnode.hitbox.width);
            bottom_right.y = bottom_right.y.max(cnode.hitbox.y + cnode.hitbox.height);
        }
        {
            let mut neg = top_left;
            neg.negate();
            a.graph_mut(self.graph).offset.reset().add(neg);
        }
        {
            let mut sz = bottom_right;
            sz.sub(top_left);
            a.graph_mut(self.graph).size.reset().add(sz);
        }

        // external port dummies may have moved — put them back
        self.apply_external_port_positions(a, cgraph, top_left, bottom_right);
    }

    fn apply_comment_positions(&self, a: &mut LGraphArena) {
        for &(comment, other, offset) in &self.comment_offsets {
            let mut target = a.node(other).pos;
            target.add(offset);
            a.node_mut(comment).pos.reset().add(target);
        }
    }

    fn apply_external_port_positions(
        &self,
        a: &mut LGraphArena,
        cgraph: &CGraph,
        top_left: KVector,
        bottom_right: KVector,
    ) {
        for cnode in &cgraph.cnodes {
            if let CNodeOrigin::LNode(n) = cnode.origin {
                let node = LNodeId(n);
                if a.node(node).node_type == NodeType::EXTERNAL_PORT {
                    let side = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
                    match side {
                        PortSide::WEST => a.node_mut(node).pos.x = top_left.x,
                        PortSide::EAST => {
                            let sx = a.node(node).size.x;
                            let mr = a.node(node).margin.right;
                            a.node_mut(node).pos.x = bottom_right.x - (sx + mr);
                        }
                        PortSide::NORTH => a.node_mut(node).pos.y = top_left.y,
                        PortSide::SOUTH => {
                            let sy = a.node(node).size.y;
                            let mb = a.node(node).margin.bottom;
                            a.node_mut(node).pos.y = bottom_right.y - (sy + mb);
                        }
                        PortSide::UNDEFINED => {}
                    }
                }
            }
        }
    }
}

impl Default for LGraphToCGraphTransformer {
    fn default() -> Self {
        Self::new()
    }
}
