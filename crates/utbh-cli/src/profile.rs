//! Profil run — muat parameter dari `configs/*.json`.
//!
//! Profil = preset parameter CLI (suite, iterasi, fuzz/stress). Bukan test
//! definition — test tetap data-driven di `suites/`.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Profil run. Semua field opsional — yang tidak diisi pakai default CLI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Daftar suite, atau `["universal"]`.
    #[serde(default)]
    pub suites: Vec<String>,
    #[serde(default)]
    pub iterations: Option<u32>,
    #[serde(default)]
    pub warmup: Option<u32>,
    #[serde(default)]
    pub fuzz_iterations: Option<usize>,
    #[serde(default)]
    pub stress_iterations: Option<usize>,
    #[serde(default)]
    pub stress_window: Option<usize>,
}

impl Profile {
    pub fn load(path: &Path) -> Result<Profile, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("{}: {}", path.display(), e))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
    }

    /// Suite pertama dalam profil (CLI positional tetap menang bila diisi).
    pub fn primary_suite(&self) -> Option<&str> {
        self.suites.first().map(String::as_str)
    }

    /// Validasi nilai yang harus positif.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(0) = self.iterations {
            return Err(format!("profil '{}': iterations harus > 0", self.name));
        }
        if let Some(0) = self.warmup {
            return Err(format!("profil '{}': warmup harus > 0", self.name));
        }
        if let Some(0) = self.fuzz_iterations {
            return Err(format!("profil '{}': fuzz_iterations harus > 0", self.name));
        }
        if let Some(0) = self.stress_iterations {
            return Err(format!("profil '{}': stress_iterations harus > 0", self.name));
        }
        if let Some(0) = self.stress_window {
            return Err(format!("profil '{}': stress_window harus > 0", self.name));
        }
        Ok(())
    }

    /// Resolusi suite: flag eksplisit > suite profil > `"universal"`.
    ///
    /// `flag == None` → pakai suite profil (atau `universal` kalau tanpa profil).
    pub fn resolve_suite_opt(prof: Option<&Profile>, flag: Option<&str>) -> String {
        if let Some(s) = flag {
            return s.to_string();
        }
        prof.and_then(|p| p.primary_suite()).unwrap_or("universal").to_string()
    }

    /// Resolusi nilai numerik: flag eksplisit > profil > default.
    pub fn resolve<T: Copy>(flag: Option<T>, from_profile: Option<T>, default: T) -> T {
        flag.or(from_profile).unwrap_or(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_repo_configs() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../configs");
        if !root.is_dir() {
            return;
        }
        let mut found = 0;
        for entry in std::fs::read_dir(&root).unwrap().flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("json") {
                let prof = Profile::load(&p).unwrap_or_else(|e| panic!("{}", e));
                assert!(!prof.name.is_empty(), "{:?} tanpa name", p);
                assert!(prof.validate().is_ok(), "{:?}", p);
                found += 1;
            }
        }
        assert!(found >= 3, "minimal quick/full/ci, dapat {}", found);
    }

    #[test]
    fn rejects_zero_values() {
        let p = Profile { name: "x".into(), iterations: Some(0), ..Default::default() };
        assert!(p.validate().is_err());
    }

    #[test]
    fn primary_suite() {
        let p = Profile { suites: vec!["cpu".into(), "gpu".into()], ..Default::default() };
        assert_eq!(p.primary_suite(), Some("cpu"));
        assert_eq!(Profile::default().primary_suite(), None);
    }
}
