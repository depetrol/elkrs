//! Restores self loop ports and computes
//! self loop types. Does not restore the self loops themselves.

use crate::core::options::PortConstraints;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::loops::{ordering, SelfLoopHolder};

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // Process every node that actually has self loops
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for l_node in nodes {
            if a.node(l_node).node_type == NodeType::NORMAL
                && a.node(l_node).self_loop_holder.is_some()
            {
                // Take the holder out of the node for the duration of the
                // processing
                let mut sl_holder = a.node_mut(l_node).self_loop_holder.take().unwrap();
                process_node(a, l_node, &mut sl_holder);
                a.node_mut(l_node).self_loop_holder = Some(sl_holder);
            }
        }
    }
    Ok(())
}

fn process_node(a: &mut LGraphArena, l_node: LNodeId, sl_holder: &mut SelfLoopHolder) {
    // Restore and order pure self loop ports if they were previously hidden
    if sl_holder.are_ports_hidden {
        match a
            .node(l_node)
            .properties
            .get::<PortConstraints>(&iprops::ORIGINAL_PORT_CONSTRAINTS)
        {
            PortConstraints::UNDEFINED | PortConstraints::FREE => {
                // We need to assign port sides first and then fall through to
                // restore ports
                ordering::assign_port_sides(a, sl_holder);

                compute_self_loop_types(a, sl_holder);
                ordering::restore_ports(a, sl_holder);
            }

            PortConstraints::FIXED_SIDE => {
                // Restore ports (which by now have port sides assigned to
                // them). After this call, arePortsHidden() will report false
                compute_self_loop_types(a, sl_holder);
                ordering::restore_ports(a, sl_holder);
            }

            _ => {
                // This should not happen. If ports were hidden this must have
                // been because their order was not fixed
                debug_assert!(false);
            }
        }
    } else {
        // Ensure that self loops types are computed
        compute_self_loop_types(a, sl_holder);
    }
}

fn compute_self_loop_types(a: &LGraphArena, sl_holder: &mut SelfLoopHolder) {
    for sl_loop in 0..sl_holder.sl_hyper_loops.len() {
        sl_holder.compute_ports_per_side(a, sl_loop);
    }
}
