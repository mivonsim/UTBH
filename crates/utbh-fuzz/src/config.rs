//! Konfigurasi fuzz + pembatasan scope operasi per suite.

/// Konfigurasi fuzz run.
pub struct FuzzConfig {
    pub seed: u64,
    pub iterations: usize,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        FuzzConfig { seed: 0x5EED, iterations: 64 }
    }
}

/// Pool operasi penuh.
pub const OPS: &[&str] = &[
    "add", "sub", "mul", "fma", "matmul", "reduce_sum", "compare", "copy", "mem_seq", "mem_rand",
];

pub const DTYPES: &[&str] = &["f32", "u64", "i64"];

/// Kategori fuzz target → scope operasi (sesuai `utbh fuzz <suite>`).
pub fn scope(suite: &str) -> Vec<&'static str> {
    match suite {
        "cpu" | "cpu.*" => {
            vec!["add", "sub", "mul", "fma", "reduce_sum", "compare", "atomic_add", "parallel_sum"]
        }
        "memory" | "cache" => vec!["copy", "mem_seq", "mem_rand", "reduce_sum", "parallel_sum"],
        "gpu" => vec!["add", "mul", "fma", "matmul"],
        "soc" | "interconnect" | "noc" => vec!["copy", "mem_seq", "mem_rand", "matmul"],
        _ => OPS.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_filters_ops() {
        let ops = scope("memory");
        assert!(ops.contains(&"mem_seq"));
        assert!(!ops.contains(&"matmul"));
    }

    #[test]
    fn default_config() {
        let c = FuzzConfig::default();
        assert_eq!(c.seed, 0x5EED);
        assert_eq!(c.iterations, 64);
    }
}
