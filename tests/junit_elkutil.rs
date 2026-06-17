//! Port of `org.eclipse.elk.core.util.ElkUtilTest`
//! (elk/test/org.eclipse.elk.core.test).

use elkrs::core::elkutil;
use elkrs::core::options_gen::{ContentAlignment, CONTENT_ALIGNMENT};
use elkrs::graph::graph::{ElkGraph, NodeId};
use elkrs::graph::math::KVector;
use elkrs::graph::properties::EnumSet;

/// Java `ElkUtilTest.createContentAlignmentTestGraph`: a graph with one node
/// of dimension (100, 100) and inner behavior of dimension (80, 80).
fn create_content_alignment_test_graph(
    content_alignment: Option<EnumSet<ContentAlignment>>,
) -> (ElkGraph, NodeId, NodeId) {
    let mut g = ElkGraph::new();
    let parent = g.root;
    let node = g.create_node(Some(parent));
    if let Some(ca) = content_alignment {
        g.node_mut(node).properties.set(&CONTENT_ALIGNMENT, ca);
    }
    let inner_behavior = g.create_node(Some(node));
    g.node_mut(node).shape.set_dimensions(100.0, 100.0);
    g.node_mut(inner_behavior).shape.set_dimensions(80.0, 80.0);
    (g, node, inner_behavior)
}

fn assert_translated(
    content_alignment: Option<EnumSet<ContentAlignment>>,
    expected_x: f64,
    expected_y: f64,
) {
    let (mut g, node, inner_behavior) = create_content_alignment_test_graph(content_alignment);
    elkutil::translate_aligned(&mut g, node, KVector::new(120.0, 120.0), KVector::new(100.0, 100.0));
    let shape = &g.node(inner_behavior).shape;
    assert!(
        (shape.x - expected_x).abs() <= 1.0,
        "x: expected {expected_x}, got {}",
        shape.x
    );
    assert!(
        (shape.y - expected_y).abs() <= 1.0,
        "y: expected {expected_y}, got {}",
        shape.y
    );
}

/// Java `ElkUtilTest.translateWithContentAlignmentTest`.
#[test]
fn translate_with_content_alignment_test() {
    use ContentAlignment::*;

    // Test no alignment set
    assert_translated(None, 0.0, 0.0);
    // Test top left (Java `ContentAlignment.topLeft()`)
    assert_translated(Some(EnumSet::of(&[V_TOP, H_LEFT])), 0.0, 0.0);
    // Test top center (Java `ContentAlignment.topCenter()`)
    assert_translated(Some(EnumSet::of(&[V_TOP, H_CENTER])), 10.0, 0.0);
    // Test top right
    assert_translated(Some(EnumSet::of(&[V_TOP, H_RIGHT])), 20.0, 0.0);
    // Test center left
    assert_translated(Some(EnumSet::of(&[V_CENTER, H_LEFT])), 0.0, 10.0);
    // Test center center (Java `ContentAlignment.centerCenter()`)
    assert_translated(Some(EnumSet::of(&[V_CENTER, H_CENTER])), 10.0, 10.0);
    // Test center right
    assert_translated(Some(EnumSet::of(&[V_CENTER, H_RIGHT])), 20.0, 10.0);
    // Test bottom left
    assert_translated(Some(EnumSet::of(&[V_BOTTOM, H_LEFT])), 0.0, 20.0);
    // Test bottom center
    assert_translated(Some(EnumSet::of(&[V_BOTTOM, H_CENTER])), 10.0, 20.0);
    // Test bottom right (Java `ContentAlignment.bottomRight()`)
    assert_translated(Some(EnumSet::of(&[V_BOTTOM, H_RIGHT])), 20.0, 20.0);
}
