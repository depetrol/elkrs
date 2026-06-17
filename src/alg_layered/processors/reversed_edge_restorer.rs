
use crate::alg_layered::graph::{LGraphArena, LGraphId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::lgraph_util::edge_reverse;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let layers = a.graph(graph).layers.clone();
    for layer in layers {
        let nodes = a.layer(layer).nodes.clone();
        for node in nodes {
            let ports = a.node(node).ports.clone();
            for port in ports {
                let outgoing = a.port(port).outgoing_edges.clone();
                for edge in outgoing {
                    if a.edge(edge).properties.get(&iprops::REVERSED) {
                        edge_reverse(a, graph, edge, false);
                    }
                }
            }
        }
    }
    Ok(())
}
