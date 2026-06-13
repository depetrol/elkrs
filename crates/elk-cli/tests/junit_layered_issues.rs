//! Rust ports of ELK layered regression tests:
//! `Issue562Test` (inside self loops must not throw) and
//! `Issue680Test` (recursive hierarchy with external ports — exercises the
//! ComponentGroupGraphPlacer path).

use serde_json::{json, Value};

const EPS: f64 = 1e-5;

fn layout(g: Value) -> Value {
    elk_cli::create_elk().layout_json(&g.to_string()).expect("layout must not fail")
}

/// `Issue562Test`: a node with two ports and a self edge, with inside-self-loops
/// activated, must lay out without raising an unsupported-configuration error.
#[test]
fn issue562_inside_self_loops() {
    let g = json!({
        "id": "g",
        "layoutOptions": {"org.eclipse.elk.algorithm": "org.eclipse.elk.layered"},
        "children": [{
            "id": "n1", "width": 40, "height": 40,
            "layoutOptions": {"org.eclipse.elk.insideSelfLoops.activate": "true"},
            "ports": [{"id": "p1", "width": 4, "height": 4}, {"id": "p2", "width": 4, "height": 4}]
        }],
        "edges": [{
            "id": "e", "sources": ["p1"], "targets": ["p2"],
            "layoutOptions": {"org.eclipse.elk.insideSelfLoops.yo": "true"}
        }]
    });
    // success (no panic / Err) is the assertion
    let _ = layout(g);
}

/// `Issue680Test`: a parent node (laid out by layered) owns two external ports
/// and a single child; recursive hierarchical layout must place the parent at
/// y=157 and the child at y=57. This reaches the components processor's
/// external-port (ComponentGroup) placement path.
#[test]
fn issue680_external_ports() {
    let opts = json!({
        "org.eclipse.elk.algorithm": "org.eclipse.elk.layered",
        "org.eclipse.elk.edgeRouting": "ORTHOGONAL",
        "org.eclipse.elk.direction": "DOWN"
    });
    let g = json!({
        "id": "graph",
        "layoutOptions": opts,
        "children": [{
            "id": "parent",
            "layoutOptions": opts,
            "ports": [
                {"id": "p1", "width": 15, "height": 165, "layoutOptions": {"org.eclipse.elk.port.borderOffset": "-20.0"}},
                {"id": "p2", "width": 15, "height": 166, "layoutOptions": {"org.eclipse.elk.port.borderOffset": "-22.0"}}
            ],
            "children": [{
                "id": "child", "width": 40.265625, "height": 75.5,
                "ports": [
                    {"id": "childP1", "width": 15, "height": 33, "layoutOptions": {"org.eclipse.elk.port.borderOffset": "-8.0"}},
                    {"id": "childP2", "width": 15, "height": 34, "layoutOptions": {"org.eclipse.elk.port.borderOffset": "-8.0"}}
                ]
            }],
            "edges": [
                {"id": "e1", "sources": ["p1"], "targets": ["childP1"]},
                {"id": "e2", "sources": ["childP2"], "targets": ["p2"]}
            ]
        }]
    });
    let o = layout(g);
    let parent = &o["children"][0];
    let child = &parent["children"][0];
    let py = parent["y"].as_f64().unwrap();
    let cy = child["y"].as_f64().unwrap();
    assert!((py - 157.0).abs() < EPS, "parent.y = {py}, expected 157.0");
    assert!((cy - 57.0).abs() < EPS, "child.y = {cy}, expected 57.0");
}
