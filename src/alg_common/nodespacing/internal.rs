//! The data
//! holders (`NodeContext`, `PortContext`) and `NodeLabelLocation`.

use crate::core::adapters::AdapterGraph;
use crate::core::options::{
    NodeLabelPlacement, PortAlignment, PortConstraints, PortLabelPlacement, PortSide,
    SizeConstraint, SizeOptions,
};
use crate::graph::math::{ElkMargin, ElkPadding, KVector};
use crate::graph::properties::{EnumSet, JavaCloneable, PropValue, Property};

use super::cellsystem::{
    CellId, CellSystem, ContainerArea, HorizontalLabelAlignment, Strip, VerticalLabelAlignment,
};

/// Enumeration over all possible label
/// placements and associated things.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(non_camel_case_types)]
pub enum NodeLabelLocation {
    /// Outside top left.
    OUT_T_L,
    /// Outside top center.
    OUT_T_C,
    /// Outside top right.
    OUT_T_R,
    /// Outside bottom left.
    OUT_B_L,
    /// Outside bottom center.
    OUT_B_C,
    /// Outside bottom right.
    OUT_B_R,
    /// Outside left top.
    OUT_L_T,
    /// Outside left center.
    OUT_L_C,
    /// Outside left bottom.
    OUT_L_B,
    /// Outside right top.
    OUT_R_T,
    /// Outside right center.
    OUT_R_C,
    /// Outside right bottom.
    OUT_R_B,
    /// Inside top left.
    IN_T_L,
    /// Inside top center.
    IN_T_C,
    /// Inside top right.
    IN_T_R,
    /// Inside center left.
    IN_C_L,
    /// Inside center center.
    IN_C_C,
    /// Inside center right.
    IN_C_R,
    /// Inside bottom left.
    IN_B_L,
    /// Inside bottom center.
    IN_B_C,
    /// Inside bottom right.
    IN_B_R,
    /// Undefined or not decidable.
    UNDEFINED,
}

use NodeLabelLocation::*;

impl NodeLabelLocation {
    /// All locations in declaration order.
    pub const VALUES: [NodeLabelLocation; 22] = [
        OUT_T_L, OUT_T_C, OUT_T_R, OUT_B_L, OUT_B_C, OUT_B_R, OUT_L_T, OUT_L_C, OUT_L_B, OUT_R_T,
        OUT_R_C, OUT_R_B, IN_T_L, IN_T_C, IN_T_R, IN_C_L, IN_C_C, IN_C_R, IN_B_L, IN_B_C, IN_B_R,
        UNDEFINED,
    ];

    pub fn ordinal(self) -> usize {
        NodeLabelLocation::VALUES.iter().position(|&l| l == self).unwrap()
    }

    /// The `NodeLabelPlacement` sets that map to this location. Built on the
    /// fly; the sets are tiny.
    fn assigned_placements(self) -> Vec<EnumSet<NodeLabelPlacement>> {
        use NodeLabelPlacement as P;
        // (outside?, base placements, with-priority variant?)
        let of = |values: &[P]| EnumSet::of(values);
        match self {
            OUT_T_L => vec![of(&[P::OUTSIDE, P::V_TOP, P::H_LEFT])],
            OUT_T_C => vec![
                of(&[P::OUTSIDE, P::V_TOP, P::H_CENTER]),
                of(&[P::OUTSIDE, P::V_TOP, P::H_CENTER, P::H_PRIORITY]),
            ],
            OUT_T_R => vec![of(&[P::OUTSIDE, P::V_TOP, P::H_RIGHT])],
            OUT_B_L => vec![of(&[P::OUTSIDE, P::V_BOTTOM, P::H_LEFT])],
            OUT_B_C => vec![
                of(&[P::OUTSIDE, P::V_BOTTOM, P::H_CENTER]),
                of(&[P::OUTSIDE, P::V_BOTTOM, P::H_CENTER, P::H_PRIORITY]),
            ],
            OUT_B_R => vec![of(&[P::OUTSIDE, P::V_BOTTOM, P::H_RIGHT])],
            OUT_L_T => vec![of(&[P::OUTSIDE, P::H_LEFT, P::V_TOP, P::H_PRIORITY])],
            OUT_L_C => vec![
                of(&[P::OUTSIDE, P::H_LEFT, P::V_CENTER]),
                of(&[P::OUTSIDE, P::H_LEFT, P::V_CENTER, P::H_PRIORITY]),
            ],
            OUT_L_B => vec![of(&[P::OUTSIDE, P::H_LEFT, P::V_BOTTOM, P::H_PRIORITY])],
            OUT_R_T => vec![of(&[P::OUTSIDE, P::H_RIGHT, P::V_TOP, P::H_PRIORITY])],
            OUT_R_C => vec![
                of(&[P::OUTSIDE, P::H_RIGHT, P::V_CENTER]),
                of(&[P::OUTSIDE, P::H_RIGHT, P::V_CENTER, P::H_PRIORITY]),
            ],
            OUT_R_B => vec![of(&[P::OUTSIDE, P::H_RIGHT, P::V_BOTTOM, P::H_PRIORITY])],
            IN_T_L => vec![
                of(&[P::INSIDE, P::V_TOP, P::H_LEFT]),
                of(&[P::INSIDE, P::V_TOP, P::H_LEFT, P::H_PRIORITY]),
            ],
            IN_T_C => vec![
                of(&[P::INSIDE, P::V_TOP, P::H_CENTER]),
                of(&[P::INSIDE, P::V_TOP, P::H_CENTER, P::H_PRIORITY]),
            ],
            IN_T_R => vec![
                of(&[P::INSIDE, P::V_TOP, P::H_RIGHT]),
                of(&[P::INSIDE, P::V_TOP, P::H_RIGHT, P::H_PRIORITY]),
            ],
            IN_C_L => vec![
                of(&[P::INSIDE, P::V_CENTER, P::H_LEFT]),
                of(&[P::INSIDE, P::V_CENTER, P::H_LEFT, P::H_PRIORITY]),
            ],
            IN_C_C => vec![
                of(&[P::INSIDE, P::V_CENTER, P::H_CENTER]),
                of(&[P::INSIDE, P::V_CENTER, P::H_CENTER, P::H_PRIORITY]),
            ],
            IN_C_R => vec![
                of(&[P::INSIDE, P::V_CENTER, P::H_RIGHT]),
                of(&[P::INSIDE, P::V_CENTER, P::H_RIGHT, P::H_PRIORITY]),
            ],
            IN_B_L => vec![
                of(&[P::INSIDE, P::V_BOTTOM, P::H_LEFT]),
                of(&[P::INSIDE, P::V_BOTTOM, P::H_LEFT, P::H_PRIORITY]),
            ],
            IN_B_C => vec![
                of(&[P::INSIDE, P::V_BOTTOM, P::H_CENTER]),
                of(&[P::INSIDE, P::V_BOTTOM, P::H_CENTER, P::H_PRIORITY]),
            ],
            IN_B_R => vec![
                of(&[P::INSIDE, P::V_BOTTOM, P::H_RIGHT]),
                of(&[P::INSIDE, P::V_BOTTOM, P::H_RIGHT, P::H_PRIORITY]),
            ],
            UNDEFINED => vec![],
        }
    }

    pub fn from_node_label_placement(
        label_placement: EnumSet<NodeLabelPlacement>,
    ) -> NodeLabelLocation {
        for location in NodeLabelLocation::VALUES {
            if location.assigned_placements().contains(&label_placement) {
                return location;
            }
        }
        UNDEFINED
    }

    pub fn horizontal_alignment(self) -> HorizontalLabelAlignment {
        use HorizontalLabelAlignment as H;
        match self {
            OUT_T_L | OUT_B_L | OUT_R_T | OUT_R_C | OUT_R_B | IN_T_L | IN_C_L | IN_B_L => H::Left,
            OUT_T_C | OUT_B_C | IN_T_C | IN_C_C | IN_B_C => H::Center,
            OUT_T_R | OUT_B_R | OUT_L_T | OUT_L_C | OUT_L_B | IN_T_R | IN_C_R | IN_B_R => H::Right,
            UNDEFINED => panic!("UNDEFINED node label location has no alignment"),
        }
    }

    pub fn vertical_alignment(self) -> VerticalLabelAlignment {
        use VerticalLabelAlignment as V;
        match self {
            OUT_B_L | OUT_B_C | OUT_B_R | OUT_L_T | OUT_R_T | IN_T_L | IN_T_C | IN_T_R => V::Top,
            OUT_L_C | OUT_R_C | IN_C_L | IN_C_C | IN_C_R => V::Center,
            OUT_T_L | OUT_T_C | OUT_T_R | OUT_L_B | OUT_R_B | IN_B_L | IN_B_C | IN_B_R => V::Bottom,
            UNDEFINED => panic!("UNDEFINED node label location has no alignment"),
        }
    }

    pub fn container_row(self) -> ContainerArea {
        match self {
            OUT_T_L | OUT_T_C | OUT_T_R | OUT_L_T | OUT_R_T | IN_T_L | IN_T_C | IN_T_R => {
                ContainerArea::Begin
            }
            OUT_L_C | OUT_R_C | IN_C_L | IN_C_C | IN_C_R => ContainerArea::Center,
            OUT_B_L | OUT_B_C | OUT_B_R | OUT_L_B | OUT_R_B | IN_B_L | IN_B_C | IN_B_R => {
                ContainerArea::End
            }
            UNDEFINED => panic!("UNDEFINED node label location has no container row"),
        }
    }

    pub fn container_column(self) -> ContainerArea {
        match self {
            OUT_T_L | OUT_B_L | OUT_L_T | OUT_L_C | OUT_L_B | IN_T_L | IN_C_L | IN_B_L => {
                ContainerArea::Begin
            }
            OUT_T_C | OUT_B_C | IN_T_C | IN_C_C | IN_B_C => ContainerArea::Center,
            OUT_T_R | OUT_B_R | OUT_R_T | OUT_R_C | OUT_R_B | IN_T_R | IN_C_R | IN_B_R => {
                ContainerArea::End
            }
            UNDEFINED => panic!("UNDEFINED node label location has no container column"),
        }
    }

    pub fn is_inside_location(self) -> bool {
        matches!(
            self,
            IN_T_L | IN_T_C | IN_T_R | IN_C_L | IN_C_C | IN_C_R | IN_B_L | IN_B_C | IN_B_R
        )
    }

    pub fn outside_side(self) -> PortSide {
        match self {
            OUT_T_L | OUT_T_C | OUT_T_R => PortSide::NORTH,
            OUT_B_L | OUT_B_C | OUT_B_R => PortSide::SOUTH,
            OUT_L_T | OUT_L_C | OUT_L_B => PortSide::WEST,
            OUT_R_T | OUT_R_C | OUT_R_B => PortSide::EAST,
            _ => PortSide::UNDEFINED,
        }
    }
}

/// Data holder for a single port.
pub struct PortContext<P> {
    /// The port we calculate stuff for.
    pub port: P,
    /// The port's side. It is immutable during the algorithm, so we cache it
    /// here.
    pub side: PortSide,
    /// The volatile id assigned by the port context creator, used for the
    /// sort order.
    pub volatile_id: usize,
    /// The port's position, to be modified by the algorithm and possibly
    /// applied later.
    pub port_position: KVector,
    /// Whether the port's labels need to be placed next to the port.
    pub labels_next_to_port: bool,
    /// Margin around the port to assume when placing the port.
    pub port_margin: ElkMargin,
    /// The cell we place our port labels in.
    pub port_label_cell: Option<CellId>,
}

impl<P: Copy> PortContext<P> {
    /// The `PortContext` constructor (minus the label cell, which the
    /// port context creator sets up).
    pub fn new<G: AdapterGraph<P = P>>(
        g: &G,
        node_ctx_placement: EnumSet<PortLabelPlacement>,
        treat_as_compound_node: bool,
        port: P,
        volatile_id: usize,
    ) -> Self {
        let port_labels_next_to_port =
            node_ctx_placement.contains(PortLabelPlacement::NEXT_TO_PORT_IF_POSSIBLE);

        // Whether labels are supposed to be placed next to their port is
        // determined differently depending on whether they are to be placed
        // inside or outside
        let labels_next_to_port = if node_ctx_placement.contains(PortLabelPlacement::INSIDE) {
            if treat_as_compound_node {
                // There might be connections to the inside. That means that we may
                // want to place port labels next to their port, if possible
                port_labels_next_to_port && !g.port_has_compound_connections(port)
            } else {
                true
            }
        } else if node_ctx_placement.contains(PortLabelPlacement::OUTSIDE) {
            if port_labels_next_to_port {
                // We can place a label next to an outside port if that port
                // doesn't have incident edges
                g.port_incoming_edges(port).is_empty() && g.port_outgoing_edges(port).is_empty()
            } else {
                false
            }
        } else {
            false
        };

        PortContext {
            port,
            side: g.port_side(port),
            volatile_id,
            port_position: g.port_position(port),
            labels_next_to_port,
            port_margin: ElkMargin::default(),
            port_label_cell: None,
        }
    }
}

pub fn individual_or_inherited<G: AdapterGraph, T: PropValue + Clone + Default + JavaCloneable>(
    g: &G,
    node: G::N,
    property: &Property<T>,
) -> T {
    if let Some(individual) = g
        .node_properties(node)
        .try_get(&crate::core::options::SPACING_INDIVIDUAL)
    {
        if individual.properties.has(property) {
            if let Some(value) = individual.properties.try_get(property) {
                return value;
            }
        }
    }
    // Use the common value; falls back to the property default if the
    // node has no graph, which `get` does implicitly.
    g.graph_properties().get(property)
}

/// Data holder passed around the algorithm. The cell
/// system lives in the `cells` arena; cells are referenced by [`CellId`].
pub struct NodeContext<G: AdapterGraph> {
    /// The node we calculate stuff for.
    pub node: G::N,
    /// The node's size, applied (or not) once the algorithm is finished.
    pub node_size: KVector,
    /// Whether this node has stuff inside it or not.
    pub treat_as_compound_node: bool,
    /// The node's size constraints.
    pub size_constraints: EnumSet<SizeConstraint>,
    /// The node's size options.
    pub size_options: EnumSet<SizeOptions>,
    /// Port constraints set on the node.
    pub port_constraints: PortConstraints,
    /// Whether port labels are placed inside or outside.
    pub port_labels_placement: EnumSet<PortLabelPlacement>,
    /// Whether to treat port labels as a group when centering them.
    pub port_labels_treat_as_group: bool,
    /// Where node labels are placed by default.
    pub node_label_placement: EnumSet<NodeLabelPlacement>,
    /// Space to leave around the node label area.
    pub node_labels_padding: ElkPadding,
    /// Space between a node and its outside labels.
    pub node_label_spacing: f64,
    /// Space between two labels.
    pub label_label_spacing: f64,
    /// Space between two different label cells.
    pub label_cell_spacing: f64,
    /// Space between a port and another port.
    pub port_port_spacing: f64,
    /// Horizontal space between a port and its labels.
    pub port_label_spacing_horizontal: f64,
    /// Vertical space between a port and its labels.
    pub port_label_spacing_vertical: f64,
    /// Margin to leave around the set of ports on each side. (Never absent,
    /// since the property has a default.)
    pub surrounding_port_margins: ElkMargin,
    /// Whether node is being laid out in top-down layout mode.
    pub topdown_layout: bool,

    /// Port contexts, ordered grouped by port side (NORTH, EAST, SOUTH,
    /// WEST), within each side sorted left-to-right / top-to-bottom.
    pub port_contexts: Vec<PortContext<G::P>>,

    /// The arena holding the cell system.
    pub cells: CellSystem<G::L>,
    /// The main cell that holds all the cells that make up the node.
    pub node_container: CellId,
    /// The main cell's middle row, which will contain further cells.
    pub node_container_middle_row: CellId,
    /// The grid container for inside node labels (and the client area).
    /// Created by the node label cell creator.
    pub inside_node_label_container: Option<CellId>,
    /// Cells describing the space required for ports and inside port labels,
    /// indexed by `PortSide` ordinal.
    pub inside_port_label_cells: [Option<CellId>; 5],
    /// Container cells holding label cells for outside node labels, indexed
    /// by `PortSide` ordinal.
    pub outside_node_label_containers: [Option<CellId>; 5],
    /// Label cells created for node labels, indexed by `NodeLabelLocation`
    /// ordinal (iteration is in ordinal order).
    pub node_label_cells: [Option<CellId>; 22],
}

impl<G: AdapterGraph> NodeContext<G> {
    /// The `NodeContext` constructor. Spacing lookups go through the node's
    /// graph, which corresponds to `g` here.
    pub fn new(g: &G, node: G::N) -> Self {
        use crate::core::options as opts;

        let node_size = g.node_size(node);
        let node_props = g.node_properties(node);

        // Top-down layout
        let topdown_layout = node_props.get(&opts::TOPDOWN_LAYOUT);

        // Compound node
        let treat_as_compound_node =
            g.is_compound_node(node) || node_props.get(&opts::INSIDE_SELF_LOOPS_ACTIVATE);

        // Core size settings
        let size_constraints = node_props.get(&opts::NODE_SIZE_CONSTRAINTS);
        let size_options = node_props.get(&opts::NODE_SIZE_OPTIONS);
        let port_constraints = node_props.get(&opts::PORT_CONSTRAINTS);
        let port_labels_placement = node_props.get(&opts::PORT_LABELS_PLACEMENT);
        if !PortLabelPlacement::is_valid(port_labels_placement) {
            panic!("Invalid port label placement: {:?}", port_labels_placement);
        }

        let port_labels_treat_as_group = node_props.get(&opts::PORT_LABELS_TREAT_AS_GROUP);
        let node_label_placement = node_props.get(&opts::NODE_LABELS_PLACEMENT);
        if !NodeLabelPlacement::is_valid(node_label_placement) {
            panic!("Invalid node label placement: {:?}", node_label_placement);
        }

        // Copy spacings for convenience
        let node_labels_padding = individual_or_inherited(g, node, &opts::NODE_LABELS_PADDING);
        let node_label_spacing = individual_or_inherited(g, node, &opts::SPACING_LABEL_NODE);
        let label_label_spacing = individual_or_inherited(g, node, &opts::SPACING_LABEL_LABEL);
        let port_port_spacing = individual_or_inherited(g, node, &opts::SPACING_PORT_PORT);
        let port_label_spacing_horizontal =
            individual_or_inherited(g, node, &opts::SPACING_LABEL_PORT_HORIZONTAL);
        let port_label_spacing_vertical =
            individual_or_inherited(g, node, &opts::SPACING_LABEL_PORT_VERTICAL);
        let surrounding_port_margins =
            individual_or_inherited(g, node, &opts::SPACING_PORTS_SURROUNDING);

        let label_cell_spacing = 2.0 * label_label_spacing;

        // Create main cells (the others will be created later)
        let symmetry = !size_options.contains(SizeOptions::ASYMMETRICAL);
        let mut cells = CellSystem::new();
        let node_container = cells.new_strip(Strip::Vertical, symmetry, 0.0);
        let node_container_middle_row = cells.new_strip(Strip::Horizontal, symmetry, 0.0);
        cells.strip_set_cell(node_container, ContainerArea::Center, node_container_middle_row);

        NodeContext {
            node,
            node_size,
            treat_as_compound_node,
            size_constraints,
            size_options,
            port_constraints,
            port_labels_placement,
            port_labels_treat_as_group,
            node_label_placement,
            node_labels_padding,
            node_label_spacing,
            label_label_spacing,
            label_cell_spacing,
            port_port_spacing,
            port_label_spacing_horizontal,
            port_label_spacing_vertical,
            surrounding_port_margins,
            topdown_layout,
            port_contexts: Vec::new(),
            cells,
            node_container,
            node_container_middle_row,
            inside_node_label_container: None,
            inside_port_label_cells: [None; 5],
            outside_node_label_containers: [None; 5],
            node_label_cells: [None; 22],
        }
    }

    pub fn apply_node_size(&self, g: &mut G) {
        g.set_node_size(self.node, self.node_size);
    }

    /// Indices into `port_contexts` for the contexts on the given side
    /// (contiguous because the vector is sorted by side).
    pub fn ports_on_side(&self, side: PortSide) -> std::ops::Range<usize> {
        let start = self
            .port_contexts
            .iter()
            .position(|pc| pc.side == side)
            .unwrap_or(self.port_contexts.len());
        let mut end = start;
        while end < self.port_contexts.len() && self.port_contexts[end].side == side {
            end += 1;
        }
        start..end
    }

    /// The inside port label cell for the given side (always present after
    /// `InsidePortLabelCellCreator` has run).
    pub fn inside_port_label_cell(&self, side: PortSide) -> CellId {
        self.inside_port_label_cells[side as usize].expect("inside port label cells not created")
    }

    /// The outside node label container for the given side.
    pub fn outside_node_label_container(&self, side: PortSide) -> CellId {
        self.outside_node_label_containers[side as usize]
            .expect("outside node label containers not created")
    }

    pub fn get_port_alignment(&self, g: &G, port_side: PortSide) -> PortAlignment {
        use crate::core::options as opts;
        let node_props = g.node_properties(self.node);
        let mut alignment: Option<PortAlignment> = None;

        match port_side {
            PortSide::NORTH => {
                if node_props.has(&opts::PORT_ALIGNMENT_NORTH) {
                    alignment = node_props.try_get(&opts::PORT_ALIGNMENT_NORTH);
                }
            }
            PortSide::SOUTH => {
                if node_props.has(&opts::PORT_ALIGNMENT_SOUTH) {
                    alignment = node_props.try_get(&opts::PORT_ALIGNMENT_SOUTH);
                }
            }
            PortSide::EAST => {
                if node_props.has(&opts::PORT_ALIGNMENT_EAST) {
                    alignment = node_props.try_get(&opts::PORT_ALIGNMENT_EAST);
                }
            }
            PortSide::WEST => {
                if node_props.has(&opts::PORT_ALIGNMENT_WEST) {
                    alignment = node_props.try_get(&opts::PORT_ALIGNMENT_WEST);
                }
            }
            PortSide::UNDEFINED => {}
        }

        // Fall back to basic port alignment if we haven't found a more specific one yet
        match alignment {
            Some(a) => a,
            None => node_props.get(&opts::PORT_ALIGNMENT_DEFAULT),
        }
    }

    pub fn sort_port_contexts(&mut self) {
        self.port_contexts.sort_by(|a, b| {
            // Compare port sides by ordinal
            let side_cmp = (a.side as usize).cmp(&(b.side as usize));
            if side_cmp != std::cmp::Ordering::Equal {
                return side_cmp;
            }
            // Ports are numbered clockwise but we want them sorted
            // left-to-right / top-to-bottom
            match a.side {
                PortSide::NORTH | PortSide::EAST => a.volatile_id.cmp(&b.volatile_id),
                PortSide::SOUTH | PortSide::WEST => b.volatile_id.cmp(&a.volatile_id),
                PortSide::UNDEFINED => std::cmp::Ordering::Equal,
            }
        });
    }
}
