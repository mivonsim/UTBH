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

mod args;
mod compare;
mod list;
mod printer;
mod profile;
mod report;
mod runner;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = args::Cli::parse();
    runner::execute_cli(cli)
}
