//! The Brandes & Köpf node placement
//! phase (extended by ELK to cope with ports, node sizes and node margins).

use std::collections::HashSet;

use indexmap::IndexMap;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::FixedAlignment;

use super::bk_aligned_layout::{BKAlignedLayout, HDirection, VDirection};
use super::bk_aligner;
use super::bk_compactor::BKCompactor;
use super::neighborhood_information::{lid, nid, NeighborhoodInformation};

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // Precalculate some information that we require during the following
    // processes.
    let ni = NeighborhoodInformation::build_for(a, graph);

    // a balanced layout is desired if
    //  a) no specific alignment is set and straight edges are not desired
    //  b) a balanced alignment is enforced
    let align: FixedAlignment = a
        .graph(graph)
        .properties
        .get(&lopts::NODE_PLACEMENT_BK_FIXED_ALIGNMENT);
    let favor_straight_edges: bool = a
        .graph(graph)
        .properties
        .get(&lopts::NODE_PLACEMENT_FAVOR_STRAIGHT_EDGES);
    let produce_balanced_layout = (align == FixedAlignment::NONE && !favor_straight_edges)
        || align == FixedAlignment::BALANCED;

    // Phase which marks type 1 conflicts; no difference between the
    // directions so only one run is required.
    let mut marked_edges: HashSet<LEdgeId> = HashSet::new();
    mark_conflicts(a, graph, &ni, &mut marked_edges);

    // Initialize four layouts which result from the two possible directions
    // respectively.
    let mut layouts: Vec<BKAlignedLayout> = Vec::with_capacity(4);
    match align {
        FixedAlignment::LEFTDOWN => {
            layouts.push(BKAlignedLayout::new(
                ni.node_count,
                Some(VDirection::Down),
                Some(HDirection::Left),
            ));
        }
        FixedAlignment::LEFTUP => {
            layouts.push(BKAlignedLayout::new(
                ni.node_count,
                Some(VDirection::Up),
                Some(HDirection::Left),
            ));
        }
        FixedAlignment::RIGHTDOWN => {
            layouts.push(BKAlignedLayout::new(
                ni.node_count,
                Some(VDirection::Down),
                Some(HDirection::Right),
            ));
        }
        FixedAlignment::RIGHTUP => {
            layouts.push(BKAlignedLayout::new(
                ni.node_count,
                Some(VDirection::Up),
                Some(HDirection::Right),
            ));
        }
        _ => {
            // rightdown, rightup, leftdown, leftup -- in this order
            layouts.push(BKAlignedLayout::new(
                ni.node_count,
                Some(VDirection::Down),
                Some(HDirection::Right),
            ));
            layouts.push(BKAlignedLayout::new(
                ni.node_count,
                Some(VDirection::Up),
                Some(HDirection::Right),
            ));
            layouts.push(BKAlignedLayout::new(
                ni.node_count,
                Some(VDirection::Down),
                Some(HDirection::Left),
            ));
            layouts.push(BKAlignedLayout::new(
                ni.node_count,
                Some(VDirection::Up),
                Some(HDirection::Left),
            ));
        }
    }

    for bal in layouts.iter_mut() {
        // Phase which determines the nodes' memberships in blocks. This
        // happens in four different ways, either from processing the nodes
        // from the first layer to the last or vice versa.
        bk_aligner::vertical_alignment(a, graph, &ni, bal, &marked_edges);

        // Additional phase which is not included in the original
        // Brandes-Koepf Algorithm. It makes sure that the connected ports
        // within a block are aligned to avoid unnecessary bend points. Also,
        // the required size of each block is determined.
        bk_aligner::inside_block_shift(a, graph, bal);
    }

    let mut compactor = BKCompactor::new(a, graph);
    for bal in layouts.iter_mut() {
        // This phase determines the y coordinates of the blocks and thus the
        // vertical coordinates of all nodes.
        compactor.horizontal_compaction(a, graph, &ni, bal);
    }

    // Choose a layout from the four calculated layouts. Layouts that contain
    // errors are skipped. The layout with the smallest size is selected. If
    // more than one smallest layout exists, the first one of the competing
    // layouts is selected.

    // If layout options chose to use the balanced layout, it is calculated
    // and added here. If it is broken for any reason, one of the four other
    // layouts is selected by the given criteria.
    let balanced = if produce_balanced_layout {
        let balanced = create_balanced_layout(a, graph, &layouts, ni.node_count);
        if check_order_constraint(a, graph, &balanced) {
            Some(balanced)
        } else {
            None
        }
    } else {
        None
    };

    // Either if no balanced layout is requested, or, if the balanced layout
    // violates order constraints, pick the one with the smallest height
    let chosen_layout: &BKAlignedLayout = match &balanced {
        Some(balanced) => balanced,
        None => {
            let mut chosen: Option<&BKAlignedLayout> = None;
            for bal in &layouts {
                if check_order_constraint(a, graph, bal) {
                    let better = match chosen {
                        None => true,
                        Some(current) => current.layout_size(a, graph) > bal.layout_size(a, graph),
                    };
                    if better {
                        chosen = Some(bal);
                    }
                }
            }
            // If no layout is correct (which should never happen but is not
            // strictly impossible), the first layout is chosen by default.
            chosen.unwrap_or(&layouts[0])
        }
    };

    // Apply calculated positions to nodes.
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            let i = nid(a, node);
            a.node_mut(node).pos.y =
                chosen_layout.y[i].unwrap() + chosen_layout.inner_shift[i].unwrap();
        }
    }

    Ok(())
}

/////////////////////////////////////////////////////////////////////////////
// Conflict Detection

/// The minimum number of layers we need to have conflicts.
const MIN_LAYERS_FOR_CONFLICTS: usize = 3;

/// This phase of the node placer marks all type 1 and type 2 conflicts.
///
/// Type 1 conflicts happen when a non-inner segment and an inner segment (of
/// a long edge) cross. The markers are later used to solve conflicts in favor
/// of long edges.
fn mark_conflicts(
    a: &LGraphArena,
    graph: LGraphId,
    ni: &NeighborhoodInformation,
    marked_edges: &mut HashSet<LEdgeId>,
) {
    let layers = a.graph(graph).layers.clone();
    let number_of_layers = layers.len();

    // Check if there are enough layers to detect conflicts
    if number_of_layers < MIN_LAYERS_FOR_CONFLICTS {
        return;
    }

    // We'll need the number of nodes in the different layers quite often in
    // this method, so save them up front
    let layer_size: Vec<usize> = layers.iter().map(|&l| a.layer(l).nodes.len()).collect();

    for i in 1..number_of_layers - 1 {
        // The variable naming here follows the notation of the corresponding
        // paper.
        let current_layer = layers[i + 1];
        let current_nodes = &a.layer(current_layer).nodes;

        let mut k_0: i32 = 0;
        let mut l: usize = 0;

        for l_1 in 0..layer_size[i + 1] {
            // In the paper, l and i are indices for the layer and the
            // position in the layer
            let v_l_i = current_nodes[l_1];

            if l_1 == layer_size[i + 1] - 1
                || incident_to_inner_segment(a, ni, v_l_i, (i + 1) as i32, i as i32)
            {
                let mut k_1: i32 = layer_size[i] as i32 - 1;
                if incident_to_inner_segment(a, ni, v_l_i, (i + 1) as i32, i as i32) {
                    k_1 = ni.node_index[nid(a, ni.left_neighbors[nid(a, v_l_i)][0].0)];
                }

                while l <= l_1 {
                    let v_l = current_nodes[l];

                    if !incident_to_inner_segment(a, ni, v_l, (i + 1) as i32, i as i32) {
                        for &(upper_neighbor, upper_edge) in &ni.left_neighbors[nid(a, v_l)] {
                            let k = ni.node_index[nid(a, upper_neighbor)];

                            if k < k_0 || k > k_1 {
                                // The upper neighbor relationship between v_l
                                // and upperNeighbor enforces the existence of
                                // at least one edge between the two nodes
                                marked_edges.insert(upper_edge);
                            }
                        }
                    }

                    l += 1;
                }

                k_0 = k_1;
            }
        }
    }
}

/// Checks whether the given node is part of a long edge between the two given
/// layers (`layer2` is left of, or before, `layer1`).
fn incident_to_inner_segment(
    a: &LGraphArena,
    ni: &NeighborhoodInformation,
    node: LNodeId,
    layer1: i32,
    layer2: i32,
) -> bool {
    if a.node(node).node_type == NodeType::LONG_EDGE {
        for edge in a.node_incoming_edges(node) {
            let source_node = a.edge_source_node(edge);

            if a.node(source_node).node_type == NodeType::LONG_EDGE
                && ni.layer_index[lid(a, a.node(source_node).layer.unwrap())] == layer2
                && ni.layer_index[lid(a, a.node(node).layer.unwrap())] == layer1
            {
                return true;
            }
        }
    }
    false
}

/////////////////////////////////////////////////////////////////////////////
// Layout Balancing

/// Calculates a balanced layout by determining the median of the four
/// layouts. A node's inner shift value is regarded during this process; the
/// balanced layout's own inner shifts are all zero afterwards.
fn create_balanced_layout(
    a: &LGraphArena,
    graph: LGraphId,
    layouts: &[BKAlignedLayout],
    node_count: usize,
) -> BKAlignedLayout {
    let no_of_layouts = layouts.len();
    let mut balanced = BKAlignedLayout::new(node_count, None, None);
    let mut width = vec![0.0f64; no_of_layouts];
    // initialized to the integer min/max bounds
    let mut min = vec![i32::MAX as f64; no_of_layouts];
    let mut max = vec![i32::MIN as f64; no_of_layouts];
    let mut min_width_layout = 0usize;

    // Find the smallest layout
    for (i, bal) in layouts.iter().enumerate() {
        width[i] = bal.layout_size(a, graph);
        if width[min_width_layout] > width[i] {
            min_width_layout = i;
        }

        for &l in &a.graph(graph).layers {
            for &n in &a.layer(l).nodes {
                let node_pos_y = bal.y[nid(a, n)].unwrap() + bal.inner_shift[nid(a, n)].unwrap();
                min[i] = f64::min(min[i], node_pos_y);
                max[i] = f64::max(max[i], node_pos_y + a.node(n).size.y);
            }
        }
    }

    // Find the shift between the smallest and the four layouts
    let mut shift = vec![0.0f64; no_of_layouts];
    for i in 0..no_of_layouts {
        if layouts[i].vdir == Some(VDirection::Down) {
            shift[i] = min[min_width_layout] - min[i];
        } else {
            shift[i] = max[min_width_layout] - max[i];
        }
    }

    // Calculated y-coordinates for a balanced placement
    let mut calculated_ys = vec![0.0f64; no_of_layouts];
    for &layer in &a.graph(graph).layers {
        for &node in &a.layer(layer).nodes {
            let idx = nid(a, node);
            for i in 0..no_of_layouts {
                // it's important to include the innerShift here!
                calculated_ys[i] =
                    layouts[i].y[idx].unwrap() + layouts[i].inner_shift[idx].unwrap() + shift[i];
            }

            calculated_ys.sort_by(f64::total_cmp);
            balanced.y[idx] = Some((calculated_ys[1] + calculated_ys[2]) / 2.0);
            // since we include the inner shift in the calculation of a
            // balanced y coordinate we don't need it any more. Note that
            // after this step no further processing of the graph that would
            // include the inner shift is possible.
            balanced.inner_shift[idx] = Some(0.0);
        }
    }

    balanced
}

/////////////////////////////////////////////////////////////////////////////
// Utility Methods

/// Find an edge between two given nodes, or
/// `None` if there is none.
pub fn get_edge(a: &LGraphArena, source: LNodeId, target: LNodeId) -> Option<LEdgeId> {
    for edge in a.node_connected_edges(source) {
        if a.edge_target_node(edge) == target || a.edge_source_node(edge) == target {
            return Some(edge);
        }
    }
    None
}

/// Finds all blocks of a given layout,
/// mapped from root node to block contents (keys in first-discovery order).
pub fn get_blocks(
    a: &LGraphArena,
    graph: LGraphId,
    bal: &BKAlignedLayout,
) -> IndexMap<LNodeId, Vec<LNodeId>> {
    let mut blocks: IndexMap<LNodeId, Vec<LNodeId>> = IndexMap::new();

    for &layer in &a.graph(graph).layers {
        for &node in &a.layer(layer).nodes {
            let root = bal.root[nid(a, node)].unwrap();
            blocks.entry(root).or_default().push(node);
        }
    }

    blocks
}

/// Checks whether all nodes are placed in the correct order in their layers
/// and do not overlap each other.
fn check_order_constraint(a: &LGraphArena, graph: LGraphId, bal: &BKAlignedLayout) -> bool {
    // Flag indicating whether the layout is feasible or not
    let mut feasible = true;

    // Iterate over the layers
    for &layer in &a.graph(graph).layers {
        // Current Y position in the layer
        let mut pos = f64::NEG_INFINITY;

        // Iterate through the layer's nodes
        for &node in &a.layer(layer).nodes {
            // For the layout to be correct, both the node's top border and
            // its bottom border must be beyond the current position in the
            // layer
            let i = nid(a, node);
            let top = bal.y[i].unwrap() + bal.inner_shift[i].unwrap() - a.node(node).margin.top;
            let bottom = bal.y[i].unwrap()
                + bal.inner_shift[i].unwrap()
                + a.node(node).size.y
                + a.node(node).margin.bottom;

            if top > pos && bottom > pos {
                // Update the position inside the layer
                pos = bottom;
            } else {
                // We've found an overlap
                feasible = false;
                break;
            }
        }

        // Don't bother continuing if we've already determined that the layout
        // is infeasible
        if !feasible {
            break;
        }
    }

    feasible
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alg_layered::graph::LayerId;
    use crate::core::options::PortSide;

    fn new_layer(a: &mut LGraphArena, g: LGraphId) -> LayerId {
        let l = a.create_layer(g);
        a.graph_mut(g).layers.push(l);
        l
    }

    fn new_node(a: &mut LGraphArena, g: LGraphId, layer: LayerId, height: f64) -> LNodeId {
        let n = a.create_node(g);
        a.node_mut(n).size.y = height;
        a.node_set_layer(n, Some(layer));
        n
    }

    fn new_edge(a: &mut LGraphArena, src: LNodeId, tgt: LNodeId) -> LEdgeId {
        let sp = a.create_port();
        a.port_set_node(sp, Some(src));
        a.port_set_side(sp, PortSide::EAST);
        let tp = a.create_port();
        a.port_set_node(tp, Some(tgt));
        a.port_set_side(tp, PortSide::WEST);
        let e = a.create_edge();
        a.edge_set_source(e, Some(sp));
        a.edge_set_target(e, Some(tp));
        e
    }

    /// Test graph (vertical node-node spacing 20, all ports at (0,0)):
    ///
    /// layers:  [a]   [b, c]   [d]
    /// heights: a=30, b=10, c=10, d=30
    /// margins: only b (top=2, bottom=3)
    /// edges:   a->b, a->c, b->d, c->d
    fn build_diamond(a: &mut LGraphArena) -> (LGraphId, [LNodeId; 4]) {
        let g = a.create_graph();
        a.graph(g).properties.set(&lopts::SPACING_NODE_NODE, 20.0);
        let l0 = new_layer(a, g);
        let l1 = new_layer(a, g);
        let l2 = new_layer(a, g);
        let na = new_node(a, g, l0, 30.0);
        let nb = new_node(a, g, l1, 10.0);
        let nc = new_node(a, g, l1, 10.0);
        let nd = new_node(a, g, l2, 30.0);
        a.node_mut(nb).margin.top = 2.0;
        a.node_mut(nb).margin.bottom = 3.0;
        new_edge(a, na, nb);
        new_edge(a, na, nc);
        new_edge(a, nb, nd);
        new_edge(a, nc, nd);
        (g, [na, nb, nc, nd])
    }

    /// Hand trace for the diamond graph, RIGHTDOWN layout
    /// (the layout selected with favorStraightEdges=true since it is the
    /// first of the two smallest layouts; sizes are RD=45, RU=65, LD=45,
    /// LU=65):
    ///
    /// - verticalAlignment groups blocks {a, b, d} (root a) and {c}: b's only
    ///   left neighbor is a; d's median left neighbors are [b, c] and b is
    ///   tried first.
    /// - insideBlockShift: spaceAbove of block a is b's top margin 2, so
    ///   innerShift(a) = innerShift(b) = innerShift(d) = 2, blockSize(a) =
    ///   2 + 30 = 32; innerShift(c) = 0, blockSize(c) = 10.
    /// - horizontalCompaction: y(a) = 0; placing block c against neighbor b:
    ///   y(c) = y(a) + innerShift(b) + size(b) + marginBottom(b) + spacing
    ///          + marginTop(c) - innerShift(c) = 0+2+10+3+20+0-0 = 35.
    ///   (The threshold for c via edge a->c is y(a)+innerShift(a) = 2 and
    ///   does not win the max(35, 2).)
    /// - final positions: pos.y = y + innerShift.
    #[test]
    fn diamond_smallest_layout() {
        let mut a = LGraphArena::new();
        let (g, [na, nb, nc, nd]) = build_diamond(&mut a);
        a.graph(g)
            .properties
            .set(&lopts::NODE_PLACEMENT_FAVOR_STRAIGHT_EDGES, true);

        process(&mut a, g).unwrap();

        assert_eq!(a.node(na).pos.y, 2.0);
        assert_eq!(a.node(nb).pos.y, 2.0);
        assert_eq!(a.node(nc).pos.y, 35.0);
        assert_eq!(a.node(nd).pos.y, 2.0);
    }

    /// Hand trace for the balanced (median) layout of the diamond graph
    /// (favorStraightEdges=false, alignment NONE).
    ///
    /// The four layouts produce node positions (y + innerShift):
    ///   RIGHTDOWN: a=2,  b=2,   c=35, d=2   (min=2,   max=45, size 45)
    ///   RIGHTUP:   a=0,  b=-33, c=0,  d=0   (min=-33, max=30, size 65)
    ///   LEFTDOWN:  a=2,  b=2,   c=35, d=2   (min=2,   max=45, size 45)
    ///   LEFTUP:    a=0,  b=-33, c=0,  d=0   (min=-33, max=30, size 65)
    /// minWidthLayout = 0 (RIGHTDOWN). Shifts: DOWN layouts 0, UP layouts
    /// max[0]-max[i] = 45-30 = 15. Medians of the shifted positions:
    ///   a: sort[2,15,2,15]   -> (2+15)/2  = 8.5
    ///   b: sort[2,-18,2,-18] -> (-18+2)/2 = -8
    ///   c: sort[35,15,35,15] -> (15+35)/2 = 25
    ///   d: same as a = 8.5
    #[test]
    fn diamond_balanced_layout() {
        let mut a = LGraphArena::new();
        let (g, [na, nb, nc, nd]) = build_diamond(&mut a);
        a.graph(g)
            .properties
            .set(&lopts::NODE_PLACEMENT_FAVOR_STRAIGHT_EDGES, false);

        process(&mut a, g).unwrap();

        assert_eq!(a.node(na).pos.y, 8.5);
        assert_eq!(a.node(nb).pos.y, -8.0);
        assert_eq!(a.node(nc).pos.y, 25.0);
        assert_eq!(a.node(nd).pos.y, 8.5);
    }

    /// Hand trace for the fixed LEFTUP layout of the diamond graph: blocks
    /// {d, c, a} (root d, all innerShift 0) and {b} (innerShift 2,
    /// blockSize 15). Block b is placed against lower neighbor c:
    /// y(b) = y(d) + innerShift(c) - marginTop(c) - spacing - marginBottom(b)
    ///        - size(b) - innerShift(b) = 0-0-20-3-10-2 = -35.
    /// (The simple threshold strategy's bound for b via edge b->d is
    /// -innerShift(b) = -2, but UP takes min(-35, -2) = -35.)
    #[test]
    fn diamond_fixed_leftup() {
        let mut a = LGraphArena::new();
        let (g, [na, nb, nc, nd]) = build_diamond(&mut a);
        a.graph(g)
            .properties
            .set(&lopts::NODE_PLACEMENT_BK_FIXED_ALIGNMENT, FixedAlignment::LEFTUP);

        process(&mut a, g).unwrap();

        assert_eq!(a.node(na).pos.y, 0.0);
        assert_eq!(a.node(nb).pos.y, -33.0);
        assert_eq!(a.node(nc).pos.y, 0.0);
        assert_eq!(a.node(nd).pos.y, 0.0);
    }

    /// Exercises the class graph post-processing (placeClasses). Graph:
    ///
    /// layers: [a, c0]  [b, c1]; edge c0->c1; heights a=10, c0=10, b=30,
    /// c1=10; spacing 20; fixed RIGHTDOWN.
    ///
    /// Hand trace: blocks {a}, {c0, c1} (root c0), {b}. placeBlock(a) puts
    /// y(a)=0, sink(a)=a. placeBlock(c0): against a -> same class,
    /// y(c0) = 10+20 = 30; at c1 the neighbor is b whose block is placed
    /// first (y(b)=0, sink(b)=b). sink(c0)=a != sink(b)=b, so a class edge
    /// a->b with requiredSpace = y(c0) - (y(b)+size(b)) - spacing
    /// = 30 - 30 - 20 = -20 is added. placeClasses propagates
    /// classShift(a)=0, classShift(b)=-20, so block b ends up at y(b) = -20.
    #[test]
    fn class_compaction() {
        let mut a = LGraphArena::new();
        let g = a.create_graph();
        a.graph(g).properties.set(&lopts::SPACING_NODE_NODE, 20.0);
        a.graph(g)
            .properties
            .set(&lopts::NODE_PLACEMENT_BK_FIXED_ALIGNMENT, FixedAlignment::RIGHTDOWN);
        let l0 = new_layer(&mut a, g);
        let l1 = new_layer(&mut a, g);
        let na = new_node(&mut a, g, l0, 10.0);
        let nc0 = new_node(&mut a, g, l0, 10.0);
        let nb = new_node(&mut a, g, l1, 30.0);
        let nc1 = new_node(&mut a, g, l1, 10.0);
        new_edge(&mut a, nc0, nc1);

        process(&mut a, g).unwrap();

        assert_eq!(a.node(na).pos.y, 0.0);
        assert_eq!(a.node(nc0).pos.y, 30.0);
        assert_eq!(a.node(nb).pos.y, -20.0);
        assert_eq!(a.node(nc1).pos.y, 30.0);
    }
}
