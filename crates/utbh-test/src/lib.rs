//! `utbh-test` — Layer 4: Test Engine.
//!
//! Menjawab satu pertanyaan: **apakah hardware benar?**
//!
//! Testing ≠ benchmark (Layer 5). Test tidak mengukur kecepatan; ia
//! memvalidasi correctness: instruction, memory, cache, atomic, GPU, DMA,
//! interrupt. Output: PASS / FAIL.
//!
//! Contoh hasil terpisah per test:
//! ```text
//! Matrix multiplication   Correctness: PASS
//! ```

use utbh_core::{Status, TestDef, TestOutcome};
use utbh_validation::validate;

/// Jalankan satu test definition → PASS / FAIL.
///
/// Setiap test wajib punya `[validation]` — test tanpa validasi = bug
/// (format loader menolak test tanpa `validation.mode`).
pub fn run(def: &TestDef) -> TestOutcome {
    let start = std::time::Instant::now();

    if def.validation.mode.is_empty() {
        return TestOutcome::failed(
            &def.test.name,
            &def.test.category,
            "test tidak punya [validation].mode — setiap test wajib divalidasi".into(),
        );
    }

    match validate(def) {
        Ok(info) => TestOutcome {
            name: def.test.name.clone(),
            category: def.test.category.clone(),
            status: if info.matched {
                Status::Pass
            } else {
                Status::Fail
            },
            duration_ns: start.elapsed().as_nanos() as u64,
            validation: Some(info),
            metrics: Vec::new(), // metrik kecepatan = urusan Layer 5
            error: None,
        },
        Err(e) => TestOutcome::failed(&def.test.name, &def.test.category, e),
    }
}

/// Jalankan banyak test (satu suite).
pub fn run_all(defs: &[TestDef]) -> Vec<TestOutcome> {
    defs.iter().map(run).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(mode: &str) -> TestDef {
        let s = format!(
            r#"
[test]
name = "vector_add"
category = "cpu.vector"

[input]
elements = 4096
type = "f32"

[operation]
op = "add"

[validation]
mode = "{}"
"#,
            mode
        );
        toml::from_str(&s).unwrap()
    }

    #[test]
    fn exact_add_passes() {
        let o = run(&def("exact"));
        assert_eq!(o.status, Status::Pass);
        assert!(
            o.metrics.is_empty(),
            "test tidak boleh menghasilkan metrik kecepatan"
        );
    }

    #[test]
    fn invalid_op_fails() {
        let s = r#"
[test]
name = "bad"
category = "cpu"

[input]
elements = 8
type = "f32"

[operation]
op = "does_not_exist"

[validation]
mode = "exact"
"#;
        let o = run(&toml::from_str(s).unwrap());
        assert_eq!(o.status, Status::Fail);
        assert!(o.error.is_some());
    }

    #[test]
    fn missing_validation_is_bug() {
        let s = r#"
[test]
name = "bad"
category = "cpu"

[input]
elements = 8
type = "f32"

[operation]
op = "add"

[validation]
mode = ""
"#;
        let o = run(&toml::from_str(s).unwrap());
        assert_eq!(o.status, Status::Fail);
    }
}
