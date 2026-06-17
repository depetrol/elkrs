
use crate::alg_layered::graph::{LGraphArena, LGraphId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen::InLayerConstraint;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let mut top_insertion_index: i32 = -1;
        let mut bottom_constrained_nodes = Vec::new();

        let nodes = a.layer(layer).nodes.clone();
        for (i, &node) in nodes.iter().enumerate() {
            let constraint: InLayerConstraint =
                a.node(node).properties.get(&iprops::IN_LAYER_CONSTRAINT);

            if top_insertion_index == -1 {
                if constraint != InLayerConstraint::TOP {
                    top_insertion_index = i as i32;
                }
            } else if constraint == InLayerConstraint::TOP {
                a.node_set_layer(node, None);
                a.node_set_layer_at(node, Some(layer), top_insertion_index as usize);
                top_insertion_index += 1;
            }

            if constraint == InLayerConstraint::BOTTOM {
                bottom_constrained_nodes.push(node);
            }
        }

        for node in bottom_constrained_nodes {
            a.node_set_layer(node, None);
            a.node_set_layer(node, Some(layer));
        }
    }
    Ok(())
}
