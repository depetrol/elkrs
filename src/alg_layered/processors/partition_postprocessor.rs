//! Removes the ports (and thereby the
//! constraint edges) added by the `PartitionMidprocessor`.

use crate::alg_layered::graph::{LGraphArena, LGraphId};
use crate::alg_layered::internal_properties as iprops;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            // Remove the port from the node's port list only; the LPort object
            // and its edges remain but become unreachable through node
            // traversal.
            let ports = a.node(node).ports.clone();
            let keep: Vec<_> = ports
                .into_iter()
                .filter(|&p| !a.port(p).properties.get(&iprops::PARTITION_DUMMY))
                .collect();
            a.node_mut(node).ports = keep;
        }
    }
    Ok(())
}
