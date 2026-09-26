//! `utbh-fuzz` — UTBH Fuzz Engine.
//!
//! ```text
//! Random Generator → Hardware Workload → Hardware API → Hardware VM
//!                                                            │
//!                                                     Validation
//!                                                   ┌────┴────┐
//!                                                 PASS       FAIL
//!                                                            │
//!                                                        Minimizer
//!                                                            │
//!                                                 Reproducible Test
//! ```
//!
//! **1 file = 1 tanggung jawab:**
//! - [`config`] — konfigurasi + scope operasi per suite
//! - [`generate`] — generator test definition acak (deklaratif)
//! - [`minimize`] — perkecil kasus FAIL sampai tetap reproduksi
//! - [`runner`] — orkestrasi fuzz run
//!
//! Setiap kasus **reproducible**: seed selalu dicatat. Tidak ada test
//! hardcode — fuzz menghasilkan `TestDef` dinamis (bukan `const TEST_001`).

pub mod config;
pub mod generate;
pub mod minimize;
pub mod runner;

pub use config::FuzzConfig;
pub use runner::{fuzz, FuzzCase, FuzzReport};
