
use crate::graph::graph::{ElkGraph, NodeId};

use crate::alg_rectpacking::options;
use crate::alg_rectpacking::util::{PackArena, RowId};

fn expand(
    arena: &mut PackArena,
    g: &mut ElkGraph,
    rows: &[RowId],
    drawing_width: f64,
    additional_height: f64,
    _node_node_spacing: f64,
) {
    let height_per_row = additional_height / rows.len() as f64;
    for (index, &row) in rows.iter().enumerate() {
        let new_y = arena.row(row).y + height_per_row * index as f64;
        arena.row_set_y(g, row, new_y);
        arena.row_expand(g, row, drawing_width, height_per_row);
    }
}

pub fn equal_whitespace_eliminator(
    arena: &mut PackArena,
    g: &mut ElkGraph,
    graph: NodeId,
    rows: Option<&[RowId]>,
) -> Result<(), String> {
    match rows {
        Some(rows) => {
            let drawing_width: f64 = g.node(graph).properties.get(&options::DRAWING_WIDTH);
            let additional_height: f64 =
                g.node(graph).properties.get(&options::ADDITIONAL_HEIGHT);
            let node_node_spacing: f64 =
                g.node(graph).properties.get(&options::SPACING_NODE_NODE);
            expand(arena, g, rows, drawing_width, additional_height, node_node_spacing);
            Ok(())
        }
        None => Err("The graph does not contain rows.".to_string()),
    }
}

pub fn to_aspectratio_node_expander(
    arena: &mut PackArena,
    g: &mut ElkGraph,
    graph: NodeId,
    rows: Option<&[RowId]>,
) -> Result<(), String> {
    let mut width: f64 = g.node(graph).properties.get(&options::DRAWING_WIDTH);
    let height: f64 = g.node(graph).properties.get(&options::DRAWING_HEIGHT);
    let desired_aspect_ratio: f64 = g.node(graph).properties.get(&options::ASPECT_RATIO);
    let mut additional_height: f64 = g.node(graph).properties.get(&options::ADDITIONAL_HEIGHT);
    let aspect_ratio = width / height;
    if aspect_ratio < desired_aspect_ratio {
        width = height * desired_aspect_ratio;
        g.node(graph).properties.set(&options::DRAWING_WIDTH, width);
    } else {
        additional_height += (width / desired_aspect_ratio) - height;
        g.node(graph)
            .properties
            .set(&options::ADDITIONAL_HEIGHT, additional_height);
        g.node(graph)
            .properties
            .set(&options::DRAWING_HEIGHT, height + additional_height);
    }
    equal_whitespace_eliminator(arena, g, graph, rows)
}
