//! Eksekusi workload paralel & atomik (multicore).
//!
//! Dua target koreksi yang berbeda dari workload serial:
//! - `atomic_add` — **atomic correctness**: banyak thread `fetch_add` ke satu
//!   lokasi. Hasil = jumlah wrapping (komutatif) → deterministik. Kalau
//!   hardware kehilangan update (atomic rusak), hasil beda → FAIL.
//! - `parallel_sum` — **multicore reduce**: partial sum per thread, digabung.
//!   u64 wrapping asosiatif → identik dengan serial; f32 tidak (urutan beda)
//!   → wajib mode `approx`.
//!
//! Pembagian chunk deterministik (ukuran tetap, terurut) — seed sama = kerja
//! sama, hasil identik selama hardware benar.

use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use utbh_core::{Data, TestDef};

/// Jumlah logical CPU (dari OS — bukan pembacaan /proc langsung).
fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, 64)
}

/// `atomic_add` — semua thread menambah shared counter atomik.
/// Output: satu elemen = total wrapping.
pub fn atomic_add(def: &TestDef, a: Data) -> Result<Data, String> {
    let Data::U64(v) = &a else {
        return Err("atomic_add hanya mendukung u64".into());
    };
    let total = AtomicU64::new(0);
    let n = threads();
    let chunk = v.len().div_ceil(n.max(1));

    thread::scope(|s| {
        for part in v.chunks(chunk.max(1)) {
            let part: &[u64] = part;
            let counter = &total; // shared ref — aman untuk banyak closure `move`
            s.spawn(move || {
                for &x in part {
                    counter.fetch_add(x, Ordering::SeqCst);
                }
            });
        }
    });

    let _ = def; // dipakai untuk logging/test context bila perlu
    Ok(Data::U64(vec![total.into_inner()]))
}

/// `parallel_sum` — partial sum per thread, digabung jadi satu.
/// u64/i64: wrapping asosiatif → identik serial. f32: urutan beda → approx.
pub fn parallel_sum(a: Data) -> Result<Data, String> {
    let n = threads();
    match a {
        Data::U64(v) => {
            let partials = sum_chunks(&v, n, |p| p.iter().fold(0u64, |s, x| s.wrapping_add(*x)));
            Ok(Data::U64(vec![partials
                .iter()
                .fold(0u64, |s, x| s.wrapping_add(*x))]))
        }
        Data::I64(v) => {
            let partials = sum_chunks(&v, n, |p| p.iter().fold(0i64, |s, x| s.wrapping_add(*x)));
            Ok(Data::I64(vec![partials
                .iter()
                .fold(0i64, |s, x| s.wrapping_add(*x))]))
        }
        Data::F32(v) => {
            let partials = sum_chunks(&v, n, |p| p.iter().fold(0f32, |s, x| s + x));
            Ok(Data::F32(vec![partials.iter().fold(0f32, |s, x| s + x)]))
        }
    }
}

/// Bagi `v` jadi `n` chunk paralel, hitung partial per chunk (deterministik:
/// chunk berurutan, ukuran tetap).
fn sum_chunks<T: Clone, R: Send>(v: &[T], n: usize, f: impl Fn(&[T]) -> R + Sync) -> Vec<R>
where
    T: Sync,
{
    let chunk = v.len().div_ceil(n.max(1)).max(1);
    let parts: Vec<&[T]> = v.chunks(chunk).collect();
    let mut partials: Vec<Option<R>> = (0..parts.len()).map(|_| None).collect();

    thread::scope(|s| {
        let mut handles = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            let f = &f;
            handles.push(s.spawn(move || (i, f(*part))));
        }
        for h in handles {
            let (i, r) = h.join().expect("worker thread panic");
            partials[i] = Some(r);
        }
    });

    partials
        .into_iter()
        .map(|p| p.expect("semua chunk selesai"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def_u64(op: &str, elements: usize) -> TestDef {
        toml::from_str(&format!(
            r#"
[test]
name = "{}"
category = "cpu.multicore"
[input]
elements = {}
type = "u64"
[operation]
op = "{}"
[validation]
mode = "exact"
"#,
            op, elements, op
        ))
        .unwrap()
    }

    #[test]
    fn atomic_add_equals_sequential_sum() {
        let def = def_u64("atomic_add", 10_000);
        let a = utbh_core::inputs(&def).a;
        let Data::U64(v) = &a else { panic!() };
        let expected = v.iter().fold(0u64, |s, x| s.wrapping_add(*x));
        let out = atomic_add(&def, a).unwrap();
        assert_eq!(out, Data::U64(vec![expected]));
    }

    #[test]
    fn parallel_sum_matches_serial_u64() {
        let def = def_u64("parallel_sum", 50_000);
        let a = utbh_core::inputs(&def).a;
        let Data::U64(v) = &a else { panic!() };
        let expected = v.iter().fold(0u64, |s, x| s.wrapping_add(*x));
        assert_eq!(parallel_sum(a).unwrap(), Data::U64(vec![expected]));
    }

    #[test]
    fn parallel_sum_f32_close_to_serial() {
        let def = toml::from_str::<TestDef>(
            r#"
[test]
name = "ps"
category = "cpu.multicore"
[input]
elements = 20000
type = "f32"
[operation]
op = "parallel_sum"
[validation]
mode = "approx"
"#,
        )
        .unwrap();
        let a = utbh_core::inputs(&def).a;
        let Data::F32(v) = &a else { panic!() };
        let serial: f32 = v.iter().sum();
        let Data::F32(out) = parallel_sum(a).unwrap() else {
            panic!()
        };
        let rel = ((out[0] - serial) / serial.abs().max(1e-12)).abs();
        assert!(rel < 1e-4, "rel={}", rel);
    }

    #[test]
    fn atomic_rejects_f32() {
        let def = toml::from_str::<TestDef>(
            r#"
[test]
name = "a"
category = "cpu"
[input]
elements = 8
type = "f32"
[operation]
op = "atomic_add"
[validation]
mode = "exact"
"#,
        )
        .unwrap();
        let a = utbh_core::inputs(&def).a;
        assert!(atomic_add(&def, a).is_err());
    }

    #[test]
    fn chunks_partition_evenly() {
        let v: Vec<u64> = (0..1000).collect();
        let partials = sum_chunks(&v, 7, |p| p.iter().sum::<u64>());
        let total: u64 = partials.iter().sum();
        assert_eq!(total, (0..1000u64).sum::<u64>());
    }
}
