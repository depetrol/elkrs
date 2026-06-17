//!
//! Edge routing implementation that creates orthogonal bend points, inspired
//! by Sander's hypergraph routing and the cycle breaking from di Battista et
//! al. The actual routing direction is handled by a
//! [`BaseRoutingDirectionStrategy`].

use std::collections::HashMap;

use crate::core::javacompat::JavaRandom;
use crate::core::options::PortSide;

use crate::alg_layered::graph::{LGraphArena, LNodeId, LPortId};

use super::direction::{BaseRoutingDirectionStrategy, RoutingDirection};
use super::hyper_edge_cycle_detector;
use super::hyper_edge_segment::{SegmentId, SegmentStore};
use super::hyper_edge_segment_dependency as dependency;
use super::hyper_edge_segment_splitter;

/// differences below this tolerance value are treated as zero.
pub const TOLERANCE: f64 = 1e-3;

/// a special return value used by the conflict counting method.
const CRITICAL_CONFLICTS_DETECTED: i32 = -1;

/// factor for edge spacing used to determine the conflict threshold.
const CONFLICT_THRESHOLD_FACTOR: f64 = 0.5;
/// factor to compute the critical conflict threshold.
const CRITICAL_CONFLICT_THRESHOLD_FACTOR: f64 = 0.2;

/// weight penalty for (non-critical) conflicts.
const CONFLICT_PENALTY: i32 = 1;
/// weight penalty for crossings.
const CROSSING_PENALTY: i32 = 16;

pub struct OrthogonalRoutingGenerator {
    /// routing direction strategy.
    pub routing_strategy: BaseRoutingDirectionStrategy,
    /// spacing between edges.
    edge_spacing: f64,
    /// threshold at which horizontal line segments are considered to be too
    /// close to one another.
    conflict_threshold: f64,
    /// threshold at which horizontal line segments are considered to overlap
    /// (recomputed for each pair of layers).
    critical_conflict_threshold: f64,
}

impl OrthogonalRoutingGenerator {
    /// Constructor (the debug prefix is accepted for parity but unused;
    /// debug graph output is not ported).
    pub fn new(direction: RoutingDirection, edge_spacing: f64, _debug_prefix: &str) -> Self {
        OrthogonalRoutingGenerator {
            routing_strategy: BaseRoutingDirectionStrategy::for_routing_direction(direction),
            edge_spacing,
            conflict_threshold: CONFLICT_THRESHOLD_FACTOR * edge_spacing,
            critical_conflict_threshold: 0.0,
        }
    }

    ///////////////////////////////////////////////////////////////////////////////
    // Edge Routing

    /// `routeEdges`: routes edges between the given layers and returns
    /// the number of routing slots. The random number generator replaces
    /// the `InternalProperties.RANDOM` graph property.
    pub fn route_edges(
        &mut self,
        a: &mut LGraphArena,
        source_layer_nodes: Option<&[LNodeId]>,
        _source_layer_index: i32,
        target_layer_nodes: Option<&[LNodeId]>,
        start_pos: f64,
        random: &mut JavaRandom,
    ) -> i32 {
        // Keep track of our hyperedge segments, and which ports they were created for
        let mut store = SegmentStore::new();
        let mut port_to_edge_segment_map: HashMap<LPortId, SegmentId> = HashMap::new();
        let mut edge_segments: Vec<SegmentId> = Vec::new();

        // create hyperedge segments for eastern output ports of the left layer and
        // for western output ports of the right layer
        self.create_hyper_edge_segments(
            a,
            source_layer_nodes,
            self.routing_strategy.source_port_side(),
            &mut store,
            &mut edge_segments,
            &mut port_to_edge_segment_map,
        );
        self.create_hyper_edge_segments(
            a,
            target_layer_nodes,
            self.routing_strategy.target_port_side(),
            &mut store,
            &mut edge_segments,
            &mut port_to_edge_segment_map,
        );

        // Our critical conflict threshold is a fraction of the minimum distance
        // between two horizontal hyperedge segments
        self.critical_conflict_threshold = CRITICAL_CONFLICT_THRESHOLD_FACTOR
            * minimum_horizontal_segment_distance(&store, &edge_segments);

        // create dependencies for the hyperedge segment ordering graph
        let mut critical_dependency_count = 0;
        for first_idx in 0..edge_segments.len().saturating_sub(1) {
            let first_segment = edge_segments[first_idx];
            for second_idx in first_idx + 1..edge_segments.len() {
                critical_dependency_count += self.create_dependency_if_necessary(
                    &mut store,
                    first_segment,
                    edge_segments[second_idx],
                );
            }
        }

        // if there are at least two critical dependencies, there may be critical
        // cycles that need to be broken
        if critical_dependency_count >= 2 {
            self.break_critical_cycles(&mut store, &mut edge_segments, random);
        }

        // break non-critical cycles
        break_non_critical_cycles(&mut store, &edge_segments, random);

        // assign ranks to the edge segments
        topological_numbering(&mut store, &edge_segments);

        // set bend points with appropriate coordinates
        let mut rank_count = -1;
        for &node in &edge_segments {
            // edges that are just straight lines don't take up a slot and don't need bend points
            if (store.segments[node].start_coordinate() - store.segments[node].end_coordinate())
                .abs()
                < TOLERANCE
            {
                continue;
            }

            rank_count = rank_count.max(store.segments[node].routing_slot);

            self.routing_strategy.calculate_bend_points(
                a,
                &store,
                node,
                start_pos,
                self.edge_spacing,
            );
        }

        // release the created resources
        self.routing_strategy.clear_created_junction_points();
        rank_count + 1
    }

    ///////////////////////////////////////////////////////////////////////////////
    // Hyper Edge Graph Creation

    /// `createHyperEdgeSegments`: creates hyperedge segments for the given layer.
    fn create_hyper_edge_segments(
        &self,
        a: &LGraphArena,
        nodes: Option<&[LNodeId]>,
        port_side: PortSide,
        store: &mut SegmentStore,
        hyper_edges: &mut Vec<SegmentId>,
        port_to_hyper_edge_segment_map: &mut HashMap<LPortId, SegmentId>,
    ) {
        if let Some(nodes) = nodes {
            for &node in nodes {
                for &port in &a.node(node).ports {
                    if a.port(port).outgoing_edges.is_empty() || a.port(port).side != port_side {
                        continue;
                    }
                    if !port_to_hyper_edge_segment_map.contains_key(&port) {
                        let hyper_edge = store.create_segment();
                        hyper_edges.push(hyper_edge);
                        store.add_port_positions(
                            a,
                            hyper_edge,
                            port,
                            port_to_hyper_edge_segment_map,
                            &self.routing_strategy,
                        );
                    }
                }
            }
        }
    }

    /// `createDependencyIfNecessary`: creates dependencies between the
    /// two given hyperedge segments, if one is needed. Returns the number of
    /// critical dependencies that were added.
    pub(super) fn create_dependency_if_necessary(
        &self,
        store: &mut SegmentStore,
        he1: SegmentId,
        he2: SegmentId,
    ) -> i32 {
        // check if at least one of the two nodes is just a straight line; those
        // don't create dependencies since they don't take up a slot
        if (store.segments[he1].start_coordinate() - store.segments[he1].end_coordinate()).abs()
            < TOLERANCE
            || (store.segments[he2].start_coordinate() - store.segments[he2].end_coordinate())
                .abs()
                < TOLERANCE
        {
            return 0;
        }

        // compare number of conflicts for both variants
        let conflicts1 = self.count_conflicts(
            &store.segments[he1].outgoing_connection_coordinates,
            &store.segments[he2].incoming_connection_coordinates,
        );
        let conflicts2 = self.count_conflicts(
            &store.segments[he2].outgoing_connection_coordinates,
            &store.segments[he1].incoming_connection_coordinates,
        );

        let critical_conflicts_detected = conflicts1 == CRITICAL_CONFLICTS_DETECTED
            || conflicts2 == CRITICAL_CONFLICTS_DETECTED;
        let mut critical_dependency_count = 0;

        if critical_conflicts_detected {
            // Check which critical dependencies have to be added
            if conflicts1 == CRITICAL_CONFLICTS_DETECTED {
                // hyperedge 1 MUST NOT be left of hyperedge 2
                dependency::create_and_add_critical(store, he2, he1);
                critical_dependency_count += 1;
            }

            if conflicts2 == CRITICAL_CONFLICTS_DETECTED {
                // hyperedge 2 MUST NOT be left of hyperedge 1
                dependency::create_and_add_critical(store, he1, he2);
                critical_dependency_count += 1;
            }
        } else {
            // we did not detect critical conflicts, so count the number of
            // crossings for both variants
            let mut crossings1 = count_crossings(
                &store.segments[he1].outgoing_connection_coordinates,
                store.segments[he2].start_coordinate(),
                store.segments[he2].end_coordinate(),
            );
            crossings1 += count_crossings(
                &store.segments[he2].incoming_connection_coordinates,
                store.segments[he1].start_coordinate(),
                store.segments[he1].end_coordinate(),
            );
            let mut crossings2 = count_crossings(
                &store.segments[he2].outgoing_connection_coordinates,
                store.segments[he1].start_coordinate(),
                store.segments[he1].end_coordinate(),
            );
            crossings2 += count_crossings(
                &store.segments[he1].incoming_connection_coordinates,
                store.segments[he2].start_coordinate(),
                store.segments[he2].end_coordinate(),
            );

            // compute the penalty; crossings are deemed worse than conflicts
            let dep_value1 = CONFLICT_PENALTY * conflicts1 + CROSSING_PENALTY * crossings1;
            let dep_value2 = CONFLICT_PENALTY * conflicts2 + CROSSING_PENALTY * crossings2;

            if dep_value1 < dep_value2 {
                // hyperedge 1 wants to be left of hyperedge 2
                dependency::create_and_add_regular(store, he1, he2, dep_value2 - dep_value1);
            } else if dep_value1 > dep_value2 {
                // hyperedge 2 wants to be left of hyperedge 1
                dependency::create_and_add_regular(store, he2, he1, dep_value1 - dep_value2);
            } else if dep_value1 > 0 && dep_value2 > 0 {
                // create two dependencies with zero weight
                dependency::create_and_add_regular(store, he1, he2, 0);
                dependency::create_and_add_regular(store, he2, he1, 0);
            }
        }

        critical_dependency_count
    }

    /// `countConflicts`: counts the number of conflicts for the given
    /// (sorted) lists of positions, or [`CRITICAL_CONFLICTS_DETECTED`] if a
    /// critical conflict was detected.
    fn count_conflicts(&self, posis1: &[f64], posis2: &[f64]) -> i32 {
        let mut conflicts = 0;

        if !posis1.is_empty() && !posis2.is_empty() {
            let mut idx1 = 0;
            let mut idx2 = 0;
            let mut pos1 = posis1[0];
            let mut pos2 = posis2[0];

            loop {
                if pos1 > pos2 - self.critical_conflict_threshold
                    && pos1 < pos2 + self.critical_conflict_threshold
                {
                    // We're done as soon as we find a single critical conflict
                    return -1;
                } else if pos1 > pos2 - self.conflict_threshold
                    && pos1 < pos2 + self.conflict_threshold
                {
                    conflicts += 1;
                }

                if pos1 <= pos2 && idx1 + 1 < posis1.len() {
                    idx1 += 1;
                    pos1 = posis1[idx1];
                } else if pos2 <= pos1 && idx2 + 1 < posis2.len() {
                    idx2 += 1;
                    pos2 = posis2[idx2];
                } else {
                    break;
                }
            }
        }

        conflicts
    }

    ///////////////////////////////////////////////////////////////////////////////
    // Cycle Breaking

    /// `breakCriticalCycles`: finds and breaks critical cycles by
    /// splitting edge segments.
    fn break_critical_cycles(
        &self,
        store: &mut SegmentStore,
        edge_segments: &mut Vec<SegmentId>,
        random: &mut JavaRandom,
    ) {
        let cycle_dependencies =
            hyper_edge_cycle_detector::detect_cycles(store, edge_segments, true, random);

        hyper_edge_segment_splitter::split_segments(
            self,
            store,
            &cycle_dependencies,
            edge_segments,
            self.critical_conflict_threshold,
        );
    }
}

/// `countCrossings`: counts the number of positions in the
/// critical area between `start` and `end`.
pub(super) fn count_crossings(posis: &[f64], start: f64, end: f64) -> i32 {
    let mut crossings = 0;
    for &pos in posis {
        if pos > end {
            break;
        } else if pos >= start {
            crossings += 1;
        }
    }
    crossings
}

/// `minimumHorizontalSegmentDistance`: minimum distance between any two
/// adjacent source connections and any two adjacent target connections.
fn minimum_horizontal_segment_distance(store: &SegmentStore, edge_segments: &[SegmentId]) -> f64 {
    let min_incoming_distance = minimum_difference(
        edge_segments
            .iter()
            .flat_map(|&s| store.segments[s].incoming_connection_coordinates.iter().copied()),
    );
    let min_outgoing_distance = minimum_difference(
        edge_segments
            .iter()
            .flat_map(|&s| store.segments[s].outgoing_connection_coordinates.iter().copied()),
    );

    // Math.min; operands are never NaN
    if min_incoming_distance <= min_outgoing_distance {
        min_incoming_distance
    } else {
        min_outgoing_distance
    }
}

/// `minimumDifference`: the smallest difference between any two numbers
/// in the given stream; `Double.MAX_VALUE` if there are less than two.
fn minimum_difference(numbers: impl Iterator<Item = f64>) -> f64 {
    let mut numbers: Vec<f64> = numbers.collect();
    numbers.sort_by(|x, y| x.total_cmp(y));
    numbers.dedup_by(|x, y| x.to_bits() == y.to_bits());

    let mut min_difference = f64::MAX;

    if numbers.len() >= 2 {
        for window in numbers.windows(2) {
            // This relies on the fact that the numbers are distinct and sorted ascendingly
            let difference = window[1] - window[0];
            if difference < min_difference {
                min_difference = difference;
            }
        }
    }

    min_difference
}

/// `breakNonCriticalCycles`: finds and breaks non-critical cycles
/// by removing and reversing non-critical dependencies. (Also used by the
/// self loop routing code.)
pub fn break_non_critical_cycles(
    store: &mut SegmentStore,
    edge_segments: &[SegmentId],
    random: &mut JavaRandom,
) {
    let cycle_dependencies =
        hyper_edge_cycle_detector::detect_cycles(store, edge_segments, false, random);

    for cycle_dependency in cycle_dependencies {
        if store.dependencies[cycle_dependency].weight == 0 {
            // Simply remove this dependency. This assumes that only two-cycles
            // will have dependency weight 0
            dependency::remove(store, cycle_dependency);
        } else {
            dependency::reverse(store, cycle_dependency);
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// Topological Ordering

/// `topologicalNumbering`: performs a topological numbering of the given
/// hyperedge segments.
fn topological_numbering(store: &mut SegmentStore, segments: &[SegmentId]) {
    // determine sources, targets, incoming count and outgoing count; targets
    // are only added to the list if all their horizontal segments point right
    let mut sources: Vec<SegmentId> = Vec::new();
    let mut rightward_targets: Vec<SegmentId> = Vec::new();
    for &node in segments {
        let s = &mut store.segments[node];
        s.in_dep_weight = s.incoming_segment_dependencies.len() as i32;
        s.out_dep_weight = s.outgoing_segment_dependencies.len() as i32;

        if s.in_dep_weight == 0 {
            sources.push(node);
        }

        if s.out_dep_weight == 0 && s.incoming_connection_coordinates.is_empty() {
            rightward_targets.push(node);
        }
    }

    let mut max_rank = -1;

    // assign ranks using topological numbering
    while !sources.is_empty() {
        let node = sources.remove(0);
        let out_deps = store.segments[node].outgoing_segment_dependencies.clone();
        for dep in out_deps {
            let target = store.dependencies[dep].target.unwrap();
            let new_slot = store.segments[node].routing_slot + 1;
            let t = &mut store.segments[target];
            t.routing_slot = t.routing_slot.max(new_slot);
            max_rank = max_rank.max(t.routing_slot);

            t.in_dep_weight -= 1;
            if t.in_dep_weight == 0 {
                sources.push(target);
            }
        }
    }

    /* Move all hyperedge segments with horizontal segments only pointing
     * rightwards as far right as possible. */
    if max_rank > -1 {
        // assign all target nodes with horizontal segments pointing to the
        // right the rightmost rank
        for &node in &rightward_targets {
            store.segments[node].routing_slot = max_rank;
        }

        // let all other segments with horizontal segments pointing rightwards
        // move as far right as possible
        let mut rightward_targets = rightward_targets;
        while !rightward_targets.is_empty() {
            let node = rightward_targets.remove(0);

            // The node only has connections to western ports
            let in_deps = store.segments[node].incoming_segment_dependencies.clone();
            for dep in in_deps {
                let source = store.dependencies[dep].source.unwrap();
                if !store.segments[source].incoming_connection_coordinates.is_empty() {
                    continue;
                }

                let new_slot = store.segments[node].routing_slot - 1;
                let s = &mut store.segments[source];
                s.routing_slot = s.routing_slot.min(new_slot);

                s.out_dep_weight -= 1;
                if s.out_dep_weight == 0 {
                    rightward_targets.push(source);
                }
            }
        }
    }
}
