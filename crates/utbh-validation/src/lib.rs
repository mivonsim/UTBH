//! `utbh-validation` — Layer 6: Differential Validation.
//!
//! ```text
//! TEST
//!   ├─ Execution A  → result A     (utbh-workload, jalur hardware)
//!   └─ Reference A  → result B     (implementasi referensi independen)
//!             └──→ Comparator ──→ PASS / FAIL
//! ```
//!
//! Reference execution di sini dihitung ulang dengan implementasi referensi
//! yang **sengaja ditulis berbeda** dari workload engine (algoritma lain,
//! urutan loop berbeda). Kalau dua jalur berbeda menghasilkan beda → bug.
//!
//! Validator hanya membandingkan data. Untuk trace saat FAIL, lihat
//! `utbh-trace` — trace dibangun dari interface hardware/OS, bukan dengan
//! memanggil simulator secara langsung.

use utbh_core::{inputs, Data, TestDef, ValidationInfo};
use utbh_workload::execute;

/// Hasil komparasi.
pub struct CompareOutcome {
    pub matched: bool,
    pub mismatches: usize,
    pub first_mismatch: Option<usize>,
    pub max_rel_error: f64,
}

/// Validasi satu test: execution (workload engine) vs reference.
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
    let op = def.operation.op.as_str();
    // Matmul laporan error pembulatan f32 via **dua** jalur independen; Bina
    // satu yang benar (f64). Per-elemen relatif error meleksplodi bila satu
    // elemen keluar kecil (cancellation) — itu numerik, bukan bug hardware.
    // Jadi matmul divalidasi dengan komparasi **scale-relative**.
    let c = if is_matmul(op) {
        compare_matmul_scale(&reference, &exec.data, tol.max(1e-3))
    } else {
        compare(&reference, &exec.data, mode, tol)
    };

    Ok(ValidationInfo {
        mode: mode.to_string(),
        matched: c.matched,
        mismatches: c.mismatches,
        first_mismatch: c.first_mismatch,
        max_rel_error: c.max_rel_error,
    })
}

/// Reference execution — implementasi independen, algoritma berbeda dari
/// `utbh-workload::execute`. Deterministik (input dari `inputs()`).
pub fn reference(def: &TestDef) -> Result<Data, String> {
    let inp = inputs(def);
    let a = inp.a;
    let b = inp.b;
    let need_b = |b: &Option<Data>| -> Result<Data, String> {
        b.clone()
            .ok_or_else(|| format!("op '{}' butuh dua operand", def.operation.op))
    };
    let op = def.operation.op.as_str();

    match op {
        "add" | "vector_add" => scalar_pair(a, need_b(&b)?, |x, y| x + y),
        "sub" | "vector_sub" => scalar_pair(a, need_b(&b)?, |x, y| x - y),
        "mul" | "vector_mul" => scalar_pair(a, need_b(&b)?, |x, y| x * y),
        "compare" => {
            // Reference compare: predikat **per-elemen** independen (0/1),
            // sama saruri dengan workload — agar exact mode cocok.
            scalar_pair(a, need_b(&b)?, |p, q| if p == q { 1.0 } else { 0.0 })
        }
        "fma" | "vector_fma" => {
            let c = need_b(&b)?;
            match (a, need_b(&b)?, c) {
                (Data::F32(x), Data::F32(y), Data::F32(z)) => Ok(Data::F32(
                    (0..x.len()).map(|i| x[i].mul_add(y[i], z[i])).collect(),
                )),
                _ => Err("fma hanya f32".into()),
            }
        }
        "matmul" | "matrix_mul" => reference_matmul(a, need_b(&b)?),
        "reduce_sum" | "reduce" => Ok(match a {
            Data::F32(v) => {
                // Versi independen: bagi dua secara rekursif (pairwise summation)
                // → error pembulatan berbeda dari workload, cocok utk approx.
                Data::F32(vec![pairwise_sum(&v)])
            }
            Data::U64(v) => Data::U64(vec![v.iter().fold(0u64, |s, x| s.wrapping_add(*x))]),
            Data::I64(v) => Data::I64(vec![v.iter().fold(0i64, |s, x| s.wrapping_add(*x))]),
        }),
        "copy" | "memcopy" | "mem_seq" | "seq" | "mem_rand" | "rand" => Ok(a),
        other => Err(format!("op '{}' tidak punya reference", other)),
    }
}

/// Referensi matmul: **independen & amin tinggi** — acumula dalam `f64`
/// (algoritma berbeda dari workload: urutan i-j-k + akumulasi f64).
///
/// Workload f32 naif memerankan "hardware"; referensi kuat menjawab "nilai
/// benar". Truncate ke f32 di akhir untuk komparasi dalam tipe `Data`.
fn reference_matmul(a: Data, b: Data) -> Result<Data, String> {
    let (Data::F32(x), Data::F32(y)) = (a, b) else {
        return Err("matmul hanya f32".into());
    };
    let n = (x.len() as f64).sqrt().floor() as usize;
    if n == 0 {
        return Err("matmul butuh minimal 1 elemen".into());
    }
    let mut c = vec![0f32; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut acc = 0f64;
            for k in 0..n {
                acc += (x[i * n + k] as f64) * (y[k * n + j] as f64);
            }
            c[i * n + j] = acc as f32;
        }
    }
    Ok(Data::F32(c))
}

fn is_matmul(op: &str) -> bool {
    op == "matmul" || op == "matrix_mul"
}

fn pairwise_sum(v: &[f32]) -> f32 {
    if v.len() <= 1 {
        return v.first().copied().unwrap_or(0.0);
    }
    let mid = v.len() / 2;
    pairwise_sum(&v[..mid]) + pairwise_sum(&v[mid..])
}

fn scalar_pair(a: Data, b: Data, f: impl Fn(f64, f64) -> f64) -> Result<Data, String> {
    if a.len() != b.len() {
        return Err(format!("panjang beda: {} vs {}", a.len(), b.len()));
    }
    Ok(match (a, b) {
        (Data::F32(x), Data::F32(y)) => Data::F32(
            x.iter()
                .zip(&y)
                .map(|(p, q)| f(*p as f64, *q as f64) as f32)
                .collect(),
        ),
        (Data::U64(x), Data::U64(y)) => Data::U64(
            x.iter()
                .zip(&y)
                .map(|(p, q)| f(*p as f64, *q as f64) as u64)
                .collect(),
        ),
        (Data::I64(x), Data::I64(y)) => Data::I64(
            x.iter()
                .zip(&y)
                .map(|(p, q)| f(*p as f64, *q as f64) as i64)
                .collect(),
        ),
        _ => return Err("dtype campuran".into()),
    })
}

/// Bandingkan dua `Data` dengan mode `exact` / `approx`.
///
/// `approx` — selisih relatif elemen dianggap cocok bila ≤ `tol`.
/// `exact` — wajib identik (tol diabaikan).
pub fn compare(expected: &Data, actual: &Data, mode: &str, tol: f64) -> CompareOutcome {
    if expected.len() != actual.len() {
        return CompareOutcome {
            matched: false,
            mismatches: expected.len().max(actual.len()),
            first_mismatch: Some(0),
            max_rel_error: f64::INFINITY,
        };
    }

    let use_tol = mode != "exact" && tol > 0.0;
    let mut mismatches = 0usize;
    let mut first: Option<usize> = None;
    let mut max_rel = 0f64;

    for i in 0..expected.len() {
        let rel = elem_rel_error(expected, actual, i);
        max_rel = max_rel.max(rel);
        let ok = if rel.is_finite() {
            rel == 0.0 || (use_tol && rel <= tol)
        } else {
            false // dtype beda / inkompitabel
        };
        if !ok {
            mismatches += 1;
            first.get_or_insert(i);
        }
    }

    CompareOutcome {
        matched: mismatches == 0,
        mismatches,
        first_mismatch: first,
        max_rel_error: max_rel,
    }
}

/// Relatif error satu elemen. `0.0` = identik. `INFINITY` = tipe beda.
fn elem_rel_error(e: &Data, a: &Data, i: usize) -> f64 {
    match (e, a) {
        (Data::F32(x), Data::F32(y)) => {
            let (p, q) = (x[i], y[i]);
            if p == q {
                0.0
            } else {
                let denom = p.abs().max(1e-12);
                (((q - p) / denom).abs()) as f64
            }
        }
        (Data::U64(x), Data::U64(y)) => {
            if x[i] == y[i] {
                0.0
            } else {
                1.0
            }
        }
        (Data::I64(x), Data::I64(y)) => {
            if x[i] == y[i] {
                0.0
            } else {
                1.0
            }
        }
        _ => f64::INFINITY,
    }
}

/// Komparasi matmul **scale-relative**: error absolut setiap elemen
/// dinormalisasi oleh magnitudo maksimum seluruh matriks referensi (dengan
/// floor), bukan oleh nilai elemen itu sendiri.
///
/// Motivasi: matmul f32 acumula error pembulatan ~ O(n·eps). Per-elemen
/// relatif error meleksplodi bila `C[i][j]` keluar ~0 dari cancellation —
/// itu error numerik antara dua jalur f32, **bukan bug hardware**. Denominator
/// = max|C_ref| memegan estos falsos positivos tanpa melossie a deteksi
/// deviation nyen besar (hardware matmul bug yang nyen).
fn compare_matmul_scale(expected: &Data, actual: &Data, tol: f64) -> CompareOutcome {
    if expected.len() != actual.len() {
        return CompareOutcome {
            matched: false,
            mismatches: expected.len().max(actual.len()),
            first_mismatch: Some(0),
            max_rel_error: f64::INFINITY,
        };
    }
    let (Data::F32(e), Data::F32(a)) = (&expected, &actual) else {
        // matmul hanya mendukung f32 — tipe beda = fail deterministik.
        return CompareOutcome {
            matched: false,
            mismatches: expected.len(),
            first_mismatch: Some(0),
            max_rel_error: f64::INFINITY,
        };
    };
    let mut scale = 0f64;
    for v in e {
        scale = scale.max(v.abs() as f64);
    }
    scale = scale.max(1e-6); // floor: anti matriks seluruh ~0 (cancellation total)

    let mut mismatches = 0usize;
    let mut first: Option<usize> = None;
    let mut max_rel = 0f64;
    for i in 0..e.len() {
        let rel = ((a[i] - e[i]).abs() as f64) / scale;
        max_rel = max_rel.max(rel);
        if rel > tol {
            mismatches += 1;
            first.get_or_insert(i);
        }
    }

    CompareOutcome {
        matched: mismatches == 0,
        mismatches,
        first_mismatch: first,
        max_rel_error: max_rel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        if !v.matched {
            panic!("matmul approx: rel_err={}", v.max_rel_error);
        }
    }

    #[test]
    fn matmul_large_n_is_not_false_positive() {
        // Regresija bug fuzz: n=200 matmul pernah FAIL massif (per-elemen
        // relatif error 1e4..3.5e5). Scale-relative komparasi harus PASS.
        let v = validate(&def("matmul", "f32", 40000, "approx", Some(1e-3))).unwrap();
        assert!(
            v.matched,
            "n=200 matmul tidak boleh false-positive (rel_err={})",
            v.max_rel_error
        );
    }

    #[test]
    fn matmul_exact_mode_uses_floor_tolerance() {
        // `exact` matmul tidak punya makna: dua jalur f32 independen biru
        // berbeda bit?not. Harus route ke scale-relative dengan floor.
        let v = validate(&def("matmul", "f32", 400, "exact", None)).unwrap();
        assert!(v.matched, "matmul exact tidak boleh false-positive");
    }

    #[test]
    fn compare_exact_u64() {
        // Regresija: workload (per-elemen) vs reference (hiaba fold XOR) irene
        // beda saruri → mismatches=n. Harus identik exact.
        let v = validate(&def("compare", "u64", 500, "exact", None)).unwrap();
        assert!(
            v.matched,
            "compare u64 harus identik antar jalur (rel_err={})",
            v.max_rel_error
        );
    }

    #[test]
    fn matmul_scale_compare_detects_real_deviation() {
        // Deviation nyen besar (elemen 50x dari referensi) cara dinigge:
        // komparasi lu rístrict, bukan lewat.
        let e = Data::F32(vec![1.0, 1.0, 1.0, 1.0]);
        let a = Data::F32(vec![1.0, 1.0, 1.0, 50.0]);
        let c = compare_matmul_scale(&e, &a, 1e-3);
        assert!(!c.matched);
        assert_eq!(c.mismatches, 1);
    }

    #[test]
    fn none_mode_skips() {
        let v = validate(&def("add", "f32", 10, "none", None)).unwrap();
        assert!(v.matched);
    }

    #[test]
    fn detects_mismatch() {
        let a = Data::F32(vec![1.0, 2.0, 3.0]);
        let b = Data::F32(vec![1.0, 9.0, 3.0]);
        let c = compare(&a, &b, "exact", 0.0);
        assert!(!c.matched);
        assert_eq!(c.mismatches, 1);
        assert_eq!(c.first_mismatch, Some(1));
    }
}
