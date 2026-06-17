//!
//! Unlike the other strategies this one minimises total edge length rather than
//! width, by modelling the constraint graph as a network-simplex problem. It
//! mutates port positions and group offsets of "same edge" vertical segments,
//! hence it needs access to the [`LGraphArena`].

use crate::alg_common::compaction::one_dimensional_compactor::OneDimensionalCompactor;
use crate::alg_common::compaction::{CNodeId, CNodeOrigin};
use crate::alg_common::networksimplex::{NGraph, NNodeId, NetworkSimplex};
use crate::core::options::PortSide;

use crate::alg_layered::graph::{LGraphArena, LNodeId, LPortId};

use super::transformer::LGraphToCGraphTransformer;

const SEPARATION_WEIGHT: f64 = 1.0;
const EDGE_WEIGHT: f64 = 100.0;

/// Snapshot of the spacing data the network simplex needs (mirrors the special
/// spacings handler's horizontal spacing + the "same edge" predicate).
pub trait NsSpacings {
    fn horizontal_spacing(&self, cg: &crate::alg_common::compaction::CGraph, n1: CNodeId, n2: CNodeId)
        -> f64;
    fn is_vertical_segments_of_same_edge(
        &self,
        cg: &crate::alg_common::compaction::CGraph,
        n1: CNodeId,
        n2: CNodeId,
    ) -> bool;
}

/// Runs the network-simplex-based compaction. Mirrors `compact()`: constraints
/// must already be calculated (LEFT direction) by the caller.
pub fn compact(
    odc: &mut OneDimensionalCompactor,
    a: &mut LGraphArena,
    transformer: &LGraphToCGraphTransformer,
    spacings: &dyn NsSpacings,
) {
    let mut ng = NGraph::new();

    // one network-simplex node per CGroup
    let group_count = odc.cgraph.cgroups.len();
    let mut n_nodes: Vec<NNodeId> = Vec::with_capacity(group_count);
    for (index, g) in odc.cgraph.cgroups.iter_mut().enumerate() {
        g.id = index as i32;
        let nn = ng.add_node();
        n_nodes.push(nn);
    }

    // #2 separation constraints
    add_separation_constraints(&mut ng, &mut n_nodes, odc, a, transformer, spacings);

    // #3 original edges for edge-length minimization
    add_edge_constraints(&mut ng, &n_nodes, odc, a, transformer);

    // #4 make connected
    add_artificial_source_node(&mut ng);

    // #5 execute
    NetworkSimplex::for_graph(&mut ng).execute();

    // #6 apply positions
    for cnode in &mut odc.cgraph.cnodes {
        let g = cnode.cgroup.unwrap();
        cnode.hitbox.x = ng.node(n_nodes[g]).layer as f64 + cnode.cgroup_offset.x;
    }
}

#[allow(clippy::too_many_arguments)]
fn add_separation_constraints(
    ng: &mut NGraph,
    n_nodes: &mut [NNodeId],
    odc: &mut OneDimensionalCompactor,
    a: &mut LGraphArena,
    transformer: &LGraphToCGraphTransformer,
    spacings: &dyn NsSpacings,
) {
    let count = odc.cgraph.cnodes.len();
    for c in 0..count {
        let constraints = odc.cgraph.cnodes[c].constraints.clone();
        for inc in constraints {
            let c_group = odc.cgraph.cnodes[c].cgroup.unwrap();
            let inc_group = odc.cgraph.cnodes[inc].cgroup.unwrap();
            if c_group == inc_group {
                continue;
            }

            // horizontal direction (EDGE_LENGTH only runs horizontally here)
            let spacing = spacings.horizontal_spacing(&odc.cgraph, c, inc);

            let delta = odc.cgraph.cnodes[c].cgroup_offset.x + odc.cgraph.cnodes[c].hitbox.width
                + spacing
                - odc.cgraph.cnodes[inc].cgroup_offset.x;
            let delta = delta.ceil();
            let delta = delta.max(0.0);

            if !spacings.is_vertical_segments_of_same_edge(&odc.cgraph, c, inc) {
                let mut weight = SEPARATION_WEIGHT;
                let c_is_vs = matches!(odc.cgraph.cnodes[c].origin, CNodeOrigin::VerticalSegment(_));
                let c_is_ln = matches!(odc.cgraph.cnodes[c].origin, CNodeOrigin::LNode(_));
                let inc_is_vs =
                    matches!(odc.cgraph.cnodes[inc].origin, CNodeOrigin::VerticalSegment(_));
                let inc_is_ln = matches!(odc.cgraph.cnodes[inc].origin, CNodeOrigin::LNode(_));
                if (c_is_vs && inc_is_ln) || (inc_is_vs && c_is_ln) {
                    weight = 2.0;
                }
                ng.add_edge(n_nodes[c_group], n_nodes[inc_group], weight, delta as i32);
            } else {
                // helper node to allow reordering of same-edge vertical segments
                let helper = ng.add_node();
                let off_delta = (odc.cgraph.cnodes[inc].cgroup_offset.x
                    - odc.cgraph.cnodes[c].cgroup_offset.x)
                    .ceil();
                let mut adjust = off_delta
                    - (odc.cgraph.cnodes[inc].cgroup_offset.x - odc.cgraph.cnodes[c].cgroup_offset.x);

                let mut port = segment_a_port(odc, transformer, c);
                let mut alter_offset = c;
                if port.is_none() {
                    port = segment_a_port(odc, transformer, inc);
                    adjust = -adjust;
                    alter_offset = inc;
                }

                if let Some(p) = port {
                    odc.cgraph.cnodes[alter_offset].cgroup_offset.x -= adjust;
                    a.port_mut(p).pos.x -= adjust;
                }

                ng.add_edge(
                    helper,
                    n_nodes[c_group],
                    SEPARATION_WEIGHT,
                    off_delta.max(0.0) as i32,
                );
                ng.add_edge(
                    helper,
                    n_nodes[inc_group],
                    SEPARATION_WEIGHT,
                    (-off_delta).max(0.0) as i32,
                );
            }
        }
    }

    // n_nodes grew via helper nodes; n_nodes length stays for groups only. The
    // helper node ids are not indexed by group, so nothing else to update.
    let _ = n_nodes;
}

/// The `aPort` of the vertical segment represented by CNode `c`, if any.
fn segment_a_port(
    odc: &OneDimensionalCompactor,
    transformer: &LGraphToCGraphTransformer,
    c: CNodeId,
) -> Option<LPortId> {
    match odc.cgraph.cnodes[c].origin {
        CNodeOrigin::VerticalSegment(vs) => transformer.segments[vs as usize].a_port,
        _ => None,
    }
}

fn add_edge_constraints(
    ng: &mut NGraph,
    n_nodes: &[NNodeId],
    odc: &mut OneDimensionalCompactor,
    a: &LGraphArena,
    transformer: &LGraphToCGraphTransformer,
) {
    // map LNode -> CNode and LEdge -> list of vertical-segment CNodes
    let count = odc.cgraph.cnodes.len();
    let mut l_node_cnode: Vec<Option<CNodeId>> = vec![None; a.nodes.len()];
    // LEdge -> Vec<CNode> (vertical segments representing it)
    let mut l_edge_cnodes: Vec<(crate::alg_layered::graph::LEdgeId, CNodeId)> = Vec::new();
    for c in 0..count {
        match odc.cgraph.cnodes[c].origin {
            CNodeOrigin::LNode(n) => l_node_cnode[n as usize] = Some(c),
            CNodeOrigin::VerticalSegment(vs) => {
                for &e in &transformer.segments[vs as usize].represented_ledges {
                    l_edge_cnodes.push((e, c));
                }
            }
            CNodeOrigin::None => {}
        }
    }

    for c in 0..count {
        let l_node = match odc.cgraph.cnodes[c].origin {
            CNodeOrigin::LNode(n) => LNodeId(n),
            _ => continue,
        };
        for l_edge in a.node_outgoing_edges(l_node) {
            if a.edge_is_self_loop(l_edge) {
                continue;
            }
            let src_port = a.edge(l_edge).source.unwrap();
            let tgt_port = a.edge(l_edge).target.unwrap();
            let src_side = a.port(src_port).side;
            let tgt_side = a.port(tgt_port).side;

            // both n/s? skip (handled via separation), else pull node close
            if is_north_south(src_side) && is_north_south(tgt_side) {
                continue;
            }

            let target_node = a.port(tgt_port).node.unwrap();
            let target = match l_node_cnode[target_node.index()] {
                Some(t) => t,
                None => continue,
            };
            let c_group = odc.cgraph.cnodes[c].cgroup.unwrap();
            let t_group = odc.cgraph.cnodes[target].cgroup.unwrap();
            ng.add_edge(n_nodes[c_group], n_nodes[t_group], EDGE_WEIGHT, 0);

            // inverted ports: keep vertical segments close to the node
            if src_side == PortSide::WEST && is_output(a, src_port) {
                for &(e, n) in &l_edge_cnodes {
                    if e == l_edge && odc.cgraph.cnodes[n].hitbox.x < odc.cgraph.cnodes[c].hitbox.x {
                        let ng2 = odc.cgraph.cnodes[n].cgroup.unwrap();
                        if n_nodes[ng2] != n_nodes[c_group] {
                            ng.add_edge(n_nodes[ng2], n_nodes[c_group], EDGE_WEIGHT, 1);
                        }
                    }
                }
            }
            if tgt_side == PortSide::EAST && is_input(a, tgt_port) {
                for &(e, n) in &l_edge_cnodes {
                    if e == l_edge && odc.cgraph.cnodes[n].hitbox.x > odc.cgraph.cnodes[c].hitbox.x {
                        let ng2 = odc.cgraph.cnodes[n].cgroup.unwrap();
                        if n_nodes[c_group] != n_nodes[ng2] {
                            ng.add_edge(n_nodes[c_group], n_nodes[ng2], EDGE_WEIGHT, 1);
                        }
                    }
                }
            }
        }
    }
}

fn add_artificial_source_node(ng: &mut NGraph) {
    let sources: Vec<NNodeId> = ng
        .nodes
        .iter()
        .copied()
        .filter(|&n| ng.node(n).incoming_edges.is_empty())
        .collect();
    if sources.len() > 1 {
        let dummy = ng.add_node();
        for src in sources {
            ng.add_edge(dummy, src, 0.0, 1);
        }
    }
}

fn is_north_south(side: PortSide) -> bool {
    side == PortSide::NORTH || side == PortSide::SOUTH
}

/// The output predicate: the port has outgoing but no incoming edges.
fn is_output(a: &LGraphArena, port: LPortId) -> bool {
    let p = a.port(port);
    !p.outgoing_edges.is_empty() && p.incoming_edges.is_empty()
}

/// The input predicate: the port has incoming but no outgoing edges.
fn is_input(a: &LGraphArena, port: LPortId) -> bool {
    let p = a.port(port);
    !p.incoming_edges.is_empty() && p.outgoing_edges.is_empty()
}
