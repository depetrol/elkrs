//! Port of `org.eclipse.elk.graph.util.ElkGraphUtilTest`
//! (elk/test/org.eclipse.elk.graph.test).
//!
//! Java's `ElkGraphUtil` free functions live on `ElkGraph` in the Rust port
//! (`shape_node`, `shape_port`, `find_best_edge_containment`). The two
//! `NullPointerException` tests have no Rust counterpart: passing null is
//! not expressible in the type system.

use elkrs::graph::graph::{ElkGraph, EdgeId, NodeId, ShapeId};

/// Java `ElkGraphUtilTest.testConnectableShapeToNode`.
#[test]
fn test_connectable_shape_to_node() {
    let mut g = ElkGraph::new();
    let node = g.create_node(None);
    assert_eq!(node, g.shape_node(ShapeId::Node(node)));

    let port = g.create_port(node);
    assert_eq!(node, g.shape_node(ShapeId::Port(port)));
}

/// Java `ElkGraphUtilTest.testConnectableShapeToPort`.
#[test]
fn test_connectable_shape_to_port() {
    let mut g = ElkGraph::new();
    let node = g.create_node(None);
    assert_eq!(None, g.shape_port(ShapeId::Node(node)));

    let port = g.create_port(node);
    assert_eq!(Some(port), g.shape_port(ShapeId::Port(port)));
}

/// Creates an edge which connects the two shapes, but does not do so in a way
/// that automatically sets the edge's containment (Java
/// `createEdgeWithoutContainment`).
fn create_edge_without_containment(
    g: &mut ElkGraph,
    source: Option<NodeId>,
    target: Option<NodeId>,
) -> EdgeId {
    let edge = g.create_edge(None);
    if let Some(s) = source {
        g.edge_mut(edge).sources.push(ShapeId::Node(s));
    }
    if let Some(t) = target {
        g.edge_mut(edge).targets.push(ShapeId::Node(t));
    }
    edge
}

/// Java `ElkGraphUtilTest.testFindBestEdgeContainment`.
#[test]
fn test_find_best_edge_containment() {
    let mut g = ElkGraph::new();

    // Create a basic hierarchical graph that covers all possibilities
    let graph1 = g.root;

    let node1 = g.create_node(Some(graph1));
    let node2 = g.create_node(Some(graph1));
    let node3 = g.create_node(Some(graph1));
    let node3_1 = g.create_node(Some(node3));
    let node3_1_1 = g.create_node(Some(node3_1));
    let node4 = g.create_node(Some(graph1));
    let node4_1 = g.create_node(Some(node4));

    // Create a second graph to test special cases later (a second parentless
    // tree in the same arena)
    let graph2 = g.create_node(None);

    let node_a = g.create_node(Some(graph2));

    // Edge which connects two top-level nodes
    let same_level_edge = create_edge_without_containment(&mut g, Some(node1), Some(node2));
    assert_eq!(Some(graph1), g.find_best_edge_containment(same_level_edge));

    // Self loop
    let self_loop_edge = create_edge_without_containment(&mut g, Some(node1), Some(node1));
    assert_eq!(Some(graph1), g.find_best_edge_containment(self_loop_edge));

    // Edge which connects a source on a higher level to a target on a lower level
    let down_level_edge = create_edge_without_containment(&mut g, Some(node1), Some(node3_1));
    assert_eq!(Some(graph1), g.find_best_edge_containment(down_level_edge));

    // Edge which connects a source on a lower level to a target on a higher level
    let up_level_edge = create_edge_without_containment(&mut g, Some(node3_1), Some(node2));
    assert_eq!(Some(graph1), g.find_best_edge_containment(up_level_edge));

    // Edge which connects a hierarchical node to its child
    let to_child_edge = create_edge_without_containment(&mut g, Some(node3), Some(node3_1));
    assert_eq!(Some(node3), g.find_best_edge_containment(to_child_edge));

    // Edge which connects a node to its parent
    let to_parent_edge = create_edge_without_containment(&mut g, Some(node3_1), Some(node3));
    assert_eq!(Some(node3), g.find_best_edge_containment(to_parent_edge));

    // Edge which connects a hierarchical node to its grand child
    let to_grand_child_edge = create_edge_without_containment(&mut g, Some(node3), Some(node3_1_1));
    assert_eq!(Some(node3), g.find_best_edge_containment(to_grand_child_edge));

    // Edge which connects a node to its grand parent
    let to_grand_parent_edge =
        create_edge_without_containment(&mut g, Some(node3_1_1), Some(node3));
    assert_eq!(Some(node3), g.find_best_edge_containment(to_grand_parent_edge));

    // Edge which connects two nodes on different branches of the inclusion tree
    let cross_hierarchy_edge = create_edge_without_containment(&mut g, Some(node3_1), Some(node4_1));
    assert_eq!(Some(graph1), g.find_best_edge_containment(cross_hierarchy_edge));

    // Edge that connects nodes of different graphs
    let cross_graph_edge = create_edge_without_containment(&mut g, Some(node1), Some(node_a));
    assert_eq!(None, g.find_best_edge_containment(cross_graph_edge));

    // Partially specified edges
    let source_missing_edge = create_edge_without_containment(&mut g, None, Some(node1));
    assert_eq!(Some(graph1), g.find_best_edge_containment(source_missing_edge));

    let target_missing_edge = create_edge_without_containment(&mut g, Some(node1), None);
    assert_eq!(Some(graph1), g.find_best_edge_containment(target_missing_edge));
}

/// Java `ElkGraphUtilTest.testFindBestEdgeContainmentWithUnconnectedEdge`
/// (Java expects `IllegalArgumentException`).
#[test]
#[should_panic(expected = "The edge must have at least one source or target.")]
fn test_find_best_edge_containment_with_unconnected_edge() {
    let mut g = ElkGraph::new();
    let edge = g.create_edge(None);
    g.find_best_edge_containment(edge);
}
