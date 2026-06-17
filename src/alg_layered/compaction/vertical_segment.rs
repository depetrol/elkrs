//!
//! Represents a vertical segment on a single `LEdge` that is merged with
//! intersecting `VerticalSegment`s. The
//! `KVector` affected-bend lists reference the actual bend points of edges, so
//! here we keep the indices needed to mutate them back during `applyLayout`.

use std::cmp::Ordering;

use crate::graph::math::{ElkRectangle, KVector};

use crate::alg_layered::graph::{LEdgeId, LPortId};
use crate::alg_common::compaction::Quadruplet;

use super::compare_fuzzy;

/// A reference to a single bend point inside an edge's bend chain.
#[derive(Clone, Copy, Debug)]
pub struct BendRef {
    pub edge: LEdgeId,
    pub index: usize,
}

/// A reference to a single junction point inside an edge's `JUNCTION_POINTS`
/// chain; we keep the location so the original can be offset during
/// `applyLayout`.
#[derive(Clone, Copy, Debug)]
pub struct JpRef {
    pub edge: LEdgeId,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct VerticalSegment {
    /// Nodes that may become the parent of the CNode representing this segment
    /// (`potentialGroupParents`, holding CNode ids).
    pub potential_group_parents: Vec<usize>,
    /// Edges that contribute at least partly to this vertical segment.
    pub represented_ledges: Vec<LEdgeId>,
    /// Bend points within this segment's hitbox; adjusted after compaction.
    /// Each entry references a concrete bend in an edge's bend chain so that we
    /// can mutate the originals.
    pub affected_bends: Vec<BendRef>,
    /// Bounding boxes (e.g. of splines) to be adjusted after compaction
    /// (unused for orthogonal; kept for fidelity / future spline support).
    pub affected_bounding_boxes: Vec<usize>,
    /// The area occupied by this vertical segment.
    pub hitbox: ElkRectangle,
    /// Junction points of the original edge, between the bend points; offset
    /// after compaction. Each references the original chain entry.
    pub junction_points: Vec<JpRef>,
    /// Whether spacing on a particular side should be ignored.
    pub ignore_spacing: Quadruplet,
    /// Pre-computed constraints added to the CNode representing this segment
    /// (indices into the transformer's segment vec).
    pub constraints: Vec<usize>,
    /// North/south port this segment leaves/enters (orthogonal edges).
    pub a_port: Option<LPortId>,
    /// Segments that have been joined with this one (indices into the vec).
    pub joined: Vec<usize>,
}

impl VerticalSegment {
    /// Constructs a vertical segment from two bend points, a CNode and an LEdge.
    ///
    /// `bend1_ref`/`bend2_ref` reference the concrete bend points (when they
    /// originate from the edge's bend chain). Synthetic bends (created for the
    /// n/s port segment) have `None` and are not adjusted afterwards — the
    /// synthetic `KVector` is a throwaway not part of the edge.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bend1: KVector,
        bend2: KVector,
        bend1_ref: Option<BendRef>,
        bend2_ref: Option<BendRef>,
        c_node: Option<usize>,
        in_junction_points: &[(JpRef, KVector)],
    ) -> Self {
        let mut affected_bends = Vec::new();
        if let Some(b) = bend1_ref {
            affected_bends.push(b);
        }
        if let Some(b) = bend2_ref {
            affected_bends.push(b);
        }

        let hitbox = ElkRectangle::new(
            bend1.x.min(bend2.x),
            bend1.y.min(bend2.y),
            (bend1.x - bend2.x).abs(),
            (bend1.y - bend2.y).abs(),
        );

        let mut junction_points = Vec::new();
        for (jp_ref, jp) in in_junction_points {
            if compare_fuzzy::eq(jp.x, bend1.x) {
                junction_points.push(*jp_ref);
            }
        }

        let mut potential_group_parents = Vec::new();
        if let Some(c) = c_node {
            potential_group_parents.push(c);
        }

        VerticalSegment {
            potential_group_parents,
            represented_ledges: Vec::new(),
            affected_bends,
            affected_bounding_boxes: Vec::new(),
            hitbox,
            junction_points,
            ignore_spacing: Quadruplet::default(),
            constraints: Vec::new(),
            a_port: None,
            joined: Vec::new(),
        }
    }

    /// Joins this segment with `other` (`joinWith`). `other` is unaltered.
    pub fn join_with(&mut self, other: &VerticalSegment, other_index: usize) {
        self.represented_ledges.extend(other.represented_ledges.iter().copied());
        self.affected_bends.extend(other.affected_bends.iter().copied());
        self.affected_bounding_boxes
            .extend(other.affected_bounding_boxes.iter().copied());
        self.junction_points.extend(other.junction_points.iter().copied());
        self.constraints.extend(other.constraints.iter().copied());
        self.potential_group_parents
            .extend(other.potential_group_parents.iter().copied());

        let new_x = self.hitbox.x.min(other.hitbox.x);
        let new_y = self.hitbox.y.min(other.hitbox.y);
        let max_x = (self.hitbox.x + self.hitbox.width).max(other.hitbox.x + other.hitbox.width);
        let new_w = max_x - new_x;
        let max_y = (self.hitbox.y + self.hitbox.height).max(other.hitbox.y + other.hitbox.height);
        let new_h = max_y - new_y;
        self.hitbox = ElkRectangle::new(new_x, new_y, new_w, new_h);

        self.ignore_spacing.apply_or(&other.ignore_spacing);

        if self.a_port.is_none() {
            self.a_port = other.a_port;
        }

        self.joined.extend(other.joined.iter().copied());
        self.joined.push(other_index);
    }

    /// `intersects`.
    pub fn intersects(&self, o: &VerticalSegment) -> bool {
        compare_fuzzy::eq(self.hitbox.x, o.hitbox.x)
            && !(compare_fuzzy::lt(self.hitbox.bottom_left().y, o.hitbox.y)
                || compare_fuzzy::lt(o.hitbox.bottom_left().y, self.hitbox.y))
    }

    /// `compareTo`.
    pub fn compare_to(&self, o: &VerticalSegment) -> Ordering {
        let d = compare_fuzzy::fuzzy_compare(self.hitbox.x, o.hitbox.x, compare_fuzzy::TOLERANCE);
        if d == Ordering::Equal {
            self.hitbox.y.partial_cmp(&o.hitbox.y).unwrap_or(Ordering::Equal)
        } else {
            d
        }
    }
}
