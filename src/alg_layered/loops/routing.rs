
use std::collections::VecDeque;

use crate::core::javacompat::JavaRandom;
use crate::core::options::PortSide;
use crate::graph::math::{KVector, KVectorChain, Spacing};

use crate::alg_layered::graph::{LGraphArena, LPortId};
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::SelfLoopOrderingStrategy;
use crate::alg_layered::p5edges::hyper_edge_segment::{SegmentId, SegmentStore};
use crate::alg_layered::p5edges::hyper_edge_segment_dependency as dependency;
use crate::alg_layered::p5edges::orthogonal_routing_generator;

use super::ordering::sorted_two_side_loop_port_sides;
use super::{
    get_individual_or_inherited, Alignment, SelfLoopHolder, SelfLoopType, SlEdgeIdx, SlLoopIdx,
    SlPortIdx, PORT_SIDE_COUNT,
};

// ---------------------------------------------------------------------------
// RoutingDirector

/// The penalty an edge incurs for passing a port without any connections.
const UNCONNECTED_PORT_PENALTY: i32 = 1;
/// The penalty an edge incurs for passing a port with connections.
const CONNECTED_PORT_PENALTY: i32 = 3;

/// The `LPort` ID of a self loop port (the `COMPARE_BY_ID` comparator key).
fn l_port_id(a: &LGraphArena, sl_holder: &SelfLoopHolder, sl_port: SlPortIdx) -> i32 {
    a.port(sl_holder.sl_ports[sl_port].l_port).id
}

/// Sets the leftmost and
/// rightmost ports of each hyper loop in the given holder.
pub fn determine_loop_routes(a: &mut LGraphArena, sl_holder: &mut SelfLoopHolder) {
    // Start by giving IDs to all ports according to their order in the port list
    let l_ports = a.node(sl_holder.l_node).ports.clone();
    assign_port_ids(a, &l_ports);
    sort_hyper_loop_port_lists(a, sl_holder);

    // Penalty array, computed on demand
    let mut port_penalties: Option<Vec<i32>> = None;

    // Now assign stuff! Preferrably, assign leftmost and rightmost ports...
    for sl_loop in 0..sl_holder.sl_hyper_loops.len() {
        match sl_holder.sl_hyper_loops[sl_loop].self_loop_type.unwrap() {
            SelfLoopType::OneSide => determine_one_side_loop_routes(a, sl_holder, sl_loop),
            SelfLoopType::TwoSidesCorner => {
                determine_two_side_corner_loop_routes(a, sl_holder, sl_loop)
            }
            SelfLoopType::TwoSidesOpposing => {
                determine_two_side_opposing_loop_routes(a, sl_holder, sl_loop, &mut port_penalties)
            }
            SelfLoopType::ThreeSides => determine_three_side_loop_routes(a, sl_holder, sl_loop),
            SelfLoopType::FourSides => {
                determine_four_side_loop_routes(a, sl_holder, sl_loop, &mut port_penalties)
            }
        }

        // Setup the loop's set of occupied port sides
        compute_occupied_port_sides(a, sl_holder, sl_loop);
    }
}

fn assign_port_ids(a: &mut LGraphArena, l_ports: &[LPortId]) {
    for (i, &l_port) in l_ports.iter().enumerate() {
        a.port_mut(l_port).id = i as i32;
    }
}

fn sort_hyper_loop_port_lists(a: &LGraphArena, sl_holder: &mut SelfLoopHolder) {
    let SelfLoopHolder { ref mut sl_hyper_loops, ref sl_ports, .. } = *sl_holder;
    for sl_loop in sl_hyper_loops {
        sl_loop.sl_ports.sort_by_key(|&p| a.port(sl_ports[p].l_port).id);
    }
}

fn compute_occupied_port_sides(a: &LGraphArena, sl_holder: &mut SelfLoopHolder, sl_loop: SlLoopIdx) {
    let slh_loop = &sl_holder.sl_hyper_loops[sl_loop];
    let mut curr_port_side =
        a.port(sl_holder.sl_ports[slh_loop.leftmost_port.unwrap()].l_port).side;
    let target_side = a.port(sl_holder.sl_ports[slh_loop.rightmost_port.unwrap()].l_port).side;

    let occupied = &mut sl_holder.sl_hyper_loops[sl_loop].occupied_port_sides;
    while curr_port_side != target_side {
        occupied.add(curr_port_side);
        curr_port_side = curr_port_side.right();
    }
    occupied.add(curr_port_side);
}

/// One-sided self loops should always be routed within their one side.
fn determine_one_side_loop_routes(a: &LGraphArena, sl_holder: &mut SelfLoopHolder, sl_loop: SlLoopIdx) {
    let side =
        a.port(sl_holder.sl_ports[sl_holder.sl_hyper_loops[sl_loop].sl_ports[0]].l_port).side;
    assign_leftmost_rightmost_ports(a, sl_holder, sl_loop, side, side);
}

/// Two-sided corner self loops only want to span that corner.
fn determine_two_side_corner_loop_routes(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
) {
    let sides = sorted_two_side_loop_port_sides(&sl_holder.sl_hyper_loops[sl_loop]);
    assign_leftmost_rightmost_ports(a, sl_holder, sl_loop, sides[0], sides[1]);
}

/// Opposing side loops have two ways to be routed; we choose the one with the
/// lower penalty.
fn determine_two_side_opposing_loop_routes(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
    port_penalties: &mut Option<Vec<i32>>,
) {
    // We use first-insertion order. This only matters
    // when both options have equal penalties.
    let sides: Vec<PortSide> = sl_holder.sl_hyper_loops[sl_loop].sl_port_sides.clone();
    debug_assert!(sides.len() == 2);

    // We basically play through both possible options and use the one which
    // has a lower penalty
    let option1_leftmost_port = lowest_port_on_side(a, sl_holder, sl_loop, sides[0]);
    let option1_rightmost_port = highest_port_on_side(a, sl_holder, sl_loop, sides[1]);
    let option1_penalty =
        compute_edge_penalty(a, sl_holder, port_penalties, option1_leftmost_port, option1_rightmost_port);

    let option2_leftmost_port = lowest_port_on_side(a, sl_holder, sl_loop, sides[1]);
    let option2_rightmost_port = highest_port_on_side(a, sl_holder, sl_loop, sides[0]);
    let option2_penalty =
        compute_edge_penalty(a, sl_holder, port_penalties, option2_leftmost_port, option2_rightmost_port);

    let slh_loop = &mut sl_holder.sl_hyper_loops[sl_loop];
    if option1_penalty <= option2_penalty {
        slh_loop.leftmost_port = Some(option1_leftmost_port);
        slh_loop.rightmost_port = Some(option1_rightmost_port);
    } else {
        slh_loop.leftmost_port = Some(option2_leftmost_port);
        slh_loop.rightmost_port = Some(option2_rightmost_port);
    }
}

/// Three-sided self loops only have one possible way of being routed.
fn determine_three_side_loop_routes(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
) {
    // Determine the leftmost and rightmost of the three port sides (we cannot
    // simply sort the port sides since loops can span the WEST-NORTH corner)
    let (leftmost_side, rightmost_side) =
        match compute_missing_port_side(&sl_holder.sl_hyper_loops[sl_loop]) {
            PortSide::NORTH => (PortSide::EAST, PortSide::WEST),
            PortSide::EAST => (PortSide::SOUTH, PortSide::NORTH),
            PortSide::SOUTH => (PortSide::WEST, PortSide::EAST),
            PortSide::WEST => (PortSide::NORTH, PortSide::SOUTH),
            PortSide::UNDEFINED => unreachable!(),
        };

    assign_leftmost_rightmost_ports(a, sl_holder, sl_loop, leftmost_side, rightmost_side);
}

fn compute_missing_port_side(slh_loop: &super::SelfHyperLoop) -> PortSide {
    debug_assert!(slh_loop.sl_port_sides.len() == 3);

    for side in [PortSide::NORTH, PortSide::EAST, PortSide::SOUTH, PortSide::WEST] {
        if !slh_loop.sl_port_sides.contains(&side) {
            return side;
        }
    }
    unreachable!()
}

/// For four-sided self loops, find the pair of adjacent ports with the
/// maximum penalty between them; that's where we split the loop.
fn determine_four_side_loop_routes(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
    port_penalties: &mut Option<Vec<i32>>,
) {
    // The self loop ports are sorted by ID
    let sorted_sl_ports = sl_holder.sl_hyper_loops[sl_loop].sl_ports.clone();

    // Go through each pair of adjacent ports and find the one which incurs
    // the highest penalty if we drew an edge between them. We start with the
    // uppermost port on the western side and the leftmost on the northern
    // side and then compare successive pairs against those two
    let mut worst_left_port = sorted_sl_ports[sorted_sl_ports.len() - 1];
    let mut worst_right_port = sorted_sl_ports[0];
    let mut worst_penalty =
        compute_edge_penalty(a, sl_holder, port_penalties, worst_left_port, worst_right_port);

    for right_port_index in 1..sorted_sl_ports.len() {
        let curr_left_port = sorted_sl_ports[right_port_index - 1];
        let curr_right_port = sorted_sl_ports[right_port_index];
        let curr_penalty =
            compute_edge_penalty(a, sl_holder, port_penalties, curr_left_port, curr_right_port);

        if curr_penalty > worst_penalty {
            worst_left_port = curr_left_port;
            worst_right_port = curr_right_port;
            worst_penalty = curr_penalty;
        }
    }

    // Since we _don't_ want to draw the self loop between the left and right
    // ports, we switch them here
    let slh_loop = &mut sl_holder.sl_hyper_loops[sl_loop];
    slh_loop.leftmost_port = Some(worst_right_port);
    slh_loop.rightmost_port = Some(worst_left_port);
}

/// Assigns the loop's leftmost and rightmost ports from the given sides.
fn assign_leftmost_rightmost_ports(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
    leftmost_side: PortSide,
    rightmost_side: PortSide,
) {
    let leftmost = lowest_port_on_side(a, sl_holder, sl_loop, leftmost_side);
    let rightmost = highest_port_on_side(a, sl_holder, sl_loop, rightmost_side);
    let slh_loop = &mut sl_holder.sl_hyper_loops[sl_loop];
    slh_loop.leftmost_port = Some(leftmost);
    slh_loop.rightmost_port = Some(rightmost);
}

/// Returns the port with the lowest ID on the given side.
fn lowest_port_on_side(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    sl_loop: SlLoopIdx,
    side: PortSide,
) -> SlPortIdx {
    sl_holder.sl_hyper_loops[sl_loop]
        .sl_ports_on_side(side)
        .iter()
        .copied()
        .min_by_key(|&p| l_port_id(a, sl_holder, p))
        .unwrap()
}

/// Returns the port with the highest ID on the given side.
fn highest_port_on_side(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    sl_loop: SlLoopIdx,
    side: PortSide,
) -> SlPortIdx {
    sl_holder.sl_hyper_loops[sl_loop]
        .sl_ports_on_side(side)
        .iter()
        .copied()
        .max_by_key(|&p| l_port_id(a, sl_holder, p))
        .unwrap()
}

/// `computeEdgePenalty`: the penalty incurred by an edge running from
/// the leftmost port clockwise to the rightmost port.
fn compute_edge_penalty(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    port_penalties: &mut Option<Vec<i32>>,
    leftmost_port: SlPortIdx,
    rightmost_port: SlPortIdx,
) -> i32 {
    // Compute penalties on demand
    if port_penalties.is_none() {
        *port_penalties = Some(compute_penalties(a, sl_holder));
    }
    let penalties = port_penalties.as_ref().unwrap();

    let port_count = a.node(sl_holder.l_node).ports.len() as i32;

    let leftmost_port_id = l_port_id(a, sl_holder, leftmost_port);
    let rightmost_port_id = l_port_id(a, sl_holder, rightmost_port);
    let mut left_of_rightmost_port_id = rightmost_port_id - 1;

    // If rightmostPortId == 0 we need to adjust leftOfRightmostPortId to be a
    // valid index
    if left_of_rightmost_port_id < 0 {
        left_of_rightmost_port_id = port_count - 1;
    }

    if leftmost_port_id <= left_of_rightmost_port_id {
        // This can be computed directly
        penalties[left_of_rightmost_port_id as usize] - penalties[leftmost_port_id as usize]
    } else {
        // Our edge from the leftmost to the rightmost port would pass the top
        // left corner, where indices reset to zero
        penalties[port_count as usize - 1] - penalties[leftmost_port_id as usize]
            + penalties[left_of_rightmost_port_id as usize]
    }
}

/// `computePenalties`: accumulated port penalties.
fn compute_penalties(a: &LGraphArena, sl_holder: &SelfLoopHolder) -> Vec<i32> {
    let ports = &a.node(sl_holder.l_node).ports;
    let mut port_penalties = Vec::with_capacity(ports.len());
    let mut penalty_sum = 0;

    for &curr_port in ports {
        if a.port(curr_port).incoming_edges.is_empty() && a.port(curr_port).outgoing_edges.is_empty()
        {
            penalty_sum += UNCONNECTED_PORT_PENALTY;
        } else {
            penalty_sum += CONNECTED_PORT_PENALTY;
        }

        port_penalties.push(penalty_sum);
    }

    port_penalties
}

// ---------------------------------------------------------------------------
// LabelPlacer

/// Label management is not ported; it only
/// runs when a label manager is configured on the graph.
pub fn place_labels(a: &mut LGraphArena, sl_holder: &mut SelfLoopHolder) {
    assign_side_and_alignment(a, sl_holder);

    for sl_loop in 0..sl_holder.sl_hyper_loops.len() {
        if sl_holder.sl_hyper_loops[sl_loop].sl_labels.is_some() {
            compute_coordinates(a, sl_holder, sl_loop);
        }
    }
}

fn assign_side_and_alignment(a: &mut LGraphArena, sl_holder: &mut SelfLoopHolder) {
    // For sequenced one-sided northern / southern loops, we collect the loops
    // first and process them afterwards
    let ordering_strategy: SelfLoopOrderingStrategy = a
        .node(sl_holder.l_node)
        .properties
        .get(&lopts::EDGE_ROUTING_SELF_LOOP_ORDERING);
    let sequenced = ordering_strategy == SelfLoopOrderingStrategy::SEQUENCED;

    let mut northern_one_sided_sl_loops: Vec<SlLoopIdx> = Vec::new();
    let mut southern_one_sided_sl_loops: Vec<SlLoopIdx> = Vec::new();

    // Assign sides and alignments; how this works differs for the different
    // kinds of labels
    for sl_loop in 0..sl_holder.sl_hyper_loops.len() {
        // If this loop doesn't have any labels, don't bother
        if sl_holder.sl_hyper_loops[sl_loop].sl_labels.is_none() {
            continue;
        }

        // How we place labels largely depends on the self loop type
        match sl_holder.sl_hyper_loops[sl_loop].self_loop_type.unwrap() {
            SelfLoopType::OneSide => {
                let loop_side = sl_holder.sl_hyper_loops[sl_loop]
                    .occupied_port_sides
                    .iter()
                    .next()
                    .unwrap();

                if sequenced && loop_side == PortSide::NORTH {
                    // Collect for deferred processing
                    northern_one_sided_sl_loops.push(sl_loop);
                } else if sequenced && loop_side == PortSide::SOUTH {
                    // Collect for deferred processing
                    southern_one_sided_sl_loops.push(sl_loop);
                } else {
                    assign_one_sided_simple_side_and_alignment(a, sl_holder, sl_loop, loop_side);
                }
            }

            SelfLoopType::TwoSidesCorner => {
                assign_two_sides_corner_side_and_alignment(a, sl_holder, sl_loop);
            }

            SelfLoopType::TwoSidesOpposing | SelfLoopType::ThreeSides => {
                assign_two_sides_opposing_and_three_sides_side_and_alignment(a, sl_holder, sl_loop);
            }

            SelfLoopType::FourSides => {
                assign_four_sides_side_and_alignment(a, sl_holder, sl_loop);
            }
        }
    }

    // Process deferred loops
    if sequenced {
        if !northern_one_sided_sl_loops.is_empty() {
            assign_one_sided_sequenced_side_and_alignment(
                a,
                sl_holder,
                northern_one_sided_sl_loops,
                PortSide::NORTH,
            );
        }

        if !southern_one_sided_sl_loops.is_empty() {
            assign_one_sided_sequenced_side_and_alignment(
                a,
                sl_holder,
                southern_one_sided_sl_loops,
                PortSide::SOUTH,
            );
        }
    }
}

/// Removes the inline edge label property from the loop's labels.
fn remove_inline_property(a: &LGraphArena, sl_holder: &SelfLoopHolder, sl_loop: SlLoopIdx) {
    for &label in &sl_holder.sl_hyper_loops[sl_loop].sl_labels.as_ref().unwrap().l_labels {
        a.label(label).properties.unset(&lopts::EDGE_LABELS_INLINE);
    }
}

/// `assignOneSidedSimpleSideAndAlignment`.
fn assign_one_sided_simple_side_and_alignment(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
    loop_side: PortSide,
) {
    // Remove inline edge label property since it is not valid in this case.
    remove_inline_property(a, sl_holder, sl_loop);

    match loop_side {
        PortSide::EAST | PortSide::WEST => {
            // The alignment will be relative to the topmost port (which must
            // be either the leftmost or the rightmost port)
            let slh_loop = &sl_holder.sl_hyper_loops[sl_loop];
            let mut topmost_port = slh_loop.leftmost_port.unwrap();
            let rightmost_port = slh_loop.rightmost_port.unwrap();
            if a.port(sl_holder.sl_ports[rightmost_port].l_port).pos.y
                < a.port(sl_holder.sl_ports[topmost_port].l_port).pos.y
            {
                topmost_port = rightmost_port;
            }

            set_side_and_alignment(sl_holder, sl_loop, loop_side, Alignment::Top, Some(topmost_port));
        }

        PortSide::NORTH | PortSide::SOUTH => {
            set_side_and_alignment(sl_holder, sl_loop, loop_side, Alignment::Center, None);
        }

        PortSide::UNDEFINED => unreachable!(),
    }
}

/// `assignOneSidedSequencedSideAndAlignment`.
fn assign_one_sided_sequenced_side_and_alignment(
    a: &mut LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    mut sl_loops: Vec<SlLoopIdx>,
    port_side: PortSide,
) {
    debug_assert!(!sl_loops.is_empty());

    // Ensure sensible port IDs
    let l_ports = a.node(sl_holder.l_node).ports.clone();
    for (id, &l_port) in l_ports.iter().enumerate() {
        a.port_mut(l_port).id = id as i32;
    }

    // We start by sorting our list according to the ID of the leftmost port.
    // For northern loops, this ensures that the list is sorted from left to
    // right; for southern ones we sort descendingly
    let leftmost_id = |sl_loop: SlLoopIdx| {
        l_port_id(a, sl_holder, sl_holder.sl_hyper_loops[sl_loop].leftmost_port.unwrap())
    };
    if port_side == PortSide::NORTH {
        sl_loops.sort_by(|&l1, &l2| leftmost_id(l1).cmp(&leftmost_id(l2)));
    } else {
        sl_loops.sort_by(|&l1, &l2| leftmost_id(l2).cmp(&leftmost_id(l1)));
    }

    // Go from outside loops towards inside loops in pairs
    let mut left_sl_loop_idx = 0usize;
    let mut right_sl_loop_idx = sl_loops.len() - 1;

    while left_sl_loop_idx < right_sl_loop_idx {
        let left_sl_loop = sl_loops[left_sl_loop_idx];
        let right_sl_loop = sl_loops[right_sl_loop_idx];

        // If the loop is on the northern side, the leftmost port actually is
        // left of the rightmost port. It's flipped for the southern side
        let left_loop_alignment_reference = if port_side == PortSide::NORTH {
            sl_holder.sl_hyper_loops[left_sl_loop].rightmost_port
        } else {
            sl_holder.sl_hyper_loops[left_sl_loop].leftmost_port
        };
        let right_loop_alignment_reference = if port_side == PortSide::NORTH {
            sl_holder.sl_hyper_loops[right_sl_loop].leftmost_port
        } else {
            sl_holder.sl_hyper_loops[right_sl_loop].rightmost_port
        };

        set_side_and_alignment(
            sl_holder,
            left_sl_loop,
            port_side,
            Alignment::Right,
            left_loop_alignment_reference,
        );
        set_side_and_alignment(
            sl_holder,
            right_sl_loop,
            port_side,
            Alignment::Left,
            right_loop_alignment_reference,
        );

        left_sl_loop_idx += 1;
        right_sl_loop_idx -= 1;
    }

    // There might be a single loop in the middle
    if left_sl_loop_idx == right_sl_loop_idx {
        set_side_and_alignment(sl_holder, sl_loops[left_sl_loop_idx], port_side, Alignment::Center, None);
    }
}

/// `assignTwoSidesCornerSideAndAlignment`.
fn assign_two_sides_corner_side_and_alignment(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
) {
    let slh_loop = &sl_holder.sl_hyper_loops[sl_loop];
    let leftmost_port = slh_loop.leftmost_port.unwrap();
    let rightmost_port = slh_loop.rightmost_port.unwrap();
    let leftmost_port_side = a.port(sl_holder.sl_ports[leftmost_port].l_port).side;
    let rightmost_port_side = a.port(sl_holder.sl_ports[rightmost_port].l_port).side;

    // Remove inline edge label property since it is not valid in this case.
    remove_inline_property(a, sl_holder, sl_loop);

    if leftmost_port_side == PortSide::NORTH {
        set_side_and_alignment(sl_holder, sl_loop, PortSide::NORTH, Alignment::Left, Some(leftmost_port));
    } else if rightmost_port_side == PortSide::NORTH {
        set_side_and_alignment(sl_holder, sl_loop, PortSide::NORTH, Alignment::Right, Some(rightmost_port));
    } else if leftmost_port_side == PortSide::SOUTH {
        set_side_and_alignment(sl_holder, sl_loop, PortSide::SOUTH, Alignment::Right, Some(leftmost_port));
    } else if rightmost_port_side == PortSide::SOUTH {
        set_side_and_alignment(sl_holder, sl_loop, PortSide::SOUTH, Alignment::Left, Some(rightmost_port));
    } else {
        debug_assert!(false);
    }
}

/// `assignTwoSidesOpposingAndThreeSidesSideAndAlignment`.
fn assign_two_sides_opposing_and_three_sides_side_and_alignment(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
) {
    let slh_loop = &sl_holder.sl_hyper_loops[sl_loop];
    let occupied_sides = slh_loop.occupied_port_sides;

    // Check whether the hyperloop has inline labels
    let has_inline_labels = slh_loop
        .sl_labels
        .as_ref()
        .unwrap()
        .l_labels
        .iter()
        .any(|&l| a.label(l).properties.get(&lopts::EDGE_LABELS_INLINE));

    if !occupied_sides.contains(PortSide::NORTH) {
        // This also works for inline edge labels since the label will be "in the middle".
        set_side_and_alignment(sl_holder, sl_loop, PortSide::SOUTH, Alignment::Center, None);
    } else if !occupied_sides.contains(PortSide::SOUTH) {
        // This also works for inline edge labels since the label will be "in the middle".
        set_side_and_alignment(sl_holder, sl_loop, PortSide::NORTH, Alignment::Center, None);
    } else if !occupied_sides.contains(PortSide::WEST) {
        // If we have inline labels, we can center them on the eastern side.
        // Otherwise, we left-align them on the northern side.
        if has_inline_labels {
            set_side_and_alignment(sl_holder, sl_loop, PortSide::EAST, Alignment::Center, None);
        } else {
            let leftmost_port = sl_holder.sl_hyper_loops[sl_loop].leftmost_port;
            set_side_and_alignment(sl_holder, sl_loop, PortSide::NORTH, Alignment::Left, leftmost_port);
        }
    } else if !occupied_sides.contains(PortSide::EAST) {
        // If we have inline labels, we can center them on the western side.
        // Otherwise, we right-align them on the northern side.
        if has_inline_labels {
            set_side_and_alignment(sl_holder, sl_loop, PortSide::WEST, Alignment::Center, None);
        } else {
            let rightmost_port = sl_holder.sl_hyper_loops[sl_loop].rightmost_port;
            set_side_and_alignment(sl_holder, sl_loop, PortSide::NORTH, Alignment::Right, rightmost_port);
        }
    } else {
        debug_assert!(false);
    }
}

/// `assignFourSidesSideAndAlignment`. Note: faithfully preserves the
/// quirk that `rightmostPortSide` is computed from the *leftmost* port.
fn assign_four_sides_side_and_alignment(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
) {
    let slh_loop = &sl_holder.sl_hyper_loops[sl_loop];
    let leftmost_port_side =
        a.port(sl_holder.sl_ports[slh_loop.leftmost_port.unwrap()].l_port).side;
    let rightmost_port_side =
        a.port(sl_holder.sl_ports[slh_loop.leftmost_port.unwrap()].l_port).side;

    // Remove inline edge label property since it is not valid in this case.
    remove_inline_property(a, sl_holder, sl_loop);

    if leftmost_port_side == PortSide::NORTH || rightmost_port_side == PortSide::NORTH {
        set_side_and_alignment(sl_holder, sl_loop, PortSide::SOUTH, Alignment::Center, None);
    } else {
        set_side_and_alignment(sl_holder, sl_loop, PortSide::NORTH, Alignment::Center, None);
    }
}

/// `assignSideAndAlignment` (the setter variant).
fn set_side_and_alignment(
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
    side: PortSide,
    alignment: Alignment,
    alignment_reference: Option<SlPortIdx>,
) {
    let sl_labels = sl_holder.sl_hyper_loops[sl_loop].sl_labels.as_mut().unwrap();
    sl_labels.side = side;
    sl_labels.alignment = Some(alignment);
    sl_labels.alignment_reference_sl_port = alignment_reference;
}

/// `computeCoordinates`.
fn compute_coordinates(a: &LGraphArena, sl_holder: &mut SelfLoopHolder, sl_loop: SlLoopIdx) {
    let node_size_x = a.node(sl_holder.l_node).size.x;

    let align_ref_l_port = sl_holder.sl_hyper_loops[sl_loop]
        .sl_labels
        .as_ref()
        .unwrap()
        .alignment_reference_sl_port
        .map(|p| sl_holder.sl_ports[p].l_port);

    let sl_labels = sl_holder.sl_hyper_loops[sl_loop].sl_labels.as_mut().unwrap();
    let size = sl_labels.size;

    match sl_labels.alignment.unwrap() {
        Alignment::Center => {
            sl_labels.position.x = (node_size_x - size.x) / 2.0;
        }

        Alignment::Left => {
            let p = a.port(align_ref_l_port.unwrap());
            sl_labels.position.x = p.pos.x + p.anchor.x;
        }

        Alignment::Right => {
            let p = a.port(align_ref_l_port.unwrap());
            sl_labels.position.x = p.pos.x + p.anchor.x - size.x;
        }

        Alignment::Top => {
            let p = a.port(align_ref_l_port.unwrap());
            sl_labels.position.y = p.pos.y + p.anchor.y;
        }
    }
}

// ---------------------------------------------------------------------------
// RoutingSlotAssigner

/// Assigns routing slots to
/// all self loops per port side they span.
pub fn assign_routing_slots(
    a: &mut LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    random: &mut JavaRandom,
) {
    // To be able to check whether labels potentially overlap, we build an
    // overlap matrix
    let label_crossing_matrix = compute_label_crossing_matrix(sl_holder);

    // We're using the orthogonal edge router's cycle breaker for this, so
    // create the crossing graph for our loops
    let loop_count = sl_holder.sl_hyper_loops.len();
    let mut store = SegmentStore::new();
    let segments: Vec<SegmentId> = (0..loop_count).map(|_| store.create_segment()).collect();

    // To be able to quickly count crossings later, we remember for each loop
    // whether it's active at a given port ID or not
    let sl_loop_activity_over_ports = compute_loop_activity(a, sl_holder);

    // For each pair of hyper loops, determine the crossings for one segment
    // routed above the other and vice versa
    if loop_count > 0 {
        for first_idx in 0..loop_count - 1 {
            for second_idx in first_idx + 1..loop_count {
                create_dependencies(
                    a,
                    sl_holder,
                    &mut store,
                    &segments,
                    &sl_loop_activity_over_ports,
                    first_idx,
                    second_idx,
                    &label_crossing_matrix,
                );
            }
        }
    }

    orthogonal_routing_generator::break_non_critical_cycles(&mut store, &segments, random);

    // Assign routing slots based on the graph
    do_assign_routing_slots(
        a,
        sl_holder,
        &mut store,
        &segments,
        &sl_loop_activity_over_ports,
        &label_crossing_matrix,
    );
}

/// `computeLabelCrossingMatrix`: `true` entries mean the labels with the
/// corresponding IDs overlap.
fn compute_label_crossing_matrix(sl_holder: &mut SelfLoopHolder) -> Vec<Vec<bool>> {
    // We need to start by giving the labels proper IDs
    let mut label_id = 0;
    for sl_loop in &mut sl_holder.sl_hyper_loops {
        if let Some(sl_labels) = &mut sl_loop.sl_labels {
            sl_labels.id = label_id;
            label_id += 1;
        }
    }

    let n = label_id as usize;
    let mut crossing_matrix = vec![vec![false; n]; n];

    // Now check for each pair of labels whether or not they overlap
    let sl_loops = &sl_holder.sl_hyper_loops;
    for sl1_idx in 0..sl_loops.len() {
        if let Some(sl_labels1) = &sl_loops[sl1_idx].sl_labels {
            for sl2_idx in sl1_idx + 1..sl_loops.len() {
                if let Some(sl_labels2) = &sl_loops[sl2_idx].sl_labels {
                    let overlap = labels_overlap(&sl_loops[sl1_idx], &sl_loops[sl2_idx]);

                    crossing_matrix[sl_labels1.id as usize][sl_labels2.id as usize] = overlap;
                    crossing_matrix[sl_labels2.id as usize][sl_labels1.id as usize] = overlap;
                }
            }
        }
    }

    crossing_matrix
}

/// `labelsOverlap(SelfHyperLoop, SelfHyperLoop)`.
fn labels_overlap(sl_loop1: &super::SelfHyperLoop, sl_loop2: &super::SelfHyperLoop) -> bool {
    // There won't be overlaps unless both loops have labels
    let (sl_labels1, sl_labels2) = match (&sl_loop1.sl_labels, &sl_loop2.sl_labels) {
        (Some(l1), Some(l2)) => (l1, l2),
        _ => return false,
    };

    // The labels must be assigned to the same side, and that side (currently)
    // needs to be either north or south
    if sl_labels1.side != sl_labels2.side
        || sl_labels1.side == PortSide::EAST
        || sl_labels1.side == PortSide::WEST
    {
        return false;
    }

    // Check if the labels overlap horizontally
    let start1 = sl_labels1.position.x;
    let end1 = start1 + sl_labels1.size.x;
    let start2 = sl_labels2.position.x;
    let end2 = start2 + sl_labels2.size.x;

    start1 <= end2 && end1 >= start2
}

/// `computeLoopActivity`: each loop is mapped to an array indexed by
/// port indices indicating whether the loop runs along the given port.
fn compute_loop_activity(a: &LGraphArena, sl_holder: &SelfLoopHolder) -> Vec<Vec<bool>> {
    let l_port_count = a.node(sl_holder.l_node).ports.len();
    let mut result = Vec::with_capacity(sl_holder.sl_hyper_loops.len());

    for sl_loop in &sl_holder.sl_hyper_loops {
        let mut loop_activity = vec![false; l_port_count];

        // Run from the loop's start port to the end port, possibly wrapping
        // around, and set everything to true along the way
        let mut l_port_idx =
            a.port(sl_holder.sl_ports[sl_loop.leftmost_port.unwrap()].l_port).id - 1;
        let l_port_target_idx =
            a.port(sl_holder.sl_ports[sl_loop.rightmost_port.unwrap()].l_port).id;

        while l_port_idx != l_port_target_idx {
            l_port_idx = (l_port_idx + 1) % l_port_count as i32;
            loop_activity[l_port_idx as usize] = true;
        }

        result.push(loop_activity);
    }

    result
}

/// `createDependencies`: creates the necessary dependencies between the
/// hyper loops with the given indices.
fn create_dependencies(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    store: &mut SegmentStore,
    segments: &[SegmentId],
    sl_loop_activity_over_ports: &[Vec<bool>],
    sl_loop1: SlLoopIdx,
    sl_loop2: SlLoopIdx,
    label_crossing_matrix: &[Vec<bool>],
) {
    // Count the numbers of crossings that would ensue if placing the first
    // segment above the second segment and vice versa
    let first_above_second_crossings =
        count_crossings(a, sl_holder, sl_loop_activity_over_ports, sl_loop1, sl_loop2);
    let second_above_first_crossings =
        count_crossings(a, sl_holder, sl_loop_activity_over_ports, sl_loop2, sl_loop1);

    // Create dependencies
    let segment1 = segments[sl_loop1];
    let segment2 = segments[sl_loop2];

    if first_above_second_crossings < second_above_first_crossings {
        // The first loop should be above the second loop
        dependency::create_and_add_regular(
            store,
            segment1,
            segment2,
            second_above_first_crossings - first_above_second_crossings,
        );
    } else if second_above_first_crossings < first_above_second_crossings {
        // The second loop should be above the first loop
        dependency::create_and_add_regular(
            store,
            segment2,
            segment1,
            first_above_second_crossings - second_above_first_crossings,
        );
    } else if first_above_second_crossings != 0
        || labels_overlap_by_matrix(sl_holder, sl_loop1, sl_loop2, label_crossing_matrix)
    {
        // Either both orders cause the same number of crossings (and at least
        // one), or the labels of the two loops overlap and the loops must
        // thus be forced onto different slots
        dependency::create_and_add_regular(store, segment1, segment2, 0);
        dependency::create_and_add_regular(store, segment2, segment1, 0);
    }
}

/// `countCrossings`.
fn count_crossings(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    sl_loop_activity_over_ports: &[Vec<bool>],
    sl_upper_loop: SlLoopIdx,
    sl_lower_loop: SlLoopIdx,
) -> i32 {
    let lower_loop_activity = &sl_loop_activity_over_ports[sl_lower_loop];
    let mut crossings = 0;

    for &sl_port in &sl_holder.sl_hyper_loops[sl_upper_loop].sl_ports {
        if lower_loop_activity[a.port(sl_holder.sl_ports[sl_port].l_port).id as usize] {
            crossings += 1;
        }
    }

    crossings
}

/// `doAssignRoutingSlots`.
fn do_assign_routing_slots(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    store: &mut SegmentStore,
    segments: &[SegmentId],
    sl_loop_activity_over_ports: &[Vec<bool>],
    label_crossing_matrix: &[Vec<bool>],
) {
    // We first compute raw slots
    assign_raw_routing_slots_to_segments(store, segments);

    // We assign raw routing slots, but try to compact the routing slot
    // assignment afterwards
    assign_raw_routing_slots_to_loops(sl_holder, store, segments);
    shift_towards_node(a, sl_holder, sl_loop_activity_over_ports, label_crossing_matrix);
}

/// `assignRawRoutingSlotsToSegments`.
fn assign_raw_routing_slots_to_segments(store: &mut SegmentStore, segments: &[SegmentId]) {
    let mut sinks: VecDeque<SegmentId> = VecDeque::new();

    // Fill our queue of sinks; while we go through these, we also reset the
    // in- and out weights to the number of incoming and outgoing dependencies
    for &segment in segments {
        let s = &mut store.segments[segment];
        s.in_dep_weight = s.incoming_segment_dependencies.len() as i32;
        s.out_dep_weight = s.outgoing_segment_dependencies.len() as i32;

        if s.out_dep_weight == 0 {
            s.routing_slot = 0;
            sinks.push_back(segment);
        }
    }

    // Assign raw routing slots!
    while let Some(segment) = sinks.pop_front() {
        let next_routing_slot = store.segments[segment].routing_slot + 1;

        for in_dependency in store.segments[segment].incoming_segment_dependencies.clone() {
            let source_segment = store.dependencies[in_dependency].source.unwrap();
            let s = &mut store.segments[source_segment];
            s.routing_slot = i32::max(s.routing_slot, next_routing_slot);

            s.out_dep_weight -= 1;
            if s.out_dep_weight == 0 {
                sinks.push_back(source_segment);
            }
        }
    }
}

/// `assignRawRoutingSlotsToLoops`.
fn assign_raw_routing_slots_to_loops(
    sl_holder: &mut SelfLoopHolder,
    store: &SegmentStore,
    segments: &[SegmentId],
) {
    for sl_loop in 0..sl_holder.sl_hyper_loops.len() {
        let slot = store.segments[segments[sl_loop]].routing_slot;
        let sides: Vec<PortSide> =
            sl_holder.sl_hyper_loops[sl_loop].occupied_port_sides.iter().collect();
        for port_side in sides {
            sl_holder.set_routing_slot(sl_loop, port_side, slot);
        }
    }
}

/// `shiftTowardsNode`: moves the self loops towards the node on each of
/// the node's sides to avoid empty routing slots.
fn shift_towards_node(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop_activity_over_ports: &[Vec<bool>],
    label_crossing_matrix: &[Vec<bool>],
) {
    // For each port, this array specifies the next routing slot we can assign
    // a self loop on that side to
    let mut next_free_routing_slot_at_port = vec![0i32; a.node(sl_holder.l_node).ports.len()];

    for side in [PortSide::NORTH, PortSide::EAST, PortSide::SOUTH, PortSide::WEST] {
        shift_towards_node_on_side(
            a,
            sl_holder,
            side,
            &mut next_free_routing_slot_at_port,
            sl_loop_activity_over_ports,
            label_crossing_matrix,
        );
    }
}

/// `shiftTowardsNodeOnSide`.
fn shift_towards_node_on_side(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    side: PortSide,
    next_free_routing_slot_at_port: &mut [i32],
    sl_loop_activity_over_ports: &[Vec<bool>],
    label_crossing_matrix: &[Vec<bool>],
) {
    // We will iterate over the self loops that occupy that port side, sorted
    // ascendingly by routing slot
    let mut sl_loops: Vec<SlLoopIdx> = (0..sl_holder.sl_hyper_loops.len())
        .filter(|&l| sl_holder.sl_hyper_loops[l].occupied_port_sides.contains(side))
        .collect();
    sl_loops.sort_by_key(|&l| sl_holder.sl_hyper_loops[l].routing_slot(side));

    // Find the indices of the first and last regular port on the port side
    let mut min_l_port_index = i32::MAX;
    let mut max_l_port_index = i32::MIN;
    for &l_port in &a.node(sl_holder.l_node).ports {
        if a.port(l_port).side == side {
            min_l_port_index = i32::min(min_l_port_index, a.port(l_port).id);
            max_l_port_index = i32::max(max_l_port_index, a.port(l_port).id);
        }
    }

    if min_l_port_index == i32::MAX {
        // There are no ports on this side, so we simply assign the loops to
        // consecutive slots starting with 0. We won't cause label overlaps.
        for (i, &sl_loop) in sl_loops.iter().enumerate() {
            sl_holder.set_routing_slot(sl_loop, side, i as i32);
        }
    } else {
        let mut slot_assigned_to_label = vec![-1i32; label_crossing_matrix.len()];

        // There are ports on this side. Find the lowest free slot across all
        // ports our loop spans, and ensure that no label our loop label
        // conflicts with is assigned to that slot
        for sl_loop in sl_loops {
            let active_at_port = &sl_loop_activity_over_ports[sl_loop];
            let mut lowest_available_slot = 0i32;

            for port_index in min_l_port_index..=max_l_port_index {
                if active_at_port[port_index as usize] {
                    lowest_available_slot = i32::max(
                        lowest_available_slot,
                        next_free_routing_slot_at_port[port_index as usize],
                    );
                }
            }

            // If we have a label, it could be that we are in conflict with
            // another label placed at the lowest available slot or higher
            if let Some(sl_labels) = &sl_holder.sl_hyper_loops[sl_loop].sl_labels {
                let our_label_idx = sl_labels.id as usize;
                let mut slots_with_label_conflicts: Vec<i32> = Vec::new();

                for other_label_idx in 0..label_crossing_matrix.len() {
                    if label_crossing_matrix[our_label_idx][other_label_idx] {
                        slots_with_label_conflicts.push(slot_assigned_to_label[other_label_idx]);
                    }
                }

                // Find the first slot (starting with our lowest available)
                // that does not appear in the set
                while slots_with_label_conflicts.contains(&lowest_available_slot) {
                    lowest_available_slot += 1;
                }
            }

            // Assign the loop to that routing slot and update our routing
            // slot array
            sl_holder.set_routing_slot(sl_loop, side, lowest_available_slot);
            for port_index in min_l_port_index..=max_l_port_index {
                if active_at_port[port_index as usize] {
                    next_free_routing_slot_at_port[port_index as usize] = lowest_available_slot + 1;
                }
            }

            // If we have a label, update the label's routing slot
            if let Some(sl_labels) = &sl_holder.sl_hyper_loops[sl_loop].sl_labels {
                slot_assigned_to_label[sl_labels.id as usize] = lowest_available_slot;
            }
        }
    }
}

/// `labelsOverlap(..., labelCrossingMatrix)`.
fn labels_overlap_by_matrix(
    sl_holder: &SelfLoopHolder,
    sl_loop1: SlLoopIdx,
    sl_loop2: SlLoopIdx,
    label_crossing_matrix: &[Vec<bool>],
) -> bool {
    match (
        &sl_holder.sl_hyper_loops[sl_loop1].sl_labels,
        &sl_holder.sl_hyper_loops[sl_loop2].sl_labels,
    ) {
        (Some(l1), Some(l2)) => label_crossing_matrix[l1.id as usize][l2.id as usize],
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// OrthogonalSelfLoopRouter

/// The direction in
/// which an edge goes around a node in its quest to reach its target.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeRoutingDirection {
    Clockwise,
    CounterClockwise,
}

/// Selects which of the self loop routers runs: `OrthogonalSelfLoopRouter`,
/// `PolylineSelfLoopRouter` or `SplineSelfLoopRouter` (the latter two only
/// override `modifyBendPoints`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SelfLoopRouterKind {
    Orthogonal,
    Polyline,
    Spline,
}

pub fn route_self_loops(a: &mut LGraphArena, sl_holder: &mut SelfLoopHolder, kind: SelfLoopRouterKind) {
    let l_node = sl_holder.l_node;

    let node_size = a.node(l_node).size;
    let node_margins = a.node(l_node).margin;

    let edge_edge_distance = get_individual_or_inherited(a, l_node, &lopts::SPACING_EDGE_EDGE);
    let edge_label_distance = get_individual_or_inherited(a, l_node, &lopts::SPACING_EDGE_LABEL);
    let node_sl_distance = get_individual_or_inherited(a, l_node, &lopts::SPACING_NODE_SELF_LOOP);

    let mut new_node_margins: Spacing = node_margins;

    // Compute how far away from the node each routing slot on each side is
    // (this takes labels into account)
    let routing_slot_positions = compute_routing_slot_positions(
        a,
        sl_holder,
        edge_edge_distance,
        edge_label_distance,
        node_sl_distance,
    );

    for sl_loop in 0..sl_holder.sl_hyper_loops.len() {
        for sl_edge in sl_holder.sl_hyper_loops[sl_loop].sl_edges.clone() {
            let l_edge = sl_holder.sl_edges[sl_edge].l_edge;

            let routing_direction = compute_edge_routing_direction(a, sl_holder, sl_edge);

            // Compute orthogonal bend points and give subclasses a chance to
            // modify them to suit their particular routing style
            let mut bend_points =
                compute_orthogonal_bend_points(a, sl_holder, sl_edge, routing_direction, &routing_slot_positions);
            bend_points = match kind {
                SelfLoopRouterKind::Orthogonal => bend_points,
                SelfLoopRouterKind::Polyline => {
                    polyline_modify_bend_points(a, sl_holder, sl_edge, bend_points)
                }
                SelfLoopRouterKind::Spline => {
                    spline_modify_bend_points(a, sl_holder, sl_edge, routing_direction, bend_points)
                }
            };

            for bp in &bend_points {
                update_new_node_margins(node_size, &mut new_node_margins, *bp);
            }

            a.edge_mut(l_edge).bend_points = KVectorChain(bend_points);
        }

        // Place the self loop's labels (the edges were routed such that there
        // is enough space available)
        if sl_holder.sl_hyper_loops[sl_loop].sl_labels.is_some() {
            place_loop_labels(a, sl_holder, sl_loop, &routing_slot_positions, edge_label_distance);

            let sl_labels = sl_holder.sl_hyper_loops[sl_loop].sl_labels.as_ref().unwrap();
            let mut pos = sl_labels.position;
            update_new_node_margins(node_size, &mut new_node_margins, pos);
            pos.add(sl_labels.size);
            update_new_node_margins(node_size, &mut new_node_margins, pos);
        }
    }

    // Update the node's margins to include the space required for self loops
    a.node_mut(l_node).margin = new_node_margins;
}

/// `computeEdgeRoutingDirection`: computes how the edge reaches its
/// target.
fn compute_edge_routing_direction(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    sl_edge: SlEdgeIdx,
) -> EdgeRoutingDirection {
    let edge = &sl_holder.sl_edges[sl_edge];
    let source_l_port = sl_holder.sl_ports[edge.sl_source].l_port;
    let source_port_side = a.port(source_l_port).side;

    let target_l_port = sl_holder.sl_ports[edge.sl_target].l_port;
    let target_port_side = a.port(target_l_port).side;

    if source_port_side == target_port_side {
        // If the port sides are equal, it doesn't really matter what we
        // return, but we'll make a best effort based on port IDs
        if a.port(source_l_port).id < a.port(target_l_port).id {
            EdgeRoutingDirection::Clockwise
        } else {
            EdgeRoutingDirection::CounterClockwise
        }
    } else if source_port_side.right() == target_port_side {
        EdgeRoutingDirection::Clockwise
    } else if source_port_side.left() == target_port_side {
        EdgeRoutingDirection::CounterClockwise
    } else {
        debug_assert!(source_port_side.opposed() == target_port_side);

        // What we do here totally depends on the port sides occupied by the
        // self hyper loop. We prefer clockwise routing.
        let sl_loop = &sl_holder.sl_hyper_loops[edge.sl_hyper_loop];
        if sl_loop.occupied_port_sides.contains(source_port_side.right()) {
            EdgeRoutingDirection::Clockwise
        } else {
            debug_assert!(sl_loop.occupied_port_sides.contains(source_port_side.left()));
            EdgeRoutingDirection::CounterClockwise
        }
    }
}

/// `placeLabels` (the router's private method): places any labels of the
/// given self loop.
fn place_loop_labels(
    a: &LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
    routing_slot_positions: &[Vec<f64>; PORT_SIDE_COUNT],
    mut edge_label_distance: f64,
) {
    let slh_loop = &sl_holder.sl_hyper_loops[sl_loop];
    let sl_labels = slh_loop.sl_labels.as_ref().unwrap();

    // Find the baseline of the routing slot (we need to offset this by the
    // spacing to be left between label and edge)
    let label_side = sl_labels.side;
    let mut label_position =
        routing_slot_positions[label_side as usize][slh_loop.routing_slot(label_side) as usize];

    let inline = sl_labels
        .l_labels
        .iter()
        .any(|&l| a.label(l).properties.get(&lopts::EDGE_LABELS_INLINE));

    // Do not use edgeLabelDistance if the labels are inline
    if inline {
        edge_label_distance = 0.0;
    }

    let label_size = sl_labels.size;
    let sl_labels = sl_holder.sl_hyper_loops[sl_loop].sl_labels.as_mut().unwrap();

    match label_side {
        PortSide::NORTH => {
            label_position -= edge_label_distance + label_size.y;
            sl_labels.position.y = label_position;
        }

        PortSide::SOUTH => {
            label_position += edge_label_distance;
            sl_labels.position.y = label_position;
        }

        PortSide::WEST => {
            label_position -= edge_label_distance + label_size.x;
            sl_labels.position.x = label_position;
        }

        PortSide::EAST => {
            label_position += edge_label_distance;
            sl_labels.position.x = label_position;
        }

        PortSide::UNDEFINED => debug_assert!(false),
    }
}

/// `updateNewNodeMargins(KVector, LMargin, KVector)`: extends the node
/// margins to include the given bend point.
fn update_new_node_margins(node_size: KVector, new_node_margins: &mut Spacing, bend_point: KVector) {
    new_node_margins.left = f64::max(new_node_margins.left, -bend_point.x);
    new_node_margins.right = f64::max(new_node_margins.right, bend_point.x - node_size.x);

    new_node_margins.top = f64::max(new_node_margins.top, -bend_point.y);
    new_node_margins.bottom = f64::max(new_node_margins.bottom, bend_point.y - node_size.y);
}

/// `computeRoutingSlotPositions`: the position of each routing slot on
/// each side, leaving enough space for labels between adjacent routing slots
/// on the north and south sides.
fn compute_routing_slot_positions(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    edge_edge_distance: f64,
    edge_label_distance: f64,
    node_sl_distance: f64,
) -> [Vec<f64>; PORT_SIDE_COUNT] {
    // Initialize array
    let mut positions: [Vec<f64>; PORT_SIDE_COUNT] = Default::default();
    for side in 0..PORT_SIDE_COUNT {
        positions[side] = vec![0.0; sl_holder.routing_slot_count[side] as usize];
    }

    // To know how much space we need to leave between adjacent routing slots,
    // we have to find the size of labels first (for north and south sides)
    initialize_with_max_label_height(&mut positions, sl_holder, PortSide::NORTH);
    initialize_with_max_label_height(&mut positions, sl_holder, PortSide::SOUTH);

    // Compute the positions for each side
    for side in [PortSide::NORTH, PortSide::EAST, PortSide::SOUTH, PortSide::WEST] {
        compute_positions(
            a,
            &mut positions,
            sl_holder,
            side,
            edge_edge_distance,
            edge_label_distance,
            node_sl_distance,
        );
    }

    positions
}

/// `initializeWithMaxLabelHeight`.
fn initialize_with_max_label_height(
    positions: &mut [Vec<f64>; PORT_SIDE_COUNT],
    sl_holder: &SelfLoopHolder,
    port_side: PortSide,
) {
    debug_assert!(port_side == PortSide::NORTH || port_side == PortSide::SOUTH);

    let side_positions = &mut positions[port_side as usize];

    for sl_loop in &sl_holder.sl_hyper_loops {
        if let Some(sl_labels) = &sl_loop.sl_labels {
            if sl_labels.side == port_side {
                let routing_slot = sl_loop.routing_slot(port_side) as usize;
                side_positions[routing_slot] =
                    f64::max(side_positions[routing_slot], sl_labels.size.y);
            }
        }
    }
}

/// `computePositions`.
fn compute_positions(
    a: &LGraphArena,
    positions: &mut [Vec<f64>; PORT_SIDE_COUNT],
    sl_holder: &SelfLoopHolder,
    port_side: PortSide,
    edge_edge_distance: f64,
    edge_label_distance: f64,
    node_self_loop_distance: f64,
) {
    let mut curr_pos = compute_baseline_position(a, sl_holder, port_side, node_self_loop_distance);

    // For northern and western coordinates, we have to subtract from the
    // current position
    let factor = if port_side == PortSide::NORTH || port_side == PortSide::WEST { -1.0 } else { 1.0 };

    let side_positions = &mut positions[port_side as usize];
    for slot in 0..side_positions.len() {
        // The slot entry currently contains the height or width of the
        // largest label in that slot
        let mut largest_label_size = side_positions[slot];
        if largest_label_size > 0.0 {
            // Account for label spacing
            largest_label_size += edge_label_distance;
        }

        // Place the slot at the current position and advance the position
        side_positions[slot] = curr_pos;
        curr_pos += factor * (largest_label_size + edge_edge_distance);
    }
}

/// `computeBaselinePosition`: the offset from the node origin to add to
/// escape the area occupied by ports.
fn compute_baseline_position(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    port_side: PortSide,
    node_self_loop_distance: f64,
) -> f64 {
    let l_node = a.node(sl_holder.l_node);
    let l_margins = l_node.margin;

    match port_side {
        PortSide::NORTH => -l_margins.top - node_self_loop_distance,
        PortSide::EAST => l_node.size.x + l_margins.right + node_self_loop_distance,
        PortSide::SOUTH => l_node.size.y + l_margins.bottom + node_self_loop_distance,
        PortSide::WEST => -l_margins.left - node_self_loop_distance,
        PortSide::UNDEFINED => unreachable!(),
    }
}

/// `computeOrthogonalBendPoints`.
fn compute_orthogonal_bend_points(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    sl_edge: SlEdgeIdx,
    routing_direction: EdgeRoutingDirection,
    routing_slot_positions: &[Vec<f64>; PORT_SIDE_COUNT],
) -> Vec<KVector> {
    let mut bend_points = Vec::new();

    let edge = &sl_holder.sl_edges[sl_edge];
    add_outer_bend_point(a, sl_holder, sl_edge, edge.sl_source, routing_slot_positions, &mut bend_points);
    add_corner_bend_points(a, sl_holder, sl_edge, routing_direction, routing_slot_positions, &mut bend_points);
    add_outer_bend_point(a, sl_holder, sl_edge, edge.sl_target, routing_slot_positions, &mut bend_points);

    bend_points
}

/// `addOuterBendPoint`.
fn add_outer_bend_point(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    sl_edge: SlEdgeIdx,
    sl_port: SlPortIdx,
    routing_slot_positions: &[Vec<f64>; PORT_SIDE_COUNT],
    bend_points: &mut Vec<KVector>,
) {
    let sl_loop = &sl_holder.sl_hyper_loops[sl_holder.sl_edges[sl_edge].sl_hyper_loop];
    let l_port = a.port(sl_holder.sl_ports[sl_port].l_port);
    let port_side = l_port.side;

    // We'll start by computing the coordinate of the level we're on
    let mut result =
        get_base_vector(port_side, sl_loop.routing_slot(port_side), routing_slot_positions);

    // Now take care of the port anchor
    let mut anchor = l_port.pos;
    anchor.add(l_port.anchor);

    match port_side {
        PortSide::NORTH | PortSide::SOUTH => result.x += anchor.x,
        PortSide::EAST | PortSide::WEST => result.y += anchor.y,
        PortSide::UNDEFINED => debug_assert!(false),
    }

    bend_points.push(result);
}

/// `addCornerBendPoints`.
fn add_corner_bend_points(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    sl_edge: SlEdgeIdx,
    routing_direction: EdgeRoutingDirection,
    routing_slot_positions: &[Vec<f64>; PORT_SIDE_COUNT],
    bend_points: &mut Vec<KVector>,
) {
    let edge = &sl_holder.sl_edges[sl_edge];

    // Check if we even need corner bend points
    let l_source_port_side = a.port(sl_holder.sl_ports[edge.sl_source].l_port).side;
    let l_target_port_side = a.port(sl_holder.sl_ports[edge.sl_target].l_port).side;

    if l_source_port_side == l_target_port_side {
        return;
    }

    let sl_loop = &sl_holder.sl_hyper_loops[edge.sl_hyper_loop];

    // Use inline labels side and its size to determine the bendpoint offset
    // required to place the label
    let mut label_side = None;
    let mut l_size = None;
    let inline = edge.is_inline(a);
    if inline {
        if let Some(sl_labels) = &sl_loop.sl_labels {
            label_side = Some(sl_labels.side);
            l_size = Some(sl_labels.size);
        }
    }

    // Compute corner points
    let mut curr_port_side = l_source_port_side;

    while curr_port_side != l_target_port_side {
        // Next port side depends on the direction we're going
        let next_port_side = if routing_direction == EdgeRoutingDirection::Clockwise {
            curr_port_side.right()
        } else {
            curr_port_side.left()
        };

        // Compute the coordinates contributed by the current and next port sides
        let mut curr_port_side_component = get_base_vector(
            curr_port_side,
            sl_loop.routing_slot(curr_port_side),
            routing_slot_positions,
        );
        let mut next_port_side_component = get_base_vector(
            next_port_side,
            sl_loop.routing_slot(next_port_side),
            routing_slot_positions,
        );

        // If the label is inline, we need to reserve space for each side to
        // accommodate it
        if let (true, Some(label_side), Some(l_size)) = (inline, label_side, l_size) {
            if curr_port_side == label_side {
                adjust_vector_for_label_side(&mut curr_port_side_component, label_side, l_size);
            } else if next_port_side == label_side {
                adjust_vector_for_label_side(&mut next_port_side_component, label_side, l_size);
            }
        }

        // One has its x coordinate set, the other has its y coordinate set --
        // their sum is our final bend point
        curr_port_side_component.add(next_port_side_component);
        bend_points.push(curr_port_side_component);

        // Advance to next port side
        curr_port_side = next_port_side;
    }
}

/// `getBaseVector`.
fn get_base_vector(
    port_side: PortSide,
    routing_slot: i32,
    routing_slot_positions: &[Vec<f64>; PORT_SIDE_COUNT],
) -> KVector {
    let position = routing_slot_positions[port_side as usize][routing_slot as usize];

    match port_side {
        PortSide::NORTH | PortSide::SOUTH => KVector::new(0.0, position),
        PortSide::EAST | PortSide::WEST => KVector::new(position, 0.0),
        PortSide::UNDEFINED => unreachable!(),
    }
}

/// `adjustVectorForLabelSide`: ensures that an inline label is centered
/// on the bend point.
fn adjust_vector_for_label_side(
    port_side_component: &mut KVector,
    label_side: PortSide,
    label_size: KVector,
) {
    match label_side {
        PortSide::NORTH => port_side_component.y -= label_size.y / 2.0,
        PortSide::SOUTH => port_side_component.y += label_size.y / 2.0,
        PortSide::WEST => port_side_component.x -= label_size.x / 2.0,
        PortSide::EAST => port_side_component.x += label_size.x / 2.0,
        PortSide::UNDEFINED => {}
    }
}

// ---------------------------------------------------------------------------
// PolylineSelfLoopRouter

/// `PolylineSelfLoopRouter.CORNER_DISTANCE`.
const CORNER_DISTANCE: f64 = 10.0;
/// `PolylineSelfLoopRouter.TOLERANCE` for double comparisons.
const POLYLINE_TOLERANCE: f64 = 0.01;

/// `PolylineSelfLoopRouter.modifyBendPoints`: turns a vector chain of
/// orthogonal bend points into polyline bend points by cutting the corners.
fn polyline_modify_bend_points(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    sl_edge: SlEdgeIdx,
    mut bend_points: Vec<KVector>,
) -> Vec<KVector> {
    // Add the source and target points
    let edge = &sl_holder.sl_edges[sl_edge];
    let l_source_port = a.port(sl_holder.sl_ports[edge.sl_source].l_port);
    let mut source_anchor = l_source_port.pos;
    source_anchor.add(l_source_port.anchor);
    bend_points.insert(0, source_anchor);

    let l_target_port = a.port(sl_holder.sl_ports[edge.sl_target].l_port);
    let mut target_anchor = l_target_port.pos;
    target_anchor.add(l_target_port.anchor);
    bend_points.push(target_anchor);

    cut_corners(&bend_points, CORNER_DISTANCE)
}

/// `Math.signum(double)`.
fn java_signum(x: f64) -> f64 {
    if x == 0.0 || x.is_nan() {
        x
    } else if x > 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// `PolylineSelfLoopRouter.nearZeroToZero`.
fn near_zero_to_zero(mut vector: KVector) -> KVector {
    if vector.x >= -POLYLINE_TOLERANCE && vector.x <= POLYLINE_TOLERANCE {
        vector.x = 0.0;
    }
    if vector.y >= -POLYLINE_TOLERANCE && vector.y <= POLYLINE_TOLERANCE {
        vector.y = 0.0;
    }
    vector
}

/// `PolylineSelfLoopRouter.cutCorners`: replaces each inner bend point by
/// two which are ideally `distance` away from the original bend point. The
/// first and last point are not included in the returned list.
fn cut_corners(bend_points: &[KVector], distance: f64) -> Vec<KVector> {
    // The incoming list should consist of more than just the two end points
    debug_assert!(bend_points.len() > 2);

    let mut result: Vec<KVector> = Vec::new();

    let mut bp_iterator = bend_points.iter();
    let mut corner = *bp_iterator.next().unwrap();
    let mut next = *bp_iterator.next().unwrap();

    for &after in bp_iterator {
        // Move to the next corner
        let previous = corner;
        corner = next;
        next = after;

        // Compute how much we need to offset the corner to get to the previous and
        // next bend points (offsets always have one coordinate at 0)
        let mut diff1 = previous;
        diff1.sub(corner);
        let offset1 = near_zero_to_zero(diff1);
        let mut diff2 = next;
        diff2.sub(corner);
        let offset2 = near_zero_to_zero(diff2);

        // We usually use the standard distance, but that might be too much
        let mut effective_distance = distance;
        effective_distance = f64::min(effective_distance, (offset1.x + offset1.y).abs() / 2.0);
        effective_distance = f64::min(effective_distance, (offset2.x + offset2.y).abs() / 2.0);

        // Limit the offset vectors to our effective distance
        let mut o1 = KVector::new(
            java_signum(offset1.x) * effective_distance,
            java_signum(offset1.y) * effective_distance,
        );
        let mut o2 = KVector::new(
            java_signum(offset2.x) * effective_distance,
            java_signum(offset2.y) * effective_distance,
        );

        // Compute the effective bend points and add them to our result
        o1.add(corner);
        result.push(o1);
        o2.add(corner);
        result.push(o2);
    }

    result
}

// ---------------------------------------------------------------------------
// SplineSelfLoopRouter

/// `SplineSelfLoopRouter.DIM`.
const SPLINE_SELF_LOOP_DIM: usize = 3;
/// `SplineSelfLoopRouter.HALF`.
const HALF: f64 = 0.5;

/// `SplineSelfLoopRouter.relativePortAnchor`.
fn relative_port_anchor(a: &LGraphArena, sl_holder: &SelfLoopHolder, sl_port: SlPortIdx) -> KVector {
    let l_port = a.port(sl_holder.sl_ports[sl_port].l_port);
    let mut anchor = l_port.pos;
    anchor.add(l_port.anchor);
    anchor
}

/// `SplineSelfLoopRouter.modifyBendPoints`.
fn spline_modify_bend_points(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    sl_edge: SlEdgeIdx,
    routing_direction: EdgeRoutingDirection,
    bend_points: Vec<KVector>,
) -> Vec<KVector> {
    let edge_label_distance =
        get_individual_or_inherited(a, sl_holder.l_node, &lopts::SPACING_EDGE_LABEL);

    // For the splines to be routed correctly, we also have to include the source and
    // target positions
    let edge = &sl_holder.sl_edges[sl_edge];
    let mut spline_bend_points = vec![relative_port_anchor(a, sl_holder, edge.sl_source)];
    add_spline_control_points(
        a,
        sl_holder,
        sl_edge,
        routing_direction,
        &bend_points,
        &mut spline_bend_points,
        edge_label_distance,
    );
    spline_bend_points.push(relative_port_anchor(a, sl_holder, edge.sl_target));

    crate::alg_layered::p5edges::splines::nub_spline::NubSpline::new_clamped(
        SPLINE_SELF_LOOP_DIM,
        spline_bend_points,
    )
    .get_bezier_cp()
}

/// `SplineSelfLoopRouter.addSplineControlPoints`: inserts spline control
/// points between each consecutive pair of bend points as computed by the
/// orthogonal self loop router, slightly offset away from the node.
fn add_spline_control_points(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    sl_edge: SlEdgeIdx,
    routing_direction: EdgeRoutingDirection,
    ortho_bend_points: &[KVector],
    new_bend_points: &mut Vec<KVector>,
    edge_label_distance: f64,
) {
    debug_assert!(ortho_bend_points.len() >= 2);

    // We want to insert a new bend point between each pair of consecutive bend points
    // in the old list
    let edge = &sl_holder.sl_edges[sl_edge];
    let mut curr_port_side = a.port(sl_holder.sl_ports[edge.sl_source].l_port).side;
    let mut first_bp = ortho_bend_points[0];

    for &second_bp in &ortho_bend_points[1..] {
        // The first bend point will go straight into our list before we compute a new
        // bend point
        new_bend_points.push(first_bp);

        // Compute a middle bend point and move it away from the node a little
        let mut mid_bp = first_bp;
        mid_bp.add(second_bp);
        mid_bp.scale(HALF);
        let mut offset = KVector::from_angle(
            crate::alg_layered::p5edges::splines::splines_math::port_side_to_direction(curr_port_side),
        );
        offset.scale(edge_label_distance);
        mid_bp.add(offset);

        new_bend_points.push(mid_bp);

        // Advance to the next pair of bend points on the next port side
        first_bp = second_bp;
        curr_port_side = if routing_direction == EdgeRoutingDirection::Clockwise {
            curr_port_side.right()
        } else {
            curr_port_side.left()
        };
    }

    // Add the last of the original bend points
    new_bend_points.push(*ortho_bend_points.last().unwrap());
}
