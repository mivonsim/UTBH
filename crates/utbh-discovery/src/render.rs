//! Render laporan discovery sebagai teks (output `utbh discover`).

use crate::Discovery;

fn kib(n: u64) -> String {
    if n >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", n as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if n >= 1024 * 1024 {
        format!("{:.1} MiB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{:.1} KiB", n as f64 / 1024.0)
    } else {
        format!("{} B", n)
    }
}

fn y(b: bool) -> &'static str {
    if b { "supported" } else { "no" }
}

/// Render laporan discovery seperti output `utbh discover`.
pub fn format_report(d: &Discovery) -> String {
    let r = &d.report;
    let mut out = String::from("UTBH Hardware Discovery\n\n");

    out.push_str("CPU\n");
    out.push_str(&format!("  Architecture : {}\n", r.cpu.architecture));
    out.push_str(&format!("  Model        : {}\n", r.cpu.model));
    out.push_str(&format!("  Cores        : {}\n", r.cpu.cores));
    out.push_str(&format!("  Threads      : {}\n", r.cpu.threads));
    out.push_str(&format!("  Vector       : {}\n", r.cpu.vector));
    if r.cpu.frequency_mhz > 0 {
        out.push_str(&format!("  Frequency    : {} MHz\n", r.cpu.frequency_mhz));
    }
    for c in &r.cache {
        out.push_str(&format!("  {:<11}: {}\n", c.kind, kib(c.size_bytes)));
    }
    out.push('\n');

    out.push_str("GPU\n");
    match &r.gpu {
        Some(g) => {
            out.push_str(&format!("  Name          : {}\n", g.name));
            out.push_str(&format!("  Compute Units : {}\n", g.compute_units));
            out.push_str(&format!(
                "  Features      : FP32={} FP16={} BF16={} INT8={}\n",
                y(g.fp32),
                y(g.fp16),
                y(g.bf16),
                y(g.int8)
            ));
        }
        None => out.push_str("  (none detected)\n"),
    }
    out.push('\n');

    out.push_str("Memory\n");
    out.push_str(&format!("  Capacity : {}\n", kib(r.memory.capacity_bytes)));
    out.push_str(&format!("  Channels : {}\n", r.memory.channels));
    if let Some(bw) = d.mem_bandwidth_gbs {
        out.push_str(&format!("  Bandwidth: {:.1} GB/s\n", bw));
    }
    out.push('\n');

    out.push_str("Interconnect\n");
    out.push_str(&format!("  Type     : {}\n", r.interconnect.kind));
    out.push_str(&format!("  Topology : {}\n", r.interconnect.topology));

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use utbh_core::HardwareReport;

    #[test]
    fn format_sizes() {
        assert_eq!(kib(0), "0 B");
        assert_eq!(kib(64 * 1024), "64.0 KiB");
        assert_eq!(kib(2 * 1024 * 1024 * 1024), "2.0 GiB");
    }

    #[test]
    fn format_default_report() {
        let d = Discovery { report: HardwareReport::default(), mem_bandwidth_gbs: None };
        let text = format_report(&d);
        assert!(text.contains("(none detected)"));
        assert!(text.contains("Interconnect"));
        assert!(text.contains("Topology"));
    }
}
