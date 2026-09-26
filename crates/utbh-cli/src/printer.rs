//! Tampilan output hasil test dan benchmark pada stdout / stderr.

use std::process::ExitCode;
use utbh_core::{Status, TestOutcome};

pub fn fail(msg: &str) -> ExitCode {
    eprintln!("error: {}", msg);
    ExitCode::FAILURE
}

pub fn finish(outcomes: &[TestOutcome]) -> ExitCode {
    if outcomes.iter().any(|o| o.status == Status::Fail) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

pub fn print_test_results(outcomes: &[TestOutcome]) {
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
