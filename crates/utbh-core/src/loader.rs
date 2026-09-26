//! Loader test package — baca `suites/**/**.utbh` (TOML) saat runtime.
//!
//! Data-driven: tambah test = tambah file, tanpa rebuild binary.

use crate::testdef::TestDef;
use std::path::{Path, PathBuf};

pub fn load_test_file(path: &Path) -> Result<TestDef, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    toml::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

fn collect_test_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {}", dir.display(), e))?;
        let path = entry.path();
        if path.is_dir() {
            collect_test_files(&path, out)?;
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // manifest bukan test definition
            if name.starts_with("manifest.") {
                continue;
            }
            if name.ends_with(".utbh") || name.ends_with(".toml") {
                out.push(path);
            }
        }
    }
    Ok(())
}

pub fn list_suites(root: &Path) -> Vec<String> {
    let mut suites = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    suites.push(name.to_string());
                }
            }
        }
    }
    suites.sort();
    suites
}

/// `suite == "all" | "universal"` → semua suite.
pub fn load_suite(root: &Path, suite: &str) -> Result<Vec<TestDef>, String> {
    if suite == "all" || suite == "universal" {
        return load_all(root);
    }
    let dir = root.join(suite);
    if !dir.is_dir() {
        let available = list_suites(root).join(", ");
        return Err(format!(
            "suite '{}' tidak ditemukan di {} (tersedia: {})",
            suite,
            root.display(),
            available
        ));
    }
    let mut files = Vec::new();
    collect_test_files(&dir, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(format!("suite '{}' tidak berisi test *.utbh", suite));
    }
    files.iter().map(|f| load_test_file(f)).collect()
}

pub fn load_all(root: &Path) -> Result<Vec<TestDef>, String> {
    let mut all = Vec::new();
    for suite in list_suites(root) {
        all.extend(load_suite(root, &suite)?);
    }
    if all.is_empty() {
        return Err(format!("tidak ada test di {}", root.display()));
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_suite_lists_available() {
        let dir = std::env::temp_dir().join(format!("utbh-loader-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("cpu")).unwrap();
        let err = load_suite(&dir, "gpu").unwrap_err();
        assert!(err.contains("tersedia: cpu"), "err = {}", err);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_repo_suites_if_present() {
        // Dijalankan dari workspace root saat `cargo test`.
        let root = Path::new("suites");
        if root.is_dir() {
            let defs = load_all(root).expect("suites repo valid");
            assert!(!defs.is_empty());
            for d in defs {
                assert!(
                    !d.validation.mode.is_empty(),
                    "{} tanpa validation",
                    d.test.name
                );
            }
        }
    }
}
