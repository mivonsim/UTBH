//! `utbh-stress` — Stress Engine.
//!
//! **1 file = 1 tanggung jawab:**
//! - [`config`] — parameter stress run (durasi, interval laporan)
//! - [`runner`] — jalankan test berulang, deteksi degradasi
//!
//! Pertanyaan stress: **apakah hardware tetap benar dan stabil di bawah
//! beban berkelanjutan?** Terpisah dari:
//! - testing (benar sekali?) — Layer 4
//! - benchmark (cepat sekali?) — Layer 5
//! - fuzz (benar pada input acak?)
//!
//! Sinyal yang dicari: PASS→FAIL setelah N iterasi (overheating/throttling/
//! race), dan lonjakan latensi antar window (degradasi performa).

pub mod config;
pub mod runner;

pub use config::StressConfig;
pub use runner::{stress, to_outcome, StressReport, Window};
