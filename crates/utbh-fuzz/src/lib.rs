//! `utbh-fuzz` — UTBH Fuzz Engine.
//!
//! ```text
//! Random Generator → Hardware Workload → Hardware API → Hardware VM
//!                                                            │
//!                                                     Validation
//!                                                   ┌────┴────┐
//!                                                 PASS       FAIL
//!                                                            │
//!                                                        Minimizer
//!                                                            │
//!                                                 Reproducible Test
//! ```
//!
//! Fuzz kombinasi (CPU instruction + memory pattern + ukuran input +
//! operasi) lebih berguna menemukan bug SoC daripada benchmark berulang.
//!
//! Setiap kasus **reproducible**: seed selalu dicatat. Tidak ada test
//! hardcode — fuzz menghasilkan `TestDef` dinamis yang dideklarasikan
//! seperti test biasa (bukan `const TEST_001`).

use utbh_core::{Rng, TestDef};
use utbh_test::run;

/// Konfigurasi fuzz run.
pub struct FuzzConfig {
    pub seed: u64,
    pub iterations: usize,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        FuzzConfig {
            seed: 0x5EED,
            iterations: 64,
        }
    }
}

/// Satu kasus fuzz.
#[derive(Debug, Clone)]
pub struct FuzzCase {
    pub seed: u64,
    pub def: TestDef,
    /// PASS/FAIL dari test engine.
    pub passed: bool,
    /// Kasus minimal yang masih FAIL (hasil minimizer), kalau ada.
    pub minimized: Option<TestDef>,
    pub error: Option<String>,
}

/// Hasil satu fuzz run.
pub struct FuzzReport {
    pub cases: Vec<FuzzCase>,
    pub passed: usize,
    pub failed: usize,
}

impl FuzzReport {
    pub fn minimized_count(&self) -> usize {
        self.cases.iter().filter(|c| c.minimized.is_some()).count()
    }
}

/// Pool operasi yang di-fuzz. Instruksi + pola akses + ukuran berbeda-beda.
const OPS: &[&str] = &[
    "add",
    "sub",
    "mul",
    "fma",
    "matmul",
    "reduce_sum",
    "compare",
    "copy",
    "mem_seq",
    "mem_rand",
];

const DTYPES: &[&str] = &["f32", "u64", "i64"];

/// Kategori fuzz target → scope operasi (sesuai `utbh fuzz <suite>`).
fn scope(suite: &str) -> Vec<&'static str> {
    match suite {
        "cpu" | "cpu.*" => vec!["add", "sub", "mul", "fma", "reduce_sum", "compare"],
        "memory" | "cache" => vec!["copy", "mem_seq", "mem_rand", "reduce_sum"],
        "gpu" => vec!["add", "mul", "fma", "matmul"],
        "soc" | "interconnect" | "noc" => vec!["copy", "mem_seq", "mem_rand", "matmul"],
        _ => OPS.to_vec(),
    }
}

/// Fuzz satu suite: generate workload acak → test → kalau FAIL, minimalkan.
pub fn fuzz(suite: &str, cfg: &FuzzConfig) -> FuzzReport {
    let ops = scope(suite);
    let mut rng = Rng::new(cfg.seed);
    let mut cases = Vec::with_capacity(cfg.iterations);
    let mut passed = 0usize;
    let mut failed = 0usize;

    for _ in 0..cfg.iterations {
        let case_seed = rng.next_u64();
        let mut case_rng = Rng::new(case_seed);
        let def = gen_def(&ops, &mut case_rng, case_seed);
        let outcome = run(&def);
        let ok = matches!(outcome.status, utbh_core::Status::Pass);
        if ok {
            passed += 1;
        } else {
            failed += 1;
        }

        let minimized = if ok { None } else { minimize(&def) };
        cases.push(FuzzCase {
            seed: case_seed,
            def,
            passed: ok,
            minimized,
            error: outcome.error,
        });
    }

    FuzzReport {
        cases,
        passed,
        failed,
    }
}

/// Generate satu test definition acak (deklaratif, sama seperti test suite).
fn gen_def(ops: &[&str], rng: &mut Rng, seed: u64) -> TestDef {
    let r1 = rng.next_u64();
    let r2 = rng.next_u64();
    let r3 = rng.next_u64();
    let r4 = rng.next_u64();

    let op = ops[(r1 as usize) % ops.len()];
    let dtype = DTYPES[(r2 as usize) % DTYPES.len()];
    // TOML integer adalah `i64`; mask seed supaya selalu muat (reproduksi tetap deterministik).
    let toml_seed = seed.rem_euclid(i64::MAX as u64);
    // Ukuran acak: 1 .. 1<<20, dengan bias kecil untuk menemukan edge case.
    let elements = 1 + (r3 as usize % (1 << 20));
    let mode = if r4 % 3 == 0 { "exact" } else { "approx" };
    // matmul/fma butuh f32 (TOML `type` harfiah).
    let dtype = if matches!(op, "matmul" | "fma") {
        "f32"
    } else {
        dtype
    };

    let toml_str = format!(
        r#"
[test]
name = "fuzz_{}_{}"
category = "{}.fuzz"

[input]
elements = {}
type = "{}"
seed = {}

[operation]
op = "{}"

[validation]
mode = "{}"
tolerance = 1e-4

[benchmark]
metrics = []
iterations = 1
"#,
        op,
        seed,
        suite_category(op),
        elements,
        dtype,
        toml_seed,
        op,
        mode
    );
    toml::from_str(&toml_str).expect("generated fuzz def valid")
}

fn suite_category(op: &str) -> &'static str {
    match op {
        "copy" | "mem_seq" | "mem_rand" => "memory",
        "matmul" => "gpu",
        _ => "cpu",
    }
}

/// Minimizer: kecilkan input secara binary-search-like sampai FAIL tetap
/// terjadi → kasus minimal yang masih mereproduksi bug.
fn minimize(failing: &TestDef) -> Option<TestDef> {
    let mut lo = 1usize;
    let mut best: Option<TestDef> = None;
    let mut hi = failing.input.elements;

    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let mut candidate = failing.clone();
        candidate.input.elements = mid;
        let out = run(&candidate);
        if matches!(out.status, utbh_core::Status::Fail) {
            best = Some(candidate);
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzz_runs_deterministic() {
        let cfg = FuzzConfig {
            seed: 42,
            iterations: 24,
        };
        let a = fuzz("cpu", &cfg);
        let b = fuzz("cpu", &cfg);
        assert_eq!(a.cases.len(), 24);
        let seeds_a: Vec<u64> = a.cases.iter().map(|c| c.seed).collect();
        let seeds_b: Vec<u64> = b.cases.iter().map(|c| c.seed).collect();
        assert_eq!(seeds_a, seeds_b, "seed harus reproducible");
        assert_eq!(a.passed + a.failed, 24);
    }

    #[test]
    fn generated_defs_are_valid_toml() {
        let cfg = FuzzConfig {
            seed: 7,
            iterations: 16,
        };
        let r = fuzz("memory", &cfg);
        for c in &r.cases {
            assert!(!c.def.test.name.is_empty());
            assert!(!c.def.validation.mode.is_empty());
        }
    }

    #[test]
    fn scope_filters_ops() {
        let ops = scope("memory");
        assert!(ops.contains(&"mem_seq"));
        assert!(!ops.contains(&"matmul"));
    }
}
