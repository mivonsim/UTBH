//! `utbh-workload` — Layer 3: Workload Engine.
//!
//! **1 file = 1 tanggung jawab:**
//! - [`op`] — parsing & klasifikasi operasi
//! - [`exec`] — eksekusi workload serial per operasi
//! - [`exec_parallel`] — eksekusi atomik & multicore (atomic correctness,
//!   parallel reduce)
//!
//! Workload tidak mengukur — ukur oleh Benchmark (Layer 5). Workload hanya
//! mengeksekusi dan menghasilkan output untuk divalidasi Layer 6.

pub mod exec;
pub mod exec_parallel;
pub mod op;

pub use exec::{execute, WorkloadOutput};
pub use op::Op;
