//!
//! Since all per-group state is reset at the start of each `processConstraints`
//! run, this builds a fresh local group arena per run.

use std::collections::HashMap;

use crate::alg_layered::graph::{LGraphArena, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;

use super::barycenter_heuristic::BarycenterState;

/// Delta that two barycenters can differ by to still be considered equal.
/// (Unused at runtime — only used in assertions.)
#[allow(unused)]
const BARYCENTER_EQUALITY_DELTA: f32 = 0.0001f32;

pub struct ForsterConstraintResolver {
    /// Whether there are successor constraints between non-dummies.
    constraints_between_non_dummies: bool,
    /// the layout units for handling dummy nodes for north / south ports
    /// (values per key in insertion order).
    layout_units: HashMap<LNodeId, Vec<LNodeId>>,
    /// the barycenter values of every node in the graph, indexed by
    /// `layer.id` and `node.id`.
    pub barycenter_states: Vec<Vec<BarycenterState>>,
}

/// The inner class `ConstraintGroup` (local arena representation;
/// groups reference each other by arena index).
struct ConstraintGroup {
    /// The sum of the node weights.
    summed_weight: f64,
    /// The number of ports relevant to the barycenter calculation.
    degree: i32,
    /// List of nodes this vertex consists of.
    nodes: Vec<LNodeId>,
    /// List of outgoing constraints (None = null).
    outgoing_constraints: Option<Vec<usize>>,
    /// List of incoming constraints (None = null).
    incoming_constraints: Option<Vec<usize>>,
    /// The number of incoming constraints.
    incoming_constraints_count: i32,
}

impl ConstraintGroup {
    fn single(node: LNodeId) -> Self {
        ConstraintGroup {
            summed_weight: 0.0,
            degree: 0,
            nodes: vec![node],
            outgoing_constraints: None,
            incoming_constraints: None,
            incoming_constraints_count: 0,
        }
    }

    fn has_outgoing_constraints(&self) -> bool {
        self.outgoing_constraints.as_ref().is_some_and(|c| !c.is_empty())
    }

    fn has_incoming_constraints(&self) -> bool {
        self.incoming_constraints.as_ref().is_some_and(|c| !c.is_empty())
    }
}

impl ForsterConstraintResolver {
    /// The constructor; the `init_at_*` traversal hooks are driven
    /// by `GraphInfoHolder`.
    pub fn new(a: &LGraphArena, current_node_order: &[Vec<LNodeId>]) -> Self {
        let mut constraints_between_non_dummies = false;
        if !current_node_order.is_empty() && !current_node_order[0].is_empty() {
            let graph = a.node_graph(current_node_order[0][0]);
            constraints_between_non_dummies = a
                .graph(graph)
                .properties
                .get(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS_BETWEEN_NON_DUMMIES);
        }
        ForsterConstraintResolver {
            constraints_between_non_dummies,
            layout_units: HashMap::new(),
            barycenter_states: vec![Vec::new(); current_node_order.len()],
        }
    }

    pub fn init_at_layer_level(&mut self, l: usize, node_order: &[Vec<LNodeId>]) {
        // The barycenterStates / constraintGroups rows are allocated here;
        // the actual entries are created at node level.
        self.barycenter_states[l] = Vec::with_capacity(node_order[l].len());
    }

    pub fn init_at_node_level(&mut self, a: &LGraphArena, l: usize, n: usize, node_order: &[Vec<LNodeId>]) {
        let node = node_order[l][n];
        // full init: barycenter state and layout units (constraint groups are
        // rebuilt per run in this port)
        debug_assert_eq!(self.barycenter_states[l].len(), n);
        self.barycenter_states[l].push(BarycenterState::new(node));

        if let Some(layout_unit) = a.node(node).properties.try_get(&iprops::IN_LAYER_LAYOUT_UNIT) {
            self.layout_units.entry(layout_unit).or_default().push(node);
        }
    }

    // -------------------------------------------------- constraint processing

    /// Finds and handles violated
    /// in-layer successor constraints.
    pub fn process_constraints(&mut self, a: &LGraphArena, nodes: &mut Vec<LNodeId>) {
        // If there are successor constraints between regular (or normal)
        // nodes, we have to apply a two-stage process.
        if self.constraints_between_non_dummies {
            self.process_constraints_stage(a, nodes, true);
            // The per-node constraint groups are re-created here
            // (initAtNodeLevel(node, false)); with the local group arena
            // below this is implicit.
        }

        self.process_constraints_stage(a, nodes, false);
    }

    fn process_constraints_stage(
        &mut self,
        a: &LGraphArena,
        nodes: &mut Vec<LNodeId>,
        only_between_normal_nodes: bool,
    ) {
        // group arena; indices 0..nodes.len() are the per-node groups in
        // current node order (mirrors `groups.add(constraintGroups[..][..])`)
        let mut groups: Vec<ConstraintGroup> = Vec::with_capacity(nodes.len());
        let mut group_of: HashMap<LNodeId, usize> = HashMap::new();
        for (i, &node) in nodes.iter().enumerate() {
            groups.push(ConstraintGroup::single(node));
            group_of.insert(node, i);
        }
        let mut list: Vec<usize> = (0..nodes.len()).collect();

        // Build the constraints graph
        self.build_constraints_graph(a, &mut groups, &group_of, &list, only_between_normal_nodes);

        // Find violated vertices
        while let Some((first, second)) = self.find_violated_constraint(a, &mut groups, &list) {
            self.handle_violated_constraint(a, &mut groups, &mut list, first, second);
        }

        // Apply the determined order
        nodes.clear();
        for &gid in &list {
            let barycenter = self.barycenter_of(a, &groups, gid);
            for node in groups[gid].nodes.clone() {
                nodes.push(node);
                self.state_mut(a, node).barycenter = barycenter;
            }
        }
    }

    fn build_constraints_graph(
        &mut self,
        a: &LGraphArena,
        groups: &mut [ConstraintGroup],
        group_of: &HashMap<LNodeId, usize>,
        list: &[usize],
        only_between_normal_nodes: bool,
    ) {
        // Reset the constraint fields
        for &gid in list {
            groups[gid].outgoing_constraints = None;
            groups[gid].incoming_constraints_count = 0;
        }

        // Iterate through the vertices, adding the necessary constraints
        let mut last_non_dummy_node: Option<LNodeId> = None;
        for &gid in list {
            // at this stage all groups should consist of a single node
            let node = groups[gid].nodes[0];

            // We may want to skip this
            if only_between_normal_nodes && a.node(node).node_type != NodeType::NORMAL {
                continue;
            }

            // Add the constraints given by the vertex's node
            let successors: Vec<LNodeId> =
                a.node(node).properties.get(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS);
            for successor in successors {
                if !only_between_normal_nodes || a.node(successor).node_type == NodeType::NORMAL {
                    let successor_gid = *group_of
                        .get(&successor)
                        .expect("in-layer successor constraint to node outside the layer");
                    groups[gid].outgoing_constraints.get_or_insert_with(Vec::new).push(successor_gid);
                    groups[successor_gid].incoming_constraints_count += 1;
                }
            }

            // Insert constraints between layout units of consecutive normal nodes
            if !only_between_normal_nodes && a.node(node).node_type == NodeType::NORMAL {
                if let Some(last) = last_non_dummy_node {
                    let last_unit_nodes = self.layout_units.get(&last).cloned().unwrap_or_default();
                    let current_unit_nodes =
                        self.layout_units.get(&node).cloned().unwrap_or_default();
                    for &last_unit_node in &last_unit_nodes {
                        for &current_unit_node in &current_unit_nodes {
                            let last_gid = *group_of.get(&last_unit_node).expect("layout unit node outside layer");
                            let current_gid =
                                *group_of.get(&current_unit_node).expect("layout unit node outside layer");
                            groups[last_gid]
                                .outgoing_constraints
                                .get_or_insert_with(Vec::new)
                                .push(current_gid);
                            groups[current_gid].incoming_constraints_count += 1;
                        }
                    }
                }

                last_non_dummy_node = Some(node);
            }
        }
    }

    /// Returns the two groups in the order
    /// they should appear in.
    fn find_violated_constraint(
        &self,
        a: &LGraphArena,
        groups: &mut [ConstraintGroup],
        list: &[usize],
    ) -> Option<(usize, usize)> {
        let mut active_groups: Option<Vec<usize>> = None;

        // Iterate through the constrained vertices
        for &gid in list {
            groups[gid].incoming_constraints = None;

            // Find sources of the constraint graph to start the constraints check
            if groups[gid].has_outgoing_constraints() && groups[gid].incoming_constraints_count == 0
            {
                active_groups.get_or_insert_with(Vec::new).push(gid);
            }
        }

        // Iterate through the active node groups to find one with violated constraints
        if let Some(mut active) = active_groups {
            while !active.is_empty() {
                let gid = active.remove(0);

                // See if we can find a violated constraint
                if groups[gid].has_incoming_constraints() {
                    let incoming = groups[gid].incoming_constraints.clone().unwrap();
                    let group_barycenter = self.barycenter_of(a, groups, gid).unwrap();
                    for predecessor in incoming {
                        let pred_barycenter = self.barycenter_of(a, groups, predecessor).unwrap();
                        // Compares Double.floatValue()s for equality
                        if pred_barycenter as f32 == group_barycenter as f32 {
                            let pred_index = list.iter().position(|&g| g == predecessor).unwrap();
                            let group_index = list.iter().position(|&g| g == gid).unwrap();
                            if pred_index > group_index {
                                // The predecessor has equal barycenter, but higher index
                                return Some((predecessor, gid));
                            }
                        } else if pred_barycenter > group_barycenter {
                            // The predecessor has greater barycenter and thus also higher index
                            return Some((predecessor, gid));
                        }
                    }
                }

                // No violated constraints; add outgoing constraints to the
                // respective incoming list
                let outgoing = groups[gid].outgoing_constraints.clone().unwrap_or_default();
                for successor in outgoing {
                    let successor_incoming =
                        groups[successor].incoming_constraints.get_or_insert_with(Vec::new);
                    successor_incoming.insert(0, gid);

                    if groups[successor].incoming_constraints_count
                        == groups[successor].incoming_constraints.as_ref().unwrap().len() as i32
                    {
                        active.push(successor);
                    }
                }
            }
        }

        // No violated constraints found
        None
    }

    fn handle_violated_constraint(
        &mut self,
        a: &LGraphArena,
        groups: &mut Vec<ConstraintGroup>,
        list: &mut Vec<usize>,
        first_node_group: usize,
        second_node_group: usize,
    ) {
        // Create a new vertex from the two constraint-violating vertices; this
        // also automatically calculates the new vertex's barycenter value
        let new_node_group = self.merge_groups(a, groups, first_node_group, second_node_group);
        let new_barycenter = self.barycenter_of(a, groups, new_node_group);

        // Iterate through the vertices (ListIterator semantics).
        let mut already_inserted = false;
        let mut i = 0;
        while i < list.len() {
            let gid = list[i];

            if gid == first_node_group || gid == second_node_group {
                // Remove the two node groups with violated constraint from the list
                list.remove(i);
                // (iterator.remove(): do not advance)
            } else if !already_inserted
                && self.barycenter_of(a, groups, gid) > new_barycenter
            {
                // Insert the new node group just before the current element;
                // the current element is examined again in the next iteration
                // (with alreadyInserted == true).
                list.insert(i, new_node_group);
                already_inserted = true;
                i += 1;
            } else {
                if groups[gid].has_outgoing_constraints() {
                    // Check if the vertex has any constraints with the former two vertices
                    let outgoing = groups[gid].outgoing_constraints.as_mut().unwrap();
                    let first_node_group_constraint =
                        remove_first(outgoing, first_node_group);
                    let second_node_group_constraint =
                        remove_first(outgoing, second_node_group);

                    if first_node_group_constraint || second_node_group_constraint {
                        outgoing.push(new_node_group);
                        groups[new_node_group].incoming_constraints_count += 1;
                    }
                }
                i += 1;
            }
        }

        // If we haven't inserted the new node group already, add it to the end
        if !already_inserted {
            list.push(new_node_group);
        }
    }

    /// The merge constructor `ConstraintGroup(ConstraintGroup,
    /// ConstraintGroup)`.
    fn merge_groups(
        &mut self,
        a: &LGraphArena,
        groups: &mut Vec<ConstraintGroup>,
        node_group_1: usize,
        node_group_2: usize,
    ) -> usize {
        // create a combined nodes array
        let mut nodes = groups[node_group_1].nodes.clone();
        nodes.extend(groups[node_group_2].nodes.iter().copied());

        // Add constraints, taking care not to add any constraints to vertex1
        // or vertex2 and to decrement the incoming constraints count of those
        // that are successors to both
        let mut outgoing_constraints: Option<Vec<usize>> = None;
        if groups[node_group_1].outgoing_constraints.is_some() {
            let mut merged = groups[node_group_1].outgoing_constraints.clone().unwrap();
            remove_first(&mut merged, node_group_2);
            if let Some(second_outgoing) = groups[node_group_2].outgoing_constraints.clone() {
                for candidate in second_outgoing {
                    if candidate == node_group_1 {
                        continue;
                    } else if merged.contains(&candidate) {
                        // The candidate was in both vertices' successor list
                        groups[candidate].incoming_constraints_count -= 1;
                    } else {
                        merged.push(candidate);
                    }
                }
            }
            outgoing_constraints = Some(merged);
        } else if groups[node_group_2].outgoing_constraints.is_some() {
            let mut merged = groups[node_group_2].outgoing_constraints.clone().unwrap();
            remove_first(&mut merged, node_group_1);
            outgoing_constraints = Some(merged);
        }

        let summed_weight = groups[node_group_1].summed_weight + groups[node_group_2].summed_weight;
        let degree = groups[node_group_1].degree + groups[node_group_2].degree;

        let barycenter_1 = self.barycenter_of(a, groups, node_group_1);
        let barycenter_2 = self.barycenter_of(a, groups, node_group_2);

        let new_gid = groups.len();
        groups.push(ConstraintGroup {
            summed_weight,
            degree,
            nodes,
            outgoing_constraints,
            incoming_constraints: None,
            incoming_constraints_count: 0,
        });

        let new_barycenter: Option<f64> = if degree > 0 {
            Some(summed_weight / degree as f64)
        } else if let (Some(b1), Some(b2)) = (barycenter_1, barycenter_2) {
            Some((b1 + b2) / 2.0)
        } else if barycenter_1.is_some() {
            barycenter_1
        } else if barycenter_2.is_some() {
            barycenter_2
        } else {
            None
        };
        if new_barycenter.is_some() {
            // setBarycenter: assign to the states of all contained nodes
            for node in groups[new_gid].nodes.clone() {
                self.state_mut(a, node).barycenter = new_barycenter;
            }
        }

        new_gid
    }

    // ------------------------------------------------------------- utilities

    /// `ConstraintGroup.getBarycenter()`: state of the group's first node.
    fn barycenter_of(&self, a: &LGraphArena, groups: &[ConstraintGroup], gid: usize) -> Option<f64> {
        self.state_of(a, groups[gid].nodes[0]).barycenter
    }

    fn state_of(&self, a: &LGraphArena, node: LNodeId) -> &BarycenterState {
        let layer = a.node(node).layer.unwrap();
        &self.barycenter_states[a.layer(layer).id as usize][a.node(node).id as usize]
    }

    fn state_mut(&mut self, a: &LGraphArena, node: LNodeId) -> &mut BarycenterState {
        let layer = a.node(node).layer.unwrap();
        &mut self.barycenter_states[a.layer(layer).id as usize][a.node(node).id as usize]
    }
}

/// `List.remove(Object)`: removes the first occurrence, returns whether the
/// list contained the element.
fn remove_first(list: &mut Vec<usize>, value: usize) -> bool {
    if let Some(pos) = list.iter().position(|&v| v == value) {
        list.remove(pos);
        true
    } else {
        false
    }
}
