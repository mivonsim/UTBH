//! Laporan hardware — diisi oleh Layer 1 (Hardware API) + Layer 2 (Discovery).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HardwareReport {
    pub cpu: CpuReport,
    pub cache: Vec<CacheLevel>,
    pub memory: MemoryReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<GpuReport>,
    pub interconnect: InterconnectReport,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuReport {
    pub architecture: String,
    pub model: String,
    pub cores: u32,
    pub threads: u32,
    pub vector: String,
    pub frequency_mhz: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheLevel {
    pub level: u8,
    /// `L1D` | `L1I` | `L2` | `L3` | ...
    pub kind: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryReport {
    pub capacity_bytes: u64,
    pub channels: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpuReport {
    pub name: String,
    pub compute_units: u32,
    pub fp32: bool,
    pub fp16: bool,
    pub bf16: bool,
    pub int8: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InterconnectReport {
    pub kind: String,
    pub topology: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_report_serializes() {
        let r = HardwareReport::default();
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("cpu"));
        assert!(json.contains("interconnect"));
    }
}
