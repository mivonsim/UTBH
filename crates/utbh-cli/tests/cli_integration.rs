//! Integrasi test CLI utama untuk memverifikasi subkomando.

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn utbh_cmd() -> Command {
    let mut cmd = Command::new(env!("CARGO"));
    cmd.current_dir(workspace_root());
    cmd.args(["run", "--bin", "utbh"]);
    cmd
}

#[test]
fn cli_help_works() {
    let mut cmd = utbh_cmd();
    cmd.arg("--").arg("--help");
    let output = cmd.output().expect("jalankan utbh --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Universal") || stdout.contains("utbh"));
}

#[test]
fn cli_discover_works() {
    let mut cmd = utbh_cmd();
    cmd.arg("--").arg("discover");
    let output = cmd.output().expect("jalankan utbh discover");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty());
}

#[test]
fn cli_list_works() {
    let mut cmd = utbh_cmd();
    cmd.arg("--").arg("list");
    let output = cmd.output().expect("jalankan utbh list");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty());
}
