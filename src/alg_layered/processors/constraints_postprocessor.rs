//!
//! Adds to each LNode the layerID and positionID that has been computed by ELK
//! Layered.

use crate::alg_layered::graph::{LGraphArena, LGraphId, NodeType};
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mut layer_index = 0i32;

    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let mut pos_index = 0i32;

        let mut node_layer = false;
        let nodes = a.layer(layer).nodes.clone();
        for current_node in nodes {
            if a.node(current_node).node_type == NodeType::NORMAL {
                node_layer = true;
                a.node_mut(current_node)
                    .properties
                    .set(&lopts::LAYERING_LAYER_ID, layer_index);
                a.node_mut(current_node)
                    .properties
                    .set(&lopts::CROSSING_MINIMIZATION_POSITION_ID, pos_index);
                pos_index += 1;
            }
        }
        // layers with no nodes in it should not increase the layer id
        if node_layer {
            layer_index += 1;
        }
    }

    Ok(())
}
