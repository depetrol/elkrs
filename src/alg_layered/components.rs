//! Splitting a graph into
//! connected components and recombining them after layout.
//!
//! Currently includes the `SimpleRowGraphPlacer`; the component-group placers
//! (needed for external ports) are ported on demand.

use crate::core::options::{EdgeRouting, PortConstraints, PortSide};
use crate::graph::math::KVector;
use crate::graph::properties::{ElkEnum, EnumSet};
use indexmap::IndexMap;

use crate::alg_layered::graph::{LGraphArena, LGraphId, LNodeId, NodeType};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::{ComponentOrderingStrategy, GraphProperties};

fn sides(parts: &[PortSide]) -> EnumSet<PortSide> {
    EnumSet::of(parts)
}

pub struct ComponentsProcessor;

impl ComponentsProcessor {
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

            // When the graph has external ports the more complex
            // `ComponentGroupGraphPlacer` is used in `combine`; the placer
            // dispatch there re-derives this from the graph properties.
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

    /// The recursive `dfs`; traversal order is per node: ports in
    /// order, per port predecessors then successors.
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

    /// Dispatch on whether the graph has
    /// external ports. The placer decision is derived from the target graph's
    /// properties (when there are external ports and the model-order strategy
    /// is `NONE`, the `ComponentGroupGraphPlacer` is used; the model-order
    /// strategies are rejected earlier in `split`).
    pub fn combine(
        a: &mut LGraphArena,
        components: &mut Vec<LGraphId>,
        target: LGraphId,
    ) -> Result<(), String> {
        let ext_ports = a
            .graph(target)
            .properties
            .get::<EnumSet<GraphProperties>>(&iprops::GRAPH_PROPERTIES)
            .contains(GraphProperties::EXTERNAL_PORTS);
        if ext_ports {
            return Self::combine_component_groups(a, components, target);
        }
        Self::combine_simple_row(a, components, target)
    }

    fn combine_simple_row(
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
            a.graph_mut(target).size = crate::graph::math::KVector::default();
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
            a.graph_mut(graph).offset = crate::graph::math::KVector::default();
            broadest_row = f64::max(broadest_row, xpos + size.x);
            highest_box = f64::max(highest_box, size.y);
            xpos += size.x + component_spacing;
        }
        a.graph_mut(target).size.x = broadest_row;
        a.graph_mut(target).size.y = ypos + highest_box;
    }

    fn combine_component_groups(
        a: &mut LGraphArena,
        components: &[LGraphId],
        target: LGraphId,
    ) -> Result<(), String> {
        a.graph_mut(target).layerless_nodes.clear();

        if components.is_empty() {
            a.graph_mut(target).size.x = 0.0;
            a.graph_mut(target).size.y = 0.0;
            return Ok(());
        }

        // Set the graph properties
        let first_component = components[0];
        let first_props = a.graph(first_component).properties.clone();
        a.graph(target).properties.copy_from(&first_props);

        // Construct component groups
        let mut groups: Vec<ComponentGroup> = Vec::new();
        for &component in components {
            let conn = a
                .graph(component)
                .properties
                .get(&iprops::EXT_PORT_CONNECTIONS);
            Self::add_component(a, &mut groups, component, conn);
        }

        // Place components in each group
        let mut offset = KVector::default();
        let component_spacing: f64 = a
            .graph(first_component)
            .properties
            .get(&lopts::SPACING_COMPONENT_COMPONENT);

        for group in &groups {
            let group_size = Self::place_group(a, group, component_spacing);
            for &component in group.all_components() {
                Self::offset_graph(a, component, offset.x, offset.y);
            }
            offset.x += group_size.x;
            offset.y += group_size.y;
        }

        // Set the graph's new size (the component group sizes include additional
        // spacing on the right and bottom sides which we need to subtract).
        a.graph_mut(target).size.x = offset.x - component_spacing;
        a.graph_mut(target).size.y = offset.y - component_spacing;

        if a.graph(first_component)
            .properties
            .get(&lopts::COMPACTION_CONNECTED_COMPONENTS)
            && a.graph(first_component)
                .properties
                .get::<EdgeRouting>(&lopts::EDGE_ROUTING)
                == EdgeRouting::ORTHOGONAL
        {
            return Err("TODO: ComponentsCompactor is not ported yet".to_string());
        }

        // Finally move the components to the combined graph
        for group in &groups {
            for &component in group.all_components() {
                Self::move_graph(a, target, component, 0.0, 0.0);
            }
        }
        Ok(())
    }

    fn add_component(
        a: &LGraphArena,
        groups: &mut Vec<ComponentGroup>,
        component: LGraphId,
        conn: EnumSet<PortSide>,
    ) {
        for group in groups.iter_mut() {
            if group.add(a, component, conn) {
                return;
            }
        }
        let mut group = ComponentGroup::new();
        group.add(a, component, conn);
        groups.push(group);
    }

    fn place_group(a: &mut LGraphArena, group: &ComponentGroup, spacing: f64) -> KVector {
        let size_c = Self::place_in_rows(a, group.get(sides(&[])), spacing);
        let size_n = Self::place_horizontally(a, group.get(sides(&[PortSide::NORTH])), spacing);
        let size_s = Self::place_horizontally(a, group.get(sides(&[PortSide::SOUTH])), spacing);
        let size_w = Self::place_vertically(a, group.get(sides(&[PortSide::WEST])), spacing);
        let size_e = Self::place_vertically(a, group.get(sides(&[PortSide::EAST])), spacing);
        let size_nw = Self::place_horizontally(
            a,
            group.get(sides(&[PortSide::NORTH, PortSide::WEST])),
            spacing,
        );
        let size_ne = Self::place_horizontally(
            a,
            group.get(sides(&[PortSide::NORTH, PortSide::EAST])),
            spacing,
        );
        let size_sw = Self::place_horizontally(
            a,
            group.get(sides(&[PortSide::SOUTH, PortSide::WEST])),
            spacing,
        );
        let size_se = Self::place_horizontally(
            a,
            group.get(sides(&[PortSide::EAST, PortSide::SOUTH])),
            spacing,
        );
        let size_we = Self::place_vertically(
            a,
            group.get(sides(&[PortSide::EAST, PortSide::WEST])),
            spacing,
        );
        let size_ns = Self::place_horizontally(
            a,
            group.get(sides(&[PortSide::NORTH, PortSide::SOUTH])),
            spacing,
        );
        let size_nwe = Self::place_horizontally(
            a,
            group.get(sides(&[PortSide::NORTH, PortSide::EAST, PortSide::WEST])),
            spacing,
        );
        let size_swe = Self::place_horizontally(
            a,
            group.get(sides(&[PortSide::EAST, PortSide::SOUTH, PortSide::WEST])),
            spacing,
        );
        let size_wns = Self::place_vertically(
            a,
            group.get(sides(&[PortSide::NORTH, PortSide::SOUTH, PortSide::WEST])),
            spacing,
        );
        let size_ens = Self::place_vertically(
            a,
            group.get(sides(&[PortSide::NORTH, PortSide::EAST, PortSide::SOUTH])),
            spacing,
        );
        let size_nesw = Self::place_horizontally(
            a,
            group.get(sides(&[
                PortSide::NORTH,
                PortSide::EAST,
                PortSide::SOUTH,
                PortSide::WEST,
            ])),
            spacing,
        );

        let col_left_width = maxd(&[size_nw.x, size_w.x, size_sw.x, size_wns.x]);
        let col_mid_width = maxd(&[size_n.x, size_c.x, size_s.x, size_nesw.x]);
        let col_ns_width = size_ns.x;
        let col_right_width = maxd(&[size_ne.x, size_e.x, size_se.x, size_ens.x]);
        let row_top_height = maxd(&[size_nw.y, size_n.y, size_ne.y, size_nwe.y]);
        let row_mid_height = maxd(&[size_w.y, size_c.y, size_e.y, size_nesw.y]);
        let row_we_height = size_we.y;
        let row_bottom_height = maxd(&[size_sw.y, size_s.y, size_se.y, size_swe.y]);

        Self::offset_graphs(
            a,
            group.get(sides(&[])),
            col_left_width + col_ns_width,
            row_top_height + row_we_height,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[
                PortSide::NORTH,
                PortSide::EAST,
                PortSide::SOUTH,
                PortSide::WEST,
            ])),
            col_left_width + col_ns_width,
            row_top_height + row_we_height,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[PortSide::NORTH])),
            col_left_width + col_ns_width,
            0.0,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[PortSide::SOUTH])),
            col_left_width + col_ns_width,
            row_top_height + row_we_height + row_mid_height,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[PortSide::WEST])),
            0.0,
            row_top_height + row_we_height,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[PortSide::EAST])),
            col_left_width + col_ns_width + col_mid_width,
            row_top_height + row_we_height,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[PortSide::NORTH, PortSide::EAST])),
            col_left_width + col_ns_width + col_mid_width,
            0.0,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[PortSide::SOUTH, PortSide::WEST])),
            0.0,
            row_top_height + row_we_height + row_mid_height,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[PortSide::EAST, PortSide::SOUTH])),
            col_left_width + col_ns_width + col_mid_width,
            row_top_height + row_we_height + row_mid_height,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[PortSide::EAST, PortSide::WEST])),
            0.0,
            row_top_height,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[PortSide::NORTH, PortSide::SOUTH])),
            col_left_width,
            0.0,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[PortSide::EAST, PortSide::SOUTH, PortSide::WEST])),
            0.0,
            row_top_height + row_we_height + row_mid_height,
        );
        Self::offset_graphs(
            a,
            group.get(sides(&[PortSide::NORTH, PortSide::EAST, PortSide::SOUTH])),
            col_left_width + col_ns_width + col_mid_width,
            0.0,
        );

        let mut component_size = KVector::default();
        component_size.x = maxd(&[
            col_left_width + col_mid_width + col_ns_width + col_right_width,
            size_we.x,
            size_nwe.x,
            size_swe.x,
        ]);
        component_size.y = maxd(&[
            row_top_height + row_mid_height + row_we_height + row_bottom_height,
            size_ns.y,
            size_wns.y,
            size_ens.y,
        ]);
        component_size
    }

    fn place_horizontally(a: &mut LGraphArena, components: &[LGraphId], spacing: f64) -> KVector {
        let mut size = KVector::default();
        for &component in components {
            Self::offset_graph(a, component, size.x, 0.0);
            let csize = a.graph(component).size;
            size.x += csize.x + spacing;
            size.y = f64::max(size.y, csize.y);
        }
        if size.y > 0.0 {
            size.y += spacing;
        }
        size
    }

    fn place_vertically(a: &mut LGraphArena, components: &[LGraphId], spacing: f64) -> KVector {
        let mut size = KVector::default();
        for &component in components {
            Self::offset_graph(a, component, 0.0, size.y);
            let csize = a.graph(component).size;
            size.y += csize.y + spacing;
            size.x = f64::max(size.x, csize.x);
        }
        if size.x > 0.0 {
            size.x += spacing;
        }
        size
    }

    fn place_in_rows(a: &mut LGraphArena, components: &[LGraphId], spacing: f64) -> KVector {
        if components.is_empty() {
            return KVector::default();
        }

        let mut max_row_width = 0.0f64;
        let mut total_area = 0.0f64;
        for &component in components {
            let csize = a.graph(component).size;
            max_row_width = f64::max(max_row_width, csize.x);
            total_area += csize.x * csize.y;
        }
        let aspect_ratio: f64 = a.graph(components[0]).properties.get(&lopts::ASPECT_RATIO);
        max_row_width = f64::max(max_row_width, (total_area.sqrt() as f32) as f64 * aspect_ratio);

        let mut xpos = 0.0f64;
        let mut ypos = 0.0f64;
        let mut highest_box = 0.0f64;
        let mut broadest_row = spacing;
        for &graph in components {
            let size = a.graph(graph).size;
            if xpos + size.x > max_row_width {
                xpos = 0.0;
                ypos += highest_box + spacing;
                highest_box = 0.0;
            }
            Self::offset_graph(a, graph, xpos, ypos);
            broadest_row = f64::max(broadest_row, xpos + size.x);
            highest_box = f64::max(highest_box, size.y);
            xpos += size.x + spacing;
        }
        KVector::new(broadest_row + spacing, ypos + highest_box + spacing)
    }

    fn offset_graphs(a: &mut LGraphArena, graphs: &[LGraphId], offsetx: f64, offsety: f64) {
        for &graph in graphs {
            Self::offset_graph(a, graph, offsetx, offsety);
        }
    }

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

    fn offset_graph(a: &mut LGraphArena, graph: LGraphId, offsetx: f64, offsety: f64) {
        let graph_offset = crate::graph::math::KVector::new(offsetx, offsety);
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

/// The maximum of the given values.
fn maxd(values: &[f64]) -> f64 {
    let mut max = values[0];
    for &v in &values[1..] {
        if v > max {
            max = v;
        }
    }
    max
}

/// A group of connected components, keyed by the set
/// of external-port sides each connects to. Keyed here by the `EnumSet` bit
/// pattern via an `IndexMap` to keep insertion order.
struct ComponentGroup {
    /// Insertion-ordered map: ext-port-side set -> components with that set.
    components: IndexMap<u64, Vec<LGraphId>>,
}

impl ComponentGroup {
    fn new() -> Self {
        ComponentGroup {
            components: IndexMap::new(),
        }
    }

    fn add(&mut self, _a: &LGraphArena, component: LGraphId, conn: EnumSet<PortSide>) -> bool {
        if self.can_add(conn) {
            self.components.entry(key(conn)).or_default().push(component);
            true
        } else {
            false
        }
    }

    fn can_add(&self, candidate_sides: EnumSet<PortSide>) -> bool {
        for &constraint in constraints_for(candidate_sides) {
            if let Some(list) = self.components.get(&constraint) {
                if !list.is_empty() {
                    return false;
                }
            }
        }
        true
    }

    /// Returns an empty
    /// slice when no components are registered for the given side set.
    fn get(&self, connections: EnumSet<PortSide>) -> &[LGraphId] {
        match self.components.get(&key(connections)) {
            Some(list) => list,
            None => &[],
        }
    }

    fn all_components(&self) -> impl Iterator<Item = &LGraphId> {
        self.components.values().flat_map(|v| v.iter())
    }
}

/// Map an `EnumSet<PortSide>` to a stable key (its bit pattern).
fn key(set: EnumSet<PortSide>) -> u64 {
    let mut bits = 0u64;
    for side in set.iter() {
        bits |= 1 << side.ordinal();
    }
    bits
}

/// For a candidate set of external-port
/// sides, the side sets that must not already exist in the group. Returns the
/// list of constraint keys to check.
fn constraints_for(candidate: EnumSet<PortSide>) -> &'static [u64] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<IndexMap<u64, Vec<u64>>> = OnceLock::new();
    let table = TABLE.get_or_init(build_constraints);
    match table.get(&key(candidate)) {
        Some(v) => v.as_slice(),
        None => &[],
    }
}

fn build_constraints() -> IndexMap<u64, Vec<u64>> {
    use PortSide::{EAST, NORTH, SOUTH, WEST};
    let none = key(sides(&[]));
    let n = key(sides(&[NORTH]));
    let e = key(sides(&[EAST]));
    let s = key(sides(&[SOUTH]));
    let w = key(sides(&[WEST]));
    let ns = key(sides(&[NORTH, SOUTH]));
    let ew = key(sides(&[EAST, WEST]));
    let nw = key(sides(&[NORTH, WEST]));
    let ne = key(sides(&[NORTH, EAST]));
    let sw = key(sides(&[SOUTH, WEST]));
    let es = key(sides(&[EAST, SOUTH]));
    let new_ = key(sides(&[NORTH, EAST, WEST]));
    let esw = key(sides(&[EAST, SOUTH, WEST]));
    let nsw = key(sides(&[NORTH, SOUTH, WEST]));
    let nes = key(sides(&[NORTH, EAST, SOUTH]));
    let nesw = key(sides(&[NORTH, EAST, SOUTH, WEST]));

    let mut m: IndexMap<u64, Vec<u64>> = IndexMap::new();
    let mut put = |k: u64, v: u64| m.entry(k).or_default().push(v);

    put(none, nesw);
    put(w, nesw);
    put(w, nsw);
    put(e, nes);
    put(e, nesw);
    put(n, nesw);
    put(n, new_);
    put(s, esw);
    put(s, nesw);
    put(ns, ew);
    put(ns, nesw);
    put(ns, new_);
    put(ns, esw);
    put(ew, ns);
    put(ew, nsw);
    put(ew, nes);
    put(ew, nesw);
    put(nw, nw);
    put(nw, new_);
    put(nw, nsw);
    put(ne, ne);
    put(ne, new_);
    put(ne, nes);
    put(sw, sw);
    put(sw, esw);
    put(sw, nsw);
    put(es, es);
    put(es, esw);
    put(es, nes);
    put(new_, n);
    put(new_, ns);
    put(new_, nw);
    put(new_, ne);
    put(new_, nesw);
    put(new_, new_);
    put(new_, nsw);
    put(new_, nes);
    put(esw, s);
    put(esw, ns);
    put(esw, sw);
    put(esw, es);
    put(esw, esw);
    put(esw, nsw);
    put(esw, nes);
    put(esw, nesw);
    put(nsw, w);
    put(nsw, ew);
    put(nsw, nw);
    put(nsw, sw);
    put(nsw, new_);
    put(nsw, esw);
    put(nsw, nsw);
    put(nsw, nesw);
    put(nes, e);
    put(nes, ew);
    put(nes, ne);
    put(nes, es);
    put(nes, new_);
    put(nes, esw);
    put(nes, nes);
    put(nes, nesw);
    put(nesw, none);
    put(nesw, w);
    put(nesw, e);
    put(nesw, n);
    put(nesw, s);
    put(nesw, ns);
    put(nesw, ew);
    put(nesw, new_);
    put(nesw, esw);
    put(nesw, nsw);
    put(nesw, nes);
    put(nesw, nesw);
    m
}
