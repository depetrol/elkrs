//!
//! Applies additional horizontal compaction to an already-routed graph by
//! transforming it into a one-dimensional constraint graph, compacting it in
//! the configured direction(s), and transferring the positions back.

use crate::core::options::Direction;

use crate::alg_common::compaction::one_dimensional_compactor::{
    OneDimensionalCompactor, QuadraticConstraintCalculation,
};
use crate::alg_common::compaction::{
    CGraph, CNodeId, CNodeOrigin, LockFun, Quadruplet, SpacingsHandler,
};

use crate::alg_layered::compaction::edge_aware_scanline::EdgeAwareScanlineConstraintCalculation;
use crate::alg_layered::compaction::transformer::LGraphToCGraphTransformer;
use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::options_gen::{self as lopts, ConstraintCalculationStrategy, GraphCompactionStrategy};
use crate::alg_layered::spacings;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let strategy = a.graph(graph).properties.get(&lopts::COMPACTION_POST_COMPACTION_STRATEGY);
    if strategy == GraphCompactionStrategy::NONE {
        return Ok(());
    }

    // transform the layered graph into a CGraph
    let mut transformer = LGraphToCGraphTransformer::new();
    let cgraph = transformer.transform(a, graph);

    // snapshot the data needed by the spacing handler and constraint algorithm
    let handler = SpecialSpacingsHandler::new(a, graph, &transformer, &cgraph);
    let seg_ignore: Vec<Quadruplet> =
        transformer.segments.iter().map(|s| s.ignore_spacing).collect();

    let mut odc = OneDimensionalCompactor::new(cgraph);
    odc.set_spacings_handler(Box::new(handler.clone()));

    // select constraint algorithm
    let constraints = a.graph(graph).properties.get(&lopts::COMPACTION_POST_COMPACTION_CONSTRAINTS);
    match constraints {
        ConstraintCalculationStrategy::SCANLINE => {
            let alg = EdgeAwareScanlineConstraintCalculation::new(a, graph, seg_ignore);
            odc.set_constraint_algorithm(Box::new(alg));
        }
        ConstraintCalculationStrategy::QUADRATIC => {
            odc.set_constraint_algorithm(Box::new(QuadraticConstraintCalculation));
        }
    }

    // build the per-CNode lock map for the connection-locking strategy
    let lock_map = transformer.lock_map.clone();

    // select compaction strategy
    match strategy {
        GraphCompactionStrategy::LEFT => {
            odc.compact();
        }
        GraphCompactionStrategy::RIGHT => {
            odc.change_direction(Direction::RIGHT);
            odc.compact();
        }
        GraphCompactionStrategy::LEFT_RIGHT_CONSTRAINT_LOCKING => {
            odc.compact();
            odc.change_direction(Direction::RIGHT);
            let f: LockFun<'static> = Box::new(|cg: &CGraph, node: CNodeId, _dir: Direction| {
                let g = cg.cnodes[node].cgroup.unwrap();
                cg.cgroups[g].out_degree_real == 0
            });
            odc.set_lock_function(Some(f));
            odc.compact();
        }
        GraphCompactionStrategy::LEFT_RIGHT_CONNECTION_LOCKING => {
            odc.compact();
            odc.change_direction(Direction::RIGHT);
            let f: LockFun<'static> =
                Box::new(move |_cg: &CGraph, node: CNodeId, dir: Direction| {
                    lock_map.get(node).map(|q| q.get(dir)).unwrap_or(false)
                });
            odc.set_lock_function(Some(f));
            odc.compact();
        }
        GraphCompactionStrategy::EDGE_LENGTH => {
            // Drive the network-simplex-based compaction externally so it can
            // access the surrounding LGraph (it mutates port positions).
            odc.prepare_external_compaction();
            crate::alg_layered::compaction::network_simplex_compaction::compact(
                &mut odc,
                a,
                &transformer,
                &handler,
            );
        }
        _ => {}
    }

    // back to LEFT orientation
    odc.finish();

    // apply the compacted positions to the LGraph
    transformer.apply_layout(a, &odc.cgraph);

    Ok(())
}

/// `getLNodeOrNull`.
fn l_node_or_null(origin: CNodeOrigin) -> Option<LNodeId> {
    match origin {
        CNodeOrigin::LNode(n) => Some(LNodeId(n)),
        _ => None,
    }
}

/// The special `ISpacingsHandler` for LGraphs.
#[derive(Clone)]
struct SpecialSpacingsHandler {
    /// per-LNode node type.
    node_type: Vec<NodeType>,
    /// per-segment represented edges (for the same-edge test).
    seg_edges: Vec<Vec<LEdgeId>>,
    /// spacing-by-type matrices (indexed [NodeType][NodeType]); NaN where the
    /// pair never occurs.
    horiz: [[f64; 8]; 8],
    vert: [[f64; 8]; 8],
}

impl SpecialSpacingsHandler {
    fn new(
        a: &LGraphArena,
        graph: LGraphId,
        transformer: &LGraphToCGraphTransformer,
        _cgraph: &CGraph,
    ) -> Self {
        let mut node_type = vec![NodeType::NORMAL; a.nodes.len()];
        for i in 0..a.nodes.len() {
            node_type[i] = a.node(LNodeId(i as u32)).node_type;
        }
        let seg_edges = transformer
            .segments
            .iter()
            .map(|s| s.represented_ledges.clone())
            .collect();

        // precompute spacing tables for the types that actually occur, plus
        // LONG_EDGE (the fallback type for vertical segments).
        let mut present = [false; 8];
        present[NodeType::LONG_EDGE as usize] = true;
        for &t in &node_type {
            present[t as usize] = true;
        }
        let nan = f64::NAN;
        let mut horiz = [[nan; 8]; 8];
        let mut vert = [[nan; 8]; 8];
        for t1 in 0..8 {
            if !present[t1] {
                continue;
            }
            for t2 in 0..8 {
                if !present[t2] {
                    continue;
                }
                let nt1 = node_type_from_index(t1);
                let nt2 = node_type_from_index(t2);
                // Some type-pair mappings are only queried for pairs that
                // actually become adjacent; here the table is precomputed, so
                // absent pairs stay NaN (never legitimately read).
                horiz[t1][t2] = spacings::try_horizontal_spacing_by_type(a, graph, nt1, nt2)
                    .unwrap_or(f64::NAN);
                vert[t1][t2] = spacings::try_vertical_spacing_by_type(a, graph, nt1, nt2)
                    .unwrap_or(f64::NAN);
            }
        }

        SpecialSpacingsHandler {
            node_type,
            seg_edges,
            horiz,
            vert,
        }
    }

    /// `isVerticalSegmentsOfSameEdge`.
    fn is_vertical_segments_of_same_edge(&self, cg: &CGraph, n1: CNodeId, n2: CNodeId) -> bool {
        let v1 = match cg.cnodes[n1].origin {
            CNodeOrigin::VerticalSegment(v) => v as usize,
            _ => return false,
        };
        let v2 = match cg.cnodes[n2].origin {
            CNodeOrigin::VerticalSegment(v) => v as usize,
            _ => return false,
        };
        // not disjoint
        self.seg_edges[v1].iter().any(|e| self.seg_edges[v2].contains(e))
    }

    fn type_or_long_edge(&self, origin: CNodeOrigin) -> NodeType {
        match l_node_or_null(origin) {
            Some(n) => self.node_type[n.index()],
            None => NodeType::LONG_EDGE,
        }
    }
}

impl SpacingsHandler for SpecialSpacingsHandler {
    fn horizontal_spacing(&self, cg: &CGraph, n1: CNodeId, n2: CNodeId) -> f64 {
        if self.is_vertical_segments_of_same_edge(cg, n1, n2) {
            return 0.0;
        }
        let node1 = l_node_or_null(cg.cnodes[n1].origin);
        let node2 = l_node_or_null(cg.cnodes[n2].origin);
        let ext = |n: Option<LNodeId>| {
            n.map(|x| self.node_type[x.index()] == NodeType::EXTERNAL_PORT).unwrap_or(false)
        };
        if ext(node1) || ext(node2) {
            return 0.0;
        }
        let t1 = self.type_or_long_edge(cg.cnodes[n1].origin);
        let t2 = self.type_or_long_edge(cg.cnodes[n2].origin);
        self.horiz[t1 as usize][t2 as usize]
    }

    fn vertical_spacing(&self, cg: &CGraph, n1: CNodeId, n2: CNodeId) -> f64 {
        if self.is_vertical_segments_of_same_edge(cg, n1, n2) {
            return 1.0;
        }
        let t1 = self.type_or_long_edge(cg.cnodes[n1].origin);
        let t2 = self.type_or_long_edge(cg.cnodes[n2].origin);
        self.vert[t1 as usize][t2 as usize]
    }
}

impl crate::alg_layered::compaction::network_simplex_compaction::NsSpacings for SpecialSpacingsHandler {
    fn horizontal_spacing(&self, cg: &CGraph, n1: CNodeId, n2: CNodeId) -> f64 {
        SpacingsHandler::horizontal_spacing(self, cg, n1, n2)
    }
    fn is_vertical_segments_of_same_edge(&self, cg: &CGraph, n1: CNodeId, n2: CNodeId) -> bool {
        SpecialSpacingsHandler::is_vertical_segments_of_same_edge(self, cg, n1, n2)
    }
}

fn node_type_from_index(i: usize) -> NodeType {
    match i {
        0 => NodeType::NORMAL,
        1 => NodeType::LONG_EDGE,
        2 => NodeType::EXTERNAL_PORT,
        3 => NodeType::NORTH_SOUTH_PORT,
        4 => NodeType::LABEL,
        5 => NodeType::BREAKING_POINT,
        6 => NodeType::PLACEHOLDER,
        7 => NodeType::NONSHIFTING_PLACEHOLDER,
        _ => unreachable!(),
    }
}
