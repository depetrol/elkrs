
use crate::core::options::PortSide;
use crate::graph::properties::EnumSet;

use crate::alg_layered::graph::{LGraphArena, LNodeId, LPortId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::{SelfLoopDistributionStrategy, SelfLoopOrderingStrategy};

use super::{SelfHyperLoop, SelfLoopHolder, SelfLoopType, SlLoopIdx, SlPortIdx, PORT_SIDE_COUNT};

// ---------------------------------------------------------------------------
// PortSideAssigner

/// The way ports are distributed when
/// self loops are assigned to all sides.
#[derive(Clone, Copy)]
struct Target {
    first_side: PortSide,
    second_side: PortSide,
}

impl Target {
    fn is_corner_target(&self) -> bool {
        self.first_side != self.second_side
    }
}

/// `Target.values()` in declaration order.
const ASSIGNMENT_TARGETS: [Target; 8] = [
    Target { first_side: PortSide::NORTH, second_side: PortSide::NORTH },
    Target { first_side: PortSide::SOUTH, second_side: PortSide::SOUTH },
    Target { first_side: PortSide::EAST, second_side: PortSide::EAST },
    Target { first_side: PortSide::WEST, second_side: PortSide::WEST },
    Target { first_side: PortSide::WEST, second_side: PortSide::NORTH },
    Target { first_side: PortSide::NORTH, second_side: PortSide::EAST },
    Target { first_side: PortSide::SOUTH, second_side: PortSide::WEST },
    Target { first_side: PortSide::EAST, second_side: PortSide::SOUTH },
];

/// Assigns port sides to all
/// hidden ports subject to the self loop distribution strategy.
pub fn assign_port_sides(a: &mut LGraphArena, sl_holder: &mut SelfLoopHolder) {
    match a
        .node(sl_holder.l_node)
        .properties
        .get(&lopts::EDGE_ROUTING_SELF_LOOP_DISTRIBUTION)
    {
        SelfLoopDistributionStrategy::NORTH => assign_to_north_side(a, sl_holder),
        SelfLoopDistributionStrategy::NORTH_SOUTH => assign_to_north_or_south_side(a, sl_holder),
        SelfLoopDistributionStrategy::EQUALLY => assign_to_all_sides(a, sl_holder),
    }
}

/// Returns the loop's hidden self loop ports (`hiddenSelfLoopPortStream`).
fn hidden_sl_ports(sl_holder: &SelfLoopHolder, sl_loop: &SelfHyperLoop) -> Vec<SlPortIdx> {
    sl_loop
        .sl_ports
        .iter()
        .copied()
        .filter(|&p| sl_holder.sl_ports[p].hidden)
        .collect()
}

/// `assignToNorthSide`.
fn assign_to_north_side(a: &mut LGraphArena, sl_holder: &SelfLoopHolder) {
    for sl_loop in &sl_holder.sl_hyper_loops {
        for sl_port in hidden_sl_ports(sl_holder, sl_loop) {
            a.port_set_side(sl_holder.sl_ports[sl_port].l_port, PortSide::NORTH);
        }
    }
}

/// `assignToNorthOrSouthSide`: greedy distribution onto north / south.
fn assign_to_north_or_south_side(a: &mut LGraphArena, sl_holder: &SelfLoopHolder) {
    let mut north_ports = 0usize;
    let mut south_ports = 0usize;

    for sl_loop in &sl_holder.sl_hyper_loops {
        let sl_hidden_ports = hidden_sl_ports(sl_holder, sl_loop);

        // Decide on a port side
        let new_port_side;
        if north_ports <= south_ports {
            new_port_side = PortSide::NORTH;
            north_ports += sl_hidden_ports.len();
        } else {
            new_port_side = PortSide::SOUTH;
            south_ports += sl_hidden_ports.len();
        }

        // Assign the ports
        for sl_port in sl_hidden_ports {
            a.port_set_side(sl_holder.sl_ports[sl_port].l_port, new_port_side);
        }
    }
}

/// `assignToAllSides`.
fn assign_to_all_sides(a: &mut LGraphArena, sl_holder: &mut SelfLoopHolder) {
    // Obtain a list of self hyper loops, ordered descendingly by the number of
    // involved ports
    let mut sl_sorted_loops: Vec<SlLoopIdx> = (0..sl_holder.sl_hyper_loops.len()).collect();
    sl_sorted_loops.sort_by(|&l1, &l2| {
        sl_holder.sl_hyper_loops[l2]
            .sl_ports
            .len()
            .cmp(&sl_holder.sl_hyper_loops[l1].sl_ports.len())
    });

    // Iterate over our self loops and assign each to the next target
    for (curr_loop, &sl_loop) in sl_sorted_loops.iter().enumerate() {
        let curr_target = ASSIGNMENT_TARGETS[curr_loop % ASSIGNMENT_TARGETS.len()];
        assign_to_target(a, sl_holder, sl_loop, curr_target);
    }
}

/// `assignToTarget`.
fn assign_to_target(
    a: &mut LGraphArena,
    sl_holder: &mut SelfLoopHolder,
    sl_loop: SlLoopIdx,
    target: Target,
) {
    // If this is a corner target, we sort the list of ports by net flow (this
    // sorts the loop's original list, which is okay since it doesn't have any
    // particular order prior to running the PortRestorer)
    if target.is_corner_target() {
        let mut sl_ports = std::mem::take(&mut sl_holder.sl_hyper_loops[sl_loop].sl_ports);
        sl_ports.sort_by(|&p1, &p2| {
            a.port_net_flow(sl_holder.sl_ports[p1].l_port)
                .cmp(&a.port_net_flow(sl_holder.sl_ports[p2].l_port))
        });
        sl_holder.sl_hyper_loops[sl_loop].sl_ports = sl_ports;
    }

    // Assign the first half of the ports to the first side, and the second
    // half to the second side. Only assign ports that have been hidden.
    let sl_ports = sl_holder.sl_hyper_loops[sl_loop].sl_ports.clone();
    let second_half_start_index = sl_ports.len() / 2;

    for &sl_port in &sl_ports[..second_half_start_index] {
        if sl_holder.sl_ports[sl_port].hidden {
            a.port_set_side(sl_holder.sl_ports[sl_port].l_port, target.first_side);
        }
    }

    for &sl_port in &sl_ports[second_half_start_index..] {
        if sl_holder.sl_ports[sl_port].hidden {
            a.port_set_side(sl_holder.sl_ports[sl_port].l_port, target.second_side);
        }
    }
}

// ---------------------------------------------------------------------------
// PortRestorer

/// The three different areas of a port
/// side, in clockwise order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PortSideArea {
    Start,
    Middle,
    End,
}

/// Whether to prepend or append items to a list.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AddMode {
    Prepend,
    Append,
}

/// A table of all of the different areas around the node to place self loop
/// ports, indexed by `PortSide` ordinal and area.
type TargetAreas = [[Vec<SlPortIdx>; 3]; PORT_SIDE_COUNT];

/// Restores all previously hidden ports
/// at proper locations and resets all hiding-related state.
pub fn restore_ports(a: &mut LGraphArena, sl_holder: &mut SelfLoopHolder) {
    let mut target_areas: TargetAreas = Default::default();

    // We distinguish different types of loops depending (mainly) on their
    // number of port sides
    let sl_loops_by_type = gather_self_loops_by_type(sl_holder);

    // Now the loops have to be added in a certain order in an attempt to
    // minimize edge crossings
    let ordering = a
        .node(sl_holder.l_node)
        .properties
        .get(&lopts::EDGE_ROUTING_SELF_LOOP_ORDERING);
    process_one_side_loops(a, sl_holder, &sl_loops_by_type, &mut target_areas, ordering);
    process_two_side_corner_loops(sl_holder, &sl_loops_by_type, &mut target_areas);
    process_three_side_loops(sl_holder, &sl_loops_by_type, &mut target_areas);
    process_four_side_loops(sl_holder, &sl_loops_by_type, &mut target_areas);
    process_two_side_opposing_loops(sl_holder, &sl_loops_by_type, &mut target_areas);

    // Actually go ahead and add the ports to the real port list
    do_restore_ports(a, sl_holder, &target_areas);

    // We're not hiding any ports anymore
    for side_areas in &target_areas {
        for area in side_areas {
            for &sl_port in area {
                sl_holder.sl_ports[sl_port].hidden = false;
            }
        }
    }
    sl_holder.are_ports_hidden = false;
}

/// `gatherSelfLoopsByType`: loop indices per [`SelfLoopType`], in holder
/// order (`ArrayListMultimap`).
fn gather_self_loops_by_type(sl_holder: &SelfLoopHolder) -> [Vec<SlLoopIdx>; 5] {
    let mut loops: [Vec<SlLoopIdx>; 5] = Default::default();
    for (idx, sl_loop) in sl_holder.sl_hyper_loops.iter().enumerate() {
        loops[type_ordinal(sl_loop.self_loop_type.unwrap())].push(idx);
    }
    loops
}

fn type_ordinal(t: SelfLoopType) -> usize {
    match t {
        SelfLoopType::OneSide => 0,
        SelfLoopType::TwoSidesCorner => 1,
        SelfLoopType::TwoSidesOpposing => 2,
        SelfLoopType::ThreeSides => 3,
        SelfLoopType::FourSides => 4,
    }
}

/// `processOneSideLoops`.
fn process_one_side_loops(
    a: &LGraphArena,
    sl_holder: &SelfLoopHolder,
    sl_loops_by_type: &[Vec<SlLoopIdx>; 5],
    target_areas: &mut TargetAreas,
    ordering: SelfLoopOrderingStrategy,
) {
    let mut one_side_loops = sl_loops_by_type[type_ordinal(SelfLoopType::OneSide)].clone();
    if ordering == SelfLoopOrderingStrategy::REVERSE_STACKED {
        one_side_loops.reverse();
    }
    for &sl_loop_idx in &one_side_loops {
        let sl_loop = &sl_holder.sl_hyper_loops[sl_loop_idx];

        // Obtain the port side
        let side = a.port(sl_holder.sl_ports[sl_loop.sl_ports[0]].l_port).side;

        // We want ports with more outgoing edges to be to the left of ports
        // with more incoming edges, so we need a sorted list of ports
        let mut sorted_ports = sl_loop.sl_ports.clone();
        sorted_ports.sort_by(|&p1, &p2| {
            sl_holder.sl_ports[p1].sl_net_flow().cmp(&sl_holder.sl_ports[p2].sl_net_flow())
        });

        match ordering {
            SelfLoopOrderingStrategy::SEQUENCED => {
                // Simply add all ports according to our list
                add_to_target_area(
                    sl_holder,
                    &sorted_ports,
                    side,
                    PortSideArea::Middle,
                    AddMode::Append,
                    target_areas,
                );
            }
            SelfLoopOrderingStrategy::REVERSE_STACKED | SelfLoopOrderingStrategy::STACKED => {
                // Compute which ports we want to have in the first group and
                // which in the second group
                let split_index = compute_port_list_split_index(sl_holder, &sorted_ports);

                // Prepend the first group to the middle list, and append the
                // second group to that same list
                add_to_target_area(
                    sl_holder,
                    &sorted_ports[..split_index],
                    side,
                    PortSideArea::Middle,
                    AddMode::Prepend,
                    target_areas,
                );
                add_to_target_area(
                    sl_holder,
                    &sorted_ports[split_index..],
                    side,
                    PortSideArea::Middle,
                    AddMode::Append,
                    target_areas,
                );
            }
        }
    }
}

/// `computePortListSplitIndex`. Note: this preserves the original
/// implementation verbatim, including the quirk that the second search also
/// tests for a *positive* (not non-negative) net flow and reuses
/// `positiveNetFlowIndex` in its bounds check.
fn compute_port_list_split_index(sl_holder: &SelfLoopHolder, sorted_ports: &[SlPortIdx]) -> usize {
    // Find index of the first port with a positive net flow
    let mut positive_net_flow_index = 0;
    while positive_net_flow_index < sorted_ports.len() {
        if sl_holder.sl_ports[sorted_ports[positive_net_flow_index]].sl_net_flow() > 0 {
            break;
        }
        positive_net_flow_index += 1;
    }

    // If this is neither the first, nor the last port, return its index
    if positive_net_flow_index > 0 && positive_net_flow_index < sorted_ports.len() - 1 {
        return positive_net_flow_index;
    }

    // Find index of the first port with a non-negative net flow
    let mut non_negative_net_flow_index = 0;
    while non_negative_net_flow_index < sorted_ports.len() {
        if sl_holder.sl_ports[sorted_ports[non_negative_net_flow_index]].sl_net_flow() > 0 {
            break;
        }
        non_negative_net_flow_index += 1;
    }

    // If this is neither the first, nor the last port, return its index
    if non_negative_net_flow_index > 0 && positive_net_flow_index < sorted_ports.len() - 1 {
        return non_negative_net_flow_index;
    }

    // We tried our best; simply return the center port's index
    sorted_ports.len() / 2
}

/// `processTwoSideCornerLoops`.
fn process_two_side_corner_loops(
    sl_holder: &SelfLoopHolder,
    sl_loops_by_type: &[Vec<SlLoopIdx>; 5],
    target_areas: &mut TargetAreas,
) {
    for &sl_loop in &sl_loops_by_type[type_ordinal(SelfLoopType::TwoSidesCorner)] {
        // Sort the port sides such that they follow a clockwise order
        let sides = sorted_two_side_loop_port_sides(&sl_holder.sl_hyper_loops[sl_loop]);

        // Add the ports to their target area
        add_loop_to_target_area(
            sl_holder, sl_loop, sides[0], PortSideArea::End, AddMode::Prepend, target_areas,
        );
        add_loop_to_target_area(
            sl_holder, sl_loop, sides[1], PortSideArea::Start, AddMode::Append, target_areas,
        );
    }
}

/// `processTwoSideOpposingLoops`.
fn process_two_side_opposing_loops(
    sl_holder: &SelfLoopHolder,
    sl_loops_by_type: &[Vec<SlLoopIdx>; 5],
    target_areas: &mut TargetAreas,
) {
    for &sl_loop in &sl_loops_by_type[type_ordinal(SelfLoopType::TwoSidesOpposing)] {
        // Sort the port sides such that they follow a clockwise order
        let sides = sorted_two_side_loop_port_sides(&sl_holder.sl_hyper_loops[sl_loop]);

        // We prepend to the start side's end area, and append to the target
        // side's start area
        add_loop_to_target_area(
            sl_holder, sl_loop, sides[0], PortSideArea::End, AddMode::Prepend, target_areas,
        );
        add_loop_to_target_area(
            sl_holder, sl_loop, sides[1], PortSideArea::Start, AddMode::Append, target_areas,
        );
    }
}

/// `sortedTwoSideLoopPortSides`: the port sides spanned by a two-side
/// self loop in a clockwise order.
pub fn sorted_two_side_loop_port_sides(sl_loop: &SelfHyperLoop) -> [PortSide; 2] {
    let mut sides = [sl_loop.sl_port_sides[0], sl_loop.sl_port_sides[1]];
    sides.sort();

    // NORTH and WEST are, of course, the exception and need to be switched...
    if sides[0] == PortSide::NORTH && sides[1] == PortSide::WEST {
        sides[0] = PortSide::WEST;
        sides[1] = PortSide::NORTH;
    }

    sides
}

// Different possible constellations of three-sided loops
const NES: [PortSide; 3] = [PortSide::NORTH, PortSide::EAST, PortSide::SOUTH];
const ESW: [PortSide; 3] = [PortSide::EAST, PortSide::SOUTH, PortSide::WEST];
const SWN: [PortSide; 3] = [PortSide::SOUTH, PortSide::WEST, PortSide::NORTH];
const WNE: [PortSide; 3] = [PortSide::WEST, PortSide::NORTH, PortSide::EAST];

/// `processThreeSideLoops`.
fn process_three_side_loops(
    sl_holder: &SelfLoopHolder,
    sl_loops_by_type: &[Vec<SlLoopIdx>; 5],
    target_areas: &mut TargetAreas,
) {
    for &sl_loop in &sl_loops_by_type[type_ordinal(SelfLoopType::ThreeSides)] {
        // This array will yield the loop's start, middle, and end sides
        let sides = determine_loop_constellation(&sl_holder.sl_hyper_loops[sl_loop]);

        // Prepend to the start area, append to the other areas
        add_loop_to_target_area(
            sl_holder, sl_loop, sides[0], PortSideArea::End, AddMode::Prepend, target_areas,
        );
        add_loop_to_target_area(
            sl_holder, sl_loop, sides[1], PortSideArea::Middle, AddMode::Append, target_areas,
        );
        add_loop_to_target_area(
            sl_holder, sl_loop, sides[2], PortSideArea::Start, AddMode::Append, target_areas,
        );
    }
}

/// `determineLoopConstellation`.
fn determine_loop_constellation(sl_loop: &SelfHyperLoop) -> [PortSide; 3] {
    let port_sides: EnumSet<PortSide> = sl_loop.sl_port_sides.iter().copied().collect();

    if !port_sides.contains(PortSide::NORTH) {
        ESW
    } else if !port_sides.contains(PortSide::EAST) {
        SWN
    } else if !port_sides.contains(PortSide::SOUTH) {
        WNE
    } else {
        debug_assert!(!port_sides.contains(PortSide::WEST));
        NES
    }
}

/// `processFourSideLoops`.
fn process_four_side_loops(
    sl_holder: &SelfLoopHolder,
    sl_loops_by_type: &[Vec<SlLoopIdx>; 5],
    target_areas: &mut TargetAreas,
) {
    for &sl_loop in &sl_loops_by_type[type_ordinal(SelfLoopType::FourSides)] {
        // Simply append to all port side's middle areas
        for side in sl_holder.sl_hyper_loops[sl_loop].sl_port_sides.clone() {
            add_loop_to_target_area(
                sl_holder, sl_loop, side, PortSideArea::Middle, AddMode::Append, target_areas,
            );
        }
    }
}

/// `addToTargetArea(SelfHyperLoop, ...)`: adds the ports of the given
/// loop on the given side to one of the target areas.
fn add_loop_to_target_area(
    sl_holder: &SelfLoopHolder,
    sl_loop: SlLoopIdx,
    port_side: PortSide,
    area: PortSideArea,
    add_mode: AddMode,
    target_areas: &mut TargetAreas,
) {
    let ports = sl_holder.sl_hyper_loops[sl_loop].sl_ports_on_side(port_side).to_vec();
    add_to_target_area(sl_holder, &ports, port_side, area, add_mode, target_areas);
}

/// `addToTargetArea(Collection, ...)`: adds a collection of ports to one
/// of the target areas.
fn add_to_target_area(
    sl_holder: &SelfLoopHolder,
    sl_ports: &[SlPortIdx],
    port_side: PortSide,
    area: PortSideArea,
    add_mode: AddMode,
    target_areas: &mut TargetAreas,
) {
    // Gather those ports that are currently hidden (if they're not hidden,
    // there's no point restoring them)
    let mut hidden_ports: Vec<SlPortIdx> = sl_ports
        .iter()
        .copied()
        .filter(|&p| sl_holder.sl_ports[p].hidden)
        .collect();

    hidden_ports.reverse();
    let target_area = &mut target_areas[port_side as usize][area as usize];
    match add_mode {
        AddMode::Prepend => {
            target_area.splice(0..0, hidden_ports);
        }
        AddMode::Append => {
            target_area.extend(hidden_ports);
        }
    }
}

/// `restorePorts(SelfLoopHolder)` (private): builds the node's new port
/// list, inserting the target areas in between the regular ports.
fn do_restore_ports(a: &mut LGraphArena, sl_holder: &SelfLoopHolder, target_areas: &TargetAreas) {
    let l_node = sl_holder.l_node;

    // We'll add the old ports in bursts and always remember where the next
    // burst starts
    let old_port_list = a.node(l_node).ports.clone();
    let mut next_old_port_index = 0;

    a.node_mut(l_node).ports.clear();

    let area = |side: PortSide, area: PortSideArea| &target_areas[side as usize][area as usize];

    // Go over the target areas and add them in between the regular ports
    add_all(a, sl_holder, area(PortSide::NORTH, PortSideArea::Start), l_node);
    next_old_port_index = add_all_that(a, &old_port_list, next_old_port_index, |a, p| {
        a.port(p).side == PortSide::NORTH && is_north_south_port_with_west_or_west_east_connections(a, p)
    }, l_node);
    add_all(a, sl_holder, area(PortSide::NORTH, PortSideArea::Middle), l_node);
    next_old_port_index = add_all_that(a, &old_port_list, next_old_port_index, |a, p| {
        a.port(p).side == PortSide::NORTH
    }, l_node);
    add_all(a, sl_holder, area(PortSide::NORTH, PortSideArea::End), l_node);

    add_all(a, sl_holder, area(PortSide::EAST, PortSideArea::Start), l_node);
    add_all(a, sl_holder, area(PortSide::EAST, PortSideArea::Middle), l_node);
    next_old_port_index = add_all_that(a, &old_port_list, next_old_port_index, |a, p| {
        a.port(p).side == PortSide::EAST
    }, l_node);
    add_all(a, sl_holder, area(PortSide::EAST, PortSideArea::End), l_node);

    add_all(a, sl_holder, area(PortSide::SOUTH, PortSideArea::Start), l_node);
    next_old_port_index = add_all_that(a, &old_port_list, next_old_port_index, |a, p| {
        a.port(p).side == PortSide::SOUTH && is_north_south_port_with_east_connections(a, p)
    }, l_node);
    add_all(a, sl_holder, area(PortSide::SOUTH, PortSideArea::Middle), l_node);
    next_old_port_index = add_all_that(a, &old_port_list, next_old_port_index, |a, p| {
        a.port(p).side == PortSide::SOUTH
    }, l_node);
    add_all(a, sl_holder, area(PortSide::SOUTH, PortSideArea::End), l_node);

    add_all(a, sl_holder, area(PortSide::WEST, PortSideArea::Start), l_node);
    let _ = add_all_that(a, &old_port_list, next_old_port_index, |a, p| {
        a.port(p).side == PortSide::WEST
    }, l_node);
    add_all(a, sl_holder, area(PortSide::WEST, PortSideArea::Middle), l_node);
    add_all(a, sl_holder, area(PortSide::WEST, PortSideArea::End), l_node);
}

/// `addAll`: adds the `LPort`s of all of the [`SelfLoopPort`]s to the
/// given node by calling `LPort.setNode(LNode)`, which automatically adds
/// them to the node's port list.
fn add_all(a: &mut LGraphArena, sl_holder: &SelfLoopHolder, sl_ports: &[SlPortIdx], l_node: LNodeId) {
    for &sl_port in sl_ports {
        a.port_set_node(sl_holder.sl_ports[sl_port].l_port, Some(l_node));
    }
}

/// `addAllThat`: adds all ports from the list that satisfy the given
/// predicate, starting at the given index, until the first port that does not
/// satisfy the predicate anymore. Returns that port's index.
fn add_all_that(
    a: &mut LGraphArena,
    l_ports: &[LPortId],
    from_index: usize,
    condition: impl Fn(&LGraphArena, LPortId) -> bool,
    l_node: LNodeId,
) -> usize {
    for (i, &l_port) in l_ports.iter().enumerate().skip(from_index) {
        // If this port is valid, add it to the list
        if condition(a, l_port) {
            a.node_mut(l_node).ports.push(l_port);
        } else {
            return i;
        }
    }
    l_ports.len()
}

/// `isNorthSouthPortWithWestOrWestEastConnections`.
fn is_north_south_port_with_west_or_west_east_connections(a: &LGraphArena, l_port: LPortId) -> bool {
    let connections = north_south_port_connection_sides(a, l_port);
    let east_connections = connections.contains(PortSide::EAST);
    let west_connections = connections.contains(PortSide::WEST);

    west_connections || (west_connections && east_connections)
}

/// `isNorthSouthPortWithEastConnections`.
fn is_north_south_port_with_east_connections(a: &LGraphArena, l_port: LPortId) -> bool {
    let connections = north_south_port_connection_sides(a, l_port);
    connections.contains(PortSide::EAST)
}

/// `northSouthPortConnectionSides`: gathers the sides to which the given
/// north / south port has connections.
fn north_south_port_connection_sides(a: &LGraphArena, l_port: LPortId) -> EnumSet<PortSide> {
    let mut connection_sides: EnumSet<PortSide> = EnumSet::none();

    if let Some(port_dummy) = a.port(l_port).properties.try_get(&iprops::PORT_DUMMY) {
        for &dummy_l_port in &a.node(port_dummy).ports {
            if a.port(dummy_l_port).properties.try_get(&iprops::ORIGIN)
                == Some(iprops::Origin::LPort(l_port))
            {
                // This dummy port was indeed created for our original port
                if !a.port(dummy_l_port).incoming_edges.is_empty()
                    || !a.port(dummy_l_port).outgoing_edges.is_empty()
                {
                    // The port has incident edges!
                    connection_sides.add(a.port(dummy_l_port).side);
                }
            }
        }
    }

    connection_sides
}
