//! Rust ports of ELK layered regression tests:
//! `Issue562Test` (inside self loops must not throw) and
//! `Issue680Test` (recursive hierarchy with external ports — exercises the
//! ComponentGroupGraphPlacer path).

use serde_json::{json, Value};

const EPS: f64 = 1e-5;

fn layout(g: Value) -> Value {
    elkrs::create_elk().layout_json(&g.to_string()).expect("layout must not fail")
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

// ---- Issue871Test: model-order layout with crossingMinimization = NONE ----

fn model_order_opts() -> Value {
    json!({
        "org.eclipse.elk.algorithm": "org.eclipse.elk.layered",
        "org.eclipse.elk.direction": "RIGHT",
        "org.eclipse.elk.layered.cycleBreaking.strategy": "MODEL_ORDER",
        "org.eclipse.elk.layered.considerModelOrder.strategy": "PREFER_EDGES",
        "org.eclipse.elk.layered.crossingMinimization.strategy": "NONE",
        "org.eclipse.elk.layered.crossingMinimization.greedySwitch.type": "OFF",
        "org.eclipse.elk.padding": "[top=0.0,left=0.0,bottom=0.0,right=0.0]",
        "org.eclipse.elk.spacing.nodeNode": "10.0",
        "org.eclipse.elk.layered.spacing.nodeNodeBetweenLayers": "20.0"
    })
}

fn node30(id: &str) -> Value {
    json!({"id": id, "width": 30, "height": 30})
}

fn y_of<'a>(o: &'a Value, id: &str) -> f64 {
    o["children"].as_array().unwrap().iter()
        .find(|c| c["id"] == id).unwrap()["y"].as_f64().unwrap()
}

#[test]
fn issue871_feedback_edge_basic() {
    let mut opts = model_order_opts();
    opts["org.eclipse.elk.layered.feedbackEdges"] = json!("true");
    let g = json!({
        "id": "parent", "layoutOptions": opts,
        "children": [node30("n1"), node30("n2"), node30("n3")],
        "edges": [
            {"id": "e1", "sources": ["n1"], "targets": ["n2"]},
            {"id": "e2", "sources": ["n2"], "targets": ["n3"]},
            {"id": "e3", "sources": ["n3"], "targets": ["n2"]}
        ]
    });
    let o = layout(g);
    // n3 should align with n2
    assert!((y_of(&o, "n3") - y_of(&o, "n2")).abs() < 0.1);
}

#[test]
fn issue871_feedback_edge_below() {
    let mut opts = model_order_opts();
    opts["org.eclipse.elk.layered.feedbackEdges"] = json!("true");
    let g = json!({
        "id": "parent", "layoutOptions": opts,
        "children": [node30("n1"), node30("n2"), node30("n3"), node30("n4")],
        "edges": [
            {"id": "e1", "sources": ["n1"], "targets": ["n2"]},
            {"id": "e2", "sources": ["n1"], "targets": ["n3"]},
            {"id": "e3", "sources": ["n2"], "targets": ["n4"]},
            {"id": "e4", "sources": ["n4"], "targets": ["n3"]}
        ]
    });
    let o = layout(g);
    // n4 should align with n2
    assert!((y_of(&o, "n4") - y_of(&o, "n2")).abs() < 0.1);
}

#[test]
fn issue871_no_feedback_edges_still_working() {
    let g = json!({
        "id": "parent", "layoutOptions": model_order_opts(),
        "children": [
            {"id": "n1", "width": 30, "height": 30, "labels": [{"text": "n1"}]},
            {"id": "n2", "width": 30, "height": 30, "labels": [{"text": "n2"}]},
            {"id": "n3", "width": 30, "height": 30, "labels": [{"text": "n3"}]},
            {"id": "n4", "width": 30, "height": 30, "labels": [{"text": "n4"}]}
        ],
        "edges": [
            {"id": "e1", "sources": ["n1"], "targets": ["n2"], "labels": [{"text": "1"}]},
            {"id": "e2", "sources": ["n1"], "targets": ["n4"], "labels": [{"text": "2"}]},
            {"id": "e3", "sources": ["n2"], "targets": ["n4"]},
            {"id": "e4", "sources": ["n3"], "targets": ["n2"]},
            {"id": "e5", "sources": ["n3"], "targets": ["n4"]}
        ]
    });
    let o = layout(g);
    let x = |id| o["children"].as_array().unwrap().iter()
        .find(|c| c["id"] == id).unwrap()["x"].as_f64().unwrap();
    for (id, ex, ey) in [("n1", 0.0, 31.0), ("n2", 70.0, 6.0), ("n3", 120.0, 11.0), ("n4", 170.0, 11.0)] {
        assert!((x(id) - ex).abs() < 0.1, "{id}.x = {}, expected {ex}", x(id));
        assert!((y_of(&o, id) - ey).abs() < 0.1, "{id}.y = {}, expected {ey}", y_of(&o, id));
    }
}

/// `Issue682Test` (parameterized over all four directions): a single node with
/// an inside-top-center label and NODE_LABELS size constraint. The label sits
/// at (54, 21) and the node grows to width 54 + 23 + 32 = 109, regardless of
/// layout direction.
#[test]
fn issue682_node_label_placement() {
    for dir in ["RIGHT", "DOWN", "UP", "LEFT"] {
        let g = json!({
            "id": "graph",
            "layoutOptions": {
                "org.eclipse.elk.algorithm": "org.eclipse.elk.layered",
                "org.eclipse.elk.edgeRouting": "ORTHOGONAL",
                "org.eclipse.elk.direction": dir,
                "org.eclipse.elk.nodeLabels.padding": "[top=21.0,left=54.0,bottom=43.0,right=32.0]"
            },
            "children": [{
                "id": "parent",
                "layoutOptions": {
                    "org.eclipse.elk.nodeSize.constraints": "NODE_LABELS",
                    "org.eclipse.elk.nodeLabels.placement": "[H_CENTER, V_TOP, INSIDE]"
                },
                "labels": [{"text": "foobar", "width": 23, "height": 22}]
            }]
        });
        let o = layout(g);
        let parent = &o["children"][0];
        let label = &parent["labels"][0];
        let lx = label["x"].as_f64().unwrap();
        let ly = label["y"].as_f64().unwrap();
        assert!((lx - 54.0).abs() < EPS, "[{dir}] label.x = {lx}");
        assert!((ly - 21.0).abs() < EPS, "[{dir}] label.y = {ly}");
        // node width = label.x + label.width + right padding (32)
        let w = parent["width"].as_f64().unwrap();
        assert!((w - (lx + 23.0 + 32.0)).abs() < EPS, "[{dir}] node.width = {w}");
    }
}
