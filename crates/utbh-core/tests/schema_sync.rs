//! Sinkronisasi `schemas/*.json` ↔ tipe `utbh-core` (AGENTS.md aturan #8).
//!
//! Cek bahwa field yang di-serialize tipe core persis terdaftar di schema
//! (required + properties). Kalau ada field baru tanpa update schema → gagal.

use utbh_core::{HardwareReport, RunResult, TestDef, SCHEMA_VERSION};

fn schema_props(schema: &str, path: &str) -> (Vec<String>, Vec<String>) {
    let v: serde_json::Value = serde_json::from_str(schema).expect("schema valid json");
    // Navigasi sederhana: "properties" di root atau di jalur dot.
    let mut node = &v;
    for part in path.split('.').filter(|p| !p.is_empty()) {
        node = &node[part];
    }
    let props = node["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{}: tidak ada properties", path));
    let names: Vec<String> = props.keys().cloned().collect();
    let required: Vec<String> = node["required"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    (names, required)
}

fn json_keys(v: &serde_json::Value) -> Vec<String> {
    v.as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn hardware_schema_matches_types() {
    let schema = include_str!("../../../schemas/hardware/hardware.schema.json");
    let (props, required) = schema_props(schema, "");
    // gpu = Some supaya field optional ikut ter-serialize (skip_serializing_if).
    let mut hw = HardwareReport::default();
    hw.gpu = Some(utbh_core::GpuReport::default());
    let json = serde_json::to_value(&hw).unwrap();
    let keys = json_keys(&json);

    for k in &props {
        assert!(
            keys.contains(k),
            "schema punya field '{}' tapi tipe tidak",
            k
        );
    }
    for k in &keys {
        assert!(
            props.contains(k),
            "tipe punya field '{}' tapi schema tidak — update schemas/hardware/",
            k
        );
    }
    for k in &required {
        assert!(keys.contains(k), "required '{}' tidak ada di tipe", k);
    }
}

#[test]
fn result_schema_matches_types() {
    let schema = include_str!("../../../schemas/result/result.schema.json");
    let (props, required) = schema_props(schema, "");
    // fuzz = Some supaya field optional ikut ter-serialize (skip_serializing_if).
    let run = RunResult::new(
        "id",
        "run",
        "universal",
        HardwareReport::default(),
        vec![],
        Some(utbh_core::FuzzSummary::default()),
    );
    let json = serde_json::to_value(&run).unwrap();
    let keys = json_keys(&json);

    for k in &props {
        assert!(
            keys.contains(k),
            "schema punya field '{}' tapi tipe tidak",
            k
        );
    }
    for k in &keys {
        assert!(
            props.contains(k),
            "tipe punya field '{}' tapi schema tidak — update schemas/result/",
            k
        );
    }
    for k in &required {
        assert!(keys.contains(k), "required '{}' tidak ada di tipe", k);
    }
    assert_eq!(json["schema_version"], SCHEMA_VERSION);
}

#[test]
fn testdef_schema_parses_repo_suite() {
    // Semua suite repo harus valid JSON ketika di-serialize (schema kompatibel).
    let schema = include_str!("../../../schemas/test/test.schema.json");
    let (props, required) = schema_props(schema, "");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../suites");
    if !root.is_dir() {
        return;
    }
    let defs = utbh_core::load_all(&root).expect("suites valid");
    assert!(!defs.is_empty());
    for def in &defs {
        let json = serde_json::to_value(def).unwrap();
        let keys = json_keys(&json);
        for k in &props {
            assert!(
                keys.contains(k),
                "test {}: schema punya '{}' tapi tipe tidak",
                def.test.name,
                k
            );
        }
        for k in &keys {
            assert!(
                props.contains(k),
                "test {}: tipe punya '{}' tapi schema tidak",
                def.test.name,
                k
            );
        }
        for k in &required {
            assert!(
                keys.contains(k),
                "test {}: required '{}' hilang",
                def.test.name,
                k
            );
        }
    }
    // referensi TestDef agar tidak unused-import
    let _: Option<TestDef> = None;
}
