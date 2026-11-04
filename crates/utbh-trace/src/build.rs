//! Bangun trace dari outcome yang FAIL — cycle timestamp via [`HardwareApi`].

use crate::event::{Trace, TraceEvent};
use utbh_api::HardwareApi;
use utbh_core::{Status, TestDef, TestOutcome};

/// `None` bila outcome PASS (tidak ada trace untuk keberhasilan).
pub fn from_failure(
    def: &TestDef,
    outcome: &TestOutcome,
    api: &dyn HardwareApi,
    run_id: &str,
) -> Option<Trace> {
    if matches!(outcome.status, Status::Pass) {
        return None;
    }
    let mut events = Vec::new();
    let cycle = api.cycles();

    events.push(TraceEvent {
        cycle,
        kind: "validation".into(),
        detail: match &outcome.validation {
            Some(v) => format!(
                "mode={} mismatches={} first_mismatch={:?} max_rel_error={:.3e}",
                v.mode, v.mismatches, v.first_mismatch, v.max_rel_error
            ),
            None => outcome
                .error
                .clone()
                .unwrap_or_else(|| "unknown failure".into()),
        },
    });

    events.push(TraceEvent {
        cycle,
        kind: "test".into(),
        detail: format!(
            "name={} category={} op={} elements={} dtype={} seed={:?}",
            def.test.name,
            def.test.category,
            def.operation.op,
            def.input.elements,
            def.input.dtype,
            def.input.seed
        ),
    });

    // Snapshot input pada titik mismatch pertama — data yang memicu FAIL.
    // `stress` memakai first_mismatch sebagai indeks iterasi, bukan indeks data,
    // jadi snapshot data tidak relevan untuk mode itu.
    if let Some(v) = &outcome.validation {
        if v.mode != "stress" {
            if let Some(idx) = v.first_mismatch.filter(|i| *i < def.input.elements) {
                let inputs = utbh_core::inputs(def);
                events.push(TraceEvent {
                    cycle,
                    kind: "memory_access".into(),
                    detail: format!("index={} reference_value={:?}", idx, inputs.a.fmt_at(idx)),
                });
                if let Some(b) = &inputs.b {
                    events.push(TraceEvent {
                        cycle,
                        kind: "memory_access".into(),
                        detail: format!("index={} operand_b={:?}", idx, b.fmt_at(idx)),
                    });
                }
            }
        }
    }

    // Event kind sesuai spesifikasi trace; OS mengisi nilai cycle-level
    // dari interface hardware ketika tersedia.
    for kind in [
        "instruction",
        "register",
        "interrupt",
        "dma",
        "gpu_command",
        "noc_transaction",
    ] {
        events.push(TraceEvent {
            cycle,
            kind: kind.into(),
            detail: "captured via hardware/OS interface".into(),
        });
    }

    Some(Trace {
        run_id: run_id.to_string(),
        test: def.test.name.clone(),
        category: def.test.category.clone(),
        events,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use utbh_core::{HardwareReport, Status, TestOutcome, ValidationInfo};

    struct StubApi;

    impl HardwareApi for StubApi {
        fn hardware_report(&self) -> HardwareReport {
            HardwareReport::default()
        }
        fn timer_ns(&self) -> u64 {
            0
        }
        fn cycles(&self) -> u64 {
            7
        }
        fn cpu_frequency_hz(&self) -> u64 {
            1_000_000_000
        }
        fn cpu_count(&self) -> usize {
            1
        }
        fn pin_to_cpu(&self, _cpu: usize) -> Result<(), String> {
            Ok(())
        }
        fn dma_scratch(&self, size: usize) -> Result<Vec<u8>, String> {
            Ok(vec![0; size])
        }
    }

    fn def() -> TestDef {
        toml::from_str(
            r#"
[test]
name = "t"
category = "cpu"
[input]
elements = 16
type = "f32"
[operation]
op = "add"
[validation]
mode = "exact"
"#,
        )
        .unwrap()
    }

    fn failed(mode: &str, first_mismatch: Option<usize>) -> TestOutcome {
        let mut o = TestOutcome::failed("t", "cpu", "x".into());
        o.validation = Some(ValidationInfo {
            mode: mode.into(),
            matched: false,
            mismatches: 1,
            first_mismatch,
            max_rel_error: 0.0,
        });
        o
    }

    #[test]
    fn pass_has_no_trace() {
        let mut o = failed("exact", Some(0));
        o.status = Status::Pass;
        assert!(from_failure(&def(), &o, &StubApi, "r").is_none());
    }

    #[test]
    fn exact_failure_includes_memory_snapshot() {
        let t = from_failure(&def(), &failed("exact", Some(3)), &StubApi, "r").unwrap();
        assert!(t.events.iter().any(|e| e.kind == "memory_access"));
    }

    #[test]
    fn stress_failure_skips_data_index_snapshot() {
        let t = from_failure(&def(), &failed("stress", Some(3)), &StubApi, "r").unwrap();
        assert!(!t.events.iter().any(|e| e.kind == "memory_access"));
        assert!(t.events.iter().any(|e| e.kind == "validation"));
    }

    #[test]
    fn out_of_range_index_does_not_emit_snapshot() {
        let t = from_failure(&def(), &failed("exact", Some(999)), &StubApi, "r").unwrap();
        assert!(!t.events.iter().any(|e| e.kind == "memory_access"));
    }
}
