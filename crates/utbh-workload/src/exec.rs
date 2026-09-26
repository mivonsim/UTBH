//! Eksekusi workload per operasi.
//!
//! Input selalu dari [`utbh_core::inputs`] — deterministik; seed sama = kerja
//! sama, input identik antara workload dan reference.

use crate::op::Op;
use utbh_core::{inputs, Data, TestDef};

/// Hasil eksekusi workload.
pub struct WorkloadOutput {
    pub data: Data,
    /// Jumlah elemen yang diproses (untuk throughput).
    pub elements: u64,
    /// Byte yang disentuh (untuk bandwidth).
    pub bytes: u64,
}

/// Eksekusi workload sesuai `[operation].op` pada test definition.
pub fn execute(def: &TestDef) -> Result<WorkloadOutput, String> {
    let op = Op::parse(&def.operation.op)?;
    let inp = inputs(def);
    let a = inp.a;
    let need_b = |b: &Option<Data>| -> Result<Data, String> {
        b.clone()
            .ok_or_else(|| format!("op '{}' butuh dua operand", def.operation.op))
    };

    let out = match op {
        Op::Add => bin(a, need_b(&inp.b)?, |x, y| x + y)?,
        Op::Sub => bin(a, need_b(&inp.b)?, |x, y| x - y)?,
        Op::Mul => bin(a, need_b(&inp.b)?, |x, y| x * y)?,
        Op::Fma => fma(a, need_b(&inp.b)?, inp.b.clone())?,
        Op::MatMul => matmul(a, need_b(&inp.b)?)?,
        Op::Compare => compare(a, need_b(&inp.b)?)?,
        Op::ReduceSum => reduce_sum(a),
        Op::Copy => copy(a),
        Op::MemSeq => mem_pattern(a, false),
        Op::MemRand => mem_pattern(a, true),
        Op::AtomicAdd => Partial {
            data: crate::exec_parallel::atomic_add(def, a)?,
        },
        Op::ParallelSum => Partial {
            data: crate::exec_parallel::parallel_sum(a)?,
        },
    };

    let elements = out.data.len() as u64;
    let bytes = elements * out.data.elem_size() as u64;
    Ok(WorkloadOutput {
        data: out.data,
        elements,
        bytes,
    })
}

struct Partial {
    data: Data,
}

fn bin(a: Data, b: Data, f: impl Fn(f64, f64) -> f64) -> Result<Partial, String> {
    if a.len() != b.len() {
        return Err(format!("panjang tidak cocok: {} vs {}", a.len(), b.len()));
    }
    Ok(Partial {
        data: match (a, b) {
            (Data::F32(x), Data::F32(y)) => Data::F32(
                x.iter()
                    .zip(&y)
                    .map(|(p, q)| f(*p as f64, *q as f64) as f32)
                    .collect(),
            ),
            (Data::U64(x), Data::U64(y)) => Data::U64(
                x.iter()
                    .zip(&y)
                    .map(|(p, q)| f(*p as f64, *q as f64) as u64)
                    .collect(),
            ),
            (Data::I64(x), Data::I64(y)) => Data::I64(
                x.iter()
                    .zip(&y)
                    .map(|(p, q)| f(*p as f64, *q as f64) as i64)
                    .collect(),
            ),
            _ => return Err("dtype campuran tidak didukung".into()),
        },
    })
}

fn fma(a: Data, b: Data, c: Option<Data>) -> Result<Partial, String> {
    let Some(c) = c else {
        return Err("fma butuh tiga operand".into());
    };
    match (a, b, c) {
        (Data::F32(x), Data::F32(y), Data::F32(z)) => Ok(Partial {
            data: Data::F32(
                x.iter()
                    .zip(&y)
                    .zip(&z)
                    .map(|((p, q), r)| p * q + r)
                    .collect(),
            ),
        }),
        _ => Err("fma hanya mendukung f32".into()),
    }
}

/// Matrix multiplication bentuk n×n (n = floor(sqrt(elements))).
fn matmul(a: Data, b: Data) -> Result<Partial, String> {
    let (Data::F32(x), Data::F32(y)) = (&a, &b) else {
        return Err("matmul hanya mendukung f32".into());
    };
    let n = (x.len() as f64).sqrt().floor() as usize;
    if n == 0 {
        return Err("matmul butuh minimal 1 elemen".into());
    }
    let mut c = vec![0f32; n * n];
    for i in 0..n {
        for k in 0..n {
            let aik = x[i * n + k];
            for j in 0..n {
                c[i * n + j] += aik * y[k * n + j];
            }
        }
    }
    Ok(Partial { data: Data::F32(c) })
}

fn compare(a: Data, b: Data) -> Result<Partial, String> {
    bin(a, b, |x, y| if x == y { 1.0 } else { 0.0 })
}

fn reduce_sum(a: Data) -> Partial {
    let data = match a {
        Data::F32(v) => Data::F32(vec![v.iter().sum::<f32>()]),
        Data::U64(v) => Data::U64(vec![v.iter().fold(0u64, |acc, x| acc.wrapping_add(*x))]),
        Data::I64(v) => Data::I64(vec![v.iter().fold(0i64, |acc, x| acc.wrapping_add(*x))]),
    };
    Partial { data }
}

fn copy(a: Data) -> Partial {
    Partial { data: a }
}

/// Sentuh semua elemen (seq) atau urutan pseudo-acak — dua jalur memory berbeda
/// untuk latency test. Output = input (data tidak diubah).
fn mem_pattern(a: Data, random: bool) -> Partial {
    let n = match &a {
        Data::F32(v) => v.len(),
        Data::U64(v) => v.len(),
        Data::I64(v) => v.len(),
    };
    let order = if random {
        fisher_yates(n)
    } else {
        (0..n).collect::<Vec<_>>()
    };
    // Checksum pada urutan order — memaksa akses sesuai pola.
    let _checksum: f64 = match &a {
        Data::F32(v) => order.iter().map(|&i| v[i] as f64).sum(),
        Data::U64(v) => order.iter().map(|&i| v[i] as f64).sum(),
        Data::I64(v) => order.iter().map(|&i| v[i] as f64).sum(),
    };
    std::hint::black_box(_checksum);
    Partial { data: a }
}

/// Fisher-Yates dengan RNG deterministik dari panjang data.
fn fisher_yates(n: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..n).collect();
    let mut state = 0x9E37_79B9_7F4A_7C15u64 ^ (n as u64);
    for i in (1..n).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state % (i as u64 + 1)) as usize;
        idx.swap(i, j);
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(op: &str, dtype: &str, elements: usize) -> TestDef {
        let s = format!(
            r#"
[test]
name = "t"
category = "cpu"

[input]
elements = {}
type = "{}"

[operation]
op = "{}"

[validation]
mode = "exact"
"#,
            elements, dtype, op
        );
        toml::from_str(&s).unwrap()
    }

    #[test]
    fn add_executes() {
        let o = execute(&def("add", "f32", 100)).unwrap();
        assert_eq!(o.data.len(), 100);
        assert_eq!(o.elements, 100);
        assert_eq!(o.bytes, 100 * 4);
    }

    #[test]
    fn reduce_sums_to_one() {
        let o = execute(&def("reduce_sum", "u64", 64)).unwrap();
        assert_eq!(o.data.len(), 1);
    }

    #[test]
    fn matmul_shapes() {
        let o = execute(&def("matmul", "f32", 100)).unwrap(); // 10×10
        assert_eq!(o.data.len(), 100);
    }

    #[test]
    fn unknown_op_rejected() {
        assert!(execute(&def("bogus", "f32", 8)).is_err());
    }

    #[test]
    fn copy_is_identity() {
        let d = def("copy", "f32", 16);
        let o = execute(&d).unwrap();
        let inp = utbh_core::inputs(&d);
        assert_eq!(o.data, inp.a);
    }

    #[test]
    fn fisher_yates_permutation() {
        let mut p = fisher_yates(32);
        p.sort_unstable();
        assert_eq!(p, (0..32).collect::<Vec<_>>());
    }

    #[test]
    fn mem_pattern_deterministic() {
        let a = Data::U64(vec![1, 2, 3, 4, 5, 6, 7, 8]);
        let (Data::U64(x), Data::U64(y)) = (a.clone(), mem_pattern(a, true).data) else {
            panic!("u64");
        };
        assert_eq!(x, y);
    }
}
