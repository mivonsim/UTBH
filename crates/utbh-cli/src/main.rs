//! `utbh` — Universal Test Benchmark Hardware CLI.
//!
//! Berjalan **di dalam Mivon Hardware OS** (guest), bukan di host.
//!
//! ```sh
//! utbh discover
//! utbh test cpu
//! utbh benchmark memory
//! utbh run universal
//! utbh fuzz soc --seed 1 --iterations 64
//! utbh report
//! utbh trace <run-id>
//! ```

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use utbh_api::{HardwareApi, MivonHardwareApi};
use utbh_core::{
    RunResult, Status, TestDef, TestOutcome, DEFAULT_RESULTS_DIR, DEFAULT_SUITES_DIR,
    SCHEMA_VERSION,
};
use utbh_discovery::{discover, format_report};

#[derive(Parser)]
#[command(
    name = "utbh",
    about = "Universal Test Benchmark Hardware — jalankan di dalam Mivon Hardware OS",
    version
)]
struct Cli {
    /// Direktori test package (data-driven, tanpa rebuild).
    #[arg(long, global = true, default_value = DEFAULT_SUITES_DIR)]
    suites: PathBuf,

    /// Direktori output hasil.
    #[arg(long, global = true, default_value = DEFAULT_RESULTS_DIR)]
    results: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Layer 2 — deteksi kemampuan hardware.
    Discover,
    /// Layer 4 — correctness: apakah hardware benar? (PASS/FAIL)
    Test {
        /// Nama suite (folder di `suites/`) atau `all` / `universal`.
        suite: String,
    },
    /// Layer 5 — seberapa cepat? (latency, throughput, bandwidth, GFLOPS)
    Benchmark {
        suite: String,
        /// Iterasi pengukuran (default 10).
        #[arg(long, default_value_t = 10)]
        iterations: u32,
        #[arg(long, default_value_t = 3)]
        warmup: u32,
    },
    /// Test + benchmark dalam satu run (keduanya dilaporkan terpisah).
    Run {
        suite: String,
        #[arg(long, default_value_t = 10)]
        iterations: u32,
        #[arg(long, default_value_t = 3)]
        warmup: u32,
    },
    /// Fuzz engine: random workload + minimizer.
    Fuzz {
        suite: String,
        #[arg(long, default_value_t = utbh_fuzz::FuzzConfig::default().seed)]
        seed: u64,
        #[arg(long, default_value_t = 64)]
        iterations: usize,
    },
    /// Render `.utbh-report` dari semua hasil di results/.
    Report,
    /// Tampilkan `.utbh-trace` untuk run-id tertentu.
    Trace { run_id: String },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let api = MivonHardwareApi::new();

    match cli.command {
        Command::Discover => {
            let d = discover(&api);
            print!("{}", format_report(&d));
            ExitCode::SUCCESS
        }
        Command::Test { suite } => {
            let run_id = utbh_core::new_run_id();
            match load(&cli.suites, &suite) {
                Ok(defs) => {
                    let outcomes = utbh_test::run_all(&defs);
                    print_test_results(&outcomes);
                    save_run(&cli.results, &run_id, "test", &suite, &api, &outcomes, None);
                    finish(&outcomes)
                }
                Err(e) => fail(&e),
            }
        }
        Command::Benchmark {
            suite,
            iterations,
            warmup,
        } => {
            let run_id = utbh_core::new_run_id();
            match load(&cli.suites, &suite) {
                Ok(defs) => {
                    let cfg = utbh_benchmark::BenchConfig { warmup, iterations };
                    // Benchmark selalu bersama correctness — jangan ukur yang salah.
                    let mut outcomes = Vec::new();
                    for def in &defs {
                        let mut o = utbh_test::run(def);
                        if matches!(o.status, Status::Pass) {
                            o.metrics = utbh_benchmark::run(def, &api, &cfg);
                        }
                        outcomes.push(o);
                    }
                    print_test_results(&outcomes);
                    save_run(
                        &cli.results,
                        &run_id,
                        "benchmark",
                        &suite,
                        &api,
                        &outcomes,
                        None,
                    );
                    finish(&outcomes)
                }
                Err(e) => fail(&e),
            }
        }
        Command::Run {
            suite,
            iterations,
            warmup,
        } => {
            let run_id = utbh_core::new_run_id();
            match load(&cli.suites, &suite) {
                Ok(defs) => {
                    let cfg = utbh_benchmark::BenchConfig { warmup, iterations };
                    let mut outcomes = Vec::new();
                    for def in &defs {
                        let mut o = utbh_test::run(def);
                        if matches!(o.status, Status::Pass) {
                            o.metrics = utbh_benchmark::run(def, &api, &cfg);
                        }
                        outcomes.push(o);
                    }
                    print_test_results(&outcomes);
                    save_run(&cli.results, &run_id, "run", &suite, &api, &outcomes, None);
                    finish(&outcomes)
                }
                Err(e) => fail(&e),
            }
        }
        Command::Fuzz {
            suite,
            seed,
            iterations,
        } => {
            let run_id = utbh_core::new_run_id();
            let cfg = utbh_fuzz::FuzzConfig { seed, iterations };
            let report = utbh_fuzz::fuzz(&suite, &cfg);

            println!(
                "UTBH Fuzz — suite={} seed={} iterations={}",
                suite, seed, iterations
            );
            for c in &report.cases {
                if !c.passed {
                    println!("  FAIL  {} (seed={})", c.def.test.name, c.seed);
                    if let Some(m) = &c.minimized {
                        println!(
                            "        minimized → elements={} (repro: seed={})",
                            m.input.elements, c.seed
                        );
                    }
                }
            }
            println!(
                "\nFuzz result: {} PASS / {} FAIL / {} minimized",
                report.passed,
                report.failed,
                report.minimized_count()
            );

            // Setiap kasus FAIL → trace.
            let outcomes: Vec<TestOutcome> = report
                .cases
                .iter()
                .filter(|c| !c.passed)
                .map(|c| {
                    let o = utbh_test::run(&c.def);
                    if let Some(t) = utbh_trace::Trace::from_failure(&c.def, &o, &api, &run_id) {
                        let _ = t.save(&cli.results);
                    }
                    o
                })
                .collect();

            let summary = utbh_core::FuzzSummary {
                base_seed: seed,
                iterations,
                passed: report.passed,
                failed: report.failed,
            };
            save_run(
                &cli.results,
                &run_id,
                "fuzz",
                &suite,
                &api,
                &outcomes,
                Some(summary),
            );
            if report.failed > 0 {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Command::Report => match render_report(&cli.results) {
            Ok(text) => {
                print!("{}", text);
                ExitCode::SUCCESS
            }
            Err(e) => fail(&e),
        },
        Command::Trace { run_id } => {
            let path = cli.results.join(format!("{}.utbh-trace.json", run_id));
            match utbh_trace::Trace::load(&path) {
                Ok(t) => {
                    print!("{}", t.render());
                    ExitCode::SUCCESS
                }
                Err(e) => fail(&e),
            }
        }
    }
}

fn load(root: &Path, suite: &str) -> Result<Vec<TestDef>, String> {
    utbh_core::load_suite(root, suite)
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("error: {}", msg);
    ExitCode::FAILURE
}

fn finish(outcomes: &[TestOutcome]) -> ExitCode {
    if outcomes.iter().any(|o| o.status == Status::Fail) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_test_results(outcomes: &[TestOutcome]) {
    println!(
        "\n{:<28} {:<14} {:<8} {}",
        "TEST", "CATEGORY", "RESULT", "DETAIL"
    );
    println!("{}", "-".repeat(78));
    for o in outcomes {
        let result = match o.status {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
        };
        let detail = if let Some(v) = &o.validation {
            if v.matched {
                "correctness ok".to_string()
            } else {
                format!("mismatches={}", v.mismatches)
            }
        } else if let Some(e) = &o.error {
            e.clone()
        } else {
            "-".into()
        };
        println!("{:<28} {:<14} {:<8} {}", o.name, o.category, result, detail);

        // Benchmark metrics dilaporkan terpisah dari correctness.
        if !o.metrics.is_empty() {
            let mut line = String::from("    metrics: ");
            for (i, m) in o.metrics.iter().enumerate() {
                if i > 0 {
                    line.push_str(", ");
                }
                line.push_str(&format!("{}={:.3} {}", m.name, m.value, m.unit));
            }
            println!("{}", line);
        }
    }
    let pass = outcomes.iter().filter(|o| o.status == Status::Pass).count();
    let fail = outcomes.iter().filter(|o| o.status == Status::Fail).count();
    println!("{}", "-".repeat(78));
    println!("Total: {} PASS, {} FAIL\n", pass, fail);
}

#[allow(clippy::too_many_arguments)]
fn save_run(
    results_dir: &Path,
    run_id: &str,
    kind: &str,
    suite: &str,
    api: &dyn HardwareApi,
    outcomes: &[TestOutcome],
    fuzz: Option<utbh_core::FuzzSummary>,
) {
    let discovery = discover(api);
    let run = RunResult {
        schema_version: SCHEMA_VERSION,
        run_id: run_id.to_string(),
        unix_time: utbh_core::now_unix(),
        kind: kind.to_string(),
        suite: suite.to_string(),
        hardware: discovery.report,
        outcomes: outcomes.to_vec(),
        fuzz,
    };
    match run.save(results_dir) {
        Ok(p) => println!("result → {}", p.display()),
        Err(e) => eprintln!("gagal menulis hasil: {}", e),
    }
}

/// Render `.utbh-report` (markdown) dari semua run di results/.
fn render_report(results_dir: &Path) -> Result<String, String> {
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
