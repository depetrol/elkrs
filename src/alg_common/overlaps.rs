//! Removal of overlaps between
//! rectangles that have a fixed position along one dimension.

use crate::graph::math::ElkRectangle;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OverlapRemovalDirection {
    /// Remove horizontal overlaps by moving rectangles upwards.
    Up,
    /// Remove horizontal overlaps by moving rectangles downwards.
    Down,
    /// Remove vertical overlaps by moving rectangles leftwards.
    Left,
    /// Remove vertical overlaps by moving rectangles rightwards.
    Right,
}

struct RectangleNode {
    /// The original rectangle represented by this node (a copy of the input).
    original: ElkRectangle,
    /// The rectangle after the coordinate transformation. For UP and
    /// DOWN this aliases the original rectangle; here we keep both and
    /// replicate the aliasing in `export_rectangle`.
    rect: ElkRectangle,
    /// Indices of nodes that this node overlaps with.
    overlapping: Vec<usize>,
}

/// The default gap is 5 on both axes.
///
/// Usage: create for a direction, configure via `with_gap` /
/// `with_start_coordinate`, add rectangles (each `add_rectangle` returns a
/// handle), call `remove_overlaps`, then fetch the moved rectangles back via
/// `rectangle(handle)`.
pub struct RectangleStripOverlapRemover {
    direction: OverlapRemovalDirection,
    gap_vertical: f64,
    gap_horizontal: f64,
    start_coordinate: f64,
    nodes: Vec<RectangleNode>,
}

const DEFAULT_GAP: f64 = 5.0;

impl RectangleStripOverlapRemover {
    pub fn create_for_direction(direction: OverlapRemovalDirection) -> Self {
        RectangleStripOverlapRemover {
            direction,
            gap_vertical: DEFAULT_GAP,
            gap_horizontal: DEFAULT_GAP,
            start_coordinate: 0.0,
            nodes: Vec::new(),
        }
    }

    pub fn with_gap(mut self, horizontal_gap: f64, vertical_gap: f64) -> Self {
        self.gap_horizontal = horizontal_gap;
        self.gap_vertical = vertical_gap;
        self
    }

    pub fn with_start_coordinate(mut self, coordinate: f64) -> Self {
        self.start_coordinate = coordinate;
        self
    }

    pub fn add_rectangle(&mut self, rectangle: ElkRectangle) -> usize {
        let imported = self.import_rectangle(&rectangle);
        self.nodes.push(RectangleNode { original: rectangle, rect: imported, overlapping: Vec::new() });
        self.nodes.len() - 1
    }

    /// The rectangle for the given handle, with overlap removal applied (only
    /// valid after `remove_overlaps`).
    pub fn rectangle(&self, handle: usize) -> ElkRectangle {
        self.nodes[handle].original
    }

    fn import_rectangle(&self, rectangle: &ElkRectangle) -> ElkRectangle {
        match self.direction {
            OverlapRemovalDirection::Up | OverlapRemovalDirection::Down => *rectangle,
            OverlapRemovalDirection::Left | OverlapRemovalDirection::Right => {
                ElkRectangle::new(rectangle.y, 0.0, rectangle.height, rectangle.width)
            }
        }
    }

    /// The transformed rectangle aliases
    /// the original for UP/DOWN, so the strategy's `rect.y` is already stored
    /// in the original's y before the export applies the start coordinate; we
    /// replicate the resulting arithmetic here.
    fn export_rectangle(&mut self, index: usize) {
        let rect = self.nodes[index].rect;
        let original = &mut self.nodes[index].original;
        match self.direction {
            OverlapRemovalDirection::Up => {
                original.y = self.start_coordinate - rect.height - rect.y;
            }
            OverlapRemovalDirection::Down => {
                original.y = rect.y + self.start_coordinate;
            }
            OverlapRemovalDirection::Left => {
                original.x = self.start_coordinate - rect.height - rect.y;
            }
            OverlapRemovalDirection::Right => {
                original.x = self.start_coordinate + rect.y;
            }
        }
    }

    /// Returns the size of the resulting strip.
    pub fn remove_overlaps(&mut self) -> f64 {
        // Sort the list of rectangles by left border (stable).
        // We sort indices to keep handles valid.
        let mut order: Vec<usize> = (0..self.nodes.len()).collect();
        order.sort_by(|&a, &b| self.nodes[a].rect.x.total_cmp(&self.nodes[b].rect.x));

        // Compute and remove overlaps
        self.compute_overlaps(&order);
        let strip_size = self.greedy_remove_overlaps(&order);

        // Apply the results
        for i in 0..self.nodes.len() {
            self.export_rectangle(i);
        }

        strip_size
    }

    /// Uses a `TreeSet` ordered by right
    /// border coordinate; note that a TreeSet drops elements that compare
    /// equal, so a rectangle whose right border coincides exactly with an
    /// already-present rectangle's right border is never added to the set of
    /// scanline-intersecting nodes. We faithfully replicate that quirk.
    fn compute_overlaps(&mut self, order: &[usize]) {
        // Sorted by right border (ascending, Double.compare == total_cmp).
        let mut intersecting: Vec<usize> = Vec::new();
        let right = |nodes: &Vec<RectangleNode>, i: usize| nodes[i].rect.x + nodes[i].rect.width;

        for &curr in order {
            // Move the scanline to the new node's left border
            let scanline_pos = self.nodes[curr].rect.x;

            // Remove intersecting nodes which do not intersect the scanline anymore
            while !intersecting.is_empty() {
                let first = intersecting[0];
                if right(&self.nodes, first) < scanline_pos {
                    intersecting.remove(0);
                } else {
                    break;
                }
            }

            // Add overlaps between the currently intersecting nodes and the new node
            for &other in &intersecting {
                self.nodes[other].overlapping.push(curr);
                self.nodes[curr].overlapping.push(other);
            }

            // Insert the new node, keeping the set ordered by right border and
            // rejecting elements that compare equal (TreeSet semantics).
            let curr_right = right(&self.nodes, curr);
            let mut insert_at = intersecting.len();
            let mut duplicate = false;
            for (i, &other) in intersecting.iter().enumerate() {
                match right(&self.nodes, other).total_cmp(&curr_right) {
                    std::cmp::Ordering::Equal => {
                        duplicate = true;
                        break;
                    }
                    std::cmp::Ordering::Greater => {
                        insert_at = i;
                        break;
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            if !duplicate {
                intersecting.insert(insert_at, curr);
            }
        }
    }

    /// Greedily
    /// chooses the smallest y position that won't cause overlaps.
    fn greedy_remove_overlaps(&mut self, order: &[usize]) -> f64 {
        let vertical_gap = self.gap_vertical;
        let mut already_placed = vec![false; self.nodes.len()];
        let mut strip_size = 0.0f64;

        for &curr in order {
            // We start with an initial y coordinate of zero
            let mut y_pos = 0.0f64;

            // Sort the node's list of overlapping nodes by y coordinate (stable)
            let mut overlapping = std::mem::take(&mut self.nodes[curr].overlapping);
            overlapping.sort_by(|&a, &b| self.nodes[a].rect.y.total_cmp(&self.nodes[b].rect.y));

            // Check every conflicting rectangle node that we have already placed
            for &overlap in &overlapping {
                if already_placed[overlap] {
                    let curr_rect = self.nodes[curr].rect;
                    let overlap_rect = self.nodes[overlap].rect;

                    if y_pos < overlap_rect.y + overlap_rect.height + vertical_gap
                        && y_pos + curr_rect.height + vertical_gap > overlap_rect.y
                    {
                        y_pos = overlap_rect.y + overlap_rect.height + vertical_gap;
                    }
                }
            }
            self.nodes[curr].overlapping = overlapping;

            // Apply the y coordinate and remember that this node is now placed
            self.nodes[curr].rect.y = y_pos;
            already_placed[curr] = true;

            // Update the strip size
            strip_size = strip_size.max(self.nodes[curr].rect.y + self.nodes[curr].rect.height);
        }

        strip_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_overlap_keeps_row() {
        let mut remover =
            RectangleStripOverlapRemover::create_for_direction(OverlapRemovalDirection::Down)
                .with_gap(2.0, 2.0)
                .with_start_coordinate(10.0);
        let a = remover.add_rectangle(ElkRectangle::new(0.0, 0.0, 10.0, 5.0));
        let b = remover.add_rectangle(ElkRectangle::new(20.0, 0.0, 10.0, 7.0));
        let strip = remover.remove_overlaps();
        assert_eq!(strip, 7.0);
        assert_eq!(remover.rectangle(a).y, 10.0);
        assert_eq!(remover.rectangle(b).y, 10.0);
    }

    #[test]
    fn overlapping_rectangles_are_stacked() {
        let mut remover =
            RectangleStripOverlapRemover::create_for_direction(OverlapRemovalDirection::Down)
                .with_gap(1.0, 1.0)
                .with_start_coordinate(0.0);
        let a = remover.add_rectangle(ElkRectangle::new(0.0, 0.0, 10.0, 5.0));
        let b = remover.add_rectangle(ElkRectangle::new(5.0, 0.0, 10.0, 5.0));
        let strip = remover.remove_overlaps();
        assert_eq!(strip, 11.0);
        assert_eq!(remover.rectangle(a).y, 0.0);
        assert_eq!(remover.rectangle(b).y, 6.0);
    }
}
