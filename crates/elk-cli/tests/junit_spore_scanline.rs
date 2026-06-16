//! Rust port of `org.eclipse.elk.alg.spore.test.ScanlineOverlapRemovalTest`.
//! Runs spore overlap removal with the scanline pass on and off; the on-pass
//! must remove all overlaps, the off-pass must leave some.

use serde_json::{json, Value};

fn base_graph(run_scanline: bool) -> Value {
    json!({
        "id": "g",
        "layoutOptions": {
            "org.eclipse.elk.algorithm": "org.eclipse.elk.sporeOverlap",
            "org.eclipse.elk.overlapRemoval.runScanline": run_scanline.to_string()
        },
        "children": [
            {"id":"n0","width":160,"height":20,"x":0,"y":30},
            {"id":"n1","width":160,"height":20,"x":150,"y":40},
            {"id":"n2","width":20,"height":20,"x":150,"y":0},
            {"id":"n3","width":20,"height":20,"x":150,"y":70}
        ]
    })
}

/// Mirror of `ElkMath.shortestDistance` for axis-aligned rectangles.
fn shortest_distance(a: &Value, b: &Value) -> f64 {
    let (ax, ay, aw, ah) = rect(a);
    let (bx, by, bw, bh) = rect(b);
    // horizontal/vertical gaps (negative when projections overlap)
    let dx = f64::max(bx - (ax + aw), ax - (bx + bw));
    let dy = f64::max(by - (ay + ah), ay - (by + bh));
    if dx >= 0.0 && dy >= 0.0 {
        (dx * dx + dy * dy).sqrt()
    } else if dx >= 0.0 {
        dx
    } else if dy >= 0.0 {
        dy
    } else {
        f64::max(dx, dy) // both negative: penetration depth (negative)
    }
}

fn rect(n: &Value) -> (f64, f64, f64, f64) {
    (
        n.get("x").and_then(Value::as_f64).unwrap_or(0.0),
        n.get("y").and_then(Value::as_f64).unwrap_or(0.0),
        n["width"].as_f64().unwrap(),
        n["height"].as_f64().unwrap(),
    )
}

fn has_overlaps(out: &Value) -> bool {
    let ch = out["children"].as_array().unwrap();
    for (i, a) in ch.iter().enumerate() {
        for (j, b) in ch.iter().enumerate() {
            if i != j && shortest_distance(a, b) < -1e-4 {
                return true;
            }
        }
    }
    false
}

fn layout(run_scanline: bool) -> Value {
    elkrs::create_elk()
        .layout_json(&base_graph(run_scanline).to_string())
        .expect("layout failed")
}

#[test]
fn scanline_test() {
    assert!(!has_overlaps(&layout(true)), "scanline pass should remove all overlaps");
    assert!(has_overlaps(&layout(false)), "without scanline, overlaps should remain");
}
