//! Jalankan test berulang: deteksi kegagalan kumulatif + degradasi latensi.

use crate::config::StressConfig;
use utbh_api::HardwareApi;
use utbh_core::{Status, TestDef, TestOutcome, ValidationInfo};
use utbh_validation::validate;

/// Satu window pengamatan latensi.
pub struct Window {
    pub index: usize,
    pub avg_ns: f64,
    pub min_ns: f64,
    pub max_ns: f64,
}

/// Hasil stress satu test.
pub struct StressReport {
    pub test: String,
    pub category: String,
    pub iterations: usize,
    pub pass: usize,
    pub fail: usize,
    /// Iterasi pertama FAIL, kalau ada — sinyal degradasi/race.
    pub first_fail_at: Option<usize>,
    /// Window latensi (untuk deteksi throttling/degradasi).
    pub windows: Vec<Window>,
    /// Ratio avg window terakhir vs window pertama. >1 = melambat.
    pub degradation_ratio: Option<f64>,
    pub error: Option<String>,
}

impl StressReport {
    pub fn stable(&self) -> bool {
        self.fail == 0 && self.error.is_none()
    }

    /// Ambang degradasi yang dianggap mencurigakan (>20% melambat).
    pub fn degraded(&self) -> bool {
        self.degradation_ratio.map(|r| r > 1.2).unwrap_or(false)
    }
}

/// Stress satu test: `iterations` × validate + ukur latensi.
pub fn stress(def: &TestDef, api: &dyn HardwareApi, cfg: &StressConfig) -> StressReport {
    if let Err(e) = cfg.validate() {
        return StressReport {
            test: def.test.name.clone(),
            category: def.test.category.clone(),
            iterations: 0,
            pass: 0,
            fail: 0,
            first_fail_at: None,
            windows: Vec::new(),
            degradation_ratio: None,
            error: Some(e),
        };
    }

    let mut latencies = Vec::with_capacity(cfg.iterations);
    let mut pass = 0usize;
    let mut fail = 0usize;
    let mut first_fail_at = None;
    let mut first_error = None;

    for i in 0..cfg.iterations {
        let t0 = api.timer_ns();
        let outcome = validate(def);
        let dt = api.timer_ns().saturating_sub(t0);
        latencies.push(dt as f64);

        match outcome {
            Ok(v) if v.matched => pass += 1,
            Ok(v) => {
                fail += 1;
                first_fail_at.get_or_insert(i);
                if first_error.is_none() {
                    first_error = Some(format!(
                        "iterasi {}: mismatches={} first_mismatch={:?}",
                        i, v.mismatches, v.first_mismatch
                    ));
                }
            }
            Err(e) => {
                fail += 1;
                first_fail_at.get_or_insert(i);
                if first_error.is_none() {
                    first_error = Some(format!("iterasi {}: {}", i, e));
                }
            }
        }
    }

    let windows = build_windows(&latencies, cfg.window);
    let degradation_ratio = ratio(&windows);

    StressReport {
        test: def.test.name.clone(),
        category: def.test.category.clone(),
        iterations: cfg.iterations,
        pass,
        fail,
        first_fail_at,
        windows,
        degradation_ratio,
        error: first_error,
    }
}

/// Potong latensi jadi window-window berukuran `window`.
fn build_windows(latencies: &[f64], window: usize) -> Vec<Window> {
    latencies
        .chunks(window.max(1))
        .enumerate()
        .map(|(index, chunk)| {
            let sum: f64 = chunk.iter().sum();
            Window {
                index,
                avg_ns: sum / chunk.len() as f64,
                min_ns: chunk.iter().cloned().fold(f64::INFINITY, f64::min),
                max_ns: chunk.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            }
        })
        .collect()
}

/// Avg window terakhir ÷ window pertama.
fn ratio(windows: &[Window]) -> Option<f64> {
    let first = windows.first()?;
    let last = windows.last()?;
    if windows.len() < 2 || first.avg_ns <= 0.0 {
        return None;
    }
    Some(last.avg_ns / first.avg_ns)
}

/// Konversi stress report jadi `TestOutcome` (untuk `.utbh-result.json`).
///
/// PASS = stabil (tanpa FAIL). Metrik latensi dilaporkan terpisah —
/// degradasi bukan correctness failure.
pub fn to_outcome(r: &StressReport) -> TestOutcome {
    use utbh_core::Metric;

    let mut metrics = Vec::new();
    if let Some(first) = r.windows.first() {
        metrics.push(Metric {
            name: "stress_first_window_ns".into(),
            value: first.avg_ns,
            unit: "ns".into(),
        });
    }
    if let Some(last) = r.windows.last() {
        metrics.push(Metric {
            name: "stress_last_window_ns".into(),
            value: last.avg_ns,
            unit: "ns".into(),
        });
    }
    if let Some(ratio) = r.degradation_ratio {
        metrics.push(Metric {
            name: "stress_degradation_ratio".into(),
            value: ratio,
            unit: "x".into(),
        });
    }
    metrics.push(Metric {
        name: "stress_pass_rate".into(),
        value: if r.iterations > 0 {
            r.pass as f64 / r.iterations as f64
        } else {
            0.0
        },
        unit: "ratio".into(),
    });

    let status = if r.stable() {
        Status::Pass
    } else {
        Status::Fail
    };
    let error = r.error.clone().or_else(|| {
        r.degraded().then(|| {
            format!(
                "degradasi latensi: ratio {:.2}x (>1.2x)",
                r.degradation_ratio.unwrap_or(0.0)
            )
        })
    });

    TestOutcome {
        name: r.test.clone(),
        category: r.category.clone(),
        status,
        duration_ns: 0,
        validation: Some(ValidationInfo {
            mode: "stress".into(),
            matched: r.stable(),
            mismatches: r.fail,
            first_mismatch: r.first_fail_at,
            max_rel_error: 0.0,
        }),
        metrics,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> TestDef {
        toml::from_str(
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
mode = "exact"
"#,
        )
        .unwrap()
    }

    #[test]
    fn stable_test_passes() {
        let api = utbh_api::MivonHardwareApi::new();
        let cfg = StressConfig {
            iterations: 30,
            window: 10,
        };
        let r = stress(&def(), &api, &cfg);
        assert!(r.stable(), "err={:?}", r.error);
        assert_eq!(r.pass, 30);
        assert_eq!(r.windows.len(), 3);
        let o = to_outcome(&r);
        assert_eq!(o.status, Status::Pass);
        assert!(!o.metrics.is_empty());
    }

    #[test]
    fn windows_partition_all_samples() {
        let lat = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let w = build_windows(&lat, 2);
        assert_eq!(w.len(), 3); // 2 + 2 + 1
        assert_eq!(w[0].avg_ns, 1.5);
        assert_eq!(w[0].min_ns, 1.0);
        assert_eq!(w[0].max_ns, 2.0);
        assert_eq!(w[2].avg_ns, 5.0);
    }

    #[test]
    fn ratio_needs_two_windows() {
        assert_eq!(ratio(&[]), None);
        let one = vec![Window {
            index: 0,
            avg_ns: 100.0,
            min_ns: 0.0,
            max_ns: 0.0,
        }];
        assert_eq!(ratio(&one), None);
        let two = vec![
            Window {
                index: 0,
                avg_ns: 100.0,
                min_ns: 0.0,
                max_ns: 0.0,
            },
            Window {
                index: 1,
                avg_ns: 150.0,
                min_ns: 0.0,
                max_ns: 0.0,
            },
        ];
        assert_eq!(ratio(&two), Some(1.5));
    }

    #[test]
    fn outcome_reports_degradation_as_error() {
        let r = StressReport {
            test: "t".into(),
            category: "cpu".into(),
            iterations: 40,
            pass: 40,
            fail: 0,
            first_fail_at: None,
            windows: vec![
                Window {
                    index: 0,
                    avg_ns: 100.0,
                    min_ns: 0.0,
                    max_ns: 0.0,
                },
                Window {
                    index: 1,
                    avg_ns: 150.0,
                    min_ns: 0.0,
                    max_ns: 0.0,
                },
            ],
            degradation_ratio: Some(1.5),
            error: None,
        };
        assert!(r.degraded());
        let o = to_outcome(&r);
        assert_eq!(
            o.status,
            Status::Pass,
            "degradasi bukan correctness failure"
        );
        assert!(o.error.as_deref().unwrap_or("").contains("degradasi"));
    }

    #[test]
    fn invalid_config_reports_error() {
        let api = utbh_api::MivonHardwareApi::new();
        let r = stress(
            &def(),
            &api,
            &StressConfig {
                iterations: 0,
                window: 10,
            },
        );
        assert!(!r.stable());
        assert!(r.error.is_some());
    }
}
