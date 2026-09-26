//! Orkestrasi eksekusi subcommand CLI: discover, test, benchmark, run, fuzz, report, trace.

use std::path::Path;
use std::process::ExitCode;
use utbh_api::{HardwareApi, MivonHardwareApi};
use utbh_core::{
    RunResult, Status, TestDef, TestOutcome, SCHEMA_VERSION,
};
use utbh_discovery::{discover, format_report};

use crate::args::{Cli, Command};
use crate::printer::{fail, finish, print_test_results};
use crate::report::render_report;

pub fn execute_cli(cli: Cli) -> ExitCode {
    let api = MivonHardwareApi::new();

    // Profil: preset suite + iterasi. Flag eksplisit selalu menang.
    let profile = match &cli.profile {
        Some(path) => match crate::profile::Profile::load(path) {
            Ok(p) => {
                if let Err(e) = p.validate() {
                    return fail(&e);
                }
                println!("profil: {} — {}", p.name, p.description);
                Some(p)
            }
            Err(e) => return fail(&e),
        },
        None => None,
    };
    let prof = profile.as_ref();
    let suite_of = |flag: &Option<String>| -> String {
        crate::profile::Profile::resolve_suite_opt(prof, flag.as_deref())
    };

    match cli.command {
        Command::Discover => {
            let d = discover(&api);
            print!("{}", format_report(&d));
            ExitCode::SUCCESS
        }
        Command::Test { suite } => {
            let suite = suite_of(&suite);
            let run_id = utbh_core::new_run_id();
            match load(&cli.suites, &suite) {
                Ok(defs) => {
                    let outcomes = utbh_test::run_all(&defs);
                    print_test_results(&outcomes);
                    save_traces(&defs, &outcomes, &api, &cli.results, &run_id);
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
            let suite = suite_of(&suite);
            let iterations = crate::profile::Profile::resolve(
                iterations,
                prof.and_then(|p| p.iterations),
                10,
            );
            let warmup = crate::profile::Profile::resolve(warmup, prof.and_then(|p| p.warmup), 3);
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
                    save_traces(&defs, &outcomes, &api, &cli.results, &run_id);
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
            let suite = suite_of(&suite);
            let iterations =
                crate::profile::Profile::resolve(iterations, prof.and_then(|p| p.iterations), 10);
            let warmup = crate::profile::Profile::resolve(warmup, prof.and_then(|p| p.warmup), 3);
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
                    save_traces(&defs, &outcomes, &api, &cli.results, &run_id);
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
            let suite = suite_of(&suite);
            let iterations =
                crate::profile::Profile::resolve(iterations, prof.and_then(|p| p.fuzz_iterations), 64);
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
                    if let Some(t) = utbh_trace::from_failure(&c.def, &o, &api, &run_id) {
                        let _ = utbh_trace::save(&t, &cli.results);
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
        Command::Stress { suite, iterations, window } => {
            let suite = suite_of(&suite);
            let iterations = crate::profile::Profile::resolve(
                iterations,
                prof.and_then(|p| p.stress_iterations),
                100,
            );
            let window = crate::profile::Profile::resolve(
                window,
                prof.and_then(|p| p.stress_window),
                20,
            );
            let run_id = utbh_core::new_run_id();
            match load(&cli.suites, &suite) {
                Ok(defs) => {
                    let cfg = utbh_stress::StressConfig { iterations, window };
                    println!(
                        "UTBH Stress — suite={} iterations={} window={}",
                        suite, iterations, window
                    );
                    let mut reports = Vec::new();
                    for def in &defs {
                        let r = utbh_stress::stress(def, &api, &cfg);
                        reports.push(r);
                    }
                    let outcomes: Vec<TestOutcome> =
                        reports.iter().map(utbh_stress::to_outcome).collect();
                    print_test_results(&outcomes);
                    for r in &reports {
                        if let Some(first) = r.first_fail_at {
                            println!(
                                "  {} — FAIL pertama di iterasi {}/{}",
                                r.test, first, r.iterations
                            );
                        }
                        if r.degraded() {
                            println!(
                                "  {} — degradasi latensi {:.2}x (window awal vs akhir)",
                                r.test,
                                r.degradation_ratio.unwrap_or(0.0)
                            );
                        }
                    }
                    save_run(&cli.results, &run_id, "stress", &suite, &api, &outcomes, None);
                    finish(&outcomes)
                }
                Err(e) => fail(&e),
            }
        }
        Command::Report => match render_report(&cli.results) {
            Ok(text) => {
                print!("{}", text);
                ExitCode::SUCCESS
            }
            Err(e) => fail(&e),
        },
        Command::Compare { a, b } => {
            let run_a = match RunResult::load(&a) {
                Ok(r) => r,
                Err(e) => return fail(&e),
            };
            let run_b = match RunResult::load(&b) {
                Ok(r) => r,
                Err(e) => return fail(&e),
            };
            let comparison = crate::compare::compare(&run_a, &run_b);
            print!("{}", crate::compare::render(&comparison));
            if comparison.regressions.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Command::Trace { run_id } => match utbh_trace::load_all(&cli.results, &run_id) {
            Ok(traces) => {
                for (i, t) in traces.iter().enumerate() {
                    if i > 0 {
                        println!();
                    }
                    print!("{}", utbh_trace::render(t));
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(&e),
        },
    }
}

fn load(root: &Path, suite: &str) -> Result<Vec<TestDef>, String> {
    utbh_core::load_suite(root, suite)
}

/// Tulis trace untuk setiap outcome FAIL (Layer 7).
#[allow(clippy::too_many_arguments)]
fn save_traces(
    defs: &[TestDef],
    outcomes: &[TestOutcome],
    api: &dyn HardwareApi,
    results_dir: &Path,
    run_id: &str,
) {
    for (def, outcome) in defs.iter().zip(outcomes.iter()) {
        if let Some(t) = utbh_trace::from_failure(def, outcome, api, run_id) {
            match utbh_trace::save(&t, results_dir) {
                Ok(p) => println!("trace → {}", p.display()),
                Err(e) => eprintln!("gagal menulis trace {}: {}", t.test, e),
            }
        }
    }
}

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
