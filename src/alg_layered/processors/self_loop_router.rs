//! Computes bend points for self loops and places
//! self loop labels.

use crate::core::javacompat::JavaRandom;
use crate::core::options::EdgeRouting;

use crate::alg_layered::graph::{LGraphArena, LGraphId, NodeType};
use crate::alg_layered::loops::routing;
use crate::alg_layered::options_gen as lopts;

pub fn process(a: &mut LGraphArena, graph: LGraphId, random: &mut JavaRandom) -> Result<(), String> {
    // `routerForGraph`
    let router_kind = match a.graph(graph).properties.get(&lopts::EDGE_ROUTING) {
        EdgeRouting::POLYLINE => routing::SelfLoopRouterKind::Polyline,
        EdgeRouting::SPLINES => routing::SelfLoopRouterKind::Spline,
        _ => routing::SelfLoopRouterKind::Orthogonal,
    };

    // Process every node that actually has self loops
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for l_node in nodes {
            if a.node(l_node).node_type == NodeType::NORMAL
                && a.node(l_node).self_loop_holder.is_some()
            {
                let mut sl_holder = a.node_mut(l_node).self_loop_holder.take().unwrap();

                // Compute how each hyper loop is routed around the node
                routing::determine_loop_routes(a, &mut sl_holder);

                // Place self loop labels. This will allow the routing slot
                // assigner to make sure that no two overlapping labels end up
                // in the same slot.
                routing::place_labels(a, &mut sl_holder);

                // Find out which port side each hyper loop appears on and
                // assign routing slots such that the self loop "trunks" do
                // not intersect
                routing::assign_routing_slots(a, &mut sl_holder, random);

                // Finally route the self loops
                routing::route_self_loops(a, &mut sl_holder, router_kind);

                a.node_mut(l_node).self_loop_holder = Some(sl_holder);
            }
        }
    }
    Ok(())
}
