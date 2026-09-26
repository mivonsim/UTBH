//! Test definition deklaratif — dibaca dari file TOML `*.utbh`.
//!
//! Data-driven: menambah test = menambah file, tanpa rebuild binary.
//! Tidak ada `const TEST_001 = ...` di Rust (AGENTS.md).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDef {
    pub test: TestMeta,
    pub input: InputSpec,
    pub operation: OperationSpec,
    pub validation: ValidationSpec,
    #[serde(default)]
    pub benchmark: BenchmarkSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestMeta {
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSpec {
    pub elements: usize,
    #[serde(rename = "type", default = "default_dtype")]
    pub dtype: String,
    #[serde(default)]
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationSpec {
    pub op: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSpec {
    /// `exact` | `approx` | `none`
    pub mode: String,
    #[serde(default)]
    pub tolerance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSpec {
    #[serde(default)]
    pub metrics: Vec<String>,
    #[serde(default = "default_iterations")]
    pub iterations: u32,
}

impl Default for BenchmarkSpec {
    fn default() -> Self {
        Self { metrics: Vec::new(), iterations: default_iterations() }
    }
}

fn default_dtype() -> String {
    "f32".to_string()
}

fn default_iterations() -> u32 {
    5
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[test]
name = "vector_add"
category = "cpu.vector"

[input]
elements = 1024
type = "f32"

[operation]
op = "add"

[validation]
mode = "exact"

[benchmark]
metrics = ["latency", "throughput"]
"#;

    #[test]
    fn parse_test_def() {
        let def: TestDef = toml::from_str(SAMPLE).expect("parse");
        assert_eq!(def.test.name, "vector_add");
        assert_eq!(def.input.elements, 1024);
        assert_eq!(def.input.dtype, "f32");
        assert_eq!(def.operation.op, "add");
        assert_eq!(def.validation.mode, "exact");
        assert_eq!(def.benchmark.metrics.len(), 2);
        assert_eq!(def.benchmark.iterations, 5);
    }

    #[test]
    fn defaults_applied() {
        let def: TestDef = toml::from_str(SAMPLE).unwrap();
        assert_eq!(def.benchmark.iterations, 5);
        assert!(def.input.seed.is_none());
        assert_eq!(def.input.dtype, "f32");
    }
}
