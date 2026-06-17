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
    elkrs::create_elk().layout_json(&g.to_string()).expect("layout failed")
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

// ---- Rust port of `CompactionTest` (targetWidth width-approximation) ----

/// Lay out `dims` (id,w,h) with the TARGET_WIDTH strategy and given target
/// width + node spacing; returns the parsed output graph.
fn compact(target_width: f64, spacing: f64, dims: &[(u32, f64, f64)]) -> Value {
    let children: Vec<Value> = dims.iter().map(|(i, w, h)| {
        json!({"id": format!("n{i}"), "width": w, "height": h})
    }).collect();
    let g = json!({
        "id": "parent",
        "layoutOptions": {
            "org.eclipse.elk.algorithm": "org.eclipse.elk.rectpacking",
            "org.eclipse.elk.rectpacking.widthApproximation.targetWidth": target_width.to_string(),
            "org.eclipse.elk.rectpacking.widthApproximation.strategy": "TARGET_WIDTH",
            "org.eclipse.elk.spacing.nodeNode": spacing.to_string(),
            "org.eclipse.elk.padding": "[top=0.0,left=0.0,bottom=0.0,right=0.0]"
        },
        "children": children
    });
    elkrs::create_elk().layout_json(&g.to_string()).expect("layout failed")
}

/// Assert parent size and the given (id,x,y) node positions.
fn check(o: &Value, pw: f64, ph: f64, expect: &[(u32, f64, f64)]) {
    close(o["width"].as_f64().unwrap(), pw);
    close(o["height"].as_f64().unwrap(), ph);
    for (i, ex, ey) in expect {
        let (x, y) = pos(o, &format!("n{i}"));
        close(x, *ex); close(y, *ey);
    }
}

#[test]
fn test_place_block_from_next_row_on_top() {
    let o = compact(110.0, 10.0, &[(1,30.,70.),(2,30.,10.),(3,30.,10.),(4,30.,50.),(5,30.,50.),(6,100.,10.),(7,100.,10.)]);
    check(&o, 110.0, 110.0, &[(1,0.,0.),(2,40.,0.),(3,80.,0.),(4,40.,20.),(5,80.,20.),(6,0.,80.)]);
}

#[test]
fn test_place_block_from_next_row_on_top_does_not_work() {
    let o = compact(110.0, 10.0, &[(1,30.,70.),(2,40.,10.),(3,30.,10.),(4,30.,50.),(5,30.,50.),(6,100.,10.),(7,100.,10.)]);
    check(&o, 100.0, 170.0, &[(1,0.,0.),(2,40.,0.),(3,40.,20.),(4,0.,80.),(5,40.,80.)]);
}

#[test]
fn test_absorb_block() {
    let o = compact(110.0, 10.0, &[(1,30.,70.),(2,30.,30.),(3,30.,30.),(4,30.,30.),(5,30.,30.),(6,100.,10.),(7,100.,10.)]);
    check(&o, 110.0, 110.0, &[(1,0.,0.),(2,40.,0.),(3,80.,0.),(4,40.,40.),(5,80.,40.),(6,0.,80.)]);
}

#[test]
fn test_absorb_block_in_stack() {
    let mut dims = vec![(1u32, 178., 162.)];
    for i in 2..=9 { dims.push((i, 75., 23.)); }
    for i in 10..=19 { dims.push((i, 47., 23.)); }
    let o = compact(330.0, 1.0, &dims);
    check(&o, 178. + 2. + 2.*75., 162. + 1. + 23., &[]);
}

#[test]
fn test_compact_block() {
    let o = compact(190.0, 10.0, &[(1,30.,70.),(2,30.,30.),(3,30.,30.),(4,30.,30.),(5,30.,30.)]);
    check(&o, 110.0, 70.0, &[(1,0.,0.),(2,40.,0.),(3,80.,0.),(4,40.,40.),(5,80.,40.)]);
}

#[test]
fn test_split_block() {
    let o = compact(110.0, 10.0, &[(1,30.,70.),(2,30.,30.),(3,30.,30.),(4,30.,30.),(5,30.,30.),(6,30.,30.),(7,100.,10.)]);
    check(&o, 110.0, 130.0, &[(1,0.,0.),(2,40.,0.),(3,80.,0.),(4,40.,40.),(5,80.,40.),(6,0.,80.),(7,0.,120.)]);
}

#[test]
fn test_place_block_from_next_row_right() {
    let o = compact(110.0, 10.0, &[(1,30.,70.),(2,30.,30.),(3,20.,20.),(4,30.,70.)]);
    check(&o, 110.0, 70.0, &[(1,0.,0.),(2,40.,0.),(3,40.,40.),(4,80.,0.)]);
}

#[test]
fn test_place_block_from_current_row_right() {
    let o = compact(150.0, 10.0, &[(1,30.,70.),(2,30.,30.),(3,20.,20.),(4,30.,70.)]);
    check(&o, 110.0, 70.0, &[(1,0.,0.),(2,40.,0.),(3,40.,40.),(4,80.,0.)]);
}

#[test]
fn test_place_block_from_current_row_on_top() {
    let o = compact(150.0, 10.0, &[(1,30.,70.),(2,30.,10.),(3,30.,10.),(4,30.,50.)]);
    check(&o, 110.0, 70.0, &[(1,0.,0.),(2,40.,0.),(3,80.,0.),(4,40.,20.)]);
}

#[test]
fn test_place_stack_next_to_multiple_block_stack() {
    let o = compact(135.0, 5.0, &[(1,30.,100.),(2,30.,10.),(3,30.,10.),(4,30.,85.),(5,30.,85.),(6,30.,30.)]);
    check(&o, 135.0, 100.0, &[(1,0.,0.),(2,35.,0.),(3,70.,0.),(4,35.,15.),(5,70.,15.),(6,105.,0.)]);
}
