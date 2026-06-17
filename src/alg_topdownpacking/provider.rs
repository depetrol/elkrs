//!
//! Neither phase requests intermediate processors, so the algorithm is always
//! `[node placer, whitespace eliminator]`.

use crate::core::registry::LayoutProvider;
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::math::ElkPadding;

use crate::alg_topdownpacking::options::{self, NodeArrangementStrategy, WhitespaceEliminationStrategy};

/// The grid state of `GridElkNode`. `setGridSize` pre-fills every row
/// with `cols` nulls and `put(col, row, node)` *inserts* at the column index,
/// so each row list is the placed nodes followed by the original nulls (its
/// length grows beyond `cols`).
struct Grid {
    cells: Vec<Vec<Option<NodeId>>>,
    cols: i32,
}

pub struct TopdownpackingLayoutProvider;

impl LayoutProvider for TopdownpackingLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        // assembleAlgorithm: both strategy enums have a single value, so the
        // irrefutable lets mirror the phase factories exactly.
        let NodeArrangementStrategy::LEFT_RIGHT_TOP_DOWN_NODE_PLACER = g
            .node(layout_node)
            .properties
            .get(&options::NODE_ARRANGEMENT_STRATEGY);
        let WhitespaceEliminationStrategy::BOTTOM_ROW_EQUAL_WHITESPACE_ELIMINATOR = g
            .node(layout_node)
            .properties
            .get(&options::WHITESPACE_ELIMINATION_STRATEGY);

        let grid = place_nodes(g, layout_node);
        eliminate_whitespace(g, layout_node, &grid);
        Ok(())
    }
}

fn place_nodes(g: &mut ElkGraph, layout_graph: NodeId) -> Grid {
    let padding: ElkPadding = g.node(layout_graph).properties.get(&options::PADDING);
    let node_node_spacing: f64 = g
        .node(layout_graph)
        .properties
        .get(&options::SPACING_NODE_NODE);

    // get hierarchical node sizes from parent for this layout
    let desired_node_width: f64 = g
        .node(layout_graph)
        .properties
        .get(&options::TOPDOWN_HIERARCHICAL_NODE_WIDTH);
    let aspect_ratio: f64 = g
        .node(layout_graph)
        .properties
        .get(&options::TOPDOWN_HIERARCHICAL_NODE_ASPECT_RATIO);

    // Get the list of nodes to lay out
    let nodes: Vec<NodeId> = g.node(layout_graph).children.clone();

    // Compute number of rows and columns to use to arrange nodes to maintain
    // the aspect ratio. This corresponds to filling up a square grid and
    // removing empty rows at the bottom.
    let cols = (nodes.len() as f64).sqrt().ceil() as i32;
    let rows = if nodes.len() as i32 > cols * cols - cols || cols == 0 {
        cols
    } else {
        // N <= W^2 - W
        cols - 1
    };

    // In case the graph dimensions have not been set yet, set them now; this
    // is needed for the standalone usage (`getPredictedSize`).
    let required_width = cols as f64 * desired_node_width
        + padding.left
        + padding.right
        + (cols - 1) as f64 * node_node_spacing;
    let required_height = rows as f64 * desired_node_width / aspect_ratio
        + padding.top
        + padding.bottom
        + (rows - 1) as f64 * node_node_spacing;
    let width = g.node(layout_graph).shape.width.max(required_width);
    let height = g.node(layout_graph).shape.height.max(required_height);
    g.node_mut(layout_graph).shape.set_dimensions(width, height);

    // set size of grid (`setGridSize` fills each row with `cols` nulls)
    let mut grid = Grid {
        cells: vec![vec![None; cols as usize]; rows as usize],
        cols,
    };

    // Place the nodes
    let mut curr_x = padding.left;
    let mut curr_y = padding.top;
    let mut current_col: i32 = 0;
    let mut current_row: usize = 0;

    for &node in &nodes {
        // Set the node's size
        g.node_mut(node)
            .shape
            .set_dimensions(desired_node_width, desired_node_width / aspect_ratio);
        // Set the node's coordinates
        g.node_mut(node).shape.x = curr_x;
        g.node_mut(node).shape.y = curr_y;
        // Store node's grid position (`put` inserts at the column index)
        grid.cells[current_row].insert(current_col as usize, Some(node));

        // Advance the coordinates
        curr_x += g.node(node).shape.width + node_node_spacing;
        current_col += 1;

        // go to next row if no space left
        // sizes are pre-computed so that everything fits nicely
        if current_col >= cols {
            curr_x = padding.left;
            curr_y += desired_node_width / aspect_ratio + node_node_spacing;
            current_col = 0;
            current_row += 1;
        }
    }

    grid
}

fn eliminate_whitespace(g: &mut ElkGraph, layout_graph: NodeId, grid: &Grid) {
    if g.node(layout_graph).shape.width == 0.0 || grid.cols == 0 {
        // Parent node has no width, skipping phase
        return;
    }

    let padding: ElkPadding = g.node(layout_graph).properties.get(&options::PADDING);

    // for each row check whether there is white space, if there is expand and
    // shift nodes
    let graph_width = g.node(layout_graph).shape.width;
    for row in &grid.cells {
        // check for whitespace next to last node: walk back over the trailing
        // nulls to the last placed node
        let mut last_index = row.len();
        let last = loop {
            last_index -= 1;
            if let Some(n) = row[last_index] {
                break n;
            }
        };
        let right_border = g.node(last).shape.x + g.node(last).shape.width;

        if right_border + padding.right < graph_width {
            let extra_space = graph_width - (right_border + padding.right);
            let extra_space_per_node = extra_space / (last_index + 1) as f64;
            let mut accumulated_shift = 0.0;
            // go through all nodes in row, shift and enlargen them
            for cell in row.iter().take(last_index + 1) {
                let node = cell.expect("cells up to lastIndex hold nodes");
                g.node_mut(node).shape.x += accumulated_shift;
                g.node_mut(node).shape.width += extra_space_per_node;
                accumulated_shift += extra_space_per_node;
            }
        }
    }

    // check whether there is vertical white space below the first column and
    // expand all columns accordingly, to prevent expanding into same space as
    // horizontal expansion (`getColumn(0)` has one entry per row)
    let col_size = grid.cells.len();
    let last = grid.cells[col_size - 1][0].expect("every grid row holds at least one node");
    let bottom_border = g.node(last).shape.y + g.node(last).shape.height;
    let graph_height = g.node(layout_graph).shape.height;
    let extra_space = graph_height - (bottom_border + padding.bottom);
    // Divides by `col.size() + 1` (the number of rows plus one), not by
    // the number of expanded nodes.
    let extra_space_per_node = extra_space / (col_size + 1) as f64;
    let mut accumulated_shift = 0.0;
    if bottom_border + padding.bottom < graph_height {
        for row in &grid.cells {
            // go through all nodes in row, shift and enlarge them; the shift
            // keeps accumulating across rows
            for cell in row {
                // `if (node == null) break;`
                let Some(node) = *cell else { break };
                g.node_mut(node).shape.y += accumulated_shift;
                g.node_mut(node).shape.height += extra_space_per_node;
                accumulated_shift += extra_space_per_node;
            }
        }
    }
}
