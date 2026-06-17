//! Ports of `org.eclipse.elk.alg.common.polyomino` (PolyominoCompactor,
//! ProfileFill, the Successor* traversal functions) and
//! `polyomino.structures` (TwoBitGrid, PlanarGrid, Polyomino, Polyominoes).
//!
//! The class hierarchy `Polyomino extends PlanarGrid extends TwoBitGrid`
//! is flattened into [`Grid`] (TwoBitGrid + the PlanarGrid center logic) and
//! [`Polyomino`] (a `Grid` plus position and extensions).
//!
//! Out-of-bounds semantics: reads within the padding bits of the last word of
//! a row silently return "empty", while reads outside the allocated arrays
//! panic. All coordinates are `i32` to keep the signed arithmetic.

use crate::elk_enum;

elk_enum! {
    pub enum Direction {
        NORTH,
        EAST,
        SOUTH,
        WEST,
    }
}

impl Direction {
    pub fn is_horizontal(self) -> bool {
        matches!(self, Direction::EAST | Direction::WEST)
    }
}

elk_enum! {
    pub enum HighLevelSortingCriterion {
        NUM_OF_EXTERNAL_SIDES_THAN_NUM_OF_EXTENSIONS_LAST,
        CORNER_CASES_THAN_SINGLE_SIDE_LAST,
    }
}

elk_enum! {
    pub enum LowLevelSortingCriterion {
        BY_SIZE,
        BY_SIZE_AND_SHAPE,
    }
}

elk_enum! {
    pub enum TraversalStrategy {
        SPIRAL,
        LINE_BY_LINE,
        MANHATTAN,
        JITTER,
        QUADRANTS_LINE_BY_LINE,
        QUADRANTS_MANHATTAN,
        QUADRANTS_JITTER,
        COMBINE_LINE_BY_LINE_MANHATTAN,
        COMBINE_JITTER_MANHATTAN,
    }
}

const EMPTY: u64 = 0x00;
const BLOCKED: u64 = 0x01;
const WEAKLY_BLOCKED: u64 = 0x02;

#[derive(Clone)]
pub struct Grid {
    /// `long[height][ceil(width/32)]`, two bits per cell.
    rows: Vec<Vec<u64>>,
    x_size: i32,
    y_size: i32,
    x_center: i32,
    y_center: i32,
}

impl Grid {
    pub fn new(width: i32, height: i32) -> Self {
        let words = ((width as f64) / 32.0).ceil() as usize;
        Grid {
            rows: vec![vec![0u64; words]; height.max(0) as usize],
            x_size: width,
            y_size: height,
            x_center: (width - 1) >> 1,
            y_center: (height - 1) >> 1,
        }
    }

    pub fn reinitialize(&mut self, width: i32, height: i32) {
        *self = Grid::new(width, height);
    }

    pub fn width(&self) -> i32 {
        self.x_size
    }

    pub fn height(&self) -> i32 {
        self.y_size
    }

    pub fn center_x(&self) -> i32 {
        self.x_center
    }

    pub fn center_y(&self) -> i32 {
        self.y_center
    }

    /// `TwoBitGrid.retrieve`: panics when the underlying array access is out
    /// of bounds; reads within the padding bits of the last word return EMPTY
    /// without error.
    fn retrieve(&self, x: i32, y: i32) -> u64 {
        let x_word = x >> 5; // RIGHT_SHIFT
        if y < 0 || y >= self.rows.len() as i32 || x_word < 0 {
            panic!("Grid is only of size {}*{}. Requested point ({}, {}) is out of bounds.", self.x_size, self.y_size, x, y);
        }
        let row = &self.rows[y as usize];
        if x_word as usize >= row.len() {
            panic!("Grid is only of size {}*{}. Requested point ({}, {}) is out of bounds.", self.x_size, self.y_size, x, y);
        }
        let x_rest = (x & 0x1F) as u64;
        (row[x_word as usize] >> (x_rest << 1)) & 0x03
    }

    fn set(&mut self, x: i32, y: i32, msb: bool, lsb: bool) {
        if x >= self.x_size {
            panic!("Grid is only of size {}*{}. Requested point ({}, {}) is out of bounds.", self.x_size, self.y_size, x, y);
        }
        let x_word = x >> 5;
        if y < 0 || y >= self.rows.len() as i32 || x_word < 0 || x_word as usize >= self.rows[y.max(0) as usize].len() {
            panic!("Grid is only of size {}*{}. Requested point ({}, {}) is out of bounds.", self.x_size, self.y_size, x, y);
        }
        let x_rest = (x & 0x1F) as u64;
        let mut mask = 1u64 << (x_rest << 1);
        let word = &mut self.rows[y as usize][x_word as usize];
        if lsb {
            *word |= mask;
        } else {
            *word &= !mask;
        }
        mask <<= 1;
        if msb {
            *word |= mask;
        } else {
            *word &= !mask;
        }
    }

    pub fn is_empty(&self, x: i32, y: i32) -> bool {
        self.retrieve(x, y) == EMPTY
    }

    pub fn is_blocked(&self, x: i32, y: i32) -> bool {
        self.retrieve(x, y) == BLOCKED
    }

    pub fn is_weakly_blocked(&self, x: i32, y: i32) -> bool {
        self.retrieve(x, y) == WEAKLY_BLOCKED
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.x_size && y < self.y_size
    }

    pub fn set_empty(&mut self, x: i32, y: i32) {
        self.set(x, y, false, false);
    }

    pub fn set_blocked(&mut self, x: i32, y: i32) {
        self.set(x, y, false, true);
    }

    pub fn set_weakly_blocked(&mut self, x: i32, y: i32) {
        self.set(x, y, true, false);
    }

    // --------------------------------------------------- PlanarGrid methods

    pub fn is_empty_center_based(&self, x: i32, y: i32) -> bool {
        self.is_empty(x + self.x_center, y + self.y_center)
    }

    pub fn is_blocked_center_based(&self, x: i32, y: i32) -> bool {
        self.is_blocked(x + self.x_center, y + self.y_center)
    }

    pub fn is_weakly_blocked_center_based(&self, x: i32, y: i32) -> bool {
        self.is_weakly_blocked(x + self.x_center, y + self.y_center)
    }

    pub fn in_bounds_center_based(&self, x: i32, y: i32) -> bool {
        self.in_bounds(x + self.x_center, y + self.y_center)
    }

    pub fn set_blocked_center_based(&mut self, x: i32, y: i32) {
        self.set_blocked(x + self.x_center, y + self.y_center);
    }

    pub fn set_weakly_blocked_center_based(&mut self, x: i32, y: i32) {
        self.set_weakly_blocked(x + self.x_center, y + self.y_center);
    }

    /// `PlanarGrid.intersectsWithCenterBased(PlanarGrid, ...)`.
    pub fn intersects_with_center_based_grid(&self, other: &Grid, x_offset: i32, y_offset: i32) -> bool {
        for x in 0..other.width() {
            let x_translated = x - other.center_x() + x_offset;
            for y in 0..other.height() {
                let y_translated = y - other.center_y() + y_offset;
                if self.in_bounds_center_based(x_translated, y_translated)
                    && ((!other.is_empty(x, y) && self.is_blocked_center_based(x_translated, y_translated))
                        || (other.is_blocked(x, y) && !self.is_empty_center_based(x_translated, y_translated)))
                {
                    return true;
                }
            }
        }
        false
    }

    /// `PlanarGrid.intersectsWithCenterBased(Polyomino, ...)`.
    pub fn intersects_with_center_based(&self, other: &Polyomino, x_offset: i32, y_offset: i32) -> bool {
        if self.intersects_with_center_based_grid(&other.grid, x_offset, y_offset) {
            return true;
        }
        for &(dir, second, third) in &other.extensions {
            // Transform center based coordinates for the extension checks.
            let left_x = self.center_x() - other.grid.center_x() + x_offset;
            let right_x = left_x + other.grid.width();
            let top_y = self.center_y() - other.grid.center_y() + y_offset;
            let bottom_y = top_y + other.grid.height();

            let intersects = match dir {
                Direction::NORTH => {
                    self.weakly_intersects_area(left_x + second, 0, left_x + third, top_y - 1)
                }
                Direction::EAST => self.weakly_intersects_area(
                    right_x,
                    top_y + second,
                    self.width() - 1,
                    top_y + third,
                ),
                Direction::SOUTH => self.weakly_intersects_area(
                    left_x + second,
                    bottom_y,
                    left_x + third,
                    self.height() - 1,
                ),
                Direction::WEST => {
                    self.weakly_intersects_area(0, top_y + second, left_x - 1, top_y + third)
                }
            };
            if intersects {
                return true;
            }
        }
        false
    }

    /// `PlanarGrid.addFilledCellsFrom(PlanarGrid, ...)`.
    pub fn add_filled_cells_from_grid(&mut self, other: &Grid, x_offset: i32, y_offset: i32) {
        for x in 0..other.width() {
            let x_translated = x - other.center_x() + x_offset;
            for y in 0..other.height() {
                let y_translated = y - other.center_y() + y_offset;
                if other.is_blocked(x, y) {
                    if !self.is_weakly_blocked_center_based(x_translated, y_translated) {
                        self.set_blocked_center_based(x_translated, y_translated);
                    }
                } else if other.is_weakly_blocked(x, y)
                    && !self.is_blocked_center_based(x_translated, y_translated)
                {
                    self.set_weakly_blocked_center_based(x_translated, y_translated);
                }
            }
        }
    }

    /// `PlanarGrid.addFilledCellsFrom(Polyomino, ...)`: also stores the
    /// polyomino's resulting corner position.
    pub fn add_filled_cells_from(&mut self, other: &mut Polyomino, x_offset: i32, y_offset: i32) {
        self.add_filled_cells_from_grid(&other.grid, x_offset, y_offset);
        other.x = self.x_center - other.grid.center_x() + x_offset;
        other.y = self.y_center - other.grid.center_y() + y_offset;

        for &(dir, second, third) in &other.extensions.clone() {
            match dir {
                Direction::NORTH => self.weakly_block_area(
                    other.x + second,
                    0,
                    other.x + third,
                    other.y - 1,
                ),
                Direction::EAST => self.weakly_block_area(
                    other.x + other.grid.width(),
                    other.y + second,
                    self.width() - 1,
                    other.y + third,
                ),
                Direction::SOUTH => self.weakly_block_area(
                    other.x + second,
                    other.y + other.grid.height(),
                    other.x + third,
                    self.height() - 1,
                ),
                Direction::WEST => self.weakly_block_area(
                    0,
                    other.y + second,
                    other.x - 1,
                    other.y + third,
                ),
            }
        }
    }

    /// `PlanarGrid.getFilledBounds`: (x, y, width, height) of the bounding
    /// box of all blocked cells.
    pub fn get_filled_bounds(&self) -> (i32, i32, i32, i32) {
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_y = i32::MAX;
        let mut max_y = i32::MIN;
        for xi in 0..self.width() {
            for yi in 0..self.height() {
                if self.is_blocked(xi, yi) {
                    min_x = min_x.min(xi);
                    max_x = max_x.max(xi);
                    min_y = min_y.min(yi);
                    max_y = max_y.max(yi);
                }
            }
        }
        (min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
    }

    /// `PlanarGrid.weaklyBlockArea` (doesn't overwrite blocked cells).
    pub fn weakly_block_area(&mut self, x_upper_left: i32, y_upper_left: i32, x_bottom_right: i32, y_bottom_right: i32) {
        let mut yi = y_upper_left;
        while yi <= y_bottom_right {
            let mut xi = x_upper_left;
            while xi <= x_bottom_right {
                if !self.is_blocked(xi, yi) {
                    self.set_weakly_blocked(xi, yi);
                }
                xi += 1;
            }
            yi += 1;
        }
    }

    /// `PlanarGrid.weaklyIntersectsArea`: whether the area contains at least
    /// one blocked cell.
    pub fn weakly_intersects_area(&self, x_upper_left: i32, y_upper_left: i32, x_bottom_right: i32, y_bottom_right: i32) -> bool {
        let mut yi = y_upper_left;
        while yi <= y_bottom_right {
            let mut xi = x_upper_left;
            while xi <= x_bottom_right {
                if self.is_blocked(xi, yi) {
                    return true;
                }
                xi += 1;
            }
            yi += 1;
        }
        false
    }

    pub fn java_to_string(&self) -> String {
        let mut output = String::from(" ");
        let inc_mod_ten = |num: i32| if num > 8 { 0 } else { num + 1 };
        let mut count = 0;
        for _ in 0..self.x_size {
            output.push_str(&count.to_string());
            count = inc_mod_ten(count);
        }
        output.push('\n');
        count = 0;
        for y in 0..self.y_size {
            output.push_str(&count.to_string());
            count = inc_mod_ten(count);
            for x in 0..self.x_size {
                let item = self.retrieve(x, y);
                if item == EMPTY {
                    output.push('_');
                } else if item == BLOCKED {
                    output.push('X');
                } else {
                    output.push('0');
                }
            }
            output.push('\n');
        }
        output.pop(); // substring(0, length - 1)
        output
    }
}

/// A grid plus its position on the
/// packing grid and its extensions `(direction, offset, width)`.
#[derive(Clone)]
pub struct Polyomino {
    pub grid: Grid,
    pub x: i32,
    pub y: i32,
    pub extensions: Vec<(Direction, i32, i32)>,
}

impl Polyomino {
    pub fn new(width: i32, height: i32) -> Self {
        Polyomino { grid: Grid::new(width, height), x: 0, y: 0, extensions: Vec::new() }
    }

    pub fn add_extension(&mut self, dir: Direction, offset: i32, width: i32) {
        self.extensions.push((dir, offset, width));
    }

    /// Number of distinct extension directions.
    fn num_extension_directions(&self) -> usize {
        let mut seen = [false; 4];
        for &(d, _, _) in &self.extensions {
            seen[d.ordinal()] = true;
        }
        seen.iter().filter(|&&b| b).count()
    }

    fn horizontal_direction_count(&self) -> usize {
        let mut seen = [false; 4];
        for &(d, _, _) in &self.extensions {
            seen[d.ordinal()] = true;
        }
        Direction::VALUES
            .iter()
            .filter(|d| seen[d.ordinal()] && d.is_horizontal())
            .count()
    }

    fn has_extension_direction(&self, dir: Direction) -> bool {
        self.extensions.iter().any(|&(d, _, _)| d == dir)
    }
}

use crate::graph::properties::ElkEnum;

/// Access to the generic [`Polyomino`] part of a more specific polyomino
/// type.
pub trait AsPolyomino {
    fn poly(&self) -> &Polyomino;
    fn poly_mut(&mut self) -> &mut Polyomino;
}

impl AsPolyomino for Polyomino {
    fn poly(&self) -> &Polyomino {
        self
    }
    fn poly_mut(&mut self) -> &mut Polyomino {
        self
    }
}

// -------------------------------------------------------------- ProfileFill

pub fn fill_polyomino(poly: &mut Polyomino) {
    let width = poly.grid.width();
    let height = poly.grid.height();
    let mut north_profile = vec![0i32; width.max(0) as usize];
    let mut south_profile = vec![0i32; width.max(0) as usize];
    let mut east_profile = vec![0i32; height.max(0) as usize];
    let mut west_profile = vec![0i32; height.max(0) as usize];

    for xi in 0..width {
        let mut y = 0;
        while y < height && !poly.grid.is_blocked(xi, y) {
            y += 1;
        }
        north_profile[xi as usize] = y;
    }
    for xi in 0..width {
        let mut y = height - 1;
        while y >= 0 && !poly.grid.is_blocked(xi, y) {
            y -= 1;
        }
        south_profile[xi as usize] = y;
    }
    for yi in 0..height {
        let mut x = 0;
        while x < width && !poly.grid.is_blocked(x, yi) {
            x += 1;
        }
        east_profile[yi as usize] = x;
    }
    for yi in 0..height {
        let mut x = width - 1;
        while x >= 0 && !poly.grid.is_blocked(x, yi) {
            x -= 1;
        }
        west_profile[yi as usize] = x;
    }

    for xi in 0..width {
        for yi in 0..height {
            if xi < west_profile[yi as usize]
                && xi > east_profile[yi as usize]
                && yi < south_profile[xi as usize]
                && yi > north_profile[xi as usize]
            {
                poly.grid.set_blocked(xi, yi);
            }
        }
    }
}

// -------------------------------------------------------------- Polyominoes

/// The `Polyominoes` constructor: optionally fills the polyominoes
/// and creates the packing grid.
pub fn create_packing_grid<P: AsPolyomino>(polys: &mut [P], aspect_ratio: f64, fill: bool) -> Grid {
    let mut grid_width: i32 = 0;
    let mut grid_height: i32 = 0;

    for p in polys.iter_mut() {
        if fill {
            fill_polyomino(p.poly_mut());
        }
        grid_width += p.poly().grid.width();
        grid_height += p.poly().grid.height();
    }

    // Add width and height of the future center polyomino once again.
    if !polys.is_empty() {
        grid_width += polys[0].poly().grid.width();
        grid_height += polys[0].poly().grid.height();
    }

    grid_width *= 2;
    grid_height *= 2;

    if aspect_ratio > 1.0 {
        grid_width = ((grid_width as f64) * aspect_ratio).ceil() as i32;
    } else {
        grid_height = ((grid_height as f64) / aspect_ratio).ceil() as i32;
    }

    Grid::new(grid_width, grid_height)
}

// ------------------------------------------------------ Successor functions

fn successor_line_by_line(x: i32, y: i32) -> (i32, i32) {
    if x >= 0 {
        if x == y {
            return (-x - 1, -x - 1);
        }
        if x == -y {
            return (-x, y + 1);
        }
    }
    if x.abs() > y.abs() {
        if x < 0 {
            return (-x, y);
        }
        return (-x, y + 1);
    }
    (x + 1, y)
}

fn successor_manhattan(x: i32, y: i32) -> (i32, i32) {
    let mut new_x = x;
    let mut new_y = y;
    if x == 0 && y == 0 {
        new_y -= 1;
    } else if x == -1 && y <= 0 {
        new_x = 0;
        new_y -= 2;
    } else if x <= 0 && y > 0 {
        new_x -= 1;
        new_y -= 1;
    } else if x >= 0 && y < 0 {
        new_x += 1;
        new_y += 1;
    } else if x > 0 && y >= 0 {
        new_x -= 1;
        new_y += 1;
    } else {
        new_x += 1;
        new_y -= 1;
    }
    (new_x, new_y)
}

fn successor_jitter(x: i32, y: i32) -> (i32, i32) {
    let cost = x.abs().max(y.abs());
    if x <= 0 && x == y {
        (0, y - 1)
    } else if x == -cost && y != cost {
        let mut new_x = y;
        let new_y = x;
        if y >= 0 {
            new_x += 1;
        }
        (new_x, new_y)
    } else {
        (-y, x)
    }
}

fn successor_spiral(x: i32, y: i32) -> (i32, i32) {
    let cost = x.abs().max(y.abs());
    if x < cost && y == -cost {
        return (x + 1, y);
    }
    if x == cost && y < cost {
        return (x, y + 1);
    }
    if x >= -cost && y == cost {
        return (x - 1, y);
    }
    (x, y - 1)
}

/// The successor functions, including the quadrant/combination wrappers.
/// `poly_id` identifies the current polyomino so that
/// `SuccessorQuadrantsGeneric` can cache its quadrant restrictions per
/// polyomino.
pub struct Successor {
    strategy: TraversalStrategy,
    // SuccessorQuadrantsGeneric state
    last_poly: Option<usize>,
    pos_x: bool,
    pos_y: bool,
    neg_x: bool,
    neg_y: bool,
}

impl Successor {
    pub fn new(strategy: TraversalStrategy) -> Self {
        Successor {
            strategy,
            last_poly: None,
            pos_x: true,
            pos_y: true,
            neg_x: true,
            neg_y: true,
        }
    }

    pub fn apply(&mut self, coords: (i32, i32), poly: &Polyomino, poly_id: usize) -> (i32, i32) {
        match self.strategy {
            TraversalStrategy::SPIRAL => successor_spiral(coords.0, coords.1),
            TraversalStrategy::LINE_BY_LINE => successor_line_by_line(coords.0, coords.1),
            TraversalStrategy::MANHATTAN => successor_manhattan(coords.0, coords.1),
            TraversalStrategy::JITTER => successor_jitter(coords.0, coords.1),
            TraversalStrategy::QUADRANTS_LINE_BY_LINE => {
                self.quadrants(coords, poly, poly_id, successor_line_by_line)
            }
            TraversalStrategy::QUADRANTS_MANHATTAN => {
                self.quadrants(coords, poly, poly_id, successor_manhattan)
            }
            TraversalStrategy::QUADRANTS_JITTER => {
                self.quadrants(coords, poly, poly_id, successor_jitter)
            }
            TraversalStrategy::COMBINE_LINE_BY_LINE_MANHATTAN => {
                // SuccessorCombination(QUADRANTS_LINE_BY_LINE, QUADRANTS_MANHATTAN)
                if !poly.extensions.is_empty() {
                    self.quadrants(coords, poly, poly_id, successor_manhattan)
                } else {
                    self.quadrants(coords, poly, poly_id, successor_line_by_line)
                }
            }
            TraversalStrategy::COMBINE_JITTER_MANHATTAN => {
                if !poly.extensions.is_empty() {
                    self.quadrants(coords, poly, poly_id, successor_manhattan)
                } else {
                    self.quadrants(coords, poly, poly_id, successor_jitter)
                }
            }
        }
    }

    fn quadrants(
        &mut self,
        coords: (i32, i32),
        poly: &Polyomino,
        poly_id: usize,
        f: fn(i32, i32) -> (i32, i32),
    ) -> (i32, i32) {
        if self.last_poly != Some(poly_id) {
            self.last_poly = Some(poly_id);
            self.pos_x = true;
            self.pos_y = true;
            self.neg_x = true;
            self.neg_y = true;

            let contains_pos = poly.has_extension_direction(Direction::NORTH);
            let contains_neg = poly.has_extension_direction(Direction::SOUTH);
            if contains_pos && !contains_neg {
                self.pos_y = false;
            }
            if !contains_pos && contains_neg {
                self.neg_y = false;
            }

            let contains_pos = poly.has_extension_direction(Direction::EAST);
            let contains_neg = poly.has_extension_direction(Direction::WEST);
            if contains_pos && !contains_neg {
                self.neg_x = false;
            }
            if !contains_pos && contains_neg {
                self.pos_x = false;
            }
        }

        let mut current = coords;
        loop {
            let next = f(current.0, current.1);
            let (new_x, new_y) = next;

            let mut invalid = false;
            if new_x < 0 {
                if !self.neg_x {
                    invalid = true;
                }
            } else if !self.pos_x {
                invalid = true;
            }
            if new_y < 0 {
                if !self.neg_y {
                    invalid = true;
                }
            } else if !self.pos_y {
                invalid = true;
            }

            if invalid {
                current = next;
            } else {
                return next;
            }
        }
    }
}

// ------------------------------------------------------ PolyominoCompactor

/// The subset of `PolyominoOptions` consulted by `packPolyominoes`. Note
/// that DisCo never copies the user's options onto the `Polyominoes` holder,
/// so the defaults always apply; they are replicated by
/// `PackingOptions::default()`.
pub struct PackingOptions {
    pub low_level_sort: LowLevelSortingCriterion,
    pub high_level_sort: HighLevelSortingCriterion,
    pub traversal_strategy: TraversalStrategy,
}

impl Default for PackingOptions {
    fn default() -> Self {
        PackingOptions {
            low_level_sort: LowLevelSortingCriterion::BY_SIZE_AND_SHAPE,
            high_level_sort:
                HighLevelSortingCriterion::NUM_OF_EXTERNAL_SIDES_THAN_NUM_OF_EXTENSIONS_LAST,
            traversal_strategy: TraversalStrategy::QUADRANTS_LINE_BY_LINE,
        }
    }
}

pub fn pack_polyominoes<P: AsPolyomino>(polys: &mut Vec<P>, grid: &mut Grid, options: &PackingOptions) {
    // 1. Sort polyominoes (successive stable sorts).
    match options.low_level_sort {
        LowLevelSortingCriterion::BY_SIZE => {
            // MinPerimeterComparator().reversed()
            polys.sort_by(|a, b| {
                let half_peri1 = a.poly().grid.width() + a.poly().grid.height();
                let half_peri2 = b.poly().grid.width() + b.poly().grid.height();
                // reversed: compare(o2, o1)
                half_peri2.cmp(&half_peri1)
            });
        }
        LowLevelSortingCriterion::BY_SIZE_AND_SHAPE => {
            // MinPerimeterComparatorWithShape().reversed()
            let val = |p: &Polyomino| {
                let mut width = p.grid.width();
                let mut height = p.grid.height();
                if width < height {
                    width *= width;
                } else {
                    height *= height;
                }
                width + height
            };
            polys.sort_by(|a, b| val(b.poly()).cmp(&val(a.poly())));
        }
    }

    match options.high_level_sort {
        HighLevelSortingCriterion::CORNER_CASES_THAN_SINGLE_SIDE_LAST => {
            // MinNumOfExtensionsComparator
            polys.sort_by(|a, b| a.poly().extensions.len().cmp(&b.poly().extensions.len()));
            // SingleExtensionSideGreaterThanRestComparator
            let single = |p: &Polyomino| -> i32 {
                if p.num_extension_directions() == 1 {
                    1
                } else {
                    0
                }
            };
            polys.sort_by(|a, b| single(a.poly()).cmp(&single(b.poly())));
            // CornerCasesGreaterThanRestComparator
            let corner = |p: &Polyomino| -> i32 {
                let mut num = if p.num_extension_directions() == 2 { 1 } else { 0 };
                if num == 1 && p.horizontal_direction_count() % 2 == 0 {
                    num = 0;
                }
                num
            };
            polys.sort_by(|a, b| corner(a.poly()).cmp(&corner(b.poly())));
        }
        HighLevelSortingCriterion::NUM_OF_EXTERNAL_SIDES_THAN_NUM_OF_EXTENSIONS_LAST => {
            // MinNumOfExtensionsComparator
            polys.sort_by(|a, b| a.poly().extensions.len().cmp(&b.poly().extensions.len()));
            // MinNumOfExtensionDirectionsComparator
            polys.sort_by(|a, b| {
                a.poly()
                    .num_extension_directions()
                    .cmp(&b.poly().num_extension_directions())
            });
        }
    }

    // 2.-5. Place each polyomino.
    let mut successor = Successor::new(options.traversal_strategy);
    for (poly_id, p) in polys.iter_mut().enumerate() {
        let mut off_x = 0;
        let mut off_y = 0;
        let mut next = (off_x, off_y);

        while grid.intersects_with_center_based(p.poly(), off_x, off_y) {
            next = successor.apply(next, p.poly(), poly_id);
            off_x = next.0;
            off_y = next.1;
        }
        grid.add_filled_cells_from(p.poly_mut(), off_x, off_y);
    }
}
