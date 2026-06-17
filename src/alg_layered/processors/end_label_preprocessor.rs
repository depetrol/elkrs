//! Puts all edge end (head/tail) labels into
//! label cells stored per node via the `END_LABELS` internal property, and
//! enlarges node margins accordingly.
//!
//! Also contains the Rust model of the nodespacing `LabelCell` as used by the
//! end label machinery, modeled as the value type [`EndLabelCells`], an
//! ordered list of (port, cell) pairs.

use crate::alg_common::nodespacing::cellsystem::{HorizontalLabelAlignment, VerticalLabelAlignment};
use crate::alg_common::overlaps::{OverlapRemovalDirection, RectangleStripOverlapRemover};
use crate::core::adapters::LabelSide;
use crate::core::options::{EdgeLabelPlacement, PortSide};
use crate::graph::math::{ElkRectangle, KVector};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LLabelId, LNodeId, LPortId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::options_gen as lopts;

// ---------------------------------------------------------------- LabelCell

#[derive(Clone, Debug, PartialEq)]
pub struct LabelCell {
    /// Whether we operate in horizontal or vertical layout mode.
    pub horizontal_layout_mode: bool,
    /// Horizontal alignment of labels (default CENTER).
    pub horizontal_alignment: HorizontalLabelAlignment,
    /// Vertical alignment of labels (default CENTER).
    pub vertical_alignment: VerticalLabelAlignment,
    /// The gap inserted between two consecutive labels.
    pub gap: f64,
    /// The labels in this cell.
    pub labels: Vec<LLabelId>,
    /// Minimum space needed to place the labels.
    pub min_content_area_size: KVector,
    /// The cell rectangle (`getCellRectangle()`).
    pub rect: ElkRectangle,
}

impl LabelCell {
    /// `new LabelCell(gap, horizontalLayoutMode)`.
    pub fn new(gap: f64, horizontal_layout_mode: bool) -> Self {
        LabelCell {
            horizontal_layout_mode,
            horizontal_alignment: HorizontalLabelAlignment::Center,
            vertical_alignment: VerticalLabelAlignment::Center,
            gap,
            labels: Vec::new(),
            min_content_area_size: KVector::default(),
            rect: ElkRectangle::default(),
        }
    }

    /// `LabelCell.addLabel`.
    pub fn add_label(&mut self, label: LLabelId, label_size: KVector) {
        self.labels.push(label);

        if self.horizontal_layout_mode {
            self.min_content_area_size.x = self.min_content_area_size.x.max(label_size.x);
            self.min_content_area_size.y += label_size.y;
            if self.labels.len() > 1 {
                self.min_content_area_size.y += self.gap;
            }
        } else {
            self.min_content_area_size.x += label_size.x;
            self.min_content_area_size.y = self.min_content_area_size.y.max(label_size.y);
            if self.labels.len() > 1 {
                self.min_content_area_size.x += self.gap;
            }
        }
    }

    /// `LabelCell.getMinimumWidth` (padding is always zero here).
    pub fn minimum_width(&self) -> f64 {
        self.min_content_area_size.x
    }

    /// `LabelCell.getMinimumHeight` (padding is always zero here).
    pub fn minimum_height(&self) -> f64 {
        self.min_content_area_size.y
    }

    /// `LabelCell.hasLabels`.
    pub fn has_labels(&self) -> bool {
        !self.labels.is_empty()
    }

    /// `LabelCell.applyLabelLayout`.
    pub fn apply_label_layout(&self, a: &mut LGraphArena) {
        if self.horizontal_layout_mode {
            self.apply_horizontal_mode_label_layout(a);
        } else {
            self.apply_vertical_mode_label_layout(a);
        }
    }

    /// `LabelCell.applyHorizontalModeLabelLayout` (padding zero).
    fn apply_horizontal_mode_label_layout(&self, a: &mut LGraphArena) {
        let cell_rect = self.rect;

        // Calculate our starting y coordinate
        let mut y_pos = cell_rect.y;
        if self.vertical_alignment == VerticalLabelAlignment::Center {
            y_pos += (cell_rect.height - self.min_content_area_size.y) / 2.0;
        } else if self.vertical_alignment == VerticalLabelAlignment::Bottom {
            y_pos += cell_rect.height - self.min_content_area_size.y;
        }

        for &label in &self.labels {
            let label_size = a.label(label).size;
            let mut label_pos = KVector::default();

            // Y coordinate
            label_pos.y = y_pos;
            y_pos += label_size.y + self.gap;

            // X coordinate
            match self.horizontal_alignment {
                HorizontalLabelAlignment::Left => {
                    label_pos.x = cell_rect.x;
                }
                HorizontalLabelAlignment::Center => {
                    label_pos.x = cell_rect.x + (cell_rect.width - label_size.x) / 2.0;
                }
                HorizontalLabelAlignment::Right => {
                    label_pos.x = cell_rect.x + cell_rect.width - label_size.x;
                }
            }

            a.label_mut(label).pos = label_pos;
        }
    }

    /// `LabelCell.applyVerticalModeLabelLayout` (padding zero).
    fn apply_vertical_mode_label_layout(&self, a: &mut LGraphArena) {
        let cell_rect = self.rect;

        // Calculate our starting x coordinate
        let mut x_pos = cell_rect.x;
        if self.horizontal_alignment == HorizontalLabelAlignment::Center {
            x_pos += (cell_rect.width - self.min_content_area_size.x) / 2.0;
        } else if self.horizontal_alignment == HorizontalLabelAlignment::Right {
            x_pos += cell_rect.width - self.min_content_area_size.x;
        }

        for &label in &self.labels {
            let label_size = a.label(label).size;
            let mut label_pos = KVector::default();

            // X coordinate
            label_pos.x = x_pos;
            x_pos += label_size.x + self.gap;

            // Y coordinate
            match self.vertical_alignment {
                VerticalLabelAlignment::Top => {
                    label_pos.y = cell_rect.y;
                }
                VerticalLabelAlignment::Center => {
                    label_pos.y = cell_rect.y + (cell_rect.height - label_size.y) / 2.0;
                }
                VerticalLabelAlignment::Bottom => {
                    label_pos.y = cell_rect.y + cell_rect.height - label_size.y;
                }
            }

            a.label_mut(label).pos = label_pos;
        }
    }
}

/// Value of the `END_LABELS` internal property: the
/// `Map<LPort, LabelCell>` as an ordered list of pairs (port list order; the
/// map is never iterated in an order-sensitive way).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EndLabelCells(pub Vec<(LPortId, LabelCell)>);

impl EndLabelCells {
    pub fn get(&self, port: LPortId) -> Option<&LabelCell> {
        self.0.iter().find(|(p, _)| *p == port).map(|(_, c)| c)
    }

    pub fn get_mut(&mut self, port: LPortId) -> Option<&mut LabelCell> {
        self.0.iter_mut().find(|(p, _)| *p == port).map(|(_, c)| c)
    }
}

// ---------------------------------------------------------------- processor

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let edge_label_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_EDGE_LABEL);
    let label_label_spacing: f64 = a.graph(graph).properties.get(&lopts::SPACING_LABEL_LABEL);
    let vertical_layout = a
        .graph(graph)
        .properties
        .get::<crate::core::options::Direction>(&lopts::DIRECTION)
        .is_vertical();

    // We iterate over each node and place the end labels of its incident edges
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            process_node(a, node, edge_label_spacing, label_label_spacing, vertical_layout);
        }
    }
    Ok(())
}

fn process_node(
    a: &mut LGraphArena,
    node: LNodeId,
    edge_label_spacing: f64,
    label_label_spacing: f64,
    vertical_layout: bool,
) {
    // Iterate over all ports and collect their labels in label cells
    let ports = a.node(node).ports.clone();
    let mut port_label_cells: Vec<Option<LabelCell>> = Vec::with_capacity(ports.len());

    for (port_index, &port) in ports.iter().enumerate() {
        a.port_mut(port).id = port_index as i32;

        let gathered_labels = gather_labels(a, port);
        port_label_cells.push(create_configured_label_cell(
            a,
            gathered_labels,
            label_label_spacing,
            vertical_layout,
        ));
    }

    // Actually go off and place them labels!
    place_labels(
        a,
        node,
        &mut port_label_cells,
        label_label_spacing,
        edge_label_spacing,
        vertical_layout,
    );

    // Turn the array into a map and save that in the node
    let mut port_to_label_cell_map: Vec<(LPortId, LabelCell)> = Vec::new();
    for (index, cell) in port_label_cells.iter().enumerate() {
        if let Some(cell) = cell {
            port_to_label_cell_map.push((ports[index], cell.clone()));
        }
    }

    if !port_to_label_cell_map.is_empty() {
        a.node(node)
            .properties
            .set(&iprops::END_LABELS, EndLabelCells(port_to_label_cell_map));

        // Update the node's margins
        update_node_margins(a, node, &port_label_cells);
    }
}

/// `createConfiguredLabelCell`: creates a label cell for the given
/// labels, if any; otherwise returns `None`.
fn create_configured_label_cell(
    a: &LGraphArena,
    labels: Option<Vec<LLabelId>>,
    label_label_spacing: f64,
    vertical_layout: bool,
) -> Option<LabelCell> {
    let labels = labels?;
    if labels.is_empty() {
        return None;
    }

    // Create the new label cell and setup its alignments depending on the port's side
    let mut label_cell = LabelCell::new(label_label_spacing, !vertical_layout);

    for label in labels {
        let size = a.label(label).size;
        label_cell.add_label(label, size);
    }

    // Setup the label cell's size
    label_cell.rect.height = label_cell.minimum_height();
    label_cell.rect.width = label_cell.minimum_width();

    Some(label_cell)
}

//////////////////////////////////////////////////////////////////////////////
// Label Gathering

/// Special value to indicate that there are no edges incident to a port.
const NO_INCIDENT_EDGE_THICKNESS: f64 = -1.0;

/// The `EndLabelPreprocessor.gatherLabels(LPort)` (also used
/// by `LabelSideSelector`). Returns the end labels to be placed at the given
/// port (`Some`, possibly empty) or `None` if there are no incident edges.
pub fn gather_labels(a: &mut LGraphArena, port: LPortId) -> Option<Vec<LLabelId>> {
    let mut labels: Vec<LLabelId> = Vec::new();

    // Gather labels of the port itself
    let mut max_edge_thickness = gather_labels_into(a, port, &mut labels);

    // If it has a dummy associated with it, we need to go through the dummy's ports and
    // process those that were created for the current port (see NorthSouthPortPreprocessor)
    if let Some(dummy_node) = a.port(port).properties.try_get(&iprops::PORT_DUMMY) {
        let dummy_ports = a.node(dummy_node).ports.clone();
        for dummy_port in dummy_ports {
            if a.port(dummy_port).properties.try_get(&iprops::ORIGIN) == Some(Origin::LPort(port))
            {
                max_edge_thickness =
                    max_edge_thickness.max(gather_labels_into(a, dummy_port, &mut labels));
            }
        }
    }

    // Only save the maximum edge thickness if we'll be interested in it later
    if !labels.is_empty() {
        a.port(port)
            .properties
            .set(&iprops::MAX_EDGE_THICKNESS, max_edge_thickness);
    }

    if max_edge_thickness != NO_INCIDENT_EDGE_THICKNESS {
        Some(labels)
    } else {
        None
    }
}

/// `gatherLabels(LPort, List<LLabel>)`: puts all relevant end labels of
/// edges connected to the given port into the given list; returns the maximum
/// edge thickness of any incident edge, or [`NO_INCIDENT_EDGE_THICKNESS`].
fn gather_labels_into(a: &mut LGraphArena, port: LPortId, target_list: &mut Vec<LLabelId>) -> f64 {
    let mut max_edge_thickness = -1.0f64;

    let mut labels: Vec<LLabelId> = Vec::new();

    for incident_edge in a.port_connected_edges(port) {
        max_edge_thickness = max_edge_thickness
            .max(a.edge(incident_edge).properties.get(&lopts::EDGE_THICKNESS));

        if a.edge(incident_edge).source == Some(port) {
            // It's an outgoing edge; all tail labels belong to this port
            for &label in &a.edge(incident_edge).labels {
                if a.label(label)
                    .properties
                    .get::<EdgeLabelPlacement>(&lopts::EDGE_LABELS_PLACEMENT)
                    == EdgeLabelPlacement::TAIL
                {
                    labels.push(label);
                }
            }
        } else {
            // It's an incoming edge; all head labels belong to this port
            for &label in &a.edge(incident_edge).labels {
                if a.label(label)
                    .properties
                    .get::<EdgeLabelPlacement>(&lopts::EDGE_LABELS_PLACEMENT)
                    == EdgeLabelPlacement::HEAD
                {
                    labels.push(label);
                }
            }
        }

        // Remember the edge each label came from
        for &label in &labels {
            if !a.label(label).properties.has(&iprops::END_LABEL_EDGE) {
                a.label(label)
                    .properties
                    .set(&iprops::END_LABEL_EDGE, incident_edge);
            }
        }

        target_list.append(&mut labels);
    }

    max_edge_thickness
}

//////////////////////////////////////////////////////////////////////////////
// Label Placement

/// `placeLabels(LNode, ...)`: places end labels of all of the node's
/// ports.
fn place_labels(
    a: &mut LGraphArena,
    node: LNodeId,
    port_label_cells: &mut [Option<LabelCell>],
    label_label_spacing: f64,
    edge_label_spacing: f64,
    vertical_layout: bool,
) {
    // First, place them as we usually would. This step can result in overlaps for
    // northern / southern labels (for horizontal layout directions) or for eastern /
    // western labels (for vertical layout directions).
    let ports = a.node(node).ports.clone();
    for &port in &ports {
        let port_id = a.port(port).id as usize;
        if let Some(cell) = port_label_cells[port_id].take() {
            let cell = place_labels_for_port(a, port, cell, edge_label_spacing);
            port_label_cells[port_id] = Some(cell);
        }
    }

    // If there are ports on the problematic sides, go ahead and remove overlaps between them
    if vertical_layout {
        remove_label_overlaps(
            a,
            node,
            port_label_cells,
            PortSide::EAST,
            2.0 * label_label_spacing,
            edge_label_spacing,
        );
        remove_label_overlaps(
            a,
            node,
            port_label_cells,
            PortSide::WEST,
            2.0 * label_label_spacing,
            edge_label_spacing,
        );
    } else {
        remove_label_overlaps(
            a,
            node,
            port_label_cells,
            PortSide::NORTH,
            2.0 * label_label_spacing,
            edge_label_spacing,
        );
        remove_label_overlaps(
            a,
            node,
            port_label_cells,
            PortSide::SOUTH,
            2.0 * label_label_spacing,
            edge_label_spacing,
        );
    }
}

/// `placeLabels(LPort, LabelCell, double)`: places the edge end labels
/// that are to be placed near the given port.
fn place_labels_for_port(
    a: &LGraphArena,
    port: LPortId,
    mut label_cell: LabelCell,
    edge_label_spacing: f64,
) -> LabelCell {
    // Some necessary position information
    let p = a.port(port);
    let node = p.node.unwrap();
    let node_size = a.node(node).size;
    let node_margin = a.node(node).margin;
    let port_pos = p.pos;
    let port_anchor = KVector::new(port_pos.x + p.anchor.x, port_pos.y + p.anchor.y);

    let label_side = get_label_side(a, &label_cell);
    let max_edge_thickness: f64 = p.properties.get(&iprops::MAX_EDGE_THICKNESS);

    // Calculate cell position depending on port side
    match p.side {
        PortSide::NORTH => {
            label_cell.vertical_alignment = VerticalLabelAlignment::Bottom;
            label_cell.rect.y = -node_margin.top - edge_label_spacing - label_cell.rect.height;

            if label_side == LabelSide::ABOVE {
                label_cell.horizontal_alignment = HorizontalLabelAlignment::Right;
                label_cell.rect.x = port_anchor.x
                    - max_edge_thickness
                    - edge_label_spacing
                    - label_cell.rect.width;
            } else {
                label_cell.horizontal_alignment = HorizontalLabelAlignment::Left;
                label_cell.rect.x = port_anchor.x + max_edge_thickness + edge_label_spacing;
            }
        }

        PortSide::EAST => {
            label_cell.horizontal_alignment = HorizontalLabelAlignment::Left;
            label_cell.rect.x = node_size.x + node_margin.right + edge_label_spacing;

            if label_side == LabelSide::ABOVE {
                label_cell.vertical_alignment = VerticalLabelAlignment::Bottom;
                label_cell.rect.y = port_anchor.y
                    - max_edge_thickness
                    - edge_label_spacing
                    - label_cell.rect.height;
            } else {
                label_cell.vertical_alignment = VerticalLabelAlignment::Top;
                label_cell.rect.y = port_anchor.y + max_edge_thickness + edge_label_spacing;
            }
        }

        PortSide::SOUTH => {
            label_cell.vertical_alignment = VerticalLabelAlignment::Top;
            label_cell.rect.y = node_size.y + node_margin.bottom + edge_label_spacing;

            if label_side == LabelSide::ABOVE {
                label_cell.horizontal_alignment = HorizontalLabelAlignment::Right;
                label_cell.rect.x = port_anchor.x
                    - max_edge_thickness
                    - edge_label_spacing
                    - label_cell.rect.width;
            } else {
                label_cell.horizontal_alignment = HorizontalLabelAlignment::Left;
                label_cell.rect.x = port_anchor.x + max_edge_thickness + edge_label_spacing;
            }
        }

        PortSide::WEST => {
            label_cell.horizontal_alignment = HorizontalLabelAlignment::Right;
            label_cell.rect.x = -node_margin.left - edge_label_spacing - label_cell.rect.width;

            if label_side == LabelSide::ABOVE {
                label_cell.vertical_alignment = VerticalLabelAlignment::Bottom;
                label_cell.rect.y = port_anchor.y
                    - max_edge_thickness
                    - edge_label_spacing
                    - label_cell.rect.height;
            } else {
                label_cell.vertical_alignment = VerticalLabelAlignment::Top;
                label_cell.rect.y = port_anchor.y + max_edge_thickness + edge_label_spacing;
            }
        }

        PortSide::UNDEFINED => {}
    }

    label_cell
}

/// `removeLabelOverlaps`: calls the rectangle overlap removal code to
/// remove overlaps between end labels of edges connected to ports on the
/// given side.
fn remove_label_overlaps(
    a: &LGraphArena,
    node: LNodeId,
    port_label_cells: &mut [Option<LabelCell>],
    port_side: PortSide,
    label_label_spacing: f64,
    edge_label_spacing: f64,
) {
    let mut overlap_remover = RectangleStripOverlapRemover::create_for_direction(
        port_side_to_overlap_removal_direction(port_side),
    )
    .with_gap(label_label_spacing, label_label_spacing)
    .with_start_coordinate(calculate_overlap_start_coordinate(
        a,
        node,
        port_side,
        edge_label_spacing,
    ));

    // Gather the rectangles. We remember handles and copy the results back.
    let mut handles: Vec<(usize, usize)> = Vec::new();
    for port in a.node_ports_on_side(node, port_side) {
        let port_id = a.port(port).id as usize;
        if let Some(cell) = &port_label_cells[port_id] {
            debug_assert!(cell.rect.height > 0.0 && cell.rect.width > 0.0);
            let handle = overlap_remover.add_rectangle(cell.rect);
            handles.push((port_id, handle));
        }
    }

    // Remove overlaps
    overlap_remover.remove_overlaps();

    for (port_id, handle) in handles {
        if let Some(cell) = &mut port_label_cells[port_id] {
            cell.rect = overlap_remover.rectangle(handle);
        }
    }
}

/// `calculateOverlapStartCoordinate`.
fn calculate_overlap_start_coordinate(
    a: &LGraphArena,
    node: LNodeId,
    port_side: PortSide,
    edge_label_spacing: f64,
) -> f64 {
    let node_size = a.node(node).size;
    let node_margin = a.node(node).margin;

    match port_side {
        PortSide::NORTH => -node_margin.top - edge_label_spacing,
        PortSide::SOUTH => node_size.y + node_margin.bottom + edge_label_spacing,
        PortSide::EAST => node_size.x + node_margin.right + edge_label_spacing,
        PortSide::WEST => -node_margin.left - edge_label_spacing,
        PortSide::UNDEFINED => 0.0,
    }
}

//////////////////////////////////////////////////////////////////////////////
// Node Margins

/// `updateNodeMargins`: updates the node's margins to account for its
/// end labels.
fn update_node_margins(a: &mut LGraphArena, node: LNodeId, label_cells: &[Option<LabelCell>]) {
    let mut node_margin = a.node(node).margin;
    let node_size = a.node(node).size;

    // Calculate the rectangle that describes the node's current margin
    let mut node_margin_rectangle = ElkRectangle::new(
        -node_margin.left,
        -node_margin.top,
        node_margin.left + node_size.x + node_margin.right,
        node_margin.top + node_size.y + node_margin.bottom,
    );

    // Union the rectangle with each rectangle that describes a label cell
    for cell in label_cells.iter().flatten() {
        node_margin_rectangle.union(&cell.rect);
    }

    // Reapply the new rectangle to the margin
    node_margin.left = -node_margin_rectangle.x;
    node_margin.top = -node_margin_rectangle.y;
    node_margin.right = node_margin_rectangle.width - node_margin.left - node_size.x;
    node_margin.bottom = node_margin_rectangle.height - node_margin.top - node_size.y;
    a.node_mut(node).margin = node_margin;
}

//////////////////////////////////////////////////////////////////////////////
// Utility Methods

/// `getLabelSide(LabelCell)`: the label side of the cell's first label.
fn get_label_side(a: &LGraphArena, label_cell: &LabelCell) -> LabelSide {
    debug_assert!(label_cell.has_labels());
    a.label(label_cell.labels[0])
        .properties
        .get(&iprops::LABEL_SIDE)
}

/// `portSideToOverlapRemovalDirection`.
fn port_side_to_overlap_removal_direction(port_side: PortSide) -> OverlapRemovalDirection {
    match port_side {
        PortSide::NORTH => OverlapRemovalDirection::Up,
        PortSide::SOUTH => OverlapRemovalDirection::Down,
        PortSide::EAST => OverlapRemovalDirection::Right,
        PortSide::WEST => OverlapRemovalDirection::Left,
        // Shouldn't happen
        PortSide::UNDEFINED => unreachable!("undefined port side in overlap removal"),
    }
}
