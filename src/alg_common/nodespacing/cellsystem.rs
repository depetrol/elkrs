//!
//! All cells live in a [`CellSystem`] arena and are referenced by
//! [`CellId`]. Container cells reference their children by id.

use crate::core::adapters::AdapterGraph;
use crate::graph::math::{ElkPadding, ElkRectangle, KVector};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContainerArea {
    /// The top row or left column of the container.
    Begin,
    /// The center row or column of the container.
    Center,
    /// The bottom row or right column of the container.
    End,
}

impl ContainerArea {
    pub const VALUES: [ContainerArea; 3] =
        [ContainerArea::Begin, ContainerArea::Center, ContainerArea::End];

    pub fn ordinal(self) -> usize {
        match self {
            ContainerArea::Begin => 0,
            ContainerArea::Center => 1,
            ContainerArea::End => 2,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HorizontalLabelAlignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VerticalLabelAlignment {
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strip {
    /// In a vertical strip, the container's child cells are its rows.
    Vertical,
    /// In a horizontal strip, the container's child cells are its columns.
    Horizontal,
}

/// Identifies a cell within a [`CellSystem`].
pub type CellId = usize;

/// Payload of an `AtomicCell`.
pub struct AtomicData {
    /// The minimum size of the cell's content area (excludes the padding).
    pub min_content_area_size: KVector,
}

/// Payload of a `LabelCell`.
pub struct LabelData<L> {
    /// Whether we operate in horizontal or vertical layout mode.
    pub horizontal_layout_mode: bool,
    /// Horizontal alignment of labels.
    pub horizontal_alignment: HorizontalLabelAlignment,
    /// Vertical alignment of labels.
    pub vertical_alignment: VerticalLabelAlignment,
    /// The gap inserted between two consecutive labels.
    pub gap: f64,
    /// The labels in this cell.
    pub labels: Vec<L>,
    /// Minimum space needed to place the labels.
    pub min_content_area_size: KVector,
}

/// Payload of a `StripContainerCell`.
pub struct StripData {
    /// Whether we lay children out in rows or columns.
    pub container_mode: Strip,
    /// Whether the outer cells should be the same width or height.
    pub symmetrical: bool,
    /// Gap between consecutive cells.
    pub gap: f64,
    /// Child cells, indexed by `ContainerArea` ordinal.
    pub cells: [Option<CellId>; 3],
}

/// Payload of a `GridContainerCell`.
pub struct GridData {
    /// Whether the container works in tabular mode or not.
    pub tabular: bool,
    /// Whether the outer columns/rows should be the same width/height.
    pub symmetrical: bool,
    /// Gap between consecutive cells.
    pub gap: f64,
    /// Child cells, indexed `[row][column]` by `ContainerArea` ordinal.
    pub cells: [[Option<CellId>; 3]; 3],
    /// Custom minimum size for the center cell, if set.
    pub center_cell_minimum_size: Option<KVector>,
    /// Whether only the center cell contributes to the minimum size.
    pub only_center_cell_contributes_to_minimum_size: bool,
    /// Rectangle that describes the part of the grid free of outer grid cells.
    pub center_cell_rect: ElkRectangle,
}

/// What kind of cell a [`CellData`] is.
pub enum CellKind<L> {
    Atomic(AtomicData),
    Label(LabelData<L>),
    Strip(StripData),
    Grid(GridData),
}

/// Padding, rectangle, contribution flags, plus the
/// kind-specific payload.
pub struct CellData<L> {
    /// A cell has a padding.
    pub padding: ElkPadding,
    /// The actual size and position of the cell. Includes the padding.
    pub rect: ElkRectangle,
    /// Whether the cell contributes to a container's minimum width.
    pub contributes_to_minimum_width: bool,
    /// Whether the cell contributes to a container's minimum height.
    pub contributes_to_minimum_height: bool,
    /// Kind-specific data.
    pub kind: CellKind<L>,
}

/// Arena holding all cells of one node's cell system. `L` is the label handle
/// type of the graph adapter the system is built for.
pub struct CellSystem<L> {
    cells: Vec<CellData<L>>,
}

impl<L: Copy> CellSystem<L> {
    pub fn new() -> Self {
        CellSystem { cells: Vec::new() }
    }

    fn push(&mut self, kind: CellKind<L>) -> CellId {
        self.cells.push(CellData {
            padding: ElkPadding::default(),
            rect: ElkRectangle::default(),
            contributes_to_minimum_width: false,
            contributes_to_minimum_height: false,
            kind,
        });
        self.cells.len() - 1
    }

    /// Creates a new `AtomicCell`.
    pub fn new_atomic(&mut self) -> CellId {
        self.push(CellKind::Atomic(AtomicData { min_content_area_size: KVector::default() }))
    }

    /// Creates a new `LabelCell` (default alignments are CENTER/CENTER).
    pub fn new_label(&mut self, gap: f64, horizontal_layout_mode: bool) -> CellId {
        self.push(CellKind::Label(LabelData {
            horizontal_layout_mode,
            horizontal_alignment: HorizontalLabelAlignment::Center,
            vertical_alignment: VerticalLabelAlignment::Center,
            gap,
            labels: Vec::new(),
            min_content_area_size: KVector::default(),
        }))
    }

    /// Creates a new `StripContainerCell`.
    pub fn new_strip(&mut self, mode: Strip, symmetrical: bool, gap: f64) -> CellId {
        self.push(CellKind::Strip(StripData {
            container_mode: mode,
            symmetrical,
            gap,
            cells: [None; 3],
        }))
    }

    /// Creates a new `GridContainerCell`.
    pub fn new_grid(&mut self, tabular: bool, symmetrical: bool, gap: f64) -> CellId {
        self.push(CellKind::Grid(GridData {
            tabular,
            symmetrical,
            gap,
            cells: [[None; 3]; 3],
            center_cell_minimum_size: None,
            only_center_cell_contributes_to_minimum_size: false,
            center_cell_rect: ElkRectangle::default(),
        }))
    }

    // ------------------------------------------------------------ accessors

    pub fn cell(&self, id: CellId) -> &CellData<L> {
        &self.cells[id]
    }

    pub fn cell_mut(&mut self, id: CellId) -> &mut CellData<L> {
        &mut self.cells[id]
    }

    pub fn padding(&self, id: CellId) -> ElkPadding {
        self.cells[id].padding
    }

    pub fn padding_mut(&mut self, id: CellId) -> &mut ElkPadding {
        &mut self.cells[id].padding
    }

    pub fn rect(&self, id: CellId) -> ElkRectangle {
        self.cells[id].rect
    }

    pub fn rect_mut(&mut self, id: CellId) -> &mut ElkRectangle {
        &mut self.cells[id].rect
    }

    pub fn set_contributes_to_minimum_width(&mut self, id: CellId, v: bool) {
        self.cells[id].contributes_to_minimum_width = v;
    }

    pub fn set_contributes_to_minimum_height(&mut self, id: CellId, v: bool) {
        self.cells[id].contributes_to_minimum_height = v;
    }

    pub fn is_contributing_to_minimum_width(&self, id: CellId) -> bool {
        self.cells[id].contributes_to_minimum_width
    }

    pub fn is_contributing_to_minimum_height(&self, id: CellId) -> bool {
        self.cells[id].contributes_to_minimum_height
    }

    fn atomic(&self, id: CellId) -> &AtomicData {
        match &self.cells[id].kind {
            CellKind::Atomic(a) => a,
            _ => panic!("cell {id} is not an AtomicCell"),
        }
    }

    /// The minimum content area size of an `AtomicCell`, for mutation.
    pub fn atomic_min_content_area_size_mut(&mut self, id: CellId) -> &mut KVector {
        match &mut self.cells[id].kind {
            CellKind::Atomic(a) => &mut a.min_content_area_size,
            _ => panic!("cell {id} is not an AtomicCell"),
        }
    }

    pub fn atomic_min_content_area_size(&self, id: CellId) -> KVector {
        self.atomic(id).min_content_area_size
    }

    pub fn label(&self, id: CellId) -> &LabelData<L> {
        match &self.cells[id].kind {
            CellKind::Label(l) => l,
            _ => panic!("cell {id} is not a LabelCell"),
        }
    }

    pub fn label_mut(&mut self, id: CellId) -> &mut LabelData<L> {
        match &mut self.cells[id].kind {
            CellKind::Label(l) => l,
            _ => panic!("cell {id} is not a LabelCell"),
        }
    }

    fn strip(&self, id: CellId) -> &StripData {
        match &self.cells[id].kind {
            CellKind::Strip(s) => s,
            _ => panic!("cell {id} is not a StripContainerCell"),
        }
    }

    fn strip_mut(&mut self, id: CellId) -> &mut StripData {
        match &mut self.cells[id].kind {
            CellKind::Strip(s) => s,
            _ => panic!("cell {id} is not a StripContainerCell"),
        }
    }

    pub fn grid(&self, id: CellId) -> &GridData {
        match &self.cells[id].kind {
            CellKind::Grid(g) => g,
            _ => panic!("cell {id} is not a GridContainerCell"),
        }
    }

    pub fn grid_mut(&mut self, id: CellId) -> &mut GridData {
        match &mut self.cells[id].kind {
            CellKind::Grid(g) => g,
            _ => panic!("cell {id} is not a GridContainerCell"),
        }
    }

    pub fn strip_set_cell(&mut self, id: CellId, area: ContainerArea, cell: CellId) {
        self.strip_mut(id).cells[area.ordinal()] = Some(cell);
    }

    pub fn strip_get_cell(&self, id: CellId, area: ContainerArea) -> Option<CellId> {
        self.strip(id).cells[area.ordinal()]
    }

    pub fn strip_gap(&self, id: CellId) -> f64 {
        self.strip(id).gap
    }

    pub fn grid_set_cell(&mut self, id: CellId, row: ContainerArea, col: ContainerArea, cell: CellId) {
        self.grid_mut(id).cells[row.ordinal()][col.ordinal()] = Some(cell);
    }

    pub fn grid_get_cell(&self, id: CellId, row: ContainerArea, col: ContainerArea) -> Option<CellId> {
        self.grid(id).cells[row.ordinal()][col.ordinal()]
    }

    pub fn grid_gap(&self, id: CellId) -> f64 {
        self.grid(id).gap
    }

    pub fn grid_set_center_cell_minimum_size(&mut self, id: CellId, minimum_size: KVector) {
        self.grid_mut(id).center_cell_minimum_size = Some(minimum_size);
    }

    pub fn grid_set_only_center_cell_contributes(&mut self, id: CellId, contribution: bool) {
        self.grid_mut(id).only_center_cell_contributes_to_minimum_size = contribution;
    }

    pub fn grid_center_cell_rectangle(&self, id: CellId) -> ElkRectangle {
        self.grid(id).center_cell_rect
    }

    // -------------------------------------------------------------- LabelCell

    /// The label's size must be passed in since
    /// the arena has no access to the graph adapter.
    pub fn label_add_label(&mut self, id: CellId, label: L, label_size: KVector) {
        let data = self.label_mut(id);
        data.labels.push(label);

        if data.horizontal_layout_mode {
            data.min_content_area_size.x = data.min_content_area_size.x.max(label_size.x);
            data.min_content_area_size.y += label_size.y;

            // If this is not our first label, insert a gap
            if data.labels.len() > 1 {
                data.min_content_area_size.y += data.gap;
            }
        } else {
            data.min_content_area_size.x += label_size.x;
            data.min_content_area_size.y = data.min_content_area_size.y.max(label_size.y);

            // If this is not our first label, insert a gap
            if data.labels.len() > 1 {
                data.min_content_area_size.x += data.gap;
            }
        }
    }

    pub fn label_has_labels(&self, id: CellId) -> bool {
        !self.label(id).labels.is_empty()
    }

    // ---------------------------------------------------------- minimum size

    pub fn min_width(&self, id: CellId) -> f64 {
        let cell = &self.cells[id];
        let padding = cell.padding;
        match &cell.kind {
            CellKind::Atomic(a) => a.min_content_area_size.x + padding.left + padding.right,
            CellKind::Label(l) => l.min_content_area_size.x + padding.left + padding.right,
            CellKind::Strip(s) => {
                let mut width = 0.0f64;
                if s.container_mode == Strip::Vertical {
                    // Take the maximum of the child cells. Note that the
                    // contribution flag is used directly here, without the
                    // atomic-cell-with-zero-content special case.
                    for cell_id in s.cells.iter().flatten() {
                        if self.is_contributing_to_minimum_width(*cell_id) {
                            width = width.max(self.min_width(*cell_id));
                        }
                    }
                } else {
                    let cell_widths = self.strip_min_cell_widths(id, true);
                    let mut active_cells = 0;
                    for cell_width in cell_widths {
                        if cell_width > 0.0 {
                            width += cell_width;
                            active_cells += 1;
                        }
                    }
                    if active_cells > 1 {
                        width += s.gap * (active_cells - 1) as f64;
                    }
                }
                // If we don't have cells, we don't have width
                if width > 0.0 {
                    width + padding.left + padding.right
                } else {
                    0.0
                }
            }
            CellKind::Grid(g) => {
                let mut width = 0.0f64;
                if g.only_center_cell_contributes_to_minimum_size {
                    if let Some(min_size) = g.center_cell_minimum_size {
                        width = min_size.x;
                    } else if let Some(center) = g.cells[1][1] {
                        width = self.min_width(center);
                    }
                } else if g.tabular {
                    // Use aggregated widths
                    width = sum_with_gaps(&self.grid_min_column_widths(id, None, true), g.gap);
                } else {
                    // Use maximum width over each row
                    for area in ContainerArea::VALUES {
                        width = width.max(sum_with_gaps(
                            &self.grid_min_column_widths(id, Some(area), true),
                            g.gap,
                        ));
                    }
                }
                if width > 0.0 {
                    width + padding.left + padding.right
                } else {
                    0.0
                }
            }
        }
    }

    pub fn min_height(&self, id: CellId) -> f64 {
        let cell = &self.cells[id];
        let padding = cell.padding;
        match &cell.kind {
            CellKind::Atomic(a) => a.min_content_area_size.y + padding.top + padding.bottom,
            CellKind::Label(l) => l.min_content_area_size.y + padding.top + padding.bottom,
            CellKind::Strip(s) => {
                let mut height = 0.0f64;
                if s.container_mode == Strip::Vertical {
                    let cell_heights = self.strip_min_cell_heights(id, true);
                    let mut active_cells = 0;
                    for cell_height in cell_heights {
                        if cell_height > 0.0 {
                            height += cell_height;
                            active_cells += 1;
                        }
                    }
                    if active_cells > 1 {
                        height += s.gap * (active_cells - 1) as f64;
                    }
                } else {
                    // Take the maximum of the child cells (see min_width note)
                    for cell_id in s.cells.iter().flatten() {
                        if self.is_contributing_to_minimum_height(*cell_id) {
                            height = height.max(self.min_height(*cell_id));
                        }
                    }
                }
                if height > 0.0 {
                    height + padding.top + padding.bottom
                } else {
                    0.0
                }
            }
            CellKind::Grid(g) => {
                let mut height = 0.0f64;
                if g.only_center_cell_contributes_to_minimum_size {
                    if let Some(min_size) = g.center_cell_minimum_size {
                        height = min_size.y;
                    } else if let Some(center) = g.cells[1][1] {
                        height = self.min_height(center);
                    }
                } else {
                    // Minimum height of the different rows (independent of tabular mode)
                    height = sum_with_gaps(&self.grid_min_row_heights(id, true), g.gap);
                }
                if height > 0.0 {
                    height + padding.top + padding.bottom
                } else {
                    0.0
                }
            }
        }
    }

    fn min_width_of_cell(&self, cell: Option<CellId>, respect_contribution_flag: bool) -> f64 {
        // If there's no cell, there's no minimum width
        let Some(id) = cell else { return 0.0 };

        // If the cell doesn't have its contribution flag activated, there's no minimum width
        if respect_contribution_flag && !self.is_contributing_to_minimum_width(id) {
            return 0.0;
        }

        // If the cell is an atomic cell with a content area of no width, there's no minimum width
        if let CellKind::Atomic(a) = &self.cells[id].kind {
            if a.min_content_area_size.x == 0.0 {
                return 0.0;
            }
        }

        self.min_width(id)
    }

    fn min_height_of_cell(&self, cell: Option<CellId>, respect_contribution_flag: bool) -> f64 {
        let Some(id) = cell else { return 0.0 };

        if respect_contribution_flag && !self.is_contributing_to_minimum_height(id) {
            return 0.0;
        }

        if let CellKind::Atomic(a) = &self.cells[id].kind {
            if a.min_content_area_size.y == 0.0 {
                return 0.0;
            }
        }

        self.min_height(id)
    }

    fn apply_horizontal_layout(&mut self, cell: Option<CellId>, x: f64, width: f64) {
        if let Some(id) = cell {
            let rect = &mut self.cells[id].rect;
            rect.x = x;
            rect.width = width;
        }
    }

    fn apply_vertical_layout(&mut self, cell: Option<CellId>, y: f64, height: f64) {
        if let Some(id) = cell {
            let rect = &mut self.cells[id].rect;
            rect.y = y;
            rect.height = height;
        }
    }

    // -------------------------------------------- StripContainerCell layout

    fn strip_min_cell_widths(&self, id: CellId, respect_contribution_flag: bool) -> [f64; 3] {
        let s = self.strip(id);
        let mut cell_widths = [
            self.min_width_of_cell(s.cells[0], respect_contribution_flag),
            self.min_width_of_cell(s.cells[1], respect_contribution_flag),
            self.min_width_of_cell(s.cells[2], respect_contribution_flag),
        ];
        // If we are to be symmetrical, the outer cells need to be the same size
        if s.symmetrical {
            cell_widths[0] = cell_widths[0].max(cell_widths[2]);
            cell_widths[2] = cell_widths[0];
        }
        cell_widths
    }

    fn strip_min_cell_heights(&self, id: CellId, respect_contribution_flag: bool) -> [f64; 3] {
        let s = self.strip(id);
        let mut cell_heights = [
            self.min_height_of_cell(s.cells[0], respect_contribution_flag),
            self.min_height_of_cell(s.cells[1], respect_contribution_flag),
            self.min_height_of_cell(s.cells[2], respect_contribution_flag),
        ];
        if s.symmetrical {
            cell_heights[0] = cell_heights[0].max(cell_heights[2]);
            cell_heights[2] = cell_heights[0];
        }
        cell_heights
    }

    pub fn layout_children_horizontally(&mut self, id: CellId) {
        match &self.cells[id].kind {
            CellKind::Strip(_) => self.strip_layout_children_horizontally(id),
            CellKind::Grid(_) => self.grid_layout_children_horizontally(id),
            _ => panic!("cell {id} is not a container cell"),
        }
    }

    pub fn layout_children_vertically(&mut self, id: CellId) {
        match &self.cells[id].kind {
            CellKind::Strip(_) => self.strip_layout_children_vertically(id),
            CellKind::Grid(_) => self.grid_layout_children_vertically(id),
            _ => panic!("cell {id} is not a container cell"),
        }
    }

    fn is_container(&self, id: CellId) -> bool {
        matches!(self.cells[id].kind, CellKind::Strip(_) | CellKind::Grid(_))
    }

    fn strip_layout_children_horizontally(&mut self, id: CellId) {
        let cell_rectangle = self.cells[id].rect;
        let cell_padding = self.cells[id].padding;
        let s = self.strip(id);
        let mode = s.container_mode;
        let gap = s.gap;
        let children = s.cells;

        if mode == Strip::Vertical {
            // Each child cell begins at our left border (plus padding) and is as
            // large as our content area
            let x_pos = cell_rectangle.x + cell_padding.left;
            let width = cell_rectangle.width - cell_padding.left - cell_padding.right;

            for child_cell in children {
                self.apply_horizontal_layout(child_cell, x_pos, width);
            }
        } else {
            let mut cell_widths = self.strip_min_cell_widths(id, false);

            // Left cell is left-aligned with our content area, right cell is right-aligned
            self.apply_horizontal_layout(
                children[0],
                cell_rectangle.x + cell_padding.left,
                cell_widths[0],
            );
            self.apply_horizontal_layout(
                children[2],
                cell_rectangle.x + cell_rectangle.width - cell_padding.right - cell_widths[2],
                cell_widths[2],
            );

            // Size of the content area and size of the available space in the content area
            let mut free_content_area_width =
                cell_rectangle.width - cell_padding.left - cell_padding.right;

            if cell_widths[0] > 0.0 {
                free_content_area_width -= cell_widths[0] + gap;

                // We add the gap here because that will spare us to check if
                // cellWidths[0] is zero later on
                cell_widths[0] += gap;
            }

            if cell_widths[2] > 0.0 {
                free_content_area_width -= cell_widths[2] + gap;
            }

            // If the available space is larger than the current size of the center
            // cell, enlarge that thing
            cell_widths[1] = cell_widths[1].max(free_content_area_width);

            // Place the center cell, possibly enlarging it in the process
            self.apply_horizontal_layout(
                children[1],
                cell_rectangle.x + cell_padding.left + cell_widths[0]
                    - (cell_widths[1] - free_content_area_width) / 2.0,
                cell_widths[1],
            );
        }

        // Layout container cells recursively
        for child_cell in children.into_iter().flatten() {
            if self.is_container(child_cell) {
                self.layout_children_horizontally(child_cell);
            }
        }
    }

    fn strip_layout_children_vertically(&mut self, id: CellId) {
        let cell_rectangle = self.cells[id].rect;
        let cell_padding = self.cells[id].padding;
        let s = self.strip(id);
        let mode = s.container_mode;
        let gap = s.gap;
        let children = s.cells;

        if mode == Strip::Vertical {
            let mut cell_heights = self.strip_min_cell_heights(id, false);

            // Top cell is top-aligned with our content area, bottom cell is bottom-aligned
            self.apply_vertical_layout(
                children[0],
                cell_rectangle.y + cell_padding.top,
                cell_heights[0],
            );
            self.apply_vertical_layout(
                children[2],
                cell_rectangle.y + cell_rectangle.height - cell_padding.bottom - cell_heights[2],
                cell_heights[2],
            );

            // Size of the content area and size of the available space in the content area
            let content_area_height = cell_rectangle.height - cell_padding.top - cell_padding.bottom;
            let mut content_area_free_height = content_area_height;

            if cell_heights[0] > 0.0 {
                // We add the gap here because that will spare us to check if
                // cellHeights[0] is zero later on
                cell_heights[0] += gap;
                content_area_free_height -= cell_heights[0];
            }

            if cell_heights[2] > 0.0 {
                content_area_free_height -= cell_heights[2] + gap;
            }

            // If the available space is larger than the current size of the center
            // cell, enlarge that thing
            cell_heights[1] = cell_heights[1].max(content_area_free_height);

            // Place the center cell, possibly enlarging it in the process
            self.apply_vertical_layout(
                children[1],
                cell_rectangle.y + cell_padding.top + cell_heights[0]
                    - (cell_heights[1] - content_area_free_height) / 2.0,
                cell_heights[1],
            );
        } else {
            // Each child cell begins at our top border (plus padding) and is as
            // large as our content area
            let y_pos = cell_rectangle.y + cell_padding.top;
            let height = cell_rectangle.height - cell_padding.top - cell_padding.bottom;

            for child_cell in children {
                self.apply_vertical_layout(child_cell, y_pos, height);
            }
        }

        // Layout container cells recursively
        for child_cell in children.into_iter().flatten() {
            if self.is_container(child_cell) {
                self.layout_children_vertically(child_cell);
            }
        }
    }

    // --------------------------------------------- GridContainerCell layout

    fn grid_min_column_widths(
        &self,
        id: CellId,
        row: Option<ContainerArea>,
        respect_contribution_flag: bool,
    ) -> [f64; 3] {
        let g = self.grid(id);
        let mut col_widths = [
            self.grid_min_width_of_column(id, ContainerArea::Begin, row, respect_contribution_flag),
            self.grid_min_width_of_column(id, ContainerArea::Center, row, respect_contribution_flag),
            self.grid_min_width_of_column(id, ContainerArea::End, row, respect_contribution_flag),
        ];
        // If we are to be symmetrical, the outer cells need to be the same size
        if g.symmetrical {
            col_widths[0] = col_widths[0].max(col_widths[2]);
            col_widths[2] = col_widths[0];
        }
        col_widths
    }

    fn grid_min_width_of_column(
        &self,
        id: CellId,
        column: ContainerArea,
        row: Option<ContainerArea>,
        respect_contribution_flag: bool,
    ) -> f64 {
        let g = self.grid(id);
        let mut max_min_width = 0.0f64;

        match row {
            None => {
                // Aggregate values for all rows
                for row_index in 0..3 {
                    max_min_width = max_min_width.max(self.min_width_of_cell(
                        g.cells[row_index][column.ordinal()],
                        respect_contribution_flag,
                    ));
                }
            }
            Some(row) => {
                // Only concentrate on the specified row
                max_min_width = self.min_width_of_cell(
                    g.cells[row.ordinal()][column.ordinal()],
                    respect_contribution_flag,
                );
            }
        }

        // If this is the center column, we might have an explicit minimal width for that
        if column == ContainerArea::Center {
            if let Some(min_size) = g.center_cell_minimum_size {
                max_min_width = max_min_width.max(min_size.x);
            }
        }

        max_min_width
    }

    fn grid_min_row_heights(&self, id: CellId, respect_contribution_flag: bool) -> [f64; 3] {
        let g = self.grid(id);
        let mut row_heights = [
            self.grid_min_height_of_row(id, ContainerArea::Begin, respect_contribution_flag),
            self.grid_min_height_of_row(id, ContainerArea::Center, respect_contribution_flag),
            self.grid_min_height_of_row(id, ContainerArea::End, respect_contribution_flag),
        ];
        if g.symmetrical {
            row_heights[0] = row_heights[0].max(row_heights[2]);
            row_heights[2] = row_heights[0];
        }
        row_heights
    }

    fn grid_min_height_of_row(
        &self,
        id: CellId,
        row: ContainerArea,
        respect_contribution_flag: bool,
    ) -> f64 {
        let g = self.grid(id);
        let mut max_min_height = 0.0f64;
        for column in 0..3 {
            max_min_height = max_min_height.max(
                self.min_height_of_cell(g.cells[row.ordinal()][column], respect_contribution_flag),
            );
        }

        // If this is the center row, we might have an explicit minimal height for that
        if row == ContainerArea::Center {
            if let Some(min_size) = g.center_cell_minimum_size {
                max_min_height = max_min_height.max(min_size.y);
            }
        }

        max_min_height
    }

    fn grid_layout_children_horizontally(&mut self, id: CellId) {
        // How we're going to do this depends on whether we're in tabular mode or
        // not. If so, the column widths across all rows are locked
        if self.grid(id).tabular {
            let col_widths = self.grid_min_column_widths(id, None, false);
            for row in ContainerArea::VALUES {
                self.grid_apply_widths_to_row(id, row, col_widths);
            }
        } else {
            for row in ContainerArea::VALUES {
                let col_widths = self.grid_min_column_widths(id, Some(row), false);
                self.grid_apply_widths_to_row(id, row, col_widths);
            }
        }
    }

    fn grid_layout_children_vertically(&mut self, id: CellId) {
        let cell_rectangle = self.cells[id].rect;
        let cell_padding = self.cells[id].padding;
        let gap = self.grid(id).gap;

        let mut row_heights = self.grid_min_row_heights(id, false);

        // Top row is top-aligned with our content area, bottom row is bottom-aligned
        self.grid_apply_height_to_row(
            id,
            ContainerArea::Begin,
            cell_rectangle.y + cell_padding.top,
            &row_heights,
        );
        self.grid_apply_height_to_row(
            id,
            ContainerArea::End,
            cell_rectangle.y + cell_rectangle.height - cell_padding.bottom - row_heights[2],
            &row_heights,
        );

        // Size of the content area and size of the available space in the content area
        let mut free_content_area_height =
            cell_rectangle.height - cell_padding.top - cell_padding.bottom;

        if row_heights[0] > 0.0 {
            row_heights[0] += gap;
            free_content_area_height -= row_heights[0];
        }

        if row_heights[2] > 0.0 {
            row_heights[2] += gap;
            free_content_area_height -= row_heights[2];
        }

        // Compute the center cell rectangle
        {
            let g = self.grid_mut(id);
            g.center_cell_rect.height = free_content_area_height.max(0.0);
            g.center_cell_rect.y = cell_rectangle.y
                + cell_padding.top
                + (g.center_cell_rect.height - free_content_area_height) / 2.0;
        }

        // If the available space is larger than the current size of the center
        // cell, enlarge that thing
        row_heights[1] = row_heights[1].max(free_content_area_height);

        // Place the center cell, possibly enlarging it in the process
        self.grid_apply_height_to_row(
            id,
            ContainerArea::Center,
            cell_rectangle.y + cell_padding.top + row_heights[0]
                - (row_heights[1] - free_content_area_height) / 2.0,
            &row_heights,
        );
    }

    fn grid_apply_widths_to_row(&mut self, id: CellId, row: ContainerArea, mut col_widths: [f64; 3]) {
        let cell_rectangle = self.cells[id].rect;
        let cell_padding = self.cells[id].padding;
        let gap = self.grid(id).gap;

        // Left column is left-aligned with our content area, right column is right-aligned
        self.grid_apply_width_to_column(
            id,
            ContainerArea::Begin,
            cell_rectangle.x + cell_padding.left,
            &col_widths,
        );
        self.grid_apply_width_to_column(
            id,
            ContainerArea::End,
            cell_rectangle.x + cell_rectangle.width - cell_padding.right - col_widths[2],
            &col_widths,
        );

        // Size of the content area and size of the available space in the content area
        let mut free_content_area_width =
            cell_rectangle.width - cell_padding.left - cell_padding.right;

        if col_widths[0] > 0.0 {
            col_widths[0] += gap;
            free_content_area_width -= col_widths[0];
        }

        if col_widths[2] > 0.0 {
            col_widths[2] += gap;
            free_content_area_width -= col_widths[2];
        }

        // Compute how wide the center cell can be
        let center_width = free_content_area_width.max(0.0);

        // If the available space is larger than the current size of the center
        // cell, enlarge that thing
        col_widths[1] = col_widths[1].max(free_content_area_width);

        // Place the center cell, possibly enlarging it in the process
        self.grid_apply_width_to_column(
            id,
            ContainerArea::Center,
            cell_rectangle.x + cell_padding.left + col_widths[0]
                - (col_widths[1] - free_content_area_width) / 2.0,
            &col_widths,
        );

        // If this is the center row, remember the center cell's data for the
        // center cell rectangle
        if row == ContainerArea::Center {
            let g = self.grid_mut(id);
            g.center_cell_rect.width = center_width;
            g.center_cell_rect.x = cell_rectangle.x
                + cell_padding.left
                + (center_width - free_content_area_width) / 2.0;
        }
    }

    fn grid_apply_width_to_column(
        &mut self,
        id: CellId,
        column: ContainerArea,
        x: f64,
        col_widths: &[f64; 3],
    ) {
        for row in 0..3 {
            let cell = self.grid(id).cells[row][column.ordinal()];
            self.apply_horizontal_layout(cell, x, col_widths[column.ordinal()]);
        }
    }

    fn grid_apply_height_to_row(
        &mut self,
        id: CellId,
        row: ContainerArea,
        y: f64,
        row_heights: &[f64; 3],
    ) {
        for column in 0..3 {
            let cell = self.grid(id).cells[row.ordinal()][column];
            self.apply_vertical_layout(cell, y, row_heights[row.ordinal()]);
        }
    }
}

fn sum_with_gaps(values: &[f64; 3], gap: f64) -> f64 {
    let mut sum = 0.0;
    let mut active_components = 0;
    for &val in values {
        if val > 0.0 {
            sum += val;
            active_components += 1;
        }
    }
    if active_components > 1 {
        sum += gap * (active_components - 1) as f64;
    }
    sum
}

/// Assigns positions to the labels of
/// the given label cell based on its cell rectangle.
pub fn apply_label_layout<G: AdapterGraph>(cs: &CellSystem<G::L>, id: CellId, g: &mut G) {
    let data = cs.label(id);
    if data.horizontal_layout_mode {
        apply_horizontal_mode_label_layout(cs, id, g);
    } else {
        apply_vertical_mode_label_layout(cs, id, g);
    }
}

fn apply_horizontal_mode_label_layout<G: AdapterGraph>(cs: &CellSystem<G::L>, id: CellId, g: &mut G) {
    let cell_rect = cs.rect(id);
    let cell_padding = cs.padding(id);
    let data = cs.label(id);

    // Calculate our starting y coordinate
    let mut y_pos = cell_rect.y;

    if data.vertical_alignment == VerticalLabelAlignment::Center {
        y_pos += (cell_rect.height - data.min_content_area_size.y) / 2.0;
    } else if data.vertical_alignment == VerticalLabelAlignment::Bottom {
        y_pos += cell_rect.height - data.min_content_area_size.y;
    }

    // Place them labels, I say!
    for &label in &data.labels {
        let label_size = g.label_size(label);
        let mut label_pos = KVector::default();

        // Y coordinate
        label_pos.y = y_pos;
        y_pos += label_size.y + data.gap;

        // X coordinate
        match data.horizontal_alignment {
            HorizontalLabelAlignment::Left => {
                label_pos.x = cell_rect.x + cell_padding.left;
            }
            HorizontalLabelAlignment::Center => {
                label_pos.x = cell_rect.x + cell_padding.left + (cell_rect.width - label_size.x) / 2.0;
            }
            HorizontalLabelAlignment::Right => {
                label_pos.x = cell_rect.x + cell_rect.width - cell_padding.right - label_size.x;
            }
        }

        // Apply position
        g.set_label_position(label, label_pos);
    }
}

fn apply_vertical_mode_label_layout<G: AdapterGraph>(cs: &CellSystem<G::L>, id: CellId, g: &mut G) {
    let cell_rect = cs.rect(id);
    let cell_padding = cs.padding(id);
    let data = cs.label(id);

    // Calculate our starting x coordinate
    let mut x_pos = cell_rect.x;

    if data.horizontal_alignment == HorizontalLabelAlignment::Center {
        x_pos += (cell_rect.width - data.min_content_area_size.x) / 2.0;
    } else if data.horizontal_alignment == HorizontalLabelAlignment::Right {
        x_pos += cell_rect.width - data.min_content_area_size.x;
    }

    // Place them labels, I say!
    for &label in &data.labels {
        let label_size = g.label_size(label);
        let mut label_pos = KVector::default();

        // X coordinate
        label_pos.x = x_pos;
        x_pos += label_size.x + data.gap;

        // Y coordinate
        match data.vertical_alignment {
            VerticalLabelAlignment::Top => {
                label_pos.y = cell_rect.y + cell_padding.top;
            }
            VerticalLabelAlignment::Center => {
                label_pos.y = cell_rect.y + cell_padding.top + (cell_rect.height - label_size.y) / 2.0;
            }
            VerticalLabelAlignment::Bottom => {
                label_pos.y = cell_rect.y + cell_rect.height - cell_padding.bottom - label_size.y;
            }
        }

        // Apply position
        g.set_label_position(label, label_pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Cs = CellSystem<u32>;

    #[test]
    fn atomic_cell_min_size_includes_padding() {
        let mut cs = Cs::new();
        let a = cs.new_atomic();
        cs.atomic_min_content_area_size_mut(a).set(10.0, 20.0);
        cs.padding_mut(a).set(1.0, 2.0, 3.0, 4.0); // top right bottom left
        assert_eq!(cs.min_width(a), 10.0 + 4.0 + 2.0);
        assert_eq!(cs.min_height(a), 20.0 + 1.0 + 3.0);
    }

    #[test]
    fn label_cell_horizontal_mode_stacks_labels() {
        let mut cs = Cs::new();
        let l = cs.new_label(3.0, true);
        cs.label_add_label(l, 0, KVector::new(10.0, 5.0));
        cs.label_add_label(l, 1, KVector::new(20.0, 7.0));
        // widths take the max, heights add up plus gap
        assert_eq!(cs.min_width(l), 20.0);
        assert_eq!(cs.min_height(l), 5.0 + 3.0 + 7.0);
    }

    #[test]
    fn horizontal_strip_min_width_sums_active_cells_with_gaps() {
        let mut cs = Cs::new();
        let strip = cs.new_strip(Strip::Horizontal, false, 5.0);
        let left = cs.new_atomic();
        let center = cs.new_atomic();
        let right = cs.new_atomic();
        cs.atomic_min_content_area_size_mut(left).set(10.0, 4.0);
        cs.atomic_min_content_area_size_mut(center).set(20.0, 8.0);
        cs.atomic_min_content_area_size_mut(right).set(0.0, 6.0); // zero width: ignored
        cs.strip_set_cell(strip, ContainerArea::Begin, left);
        cs.strip_set_cell(strip, ContainerArea::Center, center);
        cs.strip_set_cell(strip, ContainerArea::End, right);
        // contribution flags are respected for min width; set them
        cs.set_contributes_to_minimum_width(left, true);
        cs.set_contributes_to_minimum_width(center, true);
        cs.set_contributes_to_minimum_width(right, true);
        // two active cells (right has zero content width) plus one gap
        assert_eq!(cs.min_width(strip), 10.0 + 20.0 + 5.0);
        // horizontal strip min height is the max of contributing children
        cs.set_contributes_to_minimum_height(left, true);
        cs.set_contributes_to_minimum_height(center, true);
        assert_eq!(cs.min_height(strip), 8.0);
    }

    #[test]
    fn symmetrical_strip_equalizes_outer_cells() {
        let mut cs = Cs::new();
        let strip = cs.new_strip(Strip::Horizontal, true, 2.0);
        let left = cs.new_atomic();
        let right = cs.new_atomic();
        cs.atomic_min_content_area_size_mut(left).set(5.0, 0.0);
        cs.atomic_min_content_area_size_mut(right).set(9.0, 0.0);
        cs.strip_set_cell(strip, ContainerArea::Begin, left);
        cs.strip_set_cell(strip, ContainerArea::End, right);
        cs.set_contributes_to_minimum_width(left, true);
        cs.set_contributes_to_minimum_width(right, true);
        // both outer cells become 9 wide: 9 + 9 + one gap
        assert_eq!(cs.min_width(strip), 9.0 + 9.0 + 2.0);
    }

    #[test]
    fn horizontal_strip_layout_positions_children() {
        let mut cs = Cs::new();
        let strip = cs.new_strip(Strip::Horizontal, false, 4.0);
        let left = cs.new_atomic();
        let center = cs.new_atomic();
        let right = cs.new_atomic();
        cs.atomic_min_content_area_size_mut(left).set(10.0, 0.0);
        cs.atomic_min_content_area_size_mut(center).set(6.0, 0.0);
        cs.atomic_min_content_area_size_mut(right).set(8.0, 0.0);
        cs.strip_set_cell(strip, ContainerArea::Begin, left);
        cs.strip_set_cell(strip, ContainerArea::Center, center);
        cs.strip_set_cell(strip, ContainerArea::End, right);

        let rect = cs.rect_mut(strip);
        rect.x = 0.0;
        rect.width = 50.0;
        cs.layout_children_horizontally(strip);

        assert_eq!(cs.rect(left).x, 0.0);
        assert_eq!(cs.rect(left).width, 10.0);
        assert_eq!(cs.rect(right).x, 50.0 - 8.0);
        assert_eq!(cs.rect(right).width, 8.0);
        // free space: 50 - (10 + 4) - (8 + 4) = 24, center gets enlarged to 24
        assert_eq!(cs.rect(center).width, 24.0);
        assert_eq!(cs.rect(center).x, 14.0);
    }
}
