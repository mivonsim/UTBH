//! Pengukuran satu iterasi benchmark via [`HardwareApi`].

use utbh_api::HardwareApi;
use utbh_core::TestDef;
use utbh_workload::execute;

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
#[derive(Debug, Clone)]
pub struct Sample {
    pub ns: u64,
    pub cycles: u64,
    pub elements: u64,
    pub bytes: u64,
    pub flops: u64,
}

/// Jalankan benchmark untuk satu test definition.
///
/// Mengembalikan daftar metrik terpisah — bukan skor agregat.
/// Kosong kalau workload gagal (Layer 4 yang lapor FAIL).
pub fn run(def: &TestDef, api: &dyn HardwareApi, cfg: &BenchConfig) -> Vec<utbh_core::Metric> {
    let iterations = if def.benchmark.iterations > 0 {
        def.benchmark.iterations.max(1)
    } else {
        cfg.iterations.max(1)
    };

    // Warmup: buang hasil, hanya agar cache/pipeline stabil.
    for _ in 0..cfg.warmup {
        if execute(def).is_err() {
            return Vec::new();
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
            flops: crate::aggregate::count_flops(def, out.elements),
        });
    }

    crate::aggregate::aggregate(def, &samples)
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
        let api = utbh_api::MivonHardwareApi::new();
        let metrics = run(
            &def(),
            &api,
            &BenchConfig {
                warmup: 1,
                iterations: 3,
            },
        );
        let names: Vec<&str> = metrics.iter().map(|m| m.name.as_str()).collect();
        for want in ["latency", "throughput", "bandwidth", "cycles", "gflops"] {
            assert!(
                names.contains(&want),
                "hilang: {} (dapat: {:?})",
                want,
                names
            );
        }
        assert!(names.len() >= 4, "harus multi-metrik, bukan skor tunggal");
    }

    #[test]
    fn metric_filter_respected() {
        let mut d = def();
        d.benchmark.metrics = vec!["latency".into()];
        let api = utbh_api::MivonHardwareApi::new();
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
}
