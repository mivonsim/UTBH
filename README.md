# UTBH — Universal Test Benchmark Hardware

Guest-side framework untuk **testing, validation, fuzzing, stress-testing,
dan benchmarking** hardware secara universal: CPU, GPU, SoC, memory,
interconnect, accelerator.

> **Invariant arsitektur:**
>
> ```
> HOST → Mivon VM → Mivon Hardware OS → Hardware API → UTBH
> ```
>
> UTBH berjalan **hanya di dalam Mivon Hardware OS**. Host hanya menjalankan
> VM. UTBH tidak tahu dirinya berada di VM — ia melihat hardware, bukan
> virtual hardware. Tidak ada VM backend di dalam UTBH: fast VM,
> cycle-accurate VM, dan RTL memakai binary yang sama.

Detail desain: [`doc/design.md`](doc/design.md) · Aturan kontribusi: [`AGENTS.md`](AGENTS.md)

## Developer workflow

```sh
# 1. Dari host: buat VM
mivon run --hardware aurora-172

# 2. VM boot → Mivon Hardware OS. Di dalam guest:
git clone https://github.com/mivonsim/utbh
cd utbh
cargo build --release

# 3. Jalankan
./target/release/utbh discover          # Layer 2: kemampuan hardware
./target/release/utbh test cpu          # Layer 4: benar? → PASS/FAIL
./target/release/utbh benchmark memory  # Layer 5: cepat? → multi-metrik
./target/release/utbh run universal     # test + benchmark, semua suite
./target/release/utbh fuzz soc --seed 1 --iterations 64
./target/release/utbh stress cpu --iterations 100 --window 20
./target/release/utbh report            # render results/.utbh-report.md
./target/release/utbh compare a.utbh-result.json b.utbh-result.json
./target/release/utbh trace <run-id>    # trace untuk kasus FAIL
```

### Profil

Preset parameter di `configs/` (flag eksplisit selalu menang):

```sh
./target/release/utbh --profile configs/quick.json test
./target/release/utbh --profile configs/ci.json fuzz --seed 3
./target/release/utbh --profile configs/full.json run
```

## Testing ≠ Benchmark

| | Menjawab | Output |
|---|---|---|
| Testing (Layer 4) | "Apakah benar?" | PASS / FAIL + mismatch count |
| Benchmark (Layer 5) | "Seberapa cepat?" | latency, throughput, bandwidth, GFLOPS, cycles |

Keduanya **selalu dilaporkan terpisah**. UTBH tidak pernah menghasilkan
satu angka skor tunggal sebagai satu-satunya hasil.

## Test = data (bukan kode)

Test didefinisikan deklaratif di `suites/**/**utbh` (TOML), dibaca saat
runtime — menambah test **tanpa rebuild binary**:

```toml
[test]
name = "vector_add"
category = "cpu.vector"

[input]
elements = 1048576
type = "f32"

[operation]
op = "add"

[validation]
mode = "exact"

[benchmark]
metrics = ["latency", "throughput", "bandwidth"]
```

Lihat [`suites/README.md`](suites/README.md).

## Struktur

```
crates/
├── utbh-api/          Layer 1 — Hardware API (satu-satunya jendela ke hardware)
├── utbh-discovery/    Layer 2 — deteksi ISA/core/cache/GPU/memory/NoC
├── utbh-workload/     Layer 3 — eksekusi workload
├── utbh-test/         Layer 4 — correctness (PASS/FAIL)
├── utbh-benchmark/    Layer 5 — latency/throughput/bandwidth/GFLOPS
├── utbh-validation/   Layer 6 — differential validation (execution vs reference)
├── utbh-fuzz/         random workload + minimizer
├── utbh-stress/       beban berkelanjutan + deteksi degradasi
├── utbh-trace/        Layer 7 — trace kasus FAIL
├── utbh-core/         tipe bersama + loader test
└── utbh-cli/          binary `utbh`
suites/                test packages (data-driven)
configs/               profil run (quick / full / ci)
schemas/               JSON Schema (sinkron dengan utbh-core)
results/               output (gitignored)
doc/design.md          dokumen desain
```

Satu file = satu tanggung jawab (lihat AGENTS.md).

## Output

| File | Isi |
|---|---|
| `results/<run-id>.utbh-result.json` | hasil run (schema-versioned) |
| `results/<run-id>.<test>.utbh-trace.json` | trace per test yang FAIL |
| `results/.utbh-report.md` | laporan markdown gabungan |

## Dari VM sampai Silicon

Test definition yang sama dipakai di VM, FPGA, dan ASIC — bandingkan dengan
`utbh compare`:

```sh
utbh compare vm.utbh-result.json asic.utbh-result.json
# → tabel PASS/FAIL + delta latensi per test + regresi PASS→FAIL
```
