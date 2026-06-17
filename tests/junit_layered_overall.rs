//! Rust port of `org.eclipse.elk.alg.layered.OverallLayoutTest`: a simple
//! two-node, one-edge graph must lay out with positive coordinates, a positive
//! graph size, and orthogonal edge routing.

use serde_json::{json, Value};

fn simple_graph() -> Value {
    let g = json!({
        "id": "graph",
        "layoutOptions": {"org.eclipse.elk.algorithm": "org.eclipse.elk.layered"},
        "children": [
            {"id": "node1", "width": 30, "height": 30,
             "layoutOptions": {"org.eclipse.elk.nodeSize.constraints": "[]"}},
            {"id": "node2", "width": 30, "height": 30,
             "layoutOptions": {"org.eclipse.elk.nodeSize.constraints": "[]"}}
        ],
        "edges": [{"id": "e1", "sources": ["node1"], "targets": ["node2"]}]
    });
    elkrs::create_elk().layout_json(&g.to_string()).expect("layout failed")
}

/// All section points in document order: startPoint, bendPoints..., endPoint.
fn section_points(section: &Value) -> Vec<(f64, f64)> {
    let pt = |v: &Value| (v["x"].as_f64().unwrap(), v["y"].as_f64().unwrap());
    let mut pts = vec![pt(&section["startPoint"])];
    if let Some(bps) = section["bendPoints"].as_array() {
        pts.extend(bps.iter().map(pt));
    }
    pts.push(pt(&section["endPoint"]));
    pts
}

fn sections(o: &Value) -> Vec<Value> {
    o["edges"].as_array().unwrap().iter()
        .flat_map(|e| e["sections"].as_array().unwrap().clone())
        .collect()
}

#[test]
fn test_node_coordinates() {
    let o = simple_graph();
    for c in o["children"].as_array().unwrap() {
        assert!(c["x"].as_f64().unwrap() > 0.0);
        assert!(c["y"].as_f64().unwrap() > 0.0);
    }
}

#[test]
fn test_edge_coordinates() {
    let o = simple_graph();
    for s in sections(&o) {
        for key in ["startPoint", "endPoint"] {
            assert!(s[key]["x"].as_f64().unwrap() > 0.0);
            assert!(s[key]["y"].as_f64().unwrap() > 0.0);
        }
    }
}

#[test]
fn test_graph_size() {
    let o = simple_graph();
    assert!(o["width"].as_f64().unwrap() > 0.0);
    assert!(o["height"].as_f64().unwrap() > 0.0);
}

#[test]
fn test_edge_orthogonality() {
    let o = simple_graph();
    for s in sections(&o) {
        let pts = section_points(&s);
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            assert!(a.0 == b.0 || a.1 == b.1, "non-orthogonal segment {a:?} -> {b:?}");
        }
    }
}
