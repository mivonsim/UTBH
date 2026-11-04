//! Sinkronisasi `schemas/**` ↔ tipe `utbh-core` (AGENTS.md aturan #8).
//!
//! Validator kecil ini mengecek subset JSON Schema yang dipakai repo
//! (`type`, `const`, `enum`, `required`, `properties`, `additionalProperties`,
//! `items`, `minimum`, `minLength`, `$ref` hardware). Tujuannya menangkap
//! field hilang, field ekstra, dan nilai di luar enum sebelum artefak dirilis.

use serde_json::Value;
use utbh_core::{
    CacheLevel, CpuReport, GpuReport, HardwareReport, InterconnectReport, MemoryReport, Metric,
    RunResult, Status, TestDef, TestOutcome, ValidationInfo, SCHEMA_VERSION,
};

fn json_schema(text: &str) -> Value {
    serde_json::from_str(text).expect("schema valid JSON")
}

fn hardware_schema() -> Value {
    json_schema(include_str!(
        "../../../schemas/hardware/hardware.schema.json"
    ))
}

fn type_matches(instance: &Value, ty: &str) -> bool {
    match ty {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "boolean" => instance.is_boolean(),
        "integer" => instance.is_i64() || instance.is_u64(),
        "number" => instance.is_number(),
        "null" => instance.is_null(),
        other => panic!("tipe schema tidak dikenal: {}", other),
    }
}

fn validate_instance(schema: &Value, instance: &Value, path: &str, hardware: &Value) {
    if let Some(reference) = schema.get("$ref").and_then(|r| r.as_str()) {
        assert!(
            reference.ends_with("hardware/hardware.schema.json"),
            "{}: $ref tidak didukung: {}",
            path,
            reference
        );
        validate_instance(hardware, instance, path, hardware);
        return;
    }

    if let Some(types) = schema.get("type") {
        let ok = match types {
            Value::String(ty) => type_matches(instance, ty),
            Value::Array(types) => types
                .iter()
                .filter_map(|ty| ty.as_str())
                .any(|ty| type_matches(instance, ty)),
            _ => panic!("{}: nilai type tidak valid", path),
        };
        assert!(
            ok,
            "{}: tipe tidak cocok (schema {:?}, nilai {})",
            path, types, instance
        );
    }

    if let Some(expected) = schema.get("const") {
        assert_eq!(instance, expected, "{}: const tidak cocok", path);
    }

    if let Some(values) = schema.get("enum").and_then(|e| e.as_array()) {
        assert!(
            values.contains(instance),
            "{}: nilai {} tidak ada di enum {:?}",
            path,
            instance,
            values
        );
    }

    if let Some(min) = schema.get("minimum").and_then(|m| m.as_f64()) {
        if let Some(n) = instance.as_f64() {
            assert!(n >= min, "{}: {} < minimum {}", path, n, min);
        }
    }

    if let Some(min_len) = schema.get("minLength").and_then(|m| m.as_u64()) {
        if let Some(s) = instance.as_str() {
            assert!(
                s.chars().count() as u64 >= min_len,
                "{}: string lebih pendek dari {}",
                path,
                min_len
            );
        }
    }

    if let Some(obj) = instance.as_object() {
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            for key in required.iter().filter_map(|k| k.as_str()) {
                assert!(obj.contains_key(key), "{}: required '{}' hilang", path, key);
            }
        }

        let props = schema.get("properties").and_then(|p| p.as_object());
        let deny_extra =
            schema.get("additionalProperties").and_then(|a| a.as_bool()) == Some(false);
        for (key, value) in obj {
            match props.and_then(|p| p.get(key)) {
                Some(child) => {
                    validate_instance(child, value, &format!("{}.{}", path, key), hardware)
                }
                None if deny_extra => {
                    panic!("{}: field '{}' tidak ada di schema", path, key)
                }
                None => {}
            }
        }
    }

    if let Some(items) = instance.as_array() {
        if let Some(item_schema) = schema.get("items") {
            for (i, value) in items.iter().enumerate() {
                validate_instance(item_schema, value, &format!("{}[{}]", path, i), hardware);
            }
        }
    }
}

fn realistic_hardware() -> HardwareReport {
    HardwareReport {
        cpu: CpuReport {
            architecture: "x86_64".into(),
            model: "test-cpu".into(),
            cores: 1,
            threads: 1,
            vector: "AVX2".into(),
            frequency_mhz: 1000,
        },
        cache: vec![CacheLevel {
            level: 1,
            kind: "L1D".into(),
            size_bytes: 32 * 1024,
        }],
        memory: MemoryReport {
            capacity_bytes: 1024,
            channels: 1,
        },
        gpu: Some(GpuReport {
            name: "gpu0".into(),
            compute_units: 1,
            fp32: true,
            fp16: false,
            bf16: false,
            int8: false,
        }),
        interconnect: InterconnectReport {
            kind: "on-chip".into(),
            topology: "mesh".into(),
        },
    }
}

#[test]
fn hardware_schema_matches_types() {
    let schema = hardware_schema();
    let json = serde_json::to_value(realistic_hardware()).unwrap();
    validate_instance(&schema, &json, "$", &schema);
}

#[test]
fn result_schema_matches_types() {
    let schema = json_schema(include_str!("../../../schemas/result/result.schema.json"));
    let run = RunResult::new(
        "run-1",
        "run",
        "universal",
        realistic_hardware(),
        vec![TestOutcome {
            name: "vector_add".into(),
            category: "cpu.vector".into(),
            status: Status::Pass,
            duration_ns: 10,
            validation: Some(ValidationInfo {
                mode: "exact".into(),
                matched: true,
                mismatches: 0,
                first_mismatch: None,
                max_rel_error: 0.0,
            }),
            metrics: vec![Metric {
                name: "latency".into(),
                value: 1.5,
                unit: "ns".into(),
            }],
            error: None,
        }],
        Some(utbh_core::FuzzSummary {
            base_seed: 1,
            iterations: 1,
            passed: 1,
            failed: 0,
        }),
    );
    let json = serde_json::to_value(&run).unwrap();
    assert_eq!(json["schema_version"], SCHEMA_VERSION);
    validate_instance(&schema, &json, "$", &hardware_schema());
}

#[test]
fn result_schema_accepts_finite_failure_error() {
    // Regresi: max_rel_error non-finite membuat serde_json menulis `null`,
    // yang bukan `number` dan melanggar schema hasil.
    let schema = json_schema(include_str!("../../../schemas/result/result.schema.json"));
    let run = RunResult::new(
        "run-2",
        "test",
        "cpu",
        realistic_hardware(),
        vec![TestOutcome {
            name: "broken".into(),
            category: "cpu".into(),
            status: Status::Fail,
            duration_ns: 1,
            validation: Some(ValidationInfo {
                mode: "exact".into(),
                matched: false,
                mismatches: 1,
                first_mismatch: Some(0),
                max_rel_error: f64::MAX,
            }),
            metrics: vec![],
            error: None,
        }],
        None,
    );
    let json = serde_json::to_value(&run).unwrap();
    assert!(json["outcomes"][0]["validation"]["max_rel_error"].is_number());
    validate_instance(&schema, &json, "$", &hardware_schema());
}

#[test]
fn testdef_schema_parses_repo_suite() {
    let schema = json_schema(include_str!("../../../schemas/test/test.schema.json"));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../suites");
    if !root.is_dir() {
        return;
    }

    let defs = utbh_core::load_all(&root).expect("suites valid");
    assert!(!defs.is_empty());
    for def in &defs {
        let json = serde_json::to_value(def).unwrap();
        validate_instance(
            &schema,
            &json,
            &format!("$({})", def.test.name),
            &Value::Null,
        );
    }
}

#[test]
fn testdef_schema_is_exhaustive_for_optional_benchmark() {
    // TestDef tanpa `[benchmark]` tetap men-serialize benchmark default;
    // schema harus punya properti itu.
    let schema = json_schema(include_str!("../../../schemas/test/test.schema.json"));
    let def: TestDef = toml::from_str(
        r#"
[test]
name = "no_bench"
category = "cpu"
[input]
elements = 8
type = "f32"
[operation]
op = "add"
[validation]
mode = "exact"
"#,
    )
    .unwrap();
    let json = serde_json::to_value(&def).unwrap();
    assert!(json.get("benchmark").is_some());
    validate_instance(&schema, &json, "$", &Value::Null);
}
