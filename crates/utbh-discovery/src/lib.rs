//! `utbh-discovery` — Layer 2: Hardware Discovery.
//!
//! **1 file = 1 tanggung jawab:**
//! - [`collect`] — ambil laporan hardware dari [`HardwareApi`]
//! - [`render`] — format laporan jadi teks `utbh discover`
//!
//! UTBH melihat **hardware**, bukan virtual hardware. Tidak ada deteksi
//! hypervisor/Verilator — itu urusan OS.

pub mod collect;
pub mod render;

pub use collect::discover;
pub use render::format_report;

use utbh_core::HardwareReport;

/// Hasil discovery lengkap.
#[derive(Debug, Clone)]
pub struct Discovery {
    pub report: HardwareReport,
    /// Estimasi memory bandwidth terukur (opsional, diisi probe).
    pub mem_bandwidth_gbs: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use utbh_api::MivonHardwareApi;

    #[test]
    fn discover_and_format() {
        let api = MivonHardwareApi::new();
        let d = discover(&api);
        let text = format_report(&d);
        assert!(text.contains("UTBH Hardware Discovery"));
        assert!(text.contains("CPU"));
        assert!(text.contains("Memory"));
        assert!(text.contains("Interconnect"));
    }
}
