//! Hasil satu test: status PASS/FAIL, validasi, metrik benchmark.
//!
//! Testing (PASS/FAIL) dan benchmark (metrik) dilaporkan **terpisah**
//! dalam struct yang sama — tidak pernah disatukan jadi skor tunggal.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationInfo {
    pub mode: String,
    pub matched: bool,
    pub mismatches: usize,
    pub first_mismatch: Option<usize>,
    pub max_rel_error: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Status {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestOutcome {
    pub name: String,
    pub category: String,
    pub status: Status,
    pub duration_ns: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<Metric>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TestOutcome {
    pub fn failed(name: &str, category: &str, msg: String) -> Self {
        TestOutcome {
            name: name.to_string(),
            category: category.to_string(),
            status: Status::Fail,
            duration_ns: 0,
            validation: None,
            metrics: Vec::new(),
            error: Some(msg),
        }
    }

    pub fn metric(&self, name: &str) -> Option<&Metric> {
        self.metrics.iter().find(|m| m.name == name)
    }
}

/// Ringkasan satu fuzz run (disimpan di `RunResult`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FuzzSummary {
    pub base_seed: u64,
    pub iterations: usize,
    pub passed: usize,
    pub failed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_outcome_shape() {
        let o = TestOutcome::failed("t", "cpu", "boom".into());
        assert_eq!(o.status, Status::Fail);
        assert_eq!(o.error.as_deref(), Some("boom"));
        assert!(o.metrics.is_empty());
    }

    #[test]
    fn metric_lookup() {
        let mut o = TestOutcome::failed("t", "cpu", "x".into());
        o.metrics.push(Metric {
            name: "latency".into(),
            value: 1.0,
            unit: "ns".into(),
        });
        assert!(o.metric("latency").is_some());
        assert!(o.metric("bandwidth").is_none());
    }

    #[test]
    fn status_serializes_uppercase() {
        let json = serde_json::to_string(&Status::Pass).unwrap();
        assert_eq!(json, "\"PASS\"");
    }
}
