# Baseline Performa DUCAD (Milestone M1)

Dokumen ini mencatat metrik baseline performa untuk pipeline render vektor, kamera 2D, dan coretan tinta di viewport DUCAD (`ducad-render`).

## 1. Informasi Mesin Referensi
- **Perangkat**: Apple Mac (arm64 / Apple Silicon)
- **Sistem Operasi**: Darwin Kernel Version 27.0.0 (macOS)
- **Host**: `Air15.local`
- **Tanggal Pengujian**: 24 September 2026
- **Commit Basis**: `4b82aa1` (Milestone M1)
- **Kompilator**: `rustc` / `cargo 1.96.1`

---

## 2. Hasil Benchmark Tinta (`benches/ink.rs`)

Dijalankan menggunakan kerangka benchmark `criterion`:
```bash
cargo bench -p ducad-render
```

| Benchmark | Waktu Pengukuran (Rata-rata) | Target / Budget | Status |
| :--- | :--- | :--- | :--- |
| `append_one_point` | **1.045 µs** | ≤ 50.000 µs (50 µs) | **LULUS** (~48× lebih cepat) |
| `rebuild_100_strokes_100pts` | **248.19 µs** (~2.48 µs / stroke) | N/A | **LULUS** |

### Analisis:
- `append_one_point` mengupdate buffer vertex goresan yang sedang aktif dengan hanya membuang 2 vertex cap lama dan menambahkan 2 vertex baru + 2 vertex cap baru. Eksekusi ~1.05 µs jauh berada di bawah budget 50 µs, menjamin latensi input stylus/Apple Pencil 120 Hz / 240 Hz bebas jeda (frame budget 8.3 ms).
- Pembangunan ulang 100 goresan (masing-masing 100 titik sampel) memakan waktu ~248 µs, memungkinkan rekonstruksi layer tinta instan saat rotasi bidang atau pembatalan (undo/redo).

---

## 3. Spesifikasi Memori & Geometri Render

- **VectorVertex Stride**: 48 byte (align 16, pos [f32; 3], color [f32; 4], paint u32, uv [f32; 2], pad [f32; 2]).
- **InkVertex Stride**: 36 byte (align 4, pos [f32; 3], side f32, color [f32; 4], soft f32).
- **GradientUniform**: 288 byte (std140, kind u32, count u32, p0 [f32; 2], p1 [f32; 2], pad [f32; 2], 8 stops @ 32 byte).
- **Max Gradients Per Layer Batch**: 64 gradien (18.432 byte / batch GPU uniform).
- **VectorCache Default Budget**: 64 MB dengan penggusuran LRU deterministik.
- **Batas Zoom Kamera 2D**: 0.01 px/mm hingga 1000.0 px/mm (jarak kamera 0.5 mm hingga 150.000 mm).

---

## 4. Verifikasi Smoke Benchmark CI
Untuk memastikan seluruh benchmark terkompilasi tanpa error di continuous integration:
```bash
cargo bench --no-run -p ducad-render
```
Status: **Hijau (100% lolos kompilasi)**.
