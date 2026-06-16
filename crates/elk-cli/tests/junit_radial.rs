//! Rust port of `org.eclipse.elk.alg.radial.test.CenterOnRootTest`.
//! Builds the same graphs, runs the radial algorithm through the registered
//! ELK instance, and asserts the root node is centered in the parent.

use serde_json::{json, Value};

fn layout(input: Value) -> Value {
    let elk = elkrs::create_elk();
    elk.layout_json(&input.to_string()).expect("layout failed")
}

fn child<'a>(out: &'a Value, id: &str) -> &'a Value {
    out["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no child {id}"))
}

fn f(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

/// Margin (left/top) parsed from the exported `org.eclipse.elk.margins` option.
fn margin(node: &Value, which: &str) -> f64 {
    let m = node
        .get("layoutOptions")
        .and_then(|o| o.get("org.eclipse.elk.margins"))
        .and_then(Value::as_str)
        .unwrap_or("[top=0.0,left=0.0,bottom=0.0,right=0.0]");
    // form: [top=..,left=..,bottom=..,right=..]
    for part in m.trim_matches(|c| c == '[' || c == ']').split(',') {
        let mut kv = part.split('=');
        if kv.next().map(str::trim) == Some(which) {
            return kv.next().unwrap().trim().parse().unwrap();
        }
    }
    0.0
}

fn assert_centered(out: &Value, root_id: &str) {
    let root = child(out, root_id);
    let pw = f(out, "width");
    let ph = f(out, "height");
    let cx = f(root, "x") + margin(root, "left") + f(root, "width") / 2.0;
    let cy = f(root, "y") + margin(root, "top") + f(root, "height") / 2.0;
    assert!((pw / 2.0 - cx).abs() < 0.1, "horizontal centering: {} vs {}", pw / 2.0, cx);
    assert!((ph / 2.0 - cy).abs() < 0.1, "vertical centering: {} vs {}", ph / 2.0, cy);
}

#[test]
fn test_simple_centering() {
    let g = json!({
        "id": "parent",
        "layoutOptions": { "org.eclipse.elk.algorithm": "org.eclipse.elk.radial",
                           "org.eclipse.elk.radial.centerOnRoot": "true" },
        "children": [ {"id":"root"}, {"id":"n1"}, {"id":"n2"}, {"id":"n3"} ],
        "edges": [
            {"id":"e1","sources":["root"],"targets":["n1"]},
            {"id":"e2","sources":["root"],"targets":["n2"]},
            {"id":"e3","sources":["root"],"targets":["n3"]}
        ]
    });
    assert_centered(&layout(g), "root");
}

#[test]
fn test_larger_graph_centering() {
    let g = json!({
        "id": "parent",
        "layoutOptions": { "org.eclipse.elk.algorithm": "org.eclipse.elk.radial",
                           "org.eclipse.elk.radial.centerOnRoot": "true" },
        "children": [ {"id":"root"}, {"id":"n1"}, {"id":"n2"}, {"id":"n3"},
                      {"id":"n11"}, {"id":"n12"}, {"id":"n13"} ],
        "edges": [
            {"id":"e1","sources":["root"],"targets":["n1"]},
            {"id":"e2","sources":["root"],"targets":["n2"]},
            {"id":"e3","sources":["root"],"targets":["n3"]},
            {"id":"e11","sources":["n1"],"targets":["n11"]},
            {"id":"e12","sources":["n1"],"targets":["n12"]},
            {"id":"e13","sources":["n1"],"targets":["n13"]}
        ]
    });
    assert_centered(&layout(g), "root");
}
