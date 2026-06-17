
use crate::alg_layered::graph::{LGraphArena, LGraphId};
use crate::alg_layered::spacings;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let layers = a.graph(graph).layers.clone();

    let mut max_height = 0.0f64;
    for &layer in &layers {
        let mut layer_height = 0.0f64;
        let nodes = a.layer(layer).nodes.clone();
        let mut last_node = None;
        for &node in &nodes {
            if let Some(last) = last_node {
                layer_height += spacings::vertical_spacing(a, node, last);
            }
            let n = a.node(node);
            layer_height += n.margin.top + n.size.y + n.margin.bottom;
            last_node = Some(node);
        }
        a.layer_mut(layer).size.y = layer_height;
        max_height = f64::max(max_height, layer_height);
    }

    for &layer in &layers {
        let layer_height = a.layer(layer).size.y;
        let mut pos = (max_height - layer_height) / 2.0;
        let nodes = a.layer(layer).nodes.clone();
        let mut last_node = None;
        for &node in &nodes {
            if let Some(last) = last_node {
                pos += spacings::vertical_spacing(a, node, last);
            }
            pos += a.node(node).margin.top;
            a.node_mut(node).pos.y = pos;
            pos += a.node(node).size.y + a.node(node).margin.bottom;
            last_node = Some(node);
        }
    }
    Ok(())
}
