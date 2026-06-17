//!
//! All rows, blocks and stacks live in a [`PackArena`] and reference
//! each other through ids; the rectangles themselves are `ElkNode`s
//! referenced by `NodeId` whose coordinates are read/written directly on the
//! graph.

use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::math::{ElkRectangle, KVector};

// ------------------------------------------------------ DrawingDataDescriptor

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawingDataDescriptor {
    CandidatePositionLastPlacedRight,
    CandidatePositionLastPlacedBelow,
    CandidatePositionWholeDrawingRight,
    CandidatePositionWholeDrawingBelow,
    WholeDrawing,
}

// ----------------------------------------------------------------- DrawingData

#[derive(Clone, Debug)]
pub struct DrawingData {
    scale_measure: f64,
    drawing_width: f64,
    drawing_height: f64,
    area: f64,
    aspect_ratio: f64,
    /// Desired aspect ratio.
    dar: f64,
    placement_option: DrawingDataDescriptor,
    next_x: f64,
    next_y: f64,
}

impl DrawingData {
    pub fn new(dar: f64, drawing_width: f64, drawing_height: f64, opt: DrawingDataDescriptor) -> Self {
        Self::with_coords(dar, drawing_width, drawing_height, opt, 0.0, 0.0)
    }

    pub fn with_coords(
        dar: f64,
        drawing_width: f64,
        drawing_height: f64,
        placement_option: DrawingDataDescriptor,
        next_x: f64,
        next_y: f64,
    ) -> Self {
        let mut d = DrawingData {
            scale_measure: 0.0,
            drawing_width,
            drawing_height,
            area: 0.0,
            aspect_ratio: 0.0,
            dar,
            placement_option,
            next_x,
            next_y,
        };
        d.calc_area_aspect_ratio_scale_measure();
        d
    }

    /// Only recomputes when both dimensions are positive.
    fn calc_area_aspect_ratio_scale_measure(&mut self) {
        if self.drawing_width > 0.0 && self.drawing_height > 0.0 {
            self.area = self.drawing_width * self.drawing_height;
            self.aspect_ratio = self.drawing_width / self.drawing_height;
            self.scale_measure =
                compute_scale_measure(self.drawing_width, self.drawing_height, self.dar);
        }
    }

    pub fn drawing_width(&self) -> f64 {
        self.drawing_width
    }
    pub fn set_drawing_width(&mut self, w: f64) {
        self.drawing_width = w;
        self.calc_area_aspect_ratio_scale_measure();
    }
    pub fn drawing_height(&self) -> f64 {
        self.drawing_height
    }
    pub fn set_drawing_height(&mut self, h: f64) {
        self.drawing_height = h;
        self.calc_area_aspect_ratio_scale_measure();
    }
    pub fn scale_measure(&self) -> f64 {
        self.scale_measure
    }
    pub fn placement_option(&self) -> DrawingDataDescriptor {
        self.placement_option
    }
    pub fn set_placement_option(&mut self, opt: DrawingDataDescriptor) {
        self.placement_option = opt;
    }
    pub fn next_x(&self) -> f64 {
        self.next_x
    }
    pub fn next_y(&self) -> f64 {
        self.next_y
    }
    pub fn desired_aspect_ratio(&self) -> f64 {
        self.dar
    }
}

// ------------------------------------------------------------------ DrawingUtil

pub fn compute_scale_measure(width: f64, height: f64, dar: f64) -> f64 {
    f64::min(dar / width, 1.0 / height)
}

pub fn reset_coordinates(g: &mut ElkGraph, rects: &[NodeId]) {
    for &node in rects {
        g.node_mut(node).shape.set_location(0.0, 0.0);
    }
}

pub fn calculate_dimensions_rows(arena: &PackArena, rows: &[RowId], node_node_spacing: f64) -> KVector {
    let mut max_width = 0.0f64;
    let mut new_height = 0.0f64;
    for (index, &row) in rows.iter().enumerate() {
        max_width = f64::max(max_width, arena.row(row).width);
        new_height += arena.row(row).height + if index > 0 { node_node_spacing } else { 0.0 };
    }
    KVector::new(max_width, new_height)
}

pub fn calculate_dimensions_rects(g: &ElkGraph, rects: &[NodeId]) -> KVector {
    let mut max_width = 0.0f64;
    let mut max_height = 0.0f64;
    for &node in rects {
        let s = &g.node(node).shape;
        max_width = f64::max(s.width + s.x, max_width);
        max_height = f64::max(s.height + s.y, max_height);
    }
    KVector::new(max_width, max_height)
}

// ------------------------------------------------------------------ the arena

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RowId(pub usize);
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BlockId(pub usize);
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StackId(pub usize);

#[derive(Debug)]
pub struct RectRow {
    /// Height of row, given by the highest stack.
    pub height: f64,
    /// Sum of this row's stack's widths.
    pub width: f64,
    /// Y-coordinate of this row.
    pub y: f64,
    pub node_node_spacing: f64,
    /// This row's blocks.
    pub children: Vec<BlockId>,
    /// This row's stacks of blocks.
    pub stacks: Vec<StackId>,
}

#[derive(Debug)]
pub struct BlockRow {
    pub x: f64,
    pub y: f64,
    /// Width of the row + nodeNodeSpacing.
    pub width: f64,
    /// Height of the row (height of the biggest node).
    pub height: f64,
    pub node_node_spacing: f64,
    pub rects: Vec<NodeId>,
}

impl BlockRow {
    pub fn new(x: f64, y: f64, node_node_spacing: f64) -> Self {
        BlockRow { x, y, width: 0.0, height: 0.0, node_node_spacing, rects: Vec::new() }
    }

    pub fn add_rectangle(&mut self, g: &mut ElkGraph, rect: NodeId) {
        let spacing = if self.rects.is_empty() { 0.0 } else { self.node_node_spacing };
        let shape = &mut g.node_mut(rect).shape;
        shape.x = self.x + self.width + spacing;
        shape.y = self.y;
        self.height = f64::max(self.height, shape.height);
        self.width += shape.width + spacing;
        self.rects.push(rect);
    }

    pub fn remove_rectangle(&mut self, g: &mut ElkGraph, rect: NodeId, update: bool) {
        if let Some(pos) = self.rects.iter().position(|&r| r == rect) {
            self.rects.remove(pos);
        }
        if update {
            self.update_row(g);
        }
    }

    pub fn update_row(&mut self, g: &mut ElkGraph) {
        let mut width = 0.0f64;
        let mut height = 0.0f64;
        for &rect in &self.rects {
            let shape = &mut g.node_mut(rect).shape;
            shape.x = self.x + width;
            shape.y = self.y;
            width += shape.width + self.node_node_spacing;
            height = f64::max(height, shape.height + self.node_node_spacing);
        }
        self.width = width - self.node_node_spacing;
        self.height = height - self.node_node_spacing;
    }

    pub fn expand(&mut self, g: &mut ElkGraph, width_for_row: f64, additional_height_for_row: f64, index: usize) {
        let additional_width_for_rect = (width_for_row - self.width) / self.rects.len() as f64;
        let mut i = 0usize;
        self.height += additional_height_for_row;
        self.width = width_for_row;
        for &rect in &self.rects {
            let (old_width, old_height) = {
                let shape = &mut g.node_mut(rect).shape;
                let (ow, oh) = (shape.width, shape.height);
                shape.x += i as f64 * additional_width_for_rect;
                shape.y += index as f64 * additional_height_for_row;
                shape.width += additional_width_for_rect;
                shape.height = self.height;
                (ow, oh)
            };
            i += 1;
            let (new_width, new_height) = {
                let shape = &g.node(rect).shape;
                (shape.width, shape.height)
            };
            crate::core::elkutil::translate_aligned(
                g,
                rect,
                KVector::new(new_width, new_height),
                KVector::new(old_width, old_height),
            );
        }
    }
}

#[derive(Debug)]
pub struct Block {
    pub smallest_rect_width: f64,
    /// Minimal width + spacing. All rectangles are in one column.
    pub min_width: f64,
    /// Current width + spacing.
    pub width: f64,
    /// Minimal height + spacing. All rectangles are in one row.
    pub min_height: f64,
    /// Smallest rect height + spacing.
    pub smallest_rect_height: f64,
    /// Average block height.
    pub average_height: f64,
    /// Maximum height + spacing. All rectangles are in one column.
    pub max_height: f64,
    /// Current height + spacing.
    pub height: f64,
    pub children: Vec<NodeId>,
    pub rows: Vec<BlockRow>,
    pub x: f64,
    pub y: f64,
    pub parent_row: RowId,
    pub stack: Option<StackId>,
    pub node_node_spacing: f64,
    pub fixed: bool,
    pub position_fixed: bool,
}

#[derive(Debug)]
pub struct BlockStack {
    pub blocks: Vec<BlockId>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub node_node_spacing: f64,
}

/// Arena owning all rows, blocks and stacks of one packing run.
#[derive(Default, Debug)]
pub struct PackArena {
    pub rows: Vec<RectRow>,
    pub blocks: Vec<Block>,
    pub stacks: Vec<BlockStack>,
}

impl PackArena {
    pub fn row(&self, id: RowId) -> &RectRow {
        &self.rows[id.0]
    }
    pub fn row_mut(&mut self, id: RowId) -> &mut RectRow {
        &mut self.rows[id.0]
    }
    pub fn block(&self, id: BlockId) -> &Block {
        &self.blocks[id.0]
    }
    pub fn block_mut(&mut self, id: BlockId) -> &mut Block {
        &mut self.blocks[id.0]
    }
    pub fn stack(&self, id: StackId) -> &BlockStack {
        &self.stacks[id.0]
    }
    pub fn stack_mut(&mut self, id: StackId) -> &mut BlockStack {
        &mut self.stacks[id.0]
    }

    /// `new RectRow(y, nodeNodeSpacing)`.
    pub fn new_row(&mut self, y: f64, node_node_spacing: f64) -> RowId {
        self.rows.push(RectRow {
            height: 0.0,
            width: 0.0,
            y,
            node_node_spacing,
            children: Vec::new(),
            stacks: Vec::new(),
        });
        RowId(self.rows.len() - 1)
    }

    /// `new Block(xCoord, yCoord, parentRow, nodeNodeSpacing)`.
    pub fn new_block(&mut self, x: f64, y: f64, parent_row: RowId, node_node_spacing: f64) -> BlockId {
        self.blocks.push(Block {
            smallest_rect_width: f64::INFINITY,
            min_width: 0.0,
            width: 0.0,
            min_height: 0.0,
            smallest_rect_height: f64::INFINITY,
            average_height: 0.0,
            max_height: 0.0,
            height: 0.0,
            children: Vec::new(),
            rows: Vec::new(),
            x,
            y,
            parent_row,
            stack: None,
            node_node_spacing,
            fixed: false,
            position_fixed: false,
        });
        BlockId(self.blocks.len() - 1)
    }

    /// `new BlockStack(x, y, nodeNodeSpacing)`.
    pub fn new_stack(&mut self, x: f64, y: f64, node_node_spacing: f64) -> StackId {
        self.stacks.push(BlockStack {
            blocks: Vec::new(),
            x,
            y,
            width: 0.0,
            height: 0.0,
            node_node_spacing,
        });
        StackId(self.stacks.len() - 1)
    }

    // ------------------------------------------------------------- RectRow

    pub fn row_notify_about_node_change(&mut self, row: RowId) {
        let spacing = self.row(row).node_node_spacing;
        let mut total_stack_width = 0.0f64;
        let mut new_max_height = f64::NEG_INFINITY;
        for (index, &child) in self.row(row).children.iter().enumerate() {
            total_stack_width +=
                self.block(child).width + if index > 0 { spacing } else { 0.0 };
            new_max_height = f64::max(new_max_height, self.block(child).height);
        }
        let r = self.row_mut(row);
        r.width = total_stack_width;
        r.height = new_max_height;
    }

    pub fn row_expand(&mut self, g: &mut ElkGraph, row: RowId, width: f64, additional_height: f64) {
        let additional_width = width - self.row(row).width;
        let additional_width_per_stack = additional_width / self.row(row).stacks.len() as f64;
        let row_height = self.row(row).height;
        let stacks = self.row(row).stacks.clone();
        for (index, stack) in stacks.into_iter().enumerate() {
            let additional_height_for_stack =
                row_height - self.stack(stack).height + additional_height;
            let (sx, sy) = (self.stack(stack).x, self.stack(stack).y);
            self.stack_set_location(g, stack, sx + index as f64 * additional_width_per_stack, sy);
            self.stack_expand(g, stack, additional_width_per_stack, additional_height_for_stack);
        }
    }

    pub fn row_first_block(&self, row: RowId) -> BlockId {
        self.row(row).children[0]
    }

    pub fn row_last_block(&self, row: RowId) -> BlockId {
        *self.row(row).children.last().expect("row without blocks")
    }

    pub fn row_add_block(&mut self, row: RowId, block: BlockId) {
        let block_height = self.block(block).height;
        let block_width = self.block(block).width;
        let r = self.row_mut(row);
        r.height = f64::max(r.height, block_height);
        r.width += block_width + if r.children.is_empty() { 0.0 } else { r.node_node_spacing };
        r.children.push(block);
    }

    /// The width adjustment happens even if the block is not in the list.
    pub fn row_remove_block(&mut self, row: RowId, block: BlockId) {
        let block_width = self.block(block).width;
        let r = self.row_mut(row);
        if let Some(pos) = r.children.iter().position(|&b| b == block) {
            r.children.remove(pos);
        }
        r.width -=
            block_width + if r.children.is_empty() { 0.0 } else { r.node_node_spacing };

        // smallest positive double
        let mut new_max_height = f64::from_bits(1);
        let children = self.row(row).children.clone();
        for child in children {
            new_max_height = f64::max(new_max_height, self.block(child).height);
        }
        self.row_mut(row).height = new_max_height;
    }

    pub fn row_set_y(&mut self, g: &mut ElkGraph, row: RowId, y: f64) {
        let y_change = y - self.row(row).y;
        let stacks = self.row(row).stacks.clone();
        for stack in stacks {
            let (sx, sy) = (self.stack(stack).x, self.stack(stack).y);
            self.stack_set_location(g, stack, sx, sy + y_change);
        }
        self.row_mut(row).y = y;
    }

    pub fn row_reset_stacks(&mut self, row: RowId) {
        self.row_mut(row).stacks = Vec::new();
    }

    // --------------------------------------------------------------- Block

    pub fn block_add_child(&mut self, g: &mut ElkGraph, block: BlockId, rect: NodeId) {
        if self.block(block).rows.is_empty() {
            let (x, y, spacing) = {
                let b = self.block(block);
                (b.x, b.y, b.node_node_spacing)
            };
            self.block_mut(block).rows.push(BlockRow::new(x, y, spacing));
        }
        self.block_mut(block).children.push(rect);
        self.block_mut(block)
            .rows
            .last_mut()
            .unwrap()
            .add_rectangle(g, rect);
        self.block_adjust_size_add(g, block, rect);
    }

    pub fn block_add_child_in_new_row(&mut self, g: &mut ElkGraph, block: BlockId, rect: NodeId) {
        self.block_mut(block).children.push(rect);
        let (x, new_y, spacing) = {
            let b = self.block(block);
            let last = b.rows.last().expect("block without rows");
            (b.x, last.y + last.height + b.node_node_spacing, b.node_node_spacing)
        };
        self.block_mut(block).rows.push(BlockRow::new(x, new_y, spacing));
        self.block_mut(block)
            .rows
            .last_mut()
            .unwrap()
            .add_rectangle(g, rect);
        self.block_adjust_size_add(g, block, rect);
    }

    pub fn block_remove_child(&mut self, g: &mut ElkGraph, block: BlockId, rect: NodeId) {
        if let Some(pos) = self.block(block).children.iter().position(|&r| r == rect) {
            self.block_mut(block).children.remove(pos);
        }
        let mut row_to_delete: Option<usize> = None;
        for (i, row) in self.block_mut(block).rows.iter_mut().enumerate() {
            if row.rects.contains(&rect) {
                row.remove_rectangle(g, rect, true);
                if row.rects.is_empty() {
                    row_to_delete = Some(i);
                }
                break;
            }
        }
        if let Some(i) = row_to_delete {
            self.block_mut(block).rows.remove(i);
        }
        self.block_adjust_size_after_remove(g, block);
    }

    pub fn block_set_location(&mut self, g: &mut ElkGraph, block: BlockId, x: f64, y: f64) {
        let (x_change, y_change) = {
            let b = self.block(block);
            (x - b.x, y - b.y)
        };
        // adjustChildrensXandY
        let children = self.block(block).children.clone();
        for rect in children {
            let shape = &mut g.node_mut(rect).shape;
            shape.x += x_change;
            shape.y += y_change;
        }
        let b = self.block_mut(block);
        for row in &mut b.rows {
            row.x += x_change;
            row.y += y_change;
        }
        b.x = x;
        b.y = y;
    }

    fn block_adjust_size_add(&mut self, g: &mut ElkGraph, block: BlockId, rect: NodeId) {
        let (rect_width, rect_height) = {
            let s = &g.node(rect).shape;
            (s.width, s.height)
        };
        let parent_row = {
            let b = self.block_mut(block);
            let width_of_last_row = b.rows.last().expect("block without rows").width;
            let n = b.children.len() as f64;
            let spacing = b.node_node_spacing;
            b.smallest_rect_width = f64::min(b.smallest_rect_width, rect_width);
            b.width = f64::max(b.width, width_of_last_row);
            b.min_width = f64::max(
                b.min_width,
                rect_width + if b.children.len() == 1 { 0.0 } else { spacing },
            );
            b.smallest_rect_height = f64::min(b.smallest_rect_height, rect_height);
            b.max_height += rect_height + if b.children.len() == 1 { 0.0 } else { spacing };
            b.min_height = f64::max(b.min_height, rect_height);
            let mut total_height = if !b.rows.is_empty() {
                (b.rows.len() - 1) as f64 * spacing
            } else {
                0.0
            };
            for row in &b.rows {
                total_height += row.height;
            }
            b.height = total_height;
            b.average_height = b.max_height / n - spacing * ((n - 1.0) / n);
            b.parent_row
        };
        self.row_notify_about_node_change(parent_row);
    }

    pub fn block_get_width_for_target_height(&self, g: &ElkGraph, block: BlockId, height: f64) -> f64 {
        let b = self.block(block);
        // Check whether the block would just fit if all rectangles are drawn
        // below each other.
        if b.max_height <= height {
            return b.min_width;
        }
        // Check if the minimal width of the block is enough.
        if self.block_fits_in(g, block, b.min_width, height) {
            return b.min_width;
        }
        // Binary search between minWidth and width.
        let mut upper_bound = b.width;
        let mut lower_bound = b.min_width;
        let mut viable_width = b.width;
        let mut new_width = (upper_bound - lower_bound) / 2.0 + lower_bound;
        while lower_bound + 1.0 < upper_bound {
            if self.block_fits_in(g, block, new_width, height) {
                viable_width = new_width;
                upper_bound = new_width;
            } else {
                lower_bound = new_width;
            }
            new_width = (upper_bound - lower_bound) / 2.0 + lower_bound;
        }
        viable_width
    }

    pub fn block_get_height_for_target_width(&self, g: &ElkGraph, block: BlockId, width: f64) -> f64 {
        self.block_simulate_rects_in(g, block, width).height
    }

    /// The non-placing core of `Block.placeRectsIn(width, placeRects=false)`.
    fn block_simulate_rects_in(&self, g: &ElkGraph, block: BlockId, width: f64) -> ElkRectangle {
        let b = self.block(block);
        let mut current_x = 0.0f64;
        let mut current_width = 0.0f64;
        let mut current_height = 0.0f64;
        let mut max_height_in_row = 0.0f64;
        let mut width_in_row = 0.0f64;
        let mut index = 0usize;
        for &rect in &b.children {
            let s = &g.node(rect).shape;
            let spacing = if index > 0 { b.node_node_spacing } else { 0.0 };
            if current_x + s.width + spacing > width && max_height_in_row > 0.0 {
                current_x = 0.0;
                current_width = f64::max(current_width, width_in_row);
                current_height += max_height_in_row + b.node_node_spacing;
                max_height_in_row = 0.0;
                width_in_row = 0.0;
                index = 0;
            }
            let spacing = if index > 0 { b.node_node_spacing } else { 0.0 };
            width_in_row += s.width + spacing;
            max_height_in_row = f64::max(max_height_in_row, s.height);
            current_x += s.width + spacing;
            index += 1;
        }
        current_width = f64::max(current_width, width_in_row);
        current_height += max_height_in_row;
        ElkRectangle::new(b.x, b.y, current_width, current_height)
    }

    /// The placing core of `Block.placeRectsIn(width, placeRects=true)`.
    fn block_place_rects_in_impl(&mut self, g: &mut ElkGraph, block: BlockId, width: f64) -> ElkRectangle {
        let (bx, by, spacing, children) = {
            let b = self.block(block);
            (b.x, b.y, b.node_node_spacing, b.children.clone())
        };
        let mut current_x = 0.0f64;
        let mut current_y = by;
        let mut current_width = 0.0f64;
        let mut current_height = 0.0f64;
        let mut max_height_in_row = 0.0f64;
        let mut width_in_row = 0.0f64;
        let mut row = 0usize;
        self.block_mut(block).rows.clear();
        self.block_mut(block).rows.push(BlockRow::new(bx, by, spacing));
        let mut index = 0usize;
        for rect in children {
            let (rect_width, rect_height) = {
                let s = &g.node(rect).shape;
                (s.width, s.height)
            };
            if current_x + rect_width + (if index > 0 { spacing } else { 0.0 }) > width
                && max_height_in_row > 0.0
            {
                // Case new row
                current_x = 0.0;
                current_y += max_height_in_row + spacing;
                current_width = f64::max(current_width, width_in_row);
                current_height += max_height_in_row + spacing;
                max_height_in_row = 0.0;
                width_in_row = 0.0;
                row += 1;
                self.block_mut(block).rows.push(BlockRow::new(bx, current_y, spacing));
                index = 0;
            }
            width_in_row += rect_width + if index > 0 { spacing } else { 0.0 };
            max_height_in_row = f64::max(max_height_in_row, rect_height);
            self.block_mut(block).rows[row].add_rectangle(g, rect);
            current_x += rect_width + if index > 0 { spacing } else { 0.0 };
            index += 1;
        }
        current_width = f64::max(current_width, width_in_row);
        current_height += max_height_in_row;
        let parent_row = {
            let b = self.block_mut(block);
            b.width = current_width;
            b.height = current_height;
            b.parent_row
        };
        self.row_notify_about_node_change(parent_row);
        ElkRectangle::new(bx, by, current_width, current_height)
    }

    fn block_fits_in(&self, g: &ElkGraph, block: BlockId, width: f64, height: f64) -> bool {
        let bounds = self.block_simulate_rects_in(g, block, width);
        bounds.width <= width && bounds.height <= height
    }

    pub fn block_place_rects_in_bounds(
        &mut self,
        g: &mut ElkGraph,
        block: BlockId,
        width: f64,
        height: f64,
    ) -> bool {
        let bounds = self.block_place_rects_in_impl(g, block, width);
        bounds.width <= width && bounds.height <= height
    }

    pub fn block_place_rects_in(&mut self, g: &mut ElkGraph, block: BlockId, width: f64) -> bool {
        let (old_width, old_height) = {
            let b = self.block(block);
            (b.width, b.height)
        };
        let bounds = self.block_place_rects_in_impl(g, block, width);
        bounds.width != old_width || bounds.height != old_height
    }

    pub fn block_adjust_size_after_remove(&mut self, g: &mut ElkGraph, block: BlockId) {
        let parent_row = {
            let b = self.block_mut(block);
            let spacing = b.node_node_spacing;
            let mut new_width = 0.0f64;
            let mut new_height = 0.0f64;
            let mut keep: Vec<bool> = Vec::with_capacity(b.rows.len());
            for (index, row) in b.rows.iter().enumerate() {
                if row.rects.is_empty() {
                    keep.push(false);
                } else {
                    keep.push(true);
                    new_width = f64::max(new_width, row.width);
                    new_height += row.height + if index > 0 { spacing } else { 0.0 };
                }
            }
            let mut keep_iter = keep.into_iter();
            b.rows.retain(|_| keep_iter.next().unwrap());
            b.height = new_height;
            b.width = new_width;

            b.min_width = 0.0;
            b.min_height = 0.0;
            b.max_height = 0.0;
            b.smallest_rect_height = f64::INFINITY;
            b.smallest_rect_width = f64::INFINITY;
            for &rect in &b.children {
                let s = &g.node(rect).shape;
                b.smallest_rect_width = f64::min(b.smallest_rect_width, s.width);
                b.min_width = f64::max(b.min_width, s.width);
                b.min_height = f64::max(b.min_height, s.height);
                b.smallest_rect_height = f64::min(b.smallest_rect_height, s.height);
                b.max_height += s.height + spacing;
            }
            let n = b.children.len() as f64;
            b.average_height = b.max_height / n - spacing * ((n - 1.0) / n);
            b.parent_row
        };
        self.row_notify_about_node_change(parent_row);
    }

    pub fn block_expand(
        &mut self,
        g: &mut ElkGraph,
        block: BlockId,
        additional_width_per_block: f64,
        additional_height_for_block: f64,
    ) {
        let (width_for_row, additional_height_for_row, num_rows) = {
            let b = self.block_mut(block);
            let width_for_row = b.width + additional_width_per_block;
            b.width += additional_width_per_block;
            b.height += additional_height_for_block;
            (width_for_row, additional_height_for_block / b.rows.len() as f64, b.rows.len())
        };
        for index in 0..num_rows {
            // Take the row out to satisfy the borrow checker; expand only
            // touches the row itself and the graph.
            let mut row = std::mem::replace(
                &mut self.block_mut(block).rows[index],
                BlockRow::new(0.0, 0.0, 0.0),
            );
            row.expand(g, width_for_row, additional_height_for_row, index);
            self.block_mut(block).rows[index] = row;
        }
    }

    pub fn block_last_row_new_x(&self, block: BlockId) -> f64 {
        let last = self.block(block).rows.last().expect("block without rows");
        last.x + last.width
    }

    pub fn block_last_row_y(&self, block: BlockId) -> f64 {
        self.block(block).rows.last().expect("block without rows").y
    }

    // ---------------------------------------------------------- BlockStack

    pub fn stack_add_block(&mut self, stack: StackId, block: BlockId) {
        self.block_mut(block).stack = Some(stack);
        let (block_width, block_height) = {
            let b = self.block(block);
            (b.width, b.height)
        };
        let s = self.stack_mut(stack);
        s.width = f64::max(s.width, block_width);
        s.height +=
            block_height + if s.blocks.is_empty() { 0.0 } else { s.node_node_spacing };
        s.blocks.push(block);
    }

    pub fn stack_update_dimension(&mut self, stack: StackId) {
        let mut height = 0.0f64;
        let mut width = 0.0f64;
        let spacing = self.stack(stack).node_node_spacing;
        for (index, &block) in self.stack(stack).blocks.iter().enumerate() {
            let b = self.block(block);
            width = f64::max(width, b.width);
            height += b.height + if index > 0 { spacing } else { 0.0 };
        }
        let s = self.stack_mut(stack);
        s.height = height;
        s.width = width;
    }

    pub fn stack_set_location(&mut self, g: &mut ElkGraph, stack: StackId, x: f64, y: f64) {
        let (x_diff, y_diff) = {
            let s = self.stack(stack);
            (x - s.x, y - s.y)
        };
        let blocks = self.stack(stack).blocks.clone();
        for block in blocks {
            let (bx, by) = (self.block(block).x, self.block(block).y);
            self.block_set_location(g, block, bx + x_diff, by + y_diff);
        }
        let s = self.stack_mut(stack);
        s.x = x;
        s.y = y;
    }

    pub fn stack_get_width_for_fixed_height(&self, g: &ElkGraph, stack: StackId, height: f64) -> f64 {
        let s = self.stack(stack);
        // One element special case.
        if s.blocks.len() == 1 {
            return self.block_get_width_for_target_height(g, s.blocks[0], height);
        }
        // Binary search between the widest block length and the minWidth.
        let min_width = self.stack_get_minimum_width(stack);
        let mut total_height;
        let mut upper_bound = s.width;
        let mut lower_bound = min_width;
        let mut viable_width = s.width;
        let mut new_width = (upper_bound - lower_bound) / 2.0 + lower_bound;
        while lower_bound + 1.0 < upper_bound {
            total_height = 0.0;
            for &block in &s.blocks {
                total_height += self.block_get_height_for_target_width(g, block, new_width);
            }
            if total_height < height {
                viable_width = new_width;
                upper_bound = new_width;
            } else {
                lower_bound = new_width;
            }
            new_width = (upper_bound - lower_bound) / 2.0 + lower_bound;
        }
        viable_width
    }

    pub fn stack_place_rects_in(&mut self, g: &mut ElkGraph, stack: StackId, target_width: f64) {
        let (sx, sy, spacing, blocks) = {
            let s = self.stack(stack);
            (s.x, s.y, s.node_node_spacing, s.blocks.clone())
        };
        let mut current_y = sy;
        let mut current_height = 0.0f64;
        let mut current_width = 0.0f64;
        for block in blocks {
            self.block_set_location(g, block, sx, current_y);
            self.block_place_rects_in(g, block, target_width);
            let b = self.block(block);
            current_width = f64::max(current_width, b.width);
            current_y += b.height + spacing;
            current_height = current_y;
        }
        let s = self.stack_mut(stack);
        s.width = current_width;
        s.height = current_height;
    }

    pub fn stack_expand(
        &mut self,
        g: &mut ElkGraph,
        stack: StackId,
        additional_width: f64,
        additional_height: f64,
    ) {
        let blocks = self.stack(stack).blocks.clone();
        let additional_height_per_block = additional_height / blocks.len() as f64;
        let stack_width = self.stack(stack).width;
        for (index, block) in blocks.into_iter().enumerate() {
            let (bx, by) = (self.block(block).x, self.block(block).y);
            self.block_set_location(g, block, bx, by + index as f64 * additional_height_per_block);
            let block_width = self.block(block).width;
            self.block_expand(
                g,
                block,
                stack_width - block_width + additional_width,
                additional_height_per_block,
            );
        }
    }

    fn stack_get_minimum_width(&self, stack: StackId) -> f64 {
        let mut min_width = 0.0f64;
        for &block in &self.stack(stack).blocks {
            min_width = f64::max(min_width, self.block(block).min_width);
        }
        min_width
    }
}
