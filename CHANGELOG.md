# Changelog

Semua perubahan penting pada UTBH dicatat di file ini.
Format mengikuti [Keep a Changelog](https://keepachangelog.com/), versi mengikuti
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Validator JSON Schema rekursif di test untuk hardware, result, dan test
  definition (field hilang/ekstra, enum, minimum, `$ref`).

### Changed

- `utbh compare` menolak membandingkan hasil dengan `schema_version` berbeda.
- Nama test duplikat antar file/suite kini error, bukan dibuang diam-diam.

### Fixed

- Hasil dan trace ditulis atomik (tidak ada JSON setengah jadi).
- `utbh report` pada direktori hasil yang belum ada memberi pesan jelas.
- `RunResult::load` menolak hasil dari schema yang lebih baru.

## [1.0.0] — 2026-11-04

Rilis stabil pertama: harness guest-side untuk correctness, validation,
benchmark, fuzz, stress, dan trace.

### Added

- Target build dan artefak release untuk `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`, dan `riscv64gc-unknown-linux-gnu` (ISA guest
  `mivon emu`). CI mengecek ketiganya dan merilis `utbh` per target.
- Gate kualitas di CI: `cargo fmt --check`, `cargo clippy -D warnings`, build
  release, dan test workspace.
- Test integrasi CLI end-to-end untuk seluruh subcommand (test, benchmark,
  run, report, compare, fuzz, stress, trace, list, discover).
- Schema trace (`schemas/trace/trace.schema.json`) dengan test sinkronisasi.
- `CHANGELOG.md`, `LICENSE` (Apache-2.0), dan MSRV `1.87`.

### Changed

- Satu sumber kebenaran untuk JSON Schema: hanya `schemas/<area>/` yang
  dipertahankan; duplikat draft-07 di root dihapus.
- Run-id menyertakan PID sehingga dua run paralel tidak saling menimpa hasil.

### Fixed

- `max_rel_error` tidak lagi `Infinity`; nilai non-finite membuat JSON menjadi
  `null` dan melanggar schema.
- `validation.mode` yang tidak dikenal ditolak, bukan diam-diam diperlakukan
  sebagai `exact`/`approx`.
- Loader memvalidasi `elements > 0`, dtype, mode, tolerance, iterasi benchmark,
  dan nama test sebelum test dijalankan.
- Nama file trace disanitasi sehingga test definition tidak bisa menulis keluar
  dari direktori `results/`.
- `utbh trace` pada direktori hasil yang belum ada memberi pesan yang jelas.
- Frekuensi CPU di-cache agar `cycles()` tidak membaca `/proc` dan sysfs pada
  setiap iterasi benchmark.
- Generator `i64` mencakup nilai negatif sehingga jalur signed integer benar-
  benar diuji.
- FAIL pada `utbh stress` sekarang menghasilkan trace seperti mode lain.
