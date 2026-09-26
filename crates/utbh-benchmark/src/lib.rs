//! `utbh-benchmark` — Layer 5: Benchmark Engine.
//!
//! Menjawab: **seberapa cepat?** (terpisah dari correctness = Layer 4).
//!
//! **1 file = 1 tanggung jawab:**
//! - [`sample`] — satu iterasi pengukuran via Hardware API
//! - [`aggregate`] — ubah kumpulan sample jadi daftar metrik
//!
//! **Tidak pernah** menghasilkan satu angka skor tunggal sebagai satu-satunya
//! hasil. Semua metrik dilaporkan terpisah, bersama PASS/FAIL dari Layer 4.

pub mod aggregate;
pub mod sample;

pub use aggregate::aggregate;
pub use sample::{run, BenchConfig};
