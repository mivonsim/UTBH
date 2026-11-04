//! Orkestrasi validasi: execution (workload engine) vs reference.

use crate::compare::compare;
use crate::reference::reference;
use utbh_core::{Data, TestDef, ValidationInfo};
use utbh_workload::execute;

/// Validasi satu test definition → info PASS/FAIL.
pub fn validate(def: &TestDef) -> Result<ValidationInfo, String> {
    let exec = execute(def)?;
    validate_output(def, &exec.data)
}

/// Validasi hasil eksekusi yang sudah ada terhadap reference.
///
/// Dipakai stress agar region pengukuran latency hanya berisi workload —
/// bukan generate input + eksekusi reference.
pub fn validate_output(def: &TestDef, exec: &Data) -> Result<ValidationInfo, String> {
    let mode = def.validation.mode.as_str();

    match mode {
        "none" => {
            return Ok(ValidationInfo {
                mode: mode.to_string(),
                matched: true,
                mismatches: 0,
                first_mismatch: None,
                max_rel_error: 0.0,
            });
        }
        "exact" | "approx" => {}
        other => {
            return Err(format!(
                "validation.mode '{}' tidak dikenal (pakai exact/approx/none)",
                other
            ));
        }
    }

    let reference = reference(def)?;
    let tol = def
        .validation
        .tolerance
        .unwrap_or(if mode == "approx" { 1e-5 } else { 0.0 });
    let c = compare(&reference, exec, mode, tol);

    Ok(ValidationInfo {
        mode: mode.to_string(),
        matched: c.matched,
        mismatches: c.mismatches,
        first_mismatch: c.first_mismatch,
        max_rel_error: c.max_rel_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(mode: &str) -> TestDef {
        toml::from_str(&format!(
            r#"
[test]
name = "t"
category = "cpu"
[input]
elements = 64
type = "f32"
[operation]
op = "add"
[validation]
mode = "{}"
"#,
            mode
        ))
        .unwrap()
    }

    #[test]
    fn unknown_mode_is_rejected() {
        let err = validate(&def("approx-ish")).unwrap_err();
        assert!(err.contains("tidak dikenal"), "err = {}", err);
    }

    #[test]
    fn none_mode_explicitly_matches() {
        let v = validate(&def("none")).unwrap();
        assert!(v.matched);
        assert_eq!(v.mismatches, 0);
    }

    #[test]
    fn exact_mode_validates() {
        assert!(validate(&def("exact")).unwrap().matched);
    }

    #[test]
    fn validate_output_agrees_with_validate() {
        let d = def("exact");
        let exec = utbh_workload::execute(&d).unwrap();
        let v = validate_output(&d, &exec.data).unwrap();
        assert!(v.matched);
        assert_eq!(v.mismatches, 0);
    }
}
