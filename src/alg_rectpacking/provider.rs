//!
//! The processor pipeline order is fixed (processors in a slot are sorted by
//! their `IntermediateProcessorStrategy` ordinal):
//!
//! 1. `NodeSizeReorderer` (if `orderBySize`)
//! 2. `InteractiveNodeReorderer` (if `interactive`)
//! 3. `MinSizePreProcessor`
//! 4. phase 1: width approximation (greedy / target width)
//! 5. `MinSizePostProcessor`
//! 6. phase 2: packing (compaction / simple / none)
//! 7. phase 3: whitespace elimination (equal / to-aspect-ratio / none)

use crate::core::registry::LayoutProvider;
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::math::{ElkPadding, KVector};

use crate::alg_rectpacking::options::{
    self, PackingStrategy, WhiteSpaceEliminationStrategy, WidthApproximationStrategy,
};
use crate::alg_rectpacking::util::PackArena;
use crate::alg_rectpacking::{p1widthapproximation, p2packing, p3whitespaceelimination};

#[derive(Default)]
pub struct RectPackingLayoutProvider;

impl LayoutProvider for RectPackingLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        let padding: ElkPadding = g.node(layout_node).properties.get(&options::PADDING);
        let fixed_graph_size: bool = g
            .node(layout_node)
            .properties
            .get(&options::NODE_SIZE_FIXED_GRAPH_SIZE);
        let node_node_spacing: f64 =
            g.node(layout_node).properties.get(&options::SPACING_NODE_NODE);
        let try_box: bool = g.node(layout_node).properties.get(&options::TRYBOX);
        let rectangles = g.node(layout_node).children.clone();

        // if requested, compute nodes's dimensions, place node labels, ports,
        // port labels, etc.
        if !g
            .node(layout_node)
            .properties
            .get(&options::OMIT_NODE_MICRO_LAYOUT)
        {
            execute_node_micro_layout(g, layout_node);
        }

        // Check whether regions are stackable and do box layout instead.
        let mut stackable = false;
        if try_box && rectangles.len() >= 3 {
            let h = |g: &ElkGraph, n: NodeId| g.node(n).shape.height;
            let mut region2 = rectangles[0];
            let mut region3 = rectangles[1];
            let mut counter = 0usize;
            while counter + 2 < rectangles.len() {
                let region1 = region2;
                region2 = region3;
                region3 = rectangles[counter + 2];
                if h(g, region1) >= h(g, region2) + h(g, region3) + node_node_spacing
                    || h(g, region3) >= h(g, region1) + h(g, region2) + node_node_spacing
                {
                    stackable = true;
                    break;
                } else {
                    counter += 1;
                }
            }
        } else {
            stackable = true;
        }
        if !stackable {
            // Set priority to invoke box layout.
            let mut priority = rectangles.len() as i32;
            for &elk_node in &rectangles {
                g.node(elk_node).properties.set(&options::PRIORITY, priority);
                priority -= 1;
            }
            let mut box_provider = crate::core::providers::box_layouter::BoxLayoutProvider;
            return box_provider.layout(g, layout_node);
        }

        // Assemble the algorithm (RectPackingLayoutProvider.assembleAlgorithm)
        // and invoke each layout processor.
        let width_approximation_strategy: WidthApproximationStrategy = g
            .node(layout_node)
            .properties
            .get(&options::WIDTH_APPROXIMATION_STRATEGY);
        let packing_strategy: PackingStrategy =
            g.node(layout_node).properties.get(&options::PACKING_STRATEGY);
        let white_space_elimination_strategy: WhiteSpaceEliminationStrategy = g
            .node(layout_node)
            .properties
            .get(&options::WHITE_SPACE_ELIMINATION_STRATEGY);
        let order_by_size: bool = g.node(layout_node).properties.get(&options::ORDER_BY_SIZE);
        let interactive: bool = g.node(layout_node).properties.get(&options::INTERACTIVE);

        // Before phase 1.
        if order_by_size {
            node_size_reorderer(g, layout_node);
        }
        if interactive {
            interactive_node_reorderer(g, layout_node);
        }
        min_size_pre_processor(g, layout_node);

        // Phase 1: width approximation.
        match width_approximation_strategy {
            WidthApproximationStrategy::GREEDY => {
                p1widthapproximation::greedy_width_approximator(g, layout_node);
            }
            WidthApproximationStrategy::TARGET_WIDTH => {
                p1widthapproximation::target_width_width_approximator(g, layout_node)?;
            }
        }

        // Before phase 2.
        min_size_post_processor(g, layout_node);

        // Phase 2: packing.
        let mut arena = PackArena::default();
        let rows = match packing_strategy {
            PackingStrategy::COMPACTION => {
                Some(p2packing::compactor(&mut arena, g, layout_node))
            }
            PackingStrategy::SIMPLE => {
                Some(p2packing::simple_placement(&mut arena, g, layout_node))
            }
            PackingStrategy::NONE => {
                p2packing::no_placement(g, layout_node);
                None
            }
        };

        // Phase 3: whitespace elimination.
        match white_space_elimination_strategy {
            WhiteSpaceEliminationStrategy::EQUAL_BETWEEN_STRUCTURES => {
                p3whitespaceelimination::equal_whitespace_eliminator(
                    &mut arena,
                    g,
                    layout_node,
                    rows.as_deref(),
                )?;
            }
            WhiteSpaceEliminationStrategy::TO_ASPECT_RATIO => {
                p3whitespaceelimination::to_aspectratio_node_expander(
                    &mut arena,
                    g,
                    layout_node,
                    rows.as_deref(),
                )?;
            }
            WhiteSpaceEliminationStrategy::NONE => {}
        }

        // Content alignment
        let mut real_width = 0.0f64;
        let mut real_height = 0.0f64;
        let rectangles = g.node(layout_node).children.clone();
        for &rect in &rectangles {
            let s = &g.node(rect).shape;
            real_width = f64::max(real_width, s.x + s.width);
            real_height = f64::max(real_height, s.y + s.height);
        }

        let drawing_width: f64 = g.node(layout_node).properties.get(&options::DRAWING_WIDTH);
        let drawing_height: f64 = g.node(layout_node).properties.get(&options::DRAWING_HEIGHT);
        crate::core::elkutil::translate_aligned(
            g,
            layout_node,
            KVector::new(drawing_width, drawing_height),
            KVector::new(real_width, real_height),
        );

        // Final touch.
        apply_padding(g, &rectangles, &padding);

        if !fixed_graph_size {
            let drawing_width: f64 = g.node(layout_node).properties.get(&options::DRAWING_WIDTH);
            let drawing_height: f64 =
                g.node(layout_node).properties.get(&options::DRAWING_HEIGHT);
            crate::core::elkutil::resize_node(
                g,
                layout_node,
                drawing_width + padding.horizontal(),
                drawing_height + padding.vertical(),
                false,
                true,
            );
        }

        // Do micro layout again since the whitespace elimination and other
        // things might have changed node sizes.
        if !g
            .node(layout_node)
            .properties
            .get(&options::OMIT_NODE_MICRO_LAYOUT)
        {
            execute_node_micro_layout(g, layout_node);
        }
        Ok(())
    }
}

fn apply_padding(g: &mut ElkGraph, rectangles: &[NodeId], padding: &ElkPadding) {
    for &rect in rectangles {
        let s = &mut g.node_mut(rect).shape;
        let (x, y) = (s.x, s.y);
        s.set_location(x + padding.left, y + padding.top);
    }
}

fn execute_node_micro_layout(g: &mut ElkGraph, layout_node: NodeId) {
    let mut adapter = crate::core::adapters::ElkGraphAdapter::new(g, layout_node);
    crate::alg_common::nodespacing::sort_port_lists(&mut adapter);
    crate::alg_common::nodespacing::calculate_label_and_node_sizes(&mut adapter, |_, _| true);
    crate::alg_common::nodespacing::calculate_node_margins(&mut adapter, false);
}

// ------------------------------------------------------ intermediate processors

/// Sorts the children by height,
/// descending (`NodeSizeComparator`), stably (`ECollections.sort`).
fn node_size_reorderer(g: &mut ElkGraph, graph: NodeId) {
    let mut children = g.node(graph).children.clone();
    children.sort_by(|&node0, &node1| {
        g.node(node1)
            .shape
            .height
            .total_cmp(&g.node(node0).shape.height)
    });
    g.node_mut(graph).children = children;
}

fn interactive_node_reorderer(g: &mut ElkGraph, graph: NodeId) {
    let mut rectangles = g.node(graph).children.clone();
    let mut fixed_nodes: Vec<NodeId> = Vec::new();
    for &elk_node in &rectangles {
        if g.node(elk_node).properties.has(&options::DESIRED_POSITION) {
            fixed_nodes.push(elk_node);
        }
    }
    for &elk_node in &fixed_nodes {
        if let Some(pos) = rectangles.iter().position(|&n| n == elk_node) {
            rectangles.remove(pos);
        }
    }
    java_binary_sort(&mut fixed_nodes, &mut |&a, &b| {
        let position_a: i32 = g.node(a).properties.get(&options::DESIRED_POSITION);
        let position_b: i32 = g.node(b).properties.get(&options::DESIRED_POSITION);
        if position_a == position_b {
            -1
        } else {
            match position_a.cmp(&position_b) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            }
        }
    });
    for &elk_node in &fixed_nodes {
        let position: i32 = g.node(elk_node).properties.get(&options::DESIRED_POSITION);
        let position = std::cmp::min(position as usize, rectangles.len());
        rectangles.insert(position, elk_node);
    }

    for (index, &elk_node) in rectangles.iter().enumerate() {
        g.node(elk_node)
            .properties
            .set(&options::CURRENT_POSITION, index as i32);
    }
    g.node_mut(graph).children = rectangles;
}

fn min_size_pre_processor(g: &mut ElkGraph, graph: NodeId) {
    // Get minimum size based on children.
    let mut min_size = crate::core::elkutil::effective_min_size_constraint_for(g, graph);

    // Get minimum size based on labels.
    if let Some(parent) = g.node(graph).parent {
        // only possible if a parent exists.
        let mut adapter = crate::core::adapters::ElkGraphAdapter::new(g, parent);
        let min_size2 =
            crate::alg_common::nodespacing::process_node_size(&mut adapter, graph, false, true);
        min_size.x = f64::max(min_size.x, min_size2.x);
        min_size.y = f64::max(min_size.y, min_size2.y);
    }
    g.node(graph).properties.set(&options::MIN_WIDTH, min_size.x);
    g.node(graph).properties.set(&options::MIN_HEIGHT, min_size.y);
}

fn min_size_post_processor(g: &mut ElkGraph, graph: NodeId) {
    let target_width: f64 = g.node(graph).properties.get(&options::TARGET_WIDTH);
    let min_width: f64 = g.node(graph).properties.get(&options::MIN_WIDTH);
    g.node(graph)
        .properties
        .set(&options::TARGET_WIDTH, f64::max(target_width, min_width));
}

/// `Arrays.sort(T[], Comparator)` (TimSort) for arrays shorter than
/// `MIN_MERGE` (32): `countRunAndMakeAscending` followed by `binarySort`.
/// `InteractiveNodeReorderer`'s comparator is inconsistent (returns -1 for
/// equal keys), so the exact procedure matters. For 32+ elements TimSort
/// would merge runs; with a consistent comparator the result is identical to
/// this, so the divergence is limited to 32+ nodes with duplicate desired
/// positions.
fn java_binary_sort<T: Copy>(a: &mut [T], c: &mut impl FnMut(&T, &T) -> i32) {
    let hi = a.len();
    if hi < 2 {
        return;
    }
    // countRunAndMakeAscending
    let mut run_hi = 1usize;
    if c(&a[run_hi], &a[0]) < 0 {
        // Descending
        run_hi += 1;
        while run_hi < hi && c(&a[run_hi], &a[run_hi - 1]) < 0 {
            run_hi += 1;
        }
        a[0..run_hi].reverse();
    } else {
        // Ascending
        run_hi += 1;
        while run_hi < hi && c(&a[run_hi], &a[run_hi - 1]) >= 0 {
            run_hi += 1;
        }
    }
    // binarySort(a, lo, hi, lo + initRunLen, c)
    for start in run_hi..hi {
        let pivot = a[start];
        let mut left = 0usize;
        let mut right = start;
        while left < right {
            let mid = (left + right) >> 1;
            if c(&pivot, &a[mid]) < 0 {
                right = mid;
            } else {
                left = mid + 1;
            }
        }
        for i in (left..start).rev() {
            a[i + 1] = a[i];
        }
        a[left] = pivot;
    }
}
