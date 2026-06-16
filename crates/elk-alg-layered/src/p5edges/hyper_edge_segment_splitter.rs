//!
//! Responsible for splitting hyperedge segments in order to avoid overlaps,
//! given a set of critical dependencies whose removal will break critical
//! cycles. All terminology refers to the horizontal layout case.

use super::hyper_edge_segment::{DependencyId, SegmentId, SegmentStore};
use super::hyper_edge_segment_dependency as dependency;
use super::orthogonal_routing_generator::{count_crossings, OrthogonalRoutingGenerator};

/// A free area between two horizontal edge segments.
struct FreeArea {
    start_position: f64,
    end_position: f64,
    size: f64,
}

impl FreeArea {
    fn new(start_position: f64, end_position: f64) -> Self {
        debug_assert!(end_position >= start_position);
        FreeArea { start_position, end_position, size: end_position - start_position }
    }
}

/// What would happen if a segment was connected to its
/// split partner through an area.
struct AreaRating {
    dependencies: i32,
    crossings: i32,
}

/// Breaks critical dependency cycles by resolving the
/// given dependencies, splitting one of the involved segments per dependency.
/// New segments are added to `segments`.
pub fn split_segments(
    generator: &OrthogonalRoutingGenerator,
    store: &mut SegmentStore,
    dependencies_to_resolve: &[DependencyId],
    segments: &mut Vec<SegmentId>,
    critical_conflict_threshold: f64,
) {
    // Only start preparations if there's going to be things to do
    if dependencies_to_resolve.is_empty() {
        return;
    }

    // Collect all relevant spaces between horizontal segments
    let mut free_areas = find_free_areas(store, segments, critical_conflict_threshold);

    // For each dependency, choose which segment to split
    let segments_to_split = decide_which_segments_to_split(store, dependencies_to_resolve);

    // Split the segments in order from smallest to largest (a stable sort over
    // the insertion-ordered set, comparing the lengths)
    let mut sorted_segments_to_split = segments_to_split;
    sorted_segments_to_split
        .sort_by(|&s1, &s2| store.segments[s1].length().total_cmp(&store.segments[s2].length()));

    for segment in sorted_segments_to_split {
        split(generator, store, segment, segments, &mut free_areas, critical_conflict_threshold);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Finding Space

fn find_free_areas(
    store: &SegmentStore,
    segments: &[SegmentId],
    critical_conflict_threshold: f64,
) -> Vec<FreeArea> {
    let mut free_areas: Vec<FreeArea> = Vec::new();

    // Retrieve all positions where hyperedge segments connect to ports, and sort them
    let mut sorted_coordinates: Vec<f64> = Vec::new();
    for &s in segments {
        sorted_coordinates.extend(store.segments[s].incoming_connection_coordinates.iter());
    }
    for &s in segments {
        sorted_coordinates.extend(store.segments[s].outgoing_connection_coordinates.iter());
    }
    sorted_coordinates.sort_by(|a, b| a.total_cmp(b));

    // Go through each pair of coordinates and create free areas for those
    // that are at least twice the critical threshold
    for i in 1..sorted_coordinates.len() {
        if sorted_coordinates[i] - sorted_coordinates[i - 1] >= 2.0 * critical_conflict_threshold {
            free_areas.push(FreeArea::new(
                sorted_coordinates[i - 1] + critical_conflict_threshold,
                sorted_coordinates[i] - critical_conflict_threshold,
            ));
        }
    }

    free_areas
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Split Segment Decisions

/// Returns the insertion-ordered set as a Vec.
fn decide_which_segments_to_split(
    store: &mut SegmentStore,
    dependencies: &[DependencyId],
) -> Vec<SegmentId> {
    let mut segments_to_split: Vec<SegmentId> = Vec::new();

    for &dep in dependencies {
        let source_segment = store.dependencies[dep].source.unwrap();
        let target_segment = store.dependencies[dep].target.unwrap();

        // If either of the involved segments were already selected for
        // splitting because of another dependency, that's sufficient
        if segments_to_split.contains(&source_segment)
            || segments_to_split.contains(&target_segment)
        {
            continue;
        }

        // One segment will be split, and the other one will be remembered to
        // have caused the split
        let mut segment_to_split = source_segment;
        let mut segment_causing_split = target_segment;

        // We prefer splitting regular edges since hyperedges have a higher
        // chance of causing additional crossings
        if store.segments[source_segment].represents_hyperedge()
            && !store.segments[target_segment].represents_hyperedge()
        {
            segment_to_split = target_segment;
            segment_causing_split = source_segment;
        }

        segments_to_split.push(segment_to_split);
        store.segments[segment_to_split].split_by = Some(segment_causing_split);
    }

    segments_to_split
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Actual Splitting

/// Splits the given segment at the optimal position.
fn split(
    generator: &OrthogonalRoutingGenerator,
    store: &mut SegmentStore,
    segment: SegmentId,
    segments: &mut Vec<SegmentId>,
    free_areas: &mut Vec<FreeArea>,
    critical_conflict_threshold: f64,
) {
    // Split the segment at the best position and add the new segment to our list
    let split_position = compute_position_to_split_and_update_free_areas(
        store,
        segment,
        free_areas,
        critical_conflict_threshold,
    );
    let new_segment = store.split_at(segment, split_position);
    segments.push(new_segment);

    // Update the dependencies to reflect the new situation
    update_dependencies(generator, store, segment, segments);
}

fn update_dependencies(
    generator: &OrthogonalRoutingGenerator,
    store: &mut SegmentStore,
    segment: SegmentId,
    segments: &[SegmentId],
) {
    let split_causing_segment = store.segments[segment].split_by.unwrap();
    let split_partner = store.segments[segment].split_partner.unwrap();

    // The segments need to be ordered like this:
    //    segment ---> split-causing segment ---> split partner
    dependency::create_and_add_critical(store, segment, split_causing_segment);
    dependency::create_and_add_critical(store, split_causing_segment, split_partner);

    // Now we just need to re-introduce dependencies to other segments
    for i in 0..segments.len() {
        let other_segment = segments[i];
        if other_segment != split_causing_segment
            && other_segment != segment
            && other_segment != split_partner
        {
            generator.create_dependency_if_necessary(store, other_segment, segment);
            generator.create_dependency_if_necessary(store, other_segment, split_partner);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Split Position Computation

fn compute_position_to_split_and_update_free_areas(
    store: &mut SegmentStore,
    segment: SegmentId,
    free_areas: &mut Vec<FreeArea>,
    critical_conflict_threshold: f64,
) -> f64 {
    // Find the index of the first and the last area in our segment's reach
    let mut first_possible_area_index: i32 = -1;
    let mut last_possible_area_index: i32 = -1;

    for i in 0..free_areas.len() {
        let curr_area = &free_areas[i];

        if curr_area.start_position > store.segments[segment].end_coordinate() {
            // We're past the possible areas, so stop
            break;
        } else if curr_area.end_position >= store.segments[segment].start_coordinate() {
            // We've found a possible area; it might be the first
            if first_possible_area_index < 0 {
                first_possible_area_index = i as i32;
            }

            last_possible_area_index = i as i32;
        }
    }

    // Determine the position to split the segment at
    let mut split_position =
        center(store.segments[segment].start_coordinate(), store.segments[segment].end_coordinate());

    if first_possible_area_index >= 0 {
        // There are areas we can use
        let best_area_index = choose_best_area_index(
            store,
            segment,
            free_areas,
            first_possible_area_index as usize,
            last_possible_area_index as usize,
        );

        // We'll use the best area's centre and update the area list
        split_position =
            center(free_areas[best_area_index].start_position, free_areas[best_area_index].end_position);
        use_area(free_areas, best_area_index, critical_conflict_threshold);
    }

    split_position
}

fn choose_best_area_index(
    store: &mut SegmentStore,
    segment: SegmentId,
    free_areas: &[FreeArea],
    from_index: usize,
    to_index: usize,
) -> usize {
    let mut best_area_index = from_index;

    if from_index < to_index {
        // We have more areas to choose from, so rate them and find the best
        // one; we need to simulate splitting the segment
        let (split_segment, split_partner) = store.simulate_split(segment);

        let mut best_rating =
            rate_area(store, segment, split_segment, split_partner, &free_areas[best_area_index]);

        for i in from_index + 1..=to_index {
            // Determine how good our current area is
            let curr_rating =
                rate_area(store, segment, split_segment, split_partner, &free_areas[i]);

            if is_better(&free_areas[i], &curr_rating, &free_areas[best_area_index], &best_rating) {
                best_rating = curr_rating;
                best_area_index = i;
            }
        }
    }

    best_area_index
}

/// Rates what would happen if the given split segments were
/// connected through the given area.
fn rate_area(
    store: &mut SegmentStore,
    segment: SegmentId,
    split_segment: SegmentId,
    split_partner: SegmentId,
    area: &FreeArea,
) -> AreaRating {
    // The area's centre would be used to link the two split segments
    let area_centre = center(area.start_position, area.end_position);

    store.segments[split_segment].outgoing_connection_coordinates.clear();
    store.segments[split_segment].outgoing_connection_coordinates.push(area_centre);

    store.segments[split_partner].incoming_connection_coordinates.clear();
    store.segments[split_partner].incoming_connection_coordinates.push(area_centre);

    // Count the dependencies and crossings that the split partners would
    // cause with the original segment's incident dependencies
    let mut rating = AreaRating { dependencies: 0, crossings: 0 };

    let incoming_deps = store.segments[segment].incoming_segment_dependencies.clone();
    for dep in incoming_deps {
        let other_segment = store.dependencies[dep].source.unwrap();

        update_considering_both_orderings(store, &mut rating, split_segment, other_segment);
        update_considering_both_orderings(store, &mut rating, split_partner, other_segment);
    }

    let outgoing_deps = store.segments[segment].outgoing_segment_dependencies.clone();
    for dep in outgoing_deps {
        let other_segment = store.dependencies[dep].target.unwrap();

        update_considering_both_orderings(store, &mut rating, split_segment, other_segment);
        update_considering_both_orderings(store, &mut rating, split_partner, other_segment);
    }

    // There will be two additional dependencies:
    // splitSegment --> splitBySegment --> splitPartner
    rating.dependencies += 2;

    let split_by = store.segments[segment].split_by.unwrap();
    rating.crossings += count_crossings_for_single_ordering(store, split_segment, split_by);
    rating.crossings += count_crossings_for_single_ordering(store, split_by, split_partner);

    rating
}

fn update_considering_both_orderings(
    store: &SegmentStore,
    rating: &mut AreaRating,
    s1: SegmentId,
    s2: SegmentId,
) {
    let crossings_s1_left_of_s2 = count_crossings_for_single_ordering(store, s1, s2);
    let crossings_s2_left_of_s1 = count_crossings_for_single_ordering(store, s2, s1);

    if crossings_s1_left_of_s2 == crossings_s2_left_of_s1 {
        // If the crossings are the same, we're only interested if there are more than 0
        if crossings_s1_left_of_s2 > 0 {
            // Both orderings generate the same number of crossings, so we have a two-cycle
            rating.dependencies += 2;
            rating.crossings += crossings_s1_left_of_s2;
        }
    } else {
        // One order is better than the other, so there will be a single dependency
        rating.dependencies += 1;
        rating.crossings += crossings_s1_left_of_s2.min(crossings_s2_left_of_s1);
    }
}

fn count_crossings_for_single_ordering(store: &SegmentStore, left: SegmentId, right: SegmentId) -> i32 {
    count_crossings(
        &store.segments[left].outgoing_connection_coordinates,
        store.segments[right].start_coordinate(),
        store.segments[right].end_coordinate(),
    ) + count_crossings(
        &store.segments[right].incoming_connection_coordinates,
        store.segments[left].start_coordinate(),
        store.segments[left].end_coordinate(),
    )
}

fn is_better(
    curr_area: &FreeArea,
    curr_rating: &AreaRating,
    best_area: &FreeArea,
    best_rating: &AreaRating,
) -> bool {
    if curr_rating.crossings < best_rating.crossings {
        // First criterion: number of crossings
        return true;
    } else if curr_rating.crossings == best_rating.crossings {
        if curr_rating.dependencies < best_rating.dependencies {
            // Second criterion: number of dependencies
            return true;
        } else if curr_rating.dependencies == best_rating.dependencies {
            if curr_area.size > best_area.size {
                // Third criterion: size
                return true;
            }
        }
    }

    false
}

/// When an area is used, it falls into two parts which may be
/// usable themselves.
fn use_area(free_areas: &mut Vec<FreeArea>, used_area_index: usize, critical_conflict_threshold: f64) {
    let old_area = free_areas.remove(used_area_index);

    if old_area.size / 2.0 >= critical_conflict_threshold {
        // We will probably insert new areas. Keep track of where to insert them
        let mut insert_index = used_area_index;

        let old_area_centre = center(old_area.start_position, old_area.end_position);

        // Create the two new areas (and be doubly sure that double precision does not bite us)
        let new_end_1 = old_area_centre - critical_conflict_threshold;
        if old_area.start_position <= old_area_centre - critical_conflict_threshold {
            free_areas.insert(insert_index, FreeArea::new(old_area.start_position, new_end_1));
            insert_index += 1;
        }

        let new_start_2 = old_area_centre + critical_conflict_threshold;
        if new_start_2 <= old_area.end_position {
            free_areas.insert(insert_index, FreeArea::new(new_start_2, old_area.end_position));
        }
    }
}

fn center(p1: f64, p2: f64) -> f64 {
    (p1 + p2) / 2.0
}
