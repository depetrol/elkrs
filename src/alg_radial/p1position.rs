
use crate::graph::graph::{ElkGraph, NodeId};

use crate::alg_radial::options::{self, AnnulusWedgeCriteria, RadialTranslationStrategy};
use crate::alg_radial::sorting::RadialSorter;
use crate::alg_radial::util;

const CIRCLE_DEGREES: i32 = 360;
const DEGREE_TO_RAD: f64 = std::f64::consts::PI / 180.0;

impl AnnulusWedgeCriteria {
    pub fn calculate_wedge_space(self, g: &ElkGraph, node: NodeId) -> f64 {
        match self {
            // AnnulusWedgeByLeafs
            AnnulusWedgeCriteria::LEAF_NUMBER => util::get_number_of_leaves(g, node) as f64,
            // AnnulusWedgeByNodeSpace
            AnnulusWedgeCriteria::NODE_SIZE => {
                let successors = util::get_successors(g, node);
                let shape = &g.node(node).shape;
                let node_size =
                    (shape.height * shape.height + shape.width * shape.width).sqrt();
                let mut child_space = 0.0;
                for child in successors {
                    child_space += self.calculate_wedge_space(g, child);
                }
                child_space.max(node_size)
            }
        }
    }
}

struct EadesRadial {
    radius: f64,
    sorter: Option<Box<dyn RadialSorter>>,
    annulus_wedge_criteria: AnnulusWedgeCriteria,
    optimizer: Option<RadialTranslationStrategy>,
    root: NodeId,
}

pub fn process(g: &mut ElkGraph, graph: NodeId, root: NodeId) {
    let props = &g.node(graph).properties;
    let mut phase = EadesRadial {
        radius: props.get(&options::RADIUS),
        sorter: props.get(&options::SORTER).create(),
        annulus_wedge_criteria: props.get(&options::WEDGE_CRITERIA),
        optimizer: props.get(&options::OPTIMIZATION_CRITERIA).create(),
        root,
    };
    phase.translate(g);
}

impl EadesRadial {
    /// Search for the best layout translation by looking
    /// at each degree.
    fn translate(&mut self, g: &mut ElkGraph) {
        let mut optimal_offset = 0.0;
        let mut optimal_value = f64::MAX;

        if let Some(optimizer) = self.optimizer {
            for i in 0..CIRCLE_DEGREES {
                let offset = i as f64 * DEGREE_TO_RAD;
                self.position_nodes(g, self.root, 0.0, 0.0, util::TWO_PI, offset);
                let translated_value = optimizer.evaluate(g, self.root);
                // Take the first occurence of the minimum
                if translated_value < optimal_value {
                    optimal_offset = offset;
                    optimal_value = translated_value;
                }
            }
        }
        let root = self.root;
        self.position_nodes(g, root, 0.0, 0.0, util::TWO_PI, optimal_offset);
    }

    /// Place a node in the center of a wedge and
    /// calculate the wedge for the next child.
    fn position_nodes(
        &mut self,
        g: &mut ElkGraph,
        node: NodeId,
        current_radius: f64,
        min_alpha: f64,
        max_alpha: f64,
        optimal_offset: f64,
    ) {
        let alpha_point = (min_alpha + max_alpha) / 2.0 + optimal_offset;

        // x=r*sinθ, y=r*cosθ
        let x_pos = current_radius * alpha_point.cos();
        let y_pos = current_radius * alpha_point.sin();

        // shift the nodes, such that the center of each node is on the circle
        util::center_nodes_on_radi(g, node, x_pos, y_pos);

        let number_of_leafs = self.annulus_wedge_criteria.calculate_wedge_space(g, node);
        // quirk preserved: `currentRadius / currentRadius + radius`
        // (NaN for the root, acos(1 + radius) = NaN for radius > 0), so the
        // else branch below is effectively always taken.
        let tau = 2.0 * (current_radius / current_radius + self.radius).acos();
        let s;
        let mut alpha;
        if tau < max_alpha - min_alpha {
            s = tau / number_of_leafs;
            alpha = (min_alpha + max_alpha - tau) / 2.0;
        } else {
            s = (max_alpha - min_alpha) / number_of_leafs;
            alpha = min_alpha;
        }
        let mut successors = util::get_successors(g, node);
        if let Some(sorter) = &mut self.sorter {
            sorter.initialize(g, self.root);
            sorter.sort(g, &mut successors);
        }
        for child in successors {
            let number_of_child_leafs =
                self.annulus_wedge_criteria.calculate_wedge_space(g, child);
            self.position_nodes(
                g,
                child,
                current_radius + self.radius,
                alpha,
                alpha + s * number_of_child_leafs,
                optimal_offset,
            );
            alpha += s * number_of_child_leafs;
        }
    }
}
