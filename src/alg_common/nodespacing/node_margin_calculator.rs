
use crate::core::adapters::AdapterGraph;
use crate::core::options::{self, EdgeLabelPlacement, PortLabelPlacement, PortSide};
use crate::graph::math::{ElkRectangle, KVector};

/// Sets the node margins. Node margins are
/// influenced by both port positions and sizes and label positions and sizes.
///
/// Preconditions: ports have fixed port positions, labels have fixed
/// positions. Postcondition: the node margins form a bounding box around the
/// node and its ports and labels.
pub struct NodeMarginCalculator {
    include_labels: bool,
    include_ports: bool,
    include_port_labels: bool,
    include_edge_head_tail_labels: bool,
}

impl Default for NodeMarginCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeMarginCalculator {
    /// The constructor (the graph adapter is passed to the individual
    /// methods instead of being stored).
    pub fn new() -> Self {
        NodeMarginCalculator {
            include_labels: true,
            include_ports: true,
            include_port_labels: true,
            include_edge_head_tail_labels: true,
        }
    }

    pub fn exclude_labels(mut self) -> Self {
        self.include_labels = false;
        self
    }

    pub fn exclude_ports(mut self) -> Self {
        self.include_ports = false;
        self
    }

    pub fn exclude_port_labels(mut self) -> Self {
        self.include_port_labels = false;
        self
    }

    pub fn exclude_edge_head_tail_labels(mut self) -> Self {
        self.include_edge_head_tail_labels = false;
        self
    }

    /// Calculates and assigns margins to all nodes.
    pub fn process<G: AdapterGraph>(&self, g: &mut G) {
        let spacing = g.graph_properties().get(&options::SPACING_LABEL_NODE);

        // Iterate through all nodes
        for node in g.nodes() {
            self.process_node_with_spacing(g, node, spacing);
        }
    }

    /// Calculates and assigns margins to
    /// the given node.
    pub fn process_node<G: AdapterGraph>(&self, g: &mut G, node: G::N) {
        let spacing = g.graph_properties().get(&options::SPACING_LABEL_NODE);
        self.process_node_with_spacing(g, node, spacing);
    }

    fn process_node_with_spacing<G: AdapterGraph>(&self, g: &mut G, node: G::N, label_spacing: f64) {
        // This will be our bounding box. We'll start with one that's the same
        // size as our node, and at the same position.
        let node_pos = g.node_position(node);
        let node_size = g.node_size(node);
        let mut bounding_box =
            ElkRectangle::new(node_pos.x, node_pos.y, node_size.x, node_size.y);

        // We'll reuse this rectangle as our box for elements to add to the bounding box
        let mut element_box = ElkRectangle::default();

        // Put the node's labels into the bounding box
        if self.include_labels {
            for label in g.node_labels(node) {
                let label_pos = g.label_position(label);
                let label_size = g.label_size(label);
                element_box.x = label_pos.x + node_pos.x;
                element_box.y = label_pos.y + node_pos.y;
                element_box.width = label_size.x;
                element_box.height = label_size.y;

                bounding_box.union(&element_box);
            }
        }

        // Do the same for ports and their labels
        for port in g.node_ports(node) {
            // Calculate the port's upper left corner's x and y coordinate
            let port_pos = g.port_position(port);
            let port_x = port_pos.x + node_pos.x;
            let port_y = port_pos.y + node_pos.y;

            // The port itself
            if self.include_ports {
                let port_size = g.port_size(port);
                element_box.x = port_x;
                element_box.y = port_y;
                element_box.width = port_size.x;
                element_box.height = port_size.y;

                bounding_box.union(&element_box);
            }

            // The port's labels
            if self.include_port_labels {
                for label in g.port_labels(port) {
                    let label_pos = g.label_position(label);
                    let label_size = g.label_size(label);
                    element_box.x = label_pos.x + port_x;
                    element_box.y = label_pos.y + port_y;
                    element_box.width = label_size.x;
                    element_box.height = label_size.y;

                    bounding_box.union(&element_box);
                }
            }

            // End labels of edges connected to the port
            if self.include_edge_head_tail_labels {
                let mut required_port_label_space = KVector::new(-label_spacing, -label_spacing);

                // TODO: maybe leave space for manually placed ports
                if g.node_properties(node)
                    .get(&options::PORT_LABELS_PLACEMENT)
                    .contains(PortLabelPlacement::OUTSIDE)
                {
                    for label in g.port_labels(port) {
                        let label_size = g.label_size(label);
                        required_port_label_space.x += label_size.x + label_spacing;
                        required_port_label_space.y += label_size.y + label_spacing;
                    }
                }

                required_port_label_space.x = required_port_label_space.x.max(0.0);
                required_port_label_space.y = required_port_label_space.y.max(0.0);

                self.process_edge_head_tail_labels(
                    g,
                    &mut bounding_box,
                    &g.port_outgoing_edges(port),
                    &g.port_incoming_edges(port),
                    node,
                    Some(port),
                    Some(required_port_label_space),
                    label_spacing,
                );
            }
        }

        // Process end labels of edges directly connected to the node
        if self.include_edge_head_tail_labels {
            self.process_edge_head_tail_labels(
                g,
                &mut bounding_box,
                &g.node_outgoing_edges(node),
                &g.node_incoming_edges(node),
                node,
                None,
                None,
                label_spacing,
            );
        }

        // Reset the margin (guard against very small double precision errors
        // which can cause the results to be small negative values, which
        // doesn't make sense -- see #616)
        let mut margin = g.node_margin(node);
        margin.top = 0.0f64.max(node_pos.y - bounding_box.y);
        margin.bottom =
            0.0f64.max(bounding_box.y + bounding_box.height - (node_pos.y + node_size.y));
        margin.left = 0.0f64.max(node_pos.x - bounding_box.x);
        margin.right =
            0.0f64.max(bounding_box.x + bounding_box.width - (node_pos.x + node_size.x));
        g.set_node_margin(node, margin);
    }

    #[allow(clippy::too_many_arguments)]
    fn process_edge_head_tail_labels<G: AdapterGraph>(
        &self,
        g: &G,
        bounding_box: &mut ElkRectangle,
        outgoing_edges: &[G::E],
        incoming_edges: &[G::E],
        node: G::N,
        port: Option<G::P>,
        port_label_space: Option<KVector>,
        label_spacing: f64,
    ) {
        let mut label_box = ElkRectangle::default();

        // For each edge, the tail labels of outgoing edges ...
        for &edge in outgoing_edges {
            for label in g.edge_labels(edge) {
                if g.label_properties(label).get(&options::EDGE_LABELS_PLACEMENT)
                    == EdgeLabelPlacement::TAIL
                {
                    self.compute_label_box(
                        g,
                        &mut label_box,
                        label,
                        false,
                        node,
                        port,
                        port_label_space,
                        label_spacing,
                    );
                    bounding_box.union(&label_box);
                }
            }
        }

        // ... and the head label of incoming edges shall be considered
        for &edge in incoming_edges {
            for label in g.edge_labels(edge) {
                if g.label_properties(label).get(&options::EDGE_LABELS_PLACEMENT)
                    == EdgeLabelPlacement::HEAD
                {
                    self.compute_label_box(
                        g,
                        &mut label_box,
                        label,
                        true,
                        node,
                        port,
                        port_label_space,
                        label_spacing,
                    );
                    bounding_box.union(&label_box);
                }
            }
        }
    }

    /// Computes the given edge label's bounding
    /// box. The position of the box is just a rough estimate.
    #[allow(clippy::too_many_arguments)]
    fn compute_label_box<G: AdapterGraph>(
        &self,
        g: &G,
        label_box: &mut ElkRectangle,
        label: G::L,
        incoming_edge: bool,
        node: G::N,
        port: Option<G::P>,
        port_label_space: Option<KVector>,
        label_spacing: f64,
    ) {
        let node_pos = g.node_position(node);
        label_box.x = node_pos.x;
        label_box.y = node_pos.y;
        if let Some(port) = port {
            let port_pos = g.port_position(port);
            label_box.x += port_pos.x;
            label_box.y += port_pos.y;
        }

        let label_size = g.label_size(label);
        label_box.width = label_size.x;
        label_box.height = label_size.y;

        match port {
            None => {
                // The edge is connected directly to the node
                if incoming_edge {
                    // Assume the edge enters the node at its western side
                    label_box.x -= label_spacing + label_size.x;
                } else {
                    // Assume the edge leaves the node at its eastern side
                    label_box.x += g.node_size(node).x + label_spacing;
                }
            }
            Some(port) => {
                let port_label_space = port_label_space.unwrap_or_default();
                match g.port_side(port) {
                    PortSide::UNDEFINED | PortSide::EAST => {
                        label_box.x += g.port_size(port).x
                            + label_spacing
                            + port_label_space.x
                            + label_spacing;
                    }
                    PortSide::WEST => {
                        label_box.x -= label_spacing
                            + port_label_space.x
                            + label_spacing
                            + label_size.x;
                    }
                    PortSide::NORTH => {
                        label_box.x += g.port_size(port).x + label_spacing;
                        label_box.y -= label_spacing
                            + port_label_space.y
                            + label_spacing
                            + label_size.y;
                    }
                    PortSide::SOUTH => {
                        label_box.x += g.port_size(port).x + label_spacing;
                        label_box.y += g.port_size(port).y
                            + label_spacing
                            + port_label_space.y
                            + label_spacing;
                    }
                }
            }
        }
    }
}
