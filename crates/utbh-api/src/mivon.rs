//! Implementasi [`crate::HardwareApi`] di atas guest OS (boot `mivon emu`).
//!
//! Satu-satunya tempat yang menyusun hasil [`crate::probe`] jadi laporan
//! hardware. UTBH melihat hardware apa adanya — tanpa konsep "emulator".

use crate::probe;
use crate::HardwareApi;
use utbh_core::{
    CacheLevel, CpuReport, GpuReport, HardwareReport, InterconnectReport, MemoryReport,
};

/// Hardware API standar guest UTBH.
#[derive(Default, Debug, Clone)]
pub struct MivonHardwareApi;

impl MivonHardwareApi {
    pub fn new() -> Self {
        Self
    }
}

fn cache_kind(level: &str, ctype: &str) -> String {
    match (level.parse::<u8>().unwrap_or(0), ctype) {
        (1, "Data") => "L1D".into(),
        (1, "Instruction") => "L1I".into(),
        (1, _) => "L1".into(),
        (l, _) => format!("L{}", l),
    }
}

/// CPU logis yang diizinkan OS untuk proses ini (cpuset-aware).
#[cfg(target_os = "linux")]
fn allowed_cpus() -> Result<Vec<usize>, String> {
    // SAFETY: set di-zero-init dan diisi kernel; ukuran sesuai tipe kernel.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        if libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut set) != 0 {
            return Err(format!(
                "sched_getaffinity: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok((0..libc::CPU_SETSIZE as usize)
            .filter(|i| libc::CPU_ISSET(*i, &set))
            .collect())
    }
}

impl HardwareApi for MivonHardwareApi {
    fn hardware_report(&self) -> HardwareReport {
        let arch = std::env::consts::ARCH.to_string();
        let flags = probe::cpu_flags();
        let cores = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);

        let cpu = CpuReport {
            vector: probe::detect_vector(&arch, &flags),
            architecture: arch.clone(),
            model: probe::cpu_model().unwrap_or_else(|| format!("unknown-{}", arch)),
            cores,
            threads: cores,
            frequency_mhz: probe::cpu_freq_mhz(),
        };

        let cache = probe::cache_raw()
            .into_iter()
            .map(|(level, ctype, size)| CacheLevel {
                level: level.parse().unwrap_or(0),
                kind: cache_kind(&level, &ctype),
                size_bytes: probe::parse_size_bytes(&size),
            })
            .collect();

        let memory = MemoryReport {
            capacity_bytes: probe::mem_total_bytes(),
            channels: 0, // 0 = tidak diketahui; OS dapat menimpa via sysfs
        };

        let gpu = probe::gpu_raw().map(|(name, compute_units, feats)| GpuReport {
            fp32: feats.contains("fp32") || feats.is_empty(),
            fp16: feats.contains("fp16"),
            bf16: feats.contains("bf16"),
            int8: feats.contains("int8"),
            name,
            compute_units,
        });

        let (noc_type, noc_topo) = probe::noc_raw();
        let interconnect = InterconnectReport {
            kind: noc_type.unwrap_or_else(|| "on-chip".into()),
            topology: noc_topo.unwrap_or_else(|| "unknown".into()),
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
        // CLOCK_MONOTONIC via std::time::Instant.
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        let start = START.get_or_init(std::time::Instant::now);
        start.elapsed().as_nanos() as u64
    }

    fn cycles(&self) -> u64 {
        // Estimasi ns × frekuensi. Implementasi OS cycle-accurate menimpa
        // dengan counter sejati (rdtsc/timebase) tanpa mengubah trait.
        self.timer_ns().saturating_mul(self.cpu_frequency_hz()) / 1_000_000_000
    }

    fn cpu_frequency_hz(&self) -> u64 {
        // `cycles()` memanggil ini dua kali per iterasi benchmark. Tanpa cache,
        // setiap panggilan membaca /proc/cpuinfo + sysfs dan menggeser hasil
        // pengukuran. Frekuensi hardware tidak berubah selama satu run.
        static FREQ_HZ: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
        *FREQ_HZ.get_or_init(|| {
            probe::scaling_freq_hz().unwrap_or_else(|| {
                let mhz = self.hardware_report().cpu.frequency_mhz;
                if mhz > 0 {
                    mhz * 1_000_000
                } else {
                    1_000_000_000 // konservatif: 1 GHz default
                }
            })
        })
    }

    fn cpu_count(&self) -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }

    #[cfg(target_os = "linux")]
    fn pin_to_cpu(&self, cpu: usize) -> Result<(), String> {
        let allowed = allowed_cpus()?;
        let target = allowed
            .get(cpu)
            .copied()
            .ok_or_else(|| format!("cpu {} di luar jumlah cpu {}", cpu, allowed.len()))?;
        // SAFETY: cpu_set_t di-zero-init, hanya satu bit di-set, dan ukuran
        // yang diberikan sesuai dengan tipe yang dipakai kernel.
        unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_ZERO(&mut set);
            libc::CPU_SET(target, &mut set);
            if libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) != 0 {
                return Err(format!(
                    "sched_setaffinity cpu {}: {}",
                    target,
                    std::io::Error::last_os_error()
                ));
            }
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    fn pin_to_cpu(&self, cpu: usize) -> Result<(), String> {
        let n = self.cpu_count();
        if cpu >= n {
            return Err(format!("cpu {} di luar jumlah cpu {}", cpu, n));
        }
        Err("pin_to_cpu hanya didukung di Linux".into())
    }

    fn dma_scratch(&self, size: usize) -> Result<Vec<u8>, String> {
        if size == 0 {
            return Err("size 0".into());
        }
        // Buffer besar untuk workload DMA/NoC. Alignment mengikuti allocator
        // (bukan jaminan page-aligned). OS dengan IOMMU memetakan buffer user
        // ke region DMA.
        let mut v = Vec::with_capacity(size + 64);
        v.resize(size, 0u8);
        Ok(v)
    }
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
    fn frequency_is_cached_and_positive() {
        let api = MivonHardwareApi::new();
        let first = api.cpu_frequency_hz();
        assert!(first > 0);
        assert_eq!(first, api.cpu_frequency_hz());
    }

    #[test]
    fn pin_range_checked() {
        let api = MivonHardwareApi::new();
        let n = api.cpu_count();
        assert!(api.pin_to_cpu(n + 100).is_err());
        assert!(api.pin_to_cpu(0).is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn pin_to_cpu_applies_affinity() {
        let allowed = allowed_cpus().unwrap();
        assert!(!allowed.is_empty(), "set CPU yang diizinkan kosong");

        let api = MivonHardwareApi::new();
        api.pin_to_cpu(0).unwrap();

        // SAFETY: sched_getcpu tidak punya prasyarat memori.
        let current = unsafe { libc::sched_getcpu() };
        assert!(current >= 0, "sched_getcpu gagal");
        assert_eq!(
            current as usize, allowed[0],
            "thread harus berjalan di CPU pertama yang diizinkan"
        );
    }

    #[test]
    fn dma_scratch_size() {
        let api = MivonHardwareApi::new();
        assert_eq!(api.dma_scratch(1024).unwrap().len(), 1024);
        assert!(api.dma_scratch(0).is_err());
    }
}
