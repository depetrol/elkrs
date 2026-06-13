//! Rust port of `org.eclipse.elk.alg.rectpacking.test.PaddingTest`.
//! Four 30x30 nodes packed with a 1000-unit padding on each side in turn;
//! asserts the parent size and exact node positions (Java tolerance 1.0).

use serde_json::{json, Value};

fn layout(padding: &str) -> Value {
    let g = json!({
        "id": "parent",
        "layoutOptions": {
            "org.eclipse.elk.algorithm": "org.eclipse.elk.rectpacking",
            "org.eclipse.elk.aspectRatio": "1.3",
            "org.eclipse.elk.spacing.nodeNode": "10.0",
            "org.eclipse.elk.padding": padding
        },
        "children": [
            {"id":"n1","width":30,"height":30}, {"id":"n2","width":30,"height":30},
            {"id":"n3","width":30,"height":30}, {"id":"n4","width":30,"height":30}
        ]
    });
    elk_cli::create_elk().layout_json(&g.to_string()).expect("layout failed")
}

fn pos(out: &Value, id: &str) -> (f64, f64) {
    let c = out["children"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap();
    (c.get("x").and_then(Value::as_f64).unwrap_or(0.0),
     c.get("y").and_then(Value::as_f64).unwrap_or(0.0))
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 1.0, "{a} != {b}");
}

#[test]
fn test_top_padding() {
    let o = layout("[top=1000.0,left=0.0,bottom=0.0,right=0.0]");
    close(o["width"].as_f64().unwrap(), 150.0);
    close(o["height"].as_f64().unwrap(), 1030.0);
    for (id, ex, ey) in [("n1", 0.0, 1000.0), ("n2", 40.0, 1000.0),
                         ("n3", 80.0, 1000.0), ("n4", 120.0, 1000.0)] {
        let (x, y) = pos(&o, id);
        close(x, ex); close(y, ey);
    }
}

#[test]
fn test_left_padding() {
    let o = layout("[top=0.0,left=1000.0,bottom=0.0,right=0.0]");
    close(o["width"].as_f64().unwrap(), 1030.0);
    close(o["height"].as_f64().unwrap(), 150.0);
    for (id, ex, ey) in [("n1", 1000.0, 0.0), ("n2", 1000.0, 40.0),
                         ("n3", 1000.0, 80.0), ("n4", 1000.0, 120.0)] {
        let (x, y) = pos(&o, id);
        close(x, ex); close(y, ey);
    }
}

#[test]
fn test_bottom_padding() {
    let o = layout("[top=0.0,left=0.0,bottom=1000.0,right=0.0]");
    close(o["width"].as_f64().unwrap(), 150.0);
    close(o["height"].as_f64().unwrap(), 1030.0);
    for (id, ex, ey) in [("n1", 0.0, 0.0), ("n2", 40.0, 0.0),
                         ("n3", 80.0, 0.0), ("n4", 120.0, 0.0)] {
        let (x, y) = pos(&o, id);
        close(x, ex); close(y, ey);
    }
}

#[test]
fn test_right_padding() {
    let o = layout("[top=0.0,left=0.0,bottom=0.0,right=1000.0]");
    close(o["width"].as_f64().unwrap(), 1030.0);
    close(o["height"].as_f64().unwrap(), 150.0);
    for (id, ex, ey) in [("n1", 0.0, 0.0), ("n2", 0.0, 40.0),
                         ("n3", 0.0, 80.0), ("n4", 0.0, 120.0)] {
        let (x, y) = pos(&o, id);
        close(x, ex); close(y, ey);
    }
}
