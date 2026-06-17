//! Offsets the label cells computed by the
//! `EndLabelPreprocessor` by the final node coordinates and applies label
//! positions.

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // We iterate over each node's label cells and offset and place them
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            let node_type = a.node(node).node_type;
            if (node_type == NodeType::NORMAL || node_type == NodeType::EXTERNAL_PORT)
                && a.node(node).properties.has(&iprops::END_LABELS)
            {
                process_node(a, node);
            }
        }
    }
    Ok(())
}

fn process_node(a: &mut LGraphArena, node: LNodeId) {
    // The node should have a non-empty list of label cells, or something went
    // TERRIBLY WRONG!!!
    let mut end_label_cells = a
        .node(node)
        .properties
        .try_get(&iprops::END_LABELS)
        .expect("node without END_LABELS in end label postprocessing");
    debug_assert!(!end_label_cells.0.is_empty());

    let node_pos = a.node(node).pos;

    for (_, label_cell) in &mut end_label_cells.0 {
        label_cell.rect.move_by(node_pos);
        label_cell.apply_label_layout(a);
    }

    // Remove label cells
    a.node(node).properties.unset(&iprops::END_LABELS);
}
