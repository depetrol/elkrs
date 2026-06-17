
use indexmap::IndexMap;

use crate::graph::graph::{ElkGraph, NodeId};

use crate::alg_radial::options::{self, CompactionStrategy};
use crate::alg_radial::sorting::RadialSorter;
use crate::alg_radial::util;

/// Basic logic for extending or
/// compacting radii, like overlap calculation.
pub struct RadiusExtension {
    /// The step size with which the contraction takes place. Default is one.
    pub compaction_step: i32,
    /// The spacing between nodes which has to be considered while contracting.
    pub spacing: f64,
    /// The root node of the graph.
    pub root: NodeId,
}

impl RadiusExtension {
    /// Contracts/extends a list of nodes from the
    /// same radius by moving them along their incoming edge.
    pub fn contract_layer(&self, g: &mut ElkGraph, layer_nodes: &[NodeId], is_contracting: bool) {
        for &node in layer_nodes {
            let shape = &g.node(node).shape;
            // node center
            let mut x_pos = shape.x + shape.width / 2.0;
            let mut y_pos = shape.y + shape.height / 2.0;

            let tree_parent = &g.node(self.root).shape;
            let parent_x = tree_parent.x + tree_parent.width / 2.0;
            let parent_y = tree_parent.y + tree_parent.height / 2.0;

            // vector of edge
            let mut x = x_pos - parent_x;
            let mut y = y_pos - parent_y;
            // vector length
            let length = (x * x + y * y).sqrt();

            // multiply with normalized vector
            x *= self.compaction_step as f64 / length;
            y *= self.compaction_step as f64 / length;

            if is_contracting {
                x_pos -= x;
                y_pos -= y;
            } else {
                x_pos += x;
                y_pos += y;
            }

            let shape = &mut g.node_mut(node).shape;
            shape.x = x_pos - shape.width / 2.0;
            shape.y = y_pos - shape.height / 2.0;
        }
    }

    /// Move the node by the given distance in the
    /// direction from the root node to this node.
    pub fn move_node(&self, g: &mut ElkGraph, node: NodeId, distance: f64) {
        let root_shape = &g.node(self.root).shape;
        let root_x = root_shape.x + root_shape.width / 2.0;
        let root_y = root_shape.y + root_shape.height / 2.0;
        let shape = &g.node(node).shape;
        let node_x = shape.x + shape.width / 2.0;
        let node_y = shape.y + shape.height / 2.0;
        let difference_x = node_x - root_x;
        let difference_y = node_y - root_y;
        // Calculate unit vector
        let length = (difference_x * difference_x + difference_y * difference_y).sqrt();
        let unit_x = difference_x / length;
        let unit_y = difference_y / length;
        // Move node by distance in direction of unit vector.
        let shape = &mut g.node_mut(node).shape;
        shape.x += unit_x * distance;
        shape.y += unit_y * distance;
    }

    /// Calculates if two nodes overlap with each other.
    pub fn overlap(&self, g: &ElkGraph, node1: NodeId, node2: NodeId) -> bool {
        let s1 = &g.node(node1).shape;
        let s2 = &g.node(node2).shape;
        let x1 = s1.x - self.spacing / 2.0;
        let x2 = s2.x - self.spacing / 2.0;
        let y1 = s1.y - self.spacing / 2.0;
        let y2 = s2.y - self.spacing / 2.0;

        let width1 = s1.width + self.spacing;
        let width2 = s2.width + self.spacing;
        let height1 = s1.height + self.spacing;
        let height2 = s2.height + self.spacing;

        if (x1 < x2 + width2 && x2 < x1) && (y1 < y2 + height2 && y2 < y1) {
            // left upper and right lower corner overlap
            true
        } else if (x2 < x1 + width1 && x1 < x2) && (y2 < y1 + height1 && y1 < y2) {
            // right lower and left upper corner overlap
            true
        } else if (x1 < x2 + width2 && x2 < x1) && (y1 < y2 && y2 < y1 + height1) {
            // left lower and right upper corner overlap
            true
        } else {
            // right upper and left lower corner overlap
            (x2 < x1 + width1 && x1 < x2) && (y1 < y2 + height2 && y2 < y1)
        }
    }

    /// Calculate if the nodes of one radius are
    /// overlapping each other.
    pub fn overlap_layer(&self, g: &ElkGraph, nodes: &[NodeId]) -> bool {
        if nodes.len() < 2 {
            return false;
        }
        let mut overlapping = false;
        for i in 0..nodes.len() {
            if i < nodes.len() - 1 {
                overlapping |= self.overlap(g, nodes[i], nodes[i + 1]);
            } else {
                overlapping |= self.overlap(g, nodes[i], nodes[0]);
            }
        }
        overlapping
    }
}

pub fn process(g: &mut ElkGraph, graph: NodeId, root: NodeId) {
    match g.node(graph).properties.get(&options::COMPACTOR) {
        CompactionStrategy::RADIAL_COMPACTION => RadialCompaction::new(g, graph, root).compact(g),
        CompactionStrategy::WEDGE_COMPACTION => {
            AnnulusWedgeCompaction::new(g, graph, root).compact(g)
        }
        // The provider only schedules this processor for COMPACTOR != NONE;
        // CompactionStrategy.create would throw here.
        CompactionStrategy::NONE => {
            panic!("No implementation is available for the layout option NONE")
        }
    }
}

fn extension_from_options(g: &ElkGraph, graph: NodeId, root: NodeId) -> RadiusExtension {
    let props = &g.node(graph).properties;
    RadiusExtension {
        compaction_step: props.get(&options::COMPACTION_STEP_SIZE),
        spacing: props.get(&options::SPACING_NODE_NODE),
        root,
    }
}

/// Compacts each radius one after another by edge
/// shortening.
struct RadialCompaction {
    ext: RadiusExtension,
    sorter: Option<Box<dyn RadialSorter>>,
    /// The value of the last radius.
    last_radius: f64,
}

impl RadialCompaction {
    fn new(g: &ElkGraph, graph: NodeId, root: NodeId) -> Self {
        RadialCompaction {
            ext: extension_from_options(g, graph, root),
            sorter: g.node(graph).properties.get(&options::SORTER).create(),
            last_radius: 0.0,
        }
    }

    fn compact(&mut self, g: &mut ElkGraph) {
        let mut first_level_nodes = util::get_successors(g, self.ext.root);
        if let Some(sorter) = &mut self.sorter {
            sorter.sort(g, &mut first_level_nodes);
        }
        self.contract(g, first_level_nodes);
    }

    /// Contract each radius beginning at the inner radius
    /// until an overlap occurs; the last contraction is undone.
    fn contract(&mut self, g: &mut ElkGraph, nodes: Vec<NodeId>) {
        if nodes.is_empty() {
            return;
        }
        let mut is_overlapping = self.overlapping(g, &nodes);
        let mut was_contracted = false;
        while !is_overlapping {
            self.ext.contract_layer(g, &nodes, true);
            was_contracted = true;
            is_overlapping = self.overlapping(g, &nodes);
        }
        // undo last step
        if was_contracted {
            self.ext.contract_layer(g, &nodes, false);
        }
        let mut next_level_nodes = util::get_next_level_nodes(g, &nodes);
        if let Some(sorter) = &mut self.sorter {
            sorter.sort(g, &mut next_level_nodes);
        }
        self.last_radius = self.calculate_radius(g, nodes[0]);
        self.contract(g, next_level_nodes);
    }

    fn calculate_radius(&self, g: &ElkGraph, node: NodeId) -> f64 {
        let shape = &g.node(node).shape;
        let root_shape = &g.node(self.ext.root).shape;
        let vector_x = shape.x - root_shape.x;
        let vector_y = shape.y - root_shape.y;
        (vector_x * vector_x + vector_y * vector_y).sqrt()
    }

    fn overlapping(&self, g: &ElkGraph, nodes: &[NodeId]) -> bool {
        if self.ext.overlap_layer(g, nodes) {
            return true;
        }
        for &node in nodes {
            let parent = util::get_tree_parent(g, node).expect("tree node without parent");
            if self.ext.overlap(g, node, parent) {
                return true;
            }
            if self.calculate_radius(g, node) - self.ext.spacing <= self.last_radius {
                return true;
            }
        }
        false
    }
}

/// Compacts each wedge one after another.
struct AnnulusWedgeCompaction {
    ext: RadiusExtension,
    sorter: Option<Box<dyn RadialSorter>>,
    /// The left contour of each wedge, keyed by the first node in the wedge.
    left_contour: IndexMap<NodeId, Vec<NodeId>>,
    /// The right contour of each wedge, keyed by the first node in the wedge.
    right_contour: IndexMap<NodeId, Vec<NodeId>>,
}

impl AnnulusWedgeCompaction {
    fn new(g: &ElkGraph, graph: NodeId, root: NodeId) -> Self {
        AnnulusWedgeCompaction {
            ext: extension_from_options(g, graph, root),
            sorter: g.node(graph).properties.get(&options::SORTER).create(),
            left_contour: IndexMap::new(),
            right_contour: IndexMap::new(),
        }
    }

    fn compact(&mut self, g: &mut ElkGraph) {
        let root = self.ext.root;
        // Calculate the first level nodes
        let mut successors = util::get_successors(g, root);
        if let Some(sorter) = &mut self.sorter {
            sorter.sort(g, &mut successors);
        }
        self.construct_contour(g, &successors);

        // contract each wedge
        let root_list = vec![root];
        // do it two times to assure each node is compacted as much as possible
        for _k in 0..2 {
            for i in 0..successors.len() {
                let node_as_list = vec![successors[i]];
                let right_parent = if i < successors.len() - 1 {
                    successors[i + 1]
                } else {
                    successors[0]
                };
                let left_parent = if i == 0 {
                    successors[successors.len() - 1]
                } else {
                    successors[i - 1]
                };

                self.contract_wedge(g, &root_list, left_parent, right_parent, node_as_list);
            }
        }
    }

    /// Contract each wedge by shortening the
    /// incoming edge as long as no overlap occurs.
    fn contract_wedge(
        &mut self,
        g: &mut ElkGraph,
        predecessors: &[NodeId],
        radial_predecessor: NodeId,
        radial_successor: NodeId,
        mut current_radius_nodes: Vec<NodeId>,
    ) {
        let mut is_overlapping = self.overlapping(
            g,
            predecessors,
            radial_predecessor,
            radial_successor,
            &mut current_radius_nodes,
        );
        let mut was_contracted = false;

        while !is_overlapping {
            self.ext.contract_layer(g, &current_radius_nodes, true);
            was_contracted = true;
            is_overlapping = self.overlapping(
                g,
                predecessors,
                radial_predecessor,
                radial_successor,
                &mut current_radius_nodes,
            );
        }

        // undo last step
        if was_contracted {
            self.ext.contract_layer(g, &current_radius_nodes, false);
        }
        // continue with the nodes from the next radius
        let mut next_level_nodes = util::get_next_level_nodes(g, &current_radius_nodes);
        if !next_level_nodes.is_empty() {
            if let Some(sorter) = &mut self.sorter {
                sorter.sort(g, &mut next_level_nodes);
            }
            self.contract_wedge(
                g,
                &current_radius_nodes,
                radial_predecessor,
                radial_successor,
                next_level_nodes,
            );
        }
    }

    fn overlapping(
        &mut self,
        g: &ElkGraph,
        predecessors: &[NodeId],
        left_parent: NodeId,
        right_parent: NodeId,
        layer_nodes: &mut Vec<NodeId>,
    ) -> bool {
        if let Some(sorter) = &mut self.sorter {
            sorter.sort(g, layer_nodes);
        }
        let first_node = layer_nodes[0];

        // overlap with left wedge contour
        if self.contour_overlap(g, left_parent, first_node, false) {
            return true;
        }

        // overlap with right wedge contour
        let last_node = layer_nodes[layer_nodes.len() - 1];
        if self.contour_overlap(g, right_parent, last_node, true) {
            return true;
        }

        // overlaps on the radius
        if self.ext.overlap_layer(g, layer_nodes) {
            return true;
        }

        // overlaps with the predecessors
        for &sorted_node in layer_nodes.iter() {
            for &predecessor in predecessors {
                if self.ext.overlap(g, sorted_node, predecessor) {
                    return true;
                }
            }
        }

        false
    }

    /// Check if a node overlaps with a neighboring
    /// wedge contour.
    fn contour_overlap(
        &self,
        g: &ElkGraph,
        neighbour_wedge_parent: NodeId,
        node: NodeId,
        left: bool,
    ) -> bool {
        let contour = if left {
            self.left_contour.get(&neighbour_wedge_parent)
        } else {
            self.right_contour.get(&neighbour_wedge_parent)
        };
        contour
            .map(|nodes| nodes.iter().any(|&c| self.ext.overlap(g, node, c)))
            .unwrap_or(false)
    }

    /// Calculate the left and right contour of
    /// each node from the first layer.
    fn construct_contour(&mut self, g: &ElkGraph, nodes: &[NodeId]) {
        for &node in nodes {
            self.left_contour.entry(node).or_default().push(node);
            self.right_contour.entry(node).or_default().push(node);

            let mut successors = util::get_successors(g, node);
            if !successors.is_empty() {
                if let Some(sorter) = &mut self.sorter {
                    sorter.sort(g, &mut successors);
                }
                self.left_contour.entry(node).or_default().push(successors[0]);
                self.right_contour
                    .entry(node)
                    .or_default()
                    .push(successors[successors.len() - 1]);

                loop {
                    let mut next = util::get_next_level_nodes(g, &successors);
                    if next.is_empty() {
                        break;
                    }
                    if let Some(sorter) = &mut self.sorter {
                        sorter.sort(g, &mut next);
                    }
                    successors = next;
                    self.left_contour.entry(node).or_default().push(successors[0]);
                    self.right_contour
                        .entry(node)
                        .or_default()
                        .push(successors[successors.len() - 1]);
                }
            }
        }
    }
}
