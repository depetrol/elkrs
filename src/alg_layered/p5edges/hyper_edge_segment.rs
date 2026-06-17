//!
//! Instances of this struct represent the "trunk" of a hyper edge. Segments and
//! their dependencies live in a
//! [`SegmentStore`] arena and reference each other through indices.

use std::collections::HashMap;

use crate::alg_layered::graph::{LGraphArena, LPortId};

use super::direction::BaseRoutingDirectionStrategy;
use super::hyper_edge_segment_dependency::{self as dependency, HyperEdgeSegmentDependency};

/// Index of a segment in the [`SegmentStore`].
pub type SegmentId = usize;
/// Index of a dependency in the [`SegmentStore`].
pub type DependencyId = usize;

pub struct HyperEdgeSegment {
    /// ports represented by this hypernode.
    pub ports: Vec<LPortId>,
    /// mark value used for cycle breaking.
    pub mark: i32,
    /// the routing slot determines the horizontal distance to the preceding layer.
    pub routing_slot: i32,
    /// start position of this edge segment (NaN initially).
    pub start_position: f64,
    /// end position of this edge segment (NaN initially).
    pub end_position: f64,
    /// sorted list of coordinates where incoming connections enter this segment.
    pub incoming_connection_coordinates: Vec<f64>,
    /// sorted list of coordinates where outgoing connections leave this segment.
    pub outgoing_connection_coordinates: Vec<f64>,
    /// list of outgoing dependencies to other edge segments.
    pub outgoing_segment_dependencies: Vec<DependencyId>,
    /// combined weight of all outgoing dependencies.
    pub out_dep_weight: i32,
    /// combined weight of critical outgoing dependencies.
    pub critical_out_dep_weight: i32,
    /// list of incoming dependencies from other edge segments.
    pub incoming_segment_dependencies: Vec<DependencyId>,
    /// combined weight of all incoming dependencies.
    pub in_dep_weight: i32,
    /// combined weight of critical incoming dependencies.
    pub critical_in_dep_weight: i32,
    /// if this segment is the result of a split, the other segment.
    pub split_partner: Option<SegmentId>,
    /// the segment that caused this segment to be split, if any.
    pub split_by: Option<SegmentId>,
}

impl HyperEdgeSegment {
    fn new() -> Self {
        HyperEdgeSegment {
            ports: Vec::new(),
            mark: 0,
            routing_slot: 0,
            start_position: f64::NAN,
            end_position: f64::NAN,
            incoming_connection_coordinates: Vec::new(),
            outgoing_connection_coordinates: Vec::new(),
            outgoing_segment_dependencies: Vec::new(),
            out_dep_weight: 0,
            critical_out_dep_weight: 0,
            incoming_segment_dependencies: Vec::new(),
            in_dep_weight: 0,
            critical_in_dep_weight: 0,
            split_partner: None,
            split_by: None,
        }
    }

    pub fn start_coordinate(&self) -> f64 {
        self.start_position
    }

    pub fn end_coordinate(&self) -> f64 {
        self.end_position
    }

    pub fn length(&self) -> f64 {
        self.end_coordinate() - self.start_coordinate()
    }

    pub fn represents_hyperedge(&self) -> bool {
        self.incoming_connection_coordinates.len() + self.outgoing_connection_coordinates.len() > 2
    }

    pub fn is_dummy(&self) -> bool {
        self.split_partner.is_some() && self.split_by.is_none()
    }

    pub fn recompute_extent(&mut self) {
        self.start_position = f64::NAN;
        self.end_position = f64::NAN;

        let (mut start, mut end) = (self.start_position, self.end_position);
        recompute_extent_with(&mut start, &mut end, &self.incoming_connection_coordinates);
        recompute_extent_with(&mut start, &mut end, &self.outgoing_connection_coordinates);
        self.start_position = start;
        self.end_position = end;
    }
}

/// Assumes the positions are sorted ascendingly.
fn recompute_extent_with(start_position: &mut f64, end_position: &mut f64, positions: &[f64]) {
    if !positions.is_empty() {
        let first = positions[0];
        let last = positions[positions.len() - 1];

        // set new start position
        if start_position.is_nan() {
            *start_position = first;
        } else {
            // min; operands are never NaN here
            *start_position = if *start_position <= first { *start_position } else { first };
        }

        // set new end position
        if end_position.is_nan() {
            *end_position = last;
        } else {
            // max; operands are never NaN here
            *end_position = if *end_position >= last { *end_position } else { last };
        }
    }
}

/// `insertSorted`. Note the quirk: each existing value is converted through
/// `Double.floatValue()` (a float cast) before being compared with the new
/// value.
pub(super) fn insert_sorted(list: &mut Vec<f64>, value: f64) {
    let mut insert_index = list.len();
    for (i, &existing) in list.iter().enumerate() {
        let next = existing as f32 as f64;
        if next == value {
            // an exactly equal value is already present in the list
            return;
        } else if next > value {
            insert_index = i;
            break;
        }
    }
    list.insert(insert_index, value);
}

/// Arena holding all hyperedge segments and their dependencies created while
/// routing one layer pair (plus temporary segments from split simulation).
#[derive(Default)]
pub struct SegmentStore {
    pub segments: Vec<HyperEdgeSegment>,
    pub dependencies: Vec<HyperEdgeSegmentDependency>,
}

impl SegmentStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_segment(&mut self) -> SegmentId {
        let id = self.segments.len();
        self.segments.push(HyperEdgeSegment::new());
        id
    }

    /// Adds the positions of the
    /// given port and all (transitively) connected ports.
    pub fn add_port_positions(
        &mut self,
        a: &LGraphArena,
        seg: SegmentId,
        port: LPortId,
        hyper_edge_segment_map: &mut HashMap<LPortId, SegmentId>,
        strategy: &BaseRoutingDirectionStrategy,
    ) {
        hyper_edge_segment_map.insert(port, seg);
        self.segments[seg].ports.push(port);
        let port_pos = strategy.port_position_on_hyper_node(a, port);

        // add the new port position to the respective list
        if a.port(port).side == strategy.source_port_side() {
            insert_sorted(&mut self.segments[seg].incoming_connection_coordinates, port_pos);
        } else {
            insert_sorted(&mut self.segments[seg].outgoing_connection_coordinates, port_pos);
        }

        // update start and end coordinates
        self.segments[seg].recompute_extent();

        // add connected ports (predecessor ports followed by successor ports)
        let mut connected_ports: Vec<LPortId> = Vec::new();
        for &edge in &a.port(port).incoming_edges {
            connected_ports.push(a.edge(edge).source.unwrap());
        }
        for &edge in &a.port(port).outgoing_edges {
            connected_ports.push(a.edge(edge).target.unwrap());
        }
        for other_port in connected_ports {
            if !hyper_edge_segment_map.contains_key(&other_port) {
                self.add_port_positions(a, seg, other_port, hyper_edge_segment_map, strategy);
            }
        }
    }

    /// Returns `(newSplit,
    /// newSplitPartner)`. The new segments live in this store but are not part
    /// of any segment list.
    pub fn simulate_split(&mut self, seg: SegmentId) -> (SegmentId, SegmentId) {
        let new_split = self.create_segment();
        let new_split_partner = self.create_segment();

        let incoming = self.segments[seg].incoming_connection_coordinates.clone();
        let outgoing = self.segments[seg].outgoing_connection_coordinates.clone();
        let split_by = self.segments[seg].split_by;

        {
            let s = &mut self.segments[new_split];
            s.incoming_connection_coordinates = incoming;
            s.split_by = split_by;
            s.split_partner = Some(new_split_partner);
            s.recompute_extent();
        }
        {
            let s = &mut self.segments[new_split_partner];
            s.outgoing_connection_coordinates = outgoing;
            s.split_partner = Some(new_split);
            s.recompute_extent();
        }

        (new_split, new_split_partner)
    }

    /// Splits this segment into two and returns the new segment.
    pub fn split_at(&mut self, seg: SegmentId, split_position: f64) -> SegmentId {
        let split_partner = self.create_segment();
        self.segments[seg].split_partner = Some(split_partner);
        self.segments[split_partner].split_partner = Some(seg);

        // Move all target positions over to the new segment
        let outgoing = std::mem::take(&mut self.segments[seg].outgoing_connection_coordinates);
        self.segments[split_partner].outgoing_connection_coordinates = outgoing;

        // Link the two
        self.segments[seg].outgoing_connection_coordinates.push(split_position);
        self.segments[split_partner].incoming_connection_coordinates.push(split_position);

        // Recompute their outer coordinates
        self.segments[seg].recompute_extent();
        self.segments[split_partner].recompute_extent();

        // Clear dependencies so they can be regenerated later
        while let Some(&dep) = self.segments[seg].incoming_segment_dependencies.first() {
            dependency::remove(self, dep);
        }
        while let Some(&dep) = self.segments[seg].outgoing_segment_dependencies.first() {
            dependency::remove(self, dep);
        }

        split_partner
    }
}
