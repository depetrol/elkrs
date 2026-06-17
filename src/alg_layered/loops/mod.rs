//! The self loop
//! model (`SelfLoopHolder`, `SelfHyperLoop`, `SelfLoopEdge`, `SelfLoopPort`,
//! `SelfHyperLoopLabels`, `SelfLoopType`).
//!
//! The holder owns flat vectors of ports, edges and hyper loops which
//! reference each other through indices; the holder itself lives in
//! `LNode::self_loop_holder`.

pub mod ordering;
pub mod routing;

use std::collections::VecDeque;

use crate::core::options::{Direction, PortSide};
use crate::graph::math::KVector;
use crate::graph::properties::EnumSet;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LLabelId, LNodeId, LPortId, NodeType};
use crate::alg_layered::options_gen as lopts;

/// Index of a [`SelfLoopPort`] in [`SelfLoopHolder::sl_ports`].
pub type SlPortIdx = usize;
/// Index of a [`SelfLoopEdge`] in [`SelfLoopHolder::sl_edges`].
pub type SlEdgeIdx = usize;
/// Index of a [`SelfHyperLoop`] in [`SelfLoopHolder::sl_hyper_loops`].
pub type SlLoopIdx = usize;

/// Number of `PortSide` values (arrays are indexed by `PortSide` ordinal).
pub const PORT_SIDE_COUNT: usize = 5;

// ---------------------------------------------------------------------------
// SelfLoopType

/// The different types of self loops.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelfLoopType {
    /// Connects ports that are all on the same side.
    OneSide,
    /// Connects ports on two adjacent sides.
    TwoSidesCorner,
    /// Connects ports on two opposing sides.
    TwoSidesOpposing,
    /// Connects ports spread out over three sides.
    ThreeSides,
    /// Connects ports spread out over all four sides.
    FourSides,
}

impl SelfLoopType {
    /// `SelfLoopType.fromPortSides`.
    pub fn from_port_sides(port_sides: &EnumSet<PortSide>) -> Option<SelfLoopType> {
        assert!(!port_sides.contains(PortSide::UNDEFINED));
        match port_sides.len() {
            1 => Some(SelfLoopType::OneSide),
            2 => {
                // Check if we have opposing sides or not
                let east_west = port_sides.contains(PortSide::EAST)
                    && port_sides.contains(PortSide::WEST);
                let north_south = port_sides.contains(PortSide::NORTH)
                    && port_sides.contains(PortSide::SOUTH);
                if east_west || north_south {
                    Some(SelfLoopType::TwoSidesOpposing)
                } else {
                    Some(SelfLoopType::TwoSidesCorner)
                }
            }
            3 => Some(SelfLoopType::ThreeSides),
            4 => Some(SelfLoopType::FourSides),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// SelfLoopPort

/// A port which is an end point of at least one self
/// loop.
#[derive(Debug)]
pub struct SelfLoopPort {
    /// Port represented by this instance.
    pub l_port: LPortId,
    /// Whether the port was only incident to self loop edges.
    pub had_only_self_loops: bool,
    /// List of incoming self loops.
    pub incoming_sl_edges: Vec<SlEdgeIdx>,
    /// List of outgoing self loops.
    pub outgoing_sl_edges: Vec<SlEdgeIdx>,
    /// Whether the original `LPort` is currently hidden from its node.
    pub hidden: bool,
}

impl SelfLoopPort {
    /// `getSLNetFlow`: incoming self loop edges minus outgoing ones.
    pub fn sl_net_flow(&self) -> i32 {
        self.incoming_sl_edges.len() as i32 - self.outgoing_sl_edges.len() as i32
    }
}

// ---------------------------------------------------------------------------
// SelfLoopEdge

/// A single self loop edge.
#[derive(Debug)]
pub struct SelfLoopEdge {
    /// The edge represented by this instance.
    pub l_edge: LEdgeId,
    /// The self hyper loop this edge belongs to (set during initialization).
    pub sl_hyper_loop: SlLoopIdx,
    /// The edge's source port.
    pub sl_source: SlPortIdx,
    /// The edge's target port.
    pub sl_target: SlPortIdx,
}

impl SelfLoopEdge {
    /// `isInline`: whether any of the edge's labels is an inline label.
    pub fn is_inline(&self, a: &LGraphArena) -> bool {
        a.edge(self.l_edge)
            .labels
            .iter()
            .any(|&l| a.label(l).properties.get(&lopts::EDGE_LABELS_INLINE))
    }
}

// ---------------------------------------------------------------------------
// SelfHyperLoopLabels

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Alignment {
    /// A northern or southern centered label.
    Center,
    /// A northern or southern left-aligned label.
    Left,
    /// A northern or southern right-aligned label.
    Right,
    /// An eastern or western top-aligned label.
    Top,
}

/// The labels associated with a
/// [`SelfHyperLoop`].
#[derive(Debug)]
pub struct SelfHyperLoopLabels {
    /// An ID that can be used arbitrarily.
    pub id: i32,
    /// The labels represented by this instance.
    pub l_labels: Vec<LLabelId>,
    /// The size required to place all labels.
    pub size: KVector,
    /// The position of our bounding box's top left corner.
    pub position: KVector,
    /// The graph's layout direction.
    layout_direction: Direction,
    /// Space to leave between adjacent labels.
    label_label_spacing: f64,
    /// The side the label is placed on (`null` -> `UNDEFINED`).
    pub side: PortSide,
    /// The label's alignment.
    pub alignment: Option<Alignment>,
    /// The port any non-center alignment is relative to.
    pub alignment_reference_sl_port: Option<SlPortIdx>,
}

impl SelfHyperLoopLabels {
    /// Constructor: initializes properties from the graph.
    fn new(a: &LGraphArena, l_node: LNodeId) -> Self {
        let graph = a.node_graph(l_node);
        SelfHyperLoopLabels {
            id: 0,
            l_labels: Vec::new(),
            size: KVector::default(),
            position: KVector::default(),
            layout_direction: a.graph(graph).properties.get(&lopts::DIRECTION),
            label_label_spacing: get_individual_or_inherited(a, l_node, &lopts::SPACING_LABEL_LABEL),
            side: PortSide::UNDEFINED,
            alignment: None,
            alignment_reference_sl_port: None,
        }
    }

    /// `addLLabels`.
    fn add_l_labels(&mut self, a: &LGraphArena, new_l_labels: &[LLabelId]) {
        for &new_l_label in new_l_labels {
            self.l_labels.push(new_l_label);
            self.update_size(a, new_l_label);
        }
    }

    /// `updateSize`.
    fn update_size(&mut self, a: &LGraphArena, new_l_label: LLabelId) {
        let new_l_label_size = a.label(new_l_label).size;

        if self.layout_direction.is_horizontal() {
            // The labels will be stacked vertically
            self.size.x = f64::max(self.size.x, new_l_label_size.x);
            self.size.y += new_l_label_size.y;

            // Add a label-label spacing if we already had labels
            if self.l_labels.len() > 1 {
                self.size.y += self.label_label_spacing;
            }
        } else {
            // The labels will be stacked horizontally
            self.size.x += new_l_label_size.x;
            self.size.y = f64::max(self.size.y, new_l_label_size.y);

            // Add a label-label spacing if we already had labels
            if self.l_labels.len() > 1 {
                self.size.x += self.label_label_spacing;
            }
        }
    }

    /// `applyPlacement`: applies the bounding box placement to the
    /// individual labels.
    pub fn apply_placement(&self, a: &mut LGraphArena, offset: KVector) {
        if self.layout_direction.is_horizontal() {
            self.apply_placement_for_horizontal_layout(a, offset);
        } else {
            self.apply_placement_for_vertical_layout(a, offset);
        }
    }

    fn apply_placement_for_horizontal_layout(&self, a: &mut LGraphArena, offset: KVector) {
        let x = self.position.x;
        let mut y = self.position.y;

        for &l_label in &self.l_labels {
            let label_size = a.label(l_label).size;
            let label_pos = &mut a.label_mut(l_label).pos;

            // X coordinate depends on alignment and / or side
            if self.alignment == Some(Alignment::Left) || self.side == PortSide::EAST {
                label_pos.x = x;
            } else if self.alignment == Some(Alignment::Right) || self.side == PortSide::WEST {
                label_pos.x = x + self.size.x - label_size.x;
            } else {
                // Alignment is center
                label_pos.x = x + (self.size.x - label_size.x) / 2.0;
            }

            label_pos.y = y;
            label_pos.add(offset);

            y += label_size.y + self.label_label_spacing;
        }
    }

    fn apply_placement_for_vertical_layout(&self, a: &mut LGraphArena, offset: KVector) {
        let mut x = self.position.x;
        let y = self.position.y;

        for &l_label in &self.l_labels {
            let label_size = a.label(l_label).size;
            let label_pos = &mut a.label_mut(l_label).pos;

            label_pos.x = x;

            // We always top-align, except for the northern side
            if self.side == PortSide::NORTH {
                label_pos.y = y + self.size.y - label_size.y;
            } else {
                label_pos.y = y;
            }

            label_pos.add(offset);

            x += label_size.x + self.label_label_spacing;
        }
    }
}

// ---------------------------------------------------------------------------
// SelfHyperLoop

/// A self loop hyperedge consisting of at least one
/// self loop edge.
#[derive(Debug)]
pub struct SelfHyperLoop {
    /// List of ports that belong to this hyper loop.
    pub sl_ports: Vec<SlPortIdx>,
    /// Set of edges that belong to this instance. We keep deterministic
    /// insertion order (all uses are order-insensitive).
    pub sl_edges: Vec<SlEdgeIdx>,
    /// This hyper loop's labels.
    pub sl_labels: Option<SelfHyperLoopLabels>,
    /// This self loop's loop type. Determined once port sides have been assigned.
    pub self_loop_type: Option<SelfLoopType>,
    /// List of ports per port side, indexed by `PortSide` ordinal. `None`
    /// until `compute_ports_per_side` was called.
    pub sl_ports_by_side: Option<[Vec<SlPortIdx>; PORT_SIDE_COUNT]>,
    /// The multimap's key set. The key set order is unspecified; we use
    /// first-insertion order (port list order).
    pub sl_port_sides: Vec<PortSide>,
    /// The hyper loop trunk's leftmost port. Computed after initialization.
    pub leftmost_port: Option<SlPortIdx>,
    /// The hyper loop trunk's rightmost port. Computed after initialization.
    pub rightmost_port: Option<SlPortIdx>,
    /// The set of port sides this loop is routed along.
    pub occupied_port_sides: EnumSet<PortSide>,
    /// The routing slot we're assigned to on each side, by port side ordinal.
    pub routing_slot: [i32; PORT_SIDE_COUNT],
}

impl SelfHyperLoop {
    fn new() -> Self {
        SelfHyperLoop {
            sl_ports: Vec::new(),
            sl_edges: Vec::new(),
            sl_labels: None,
            self_loop_type: None,
            sl_ports_by_side: None,
            sl_port_sides: Vec::new(),
            leftmost_port: None,
            rightmost_port: None,
            occupied_port_sides: EnumSet::none(),
            routing_slot: [0; PORT_SIDE_COUNT],
        }
    }

    /// `getSLPortsBySide(PortSide)`.
    pub fn sl_ports_on_side(&self, side: PortSide) -> &[SlPortIdx] {
        &self.sl_ports_by_side.as_ref().unwrap()[side as usize]
    }

    /// `hasSLPortsOnSide`.
    pub fn has_sl_ports_on_side(&self, side: PortSide) -> bool {
        !self.sl_ports_on_side(side).is_empty()
    }

    /// `getRoutingSlot`.
    pub fn routing_slot(&self, side: PortSide) -> i32 {
        self.routing_slot[side as usize]
    }
}

// ---------------------------------------------------------------------------
// SelfLoopHolder

/// ID of a port that hasn't been visited yet.
const UNVISITED: i32 = 0;
/// ID of a port that has already been visited.
const VISITED: i32 = 1;

/// Holds all the information required to route self
/// loops of a particular node.
#[derive(Debug)]
pub struct SelfLoopHolder {
    /// The node this instance belongs to.
    pub l_node: LNodeId,
    /// List of the node's [`SelfHyperLoop`]s.
    pub sl_hyper_loops: Vec<SelfHyperLoop>,
    /// The node's [`SelfLoopPort`]s. The vector's order is insertion order
    /// for stable iteration (lookups go through
    /// [`SelfLoopHolder::sl_port_idx`]).
    pub sl_ports: Vec<SelfLoopPort>,
    /// The node's [`SelfLoopEdge`]s (owned storage).
    pub sl_edges: Vec<SelfLoopEdge>,
    /// Whether at least one self loop port is currently hidden from its node.
    pub are_ports_hidden: bool,
    /// The number of routing slots on each side, by `PortSide` ordinal.
    pub routing_slot_count: [i32; PORT_SIDE_COUNT],
}

impl SelfLoopHolder {
    /// `needsSelfLoopProcessing`: checks if the given node is a regular
    /// node and has at least one self loop.
    pub fn needs_self_loop_processing(a: &LGraphArena, l_node: LNodeId) -> bool {
        if a.node(l_node).node_type != NodeType::NORMAL {
            return false;
        }
        a.node_outgoing_edges(l_node).iter().any(|&e| a.edge_is_self_loop(e))
    }

    /// `install` (minus storing the property; the caller attaches the
    /// returned holder to `LNode::self_loop_holder`).
    pub fn create(a: &mut LGraphArena, l_node: LNodeId) -> SelfLoopHolder {
        debug_assert!(Self::needs_self_loop_processing(a, l_node));

        let mut holder = SelfLoopHolder {
            l_node,
            sl_hyper_loops: Vec::new(),
            sl_ports: Vec::new(),
            sl_edges: Vec::new(),
            are_ports_hidden: false,
            routing_slot_count: [0; PORT_SIDE_COUNT],
        };
        holder.initialize(a);
        holder
    }

    /// `initialize`: populates the data model.
    fn initialize(&mut self, a: &mut LGraphArena) {
        // Create self loop edges and ports for every self loop
        for l_edge in a.node_outgoing_edges(self.l_node) {
            if a.edge_is_self_loop(l_edge) {
                let sl_source = self.self_loop_port_for(a, a.edge(l_edge).source.unwrap());
                let sl_target = self.self_loop_port_for(a, a.edge(l_edge).target.unwrap());

                // SelfLoopEdge constructor: adds itself to the ports'
                // edge lists
                let sl_edge = self.sl_edges.len();
                self.sl_edges.push(SelfLoopEdge {
                    l_edge,
                    sl_hyper_loop: usize::MAX,
                    sl_source,
                    sl_target,
                });
                self.sl_ports[sl_source].outgoing_sl_edges.push(sl_edge);
                self.sl_ports[sl_target].incoming_sl_edges.push(sl_edge);
            }
        }

        // Reset port IDs for the BFS we're about to run
        for sl_port in &self.sl_ports {
            a.port_mut(sl_port.l_port).id = UNVISITED;
        }

        // Run BFS at every port to gather the edges into hyperloops
        for sl_port in 0..self.sl_ports.len() {
            if a.port(self.sl_ports[sl_port].l_port).id == UNVISITED {
                self.initialize_hyper_loop(a, sl_port);
            }
        }
    }

    /// `selfLoopPortFor`: returns the [`SelfLoopPort`] representation of
    /// the given port, creating one if none exists yet.
    fn self_loop_port_for(&mut self, a: &LGraphArena, l_port: LPortId) -> SlPortIdx {
        if let Some(idx) = self.sl_port_idx(l_port) {
            return idx;
        }

        // SelfLoopPort constructor: check if the port is only incident
        // to self loops
        let had_only_self_loops =
            a.port_connected_edges(l_port).iter().all(|&e| a.edge_is_self_loop(e));

        self.sl_ports.push(SelfLoopPort {
            l_port,
            had_only_self_loops,
            incoming_sl_edges: Vec::new(),
            outgoing_sl_edges: Vec::new(),
            hidden: false,
        });
        self.sl_ports.len() - 1
    }

    /// `getSLPortMap().get(lPort)`: looks up the [`SelfLoopPort`]
    /// created for the given port, if any.
    pub fn sl_port_idx(&self, l_port: LPortId) -> Option<SlPortIdx> {
        self.sl_ports.iter().position(|p| p.l_port == l_port)
    }

    /// `initializeHyperLoop`: collects all self loops reachable from the
    /// given port and merges them into a hyper loop.
    fn initialize_hyper_loop(&mut self, a: &mut LGraphArena, sl_port: SlPortIdx) {
        let sl_loop = self.sl_hyper_loops.len();
        self.sl_hyper_loops.push(SelfHyperLoop::new());

        // Run a BFS starting at the port
        let mut bfs_queue: VecDeque<SlPortIdx> = VecDeque::new();
        bfs_queue.push_back(sl_port);

        while let Some(curr_sl_port) = bfs_queue.pop_front() {
            a.port_mut(self.sl_ports[curr_sl_port].l_port).id = VISITED;

            // Add each outgoing edge to our hyper loop
            for sl_edge in self.sl_ports[curr_sl_port].outgoing_sl_edges.clone() {
                self.add_self_loop_edge(a, sl_loop, sl_edge);

                let sl_target_port = self.sl_edges[sl_edge].sl_target;
                if a.port(self.sl_ports[sl_target_port].l_port).id == UNVISITED {
                    bfs_queue.push_back(sl_target_port);
                }
            }

            // Add each incoming edge to our hyper loop
            for sl_edge in self.sl_ports[curr_sl_port].incoming_sl_edges.clone() {
                self.add_self_loop_edge(a, sl_loop, sl_edge);

                let sl_source_port = self.sl_edges[sl_edge].sl_source;
                if a.port(self.sl_ports[sl_source_port].l_port).id == UNVISITED {
                    bfs_queue.push_back(sl_source_port);
                }
            }
        }
    }

    /// `SelfHyperLoop.addSelfLoopEdge`: adds the given edge to the loop
    /// and sets everything up accordingly, unless the edge was already part
    /// of the loop.
    fn add_self_loop_edge(&mut self, a: &LGraphArena, sl_loop: SlLoopIdx, sl_edge: SlEdgeIdx) {
        if self.sl_hyper_loops[sl_loop].sl_edges.contains(&sl_edge) {
            return;
        }
        self.sl_hyper_loops[sl_loop].sl_edges.push(sl_edge);
        self.sl_edges[sl_edge].sl_hyper_loop = sl_loop;

        let sl_source = self.sl_edges[sl_edge].sl_source;
        if !self.sl_hyper_loops[sl_loop].sl_ports.contains(&sl_source) {
            self.sl_hyper_loops[sl_loop].sl_ports.push(sl_source);
        }

        let sl_target = self.sl_edges[sl_edge].sl_target;
        if !self.sl_hyper_loops[sl_loop].sl_ports.contains(&sl_target) {
            self.sl_hyper_loops[sl_loop].sl_ports.push(sl_target);
        }

        // Check if we need to take care of any edge labels
        let l_labels = a.edge(self.sl_edges[sl_edge].l_edge).labels.clone();
        if !l_labels.is_empty() {
            let l_node = self.l_node;
            let slh_loop = &mut self.sl_hyper_loops[sl_loop];
            if slh_loop.sl_labels.is_none() {
                slh_loop.sl_labels = Some(SelfHyperLoopLabels::new(a, l_node));
            }
            slh_loop.sl_labels.as_mut().unwrap().add_l_labels(a, &l_labels);
        }
    }

    /// `SelfHyperLoop.computePortsPerSide`: fills the ports-by-side
    /// multimap and determines the self loop's type.
    pub fn compute_ports_per_side(&mut self, a: &LGraphArena, sl_loop: SlLoopIdx) {
        debug_assert!(self.sl_hyper_loops[sl_loop].sl_ports_by_side.is_none());

        // Remember ports for each side
        let mut by_side: [Vec<SlPortIdx>; PORT_SIDE_COUNT] = Default::default();
        let mut key_order: Vec<PortSide> = Vec::new();
        let mut key_set: EnumSet<PortSide> = EnumSet::none();

        for &sl_port in &self.sl_hyper_loops[sl_loop].sl_ports {
            let port_side = a.port(self.sl_ports[sl_port].l_port).side;
            debug_assert!(port_side != PortSide::UNDEFINED);

            if !key_set.contains(port_side) {
                key_set.add(port_side);
                key_order.push(port_side);
            }
            by_side[port_side as usize].push(sl_port);
        }

        // Determine this self loop's loop type
        let sl = &mut self.sl_hyper_loops[sl_loop];
        sl.sl_ports_by_side = Some(by_side);
        sl.sl_port_sides = key_order;
        sl.self_loop_type = SelfLoopType::from_port_sides(&key_set);
    }

    /// `SelfHyperLoop.setRoutingSlot`: sets which routing slot the loop
    /// should occupy on the given port side; also updates the number of
    /// routing slots kept by the holder.
    pub fn set_routing_slot(&mut self, sl_loop: SlLoopIdx, port_side: PortSide, slot: i32) {
        self.sl_hyper_loops[sl_loop].routing_slot[port_side as usize] = slot;

        let count = &mut self.routing_slot_count[port_side as usize];
        *count = i32::max(*count, slot + 1);
    }
}

// ---------------------------------------------------------------------------
// Utilities

pub(crate) fn get_individual_or_inherited(
    a: &LGraphArena,
    node: LNodeId,
    property: &crate::graph::properties::Property<f64>,
) -> f64 {
    crate::alg_layered::spacings::get_individual_or_default(a, node, property).unwrap_or(0.0)
}
