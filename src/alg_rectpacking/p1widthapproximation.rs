
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::math::ElkPadding;

use crate::alg_rectpacking::options::{self, OptimizationGoal};
use crate::alg_rectpacking::util::{self, DrawingData, DrawingDataDescriptor};

// --------------------------------------------------------------- Calculations

fn get_width_lpr_or_lpb(drawing_width: f64, x: f64, width: f64) -> f64 {
    f64::max(drawing_width, x + width)
}

fn get_height_lpr_or_lpb(drawing_height: f64, y: f64, height: f64) -> f64 {
    f64::max(drawing_height, y + height)
}

fn calculate_y_for_lpr(
    g: &ElkGraph,
    x: f64,
    placed_rects: &[NodeId],
    last_placed: NodeId,
    node_node_spacing: f64,
) -> f64 {
    let mut closest_upper_neighbor: Option<NodeId> = None;
    let mut closest_neighbor_bottom_border = 0.0f64;
    // find neighbors that lay between the upper and lower border of the
    // rectangle to be placed.
    for &placed_rect in placed_rects {
        let p = &g.node(placed_rect).shape;
        let placed_rect_bottom_border = p.y + p.height;
        if vertical_order_constraint(g, placed_rect, x, node_node_spacing) {
            // is closest neighbor?
            if closest_upper_neighbor.is_none() {
                closest_upper_neighbor = Some(placed_rect);
            } else if g.node(last_placed).shape.y - placed_rect_bottom_border
                < g.node(last_placed).shape.y - closest_neighbor_bottom_border
            {
                closest_upper_neighbor = Some(placed_rect);
            }
            let c = &g.node(closest_upper_neighbor.unwrap()).shape;
            closest_neighbor_bottom_border = c.y + c.height;
        }
    }

    match closest_upper_neighbor {
        None => 0.0,
        Some(_) => closest_neighbor_bottom_border + node_node_spacing,
    }
}

fn calculate_x_for_lpb(
    g: &ElkGraph,
    y: f64,
    placed_rects: &[NodeId],
    last_placed: NodeId,
    node_node_spacing: f64,
) -> f64 {
    let mut closest_left_neighbour: Option<NodeId> = None;
    let mut closest_neighbor_right_border = 0.0f64;
    // Find neighbors that lay in between the height of the rectangle to be placed.
    for &placed_rect in placed_rects {
        let p = &g.node(placed_rect).shape;
        let placed_rect_right_border = p.x + p.width;
        if horizontal_order_constraint(g, placed_rect, y, node_node_spacing) {
            // Is closest neighbor?
            if closest_left_neighbour.is_none() {
                closest_left_neighbour = Some(placed_rect);
            } else if g.node(last_placed).shape.x - placed_rect_right_border
                < g.node(last_placed).shape.x - closest_neighbor_right_border
            {
                closest_left_neighbour = Some(placed_rect);
            }
            let c = &g.node(closest_left_neighbour.unwrap()).shape;
            closest_neighbor_right_border = c.x + c.width;
        }
    }

    match closest_left_neighbour {
        None => 0.0,
        Some(_) => closest_neighbor_right_border + node_node_spacing,
    }
}

fn calculate_area_lpr(g: &ElkGraph, last_placed: NodeId, to_place: NodeId, lpr_opt: &DrawingData) -> f64 {
    let lp = &g.node(last_placed).shape;
    let tp = &g.node(to_place).shape;
    let last_placed_bottom_border = lp.y + lp.height;
    let to_place_bottom_border = lpr_opt.next_y() + tp.height;
    let max_y_lpr = f64::max(last_placed_bottom_border, to_place_bottom_border);

    let height_lpr = max_y_lpr - f64::min(lp.y, lpr_opt.next_y());
    let width_lpr = lpr_opt.next_x() + tp.width - lp.x;

    width_lpr * height_lpr
}

fn calculate_area_lpb(g: &ElkGraph, last_placed: NodeId, to_place: NodeId, lpb_opt: &DrawingData) -> f64 {
    let lp = &g.node(last_placed).shape;
    let tp = &g.node(to_place).shape;
    let last_placed_right_border = lp.x + lp.width;
    let to_place_right_border = lpb_opt.next_x() + tp.width;
    let max_x_lpb = f64::max(last_placed_right_border, to_place_right_border);

    let width_lpb = max_x_lpb - f64::min(lp.x, lpb_opt.next_x());
    let height_lpb = lpb_opt.next_y() + tp.height - lp.y;

    width_lpb * height_lpb
}

fn vertical_order_constraint(g: &ElkGraph, placed_rect: NodeId, x: f64, node_node_spacing: f64) -> bool {
    let p = &g.node(placed_rect).shape;
    x < p.x + p.width + node_node_spacing
}

fn horizontal_order_constraint(
    g: &ElkGraph,
    placed_rect: NodeId,
    y_coord_rect_to_place: f64,
    node_node_spacing: f64,
) -> bool {
    let p = &g.node(placed_rect).shape;
    y_coord_rect_to_place < p.y + p.height + node_node_spacing
}

// --------------------------------------------------------- BestCandidateFilter

fn area_filter(candidates: Vec<DrawingData>, _aspect_ratio: f64, padding: &ElkPadding) -> Vec<DrawingData> {
    let mut min_area = f64::INFINITY;
    for opt in &candidates {
        min_area = f64::min(
            min_area,
            (opt.drawing_width() + padding.horizontal()) * (opt.drawing_height() + padding.vertical()),
        );
    }
    candidates
        .into_iter()
        .filter(|candidate| {
            (candidate.drawing_width() + padding.horizontal())
                * (candidate.drawing_height() + padding.vertical())
                == min_area
        })
        .collect()
}

fn aspect_ratio_filter(candidates: Vec<DrawingData>, aspect_ratio: f64, padding: &ElkPadding) -> Vec<DrawingData> {
    let deviation = |opt: &DrawingData| {
        (((opt.drawing_width() + padding.horizontal()) / (opt.drawing_height() + padding.vertical()))
            - aspect_ratio)
            .abs()
    };
    let mut smallest_deviation = f64::INFINITY;
    for opt in &candidates {
        smallest_deviation = f64::min(smallest_deviation, deviation(opt));
    }
    candidates
        .into_iter()
        .filter(|candidate| deviation(candidate) == smallest_deviation)
        .collect()
}

fn scale_measure_filter(candidates: Vec<DrawingData>, _aspect_ratio: f64, padding: &ElkPadding) -> Vec<DrawingData> {
    let scale = |opt: &DrawingData| {
        util::compute_scale_measure(
            opt.drawing_width() + padding.horizontal(),
            opt.drawing_height() + padding.vertical(),
            opt.desired_aspect_ratio(),
        )
    };
    let mut max_scale = f64::NEG_INFINITY;
    for opt in &candidates {
        max_scale = f64::max(max_scale, scale(opt));
    }
    candidates
        .into_iter()
        .filter(|candidate| scale(candidate) == max_scale)
        .collect()
}

type Filter = fn(Vec<DrawingData>, f64, &ElkPadding) -> Vec<DrawingData>;

// ------------------------------------------------------------ AreaApproximation

pub struct AreaApproximation {
    aspect_ratio: f64,
    goal: OptimizationGoal,
    lp_shift: bool,
}

impl AreaApproximation {
    pub fn new(aspect_ratio: f64, goal: OptimizationGoal, lp_shift: bool) -> Self {
        AreaApproximation { aspect_ratio, goal, lp_shift }
    }

    pub fn approx_bounding_box(
        &self,
        g: &mut ElkGraph,
        rectangles: &[NodeId],
        node_node_spacing: f64,
        padding: &ElkPadding,
    ) -> DrawingData {
        // Place first box.
        let first_rect = rectangles[0];
        g.node_mut(first_rect).shape.x = 0.0;
        g.node_mut(first_rect).shape.y = 0.0;
        let mut placed_rects: Vec<NodeId> = vec![first_rect];
        let mut last_placed = first_rect;
        let mut current_values = DrawingData::new(
            self.aspect_ratio,
            g.node(first_rect).shape.width,
            g.node(first_rect).shape.height,
            DrawingDataDescriptor::WholeDrawing,
        );

        // Place the other boxes.
        for &to_place in &rectangles[1..] {
            // Determine drawing metrics for different candidate
            // positions/placement options
            let opt1 = self.calc_values_for_opt(
                g,
                DrawingDataDescriptor::CandidatePositionLastPlacedRight,
                to_place,
                last_placed,
                &current_values,
                &placed_rects,
                node_node_spacing,
            );
            let opt2 = self.calc_values_for_opt(
                g,
                DrawingDataDescriptor::CandidatePositionLastPlacedBelow,
                to_place,
                last_placed,
                &current_values,
                &placed_rects,
                node_node_spacing,
            );
            let opt3 = self.calc_values_for_opt(
                g,
                DrawingDataDescriptor::CandidatePositionWholeDrawingRight,
                to_place,
                last_placed,
                &current_values,
                &placed_rects,
                node_node_spacing,
            );
            let opt4 = self.calc_values_for_opt(
                g,
                DrawingDataDescriptor::CandidatePositionWholeDrawingBelow,
                to_place,
                last_placed,
                &current_values,
                &placed_rects,
                node_node_spacing,
            );

            let mut best_opt = self
                .find_best_candidate(g, opt1, opt2, opt3, opt4, to_place, last_placed, padding)
                .expect("no best placement option found");

            g.node_mut(to_place).shape.x = best_opt.next_x();
            g.node_mut(to_place).shape.y = best_opt.next_y();
            best_opt.set_placement_option(DrawingDataDescriptor::WholeDrawing);
            current_values = best_opt;
            last_placed = to_place;
            placed_rects.push(to_place);
        }

        current_values
    }

    #[allow(clippy::too_many_arguments)]
    fn find_best_candidate(
        &self,
        g: &ElkGraph,
        opt1: DrawingData,
        opt2: DrawingData,
        opt3: DrawingData,
        opt4: DrawingData,
        to_place: NodeId,
        last_placed: NodeId,
        padding: &ElkPadding,
    ) -> Option<DrawingData> {
        let mut candidates = vec![opt1, opt2, opt3, opt4];

        // Sets the order of the filters according to the given goal.
        let filters: [Filter; 3] = match self.goal {
            OptimizationGoal::MAX_SCALE_DRIVEN => {
                [scale_measure_filter, area_filter, aspect_ratio_filter]
            }
            OptimizationGoal::ASPECT_RATIO_DRIVEN => {
                [aspect_ratio_filter, area_filter, scale_measure_filter]
            }
            OptimizationGoal::AREA_DRIVEN => {
                [area_filter, scale_measure_filter, aspect_ratio_filter]
            }
        };

        // Filter the candidates according to the order of the filters.
        for filter in filters {
            if candidates.len() > 1 {
                candidates = filter(candidates, self.aspect_ratio, padding);
            }
        }

        // Only one candidate remains.
        if candidates.len() == 1 {
            return candidates.pop();
        }
        // Multiple options have the same value for every benchmark. These
        // special cases are caught in the following.
        if candidates.len() == 2 {
            let drawing2 = candidates.pop().unwrap();
            let drawing1 = candidates.pop().unwrap();
            return Some(self.check_special_cases(g, drawing1, drawing2, last_placed, to_place));
        }
        None
    }

    fn check_special_cases(
        &self,
        g: &ElkGraph,
        drawing1: DrawingData,
        drawing2: DrawingData,
        last_placed: NodeId,
        to_place: NodeId,
    ) -> DrawingData {
        use DrawingDataDescriptor::*;
        let first_opt = drawing1.placement_option();
        let second_opt = drawing2.placement_option();

        let first_opt_lpb_or_cdb = first_opt == CandidatePositionLastPlacedBelow
            || first_opt == CandidatePositionWholeDrawingBelow;
        let second_opt_lpb_or_cdb = second_opt == CandidatePositionLastPlacedBelow
            || second_opt == CandidatePositionWholeDrawingBelow;

        let first_opt_lpr_or_cdr = first_opt == CandidatePositionLastPlacedRight
            || first_opt == CandidatePositionWholeDrawingRight;
        let second_opt_lpr_or_cdr = second_opt == CandidatePositionLastPlacedRight
            || second_opt == CandidatePositionWholeDrawingRight;

        let first_opt_lpr_or_lpb = first_opt == CandidatePositionLastPlacedRight
            || first_opt == CandidatePositionLastPlacedBelow;
        let second_opt_lpr_or_lpb = second_opt == CandidatePositionLastPlacedRight
            || second_opt == CandidatePositionLastPlacedBelow;

        if first_opt_lpb_or_cdb && second_opt_lpb_or_cdb {
            // If placing it LPB and WDB produces the same values. Take WDB.
            if drawing1.placement_option() == CandidatePositionWholeDrawingBelow {
                drawing1
            } else {
                drawing2
            }
        } else if first_opt_lpr_or_cdr && second_opt_lpr_or_cdr {
            // If placing it LPR and WDR produces the same values. Take WDR.
            if drawing1.placement_option() == CandidatePositionWholeDrawingRight {
                drawing1
            } else {
                drawing2
            }
        } else if first_opt_lpr_or_lpb && second_opt_lpr_or_lpb {
            // If LPR AND LPB produce the same values. Take the option producing
            // less area with the last placed rectangle and rectangle to place.
            let (lpr_opt, lpb_opt) = if first_opt == CandidatePositionLastPlacedRight {
                (&drawing1, &drawing2)
            } else {
                (&drawing2, &drawing1)
            };

            let area_lpr = calculate_area_lpr(g, last_placed, to_place, lpr_opt);
            let area_lpb = calculate_area_lpb(g, last_placed, to_place, lpb_opt);

            if area_lpr <= area_lpb {
                if drawing1.placement_option() == CandidatePositionLastPlacedRight {
                    drawing1
                } else {
                    drawing2
                }
            } else if drawing1.placement_option() == CandidatePositionLastPlacedBelow {
                drawing1
            } else {
                drawing2
            }
        } else {
            drawing1
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn calc_values_for_opt(
        &self,
        g: &ElkGraph,
        option: DrawingDataDescriptor,
        to_place: NodeId,
        last_placed: NodeId,
        drawing: &DrawingData,
        placed_rects: &[NodeId],
        node_node_spacing: f64,
    ) -> DrawingData {
        let drawing_width = drawing.drawing_width();
        let drawing_height = drawing.drawing_height();
        let height_to_place = g.node(to_place).shape.height;
        let width_to_place = g.node(to_place).shape.width;
        let lp = {
            let s = &g.node(last_placed).shape;
            (s.x, s.y, s.width, s.height)
        };

        let (x, y, width, height) = match option {
            DrawingDataDescriptor::CandidatePositionLastPlacedRight => {
                let x = lp.0 + lp.2 + node_node_spacing;
                let y = if self.lp_shift {
                    calculate_y_for_lpr(g, x, placed_rects, last_placed, node_node_spacing)
                } else {
                    lp.1
                };
                let width = get_width_lpr_or_lpb(drawing_width, x, width_to_place);
                let height = get_height_lpr_or_lpb(drawing_height, y, height_to_place);
                (x, y, width, height)
            }
            DrawingDataDescriptor::CandidatePositionLastPlacedBelow => {
                let y = lp.1 + lp.3 + node_node_spacing;
                let x = if self.lp_shift {
                    calculate_x_for_lpb(g, y, placed_rects, last_placed, node_node_spacing)
                } else {
                    lp.0
                };
                let width = get_width_lpr_or_lpb(drawing_width, x, width_to_place);
                let height = get_height_lpr_or_lpb(drawing_height, y, height_to_place);
                (x, y, width, height)
            }
            DrawingDataDescriptor::CandidatePositionWholeDrawingRight => {
                let x = drawing_width + node_node_spacing;
                let y = 0.0;
                let width = drawing_width + node_node_spacing + width_to_place;
                let height = f64::max(drawing_height, height_to_place);
                (x, y, width, height)
            }
            DrawingDataDescriptor::CandidatePositionWholeDrawingBelow => {
                let x = 0.0;
                let y = drawing_height + node_node_spacing;
                let width = f64::max(drawing_width, width_to_place);
                let height = drawing_height + node_node_spacing + height_to_place;
                (x, y, width, height)
            }
            DrawingDataDescriptor::WholeDrawing => {
                panic!("IllegalPlacementOption.")
            }
        };

        DrawingData::with_coords(self.aspect_ratio, width, height, option, x, y)
    }
}

// ------------------------------------------------------------------- phases

pub fn greedy_width_approximator(g: &mut ElkGraph, graph: NodeId) {
    // The desired aspect ratio.
    let aspect_ratio: f64 = g.node(graph).properties.get(&options::ASPECT_RATIO);
    // The padding surrounding the drawing.
    let padding: ElkPadding = g.node(graph).properties.get(&options::PADDING);
    // The strategy for the initial width approximation.
    let goal: OptimizationGoal = g
        .node(graph)
        .properties
        .get(&options::WIDTH_APPROXIMATION_OPTIMIZATION_GOAL);
    // Option for better width approximation.
    let last_place_shift: bool = g
        .node(graph)
        .properties
        .get(&options::WIDTH_APPROXIMATION_LAST_PLACE_SHIFT);
    // The spacing between two nodes.
    let node_node_spacing: f64 = g.node(graph).properties.get(&options::SPACING_NODE_NODE);

    let rectangles = g.node(graph).children.clone();
    util::reset_coordinates(g, &rectangles);

    // Initial width approximation.
    let first_it = AreaApproximation::new(aspect_ratio, goal, last_place_shift);
    let drawing = first_it.approx_bounding_box(g, &rectangles, node_node_spacing, &padding);

    g.node(graph)
        .properties
        .set(&options::TARGET_WIDTH, drawing.drawing_width());
}

pub fn target_width_width_approximator(g: &mut ElkGraph, graph: NodeId) -> Result<(), String> {
    if g.node(graph).properties.has(&options::WIDTH_APPROXIMATION_TARGET_WIDTH) {
        let target: f64 = g
            .node(graph)
            .properties
            .get(&options::WIDTH_APPROXIMATION_TARGET_WIDTH);
        g.node(graph).properties.set(&options::TARGET_WIDTH, target);
        Ok(())
    } else {
        Err("A target width has to be set if the TargetWidthWidthApproximator should be used."
            .to_string())
    }
}
