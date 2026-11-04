//! Integrasi: suite repo harus deterministik dan punya validasi.
//!
//! Menjaga aturan AGENTS.md #4 (test deterministik) dan #6 (setiap test wajib
//! punya `[validation]`). Dijalankan oleh `cargo test` di CI dan lokal.

use std::collections::HashSet;
use std::path::Path;
use utbh_core::{inputs, load_all, TestDef};

fn repo_suites() -> Option<Vec<TestDef>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../suites");
    if !root.is_dir() {
        return None;
    }
    Some(load_all(&root).expect("suites repo valid"))
}

#[test]
fn all_suites_have_validation() {
    let Some(defs) = repo_suites() else {
        return;
    };
    assert!(!defs.is_empty(), "tidak ada test di suites/");

    for def in &defs {
        assert!(
            matches!(def.validation.mode.as_str(), "exact" | "approx"),
            "test {}: validation.mode '{}' tidak diizinkan (pakai exact/approx)",
            def.test.name,
            def.validation.mode
        );
        assert!(
            !def.operation.op.is_empty(),
            "test {}: operation.op kosong",
            def.test.name
        );
        assert!(
            def.input.elements > 0,
            "test {}: input.elements harus > 0",
            def.test.name
        );
        assert!(
            !def.test.category.is_empty(),
            "test {}: category kosong",
            def.test.name
        );
    }
}

#[test]
fn suite_test_names_are_unique() {
    let Some(defs) = repo_suites() else {
        return;
    };
    let mut seen = HashSet::new();
    for def in &defs {
        assert!(
            seen.insert(def.test.name.clone()),
            "nama test duplikat: {}",
            def.test.name
        );
    }
}

#[test]
fn suite_inputs_are_deterministic() {
    let Some(defs) = repo_suites() else {
        return;
    };
    for def in &defs {
        let first = inputs(def);
        let second = inputs(def);
        assert_eq!(
            first.a, second.a,
            "test {}: operand A tidak deterministik",
            def.test.name
        );
        assert_eq!(
            first.b, second.b,
            "test {}: operand B tidak deterministik",
            def.test.name
        );
        assert_eq!(
            first.c, second.c,
            "test {}: operand C tidak deterministik",
            def.test.name
        );
        assert!(!first.a.is_empty(), "test {}: input kosong", def.test.name);
    }
}
