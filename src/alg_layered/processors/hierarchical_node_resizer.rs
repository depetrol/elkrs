//! Resizes a child graph to fit
//! its parent node. Runs as the last non-hierarchical processor (after phase 5)
//! in a hierarchical graph.

use crate::core::options::{
    ContentAlignment, PortConstraints, PortSide, SizeConstraint, SizeOptions,
};
use crate::graph::math::KVector;
use crate::graph::properties::EnumSet;

use crate::alg_layered::graph::{LGraphArena, LGraphId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::internal_properties::Origin;
use crate::alg_layered::lgraph_util;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::GraphProperties;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // Move all nodes out of the layers and clear the layers.
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        a.graph_mut(graph).layerless_nodes.extend(nodes.iter().copied());
        a.layer_mut(layer).nodes.clear();
        for node in nodes {
            a.node_mut(node).layer = None;
        }
    }
    a.graph_mut(graph).layers.clear();

    resize_graph(a, graph);

    if a.graph(graph).parent_node.is_some() {
        graph_layout_to_node(a, graph)?;
    }
    Ok(())
}

/// Transfer the layout of the graph to the
/// associated parent node.
fn graph_layout_to_node(a: &mut LGraphArena, lgraph: LGraphId) -> Result<(), String> {
    let node = a.graph(lgraph).parent_node.unwrap();

    // Process external ports.
    let child_nodes = a.graph(lgraph).layerless_nodes.clone();
    for child_node in child_nodes {
        // The external-port dummy's ORIGIN points to the LPort on the parent
        // node it represents.
        if let Some(Origin::LPort(port)) = a.node(child_node).properties.try_get(&iprops::ORIGIN) {
            let psize = a.port(port).size;
            let port_position =
                lgraph_util::get_external_port_position(a, lgraph, child_node, psize.x, psize.y);
            a.port_mut(port).pos = port_position;
            let side: PortSide = a.node(child_node).properties.get(&iprops::EXT_PORT_SIDE);
            a.port_set_side(port, side);
        }
    }

    // Setup the parent node.
    let actual_graph_size = a.graph_actual_size(lgraph);
    let graph_props: EnumSet<GraphProperties> =
        a.graph(lgraph).properties.get(&iprops::GRAPH_PROPERTIES);
    if graph_props.contains(GraphProperties::EXTERNAL_PORTS) {
        // Ports have positions assigned.
        a.node(node)
            .properties
            .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
        let parent_graph = a.node_graph(node);
        let mut gp: EnumSet<GraphProperties> =
            a.graph(parent_graph).properties.get(&iprops::GRAPH_PROPERTIES);
        gp.add(GraphProperties::NON_FREE_PORTS);
        a.graph(parent_graph).properties.set(&iprops::GRAPH_PROPERTIES, gp);
        lgraph_util::resize_node(a, node, actual_graph_size, false, true);
    } else {
        // Ports have not been positioned yet - leave this for the next layouter.
        lgraph_util::resize_node(a, node, actual_graph_size, true, true);
    }
    Ok(())
}

fn resize_graph(a: &mut LGraphArena, lgraph: LGraphId) {
    let size_constraint: EnumSet<SizeConstraint> =
        a.graph(lgraph).properties.get(&lopts::NODE_SIZE_CONSTRAINTS);
    let size_options: EnumSet<SizeOptions> =
        a.graph(lgraph).properties.get(&lopts::NODE_SIZE_OPTIONS);

    let calculated_size = a.graph_actual_size(lgraph);
    let mut adjusted_size = calculated_size;

    if size_constraint.contains(SizeConstraint::MINIMUM_SIZE) {
        let mut min_size: KVector = a.graph(lgraph).properties.get(&lopts::NODE_SIZE_MINIMUM);
        if size_options.contains(SizeOptions::DEFAULT_MINIMUM_SIZE) {
            if min_size.x <= 0.0 {
                min_size.x = crate::core::elkutil::DEFAULT_MIN_WIDTH;
            }
            if min_size.y <= 0.0 {
                min_size.y = crate::core::elkutil::DEFAULT_MIN_HEIGHT;
            }
        }
        adjusted_size.x = f64::max(calculated_size.x, min_size.x);
        adjusted_size.y = f64::max(calculated_size.y, min_size.y);
    }

    resize_graph_no_really_i_mean_it(a, lgraph, calculated_size, adjusted_size);
}

fn resize_graph_no_really_i_mean_it(
    a: &mut LGraphArena,
    lgraph: LGraphId,
    old_size: KVector,
    new_size: KVector,
) {
    let content_alignment: EnumSet<ContentAlignment> =
        a.graph(lgraph).properties.get(&lopts::CONTENT_ALIGNMENT);

    // horizontal alignment
    if new_size.x > old_size.x {
        if content_alignment.contains(ContentAlignment::H_CENTER) {
            a.graph_mut(lgraph).offset.x += (new_size.x - old_size.x) / 2.0;
        } else if content_alignment.contains(ContentAlignment::H_RIGHT) {
            a.graph_mut(lgraph).offset.x += new_size.x - old_size.x;
        }
    }

    // vertical alignment
    if new_size.y > old_size.y {
        if content_alignment.contains(ContentAlignment::V_CENTER) {
            a.graph_mut(lgraph).offset.y += (new_size.y - old_size.y) / 2.0;
        } else if content_alignment.contains(ContentAlignment::V_BOTTOM) {
            a.graph_mut(lgraph).offset.y += new_size.y - old_size.y;
        }
    }

    // correct eastern / southern external ports if the graph grew
    let graph_props: EnumSet<GraphProperties> =
        a.graph(lgraph).properties.get(&iprops::GRAPH_PROPERTIES);
    if graph_props.contains(GraphProperties::EXTERNAL_PORTS)
        && (new_size.x > old_size.x || new_size.y > old_size.y)
    {
        let nodes = a.graph(lgraph).layerless_nodes.clone();
        for node in nodes {
            if a.node(node).node_type == NodeType::EXTERNAL_PORT {
                let ext_port_side: PortSide = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
                if ext_port_side == PortSide::EAST {
                    a.node_mut(node).pos.x += new_size.x - old_size.x;
                } else if ext_port_side == PortSide::SOUTH {
                    a.node_mut(node).pos.y += new_size.y - old_size.y;
                }
            }
        }
    }

    // Actually apply the new size.
    let padding = a.graph(lgraph).padding;
    a.graph_mut(lgraph).size.x = new_size.x - padding.left - padding.right;
    a.graph_mut(lgraph).size.y = new_size.y - padding.top - padding.bottom;
}
