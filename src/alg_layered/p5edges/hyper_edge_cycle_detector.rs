//!
//! Finds a set of dependencies to remove or reverse to break cycles in the
//! conflict graph of hyperedge segments (greedy feedback arc set heuristic by
//! Eades, Lin, Smyth).

use std::collections::{BTreeMap, VecDeque};

use crate::core::javacompat::JavaRandom;

use super::hyper_edge_segment::{DependencyId, SegmentId, SegmentStore};
use super::hyper_edge_segment_dependency::DependencyType;

/// `detectCycles`: finds a set of dependencies whose reversal or removal
/// will make the graph acyclic.
pub fn detect_cycles(
    store: &mut SegmentStore,
    segments: &[SegmentId],
    critical_only: bool,
    random: &mut JavaRandom,
) -> Vec<DependencyId> {
    let mut result: Vec<DependencyId> = Vec::new();

    let mut sources: VecDeque<SegmentId> = VecDeque::new();
    let mut sinks: VecDeque<SegmentId> = VecDeque::new();

    // initialize values for the algorithm
    initialize(store, segments, &mut sources, &mut sinks, critical_only);

    // assign marks to all nodes
    compute_linear_ordering_marks(store, segments, &mut sources, &mut sinks, critical_only, random);

    // process edges that point left: remove those of zero weight, reverse the others
    for &source in segments {
        for &out_dependency in &store.segments[source].outgoing_segment_dependencies {
            let dep = &store.dependencies[out_dependency];
            // Only consider critical dependencies, if required
            if !critical_only || dep.dependency_type == DependencyType::Critical {
                if store.segments[source].mark > store.segments[dep.target.unwrap()].mark {
                    result.push(out_dependency);
                }
            }
        }
    }

    result
}

/// `initialize`: sets mark, in/out weights of each segment and fills the
/// sources and sinks lists. Marks end up at -1 .. -segments.len().
fn initialize(
    store: &mut SegmentStore,
    segments: &[SegmentId],
    sources: &mut VecDeque<SegmentId>,
    sinks: &mut VecDeque<SegmentId>,
    critical_only: bool,
) {
    let mut next_mark = -1;
    for &segment in segments {
        // Sum up the weights of our critical dependencies
        let mut critical_in_weight = 0;
        let mut critical_out_weight = 0;
        let mut total_in_weight = 0;
        let mut total_out_weight = 0;

        for &dep in &store.segments[segment].incoming_segment_dependencies {
            let d = &store.dependencies[dep];
            total_in_weight += d.weight;
            if d.dependency_type == DependencyType::Critical {
                critical_in_weight += d.weight;
            }
        }
        for &dep in &store.segments[segment].outgoing_segment_dependencies {
            let d = &store.dependencies[dep];
            total_out_weight += d.weight;
            if d.dependency_type == DependencyType::Critical {
                critical_out_weight += d.weight;
            }
        }

        // If we're only considering critical dependencies, we'll ignore the others
        let in_weight = if critical_only { critical_in_weight } else { total_in_weight };
        let out_weight = if critical_only { critical_out_weight } else { total_out_weight };

        let s = &mut store.segments[segment];
        s.mark = next_mark;
        next_mark -= 1;

        // Apply the weight
        s.in_dep_weight = in_weight;
        s.critical_in_dep_weight = critical_in_weight;
        s.out_dep_weight = out_weight;
        s.critical_out_dep_weight = critical_out_weight;

        // Add the segment to either sources or sinks if the corresponding weight is zero
        if out_weight == 0 {
            sinks.push_back(segment);
        } else if in_weight == 0 {
            sources.push_back(segment);
        }
    }
}

/// `computeLinearOrderingMarks`.
fn compute_linear_ordering_marks(
    store: &mut SegmentStore,
    segments: &[SegmentId],
    sources: &mut VecDeque<SegmentId>,
    sinks: &mut VecDeque<SegmentId>,
    critical_only: bool,
    random: &mut JavaRandom,
) {
    let mut unprocessed: BTreeMap<i32, SegmentId> =
        segments.iter().map(|&s| (store.segments[s].mark, s)).collect();
    let mut max_segments: Vec<SegmentId> = Vec::new();

    // We'll mark sinks with marks < markBase and sources with marks > markBase
    let mark_base = segments.len() as i32;
    let mut next_sink_mark = mark_base - 1;
    let mut next_source_mark = mark_base + 1;

    while !unprocessed.is_empty() {
        while let Some(sink) = sinks.pop_front() {
            unprocessed.remove(&store.segments[sink].mark);
            store.segments[sink].mark = next_sink_mark;
            next_sink_mark -= 1;
            update_neighbors(store, sink, sources, sinks, critical_only);
        }

        while let Some(source) = sources.pop_front() {
            unprocessed.remove(&store.segments[source].mark);
            store.segments[source].mark = next_source_mark;
            next_source_mark += 1;
            update_neighbors(store, source, sources, sinks, critical_only);
        }

        // If any segments are still unprocessed, they are neither source nor sink.
        // Assemble the list of segments with the highest out flow.
        let mut max_outflow = i32::MIN;
        for (_, &segment) in unprocessed.iter() {
            // Once we find a segment that still has an outgoing critical
            // dependency and no incoming ones, we'll take that and leave
            if !critical_only
                && store.segments[segment].critical_out_dep_weight > 0
                && store.segments[segment].critical_in_dep_weight <= 0
            {
                max_segments.clear();
                max_segments.push(segment);
                break;
            }

            let outflow =
                store.segments[segment].out_dep_weight - store.segments[segment].in_dep_weight;
            if outflow >= max_outflow {
                if outflow > max_outflow {
                    max_segments.clear();
                    max_outflow = outflow;
                }
                max_segments.push(segment);
            }
        }

        // If there are segments with maximal out flow, select one randomly
        if !max_segments.is_empty() {
            let max_node = max_segments[random.next_int_bound(max_segments.len() as i32) as usize];
            unprocessed.remove(&store.segments[max_node].mark);
            store.segments[max_node].mark = next_source_mark;
            next_source_mark += 1;
            update_neighbors(store, max_node, sources, sinks, critical_only);
            max_segments.clear();
        }
    }

    // shift ranks that are left of the mark base so that sinks now have higher marks than sources
    let shift_base = segments.len() as i32 + 1;
    for &node in segments {
        if store.segments[node].mark < mark_base {
            store.segments[node].mark += shift_base;
        }
    }
}

/// `updateNeighbors`: updates in-weight and out-weight values of the
/// neighbors of the given node, simulating its removal from the graph.
fn update_neighbors(
    store: &mut SegmentStore,
    node: SegmentId,
    sources: &mut VecDeque<SegmentId>,
    sinks: &mut VecDeque<SegmentId>,
    critical_only: bool,
) {
    // process following nodes
    let out_deps = store.segments[node].outgoing_segment_dependencies.clone();
    for dep in out_deps {
        let (dep_type, weight, target) = {
            let d = &store.dependencies[dep];
            (d.dependency_type, d.weight, d.target.unwrap())
        };
        if !critical_only || dep_type == DependencyType::Critical {
            if store.segments[target].mark < 0 && weight > 0 {
                // Remove weight (and possibly critical weight) from the target
                let t = &mut store.segments[target];
                t.in_dep_weight -= weight;
                if dep_type == DependencyType::Critical {
                    t.critical_in_dep_weight -= weight;
                }

                if t.in_dep_weight <= 0 && t.out_dep_weight > 0 {
                    sources.push_back(target);
                }
            }
        }
    }

    // process preceding nodes
    let in_deps = store.segments[node].incoming_segment_dependencies.clone();
    for dep in in_deps {
        let (dep_type, weight, source) = {
            let d = &store.dependencies[dep];
            (d.dependency_type, d.weight, d.source.unwrap())
        };
        if !critical_only || dep_type == DependencyType::Critical {
            if store.segments[source].mark < 0 && weight > 0 {
                // Remove weight (and possibly critical weight) from the source
                let s = &mut store.segments[source];
                s.out_dep_weight -= weight;
                if dep_type == DependencyType::Critical {
                    s.critical_out_dep_weight -= weight;
                }

                if s.out_dep_weight <= 0 && s.in_dep_weight > 0 {
                    sinks.push_back(source);
                }
            }
        }
    }
}
