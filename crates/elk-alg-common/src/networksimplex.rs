//!
//! All nodes and edges live in arenas inside
//! [`NGraph`] and reference each other through [`NNodeId`] / [`NEdgeId`].
//! `NGraph.nodes` (the ordered node list) is kept as an
//! explicit `Vec<NNodeId>` so that removal/re-insertion order during subtree
//! handling is preserved.
//!
//! Not ported: `NGraph.writeDebugGraph` (EMF debug output) and the `NNode.type`
//! debug label, both without semantic effect.

use std::collections::VecDeque;

/// Index of an [`NNode`] in its [`NGraph`] arena.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NNodeId(pub u32);

/// Index of an [`NEdge`] in its [`NGraph`] arena.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NEdgeId(pub u32);

#[derive(Debug)]
pub struct NNode {
    /// A public id, unused internally, use it for whatever you want.
    pub id: i32,
    /// An index into whatever structure
    /// this node was derived from (-1 if unset).
    pub origin: i32,
    /// The layer this node is currently assigned to.
    pub layer: i32,
    /// Incoming edges.
    pub incoming_edges: Vec<NEdgeId>,
    /// Outgoing edges.
    pub outgoing_edges: Vec<NEdgeId>,
    /// Internally set and used id to index arrays.
    internal_id: usize,
    /// Whether this node is part of the spanning tree.
    tree_node: bool,
    /// Tree edges incident to this node with unknown cut values.
    unknown_cutvalues: Vec<NEdgeId>,
}

impl NNode {
    /// Incoming edges first, then outgoing edges.
    pub fn connected_edges(&self) -> Vec<NEdgeId> {
        let mut all = Vec::with_capacity(self.incoming_edges.len() + self.outgoing_edges.len());
        all.extend_from_slice(&self.incoming_edges);
        all.extend_from_slice(&self.outgoing_edges);
        all
    }
}

#[derive(Debug)]
pub struct NEdge {
    /// A public id, unused internally, use it for whatever you want.
    pub id: i32,
    /// Object origin (-1 if unset). Note that
    /// `NEdge.of(Object origin)` actually ignores its argument (a bug),
    /// so edge origins are never set by the layerer either.
    pub origin: i32,
    /// The source node of this edge.
    pub source: NNodeId,
    /// The target node of this edge.
    pub target: NNodeId,
    /// The weight of this edge.
    pub weight: f64,
    /// The minimum length of this edge (default 1).
    pub delta: i32,
    /// Internally set and used id to index arrays.
    internal_id: usize,
    /// Whether this edge is part of the spanning tree.
    tree_edge: bool,
}

impl NEdge {
    pub fn other(&self, some: NNodeId) -> NNodeId {
        if some == self.source {
            self.target
        } else if some == self.target {
            self.source
        } else {
            panic!("Node {:?} not part of edge {:?}", some, self.id);
        }
    }
}

/// Arena plus the ordered node list.
#[derive(Default, Debug)]
pub struct NGraph {
    /// The nodes of the network simplex graph (ordered).
    pub nodes: Vec<NNodeId>,
    node_arena: Vec<NNode>,
    edge_arena: Vec<NEdge>,
}

impl NGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn node(&self, id: NNodeId) -> &NNode {
        &self.node_arena[id.0 as usize]
    }
    pub fn node_mut(&mut self, id: NNodeId) -> &mut NNode {
        &mut self.node_arena[id.0 as usize]
    }
    pub fn edge(&self, id: NEdgeId) -> &NEdge {
        &self.edge_arena[id.0 as usize]
    }
    pub fn edge_mut(&mut self, id: NEdgeId) -> &mut NEdge {
        &mut self.edge_arena[id.0 as usize]
    }

    /// Creates a node and appends it to
    /// the graph's node list.
    pub fn add_node(&mut self) -> NNodeId {
        let id = NNodeId(self.node_arena.len() as u32);
        self.node_arena.push(NNode {
            id: 0,
            origin: -1,
            layer: 0,
            incoming_edges: Vec::new(),
            outgoing_edges: Vec::new(),
            internal_id: 0,
            tree_node: false,
            unknown_cutvalues: Vec::new(),
        });
        self.nodes.push(id);
        id
    }

    /// Creates an edge and registers it with
    /// the source's outgoing and the target's incoming edge lists.
    ///
    /// Panics on self-loops.
    pub fn add_edge(&mut self, source: NNodeId, target: NNodeId, weight: f64, delta: i32) -> NEdgeId {
        if source == target {
            panic!("Network simplex does not support self-loops: {:?}", source);
        }
        let id = NEdgeId(self.edge_arena.len() as u32);
        self.edge_arena.push(NEdge {
            id: 0,
            origin: -1,
            source,
            target,
            weight,
            delta,
            internal_id: 0,
            tree_edge: false,
        });
        self.node_mut(source).outgoing_edges.push(id);
        self.node_mut(target).incoming_edges.push(id);
        id
    }

    pub fn reverse_edge(&mut self, edge: NEdgeId) {
        let e = self.edge_mut(edge);
        let tmp = e.source;
        e.source = e.target;
        e.target = tmp;
        let (source, target) = (e.source, e.target);

        remove_first(&mut self.node_mut(target).outgoing_edges, edge);
        self.node_mut(target).incoming_edges.push(edge);

        remove_first(&mut self.node_mut(source).incoming_edges, edge);
        self.node_mut(source).outgoing_edges.push(edge);
    }

    /// If the graph is not connected, one
    /// representative per connected component is connected to a new artificial
    /// root node (zero-weight, zero-delta edges), which is returned.
    pub fn make_connected(&mut self) -> Option<NNodeId> {
        for (id, &n) in self.nodes.clone().iter().enumerate() {
            self.node_mut(n).internal_id = id;
        }
        let cc_rep = self.find_con_comp_representatives();
        if cc_rep.len() > 1 {
            Some(self.create_artificial_root_and_connect(&cc_rep))
        } else {
            None
        }
    }

    fn create_artificial_root_and_connect(&mut self, nodes_to_connect: &[NNodeId]) -> NNodeId {
        let root = self.add_node();
        for &src in nodes_to_connect {
            self.add_edge(root, src, 0.0, 0);
        }
        root
    }

    fn find_con_comp_representatives(&self) -> Vec<NNodeId> {
        let mut cc_rep = Vec::new();
        let mut mark = vec![false; self.nodes.len()];
        for &node in &self.nodes {
            if !mark[self.node(node).internal_id] {
                cc_rep.push(node);
                self.dfs(node, &mut mark);
            }
        }
        cc_rep
    }

    fn dfs(&self, node: NNodeId, mark: &mut [bool]) {
        if mark[self.node(node).internal_id] {
            return;
        }
        mark[self.node(node).internal_id] = true;
        for edge in self.node(node).connected_edges() {
            let other = self.edge(edge).other(node);
            self.dfs(other, mark);
        }
    }

    /// Creates a topological ordering and checks for
    /// back edges.
    pub fn is_acyclic(&mut self) -> bool {
        for (id, &n) in self.nodes.clone().iter().enumerate() {
            self.node_mut(n).internal_id = id;
        }

        // initialize the number of incident edges for each node
        let mut incident = vec![0i32; self.nodes.len()];
        let mut layer = vec![0i32; self.nodes.len()];
        for &node in &self.nodes {
            incident[self.node(node).internal_id] += self.node(node).incoming_edges.len() as i32;
        }

        let mut roots: VecDeque<NNodeId> = VecDeque::new();
        for &node in &self.nodes {
            if self.node(node).incoming_edges.is_empty() {
                roots.push_back(node);
            }
        }
        if roots.is_empty() && !self.nodes.is_empty() {
            return false;
        }
        while let Some(node) = roots.pop_front() {
            for &edge in &self.node(node).outgoing_edges {
                let target = self.edge(edge).target;
                let t = self.node(target).internal_id;
                let n = self.node(node).internal_id;
                layer[t] = layer[t].max(layer[n] + 1);
                incident[t] -= 1;
                if incident[t] == 0 {
                    roots.push_back(target);
                }
            }
        }

        // check for backward edges
        for &node in &self.nodes {
            for &edge in &self.node(node).outgoing_edges {
                let e = self.edge(edge);
                if layer[self.node(e.target).internal_id] <= layer[self.node(e.source).internal_id] {
                    return false;
                }
            }
        }

        true
    }
}

/// Removes the first occurrence of `value` from `v`.
fn remove_first<T: PartialEq>(v: &mut Vec<T>, value: T) {
    if let Some(pos) = v.iter().position(|x| *x == value) {
        v.remove(pos);
    }
}

/// Empirically determined threshold when removing subtrees pays off.
const REMOVE_SUBTREES_THRESH: usize = 40;

/// Small value smaller than zero, to deal with double imprecision of cut values.
const FUZZY_ST_ZERO: f64 = -1e-10;

/// Determines an optimal layering of all nodes in
/// the graph concerning a minimal weighted length of all edges (Gansner et al.).
///
/// Precondition: the graph has no cycles. Postcondition: all nodes have been
/// assigned a layer such that edges connect only nodes from layers with
/// increasing indices.
pub struct NetworkSimplex<'g> {
    /// The graph all methods in this struct operate on.
    graph: &'g mut NGraph,
    /// Number of nodes per layer of a previous layering.
    previous_layering_node_counts: Option<Vec<i32>>,
    /// Whether to apply balancing.
    balance: bool,
    /// A limit on the number of iterations.
    iteration_limit: i32,

    /// All edges in the graph, in iteration order.
    edges: Vec<NEdgeId>,
    /// All edges that are part of the spanning tree (insertion-ordered).
    tree_edges: Vec<NEdgeId>,
    /// All source nodes of the graph (no incoming edges).
    sources: Vec<NNodeId>,
    /// Whether an edge has been visited during DFS-traversal.
    edge_visited: Vec<bool>,
    /// The current postorder traversal number.
    post_order: i32,
    /// The postorder traversal ID of each node.
    po_id: Vec<i32>,
    /// The lowest postorder traversal ID reachable through a node lower in the
    /// traversal tree.
    lowest_po_id: Vec<i32>,
    /// The cut value of every edge.
    cutvalue: Vec<f64>,
    /// Subtree nodes removed prior to execution, with their single edge
    /// (used as a stack).
    subtree_nodes_stack: Vec<(NNodeId, NEdgeId)>,
}

impl<'g> NetworkSimplex<'g> {
    pub fn for_graph(graph: &'g mut NGraph) -> Self {
        NetworkSimplex {
            graph,
            previous_layering_node_counts: None,
            balance: false,
            iteration_limit: i32::MAX,
            edges: Vec::new(),
            tree_edges: Vec::new(),
            sources: Vec::new(),
            edge_visited: Vec::new(),
            post_order: 1,
            po_id: Vec::new(),
            lowest_po_id: Vec::new(),
            cutvalue: Vec::new(),
            subtree_nodes_stack: Vec::new(),
        }
    }

    pub fn with_balancing(mut self, do_balance: bool) -> Self {
        self.balance = do_balance;
        self
    }

    pub fn with_previous_layering(mut self, consider_previous_layering: Option<Vec<i32>>) -> Self {
        self.previous_layering_node_counts = consider_previous_layering;
        self
    }

    pub fn with_iteration_limit(mut self, limit: i32) -> Self {
        self.iteration_limit = limit;
        self
    }

    /// Determine the optimal layering. The result is
    /// stored in each node's `layer` field.
    pub fn execute(mut self) {
        if self.graph.nodes.is_empty() {
            return;
        }

        // reset any old layering
        for &node in &self.graph.nodes.clone() {
            self.graph.node_mut(node).layer = 0;
        }

        // remove leafs
        let remove_subtrees = self.graph.nodes.len() >= REMOVE_SUBTREES_THRESH;
        if remove_subtrees {
            self.remove_subtrees();
        }

        // init all the data structures we use
        self.initialize();
        // determine an initial feasible layering
        self.feasible_tree();
        // improve the initial layering until it is optimal
        let mut e = self.leave_edge();
        let mut iter = 0;
        while e.is_some() && iter < self.iteration_limit {
            // current layering is not optimal
            let leave = e.unwrap();
            let enter = self.enter_edge(leave).expect("no entering edge found");
            self.exchange(leave, enter);
            e = self.leave_edge();
            iter += 1;
        }

        // re-attach leafs
        if remove_subtrees {
            self.reattach_subtrees();
        }

        // normalize and, if desired, balance
        if self.balance {
            let mut filling = self.normalize();
            self.balance(&mut filling);
        } else {
            self.normalize();
        }
    }

    fn initialize(&mut self) {
        // initialize node attributes
        let num_nodes = self.graph.nodes.len();
        for &n in &self.graph.nodes.clone() {
            self.graph.node_mut(n).tree_node = false;
        }
        self.po_id = vec![0; num_nodes];
        self.lowest_po_id = vec![0; num_nodes];
        self.sources = Vec::new();

        // determine edges and re-index nodes
        let mut index = 0;
        let mut the_edges: Vec<NEdgeId> = Vec::new();
        for &node in &self.graph.nodes.clone() {
            self.graph.node_mut(node).internal_id = index;
            index += 1;
            // add node to sinks, resp. sources
            if self.graph.node(node).incoming_edges.is_empty() {
                self.sources.push(node);
            }
            the_edges.extend_from_slice(&self.graph.node(node).outgoing_edges);
        }
        // re-index edges
        let mut counter = 0;
        for &edge in &the_edges {
            let e = self.graph.edge_mut(edge);
            e.internal_id = counter;
            counter += 1;
            e.tree_edge = false;
        }
        // initialize edge attributes
        let num_edges = the_edges.len();
        self.cutvalue = vec![0.0; num_edges];
        self.edge_visited = vec![false; num_edges];
        self.edges = the_edges;
        self.tree_edges = Vec::with_capacity(self.edges.len());
        self.post_order = 1;
    }

    /// Recursively removes leafs from the graph
    /// until no more leafs are present.
    fn remove_subtrees(&mut self) {
        self.subtree_nodes_stack = Vec::new();

        // find initial leafs
        let mut leafs: VecDeque<NNodeId> = VecDeque::new();
        for &node in &self.graph.nodes {
            if self.graph.node(node).connected_edges().len() == 1 {
                leafs.push_back(node);
            }
        }

        // remove them from the graph like there's no tomorrow
        while let Some(node) = leafs.pop_front() {
            // was the edge already removed?
            let connected = self.graph.node(node).connected_edges();
            if connected.is_empty() {
                continue;
            }
            let edge = connected[0];
            let is_out_edge = !self.graph.node(node).outgoing_edges.is_empty();

            let other = self.graph.edge(edge).other(node);
            if is_out_edge {
                remove_first(&mut self.graph.node_mut(other).incoming_edges, edge);
            } else {
                remove_first(&mut self.graph.node_mut(other).outgoing_edges, edge);
            }

            if self.graph.node(other).connected_edges().len() == 1 {
                leafs.push_back(other);
            }

            self.subtree_nodes_stack.push((node, edge));
            // remove the node from the graph's nodes
            remove_first(&mut self.graph.nodes, node);
        }
    }

    /// Re-attaches the previously removed tree
    /// nodes in the opposite order than they were removed.
    fn reattach_subtrees(&mut self) {
        while let Some((node, edge)) = self.subtree_nodes_stack.pop() {
            let placed = self.graph.edge(edge).other(node);

            if self.graph.edge(edge).target == node {
                self.graph.node_mut(placed).outgoing_edges.push(edge);
                self.graph.node_mut(node).layer =
                    self.graph.node(placed).layer + self.graph.edge(edge).delta;
            } else {
                self.graph.node_mut(placed).incoming_edges.push(edge);
                self.graph.node_mut(node).layer =
                    self.graph.node(placed).layer - self.graph.edge(edge).delta;
            }

            self.graph.nodes.push(node);
        }
    }

    /// Determines an initial feasible (tight)
    /// spanning tree of the graph and computes initial cut values.
    fn feasible_tree(&mut self) {
        // determine initial layering
        self.layering_topological_numbering();

        if !self.edges.is_empty() {
            self.edge_visited.fill(false);
            while self.tight_tree_dfs(self.graph.nodes[0]) < self.graph.nodes.len() {
                // some nodes are still not part of the tree
                let e = self.minimal_slack().expect("no minimal slack edge found");
                let edge = self.graph.edge(e);
                let mut slack = self.graph.node(edge.target).layer
                    - self.graph.node(edge.source).layer
                    - edge.delta;
                if self.graph.node(edge.target).tree_node {
                    slack = -slack;
                }

                // update tree
                for &node in &self.graph.nodes.clone() {
                    if self.graph.node(node).tree_node {
                        self.graph.node_mut(node).layer += slack;
                    }
                }
                self.edge_visited.fill(false);
            }
            // update tree-related attributes
            self.edge_visited.fill(false);
            self.postorder_traversal(self.graph.nodes[0]);
            self.cutvalues();
        }
    }

    fn layering_topological_numbering(&mut self) {
        // initialize the number of incident edges for each node
        let mut incident = vec![0i32; self.graph.nodes.len()];
        for &node in &self.graph.nodes {
            incident[self.graph.node(node).internal_id] +=
                self.graph.node(node).incoming_edges.len() as i32;
        }

        let mut roots: VecDeque<NNodeId> = self.sources.iter().copied().collect();
        while let Some(node) = roots.pop_front() {
            for edge in self.graph.node(node).outgoing_edges.clone() {
                let e = self.graph.edge(edge);
                let target = e.target;
                let delta = e.delta;
                let new_layer = self.graph.node(node).layer + delta;
                let t = self.graph.node_mut(target);
                t.layer = t.layer.max(new_layer);
                let tid = self.graph.node(target).internal_id;
                incident[tid] -= 1;
                if incident[tid] == 0 {
                    roots.push_back(target);
                }
            }
        }
    }

    /// The length of the currently shortest
    /// incoming (first) and outgoing (second) edge of the node, or -1 if no
    /// such edge is incident.
    fn minimal_span(&self, node: NNodeId) -> (i32, i32) {
        let mut min_span_out = i32::MAX;
        let mut min_span_in = i32::MAX;

        for edge in self.graph.node(node).connected_edges() {
            let e = self.graph.edge(edge);
            let current_span = self.graph.node(e.target).layer - self.graph.node(e.source).layer;
            if e.target == node && current_span < min_span_in {
                min_span_in = current_span;
            } else if current_span < min_span_out {
                min_span_out = current_span;
            }
        }

        if min_span_in == i32::MAX {
            min_span_in = -1;
        }
        if min_span_out == i32::MAX {
            min_span_out = -1;
        }

        (min_span_in, min_span_out)
    }

    /// Determines a DFS-subtree of the graph by
    /// traversing tight edges only, returning the number of nodes in it.
    fn tight_tree_dfs(&mut self, node: NNodeId) -> usize {
        let mut node_count = 1;
        self.graph.node_mut(node).tree_node = true;
        for edge in self.graph.node(node).connected_edges() {
            let internal_id = self.graph.edge(edge).internal_id;
            if !self.edge_visited[internal_id] {
                self.edge_visited[internal_id] = true;
                let opposite = self.graph.edge(edge).other(node);
                if self.graph.edge(edge).tree_edge {
                    // edge is a tree edge already: follow this path
                    node_count += self.tight_tree_dfs(opposite);
                } else {
                    let e = self.graph.edge(edge);
                    let tight = e.delta
                        == self.graph.node(e.target).layer - self.graph.node(e.source).layer;
                    if !self.graph.node(opposite).tree_node && tight {
                        // edge is a tight non-tree edge
                        self.graph.edge_mut(edge).tree_edge = true;
                        self.tree_edges.push(edge);
                        node_count += self.tight_tree_dfs(opposite);
                    }
                }
            }
        }
        node_count
    }

    /// The non-tree edge incident on the tree with a
    /// minimal amount of slack, or `None` if no such edge exists.
    fn minimal_slack(&self) -> Option<NEdgeId> {
        let mut min_slack = i32::MAX;
        let mut min_slack_edge = None;
        for &edge in &self.edges {
            let e = self.graph.edge(edge);
            if self.graph.node(e.source).tree_node != self.graph.node(e.target).tree_node {
                // edge is non-tree edge and incident on the tree
                let cur_slack =
                    self.graph.node(e.target).layer - self.graph.node(e.source).layer - e.delta;
                if cur_slack < min_slack {
                    min_slack = cur_slack;
                    min_slack_edge = Some(edge);
                }
            }
        }
        min_slack_edge
    }

    /// Postorder DFS-traversal assigning
    /// each node a unique traversal ID (`po_id`) and the lowest ID reachable
    /// through descending paths (`lowest_po_id`).
    fn postorder_traversal(&mut self, node: NNodeId) -> i32 {
        let mut lowest = i32::MAX;
        for edge in self.graph.node(node).connected_edges() {
            let internal_id = self.graph.edge(edge).internal_id;
            if self.graph.edge(edge).tree_edge && !self.edge_visited[internal_id] {
                self.edge_visited[internal_id] = true;
                let other = self.graph.edge(edge).other(node);
                lowest = lowest.min(self.postorder_traversal(other));
            }
        }
        let nid = self.graph.node(node).internal_id;
        self.po_id[nid] = self.post_order;
        self.lowest_po_id[nid] = lowest.min(self.post_order);
        self.post_order += 1;
        self.lowest_po_id[nid]
    }

    /// Whether the node is part of the head
    /// component of the given tree edge (the component containing the edge's
    /// target if the edge were removed from the tree).
    fn is_in_head(&self, node: NNodeId, edge: NEdgeId) -> bool {
        let e = self.graph.edge(edge);
        let source = self.graph.node(e.source).internal_id;
        let target = self.graph.node(e.target).internal_id;
        let node = self.graph.node(node).internal_id;

        if self.lowest_po_id[source] <= self.po_id[node]
            && self.po_id[node] <= self.po_id[source]
            && self.lowest_po_id[target] <= self.po_id[node]
            && self.po_id[node] <= self.po_id[target]
        {
            // node is in a descending path in the DFS-Tree
            if self.po_id[source] < self.po_id[target] {
                // root is in the head component
                return false;
            }
            return true;
        }
        if self.po_id[source] < self.po_id[target] {
            // root is in the head component
            return true;
        }
        false
    }

    /// Determines the cut value of each tree edge.
    fn cutvalues(&mut self) {
        // determine incident tree edges for each node
        let mut leafs: Vec<NNodeId> = Vec::new();
        for &node in &self.graph.nodes.clone() {
            let mut tree_edge_count = 0;
            self.graph.node_mut(node).unknown_cutvalues.clear();
            for edge in self.graph.node(node).connected_edges() {
                if self.graph.edge(edge).tree_edge {
                    self.graph.node_mut(node).unknown_cutvalues.push(edge);
                    tree_edge_count += 1;
                }
            }
            if tree_edge_count == 1 {
                leafs.push(node);
            }
        }

        // determine cut values
        for &leaf in &leafs {
            let mut node = leaf;
            while self.graph.node(node).unknown_cutvalues.len() == 1 {
                // one tree edge with undetermined cut value is incident
                let to_determine = self.graph.node(node).unknown_cutvalues[0];
                let td = self.graph.edge(to_determine).internal_id;
                self.cutvalue[td] = self.graph.edge(to_determine).weight;
                let source = self.graph.edge(to_determine).source;
                let target = self.graph.edge(to_determine).target;
                for edge in self.graph.node(node).connected_edges() {
                    if edge != to_determine {
                        let e = self.graph.edge(edge);
                        if e.tree_edge {
                            // edge is tree edge
                            let eid = e.internal_id;
                            let e_weight = e.weight;
                            if source == e.source || target == e.target {
                                // edge has not the same direction as toDetermine
                                self.cutvalue[td] -= self.cutvalue[eid] - e_weight;
                            } else {
                                self.cutvalue[td] += self.cutvalue[eid] - e_weight;
                            }
                        } else {
                            // edge is non-tree edge
                            if node == source {
                                if e.source == node {
                                    self.cutvalue[td] += e.weight;
                                } else {
                                    self.cutvalue[td] -= e.weight;
                                }
                            } else if e.source == node {
                                self.cutvalue[td] -= e.weight;
                            } else {
                                self.cutvalue[td] += e.weight;
                            }
                        }
                    }
                }

                // remove edge from 'unknownCutvalues'
                remove_first(&mut self.graph.node_mut(source).unknown_cutvalues, to_determine);
                remove_first(&mut self.graph.node_mut(target).unknown_cutvalues, to_determine);

                // proceed with next node
                if source == node {
                    node = self.graph.edge(to_determine).target;
                } else {
                    node = self.graph.edge(to_determine).source;
                }
            }
        }
    }

    /// Returns a tree edge with a negative cut value,
    /// or `None` if no such edge exists (the layering is optimal).
    fn leave_edge(&self) -> Option<NEdgeId> {
        for &edge in &self.tree_edges {
            if self.graph.edge(edge).tree_edge
                && self.cutvalue[self.graph.edge(edge).internal_id] < FUZZY_ST_ZERO
            {
                return Some(edge);
            }
        }
        None
    }

    /// Determines a non-tree edge (going from the
    /// head to the tail component of `leave`) with a minimal amount of slack
    /// to replace the given tree edge.
    fn enter_edge(&self, leave: NEdgeId) -> Option<NEdgeId> {
        if !self.graph.edge(leave).tree_edge {
            panic!("The input edge is not a tree edge.");
        }

        let mut replace = None;
        let mut rep_slack = i32::MAX;
        for &edge in &self.edges {
            let e = self.graph.edge(edge);
            let (source, target) = (e.source, e.target);
            if self.is_in_head(source, leave) && !self.is_in_head(target, leave) {
                // edge is to consider
                let slack =
                    self.graph.node(target).layer - self.graph.node(source).layer - e.delta;
                if slack < rep_slack {
                    rep_slack = slack;
                    replace = Some(edge);
                }
            }
        }
        replace
    }

    /// Exchanges the tree edge `leave` with
    /// the non-tree edge `enter` and updates all tree-based values.
    fn exchange(&mut self, leave: NEdgeId, enter: NEdgeId) {
        if !self.graph.edge(leave).tree_edge {
            panic!("Given leave edge is no tree edge.");
        }
        if self.graph.edge(enter).tree_edge {
            panic!("Given enter edge is a tree edge already.");
        }

        // update tree
        self.graph.edge_mut(leave).tree_edge = false;
        remove_first(&mut self.tree_edges, leave);
        self.graph.edge_mut(enter).tree_edge = true;
        self.tree_edges.push(enter);
        let e = self.graph.edge(enter);
        let mut delta =
            self.graph.node(e.target).layer - self.graph.node(e.source).layer - e.delta;
        if !self.is_in_head(self.graph.edge(enter).target, leave) {
            delta = -delta;
        }
        for &node in &self.graph.nodes.clone() {
            if !self.is_in_head(node, leave) {
                self.graph.node_mut(node).layer += delta;
            }
        }

        // update tree-based values
        self.post_order = 1;
        self.edge_visited.fill(false);
        self.postorder_traversal(self.graph.nodes[0]);
        self.cutvalues();
    }

    /// Shifts all layers such that the lowest assigned
    /// layer is zero, and returns the number of nodes assigned to each layer.
    fn normalize(&mut self) -> Vec<i32> {
        // determine lowest assigned layer and layer count
        let mut highest = i32::MIN;
        let mut lowest = i32::MAX;
        for &node in &self.graph.nodes {
            lowest = lowest.min(self.graph.node(node).layer);
            highest = highest.max(self.graph.node(node).layer);
        }
        // normalize and determine layer filling
        let mut filling = vec![0i32; (highest - lowest + 1) as usize];
        for &node in &self.graph.nodes.clone() {
            self.graph.node_mut(node).layer -= lowest;
            filling[self.graph.node(node).layer as usize] += 1;
        }

        // also consider nodes of already layered connected components
        let mut layer_id = 0;
        if let Some(previous) = &self.previous_layering_node_counts {
            for &node_cnt_in_layer in previous {
                filling[layer_id] += node_cnt_in_layer;
                layer_id += 1;
                if filling.len() == layer_id {
                    break;
                }
            }
        }
        filling
    }

    /// Balances the layering concerning its width by
    /// moving separate nodes to a layer with a minimal amount of currently
    /// contained nodes, retaining feasibility of the layering.
    fn balance(&mut self, filling: &mut [i32]) {
        // determine possible layers
        for &node in &self.graph.nodes.clone() {
            let n = self.graph.node(node);
            if n.incoming_edges.len() == n.outgoing_edges.len() {
                // node might get shifted
                let mut new_layer = n.layer;
                let (min_span_in, min_span_out) = self.minimal_span(node);
                let node_layer = self.graph.node(node).layer;
                let mut i = node_layer - min_span_in + 1;
                while i < node_layer + min_span_out {
                    if filling[i as usize] < filling[new_layer as usize] {
                        new_layer = i;
                    }
                    i += 1;
                }
                // assign new layer
                if filling[new_layer as usize] < filling[node_layer as usize] {
                    filling[node_layer as usize] -= 1;
                    filling[new_layer as usize] += 1;
                    self.graph.node_mut(node).layer = new_layer;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layers(graph: &NGraph, nodes: &[NNodeId]) -> Vec<i32> {
        nodes.iter().map(|&n| graph.node(n).layer).collect()
    }

    /// Diamond plus a chain tail: n0->n1, n0->n2, n1->n3, n2->n3, n3->n4.
    /// Hand-traced: the initial topological
    /// numbering is already optimal, no exchanges happen, balancing moves
    /// nothing.
    #[test]
    fn diamond_with_chain() {
        let mut g = NGraph::new();
        let n: Vec<NNodeId> = (0..5).map(|_| g.add_node()).collect();
        g.add_edge(n[0], n[1], 1.0, 1);
        g.add_edge(n[0], n[2], 1.0, 1);
        g.add_edge(n[1], n[3], 1.0, 1);
        g.add_edge(n[2], n[3], 1.0, 1);
        g.add_edge(n[3], n[4], 1.0, 1);

        NetworkSimplex::for_graph(&mut g).with_balancing(true).execute();

        assert_eq!(layers(&g, &n), vec![0, 1, 1, 2, 3]);
    }

    /// Chain a->b->c->d plus x with only one outgoing edge x->d. The initial
    /// topological numbering puts x into layer 0; the tight-tree construction
    /// must shift the partial tree (minimal slack -2) so that x ends up tight
    /// at layer 2 (after normalization).
    #[test]
    fn minimal_slack_tree_shift() {
        let mut g = NGraph::new();
        let a = g.add_node();
        let b = g.add_node();
        let c = g.add_node();
        let d = g.add_node();
        let x = g.add_node();
        g.add_edge(a, b, 1.0, 1);
        g.add_edge(b, c, 1.0, 1);
        g.add_edge(c, d, 1.0, 1);
        g.add_edge(x, d, 1.0, 1);

        NetworkSimplex::for_graph(&mut g).execute();

        assert_eq!(layers(&g, &[a, b, c, d, x]), vec![0, 1, 2, 3, 2]);
    }

    /// Chain a->b->c->z plus a->m (weight 1) and m->z (weight 10). The
    /// initial feasible tree contains a->m with cut value -9, so one
    /// exchange with the entering edge m->z happens, moving m from layer 1
    /// to layer 2 (heavy edge m->z becomes short).
    #[test]
    fn exchange_with_negative_cutvalue() {
        let mut g = NGraph::new();
        let a = g.add_node();
        let b = g.add_node();
        let c = g.add_node();
        let z = g.add_node();
        let m = g.add_node();
        g.add_edge(a, b, 1.0, 1);
        g.add_edge(a, m, 1.0, 1);
        g.add_edge(b, c, 1.0, 1);
        g.add_edge(c, z, 1.0, 1);
        g.add_edge(m, z, 10.0, 1);

        NetworkSimplex::for_graph(&mut g).execute();

        assert_eq!(layers(&g, &[a, b, c, z, m]), vec![0, 1, 2, 3, 2]);
    }

    /// Same graph as above, but with balancing: balancing only looks
    /// at layer fillings (not edge weights) and moves m back to the less
    /// crowded layer 1.
    #[test]
    fn balancing_ignores_weights() {
        let mut g = NGraph::new();
        let a = g.add_node();
        let b = g.add_node();
        let c = g.add_node();
        let z = g.add_node();
        let m = g.add_node();
        g.add_edge(a, b, 1.0, 1);
        g.add_edge(a, m, 1.0, 1);
        g.add_edge(b, c, 1.0, 1);
        g.add_edge(c, z, 1.0, 1);
        g.add_edge(m, z, 10.0, 1);

        NetworkSimplex::for_graph(&mut g).with_balancing(true).execute();

        assert_eq!(layers(&g, &[a, b, c, z, m]), vec![0, 1, 2, 3, 1]);
    }

    /// Chain a->b->c->d plus x with a->x and x->d: x has equal in- and
    /// out-degree and slack, so balancing moves it from the crowded layer 1
    /// to layer 2 (hand-traced).
    #[test]
    fn balancing_moves_node() {
        let mut g = NGraph::new();
        let a = g.add_node();
        let b = g.add_node();
        let c = g.add_node();
        let d = g.add_node();
        let x = g.add_node();
        g.add_edge(a, b, 1.0, 1);
        g.add_edge(a, x, 1.0, 1);
        g.add_edge(b, c, 1.0, 1);
        g.add_edge(c, d, 1.0, 1);
        g.add_edge(x, d, 1.0, 1);

        NetworkSimplex::for_graph(&mut g).with_balancing(true).execute();

        assert_eq!(layers(&g, &[a, b, c, d, x]), vec![0, 1, 2, 3, 2]);
    }

    /// More than REMOVE_SUBTREES_THRESH nodes: a long chain. Every node is
    /// successively removed as a leaf and re-attached; layer assignment must
    /// still be the chain positions.
    #[test]
    fn subtree_removal_on_long_chain() {
        let mut g = NGraph::new();
        let n: Vec<NNodeId> = (0..45).map(|_| g.add_node()).collect();
        for i in 0..44 {
            g.add_edge(n[i], n[i + 1], 1.0, 1);
        }

        NetworkSimplex::for_graph(&mut g).with_balancing(true).execute();

        let expected: Vec<i32> = (0..45).collect();
        assert_eq!(layers(&g, &n), expected);
    }

    #[test]
    fn previous_layering_is_considered() {
        // single node, previous layering [1, 1, 1]: normalize() adds the
        // previous counts to the filling, balancing finds no better layer.
        let mut g = NGraph::new();
        let n = g.add_node();
        NetworkSimplex::for_graph(&mut g)
            .with_previous_layering(Some(vec![1, 1, 1]))
            .with_balancing(true)
            .execute();
        assert_eq!(g.node(n).layer, 0);
    }

    #[test]
    fn is_acyclic_and_make_connected() {
        let mut g = NGraph::new();
        let a = g.add_node();
        let b = g.add_node();
        let c = g.add_node();
        g.add_edge(a, b, 1.0, 1);
        assert!(g.is_acyclic());
        // two components: {a, b} and {c} -> artificial root created
        let root = g.make_connected();
        assert!(root.is_some());
        let root = root.unwrap();
        assert_eq!(g.node(root).outgoing_edges.len(), 2);
        assert_eq!(g.edge(g.node(root).outgoing_edges[0]).target, a);
        assert_eq!(g.edge(g.node(root).outgoing_edges[1]).target, c);
        // now connected: no new root
        assert!(g.make_connected().is_none());

        let mut cyclic = NGraph::new();
        let x = cyclic.add_node();
        let y = cyclic.add_node();
        cyclic.add_edge(x, y, 1.0, 1);
        cyclic.add_edge(y, x, 1.0, 1);
        assert!(!cyclic.is_acyclic());
    }

    #[test]
    fn reverse_edge_updates_lists() {
        let mut g = NGraph::new();
        let a = g.add_node();
        let b = g.add_node();
        let e = g.add_edge(a, b, 1.0, 1);
        g.reverse_edge(e);
        assert_eq!(g.edge(e).source, b);
        assert_eq!(g.edge(e).target, a);
        assert_eq!(g.node(b).outgoing_edges, vec![e]);
        assert_eq!(g.node(a).incoming_edges, vec![e]);
        assert!(g.node(a).outgoing_edges.is_empty());
        assert!(g.node(b).incoming_edges.is_empty());
    }
}
