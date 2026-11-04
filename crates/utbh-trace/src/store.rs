//! Simpan/muat `.utbh-trace.json`.
//!
//! Satu file per **(run_id, test)** — beberapa test FAIL dalam satu run tidak
//! saling menimpa. `load_all` mengambil semua trace untuk satu run-id.

use crate::event::Trace;
use std::path::{Path, PathBuf};

/// Komponen nama file yang aman: tanpa `/`, `\`, atau `..`.
///
/// `test.name` berasal dari file data, jadi tidak boleh dipercaya sebagai
/// path component apa adanya.
fn no_trace_message(run_id: &str, dir: &Path) -> String {
    format!(
        "tidak ada trace untuk run-id '{}' di {} (trace hanya dihasilkan saat ada FAIL)",
        run_id,
        dir.display()
    )
}

fn safe_component(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Nama file trace: `<run_id>.<test>.utbh-trace.json`.
pub fn filename(trace: &Trace) -> String {
    format!(
        "{}.{}.utbh-trace.json",
        safe_component(&trace.run_id),
        safe_component(&trace.test)
    )
}

pub fn save(trace: &Trace, dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(filename(trace));
    let json = serde_json::to_vec_pretty(trace)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    utbh_core::write_atomic(&path, &json)?;
    Ok(path)
}

pub fn load(path: &Path) -> Result<Trace, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {}", path.display(), e))
}

/// Semua trace milik satu run-id (prefix match), terurut per nama test.
pub fn load_all(dir: &Path, run_id: &str) -> Result<Vec<Trace>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        // Direktori results belum ada = belum ada trace, bukan I/O error mentah.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(no_trace_message(run_id, dir));
        }
        Err(e) => return Err(format!("{}: {}", dir.display(), e)),
    };
    let prefix = safe_component(run_id);
    let mut traces = Vec::new();
    let mut last_err: Option<String> = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // Prefix harus diikuti '.' — "run-1" tidak boleh menangkap "run-10".
        let after = name.strip_prefix(&prefix);
        let matched = matches!(after, Some(rest) if rest.starts_with('.'))
            && name.ends_with(".utbh-trace.json");
        if matched {
            match load(&entry.path()) {
                Ok(t) => traces.push(t),
                Err(e) => last_err = Some(e),
            }
        }
    }
    if traces.is_empty() {
        return Err(last_err.unwrap_or_else(|| no_trace_message(run_id, dir)));
    }
    traces.sort_by(|a, b| a.test.cmp(&b.test));
    Ok(traces)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(run_id: &str, test: &str) -> Trace {
        Trace {
            run_id: run_id.into(),
            test: test.into(),
            category: "cpu".into(),
            events: vec![],
        }
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("utbh-trace-{}", std::process::id()));
        let t = sample("run-42", "vector_add");
        let p = save(&t, &dir).unwrap();
        let back = load(&p).unwrap();
        assert_eq!(back.run_id, "run-42");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn multi_failure_no_overwrite() {
        let dir = std::env::temp_dir().join(format!("utbh-trace-multi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        save(&sample("run-1", "test_a"), &dir).unwrap();
        save(&sample("run-1", "test_b"), &dir).unwrap();
        let all = load_all(&dir, "run-1").unwrap();
        assert_eq!(all.len(), 2, "dua FAIL harus jadi dua file");
        let names: Vec<&str> = all.iter().map(|t| t.test.as_str()).collect();
        assert_eq!(names, vec!["test_a", "test_b"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_all_isolated_per_run() {
        let dir = std::env::temp_dir().join(format!("utbh-trace-iso-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        save(&sample("run-1", "a"), &dir).unwrap();
        save(&sample("run-10", "b"), &dir).unwrap();
        // "run-1" tidak boleh menangkap "run-10"
        let all = load_all(&dir, "run-1").unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].test, "a");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn filename_is_path_safe() {
        let t = sample("run/../1", "a/b");
        let name = filename(&t);
        assert!(
            !name.contains('/'),
            "nama trace tidak boleh berisi path: {}",
            name
        );
        assert_eq!(name, "run____1.a_b.utbh-trace.json");
    }

    #[test]
    fn sanitized_run_id_still_loads() {
        let dir = std::env::temp_dir().join(format!("utbh-trace-safe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        save(&sample("run/../1", "a/b"), &dir).unwrap();
        let all = load_all(&dir, "run/../1").unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].test, "a/b");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_run_errors() {
        let dir = std::env::temp_dir().join(format!("utbh-trace-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let err = load_all(&dir, "nope").unwrap_err();
        assert!(err.contains("tidak ada trace"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_results_dir_reports_no_trace() {
        let dir = std::env::temp_dir().join(format!("utbh-trace-nodir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let err = load_all(&dir, "nope").unwrap_err();
        assert!(
            err.contains("tidak ada trace"),
            "error harus ramah, bukan I/O mentah: {}",
            err
        );
    }
}
