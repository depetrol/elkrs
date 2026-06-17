//! ELK JSON import/export, port of `org.eclipse.elk.graph.json`
//! (`JsonImporter` / `JsonExporter`).

use crate::graph::graph::{ElementId, ElkGraph, EdgeId, NodeId, PortId, SectionId, ShapeId};
use crate::graph::properties::{PropertyHolder, PropertyMap};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use crate::core::data::LayoutMetaDataRegistry;
use crate::core::options::{self, SPACING_INDIVIDUAL};

pub struct JsonImporter<'r> {
    pub registry: &'r LayoutMetaDataRegistry,
    node_ids: IndexMap<String, NodeId>,
    port_ids: IndexMap<String, PortId>,
}

/// Converts a JSON id (string or integer) to its canonical string form.
/// The original type is only ever compared/printed, so the string form is
/// equivalent as long as integers print identically.
fn id_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn require_id(obj: &Map<String, Value>) -> Result<String, String> {
    let idv = obj
        .get("id")
        .ok_or_else(|| "Every element must have an id.".to_string())?;
    match idv {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => {
            let d = n.as_f64().unwrap_or(f64::NAN);
            if d % 1.0 == 0.0 {
                // Converts to the integer value, so "3.0" and "3" are the same id
                Ok(format!("{}", d as i64))
            } else {
                Err(format!("Id must be a string or an integer: '{n}'."))
            }
        }
        _ => Err(format!("Id must be a string or an integer: '{idv}'.")),
    }
}

fn opt_double(obj: &Map<String, Value>, key: &str) -> Option<f64> {
    let v = obj.get(key)?.as_f64()?;
    // NaN/inf map to 0.0
    if v.is_nan() || v.is_infinite() {
        Some(0.0)
    } else {
        Some(v)
    }
}

impl<'r> JsonImporter<'r> {
    pub fn new(registry: &'r LayoutMetaDataRegistry) -> Self {
        JsonImporter { registry, node_ids: IndexMap::new(), port_ids: IndexMap::new() }
    }

    pub fn import_graph(&mut self, json: &Value) -> Result<ElkGraph, String> {
        let mut g = ElkGraph::new();
        let root_obj = json
            .as_object()
            .ok_or_else(|| "graph must be a JSON object".to_string())?;
        let root = g.root;
        self.fill_node(&mut g, root, root_obj)?;
        self.transform_edges(&mut g, root, root_obj)?;
        Ok(g)
    }

    fn fill_node(
        &mut self,
        g: &mut ElkGraph,
        node: NodeId,
        obj: &Map<String, Value>,
    ) -> Result<(), String> {
        // Registering the node: the id is mandatory.
        let id = require_id(obj)?;
        g.node_mut(node).identifier = Some(id.clone());
        self.node_ids.insert(id, node);
        self.transform_properties(g.node_mut(node).properties_mut(), obj);
        self.transform_individual_spacings(g.node_mut(node).properties_mut(), obj);
        {
            let shape = &mut g.node_mut(node).shape;
            if let Some(v) = opt_double(obj, "x") {
                shape.x = v;
            }
            if let Some(v) = opt_double(obj, "y") {
                shape.y = v;
            }
            if let Some(v) = opt_double(obj, "width") {
                shape.width = v;
            }
            if let Some(v) = opt_double(obj, "height") {
                shape.height = v;
            }
        }
        if let Some(ports) = obj.get("ports").and_then(Value::as_array) {
            for jport in ports {
                if let Some(pobj) = jport.as_object() {
                    self.transform_port(g, node, pobj)?;
                }
            }
        }
        self.transform_labels(g, ElementId::Node(node), obj)?;
        if let Some(children) = obj.get("children").and_then(Value::as_array) {
            for jchild in children {
                if let Some(cobj) = jchild.as_object() {
                    let child = g.create_node(Some(node));
                    self.fill_node(g, child, cobj)?;
                }
            }
        }
        Ok(())
    }

    fn transform_port(
        &mut self,
        g: &mut ElkGraph,
        parent: NodeId,
        obj: &Map<String, Value>,
    ) -> Result<(), String> {
        let port = g.create_port(parent);
        // Registering the port: the id is mandatory.
        let id = require_id(obj)?;
        g.port_mut(port).identifier = Some(id.clone());
        self.port_ids.insert(id, port);
        self.transform_properties(g.port_mut(port).properties_mut(), obj);
        {
            let shape = &mut g.port_mut(port).shape;
            if let Some(v) = opt_double(obj, "x") {
                shape.x = v;
            }
            if let Some(v) = opt_double(obj, "y") {
                shape.y = v;
            }
            if let Some(v) = opt_double(obj, "width") {
                shape.width = v;
            }
            if let Some(v) = opt_double(obj, "height") {
                shape.height = v;
            }
        }
        self.transform_labels(g, ElementId::Port(port), obj)?;
        Ok(())
    }

    fn transform_labels(
        &mut self,
        g: &mut ElkGraph,
        owner: ElementId,
        obj: &Map<String, Value>,
    ) -> Result<(), String> {
        if let Some(labels) = obj.get("labels").and_then(Value::as_array) {
            for jlabel in labels {
                if let Some(lobj) = jlabel.as_object() {
                    let text = lobj.get("text").and_then(Value::as_str).unwrap_or("");
                    let label = g.create_label(text, owner);
                    if let Some(idv) = lobj.get("id") {
                        g.label_mut(label).identifier = id_string(idv);
                    }
                    self.transform_properties(g.label_mut(label).properties_mut(), lobj);
                    let shape = &mut g.label_mut(label).shape;
                    if let Some(v) = opt_double(lobj, "x") {
                        shape.x = v;
                    }
                    if let Some(v) = opt_double(lobj, "y") {
                        shape.y = v;
                    }
                    if let Some(v) = opt_double(lobj, "width") {
                        shape.width = v;
                    }
                    if let Some(v) = opt_double(lobj, "height") {
                        shape.height = v;
                    }
                }
            }
        }
        Ok(())
    }

    /// Walks the node hierarchy a second time creating edges.
    fn transform_edges(
        &mut self,
        g: &mut ElkGraph,
        node: NodeId,
        obj: &Map<String, Value>,
    ) -> Result<(), String> {
        if let Some(edges) = obj.get("edges").and_then(Value::as_array) {
            for jedge in edges {
                if let Some(eobj) = jedge.as_object() {
                    let edge = if eobj.contains_key("sources") || eobj.contains_key("targets") {
                        self.transform_edge(g, node, eobj)?
                    } else {
                        self.transform_primitive_edge(g, node, eobj)?
                    };
                    g.update_containment(edge);
                }
            }
        }
        // recurse into children, matching the original child order
        let children: Vec<NodeId> = g.node(node).children.clone();
        let jchildren = obj.get("children").and_then(Value::as_array);
        if let Some(jchildren) = jchildren {
            let mut child_iter = children.into_iter();
            for jchild in jchildren {
                if let Some(cobj) = jchild.as_object() {
                    let child = child_iter
                        .next()
                        .ok_or("internal error: child count mismatch")?;
                    self.transform_edges(g, child, cobj)?;
                }
            }
        }
        Ok(())
    }

    fn shape_by_id(&self, id: &str) -> Result<ShapeId, String> {
        if let Some(&n) = self.node_ids.get(id) {
            return Ok(ShapeId::Node(n));
        }
        if let Some(&p) = self.port_ids.get(id) {
            return Ok(ShapeId::Port(p));
        }
        Err(format!("Referenced shape does not exist: {id}"))
    }

    fn transform_edge(
        &mut self,
        g: &mut ElkGraph,
        parent: NodeId,
        obj: &Map<String, Value>,
    ) -> Result<EdgeId, String> {
        let edge = g.create_edge(Some(parent));
        // Registering the edge: the id is mandatory.
        g.edge_mut(edge).identifier = Some(require_id(obj)?);
        if let Some(sources) = obj.get("sources").and_then(Value::as_array) {
            for s in sources {
                let sid = id_string(s).ok_or("edge source id must be string or number")?;
                let shape = self.shape_by_id(&sid)?;
                g.add_edge_source(edge, shape);
            }
        }
        if let Some(targets) = obj.get("targets").and_then(Value::as_array) {
            for t in targets {
                let tid = id_string(t).ok_or("edge target id must be string or number")?;
                let shape = self.shape_by_id(&tid)?;
                g.add_edge_target(edge, shape);
            }
        }
        if g.edge(edge).sources.is_empty() || g.edge(edge).targets.is_empty() {
            return Err(format!(
                "An edge must have at least one source and one target (edge id: '{:?}').",
                g.edge(edge).identifier
            ));
        }
        self.transform_properties(g.edge_mut(edge).properties_mut(), obj);
        self.transform_edge_sections(g, edge, obj)?;
        self.transform_labels(g, ElementId::Edge(edge), obj)?;
        Ok(edge)
    }

    fn transform_primitive_edge(
        &mut self,
        g: &mut ElkGraph,
        parent: NodeId,
        obj: &Map<String, Value>,
    ) -> Result<EdgeId, String> {
        let edge = g.create_edge(Some(parent));
        // Registering the edge: the id is mandatory.
        g.edge_mut(edge).identifier = Some(require_id(obj)?);
        let src_node = obj
            .get("source")
            .and_then(id_string_opt)
            .and_then(|id| self.node_ids.get(&id).copied())
            .ok_or("An edge must have a source node.")?;
        let src_port = obj
            .get("sourcePort")
            .and_then(id_string_opt)
            .and_then(|id| self.port_ids.get(&id).copied());
        if let Some(p) = src_port {
            if g.port(p).parent != Some(src_node) {
                return Err(
                    "The source port of an edge must be a port of the edge's source node.".into()
                );
            }
            g.add_edge_source(edge, ShapeId::Port(p));
        } else {
            g.add_edge_source(edge, ShapeId::Node(src_node));
        }
        let tgt_node = obj
            .get("target")
            .and_then(id_string_opt)
            .and_then(|id| self.node_ids.get(&id).copied())
            .ok_or("An edge must have a target node.")?;
        let tgt_port = obj
            .get("targetPort")
            .and_then(id_string_opt)
            .and_then(|id| self.port_ids.get(&id).copied());
        if let Some(p) = tgt_port {
            if g.port(p).parent != Some(tgt_node) {
                return Err(
                    "The target port of an edge must be a port of the edge's target node.".into()
                );
            }
            g.add_edge_target(edge, ShapeId::Port(p));
        } else {
            g.add_edge_target(edge, ShapeId::Node(tgt_node));
        }
        self.transform_properties(g.edge_mut(edge).properties_mut(), obj);
        // primitive edge layout: sourcePoint/targetPoint/bendPoints
        if obj.contains_key("sourcePoint")
            || obj.contains_key("targetPoint")
            || obj.contains_key("bendPoints")
        {
            let section = g.create_section(edge);
            if let Some(p) = obj.get("sourcePoint").and_then(Value::as_object) {
                let s = g.section_mut(section);
                s.start_x = opt_double(p, "x").unwrap_or(0.0);
                s.start_y = opt_double(p, "y").unwrap_or(0.0);
            }
            if let Some(p) = obj.get("targetPoint").and_then(Value::as_object) {
                let s = g.section_mut(section);
                s.end_x = opt_double(p, "x").unwrap_or(0.0);
                s.end_y = opt_double(p, "y").unwrap_or(0.0);
            }
            if let Some(bps) = obj.get("bendPoints").and_then(Value::as_array) {
                for bp in bps {
                    if let Some(p) = bp.as_object() {
                        g.section_mut(section).bend_points.push((
                            opt_double(p, "x").unwrap_or(0.0),
                            opt_double(p, "y").unwrap_or(0.0),
                        ));
                    }
                }
            }
        }
        self.transform_labels(g, ElementId::Edge(edge), obj)?;
        Ok(edge)
    }

    fn transform_edge_sections(
        &mut self,
        g: &mut ElkGraph,
        edge: EdgeId,
        obj: &Map<String, Value>,
    ) -> Result<(), String> {
        let mut section_ids: IndexMap<String, SectionId> = IndexMap::new();
        let mut pending_incoming: Vec<(SectionId, String)> = Vec::new();
        let mut pending_outgoing: Vec<(SectionId, String)> = Vec::new();

        if let Some(sections) = obj.get("sections").and_then(Value::as_array) {
            for jsec in sections {
                let sobj = match jsec.as_object() {
                    Some(s) => s,
                    None => continue,
                };
                let section = g.create_section(edge);
                // Registering the edge section: the id is mandatory.
                let id = require_id(sobj)?;
                g.section_mut(section).identifier = Some(id.clone());
                section_ids.insert(id, section);
                let start = sobj
                    .get("startPoint")
                    .and_then(Value::as_object)
                    .ok_or("All edge sections need a start point.")?;
                let end = sobj
                    .get("endPoint")
                    .and_then(Value::as_object)
                    .ok_or("All edge sections need an end point.")?;
                {
                    let s = g.section_mut(section);
                    s.start_x = opt_double(start, "x").unwrap_or(0.0);
                    s.start_y = opt_double(start, "y").unwrap_or(0.0);
                    s.end_x = opt_double(end, "x").unwrap_or(0.0);
                    s.end_y = opt_double(end, "y").unwrap_or(0.0);
                }
                if let Some(bps) = sobj.get("bendPoints").and_then(Value::as_array) {
                    for bp in bps {
                        if let Some(p) = bp.as_object() {
                            g.section_mut(section).bend_points.push((
                                opt_double(p, "x").unwrap_or(0.0),
                                opt_double(p, "y").unwrap_or(0.0),
                            ));
                        }
                    }
                }
                if let Some(s) = sobj.get("incomingShape").and_then(Value::as_str) {
                    let shape = self.shape_by_id(s)?;
                    g.section_mut(section).incoming_shape = Some(shape);
                }
                if let Some(s) = sobj.get("outgoingShape").and_then(Value::as_str) {
                    let shape = self.shape_by_id(s)?;
                    g.section_mut(section).outgoing_shape = Some(shape);
                }
                if let Some(ids) = sobj.get("incomingSections").and_then(Value::as_array) {
                    for idv in ids {
                        let id = id_string(idv).ok_or("section ref must be string or number")?;
                        pending_incoming.push((section, id));
                    }
                }
                if let Some(ids) = sobj.get("outgoingSections").and_then(Value::as_array) {
                    for idv in ids {
                        let id = id_string(idv).ok_or("section ref must be string or number")?;
                        pending_outgoing.push((section, id));
                    }
                }
            }
        }

        for (section, id) in pending_incoming {
            let referenced = *section_ids
                .get(&id)
                .ok_or_else(|| format!("Referenced edge section does not exist: {id}"))?;
            g.section_mut(section).incoming_sections.push(referenced);
        }
        for (section, id) in pending_outgoing {
            let referenced = *section_ids
                .get(&id)
                .ok_or_else(|| format!("Referenced edge section does not exist: {id}"))?;
            g.section_mut(section).outgoing_sections.push(referenced);
        }

        // Special case: single source/target/section without shapes
        let e = g.edge(edge);
        if e.sources.len() == 1 && e.targets.len() == 1 && e.sections.len() == 1 {
            let section = e.sections[0];
            let (src, tgt) = (e.sources[0], e.targets[0]);
            let s = g.section(section);
            if s.incoming_shape.is_none() && s.outgoing_shape.is_none() {
                let s = g.section_mut(section);
                s.incoming_shape = Some(src);
                s.outgoing_shape = Some(tgt);
            }
        }
        Ok(())
    }

    fn transform_properties(&self, props: &mut PropertyMap, obj: &Map<String, Value>) {
        let opts = obj
            .get("layoutOptions")
            .or_else(|| obj.get("properties"))
            .and_then(Value::as_object);
        if let Some(opts) = opts {
            for (k, v) in opts {
                let value_str = json_value_to_string(v);
                if let Some(data) = self.registry.option_by_suffix(k) {
                    if let Some(parsed) = data.parse_value(&value_str) {
                        props.set_by_id(data.id, parsed);
                    }
                }
            }
        }
    }

    fn transform_individual_spacings(&self, props: &mut PropertyMap, obj: &Map<String, Value>) {
        if let Some(spacings) = obj.get("individualSpacings").and_then(Value::as_object) {
            let individual = props
                .try_get(&SPACING_INDIVIDUAL)
                .unwrap_or_default();
            for (k, v) in spacings {
                let value_str = json_value_to_string(v);
                if let Some(data) = self.registry.option_by_suffix(k) {
                    if let Some(parsed) = data.parse_value(&value_str) {
                        individual.properties.set_by_id(data.id, parsed);
                    }
                }
            }
            props.set(&SPACING_INDIVIDUAL, individual);
        }
    }
}

fn id_string_opt(v: &Value) -> Option<String> {
    id_string(v)
}

/// Converts JSON primitives to their string forms.
fn json_value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

// --------------------------------------------------------------------------
//  Export
// --------------------------------------------------------------------------

pub struct JsonExporter<'r> {
    pub registry: &'r LayoutMetaDataRegistry,
    pub omit_zero_pos: bool,
    pub omit_zero_dim: bool,
    pub omit_layout: bool,
    pub omit_unknown_options: bool,
    node_ids: IndexMap<NodeId, String>,
    port_ids: IndexMap<PortId, String>,
    section_ids: IndexMap<SectionId, String>,
    used_node_ids: IndexMap<String, ()>,
    used_port_ids: IndexMap<String, ()>,
    used_edge_ids: IndexMap<String, ()>,
    used_section_ids: IndexMap<String, ()>,
    node_id_counter: usize,
    port_id_counter: usize,
    edge_id_counter: usize,
    section_id_counter: usize,
}

impl<'r> JsonExporter<'r> {
    pub fn new(registry: &'r LayoutMetaDataRegistry) -> Self {
        JsonExporter {
            registry,
            omit_zero_pos: false,
            omit_zero_dim: false,
            omit_layout: false,
            omit_unknown_options: true,
            node_ids: IndexMap::new(),
            port_ids: IndexMap::new(),
            section_ids: IndexMap::new(),
            used_node_ids: IndexMap::new(),
            used_port_ids: IndexMap::new(),
            used_edge_ids: IndexMap::new(),
            used_section_ids: IndexMap::new(),
            node_id_counter: 0,
            port_id_counter: 0,
            edge_id_counter: 0,
            section_id_counter: 0,
        }
    }

    pub fn export(&mut self, g: &ElkGraph) -> Value {
        // First pass: assign ids to nodes and ports and sections (sections are
        // assigned during edge emission, but ids only depend on
        // identifiers/counters which we replicate in the same order).
        let mut root_json = self.transform_node(g, g.root);
        self.transform_edges(g, g.root, &mut root_json);
        root_json
    }

    fn assign_id(
        identifier: &Option<String>,
        prefix: &str,
        counter: &mut usize,
        used: &mut IndexMap<String, ()>,
    ) -> String {
        let mut id = match identifier {
            Some(s) => s.clone(),
            None => {
                let id = format!("{prefix}{counter}");
                *counter += 1;
                id
            }
        };
        // Collisions only happen with malformed input. Use a deterministic
        // suffix.
        while used.contains_key(&id) {
            id.push('_');
        }
        used.insert(id.clone(), ());
        id
    }

    fn transform_node(&mut self, g: &ElkGraph, node: NodeId) -> Value {
        let mut obj = Map::new();
        let id = Self::assign_id(
            &g.node(node).identifier,
            "n",
            &mut self.node_id_counter,
            &mut self.used_node_ids,
        );
        self.node_ids.insert(node, id.clone());
        obj.insert("id".into(), Value::String(id));

        let n = g.node(node);
        if !n.labels.is_empty() {
            let labels: Vec<Value> =
                n.labels.iter().map(|&l| self.transform_label(g, l)).collect();
            obj.insert("labels".into(), Value::Array(labels));
        }
        if !n.ports.is_empty() {
            let ports: Vec<Value> =
                n.ports.iter().map(|&p| self.transform_port(g, p)).collect();
            obj.insert("ports".into(), Value::Array(ports));
        }
        if !n.children.is_empty() {
            let children: Vec<Value> = n
                .children
                .clone()
                .iter()
                .map(|&c| self.transform_node(g, c))
                .collect();
            obj.insert("children".into(), Value::Array(children));
        }
        self.transform_properties(&n.properties, &mut obj);
        self.transform_individual_spacings(&n.properties, &mut obj);
        self.transfer_shape_layout(n.shape.x, n.shape.y, n.shape.width, n.shape.height, &mut obj);
        Value::Object(obj)
    }

    fn transform_port(&mut self, g: &ElkGraph, port: PortId) -> Value {
        let mut obj = Map::new();
        let id = Self::assign_id(
            &g.port(port).identifier,
            "p",
            &mut self.port_id_counter,
            &mut self.used_port_ids,
        );
        self.port_ids.insert(port, id.clone());
        obj.insert("id".into(), Value::String(id));
        let p = g.port(port);
        if !p.labels.is_empty() {
            let labels: Vec<Value> =
                p.labels.iter().map(|&l| self.transform_label(g, l)).collect();
            obj.insert("labels".into(), Value::Array(labels));
        }
        self.transform_properties(&p.properties, &mut obj);
        self.transfer_shape_layout(p.shape.x, p.shape.y, p.shape.width, p.shape.height, &mut obj);
        Value::Object(obj)
    }

    fn transform_label(&mut self, g: &ElkGraph, label: crate::graph::graph::LabelId) -> Value {
        let mut obj = Map::new();
        let l = g.label(label);
        obj.insert("text".into(), Value::String(l.text.clone()));
        if let Some(id) = &l.identifier {
            if !id.is_empty() {
                obj.insert("id".into(), Value::String(id.clone()));
            }
        }
        self.transform_properties(&l.properties, &mut obj);
        self.transfer_shape_layout(l.shape.x, l.shape.y, l.shape.width, l.shape.height, &mut obj);
        Value::Object(obj)
    }

    fn transform_edges(&mut self, g: &ElkGraph, node: NodeId, json: &mut Value) {
        let n = g.node(node);
        if !n.contained_edges.is_empty() {
            let edges: Vec<Value> = n
                .contained_edges
                .iter()
                .map(|&e| self.transform_edge(g, e))
                .collect();
            json.as_object_mut()
                .unwrap()
                .insert("edges".into(), Value::Array(edges));
        }
        // find the json objects of children: children array order matches
        let children = n.children.clone();
        if !children.is_empty() {
            // Move out the array to avoid double borrow
            if let Some(Value::Array(child_jsons)) =
                json.as_object_mut().unwrap().get_mut("children")
            {
                for (child, child_json) in children.iter().zip(child_jsons.iter_mut()) {
                    self.transform_edges(g, *child, child_json);
                }
            }
        }
    }

    fn transform_edge(&mut self, g: &ElkGraph, edge: EdgeId) -> Value {
        let mut obj = Map::new();
        let id = Self::assign_id(
            &g.edge(edge).identifier,
            "e",
            &mut self.edge_id_counter,
            &mut self.used_edge_ids,
        );
        obj.insert("id".into(), Value::String(id));

        let e = g.edge(edge);
        let shape_id = |this: &Self, s: &ShapeId| -> Value {
            match s {
                ShapeId::Node(n) => Value::String(this.node_ids[n].clone()),
                ShapeId::Port(p) => Value::String(this.port_ids[p].clone()),
            }
        };
        let sources: Vec<Value> = e.sources.iter().map(|s| shape_id(self, s)).collect();
        obj.insert("sources".into(), Value::Array(sources));
        let targets: Vec<Value> = e.targets.iter().map(|t| shape_id(self, t)).collect();
        obj.insert("targets".into(), Value::Array(targets));

        if !e.labels.is_empty() {
            let labels: Vec<Value> =
                e.labels.clone().iter().map(|&l| self.transform_label(g, l)).collect();
            obj.insert("labels".into(), Value::Array(labels));
        }
        if !self.omit_layout && !e.sections.is_empty() {
            let sections: Vec<Value> = e
                .sections
                .clone()
                .iter()
                .map(|&s| self.transform_section(g, s))
                .collect();
            obj.insert("sections".into(), Value::Array(sections));
        }
        if !self.omit_layout {
            if let Some(jps) = e.properties.try_get(&options::JUNCTION_POINTS) {
                if !jps.is_empty() {
                    let arr: Vec<Value> = jps
                        .iter()
                        .map(|p| json!({"x": p.x, "y": p.y}))
                        .collect();
                    obj.insert("junctionPoints".into(), Value::Array(arr));
                }
            }
        }
        self.transform_properties(&g.edge(edge).properties, &mut obj);
        Value::Object(obj)
    }

    fn transform_section(&mut self, g: &ElkGraph, section: SectionId) -> Value {
        let mut obj = Map::new();
        let id = Self::assign_id(
            &g.section(section).identifier,
            "s",
            &mut self.section_id_counter,
            &mut self.used_section_ids,
        );
        self.section_ids.insert(section, id.clone());
        obj.insert("id".into(), Value::String(id));
        let s = g.section(section);
        obj.insert("startPoint".into(), json!({"x": s.start_x, "y": s.start_y}));
        obj.insert("endPoint".into(), json!({"x": s.end_x, "y": s.end_y}));
        if !self.omit_layout && !s.bend_points.is_empty() {
            let bps: Vec<Value> =
                s.bend_points.iter().map(|(x, y)| json!({"x": x, "y": y})).collect();
            obj.insert("bendPoints".into(), Value::Array(bps));
        }
        if let Some(shape) = &s.incoming_shape {
            obj.insert("incomingShape".into(), self.shape_ref(shape));
        }
        if let Some(shape) = &s.outgoing_shape {
            obj.insert("outgoingShape".into(), self.shape_ref(shape));
        }
        if !s.incoming_sections.is_empty() {
            let arr: Vec<Value> = s
                .incoming_sections
                .iter()
                .map(|sec| Value::String(self.section_ids[sec].clone()))
                .collect();
            obj.insert("incomingSections".into(), Value::Array(arr));
        }
        if !s.outgoing_sections.is_empty() {
            let arr: Vec<Value> = s
                .outgoing_sections
                .iter()
                .map(|sec| Value::String(self.section_ids[sec].clone()))
                .collect();
            obj.insert("outgoingSections".into(), Value::Array(arr));
        }
        self.transform_properties(&s.properties, &mut obj);
        Value::Object(obj)
    }

    fn shape_ref(&self, s: &ShapeId) -> Value {
        match s {
            ShapeId::Node(n) => Value::String(self.node_ids[n].clone()),
            ShapeId::Port(p) => Value::String(self.port_ids[p].clone()),
        }
    }

    fn transform_properties(&self, props: &PropertyMap, obj: &mut Map<String, Value>) {
        if props.is_empty() {
            return;
        }
        // The (possibly empty) layoutOptions object is added to the parent
        // whenever the element has any properties at all; the unknown-option
        // filtering happens only afterwards.
        let mut json_props = Map::new();
        for (key, value) in props.entries() {
            if key == SPACING_INDIVIDUAL.id {
                continue;
            }
            // A property id is resolved as known by suffix.
            if self.omit_unknown_options && self.registry.option_by_suffix(&key).is_none() {
                continue;
            }
            json_props.insert(key.to_string(), Value::String(value.to_java_string()));
        }
        obj.insert("layoutOptions".into(), Value::Object(json_props));
    }

    fn transform_individual_spacings(&self, props: &PropertyMap, obj: &mut Map<String, Value>) {
        if let Some(individual) = props.try_get(&SPACING_INDIVIDUAL) {
            if individual.properties.is_empty() {
                return;
            }
            let mut json_props = Map::new();
            for (key, value) in individual.properties.entries() {
                // A property id is resolved as known by suffix.
                if self.omit_unknown_options && self.registry.option_by_suffix(&key).is_none() {
                    continue;
                }
                json_props.insert(key.to_string(), Value::String(value.to_java_string()));
            }
            obj.insert("individualSpacings".into(), Value::Object(json_props));
        }
    }

    fn transfer_shape_layout(
        &self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        obj: &mut Map<String, Value>,
    ) {
        if !self.omit_layout {
            if !self.omit_zero_pos || x != 0.0 {
                obj.insert("x".into(), json!(x));
            }
            if !self.omit_zero_pos || y != 0.0 {
                obj.insert("y".into(), json!(y));
            }
        }
        if !self.omit_zero_dim || w != 0.0 {
            obj.insert("width".into(), json!(w));
        }
        if !self.omit_zero_dim || h != 0.0 {
            obj.insert("height".into(), json!(h));
        }
    }
}
