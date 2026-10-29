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

/// Input untuk operasi unary, binary, atau FMA tiga-operand.
pub struct Inputs {
    pub a: Data,
    pub b: Option<Data>,
    pub c: Option<Data>,
}

/// Input deterministik: seed = `[input].seed` atau hash nama test.
pub fn inputs(def: &TestDef) -> Inputs {
    let seed = def.input.seed.unwrap_or_else(|| fnv1a(&def.test.name));
    let n = def.input.elements.max(1);
    let a = gen_data(&def.input.dtype, seed, n);
    let needs_b = matches!(
        def.operation.op.as_str(),
        "add"
            | "vector_add"
            | "sub"
            | "vector_sub"
            | "mul"
            | "vector_mul"
            | "matmul"
            | "matrix_mul"
            | "fma"
            | "vector_fma"
            | "compare"
    );
    let b = needs_b.then(|| gen_data(&def.input.dtype, seed ^ 0xA5A5_A5A5_A5A5_A5A5, n));
    let c = (def.operation.op == "fma" || def.operation.op == "vector_fma")
        .then(|| gen_data(&def.input.dtype, seed ^ 0x5A5A_5A5A_5A5A_5A5A, n));
    Inputs { a, b, c }
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
    (0..n)
        .map(|_| (rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32)
        .collect()
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
        assert_eq!(a.c, b.c);
        assert_eq!(a.a.len(), 1024);
        assert!(a.b.is_some());
        assert!(a.c.is_none());
    }

    #[test]
    fn binary_ops_need_two_inputs() {
        for op in [
            "add",
            "vector_add",
            "sub",
            "vector_sub",
            "mul",
            "vector_mul",
            "matmul",
            "matrix_mul",
            "fma",
            "vector_fma",
            "compare",
        ] {
            let mut def = sample();
            def.operation.op = op.into();
            assert!(inputs(&def).b.is_some(), "{op} must generate operand B");
        }
    }

    #[test]
    fn fma_inputs_include_a_deterministic_third_operand() {
        for op in ["fma", "vector_fma"] {
            let mut def = sample();
            def.operation.op = op.into();

            let first = inputs(&def);
            let second = inputs(&def);
            assert_eq!(first.a, second.a);
            assert_eq!(first.b, second.b);
            assert_eq!(first.c, second.c);
            assert!(first.c.is_some());
            assert_ne!(first.b, first.c);
        }
    }

    #[test]
    fn gen_dtype_dispatch() {
        assert!(matches!(gen_data("f32", 1, 4), Data::F32(_)));
        assert!(matches!(gen_data("u64", 1, 4), Data::U64(_)));
        assert!(matches!(gen_data("i64", 1, 4), Data::I64(_)));
    }
}
