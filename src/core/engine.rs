
use crate::graph::graph::{EdgeId, ElkGraph, NodeId};

use crate::core::elkutil;
use crate::core::options::*;
use crate::core::registry::{AlgorithmRegistry, GraphFeature};

pub const DEFAULT_ALGORITHM_ID: &str = "org.eclipse.elk.layered";

pub struct RecursiveGraphLayoutEngine<'r> {
    pub registry: &'r AlgorithmRegistry,
}

impl<'r> RecursiveGraphLayoutEngine<'r> {
    pub fn new(registry: &'r AlgorithmRegistry) -> Self {
        RecursiveGraphLayoutEngine { registry }
    }

    pub fn layout(&self, g: &mut ElkGraph) -> Result<(), String> {
        // TODO DeprecatedLayoutOptionReplacer (only affects deprecated inputs)
        if !g.node(g.root).properties.has(&RESOLVED_ALGORITHM_TYPED) {
            self.resolve_algorithms(g)?;
        }
        let root = g.root;
        self.layout_recursively(g, root)?;
        Ok(())
    }

    fn resolve_algorithms(&self, g: &mut ElkGraph) -> Result<(), String> {
        let mut nodes = vec![g.root];
        nodes.extend(g.descendants(g.root));
        for node in nodes {
            if g.node(node).properties.get(&NO_LAYOUT) {
                continue;
            }
            let algorithm_id: String = g.node(node).properties.get(&ALGORITHM);
            if let Some(data) = self.registry.by_suffix(&algorithm_id) {
                let resolved = ResolvedAlgorithm(data.id.to_string());
                g.node_mut(node).properties.set(&RESOLVED_ALGORITHM_TYPED, resolved);
                continue;
            }
            // must resolve if the node has children or inside self loops
            let must_resolve = !g.node(node).properties.has(&RESOLVED_ALGORITHM_TYPED)
                && (!g.node(node).children.is_empty()
                    || g.node(node).properties.get(&INSIDE_SELF_LOOPS_ACTIVATE));
            if must_resolve {
                if algorithm_id.trim().is_empty() {
                    if let Some(data) = self.registry.by_suffix(DEFAULT_ALGORITHM_ID) {
                        let resolved = ResolvedAlgorithm(data.id.to_string());
                        g.node_mut(node).properties.set(&RESOLVED_ALGORITHM_TYPED, resolved);
                    } else {
                        return Err(format!(
                            "Unable to load default layout algorithm {DEFAULT_ALGORITHM_ID} \
                             for unconfigured node {:?}",
                            g.node(node).identifier
                        ));
                    }
                } else {
                    return Err(format!(
                        "Layout algorithm '{algorithm_id}' not found for {:?}",
                        node_path(g, node)
                    ));
                }
            }
        }
        Ok(())
    }

    fn layout_recursively(&self, g: &mut ElkGraph, layout_node: NodeId) -> Result<Vec<EdgeId>, String> {
        if g.node(layout_node).properties.get(&NO_LAYOUT) {
            return Ok(Vec::new());
        }

        let has_children = !g.node(layout_node).children.is_empty();
        let inside_self_loops = self.gather_inside_self_loops(g, layout_node);
        let has_inside_self_loops = !inside_self_loops.is_empty();

        if !has_children && !has_inside_self_loops {
            return Ok(Vec::new());
        }

        let algorithm = g
            .node(layout_node)
            .properties
            .try_get(&RESOLVED_ALGORITHM_TYPED)
            .ok_or_else(|| {
                "Resolved algorithm is not set; apply a LayoutAlgorithmResolver before computing layout."
                    .to_string()
            })?;
        let algorithm_data = self
            .registry
            .by_id(&algorithm.0)
            .ok_or_else(|| format!("Unknown algorithm {}", algorithm.0))?;
        let supports_inside_self_loops =
            algorithm_data.features.contains(GraphFeature::INSIDE_SELF_LOOPS);

        self.evaluate_hierarchy_handling_inheritance(g, layout_node);

        if !has_children && has_inside_self_loops && !supports_inside_self_loops {
            return Ok(Vec::new());
        }

        let mut children_inside_self_loops: Vec<EdgeId> = Vec::new();

        let include_children = g.node(layout_node).properties.get(&HIERARCHY_HANDLING)
            == HierarchyHandling::INCLUDE_CHILDREN
            && (algorithm_data.features.contains(GraphFeature::COMPOUND)
                || algorithm_data.features.contains(GraphFeature::CLUSTERS));

        if include_children {
            if g.node(layout_node).properties.get(&TOPDOWN_LAYOUT) {
                return Err(
                    "Topdown layout cannot be used together with hierarchy handling.".to_string()
                );
            }
            // Look for nodes that stop the hierarchy handling
            let mut node_queue: std::collections::VecDeque<NodeId> =
                g.node(layout_node).children.iter().copied().collect();
            while let Some(node) = node_queue.pop_front() {
                self.evaluate_hierarchy_handling_inheritance(g, node);
                let stop_hierarchy = g.node(node).properties.get(&HIERARCHY_HANDLING)
                    == HierarchyHandling::SEPARATE_CHILDREN;
                let switches_algorithm = g.node(node).properties.has(&ALGORITHM)
                    && g.node(node)
                        .properties
                        .try_get(&RESOLVED_ALGORITHM_TYPED)
                        .map(|r| r.0 != algorithm.0)
                        .unwrap_or(true);
                if stop_hierarchy || switches_algorithm {
                    let child_self_loops = self.layout_recursively(g, node)?;
                    children_inside_self_loops.extend(child_self_loops);
                    g.node_mut(node)
                        .properties
                        .set(&HIERARCHY_HANDLING, HierarchyHandling::SEPARATE_CHILDREN);
                    elkutil::apply_configured_node_scaling(g, node);
                } else {
                    node_queue.extend(g.node(node).children.iter().copied());
                }
            }
        } else {
            if g.node(layout_node).properties.get(&TOPDOWN_LAYOUT) {
                return Err("Topdown layout is not implemented yet in elkrs.".to_string());
            }
            let children = g.node(layout_node).children.clone();
            for child in children {
                let child_self_loops = self.layout_recursively(g, child)?;
                children_inside_self_loops.extend(child_self_loops);
                elkutil::apply_configured_node_scaling(g, child);
            }
        }

        // Exclude inside self loops of children from being laid out again
        for &self_loop in &children_inside_self_loops {
            g.edge_mut(self_loop).properties.set(&NO_LAYOUT, true);
        }

        // Run the algorithm on this node
        let mut provider = (algorithm_data.create)();
        provider.layout(g, layout_node)?;

        self.post_process_inside_self_loops(g, &children_inside_self_loops);

        if has_inside_self_loops && supports_inside_self_loops {
            Ok(inside_self_loops)
        } else {
            Ok(Vec::new())
        }
    }

    fn evaluate_hierarchy_handling_inheritance(&self, g: &mut ElkGraph, layout_node: NodeId) {
        if g.node(layout_node).properties.get(&HIERARCHY_HANDLING) == HierarchyHandling::INHERIT {
            match g.node(layout_node).parent {
                None => {
                    g.node_mut(layout_node)
                        .properties
                        .set(&HIERARCHY_HANDLING, HierarchyHandling::SEPARATE_CHILDREN);
                }
                Some(parent) => {
                    let parent_handling = g.node(parent).properties.get(&HIERARCHY_HANDLING);
                    g.node_mut(layout_node)
                        .properties
                        .set(&HIERARCHY_HANDLING, parent_handling);
                }
            }
        }
    }

    fn gather_inside_self_loops(&self, g: &ElkGraph, node: NodeId) -> Vec<EdgeId> {
        if g.node(node).properties.get(&INSIDE_SELF_LOOPS_ACTIVATE) {
            let mut result = Vec::new();
            // all outgoing edges of the node and its ports
            let mut edges: Vec<EdgeId> = g.node(node).outgoing_edges.clone();
            for &port in &g.node(node).ports {
                edges.extend(g.port(port).outgoing_edges.iter().copied());
            }
            for edge in edges {
                if is_self_loop(g, edge) && g.edge(edge).properties.get(&INSIDE_SELF_LOOPS_YO) {
                    result.push(edge);
                }
            }
            result
        } else {
            Vec::new()
        }
    }

    fn post_process_inside_self_loops(&self, g: &mut ElkGraph, self_loops: &[EdgeId]) {
        for &self_loop in self_loops {
            let source = g.edge(self_loop).sources[0];
            let node = g.shape_node(source);
            let x_offset = g.node(node).shape.x;
            let y_offset = g.node(node).shape.y;
            let section = g.edge(self_loop).sections[0];
            elkutil::translate_section(g, section, x_offset, y_offset);
            if let Some(mut jps) = g.edge(self_loop).properties.try_get(&JUNCTION_POINTS) {
                jps.offset_xy(x_offset, y_offset);
                g.edge_mut(self_loop).properties.set(&JUNCTION_POINTS, jps);
            }
        }
    }
}

/// True when all sources and targets are the same node or ports of the same
/// node.
pub fn is_self_loop(g: &ElkGraph, edge: EdgeId) -> bool {
    let e = g.edge(edge);
    let mut nodes = e
        .sources
        .iter()
        .chain(e.targets.iter())
        .map(|&s| g.shape_node(s));
    match nodes.next() {
        None => false,
        Some(first) => nodes.all(|n| n == first),
    }
}

fn node_path(g: &ElkGraph, node: NodeId) -> String {
    let mut parts = Vec::new();
    let mut current = Some(node);
    while let Some(n) = current {
        parts.push(
            g.node(n)
                .identifier
                .clone()
                .unwrap_or_else(|| format!("#{}", n.0)),
        );
        current = g.node(n).parent;
    }
    parts.reverse();
    parts.join(" > ")
}
