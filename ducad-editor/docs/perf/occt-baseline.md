# Baseline performa kernel OCCT (PLAN_MIGRASI_TRUCK F0.3)

Pembanding untuk gate F5 ("benchmark ≤ 2× baseline OCCT untuk 90 % op").
Ukur ulang di mesin yang sama sebelum membandingkan dengan backend truck.

| Item | Nilai |
|---|---|
| Tanggal | 2026-10-19 |
| Commit basis | `05c07369` (v0.5.6) + perubahan F0 |
| Mesin | Apple M2, macOS (Darwin 27.0.0) |
| Kompilator | rustc 1.96.1 |
| Profil | `bench` (release) untuk criterion; `dev` (opt-level dari workspace, + debuginfo) untuk replay contoh |

## 1. Op kernel tunggal — `crates/ducad-kernel/benches/ops.rs`

```bash
cargo bench -p ducad-kernel --bench ops -- --save-baseline occt
# nanti, backend truck:
cargo bench -p ducad-kernel --bench ops --no-default-features --features kernel-truck -- --baseline occt
```

Median criterion (20 sampel):

| Bench | Waktu |
|---|---:|
| `primitive/box` 60×40×8 | 21.8 µs |
| `primitive/cylinder` r5 h20 | 5.3 µs |
| `extrude/rect` | 69.0 µs |
| `extrude/rect_4_holes` (`Profile::WithHoles`) | 106.6 µs |
| `revolve/ring_360` | 99.6 µs |
| `boolean/subtract_hole` (plat − silinder) | 2.09 ms |
| `boolean/union_boss` (plat ∪ silinder) | 1.18 ms |
| `modify/fillet_all_r1` (box) | 4.29 ms |
| `modify/fillet_all_r1_holed` (box berlubang) | 5.00 ms |
| `modify/chamfer_all_0.5` | 4.00 ms |
| `modify/shell_open_top_t2` | 2.40 ms |
| `pattern/linear_5x4` (20 salinan, tanpa union) | 251 µs |
| `query/tessellate` (plat berlubang + fillet) | 3.04 ms |
| `query/volume` | 0.89 ms |
| `query/surface_area` | 1.66 ms |
| `query/enumerate_faces` | 3.45 ms |
| `io/step_roundtrip` (tulis + baca teks STEP) | 27.3 ms |

## 2. Replay per contoh korpus golden

Tabel lengkap per contoh dan per jenis op: [`occt-examples.md`](occt-examples.md)
(data mentah: [`occt-examples.json`](occt-examples.json)). Dibuat ulang dengan:

```bash
cargo run -p ducad-cli -- golden capture --perf docs/perf/occt-examples.md
```

Catatan membaca angka:

- Waktu satu op = kernel + tessellasi + dimensi tepi + ekstraksi tepi
  (`compute::finish`), jadi bukan biaya kernel murni.
- Angka replay bising (±2× antar-run, mis. op `helix` di `loft_helix` 3–7 s
  bergantung beban mesin). Untuk gate F5, bandingkan median beberapa run atau
  bench criterion di §1, bukan satu run replay.
- Op terberat di korpus: `helix` (sweep heliks), `mirror` pada pegas heliks,
  dan `hole` (rata-rata 81 ms, maks 233 ms). Ini kandidat pertama yang perlu
  diukur di backend truck (risiko R7).
