//! Rust port of `org.eclipse.elk.alg.mrtree.test.MrTreeGraphSizeTest`
//! (parameterized over three node-size/padding/spacing tuples).

use serde_json::{json, Value};

const EPS: f64 = 10e-5;

/// (nodeWidth, nodeHeight, padLeft, padRight, padTop, padBottom, nodeNodeSpacing)
const PARAMS: [(f64, f64, f64, f64, f64, f64, f64); 3] = [
    (20., 20., 10., 10., 10., 10., 10.),
    (15., 30., 7., 9., 11., 13., 15.),
    (25., 15., 0., 0., 0., 0., 20.),
];

fn layout(g: Value) -> Value {
    elkrs::create_elk().layout_json(&g.to_string()).expect("layout failed")
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= EPS, "{a} != {b}");
}

#[test]
fn graph_size_calculation_test() {
    for (w, h, pl, pr, pt, pb, sp) in PARAMS {
        let g = json!({
            "id": "g",
            "layoutOptions": {
                "org.eclipse.elk.algorithm": "org.eclipse.elk.mrtree",
                "org.eclipse.elk.padding": format!("[top={pt},left={pl},bottom={pb},right={pr}]"),
                "org.eclipse.elk.spacing.nodeNode": sp.to_string()
            },
            "children": [
                {"id":"n1","width":w,"height":h},
                {"id":"n2","width":w,"height":h},
                {"id":"n3","width":w,"height":h}
            ],
            "edges": [
                {"id":"e1","sources":["n1"],"targets":["n2"]},
                {"id":"e2","sources":["n1"],"targets":["n3"]}
            ]
        });
        let o = layout(g);
        close(o["width"].as_f64().unwrap(), pl + w + sp + w + pr);
        close(o["height"].as_f64().unwrap(), pt + h + sp + h + pb);
    }
}

#[test]
fn components_graph_size_calculation_test() {
    for (w, h, ..) in PARAMS {
        let g = json!({
            "id": "g",
            "layoutOptions": {
                "org.eclipse.elk.algorithm": "org.eclipse.elk.mrtree",
                "org.eclipse.elk.padding": "[top=0.0,left=0.0,bottom=0.0,right=0.0]",
                "org.eclipse.elk.spacing.nodeNode": "0.0",
                "org.eclipse.elk.aspectRatio": "1000.0"
            },
            "children": [
                {"id":"n1","width":w,"height":h},
                {"id":"n2","width":w,"height":h}
            ]
        });
        let o = layout(g);
        close(o["width"].as_f64().unwrap(), w + w);
        close(o["height"].as_f64().unwrap(), h);
    }
}
