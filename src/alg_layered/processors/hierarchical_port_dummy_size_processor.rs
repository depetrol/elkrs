//! Sets the width of hierarchical
//! port dummies and the layer alignment of north/south port dummies to CENTER.

use crate::core::options::{Alignment, PortSide};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let edge_spacing: f64 = a
        .graph(graph)
        .properties
        .get(&lopts::SPACING_EDGE_EDGE_BETWEEN_LAYERS);
    let delta = edge_spacing * 2.0;

    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let mut northern: Vec<LNodeId> = Vec::new();
        let mut southern: Vec<LNodeId> = Vec::new();

        for &node in &a.layer(layer).nodes {
            if a.node(node).node_type == NodeType::EXTERNAL_PORT {
                let side: PortSide = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
                if side == PortSide::NORTH {
                    northern.push(node);
                } else if side == PortSide::SOUTH {
                    southern.push(node);
                }
            }
        }

        set_widths(a, &northern, true, delta);
        set_widths(a, &southern, false, delta);
    }

    Ok(())
}

fn set_widths(a: &mut LGraphArena, nodes: &[LNodeId], top_down: bool, delta: f64) {
    let mut current_width = 0.0;
    let mut step = delta;
    if !top_down {
        current_width = delta * (nodes.len() as f64 - 1.0);
        step *= -1.0;
    }

    for &node in nodes {
        a.node(node).properties.set(&lopts::ALIGNMENT, Alignment::CENTER);
        a.node_mut(node).size.x = current_width;

        for port in a.node_ports_on_side(node, PortSide::EAST) {
            a.port_mut(port).pos.x = current_width;
        }

        current_width += step;
    }
}
