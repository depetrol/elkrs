//!
//! Extends the common `ScanlineConstraintCalculator` (a plain sweep) with the
//! special spacing handling between LGraph nodes and edge segments. It
//! enlarges hitboxes by half the relevant spacing (minus a small epsilon),
//! runs a sweep, then shrinks them back — repeated for several element
//! classes.

use crate::alg_common::compaction::one_dimensional_compactor::{
    ConstraintCalculationAlgorithm, OneDimensionalCompactor,
};
use crate::alg_common::compaction::{scanline, CNodeId, CNodeOrigin, Quadruplet};
use crate::core::options::EdgeRouting;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::spacings;

const EPSILON: f64 = 0.5;
const SMALL_EPSILON: f64 = 0.01;

/// Per-CNode immutable metadata needed by the edge-aware sweep, indexed by
/// `CNodeId`. Captured before compaction so the constraint algorithm needs no
/// arena access while it owns the compactor borrow.
pub struct EdgeAwareScanlineConstraintCalculation {
    vertical_edge_edge_spacing: f64,
    edge_routing: EdgeRouting,

    /// per-segment ignore-spacing flags, indexed by segment index.
    seg_ignore: Vec<Quadruplet>,
    /// per-LNode `getIndividualOrDefault(SPACING_NODE_NODE)`.
    node_node_spacing: Vec<f64>,
    /// per-LNode `getIndividualOrDefault(SPACING_EDGE_EDGE)`.
    edge_edge_node_spacing: Vec<f64>,
    /// per-LNode node type.
    node_type: Vec<NodeType>,
}

impl EdgeAwareScanlineConstraintCalculation {
    /// Reads the two graph properties and snapshots the per-element spacing
    /// data the sweep relies on.
    pub fn new(a: &LGraphArena, graph: LGraphId, seg_ignore: Vec<Quadruplet>) -> Self {
        let vertical_edge_edge_spacing = a.graph(graph).properties.get(&lopts::SPACING_EDGE_EDGE);
        let edge_routing = a.graph(graph).properties.get(&lopts::EDGE_ROUTING);

        let mut node_node_spacing = vec![0.0; a.nodes.len()];
        let mut edge_edge_node_spacing = vec![0.0; a.nodes.len()];
        let mut node_type = vec![NodeType::NORMAL; a.nodes.len()];
        for i in 0..a.nodes.len() {
            let n = LNodeId(i as u32);
            node_node_spacing[i] =
                spacings::get_individual_or_default(a, n, &lopts::SPACING_NODE_NODE).unwrap_or(0.0);
            edge_edge_node_spacing[i] =
                spacings::get_individual_or_default(a, n, &lopts::SPACING_EDGE_EDGE).unwrap_or(0.0);
            node_type[i] = a.node(n).node_type;
        }

        EdgeAwareScanlineConstraintCalculation {
            vertical_edge_edge_spacing,
            edge_routing,
            seg_ignore,
            node_node_spacing,
            edge_edge_node_spacing,
            node_type,
        }
    }

    /// Alters a CNode's hitbox by `spacing * fac`.
    fn alter_hitbox(&self, c: &mut OneDimensionalCompactor, node: CNodeId, spacing: f64, fac: f64) {
        let delta = spacing * fac;
        match c.cgraph.cnodes[node].origin {
            CNodeOrigin::VerticalSegment(vs) => {
                let ig = self.seg_ignore[vs as usize];
                let hb = &mut c.cgraph.cnodes[node].hitbox;
                if !ig.up {
                    hb.y -= delta + SMALL_EPSILON;
                    hb.height += delta + SMALL_EPSILON;
                } else if !ig.down {
                    hb.height += delta + SMALL_EPSILON;
                }
            }
            CNodeOrigin::LNode(_) => {
                let hb = &mut c.cgraph.cnodes[node].hitbox;
                hb.y -= delta;
                hb.height += 2.0 * delta;
            }
            CNodeOrigin::None => {}
        }
    }

    /// Alters the hitboxes of a group's CNodes for the orthogonal sweep.
    fn alter_grouped_hitbox_orthogonal(
        &self,
        c: &mut OneDimensionalCompactor,
        group: usize,
        spacing: f64,
        fac: f64,
    ) {
        let master = c.cgraph.cgroups[group]
            .master
            .unwrap_or_else(|| c.cgraph.cgroups[group].cnodes[0]);

        self.alter_hitbox(c, master, spacing, fac);
        if c.cgraph.cgroups[group].cnodes.len() == 1 {
            return;
        }

        let delta = spacing * fac;
        let cnodes = c.cgraph.cgroups[group].cnodes.clone();
        for n in cnodes {
            if n != master {
                if let CNodeOrigin::VerticalSegment(vs) = c.cgraph.cnodes[n].origin {
                    let ig = self.seg_ignore[vs as usize];
                    let hb = &mut c.cgraph.cnodes[n].hitbox;
                    if ig.up {
                        hb.y += delta + SMALL_EPSILON;
                        hb.height -= delta + SMALL_EPSILON;
                    } else if ig.down {
                        hb.height -= delta + SMALL_EPSILON;
                    }
                }
            }
        }
    }

    /// Minimum spacing over all CNodes (the `minSpacing` stream).
    fn min_spacing(&self, c: &OneDimensionalCompactor) -> f64 {
        let mut min = f64::INFINITY;
        let mut any = false;
        for cnode in &c.cgraph.cnodes {
            any = true;
            let v = match cnode.origin {
                CNodeOrigin::LNode(n) if self.node_type[n as usize] == NodeType::EXTERNAL_PORT => {
                    f64::INFINITY
                }
                CNodeOrigin::VerticalSegment(_) => {
                    (self.vertical_edge_edge_spacing / 2.0 - EPSILON).max(0.0)
                }
                CNodeOrigin::LNode(n) => {
                    (self.node_node_spacing[n as usize] / 2.0 - EPSILON).max(0.0)
                }
                CNodeOrigin::None => f64::INFINITY,
            };
            min = min.min(v);
        }
        if any {
            min
        } else {
            0.0
        }
    }

    fn calculate_for_orthogonal(&self, c: &mut OneDimensionalCompactor) {
        // -------------------- Vertical Segments --------------------
        let vs_nodes: Vec<CNodeId> = (0..c.cgraph.cnodes.len())
            .filter(|&i| matches!(c.cgraph.cnodes[i].origin, CNodeOrigin::VerticalSegment(_)))
            .collect();
        let spacing = (self.vertical_edge_edge_spacing / 2.0 - EPSILON).max(0.0);
        for &n in &vs_nodes {
            self.alter_hitbox(c, n, spacing, 1.0);
        }
        scanline::sweep(c, |c, n| {
            matches!(c.cgraph.cnodes[n].origin, CNodeOrigin::VerticalSegment(_))
        });
        for &n in &vs_nodes {
            self.alter_hitbox(c, n, spacing, -1.0);
        }

        // -------------------- Nodes --------------------
        let l_nodes: Vec<CNodeId> = (0..c.cgraph.cnodes.len())
            .filter(|&i| matches!(c.cgraph.cnodes[i].origin, CNodeOrigin::LNode(_)))
            .collect();
        // node spacing uses edge-edge, individual-or-default
        let mut node_spacings: Vec<(CNodeId, f64)> = Vec::new();
        for &n in &l_nodes {
            let lnode = match c.cgraph.cnodes[n].origin {
                CNodeOrigin::LNode(idx) => idx as usize,
                _ => unreachable!(),
            };
            // uses SPACING_EDGE_EDGE individual-or-default here.
            let spacing = self.edge_edge_spacing_for(lnode);
            let final_spacing = (spacing / 2.0 - EPSILON).max(0.0);
            self.alter_hitbox(c, n, final_spacing, 1.0);
            node_spacings.push((n, final_spacing));
        }
        scanline::sweep(c, |c, n| {
            matches!(c.cgraph.cnodes[n].origin, CNodeOrigin::LNode(_))
        });
        for (n, final_spacing) in node_spacings {
            self.alter_hitbox(c, n, final_spacing, -1.0);
        }

        // -------------------- Everything --------------------
        let min_spacing = self.min_spacing(c);
        let groups: Vec<usize> = (0..c.cgraph.cgroups.len()).collect();
        for &g in &groups {
            self.alter_grouped_hitbox_orthogonal(c, g, min_spacing, 1.0);
        }
        scanline::sweep(c, |_, _| true);
        for &g in &groups {
            self.alter_grouped_hitbox_orthogonal(c, g, min_spacing, -1.0);
        }
    }

    /// per-LNode individual-or-default SPACING_EDGE_EDGE (used for the node
    /// class sweep).
    fn edge_edge_spacing_for(&self, lnode: usize) -> f64 {
        self.edge_edge_node_spacing[lnode]
    }

    /// Calculates constraints for spline edge routing.
    fn calculate_for_spline(&self, c: &mut OneDimensionalCompactor) {
        // -------------------- Vertical Segments --------------------
        // Some constraints between subsequent vertical segments of the same
        // spline have been precalculated during import. The boxes are not
        // enlarged here since that risks introducing overlaps.
        scanline::sweep(c, |c, n| {
            matches!(c.cgraph.cnodes[n].origin, CNodeOrigin::VerticalSegment(_))
        });

        // -------------------- Everything --------------------
        let min_spacing = self.min_spacing(c);
        let l_nodes: Vec<CNodeId> = (0..c.cgraph.cnodes.len())
            .filter(|&i| matches!(c.cgraph.cnodes[i].origin, CNodeOrigin::LNode(_)))
            .collect();
        for &n in &l_nodes {
            self.alter_hitbox(c, n, min_spacing, 1.0);
        }
        scanline::sweep(c, |_, _| true);
        for &n in &l_nodes {
            self.alter_hitbox(c, n, min_spacing, -1.0);
        }
    }
}

impl ConstraintCalculationAlgorithm for EdgeAwareScanlineConstraintCalculation {
    fn calculate_constraints(&mut self, compactor: &mut OneDimensionalCompactor) {
        match self.edge_routing {
            EdgeRouting::ORTHOGONAL => self.calculate_for_orthogonal(compactor),
            EdgeRouting::SPLINES => self.calculate_for_spline(compactor),
            _ => panic!("Unsupported configuration."),
        }
    }
}
