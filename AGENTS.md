# AGENTS.md — UTBH

## Apa ini

UTBH (Universal Test Benchmark Hardware) adalah framework **guest-side** untuk
testing, validation, fuzzing, stress-testing, dan benchmarking hardware secara
universal. UTBH berjalan **hanya di dalam guest OS yang di-boot `mivon emu`**
(Mivon Hardware Emulator), dan berbicara ke hardware lewat **Hardware API**
yang disediakan OS.

## Invariant arsitektur (jangan dilanggar)

```
HOST
  └── mivon emu (Mivon Hardware Emulator)
        └── Guest OS (boot di dalam emulator)
              └── Hardware API
                    └── UTBH
```

- UTBH **tidak** dijalankan dari host. Host **hanya** menjalankan emulator.
- **Clone + build juga di guest**: `git clone` dan `cargo build` terjadi di
  dalam guest OS, bukan di host. Host tidak pernah membangun binary UTBH
  untuk dijalankan sendiri.
- UTBH **tidak** tahu ia berada di emulator. Ia melihat hardware, bukan
  virtual hardware (baca `/proc`, `/sys` milik guest OS = hardware menurut OS).
- Tidak ada `mivon-vm/`, `verilator/`, `rtl-backend/`, `host-runner/` di repo ini.
  Itu tanggung jawab Mivon, bukan UTBH.
- Tidak ada "VM backend" di dalam UTBH. Fidelity (fast emu / cycle-accurate /
  RTL-linked CPU / FPGA / ASIC) ditentukan environment di luar UTBH.
  Kode UTBH sama untuk ketiganya.
- Wajib compile untuk ISA guest RISC-V: dicek CI dengan
  `cargo check --target riscv64gc-unknown-linux-gnu --workspace`.
- Semua komunikasi ke hardware melewati `utbh-api` (`HardwareApi` trait).
  Crate lain **dilarang** membaca `/proc`, `/sys`, atau CPUID secara langsung.
- semua file hanya memiliki 1 tangguh jawab tidak boleh lebih   
  jika ada wajib pecah
## Struktur workspace

| Crate | Layer | Tanggung jawab |
|---|---|---|
| `utbh-api` | 1 | Hardware API interface (satu-satunya jendela ke hardware) |
| `utbh-discovery` | 2 | Deteksi ISA, core, cache, vector, GPU, memory, interconnect |
| `utbh-workload` | 3 | Menjalankan workload (integer, FP, vector, memory, matmul, dst.) |
| `utbh-validation` | 6 | Differential validation: execution vs reference |
| `utbh-test` | 4 | Test engine: apakah hardware benar? (PASS/FAIL) |
| `utbh-benchmark` | 5 | Benchmark engine: latency, throughput, bandwidth, GFLOPS |
| `utbh-fuzz` | — | Random workload generator + minimizer |
| `utbh-stress` | — | Stress engine: FAIL kumulatif + degradasi latensi |
| `utbh-trace` | 7 | Trace hasil FAIL (cycle, instruksi, register, memory, NoC) |
| `utbh-core` | — | Tipe bersama: `TestDef`, `RunResult`, `HardwareReport`, loader |
| `utbh-cli` | 7 | Binary `utbh`: discover/test/benchmark/fuzz/run/report/trace |

Test didefinisikan **data-driven** di `suites/**/**.utbh` (TOML). Menambah test
= menambah file. **Jangan** menambahkan `const TEST_001 = ...` ke Rust.

## Perintah

```sh
cargo build --release          # build (di dalam guest OS)
cargo test                     # unit test
./target/release/utbh discover
./target/release/utbh list                 # daftar semua suite + test
./target/release/utbh test cpu
./target/release/utbh benchmark memory
./target/release/utbh run universal
./target/release/utbh fuzz soc --seed 1 --iterations 64
./target/release/utbh stress cpu --iterations 100 --window 20
./target/release/utbh report
./target/release/utbh compare <file-a.utbh-result.json> <file-b.utbh-result.json>
./target/release/utbh trace <run-id>
./target/release/utbh --profile configs/ci.json run   # preset parameter
```

Suite dipilih dari folder di `suites/`: `cpu`, `gpu`, `memory`, `cache`, `noc`,
`soc`, `accelerator`. `all` / `universal` = semua suite.

## Aturan

1. **Testing ≠ benchmark.** Testing menjawab "benar?"; benchmark menjawab
   "seberapa cepat?". Keduanya selalu dilaporkan terpisah.
2. **Jangan pernah jadikan satu angka skor tunggal satu-satunya hasil.**
   Selalu laporkan PASS/FAIL + latency + throughput + bandwidth.
3. **Skor tidak berubah tanpa bump `SCHEMA_VERSION`** di `utbh-core`.
4. Test baru harus deterministik: seed tetap, input dari `generate_inputs()`.
5. Workload tidak boleh akses hardware langsung — semua lewat `HardwareApi`.
6. Setiap test wajib punya `[validation]`. Test tanpa validasi = bug.
7. Hasil ditulis ke `results/` (di-gitignore). Format: `<run-id>.utbh-result.json`
   dan `<run-id>.utbh-trace.json`.
8. Schema JSON di `schemas/` harus sinkron dengan tipe di `utbh-core`.

## Menambah test baru

1. Buat file `suites/<kategori>/<nama>.utbh` (format TOML, lihat `doc/` atau
   test yang ada).
2. Jalankan `utbh test <kategori>` — tanpa rebuild binary.
3. Pastikan `validation.mode` terisi dan test PASS.

## Menambah workload op baru

1. Tambahkan varian di `utbh-workload` (`Op` enum + `exec` serial atau
   `exec_parallel` untuk atomik/multicore).
2. Tambahkan reference implementation yang setara di `utbh-validation`.
3. Kalau operasi floating-point non-associatif (sum paralel, matmul, FMA),
   paksa `mode = approx` di fuzz generator (`utbh-fuzz/generate.rs`) —
   `exact` untuk kasus begini = false positive, bukan bug hardware.
4. Tambahkan unit test di kedua crate (harus kompatibel sebelum merge).
