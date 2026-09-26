//! `utbh-validation` — Layer 6: Differential Validation.
//!
//! ```text
//! TEST
//!   ├─ Execution A  → result A     (utbh-workload, jalur hardware)
//!   └─ Reference A  → result B     (reference.rs, implementasi independen)
//!             └──→ Comparator ──→ PASS / FAIL
//! ```
//!
//! **1 file = 1 tanggung jawab:**
//! - [`validate`] — orkestrasi: jalankan execution + reference, komparasi
//! - [`reference`] — implementasi referensi independen (algoritma beda)
//! - [`compare`] — komparator angka (`exact` / `approx`)
//!
//! Validator hanya membandingkan data. Trace saat FAIL → `utbh-trace` (dibaca
//! dari interface hardware/OS, bukan dengan memanggil simulator langsung).

pub mod compare;
pub mod reference;
pub mod validate;

pub use compare::compare;
pub use reference::reference;
pub use validate::validate;

#[cfg(test)]
mod tests {
    use super::*;
    use utbh_core::TestDef;

    fn def(op: &str, dtype: &str, elements: usize, mode: &str, tol: Option<f64>) -> TestDef {
        let tol_s = match tol {
            Some(t) => format!("tolerance = {}", t),
            None => String::new(),
        };
        let s = format!(
            r#"
[test]
name = "t"
category = "cpu"

[input]
elements = {}
type = "{}"

[operation]
op = "{}"

[validation]
mode = "{}"
{}
"#,
            elements, dtype, op, mode, tol_s
        );
        toml::from_str(&s).unwrap()
    }

    #[test]
    fn exact_match_add() {
        let v = validate(&def("add", "f32", 1000, "exact", None)).unwrap();
        assert!(v.matched, "add harus identik antar jalur");
    }

    #[test]
    fn exact_match_u64() {
        let v = validate(&def("mul", "u64", 500, "exact", None)).unwrap();
        assert!(v.matched);
    }

    #[test]
    fn matmul_close_enough() {
        let v = validate(&def("matmul", "f32", 400, "approx", Some(1e-4))).unwrap();
        assert!(v.matched, "matmul approx: rel_err={}", v.max_rel_error);
    }

    #[test]
    fn none_mode_skips() {
        let v = validate(&def("add", "f32", 10, "none", None)).unwrap();
        assert!(v.matched);
    }
}
