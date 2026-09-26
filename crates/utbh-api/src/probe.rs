//! Probe hardware — pembaca tunggal `/proc` + `/sys`.
//!
//! Seluruh crate UTBH dilarang akses langsung ke system files; hanya modul
//! ini (Layer 1) yang boleh. Hasilnya berupa data mentah untuk [`crate::mivon`].
//!
//! Path `/proc` + `/sys` = kontrak guest Linux (ISA guest mivon emu: RISC-V
//! `riscv64gc-unknown-linux-gnu`).

/// Baca file, trim, `None` bila gagal.
pub fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// `"64K"` / `"1M"` / `"16G"` → bytes.
pub fn parse_size_bytes(s: &str) -> u64 {
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

/// Model CPU dari `/proc/cpuinfo` (multi-ISA).
pub fn cpu_model() -> Option<String> {
    let info = read("/proc/cpuinfo")?;
    for key in ["model name", "Model", "cpu model"] {
        for line in info.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if k.trim() == key && !v.trim().is_empty() {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// Flags/features dari `/proc/cpuinfo` (string mentah, untuk deteksi vector).
pub fn cpu_flags() -> String {
    read("/proc/cpuinfo").unwrap_or_default()
}

/// Cache levels cpu0 dari sysfs (`index0`, `index1`, ...).
/// Mengembalikan `(level, type, size)` apa adanya.
pub fn cache_raw() -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for i in 0.. {
        let base = format!("/sys/devices/system/cpu/cpu0/cache/index{}", i);
        let (Some(level), Some(ctype), Some(size)) = (
            read(&format!("{}/level", base)),
            read(&format!("{}/type", base)),
            read(&format!("{}/size", base)),
        ) else {
            break;
        };
        out.push((level, ctype, size));
    }
    out
}

/// `MemTotal` dalam bytes.
pub fn mem_total_bytes() -> u64 {
    read("/proc/meminfo")
        .and_then(|m| {
            m.lines()
                .find_map(|l| l.strip_prefix("MemTotal:"))
                .and_then(|v| v.split_whitespace().next())
                .and_then(|kb| kb.parse::<u64>().ok())
        })
        .map(|kb| kb * 1024)
        .unwrap_or(0)
}

/// Frekuensi maksimum CPU (kHz → MHz), fallback ke `/proc/cpuinfo`.
pub fn cpu_freq_mhz() -> u64 {
    read("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        .and_then(|khz| khz.parse::<u64>().ok())
        .map(|khz| khz / 1000)
        .filter(|m| *m > 0)
        .unwrap_or_else(|| {
            read("/proc/cpuinfo")
                .and_then(|info| {
                    info.lines().find_map(|l| {
                        let (k, v) = l.split_once(':')?;
                        let k = k.trim();
                        if k == "cpu MHz" || k == "clock" {
                            v.trim().trim_end_matches(" MHz").trim().parse::<f64>().ok()
                        } else {
                            None
                        }
                    })
                })
                .map(|mhz| mhz as u64)
                .unwrap_or(0)
        })
}

/// Frekuensi berjalan saat ini (kHz → Hz), untuk siklus counter.
pub fn scaling_freq_hz() -> Option<u64> {
    read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
        .and_then(|khz| khz.parse::<u64>().ok())
        .map(|khz| khz * 1000)
}

/// GPU via DRM sysfs. `None` bila tidak terpasang.
pub fn gpu_raw() -> Option<(String, u32, String)> {
    if !std::path::Path::new("/sys/class/drm").exists() {
        return None;
    }
    let name = read("/sys/class/drm/card0/device/product")
        .or_else(|| read("/sys/class/drm/card0/device/label"))
        .unwrap_or_else(|| "gpu0".into());
    let cu = read("/sys/class/drm/card0/device/compute_units")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let feats = read("/sys/class/drm/card0/device/compute_features").unwrap_or_default();
    Some((name, cu, feats))
}

/// Interconnect dari sysfs NoC.
pub fn noc_raw() -> (Option<String>, Option<String>) {
    (read("/sys/class/noc/type"), read("/sys/class/noc/topology"))
}

/// Deteksi ISA vector dari arch + flags cpuinfo.
pub fn detect_vector(arch: &str, cpuinfo: &str) -> String {
    if arch.contains("riscv") {
        return if cpuinfo.contains("rvv") || cpuinfo.contains(" v ") {
            "RVV".into()
        } else {
            "none".into()
        };
    }
    if arch == "aarch64" {
        return if cpuinfo.contains("asimd") || cpuinfo.contains("neon") {
            "NEON/ASIMD".into()
        } else {
            "none".into()
        };
    }
    for f in ["avx512f", "avx2", "sse4.2", "sse2"] {
        if cpuinfo.contains(f) {
            return f.to_uppercase();
        }
    }
    "none".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sizes() {
        assert_eq!(parse_size_bytes("64K"), 64 * 1024);
        assert_eq!(parse_size_bytes("1M"), 1024 * 1024);
        assert_eq!(parse_size_bytes("16M"), 16 * 1024 * 1024);
    }

    #[test]
    fn read_missing_is_none() {
        assert!(read("/proc/definitely-not-here-utbh").is_none());
    }

    #[test]
    fn vector_detect_x86() {
        let flags = "flags : fpu sse2 avx2 avx512f";
        assert_eq!(detect_vector("x86_64", flags), "AVX512F");
        assert_eq!(detect_vector("x86_64", "flags : fpu sse2"), "SSE2");
        assert_eq!(detect_vector("riscv64", "rvv"), "RVV");
    }

    #[test]
    fn mem_probe_never_panics() {
        let _ = mem_total_bytes();
        let _ = cache_raw();
        let _ = cpu_freq_mhz();
    }
}
