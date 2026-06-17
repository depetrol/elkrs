//! Ports of `org.eclipse.elk.alg.disco.structures.DCPolyomino` and
//! `org.eclipse.elk.alg.disco.DisCoPolyominoCompactor`.

use crate::alg_common::polyomino::{
    create_packing_grid, pack_polyominoes, AsPolyomino, Direction, Grid, PackingOptions, Polyomino,
};
use crate::graph::math::{ElkRectangle, KVector};

use crate::alg_disco::graph::{DCDirection, DCGraph};
use crate::alg_disco::options;

pub struct DCPolyomino {
    pub poly: Polyomino,
    /// Index of the represented `DCComponent`.
    pub representee: usize,
    pub p_width: i32,
    pub p_height: i32,
    pub cell_size_x: f64,
    pub cell_size_y: f64,
}

impl AsPolyomino for DCPolyomino {
    fn poly(&self) -> &Polyomino {
        &self.poly
    }
    fn poly_mut(&mut self) -> &mut Polyomino {
        &mut self.poly
    }
}

impl DCPolyomino {
    /// The `DCPolyomino` constructor.
    pub fn new(graph: &DCGraph, comp: usize, cs_x: f64, cs_y: f64) -> Self {
        let comp_dims = graph.components[comp].dimensions_of_bounding_rectangle(&graph.elements);

        let p_width = compute_low_res_dimension(comp_dims.x, cs_x);
        let p_height = compute_low_res_dimension(comp_dims.y, cs_y);

        let mut dc_poly = DCPolyomino {
            poly: Polyomino::new(p_width, p_height),
            representee: comp,
            p_width,
            p_height,
            cell_size_x: cs_x,
            cell_size_y: cs_y,
        };

        dc_poly.fill_cells(graph);

        for &elem in &graph.components[comp].elements {
            if !graph.elements[elem].extensions.is_empty() {
                dc_poly.add_extensions_to_poly(graph, elem);
            }
        }

        dc_poly
    }

    pub fn offset(&self, graph: &DCGraph) -> KVector {
        let mut v = graph.components[self.representee]
            .dimensions_of_bounding_rectangle(&graph.elements);
        v.sub_xy(
            self.p_width as f64 * self.cell_size_x,
            self.p_height as f64 * self.cell_size_y,
        );
        v.scale(-0.5);
        v
    }

    fn fill_cells(&mut self, graph: &DCGraph) {
        let comp = &graph.components[self.representee];
        let comp_corner = comp.min_corner(&graph.elements);
        let polyo_offset = self.offset(graph);

        let base_x = comp_corner.x - polyo_offset.x;
        let mut cur_y = comp_corner.y - polyo_offset.y;

        for y in 0..self.p_height {
            let mut cur_x = base_x;
            for x in 0..self.p_width {
                let rect = ElkRectangle {
                    x: cur_x,
                    y: cur_y,
                    width: self.cell_size_x,
                    height: self.cell_size_y,
                };
                if comp.intersects(&rect, &graph.elements) {
                    self.poly.grid.set_blocked(x, y);
                }
                cur_x += self.cell_size_x;
            }
            cur_y += self.cell_size_y;
        }
    }

    fn add_extensions_to_poly(&mut self, graph: &DCGraph, elem: usize) {
        let comp = &graph.components[self.representee];
        let comp_corner = comp.min_corner(&graph.elements);
        let polyo_offset = self.offset(graph);

        let mut base_x = comp_corner.x - polyo_offset.x;
        let mut base_y = comp_corner.y - polyo_offset.y;

        let elem_pos = &graph.elements[elem].bounds;
        base_x = elem_pos.x - base_x;
        base_y = elem_pos.y - base_y;

        for extension in graph.elements[elem].extensions.clone() {
            let pos = extension.offset;
            let xe = base_x + pos.x;
            let ye = base_y + pos.y;

            let xp = (xe / self.cell_size_x) as i32;
            let yp = (ye / self.cell_size_y) as i32;

            let dir = extension.direction;
            let poly_dir = match dir {
                DCDirection::North => Direction::NORTH,
                DCDirection::East => Direction::EAST,
                DCDirection::South => Direction::SOUTH,
                DCDirection::West => Direction::WEST,
            };

            if dir.is_horizontal() {
                let yp_plus_width = ((ye + extension.width) / self.cell_size_y) as i32;
                self.poly.add_extension(poly_dir, yp, yp_plus_width);
                if dir == DCDirection::West {
                    self.poly.grid.weakly_block_area(0, yp, xp, yp_plus_width);
                } else {
                    self.poly
                        .grid
                        .weakly_block_area(xp, yp, self.p_width - 1, yp_plus_width);
                }
            } else {
                let xp_plus_width = ((xe + extension.width) / self.cell_size_x) as i32;
                self.poly.add_extension(poly_dir, xp, xp_plus_width);
                if dir == DCDirection::North {
                    self.poly.grid.weakly_block_area(xp, 0, xp_plus_width, yp);
                } else {
                    self.poly
                        .grid
                        .weakly_block_area(xp, yp, xp_plus_width, self.p_height - 1);
                }
            }
        }
    }
}

fn compute_low_res_dimension(dim: f64, cell_size: f64) -> i32 {
    let cell_fit = dim / cell_size;
    let mut fit_truncated = cell_fit as i32;
    if cell_fit > fit_truncated as f64 {
        fit_truncated += 1;
    }
    fit_truncated
}

/// Returns the exact replica of the
/// `List<DCPolyomino>.toString()` debug string stored on the graph.
pub fn compact(graph: &mut DCGraph) -> String {
    // upper bound on the size of a grid cell, from the paper
    let upper_bound = 100.0;

    let grid_cell_recommendation = compute_cell_size(graph, upper_bound);
    let mut grid_cell_size_x = grid_cell_recommendation;
    let mut grid_cell_size_y = grid_cell_recommendation;

    let fill: bool = graph.properties.get(&options::POLYOMINO_FILL);
    let aspect_ratio: f64 = graph
        .properties
        .try_get(&options::ASPECT_RATIO)
        .unwrap_or(1.0);

    if aspect_ratio > 1.0 {
        grid_cell_size_x *= aspect_ratio;
    } else {
        grid_cell_size_y /= aspect_ratio;
    }

    // 1.) Convert DCComponents into polyominoes.
    let mut polys: Vec<DCPolyomino> = Vec::new();
    for comp in 0..graph.components.len() {
        polys.push(DCPolyomino::new(graph, comp, grid_cell_size_x, grid_cell_size_y));
    }

    // 2.) Pack the polyominoes (the Polyominoes holder never receives the
    // graph's properties, so the packing always uses the defaults).
    for (id, poly) in polys.iter_mut().enumerate() {
        graph.components[poly.representee].id = id as i32;
    }
    let mut grid: Grid = create_packing_grid(&mut polys, aspect_ratio, fill);
    pack_polyominoes(&mut polys, &mut grid, &PackingOptions::default());

    // 3.) Apply layout back to the DCGraph.
    apply_to_dc_graph(graph, &polys, &grid, grid_cell_size_x, grid_cell_size_y);

    // Debug property: stores the List<DCPolyomino>; its toString is the
    // concatenation of the TwoBitGrid renditions.
    let strings: Vec<String> = polys.iter().map(|p| p.poly.grid.java_to_string()).collect();
    format!("[{}]", strings.join(", "))
}

fn compute_cell_size(graph: &DCGraph, upper_bound: f64) -> f64 {
    let mut sum_term = 0.0;
    let mut prod_term = 0.0;
    let num_of_comps = graph.components.len() as f64;

    for comp in &graph.components {
        let bounds = comp.dimensions_of_bounding_rectangle(&graph.elements);
        let width = bounds.x;
        let height = bounds.y;
        sum_term += width + height;
        prod_term += width * height;
    }

    let four = 4.0;
    let numerator = (four * upper_bound * num_of_comps * prod_term - four * prod_term
        + sum_term * sum_term)
        .sqrt()
        + sum_term;
    let denominator = 2.0 * (upper_bound * num_of_comps - 1.0);

    if denominator == 0.0 {
        return numerator;
    }
    numerator / denominator
}

fn apply_to_dc_graph(
    graph: &mut DCGraph,
    polys: &[DCPolyomino],
    grid: &Grid,
    grid_cell_size_x: f64,
    grid_cell_size_y: f64,
) {
    let (crop_x, crop_y, crop_w, crop_h) = grid.get_filled_bounds();
    let padding: crate::graph::math::ElkPadding = graph.properties.get(&options::PADDING);
    let padding_hori = padding.horizontal();
    let padding_vert = padding.vertical();
    let parent_width = (crop_w as f64 * grid_cell_size_x) + padding_hori;
    let parent_height = (crop_h as f64 * grid_cell_size_y) + padding_vert;
    graph.dimensions = KVector::new(parent_width, parent_height);

    for poly in polys {
        let absolute_int_position_x = poly.poly.x - crop_x;
        let absolute_int_position_y = poly.poly.y - crop_y;

        let mut absolute_position_on_canvas =
            KVector::new(absolute_int_position_x as f64, absolute_int_position_y as f64);
        absolute_position_on_canvas.scale_xy(poly.cell_size_x, poly.cell_size_y);
        absolute_position_on_canvas.add(poly.offset(graph));

        let original_coordinates = graph.components[poly.representee].min_corner(&graph.elements);
        let mut new_offset = absolute_position_on_canvas;
        new_offset.sub(original_coordinates);
        graph.components[poly.representee].offset = new_offset;
    }
}
