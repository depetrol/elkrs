//! Node size calculation,
//! node/port label placement, and node margin calculation.

pub mod algorithm;
pub mod cellsystem;
pub mod internal;
pub mod node_margin_calculator;

use crate::core::adapters::AdapterGraph;
use crate::core::options::{self, Direction};
use crate::graph::math::{ElkPadding, KVector};

use algorithm::{
    cell_system_configurator, horizontal_port_placement_size_calculator,
    inside_port_label_cell_creator, label_placer, node_label_and_size_utilities,
    node_label_cell_creator, node_size_calculator, port_context_creator,
    port_label_placement_calculator, port_placement_calculator,
    vertical_port_placement_size_calculator,
};
use cellsystem::ContainerArea;
use internal::NodeContext;

pub use node_margin_calculator::NodeMarginCalculator;

// ---------------------------------------------------------------------------
// NodeDimensionCalculation (entry points)

pub fn calculate_label_and_node_sizes<G: AdapterGraph>(
    g: &mut G,
    node_filter: impl Fn(&G, G::N) -> bool,
) {
    // Process all of the graph's direct children that pass the filter
    for node in g.nodes() {
        if node_filter(g, node) {
            process_node_size(g, node, true, false);
        }
    }
}

pub fn calculate_node_margins<G: AdapterGraph>(g: &mut G, exclude_edge_head_tail_labels: bool) {
    let mut calculator = NodeMarginCalculator::new();
    if exclude_edge_head_tail_labels {
        calculator = calculator.exclude_edge_head_tail_labels();
    }
    calculator.process(g);
}

/// Sorts the
/// port lists of all nodes of the graph clockwise.
pub fn sort_port_lists<G: AdapterGraph>(g: &mut G) {
    // Iterate through the nodes of all layers
    for node in g.nodes() {
        g.sort_port_list(node);
    }
}

// ---------------------------------------------------------------------------
// NodeLabelAndSizeCalculator

///
/// * `apply_stuff` — if `true`, the node is actually resized and has its
///   ports and labels positioned; if `false`, only the size that would be
///   applied is returned.
/// * `ignore_inside_port_labels` — if `true`, port labels that should be
///   placed inside are not placed. Used by layout algorithms that want a
///   lower bound on a hierarchical node's size but handle inside port labels
///   themselves.
///
/// Returns the node's size that was or would be applied.
pub fn process_node_size<G: AdapterGraph>(
    g: &mut G,
    node: G::N,
    apply_stuff: bool,
    ignore_inside_port_labels: bool,
) -> KVector {
    /* PREPARATORY PREPARATIONS
     *
     * Create the context objects that hold all of the information relevant to
     * our calculations, including pointers to all the components of the cell
     * system. Creating the port contexts will also create label cells for each
     * port that has labels.
     */
    let mut node_context = NodeContext::new(g, node);
    port_context_creator::create_port_contexts(g, &mut node_context, ignore_inside_port_labels);

    /* PHASE 1: WONDEROUS WATERFOWL
     *          Setup All Cells
     */
    let mut horizontal_layout_mode = true;
    // If no layout direction is specified, or the layout direction is set to
    // undefined, use horizontal layout mode (which yields vertically stacked
    // labels). (With our adapter the graph is always present.)
    if g.graph_properties().has(&options::DIRECTION) {
        let layout_direction: Direction = g.graph_properties().get(&options::DIRECTION);
        horizontal_layout_mode =
            layout_direction == Direction::UNDEFINED || layout_direction.is_horizontal();
    }
    node_label_cell_creator::create_node_label_cells(g, &mut node_context, false, horizontal_layout_mode);
    inside_port_label_cell_creator::create_inside_port_label_cells(&mut node_context);

    /* PHASE 2: DEFECTIVE DUCK
     *          Setup Client Area Space and Node Cell Padding
     */
    node_label_and_size_utilities::setup_minimum_client_area_size(g, &mut node_context);
    node_label_and_size_utilities::setup_node_padding_for_ports_with_offset(g, &mut node_context);

    /* PHASE 3: SALVAGEABLE SWAN
     *          Minimum Space Required to Place Ports
     */
    horizontal_port_placement_size_calculator::calculate_horizontal_port_placement_size(
        g,
        &mut node_context,
    );
    vertical_port_placement_size_calculator::calculate_vertical_port_placement_size(
        g,
        &mut node_context,
    );

    /* PHASE 4: DAMNABLE DUCKLING
     *          Setup Cell System Size Contribution Flags
     */
    cell_system_configurator::configure_cell_system_size_contributions(&mut node_context);

    /* PHASE 5: DUCK AND COVER
     *          Set Node Width and Place Horizontal Ports
     */
    node_size_calculator::set_node_width(g, &mut node_context);

    port_placement_calculator::place_horizontal_ports(g, &mut node_context);
    port_label_placement_calculator::place_horizontal_port_labels(g, &mut node_context);

    /* PHASE 6: GIGANTIC GOOSE
     *          Set Node Height and Place Vertical Ports
     */
    cell_system_configurator::update_vertical_inside_port_label_cell_padding(&mut node_context);
    node_size_calculator::set_node_height(g, &mut node_context);

    if !apply_stuff {
        return node_context.node_size;
    }

    node_label_and_size_utilities::offset_southern_ports_by_node_size(&mut node_context);

    port_placement_calculator::place_vertical_ports(g, &mut node_context);
    port_label_placement_calculator::place_vertical_port_labels(g, &mut node_context);

    /* PHASE 7: THANKSGIVING
     *          Place Labels and Apply Stuff
     */
    label_placer::place_labels(g, &mut node_context);
    node_label_and_size_utilities::set_node_padding(g, &node_context);
    node_label_and_size_utilities::apply_stuff(g, &node_context);

    // Return the size
    node_context.node_size
}

///
/// Spacing lookups go through the node's parent graph; with our adapter that
/// parent graph is `g` itself, so lookups fall back to `g`'s graph properties
/// (and from there to the property defaults for root nodes).
pub fn compute_inside_node_label_padding<G: AdapterGraph>(
    g: &G,
    node: G::N,
    layout_direction: Direction,
) -> ElkPadding {
    // Create a node context and fill it with all the inside node labels
    let mut node_context = NodeContext::new(g, node);
    node_label_cell_creator::create_node_label_cells(
        g,
        &mut node_context,
        true,
        !layout_direction.is_vertical(),
    );

    let label_cell_container = node_context
        .inside_node_label_container
        .expect("inside node label container must exist");
    let cells = &node_context.cells;
    let mut padding = ElkPadding::default();

    // Top
    for col in ContainerArea::VALUES {
        if let Some(label_cell) = cells.grid_get_cell(label_cell_container, ContainerArea::Begin, col) {
            padding.top = padding.top.max(cells.min_height(label_cell));
        }
    }

    // Bottom
    for col in ContainerArea::VALUES {
        if let Some(label_cell) = cells.grid_get_cell(label_cell_container, ContainerArea::End, col) {
            padding.bottom = padding.bottom.max(cells.min_height(label_cell));
        }
    }

    // Left
    for row in ContainerArea::VALUES {
        if let Some(label_cell) = cells.grid_get_cell(label_cell_container, row, ContainerArea::Begin) {
            padding.left = padding.left.max(cells.min_width(label_cell));
        }
    }

    // Right
    for row in ContainerArea::VALUES {
        if let Some(label_cell) = cells.grid_get_cell(label_cell_container, row, ContainerArea::End) {
            padding.right = padding.right.max(cells.min_width(label_cell));
        }
    }

    // Apply insets and gap where necessary
    let container_padding = cells.padding(label_cell_container);
    let gap = cells.grid_gap(label_cell_container);

    if padding.top > 0.0 {
        padding.top += container_padding.top;
        padding.top += gap;
    }

    if padding.bottom > 0.0 {
        padding.bottom += container_padding.bottom;
        padding.bottom += gap;
    }

    if padding.left > 0.0 {
        padding.left += container_padding.left;
        padding.left += gap;
    }

    if padding.right > 0.0 {
        padding.right += container_padding.right;
        padding.right += gap;
    }

    padding
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::adapters::ElkGraphAdapter;
    use crate::core::options::PortSide;
    use crate::graph::graph::ElkGraph;
    use crate::graph::math::Spacing;

    /// End-to-end test: a 30x30 node with two zero-size FREE ports (one WEST
    /// input, one EAST output), default options. Hand trace:
    ///
    /// * Defaults: `NODE_SIZE_CONSTRAINTS = {}` (so `areSizeConstraintsFixed`
    ///   is true and the node keeps its 30x30 size), `PORT_CONSTRAINTS =
    ///   UNDEFINED` (free placement), `PORT_LABELS_PLACEMENT = {OUTSIDE}`,
    ///   `SPACING_PORT_PORT = 10`, `SPACING_PORTS_SURROUNDING = 0`,
    ///   `PORT_ALIGNMENT_DEFAULT = DISTRIBUTED`.
    /// * Phase 3 (free vertical placement, EAST side, one 0x0 port): cell
    ///   padding top/bottom = 0; height = port heights + spacings = 0; since
    ///   alignment is DISTRIBUTED, height += 2 * portPortSpacing = 20. Same
    ///   for WEST. North/south cells have no ports (padding zeroed).
    /// * Phase 5: width fixed at 30. The middle row layout gives the WEST
    ///   atomic cell rect x=0,w=0 and the EAST cell rect x=30,w=0 (their
    ///   content width is 0 and the atomic-zero-width special case makes
    ///   their layout width 0).
    /// * Phase 6: height fixed at 30. nodeContainer is a vertical strip; the
    ///   north/south atomic cells have zero content height, so the middle row
    ///   gets rect y=0,h=30; so do the EAST/WEST cells.
    /// * placeVerticalFreePorts (EAST): availableSpace = 30 - 0 - 0 = 30;
    ///   calculatedPortPlacementHeight = 20; DISTRIBUTED with one port =>
    ///   modifiedPortPlacementSize subtracts 2 * 10 => 0, alignment becomes
    ///   CENTER; 30 >= 0, so currentYPos += (30 - 0) / 2 = 15. Port x =
    ///   nodeWidth = 30 (no PORT_BORDER_OFFSET). => EAST port at (30, 15).
    /// * WEST analogous: x = -portWidth = 0 (well, -0.0), y = 15.
    /// * calculate_node_margins: ports lie exactly on the node border with
    ///   zero size and there are no labels => all margins 0.
    #[test]
    fn thirty_by_thirty_node_with_two_free_ports() {
        let mut elk = ElkGraph::new();
        let root = elk.root;
        let node = elk.create_node(Some(root));
        elk.node_mut(node).shape.set_dimensions(30.0, 30.0);

        let west_port = elk.create_port(node);
        elk.port(west_port).properties.set(&options::PORT_SIDE, PortSide::WEST);
        let east_port = elk.create_port(node);
        elk.port(east_port).properties.set(&options::PORT_SIDE, PortSide::EAST);

        let mut adapter = ElkGraphAdapter::new(&mut elk, root);
        calculate_label_and_node_sizes(&mut adapter, |_, _| true);
        calculate_node_margins(&mut adapter, true);

        // Node size is unchanged (fixed size constraints)
        assert_eq!(elk.node(node).shape.width, 30.0);
        assert_eq!(elk.node(node).shape.height, 30.0);

        // Port positions as derived in the trace above
        assert_eq!(elk.port(east_port).shape.x, 30.0);
        assert_eq!(elk.port(east_port).shape.y, 15.0);
        assert_eq!(elk.port(west_port).shape.x, 0.0); // -0.0 == 0.0
        assert_eq!(elk.port(west_port).shape.y, 15.0);

        // Margins are all zero
        let margin: Spacing = elk.node(node).properties.get(&options::MARGINS);
        assert_eq!(margin, Spacing::default());
    }

    /// `computeInsideNodeLabelPadding` for a node without labels yields zero
    /// padding; with an inside top-center label it reserves the label height
    /// plus the node label padding and the cell gap.
    #[test]
    fn inside_node_label_padding() {
        use crate::core::options::NodeLabelPlacement;
        use crate::graph::graph::ElementId;
        use crate::graph::properties::EnumSet;

        let mut elk = ElkGraph::new();
        let root = elk.root;
        let node = elk.create_node(Some(root));
        elk.node_mut(node).shape.set_dimensions(30.0, 30.0);
        let label = elk.create_label("hi", ElementId::Node(node));
        elk.label_mut(label).shape.set_dimensions(12.0, 7.0);
        elk.node(node).properties.set(
            &options::NODE_LABELS_PLACEMENT,
            EnumSet::of(&[
                NodeLabelPlacement::INSIDE,
                NodeLabelPlacement::V_TOP,
                NodeLabelPlacement::H_CENTER,
            ]),
        );

        let adapter_holder = &mut elk;
        let adapter = ElkGraphAdapter::new(adapter_holder, root);
        let padding = compute_inside_node_label_padding(&adapter, node, Direction::RIGHT);

        // label height 7 + node label padding top (default 5) + gap
        // (labelCellSpacing = 2 * labelLabelSpacing = 0)
        assert_eq!(padding.top, 12.0);
        assert_eq!(padding.bottom, 0.0);
        assert_eq!(padding.left, 0.0);
        assert_eq!(padding.right, 0.0);
    }
}
