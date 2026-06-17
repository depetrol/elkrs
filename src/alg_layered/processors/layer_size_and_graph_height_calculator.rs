
use crate::alg_layered::graph::{LGraphArena, LGraphId, NodeType};
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut found_nodes = false;

    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        a.layer_mut(layer).size = crate::graph::math::KVector::default();
        if a.layer(layer).nodes.is_empty() {
            continue;
        }
        found_nodes = true;

        let nodes = a.layer(layer).nodes.clone();
        let mut layer_width = 0.0f64;
        for &node in &nodes {
            let n = a.node(node);
            layer_width = f64::max(layer_width, n.size.x + n.margin.left + n.margin.right);
        }
        a.layer_mut(layer).size.x = layer_width;

        let first_node = nodes[0];
        let mut top = a.node(first_node).pos.y - a.node(first_node).margin.top;
        if a.node(first_node).node_type == NodeType::EXTERNAL_PORT {
            top -= a
                .graph(graph)
                .properties
                .get(&lopts::SPACING_PORTS_SURROUNDING)
                .top;
        }
        let last_node = *nodes.last().unwrap();
        let mut bottom =
            a.node(last_node).pos.y + a.node(last_node).size.y + a.node(last_node).margin.bottom;
        if a.node(last_node).node_type == NodeType::EXTERNAL_PORT {
            bottom += a
                .graph(graph)
                .properties
                .get(&lopts::SPACING_PORTS_SURROUNDING)
                .bottom;
        }
        a.layer_mut(layer).size.y = bottom - top;

        min_y = f64::min(min_y, top);
        max_y = f64::max(max_y, bottom);
    }

    if !found_nodes {
        min_y = 0.0;
        max_y = 0.0;
    }

    a.graph_mut(graph).size.y = max_y - min_y;
    a.graph_mut(graph).offset.y -= min_y;
    Ok(())
}
