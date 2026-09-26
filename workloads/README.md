# workloads/ — data mentah workload

Belum terpakai oleh engine saat ini. Rencana (lihat `doc/design.md` §7):

- Dataset referensi untuk workload berbasis file (mis. pola akses memory
  acak yang di-share antar test, corpus untuk fuzzing).
- Payload GPU kernel yang dibuat sekali, dipakai ulang lintas suite.

Saat ini semua input dihasilkan deterministik dari seed
(`utbh_core::inputs()`), jadi belum ada file yang harus disimpan di sini.
