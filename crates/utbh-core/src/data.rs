//! Data values + generator input deterministik.
//!
//! Workload (Layer 3) dan reference (Layer 6) **wajib** memakai [`inputs`] —
//! seed sama = kerja sama, input identik.

use crate::rng::{fnv1a, Rng};
use crate::testdef::TestDef;
use serde::{Deserialize, Serialize};

/// Data value yang dipakai workload & validator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Data {
    F32(Vec<f32>),
    U64(Vec<u64>),
    I64(Vec<i64>),
}

impl Data {
    pub fn len(&self) -> usize {
        match self {
            Data::F32(v) => v.len(),
            Data::U64(v) => v.len(),
            Data::I64(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dtype(&self) -> &'static str {
        match self {
            Data::F32(_) => "f32",
            Data::U64(_) => "u64",
            Data::I64(_) => "i64",
        }
    }

    pub fn elem_size(&self) -> usize {
        match self {
            Data::F32(_) => 4,
            Data::U64(_) | Data::I64(_) => 8,
        }
    }

    /// Representasi string satu elemen (untuk trace mismatch).
    pub fn fmt_at(&self, i: usize) -> Option<String> {
        match self {
            Data::F32(v) => v.get(i).map(|x| format!("{}", x)),
            Data::U64(v) => v.get(i).map(|x| x.to_string()),
            Data::I64(v) => v.get(i).map(|x| x.to_string()),
        }
    }
}

/// Pasangan input untuk operasi. `b` hanya untuk operasi dua-operand.
pub struct Inputs {
    pub a: Data,
    pub b: Option<Data>,
}

/// Input deterministik: seed = `[input].seed` atau hash nama test.
pub fn inputs(def: &TestDef) -> Inputs {
    let seed = def.input.seed.unwrap_or_else(|| fnv1a(&def.test.name));
    let n = def.input.elements.max(1);
    let a = gen_data(&def.input.dtype, seed, n);
    let needs_b = matches!(def.operation.op.as_str(), "add" | "sub" | "mul" | "matmul" | "fma" | "compare");
    let b = needs_b.then(|| gen_data(&def.input.dtype, seed ^ 0xA5A5_A5A5_A5A5_A5A5, n));
    Inputs { a, b }
}

pub fn gen_data(dtype: &str, seed: u64, n: usize) -> Data {
    match dtype {
        "u64" => Data::U64(gen_u64(seed, n)),
        "i64" => Data::I64(gen_i64(seed, n)),
        _ => Data::F32(gen_f32(seed, n)),
    }
}

pub fn gen_f32(seed: u64, n: usize) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    (0..n).map(|_| (rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32).collect()
}

pub fn gen_u64(seed: u64, n: usize) -> Vec<u64> {
    let mut rng = Rng::new(seed);
    (0..n).map(|_| rng.next_u64()).collect()
}

pub fn gen_i64(seed: u64, n: usize) -> Vec<i64> {
    let mut rng = Rng::new(seed);
    (0..n).map(|_| (rng.next_u64() >> 1) as i64).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdef::TestDef;

    fn sample() -> TestDef {
        toml::from_str(
            r#"
[test]
name = "vector_add"
category = "cpu.vector"
[input]
elements = 1024
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
    fn inputs_deterministic() {
        let def = sample();
        let a = inputs(&def);
        let b = inputs(&def);
        assert_eq!(a.a, b.a);
        assert_eq!(a.b, b.b);
        assert_eq!(a.a.len(), 1024);
        assert!(a.b.is_some());
    }

    #[test]
    fn binary_ops_need_two_inputs() {
        let def = sample();
        assert!(inputs(&def).b.is_some());
    }

    #[test]
    fn gen_dtype_dispatch() {
        assert!(matches!(gen_data("f32", 1, 4), Data::F32(_)));
        assert!(matches!(gen_data("u64", 1, 4), Data::U64(_)));
        assert!(matches!(gen_data("i64", 1, 4), Data::I64(_)));
    }
}
