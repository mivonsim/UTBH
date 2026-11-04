//! Perbandingan dua hasil run — §11 design: VM vs FPGA vs ASIC.
//!
//! Satu test definition yang sama dijalankan di environment berbeda
//! (fast VM / cycle VM / RTL / FPGA / silicon), hasilnya dibandingkan di sini:
//!
//! ```text
//! vector_add.utbh   VM: PASS 1.23 ns   →   FPGA: PASS 1.31 ns   →   ASIC: PASS 0.94 ns
//! ```
//!
//! Bandingkan: status PASS/FAIL, latensi (delta relatif), dan drift metrik.

use utbh_core::{RunResult, Status};

/// Satu baris perbandingan.
pub struct Row {
    pub test: String,
    pub category: String,
    pub status_a: Status,
    pub status_b: Status,
    pub latency_a: Option<f64>,
    pub latency_b: Option<f64>,
    pub throughput_a: Option<f64>,
    pub throughput_b: Option<f64>,
    pub bandwidth_a: Option<f64>,
    pub bandwidth_b: Option<f64>,
}

/// Hasil perbandingan dua run.
pub struct Comparison {
    pub run_a: String,
    pub run_b: String,
    /// Schema versi masing-masing run — beda versi = tidak sebanding.
    pub schema_a: u32,
    pub schema_b: u32,
    pub rows: Vec<Row>,
    /// Test yang PASS di A tapi FAIL di B (regresi).
    pub regressions: Vec<String>,
    /// Test yang FAIL di A tapi PASS di B.
    pub recoveries: Vec<String>,
    /// Test yang hanya ada di salah satu run.
    pub only_a: Vec<String>,
    pub only_b: Vec<String>,
}

impl Comparison {
    fn avg_delta(&self, pick: impl Fn(&Row) -> (Option<f64>, Option<f64>)) -> Option<f64> {
        let pairs: Vec<(f64, f64)> = self
            .rows
            .iter()
            .filter_map(|r| {
                let (a, b) = pick(r);
                Some((a?, b?))
            })
            .filter(|(a, b)| *a > 0.0 && b.is_finite())
            .collect();
        if pairs.is_empty() {
            return None;
        }
        let sum: f64 = pairs.iter().map(|(a, b)| (b / a - 1.0) * 100.0).sum();
        Some(sum / pairs.len() as f64)
    }

    /// Delta latensi rata-rata (b/a - 1), dalam persen. `None` bila tak ada pasangan.
    pub fn avg_latency_delta_pct(&self) -> Option<f64> {
        self.avg_delta(|r| (r.latency_a, r.latency_b))
    }

    /// Delta throughput rata-rata; positif = kandidat lebih cepat.
    pub fn avg_throughput_delta_pct(&self) -> Option<f64> {
        self.avg_delta(|r| (r.throughput_a, r.throughput_b))
    }

    /// Delta bandwidth rata-rata; positif = kandidat lebih cepat.
    pub fn avg_bandwidth_delta_pct(&self) -> Option<f64> {
        self.avg_delta(|r| (r.bandwidth_a, r.bandwidth_b))
    }
}

fn metric_value(r: &RunResult, test: &str, metric: &str) -> Option<f64> {
    r.outcomes
        .iter()
        .find(|o| o.name == test)?
        .metric(metric)
        .map(|m| m.value)
}

fn status_of(r: &RunResult, test: &str) -> Option<Status> {
    r.outcomes.iter().find(|o| o.name == test).map(|o| o.status)
}

/// Bandingkan dua `RunResult` (urutan: A = baseline, B = kandidat).
pub fn compare(a: &RunResult, b: &RunResult) -> Comparison {
    let mut rows = Vec::new();
    let mut regressions = Vec::new();
    let mut recoveries = Vec::new();
    let mut only_a = Vec::new();

    for oa in &a.outcomes {
        match status_of(b, &oa.name) {
            Some(sb) => {
                if oa.status == Status::Pass && sb == Status::Fail {
                    regressions.push(oa.name.clone());
                }
                if oa.status == Status::Fail && sb == Status::Pass {
                    recoveries.push(oa.name.clone());
                }
                rows.push(Row {
                    test: oa.name.clone(),
                    category: oa.category.clone(),
                    status_a: oa.status,
                    status_b: sb,
                    latency_a: metric_value(a, &oa.name, "latency"),
                    latency_b: metric_value(b, &oa.name, "latency"),
                    throughput_a: metric_value(a, &oa.name, "throughput"),
                    throughput_b: metric_value(b, &oa.name, "throughput"),
                    bandwidth_a: metric_value(a, &oa.name, "bandwidth"),
                    bandwidth_b: metric_value(b, &oa.name, "bandwidth"),
                });
            }
            None => only_a.push(oa.name.clone()),
        }
    }

    let mut only_b = Vec::new();
    for ob in &b.outcomes {
        if status_of(a, &ob.name).is_none() {
            only_b.push(ob.name.clone());
        }
    }

    Comparison {
        run_a: a.run_id.clone(),
        run_b: b.run_id.clone(),
        schema_a: a.schema_version,
        schema_b: b.schema_version,
        rows,
        regressions,
        recoveries,
        only_a,
        only_b,
    }
}

/// Render tabel perbandingan untuk stdout / report.
pub fn render(c: &Comparison) -> String {
    let mut out = format!("UTBH Compare\n  A = {}\n  B = {}\n\n", c.run_a, c.run_b);
    if c.schema_a != c.schema_b {
        out.push_str(&format!(
            "PERINGATAN: schema_version berbeda ({} vs {}) — hasil tidak sebanding.\n\n",
            c.schema_a, c.schema_b
        ));
    }
    out.push_str(&format!(
        "{:<28} {:<14} {:<7} {:<7} {:>14} {:>14} {:>10}\n",
        "TEST", "CATEGORY", "A", "B", "LAT_A", "LAT_B", "DELTA"
    ));
    out.push_str(&"-".repeat(98));
    out.push('\n');

    for r in &c.rows {
        let fmt = |s: Status| match s {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
        };
        let (la, lb) = (r.latency_a, r.latency_b);
        let delta = match (la, lb) {
            (Some(a), Some(b)) if a > 0.0 => format!("{:+.1}%", (b / a - 1.0) * 100.0),
            _ => "-".to_string(),
        };
        out.push_str(&format!(
            "{:<28} {:<14} {:<7} {:<7} {:>14} {:>14} {:>10}\n",
            r.test,
            r.category,
            fmt(r.status_a),
            fmt(r.status_b),
            la.map(|v| format!("{:.3} ns", v))
                .unwrap_or_else(|| "-".into()),
            lb.map(|v| format!("{:.3} ns", v))
                .unwrap_or_else(|| "-".into()),
            delta
        ));
    }

    out.push('\n');
    if let Some(d) = c.avg_latency_delta_pct() {
        out.push_str(&format!("Avg latency delta (B vs A): {:+.1}%\n", d));
    }
    if let Some(d) = c.avg_throughput_delta_pct() {
        out.push_str(&format!(
            "Avg throughput delta (B vs A): {:+.1}% (positif = lebih cepat)\n",
            d
        ));
    }
    if let Some(d) = c.avg_bandwidth_delta_pct() {
        out.push_str(&format!(
            "Avg bandwidth delta (B vs A): {:+.1}% (positif = lebih cepat)\n",
            d
        ));
    }
    if !c.regressions.is_empty() {
        out.push_str(&format!(
            "REGRESSIONS (PASS→FAIL): {}\n",
            c.regressions.join(", ")
        ));
    }
    if !c.recoveries.is_empty() {
        out.push_str(&format!(
            "Recoveries (FAIL→PASS): {}\n",
            c.recoveries.join(", ")
        ));
    }
    if !c.only_a.is_empty() {
        out.push_str(&format!("Only in A: {}\n", c.only_a.join(", ")));
    }
    if !c.only_b.is_empty() {
        out.push_str(&format!("Only in B: {}\n", c.only_b.join(", ")));
    }
    if c.regressions.is_empty() && c.recoveries.is_empty() {
        out.push_str("Status: tidak ada perubahan PASS/FAIL.\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use utbh_core::{HardwareReport, Metric, TestOutcome};

    fn outcome(name: &str, status: Status, latency: Option<f64>) -> TestOutcome {
        TestOutcome {
            name: name.into(),
            category: "cpu".into(),
            status,
            duration_ns: 0,
            validation: None,
            metrics: latency
                .map(|v| {
                    vec![Metric {
                        name: "latency".into(),
                        value: v,
                        unit: "ns".into(),
                    }]
                })
                .unwrap_or_default(),
            error: None,
        }
    }

    fn run(id: &str, outcomes: Vec<TestOutcome>) -> RunResult {
        RunResult::new(
            id,
            "run",
            "universal",
            HardwareReport::default(),
            outcomes,
            None,
        )
    }

    #[test]
    fn detects_regression_and_delta() {
        let a = run(
            "vm",
            vec![
                outcome("vadd", Status::Pass, Some(1.0)),
                outcome("mmul", Status::Pass, Some(2.0)),
            ],
        );
        let b = run(
            "asic",
            vec![
                outcome("vadd", Status::Pass, Some(0.5)),
                outcome("mmul", Status::Fail, None),
            ],
        );
        let c = compare(&a, &b);
        assert_eq!(c.regressions, vec!["mmul"]);
        assert!(c.recoveries.is_empty());
        let d = c.avg_latency_delta_pct();
        assert_eq!(d, Some(-50.0), "latensi turun 50% (0.5 vs 1.0)");
        let text = render(&c);
        assert!(text.contains("REGRESSIONS"));
        assert!(text.contains("-50.0%"));
    }

    #[test]
    fn schema_mismatch_is_reported() {
        let a = run("old", vec![]);
        let mut b = run("new", vec![]);
        b.schema_version = 2;
        let c = compare(&a, &b);
        assert_ne!(c.schema_a, c.schema_b);
        assert!(render(&c).contains("schema_version berbeda"));
    }

    #[test]
    fn only_in_one_side() {
        let a = run("vm", vec![outcome("x", Status::Pass, None)]);
        let b = run("fpga", vec![outcome("y", Status::Pass, None)]);
        let c = compare(&a, &b);
        assert_eq!(c.only_a, vec!["x"]);
        assert_eq!(c.only_b, vec!["y"]);
        assert_eq!(c.avg_latency_delta_pct(), None);
    }

    #[test]
    fn reports_metric_drift_not_only_latency() {
        let metric = |name: &str, value: f64| Metric {
            name: name.into(),
            value,
            unit: "x".into(),
        };
        let mut a = run("vm", vec![outcome("vadd", Status::Pass, Some(1.0))]);
        a.outcomes[0].metrics.push(metric("throughput", 100.0));
        a.outcomes[0].metrics.push(metric("bandwidth", 10.0));
        let mut b = run("asic", vec![outcome("vadd", Status::Pass, Some(1.0))]);
        b.outcomes[0].metrics.push(metric("throughput", 50.0));
        b.outcomes[0].metrics.push(metric("bandwidth", 5.0));

        let c = compare(&a, &b);
        assert_eq!(c.avg_throughput_delta_pct(), Some(-50.0));
        assert_eq!(c.avg_bandwidth_delta_pct(), Some(-50.0));
        let text = render(&c);
        assert!(text.contains("Avg throughput delta"), "{}", text);
        assert!(text.contains("Avg bandwidth delta"), "{}", text);
    }

    #[test]
    fn stable_run_reports_no_change() {
        let a = run("vm", vec![outcome("x", Status::Pass, Some(1.0))]);
        let b = run("rtl", vec![outcome("x", Status::Pass, Some(1.0))]);
        let c = compare(&a, &b);
        assert!(c.regressions.is_empty());
        assert_eq!(c.avg_latency_delta_pct(), Some(0.0));
        assert!(render(&c).contains("tidak ada perubahan"));
    }
}
