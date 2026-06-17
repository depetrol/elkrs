//!
//! A node placer that keeps the pre-existing y coordinates of nodes. As far as
//! dummy nodes are concerned, the interactive node placer tries to compute
//! sensible coordinates for them based on the pre-existing routing of the edges
//! they represent. If nodes overlap, they are moved as far down as necessary to
//! remove the overlaps.

use crate::alg_layered::graph::{LGraphArena, LGraphId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::spacings;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // Place the nodes in each layer
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        place_nodes(a, graph, layer);
    }
    Ok(())
}

/// Places the nodes in the given layer.
fn place_nodes(a: &mut LGraphArena, graph: LGraphId, layer: LayerId) {
    // The minimum value for the next valid y coordinate
    let mut min_valid_y = f64::NEG_INFINITY;

    // The node type of the last node
    let mut prev_node_type = NodeType::NORMAL;

    let nodes = a.layer(layer).nodes.clone();
    for node in nodes {
        // Check which kind of node it is
        let node_type = a.node(node).node_type;
        if node_type != NodeType::NORMAL {
            // While normal nodes have their original position already in them,
            // with dummy nodes it's more complicated. Check if the interactive
            // crossing minimizer has calculated an original position for the
            // dummy node. If not, we compute one.
            let original_y: Option<f64> =
                a.node(node).properties.try_get(&iprops::ORIGINAL_DUMMY_NODE_POSITION);

            match original_y {
                None => {
                    // Make sure that the minimum valid Y position is usable
                    min_valid_y = min_valid_y.max(0.0);
                    a.node_mut(node).pos.y = min_valid_y
                        + spacings::vertical_spacing_by_type(a, graph, node_type, prev_node_type);
                }
                Some(y) => {
                    a.node_mut(node).pos.y = y;
                }
            }
        }

        // If the node extends into nodes we already placed above, we need to
        // move it down
        let spacing = spacings::vertical_spacing_by_type(a, graph, node_type, prev_node_type);
        let margin_top = a.node(node).margin.top;
        if a.node(node).pos.y < min_valid_y + spacing + margin_top {
            a.node_mut(node).pos.y = min_valid_y + spacing + margin_top;
        }

        // Update minimum valid y coordinate and remember node type
        min_valid_y = a.node(node).pos.y + a.node(node).size.y + a.node(node).margin.bottom;
        prev_node_type = node_type;
    }
}
