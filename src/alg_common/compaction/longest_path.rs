
use std::collections::VecDeque;

use crate::core::options::Direction;

use super::one_dimensional_compactor::OneDimensionalCompactor;
use super::CGroupId;

/// Compacts a constraint graph using a technique similar to longest-path
/// layering.
pub fn longest_path_compact(compactor: &mut OneDimensionalCompactor) {
    // calculating the left-most position of any element
    let mut min_start_pos = f64::INFINITY;
    for i in 0..compactor.cgraph.cnodes.len() {
        let cnode = &compactor.cgraph.cnodes[i];
        let reference = compactor.cgraph.cgroups[cnode.cgroup.unwrap()].reference.unwrap();
        let pos = compactor.cgraph.cnodes[reference].hitbox.x + cnode.cgroup_offset.x;
        min_start_pos = min_start_pos.min(pos);
    }

    // finding the sinks of the constraint graph
    let mut sinks: VecDeque<CGroupId> = VecDeque::new();
    for g in 0..compactor.cgraph.cgroups.len() {
        compactor.cgraph.cgroups[g].start_pos = min_start_pos;
        if compactor.cgraph.cgroups[g].out_degree == 0 {
            sinks.push_back(g);
        }
    }

    let direction = compactor.direction;

    while let Some(group) = sinks.pop_front() {
        let reference = compactor.cgraph.cgroups[group].reference.unwrap();
        let mut diff = compactor.cgraph.cnodes[reference].hitbox.x;

        // #1 final positions for this group's nodes
        let group_start_pos = compactor.cgraph.cgroups[group].start_pos;
        let cnodes = compactor.cgraph.cgroups[group].cnodes.clone();
        let group_locked = compactor.is_locked_group(group, direction);
        for &node in &cnodes {
            let suggested_x = group_start_pos + compactor.cgraph.cnodes[node].cgroup_offset.x;
            if !group_locked || (compactor.cgraph.cnodes[node].hitbox.x < suggested_x) {
                compactor.cgraph.cnodes[node].start_pos = suggested_x;
            } else {
                compactor.cgraph.cnodes[node].start_pos = compactor.cgraph.cnodes[node].hitbox.x;
            }
        }

        diff -= compactor.cgraph.cnodes[reference].start_pos;
        compactor.cgraph.cgroups[group].delta += diff;
        if direction == Direction::RIGHT || direction == Direction::DOWN {
            compactor.cgraph.cgroups[group].delta_normalized += diff;
        } else {
            compactor.cgraph.cgroups[group].delta_normalized -= diff;
        }

        // #2 propagate start positions to constrained groups
        for &node in &cnodes {
            let constraints = compactor.cgraph.cnodes[node].constraints.clone();
            for inc_node in constraints {
                let spacing = if direction.is_horizontal() {
                    compactor.spacings_handler.horizontal_spacing(&compactor.cgraph, node, inc_node)
                } else {
                    compactor.spacings_handler.vertical_spacing(&compactor.cgraph, node, inc_node)
                };

                let node_start = compactor.cgraph.cnodes[node].start_pos;
                let node_width = compactor.cgraph.cnodes[node].hitbox.width;
                let inc_offset = compactor.cgraph.cnodes[inc_node].cgroup_offset.x;
                let inc_group = compactor.cgraph.cnodes[inc_node].cgroup.unwrap();

                let candidate = node_start + node_width + spacing - inc_offset;
                let cur = compactor.cgraph.cgroups[inc_group].start_pos;
                compactor.cgraph.cgroups[inc_group].start_pos = cur.max(candidate);

                if compactor.is_locked_node(inc_node, direction) {
                    let inc_hb_x = compactor.cgraph.cnodes[inc_node].hitbox.x;
                    let cur = compactor.cgraph.cgroups[inc_group].start_pos;
                    compactor.cgraph.cgroups[inc_group].start_pos = cur.max(inc_hb_x - inc_offset);
                }

                compactor.cgraph.cgroups[inc_group].out_degree -= 1;
                if compactor.cgraph.cgroups[inc_group].out_degree == 0 {
                    sinks.push_back(inc_group);
                }
            }
        }
    }

    // #3 setting hitbox positions to new starting positions
    for n in &mut compactor.cgraph.cnodes {
        n.hitbox.x = n.start_pos;
    }
}
