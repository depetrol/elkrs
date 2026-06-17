//!
//! The comparators carry mutable transitive-ordering state
//! (`biggerThan` / `smallerThan`) that is updated during the sort, so they
//! cannot be plain stateless closures. Each comparator is a struct that
//! borrows the arena and owns its ordering maps; sorts are driven through
//! [`crate::core::javacompat::tim_sort`] (which calls `compare` in the exact
//! OpenJDK TimSort sequence) or the processor's bespoke insertion sort.

use std::collections::{HashMap, HashSet};

use crate::core::options::{PortConstraints, PortSide};

use crate::alg_layered::graph::{LGraphArena, LNodeId, LPortId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::TargetNodeModelOrder;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::{GroupOrderStrategy, LongEdgeOrderingStrategy, OrderingStrategy};

/// An element that can carry MODEL_ORDER / group-model-order properties:
/// an `LNode`, an `LPort` or an `LEdge`.
#[derive(Clone, Copy)]
pub enum Elem {
    Node(LNodeId),
    Port(LPortId),
    Edge(crate::alg_layered::graph::LEdgeId),
}

impl Elem {
    fn has_model_order(self, a: &LGraphArena) -> bool {
        match self {
            Elem::Node(n) => a.node(n).properties.has(&iprops::MODEL_ORDER),
            Elem::Port(p) => a.port(p).properties.has(&iprops::MODEL_ORDER),
            Elem::Edge(e) => a.edge(e).properties.has(&iprops::MODEL_ORDER),
        }
    }
    fn model_order(self, a: &LGraphArena) -> i32 {
        match self {
            Elem::Node(n) => a.node(n).properties.get(&iprops::MODEL_ORDER),
            Elem::Port(p) => a.port(p).properties.get(&iprops::MODEL_ORDER),
            Elem::Edge(e) => a.edge(e).properties.get(&iprops::MODEL_ORDER),
        }
    }
    fn cm_id(self, a: &LGraphArena) -> i32 {
        let p = &lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CROSSING_MINIMIZATION_ID;
        match self {
            Elem::Node(n) => a.node(n).properties.get(p),
            Elem::Port(pt) => a.port(pt).properties.get(p),
            Elem::Edge(e) => a.edge(e).properties.get(p),
        }
    }
}

pub fn calculate_model_order_or_group_model_order(
    a: &LGraphArena,
    parent: crate::alg_layered::graph::LGraphId,
    element: Elem,
    other: Elem,
    offset: i32,
) -> i32 {
    let enforce_group_model_order = a
        .graph(parent)
        .properties
        .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CM_GROUP_ORDER_STRATEGY)
        == GroupOrderStrategy::ENFORCED;
    let enforced_orders = a
        .graph(parent)
        .properties
        .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CM_ENFORCED_GROUP_ORDERS);
    if !element.has_model_order(a) {
        return -1;
    } else if enforce_group_model_order {
        if enforced_orders.contains(&element.cm_id(a)) && enforced_orders.contains(&other.cm_id(a)) {
            return offset
                .wrapping_mul(element.cm_id(a))
                .wrapping_add(element.model_order(a));
        }
        // Fallthrough
    } else {
        return element.model_order(a);
    }
    element.model_order(a)
}

// ===========================================================================
// ModelOrderNodeComparator
// ===========================================================================

pub struct ModelOrderNodeComparator<'a> {
    a: &'a LGraphArena,
    graph: crate::alg_layered::graph::LGraphId,
    previous_layer: Vec<LNodeId>,
    ordering_strategy: OrderingStrategy,
    #[allow(dead_code)]
    group_order_strategy: GroupOrderStrategy,
    long_edge_node_order: LongEdgeOrderingStrategy,
    before_ports: bool,
    bigger_than: HashMap<LNodeId, HashSet<LNodeId>>,
    smaller_than: HashMap<LNodeId, HashSet<LNodeId>>,
}

impl<'a> ModelOrderNodeComparator<'a> {
    pub fn new(
        a: &'a LGraphArena,
        graph: crate::alg_layered::graph::LGraphId,
        previous_layer: Vec<LNodeId>,
        ordering_strategy: OrderingStrategy,
        long_edge_ordering_strategy: LongEdgeOrderingStrategy,
        group_order_strategy: GroupOrderStrategy,
        before_ports: bool,
    ) -> Self {
        ModelOrderNodeComparator {
            a,
            graph,
            previous_layer,
            ordering_strategy,
            group_order_strategy,
            long_edge_node_order: long_edge_ordering_strategy,
            before_ports,
            bigger_than: HashMap::new(),
            smaller_than: HashMap::new(),
        }
    }

    pub fn clear_transitive_ordering(&mut self) {
        self.bigger_than = HashMap::new();
        self.smaller_than = HashMap::new();
    }

    fn layer_id(&self, n: LNodeId) -> i32 {
        self.a.layer(self.a.node(n).layer.unwrap()).id
    }

    pub fn compare(&mut self, n1: LNodeId, n2: LNodeId) -> i32 {
        if !self.bigger_than.contains_key(&n1) {
            self.bigger_than.insert(n1, HashSet::new());
        } else if self.bigger_than[&n1].contains(&n2) {
            return 1;
        }
        if !self.bigger_than.contains_key(&n2) {
            self.bigger_than.insert(n2, HashSet::new());
        } else if self.bigger_than[&n2].contains(&n1) {
            return -1;
        }
        if !self.smaller_than.contains_key(&n1) {
            self.smaller_than.insert(n1, HashSet::new());
        } else if self.smaller_than[&n1].contains(&n2) {
            return -1;
        }
        if !self.smaller_than.contains_key(&n2) {
            self.smaller_than.insert(n2, HashSet::new());
        } else if self.bigger_than[&n2].contains(&n1) {
            return 1;
        }

        let a = self.a;
        let n1_has_mo = a.node(n1).properties.has(&iprops::MODEL_ORDER);
        let n2_has_mo = a.node(n2).properties.has(&iprops::MODEL_ORDER);

        if self.ordering_strategy == OrderingStrategy::PREFER_EDGES || !n1_has_mo || !n2_has_mo {
            let p1_source_port = self.first_previous_layer_source_port(n1);
            let p2_source_port = self.first_previous_layer_source_port(n2);

            if let (Some(p1sp), Some(p2sp)) = (p1_source_port, p2_source_port) {
                let p1_node = a.port(p1sp).node;
                let p2_node = a.port(p2sp).node;

                if p1_node.is_some() && p1_node == p2_node {
                    let pnode = p1_node.unwrap();
                    for port in a.node(pnode).ports.clone() {
                        if port == p1sp {
                            self.update_bigger_and_smaller(n2, n1);
                            return -1;
                        } else if port == p2sp {
                            self.update_bigger_and_smaller(n1, n2);
                            return 1;
                        }
                    }
                    // assert(false) here; fall back to edge model order.
                    let n1_edge_order = self.model_order_from_connected_edges(n1);
                    let n2_edge_order = self.model_order_from_connected_edges(n2);
                    if n1_edge_order > n2_edge_order {
                        self.update_bigger_and_smaller(n1, n2);
                        return 1;
                    } else {
                        self.update_bigger_and_smaller(n2, n1);
                        return -1;
                    }
                }

                for &previous_node in &self.previous_layer.clone() {
                    if Some(previous_node) == p1_node {
                        self.update_bigger_and_smaller(n2, n1);
                        return -1;
                    } else if Some(previous_node) == p2_node {
                        self.update_bigger_and_smaller(n1, n2);
                        return 1;
                    }
                }
            }

            // One node has no source port.
            if p1_source_port.is_some() != p2_source_port.is_some() {
                let compared = self.handle_helper_dummy_nodes(n1, n2);
                if compared != 0 {
                    if compared > 0 {
                        self.update_bigger_and_smaller(n1, n2);
                    } else {
                        self.update_bigger_and_smaller(n2, n1);
                    }
                    return compared;
                }
                if !n1_has_mo || !n2_has_mo {
                    let n1_model_order = self.model_order_from_connected_edges(n1);
                    let n2_model_order = self.model_order_from_connected_edges(n2);
                    if n1_model_order > n2_model_order {
                        self.update_bigger_and_smaller(n1, n2);
                        return 1;
                    } else {
                        self.update_bigger_and_smaller(n2, n1);
                        return -1;
                    }
                }
            }

            // Both nodes are not connected to the previous layer.
            if p1_source_port.is_none() && p2_source_port.is_none() {
                let compared = self.handle_helper_dummy_nodes(n1, n2);
                if compared != 0 {
                    if compared > 0 {
                        self.update_bigger_and_smaller(n1, n2);
                    } else {
                        self.update_bigger_and_smaller(n2, n1);
                    }
                    return compared;
                }
            }
        }

        // Order nodes by their order in the model.
        if n1_has_mo && n2_has_mo {
            let offset = a.graph(self.graph).properties.get(&iprops::MAX_MODEL_ORDER_NODES);
            let n1_model_order = calculate_model_order_or_group_model_order(
                a,
                self.graph,
                Elem::Node(n1),
                Elem::Node(n2),
                offset,
            );
            let n2_model_order = calculate_model_order_or_group_model_order(
                a,
                self.graph,
                Elem::Node(n2),
                Elem::Node(n1),
                offset,
            );
            if n1_model_order > n2_model_order {
                self.update_bigger_and_smaller(n1, n2);
                1
            } else {
                self.update_bigger_and_smaller(n2, n1);
                -1
            }
        } else {
            self.update_bigger_and_smaller(n2, n1);
            -1
        }
    }

    /// First incoming source port that actually connects to the previous layer.
    fn first_previous_layer_source_port(&self, n: LNodeId) -> Option<LPortId> {
        let a = self.a;
        let layer_id = self.layer_id(n);
        for &p in &a.node(n).ports {
            if let Some(&edge) = a.port(p).incoming_edges.first() {
                let source = a.edge(edge).source.unwrap();
                let src_node = a.port(source).node.unwrap();
                if self.layer_id(src_node) == layer_id - 1 {
                    return Some(source);
                }
            }
        }
        None
    }

    fn model_order_from_connected_edges(&self, n: LNodeId) -> i32 {
        let a = self.a;
        let source_port = a
            .node(n)
            .ports
            .iter()
            .copied()
            .find(|&p| !a.port(p).incoming_edges.is_empty());
        if let Some(sp) = source_port {
            let edge = a.port(sp).incoming_edges[0];
            return a.edge(edge).properties.get(&iprops::MODEL_ORDER);
        }
        self.long_edge_node_order.return_value()
    }

    fn update_bigger_and_smaller(&mut self, bigger: LNodeId, smaller: LNodeId) {
        let smaller_node_bigger_than: Vec<LNodeId> =
            self.bigger_than.get(&smaller).into_iter().flatten().copied().collect();
        let bigger_node_smaller_than: Vec<LNodeId> =
            self.smaller_than.get(&bigger).into_iter().flatten().copied().collect();

        self.bigger_than.get_mut(&bigger).unwrap().insert(smaller);
        self.smaller_than.get_mut(&smaller).unwrap().insert(bigger);

        for very_small in &smaller_node_bigger_than {
            self.bigger_than.get_mut(&bigger).unwrap().insert(*very_small);
            self.smaller_than.get_mut(very_small).unwrap().insert(bigger);
            for &x in &bigger_node_smaller_than {
                self.smaller_than.get_mut(very_small).unwrap().insert(x);
            }
        }

        for very_big in &bigger_node_smaller_than {
            self.smaller_than.get_mut(&smaller).unwrap().insert(*very_big);
            self.bigger_than.get_mut(very_big).unwrap().insert(smaller);
            for &x in &smaller_node_bigger_than {
                self.bigger_than.get_mut(very_big).unwrap().insert(x);
            }
        }
    }

    fn handle_helper_dummy_nodes(&mut self, n1: LNodeId, n2: LNodeId) -> i32 {
        let a = self.a;
        let t1 = a.node(n1).node_type;
        let t2 = a.node(n2).node_type;
        if t1 == NodeType::LONG_EDGE && t2 == NodeType::NORMAL {
            let dummy_source_node = self.first_incoming_source_node(n1);
            let dummy_target_node = self.first_outgoing_target_node(n1);
            let dummy_layer_id = self.layer_id(n1);
            if self.layer_id(dummy_source_node) != dummy_layer_id
                && self.layer_id(dummy_target_node) != dummy_layer_id
            {
                return 0;
            }
            if dummy_source_node == n2 {
                self.update_bigger_and_smaller(n1, n2);
                return 1;
            } else {
                if dummy_target_node == n2 {
                    self.update_bigger_and_smaller(n1, n2);
                    return 1;
                }
                return self.compare(dummy_source_node, n2);
            }
        } else if t1 == NodeType::NORMAL && t2 == NodeType::LONG_EDGE {
            let dummy_source_node = self.first_incoming_source_node(n2);
            let dummy_target_node = self.first_outgoing_target_node(n2);
            // Uses n1.getLayer().id here (quirk preserved).
            let dummy_layer_id = self.layer_id(n1);
            if self.layer_id(dummy_source_node) != dummy_layer_id
                && self.layer_id(dummy_target_node) != dummy_layer_id
            {
                return 0;
            }
            if dummy_source_node == n1 {
                self.update_bigger_and_smaller(n2, n1);
                return -1;
            } else {
                if dummy_target_node == n1 {
                    self.update_bigger_and_smaller(n2, n1);
                    return -1;
                }
                return self.compare(n1, dummy_source_node);
            }
        } else if t1 == NodeType::LONG_EDGE && t2 == NodeType::LONG_EDGE {
            let n1_dummy_source_port = self.first_incoming_source_port(n1);
            let n1_dummy_target_port = self.first_outgoing_target_port(n1);
            let n1_dummy_source_node = a.port(n1_dummy_source_port).node.unwrap();
            let n1_dummy_target_node = a.port(n1_dummy_target_port).node.unwrap();
            let n1_layer_id = self.layer_id(n1);
            let mut n1_source_feedback = false;
            let mut n1_target_feedback = false;

            let n2_dummy_source_port = self.first_incoming_source_port(n2);
            let n2_dummy_target_port = self.first_outgoing_target_port(n2);
            let n2_dummy_source_node = a.port(n2_dummy_source_port).node.unwrap();
            let n2_dummy_target_node = a.port(n2_dummy_target_port).node.unwrap();
            let n2_layer_id = self.layer_id(n2);
            let mut n2_source_feedback = false;
            let mut n2_target_feedback = false;

            let mut n1_reference_node = n1;
            let mut n2_reference_node = n2;
            if self.layer_id(n1_dummy_source_node) == n1_layer_id {
                n1_source_feedback = true;
                n1_reference_node = n1_dummy_source_node;
            } else if self.layer_id(n1_dummy_target_node) == n1_layer_id {
                n1_target_feedback = true;
                n1_reference_node = n1_dummy_target_node;
            }
            if self.layer_id(n2_dummy_source_node) == n2_layer_id {
                n2_source_feedback = true;
                n2_reference_node = n2_dummy_source_node;
            } else if self.layer_id(n2_dummy_target_node) == n2_layer_id {
                n2_target_feedback = true;
                n2_reference_node = n2_dummy_target_node;
            }

            if n1_reference_node == n2_reference_node {
                if self.before_ports {
                    if n1_source_feedback && n2_source_feedback {
                        let mut pc = ModelOrderPortComparator::new(
                            a,
                            self.graph,
                            self.previous_layer.clone(),
                            self.ordering_strategy,
                            None,
                            n2_target_feedback,
                        );
                        let return_value =
                            pc.compare(n1_dummy_source_port, n2_dummy_source_port);
                        if return_value > 0 {
                            self.update_bigger_and_smaller(n2, n1);
                            return 1;
                        } else {
                            self.update_bigger_and_smaller(n1, n2);
                            return -1;
                        }
                    } else if n1_source_feedback && n2_target_feedback {
                        self.update_bigger_and_smaller(n2, n1);
                        return 1;
                    } else if n1_target_feedback && n2_source_feedback {
                        self.update_bigger_and_smaller(n1, n2);
                        return -1;
                    } else if n1_target_feedback && n2_target_feedback {
                        return 0;
                    }
                } else {
                    for port in a.node(n1_reference_node).ports.clone() {
                        if n1_dummy_source_port == port {
                            self.update_bigger_and_smaller(n2, n1);
                            return -1;
                        } else if n2_dummy_source_port == port {
                            self.update_bigger_and_smaller(n1, n2);
                            return 1;
                        }
                    }
                }
            }

            self.compare(n1_reference_node, n2_reference_node)
        } else {
            0
        }
    }

    fn first_incoming_source_port(&self, node: LNodeId) -> LPortId {
        let a = self.a;
        let p = a
            .node(node)
            .ports
            .iter()
            .copied()
            .find(|&p| !a.port(p).incoming_edges.is_empty())
            .unwrap();
        let edge = a.port(p).incoming_edges[0];
        a.edge(edge).source.unwrap()
    }
    fn first_incoming_source_node(&self, node: LNodeId) -> LNodeId {
        self.a.port(self.first_incoming_source_port(node)).node.unwrap()
    }
    fn first_outgoing_target_port(&self, node: LNodeId) -> LPortId {
        let a = self.a;
        let p = a
            .node(node)
            .ports
            .iter()
            .copied()
            .find(|&p| !a.port(p).outgoing_edges.is_empty())
            .unwrap();
        let edge = a.port(p).outgoing_edges[0];
        a.edge(edge).target.unwrap()
    }
    fn first_outgoing_target_node(&self, node: LNodeId) -> LNodeId {
        self.a.port(self.first_outgoing_target_port(node)).node.unwrap()
    }
}

// ===========================================================================
// ModelOrderPortComparator
// ===========================================================================

pub struct ModelOrderPortComparator<'a> {
    a: &'a LGraphArena,
    graph: crate::alg_layered::graph::LGraphId,
    target_node_model_order: Option<TargetNodeModelOrder>,
    port_model_order: bool,
    previous_layer: Vec<LNodeId>,
    strategy: OrderingStrategy,
    bigger_than: HashMap<LPortId, HashSet<LPortId>>,
    smaller_than: HashMap<LPortId, HashSet<LPortId>>,
}

impl<'a> ModelOrderPortComparator<'a> {
    pub fn new(
        a: &'a LGraphArena,
        graph: crate::alg_layered::graph::LGraphId,
        previous_layer: Vec<LNodeId>,
        strategy: OrderingStrategy,
        target_node_model_order: Option<TargetNodeModelOrder>,
        port_model_order: bool,
    ) -> Self {
        ModelOrderPortComparator {
            a,
            graph,
            target_node_model_order,
            port_model_order,
            previous_layer,
            strategy,
            bigger_than: HashMap::new(),
            smaller_than: HashMap::new(),
        }
    }

    pub fn clear_transitive_ordering(&mut self) {
        self.bigger_than = HashMap::new();
        self.smaller_than = HashMap::new();
    }

    fn layer_id(&self, n: LNodeId) -> i32 {
        self.a.layer(self.a.node(n).layer.unwrap()).id
    }

    /// `PortSide` ordinal (NORTH < EAST < SOUTH < WEST). Mirrors the enum order.
    fn side_ordinal(side: PortSide) -> i32 {
        match side {
            PortSide::UNDEFINED => 0,
            PortSide::NORTH => 1,
            PortSide::EAST => 2,
            PortSide::SOUTH => 3,
            PortSide::WEST => 4,
        }
    }

    pub fn compare(&mut self, p1: LPortId, p2: LPortId) -> i32 {
        if !self.bigger_than.contains_key(&p1) {
            self.bigger_than.insert(p1, HashSet::new());
        } else if self.bigger_than[&p1].contains(&p2) {
            return 1;
        }
        if !self.bigger_than.contains_key(&p2) {
            self.bigger_than.insert(p2, HashSet::new());
        } else if self.bigger_than[&p2].contains(&p1) {
            return -1;
        }
        if !self.smaller_than.contains_key(&p1) {
            self.smaller_than.insert(p1, HashSet::new());
        } else if self.smaller_than[&p1].contains(&p2) {
            return -1;
        }
        if !self.smaller_than.contains_key(&p2) {
            self.smaller_than.insert(p2, HashSet::new());
        } else if self.bigger_than[&p2].contains(&p1) {
            return 1;
        }

        let a = self.a;
        let s1 = a.port(p1).side;
        let s2 = a.port(p2).side;
        if s1 != s2 {
            // Integer.compare(ps1.ordinal(), ps2.ordinal())
            let result = match Self::side_ordinal(s1).cmp(&Self::side_ordinal(s2)) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            if result > 0 {
                self.update_bigger_and_smaller(p1, p2, 1);
            } else {
                self.update_bigger_and_smaller(p2, p1, 1);
            }
            return result;
        }

        let mut reverse_order = 1i32;

        let p1_incoming = !a.port(p1).incoming_edges.is_empty();
        let p2_incoming = !a.port(p2).incoming_edges.is_empty();
        let p1_outgoing = !a.port(p1).outgoing_edges.is_empty();
        let p2_outgoing = !a.port(p2).outgoing_edges.is_empty();

        if p1_incoming && p2_incoming {
            if (s1 == PortSide::WEST && s2 == PortSide::WEST)
                || (s1 == PortSide::NORTH && s2 == PortSide::NORTH)
                || (s1 == PortSide::SOUTH && s2 == PortSide::SOUTH)
            {
                reverse_order = -reverse_order;
            }

            let p1_source_port = a.edge(a.port(p1).incoming_edges[0]).source.unwrap();
            let p2_source_port = a.edge(a.port(p2).incoming_edges[0]).source.unwrap();
            let p1_node = a.port(p1_source_port).node.unwrap();
            let p2_node = a.port(p2_source_port).node.unwrap();
            if p1_node == p2_node {
                for port in a.node(p1_node).ports.clone() {
                    if p1_source_port == port {
                        self.update_bigger_and_smaller(p2, p1, reverse_order);
                        return -reverse_order;
                    } else if p2_source_port == port {
                        self.update_bigger_and_smaller(p1, p2, reverse_order);
                        return reverse_order;
                    }
                }
            }
            // Both ports connect to long edges in the same layer.
            if a.node(p1_source_port_node(a, p1)).node_type == NodeType::LONG_EDGE
                && a.node(p2_source_port_node(a, p2)).node_type == NodeType::LONG_EDGE
                && self.layer_id(p1_node) == self.layer_id(p2_node)
                && self.layer_id(p1_node) == self.layer_id(a.port(p1).node.unwrap())
            {
                let in_previous_layer = self.check_reference_layer(
                    &a.layer(a.node(p1_node).layer.unwrap()).nodes.clone(),
                    p1_node,
                    p2_node,
                );
                if in_previous_layer != 0 {
                    if s1 == PortSide::EAST && s2 == PortSide::EAST {
                        reverse_order = -reverse_order;
                    }
                    if in_previous_layer > 0 {
                        self.update_bigger_and_smaller(p1, p2, reverse_order);
                        return reverse_order;
                    } else {
                        self.update_bigger_and_smaller(p2, p1, reverse_order);
                        return -reverse_order;
                    }
                }
            }

            let in_previous_layer =
                self.check_reference_layer(&self.previous_layer.clone(), p1_node, p2_node);
            if in_previous_layer != 0 {
                if in_previous_layer > 0 {
                    self.update_bigger_and_smaller(p1, p2, reverse_order);
                    return reverse_order;
                } else {
                    self.update_bigger_and_smaller(p2, p1, reverse_order);
                    return -reverse_order;
                }
            }
            if self.port_model_order {
                let result = self.check_port_model_order(p1, p2);
                if result != 0 {
                    if result > 0 {
                        self.update_bigger_and_smaller(p1, p2, reverse_order);
                        return reverse_order;
                    } else {
                        self.update_bigger_and_smaller(p2, p1, reverse_order);
                        return -reverse_order;
                    }
                }
            }
        }

        if p1_outgoing && p2_outgoing {
            if (s1 == PortSide::WEST && s2 == PortSide::WEST)
                || (s1 == PortSide::SOUTH && s2 == PortSide::SOUTH)
            {
                reverse_order = -reverse_order;
            }
            let p1_target_node = a.port(p1).properties.try_get(&iprops::LONG_EDGE_TARGET_NODE);
            let p2_target_node = a.port(p2).properties.try_get(&iprops::LONG_EDGE_TARGET_NODE);

            if self.strategy == OrderingStrategy::PREFER_NODES
                && p1_target_node.is_some()
                && p2_target_node.is_some()
                && a.node(p1_target_node.unwrap()).properties.has(&iprops::MODEL_ORDER)
                && a.node(p2_target_node.unwrap()).properties.has(&iprops::MODEL_ORDER)
            {
                let offset = a.graph(self.graph).properties.get(&iprops::MAX_MODEL_ORDER_NODES);
                let p1mo = calculate_model_order_or_group_model_order(
                    a,
                    self.graph,
                    Elem::Node(p1_target_node.unwrap()),
                    Elem::Node(p2_target_node.unwrap()),
                    offset,
                );
                let p2mo = calculate_model_order_or_group_model_order(
                    a,
                    self.graph,
                    Elem::Node(p2_target_node.unwrap()),
                    Elem::Node(p1_target_node.unwrap()),
                    offset,
                );
                if p1mo > p2mo {
                    self.update_bigger_and_smaller(p1, p2, reverse_order);
                    return reverse_order;
                } else {
                    self.update_bigger_and_smaller(p2, p1, reverse_order);
                    return -reverse_order;
                }
            }

            if self.port_model_order {
                let result = self.check_port_model_order(p1, p2);
                if result != 0 {
                    if result > 0 {
                        self.update_bigger_and_smaller(p1, p2, reverse_order);
                        return reverse_order;
                    } else {
                        self.update_bigger_and_smaller(p2, p1, reverse_order);
                        return -reverse_order;
                    }
                }
            }

            let p1_first_out = a.port(p1).outgoing_edges[0];
            let p2_first_out = a.port(p2).outgoing_edges[0];
            let mut p1_order = 0;
            let mut p2_order = 0;
            if a.edge(p1_first_out).properties.has(&iprops::MODEL_ORDER) {
                let off = (a.port(p1).outgoing_edges.len() + a.port(p1).incoming_edges.len()) as i32;
                p1_order = calculate_model_order_or_group_model_order(
                    a,
                    self.graph,
                    Elem::Edge(p1_first_out),
                    Elem::Edge(p2_first_out),
                    off,
                );
            }
            if a.edge(p2_first_out).properties.has(&iprops::MODEL_ORDER) {
                let off = (a.port(p2).outgoing_edges.len() + a.port(p2).incoming_edges.len()) as i32;
                p2_order = calculate_model_order_or_group_model_order(
                    a,
                    self.graph,
                    Elem::Edge(p2_first_out),
                    Elem::Edge(p1_first_out),
                    off,
                );
            }

            if p1_target_node.is_some() && p1_target_node == p2_target_node {
                if p1_order > p2_order {
                    self.update_bigger_and_smaller(p1, p2, reverse_order);
                    return reverse_order;
                } else {
                    self.update_bigger_and_smaller(p2, p1, reverse_order);
                    return -reverse_order;
                }
            }
            if let Some(tnmo) = &self.target_node_model_order {
                if let Some(tn) = p1_target_node {
                    if let Some(v) = tnmo.0.get(&tn) {
                        p1_order = *v;
                    }
                }
                if let Some(tn) = p2_target_node {
                    if let Some(v) = tnmo.0.get(&tn) {
                        p2_order = *v;
                    }
                }
            }
            if p1_order > p2_order {
                self.update_bigger_and_smaller(p1, p2, reverse_order);
                return reverse_order;
            } else {
                self.update_bigger_and_smaller(p2, p1, reverse_order);
                return -reverse_order;
            }
        }

        // Sort outgoing ports before incoming ports.
        if p1_incoming && p2_outgoing {
            self.update_bigger_and_smaller(p1, p2, reverse_order);
            1
        } else if p1_outgoing && p2_incoming {
            self.update_bigger_and_smaller(p2, p1, reverse_order);
            -1
        } else if a.port(p1).properties.has(&iprops::MODEL_ORDER)
            && a.port(p2).properties.has(&iprops::MODEL_ORDER)
        {
            let number_of_ports = a.node(a.port(p1).node.unwrap()).ports.len() as i32;
            let p1mo = calculate_model_order_or_group_model_order(
                a,
                self.graph,
                Elem::Port(p1),
                Elem::Port(p2),
                number_of_ports,
            );
            let p2mo = calculate_model_order_or_group_model_order(
                a,
                self.graph,
                Elem::Port(p2),
                Elem::Port(p1),
                number_of_ports,
            );
            if (s1 == PortSide::WEST && s2 == PortSide::WEST)
                || (s1 == PortSide::SOUTH && s2 == PortSide::SOUTH)
            {
                reverse_order = -reverse_order;
            }
            if p1mo > p2mo {
                self.update_bigger_and_smaller(p1, p2, reverse_order);
                reverse_order
            } else {
                self.update_bigger_and_smaller(p2, p1, reverse_order);
                -reverse_order
            }
        } else {
            self.update_bigger_and_smaller(p2, p1, reverse_order);
            -reverse_order
        }
    }

    pub fn check_port_model_order(&self, p1: LPortId, p2: LPortId) -> i32 {
        let a = self.a;
        let number_of_ports = a.node(a.port(p1).node.unwrap()).ports.len() as i32;
        if a.port(p1).properties.has(&iprops::MODEL_ORDER)
            && a.port(p2).properties.has(&iprops::MODEL_ORDER)
        {
            let p1_order = calculate_model_order_or_group_model_order(
                a,
                self.graph,
                Elem::Port(p1),
                Elem::Port(p2),
                number_of_ports,
            );
            let p2_order = calculate_model_order_or_group_model_order(
                a,
                self.graph,
                Elem::Port(p2),
                Elem::Port(p1),
                number_of_ports,
            );
            return match p1_order.cmp(&p2_order) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
        }
        0
    }

    fn check_reference_layer(&self, layer: &[LNodeId], p1_node: LNodeId, p2_node: LNodeId) -> i32 {
        for &node in layer {
            if node == p1_node {
                return -1;
            } else if node == p2_node {
                return 1;
            }
        }
        0
    }

    fn update_bigger_and_smaller(&mut self, bigger_ori: LPortId, smaller_ori: LPortId, reverse_order: i32) {
        let (bigger, smaller) = if reverse_order < 0 {
            (smaller_ori, bigger_ori)
        } else {
            (bigger_ori, smaller_ori)
        };
        let smaller_port_bigger_than: Vec<LPortId> =
            self.bigger_than.get(&smaller).into_iter().flatten().copied().collect();
        let bigger_port_smaller_than: Vec<LPortId> =
            self.smaller_than.get(&bigger).into_iter().flatten().copied().collect();

        self.bigger_than.get_mut(&bigger).unwrap().insert(smaller);
        self.smaller_than.get_mut(&smaller).unwrap().insert(bigger);

        for very_small in &smaller_port_bigger_than {
            self.bigger_than.get_mut(&bigger).unwrap().insert(*very_small);
            self.smaller_than.get_mut(very_small).unwrap().insert(bigger);
            for &x in &bigger_port_smaller_than {
                self.smaller_than.get_mut(very_small).unwrap().insert(x);
            }
        }
        for very_big in &bigger_port_smaller_than {
            self.smaller_than.get_mut(&smaller).unwrap().insert(*very_big);
            self.bigger_than.get_mut(very_big).unwrap().insert(smaller);
            for &x in &smaller_port_bigger_than {
                self.bigger_than.get_mut(very_big).unwrap().insert(x);
            }
        }
    }
}

fn p1_source_port_node(a: &LGraphArena, p: LPortId) -> LNodeId {
    let sp = a.edge(a.port(p).incoming_edges[0]).source.unwrap();
    a.port(sp).node.unwrap()
}
fn p2_source_port_node(a: &LGraphArena, p: LPortId) -> LNodeId {
    p1_source_port_node(a, p)
}

/// Helper used by `SortByInputModelProcessor`'s outgoing-port sort: query
/// whether a node has FIXED port order/positions (skips sorting).
pub fn has_fixed_port_order(a: &LGraphArena, node: LNodeId) -> bool {
    let pc: PortConstraints = a.node(node).properties.get(&lopts::PORT_CONSTRAINTS);
    pc == PortConstraints::FIXED_ORDER || pc == PortConstraints::FIXED_POS
}
