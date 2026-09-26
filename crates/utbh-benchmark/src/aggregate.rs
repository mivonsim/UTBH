//! Agregasi sample → daftar metrik + hitung FLOPS per operasi.

use crate::sample::Sample;
use utbh_core::{Metric, TestDef};
use utbh_workload::Op;

/// FLOPS per iterasi (0 kalau bukan operasi aritmetika).
pub fn count_flops(def: &TestDef, elements: u64) -> u64 {
    match Op::parse(&def.operation.op) {
        Ok(Op::Add) | Ok(Op::Sub) | Ok(Op::Mul) => elements,
        Ok(Op::Fma) => elements.saturating_mul(2),
        Ok(Op::MatMul) => {
            // n×n × n×n: 2·n³ FLOP
            let n = (elements as f64).sqrt().floor() as u64;
            2u64.saturating_mul(n.saturating_mul(n).saturating_mul(n))
        }
        Ok(Op::ReduceSum) | Ok(Op::Compare) | Ok(Op::AtomicAdd) | Ok(Op::ParallelSum) => elements,
        _ => 0,
    }
}

/// Ubah sample jadi metrik. Filter sesuai `[benchmark].metrics` bila diisi.
pub fn aggregate(def: &TestDef, samples: &[Sample]) -> Vec<Metric> {
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
        Metric { name: "latency".into(), value: avg_ns, unit: "ns".into() },
        Metric { name: "latency_min".into(), value: min_ns, unit: "ns".into() },
        Metric { name: "throughput".into(), value: elements / sec, unit: "elem/s".into() },
        Metric { name: "bandwidth".into(), value: bytes / sec / 1e9, unit: "GB/s".into() },
        Metric { name: "cycles".into(), value: avg_cycles, unit: "cycles".into() },
    ];

    if flops > 0.0 {
        metrics.push(Metric { name: "gflops".into(), value: flops / sec / 1e9, unit: "GFLOPS".into() });
    }

    let wanted = &def.benchmark.metrics;
    if wanted.is_empty() {
        return metrics;
    }
    metrics.retain(|m| wanted.iter().any(|w| m.name == *w));
    metrics
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn copy_has_no_flops() {
        let d = toml::from_str::<TestDef>(
            r#"
[test]
name = "c"
category = "memory"
[input]
elements = 64
type = "u64"
[operation]
op = "copy"
[validation]
mode = "exact"
"#,
        )
        .unwrap();
        assert_eq!(count_flops(&d, 64), 0);
    }

    #[test]
    fn empty_samples_no_metrics() {
        let d = toml::from_str::<TestDef>(
            r#"
[test]
name = "x"
category = "cpu"
[input]
elements = 8
type = "f32"
[operation]
op = "add"
[validation]
mode = "exact"
"#,
        )
        .unwrap();
        assert!(aggregate(&d, &[]).is_empty());
    }

    #[test]
    fn filter_keeps_only_requested() {
        let mut d = toml::from_str::<TestDef>(
            r#"
[test]
name = "x"
category = "cpu"
[input]
elements = 8
type = "f32"
[operation]
op = "add"
[validation]
mode = "exact"
"#,
        )
        .unwrap();
        d.benchmark.metrics = vec!["latency".into()];
        let samples = vec![Sample { ns: 100, cycles: 300, elements: 8, bytes: 32, flops: 8 }];
        let m = aggregate(&d, &samples);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].name, "latency");
    }
}
