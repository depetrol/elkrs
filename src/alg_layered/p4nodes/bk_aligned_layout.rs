//! All information about a layout
//! in one of the four direction combinations.

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId};
use crate::alg_layered::spacings;

use super::neighborhood_information::{nid, NeighborhoodInformation};

/// Vertical direction enumeration.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VDirection {
    /// Iteration direction top-down.
    Down,
    /// Iteration direction bottom-up.
    Up,
}

/// Horizontal direction enumeration.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HDirection {
    /// Iterating from right to left.
    Right,
    /// Iterating from left to right.
    Left,
}

/// All per-node arrays are indexed by the node's scratch id (`nid`), assigned
/// by `NeighborhoodInformation::build_for`. `None` entries represent
/// `null` entries in the boxed `Double[]`/`LNode[]` arrays.
pub struct BKAlignedLayout {
    /// The root node of each node in a block.
    pub root: Vec<Option<LNodeId>>,
    /// The size of a block.
    pub block_size: Vec<Option<f64>>,
    /// The next node in a block, or the first if the current node is the
    /// last, forming a ring.
    pub align: Vec<Option<LNodeId>>,
    /// The value by which a node must be shifted to stay straight inside a
    /// block.
    pub inner_shift: Vec<Option<f64>>,
    /// The root node of a class, mapped from block root nodes to class root
    /// nodes.
    pub sink: Vec<Option<LNodeId>>,
    /// The value by which a block must be shifted for a more compact
    /// placement.
    pub shift: Vec<Option<f64>>,
    /// The y-coordinate of every node, forming the final layout.
    pub y: Vec<Option<f64>>,
    /// The vertical direction of the current layout (`None` for balanced).
    pub vdir: Option<VDirection>,
    /// The horizontal direction of the current layout (`None` for balanced).
    pub hdir: Option<HDirection>,
    /// Flags blocks, represented by their root node, that are part of a
    /// straightened edge.
    pub su: Vec<bool>,
    /// Flags blocks, represented by their root node, that they are solely
    /// made up of dummy nodes.
    pub od: Vec<bool>,
}

impl BKAlignedLayout {
    /// Basic constructor for a layout.
    pub fn new(node_count: usize, vdir: Option<VDirection>, hdir: Option<HDirection>) -> Self {
        BKAlignedLayout {
            root: vec![None; node_count],
            block_size: vec![None; node_count],
            align: vec![None; node_count],
            inner_shift: vec![None; node_count],
            sink: vec![None; node_count],
            shift: vec![None; node_count],
            y: vec![None; node_count],
            vdir,
            hdir,
            su: vec![false; node_count],
            od: vec![true; node_count],
        }
    }

    /// Calculate the layout size for comparison.
    pub fn layout_size(&self, a: &LGraphArena, graph: LGraphId) -> f64 {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        // The maximal extent of the layout is based on the minimum y
        // coordinate of any node and the maximum y coordinate _plus_ the size
        // of any block (see KIPRA-1426).
        for &layer in &a.graph(graph).layers {
            for &n in &a.layer(layer).nodes {
                let y_min = self.y[nid(a, n)].unwrap();
                let y_max =
                    y_min + self.block_size[nid(a, self.root[nid(a, n)].unwrap())].unwrap();
                min = f64::min(min, y_min);
                max = f64::max(max, y_max);
            }
        }
        max - min
    }

    /// A delta larger than 0 if the `tgt` port has a larger y coordinate than
    /// `src`, and a delta smaller than zero if `src` has the larger one.
    pub fn calculate_delta(&self, a: &LGraphArena, src: LPortId, tgt: LPortId) -> f64 {
        let src_node = a.port(src).node.unwrap();
        let tgt_node = a.port(tgt).node.unwrap();
        let src_pos = self.y[nid(a, src_node)].unwrap()
            + self.inner_shift[nid(a, src_node)].unwrap()
            + a.port(src).pos.y
            + a.port(src).anchor.y;
        let tgt_pos = self.y[nid(a, tgt_node)].unwrap()
            + self.inner_shift[nid(a, tgt_node)].unwrap()
            + a.port(tgt).pos.y
            + a.port(tgt).anchor.y;
        tgt_pos - src_pos
    }

    /// Shifts the y-coordinates of all nodes of the block containing
    /// `root_node` by the specified `delta`.
    pub fn shift_block(&mut self, a: &LGraphArena, root_node: LNodeId, delta: f64) {
        let mut current = root_node;
        loop {
            let i = nid(a, current);
            let new_pos = self.y[i].unwrap() + delta;
            self.y[i] = Some(new_pos);
            current = self.align[i].unwrap();
            if current == root_node {
                break;
            }
        }
    }

    /// Checks whether a block with root node `block_root` can be shifted
    /// upwards by `delta` without overlapping any of the block's nodes'
    /// upper neighbors. Returns a value smaller or equal to `delta`.
    pub fn check_space_above(
        &self,
        a: &LGraphArena,
        block_root: LNodeId,
        delta: f64,
        ni: &NeighborhoodInformation,
    ) -> f64 {
        let mut available_space = delta;
        let root_node = block_root;
        // iterate through the block
        let mut current = root_node;
        loop {
            current = self.align[nid(a, current)].unwrap();
            // get minimum possible position of the current node
            let min_y_current = self.get_min_y(a, current);

            if let Some(neighbor) = self.get_upper_neighbor(a, ni, current) {
                let max_y_neighbor = self.get_max_y(a, neighbor);
                // minimal position at which the current block node could
                // validly be placed
                available_space = f64::min(
                    available_space,
                    min_y_current
                        - (max_y_neighbor + spacings::vertical_spacing(a, current, neighbor)),
                );
            }
            // until we wrap around
            if root_node == current {
                break;
            }
        }
        available_space
    }

    /// Checks whether a block with root node `block_root` can be shifted
    /// downwards by `delta` without overlapping any of the block's nodes'
    /// lower neighbors. Returns a value smaller or equal to `delta`.
    pub fn check_space_below(
        &self,
        a: &LGraphArena,
        block_root: LNodeId,
        delta: f64,
        ni: &NeighborhoodInformation,
    ) -> f64 {
        let mut available_space = delta;
        let root_node = block_root;
        // iterate through the block
        let mut current = root_node;
        loop {
            current = self.align[nid(a, current)].unwrap();
            // get maximum possible position of the current node
            let max_y_current = self.get_max_y(a, current);

            // get the lower neighbor and check its position allows shifting
            if let Some(neighbor) = self.get_lower_neighbor(a, ni, current) {
                let min_y_neighbor = self.get_min_y(a, neighbor);

                // minimal position at which the current block node could
                // validly be placed
                available_space = f64::min(
                    available_space,
                    min_y_neighbor
                        - (max_y_current + spacings::vertical_spacing(a, current, neighbor)),
                );
            }
            // until we wrap around
            if root_node == current {
                break;
            }
        }
        available_space
    }

    /// `y[root[n]] + innerShift[n] - margin.top` (no spacing accounted for).
    pub fn get_min_y(&self, a: &LGraphArena, n: LNodeId) -> f64 {
        let root_node = self.root[nid(a, n)].unwrap();
        self.y[nid(a, root_node)].unwrap() + self.inner_shift[nid(a, n)].unwrap()
            - a.node(n).margin.top
    }

    /// `y[root[n]] + innerShift[n] + size.y + margin.bottom`.
    pub fn get_max_y(&self, a: &LGraphArena, n: LNodeId) -> f64 {
        let root_node = self.root[nid(a, n)].unwrap();
        self.y[nid(a, root_node)].unwrap()
            + self.inner_shift[nid(a, n)].unwrap()
            + a.node(n).size.y
            + a.node(n).margin.bottom
    }

    /// The node with a larger y than `n` within `n`'s layer, if any.
    fn get_lower_neighbor(
        &self,
        a: &LGraphArena,
        ni: &NeighborhoodInformation,
        n: LNodeId,
    ) -> Option<LNodeId> {
        let l = a.node(n).layer.unwrap();
        let layer_index = ni.node_index[nid(a, n)];
        if layer_index < a.layer(l).nodes.len() as i32 - 1 {
            return Some(a.layer(l).nodes[(layer_index + 1) as usize]);
        }
        None
    }

    /// The node with a smaller y than `n` within `n`'s layer, if any.
    fn get_upper_neighbor(
        &self,
        a: &LGraphArena,
        ni: &NeighborhoodInformation,
        n: LNodeId,
    ) -> Option<LNodeId> {
        let l = a.node(n).layer.unwrap();
        let layer_index = ni.node_index[nid(a, n)];
        if layer_index > 0 {
            return Some(a.layer(l).nodes[(layer_index - 1) as usize]);
        }
        None
    }
}
