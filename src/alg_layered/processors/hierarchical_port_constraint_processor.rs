//! Handles ordering constraints
//! for east/west hierarchical port dummies and replaces north/south
//! hierarchical port dummies by temporary per-layer dummies.

use crate::core::options::{Alignment, PortConstraints, PortSide};

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, LayerId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;

const DUMMY_INPUT_PORT: usize = 0;
const DUMMY_OUTPUT_PORT: usize = 1;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    process_eastern_and_western_port_dummies(a, graph);
    process_northern_and_southern_port_dummies(a, graph);
    Ok(())
}

fn process_eastern_and_western_port_dummies(a: &mut LGraphArena, graph: LGraphId) {
    let pc: PortConstraints = a.graph(graph).properties.get(&lopts::PORT_CONSTRAINTS);
    if !pc.is_order_fixed() {
        return;
    }
    let layers = a.graph(graph).layers.clone();
    if layers.is_empty() {
        return;
    }
    process_ew_layer(a, layers[0]);
    process_ew_layer(a, layers[layers.len() - 1]);
}

fn process_ew_layer(a: &mut LGraphArena, layer: LayerId) {
    let mut nodes = a.layer(layer).nodes.clone();

    // Sort: external-port dummies to the top, by PORT_RATIO_OR_POSITION ascending.
    // Non-external nodes compare as "greater" (sorted to bottom).
    nodes.sort_by(|&n1, &n2| {
        use std::cmp::Ordering;
        let t1 = a.node(n1).node_type == NodeType::EXTERNAL_PORT;
        let t2 = a.node(n2).node_type == NodeType::EXTERNAL_PORT;
        if !t2 {
            return Ordering::Less;
        } else if !t1 {
            return Ordering::Greater;
        }
        let p1: f64 = a.node(n1).properties.get(&iprops::PORT_RATIO_OR_POSITION);
        let p2: f64 = a.node(n2).properties.get(&iprops::PORT_RATIO_OR_POSITION);
        p1.partial_cmp(&p2).unwrap_or(Ordering::Equal)
    });

    let mut last_dummy: Option<LNodeId> = None;
    for node in nodes {
        if a.node(node).node_type != NodeType::EXTERNAL_PORT {
            break;
        }
        let side: PortSide = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
        if side != PortSide::WEST && side != PortSide::EAST {
            continue;
        }
        if let Some(last) = last_dummy {
            let mut succ: Vec<LNodeId> = a
                .node(last)
                .properties
                .get(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS);
            succ.push(node);
            a.node(last)
                .properties
                .set(&iprops::IN_LAYER_SUCCESSOR_CONSTRAINTS, succ);
        }
        last_dummy = Some(node);
    }
}

fn process_northern_and_southern_port_dummies(a: &mut LGraphArena, graph: LGraphId) {
    let pc: PortConstraints = a.graph(graph).properties.get(&lopts::PORT_CONSTRAINTS);
    if !pc.is_side_fixed() {
        return;
    }

    let layer_count = a.graph(graph).layers.len();

    // For each (current + 1) "logical" layer index we keep a map from origin to
    // the dummy node we created, and a list of new dummy nodes for that layer.
    // Index 0 corresponds to a possible new first layer; index i+1 corresponds
    // to existing layer i; index layer_count+1 corresponds to a possible new
    // last layer.
    // These maps could be keyed on the original dummy's ORIGIN (an LPort). Since
    // each original north/south dummy maps 1:1 to its origin port, keying on the
    // original dummy node id is equivalent and avoids hashing Origin.
    let mut ext_port_to_dummy: Vec<indexmap::IndexMap<LNodeId, LNodeId>> =
        Vec::with_capacity(layer_count + 2);
    let mut new_dummy_nodes: Vec<Vec<LNodeId>> = Vec::with_capacity(layer_count + 2);
    ext_port_to_dummy.push(indexmap::IndexMap::new());
    ext_port_to_dummy.push(indexmap::IndexMap::new());
    new_dummy_nodes.push(Vec::new());
    new_dummy_nodes.push(Vec::new());

    let mut original_dummies: Vec<LNodeId> = Vec::new();

    let layers = a.graph(graph).layers.clone();
    for (curr_idx, &current_layer) in layers.iter().enumerate() {
        // add the maps/lists for the next layer
        ext_port_to_dummy.push(indexmap::IndexMap::new());
        new_dummy_nodes.push(Vec::new());

        let nodes = a.layer(current_layer).nodes.clone();
        for current_node in nodes {
            if is_north_south_dummy(a, current_node) {
                original_dummies.push(current_node);
                continue;
            }

            // incoming edges
            let in_edges = a.node_incoming_edges(current_node);
            for edge in in_edges {
                let source_node = a.edge_source_node(edge);
                if !is_north_south_dummy(a, source_node) {
                    continue;
                }
                let prev_dummy = match ext_port_to_dummy[curr_idx].get(&source_node).copied() {
                    Some(d) => d,
                    None => {
                        let d = create_dummy(a, graph, source_node);
                        ext_port_to_dummy[curr_idx].insert(source_node, d);
                        new_dummy_nodes[curr_idx].push(d);
                        d
                    }
                };
                let out_port = a.node(prev_dummy).ports[DUMMY_OUTPUT_PORT];
                a.edge_set_source(edge, Some(out_port));
            }

            // outgoing edges
            let out_edges = a.node_outgoing_edges(current_node);
            for edge in out_edges {
                let target_node = a.edge_target_node(edge);
                if !is_north_south_dummy(a, target_node) {
                    continue;
                }
                let next_dummy = match ext_port_to_dummy[curr_idx + 2].get(&target_node).copied() {
                    Some(d) => d,
                    None => {
                        let d = create_dummy(a, graph, target_node);
                        ext_port_to_dummy[curr_idx + 2].insert(target_node, d);
                        new_dummy_nodes[curr_idx + 2].push(d);
                        d
                    }
                };
                let in_port = a.node(next_dummy).ports[DUMMY_INPUT_PORT];
                a.edge_set_target(edge, Some(in_port));
            }
        }
    }

    // Add newly created dummy nodes to their layers.
    for i in 0..new_dummy_nodes.len() {
        let node_list = new_dummy_nodes[i].clone();
        if node_list.is_empty() {
            continue;
        }
        let layer = if i == 0 {
            let l = a.create_layer(graph);
            a.graph_mut(graph).layers.insert(0, l);
            l
        } else if i == ext_port_to_dummy.len() - 1 {
            let l = a.create_layer(graph);
            a.graph_mut(graph).layers.push(l);
            l
        } else {
            a.graph(graph).layers[i - 1]
        };
        for dummy in node_list {
            a.node_set_layer(dummy, Some(layer));
        }
    }

    // Remove original dummies from layers.
    for original_dummy in &original_dummies {
        a.node_set_layer(*original_dummy, None);
    }

    a.graph(graph)
        .properties
        .set(&iprops::EXT_PORT_REPLACED_DUMMIES, original_dummies);
}

fn is_north_south_dummy(a: &LGraphArena, node: LNodeId) -> bool {
    if a.node(node).node_type == NodeType::EXTERNAL_PORT {
        let side: PortSide = a.node(node).properties.get(&iprops::EXT_PORT_SIDE);
        side == PortSide::NORTH || side == PortSide::SOUTH
    } else {
        false
    }
}

fn create_dummy(a: &mut LGraphArena, graph: LGraphId, original_dummy: LNodeId) -> LNodeId {
    let new_dummy = a.create_node(graph);
    let props = a.node(original_dummy).properties.clone();
    a.node(new_dummy).properties.copy_from(&props);
    a.node(new_dummy)
        .properties
        .set(&iprops::EXT_PORT_REPLACED_DUMMY, original_dummy);
    a.node(new_dummy)
        .properties
        .set(&lopts::PORT_CONSTRAINTS, PortConstraints::FIXED_POS);
    a.node(new_dummy).properties.set(&lopts::ALIGNMENT, Alignment::CENTER);
    a.node_mut(new_dummy).node_type = NodeType::EXTERNAL_PORT;

    let input_port = a.create_port();
    a.port_set_node(input_port, Some(new_dummy));
    a.port_set_side(input_port, PortSide::WEST);

    let output_port = a.create_port();
    a.port_set_node(output_port, Some(new_dummy));
    a.port_set_side(output_port, PortSide::EAST);

    new_dummy
}
