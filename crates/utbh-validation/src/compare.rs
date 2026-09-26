//! Komparator: bandingkan dua `Data` dengan mode `exact` / `approx`.

use utbh_core::Data;

/// Hasil komparasi.
pub struct CompareOutcome {
    pub matched: bool,
    pub mismatches: usize,
    pub first_mismatch: Option<usize>,
    pub max_rel_error: f64,
}

/// `approx` — selisih relatif elemen cocok bila ≤ `tol`.
/// `exact` — wajib identik (tol diabaikan).
///
/// Relatif error dinormalisasi terhadap skala data (`scale`) supaya elemen
/// yang mendekati nol tidak menghasilkan relatif error semu besar.
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
    let scale = max_abs(expected).max(1e-12);
    let mut mismatches = 0usize;
    let mut first: Option<usize> = None;
    let mut max_rel = 0f64;

    for i in 0..expected.len() {
        let abs = elem_abs_error(expected, actual, i);
        let rel = if abs.is_finite() { abs / scale } else { f64::INFINITY };
        max_rel = max_rel.max(rel);
        let ok = if abs.is_finite() {
            abs == 0.0 || (use_tol && rel <= tol)
        } else {
            false // tipe beda / tidak kompatibel
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

/// Absolut error satu elemen. `0.0` = identik. `INFINITY` = tipe beda.
fn elem_abs_error(e: &Data, a: &Data, i: usize) -> f64 {
    match (e, a) {
        (Data::F32(x), Data::F32(y)) => ((y[i] - x[i]) as f64).abs(),
        (Data::U64(x), Data::U64(y)) => {
            if x[i] == y[i] {
                0.0
            } else {
                f64::INFINITY // beda = beda, tanpa derajat
            }
        }
        (Data::I64(x), Data::I64(y)) => {
            if x[i] == y[i] {
                0.0
            } else {
                f64::INFINITY
            }
        }
        _ => f64::INFINITY,
    }
}

/// Skala data: nilai absolut maksimum (untuk normalisasi relatif error).
fn max_abs(d: &Data) -> f64 {
    match d {
        Data::F32(v) => v.iter().fold(0f64, |m, x| m.max((*x as f64).abs())),
        Data::U64(v) => v.iter().fold(0f64, |m, x| m.max(*x as f64)),
        Data::I64(v) => v.iter().fold(0f64, |m, x| m.max((*x as f64).abs())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_mismatch() {
        let a = Data::F32(vec![1.0, 2.0, 3.0]);
        let b = Data::F32(vec![1.0, 9.0, 3.0]);
        let c = compare(&a, &b, "exact", 0.0);
        assert!(!c.matched);
        assert_eq!(c.mismatches, 1);
        assert_eq!(c.first_mismatch, Some(1));
    }

    #[test]
    fn approx_tolerates_small_error() {
        let a = Data::F32(vec![1.0]);
        let b = Data::F32(vec![1.0 + 1e-7]);
        assert!(!compare(&a, &b, "exact", 0.0).matched);
        assert!(compare(&a, &b, "approx", 1e-5).matched);
    }

    #[test]
    fn length_mismatch_fails() {
        let a = Data::F32(vec![1.0, 2.0]);
        let b = Data::F32(vec![1.0]);
        let c = compare(&a, &b, "exact", 0.0);
        assert!(!c.matched);
        assert!(c.first_mismatch.is_some());
    }

    #[test]
    fn dtype_mismatch_is_infinite_error() {
        let a = Data::F32(vec![1.0]);
        let b = Data::U64(vec![1]);
        assert!(!compare(&a, &b, "approx", 1.0).matched);
    }
}
