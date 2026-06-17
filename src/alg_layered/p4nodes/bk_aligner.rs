//! Block building and inner shifting.

use std::collections::HashSet;

use crate::alg_layered::graph::{LEdgeId, LGraphArena, LGraphId, NodeType};

use super::bk::{get_blocks, get_edge};
use super::bk_aligned_layout::{BKAlignedLayout, HDirection, VDirection};
use super::neighborhood_information::{nid, NeighborhoodInformation};

/// The graph is traversed in the given
/// directions and nodes are grouped into blocks. Type 1 conflicts (the
/// `marked_edges`) are resolved, so that the dummy nodes of a long edge share
/// the same block if possible.
pub fn vertical_alignment(
    a: &LGraphArena,
    graph: LGraphId,
    ni: &NeighborhoodInformation,
    bal: &mut BKAlignedLayout,
    marked_edges: &HashSet<LEdgeId>,
) {
    // Initialize root and align maps
    for &layer in &a.graph(graph).layers {
        for &v in &a.layer(layer).nodes {
            let i = nid(a, v);
            bal.root[i] = Some(v);
            bal.align[i] = Some(v);
            bal.inner_shift[i] = Some(0.0);
        }
    }

    // If the horizontal direction is LEFT, the layers are traversed from
    // right to left, thus a reverse iterator is needed
    let mut layers = a.graph(graph).layers.clone();
    if bal.hdir == Some(HDirection::Left) {
        layers.reverse();
    }

    for layer in layers {
        // r denotes the position in layer order where the last block was
        // found. It is initialized with -1, since nothing is found and the
        // ordering starts with 0.
        let mut r: i32 = -1;
        let mut nodes = a.layer(layer).nodes.clone();

        if bal.vdir == Some(VDirection::Up) {
            // If the alignment direction is UP, the nodes in a layer are
            // traversed reversely, thus we start at INT_MAX and with the
            // reversed list of nodes.
            r = i32::MAX;
            nodes.reverse();
        }

        // i denotes the index of the layer and k the position of the node
        // within the layer. m denotes the position of a neighbor in the
        // neighbor list of a node.
        for v_i_k in nodes {
            let neighbors = if bal.hdir == Some(HDirection::Left) {
                &ni.right_neighbors[nid(a, v_i_k)]
            } else {
                &ni.left_neighbors[nid(a, v_i_k)]
            };

            if !neighbors.is_empty() {
                // When a node has many upper neighbors, consider only the
                // (two) nodes in the middle.
                let d = neighbors.len() as i32;
                let low = (((d as f64 + 1.0) / 2.0).floor() as i32) - 1;
                let high = (((d as f64 + 1.0) / 2.0).ceil() as i32) - 1;

                // Check whether v_i_k can be added to a block of its
                // upper/lower neighbor(s); m iterates from high down to low
                // for UP and from low up to high otherwise.
                let ms: Vec<i32> = if bal.vdir == Some(VDirection::Up) {
                    (low..=high).rev().collect()
                } else {
                    (low..=high).collect()
                };
                for m in ms {
                    if bal.align[nid(a, v_i_k)] == Some(v_i_k) {
                        let (u_m, u_m_edge) = neighbors[m as usize];

                        let r_ok = if bal.vdir == Some(VDirection::Up) {
                            r > ni.node_index[nid(a, u_m)]
                        } else {
                            r < ni.node_index[nid(a, u_m)]
                        };
                        if !marked_edges.contains(&u_m_edge) && r_ok {
                            bal.align[nid(a, u_m)] = Some(v_i_k);
                            bal.root[nid(a, v_i_k)] = bal.root[nid(a, u_m)];
                            bal.align[nid(a, v_i_k)] = bal.root[nid(a, v_i_k)];
                            let root_i = nid(a, bal.root[nid(a, v_i_k)].unwrap());
                            bal.od[root_i] =
                                bal.od[root_i] && a.node(v_i_k).node_type == NodeType::LONG_EDGE;

                            r = ni.node_index[nid(a, u_m)];
                        }
                    }
                }
            }
        }
    }
}

/// Moves the nodes inside a block,
/// ensuring that all edges inside a block can be drawn as straight lines.
/// Also determines the required size of each block.
pub fn inside_block_shift(a: &LGraphArena, graph: LGraphId, bal: &mut BKAlignedLayout) {
    let blocks = get_blocks(a, graph, bal);
    for &root in blocks.keys() {
        // For each block, we place the top left corner of the root node at
        // coordinate (0,0). We then calculate the space required above the
        // top left corner (due to other nodes placed above and to top margins
        // of nodes, including the root node) and the space required below the
        // top left corner. The sum of both becomes the block size, and the y
        // coordinate of each node relative to the block's top border becomes
        // the inner shift of that node.

        // Reserve space for the root node
        let mut space_above = a.node(root).margin.top;
        let mut space_below = a.node(root).size.y + a.node(root).margin.bottom;
        bal.inner_shift[nid(a, root)] = Some(0.0);

        // Iterate over all other nodes of the block
        let mut current = root;
        loop {
            let next = bal.align[nid(a, current)].unwrap();
            if next == root {
                break;
            }
            // Find the edge between the current and the next node
            let edge = get_edge(a, current, next).unwrap();
            let src = a.edge(edge).source.unwrap();
            let tgt = a.edge(edge).target.unwrap();

            // Calculate the y coordinate difference between the two nodes
            // required to straighten the edge
            let port_pos_diff = if bal.hdir == Some(HDirection::Left) {
                a.port(tgt).pos.y + a.port(tgt).anchor.y
                    - a.port(src).pos.y
                    - a.port(src).anchor.y
            } else {
                a.port(src).pos.y + a.port(src).anchor.y
                    - a.port(tgt).pos.y
                    - a.port(tgt).anchor.y
            };

            // The current node already has an inner shift value that we need
            // to use as the basis to calculate the next node's inner shift
            let next_inner_shift = bal.inner_shift[nid(a, current)].unwrap() + port_pos_diff;
            bal.inner_shift[nid(a, next)] = Some(next_inner_shift);

            // Update the space required above and below the root node's top
            // left corner
            space_above = f64::max(space_above, a.node(next).margin.top - next_inner_shift);
            space_below = f64::max(
                space_below,
                next_inner_shift + a.node(next).size.y + a.node(next).margin.bottom,
            );

            // The next node is the current node in the next iteration
            current = next;
        }

        // Adjust each node's inner shift by the space required above the root
        // node's top left corner (which the inner shifts are relative to at
        // the moment)
        let mut current = root;
        loop {
            let i = nid(a, current);
            bal.inner_shift[i] = Some(bal.inner_shift[i].unwrap() + space_above);
            current = bal.align[i].unwrap();
            if current == root {
                break;
            }
        }

        // Remember the block size
        bal.block_size[nid(a, root)] = Some(space_above + space_below);
    }
}
