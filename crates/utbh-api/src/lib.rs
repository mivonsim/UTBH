//! `utbh-api` — Layer 1: **Mivon Hardware API**.
//!
//! Interface resmi antara UTBH dan guest OS (yang di-boot `mivon emu`).
//! Crate lain di workspace
//! ini **dilarang** membaca `/proc`, `/sys`, atau CPUID langsung; semua lewat
//! trait [`HardwareApi`].
//!
//! **1 file = 1 tanggung jawab:**
//! - [`probe`] — satu-satunya pembaca `/proc` + `/sys`
//! - [`mivon`] — implementasi [`HardwareApi`] di atas guest OS
//!
//! UTBH tidak tahu dirinya berada di emulator. Ia hanya melihat hardware yang
//! dilaporkan OS — apapun backend-nya (fast emu, cycle-accurate, RTL-linked
//! CPU, FPGA, ASIC). Kode UTBH sama untuk ketiganya.

pub mod mivon;
pub mod probe;

pub use mivon::MivonHardwareApi;

use utbh_core::HardwareReport;

/// Jendela resmi ke hardware.
///
/// Implementasi dibaca dari fasilitas yang disediakan guest OS (boot
/// `mivon emu`).
/// UTBH tidak membedakan "virtual" vs "physical" — semua adalah hardware
/// menurut OS.
pub trait HardwareApi {
    /// Laporan hardware lengkap (Layer 2 memakai ini).
    fn hardware_report(&self) -> HardwareReport;

    /// Monotonic timer dalam nanosecond — satuan waktu benchmark.
    fn timer_ns(&self) -> u64;

    /// Cycle counter per core, jika tersedia (untuk metrik cycles).
    fn cycles(&self) -> u64;

    /// Frekuensi nominal CPU dalam Hz.
    fn cpu_frequency_hz(&self) -> u64;

    /// Jumlah logical CPU yang tersedia untuk workload multicore.
    fn cpu_count(&self) -> usize;

    /// Pin eksekusi ke logical CPU tertentu (noise reduction di VM).
    fn pin_to_cpu(&self, cpu: usize) -> Result<(), String>;

    /// Buffer aligned untuk DMA/NoC workload, jika didukung OS.
    fn dma_scratch(&self, size: usize) -> Result<Vec<u8>, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mivon_impl_satisfies_trait() {
        let api: Box<dyn HardwareApi> = Box::new(MivonHardwareApi::new());
        let r = api.hardware_report();
        assert!(!r.cpu.architecture.is_empty());
    }
}
