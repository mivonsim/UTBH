# UTBH — Design Document

**Universal Test Benchmark Hardware**
Version 0.1 · Status: Living document

---

## 1. Definisi

UTBH adalah **guest-side universal hardware testing, validation, fuzzing,
stress-testing, dan benchmarking framework** yang dikompilasi dan dijalankan di
dalam Mivon Hardware OS, menggunakan Hardware API yang disediakan OS untuk
menguji CPU, GPU, SoC, memory, interconnect, accelerator, dan hardware custom
secara hardware-independent.

---

## 2. Invariant arsitektur

```
HOST
  └── Mivon VM
        └── Mivon Hardware OS
              └── Hardware API
                    └── UTBH
```

Bentuk yang DILARANG:

```
HOST
  └── UTBH
        └── Mivon VM      ← tidak ada
```

Konsekuensi:

- Host **hanya** menjalankan VM. Host tidak pernah menjalankan UTBH.
- UTBH tidak tahu dirinya berada di VM. Ia melihat **hardware**, bukan
  virtual hardware. Deteksi hypervisor/Verilator adalah urusan OS, bukan UTBH.
- UTBH tidak memiliki "VM backend". Fidelity ditentukan hardware environment
  di luar UTBH.

---

## 3. Arsitektur final

```
┌───────────────────────────────────────────────────────────┐
│                        HOST OS                            │
│                   Mivon Hardware VM                       │
└──────────────────────────┬────────────────────────────────┘
                           ▼
┌───────────────────────────────────────────────────────────┐
│                   MIVON HARDWARE VM                       │
│ ┌───────────────────────────────────────────────────────┐ │
│ │                MIVON HARDWARE OS                      │ │
│ │  Kernel / Syscall / Driver / HAL / Scheduler          │ │
│ │  ┌─────────────────────────────────────────────────┐  │ │
│ │  │          HARDWARE ABSTRACTION API               │  │ │
│ │  └───────────────────────┬─────────────────────────┘  │ │
│ │       CPU            GPU            SoC               │ │
│ │                          │                            │ │
│ │                    USER SPACE                          │ │
│ │                   ┌───────▼───────┐                   │ │
│ │                   │     UTBH      │                   │ │
│ │                   └───────┬───────┘                   │ │
│ │        TESTING      BENCHMARK      FUZZING            │ │
│ │                 ┌─────▼──────┐                        │ │
│ │                 │ VALIDATION │                        │ │
│ │                 └─────┬──────┘                        │ │
│ │               TRACE / RESULT                          │ │
│ └───────────────────────────────────────────────────────┘ │
│                   VIRTUAL HARDWARE                        │
│   CPU │ GPU │ NPU │ RAM │ CACHE │ NoC │ DMA │ PCIe │ IO  │
└───────────────────────────────────────────────────────────┘
```

### Hubungan Mivon ↔ UTBH

| | Mivon | UTBH |
|---|---|---|
| Menyediakan | Hardware VM, Hardware OS, kernel, HAL, drivers, hardware model, RTL integration | — |
| Mengonsumsi | — | Hardware API |
| Bertanggung jawab | Menjalankan & menyediakan hardware | Menguji hardware |

Mivon menyediakan hardware. UTBH menguji hardware. UTBH tidak mengendalikan
VM dari luar.

---

## 4. Developer workflow (resmi)

```sh
# 1. Developer membuat VM (dari host)
mivon run --hardware aurora-172

# 2. VM boot → Mivon Hardware OS
# 3. Di dalam guest:
git clone https://github.com/mivonsim/utbh
cd utbh
cargo build --release

# 4. Jalankan
./target/release/utbh discover
./target/release/utbh benchmark all
```

Urutan: `host → VM → Mivon OS → UTBH`.

---

## 5. Tujuh layer

```
┌───────────────────────────────┐
│ 7. RESULT / REPORT            │
├───────────────────────────────┤
│ 6. VALIDATION                 │
├───────────────────────────────┤
│ 5. BENCHMARK ENGINE           │
├───────────────────────────────┤
│ 4. TEST ENGINE                │
├───────────────────────────────┤
│ 3. WORKLOAD ENGINE            │
├───────────────────────────────┤
│ 2. HARDWARE DISCOVERY         │
├───────────────────────────────┤
│ 1. MIVON HARDWARE API         │
└───────────────────────────────┘
```

### Layer 1 — Hardware API (`utbh-api`)

Interface resmi antara UTBH dan Mivon OS. Satu-satunya jendela ke hardware.
Crate lain **dilarang** akses `/proc`, `/sys`, atau CPUID langsung.

Domain: CPU, GPU, Memory, Cache, DMA, NoC, Interrupt, Timer, Accelerator, IO.

### Layer 2 — Discovery (`utbh-discovery`)

Menemukan kemampuan hardware: ISA, core, cache, vector, GPU, memory,
bandwidth, accelerator, interconnect.

Output `utbh discover`:

```
UTBH Hardware Discovery

CPU
  Architecture : Mivon-RV64
  Cores        : 8
  Vector       : RVV
  L1           : 64 KiB
  L2           : 1 MiB
  L3           : 16 MiB

GPU
  Compute Units : 32
  FP32 / FP16 / INT8 : supported

Memory
  Capacity : 8 GiB
  Channels : 2

Interconnect
  Type : NoC   Topology : Mesh
```

### Layer 3 — Workload (`utbh-workload`)

Menyiapkan & mengeksekusi pekerjaan: integer, floating point, vector, matrix,
memory, GPU kernel, DMA, NoC traffic, multicore.

### Layer 4 — Test Engine (`utbh-test`)

Menguji apakah hardware **benar**: instruction, memory, cache, atomic, GPU,
DMA, interrupt correctness. Output: **PASS / FAIL**.

### Layer 5 — Benchmark Engine (`utbh-benchmark`)

Mengukur: latency, throughput, bandwidth, IPC, GFLOPS, GIOPS, memory
bandwidth, GPU utilization.

### Layer 6 — Validation (`utbh-validation`)

Differential validation:

```
TEST
  ├─ Execution A   → result A
  └─ Reference A   → result B
            └──→ Comparator ──→ PASS / FAIL
```

Jika FAIL → `utbh-trace` berisi: cycle, instruction, register, memory access,
interrupt, DMA, GPU command, NoC transaction. Validator berbicara lewat
hardware/OS interface, tidak memanggil simulator secara langsung.

### Layer 7 — Result (`utbh-cli` + `utbh-trace`)

Output: `.utbh-result`, `.utbh-trace`, `.utbh-report`.

---

## 6. Testing ≠ Benchmark

```
              UTBH
        ┌──────┴──────┐
     TESTING       BENCHMARK
  "benar?"      "seberapa cepat?"
```

Contoh hasil matrix multiplication:

```
Correctness : PASS
Latency     : 12,430 cycles
Throughput  : 185 GFLOPS
Bandwidth   : 113 GB/s
```

**Larangan:** menghasilkan `Score = 8,921` sebagai satu-satunya hasil. Angka
tunggal membuat manusia merasa memahami sesuatu yang sebenarnya belum dipahami.

---

## 7. Data-driven test package

Test **tidak di-hardcode di Rust**. Tidak ada `const TEST_001 = ...` untuk
ribuan test. Test didefinisikan deklaratif dan dibaca saat runtime:

```
UTBH binary  +  test packages  =  benchmark suite
```

Bukan `test → compile ulang UTBH`.

### Struktur package

```
utbh/
├── manifest.utbh
├── suites/
│   ├── cpu/  gpu/  memory/  cache/
│   ├── interconnect/  soc/  accelerator/
└── workloads/
```

### Format definisi test (TOML)

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

Test ditambahkan **tanpa mengubah binary utama UTBH**.

---

## 8. Kategori benchmark universal

| Domain | Kuantitas |
|---|---|
| CPU | Integer, FP, Branch, Atomic, SIMD, Vector, Crypto, Compression |
| GPU | FP32, FP16, BF16, INT8, Matrix, Vector, Reduction, Convolution, FFT |
| Memory | Atomic, Sync, Read/Write latency, Sequential/Random/Copy, Bandwidth, Cache, TLB, NUMA, Coherency |
| SoC | CPU↔GPU, CPU↔NPU, CPU↔DMA, GPU↔Mem, NPU↔Mem, DMA↔Mem |
| Interconnect | Latency, Bandwidth, Contention, Arbitration, Fairness, Ordering, Deadlock, Starvation |

---

## 9. Execution modes

UTBH tidak memiliki VM backend. Hardware environment berbeda memberi fidelity
berbeda, kode UTBH tetap sama:

```
UTBH → Mivon Hardware API → ┌ Fast VM        (architectural model)
                            ├ Accurate VM    (cycle model)
                            └ RTL            (implementation model)
```

---

## 10. Fuzzing

```
Random Generator → Hardware Workload → Mivon Hardware API → Hardware VM
                                                            │
                                                     Validation
                                                   ┌────┴────┐
                                                 PASS       FAIL
                                                            │
                                                        Minimizer
                                                            │
                                                 Reproducible Test
```

Fuzz kombinasi: CPU instruction + DMA + interrupt + cache pressure + GPU
workload + memory contention. Jauh lebih berguna untuk menemukan bug SoC
daripada menjalankan benchmark CPU berulang-ulang.

---

## 11. Dari VM sampai Silicon

Satu test definition dipakai di tiga target:

| Target | Model | Contoh hasil `vector_add.utbh` |
|---|---|---|
| VM | architectural / cycle | PASS · 1.23 ns |
| FPGA | implementation | PASS · 1.31 ns |
| ASIC | silicon | PASS · 0.94 ns |

Definisi test tetap sama. Hasilnya dibandingkan.

---

## 12. Repository layout

```
utbh/
├── Cargo.toml / Cargo.lock
├── AGENTS.md / design.md
├── crates/
│   ├── utbh-core/         # tipe bersama, loader test
│   ├── utbh-api/          # Layer 1 — Hardware API
│   ├── utbh-discovery/    # Layer 2
│   ├── utbh-workload/     # Layer 3
│   ├── utbh-test/         # Layer 4
│   ├── utbh-benchmark/    # Layer 5
│   ├── utbh-validation/   # Layer 6
│   ├── utbh-fuzz/
│   ├── utbh-trace/
│   └── utbh-cli/          # Layer 7
├── suites/                # data-driven test packages
├── workloads/
├── schemas/
│   ├── test/  hardware/  result/
├── configs/
├── results/               # gitignored
└── doc/design.md
```

Tidak ada: `mivon-vm/`, `verilator/`, `rtl-backend/`, `host-runner/`.

---

## 13. CLI final

```sh
utbh discover
utbh list                    # daftar semua suite + test
utbh test <suite>            # cpu | gpu | soc | ...
utbh benchmark <suite>
utbh run universal           # test + benchmark semua suite
utbh fuzz <suite> [--seed N --iterations N]
utbh stress <suite> [--iterations N --window N]
utbh report
utbh compare <a.utbh-result.json> <b.utbh-result.json>
utbh trace <run-id>
```

- `compare` — perbandingan dua run (§11: VM vs FPGA vs ASIC): status PASS/FAIL
  per test, delta latensi, regresi PASS→FAIL.
- `stress` — beban berkelanjutan: deteksi FAIL kumulatif (iterasi pertama)
  + degradasi latensi antar window (throttling/race).

---

## 14. Lifecycle final

```
Developer
  │ mivon run
  ▼
Mivon Hardware VM
  → Boot Hardware OS
  → Network / Git
  → git clone UTBH
  → cargo build
  → UTBH
      ├─ TEST ─┐
      ├─ BENCHMARK ─┼→ VALIDATION → TRACE / RESULT → REPORT / EXPORT
      └─ FUZZ ─┘
```

---

## 15. Keputusan desain (ADR ringkas)

| # | Keputusan | Alasan |
|---|---|---|
| D1 | Rust workspace multi-crate | Layer tegas; `utbh-api` jadi satu-satunya pintu hardware |
| D2 | Test deklaratif TOML | Tambah test tanpa rebuild |
| D3 | Skor = multi-metrik + PASS/FAIL | Cegah ilusi pemahaman dari skor tunggal |
| D4 | Differential validation (execution vs reference) | Menemukan bug implementasi, bukan hanya crash |
| D5 | Reproducible fuzz seed | Setiap FAIL bisa direproduksi |
| D6 | Tidak ada VM backend di UTBH | Satu binary untuk fast VM / cycle VM / RTL / FPGA / ASIC |
| D7 | Output terpisah `.utbh-result` / `.utbh-trace` / `.utbh-report` | Mesin-baca vs manusia-baca dipisah |

---

## 16. Roadmap

- [x] M1: Workspace build, `discover` + `test cpu` jalan
- [x] M2: Full suite cpu/memory/cache + benchmark engine + report
- [x] M3: Suite GPU + SoC + interconnect (payload workload generik; kernel
      GPU khusus menunggu Hardware API compute Mivon)
- [x] M4: Fuzz engine + minimizer + trace FAIL (`utbh fuzz`, `utbh trace`)
- [x] M4b: Stress engine (`utbh stress`) + perbandingan run (`utbh compare`)
- [x] M4c: Atomic correctness (`atomic_add`) + multicore reduce (`parallel_sum`),
      `utbh list`, CI (`.github/workflows/ci.yml`)
- [ ] M5: Differential validation vs RTL reference (butuh jalur reference
      dari Mivon OS — saat ini reference = implementasi independen di UTBH)
- [ ] M6: Perbandingan hasil VM → FPGA → ASIC lewat `utbh compare`
