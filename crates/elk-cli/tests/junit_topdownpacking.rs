//! Rust port of `org.eclipse.elk.alg.topdown.test.TopdownPackingTest`.
//! Runs the topdownpacking algorithm standalone and checks child placement
//! against the same closed-form expressions ELK's test uses. Default property
//! values match CoreOptions: hierarchicalWidth=150, aspectRatio=1.414,
//! padding=12 each side, nodeNodeSpacing=20.

use serde_json::{json, Value};

const W: f64 = 150.0; // TOPDOWN_HIERARCHICAL_NODE_WIDTH
const AR: f64 = 1.414; // TOPDOWN_HIERARCHICAL_NODE_ASPECT_RATIO
const PAD: f64 = 12.0; // default ElkPadding
const SP: f64 = 20.0; // SPACING_NODE_NODE
const EPS: f64 = 0.00001;

fn layout(n: usize) -> Vec<Value> {
    let children: Vec<Value> = (0..n).map(|i| json!({"id": format!("n{i}")})).collect();
    let g = json!({
        "id": "g",
        "layoutOptions": {"org.eclipse.elk.algorithm": "org.eclipse.elk.topdownpacking"},
        "children": children
    });
    let o = elkrs::create_elk().layout_json(&g.to_string()).expect("layout failed");
    // an empty graph is exported without a "children" key
    o["children"].as_array().cloned().unwrap_or_default()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= EPS, "{a} != {b}");
}

/// Assert child `idx` has the given x, y, width, height.
fn check(c: &[Value], idx: usize, x: f64, y: f64, w: f64, h: f64) {
    let n = &c[idx];
    close(n["x"].as_f64().unwrap(), x);
    close(n["y"].as_f64().unwrap(), y);
    close(n["width"].as_f64().unwrap(), w);
    close(n["height"].as_f64().unwrap(), h);
}

#[test]
fn test_empty_graph() {
    let _ = layout(0); // must not crash
}

#[test]
fn test_two_nodes() {
    let h = W / AR;
    let c = layout(2);
    check(&c, 0, PAD, PAD, W, h);
    check(&c, 1, PAD + W + SP, PAD, W, h);
}

#[test]
fn test_three_nodes() {
    let h = W / AR;
    let c = layout(3);
    check(&c, 0, PAD, PAD, W, h);
    check(&c, 1, PAD + W + SP, PAD, W, h);
    // third node expands to fill the bottom row
    check(&c, 2, PAD, PAD + h + SP, 2.0 * W + SP, h);
}

#[test]
fn test_five_nodes() {
    let h = W / AR;
    let expanded = W + 0.5 * (W + SP); // whitespace eliminated across two cells
    let c = layout(5);
    check(&c, 3, PAD, PAD + h + SP, expanded, h);
    check(&c, 4, PAD + expanded + SP, PAD + h + SP, expanded, h);
}
