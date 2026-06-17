
use crate::alg_common::jhash::JavaHashSet;
use crate::alg_common::spore::Node;
use crate::alg_common::tree::Forest;
use crate::alg_common::triangulation::TEdge;

use crate::alg_spore::options::{CompactionStrategy, TreeConstructionStrategy};

/// The SPOrE graph. Node identity is the index into `vertices`.
pub struct Graph {
    /// All vertices of this graph.
    pub vertices: Vec<Node>,
    /// All edges of this graph (`null` until the structure phase ran).
    pub t_edges: Option<JavaHashSet<TEdge>>,
    /// Determines the kind of spanning tree to be generated.
    pub tree_construction_strategy: TreeConstructionStrategy,
    /// Holds the tree structure for the processing order (indices into
    /// `vertices`).
    pub tree: Option<Forest<usize>>,
    /// Determines the compaction method applied to the spanning tree.
    pub compaction_strategy: CompactionStrategy,
    /// One of the vertices can be used as the root of the tree construction.
    pub preferred_root: Option<usize>,
    /// Restricts the translation of nodes to orthogonal directions.
    pub orthogonal_compaction: bool,
    /// `InternalProperties.OVERLAPS_EXISTED` (property default: true).
    pub overlaps_existed: bool,
}

impl Graph {
    pub fn new(
        tree_construction_strategy: TreeConstructionStrategy,
        compaction_strategy: CompactionStrategy,
    ) -> Self {
        Graph {
            vertices: Vec::new(),
            t_edges: None,
            tree_construction_strategy,
            tree: None,
            compaction_strategy,
            preferred_root: None,
            orthogonal_compaction: false,
            overlaps_existed: true,
        }
    }
}
