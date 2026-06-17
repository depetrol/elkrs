//! Rust port of `org.eclipse.elk.alg.force.ForceImportTest`.
//! Imports ELK graphs into the force model and checks node/edge/label counts,
//! plus connected-component splitting.

use elkrs::alg_force::components::split;
use elkrs::alg_force::graph::{FArena, FGraph};
use elkrs::alg_force::importer::import_graph;
use elkrs::alg_force::options::SEPARATE_CONNECTED_COMPONENTS;
use elkrs::graph::graph::{ElkGraph, ElementId, NodeId, ShapeId};

/// Add one 3-node, 2-edge, 2-label component under `root` (mirrors the test's
/// `createElkGraph` body); returns the three node ids.
fn add_component(g: &mut ElkGraph, root: NodeId) -> [NodeId; 3] {
    let n1 = g.create_node(Some(root));
    let n2 = g.create_node(Some(root));
    let n3 = g.create_node(Some(root));
    let e1 = g.create_simple_edge(ShapeId::Node(n1), ShapeId::Node(n2));
    g.create_label("test", ElementId::Edge(e1));
    let e2 = g.create_simple_edge(ShapeId::Node(n1), ShapeId::Node(n3));
    g.create_label("test2", ElementId::Edge(e2));
    [n1, n2, n3]
}

fn import_root(g: &ElkGraph, root: NodeId) -> (FArena, FGraph) {
    import_graph(g, root).expect("import failed")
}

fn check_simple_graph(f: &FGraph) {
    assert_eq!(f.nodes.len(), 3);
    assert_eq!(f.edges.len(), 2);
    assert_eq!(f.labels.len(), 2);
}

#[test]
fn test_import() {
    let mut g = ElkGraph::new();
    let root = g.create_node(None);
    add_component(&mut g, root);
    let (_arena, fgraph) = import_root(&g, root);
    check_simple_graph(&fgraph);
}

#[test]
fn test_separate_connected_components() {
    let mut g = ElkGraph::new();
    let root = g.create_node(None);
    add_component(&mut g, root);
    add_component(&mut g, root);
    g.node(root)
        .properties
        .set(&SEPARATE_CONNECTED_COMPONENTS, true);

    let (mut arena, fgraph) = import_root(&g, root);
    let graphs = split(&mut arena, fgraph);
    assert_eq!(graphs.len(), 2);
    for sub in &graphs {
        check_simple_graph(sub);
    }
}

#[test]
fn test_do_not_separate_connected_components() {
    let mut g = ElkGraph::new();
    let root = g.create_node(None);
    add_component(&mut g, root);
    add_component(&mut g, root);
    g.node(root)
        .properties
        .set(&SEPARATE_CONNECTED_COMPONENTS, false);

    let (mut arena, fgraph) = import_root(&g, root);
    let graphs = split(&mut arena, fgraph);
    assert_eq!(graphs.len(), 1);
    let f = &graphs[0];
    assert_eq!(f.nodes.len(), 6);
    assert_eq!(f.edges.len(), 4);
    assert_eq!(f.labels.len(), 4);
}
