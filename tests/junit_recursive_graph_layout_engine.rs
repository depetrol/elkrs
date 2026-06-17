//! Port of `org.eclipse.elk.core.RecursiveGraphLayoutEngineTest`
//! (elk/test/org.eclipse.elk.core.test).

use elkrs::core::engine::RecursiveGraphLayoutEngine;
use elkrs::core::options::{ResolvedAlgorithm, RESOLVED_ALGORITHM_TYPED};
use elkrs::core::options_gen::ALGORITHM;
use elkrs::core::providers::box_layouter::BoxLayoutProvider;
use elkrs::core::registry::{AlgorithmData, AlgorithmRegistry};
use elkrs::core::Elk;
use elkrs::graph::graph::ElkGraph;
use elkrs::graph::properties::EnumSet;

/// Java `RecursiveGraphLayoutEngineTest.Graph`: a root with two 10x10 nodes.
fn create_graph() -> ElkGraph {
    let mut g = ElkGraph::new();
    let root = g.root;
    let n1 = g.create_node(Some(root));
    g.node_mut(n1).shape.set_dimensions(10.0, 10.0);
    let n2 = g.create_node(Some(root));
    g.node_mut(n2).shape.set_dimensions(10.0, 10.0);
    g
}

/// Java `RecursiveGraphLayoutEngineTest.testUnresolvedGraph`.
#[test]
fn test_unresolved_graph() {
    let elk = Elk::new();
    let mut g = create_graph();
    let root = g.root;
    g.node_mut(root).properties.set(&ALGORITHM, "org.eclipse.elk.box".to_string());
    let engine = RecursiveGraphLayoutEngine::new(&elk.algorithms);
    engine.layout(&mut g).unwrap();

    assert!(g.node(g.root).shape.width > 0.0);
    assert!(g.node(g.root).shape.height > 0.0);
}

/// Java `RecursiveGraphLayoutEngineTest.testResolvedGraph`.
#[test]
fn test_resolved_graph() {
    let elk = Elk::new();
    let mut g = create_graph();
    let root = g.root;
    // Java sets CoreOptions.RESOLVED_ALGORITHM to the LayoutAlgorithmData
    // fetched from the LayoutMetaDataService.
    let data = elk.algorithms.by_id("org.eclipse.elk.box").unwrap();
    g.node_mut(root)
        .properties
        .set(&RESOLVED_ALGORITHM_TYPED, ResolvedAlgorithm(data.id.to_string()));
    let engine = RecursiveGraphLayoutEngine::new(&elk.algorithms);
    engine.layout(&mut g).unwrap();

    assert!(g.node(g.root).shape.width > 0.0);
    assert!(g.node(g.root).shape.height > 0.0);
}

/// Java `RecursiveGraphLayoutEngineTest.testUnknownAlgorithmId`
/// (Java expects `UnsupportedConfigurationException`; the Rust port
/// returns `Err`).
#[test]
fn test_unknown_algorithm_id() {
    let elk = Elk::new();
    let mut g = create_graph();
    let root = g.root;
    g.node_mut(root).properties.set(&ALGORITHM, "foo.Bar".to_string());
    let engine = RecursiveGraphLayoutEngine::new(&elk.algorithms);
    assert!(engine.layout(&mut g).is_err());
}

/// Java `RecursiveGraphLayoutEngineTest.testEmptyAlgorithmId`: with no
/// algorithm configured, the engine resolves to the default algorithm
/// `org.eclipse.elk.layered`.
///
/// elk-core cannot depend on elk-alg-layered, so a stub registration with
/// the layered id (backed by the box provider) stands in for the real
/// algorithm; the assertion on the resolved id is identical to Java's.
#[test]
fn test_empty_algorithm_id() {
    let mut algorithms = AlgorithmRegistry::default();
    elkrs::core::providers::register_core_algorithms(&mut algorithms);
    algorithms.register(AlgorithmData {
        id: "org.eclipse.elk.layered",
        name: "ELK Layered (stub)",
        features: EnumSet::none(),
        create: || Box::new(BoxLayoutProvider),
    });

    let mut g = create_graph();
    let engine = RecursiveGraphLayoutEngine::new(&algorithms);
    engine.layout(&mut g).unwrap();

    assert_eq!(
        "org.eclipse.elk.layered",
        g.node(g.root).properties.try_get(&RESOLVED_ALGORITHM_TYPED).unwrap().0
    );
}
