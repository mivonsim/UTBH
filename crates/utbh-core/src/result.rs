//! `RunResult` — artefak hasil satu run, disimpan sebagai `.utbh-result.json`.

use crate::hardware::HardwareReport;
use crate::outcome::{FuzzSummary, Status, TestOutcome};
use crate::SCHEMA_VERSION;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub schema_version: u32,
    pub run_id: String,
    pub unix_time: u64,
    /// `test` | `benchmark` | `run` | `fuzz` | `stress`
    pub kind: String,
    pub suite: String,
    pub hardware: HardwareReport,
    pub outcomes: Vec<TestOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuzz: Option<FuzzSummary>,
}

impl RunResult {
    /// Konstruksi dengan schema version aktif.
    pub fn new(
        run_id: impl Into<String>,
        kind: impl Into<String>,
        suite: impl Into<String>,
        hardware: HardwareReport,
        outcomes: Vec<TestOutcome>,
        fuzz: Option<FuzzSummary>,
    ) -> Self {
        RunResult {
            schema_version: SCHEMA_VERSION,
            run_id: run_id.into(),
            unix_time: crate::util::now_unix(),
            kind: kind.into(),
            suite: suite.into(),
            hardware,
            outcomes,
            fuzz,
        }
    }

    pub fn passed(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.status == Status::Pass)
            .count()
    }

    pub fn failed(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.status == Status::Fail)
            .count()
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{}.utbh-result.json", self.run_id));
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&path, json)?;
        Ok(path)
    }

    pub fn load(path: &Path) -> Result<RunResult, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {}", path.display(), e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RunResult {
        RunResult::new(
            "run-1",
            "test",
            "cpu",
            HardwareReport::default(),
            vec![],
            None,
        )
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("utbh-test-{}", std::process::id()));
        let run = sample();
        let path = run.save(&dir).unwrap();
        let back = RunResult::load(&path).unwrap();
        assert_eq!(back.run_id, "run-1");
        assert_eq!(back.schema_version, SCHEMA_VERSION);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn counts_pass_fail() {
        let mut run = sample();
        run.outcomes
            .push(crate::outcome::TestOutcome::failed("a", "cpu", "x".into()));
        assert_eq!(run.failed(), 1);
        assert_eq!(run.passed(), 0);
    }
}
