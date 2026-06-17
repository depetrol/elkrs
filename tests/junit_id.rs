//! Port of `org.eclipse.elk.graph.json.test.IdTest`
//! (elk/test/org.eclipse.elk.graph.json.test).
//!
//! Java parses with Gson in lenient mode, so the test sources use JSON5-ish
//! syntax (`{ id: 'foo' }`). The Rust port consumes `serde_json::Value`s, so
//! the equivalent strict-JSON documents are used; the import behavior under
//! test (id presence and type validation, Java `JsonImportException`) is the
//! same.

use elkrs::core::json::JsonImporter;
use elkrs::core::Elk;
use serde_json::json;

fn import(value: serde_json::Value) -> Result<(), String> {
    let elk = Elk::new();
    let mut importer = JsonImporter::new(&elk.options);
    importer.import_graph(&value).map(|_| ())
}

/// Java `IdTest.testNoId` (expects `JsonImportException`).
#[test]
fn test_no_id() {
    assert!(import(json!({})).is_err());
}

/// Java `IdTest.testWrongIdTypeNumber` (expects `JsonImportException`).
#[test]
fn test_wrong_id_type_number() {
    assert!(import(json!({ "id": 1.2 })).is_err());
}

/// Java `IdTest.testWrongIdTypeObject` (expects `JsonImportException`).
#[test]
fn test_wrong_id_type_object() {
    assert!(import(json!({ "id": {} })).is_err());
}

/// Java `IdTest.testWrongIdTypeArray` (expects `JsonImportException`).
#[test]
fn test_wrong_id_type_array() {
    assert!(import(json!({ "id": [] })).is_err());
}

/// Java `IdTest.testWrongIdTypeBoolean` (expects `JsonImportException`).
#[test]
fn test_wrong_id_type_boolean() {
    assert!(import(json!({ "id": true })).is_err());
}

/// Java `IdTest.testGoodIdString`.
#[test]
fn test_good_id_string() {
    import(json!({ "id": "foo" })).unwrap();
}

/// Java `IdTest.testGoodIdInt`.
#[test]
fn test_good_id_int() {
    import(json!({ "id": 3 })).unwrap();
}
