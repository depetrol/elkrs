//! Sets the y coordinate of
//! east/west external port dummies.

use crate::core::options::{PortConstraints, PortSide};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let layers = a.graph(graph).layers.clone();
    if !layers.is_empty() {
        fix_coordinates(a, layers[0], graph);
    }
    if layers.len() > 1 {
        fix_coordinates(a, layers[layers.len() - 1], graph);
    }
    Ok(())
}

fn fix_coordinates(a: &mut LGraphArena, layer: LayerId, graph: LGraphId) {
    let port_constraints: PortConstraints = a.graph(graph).properties.get(&lopts::PORT_CONSTRAINTS);
    if !(port_constraints.is_ratio_fixed() || port_constraints.is_pos_fixed()) {
        return;
    }

    let graph_height = a.graph_actual_size(graph).y;

    let nodes = a.layer(layer).nodes.clone();
    for node in nodes {
        if a.node(node).node_type != NodeType::EXTERNAL_PORT {
            continue;
        }
        let ext_port_side: PortSide = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
        if ext_port_side != PortSide::EAST && ext_port_side != PortSide::WEST {
            continue;
        }

        let mut final_y: f64 = a.node(node).properties.get(&iprops::PORT_RATIO_OR_POSITION);
        if port_constraints == PortConstraints::FIXED_RATIO {
            final_y *= graph_height;
        }

        let anchor_y = a
            .node(node)
            .properties
            .get::<crate::graph::math::KVector>(&lopts::PORT_ANCHOR)
            .y;
        a.node_mut(node).pos.y = final_y - anchor_y;
        a.node_border_to_content_area_coordinates(node, false, true);
    }
}
