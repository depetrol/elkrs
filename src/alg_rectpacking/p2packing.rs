
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::math::ElkPadding;

use crate::alg_rectpacking::options;
use crate::alg_rectpacking::util::{self, BlockId, DrawingData, DrawingDataDescriptor, PackArena, RowId, StackId};

// ------------------------------------------------------------ InitialPlacement

pub fn place(
    arena: &mut PackArena,
    g: &mut ElkGraph,
    rectangles: &[NodeId],
    bounding_width: f64,
    node_node_spacing: f64,
) -> Vec<RowId> {
    let mut rows: Vec<RowId> = Vec::new();
    let mut row = arena.new_row(0.0, node_node_spacing);
    let mut drawing_height = 0.0f64;
    let first_block = arena_new_block_at(arena, 0.0, 0.0, row, node_node_spacing);
    arena.row_add_block(row, first_block);
    let mut current_width = 0.0f64;

    for &rect in rectangles {
        // Check whether current rectangle can be added to the last block
        let mut block = arena.row_last_block(row);
        let first_block_empty = arena
            .block(arena.row(row).children[0])
            .children
            .is_empty();
        let potential_row_width = current_width
            + g.node(rect).shape.width
            + if first_block_empty { 0.0 } else { node_node_spacing };
        if potential_row_width > bounding_width
            || g.node(rect).properties.get(&options::IN_NEW_ROW)
        {
            // Add rect in new block in new row.
            drawing_height += arena.row(row).height + node_node_spacing;
            rows.push(row);
            row = arena.new_row(drawing_height, node_node_spacing);
            let row_y = arena.row(row).y;
            block = arena_new_block_at(arena, 0.0, row_y, row, node_node_spacing);
            arena.row_add_block(row, block);
        }

        let parent = g.node(rect).parent.expect("rectangle without parent");
        let reevaluation: bool = g
            .node(parent)
            .properties
            .get(&options::PACKING_COMPACTION_ROW_HEIGHT_REEVALUATION);
        if arena.block(block).children.is_empty()
            || !reevaluation && is_similar_height(arena, g, block, rect, node_node_spacing)
        {
            // Every rect is in its own block before compaction.
            arena.block_add_child(g, block, rect);
        } else {
            // Case rect does not fit in block. Add new block to the right of it.
            let x = arena.block(block).x + arena.block(block).width + node_node_spacing;
            let row_y = arena.row(row).y;
            let new_block = arena_new_block_at(arena, x, row_y, row, node_node_spacing);
            arena.row_add_block(row, new_block);
            arena.block_add_child(g, new_block, rect);
        }
        current_width = g.node(rect).shape.x + g.node(rect).shape.width;
    }
    rows.push(row);
    rows
}

fn arena_new_block_at(arena: &mut PackArena, x: f64, y: f64, row: RowId, spacing: f64) -> BlockId {
    arena.new_block(x, y, row, spacing)
}

pub fn place_rect_in_block(
    arena: &mut PackArena,
    g: &mut ElkGraph,
    row: RowId,
    block: BlockId,
    rect: NodeId,
    bounding_width: f64,
    node_node_spacing: f64,
) -> bool {
    if is_similar_height(arena, g, block, rect, node_node_spacing) {
        let (rect_width, rect_height) = {
            let s = &g.node(rect).shape;
            (s.width, s.height)
        };
        let row_y = arena.row(row).y;
        let row_height = arena.row(row).height;
        if arena.block_last_row_new_x(block) + rect_width + node_node_spacing <= bounding_width
            && (arena.block_last_row_y(block) - row_y + rect_height <= row_height
                || arena.row(row).children.len() == 1)
        {
            // Case it fits in a row in the same block
            arena.block_add_child(g, block, rect);
            return true;
        } else if arena.block(block).x + rect_width <= bounding_width
            && (arena.block(block).y + arena.block(block).height + rect_height + node_node_spacing
                <= row_y + row_height)
        {
            // Case a new row in the block can be opened
            arena.block_add_child_in_new_row(g, block, rect);
            return true;
        }
    }
    false
}

pub fn is_similar_height(
    arena: &PackArena,
    g: &ElkGraph,
    block: BlockId,
    rect: NodeId,
    _node_node_spacing: f64,
) -> bool {
    let rect_height = g.node(rect).shape.height;
    let b = arena.block(block);
    if rect_height >= b.smallest_rect_height && rect_height <= b.min_height {
        true
    } else {
        b.average_height * 0.5 <= rect_height && b.average_height * 1.5 >= rect_height
    }
}

// ----------------------------------------------------------------- Compaction

/// Returns (somethingWasChanged, compactRowAgain).
#[allow(clippy::too_many_arguments)]
fn compact(
    arena: &mut PackArena,
    g: &mut ElkGraph,
    row_idx: usize,
    rows: &mut Vec<RowId>,
    bounding_width: f64,
    node_node_spacing: f64,
    row_height_reevaluation: bool,
) -> (bool, bool) {
    let mut something_was_changed = false;
    let mut compact_row_again = false;
    let next_row_index = row_idx + 1;
    let row = rows[row_idx];
    let mut current_stack: Option<StackId> = None;

    // Check for each block whether:
    // Part of the next block can be added to the current block,
    // the next (or part of the next) block can be put on top of it,
    // the next block can be put next to it,
    // or the current block can be drawn higher.
    let mut block_id: usize = 0;
    while block_id < arena.row(row).children.len() {
        let block = arena.row(row).children[block_id];
        if arena.block(block).fixed {
            block_id += 1;
            continue;
        }
        if arena.block(block).children.is_empty() {
            eprintln!("There should not be an empty block. Empty blocks are directly removed.");
            arena.row_remove_block(row, block);
            something_was_changed = true;
            continue;
        }

        // Move the block to its new position if something before it was
        // changed and it is moveable.
        if !arena.block(block).position_fixed {
            if let Some(stack) = current_stack {
                arena.stack_update_dimension(stack);
            }
            let new_x = match current_stack {
                None => 0.0,
                Some(stack) => arena.stack(stack).x + arena.stack(stack).width + node_node_spacing,
            };
            let row_y = arena.row(row).y;
            let stack = arena.new_stack(new_x, row_y, node_node_spacing);
            current_stack = Some(stack);
            let loc_x = arena.stack(stack).x + arena.stack(stack).width;
            arena.block_set_location(g, block, loc_x, row_y);
            arena.row_mut(row).stacks.push(stack);
            arena.stack_add_block(stack, block);
            arena.block_mut(block).position_fixed = true;
        }

        // Optimization 1: Does the next block fit on top of me?
        let mut next_block = get_next_block(arena, rows, row, block_id, next_row_index);
        let mut was_from_next_row = false;
        if let Some(nb) = next_block {
            was_from_next_row = arena.block(nb).parent_row != row;
        }

        if let Some(nb) = next_block {
            // Decide whether the block can be merged with the previous block.
            // Try to move as many rects as possible from the next block in
            // this block. First flatten the current block.
            if !arena.block(nb).children.is_empty()
                && !g
                    .node(arena.block(nb).children[0])
                    .properties
                    .get(&options::IN_NEW_ROW)
            {
                use_row_width(arena, g, block, bounding_width);
                // Absorb all rectangles that fit in the current block from the
                // next block if they fit the row.
                something_was_changed |=
                    absorb_blocks(arena, g, row, block, nb, bounding_width, node_node_spacing);
            } else {
                // Delete empty nextBlock
                arena.row_remove_block(row, nb);
                break;
            }

            // From the previous step the next block and the next row might be
            // empty. Delete all empty blocks and rows.
            if arena.block(nb).children.is_empty() {
                if rows.len() > next_row_index {
                    arena.row_remove_block(rows[next_row_index], nb);
                }
                next_block = None;
                while rows.len() > next_row_index
                    && arena.row(rows[next_row_index]).children.is_empty()
                {
                    rows.remove(next_row_index);
                }
            }
            let nb = match next_block {
                None => {
                    continue;
                }
                Some(nb) => nb,
            };

            // Try to fit next block on top of the current block.
            if !g
                .node(arena.block(nb).children[0])
                .properties
                .get(&options::IN_NEW_ROW)
                && place_below(
                    arena,
                    g,
                    rows,
                    row,
                    block,
                    nb,
                    was_from_next_row,
                    bounding_width,
                    next_row_index,
                    node_node_spacing,
                )
            {
                something_was_changed = true;
                block_id += 1;
                continue;
            }

            if was_from_next_row {
                // Try to place the next block next to the current one.
                // Draw the current block as slim as possible.
                let old_row_height = arena.row(row).height;
                let next_block_min_height = arena.block(nb).min_height;
                if !g
                    .node(arena.block(nb).children[0])
                    .properties
                    .get(&options::IN_NEW_ROW)
                    && place_beside(
                        arena,
                        g,
                        rows,
                        row,
                        block,
                        nb,
                        was_from_next_row,
                        bounding_width,
                        next_row_index,
                        node_node_spacing,
                        row_height_reevaluation,
                    )
                {
                    something_was_changed = true;
                    // The next block was inserted and it dominates the current
                    // row height. Therefore, the current node has to be
                    // repacked to fit the row height better.
                    if old_row_height < next_block_min_height {
                        compact_row_again = true;
                        arena.block_mut(nb).parent_row = row;
                        break;
                    }
                    block_id += 1;
                    continue;
                } else if use_row_height(arena, g, row, block) {
                    arena.block_mut(block).fixed = true;
                    something_was_changed = true;
                    block_id += 1;
                    continue;
                }
            } else if use_row_height(arena, g, row, block) {
                arena.block_mut(block).fixed = true;
                something_was_changed = true;
                block_id += 1;
                continue;
            }

            // Case only parts of the next block where added, but no full next
            // block could be added.
            if something_was_changed {
                block_id += 1;
                continue;
            }
        }

        // Optimization 2: Let blocks use the row width if they can
        if use_row_height(arena, g, row, block) {
            arena.block_mut(block).fixed = true;
            something_was_changed = true;
            if let Some(nb) = next_block {
                arena.block_mut(nb).position_fixed = false;
            }
            block_id += 1;
            continue;
        } else {
            let stack = arena.block(block).stack.expect("block without stack");
            arena.stack_update_dimension(stack);
        }
        block_id += 1;
    }

    (something_was_changed, compact_row_again)
}

fn get_next_block(
    arena: &PackArena,
    rows: &[RowId],
    row: RowId,
    block_id: usize,
    next_row_index: usize,
) -> Option<BlockId> {
    if block_id + 1 < arena.row(row).children.len() {
        // Get block from this row.
        Some(arena.row(row).children[block_id + 1])
    } else if next_row_index < rows.len()
        && !arena.row(rows[next_row_index]).children.is_empty()
    {
        // Get block from next row.
        Some(arena.row(rows[next_row_index]).children[0])
    } else {
        None
    }
}

fn use_row_height(arena: &mut PackArena, g: &mut ElkGraph, row: RowId, block: BlockId) -> bool {
    let mut something_was_changed = false;
    let stack = arena.block(block).stack.expect("block without stack");
    let previous_width = arena.stack(stack).width;
    if arena.block(block).height < arena.row(row).height {
        let row_height = arena.row(row).height;
        let target_width = arena.stack_get_width_for_fixed_height(g, stack, row_height);
        if arena.stack(stack).width > target_width {
            arena.stack_place_rects_in(g, stack, target_width);
            something_was_changed = previous_width != arena.stack(stack).width;
        }
    }
    something_was_changed
}

fn use_row_width(arena: &mut PackArena, g: &mut ElkGraph, block: BlockId, bounding_width: f64) {
    let width = bounding_width - arena.block(block).x;
    arena.block_place_rects_in(g, block, width);
    let stack = arena.block(block).stack.expect("block without stack");
    arena.stack_update_dimension(stack);
}

fn absorb_blocks(
    arena: &mut PackArena,
    g: &mut ElkGraph,
    row: RowId,
    block: BlockId,
    next_block: BlockId,
    bounding_width: f64,
    node_node_spacing: f64,
) -> bool {
    let mut something_was_changed = false;
    let mut rect = arena.block(next_block).children[0];
    while place_rect_in_block(arena, g, row, block, rect, bounding_width, node_node_spacing) {
        // The rectangle was added to this block.
        something_was_changed = true;
        arena.block_remove_child(g, next_block, rect);
        if arena.block(next_block).children.is_empty() {
            break;
        }
        rect = arena.block(next_block).children[0];
    }

    // Cleanup.
    if arena.block(next_block).children.is_empty() {
        let parent_row = arena.block(next_block).parent_row;
        arena.row_remove_block(parent_row, next_block);
    }
    if something_was_changed {
        let stack = arena.block(block).stack.expect("block without stack");
        arena.stack_update_dimension(stack);
    }

    something_was_changed
}

#[allow(clippy::too_many_arguments)]
fn place_below(
    arena: &mut PackArena,
    g: &mut ElkGraph,
    rows: &mut Vec<RowId>,
    row: RowId,
    block: BlockId,
    next_block: BlockId,
    was_from_next_row: bool,
    bounding_width: f64,
    next_row_index: usize,
    node_node_spacing: f64,
) -> bool {
    let mut something_was_changed = false;
    // Flatten both blocks and check whether they fit on top of each other.
    let remaining_width = bounding_width - arena.block(block).x;
    let current_block_min_height = arena.block(block).y - arena.row(row).y
        + arena.block_get_height_for_target_width(g, block, remaining_width);
    // Case that the next block cannot fit in any case.
    if arena.block(next_block).min_width + node_node_spacing > remaining_width {
        return false;
    }
    let next_block_min_height =
        arena.block_get_height_for_target_width(g, next_block, remaining_width);

    if current_block_min_height + node_node_spacing + next_block_min_height
        <= arena.row(row).height
    {
        // Case they fit on top of each other.
        let width = bounding_width - arena.block(block).x;
        arena.block_place_rects_in(g, block, width);
        arena.block_mut(block).fixed = true;
        let width = bounding_width - arena.block(block).x;
        arena.block_place_rects_in(g, next_block, width);
        let (bx, by, bh) = {
            let b = arena.block(block);
            (b.x, b.y, b.height)
        };
        arena.block_set_location(g, next_block, bx, by + bh + node_node_spacing);
        arena.block_mut(next_block).position_fixed = true;
        let stack = arena.block(block).stack.expect("block without stack");
        arena.stack_add_block(stack, next_block);
        something_was_changed = true;

        // Remove next block from next row if it is from there.
        if was_from_next_row {
            arena.row_add_block(row, next_block);
            arena.block_mut(next_block).parent_row = row;
            if rows.len() > next_row_index {
                arena.row_remove_block(rows[next_row_index], next_block);
                if arena.row(rows[next_row_index]).children.is_empty() {
                    rows.remove(next_row_index);
                }
            }
        }
    }
    something_was_changed
}

#[allow(clippy::too_many_arguments)]
fn place_beside(
    arena: &mut PackArena,
    g: &mut ElkGraph,
    rows: &mut Vec<RowId>,
    row: RowId,
    block: BlockId,
    next_block: BlockId,
    _was_from_next_row: bool,
    bounding_width: f64,
    next_row_index: usize,
    node_node_spacing: f64,
    row_height_reevaluation: bool,
) -> bool {
    let mut something_was_changed = false;
    // Get minimum width for current stack that would fit the height.
    let stack = arena.block(block).stack.expect("block without stack");
    let fixed_height = arena.row(row).y + arena.row(row).height - arena.stack(stack).y;
    let current_block_min_width = arena.stack_get_width_for_fixed_height(g, stack, fixed_height);
    // Row height is reevaluated if the next block will dominate the current
    // row in height.
    let should_row_height_be_reevaluated =
        arena.block(next_block).min_height > arena.row(row).height && row_height_reevaluation;

    // Get total width of the current stack.
    let mut target_width_of_next_block =
        bounding_width - (arena.stack(stack).x + current_block_min_width - node_node_spacing);

    // Get height of next block.
    let next_block_height =
        arena.block_get_height_for_target_width(g, next_block, target_width_of_next_block);
    // If the height to fit in the current row is bigger than the minimum
    // height the next block does not provide a node that will define the row
    // height by its height.
    if should_row_height_be_reevaluated && next_block_height > arena.block(next_block).min_height {
        return false;
    }

    if should_row_height_be_reevaluated {
        // Calculate available width for next block.
        let mut potential_width = 0.0f64;
        let next_block_min_height = arena.block(next_block).min_height;
        let stacks = arena.row(row).stacks.clone();
        for s in stacks {
            potential_width +=
                arena.stack_get_width_for_fixed_height(g, s, next_block_min_height)
                    + node_node_spacing;
        }
        target_width_of_next_block = bounding_width - potential_width;
    }

    // Check width of next block.
    if target_width_of_next_block < arena.block(next_block).min_width {
        return false;
    }
    // Handle last layer case
    let last_row_optimization = next_row_index == rows.len() - 1
        && target_width_of_next_block >= arena.row(rows[next_row_index]).width;

    // If the next block does not contain a node that would dominate the row
    // height and it would otherwise exceed the row height if drawn in the
    // available width, it cannot be placed in this row (if it is not the last
    // row).
    if !should_row_height_be_reevaluated
        && next_block_height > arena.row(row).height
        && !last_row_optimization
    {
        return false;
    }
    if last_row_optimization || should_row_height_be_reevaluated || next_block_height <= arena.row(row).height
    {
        if last_row_optimization && next_block_height > arena.row(row).height {
            arena.block_mut(block).height = next_block_height;
            let width = arena.block_get_width_for_target_height(g, block, next_block_height);
            arena.block_place_rects_in(g, block, width);
        } else {
            arena.stack_place_rects_in(g, stack, current_block_min_width);
            arena.block_mut(block).fixed = true;
        }

        // Place next block in remaining width.
        let width = bounding_width - (arena.block(block).x + arena.block(block).width);
        arena.block_place_rects_in(g, next_block, width);
        let (loc_x, row_y) = (
            arena.stack(stack).x + arena.stack(stack).width,
            arena.row(row).y,
        );
        arena.block_set_location(g, next_block, loc_x, row_y);
        arena.row_add_block(row, next_block);

        // Delete empty rows if needed.
        if rows.len() > next_row_index {
            arena.row_remove_block(rows[next_row_index], next_block);
            if arena.row(rows[next_row_index]).children.is_empty() {
                rows.remove(next_row_index);
            }
        }
        something_was_changed = true;
    }
    something_was_changed
}

// ----------------------------------------------------- RowFillingAndCompaction

pub struct RowFillingAndCompaction {
    aspect_ratio: f64,
    node_node_spacing: f64,
    pub potential_row_width_decrease_min: f64,
    pub potential_row_width_decrease_max: f64,
    pub potential_row_width_increase_min: f64,
    pub potential_row_width_increase_max: f64,
}

impl RowFillingAndCompaction {
    pub fn new(aspect_ratio: f64, node_node_spacing: f64) -> Self {
        RowFillingAndCompaction {
            aspect_ratio,
            node_node_spacing,
            potential_row_width_decrease_min: f64::INFINITY,
            potential_row_width_decrease_max: 0.0,
            potential_row_width_increase_min: f64::INFINITY,
            potential_row_width_increase_max: 0.0,
        }
    }

    pub fn start(
        &mut self,
        arena: &mut PackArena,
        g: &mut ElkGraph,
        layout_graph: NodeId,
        padding: &ElkPadding,
    ) -> (DrawingData, Vec<RowId>) {
        let target_width: f64 = g.node(layout_graph).properties.get(&options::TARGET_WIDTH);
        let min_width: f64 = g.node(layout_graph).properties.get(&options::MIN_WIDTH);
        let min_height: f64 = g.node(layout_graph).properties.get(&options::MIN_HEIGHT);
        // Reset coordinates potentially set by width approximation.
        let children = g.node(layout_graph).children.clone();
        util::reset_coordinates(g, &children);

        // Initial placement for rectangles in blocks in each row.
        let mut rows = place(arena, g, &children, target_width, self.node_node_spacing);

        // Compaction of blocks.
        let mut row_idx: usize = 0;
        while row_idx < rows.len() {
            let current_row = rows[row_idx];
            if row_idx != 0 {
                let previous_row = rows[row_idx - 1];
                let new_y = arena.row(previous_row).y
                    + arena.row(previous_row).height
                    + self.node_node_spacing;
                arena.row_set_y(g, current_row, new_y);
            }
            let row_height_reevaluation: bool = g
                .node(layout_graph)
                .properties
                .get(&options::PACKING_COMPACTION_ROW_HEIGHT_REEVALUATION);
            let (_changed, compact_row_again) = compact(
                arena,
                g,
                row_idx,
                &mut rows,
                target_width,
                self.node_node_spacing,
                row_height_reevaluation,
            );
            if compact_row_again {
                // Reset the row such that stacks are removed, blocks are not fixed.
                let blocks = arena.row(current_row).children.clone();
                for block in blocks {
                    arena.block_mut(block).fixed = false;
                    arena.block_mut(block).position_fixed = false;
                    // Reset precalculated min/max width/heights.
                    arena.block_adjust_size_after_remove(g, block);
                }
                arena.row_reset_stacks(current_row);
                arena.row_mut(current_row).width = target_width;
                continue;
            } else {
                self.adjust_width_and_height(arena, current_row);
                // Check how much space would be needed in the current row to
                // add the first block from the next one. And how much space
                // could be removed from each row by removing the last block.
                if row_idx + 1 < rows.len() {
                    let first_block_width =
                        arena.block(arena.row_first_block(rows[row_idx + 1])).width;
                    let current_row_width = arena.row(current_row).width;
                    self.potential_row_width_increase_max = f64::max(
                        current_row_width + self.node_node_spacing + first_block_width
                            - target_width,
                        self.potential_row_width_decrease_max,
                    );
                    self.potential_row_width_increase_min = f64::min(
                        current_row_width + self.node_node_spacing + first_block_width
                            - target_width,
                        self.potential_row_width_decrease_min,
                    );
                    let num_stacks = arena.row(current_row).stacks.len();
                    if num_stacks != 0 {
                        let last_stack_width =
                            arena.stack(arena.row(current_row).stacks[num_stacks - 1]).width;
                        let extra = if num_stacks <= 1 { 0.0 } else { self.node_node_spacing };
                        self.potential_row_width_decrease_max = f64::max(
                            self.potential_row_width_decrease_max,
                            last_stack_width + extra,
                        );
                        // Quirk: min over the just-updated *max*.
                        self.potential_row_width_decrease_min = f64::min(
                            self.potential_row_width_decrease_max,
                            last_stack_width + extra,
                        );
                    }
                }
                // Special case the graph has only one row with one block with
                // several subrows.
                if rows.len() == 1 {
                    let last_stack = *arena.row(current_row).stacks.last().unwrap();
                    let last_block = *arena.stack(last_stack).blocks.last().unwrap();
                    let last_block_width = arena.block(last_block).width;
                    let block_row_widths: Vec<f64> =
                        arena.block(last_block).rows.iter().map(|r| r.width).collect();
                    for block_row_width in block_row_widths {
                        self.potential_row_width_decrease_max = f64::max(
                            self.potential_row_width_decrease_max,
                            last_block_width - block_row_width,
                        );
                        self.potential_row_width_decrease_min = f64::min(
                            self.potential_row_width_decrease_min,
                            last_block_width - block_row_width,
                        );
                        self.potential_row_width_increase_max = f64::max(
                            self.potential_row_width_increase_max,
                            block_row_width + self.node_node_spacing,
                        );
                        self.potential_row_width_increase_min = f64::min(
                            self.potential_row_width_increase_min,
                            block_row_width + self.node_node_spacing,
                        );
                    }
                }
            }
            row_idx += 1;
        }

        let size = util::calculate_dimensions_rows(arena, &rows, self.node_node_spacing);

        let total_width = f64::max(size.x, min_width - padding.horizontal());
        let height = f64::max(size.y, min_height - padding.vertical());
        let additional_height = height - size.y;
        g.node(layout_graph)
            .properties
            .set(&options::ADDITIONAL_HEIGHT, additional_height);

        (
            DrawingData::new(
                self.aspect_ratio,
                total_width,
                size.y + additional_height,
                DrawingDataDescriptor::WholeDrawing,
            ),
            rows,
        )
    }

    fn adjust_width_and_height(&self, arena: &mut PackArena, row: RowId) {
        let mut max_height = 0.0f64;
        let mut max_width = 0.0f64;
        let stacks = arena.row(row).stacks.clone();
        for (index, stack) in stacks.into_iter().enumerate() {
            arena.stack_update_dimension(stack);
            max_height = f64::max(max_height, arena.stack(stack).height);
            max_width += arena.stack(stack).width
                + if index > 0 { self.node_node_spacing } else { 0.0 };
        }
        let r = arena.row_mut(row);
        r.height = max_height;
        r.width = max_width;
    }
}

// ------------------------------------------------------------------ Compactor

/// Returns the rows of the first packing run;
/// they take the place of the `InternalProperties.ROWS` graph property.
pub fn compactor(arena: &mut PackArena, g: &mut ElkGraph, graph: NodeId) -> Vec<RowId> {
    let aspect_ratio: f64 = g.node(graph).properties.get(&options::ASPECT_RATIO);
    let node_node_spacing: f64 = g.node(graph).properties.get(&options::SPACING_NODE_NODE);
    let padding: ElkPadding = g.node(graph).properties.get(&options::PADDING);

    let mut second_it = RowFillingAndCompaction::new(aspect_ratio, node_node_spacing);
    let (mut drawing, rows) = second_it.start(arena, g, graph, &padding);
    // Begin possible iterations to improve rectpacking by setting a new target
    // width and repeating the compaction.
    copy_row_width_change_values(g, graph, &second_it);

    // Begin more compaction iterations if more than one iteration is specified.
    let mut iterations: i32 = g
        .node(graph)
        .properties
        .get(&options::PACKING_COMPACTION_ITERATIONS);
    while iterations > 1 {
        // Create a shallow clone based on properties and sizes of children
        // (not grandchildren).
        let clone = clone_node(g, graph);
        let old_sm = drawing.scale_measure();
        // Calculate new target width and configure clone.
        configure_second_iteration(g, graph, clone, &drawing);
        // Run additional compaction step. (The rows of this run only live on
        // the clone and are discarded.)
        let mut second_it = RowFillingAndCompaction::new(aspect_ratio, node_node_spacing);
        let (new_drawing, _clone_rows) = second_it.start(arena, g, clone, &padding);

        // Compare scale measure and choose the best packing.
        let new_sm = new_drawing.scale_measure();

        // NaN check.
        if new_sm >= old_sm && !new_sm.is_nan() {
            // If the new packing is better apply packing to original graph.
            let clone_children = g.node(clone).children.clone();
            let graph_children = g.node(graph).children.clone();
            for i in 0..clone_children.len() {
                copy_position(g, clone_children[i], graph_children[i]);
            }
            copy_row_width_change_values(g, graph, &second_it);
            drawing.set_drawing_width(new_drawing.drawing_width());
            drawing.set_drawing_height(new_drawing.drawing_height());
        }
        iterations -= 1;
    }

    g.node(graph)
        .properties
        .set(&options::DRAWING_HEIGHT, drawing.drawing_height());
    g.node(graph)
        .properties
        .set(&options::DRAWING_WIDTH, drawing.drawing_width());
    rows
}

fn copy_row_width_change_values(g: &mut ElkGraph, graph: NodeId, compaction: &RowFillingAndCompaction) {
    let p = &g.node(graph).properties;
    p.set(&options::MIN_ROW_INCREASE, compaction.potential_row_width_increase_min);
    p.set(&options::MAX_ROW_INCREASE, compaction.potential_row_width_increase_max);
    p.set(&options::MIN_ROW_DECREASE, compaction.potential_row_width_decrease_min);
    p.set(&options::MAX_ROW_DECREASE, compaction.potential_row_width_decrease_max);
}

fn configure_second_iteration(g: &mut ElkGraph, layout_graph: NodeId, clone: NodeId, drawing: &DrawingData) {
    let padding: ElkPadding = g.node(layout_graph).properties.get(&options::PADDING);
    let aspect_ratio: f64 = g.node(layout_graph).properties.get(&options::ASPECT_RATIO);
    let num_children = g.node(layout_graph).children.len();
    let min_row_increase: f64 = g.node(layout_graph).properties.get(&options::MIN_ROW_INCREASE);
    let min_row_decrease: f64 = g.node(layout_graph).properties.get(&options::MIN_ROW_DECREASE);
    // Try to layout again if the aspect ratio seems to be bad
    if num_children > 1
        && min_row_increase != f64::INFINITY
        && (drawing.drawing_width() + padding.horizontal())
            / (drawing.drawing_height() + padding.vertical())
            < aspect_ratio
    {
        // The drawing is too high, this means the approximated target width is
        // too low. The new target width will be set to the next higher value
        // that would change something.
        let target: f64 = g.node(layout_graph).properties.get(&options::TARGET_WIDTH);
        g.node(clone)
            .properties
            .set(&options::TARGET_WIDTH, target + min_row_increase);
    } else if num_children > 1
        && min_row_decrease != f64::INFINITY
        && (drawing.drawing_width() + padding.horizontal())
            / (drawing.drawing_height() + padding.vertical())
            > aspect_ratio
    {
        // The drawing is too wide, this means the approximated target width is
        // too high. The new target width will be set to the next smaller value
        // that would change something.
        let min_width: f64 = g.node(layout_graph).properties.get(&options::MIN_WIDTH);
        let clone_target: f64 = g.node(clone).properties.get(&options::TARGET_WIDTH);
        g.node(clone)
            .properties
            .set(&options::TARGET_WIDTH, f64::max(min_width, clone_target - min_row_decrease));
    }
}

fn clone_node(g: &mut ElkGraph, node: NodeId) -> NodeId {
    let clone = g.create_node(None);
    let node_props = g.node(node).properties.clone();
    g.node(clone).properties.copy_from(&node_props);
    let children = g.node(node).children.clone();
    for child in children {
        let new_child = g.create_node(Some(clone));
        let (x, y, w, h, identifier, props) = {
            let c = g.node(child);
            (c.shape.x, c.shape.y, c.shape.width, c.shape.height, c.identifier.clone(), c.properties.clone())
        };
        let nc = g.node_mut(new_child);
        nc.shape.set_dimensions(w, h);
        nc.identifier = identifier;
        nc.shape.set_location(x, y);
        nc.properties.copy_from(&props);
    }
    clone
}

fn copy_position(g: &mut ElkGraph, node: NodeId, other: NodeId) {
    let (x, y, w, h) = {
        let s = &g.node(node).shape;
        (s.x, s.y, s.width, s.height)
    };
    let o = &mut g.node_mut(other).shape;
    o.set_dimensions(w, h);
    o.set_location(x, y);
    let node_children = g.node(node).children.clone();
    let other_children = g.node(other).children.clone();
    for i in 0..node_children.len() {
        copy_position(g, node_children[i], other_children[i]);
    }
}

// ------------------------------------------------------------- SimplePlacement

/// Returns the rows (the `ROWS` structure).
pub fn simple_placement(arena: &mut PackArena, g: &mut ElkGraph, graph: NodeId) -> Vec<RowId> {
    let target_width: f64 = g.node(graph).properties.get(&options::TARGET_WIDTH);
    let node_node_spacing: f64 = g.node(graph).properties.get(&options::SPACING_NODE_NODE);
    let padding: ElkPadding = g.node(graph).properties.get(&options::PADDING);
    // Reset coordinates potentially set by width approximation.
    let children = g.node(graph).children.clone();
    util::reset_coordinates(g, &children);

    // Initial placement for rectangles in blocks in each row.
    let rows = place(arena, g, &children, target_width, node_node_spacing);
    // Put every block in its own block stack.
    for &row in &rows {
        let blocks = arena.row(row).children.clone();
        for block in blocks {
            let spacing: f64 = g.node(graph).properties.get(&options::SPACING_NODE_NODE);
            let (bx, by) = (arena.block(block).x, arena.block(block).y);
            let stack = arena.new_stack(bx, by, spacing);
            arena.stack_add_block(stack, block);
            arena.row_mut(row).stacks.push(stack);
        }
    }
    let size = util::calculate_dimensions_rows(arena, &rows, node_node_spacing);

    let min_width: f64 = g.node(graph).properties.get(&options::MIN_WIDTH);
    let min_height: f64 = g.node(graph).properties.get(&options::MIN_HEIGHT);
    let width = f64::max(size.x, min_width - padding.horizontal());
    let height = f64::max(size.y, min_height - padding.vertical());
    let additional_height = height - size.y;
    let p = &g.node(graph).properties;
    p.set(&options::ADDITIONAL_HEIGHT, additional_height);
    p.set(&options::DRAWING_WIDTH, width);
    p.set(&options::DRAWING_HEIGHT, height + additional_height);
    rows
}

// ---------------------------------------------------------------- NoPlacement

pub fn no_placement(g: &mut ElkGraph, graph: NodeId) {
    let padding: ElkPadding = g.node(graph).properties.get(&options::PADDING);
    let rectangles = g.node(graph).children.clone();

    let size = util::calculate_dimensions_rects(g, &rectangles);
    let min_width: f64 = g.node(graph).properties.get(&options::MIN_WIDTH);
    let min_height: f64 = g.node(graph).properties.get(&options::MIN_HEIGHT);
    let width = f64::max(size.x, min_width - padding.horizontal());
    let height = f64::max(size.y, min_height - padding.vertical());
    let additional_height = height - size.y;
    let p = &g.node(graph).properties;
    p.set(&options::ADDITIONAL_HEIGHT, additional_height);
    p.set(&options::DRAWING_WIDTH, width);
    p.set(&options::DRAWING_HEIGHT, height + additional_height);
}
