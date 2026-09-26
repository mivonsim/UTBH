//! Orkestrasi fuzz run: generate → test → (kalau FAIL) minimize.

use crate::config::{scope, FuzzConfig};
use crate::generate::gen_def;
use crate::minimize::minimize;
use utbh_core::{Rng, Status, TestDef};

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
        let def = gen_def(suite, &ops, &mut case_rng, case_seed);
        let outcome = utbh_test::run(&def);
        let ok = matches!(outcome.status, Status::Pass);
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
    fn every_case_has_seed() {
        let cfg = FuzzConfig {
            seed: 7,
            iterations: 8,
        };
        let r = fuzz("memory", &cfg);
        for c in &r.cases {
            assert_ne!(c.seed, 0);
        }
    }
}
