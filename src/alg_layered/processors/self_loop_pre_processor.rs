//! Finds regular nodes with self loops and
//! preprocesses those loops.
//!
//! Postconditions: each node with self loops has a `SelfLoopHolder` stored in
//! `LNode::self_loop_holder` (the port of the `SELF_LOOP_HOLDER` property);
//! all self loop edges are removed from their ports; unless port orders are
//! fixed, all ports with only self loop edges are removed from their nodes.

use crate::core::options::PortConstraints;
use crate::graph::properties::EnumSet;

use crate::alg_layered::graph::{LGraphArena, LGraphId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::loops::SelfLoopHolder;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::GraphProperties;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let nodes = a.graph(graph).layerless_nodes.clone();
    for lnode in nodes {
        if SelfLoopHolder::needs_self_loop_processing(a, lnode) {
            let mut sl_holder = SelfLoopHolder::create(a, lnode);
            hide_self_loops(a, &sl_holder);
            hide_ports(a, &mut sl_holder);
            a.node_mut(lnode).self_loop_holder = Some(Box::new(sl_holder));
        }
    }
    Ok(())
}

/// `hideSelfLoops` / `hideSelfLoop`: hides all self loop edges by removing
/// them from their ports, to be restored once edge routing has finished.
fn hide_self_loops(a: &mut LGraphArena, sl_holder: &SelfLoopHolder) {
    for sl_loop in &sl_holder.sl_hyper_loops {
        for &sl_edge in &sl_loop.sl_edges {
            let l_edge = sl_holder.sl_edges[sl_edge].l_edge;
            a.edge_set_source(l_edge, None);
            a.edge_set_target(l_edge, None);
        }
    }
}

/// `hidePorts`: possibly hides all ports whose only incident edges are self
/// loops. This is only done if port constraints are not at least
/// `FIXED_ORDER`.
fn hide_ports(a: &mut LGraphArena, sl_holder: &mut SelfLoopHolder) {
    let l_node = sl_holder.l_node;

    /* There are two cases in which we want to refrain from hiding ports:
     * 1. The port order is already fixed.
     * 2. The self loop holder has another graph inside of it which contains
     *    external ports.
     */
    let order_fixed = a
        .node(l_node)
        .properties
        .get::<PortConstraints>(&lopts::PORT_CONSTRAINTS)
        .is_order_fixed();
    let hierarchy_mode = match a.node(l_node).nested_graph {
        Some(nested) => a
            .graph(nested)
            .properties
            .get::<EnumSet<GraphProperties>>(&iprops::GRAPH_PROPERTIES)
            .contains(GraphProperties::EXTERNAL_PORTS),
        None => false,
    };

    if order_fixed || hierarchy_mode {
        // No need to hide any ports
        return;
    }

    for sl_port_idx in 0..sl_holder.sl_ports.len() {
        if sl_holder.sl_ports[sl_port_idx].had_only_self_loops {
            // Hide the port
            let l_port = sl_holder.sl_ports[sl_port_idx].l_port;
            a.port_set_node(l_port, None);

            // Remember that we actually did so
            sl_holder.sl_ports[sl_port_idx].hidden = true;
            sl_holder.are_ports_hidden = true;

            debug_assert!(!a.port(l_port).properties.has(&iprops::PORT_DUMMY));
        }
    }
}
