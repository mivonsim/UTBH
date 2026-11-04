//! Sinkronisasi `schemas/trace/trace.schema.json` dengan tipe `utbh-trace`.
//!
//! Menjaga agar field yang di-serialize tidak pernah lepas dari schema yang
//! dipublikasikan (AGENTS.md aturan #8).

use utbh_trace::{Trace, TraceEvent};

fn schema() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../schemas/trace/trace.schema.json"))
        .expect("schema trace valid JSON")
}

fn assert_same_fields(schema: &serde_json::Value, value: &serde_json::Value, label: &str) {
    let props = schema["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{}: properties hilang", label));
    let keys = value
        .as_object()
        .unwrap_or_else(|| panic!("{}: bukan object", label));

    for k in props.keys() {
        assert!(
            keys.contains_key(k),
            "{}: tipe tidak punya field '{}'",
            label,
            k
        );
    }
    for k in keys.keys() {
        assert!(
            props.contains_key(k),
            "{}: field '{}' tidak ada di schema",
            label,
            k
        );
    }
    for required in schema["required"].as_array().into_iter().flatten() {
        let name = required.as_str().unwrap();
        assert!(
            keys.contains_key(name),
            "{}: required '{}' hilang dari tipe",
            label,
            name
        );
    }
}

#[test]
fn trace_schema_matches_types() {
    let schema = schema();
    let trace = Trace {
        run_id: "run-1".into(),
        test: "vector_add".into(),
        category: "cpu.vector".into(),
        events: vec![TraceEvent {
            cycle: 42,
            kind: "validation".into(),
            detail: "mismatch".into(),
        }],
    };
    let json = serde_json::to_value(&trace).unwrap();
    assert_same_fields(&schema, &json, "trace");

    let event_schema = &schema["properties"]["events"]["items"];
    assert_same_fields(event_schema, &json["events"][0], "trace.event");
}
