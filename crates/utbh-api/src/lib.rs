//! `utbh-api` — Layer 1: **Mivon Hardware API**.
//!
//! Interface resmi antara UTBH dan Mivon Hardware OS. Crate lain di workspace
//! ini **dilarang** membaca `/proc`, `/sys`, atau CPUID langsung; semua lewat
//! trait [`HardwareApi`].
//!
//! UTBH tidak tahu dirinya berada di VM. Ia hanya melihat hardware yang
//! dilaporkan OS melalui API ini — apapun backend-nya (fast VM, cycle-accurate
//! VM, RTL, FPGA, ASIC). Kode UTBH sama untuk ketiganya.

use utbh_core::{
    CacheLevel, CpuReport, GpuReport, HardwareReport, InterconnectReport, MemoryReport,
};

/// Jendela resmi ke hardware.
///
/// Implementasi dibaca dari fasilitas yang disediakan Mivon Hardware OS.
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

    /// Koordinat scratch memory untuk DMA/NoC workload, jika didukung OS.
    fn dma_scratch(&self, size: usize) -> Result<Vec<u8>, String>;
}

/// Implementasi standar di atas Mivon Hardware OS.
///
/// Sumber: `/proc` + `/sys` yang diekspos OS. Jika sebuah jalur tidak ada
/// (mis. GPU tidak terpasang), field diisi default wajar — bukan error.
/// Discovery (Layer 2) yang menerjemahkan ke laporan.
#[derive(Default, Debug, Clone)]
pub struct MivonHardwareApi;

impl MivonHardwareApi {
    pub fn new() -> Self {
        Self
    }
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

fn parse_size_bytes(s: &str) -> u64 {
    let s = s.trim();
    let (num, factor) = if let Some(r) = s.strip_suffix('K') {
        (r, 1024u64)
    } else if let Some(r) = s.strip_suffix('M') {
        (r, 1024 * 1024)
    } else if let Some(r) = s.strip_suffix('G') {
        (r, 1024 * 1024 * 1024)
    } else {
        (s, 1)
    };
    num.trim()
        .parse::<f64>()
        .map(|n| (n * factor as f64) as u64)
        .unwrap_or(0)
}

impl HardwareApi for MivonHardwareApi {
    fn hardware_report(&self) -> HardwareReport {
        // --- CPU ---------------------------------------------------------
        let arch = std::env::consts::ARCH.to_string();
        let model = read("/proc/cpuinfo")
            .and_then(|info| {
                info.lines()
                    .find_map(|l| l.split_once(':').map(|(k, v)| (k, v)))
                    .filter(|(k, _)| {
                        let k = k.trim();
                        k == "model name" || k == "Model" || k == "cpu model"
                    })
                    .map(|(_, v)| v.trim().to_string())
            })
            .unwrap_or_else(|| format!("unknown-{}", arch));

        let flags = read("/proc/cpuinfo").unwrap_or_default();
        let vector = detect_vector(&arch, &flags);

        let cores = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);

        let frequency_mhz = read("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
            .map(|khz| khz.parse::<u64>().unwrap_or(0) / 1000)
            .filter(|m| *m > 0)
            .unwrap_or_else(|| {
                read("/proc/cpuinfo")
                    .and_then(|info| {
                        info.lines()
                            .find_map(|l| {
                                let (k, v) = l.split_once(':')?;
                                let k = k.trim();
                                (k == "cpu MHz" || k == "clock").then(|| {
                                    v.trim().trim_end_matches(" MHz").trim().parse::<f64>().ok()
                                })
                            })
                            .flatten()
                    })
                    .map(|mhz| mhz as u64)
                    .unwrap_or(0)
            });

        let cpu = CpuReport {
            architecture: arch,
            model,
            cores,
            threads: cores,
            vector,
            frequency_mhz,
        };

        // --- Cache -------------------------------------------------------
        let mut cache = Vec::new();
        for i in 0.. {
            let base = format!("/sys/devices/system/cpu/cpu0/cache/index{}", i);
            let Some(level) = read(&format!("{}/level", base)) else {
                break;
            };
            let Some(ctype) = read(&format!("{}/type", base)) else {
                break;
            };
            let Some(size) = read(&format!("{}/size", base)) else {
                break;
            };
            let kind = match (level.parse::<u8>().unwrap_or(0), ctype.as_str()) {
                (1, "Data") => "L1D".to_string(),
                (1, "Instruction") => "L1I".to_string(),
                (1, _) => "L1".to_string(),
                (l, _) => format!("L{}", l),
            };
            cache.push(CacheLevel {
                level: level.parse().unwrap_or(0),
                kind,
                size_bytes: parse_size_bytes(&size),
            });
        }

        // --- Memory ------------------------------------------------------
        let capacity_bytes = read("/proc/meminfo")
            .and_then(|m| {
                m.lines()
                    .find_map(|l| l.strip_prefix("MemTotal:"))
                    .and_then(|v| v.split_whitespace().next())
                    .and_then(|kb| kb.parse::<u64>().ok())
            })
            .map(|kb| kb * 1024)
            .unwrap_or(0);

        let channels = read("/sys/devices/system/memory").map(|_| 2).unwrap_or(1);

        let memory = MemoryReport {
            capacity_bytes,
            channels: channels.max(1),
        };

        // --- GPU ---------------------------------------------------------
        let gpu = discover_gpu();

        // --- Interconnect ------------------------------------------------
        let interconnect = InterconnectReport {
            kind: read("/sys/class/noc/type").unwrap_or_else(|| "on-chip".into()),
            topology: read("/sys/class/noc/topology").unwrap_or_else(|| "unknown".into()),
        };

        HardwareReport {
            cpu,
            cache,
            memory,
            gpu,
            interconnect,
        }
    }

    fn timer_ns(&self) -> u64 {
        // Monotonic clock dari OS. std::time::Instant berbasis CLOCK_MONOTONIC.
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        let start = START.get_or_init(std::time::Instant::now);
        start.elapsed().as_nanos() as u64
    }

    fn cycles(&self) -> u64 {
        // Fallback: estimasi dari ns × frekuensi. OS cycle-accurate dapat
        // menimpa lewat CPU counter (rdtsc / timebase) pada implementasi asli.
        let hz = self.cpu_frequency_hz();
        self.timer_ns().saturating_mul(hz) / 1_000_000_000
    }

    fn cpu_frequency_hz(&self) -> u64 {
        read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
            .and_then(|khz| khz.parse::<u64>().ok())
            .map(|khz| khz * 1000)
            .unwrap_or_else(|| {
                let mhz = self.hardware_report().cpu.frequency_mhz;
                if mhz > 0 {
                    mhz * 1_000_000
                } else {
                    1_000_000_000 // konservatif: 1 GHz default
                }
            })
    }

    fn cpu_count(&self) -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }

    fn pin_to_cpu(&self, cpu: usize) -> Result<(), String> {
        // Best-effort: OS yang mendukung isolasi core akan menegakkan ini.
        // Di environment tanpa dukungan, tetap lanjut — bukan fatal.
        let n = self.cpu_count();
        if cpu >= n {
            return Err(format!("cpu {} di luar jumlah cpu {}", cpu, n));
        }
        let _ = cpu;
        Ok(())
    }

    fn dma_scratch(&self, size: usize) -> Result<Vec<u8>, String> {
        // Aligned buffer besar ≈ DMA-able region di memory biasa.
        // OS dengan IOMMU akan memetakan buffer user ke region DMA.
        if size == 0 {
            return Err("size 0".into());
        }
        let mut v = Vec::with_capacity(size + 64);
        v.resize(size, 0u8);
        Ok(v)
    }
}

fn detect_vector(arch: &str, cpuinfo: &str) -> String {
    if arch.contains("riscv") {
        if cpuinfo.contains("rvv") || cpuinfo.contains("v ") {
            return "RVV".into();
        }
        return "none".into();
    }
    if arch == "aarch64" {
        return if cpuinfo.contains("asimd") || cpuinfo.contains("neon") {
            "NEON/ASIMD".into()
        } else {
            "none".into()
        };
    }
    // x86 family
    for f in ["avx512f", "avx2", "sse4.2", "sse2"] {
        if cpuinfo.contains(f) {
            return f.to_uppercase().replace('F', "F");
        }
    }
    "none".into()
}

fn discover_gpu() -> Option<GpuReport> {
    // Mivon OS mengekspos GPU melalui /sys/class/drm bila ada.
    let drm = std::path::Path::new("/sys/class/drm");
    if !drm.exists() {
        return None;
    }
    let name = read("/sys/class/drm/card0/device/product")
        .or_else(|| read("/sys/class/drm/card0/device/label"))
        .unwrap_or_else(|| "gpu0".into());
    let cu = read("/sys/class/drm/card0/device/compute_units")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let feats = read("/sys/class/drm/card0/device/compute_features").unwrap_or_default();
    Some(GpuReport {
        name,
        compute_units: cu,
        fp32: feats.contains("fp32") || feats.is_empty(),
        fp16: feats.contains("fp16"),
        bf16: feats.contains("bf16"),
        int8: feats.contains("int8"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_never_panics() {
        let api = MivonHardwareApi::new();
        let r = api.hardware_report();
        assert!(!r.cpu.architecture.is_empty());
        assert!(r.cpu.cores >= 1);
    }

    #[test]
    fn timer_monotonic() {
        let api = MivonHardwareApi::new();
        let a = api.timer_ns();
        let b = api.timer_ns();
        assert!(b >= a);
    }

    #[test]
    fn parse_sizes() {
        assert_eq!(parse_size_bytes("64K"), 64 * 1024);
        assert_eq!(parse_size_bytes("1M"), 1024 * 1024);
        assert_eq!(parse_size_bytes("16M"), 16 * 1024 * 1024);
    }

    #[test]
    fn pin_range_checked() {
        let api = MivonHardwareApi::new();
        let n = api.cpu_count();
        assert!(api.pin_to_cpu(n + 100).is_err());
    }
}
