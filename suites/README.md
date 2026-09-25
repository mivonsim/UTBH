# suites/ — data-driven test package

Setiap file `*.utbh` (TOML) adalah satu test definition. UTBH membacanya saat
runtime — **menambah test tanpa mengubah binary**.

Struktur:

```
suites/
├── cpu/          # integer, FP, branch, SIMD, vector, matrix
├── gpu/          # FP32/FP16/INT8 compute, matrix, vector
├── memory/       # bandwidth, latency, seq/random
├── cache/        # working set, coherency
├── noc/          # interconnect: bandwidth, contention
├── soc/          # CPU↔GPU, DMA, dataflow
└── accelerator/  # unit akseleserat khusus
```

Format lihat AGENTS.md § "Menambah test baru".
