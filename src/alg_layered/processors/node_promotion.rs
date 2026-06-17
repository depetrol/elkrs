//!
//! Node promotion heuristic of Nikolov and Tarassov with a few more options
//! for handling and stopping the promotion earlier. The goal is a layering
//! with fewer dummy nodes.

use std::collections::HashMap;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::NodePromotionStrategy;

/// Order-preserving
/// key -> list-of-values map with a reverse value -> key map.
#[derive(Default)]
struct BiLayerMap {
    /// keys in insertion order
    keys: Vec<i32>,
    /// key -> ordered values
    key_to_values: HashMap<i32, Vec<LNodeId>>,
    /// value -> key
    value_to_key: HashMap<LNodeId, i32>,
}

impl BiLayerMap {
    fn put_all(&mut self, key: i32, values: &[LNodeId]) {
        for &value in values {
            self.put(key, value);
        }
    }

    fn put(&mut self, key: i32, value: LNodeId) {
        // Remove old value.
        if let Some(&old_key) = self.value_to_key.get(&value) {
            if let Some(values) = self.key_to_values.get_mut(&old_key) {
                if let Some(pos) = values.iter().position(|&v| v == value) {
                    values.remove(pos);
                }
            }
        }
        // Add new value.
        if !self.key_to_values.contains_key(&key) {
            self.keys.push(key);
            self.key_to_values.insert(key, Vec::new());
        }
        self.key_to_values.get_mut(&key).unwrap().push(value);
        self.value_to_key.insert(value, key);
    }

    fn get_key(&self, value: LNodeId) -> i32 {
        self.value_to_key[&value]
    }

    /// Returns a copy of the values (the callers below re-read the map on
    /// every access to replicate live-list semantics).
    fn get_values(&self, key: i32) -> Vec<LNodeId> {
        self.key_to_values.get(&key).cloned().unwrap_or_default()
    }

    fn values_len(&self, key: i32) -> usize {
        self.key_to_values.get(&key).map_or(0, |v| v.len())
    }

    fn value_at(&self, key: i32, index: usize) -> LNodeId {
        self.key_to_values[&key][index]
    }

    fn key_count(&self) -> usize {
        self.keys.len()
    }

    fn is_maximal_key(&self, key: i32) -> bool {
        self.keys.iter().all(|&other| key >= other)
    }

    fn is_minimal_key(&self, key: i32) -> bool {
        self.keys.iter().all(|&other| key <= other)
    }
}

/// All state of one NodePromotion run.
struct NodePromotion {
    /// Holds all nodes of the graph that have incoming edges.
    nodes_with_incoming_edges: Vec<LNodeId>,
    /// Stores all nodes of the graph.
    nodes: Vec<LNodeId>,
    /// Per layer: current number of original and dummy nodes inside it.
    current_width: Vec<i32>,
    /// Per layer: current approximated width in pixels.
    current_width_pixel: Vec<f64>,
    /// Per node id: the index of the layer it is currently assigned to.
    layers: Vec<i32>,
    /// Per node id: [out - in, in, out] degree information.
    degree_diff: Vec<[i32; 3]>,
    /// Maximal accepted width of the graph before processing.
    max_width: i32,
    /// Approximated maximal accepted width in pixels before processing.
    max_width_pixel: f64,
    /// Current number of dummy nodes in the graph.
    dummy_node_count: i32,
    /// Current height of the graph.
    max_height: i32,
    /// Approximated additional width per node.
    node_size_affix: f64,
    /// Approximated size in pixels for a dummy node.
    dummy_size: f64,
    /// The strategy which is used for the node promotion.
    promotion_strategy: NodePromotionStrategy,
    bi_layer_map: BiLayerMap,
}

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let strategy: NodePromotionStrategy = a
        .graph(graph)
        .properties
        .get(&lopts::LAYERING_NODE_PROMOTION_STRATEGY);

    let mut np = NodePromotion {
        nodes_with_incoming_edges: Vec::new(),
        nodes: Vec::new(),
        current_width: Vec::new(),
        current_width_pixel: Vec::new(),
        layers: Vec::new(),
        degree_diff: Vec::new(),
        max_width: 0,
        max_width_pixel: 0.0,
        dummy_node_count: 0,
        max_height: 0,
        node_size_affix: 0.0,
        dummy_size: 0.0,
        promotion_strategy: strategy,
        bi_layer_map: BiLayerMap::default(),
    };

    let model_order = strategy == NodePromotionStrategy::MODEL_ORDER_LEFT_TO_RIGHT
        || strategy == NodePromotionStrategy::MODEL_ORDER_RIGHT_TO_LEFT;
    if !model_order {
        np.precalculate_and_set_information(a, graph);
    } else {
        np.precalculate_and_set_information_model_order(a, graph);
    }

    // If the promotion strategy is set to DUMMYNODE_PERCENTAGE or
    // NODECOUNT_PERCENTAGE this value affects the termination criterion.
    let promote_until: i32 = a
        .graph(graph)
        .properties
        .get(&lopts::LAYERING_NODE_PROMOTION_MAX_ITERATIONS);

    match strategy {
        NodePromotionStrategy::NIKOLOV => {
            np.promotion_magic(a, |_, _| true);
        }
        NodePromotionStrategy::NIKOLOV_PIXEL => {
            np.promotion_magic(a, |_, _| true);
        }
        NodePromotionStrategy::NIKOLOV_IMPROVED => {
            np.promotion_strategy = NodePromotionStrategy::NO_BOUNDARY;
            np.promotion_magic(a, |_, _| true);
            // Determine if the max width of original plus dummy nodes is
            // bigger than before the promotion. If so, use a more cautious
            // style of promotion.
            let mut new_max_width = 0;
            for &martha in &np.current_width {
                new_max_width = i32::max(new_max_width, martha);
            }
            if new_max_width > np.max_width {
                // maximal width exceeded
                np.promotion_strategy = NodePromotionStrategy::NIKOLOV;
                np.promotion_magic(a, |_, _| true);
            }
        }
        NodePromotionStrategy::NIKOLOV_IMPROVED_PIXEL => {
            np.promotion_strategy = NodePromotionStrategy::NO_BOUNDARY;
            np.promotion_magic(a, |_, _| true);
            let mut new_max_width_pixel = 0.0f64;
            for &donna in &np.current_width_pixel {
                new_max_width_pixel = f64::max(new_max_width_pixel, donna);
            }
            if new_max_width_pixel > np.max_width_pixel {
                // maximal width exceeded
                np.promotion_strategy = NodePromotionStrategy::NIKOLOV_PIXEL;
                np.promotion_magic(a, |_, _| true);
            }
        }
        NodePromotionStrategy::NODECOUNT_PERCENTAGE => {
            // Maximal number of iterations depending on the number of nodes.
            let promote_until_n =
                ((np.layers.len() as i32).wrapping_mul(promote_until) as f64 / 100.0).ceil()
                    as i32;
            np.promotion_magic(a, move |_, iterations| iterations < promote_until_n);
        }
        NodePromotionStrategy::DUMMYNODE_PERCENTAGE => {
            // Number of dummy nodes the algorithm shall ideally reduce.
            let promote_until_d =
                (np.dummy_node_count.wrapping_mul(promote_until) as f64 / 100.0).ceil() as i32;
            np.promotion_magic(a, move |reduced_dummies, _| reduced_dummies < promote_until_d);
        }
        NodePromotionStrategy::MODEL_ORDER_LEFT_TO_RIGHT => {
            np.model_order_node_promotion(a, true);
        }
        NodePromotionStrategy::MODEL_ORDER_RIGHT_TO_LEFT => {
            np.model_order_node_promotion(a, false);
        }
        _ => {
            np.promotion_magic(a, |_, _| true);
        }
    }

    if np.promotion_strategy != NodePromotionStrategy::MODEL_ORDER_LEFT_TO_RIGHT
        && np.promotion_strategy != NodePromotionStrategy::MODEL_ORDER_RIGHT_TO_LEFT
    {
        np.set_new_layering(a, graph);
    } else {
        np.set_new_layering_model_order(a, graph);
    }

    Ok(())
}

impl NodePromotion {
    fn precalculate_and_set_information(&mut self, a: &mut LGraphArena, graph: LGraphId) {
        // Calculate approximative addition of space for a node.
        self.node_size_affix = a.graph(graph).properties.get(&lopts::SPACING_NODE_NODE);
        // And an approximative size of a dummy node inside the graph.
        self.dummy_size = a
            .graph(graph)
            .properties
            .get(&lopts::SPACING_EDGE_NODE_BETWEEN_LAYERS);

        self.max_height = a.graph(graph).layers.len() as i32;
        let mut layer_id = self.max_height - 1;
        let mut node_id = 0i32;
        self.max_width = 0;
        self.max_width_pixel = 0.0;
        self.current_width = vec![0; self.max_height as usize];
        self.current_width_pixel = vec![0.0; self.max_height as usize];

        // Set IDs for all layers and nodes.
        // Layer IDs are reversed for easier handling in the heuristic.
        let graph_layers: Vec<LayerId> = a.graph(graph).layers.clone();
        for &layer in &graph_layers {
            a.layer_mut(layer).id = layer_id;
            for node in a.layer(layer).nodes.clone() {
                a.node_mut(node).id = node_id;
                node_id += 1;
            }
            layer_id -= 1;
        }

        self.layers = vec![0; node_id as usize];
        self.degree_diff = vec![[0; 3]; node_id as usize];
        self.nodes = Vec::new();
        self.nodes_with_incoming_edges = Vec::new();
        let mut dummy_baggage = 0i32; // number of dummy nodes between the layers
        self.dummy_node_count = 0;

        // Calculate difference and determine all nodes with incoming edges.
        for &layer in &graph_layers {
            let layer_id = a.layer(layer).id;
            let mut incoming = 0i32;
            let mut outcoming = 0i32;
            let layer_size = a.layer(layer).nodes.len() as i32;
            let mut layer_size_pixel = 0f64;

            for node in a.layer(layer).nodes.clone() {
                let node_id = a.node(node).id as usize;
                self.layers[node_id] = a.layer(a.node(node).layer.unwrap()).id;
                // Accumulate width of every node.
                layer_size_pixel += a.node(node).size.y + self.node_size_affix;
                let in_degree = a.node_incoming_edges(node).len() as i32;
                let out_degree = a.node_outgoing_edges(node).len() as i32;
                self.degree_diff[node_id][0] = out_degree - in_degree;
                self.degree_diff[node_id][1] = in_degree;
                self.degree_diff[node_id][2] = out_degree;
                incoming += in_degree;
                outcoming += out_degree;
                if in_degree > 0 {
                    self.nodes_with_incoming_edges.push(node);
                }
                self.nodes.push(node);
            }

            // Edges that end here don't create dummy nodes in this layer.
            dummy_baggage -= incoming;
            let nodes_n_dummies = layer_size + dummy_baggage;
            layer_size_pixel += dummy_baggage as f64 * self.dummy_size;

            self.current_width[layer_id as usize] = nodes_n_dummies;
            self.current_width_pixel[layer_id as usize] = layer_size_pixel;
            self.max_width = i32::max(self.max_width, nodes_n_dummies);
            self.max_width_pixel = f64::max(self.max_width_pixel, layer_size_pixel);
            self.dummy_node_count += dummy_baggage;
            dummy_baggage += outcoming;
        }
    }

    fn precalculate_and_set_information_model_order(
        &mut self,
        a: &mut LGraphArena,
        graph: LGraphId,
    ) {
        // Set layer and node ids.
        self.bi_layer_map = BiLayerMap::default();
        let mut node_id = 0i32;
        let mut layer_id = 0i32;
        let graph_layers: Vec<LayerId> = a.graph(graph).layers.clone();
        for &layer in &graph_layers {
            a.layer_mut(layer).id = layer_id;
            for node in a.layer(layer).nodes.clone() {
                a.node_mut(node).id = node_id;
                node_id += 1;
            }
            layer_id += 1;
        }
        // Initialize the data structure.
        let left_to_right =
            self.promotion_strategy == NodePromotionStrategy::MODEL_ORDER_LEFT_TO_RIGHT;
        for &layer in &graph_layers {
            // Sort the layer's actual node list (stable sort, comparator
            // returns 0 unless both nodes have a model order).
            let mut nodes = a.layer(layer).nodes.clone();
            nodes.sort_by(|&o1, &o2| {
                let m1: Option<i32> = a.node(o1).properties.try_get(&iprops::MODEL_ORDER);
                let m2: Option<i32> = a.node(o2).properties.try_get(&iprops::MODEL_ORDER);
                match (m1, m2) {
                    (Some(m1), Some(m2)) => {
                        if left_to_right {
                            m2.cmp(&m1) // descending
                        } else {
                            m1.cmp(&m2) // ascending
                        }
                    }
                    _ => std::cmp::Ordering::Equal,
                }
            });
            a.layer_mut(layer).nodes = nodes.clone();
            self.bi_layer_map.put_all(a.layer(layer).id, &nodes);
        }
    }

    fn model_order_node_promotion(&mut self, a: &LGraphArena, left_to_right: bool) {
        loop {
            let mut something_changed = false;
            let mut current_layer_id = if left_to_right {
                self.bi_layer_map.key_count() as i32 - 2
            } else {
                1
            };
            while if left_to_right {
                current_layer_id >= 0
            } else {
                current_layer_id < self.bi_layer_map.key_count() as i32
            } {
                let mut node_index = 0i32;
                while (node_index as usize) < self.bi_layer_map.values_len(current_layer_id) {
                    let node = self.bi_layer_map.value_at(current_layer_id, node_index as usize);
                    // Only nodes with a model order can sensibly be promoted.
                    if !a.node(node).properties.has(&iprops::MODEL_ORDER) {
                        node_index += 1;
                        continue;
                    }
                    // The last/first node shall not be promoted if no other
                    // node is there to compare it to.
                    if self.bi_layer_map.is_maximal_key(current_layer_id)
                        && self.promotion_strategy
                            == NodePromotionStrategy::MODEL_ORDER_LEFT_TO_RIGHT
                        || self.bi_layer_map.is_minimal_key(current_layer_id)
                            && self.promotion_strategy
                                == NodePromotionStrategy::MODEL_ORDER_RIGHT_TO_LEFT
                    {
                        node_index += 1;
                        continue;
                    }
                    // Check whether this layer has a model order that
                    // prevents node promotion.
                    let node_order: i32 = a.node(node).properties.get(&iprops::MODEL_ORDER);
                    let mut shall_be_promoted = true;
                    for other_node in self.bi_layer_map.get_values(current_layer_id) {
                        if let Some(other_order) =
                            a.node(other_node).properties.try_get::<i32>(&iprops::MODEL_ORDER)
                        {
                            if left_to_right && node_order < other_order
                                || !left_to_right && node_order > other_order
                            {
                                shall_be_promoted = false;
                            }
                        }
                    }
                    if !shall_be_promoted {
                        node_index += 1;
                        continue;
                    }
                    // If the next layer has a node with a smaller/bigger
                    // model order, promote the current node.
                    let next_layer_id = if left_to_right {
                        current_layer_id + 1
                    } else {
                        current_layer_id - 1
                    };
                    let next_layer = self.bi_layer_map.get_values(next_layer_id);
                    let mut model_order_allows_promotion = false;
                    let mut promote_through_dummy_layer = true;
                    let mut contains_labels = false;
                    for next_layer_node in next_layer {
                        if let Some(other_order) = a
                            .node(next_layer_node)
                            .properties
                            .try_get::<i32>(&iprops::MODEL_ORDER)
                        {
                            if a.node(next_layer_node).id != a.node(node).id {
                                model_order_allows_promotion |= if left_to_right {
                                    other_order < node_order
                                } else {
                                    other_order > node_order
                                };
                                promote_through_dummy_layer = false;
                            }
                        } else if !model_order_allows_promotion && promote_through_dummy_layer {
                            // Check whether the node can be promoted through
                            // a label layer.
                            if a.node(next_layer_node).node_type == NodeType::LABEL {
                                contains_labels = true;
                                let node_connected_to_next_layer = if left_to_right {
                                    a.edge_source_node(a.node_incoming_edges(next_layer_node)[0])
                                } else {
                                    a.edge_target_node(a.node_outgoing_edges(next_layer_node)[0])
                                };
                                if node_connected_to_next_layer == node {
                                    let connected_node = if left_to_right {
                                        a.edge_target_node(
                                            a.node_outgoing_edges(next_layer_node)[0],
                                        )
                                    } else {
                                        a.edge_source_node(
                                            a.node_incoming_edges(next_layer_node)[0],
                                        )
                                    };
                                    let diff = if left_to_right {
                                        self.bi_layer_map.get_key(connected_node)
                                            - self
                                                .bi_layer_map
                                                .get_key(node_connected_to_next_layer)
                                    } else {
                                        self.bi_layer_map.get_key(node_connected_to_next_layer)
                                            - self.bi_layer_map.get_key(connected_node)
                                    };
                                    if diff <= 2 {
                                        promote_through_dummy_layer = false;
                                    }
                                }
                            }
                        }
                    }
                    if contains_labels && promote_through_dummy_layer {
                        // Check whether the current node has a long enough
                        // edge to move through the whole label layer.
                        let connected_node = if left_to_right {
                            a.edge_target_node(a.node_outgoing_edges(node)[0])
                        } else {
                            a.edge_source_node(a.node_incoming_edges(node)[0])
                        };
                        let diff = if left_to_right {
                            self.bi_layer_map.get_key(connected_node)
                                - self.bi_layer_map.get_key(node)
                        } else {
                            self.bi_layer_map.get_key(node)
                                - self.bi_layer_map.get_key(connected_node)
                        };
                        if diff <= 2 && a.node(connected_node).node_type == NodeType::NORMAL {
                            promote_through_dummy_layer = false;
                        }
                    }
                    // Promote, if allowed.
                    if model_order_allows_promotion || promote_through_dummy_layer {
                        let mut nodes_to_promote =
                            self.promote_node_by_model_order(a, node, left_to_right);
                        // Promote nodes to promote, which again create other
                        // nodes to promote until all nodes are promoted.
                        while !nodes_to_promote.is_empty() {
                            let node_to_promote = nodes_to_promote.remove(0);
                            for n in
                                self.promote_node_by_model_order(a, node_to_promote, left_to_right)
                            {
                                if !nodes_to_promote.contains(&n) {
                                    nodes_to_promote.push(n);
                                }
                            }
                        }
                        // Select next node.
                        node_index -= 1;
                        something_changed = true;
                    }
                    node_index += 1;
                }
                current_layer_id += if left_to_right { -1 } else { 1 };
            }
            if !something_changed {
                break;
            }
        }
    }

    /// Promotes a node and
    /// returns the connected nodes that now have to be promoted as well
    /// (insertion-ordered set).
    fn promote_node_by_model_order(
        &mut self,
        a: &LGraphArena,
        node: LNodeId,
        left_to_right: bool,
    ) -> Vec<LNodeId> {
        // Promote the node.
        let old_layer_id = self.bi_layer_map.get_key(node);
        if left_to_right {
            self.bi_layer_map.put(old_layer_id + 1, node);
        } else {
            self.bi_layer_map.put(old_layer_id - 1, node);
        }
        // Recursively promote connected nodes if necessary.
        let mut nodes_to_promote: Vec<LNodeId> = Vec::new();
        let edges = if left_to_right {
            a.node_outgoing_edges(node)
        } else {
            a.node_incoming_edges(node)
        };
        for edge in edges {
            let next_node = if left_to_right {
                a.edge_target_node(edge)
            } else {
                a.edge_source_node(edge)
            };
            // If the current node is now in the same layer as a node
            // connected to it, promote the connected node.
            if self.bi_layer_map.get_key(next_node) == self.bi_layer_map.get_key(node)
                && !nodes_to_promote.contains(&next_node)
            {
                nodes_to_promote.push(next_node);
            }
        }
        nodes_to_promote
    }

    /// `funky(reduced_dummies, iteration_counter)` decides whether to go on.
    ///
    /// Note one subtle behavior that must be preserved exactly: the initial
    /// `current_width_backup`/`current_width_pixel_backup` are *aliases* of the
    /// live lists (no copy!), while `layering_backup` is a real copy. Until the
    /// first *successful* promotion replaces the backups with real copies, a
    /// failed promotion therefore does not roll the width lists back: the first
    /// failure freezes the (already mutated) state as the backup, and later
    /// failures restore to that frozen state.
    fn promotion_magic(&mut self, a: &LGraphArena, funky: impl Fn(i32, i32) -> bool) {
        let mut iteration_counter = 0i32;
        let mut reduced_dummies = 0i32;

        let mut layering_backup = self.layers.clone();
        let mut dummy_backup = self.dummy_node_count;
        let mut height_backup = self.max_height;
        let mut width_backup_is_alias = true;
        let mut current_width_backup: Vec<i32> = Vec::new();
        let mut current_width_pixel_backup: Vec<f64> = Vec::new();

        loop {
            let mut promotions = 0i32;
            // Start promotion for all nodes with incoming edges.
            for i in 0..self.nodes_with_incoming_edges.len() {
                let node = self.nodes_with_incoming_edges[i];
                let (dummydiff, max_width_not_exceeded) = self.promote_node(a, node);

                // NIKOLOV and NIKOLOV_PIXEL can cancel a legitimate promotion
                // because of an exceeding of the maximal accepted width.
                let mut apply = true;
                if self.promotion_strategy == NodePromotionStrategy::NIKOLOV
                    || self.promotion_strategy == NodePromotionStrategy::NIKOLOV_PIXEL
                {
                    apply = max_width_not_exceeded;
                }

                if dummydiff < 0 && apply {
                    // Promotion is valid and will be applied.
                    promotions += 1;
                    layering_backup = self.layers.clone();
                    self.dummy_node_count += dummydiff;
                    reduced_dummies += dummy_backup - self.dummy_node_count;
                    dummy_backup = self.dummy_node_count + dummydiff;
                    height_backup = self.max_height;
                    current_width_backup = self.current_width.clone();
                    current_width_pixel_backup = self.current_width_pixel.clone();
                    width_backup_is_alias = false;
                } else {
                    // Promotion is invalid; restore the last valid state.
                    self.layers = layering_backup.clone();
                    self.dummy_node_count = dummy_backup;
                    if width_backup_is_alias {
                        current_width_backup = self.current_width.clone();
                        current_width_pixel_backup = self.current_width_pixel.clone();
                        width_backup_is_alias = false;
                    } else {
                        self.current_width = current_width_backup.clone();
                        self.current_width_pixel = current_width_pixel_backup.clone();
                    }
                    self.max_height = height_backup;
                }
            }
            iteration_counter += 1;
            let promotion_flag =
                promotions != 0 && funky(reduced_dummies, iteration_counter);
            if !promotion_flag {
                break;
            }
        }
    }

    /// Returns (estimated difference of dummy
    /// nodes, whether the maximal accepted width has NOT been exceeded).
    fn promote_node(&mut self, a: &LGraphArena, node: LNodeId) -> (i32, bool) {
        let mut max_width_not_exceeded = true;
        let mut dummydiff = 0i32;
        let node_id = a.node(node).id as usize;
        let mut node_layer_pos = self.layers[node_id];
        let node_size = a.node(node).size.y + self.node_size_affix;
        let dummies_built = self.degree_diff[node_id][2];

        // Update the width of the layer the node came from.
        self.current_width[node_layer_pos as usize] += -1 + dummies_built;
        self.current_width_pixel[node_layer_pos as usize] +=
            -node_size + dummies_built as f64 * self.dummy_size;

        // Calculate index of the layer for the promoted node.
        node_layer_pos += 1;
        if node_layer_pos >= self.max_height {
            self.max_height += 1;
            self.current_width.push(1);
            self.current_width_pixel.push(node_size);
        } else {
            // Update the width of the layer the node is promoted to.
            let dummies_reduced = self.degree_diff[node_id][1];
            self.current_width[node_layer_pos as usize] += 1 - dummies_reduced;
            self.current_width_pixel[node_layer_pos as usize] +=
                node_size - dummies_reduced as f64 * self.dummy_size;
        }

        // Check whether the promotion exceeds the max width of the previous
        // layer or the new layer.
        if (self.promotion_strategy == NodePromotionStrategy::NIKOLOV
            && (self.current_width[node_layer_pos as usize] > self.max_width
                || self.current_width[(node_layer_pos - 1) as usize] > self.max_width))
            || (self.promotion_strategy == NodePromotionStrategy::NIKOLOV_PIXEL
                && (self.current_width_pixel[node_layer_pos as usize] > self.max_width_pixel
                    || self.current_width_pixel[(node_layer_pos - 1) as usize]
                        > self.max_width_pixel))
        {
            max_width_not_exceeded = false;
        }

        // Promote preceding nodes in the above neighboring layer recursively
        // and calculate the difference of dummy nodes.
        for edge in a.node_incoming_edges(node) {
            let master_node = a.edge_source_node(edge);
            if self.layers[a.node(master_node).id as usize] == node_layer_pos {
                let (diff, not_exceeded) = self.promote_node(a, master_node);
                dummydiff += diff;
                max_width_not_exceeded = max_width_not_exceeded && not_exceeded;
            }
        }

        self.layers[node_id] = node_layer_pos;
        dummydiff += self.degree_diff[node_id][0];

        (dummydiff, max_width_not_exceeded)
    }

    fn set_new_layering(&mut self, a: &mut LGraphArena, graph: LGraphId) {
        // Create maxHeight + 1 layers with reversed IDs.
        let mut lay_list: Vec<LayerId> = Vec::new();
        for i in 0..=self.max_height {
            let la_la_layer = a.create_layer(graph);
            a.layer_mut(la_la_layer).id = self.max_height - i;
            lay_list.push(la_la_layer);
        }

        // Assign all nodes to the beforehand created (laLa)layers.
        for &node in &self.nodes {
            let layer = lay_list[(self.max_height - self.layers[a.node(node).id as usize]) as usize];
            a.node_set_layer(node, Some(layer));
        }

        // Exterminate all layers that don't contain any nodes.
        lay_list.retain(|&layer| !a.layer(layer).nodes.is_empty());

        a.graph_mut(graph).layers.clear();
        a.graph_mut(graph).layers.extend(lay_list);
    }

    fn set_new_layering_model_order(&mut self, a: &mut LGraphArena, graph: LGraphId) {
        a.graph_mut(graph).layers.clear();
        // Get the layer indices, sorted.
        let mut key_set = self.bi_layer_map.keys.clone();
        key_set.sort();
        let mut layer_list: Vec<LayerId> = Vec::new();
        for layer_index in key_set {
            let layer_nodes = self.bi_layer_map.get_values(layer_index);
            if !layer_nodes.is_empty() {
                let new_layer = a.create_layer(graph);
                layer_list.push(new_layer);
                a.layer_mut(new_layer).id = layer_index;
                for node in layer_nodes {
                    a.node_set_layer(node, Some(new_layer));
                }
            }
        }
        // Apply new layering.
        a.graph_mut(graph).layers.extend(layer_list);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alg_layered::graph::LEdgeId;

    fn make_node(a: &mut LGraphArena, layer: LayerId) -> LNodeId {
        let g = a.layer(layer).graph.unwrap();
        let n = a.create_node(g);
        a.node_mut(n).size.y = 20.0;
        a.node_set_layer(n, Some(layer));
        n
    }

    fn connect(a: &mut LGraphArena, source: LNodeId, target: LNodeId) -> LEdgeId {
        let sp = a.create_port();
        a.port_set_node(sp, Some(source));
        let tp = a.create_port();
        a.port_set_node(tp, Some(target));
        let e = a.create_edge();
        a.edge_set_source(e, Some(sp));
        a.edge_set_target(e, Some(tp));
        e
    }

    /// Layers: [a, b] [c] [d] with edges a->c and b->d; b->d spans layer 1
    /// (creates a dummy). Promoting d into c's layer removes that dummy
    /// (hand-traced).
    #[test]
    fn promotes_node_to_reduce_dummies() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        a.graph(g)
            .properties
            .set(&lopts::LAYERING_NODE_PROMOTION_STRATEGY, NodePromotionStrategy::NIKOLOV);
        let l0 = a.create_layer(g);
        let l1 = a.create_layer(g);
        let l2 = a.create_layer(g);
        a.graph_mut(g).layers.extend([l0, l1, l2]);
        let na = make_node(&mut a, l0);
        let nb = make_node(&mut a, l0);
        let nc = make_node(&mut a, l1);
        let nd = make_node(&mut a, l2);
        connect(&mut a, na, nc);
        connect(&mut a, nb, nd);

        process(&mut a, g).unwrap();

        // d must have been promoted next to c, leaving two layers
        let layer_of = |a: &LGraphArena, n: LNodeId| {
            a.graph(g)
                .layers
                .iter()
                .position(|&l| Some(l) == a.node(n).layer)
                .unwrap()
        };
        assert_eq!(a.graph(g).layers.len(), 2);
        assert_eq!(layer_of(&a, na), 0);
        assert_eq!(layer_of(&a, nb), 0);
        assert_eq!(layer_of(&a, nc), 1);
        assert_eq!(layer_of(&a, nd), 1);
    }
}
