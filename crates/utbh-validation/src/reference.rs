//! Reference execution — implementasi independen, algoritma berbeda dari
//! `utbh-workload::execute`. Deterministik (input dari `utbh_core::inputs`).
//!
//! Sengaja ditulis beda (urutan loop, FMA, pairwise summation) supaya bug
//! implementasi di jalur hardware ketahuan lewat differential validation.

use utbh_core::{inputs, Data, TestDef};

pub fn reference(def: &TestDef) -> Result<Data, String> {
    let inp = inputs(def);
    let a = inp.a;
    let b = inp.b;
    let need_b = |b: &Option<Data>| -> Result<Data, String> {
        b.clone().ok_or_else(|| format!("op '{}' butuh dua operand", def.operation.op))
    };

    match def.operation.op.as_str() {
        "add" | "vector_add" => scalar_pair(a, need_b(&b)?, |x, y| x + y),
        "sub" | "vector_sub" => scalar_pair(a, need_b(&b)?, |x, y| x - y),
        "mul" | "vector_mul" => scalar_pair(a, need_b(&b)?, |x, y| x * y),
        "compare" => scalar_pair(a, need_b(&b)?, |x, y| if x == y { 1.0 } else { 0.0 }),
        "fma" | "vector_fma" => {
            let c = need_b(&b)?;
            match (a, need_b(&b)?, c) {
                (Data::F32(x), Data::F32(y), Data::F32(z)) => Ok(Data::F32(
                    (0..x.len()).map(|i| x[i].mul_add(y[i], z[i])).collect(),
                )),
                _ => Err("fma hanya f32".into()),
            }
        }
        "matmul" | "matrix_mul" => matmul_i_k_j(a, need_b(&b)?),
        "reduce_sum" | "reduce" => Ok(match a {
            Data::F32(v) => Data::F32(vec![pairwise_sum(&v)]),
            Data::U64(v) => Data::U64(vec![v.iter().fold(0u64, |s, x| s.wrapping_add(*x))]),
            Data::I64(v) => Data::I64(vec![v.iter().fold(0i64, |s, x| s.wrapping_add(*x))]),
        }),
        "copy" | "memcopy" | "mem_seq" | "seq" | "mem_rand" | "rand" => Ok(a),
        // Reference serial — deterministik vs jalur paralel (wrapping u64/i64
        // asosiatif; f32 sekunder, suite memakai u64).
        "atomic_add" => Ok(match a {
            Data::U64(v) => Data::U64(vec![v.iter().fold(0u64, |s, x| s.wrapping_add(*x))]),
            _ => return Err("atomic_add reference hanya u64".into()),
        }),
        "parallel_sum" | "multicore_sum" => Ok(match a {
            Data::U64(v) => Data::U64(vec![v.iter().fold(0u64, |s, x| s.wrapping_add(*x))]),
            Data::I64(v) => Data::I64(vec![v.iter().fold(0i64, |s, x| s.wrapping_add(*x))]),
            Data::F32(v) => Data::F32(vec![v.iter().fold(0f32, |s, x| s + x)]),
        }),
        other => Err(format!("op '{}' tidak punya reference", other)),
    }
}

/// Matmul referensi: urutan loop i-j-k + `mul_add` (berbeda dari workload
/// i-k-j tanpa kontrak FMA).
fn matmul_i_k_j(a: Data, b: Data) -> Result<Data, String> {
    let (Data::F32(x), Data::F32(y)) = (a, b) else {
        return Err("matmul hanya f32".into());
    };
    let n = (x.len() as f64).sqrt().floor() as usize;
    if n == 0 {
        return Err("matmul butuh minimal 1 elemen".into());
    }
    let mut c = vec![0f32; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut acc = 0f32;
            for k in 0..n {
                // x*y + acc — urutan operand mul_add penting:
                // acc.mul_add(x, y) = acc*x + y (SALAH untuk akumulasi).
                acc = x[i * n + k].mul_add(y[k * n + j], acc);
            }
            c[i * n + j] = acc;
        }
    }
    Ok(Data::F32(c))
}

fn pairwise_sum(v: &[f32]) -> f32 {
    if v.len() <= 1 {
        return v.first().copied().unwrap_or(0.0);
    }
    let mid = v.len() / 2;
    pairwise_sum(&v[..mid]) + pairwise_sum(&v[mid..])
}

fn scalar_pair(a: Data, b: Data, f: impl Fn(f64, f64) -> f64) -> Result<Data, String> {
    if a.len() != b.len() {
        return Err(format!("panjang beda: {} vs {}", a.len(), b.len()));
    }
    Ok(match (a, b) {
        (Data::F32(x), Data::F32(y)) => {
            Data::F32(x.iter().zip(&y).map(|(p, q)| f(*p as f64, *q as f64) as f32).collect())
        }
        (Data::U64(x), Data::U64(y)) => {
            Data::U64(x.iter().zip(&y).map(|(p, q)| f(*p as f64, *q as f64) as u64).collect())
        }
        (Data::I64(x), Data::I64(y)) => {
            Data::I64(x.iter().zip(&y).map(|(p, q)| f(*p as f64, *q as f64) as i64).collect())
        }
        _ => return Err("dtype campuran".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(op: &str, elements: usize, dtype: &str) -> TestDef {
        toml::from_str(&format!(
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
        ))
        .unwrap()
    }

    #[test]
    fn reference_matches_workload_on_add() {
        let d = def("add", 256, "f32");
        let r = reference(&d).unwrap();
        let e = utbh_workload::execute(&d).unwrap();
        assert_eq!(r, e.data);
    }

    #[test]
    fn pairwise_differs_from_linear_on_large_sum() {
        // Bukan jaminan beda, tapi bentuknya berbeda → tetap deterministik.
        let d = def("reduce_sum", 4096, "f32");
        let r = reference(&d).unwrap();
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn unknown_op_has_no_reference() {
        let d = def("bogus", 8, "f32");
        assert!(reference(&d).is_err());
    }

    #[test]
    fn matmul_reference_shape() {
        let d = def("matmul", 400, "f32");
        assert_eq!(reference(&d).unwrap().len(), 400);
    }
}
