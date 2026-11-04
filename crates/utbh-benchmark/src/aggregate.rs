//! Agregasi sample → daftar metrik + hitung FLOPS per operasi.

use crate::sample::Sample;
use utbh_core::{Metric, TestDef};
use utbh_workload::Op;

/// FLOPS per iterasi (0 kalau bukan operasi floating-point aritmetika).
///
/// GFLOPS hanya bermakna untuk f32. Operasi integer dan `compare` tidak boleh
/// dilaporkan sebagai FLOP — itu metrik yang menyesatkan.
pub fn count_flops(def: &TestDef, elements: u64) -> u64 {
    if def.input.dtype != "f32" {
        return 0;
    }
    match Op::parse(&def.operation.op) {
        Ok(Op::Add) | Ok(Op::Sub) | Ok(Op::Mul) | Ok(Op::ReduceSum) | Ok(Op::ParallelSum) => {
            elements
        }
        Ok(Op::Fma) => elements.saturating_mul(2),
        Ok(Op::MatMul) => {
            // n×n × n×n: 2·n³ FLOP
            let n = (elements as f64).sqrt().floor() as u64;
            2u64.saturating_mul(n.saturating_mul(n).saturating_mul(n))
        }
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
            name: "cycles".into(),
            value: avg_cycles,
            unit: "cycles".into(),
        },
    ];

    // Durasi 0 = di bawah resolusi timer. Rate tidak bisa dihitung tanpa
    // membagi nol; jangan fabrikasi angka raksasa dari EPSILON.
    if avg_ns > 0.0 {
        let sec = avg_ns / 1e9;
        metrics.insert(
            2,
            Metric {
                name: "throughput".into(),
                value: elements / sec,
                unit: "elem/s".into(),
            },
        );
        metrics.insert(
            3,
            Metric {
                name: "bandwidth".into(),
                value: bytes / sec / 1e9,
                unit: "GB/s".into(),
            },
        );
        if flops > 0.0 {
            metrics.push(Metric {
                name: "gflops".into(),
                value: flops / sec / 1e9,
                unit: "GFLOPS".into(),
            });
        }
    }

    let wanted = &def.benchmark.metrics;
    if wanted.is_empty() {
        return metrics;
    }
    metrics.retain(|m| wanted.contains(&m.name));
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
    fn integer_add_has_no_flops() {
        let d = toml::from_str::<TestDef>(
            r#"
[test]
name = "i"
category = "cpu.integer"
[input]
elements = 128
type = "u64"
[operation]
op = "add"
[validation]
mode = "exact"
"#,
        )
        .unwrap();
        assert_eq!(count_flops(&d, 128), 0);
    }

    #[test]
    fn compare_has_no_flops() {
        let d = toml::from_str::<TestDef>(
            r#"
[test]
name = "c"
category = "cpu"
[input]
elements = 64
type = "f32"
[operation]
op = "compare"
[validation]
mode = "exact"
"#,
        )
        .unwrap();
        assert_eq!(count_flops(&d, 64), 0);
    }

    #[test]
    fn fma_counts_two_flops_per_element() {
        let d = toml::from_str::<TestDef>(
            r#"
[test]
name = "f"
category = "cpu"
[input]
elements = 32
type = "f32"
[operation]
op = "fma"
[validation]
mode = "approx"
"#,
        )
        .unwrap();
        assert_eq!(count_flops(&d, 32), 64);
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
    fn zero_duration_does_not_fabricate_rates() {
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
        let samples = vec![Sample {
            ns: 0,
            cycles: 0,
            elements: 8,
            bytes: 96,
            flops: 8,
        }];
        let m = aggregate(&d, &samples);
        let names: Vec<&str> = m.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["latency", "latency_min", "cycles"],
            "durasi nol tidak boleh menghasilkan rate"
        );
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
        let samples = vec![Sample {
            ns: 100,
            cycles: 300,
            elements: 8,
            bytes: 32,
            flops: 8,
        }];
        let m = aggregate(&d, &samples);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].name, "latency");
    }
}
