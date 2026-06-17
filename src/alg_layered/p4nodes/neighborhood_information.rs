//! Precalculated
//! neighborhood information for the BK node placer.

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, LNodeId, LayerId};
use crate::alg_layered::options_gen as lopts;

/// Scratch id of a node (assigned by [`NeighborhoodInformation::build_for`]);
/// used to index the placer's per-node arrays (`n.id`).
#[inline]
pub fn nid(a: &LGraphArena, n: LNodeId) -> usize {
    a.node(n).id as usize
}

/// Scratch id of a layer (`l.id`).
#[inline]
pub fn lid(a: &LGraphArena, l: LayerId) -> usize {
    a.layer(l).id as usize
}

/// Neighborhood information for a layered graph used during bk node placing.
pub struct NeighborhoodInformation {
    /// Number of nodes in the graph.
    pub node_count: usize,
    /// For a layer `l` the entry at `layer_index[lid(l)]` holds the index of `l`.
    pub layer_index: Vec<i32>,
    /// For a node `n` the entry at `node_index[nid(n)]` holds the index of `n`
    /// within its layer.
    pub node_index: Vec<i32>,
    /// For a node `n`, `left_neighbors[nid(n)]` holds a list with all left
    /// neighbors along with any edge that connects `n` to its neighbor.
    pub left_neighbors: Vec<Vec<(LNodeId, LEdgeId)>>,
    /// See [`Self::left_neighbors`].
    pub right_neighbors: Vec<Vec<(LNodeId, LEdgeId)>>,
}

impl NeighborhoodInformation {
    pub fn build_for(a: &mut LGraphArena, graph: LGraphId) -> Self {
        let layers = a.graph(graph).layers.clone();

        let mut node_count = 0usize;
        for &layer in &layers {
            node_count += a.layer(layer).nodes.len();
        }

        // cache indexes of layers and of nodes
        let mut layer_index = vec![0i32; layers.len()];
        let mut node_index = vec![0i32; node_count];
        let mut l_id = 0i32;
        let mut l_index = 0i32;
        let mut n_id = 0i32;
        for &l in &layers {
            a.layer_mut(l).id = l_id;
            l_id += 1;
            layer_index[lid(a, l)] = l_index;
            l_index += 1;
            let mut n_index = 0i32;
            let nodes = a.layer(l).nodes.clone();
            for n in nodes {
                a.node_mut(n).id = n_id;
                n_id += 1;
                node_index[nid(a, n)] = n_index;
                n_index += 1;
            }
        }

        let mut ni = NeighborhoodInformation {
            node_count,
            layer_index,
            node_index,
            left_neighbors: Vec::with_capacity(node_count),
            right_neighbors: Vec::with_capacity(node_count),
        };

        // determine all left and right neighbors of the graph's nodes
        ni.determine_all_left_neighbors(a, &layers);
        ni.determine_all_right_neighbors(a, &layers);

        ni
    }

    /// Gives all left neighbors (originally known as upper neighbors) of a
    /// given node: nodes in a previous layer with an edge pointing to it.
    fn determine_all_left_neighbors(&mut self, a: &LGraphArena, layers: &[LayerId]) {
        for &l in layers {
            for &n in &a.layer(l).nodes {
                let mut result: Vec<(LNodeId, LEdgeId)> = Vec::new();
                let mut max_priority = 0i32;

                for edge in a.node_incoming_edges(n) {
                    if a.edge_is_self_loop(edge) || a.edge_is_in_layer(edge) {
                        continue;
                    }
                    let edge_prio: i32 =
                        a.edge(edge).properties.get(&lopts::PRIORITY_STRAIGHTNESS);
                    if edge_prio > max_priority {
                        max_priority = edge_prio;
                        result.clear();
                    }
                    if edge_prio == max_priority {
                        result.push((a.edge_source_node(edge), edge));
                    }
                }

                // stable sort by the neighbor's index within its layer.
                result.sort_by_key(|&(neighbor, _)| self.node_index[nid(a, neighbor)]);

                self.left_neighbors.push(result);
            }
        }
    }

    /// Gives all right neighbors (originally known as lower neighbors) of a
    /// given node: nodes in a following layer with an edge coming from it.
    fn determine_all_right_neighbors(&mut self, a: &LGraphArena, layers: &[LayerId]) {
        for &l in layers {
            for &n in &a.layer(l).nodes {
                let mut result: Vec<(LNodeId, LEdgeId)> = Vec::new();
                let mut max_priority = 0i32;

                for edge in a.node_outgoing_edges(n) {
                    if a.edge_is_self_loop(edge) || a.edge_is_in_layer(edge) {
                        continue;
                    }
                    let edge_prio: i32 =
                        a.edge(edge).properties.get(&lopts::PRIORITY_STRAIGHTNESS);
                    if edge_prio > max_priority {
                        max_priority = edge_prio;
                        result.clear();
                    }
                    if edge_prio == max_priority {
                        result.push((a.edge_target_node(edge), edge));
                    }
                }

                result.sort_by_key(|&(neighbor, _)| self.node_index[nid(a, neighbor)]);

                self.right_neighbors.push(result);
            }
        }
    }
}
