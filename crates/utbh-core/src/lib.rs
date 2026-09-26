//! `utbh-core` — shared types workspace UTBH.
//!
//! **1 file = 1 tanggung jawab.** Modul di bawah masing-masing memegang satu
//! area; `lib.rs` hanya deklarasi modul + re-export.
//!
//! - [`testdef`] — definisi test deklaratif (TOML `*.utbh`)
//! - [`data`] — nilai data + generator input deterministik
//! - [`rng`] — RNG deterministik (xorshift64*)
//! - [`outcome`] — status/metrik hasil satu test
//! - [`result`] — `RunResult` + simpan/muat `.utbh-result.json`
//! - [`hardware`] — laporan hardware (diisi Layer 1/2)
//! - [`loader`] — loader test package (data-driven)
//! - [`util`] — run id + timestamp

pub mod data;
pub mod hardware;
pub mod loader;
pub mod outcome;
pub mod result;
pub mod rng;
pub mod testdef;
pub mod util;

pub use data::{gen_data, gen_f32, gen_i64, gen_u64, inputs, Data, Inputs};
pub use hardware::{
    CacheLevel, CpuReport, GpuReport, HardwareReport, InterconnectReport, MemoryReport,
};
pub use loader::{list_suites, load_all, load_suite, load_test_file};
pub use outcome::{FuzzSummary, Metric, Status, TestOutcome, ValidationInfo};
pub use result::RunResult;
pub use rng::Rng;
pub use testdef::{
    BenchmarkSpec, InputSpec, OperationSpec, TestDef, TestMeta, ValidationSpec,
};
pub use util::{new_run_id, now_unix};

/// Versi schema hasil. **Bump setiap format hasil berubah** (AGENTS.md #3).
pub const SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_SUITES_DIR: &str = "suites";
pub const DEFAULT_RESULTS_DIR: &str = "results";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_pinned() {
        assert_eq!(SCHEMA_VERSION, 1);
    }
}
