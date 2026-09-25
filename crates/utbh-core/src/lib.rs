//! `utbh-core` — shared types, data-driven test loader, deterministic inputs.
//!
//! Semua tipe hasil/schema UTBH ada di sini. `SCHEMA_VERSION` wajib di-bump
//! setiap kali format hasil berubah (lihat AGENTS.md aturan #3).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_SUITES_DIR: &str = "suites";
pub const DEFAULT_RESULTS_DIR: &str = "results";

// ---------------------------------------------------------------------------
// Test definition (data-driven, TOML `*.utbh`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDef {
    pub test: TestMeta,
    pub input: InputSpec,
    pub operation: OperationSpec,
    pub validation: ValidationSpec,
    #[serde(default)]
    pub benchmark: BenchmarkSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestMeta {
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSpec {
    pub elements: usize,
    #[serde(rename = "type", default = "default_dtype")]
    pub dtype: String,
    #[serde(default)]
    pub seed: Option<u64>,
}

fn default_dtype() -> String {
    "f32".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationSpec {
    pub op: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSpec {
    /// `exact` | `approx` | `none`
    pub mode: String,
    #[serde(default)]
    pub tolerance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSpec {
    #[serde(default)]
    pub metrics: Vec<String>,
    #[serde(default = "default_iterations")]
    pub iterations: u32,
}

impl Default for BenchmarkSpec {
    fn default() -> Self {
        Self {
            metrics: Vec::new(),
            iterations: default_iterations(),
        }
    }
}

fn default_iterations() -> u32 {
    5
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

/// Data value yang dipakai workload & validator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Data {
    F32(Vec<f32>),
    U64(Vec<u64>),
    I64(Vec<i64>),
}

impl Data {
    pub fn len(&self) -> usize {
        match self {
            Data::F32(v) => v.len(),
            Data::U64(v) => v.len(),
            Data::I64(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dtype(&self) -> &'static str {
        match self {
            Data::F32(_) => "f32",
            Data::U64(_) => "u64",
            Data::I64(_) => "i64",
        }
    }

    pub fn elem_size(&self) -> usize {
        match self {
            Data::F32(_) => 4,
            Data::U64(_) | Data::I64(_) => 8,
        }
    }

    /// Representasi string satu elemen (untuk trace mismatch).
    pub fn fmt_at(&self, i: usize) -> Option<String> {
        match self {
            Data::F32(v) => v.get(i).map(|x| format!("{}", x)),
            Data::U64(v) => v.get(i).map(|x| x.to_string()),
            Data::I64(v) => v.get(i).map(|x| x.to_string()),
        }
    }
}

/// Pasangan input untuk operasi. `b` hanya untuk operasi dua-operand.
pub struct Inputs {
    pub a: Data,
    pub b: Option<Data>,
}

// ---------------------------------------------------------------------------
// RNG deterministik (xorshift64*) — dipakai input generator & fuzzer
// ---------------------------------------------------------------------------

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Input deterministik: seed = `[input].seed` atau hash nama test.
/// Workload dan reference WAJIB memakai fungsi ini supaya input identik.
pub fn inputs(def: &TestDef) -> Inputs {
    let seed = def.input.seed.unwrap_or_else(|| fnv1a(&def.test.name));
    let n = def.input.elements.max(1);
    let a = gen_data(&def.input.dtype, seed, n);
    let needs_b = matches!(def.operation.op.as_str(), "add" | "mul" | "matmul");
    let b = needs_b.then(|| gen_data(&def.input.dtype, seed ^ 0xA5A5_A5A5_A5A5_A5A5, n));
    Inputs { a, b }
}

pub fn gen_data(dtype: &str, seed: u64, n: usize) -> Data {
    match dtype {
        "u64" => Data::U64(gen_u64(seed, n)),
        "i64" => Data::I64(gen_i64(seed, n)),
        _ => Data::F32(gen_f32(seed, n)),
    }
}

pub fn gen_f32(seed: u64, n: usize) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|_| (rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32)
        .collect()
}

pub fn gen_u64(seed: u64, n: usize) -> Vec<u64> {
    let mut rng = Rng::new(seed);
    (0..n).map(|_| rng.next_u64()).collect()
}

pub fn gen_i64(seed: u64, n: usize) -> Vec<i64> {
    let mut rng = Rng::new(seed);
    (0..n).map(|_| (rng.next_u64() >> 1) as i64).collect()
}

// ---------------------------------------------------------------------------
// Metric / outcome / result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationInfo {
    pub mode: String,
    pub matched: bool,
    pub mismatches: usize,
    pub first_mismatch: Option<usize>,
    pub max_rel_error: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Status {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestOutcome {
    pub name: String,
    pub category: String,
    pub status: Status,
    pub duration_ns: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<Metric>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TestOutcome {
    pub fn failed(name: &str, category: &str, msg: String) -> Self {
        TestOutcome {
            name: name.to_string(),
            category: category.to_string(),
            status: Status::Fail,
            duration_ns: 0,
            validation: None,
            metrics: Vec::new(),
            error: Some(msg),
        }
    }

    pub fn metric(&self, name: &str) -> Option<&Metric> {
        self.metrics.iter().find(|m| m.name == name)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FuzzSummary {
    pub base_seed: u64,
    pub iterations: usize,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub schema_version: u32,
    pub run_id: String,
    pub unix_time: u64,
    /// `test` | `benchmark` | `run` | `fuzz`
    pub kind: String,
    pub suite: String,
    pub hardware: HardwareReport,
    pub outcomes: Vec<TestOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuzz: Option<FuzzSummary>,
}

impl RunResult {
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

// ---------------------------------------------------------------------------
// Hardware report (diisi Layer 1/2)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Loader test package (data-driven — tanpa rebuild binary)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Util
// ---------------------------------------------------------------------------

pub fn new_run_id() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}-{:06}", d.as_secs(), d.subsec_micros())
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[test]
name = "vector_add"
category = "cpu.vector"

[input]
elements = 1024
type = "f32"

[operation]
op = "add"

[validation]
mode = "exact"

[benchmark]
metrics = ["latency", "throughput"]
"#;

    #[test]
    fn parse_test_def() {
        let def: TestDef = toml::from_str(SAMPLE).expect("parse");
        assert_eq!(def.test.name, "vector_add");
        assert_eq!(def.input.elements, 1024);
        assert_eq!(def.input.dtype, "f32");
        assert_eq!(def.operation.op, "add");
        assert_eq!(def.validation.mode, "exact");
        assert_eq!(def.benchmark.metrics.len(), 2);
        assert_eq!(def.benchmark.iterations, 5);
    }

    #[test]
    fn inputs_deterministic() {
        let def: TestDef = toml::from_str(SAMPLE).unwrap();
        let a = inputs(&def);
        let b = inputs(&def);
        assert_eq!(a.a, b.a);
        assert_eq!(a.b, b.b);
        assert_eq!(a.a.len(), 1024);
        assert!(a.b.is_some());
    }

    #[test]
    fn rng_reproducible() {
        let mut r1 = Rng::new(42);
        let mut r2 = Rng::new(42);
        for _ in 0..16 {
            assert_eq!(r1.next_u64(), r2.next_u64());
        }
    }

    #[test]
    fn schema_version_pinned() {
        assert_eq!(SCHEMA_VERSION, 1);
    }
}
