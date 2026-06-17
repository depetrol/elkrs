//! Threshold calculation used by
//! the BK compactor to favor additional straight edges over compactness.
//!
//! The `NullThresholdStrategy` and `SimpleThresholdStrategy` variants are
//! folded into one struct distinguished by the `simple` flag.

use std::collections::{HashSet, VecDeque};

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LNodeId};

use super::bk_aligned_layout::{BKAlignedLayout, HDirection, VDirection};
use super::neighborhood_information::{nid, NeighborhoodInformation};

// TODO make this an option?!
const THRESHOLD: f64 = f64::MAX;

/// Represents a unit to be post-processed (`Postprocessable`).
struct Postprocessable {
    /// the node whose block can potentially be moved.
    free: LNodeId,
    /// whether `free` is the root node of its block.
    is_root: bool,
    /// whether `free` has edges.
    has_edges: bool,
    /// the edge that was selected to be straightened.
    edge: Option<LEdgeId>,
}

pub struct ThresholdStrategy {
    /// `true` selects the simple strategy, `false` the null strategy.
    simple: bool,
    /// We keep track of which blocks have been completely finished.
    block_finished: HashSet<LNodeId>,
    /// A queue with blocks that are postponed during compaction.
    postprocessables_queue: VecDeque<Postprocessable>,
    /// A stack that is used to treat postponed nodes in reversed order.
    postprocessables_stack: Vec<Postprocessable>,
}

impl ThresholdStrategy {
    pub fn new(simple: bool) -> Self {
        ThresholdStrategy {
            simple,
            block_finished: HashSet::new(),
            postprocessables_queue: VecDeque::new(),
            postprocessables_stack: Vec::new(),
        }
    }

    /// Resets the internal state.
    pub fn init(&mut self) {
        self.block_finished.clear();
        self.postprocessables_queue.clear();
        self.postprocessables_stack.clear();
    }

    /// Marks the block of which `n` is the root to be completely placed.
    pub fn finish_block(&mut self, n: LNodeId) {
        self.block_finished.insert(n);
    }

    /// Returns a threshold value representing a bound that would allow an
    /// additional edge to be drawn straight.
    pub fn calculate_threshold(
        &mut self,
        a: &LGraphArena,
        bal: &mut BKAlignedLayout,
        old_thresh: f64,
        block_root: LNodeId,
        current_node: LNodeId,
    ) -> f64 {
        if !self.simple {
            // NullThresholdStrategy: a threshold value such that it has no
            // effect. New value calculated using min(a, thresh) for UP, so
            // thresh = +infty has no effect (and vice versa for DOWN).
            return if bal.vdir == Some(VDirection::Up) {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            };
        }

        // Simple strategy: just the root or last node of a block.
        // Remember that for blocks with a single node both flags can be true.
        let is_root = block_root == current_node;
        let is_last = bal.align[nid(a, current_node)] == Some(block_root);

        if !(is_root || is_last) {
            return old_thresh;
        }

        // Remember two things:
        //  1) it is not guaranteed that adjacent nodes are already placed
        //  2) blocks can consist of a single node implying that the current
        //     node is both the root and the last node
        let mut t = old_thresh;
        if is_root {
            t = self.get_bound(a, bal, block_root, true);
        }
        if t.is_infinite() && is_last {
            t = self.get_bound(a, bal, current_node, false);
        }
        t
    }

    /// For an edge `(o, n)`, return `o`.
    fn get_other(a: &LGraphArena, edge: LEdgeId, n: LNodeId) -> LNodeId {
        if a.edge_source_node(edge) == n {
            a.edge_target_node(edge)
        } else if a.edge_target_node(edge) == n {
            a.edge_source_node(edge)
        } else {
            panic!("Node {n:?} is neither source nor target of edge {edge:?}");
        }
    }

    /// Mutates `pp`: if no valid
    /// edge was picked, `pp.edge` is `None` and `pp.has_edges` indicates if
    /// there are possible candidate edges that might become valid later.
    fn pick_edge(&self, a: &LGraphArena, bal: &BKAlignedLayout, pp: &mut Postprocessable) {
        let edges = if pp.is_root {
            if bal.hdir == Some(HDirection::Right) {
                a.node_incoming_edges(pp.free)
            } else {
                a.node_outgoing_edges(pp.free)
            }
        } else if bal.hdir == Some(HDirection::Left) {
            a.node_incoming_edges(pp.free)
        } else {
            a.node_outgoing_edges(pp.free)
        };

        let mut has_edges = false;
        for e in edges {
            // ignore in-layer edges unless the block is solely connected by
            // in-layer edges
            let only_dummies = bal.od[nid(a, bal.root[nid(a, pp.free)].unwrap())];
            if !only_dummies && a.edge_is_in_layer(e) {
                continue;
            }

            // in order to straighten 'e' the block represented by 'pp.free'
            // would have to be moved. However, since that block is already
            // part of a straightened edge, it cannot be moved again
            if bal.su[nid(a, bal.root[nid(a, pp.free)].unwrap())] {
                continue;
            }

            has_edges = true;

            // if the other node does not have a position yet, ignore this edge
            if self
                .block_finished
                .contains(&bal.root[nid(a, Self::get_other(a, e, pp.free))].unwrap())
            {
                pp.has_edges = true;
                pp.edge = Some(e);
                return;
            }
        }

        // no edge picked
        pp.has_edges = has_edges;
        pp.edge = None;
    }

    /// Only regards root and last
    /// nodes of a block.
    fn get_bound(
        &mut self,
        a: &LGraphArena,
        bal: &mut BKAlignedLayout,
        block_node: LNodeId,
        is_root: bool,
    ) -> f64 {
        let invalid = if bal.vdir == Some(VDirection::Up) {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };

        let mut pick = Postprocessable {
            free: block_node,
            is_root,
            has_edges: false,
            edge: None,
        };
        self.pick_edge(a, bal, &mut pick);

        // if edges exist but we couldn't find a good one
        if pick.edge.is_none() && pick.has_edges {
            self.postprocessables_queue.push_back(pick);
            return invalid;
        } else if let Some(edge) = pick.edge {
            let left = a.edge(edge).source.unwrap();
            let right = a.edge(edge).target.unwrap();

            // We handle the root (first) node of a block and the last node of
            // a block; they only differ in the port selection.
            let (root_port, other_port) = if is_root {
                if bal.hdir == Some(HDirection::Right) {
                    (right, left)
                } else {
                    (left, right)
                }
            } else if bal.hdir == Some(HDirection::Left) {
                (right, left)
            } else {
                (left, right)
            };

            let other_node = a.port(other_port).node.unwrap();
            let root_port_node = a.port(root_port).node.unwrap();
            let other_root = bal.root[nid(a, other_node)].unwrap();
            let threshold = bal.y[nid(a, other_root)].unwrap()
                + bal.inner_shift[nid(a, other_node)].unwrap()
                + a.port(other_port).pos.y
                + a.port(other_port).anchor.y
                // root node
                - bal.inner_shift[nid(a, root_port_node)].unwrap()
                - a.port(root_port).pos.y
                - a.port(root_port).anchor.y;

            // we are not allowed to move this block anymore
            // in order to straighten another edge
            let left_root = bal.root[nid(a, a.port(left).node.unwrap())].unwrap();
            let right_root = bal.root[nid(a, a.port(right).node.unwrap())].unwrap();
            bal.su[nid(a, left_root)] = true;
            bal.su[nid(a, right_root)] = true;

            return threshold;
        }
        invalid
    }

    /// Handle nodes that have been marked as having potential to lead to
    /// further straight edges after all blocks were initially placed.
    pub fn post_process(
        &mut self,
        a: &LGraphArena,
        ni: &NeighborhoodInformation,
        bal: &mut BKAlignedLayout,
    ) {
        if !self.simple {
            // NullThresholdStrategy: nothing to do.
            return;
        }

        // try original iteration order
        while let Some(mut pp) = self.postprocessables_queue.pop_front() {
            // first is the node, second whether it is regarded as root
            self.pick_edge(a, bal, &mut pp);

            let Some(edge) = pp.edge else {
                continue;
            };

            // ignore in-layer edges
            let only_dummies = bal.od[nid(a, bal.root[nid(a, pp.free)].unwrap())];
            if !only_dummies && a.edge_is_in_layer(edge) {
                continue;
            }

            // try to straighten the edge ...
            let moved = Self::process_postprocessable(a, ni, bal, &pp);
            // if it wasn't possible try again later in the opposite iteration
            // direction
            if !moved {
                self.postprocessables_stack.push(pp);
            }
        }

        // reversed iteration order
        while let Some(pp) = self.postprocessables_stack.pop() {
            Self::process_postprocessable(a, ni, bal, &pp);
        }
    }

    fn process_postprocessable(
        a: &LGraphArena,
        ni: &NeighborhoodInformation,
        bal: &mut BKAlignedLayout,
        pp: &Postprocessable,
    ) -> bool {
        let edge = pp.edge.unwrap();

        let (fix, block) = if a.edge_source_node(edge) == pp.free {
            (a.edge(edge).target.unwrap(), a.edge(edge).source.unwrap())
        } else {
            (a.edge(edge).source.unwrap(), a.edge(edge).target.unwrap())
        };

        // t has to be the root node of a different block
        let delta = bal.calculate_delta(a, fix, block);

        if delta > 0.0 && delta < THRESHOLD {
            // target y larger than source y --> shift upwards?
            let block_node = a.port(block).node.unwrap();
            let available_space = bal.check_space_above(a, block_node, delta, ni);
            bal.shift_block(a, block_node, -available_space);
            available_space > 0.0
        } else if delta < 0.0 && -delta < THRESHOLD {
            // direction is up, we possibly shifted some blocks too far upward
            // for an edge to be straight, so check if we can shift down again
            let block_node = a.port(block).node.unwrap();
            let available_space = bal.check_space_below(a, block_node, -delta, ni);
            bal.shift_block(a, block_node, available_space);
            available_space > 0.0
        } else {
            false
        }
    }
}
