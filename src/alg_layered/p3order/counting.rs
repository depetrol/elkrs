//!
//! The `IInitializable` pattern is flattened into explicit `init_at_*`
//! functions that are called by `GraphInfoHolder` in exactly the same
//! traversal order as `IInitializable.init`.

use std::cell::{RefCell, RefMut};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::options::PortSide;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;

// ---------------------------------------------------------------------------
// CrossMinUtil

pub fn in_north_south_east_west_order(
    a: &LGraphArena,
    node: LNodeId,
    side: PortSide,
) -> Vec<LPortId> {
    match side {
        PortSide::EAST | PortSide::NORTH => a.node_port_side_view(node, side),
        PortSide::SOUTH | PortSide::WEST => {
            let mut ports = a.node_port_side_view(node, side);
            ports.reverse();
            ports
        }
        PortSide::UNDEFINED => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// BinaryIndexedTree

/// Sorted multiset of integers in
/// `0..max_num` with O(log n) add / rank / removeAll.
pub struct BinaryIndexedTree {
    binary_sums: Vec<i32>,
    nums_per_index: Vec<i32>,
    size: i32,
    max_num: usize,
}

impl BinaryIndexedTree {
    pub fn new(max_num: usize) -> Self {
        BinaryIndexedTree {
            binary_sums: vec![0; max_num + 1],
            nums_per_index: vec![0; max_num],
            size: 0,
            max_num,
        }
    }

    /// Increment given index.
    pub fn add(&mut self, index: usize) {
        self.size += 1;
        self.nums_per_index[index] += 1;
        let mut i = (index + 1) as i64;
        while (i as usize) < self.binary_sums.len() {
            self.binary_sums[i as usize] += 1;
            i += i & -i;
        }
    }

    /// Sum of all entries before the given index (exclusive).
    pub fn rank(&self, index: usize) -> i32 {
        let mut i = index as i64;
        let mut sum = 0;
        while i > 0 {
            sum += self.binary_sums[i as usize];
            i -= i & -i;
        }
        sum
    }

    pub fn size(&self) -> i32 {
        self.size
    }

    /// Remove all entries for one index.
    pub fn remove_all(&mut self, index: usize) {
        let num_entries = self.nums_per_index[index];
        if num_entries == 0 {
            return;
        }
        self.nums_per_index[index] = 0;
        self.size -= num_entries;
        let mut i = (index + 1) as i64;
        while (i as usize) < self.binary_sums.len() {
            self.binary_sums[i as usize] -= num_entries;
            i += i & -i;
        }
    }

    /// Clears contents of tree.
    pub fn clear(&mut self) {
        self.binary_sums = vec![0; self.max_num + 1];
        self.nums_per_index = vec![0; self.max_num];
        self.size = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

// ---------------------------------------------------------------------------
// CrossingsCounter

const INDEXING_SIDE: PortSide = PortSide::WEST;
const STACK_SIDE: PortSide = PortSide::EAST;

/// Port positions are tracked in an array
/// indexed by the `LPort.id` scratch field (`portPositions[port.id]`; the
/// ids are assigned 0..nPorts-1 per graph by the initialization traversal).
/// The array can be shared between several counters (the same `int[]` is
/// passed to multiple counters, e.g. in the greedy switch `SwitchDecider`).
pub struct CrossingsCounter {
    port_positions: Rc<RefCell<Vec<i32>>>,
    index_tree: Option<BinaryIndexedTree>,
    ends: Vec<i32>,
    node_cardinalities: Vec<i32>,
}

impl CrossingsCounter {
    pub fn new(port_positions: Vec<i32>) -> Self {
        Self::new_shared(Rc::new(RefCell::new(port_positions)))
    }

    /// `new CrossingsCounter(int[] portPositions)` with a shared array.
    pub fn new_shared(port_positions: Rc<RefCell<Vec<i32>>>) -> Self {
        CrossingsCounter {
            port_positions,
            index_tree: None,
            ends: Vec::new(),
            node_cardinalities: Vec::new(),
        }
    }

    /// Mutable access to the shared port position array (used by the
    /// hyperedge crossings counter, which shares it).
    pub fn port_positions_mut(&mut self) -> RefMut<'_, Vec<i32>> {
        self.port_positions.borrow_mut()
    }

    // -------------------------------------------------------------- public

    /// Count in-layer and between-layer crossings between the two given layers.
    pub fn count_crossings_between_layers(
        &mut self,
        a: &LGraphArena,
        left_layer_nodes: &[LNodeId],
        right_layer_nodes: &[LNodeId],
    ) -> i32 {
        let ports = self.init_port_positions_counter_clockwise(a, left_layer_nodes, right_layer_nodes);
        self.index_tree = Some(BinaryIndexedTree::new(ports.len()));
        self.count_crossings_on_ports(a, &ports)
    }

    /// Only count in-layer crossings on the given side.
    pub fn count_in_layer_crossings_on_side(
        &mut self,
        a: &LGraphArena,
        nodes: &[LNodeId],
        side: PortSide,
    ) -> i32 {
        let ports = self.init_port_positions_for_in_layer_crossings(a, nodes, side);
        self.count_in_layer_crossings_on_ports(a, &ports)
    }

    /// Count crossings between edges connected to north/south ports of the
    /// passed layer's nodes.
    pub fn count_north_south_port_crossings_in_layer(
        &mut self,
        a: &LGraphArena,
        layer: &[LNodeId],
    ) -> i32 {
        let ports = self.init_positions_for_north_south_counting(a, layer);
        self.index_tree = Some(BinaryIndexedTree::new(ports.len()));
        self.count_north_south_crossings_on_ports(a, &ports)
    }

    pub fn count_crossings_between_ports_in_both_orders(
        &mut self,
        a: &LGraphArena,
        upper_port: LPortId,
        lower_port: LPortId,
    ) -> (i32, i32) {
        let mut ports = self.connected_ports_sorted_by_position(a, upper_port, lower_port);
        let upper_lower_crossings = self.count_crossings_on_ports(a, &ports);
        // Since we might add end positions of ports which are not in the ports
        // list, we need to explicitly clear the index tree.
        self.index_tree.as_mut().unwrap().clear();
        self.switch_ports(a, upper_port, lower_port);
        ports.sort_by(|&x, &y| self.position_of(a, x).cmp(&self.position_of(a, y)));
        let lower_upper_crossings = self.count_crossings_on_ports(a, &ports);
        self.index_tree.as_mut().unwrap().clear();
        self.switch_ports(a, lower_port, upper_port);
        (upper_lower_crossings, lower_upper_crossings)
    }

    pub fn count_in_layer_crossings_between_nodes_in_both_orders(
        &mut self,
        a: &LGraphArena,
        upper_node: LNodeId,
        lower_node: LNodeId,
        side: PortSide,
    ) -> (i32, i32) {
        let mut ports =
            self.connected_in_layer_ports_sorted_by_position(a, upper_node, lower_node, side);
        let upper_lower_crossings = self.count_in_layer_crossings_on_ports(a, &ports);
        self.switch_nodes(a, upper_node, lower_node, side);
        self.index_tree.as_mut().unwrap().clear();
        ports.sort_by(|&x, &y| self.position_of(a, x).cmp(&self.position_of(a, y)));
        let lower_upper_crossings = self.count_in_layer_crossings_on_ports(a, &ports);
        self.switch_nodes(a, lower_node, upper_node, side);
        self.index_tree.as_mut().unwrap().clear();
        (upper_lower_crossings, lower_upper_crossings)
    }

    pub fn init_for_counting_between(
        &mut self,
        a: &LGraphArena,
        left_layer_nodes: &[LNodeId],
        right_layer_nodes: &[LNodeId],
    ) {
        let ports = self.init_port_positions_counter_clockwise(a, left_layer_nodes, right_layer_nodes);
        self.index_tree = Some(BinaryIndexedTree::new(ports.len()));
    }

    pub fn init_port_positions_for_in_layer_crossings(
        &mut self,
        a: &LGraphArena,
        nodes: &[LNodeId],
        side: PortSide,
    ) -> Vec<LPortId> {
        let mut ports = Vec::new();
        self.init_positions(a, nodes, &mut ports, side, true, true);
        self.index_tree = Some(BinaryIndexedTree::new(ports.len()));
        ports
    }

    /// Notify counter of port switch.
    pub fn switch_ports(&mut self, a: &LGraphArena, top_port: LPortId, bottom_port: LPortId) {
        let top_id = a.port(top_port).id as usize;
        let bottom_id = a.port(bottom_port).id as usize;
        let mut pp = self.port_positions.borrow_mut();
        let top_port_pos = pp[top_id];
        pp[top_id] = pp[bottom_id];
        pp[bottom_id] = top_port_pos;
    }

    /// Notify counter of a node switch (was-upper / was-lower).
    pub fn switch_nodes(
        &mut self,
        a: &LGraphArena,
        was_upper_node: LNodeId,
        was_lower_node: LNodeId,
        side: PortSide,
    ) {
        let ports = in_north_south_east_west_order(a, was_upper_node, side);
        for port in ports {
            let new_pos = self.position_of(a, port)
                + self.node_cardinalities[a.node(was_lower_node).id as usize];
            self.port_positions.borrow_mut()[a.port(port).id as usize] = new_pos;
        }

        let ports = in_north_south_east_west_order(a, was_lower_node, side);
        for port in ports {
            let new_pos = self.position_of(a, port)
                - self.node_cardinalities[a.node(was_upper_node).id as usize];
            self.port_positions.borrow_mut()[a.port(port).id as usize] = new_pos;
        }
    }

    // ------------------------------------------------------------- private

    fn connected_in_layer_ports_sorted_by_position(
        &self,
        a: &LGraphArena,
        upper_node: LNodeId,
        lower_node: LNodeId,
        side: PortSide,
    ) -> Vec<LPortId> {
        // Ordered by port position; ports with equal positions are
        // deduplicated (positions are unique per port here).
        let mut ports: BTreeMap<i32, LPortId> = BTreeMap::new();
        for node in [upper_node, lower_node] {
            for port in in_north_south_east_west_order(a, node, side) {
                for edge in a.port_connected_edges(port) {
                    if !a.edge_is_self_loop(edge) {
                        ports.entry(self.position_of(a, port)).or_insert(port);
                        if self.is_in_layer(a, edge) {
                            let other = self.other_end_of(a, edge, port);
                            ports.entry(self.position_of(a, other)).or_insert(other);
                        }
                    }
                }
            }
        }
        ports.into_values().collect()
    }

    fn connected_ports_sorted_by_position(
        &self,
        a: &LGraphArena,
        upper_port: LPortId,
        lower_port: LPortId,
    ) -> Vec<LPortId> {
        let mut ports: BTreeMap<i32, LPortId> = BTreeMap::new();
        for port in [upper_port, lower_port] {
            ports.entry(self.position_of(a, port)).or_insert(port);
            for edge in a.port_connected_edges(port) {
                if !self.is_port_self_loop(a, edge) {
                    let other = self.other_end_of(a, edge, port);
                    ports.entry(self.position_of(a, other)).or_insert(other);
                }
            }
        }
        ports.into_values().collect()
    }

    fn count_crossings_on_ports(&mut self, a: &LGraphArena, ports: &[LPortId]) -> i32 {
        let mut crossings = 0;
        for &port in ports {
            let port_pos = self.position_of(a, port);
            let index_tree = self.index_tree.as_mut().unwrap();
            index_tree.remove_all(port_pos as usize);
            // First get crossings for all edges.
            for edge in a.port_connected_edges(port) {
                let end_position = self.position_of(a, self.other_end_of(a, edge, port));
                if end_position > port_pos {
                    crossings += self.index_tree.as_ref().unwrap().rank(end_position as usize);
                    self.ends.push(end_position);
                }
            }
            // Then add end points.
            while let Some(end) = self.ends.pop() {
                self.index_tree.as_mut().unwrap().add(end as usize);
            }
        }
        crossings
    }

    fn count_in_layer_crossings_on_ports(&mut self, a: &LGraphArena, ports: &[LPortId]) -> i32 {
        let mut crossings = 0;
        for &port in ports {
            let port_pos = self.position_of(a, port);
            self.index_tree.as_mut().unwrap().remove_all(port_pos as usize);
            let mut num_between_layer_edges = 0;
            // First get crossings for all edges.
            for edge in a.port_connected_edges(port) {
                if self.is_in_layer(a, edge) {
                    let end_position = self.position_of(a, self.other_end_of(a, edge, port));
                    if end_position > port_pos {
                        crossings +=
                            self.index_tree.as_ref().unwrap().rank(end_position as usize);
                        self.ends.push(end_position);
                    }
                } else {
                    num_between_layer_edges += 1;
                }
            }
            crossings += self.index_tree.as_ref().unwrap().size() * num_between_layer_edges;
            // Then add end points.
            while let Some(end) = self.ends.pop() {
                self.index_tree.as_mut().unwrap().add(end as usize);
            }
        }
        crossings
    }

    fn count_north_south_crossings_on_ports(&mut self, a: &LGraphArena, ports: &[LPortId]) -> i32 {
        let mut crossings = 0;
        let mut targets_and_degrees: Vec<(LPortId, i32)> = Vec::new();

        for &port in ports {
            let port_pos = self.position_of(a, port);
            self.index_tree.as_mut().unwrap().remove_all(port_pos as usize);
            targets_and_degrees.clear();

            // collect the edges that are incident to the port
            let node = a.port(port).node.unwrap();
            match a.node(node).node_type {
                NodeType::NORMAL => {
                    let dummy: LNodeId = a
                        .port(port)
                        .properties
                        .try_get(&iprops::PORT_DUMMY)
                        .expect("port dummy expected"); // guarded in init_positions_for_north_south_counting
                    for &p in &a.node(dummy).ports {
                        targets_and_degrees.push((p, a.port_degree(p) as i32));
                    }
                }
                NodeType::LONG_EDGE => {
                    if let Some(&p) =
                        a.node(node).ports.iter().find(|&&p| p != port)
                    {
                        targets_and_degrees.push((p, a.port_degree(p) as i32));
                    }
                }
                NodeType::NORTH_SOUTH_PORT => {
                    let dummy_port = match a.port(port).properties.try_get(&iprops::ORIGIN) {
                        Some(Origin::LPort(p)) => p,
                        other => panic!("expected LPort origin on north/south dummy port, got {other:?}"),
                    };
                    targets_and_degrees.push((dummy_port, a.port_degree(port) as i32));
                }
                _ => {}
            }

            // First get crossings for all edges.
            for &(target, degree) in &targets_and_degrees {
                let end_position = self.position_of(a, target);
                if end_position > port_pos {
                    crossings +=
                        self.index_tree.as_ref().unwrap().rank(end_position as usize) * degree;
                    self.ends.push(end_position);
                }
            }

            // Then add end points.
            while let Some(end) = self.ends.pop() {
                self.index_tree.as_mut().unwrap().add(end as usize);
            }
        }

        crossings
    }

    fn init_positions(
        &mut self,
        a: &LGraphArena,
        nodes: &[LNodeId],
        ports: &mut Vec<LPortId>,
        side: PortSide,
        top_down: bool,
        get_cardinalities: bool,
    ) {
        let mut num_ports = ports.len() as i32;
        if get_cardinalities {
            self.node_cardinalities = vec![0; nodes.len()];
        }
        let mut i = Self::start(nodes, top_down);
        while Self::end(i, top_down, nodes) {
            let node = nodes[i as usize];
            let node_ports = self.get_ports(a, node, side, top_down);
            if get_cardinalities {
                self.node_cardinalities[a.node(node).id as usize] = node_ports.len() as i32;
            }
            for &port in &node_ports {
                self.port_positions.borrow_mut()[a.port(port).id as usize] = num_ports;
                num_ports += 1;
            }
            ports.extend(node_ports);
            i += Self::step(top_down);
        }
    }

    fn init_port_positions_counter_clockwise(
        &mut self,
        a: &LGraphArena,
        left_layer_nodes: &[LNodeId],
        right_layer_nodes: &[LNodeId],
    ) -> Vec<LPortId> {
        let mut ports = Vec::new();
        self.init_positions(a, left_layer_nodes, &mut ports, PortSide::EAST, true, false);
        self.init_positions(a, right_layer_nodes, &mut ports, PortSide::WEST, false, false);
        ports
    }

    fn init_positions_for_north_south_counting(
        &mut self,
        a: &LGraphArena,
        nodes: &[LNodeId],
    ) -> Vec<LPortId> {
        let mut ports: Vec<LPortId> = Vec::new();
        let mut stack: Vec<LNodeId> = Vec::new();

        let mut last_layout_unit: Option<LNodeId> = None;
        let mut index = 0;
        for &current in nodes {
            if self.is_layout_unit_changed(a, last_layout_unit, current) {
                // work the stack (filled with southern dummies)
                index = self.empty_stack(a, &mut stack, &mut ports, STACK_SIDE, index);
            }
            if let Some(unit) =
                a.node(current).properties.try_get(&iprops::IN_LAYER_LAYOUT_UNIT)
            {
                last_layout_unit = Some(unit);
            }

            match a.node(current).node_type {
                // what we consider normal
                NodeType::NORMAL => {
                    // index the northern ports west-to-east
                    for p in self.get_north_south_ports_with_incident_edges(a, current, PortSide::NORTH) {
                        self.port_positions.borrow_mut()[a.port(p).id as usize] = index;
                        index += 1;
                        ports.push(p);
                    }

                    // work the stack (filled with northern dummies)
                    index = self.empty_stack(a, &mut stack, &mut ports, STACK_SIDE, index);

                    // index the southern ports in regular clock-wise order
                    for p in self.get_north_south_ports_with_incident_edges(a, current, PortSide::SOUTH) {
                        self.port_positions.borrow_mut()[a.port(p).id as usize] = index;
                        index += 1;
                        ports.push(p);
                    }
                }

                NodeType::NORTH_SOUTH_PORT => {
                    let indexing_view = a.node_port_side_view(current, INDEXING_SIDE);
                    if !indexing_view.is_empty() {
                        // should be only one
                        let p = indexing_view[0];
                        self.port_positions.borrow_mut()[a.port(p).id as usize] = index;
                        index += 1;
                        ports.push(p);
                    }
                    if !a.node_port_side_view(current, STACK_SIDE).is_empty() {
                        stack.push(current);
                    }
                }

                NodeType::LONG_EDGE => {
                    for p in a.node_port_side_view(current, PortSide::WEST) {
                        self.port_positions.borrow_mut()[a.port(p).id as usize] = index;
                        index += 1;
                        ports.push(p);
                    }
                    for _p in a.node_port_side_view(current, PortSide::EAST) {
                        stack.push(current);
                    }
                }

                _ => {} // nothing to do here
            }
        }

        // are there any southern dummy nodes left on the stack?
        self.empty_stack(a, &mut stack, &mut ports, STACK_SIDE, index);

        ports
    }

    fn empty_stack(
        &mut self,
        a: &LGraphArena,
        stack: &mut Vec<LNodeId>,
        ports: &mut Vec<LPortId>,
        side: PortSide,
        start_index: i32,
    ) -> i32 {
        let mut index = start_index;
        while let Some(dummy) = stack.pop() {
            // dummy is either a north/south port dummy or a long edge dummy
            // both of which have only a single port on the west and/or east side
            let p = a.node_port_side_view(dummy, side)[0];
            self.port_positions.borrow_mut()[a.port(p).id as usize] = index;
            index += 1;
            ports.push(p);
        }
        index
    }

    // --------------------------------------------------------- convenience

    fn get_ports(&self, a: &LGraphArena, node: LNodeId, side: PortSide, top_down: bool) -> Vec<LPortId> {
        if side == PortSide::EAST {
            if top_down {
                a.node_port_side_view(node, side)
            } else {
                let mut v = a.node_port_side_view(node, side);
                v.reverse();
                v
            }
        } else if top_down {
            let mut v = a.node_port_side_view(node, side);
            v.reverse();
            v
        } else {
            a.node_port_side_view(node, side)
        }
    }

    fn get_north_south_ports_with_incident_edges(
        &self,
        a: &LGraphArena,
        node: LNodeId,
        side: PortSide,
    ) -> Vec<LPortId> {
        a.node_port_side_view(node, side)
            .into_iter()
            .filter(|&p| a.port(p).properties.has(&iprops::PORT_DUMMY))
            .collect()
    }

    fn start(nodes: &[LNodeId], top_down: bool) -> i32 {
        if top_down {
            0
        } else {
            nodes.len() as i32 - 1
        }
    }

    fn end(i: i32, top_down: bool, nodes: &[LNodeId]) -> bool {
        if top_down {
            i < nodes.len() as i32
        } else {
            i >= 0
        }
    }

    fn step(top_down: bool) -> i32 {
        if top_down {
            1
        } else {
            -1
        }
    }

    fn is_in_layer(&self, a: &LGraphArena, edge: LEdgeId) -> bool {
        let source_layer = a.node(a.edge_source_node(edge)).layer;
        let target_layer = a.node(a.edge_target_node(edge)).layer;
        source_layer == target_layer
    }

    fn position_of(&self, a: &LGraphArena, port: LPortId) -> i32 {
        self.port_positions.borrow()[a.port(port).id as usize]
    }

    fn other_end_of(&self, a: &LGraphArena, edge: LEdgeId, from_port: LPortId) -> LPortId {
        let e = a.edge(edge);
        if from_port == e.source.unwrap() {
            e.target.unwrap()
        } else {
            e.source.unwrap()
        }
    }

    fn is_port_self_loop(&self, a: &LGraphArena, edge: LEdgeId) -> bool {
        a.edge(edge).source == a.edge(edge).target
    }

    fn is_layout_unit_changed(
        &self,
        a: &LGraphArena,
        last_unit: Option<LNodeId>,
        node: LNodeId,
    ) -> bool {
        let last_unit = match last_unit {
            None => return false,
            Some(l) => l,
        };
        if last_unit == node || !a.node(node).properties.has(&iprops::IN_LAYER_LAYOUT_UNIT) {
            return false;
        }
        let unit: LNodeId = a.node(node).properties.try_get(&iprops::IN_LAYER_LAYOUT_UNIT).unwrap();
        unit != last_unit
    }
}

// ---------------------------------------------------------------------------
// HyperedgeCrossingsCounter

///
/// NOTE on fidelity: `Hyperedge.compareTo` and
/// `HyperedgeCorner.compareTo` fall back to `hashCode()` differences as a
/// tiebreaker, which is JVM-nondeterministic. We use the (deterministic)
/// hyperedge creation order instead, which is one valid instance of that
/// nondeterministic behavior.
struct Hyperedge {
    /// creation index, replaces the identity hash code as a tiebreaker
    id: usize,
    edges: Vec<LEdgeId>,
    ports: Vec<LPortId>,
    upper_left: i32,
    lower_left: i32,
    upper_right: i32,
    lower_right: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CornerType {
    Upper,
    Lower,
}

struct HyperedgeCorner {
    hyperedge: usize,
    position: i32,
    opposite_position: i32,
    corner_type: CornerType,
}

impl HyperedgeCorner {
    fn cmp(&self, other: &HyperedgeCorner) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        if self.position < other.position {
            Ordering::Less
        } else if self.position > other.position {
            Ordering::Greater
        } else if self.opposite_position < other.opposite_position {
            Ordering::Less
        } else if self.opposite_position > other.opposite_position {
            Ordering::Greater
        } else if self.hyperedge != other.hyperedge {
            self.hyperedge.cmp(&other.hyperedge)
        } else if self.corner_type == CornerType::Upper && other.corner_type == CornerType::Lower {
            Ordering::Less
        } else if self.corner_type == CornerType::Lower && other.corner_type == CornerType::Upper {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

/// Special crossings counting method for hyperedges (Spönemann et al.).
pub fn count_hyperedge_crossings(
    a: &LGraphArena,
    port_pos: &mut [i32],
    left_layer: &[LNodeId],
    right_layer: &[LNodeId],
) -> i32 {
    // Assign index values to the ports of the left layer
    let mut source_count = 0;
    for &node in left_layer {
        // Assign index values in the order north - east - south - west
        for &port in &a.node(node).ports {
            let mut port_edges = 0;
            for &edge in &a.port(port).outgoing_edges {
                if a.node(node).layer != a.node(a.edge_target_node(edge)).layer {
                    port_edges += 1;
                }
            }
            if port_edges > 0 {
                port_pos[a.port(port).id as usize] = source_count;
                source_count += 1;
            }
        }
    }

    // Assign index values to the ports of the right layer
    let mut target_count = 0;
    for &node in right_layer {
        // Determine how many input ports there are on the north side
        // (note that the standard port order is north - east - south - west)
        let mut north_input_ports = 0;
        for &port in &a.node(node).ports {
            if a.port(port).side == PortSide::NORTH {
                for &edge in &a.port(port).incoming_edges {
                    if a.node(node).layer != a.node(a.edge_source_node(edge)).layer {
                        north_input_ports += 1;
                        break;
                    }
                }
            } else {
                break;
            }
        }
        // Assign index values in the order north - west - south - east
        let mut other_input_ports = 0;
        for &port in a.node(node).ports.iter().rev() {
            let mut port_edges = 0;
            for &edge in &a.port(port).incoming_edges {
                if a.node(node).layer != a.node(a.edge_source_node(edge)).layer {
                    port_edges += 1;
                }
            }
            if port_edges > 0 {
                if a.port(port).side == PortSide::NORTH {
                    port_pos[a.port(port).id as usize] = target_count;
                    target_count += 1;
                } else {
                    port_pos[a.port(port).id as usize] = target_count + north_input_ports + other_input_ports;
                    other_input_ports += 1;
                }
            }
        }
        target_count += other_input_ports;
    }

    // Gather hyperedges
    let mut hyperedges: Vec<Hyperedge> = Vec::new();
    let mut port2hyperedge: HashMap<LPortId, usize> = HashMap::new();
    // insertion-ordered set of live hyperedge ids
    let mut hyperedge_set: Vec<usize> = Vec::new();
    for &node in left_layer {
        for &source_port in &a.node(node).ports {
            for &edge in &a.port(source_port).outgoing_edges {
                let target_port = a.edge(edge).target.unwrap();
                if a.node(node).layer != a.node(a.port(target_port).node.unwrap()).layer {
                    let source_he = port2hyperedge.get(&source_port).copied();
                    let target_he = port2hyperedge.get(&target_port).copied();
                    match (source_he, target_he) {
                        (None, None) => {
                            let id = hyperedges.len();
                            hyperedges.push(Hyperedge {
                                id,
                                edges: vec![edge],
                                ports: vec![source_port, target_port],
                                upper_left: 0,
                                lower_left: 0,
                                upper_right: 0,
                                lower_right: 0,
                            });
                            hyperedge_set.push(id);
                            port2hyperedge.insert(source_port, id);
                            port2hyperedge.insert(target_port, id);
                        }
                        (None, Some(t)) => {
                            hyperedges[t].edges.push(edge);
                            hyperedges[t].ports.push(source_port);
                            port2hyperedge.insert(source_port, t);
                        }
                        (Some(s), None) => {
                            hyperedges[s].edges.push(edge);
                            hyperedges[s].ports.push(target_port);
                            port2hyperedge.insert(target_port, s);
                        }
                        (Some(s), Some(t)) if s == t => {
                            hyperedges[s].edges.push(edge);
                        }
                        (Some(s), Some(t)) => {
                            hyperedges[s].edges.push(edge);
                            let target_ports = hyperedges[t].ports.clone();
                            for p in &target_ports {
                                port2hyperedge.insert(*p, s);
                            }
                            let target_edges = hyperedges[t].edges.clone();
                            hyperedges[s].edges.extend(target_edges);
                            hyperedges[s].ports.extend(target_ports);
                            hyperedge_set.retain(|&h| h != t);
                        }
                    }
                }
            }
        }
    }

    // Determine top and bottom positions for each hyperedge
    let left_layer_ref = a.node(left_layer[0]).layer;
    let right_layer_ref = a.node(right_layer[0]).layer;
    for &he in &hyperedge_set {
        hyperedges[he].upper_left = source_count;
        hyperedges[he].upper_right = target_count;
        let he_ports = hyperedges[he].ports.clone();
        for port in he_ports {
            let pos = port_pos[a.port(port).id as usize];
            let port_layer = a.node(a.port(port).node.unwrap()).layer;
            if port_layer == left_layer_ref {
                if pos < hyperedges[he].upper_left {
                    hyperedges[he].upper_left = pos;
                }
                if pos > hyperedges[he].lower_left {
                    hyperedges[he].lower_left = pos;
                }
            } else if port_layer == right_layer_ref {
                if pos < hyperedges[he].upper_right {
                    hyperedges[he].upper_right = pos;
                }
                if pos > hyperedges[he].lower_right {
                    hyperedges[he].lower_right = pos;
                }
            }
        }
    }

    // Determine the sequence of edge target positions sorted by source and
    // target index
    let mut sorted: Vec<usize> = hyperedge_set.clone();
    sorted.sort_by(|&x, &y| {
        let hx = &hyperedges[x];
        let hy = &hyperedges[y];
        hx.upper_left
            .cmp(&hy.upper_left)
            .then(hx.upper_right.cmp(&hy.upper_right))
            .then(hx.id.cmp(&hy.id))
    });
    let mut south_sequence = vec![0i32; sorted.len()];
    let mut compress_deltas = vec![0i32; target_count as usize + 1];
    for (i, &he) in sorted.iter().enumerate() {
        south_sequence[i] = hyperedges[he].upper_right;
        compress_deltas[south_sequence[i] as usize] = 1;
    }
    let mut delta = 0;
    for cd in compress_deltas.iter_mut() {
        if *cd == 1 {
            *cd = delta;
        } else {
            delta -= 1;
        }
    }
    let mut q = 0;
    for s in south_sequence.iter_mut() {
        *s += compress_deltas[*s as usize];
        q = q.max(*s + 1);
    }

    // Build the accumulator tree
    let mut first_index: i32 = 1;
    while first_index < q {
        first_index *= 2;
    }
    let tree_size = (2 * first_index - 1) as usize;
    first_index -= 1;
    let mut tree = vec![0i32; tree_size];

    // Count the straight-line crossings of the topmost edges
    let mut crossings = 0;
    for &s in &south_sequence {
        let mut index = s + first_index;
        tree[index as usize] += 1;
        while index > 0 {
            if index % 2 > 0 {
                crossings += tree[(index + 1) as usize];
            }
            index = (index - 1) / 2;
            tree[index as usize] += 1;
        }
    }

    // Create corners for the left side
    let mut left_corners: Vec<HyperedgeCorner> = Vec::with_capacity(sorted.len() * 2);
    for &he in &sorted {
        left_corners.push(HyperedgeCorner {
            hyperedge: hyperedges[he].id,
            position: hyperedges[he].upper_left,
            opposite_position: hyperedges[he].lower_left,
            corner_type: CornerType::Upper,
        });
        left_corners.push(HyperedgeCorner {
            hyperedge: hyperedges[he].id,
            position: hyperedges[he].lower_left,
            opposite_position: hyperedges[he].upper_left,
            corner_type: CornerType::Lower,
        });
    }
    left_corners.sort_by(|x, y| x.cmp(y));

    // Count crossings caused by overlapping hyperedge areas on the left side
    let mut open_hyperedges = 0;
    for corner in &left_corners {
        match corner.corner_type {
            CornerType::Upper => open_hyperedges += 1,
            CornerType::Lower => {
                open_hyperedges -= 1;
                crossings += open_hyperedges;
            }
        }
    }

    // Create corners for the right side
    let mut right_corners: Vec<HyperedgeCorner> = Vec::with_capacity(sorted.len() * 2);
    for &he in &sorted {
        right_corners.push(HyperedgeCorner {
            hyperedge: hyperedges[he].id,
            position: hyperedges[he].upper_right,
            opposite_position: hyperedges[he].lower_right,
            corner_type: CornerType::Upper,
        });
        right_corners.push(HyperedgeCorner {
            hyperedge: hyperedges[he].id,
            position: hyperedges[he].lower_right,
            opposite_position: hyperedges[he].upper_right,
            corner_type: CornerType::Lower,
        });
    }
    right_corners.sort_by(|x, y| x.cmp(y));

    // Count crossings caused by overlapping hyperedge areas on the right side
    let mut open_hyperedges = 0;
    for corner in &right_corners {
        match corner.corner_type {
            CornerType::Upper => open_hyperedges += 1,
            CornerType::Lower => {
                open_hyperedges -= 1;
                crossings += open_hyperedges;
            }
        }
    }

    crossings
}

// ---------------------------------------------------------------------------
// AllCrossingsCounter

pub struct AllCrossingsCounter {
    crossing_counter: Option<CrossingsCounter>,
    has_hyperedges_east_of_index: Vec<bool>,
    has_north_south_ports: Vec<bool>,
    in_layer_edge_counts: Vec<i32>,
    n_ports: i32,
}

impl AllCrossingsCounter {
    pub fn new(num_layers: usize) -> Self {
        AllCrossingsCounter {
            crossing_counter: None,
            has_hyperedges_east_of_index: vec![false; num_layers],
            has_north_south_ports: vec![false; num_layers],
            in_layer_edge_counts: vec![0; num_layers],
            n_ports: 0,
        }
    }

    /// Count all crossings.
    pub fn count_all_crossings(&mut self, a: &LGraphArena, current_order: &[Vec<LNodeId>]) -> i32 {
        if current_order.is_empty() {
            return 0;
        }
        let counter = self.crossing_counter.as_mut().unwrap();
        let mut crossings =
            counter.count_in_layer_crossings_on_side(a, &current_order[0], PortSide::WEST);
        crossings += counter.count_in_layer_crossings_on_side(
            a,
            &current_order[current_order.len() - 1],
            PortSide::EAST,
        );
        for layer_index in 0..current_order.len() {
            crossings += self.count_crossings_at(a, layer_index, current_order);
        }
        crossings
    }

    fn count_crossings_at(
        &mut self,
        a: &LGraphArena,
        layer_index: usize,
        current_order: &[Vec<LNodeId>],
    ) -> i32 {
        let mut total_crossings = 0;
        let left_layer = &current_order[layer_index];
        if layer_index < current_order.len() - 1 {
            let right_layer = &current_order[layer_index + 1];
            if self.has_hyperedges_east_of_index[layer_index] {
                let counter = self.crossing_counter.as_mut().unwrap();
                total_crossings = {
                    let mut port_pos = counter.port_positions_mut();
                    count_hyperedge_crossings(a, &mut port_pos, left_layer, right_layer)
                };
                total_crossings +=
                    counter.count_in_layer_crossings_on_side(a, left_layer, PortSide::EAST);
                total_crossings +=
                    counter.count_in_layer_crossings_on_side(a, right_layer, PortSide::WEST);
            } else {
                total_crossings = self
                    .crossing_counter
                    .as_mut()
                    .unwrap()
                    .count_crossings_between_layers(a, left_layer, right_layer);
            }
        }

        if self.has_north_south_ports[layer_index] {
            total_crossings += self
                .crossing_counter
                .as_mut()
                .unwrap()
                .count_north_south_port_crossings_in_layer(a, left_layer);
        }

        total_crossings
    }

    // ---------------------------------------------------- initialization

    pub fn init_at_node_level(&mut self, a: &LGraphArena, l: usize, n: usize, node_order: &[Vec<LNodeId>]) {
        let node = node_order[l][n];
        self.has_north_south_ports[l] |= a.node(node).node_type == NodeType::NORTH_SOUTH_PORT;
    }

    pub fn init_at_port_level(
        &mut self,
        a: &mut LGraphArena,
        l: usize,
        n: usize,
        p: usize,
        node_order: &[Vec<LNodeId>],
    ) {
        let port = a.node(node_order[l][n]).ports[p];
        a.port_mut(port).id = self.n_ports;
        self.n_ports += 1;
        if a.port(port).outgoing_edges.len() + a.port(port).incoming_edges.len() > 1 {
            if a.port(port).side == PortSide::EAST {
                self.has_hyperedges_east_of_index[l] = true;
            } else if a.port(port).side == PortSide::WEST && l > 0 {
                self.has_hyperedges_east_of_index[l - 1] = true;
            }
        }
    }

    pub fn init_at_edge_level(
        &mut self,
        a: &LGraphArena,
        l: usize,
        n: usize,
        p: usize,
        _e: usize,
        edge: LEdgeId,
        node_order: &[Vec<LNodeId>],
    ) {
        let port = a.node(node_order[l][n]).ports[p];
        if a.edge(edge).source == Some(port)
            && a.node(a.edge_source_node(edge)).layer == a.node(a.edge_target_node(edge)).layer
        {
            self.in_layer_edge_counts[l] += 1;
        }
    }

    pub fn init_after_traversal(&mut self) {
        let port_pos = vec![0; self.n_ports as usize];
        // The hyperedge counter is a function that borrows this array on
        // demand instead of holding its own reference.
        self.crossing_counter = Some(CrossingsCounter::new(port_pos));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alg_layered::graph::LGraphArena;

    #[test]
    fn binary_indexed_tree() {
        let mut tree = BinaryIndexedTree::new(8);
        assert!(tree.is_empty());
        tree.add(2);
        tree.add(3);
        tree.add(3);
        tree.add(7);
        assert_eq!(tree.size(), 4);
        assert_eq!(tree.rank(0), 0);
        assert_eq!(tree.rank(2), 0);
        assert_eq!(tree.rank(3), 1);
        assert_eq!(tree.rank(4), 3);
        assert_eq!(tree.rank(8), 4);
        tree.remove_all(3);
        assert_eq!(tree.size(), 2);
        assert_eq!(tree.rank(8), 2);
        tree.clear();
        assert!(tree.is_empty());
        assert_eq!(tree.rank(8), 0);
    }

    /// Two layers:
    ///   left:  a (east port pa), b (east port pb)
    ///   right: c (west port pc), d (west port pd)
    /// edges a->d and b->c cross exactly once.
    #[test]
    fn count_crossings_between_layers() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        let l0 = a.create_layer(g);
        let l1 = a.create_layer(g);
        a.graph_mut(g).layers.push(l0);
        a.graph_mut(g).layers.push(l1);

        let mk_node = |arena: &mut LGraphArena, layer, side| {
            let n = arena.create_node(g);
            arena.node_set_layer(n, Some(layer));
            let p = arena.create_port();
            arena.port_set_node(p, Some(n));
            arena.port_set_side(p, side);
            (n, p)
        };
        let (na, pa) = mk_node(&mut a, l0, PortSide::EAST);
        let (nb, pb) = mk_node(&mut a, l0, PortSide::EAST);
        let (nc, pc) = mk_node(&mut a, l1, PortSide::WEST);
        let (nd, pd) = mk_node(&mut a, l1, PortSide::WEST);

        let e1 = a.create_edge();
        a.edge_set_source(e1, Some(pa));
        a.edge_set_target(e1, Some(pd));
        let e2 = a.create_edge();
        a.edge_set_source(e2, Some(pb));
        a.edge_set_target(e2, Some(pc));

        // assign port ids as the initialization traversal would
        for (i, p) in [pa, pb, pc, pd].iter().enumerate() {
            a.port_mut(*p).id = i as i32;
        }
        for n in [na, nb, nc, nd] {
            a.node_cache_port_sides(n);
        }

        let mut counter = CrossingsCounter::new(vec![0; 4]);
        // Hand trace: positions counter-clockwise: pa=0, pb=1 (left, east,
        // top-down); pd=2, pc=3 (right, west, bottom-up). Port sweep order
        // [pa, pb, pd, pc]: pa adds end 2; pb sees rank(3) = |{2}| = 1.
        assert_eq!(counter.count_crossings_between_layers(&a, &[na, nb], &[nc, nd]), 1);
        // switched order of the right layer removes the crossing
        assert_eq!(counter.count_crossings_between_layers(&a, &[na, nb], &[nd, nc]), 0);
        // switching the left layer also removes it (edges become parallel)
        assert_eq!(counter.count_crossings_between_layers(&a, &[nb, na], &[nc, nd]), 0);
        // switching both layers crosses again
        assert_eq!(counter.count_crossings_between_layers(&a, &[nb, na], &[nd, nc]), 1);
    }
}
