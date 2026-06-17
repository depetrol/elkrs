//!
//! Introduces pair-wise in-layer successor constraints between NORMAL nodes
//! that carry an explicit `POSITION`, ordered by ascending y-coordinate.

use crate::core::javacompat::tim_sort;
use crate::core::options::POSITION;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mut added_constraints = false;
    let layers = a.graph(graph).layers.clone();

    for layer in layers {
        // #1 extract relevant nodes
        let mut nodes: Vec<LNodeId> = a
            .layer(layer)
            .nodes
            .iter()
            .copied()
            .filter(|&n| a.node(n).node_type == NodeType::NORMAL)
            .filter(|&n| a.node(n).properties.has(&POSITION))
            .collect();
        // #2 sort with ascending y coordinate (stable, Double.compare).
        tim_sort(&mut nodes, |&n1, &n2| {
            let y1 = a.node(n1).properties.get(&POSITION).y;
            let y2 = a.node(n2).properties.get(&POSITION).y;
            match y1.total_cmp(&y2) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            }
        });
        // #3 introduce pair-wise in-layer successor constraints (Stream.reduce).
        if !nodes.is_empty() {
            added_constraints = true;
            for window in nodes.windows(2) {
                let prev = window[0];
                let cur = window[1];
                let mut constraints: Vec<LNodeId> =
                    a.node(prev).properties.get(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS);
                constraints.push(cur);
                a.node(prev).properties.set(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS, constraints);
            }
        }
    }

    if added_constraints {
        a.graph(graph)
            .properties
            .set(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS_BETWEEN_NON_DUMMIES, true);
    }

    Ok(())
}
