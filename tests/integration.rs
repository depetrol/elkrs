//! End-to-end tests: registration, suffix resolution, and full layout runs
//! through the recursive layout engine and the JSON pipeline.

fn elk() -> elkrs::core::Elk {
    let mut elk = elkrs::core::Elk::new();
    elkrs::alg_force::register(&mut elk.options, &mut elk.algorithms);
    elk
}

#[test]
fn algorithms_and_options_are_registered() {
    let elk = elk();
    assert_eq!(elk.algorithms.by_suffix("force").unwrap().id, "org.eclipse.elk.force");
    assert_eq!(elk.algorithms.by_suffix("stress").unwrap().id, "org.eclipse.elk.stress");
    for suffix in [
        "force.model",
        "force.iterations",
        "force.repulsivePower",
        "force.temperature",
        "force.repulsion",
        "stress.fixed",
        "stress.desiredEdgeLength",
        "stress.dimension",
        "stress.epsilon",
        "stress.iterationLimit",
    ] {
        let opt = elk.options.option_by_suffix(suffix);
        assert!(opt.is_some(), "option {suffix} not registered");
    }
    // enum values parse like Java's enumForString
    let model = elk.options.option_by_suffix("force.model").unwrap();
    assert_eq!(model.parse_value("EADES").unwrap().to_java_string(), "EADES");
    let dim = elk.options.option_by_suffix("stress.dimension").unwrap();
    assert_eq!(dim.parse_value("XY").unwrap().to_java_string(), "XY");
}

fn graph_json(algorithm: &str) -> String {
    format!(
        r#"{{
          "id": "root",
          "layoutOptions": {{ "elk.algorithm": "{algorithm}" }},
          "children": [
            {{ "id": "n1", "width": 30, "height": 30 }},
            {{ "id": "n2", "width": 30, "height": 30 }},
            {{ "id": "n3", "width": 30, "height": 30 }}
          ],
          "edges": [
            {{ "id": "e1", "sources": ["n1"], "targets": ["n2"] }},
            {{ "id": "e2", "sources": ["n2"], "targets": ["n3"] }}
          ]
        }}"#
    )
}

#[test]
fn force_layout_runs_and_is_deterministic() {
    let elk = elk();
    let out1 = elk.layout_json(&graph_json("force")).expect("force layout failed");
    let out2 = elk.layout_json(&graph_json("force")).expect("force layout failed");
    assert_eq!(out1, out2);
    // every node received finite coordinates
    for child in out1["children"].as_array().unwrap() {
        assert!(child["x"].as_f64().unwrap().is_finite());
        assert!(child["y"].as_f64().unwrap().is_finite());
    }
    // edges got routed (sections with start/end points)
    for edge in out1["edges"].as_array().unwrap() {
        let section = &edge["sections"][0];
        assert!(section["startPoint"]["x"].as_f64().unwrap().is_finite());
        assert!(section["endPoint"]["x"].as_f64().unwrap().is_finite());
    }
}

#[test]
fn stress_layout_runs_and_is_deterministic() {
    let elk = elk();
    let out1 = elk.layout_json(&graph_json("stress")).expect("stress layout failed");
    let out2 = elk.layout_json(&graph_json("stress")).expect("stress layout failed");
    assert_eq!(out1, out2);
    for child in out1["children"].as_array().unwrap() {
        assert!(child["x"].as_f64().unwrap().is_finite());
        assert!(child["y"].as_f64().unwrap().is_finite());
    }
}

#[test]
fn force_components_are_packed() {
    // two disconnected pairs => ComponentsProcessor split/recombine path
    let elk = elk();
    let json = r#"{
      "id": "root",
      "layoutOptions": { "elk.algorithm": "force", "org.eclipse.elk.force.model": "EADES" },
      "children": [
        { "id": "a1", "width": 20, "height": 20 },
        { "id": "a2", "width": 20, "height": 20 },
        { "id": "b1", "width": 20, "height": 20 },
        { "id": "b2", "width": 20, "height": 20 }
      ],
      "edges": [
        { "id": "ea", "sources": ["a1"], "targets": ["a2"] },
        { "id": "eb", "sources": ["b1"], "targets": ["b2"] }
      ]
    }"#;
    let out = elk.layout_json(json).expect("force layout failed");
    let children = out["children"].as_array().unwrap();
    assert_eq!(children.len(), 4);
    for child in children {
        assert!(child["x"].as_f64().unwrap().is_finite());
        assert!(child["y"].as_f64().unwrap().is_finite());
    }
}
