//! `utbh-trace` — Layer 7: Trace untuk kasus FAIL.
//!
//! Ketika validation FAIL, trace berisi: cycle, instruction, register,
//! memory access, interrupt, DMA, GPU command, NoC transaction.
//!
//! **1 file = 1 tanggung jawab:**
//! - [`event`] — tipe `TraceEvent` + `Trace`
//! - [`build`] — bangun trace dari outcome FAIL (via Hardware API)
//! - [`store`] — simpan/muat `.utbh-trace.json`
//! - [`render`] — format teks untuk `utbh trace <run-id>`
//!
//! Trace dibaca dari interface hardware/OS — **bukan** dengan memanggil
//! simulator secara langsung.

pub mod build;
pub mod event;
pub mod render;
pub mod store;

pub use build::from_failure;
pub use event::{Trace, TraceEvent};
pub use render::render;
pub use store::{filename, load, load_all, save};
