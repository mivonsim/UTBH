//! Parsing argumen CLI menggunakan clap.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use utbh_core::{DEFAULT_RESULTS_DIR, DEFAULT_SUITES_DIR};

#[derive(Parser)]
#[command(
    name = "utbh",
    about = "Universal Test Benchmark Hardware — jalankan di dalam guest OS (mivon emu)",
    version
)]
pub struct Cli {
    /// Direktori test package (data-driven, tanpa rebuild).
    #[arg(long, global = true, default_value = DEFAULT_SUITES_DIR)]
    pub suites: PathBuf,

    /// Direktori output hasil.
    #[arg(long, global = true, default_value = DEFAULT_RESULTS_DIR)]
    pub results: PathBuf,

    /// Profil run (`configs/*.json`) — preset suite + iterasi.
    /// Nilai flag eksplisit selalu menang atas profil.
    #[arg(long, global = true)]
    pub profile: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Layer 2 — deteksi kemampuan hardware.
    Discover,
    /// Daftar semua suite + test (tanpa menjalankan apa pun).
    List,
    /// Layer 4 — correctness: apakah hardware benar? (PASS/FAIL)
    Test {
        /// Nama suite (folder di `suites/`), `all` / `universal`,
        /// atau `[PROFILE]` untuk suite dari profil.
        suite: Option<String>,
    },
    /// Layer 5 — seberapa cepat? (latency, throughput, bandwidth, GFLOPS)
    Benchmark {
        suite: Option<String>,
        /// Iterasi pengukuran.
        #[arg(long)]
        iterations: Option<u32>,
        #[arg(long)]
        warmup: Option<u32>,
    },
    /// Test + benchmark dalam satu run (keduanya dilaporkan terpisah).
    Run {
        suite: Option<String>,
        #[arg(long)]
        iterations: Option<u32>,
        #[arg(long)]
        warmup: Option<u32>,
    },
    /// Fuzz engine: random workload + minimizer.
    Fuzz {
        suite: Option<String>,
        #[arg(long, default_value_t = utbh_fuzz::FuzzConfig::default().seed)]
        seed: u64,
        #[arg(long)]
        iterations: Option<usize>,
    },
    /// Stress engine: beban berkelanjutan — deteksi FAIL kumulatif +
    /// degradasi latensi (throttling/race).
    Stress {
        suite: Option<String>,
        /// Iterasi per test.
        #[arg(long)]
        iterations: Option<usize>,
        /// Ukuran window deteksi degradasi.
        #[arg(long)]
        window: Option<usize>,
    },
    /// Render `.utbh-report` dari semua hasil di results/.
    Report,
    /// Bandingkan dua hasil run (§11: VM vs FPGA vs ASIC).
    ///
    /// `utbh compare <file-a> <file-b>` — A = baseline, B = kandidat.
    Compare {
        /// File `.utbh-result.json` (baseline, mis. hasil VM).
        a: PathBuf,
        /// File `.utbh-result.json` (kandidat, mis. hasil ASIC).
        b: PathBuf,
    },
    /// Tampilkan `.utbh-trace` untuk run-id tertentu.
    Trace { run_id: String },
}
