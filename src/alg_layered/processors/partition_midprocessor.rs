//! Adds constraint edges between
//! consecutive partitions so layering adheres to the partitions.

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::core::options::PortSide;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // Collect partition IDs in use, mapping each to the nodes carrying it
    // (in layerless declaration order). We use insertion order for the
    // per-partition node lists (see README.md divergence 2 — possible
    // divergence with several nodes per partition).
    let layerless = a.graph(graph).layerless_nodes.clone();
    let mut partition_ids: Vec<i32> = Vec::new();
    let mut buckets: Vec<Vec<LNodeId>> = Vec::new();
    for node in layerless {
        if let Some(p) = a.node(node).properties.try_get(&lopts::PARTITIONING_PARTITION) {
            match partition_ids.iter().position(|&id| id == p) {
                Some(i) => buckets[i].push(node),
                None => {
                    partition_ids.push(p);
                    buckets.push(vec![node]);
                }
            }
        }
    }

    if partition_ids.is_empty() {
        return Ok(());
    }

    // Sort partition IDs ascending, keeping bucket association.
    let mut order: Vec<usize> = (0..partition_ids.len()).collect();
    order.sort_by_key(|&i| partition_ids[i]);

    // Connect each consecutive pair.
    for w in order.windows(2) {
        let first = buckets[w[0]].clone();
        let second = buckets[w[1]].clone();
        connect_nodes(a, &first, &second);
    }

    Ok(())
}

fn connect_nodes(a: &mut LGraphArena, first_partition: &[LNodeId], second_partition: &[LNodeId]) {
    for &node in first_partition {
        let source_port = a.create_port();
        a.port_set_node(source_port, Some(node));
        a.port_set_side(source_port, PortSide::EAST);
        a.port(source_port).properties.set(&iprops::PARTITION_DUMMY, true);

        for &other_node in second_partition {
            let target_port = a.create_port();
            a.port_set_node(target_port, Some(other_node));
            a.port_set_side(target_port, PortSide::WEST);
            a.port(target_port).properties.set(&iprops::PARTITION_DUMMY, true);

            let edge = a.create_edge();
            a.edge(edge).properties.set(&iprops::PARTITION_DUMMY, true);
            a.edge_set_source(edge, Some(source_port));
            a.edge_set_target(edge, Some(target_port));
        }
    }
}
