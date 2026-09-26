//! Orkestrasi validasi: execution (workload engine) vs reference.

use crate::compare::compare;
use crate::reference::reference;
use utbh_core::{TestDef, ValidationInfo};
use utbh_workload::execute;

/// Validasi satu test definition → info PASS/FAIL.
pub fn validate(def: &TestDef) -> Result<ValidationInfo, String> {
    let exec = execute(def)?;
    let reference = reference(def)?;
    let mode = def.validation.mode.as_str();

    if mode == "none" {
        return Ok(ValidationInfo {
            mode: mode.to_string(),
            matched: true,
            mismatches: 0,
            first_mismatch: None,
            max_rel_error: 0.0,
        });
    }

    let tol = def
        .validation
        .tolerance
        .unwrap_or(if mode == "approx" { 1e-5 } else { 0.0 });
    let c = compare(&reference, &exec.data, mode, tol);

    Ok(ValidationInfo {
        mode: mode.to_string(),
        matched: c.matched,
        mismatches: c.mismatches,
        first_mismatch: c.first_mismatch,
        max_rel_error: c.max_rel_error,
    })
}
