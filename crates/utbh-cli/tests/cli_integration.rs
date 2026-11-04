//! Integrasi CLI end-to-end: setiap subcommand dijalankan sebagai binary asli.
//!
//! Fixture ditulis ke direktori sementara supaya test tidak bergantung pada
//! `suites/` repo dan tidak menulis ke `results/` kerja.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TINY: &str = r#"[test]
name = "tiny_add"
category = "cpu.vector"
description = "fixture integrasi CLI"

[input]
elements = 64
type = "f32"
seed = 7

[operation]
op = "add"

[validation]
mode = "exact"

[benchmark]
metrics = ["latency", "throughput", "bandwidth", "cycles"]
iterations = 2
"#;

fn utbh() -> Command {
    Command::new(env!("CARGO_BIN_EXE_utbh"))
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Fixture {
        let root =
            std::env::temp_dir().join(format!("utbh-cli-it-{}-{}", std::process::id(), label));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("suites/tiny")).unwrap();
        fs::write(root.join("suites/tiny/add.utbh"), TINY).unwrap();
        Fixture { root }
    }

    fn suites(&self) -> PathBuf {
        self.root.join("suites")
    }

    fn results(&self) -> PathBuf {
        self.root.join("results")
    }

    fn run(&self, args: &[&str]) -> Output {
        utbh()
            .arg("--suites")
            .arg(self.suites())
            .arg("--results")
            .arg(self.results())
            .args(args)
            .output()
            .expect("jalankan utbh")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn files_ending_with(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.to_string_lossy().ends_with(suffix))
                .collect()
        })
        .unwrap_or_default()
}

fn result_files(dir: &Path) -> Vec<PathBuf> {
    files_ending_with(dir, ".utbh-result.json")
}

fn load_result(path: &Path) -> serde_json::Value {
    let bytes = fs::read(path).expect("baca result");
    serde_json::from_slice(&bytes).expect("result JSON valid")
}

#[test]
fn cli_help_works() {
    let output = utbh().arg("--help").output().expect("jalankan utbh --help");
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(
        text.contains("Universal") || text.contains("utbh"),
        "{}",
        text
    );
}

#[test]
fn cli_discover_works() {
    let output = utbh()
        .arg("discover")
        .output()
        .expect("jalankan utbh discover");
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("UTBH Hardware Discovery"), "{}", text);
    assert!(text.contains("CPU"));
}

#[test]
fn cli_list_works() {
    let f = Fixture::new("list");
    let output = f.run(&["list"]);
    assert!(output.status.success(), "stderr={}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("tiny/"), "{}", text);
    assert!(text.contains("tiny_add"), "{}", text);
}

#[test]
fn cli_test_writes_schema_valid_result_without_trace() {
    let f = Fixture::new("test");
    let output = f.run(&["test", "tiny"]);
    assert!(output.status.success(), "stderr={}", stderr(&output));
    assert!(stdout(&output).contains("PASS"));

    let results = result_files(&f.results());
    assert_eq!(results.len(), 1, "satu run = satu result");
    let run = load_result(&results[0]);
    assert_eq!(run["schema_version"], 1);
    assert_eq!(run["kind"], "test");
    assert_eq!(run["suite"], "tiny");
    assert_eq!(run["outcomes"][0]["status"], "PASS");
    assert!(
        files_ending_with(&f.results(), ".utbh-trace.json").is_empty(),
        "test PASS tidak boleh menghasilkan trace"
    );
}

#[test]
fn cli_benchmark_reports_multiple_metrics() {
    let f = Fixture::new("benchmark");
    let output = f.run(&["benchmark", "tiny", "--iterations", "2", "--warmup", "0"]);
    assert!(output.status.success(), "stderr={}", stderr(&output));

    let results = result_files(&f.results());
    assert_eq!(results.len(), 1);
    let run = load_result(&results[0]);
    assert_eq!(run["kind"], "benchmark");
    let metrics = run["outcomes"][0]["metrics"]
        .as_array()
        .expect("metrics harus array");
    let names: Vec<&str> = metrics.iter().filter_map(|m| m["name"].as_str()).collect();
    for want in ["latency", "throughput", "bandwidth", "cycles"] {
        assert!(names.contains(&want), "hilang {}: {:?}", want, names);
    }
    assert!(metrics.len() >= 3, "bukan skor tunggal: {:?}", names);
}

#[test]
fn cli_run_report_and_compare() {
    let f = Fixture::new("report");

    let first = f.run(&["run", "tiny"]);
    assert!(first.status.success(), "stderr={}", stderr(&first));
    let second = f.run(&["run", "tiny"]);
    assert!(second.status.success(), "stderr={}", stderr(&second));

    let results = result_files(&f.results());
    assert_eq!(results.len(), 2, "run-id harus unik");
    let a = results[0].to_str().unwrap();
    let b = results[1].to_str().unwrap();

    let report = f.run(&["report"]);
    assert!(report.status.success(), "stderr={}", stderr(&report));
    let report_text = stdout(&report);
    assert!(report_text.contains("UTBH Report"), "{}", report_text);
    assert!(report_text.contains("tiny_add"), "{}", report_text);

    let comparison = f.run(&["compare", a, b]);
    assert!(
        comparison.status.success(),
        "stderr={}",
        stderr(&comparison)
    );
    assert!(stdout(&comparison).contains("tidak ada perubahan"));
}

#[test]
fn cli_unknown_suite_fails_with_hint() {
    let f = Fixture::new("unknown");
    let output = f.run(&["test", "tidak_ada"]);
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("tidak ditemukan"), "stderr={}", err);
    assert!(err.contains("tiny"), "error harus menyebut suite tersedia");
}

#[test]
fn cli_trace_without_failure_is_an_error() {
    let f = Fixture::new("trace");
    let output = f.run(&["trace", "run-tidak-ada"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("tidak ada trace"));
}

#[test]
fn cli_fuzz_is_deterministic_and_passes() {
    let f = Fixture::new("fuzz");
    let first = f.run(&["fuzz", "tiny", "--seed", "1", "--iterations", "8"]);
    assert!(first.status.success(), "stderr={}", stderr(&first));
    let text = stdout(&first);
    assert!(text.contains("Fuzz result"), "{}", text);
    assert!(text.contains("0 FAIL"), "{}", text);

    let second = f.run(&["fuzz", "tiny", "--seed", "1", "--iterations", "8"]);
    assert!(second.status.success(), "stderr={}", stderr(&second));
    // Seed sama → hasil sama (reproducible).
    let pass_line = |s: &str| {
        s.lines()
            .find(|l| l.contains("Fuzz result"))
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(pass_line(&text), pass_line(&stdout(&second)));
}

#[test]
fn cli_stress_reports_no_failure() {
    let f = Fixture::new("stress");
    let output = f.run(&["stress", "tiny", "--iterations", "6", "--window", "2"]);
    assert!(output.status.success(), "stderr={}", stderr(&output));
    assert!(stdout(&output).contains("UTBH Stress"));
}

#[test]
fn cli_fails_when_result_cannot_be_written() {
    let f = Fixture::new("unwritable");
    // results/ sebagai file biasa → create_dir_all gagal.
    fs::write(f.results(), b"not a directory").unwrap();

    let output = f.run(&["test", "tiny"]);
    assert!(
        !output.status.success(),
        "artefak gagal ditulis tidak boleh exit 0"
    );
    assert!(
        stderr(&output).contains("gagal menulis hasil"),
        "stderr={}",
        stderr(&output)
    );
}

#[test]
fn cli_rejects_zero_iterations_and_window() {
    let f = Fixture::new("zero");
    let cases: [&[&str]; 4] = [
        &["benchmark", "tiny", "--iterations", "0"],
        &["run", "tiny", "--iterations", "0"],
        &["fuzz", "tiny", "--iterations", "0"],
        &["stress", "tiny", "--window", "0"],
    ];
    for args in cases {
        let output = f.run(args);
        assert!(!output.status.success(), "harus ditolak: {:?}", args);
        assert!(
            stderr(&output).contains("> 0"),
            "{:?}: stderr={}",
            args,
            stderr(&output)
        );
    }
}

#[test]
fn cli_duplicate_test_names_are_rejected() {
    let f = Fixture::new("duplicate");
    fs::create_dir_all(f.suites().join("extra")).unwrap();
    fs::write(f.suites().join("extra/dup.utbh"), TINY).unwrap();

    let output = f.run(&["test", "universal"]);
    assert!(!output.status.success(), "duplikat harus ditolak");
    assert!(
        stderr(&output).contains("duplikat"),
        "stderr={}",
        stderr(&output)
    );
}

#[test]
fn cli_rejects_invalid_test_definition() {
    let f = Fixture::new("invalid");
    let invalid = TINY.replace("mode = \"exact\"", "mode = \"approximate\"");
    fs::write(f.suites().join("tiny/add.utbh"), invalid).unwrap();

    let output = f.run(&["test", "tiny"]);
    assert!(!output.status.success(), "mode tak dikenal harus ditolak");
    assert!(
        stderr(&output).contains("validation.mode"),
        "stderr={}",
        stderr(&output)
    );
}
