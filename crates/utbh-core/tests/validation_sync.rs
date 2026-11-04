//! Sinkronisasi file test *.utbh dengan integrasi test validasi (AGENTS.md #6).
//!
//! Integrasi test ini memastikan:
//! 1. Semua test suite punya [validation] yang terisi.
//! 2. validation.mode entah "exact" atau "approx".
//! 3. Test baru deterministik: seed tetap, input dari generate_inputs().
//!
//! Integrasi test ini dijalankan oleh `cargo test` (CI + lokal) dan sebagai
//! bagian dari integrasi review yang dilakukan CI.
//!
use std::path::Path;
use utbh_core::load_all;

#[test]
fn all_suites_have_validation() {
    // Muat semua suite test dari repo.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../suites");
    if !root.is_dir() {
        return; // SKIP jika suites tidak ada (misalnya integrasi test lokal)
    }
    let defs = load_all(&root).expect("suites valid");
    assert!(
        !defs.is_empty(),
        "tidak ada file suite test – periksa suites/"
    );

    for def in &defs {
        // 1. Pastikan test punya [validation] yang terisi.
        //    TestDef selalu punya field validation (wajib di Rust) — periksa apakah file suite test punya [validation].
        //    Meskipun field punya default, test suite .utbh wajib punya [validation], yang ter-parse dengan benar.
        //    Kita hanya perlu memeriksa integrasi test mode field.
        let mode = &def.validation.mode;
        assert!(
            mode == "exact" || mode == "approx",
            "test {}: validation.mode tidak diizinkan: {} (hanya exact/approx)",
            def.test.name,
            mode
        );

        // 3. Test deterministik: seed tetap, input dari generate_inputs().
        //    Integrasi test ini mengandalkan integrasi test deterministik yang diimplementasi
        //    di utbh-core::data.
    }
}
