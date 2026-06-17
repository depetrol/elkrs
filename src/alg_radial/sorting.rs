
use crate::graph::graph::{ElkGraph, NodeId};

use crate::alg_radial::options::{SortingStrategy, ORDER_ID};
use crate::alg_radial::util;

pub trait RadialSorter {
    fn initialize(&mut self, g: &ElkGraph, root: NodeId);
    fn sort(&mut self, g: &ElkGraph, nodes: &mut Vec<NodeId>);
}

impl SortingStrategy {
    pub fn create(self) -> Option<Box<dyn RadialSorter>> {
        match self {
            SortingStrategy::NONE => None,
            SortingStrategy::POLAR_COORDINATE => Some(Box::new(PolarCoordinateSorter::default())),
            SortingStrategy::ID => Some(Box::new(IdSorter)),
        }
    }
}

/// The `IDSorter` comparator (stable sort by `RadialOptions.ORDER_ID`).
fn id_sort(g: &ElkGraph, nodes: &mut [NodeId]) {
    nodes.sort_by(|&n1, &n2| {
        let order_id1: i32 = g.node(n1).properties.get(&ORDER_ID);
        let order_id2: i32 = g.node(n2).properties.get(&ORDER_ID);
        order_id1.cmp(&order_id2)
    });
}

pub struct IdSorter;

impl RadialSorter for IdSorter {
    fn initialize(&mut self, _g: &ElkGraph, _root: NodeId) {
        // nothing to do here
    }

    fn sort(&mut self, g: &ElkGraph, nodes: &mut Vec<NodeId>) {
        id_sort(g, nodes);
    }
}

#[derive(Default)]
pub struct PolarCoordinateSorter {
    /// The lazily created `idSorter` field.
    initialized: bool,
}

const DEGREE_45: f64 = 0.25 * std::f64::consts::PI;
const DEGREE_90: f64 = 0.5 * std::f64::consts::PI;
const DEGREE_135: f64 = 0.75 * std::f64::consts::PI;
const DEGREE_225: f64 = 1.25 * std::f64::consts::PI;
const DEGREE_270: f64 = 1.5 * std::f64::consts::PI;
const DEGREE_315: f64 = 1.75 * std::f64::consts::PI;

impl PolarCoordinateSorter {
    /// Assigns `ORDER_ID` over the whole tree.
    fn set_id_for_nodes(&self, g: &ElkGraph, nodes: &[NodeId], id_offset: i32) -> i32 {
        let mut id = id_offset;
        let mut next_layer_id = 0;
        for &node in nodes {
            g.node(node).properties.set(&ORDER_ID, id);
            id += 1;
            let mut node_successors = util::get_successors(g, node);
            let shape = &g.node(node).shape;
            let mut arc = (shape.y + shape.height / 2.0).atan2(shape.x + shape.width / 2.0);
            if arc < 0.0 {
                arc += util::TWO_PI;
            }

            // node is right of parent node
            if arc < DEGREE_45 || arc > DEGREE_315 {
                node_successors.sort_by(|&a, &b| {
                    util::polar_compare(g, std::f64::consts::PI, 0.0, a, b)
                });
            } else if arc <= DEGREE_315 && arc > DEGREE_225 {
                // node is below parent node
                node_successors.sort_by(|&a, &b| util::polar_compare(g, DEGREE_270, 0.0, a, b));
            } else if arc <= DEGREE_225 && arc > DEGREE_135 {
                // node is left
                node_successors.sort_by(|&a, &b| util::polar_compare(g, 0.0, 0.0, a, b));
            } else if arc <= DEGREE_135 {
                // node is top
                node_successors.sort_by(|&a, &b| util::polar_compare(g, DEGREE_90, 0.0, a, b));
            }

            next_layer_id = self.set_id_for_nodes(g, &node_successors, next_layer_id);
        }
        id
    }
}

impl RadialSorter for PolarCoordinateSorter {
    fn initialize(&mut self, g: &ElkGraph, root: NodeId) {
        self.initialized = true;
        let mut successors = util::get_successors(g, root);
        // sort the first layer, the sorting starts at degree 0 the first polar
        // coordinate position, which is on the right of the circle.
        successors.sort_by(|&a, &b| util::polar_compare(g, 0.0, 0.0, a, b));
        self.set_id_for_nodes(g, &successors, 0);
    }

    fn sort(&mut self, g: &ElkGraph, nodes: &mut Vec<NodeId>) {
        if !nodes.is_empty() {
            if !self.initialized {
                let root = util::find_root_of_node(g, nodes[0]);
                self.initialize(g, root);
            }
            id_sort(g, nodes);
        }
    }
}
