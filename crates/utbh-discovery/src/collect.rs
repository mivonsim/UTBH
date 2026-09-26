//! Kumpulkan laporan hardware dari Hardware API (tanpa rendering).

use crate::Discovery;
use utbh_api::HardwareApi;

/// Jalankan discovery melalui Hardware API.
pub fn discover(api: &dyn HardwareApi) -> Discovery {
    Discovery {
        report: api.hardware_report(),
        mem_bandwidth_gbs: None,
    }
}
