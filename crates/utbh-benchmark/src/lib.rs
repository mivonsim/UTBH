//! `utbh-benchmark` — Layer 5: Benchmark Engine.
//!
//! Menjawab: **seberapa cepat?** (terpisah dari correctness = Layer 4).
//!
//! Metrik yang diukur:
//! - `latency` — ns per eksekusi workload
//! - `throughput` — elemen per detik
//! - `bandwidth` — GB/s (byte yang disentuh per detik)
//! - `gflops` — FLOPS untuk operasi aritmetika
//! - `cycles` — siklus CPU per eksekusi (via [`HardwareApi`])
//!
//! **Tidak pernah** menghasilkan satu angka skor tunggal sebagai satu-satunya
//! hasil. Semua metrik dilaporkan terpisah, bersama PASS/FAIL dari Layer 4.

use utbh_api::HardwareApi;
use utbh_core::{Metric, TestDef};
use utbh_workload::{execute, Op};

/// Konfigurasi pengukuran.
pub struct BenchConfig {
    /// Iterasi warmup (dibuang dari hasil).
    pub warmup: u32,
    /// Iterasi pengukuran.
    pub iterations: u32,
}

impl Default for BenchConfig {
    fn default() -> Self {
        BenchConfig {
            warmup: 3,
            iterations: 10,
        }
    }
}

/// Satu iterasi pengukuran.
struct Sample {
    ns: u64,
    cycles: u64,
    elements: u64,
    bytes: u64,
    flops: u64,
}

/// Jalankan benchmark untuk satu test definition.
///
/// Mengembalikan daftar metrik terpisah — bukan skor agregat.
pub fn run(def: &TestDef, api: &dyn HardwareApi, cfg: &BenchConfig) -> Vec<Metric> {
    let iterations = if def.benchmark.iterations > 0 {
        def.benchmark.iterations.max(1)
    } else {
        cfg.iterations.max(1)
    };

    // Warmup: buang hasil, hanya agar cache/pipeline stabil.
    for _ in 0..cfg.warmup {
        if execute(def).is_err() {
            return Vec::new(); // workload gagal → Layer 4 yang lapor FAIL
        }
    }

    let mut samples: Vec<Sample> = Vec::with_capacity(iterations as usize);
    for _ in 0..iterations {
        let c0 = api.cycles();
        let t0 = api.timer_ns();
        let Ok(out) = execute(def) else {
            return Vec::new();
        };
        let t1 = api.timer_ns();
        let c1 = api.cycles();
        samples.push(Sample {
            ns: t1.saturating_sub(t0),
            cycles: c1.saturating_sub(c0),
            elements: out.elements,
            bytes: out.bytes,
            flops: count_flops(def, out.elements),
        });
    }

    aggregate(def, &samples, cfg)
}

/// FLOPS per iterasi (0 kalau bukan operasi aritmetika).
fn count_flops(def: &TestDef, elements: u64) -> u64 {
    match Op::parse(&def.operation.op) {
        Ok(Op::Add) | Ok(Op::Sub) | Ok(Op::Mul) => elements,
        Ok(Op::Fma) => elements.saturating_mul(2),
        Ok(Op::MatMul) => {
            // n×n × n×n: 2·n³ FLOP
            let n = (elements as f64).sqrt().floor() as u64;
            2u64.saturating_mul(n.saturating_mul(n).saturating_mul(n))
        }
        Ok(Op::ReduceSum) | Ok(Op::Compare) => elements,
        _ => 0,
    }
}

fn aggregate(def: &TestDef, samples: &[Sample], cfg: &BenchConfig) -> Vec<Metric> {
    if samples.is_empty() {
        return Vec::new();
    }

    let n = samples.len() as f64;
    let avg_ns = samples.iter().map(|s| s.ns as f64).sum::<f64>() / n;
    let min_ns = samples.iter().map(|s| s.ns).min().unwrap_or(0) as f64;
    let avg_cycles = samples.iter().map(|s| s.cycles as f64).sum::<f64>() / n;

    let elements = samples[0].elements as f64;
    let bytes = samples[0].bytes as f64;
    let flops = samples[0].flops as f64;
    let sec = (avg_ns / 1e9).max(f64::EPSILON);

    let mut metrics = vec![
        Metric {
            name: "latency".into(),
            value: avg_ns,
            unit: "ns".into(),
        },
        Metric {
            name: "latency_min".into(),
            value: min_ns,
            unit: "ns".into(),
        },
        Metric {
            name: "throughput".into(),
            value: elements / sec,
            unit: "elem/s".into(),
        },
        Metric {
            name: "bandwidth".into(),
            value: bytes / sec / 1e9,
            unit: "GB/s".into(),
        },
        Metric {
            name: "cycles".into(),
            value: avg_cycles,
            unit: "cycles".into(),
        },
    ];

    if flops > 0.0 {
        metrics.push(Metric {
            name: "gflops".into(),
            value: flops / sec / 1e9,
            unit: "GFLOPS".into(),
        });
    }

    // Filter sesuai [benchmark].metrics bila diisi — kalau kosong, semua.
    let wanted = &def.benchmark.metrics;
    if wanted.is_empty() {
        return metrics;
    }
    metrics.retain(|m| {
        wanted.iter().any(|w| {
            m.name == *w
                || (w == "latency" && m.name == "latency")
                || (w == "gflops" && m.name == "gflops")
        })
    });
    let _ = cfg;
    metrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use utbh_api::MivonHardwareApi;

    fn def() -> TestDef {
        toml::from_str(
            r#"
[test]
name = "vector_add"
category = "cpu.vector"

[input]
elements = 65536
type = "f32"

[operation]
op = "add"

[validation]
mode = "exact"

[benchmark]
metrics = []
iterations = 3
"#,
        )
        .unwrap()
    }

    #[test]
    fn produces_all_metric_families_not_one_score() {
        let api = MivonHardwareApi::new();
        let metrics = run(
            &def(),
            &api,
            &BenchConfig {
                warmup: 1,
                iterations: 3,
            },
        );
        let names: Vec<&str> = metrics.iter().map(|m| m.name.as_str()).collect();
        for want in ["latency", "throughput", "bandwidth", "cycles"] {
            assert!(
                names.contains(&want),
                "hilang: {} (dapat: {:?})",
                want,
                names
            );
        }
        assert!(
            names.contains(&gflops_or_missing()),
            "gflops harus ada untuk op aritmetika"
        );
        assert!(names.len() >= 4, "harus multi-metrik, bukan skor tunggal");
    }

    fn gflops_or_missing() -> &'static str {
        "gflops"
    }

    #[test]
    fn metric_filter_respected() {
        let mut d = def();
        d.benchmark.metrics = vec!["latency".into()];
        let api = MivonHardwareApi::new();
        let metrics = run(
            &d,
            &api,
            &BenchConfig {
                warmup: 1,
                iterations: 2,
            },
        );
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "latency");
    }

    #[test]
    fn matmul_counts_flops() {
        let d = toml::from_str::<TestDef>(
            r#"
[test]
name = "m"
category = "cpu.matrix"
[input]
elements = 400
type = "f32"
[operation]
op = "matmul"
[validation]
mode = "approx"
"#,
        )
        .unwrap();
        assert_eq!(count_flops(&d, 400), 2 * 20 * 20 * 20);
    }
}
