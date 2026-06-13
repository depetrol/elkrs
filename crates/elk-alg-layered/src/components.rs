//! Port of `org.eclipse.elk.alg.layered.components`: splitting a graph into
//! connected components and recombining them after layout.
//!
//! Currently includes the `SimpleRowGraphPlacer`; the component-group placers
//! (needed for external ports) are ported on demand.

use elk_core::options::{PortConstraints, PortSide};
use elk_graph::properties::EnumSet;

use crate::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::internal_properties as iprops;
use crate::options_gen as lopts;
use crate::options_gen::{ComponentOrderingStrategy, GraphProperties};

/// Port of `ComponentsProcessor` (split + combine via SimpleRowGraphPlacer).
pub struct ComponentsProcessor;

impl ComponentsProcessor {
    /// Port of `split`.
    pub fn split(a: &mut LGraphArena, graph: LGraphId) -> Result<Vec<LGraphId>, String> {
        let separate: bool = a
            .graph(graph)
            .properties
            .get_opt(&lopts::SEPARATE_CONNECTED_COMPONENTS)
            .unwrap_or(true);

        let ext_ports = a
            .graph(graph)
            .properties
            .get::<EnumSet<GraphProperties>>(&iprops::GRAPH_PROPERTIES)
            .contains(GraphProperties::EXTERNAL_PORTS);

        let ext_port_constraints: PortConstraints =
            a.graph(graph).properties.get(&lopts::PORT_CONSTRAINTS);
        let compatible_port_constraints = !ext_port_constraints.is_order_fixed();

        let mut result: Vec<LGraphId>;
        if separate && (compatible_port_constraints || !ext_ports) {
            let layerless = a.graph(graph).layerless_nodes.clone();
            for &node in &layerless {
                a.node_mut(node).id = 0;
            }
            result = Vec::new();
            for &node in &layerless {
                let mut component: Vec<LNodeId> = Vec::new();
                let mut ext_port_sides = EnumSet::<PortSide>::none();
                Self::dfs(a, node, &mut component, &mut ext_port_sides);

                if !component.is_empty() {
                    let new_graph = a.create_graph();
                    let src_props = a.graph(graph).properties.clone();
                    a.graph(new_graph).properties.copy_from(&src_props);
                    a.graph(new_graph)
                        .properties
                        .set(&iprops::EXT_PORT_CONNECTIONS, ext_port_sides);
                    a.graph_mut(new_graph).padding = a.graph(graph).padding;
                    a.graph(new_graph).properties.unset(&lopts::NODE_SIZE_MINIMUM);

                    for &n in &component {
                        a.graph_mut(new_graph).layerless_nodes.push(n);
                        a.node_mut(n).graph = Some(new_graph);
                    }
                    result.push(new_graph);
                }
            }

            if ext_ports {
                // The ComponentGroupGraphPlacer path. Unreachable through the
                // public layout API: external ports only ever exist on nested
                // graphs (top-level external ports throw an NPE during import in
                // `transformExternalPort`), and nested graphs are laid out by
                // `hierarchicalLayout`, which never invokes the components
                // processor. We therefore keep this as a guard (crash-for-crash
                // with Java's behavior) instead of porting the dead placer.
                return Err(
                    "component placers for graphs with external ports are unreachable via the \
                     public layout API (top-level external ports are unsupported)"
                        .to_string(),
                );
            }
        } else {
            result = vec![graph];
        }

        if a.graph(graph)
            .properties
            .get::<ComponentOrderingStrategy>(&lopts::CONSIDER_MODEL_ORDER_COMPONENTS)
            != ComponentOrderingStrategy::NONE
        {
            return Err("TODO: model-order component sorting is not ported yet".to_string());
        }

        Ok(result)
    }

    /// Port of the recursive `dfs`; same traversal order (per node: ports in
    /// order, per port predecessors then successors).
    fn dfs(
        a: &mut LGraphArena,
        node: LNodeId,
        component: &mut Vec<LNodeId>,
        ext_port_sides: &mut EnumSet<PortSide>,
    ) {
        if a.node(node).id != 0 {
            return;
        }
        a.node_mut(node).id = 1;
        component.push(node);
        if a.node(node).node_type == NodeType::EXTERNAL_PORT {
            ext_port_sides.add(a.node(node).properties.get(&iprops::EXT_PORT_SIDE));
        }
        let ports = a.node(node).ports.clone();
        for port in ports {
            // predecessor ports (sources of incoming edges)...
            let incoming = a.port(port).incoming_edges.clone();
            for edge in incoming {
                if let Some(src) = a.edge(edge).source {
                    if let Some(n) = a.port(src).node {
                        Self::dfs(a, n, component, ext_port_sides);
                    }
                }
            }
            // ...then successor ports (targets of outgoing edges)
            let outgoing = a.port(port).outgoing_edges.clone();
            for edge in outgoing {
                if let Some(tgt) = a.edge(edge).target {
                    if let Some(n) = a.port(tgt).node {
                        Self::dfs(a, n, component, ext_port_sides);
                    }
                }
            }
        }
    }

    /// Port of `combine` (SimpleRowGraphPlacer only).
    pub fn combine(
        a: &mut LGraphArena,
        components: &mut Vec<LGraphId>,
        target: LGraphId,
    ) -> Result<(), String> {
        if components.len() == 1 {
            let source = components[0];
            if source != target {
                a.graph_mut(target).layerless_nodes.clear();
                Self::move_graph(a, target, source, 0.0, 0.0);
                let src_props = a.graph(source).properties.clone();
                a.graph(target).properties.copy_from(&src_props);
                a.graph_mut(target).padding = a.graph(source).padding;
                let size = a.graph(source).size;
                a.graph_mut(target).size = size;
            }
            return Ok(());
        } else if components.is_empty() {
            a.graph_mut(target).layerless_nodes.clear();
            a.graph_mut(target).size = elk_graph::math::KVector::default();
            return Ok(());
        }

        Self::sort_components(a, components, target);

        let first_component = components[0];
        a.graph_mut(target).layerless_nodes.clear();
        let first_props = a.graph(first_component).properties.clone();
        a.graph(target).properties.copy_from(&first_props);

        let mut max_row_width = 0.0f64;
        let mut total_area = 0.0f64;
        for &graph in components.iter() {
            let size = a.graph(graph).size;
            max_row_width = f64::max(max_row_width, size.x);
            total_area += size.x * size.y;
        }
        // Java: (float) Math.sqrt(totalArea) * aspectRatio
        let aspect_ratio: f64 = a.graph(target).properties.get(&lopts::ASPECT_RATIO);
        max_row_width = f64::max(max_row_width, (total_area.sqrt() as f32) as f64 * aspect_ratio);
        let component_spacing: f64 = a
            .graph(target)
            .properties
            .get(&lopts::SPACING_COMPONENT_COMPONENT);

        Self::place_components(a, components, target, max_row_width, component_spacing);

        if a.graph(first_component)
            .properties
            .get(&lopts::COMPACTION_CONNECTED_COMPONENTS)
        {
            return Err("TODO: ComponentsCompactor is not ported yet".to_string());
        }

        let comps = components.clone();
        for source in comps {
            Self::move_graph(a, target, source, 0.0, 0.0);
        }
        Ok(())
    }

    /// Port of `SimpleRowGraphPlacer.sortComponents`.
    fn sort_components(a: &mut LGraphArena, components: &mut [LGraphId], target: LGraphId) {
        if a.graph(target)
            .properties
            .get::<ComponentOrderingStrategy>(&lopts::CONSIDER_MODEL_ORDER_COMPONENTS)
            == ComponentOrderingStrategy::NONE
        {
            for &graph in components.iter() {
                let mut priority = 0i32;
                for &node in &a.graph(graph).layerless_nodes {
                    priority += a
                        .node(node)
                        .properties
                        .get_opt(&lopts::PRIORITY)
                        .unwrap_or(0);
                }
                a.graph_mut(graph).id = priority;
            }
            components.sort_by(|&g1, &g2| {
                let prio = a.graph(g2).id - a.graph(g1).id;
                if prio == 0 {
                    let size1 = a.graph(g1).size.x * a.graph(g1).size.y;
                    let size2 = a.graph(g2).size.x * a.graph(g2).size.y;
                    size1.total_cmp(&size2)
                } else {
                    prio.cmp(&0)
                }
            });
        }
    }

    /// Port of `SimpleRowGraphPlacer.placeComponents`.
    fn place_components(
        a: &mut LGraphArena,
        components: &[LGraphId],
        target: LGraphId,
        max_row_width: f64,
        component_spacing: f64,
    ) {
        let mut xpos = 0.0f64;
        let mut ypos = 0.0f64;
        let mut highest_box = 0.0f64;
        let mut broadest_row = component_spacing;
        for &graph in components {
            let size = a.graph(graph).size;
            if xpos + size.x > max_row_width {
                xpos = 0.0;
                ypos += highest_box + component_spacing;
                highest_box = 0.0;
            }
            let offset = a.graph(graph).offset;
            Self::offset_graph(a, graph, xpos + offset.x, ypos + offset.y);
            a.graph_mut(graph).offset = elk_graph::math::KVector::default();
            broadest_row = f64::max(broadest_row, xpos + size.x);
            highest_box = f64::max(highest_box, size.y);
            xpos += size.x + component_spacing;
        }
        a.graph_mut(target).size.x = broadest_row;
        a.graph_mut(target).size.y = ypos + highest_box;
    }

    /// Port of `AbstractGraphPlacer.moveGraph`.
    fn move_graph(
        a: &mut LGraphArena,
        dest_graph: LGraphId,
        source_graph: LGraphId,
        offsetx: f64,
        offsety: f64,
    ) {
        let graph_offset = {
            let off = &mut a.graph_mut(source_graph).offset;
            off.add_xy(offsetx, offsety);
            *off
        };

        let nodes = a.graph(source_graph).layerless_nodes.clone();
        for node in nodes {
            a.node_mut(node).pos.add(graph_offset);
            let ports = a.node(node).ports.clone();
            for port in ports {
                let outgoing = a.port(port).outgoing_edges.clone();
                for edge in outgoing {
                    a.edge_mut(edge).bend_points.offset(graph_offset);
                    if let Some(mut jps) = a.edge(edge).properties.try_get(&lopts::JUNCTION_POINTS)
                    {
                        jps.offset(graph_offset);
                        a.edge(edge).properties.set(&lopts::JUNCTION_POINTS, jps);
                    }
                    let labels = a.edge(edge).labels.clone();
                    for label in labels {
                        a.label_mut(label).pos.add(graph_offset);
                    }
                }
            }
            a.graph_mut(dest_graph).layerless_nodes.push(node);
            a.node_mut(node).graph = Some(dest_graph);
        }
    }

    /// Port of `AbstractGraphPlacer.offsetGraph`.
    fn offset_graph(a: &mut LGraphArena, graph: LGraphId, offsetx: f64, offsety: f64) {
        let graph_offset = elk_graph::math::KVector::new(offsetx, offsety);
        let nodes = a.graph(graph).layerless_nodes.clone();
        for node in nodes {
            a.node_mut(node).pos.add(graph_offset);
            let ports = a.node(node).ports.clone();
            for port in ports {
                let outgoing = a.port(port).outgoing_edges.clone();
                for edge in outgoing {
                    a.edge_mut(edge).bend_points.offset(graph_offset);
                    if let Some(mut jps) = a.edge(edge).properties.try_get(&lopts::JUNCTION_POINTS)
                    {
                        jps.offset(graph_offset);
                        a.edge(edge).properties.set(&lopts::JUNCTION_POINTS, jps);
                    }
                    let labels = a.edge(edge).labels.clone();
                    for label in labels {
                        a.label_mut(label).pos.add(graph_offset);
                    }
                }
            }
        }
    }
}
