
use crate::core::options::{PortConstraints, PortSide};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LPortId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mut nodes: Vec<LNodeId> = a.graph(graph).layerless_nodes.clone();
    for &layer in &a.graph(graph).layers.clone() {
        nodes.extend(a.layer(layer).nodes.iter().copied());
    }
    for node in nodes {
        process_node(a, node);
    }
    Ok(())
}

fn process_node(a: &mut LGraphArena, node: LNodeId) {
    let side_fixed = a
        .node(node)
        .properties
        .get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
        .is_side_fixed();
    let ports = a.node(node).ports.clone();
    if side_fixed {
        for port in ports {
            if a.port(port).side == PortSide::UNDEFINED {
                set_port_side(a, port);
            }
        }
    } else {
        for port in ports {
            set_port_side(a, port);
        }
        a.node(node)
            .properties
            .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_SIDE);
    }
}

pub fn set_port_side(a: &mut LGraphArena, port: LPortId) {
    if let Some(port_dummy) = a.port(port).properties.try_get(&iprops::PORT_DUMMY) {
        let side = a.node(port_dummy).properties.get(&iprops::EXT_PORT_SIDE);
        a.port_set_side(port, side);
    } else if a.port_net_flow(port) < 0 {
        a.port_set_side(port, PortSide::EAST);
    } else {
        a.port_set_side(port, PortSide::WEST);
    }
}
