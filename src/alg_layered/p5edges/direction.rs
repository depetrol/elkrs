
use crate::core::options::PortSide;
use crate::graph::math::KVector;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LPortId};
use crate::alg_layered::options_gen as lopts;

use super::hyper_edge_segment::{SegmentId, SegmentStore};
use super::orthogonal_routing_generator::TOLERANCE;

/// Enumeration of available routing directions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoutingDirection {
    /// west to east routing direction.
    WestToEast,
    /// north to south routing direction.
    NorthToSouth,
    /// south to north routing direction.
    SouthToNorth,
}

pub struct BaseRoutingDirectionStrategy {
    direction: RoutingDirection,
    /// set of already created junction points, to avoid multiple points at
    /// the same position (a Vec with linear lookup, using coordinate-based
    /// equality).
    created_junction_points: Vec<KVector>,
}

impl BaseRoutingDirectionStrategy {
    /// `forRoutingDirection`.
    pub fn for_routing_direction(direction: RoutingDirection) -> Self {
        BaseRoutingDirectionStrategy { direction, created_junction_points: Vec::new() }
    }

    /// `getPortPositionOnHyperNode`.
    pub fn port_position_on_hyper_node(&self, a: &LGraphArena, port: LPortId) -> f64 {
        let p = a.port(port);
        let node = a.node(p.node.unwrap());
        match self.direction {
            RoutingDirection::WestToEast => node.pos.y + p.pos.y + p.anchor.y,
            RoutingDirection::NorthToSouth | RoutingDirection::SouthToNorth => {
                node.pos.x + p.pos.x + p.anchor.x
            }
        }
    }

    /// `getSourcePortSide`.
    pub fn source_port_side(&self) -> PortSide {
        match self.direction {
            RoutingDirection::WestToEast => PortSide::EAST,
            RoutingDirection::NorthToSouth => PortSide::SOUTH,
            RoutingDirection::SouthToNorth => PortSide::NORTH,
        }
    }

    /// `getTargetPortSide`.
    pub fn target_port_side(&self) -> PortSide {
        match self.direction {
            RoutingDirection::WestToEast => PortSide::WEST,
            RoutingDirection::NorthToSouth => PortSide::NORTH,
            RoutingDirection::SouthToNorth => PortSide::SOUTH,
        }
    }

    /// `getCreatedJunctionPoints`.
    pub fn created_junction_points(&self) -> &[KVector] {
        &self.created_junction_points
    }

    /// `clearCreatedJunctionPoints`.
    pub fn clear_created_junction_points(&mut self) {
        self.created_junction_points.clear();
    }

    /// `calculateBendPoints` (dispatches to the strategy subclass).
    pub fn calculate_bend_points(
        &mut self,
        a: &mut LGraphArena,
        store: &SegmentStore,
        segment: SegmentId,
        start_pos: f64,
        edge_spacing: f64,
    ) {
        // We don't do anything with dummy segments; they are dealt with when
        // their partner is processed
        if store.segments[segment].is_dummy() {
            return;
        }

        // Calculate the coordinate of this segment's trunk; the sign of the
        // routing slot offset is the only difference between the strategies
        // apart from the axes used.
        let slot_sign = match self.direction {
            RoutingDirection::SouthToNorth => -1.0,
            _ => 1.0,
        };
        let segment_coordinate = start_pos
            + slot_sign * (store.segments[segment].routing_slot as f64 * edge_spacing);

        let ports = store.segments[segment].ports.clone();
        for port in ports {
            let source_pos = self.absolute_anchor_coordinate(a, port);

            let outgoing_edges = a.port(port).outgoing_edges.clone();
            for edge in outgoing_edges {
                if !a.edge_is_self_loop(edge) {
                    let target = a.edge(edge).target.unwrap();
                    let target_pos = self.absolute_anchor_coordinate(a, target);

                    if (source_pos - target_pos).abs() > TOLERANCE {
                        // We'll update these if we find that the segment was split
                        let mut current_coordinate = segment_coordinate;
                        let mut current_segment = segment;

                        let bend = self.make_bend(current_coordinate, source_pos);
                        a.edge_mut(edge).bend_points.add_last(bend);
                        self.add_junction_point_if_necessary(a, edge, store, current_segment, bend);

                        // If this segment was split, we need two additional bend points
                        let split_partner = store.segments[segment].split_partner;
                        if let Some(split_partner) = split_partner {
                            let split_pos =
                                store.segments[split_partner].incoming_connection_coordinates[0];

                            let bend = self.make_bend(current_coordinate, split_pos);
                            a.edge_mut(edge).bend_points.add_last(bend);
                            self.add_junction_point_if_necessary(
                                a, edge, store, current_segment, bend,
                            );

                            // Advance to the split partner's routing slot
                            current_coordinate = start_pos
                                + slot_sign
                                    * (store.segments[split_partner].routing_slot as f64
                                        * edge_spacing);
                            current_segment = split_partner;

                            let bend = self.make_bend(current_coordinate, split_pos);
                            a.edge_mut(edge).bend_points.add_last(bend);
                            self.add_junction_point_if_necessary(
                                a, edge, store, current_segment, bend,
                            );
                        }

                        let bend = self.make_bend(current_coordinate, target_pos);
                        a.edge_mut(edge).bend_points.add_last(bend);
                        self.add_junction_point_if_necessary(a, edge, store, current_segment, bend);
                    }
                }
            }
        }
    }

    /// `port.getAbsoluteAnchor()` projected onto the hyperedge axis: the y
    /// coordinate for west-to-east routing, the x coordinate otherwise.
    fn absolute_anchor_coordinate(&self, a: &LGraphArena, port: LPortId) -> f64 {
        let p = a.port(port);
        let node = a.node(p.node.unwrap());
        match self.direction {
            RoutingDirection::WestToEast => node.pos.y + p.pos.y + p.anchor.y,
            RoutingDirection::NorthToSouth | RoutingDirection::SouthToNorth => {
                node.pos.x + p.pos.x + p.anchor.x
            }
        }
    }

    /// Builds a bend point from the segment trunk coordinate and the
    /// connection coordinate, depending on the routing direction.
    fn make_bend(&self, segment_coordinate: f64, connection_coordinate: f64) -> KVector {
        match self.direction {
            RoutingDirection::WestToEast => KVector::new(segment_coordinate, connection_coordinate),
            RoutingDirection::NorthToSouth | RoutingDirection::SouthToNorth => {
                KVector::new(connection_coordinate, segment_coordinate)
            }
        }
    }

    /// `addJunctionPointIfNecessary`. The `vertical` flag is implied by
    /// the routing direction (`true` for west-to-east).
    fn add_junction_point_if_necessary(
        &mut self,
        a: &mut LGraphArena,
        edge: LEdgeId,
        store: &SegmentStore,
        segment: SegmentId,
        pos: KVector,
    ) {
        let vertical = self.direction == RoutingDirection::WestToEast;
        let p = if vertical { pos.y } else { pos.x };

        // If we already have this junction point, don't bother
        // (KVector equality: coordinate comparison with ==)
        if self.created_junction_points.iter().any(|jp| jp.x == pos.x && jp.y == pos.y) {
            return;
        }

        let seg = &store.segments[segment];

        // Whether the point lies somewhere inside the edge segment (without boundaries)
        let point_inside_edge_segment = p > seg.start_coordinate() && p < seg.end_coordinate();

        // Check if the point lies somewhere at the segment's boundary
        let mut point_at_segment_boundary = false;
        if !seg.incoming_connection_coordinates.is_empty()
            && !seg.outgoing_connection_coordinates.is_empty()
        {
            let incoming = &seg.incoming_connection_coordinates;
            let outgoing = &seg.outgoing_connection_coordinates;

            // Is the bend point at the start and joins another edge at the same position?
            point_at_segment_boundary |= (p - incoming[0]).abs() < TOLERANCE
                && (p - outgoing[0]).abs() < TOLERANCE;

            // Is the bend point at the end and joins another edge at the same position?
            point_at_segment_boundary |= (p - incoming[incoming.len() - 1]).abs() < TOLERANCE
                && (p - outgoing[outgoing.len() - 1]).abs() < TOLERANCE;
        }

        if point_inside_edge_segment || point_at_segment_boundary {
            // create a new junction point for the edge at the bend point's position
            let mut junction_points = a.edge(edge).properties.get(&lopts::JUNCTION_POINTS);
            junction_points.add_last(pos);
            a.edge(edge).properties.set(&lopts::JUNCTION_POINTS, junction_points);

            self.created_junction_points.push(pos);
        }
    }
}
