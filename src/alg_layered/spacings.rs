//! Spacing lookups
//! between node-type pairs, with individual overrides.

use crate::graph::properties::{JavaCloneable, PropValue, Property};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::options_gen as lopts;

type SpacingProp = &'static Property<f64>;

/// Spacing property tables indexed by `[NodeType][NodeType]`; mirrors the
/// `precalculateNodeTypeSpacings` assignments.
struct Tables {
    horizontal: [[Option<SpacingProp>; 8]; 8],
    vertical: [[Option<SpacingProp>; 8]; 8],
}

fn tables() -> &'static Tables {
    static TABLES: std::sync::OnceLock<Tables> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        let mut t = Tables {
            horizontal: [[None; 8]; 8],
            vertical: [[None; 8]; 8],
        };
        // self-spacing with horizontal variant
        let mut same_hv = |nt: NodeType, vert: SpacingProp, horz: SpacingProp| {
            t.vertical[nt as usize][nt as usize] = Some(vert);
            t.horizontal[nt as usize][nt as usize] = Some(horz);
        };
        same_hv(
            NodeType::NORMAL,
            &lopts::SPACING_NODE_NODE,
            &lopts::SPACING_NODE_NODE_BETWEEN_LAYERS,
        );
        same_hv(
            NodeType::LONG_EDGE,
            &lopts::SPACING_EDGE_EDGE,
            &lopts::SPACING_EDGE_EDGE_BETWEEN_LAYERS,
        );
        same_hv(
            NodeType::LABEL,
            &lopts::SPACING_EDGE_EDGE,
            &lopts::SPACING_EDGE_EDGE,
        );
        same_hv(
            NodeType::BREAKING_POINT,
            &lopts::SPACING_EDGE_EDGE,
            &lopts::SPACING_EDGE_EDGE_BETWEEN_LAYERS,
        );

        // self-spacing, vertical only
        let mut same_v = |nt: NodeType, vert: SpacingProp| {
            t.vertical[nt as usize][nt as usize] = Some(vert);
        };
        same_v(NodeType::NORTH_SOUTH_PORT, &lopts::SPACING_EDGE_EDGE);
        same_v(NodeType::EXTERNAL_PORT, &lopts::SPACING_PORT_PORT);

        // pair spacing with horizontal variant
        let mut pair_hv = |n1: NodeType, n2: NodeType, vert: SpacingProp, horz: SpacingProp| {
            t.vertical[n1 as usize][n2 as usize] = Some(vert);
            t.vertical[n2 as usize][n1 as usize] = Some(vert);
            t.horizontal[n1 as usize][n2 as usize] = Some(horz);
            t.horizontal[n2 as usize][n1 as usize] = Some(horz);
        };
        pair_hv(
            NodeType::NORMAL,
            NodeType::LONG_EDGE,
            &lopts::SPACING_EDGE_NODE,
            &lopts::SPACING_EDGE_NODE_BETWEEN_LAYERS,
        );
        pair_hv(
            NodeType::NORMAL,
            NodeType::LABEL,
            &lopts::SPACING_NODE_NODE,
            &lopts::SPACING_NODE_NODE_BETWEEN_LAYERS,
        );
        pair_hv(
            NodeType::LONG_EDGE,
            NodeType::LABEL,
            &lopts::SPACING_EDGE_NODE,
            &lopts::SPACING_EDGE_NODE_BETWEEN_LAYERS,
        );
        pair_hv(
            NodeType::EXTERNAL_PORT,
            NodeType::LABEL,
            &lopts::SPACING_LABEL_PORT_VERTICAL,
            &lopts::SPACING_LABEL_PORT_HORIZONTAL,
        );
        pair_hv(
            NodeType::BREAKING_POINT,
            NodeType::NORMAL,
            &lopts::SPACING_EDGE_NODE,
            &lopts::SPACING_EDGE_NODE_BETWEEN_LAYERS,
        );
        pair_hv(
            NodeType::BREAKING_POINT,
            NodeType::LABEL,
            &lopts::SPACING_EDGE_NODE,
            &lopts::SPACING_EDGE_NODE_BETWEEN_LAYERS,
        );
        pair_hv(
            NodeType::BREAKING_POINT,
            NodeType::LONG_EDGE,
            &lopts::SPACING_EDGE_NODE,
            &lopts::SPACING_EDGE_NODE_BETWEEN_LAYERS,
        );

        // pair spacing, vertical only
        let mut pair_v = |n1: NodeType, n2: NodeType, vert: SpacingProp| {
            t.vertical[n1 as usize][n2 as usize] = Some(vert);
            t.vertical[n2 as usize][n1 as usize] = Some(vert);
        };
        pair_v(NodeType::NORMAL, NodeType::NORTH_SOUTH_PORT, &lopts::SPACING_EDGE_NODE);
        pair_v(NodeType::NORMAL, NodeType::EXTERNAL_PORT, &lopts::SPACING_EDGE_NODE);
        pair_v(NodeType::LONG_EDGE, NodeType::NORTH_SOUTH_PORT, &lopts::SPACING_EDGE_EDGE);
        pair_v(NodeType::LONG_EDGE, NodeType::EXTERNAL_PORT, &lopts::SPACING_EDGE_EDGE);
        pair_v(NodeType::NORTH_SOUTH_PORT, NodeType::EXTERNAL_PORT, &lopts::SPACING_EDGE_EDGE);
        pair_v(NodeType::NORTH_SOUTH_PORT, NodeType::LABEL, &lopts::SPACING_LABEL_NODE);

        t
    })
}

pub fn get_individual_or_default<T: PropValue + Clone + JavaCloneable>(
    a: &LGraphArena,
    node: LNodeId,
    property: &Property<T>,
) -> Option<T> {
    if a.node(node).properties.has(&lopts::SPACING_INDIVIDUAL) {
        if let Some(individual) = a
            .node(node)
            .properties
            .try_get(&lopts::SPACING_INDIVIDUAL)
        {
            if individual.properties.has(property) {
                if let Some(v) = individual.properties.try_get(property) {
                    return Some(v);
                }
            }
        }
    }
    let graph = a.node_graph(node);
    a.graph(graph).properties.get_opt(property)
}

fn local_spacing(
    a: &LGraphArena,
    n1: LNodeId,
    n2: LNodeId,
    table: &[[Option<SpacingProp>; 8]; 8],
) -> f64 {
    let t1 = a.node(n1).node_type;
    let t2 = a.node(n2).node_type;
    let prop = table[t1 as usize][t2 as usize]
        .unwrap_or_else(|| panic!("unspecified spacing between {t1:?} and {t2:?}"));
    let s1 = get_individual_or_default(a, n1, prop).unwrap_or(0.0);
    let s2 = get_individual_or_default(a, n2, prop).unwrap_or(0.0);
    f64::max(s1, s2)
}

pub fn horizontal_spacing(a: &LGraphArena, n1: LNodeId, n2: LNodeId) -> f64 {
    local_spacing(a, n1, n2, &tables().horizontal)
}

pub fn vertical_spacing(a: &LGraphArena, n1: LNodeId, n2: LNodeId) -> f64 {
    local_spacing(a, n1, n2, &tables().vertical)
}

pub fn horizontal_spacing_by_type(
    a: &LGraphArena,
    graph: LGraphId,
    t1: NodeType,
    t2: NodeType,
) -> f64 {
    let prop = tables().horizontal[t1 as usize][t2 as usize]
        .unwrap_or_else(|| panic!("unspecified spacing between {t1:?} and {t2:?}"));
    a.graph(graph).properties.get(prop)
}

pub fn vertical_spacing_by_type(
    a: &LGraphArena,
    graph: LGraphId,
    t1: NodeType,
    t2: NodeType,
) -> f64 {
    let prop = tables().vertical[t1 as usize][t2 as usize]
        .unwrap_or_else(|| panic!("unspecified spacing between {t1:?} and {t2:?}"));
    a.graph(graph).properties.get(prop)
}

/// Like [`horizontal_spacing_by_type`] but returns `None` when no spacing
/// property is defined for the type pair (the value is only ever queried for
/// pairs that actually occur).
/// Used by the compaction spacing handler, which eagerly precomputes a table.
pub fn try_horizontal_spacing_by_type(
    a: &LGraphArena,
    graph: LGraphId,
    t1: NodeType,
    t2: NodeType,
) -> Option<f64> {
    tables().horizontal[t1 as usize][t2 as usize].map(|prop| a.graph(graph).properties.get(prop))
}

/// Like [`vertical_spacing_by_type`] but returns `None` for an undefined pair.
pub fn try_vertical_spacing_by_type(
    a: &LGraphArena,
    graph: LGraphId,
    t1: NodeType,
    t2: NodeType,
) -> Option<f64> {
    tables().vertical[t1 as usize][t2 as usize].map(|prop| a.graph(graph).properties.get(prop))
}
