//! Ports of `org.eclipse.elk.alg.disco.transform`
//! (`ElkGraphComponentsProcessor`, `ElkGraphTransformer`).

use crate::graph::graph::{EdgeId, ElkGraph, LabelId, NodeId, PortId};
use crate::graph::math::{KVector, KVectorChain};

use crate::alg_disco::graph::{DCDirection, DCElement, DCExtension, DCGraph};
use crate::alg_disco::options;

const HALF_PI: f64 = std::f64::consts::PI / 2.0;

// ----------------------------------------- ElkGraphComponentsProcessor

/// All edges incoming at the node, directly or through one of its ports
/// (`ElkGraphUtil.allIncomingEdges`).
fn all_incoming_edges(g: &ElkGraph, node: NodeId) -> Vec<EdgeId> {
    let mut edges = g.node(node).incoming_edges.clone();
    for &port in &g.node(node).ports {
        edges.extend(g.port(port).incoming_edges.iter().copied());
    }
    edges
}

/// `ElkGraphUtil.allOutgoingEdges`.
fn all_outgoing_edges(g: &ElkGraph, node: NodeId) -> Vec<EdgeId> {
    let mut edges = g.node(node).outgoing_edges.clone();
    for &port in &g.node(node).ports {
        edges.extend(g.port(port).outgoing_edges.iter().copied());
    }
    edges
}

fn source_node(g: &ElkGraph, edge: EdgeId) -> NodeId {
    g.shape_node(g.edge(edge).sources[0])
}

fn target_node(g: &ElkGraph, edge: EdgeId) -> NodeId {
    g.shape_node(g.edge(edge).targets[0])
}

fn source_port(g: &ElkGraph, edge: EdgeId) -> Option<PortId> {
    g.shape_port(g.edge(edge).sources[0])
}

fn target_port(g: &ElkGraph, edge: EdgeId) -> Option<PortId> {
    g.shape_port(g.edge(edge).targets[0])
}

/// Connected components via
/// depth-first search. (Only component membership matters for the result.)
pub fn split(g: &ElkGraph, graph: NodeId) -> Vec<Vec<NodeId>> {
    let children = g.node(graph).children.clone();

    // computeIncidences
    let mut incidence: std::collections::HashMap<NodeId, Vec<NodeId>> =
        std::collections::HashMap::new();
    // Cache of nodes adjacent through a parent port.
    let mut adjacent_and_inside_parent: std::collections::HashMap<PortId, Vec<NodeId>> =
        std::collections::HashMap::new();

    let same_hierarchy_level = |g: &ElkGraph, edge: EdgeId| {
        g.node(source_node(g, edge)).parent == g.node(target_node(g, edge)).parent
    };

    for &node in &children {
        let mut adjacent_nodes: Vec<NodeId> = Vec::new();
        let add = |n: NodeId, adj: &mut Vec<NodeId>| {
            if !adj.contains(&n) {
                adj.push(n);
            }
        };

        let incoming = all_incoming_edges(g, node);
        for &edge in incoming.iter().filter(|&&e| same_hierarchy_level(g, e)) {
            add(source_node(g, edge), &mut adjacent_nodes);
        }
        // edges coming in from a port of the parent node
        for &edge in incoming
            .iter()
            .filter(|&&e| !same_hierarchy_level(g, e))
            .filter(|&&e| Some(source_node(g, e)) == g.node(target_node(g, e)).parent)
        {
            if let Some(port) = source_port(g, edge) {
                let nodes_at_port = adjacent_and_inside_parent
                    .entry(port)
                    .or_insert_with(|| inner_neighbors_of_port(g, port));
                for &n in nodes_at_port.iter() {
                    if !adjacent_nodes.contains(&n) {
                        adjacent_nodes.push(n);
                    }
                }
            }
        }

        let outgoing = all_outgoing_edges(g, node);
        for &edge in outgoing.iter().filter(|&&e| same_hierarchy_level(g, e)) {
            add(target_node(g, edge), &mut adjacent_nodes);
        }
        for &edge in outgoing
            .iter()
            .filter(|&&e| !same_hierarchy_level(g, e))
            .filter(|&&e| Some(target_node(g, e)) == g.node(source_node(g, e)).parent)
        {
            if let Some(port) = target_port(g, edge) {
                let nodes_at_port = adjacent_and_inside_parent
                    .entry(port)
                    .or_insert_with(|| inner_neighbors_of_port(g, port));
                for &n in nodes_at_port.iter() {
                    if !adjacent_nodes.contains(&n) {
                        adjacent_nodes.push(n);
                    }
                }
            }
        }

        incidence.insert(node, adjacent_nodes);
    }

    // dfs
    let mut visited: Vec<NodeId> = Vec::new();
    let mut components: Vec<Vec<NodeId>> = Vec::new();
    for &node in &children {
        if !visited.contains(&node) {
            let mut component = Vec::new();
            dfs(node, &incidence, &mut visited, &mut component);
            components.push(component);
        }
    }
    components
}

fn inner_neighbors_of_port(g: &ElkGraph, port: PortId) -> Vec<NodeId> {
    let port_parent = g.port(port).parent.unwrap();
    let mut all_edges: Vec<EdgeId> = g.port(port).incoming_edges.clone();
    all_edges.extend(g.port(port).outgoing_edges.iter().copied());
    let mut result = Vec::new();
    for edge in all_edges {
        let src = source_node(g, edge);
        let tgt = target_node(g, edge);
        let inwards = Some(port_parent) == g.node(src).parent || Some(port_parent) == g.node(tgt).parent;
        if inwards {
            let n = if port_parent == src { tgt } else { src };
            if !result.contains(&n) {
                result.push(n);
            }
        }
    }
    result
}

fn dfs(
    start: NodeId,
    incidence: &std::collections::HashMap<NodeId, Vec<NodeId>>,
    visited: &mut Vec<NodeId>,
    component: &mut Vec<NodeId>,
) {
    visited.push(start);
    component.push(start);
    if let Some(adjacent) = incidence.get(&start) {
        for &node in adjacent {
            if !visited.contains(&node) {
                dfs(node, incidence, visited, component);
            }
        }
    }
}

// --------------------------------------------------- ElkGraphTransformer

/// Original graph element keys of the `elementMapping`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ElementKey {
    Node(NodeId),
    Label(LabelId),
    Edge(EdgeId),
}

pub struct ElkGraphTransformer {
    parent: NodeId,
    /// element -> DCElement index (insertion-ordered; order does not
    /// influence geometry).
    element_mapping: Vec<(ElementKey, usize)>,
    incoming_extensions_mapping: Vec<(EdgeId, DCDirection)>,
    outgoing_extensions_mapping: Vec<(EdgeId, DCDirection)>,
    component_spacing: f64,
}

impl ElkGraphTransformer {
    pub fn new(component_spacing: f64) -> Self {
        ElkGraphTransformer {
            parent: NodeId(u32::MAX),
            element_mapping: Vec::new(),
            incoming_extensions_mapping: Vec::new(),
            outgoing_extensions_mapping: Vec::new(),
            component_spacing,
        }
    }

    fn mapping_get(&self, key: ElementKey) -> Option<usize> {
        self.element_mapping
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    }

    pub fn import_graph(&mut self, g: &mut ElkGraph, graph: NodeId) -> DCGraph {
        self.parent = graph;
        let components = split(&*g, graph);

        let mut elements: Vec<DCElement> = Vec::new();
        let mut result: Vec<Vec<usize>> = Vec::new();

        for component in components {
            let mut sub_result: Vec<usize> = Vec::new();
            let mut edge_set: Vec<EdgeId> = Vec::new();
            for node in component {
                let shape = &g.node(node).shape;
                let component_node = self.import_shape(
                    &mut elements,
                    (shape.x, shape.y, shape.width, shape.height),
                    Some(ElementKey::Node(node)),
                    0.0,
                    0.0,
                );
                sub_result.push(component_node);

                let node_x = g.node(node).shape.x;
                let node_y = g.node(node).shape.y;

                for &label in &g.node(node).labels {
                    let s = &g.label(label).shape;
                    let component_label = self.import_shape(
                        &mut elements,
                        (s.x, s.y, s.width, s.height),
                        None,
                        node_x,
                        node_y,
                    );
                    sub_result.push(component_label);
                }

                for &port in &g.node(node).ports {
                    let s = &g.port(port).shape;
                    let component_port = self.import_shape(
                        &mut elements,
                        (s.x, s.y, s.width, s.height),
                        None,
                        node_x,
                        node_y,
                    );
                    sub_result.push(component_port);

                    let port_x = g.port(port).shape.x + node_x;
                    let port_y = g.port(port).shape.y + node_y;

                    for &label in &g.port(port).labels {
                        let s = &g.label(label).shape;
                        let component_label = self.import_shape(
                            &mut elements,
                            (s.x, s.y, s.width, s.height),
                            None,
                            port_x,
                            port_y,
                        );
                        sub_result.push(component_label);
                    }
                }

                // edgeSet.addAll(allIncidentEdges(node))
                for e in all_incoming_edges(g, node)
                    .into_iter()
                    .chain(all_outgoing_edges(g, node))
                {
                    if !edge_set.contains(&e) {
                        edge_set.push(e);
                    }
                }
            }
            self.import_elk_edges(g, &mut elements, &edge_set, &mut sub_result);
            result.push(sub_result);
        }

        let transformed_graph = DCGraph::new(elements, result);
        // copy properties of the parent ElkNode to the DCGraph
        transformed_graph
            .properties
            .copy_from(&g.node(graph).properties);
        transformed_graph
    }

    fn import_shape(
        &mut self,
        elements: &mut Vec<DCElement>,
        (x, y, width, height): (f64, f64, f64, f64),
        key: Option<ElementKey>,
        offset_x: f64,
        offset_y: f64,
    ) -> usize {
        let half_component_spacing = self.component_spacing / 2.0;
        let x0 = x + offset_x - half_component_spacing;
        let y0 = y + offset_y - half_component_spacing;
        let x1 = x0 + width + self.component_spacing;
        let y1 = y0 + height + self.component_spacing;

        let mut coords = KVectorChain::new();
        coords.add(x0, y0);
        coords.add(x0, y1);
        coords.add(x1, y1);
        coords.add(x1, y0);

        let idx = elements.len();
        elements.push(DCElement::new(coords));

        if let Some(key) = key {
            self.element_mapping.push((key, idx));
        }

        idx
    }

    fn import_elk_edges(
        &mut self,
        g: &mut ElkGraph,
        elements: &mut Vec<DCElement>,
        edges: &[EdgeId],
        new_component: &mut Vec<usize>,
    ) {
        for &edge in edges {
            if self.mapping_get(ElementKey::Edge(edge)).is_some() {
                continue;
            }
            let src = source_node(g, edge);
            let tgt = target_node(g, edge);
            if g.node(src).parent == g.node(tgt).parent {
                self.import_elk_edge(g, elements, edge, new_component);
            } else if Some(src) == g.node(tgt).parent {
                // incoming extension
                if !self.incoming_extensions_mapping.iter().any(|(e, _)| *e == edge)
                    && self.mapping_get(ElementKey::Node(tgt)).is_some()
                {
                    self.import_extension(g, elements, edge, new_component, false);
                }
            } else {
                // outgoing extension
                if !self.outgoing_extensions_mapping.iter().any(|(e, _)| *e == edge)
                    && self.mapping_get(ElementKey::Node(src)).is_some()
                {
                    self.import_extension(g, elements, edge, new_component, true);
                }
            }
        }
    }

    fn import_elk_edge(
        &mut self,
        g: &mut ElkGraph,
        elements: &mut Vec<DCElement>,
        edge: EdgeId,
        new_component: &mut Vec<usize>,
    ) {
        let section = g.first_edge_section(edge, false);
        let points = g.section_chain(section);

        let thickness: f64 = g.edge(edge).properties.get(&options::EDGE_THICKNESS);
        let contour = get_contour(&points.0, thickness + self.component_spacing);

        let idx = elements.len();
        elements.push(DCElement::new(contour));
        self.element_mapping.push((ElementKey::Edge(edge), idx));
        new_component.push(idx);

        // ElkEdges can have labels, too!
        for &label in &g.edge(edge).labels {
            let s = &g.label(label).shape;
            let component_label = self.import_shape(
                elements,
                (s.x, s.y, s.width, s.height),
                Some(ElementKey::Label(label)),
                0.0,
                0.0,
            );
            new_component.push(component_label);
        }
    }

    fn import_extension(
        &mut self,
        g: &mut ElkGraph,
        elements: &mut Vec<DCElement>,
        edge: EdgeId,
        new_component: &mut Vec<usize>,
        outgoing_extension: bool,
    ) {
        let section = g.first_edge_section(edge, false);
        let mut points = g.section_chain(section);
        if outgoing_extension {
            points = KVectorChain::reverse(&points);
        }

        let thickness: f64 = g.edge(edge).properties.get(&options::EDGE_THICKNESS);

        let outer_point = points.0[0];
        let inner_point = points.0[1];

        let shape_idx: usize;
        if points.len() > 2 {
            let fixed_edge_points: Vec<KVector> = points.0[1..].to_vec();
            let contour = get_contour(&fixed_edge_points, thickness + self.component_spacing);
            shape_idx = elements.len();
            elements.push(DCElement::new(contour));
            new_component.push(shape_idx);
        } else if outgoing_extension {
            shape_idx = self
                .mapping_get(ElementKey::Node(source_node(g, edge)))
                .expect("source node element");
        } else {
            shape_idx = self
                .mapping_get(ElementKey::Node(target_node(g, edge)))
                .expect("target node element");
        }

        // Construct the extension and add to mapping
        let ext_parent = if outgoing_extension {
            target_node(g, edge)
        } else {
            source_node(g, edge)
        };
        let dir = nearest_side(g, outer_point, ext_parent);
        let mut extension_width = thickness + self.component_spacing;
        let middle_pos;
        if dir.is_horizontal() {
            extension_width += (outer_point.y - inner_point.y).abs();
            middle_pos = KVector::new(inner_point.x, (inner_point.y + outer_point.y) / 2.0);
        } else {
            extension_width += (outer_point.x - inner_point.x).abs();
            middle_pos = KVector::new((inner_point.x + outer_point.x) / 2.0, inner_point.y);
        }

        let ext = DCExtension::new(&elements[shape_idx].bounds, dir, middle_pos, extension_width);
        elements[shape_idx].extensions.push(ext);
        if outgoing_extension {
            self.outgoing_extensions_mapping.push((edge, dir));
        } else {
            self.incoming_extensions_mapping.push((edge, dir));
        }
        self.element_mapping.push((ElementKey::Edge(edge), shape_idx));

        // ElkEdges can have labels, too!
        for &label in &g.edge(edge).labels {
            let s = &g.label(label).shape;
            let component_label = self.import_shape(
                elements,
                (s.x, s.y, s.width, s.height),
                Some(ElementKey::Label(label)),
                0.0,
                0.0,
            );
            new_component.push(component_label);
        }
    }

    pub fn apply_layout(&mut self, g: &mut ElkGraph, graph: &DCGraph) {
        let graph_dimensions = graph.dimensions;
        let new_width = graph_dimensions.x;
        let new_height = graph_dimensions.y;

        let old_width = g.node(self.parent).shape.width;
        let old_height = g.node(self.parent).shape.height;

        // Adjust size of layout
        g.node_mut(self.parent)
            .shape
            .set_dimensions(graph_dimensions.x, graph_dimensions.y);

        let x_factor = new_width / old_width;
        let y_factor = new_height / old_height;

        let labels = g.node(self.parent).labels.clone();
        for label in labels {
            let s = &mut g.label_mut(label).shape;
            s.x *= x_factor;
            s.y *= y_factor;
        }

        let ports = g.node(self.parent).ports.clone();
        for port in ports {
            let s = &mut g.port_mut(port).shape;
            if s.x > 0.0 {
                s.x *= x_factor;
            }
            if s.y > 0.0 {
                s.y *= y_factor;
            }
        }

        // Apply offsets, whenever necessary (OffsetApplier).
        for &(key, elem) in &self.element_mapping {
            let offset = graph.components[graph.elements[elem].component].offset;
            match key {
                ElementKey::Edge(edge) => {
                    let section = g.edge(edge).sections[0];
                    let mut points = g.section_chain(section);
                    points.offset(offset);
                    crate::core::elkutil::apply_vector_chain(g, &points, section);
                    // getProperty(JUNCTION_POINTS) materializes the default
                    // (an empty KVectorChain).
                    let mut jps: KVectorChain = g
                        .edge(edge)
                        .properties
                        .get(&crate::core::options::JUNCTION_POINTS);
                    jps.offset(offset);
                    g.edge(edge)
                        .properties
                        .set(&crate::core::options::JUNCTION_POINTS, jps);
                }
                ElementKey::Node(node) => {
                    let s = &mut g.node_mut(node).shape;
                    s.x += offset.x;
                    s.y += offset.y;
                }
                ElementKey::Label(label) => {
                    let s = &mut g.label_mut(label).shape;
                    s.x += offset.x;
                    s.y += offset.y;
                }
            }
        }

        let mut adjusted_ports: Vec<PortId> = Vec::new();

        for &(edge, dir) in &self.incoming_extensions_mapping {
            let section = g.edge(edge).sections[0];
            let chain = g.section_chain(section);
            let new_points = adjust_first_segment(g, source_node(g, edge), chain, dir);
            crate::core::elkutil::apply_vector_chain(g, &new_points, section);

            if let Some(port_to_adjust) = source_port(g, edge) {
                if !adjusted_ports.contains(&port_to_adjust) {
                    adjusted_ports.push(port_to_adjust);
                    adjust_related_port(g, port_to_adjust, points_first(&new_points), dir);
                }
            }
        }
        for &(edge, dir) in &self.outgoing_extensions_mapping {
            let section = g.edge(edge).sections[0];
            let chain = KVectorChain::reverse(&g.section_chain(section));
            let new_points = adjust_first_segment(g, target_node(g, edge), chain, dir);
            let new_points = KVectorChain::reverse(&new_points);
            crate::core::elkutil::apply_vector_chain(g, &new_points, section);

            if let Some(port_to_adjust) = target_port(g, edge) {
                if !adjusted_ports.contains(&port_to_adjust) {
                    adjusted_ports.push(port_to_adjust);
                    adjust_related_port(g, port_to_adjust, points_last(&new_points), dir);
                }
            }
        }
    }
}

fn points_first(chain: &KVectorChain) -> KVector {
    chain.first()
}

fn points_last(chain: &KVectorChain) -> KVector {
    chain.last()
}

fn adjust_related_port(g: &mut ElkGraph, port: PortId, edge_point: KVector, dir: DCDirection) {
    let s = &mut g.port_mut(port).shape;
    if dir.is_horizontal() {
        s.y = edge_point.y - s.height / 2.0;
    } else {
        s.x = edge_point.x - s.width / 2.0;
    }
}

fn adjust_first_segment(
    g: &ElkGraph,
    source: NodeId,
    mut chain: KVectorChain,
    dir: DCDirection,
) -> KVectorChain {
    let mut first_point = chain.0.remove(0);
    match dir {
        DCDirection::North => first_point.y = 0.0,
        DCDirection::South => first_point.y = g.node(source).shape.height,
        DCDirection::West => first_point.x = 0.0,
        DCDirection::East => first_point.x = g.node(source).shape.width,
    }
    chain.0.insert(0, first_point);
    chain
}

fn nearest_side(g: &ElkGraph, point: KVector, node: NodeId) -> DCDirection {
    let mut result = DCDirection::North;
    // NORTHVALUE
    let mut shortest_distance = point.y.abs();
    // SOUTHVALUE
    let mut distance = (g.node(node).shape.height - point.y).abs();
    if distance < shortest_distance {
        shortest_distance = distance;
        result = DCDirection::South;
    }
    // WESTVALUE
    distance = point.x.abs();
    if distance < shortest_distance {
        shortest_distance = distance;
        result = DCDirection::West;
    }
    // EASTVALUE
    distance = (g.node(node).shape.width - point.x).abs();
    if distance < shortest_distance {
        result = DCDirection::East;
    }
    result
}

fn get_orthogonal_points(cur_x: f64, cur_y: f64, nxt_x: f64, nxt_y: f64, radius: f64) -> [KVector; 2] {
    let dif_x = nxt_x - cur_x;
    let dif_y = nxt_y - cur_y;

    let angle_radians = dif_x.atan2(dif_y);
    let orth_angle_ccw = angle_radians + HALF_PI;
    let orth_angle_cw = angle_radians - HALF_PI;

    let x_ccw = radius * orth_angle_ccw.sin() + cur_x;
    let y_ccw = radius * orth_angle_ccw.cos() + cur_y;
    let x_cw = radius * orth_angle_cw.sin() + cur_x;
    let y_cw = radius * orth_angle_cw.cos() + cur_y;

    [KVector::new(x_ccw, y_ccw), KVector::new(x_cw, y_cw)]
}

fn compute_intersection(p1: KVector, p2: KVector, p3: KVector, p4: KVector) -> KVector {
    let (x1, y1) = (p1.x, p1.y);
    let (x2, y2) = (p2.x, p2.y);
    let (x3, y3) = (p3.x, p3.y);
    let (x4, y4) = (p4.x, p4.y);

    let factor1 = x1 * y2 - y1 * x2;
    let factor2 = x3 * y4 - y3 * x4;
    let denominator = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);

    let x = (factor1 * (x3 - x4) - factor2 * (x1 - x2)) / denominator;
    let y = (factor1 * (y3 - y4) - factor2 * (y1 - y2)) / denominator;

    KVector::new(x, y)
}

fn get_contour(edge_points: &[KVector], thickness: f64) -> KVectorChain {
    let mut ccw_points: Vec<KVector> = Vec::new();
    let mut cw_points: Vec<KVector> = Vec::new();
    let radius = thickness / 2.0;

    let number_of_points = edge_points.len();

    // special case: first edge segment
    let mut current = edge_points[0];
    let mut successor = edge_points[1];
    let orth_points = get_orthogonal_points(current.x, current.y, successor.x, successor.y, radius);
    ccw_points.push(orth_points[0]);
    cw_points.push(orth_points[1]);

    // normal case: bendpoints have a preceding and a succeeding neighbor
    for i in 2..number_of_points {
        let predecessor = current;
        current = successor;
        successor = edge_points[i];
        let orth_points =
            get_orthogonal_points(current.x, current.y, predecessor.x, predecessor.y, radius);
        ccw_points.push(orth_points[1]);
        cw_points.push(orth_points[0]);

        let orth_points =
            get_orthogonal_points(current.x, current.y, successor.x, successor.y, radius);
        ccw_points.push(orth_points[0]);
        cw_points.push(orth_points[1]);
    }
    // last point: consider the line connecting back to the previous point
    let orth_points = get_orthogonal_points(successor.x, successor.y, current.x, current.y, radius);
    ccw_points.push(orth_points[1]);
    cw_points.push(orth_points[0]);

    // Compute the intersections of the line segments of the orthogonal points
    let mut ccw_merged = KVectorChain::new();
    let mut cw_merged: Vec<KVector> = Vec::new();

    ccw_merged.add_last(ccw_points[0]);
    let mut i = 1;
    while i + 2 < ccw_points.len() {
        let current_point = ccw_points[i];
        let intersection_point = compute_intersection(
            ccw_points[i - 1],
            current_point,
            ccw_points[i + 1],
            ccw_points[i + 2],
        );
        if !intersection_point.x.is_finite() || !intersection_point.y.is_finite() {
            ccw_merged.add_last(current_point);
        } else {
            ccw_merged.add_last(intersection_point);
        }
        i += 2;
    }
    ccw_merged.add_last(ccw_points[ccw_points.len() - 1]);

    cw_merged.push(cw_points[0]);
    let mut i = 1;
    while i + 2 < cw_points.len() {
        let current_point = cw_points[i];
        let intersection_point = compute_intersection(
            cw_points[i - 1],
            current_point,
            cw_points[i + 1],
            cw_points[i + 2],
        );
        if !intersection_point.x.is_finite() || !intersection_point.y.is_finite() {
            cw_merged.push(current_point);
        } else {
            cw_merged.push(intersection_point);
        }
        i += 2;
    }
    cw_merged.push(cw_points[cw_points.len() - 1]);

    // merge lists (one of them in reverse order)
    for v in cw_merged.into_iter().rev() {
        ccw_merged.add_last(v);
    }

    ccw_merged
}
