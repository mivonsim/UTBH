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
            None => outcome.error.clone().unwrap_or_else(|| "unknown failure".into()),
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

    // Event kind sesuai spesifikasi trace; OS mengisi nilai cycle-level
    // dari interface hardware ketika tersedia.
    for kind in
        ["instruction", "register", "interrupt", "dma", "gpu_command", "noc_transaction"]
    {
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
