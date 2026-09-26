//! Generator laporan gabungan markdown (.utbh-report.md) dari riwayat eksekusi di results/.

use std::path::Path;
use utbh_core::{RunResult, Status, SCHEMA_VERSION};

/// Render `.utbh-report` (markdown) dari semua run di results/.
pub fn render_report(results_dir: &Path) -> Result<String, String> {
    let mut runs = Vec::new();
    let entries =
        std::fs::read_dir(results_dir).map_err(|e| format!("{}: {}", results_dir.display(), e))?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("json")
            && p.to_string_lossy().ends_with(".utbh-result.json")
        {
            match RunResult::load(&p) {
                Ok(r) => runs.push(r),
                Err(e) => eprintln!("skip {}: {}", p.display(), e),
            }
        }
    }
    if runs.is_empty() {
        return Err(format!("tidak ada hasil di {}", results_dir.display()));
    }
    runs.sort_by_key(|r| r.unix_time);

    let mut out = String::from("# UTBH Report\n\n");
    out.push_str(&format!("Schema version: {}\n\n", SCHEMA_VERSION));

    for r in &runs {
        out.push_str(&format!(
            "## {} — {} `{}` ({})\n\n",
            r.kind, r.suite, r.run_id, r.unix_time
        ));
        out.push_str(&format!(
            "Hardware: {} {} / {} cores / {} GiB\n\n",
            r.hardware.cpu.architecture,
            r.hardware.cpu.model,
            r.hardware.cpu.cores,
            r.hardware.memory.capacity_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
        ));
        if let Some(f) = &r.fuzz {
            out.push_str(&format!(
                "Fuzz: seed={} iterations={} PASS={} FAIL={}\n\n",
                f.base_seed, f.iterations, f.passed, f.failed
            ));
        }

        out.push_str("| Test | Category | Result | Latency | Throughput | Bandwidth | GFLOPS |\n");
        out.push_str("|---|---|---|---|---|---|---|\n");
        for o in &r.outcomes {
            let res = match o.status {
                Status::Pass => "PASS",
                Status::Fail => "FAIL",
            };
            let get = |name: &str| {
                o.metric(name)
                    .map(|m| format!("{:.3} {}", m.value, m.unit))
                    .unwrap_or_else(|| "-".into())
            };
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                o.name,
                o.category,
                res,
                get("latency"),
                get("throughput"),
                get("bandwidth"),
                get("gflops"),
            ));
        }
        out.push('\n');
    }

    out.push_str(
        "> Testing menjawab \"benar?\", benchmark menjawab \"seberapa cepat?\". \
         Skor tunggal tidak pernah menjadi satu-satunya hasil.\n",
    );

    // Tulis file juga.
    std::fs::create_dir_all(results_dir)
        .map_err(|e| format!("{}: {}", results_dir.display(), e))?;
    let path = results_dir.join(".utbh-report.md");
    std::fs::write(&path, &out).map_err(|e| format!("{}: {}", path.display(), e))?;
    eprintln!("report → {}", path.display());
    Ok(out)
}
