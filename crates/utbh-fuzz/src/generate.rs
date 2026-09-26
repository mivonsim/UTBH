//! Generator test definition acak — deklaratif, sama seperti file suite.

use crate::config::{DTYPES, OPS};
use utbh_core::{Rng, TestDef};

/// Generate satu test definition acak (TOML in-memory).
///
/// `seed` = seed per kasus (reproducible). CATATAN: TOML integer = `i64`,
/// jadi seed di-mask ke rentang i64 saat ditulis.
pub fn gen_def(suite: &str, ops: &[&str], rng: &mut Rng, seed: u64) -> TestDef {
    let r1 = rng.next_u64();
    let r2 = rng.next_u64();
    let r3 = rng.next_u64();
    let r4 = rng.next_u64();

    let ops = if ops.is_empty() { OPS } else { ops };
    let op = ops[(r1 as usize) % ops.len()];
    let dtype = DTYPES[(r2 as usize) % DTYPES.len()];
    let toml_seed = seed.rem_euclid(i64::MAX as u64);
    // Ukuran acak: 1 .. 1<<20.
    let mut elements = 1 + (r3 as usize % (1 << 20));
    // matmul = O(n³) dari `elements` — batasi supaya fuzz tetap interaktif
    // (n ≤ 128 → 4096 elemen → ~4 juta FLOP per iterasi).
    if op == "matmul" {
        elements = elements.min(4096);
    }
    let mode = if r4 % 3 == 0 { "exact" } else { "approx" };
    // matmul/fma butuh f32; atomic_add butuh u64 (fetch_add atomik).
    let dtype = if matches!(op, "matmul" | "fma") {
        "f32"
    } else {
        dtype
    };
    let dtype = if op == "atomic_add" { "u64" } else { dtype };
    // Floating point non-associatif: urutan akumulasi / kontrak FMA berbeda
    // antara jalur workload dan reference → `exact` = false positive, bukan
    // bug hardware. Kasus ini wajib `approx`.
    let mode = if dtype == "f32" && matches!(op, "reduce_sum" | "matmul" | "fma" | "parallel_sum") {
        "approx"
    } else {
        mode
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
        suite_category(op, suite),
        elements,
        dtype,
        toml_seed,
        op,
        mode
    );
    toml::from_str(&toml_str).expect("generated fuzz def valid")
}

fn suite_category(op: &str, suite: &str) -> String {
    let fallback = match op {
        "copy" | "mem_seq" | "mem_rand" => "memory",
        "matmul" => "gpu",
        "atomic_add" | "parallel_sum" => "cpu",
        _ => "cpu",
    };
    // Suite eksplisit menang (report per suite akurat).
    match suite {
        "all" | "universal" | "" => fallback.to_string(),
        s => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::scope;

    #[test]
    fn generated_defs_are_valid_toml() {
        let mut rng = Rng::new(7);
        for i in 0..16u64 {
            let d = gen_def("cpu", &scope("cpu"), &mut rng, i.wrapping_mul(0x9E37));
            assert!(!d.test.name.is_empty());
            assert!(!d.validation.mode.is_empty());
        }
    }

    #[test]
    fn seed_deterministic() {
        let mk = || {
            let mut rng = Rng::new(99);
            gen_def("memory", &scope("memory"), &mut rng, 12345)
        };
        assert_eq!(mk().test.name, mk().test.name);
        assert_eq!(mk().input.seed, mk().input.seed);
    }
}
