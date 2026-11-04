# Changelog

Semua perubahan penting pada UTBH dicatat di file ini.
Format mengikuti [Keep a Changelog](https://keepachangelog.com/), versi mengikuti
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Validator JSON Schema rekursif di test untuk hardware, result, dan test
  definition (field hilang/ekstra, enum, minimum, `$ref`).

### Changed

- Benchmark menyiapkan input deterministik sekali di luar region pengukuran;
  latency tidak lagi memasukkan waktu generasi input/RNG.
- `utbh compare` menolak membandingkan hasil dengan `schema_version` berbeda.
- Nama test duplikat antar file/suite kini error, bukan dibuang diam-diam.

### Fixed

- CI berjalan di semua branch (bukan hanya `main`).
- `utbh list` keluar nonzero bila ada suite yang gagal dimuat, tanpa
  menyembunyikan suite lain yang valid.
- `--iterations 0` dan `--window 0` ditolak, bukan diam-diam di-clamp.
- Profil dengan nama suite kosong ditolak.
- GFLOPS hanya dihitung untuk operasi floating-point; operasi integer dan
  `compare` tidak lagi dilaporkan sebagai FLOP.
- Definisi suite menolak `validation.mode = "none"`; setiap test yang
  dijalankan dari `suites/` wajib punya validasi (AGENTS.md #6).
- Bandwidth menghitung seluruh byte yang disentuh (operand dibaca + hasil
  ditulis), bukan hanya operand pertama. `add` = 3×, `fma` = 4×, `matmul` = 3×
  ukuran elemen.
- `.utbh-report.md` ditulis atomik seperti artefak hasil/trace.
- Latency stress hanya mengukur eksekusi workload; generate input dan eksekusi
  reference tidak lagi masuk region pengukuran.
- Field `Discovery.mem_bandwidth_gbs` dihapus karena selalu `None`; bandwidth
  memori diukur benchmark (Layer 5), bukan diklaim discovery.
- Benchmark tidak lagi melaporkan throughput/bandwidth/GFLOPS saat durasi
  sampel nol (di bawah resolusi timer); hanya latency/cycles yang dilaporkan.
- Suite baru `int64_mul` menutup coverage dtype i64 (nilai negatif + wrapping).
- Kegagalan menulis `.utbh-result.json` atau trace kini membuat perintah exit
  nonzero; sebelumnya hanya `eprintln!` dan run tetap dianggap sukses.
- Profil menerima `warmup = 0`, konsisten dengan flag CLI `--warmup 0`.
- `compare` melaporkan drift throughput dan bandwidth, bukan hanya latency.
- `pin_to_cpu` kini benar-benar memanggil `sched_setaffinity` pada CPU yang
  diizinkan OS (cpuset-aware); sebelumnya hanya memvalidasi range.
- Ukuran kerja benchmark `reduce_sum`/`atomic_add`/`parallel_sum` memakai
  jumlah elemen input, bukan panjang output (=1), sehingga throughput, GFLOPS,
  dan bandwidth tidak lagi hampir nol.
- Trace tidak lagi melabeli operand input sebagai `reference_value`.
- Report menampilkan metrik non-headline (`cycles`, `latency_min`, `stress_*`)
  sehingga hasil stress tidak kehilangan informasi.
- Trace tidak lagi mengklaim event cycle-level yang tidak pernah diambil;
  event yang tidak tersedia ditandai eksplisit.
- `memory.channels` tidak lagi diisi angka palsu 2; 0 berarti tidak diketahui
  dan dirender sebagai `unknown`.
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
