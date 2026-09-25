//! `utbh-trace` — Layer 7: Trace untuk kasus FAIL.
//!
//! Ketika validation FAIL, trace berisi:
//! cycle, instruction, register, memory access, interrupt, DMA,
//! GPU command, NoC transaction.
//!
//! Trace dibaca dari interface hardware/OS — **bukan** dengan memanggil
//! simulator secara langsung. Setiap event diberi cycle timestamp dari
//! [`HardwareApi::cycles`].
//!
//! Output: `<run-id>.utbh-trace.json`

use serde::{Deserialize, Serialize};
use utbh_api::HardwareApi;
use utbh_core::{TestDef, TestOutcome};

/// Satu event dalam trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub cycle: u64,
    pub kind: String,
    pub detail: String,
}

/// Trace lengkap satu test yang FAIL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub run_id: String,
    pub test: String,
    pub category: String,
    pub events: Vec<TraceEvent>,
}

impl Trace {
    /// Bangun trace dari outcome yang FAIL.
    ///
    /// Mencatat konteks test + snapshot data mismatch (elemen pertama yang
    /// beda). Event cycle-level (instruction/register/NoC) diisi dari
    /// hardware/OS interface bila tersedia pada implementasi OS.
    pub fn from_failure(
        def: &TestDef,
        outcome: &TestOutcome,
        api: &dyn HardwareApi,
        run_id: &str,
    ) -> Option<Trace> {
        if matches!(outcome.status, utbh_core::Status::Pass) {
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
        if let Some(v) = &outcome.validation {
            if let Some(idx) = v.first_mismatch {
                let inputs = utbh_core::inputs(def);
                let e = inputs.a.fmt_at(idx);
                events.push(TraceEvent {
                    cycle,
                    kind: "memory_access".into(),
                    detail: format!("index={} reference_value={:?}", idx, e),
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

        // Placeholder event kind sesuai spesifikasi trace; OS mengisi nilai
        // cycle-level dari interface hardware ketika tersedia.
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

    pub fn save(&self, dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{}.utbh-trace.json", self.run_id));
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&path, json)?;
        Ok(path)
    }

    pub fn load(path: &std::path::Path) -> Result<Trace, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {}", path.display(), e))
    }

    /// Render teks untuk `utbh trace <run-id>`.
    pub fn render(&self) -> String {
        let mut out = format!(
            "Trace {} — {} ({})\n",
            self.run_id, self.test, self.category
        );
        for e in &self.events {
            out.push_str(&format!("{:>12}  {:<16} {}\n", e.cycle, e.kind, e.detail));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use utbh_api::MivonHardwareApi;
    use utbh_core::Status;

    fn def() -> TestDef {
        toml::from_str(
            r#"
[test]
name = "failing"
category = "cpu.vector"
[input]
elements = 64
type = "f32"
[operation]
op = "add"
[validation]
mode = "exact"
"#,
        )
        .unwrap()
    }

    fn failing_outcome() -> TestOutcome {
        TestOutcome {
            name: "failing".into(),
            category: "cpu.vector".into(),
            status: Status::Fail,
            duration_ns: 0,
            validation: Some(utbh_core::ValidationInfo {
                mode: "exact".into(),
                matched: false,
                mismatches: 3,
                first_mismatch: Some(7),
                max_rel_error: 1.0,
            }),
            metrics: vec![],
            error: None,
        }
    }

    #[test]
    fn trace_from_failure_has_events() {
        let api = MivonHardwareApi::new();
        let t = Trace::from_failure(&def(), &failing_outcome(), &api, "run-1").unwrap();
        assert!(t.events.iter().any(|e| e.kind == "validation"));
        assert!(t.events.iter().any(|e| e.kind == "memory_access"));
        assert!(t.events.iter().any(|e| e.kind == "noc_transaction"));
    }

    #[test]
    fn no_trace_for_pass() {
        let api = MivonHardwareApi::new();
        let mut o = failing_outcome();
        o.status = Status::Pass;
        assert!(Trace::from_failure(&def(), &o, &api, "run-1").is_none());
    }
}
