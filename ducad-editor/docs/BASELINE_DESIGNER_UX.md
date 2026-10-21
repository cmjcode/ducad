# Baseline Step 0 — Workflow Designer

Hasil Step 0 dari `docs/PLAN_DESIGNER_UX.md`: kondisi DUCAD saat ini untuk
workflow sasaran, diukur sebelum pekerjaan Step 1 dimulai. Dokumen ini menjadi
pembanding untuk tiap step berikutnya. Jangan diubah untuk "memperbaiki" angka;
tambahkan baris pengukuran baru di bawahnya.

**Tanggal:** 2026-10-21  
**Commit basis:** `53ecb880` (working tree berisi perubahan lain yang belum
di-commit; baseline ini hanya menyentuh berkas yang tercantum di bagian 9)  
**Mesin:** Apple Silicon (arm64), macOS (Darwin 27), `rustc 1.96.1`

## 1. Keputusan yang sudah disepakati

| Keputusan | Isi | Status |
|---|---|---|
| Mesin pemodelan | Oplog `ducad-engine` menjadi satu-satunya sumber kebenaran; `parametric_engine` GUI dipensiunkan bertahap. ADR ditulis di awal Step 1a. | ✅ disetujui |
| Platform utama persona | Desktop (macOS/Windows) dengan mouse dan keyboard. | ✅ disetujui |
| Persona | Designer produk yang membuat dan merevisi part mekanik sederhana. | ✅ disetujui |
| Tanggal berlaku rencana | Sejak kapan pekerjaan di luar cakupan (cloud sync, lisensi, akun — ada di working tree) dibekukan atau diselesaikan dulu. | ☐ **menunggu keputusan** |
| Target angka §6 | Target awal di rencana dipakai apa adanya sampai sesi uji designer pertama memberi data. | 🟡 sementara |

## 2. Proyek contoh

Bracket L yang dipakai di seluruh rencana:

- Profil L di bidang XZ, **sketsa terkunci penuh (DOF 0)** dengan dimensi dari
  parameter: alas `base`=40, tinggi `height`=35, tebal `t`=5.
- Extrude `len`=60, fillet sudut dalam `r`=3, 2 lubang M5 di alas, 2 lubang M5
  di dinding, berjarak `hole_off`=15 dari tiap ujung.
- Fixture: `crates/ducad-engine/tests/fixtures/sample_bracket.ops.json`.
- Tes alur penuh: `crates/ducad-engine/tests/sample_project.rs` (berjalan di CI
  lewat `cargo test --workspace`).
- Skenario uji manual: `docs/CEKLIS_UJI_GUI.md` Bagian P (tugas T1–T6).

Referensi face/tepi di fixture sengaja **tidak memakai koordinat tetap**:

| Fitur | Selector | Arti |
|---|---|---|
| Fillet sudut dalam | `of(+Z except >Z) and of(+X except >X)` | tepi bersama antara permukaan atas alas dan sisi dalam dinding |
| Lubang alas | `+Z except >Z` | face menghadap +Z selain yang paling atas |
| Lubang dinding | `-X` | sisi belakang dinding |

Posisi lubang memakai `at_world` berupa ekspresi parameter, mis.
`"$t + ($base - $t)/2"`.

## 3. Baseline jalur oplog (otomatis)

Diukur lewat `sample_project.rs`. ✅ = lulus otomatis, ❌ = gagal (tes target
`#[ignore]` menunggu step yang disebut).

| Tugas | Hasil | Bukti |
|---|---|---|
| T1 Buat model dari sketsa berdimensi | ✅ DOF 0, satu body valid, volume = analitik (rel < 1e-4) | `builds_fully_constrained_bracket` |
| T2 Ubah dimensi awal | ✅ 5 variasi (t, base+height, r, len, kombinasi): fillet dan 4 lubang ikut, volume = analitik | `editing_initial_dimensions_keeps_downstream_features` |
| T2 Selector koordinat tetap | ✅ gagal secara eksplisit (`selector_empty` pada `inner_fillet`), sesi utuh — tidak memilih tepi lain diam-diam | `coordinate_selector_fails_explicitly_after_edit` |
| T3 Pulih dari operasi gagal | ✅ `fillet_radius_too_large` + hint + batas, rollback utuh, param lama dipertahankan | `failed_edit_rolls_back_and_offers_verified_fix` |
| T3 Saran perbaikan | ❌ fix bertanda `verified` bisa merusak op hilir (lihat B0-2) | `verified_fixes_survive_full_replay` (ignored, Step 3) |
| T3 Undo perubahan parameter | ❌ tidak bisa (lihat B0-1) | `param_edit_is_one_undo_step` (ignored, Step 1a) |
| T4 Simpan → buka ulang | ✅ oplog, param, volume, bbox sama | `save_and_reopen_roundtrip` |
| T5 Ekspor STEP/STL | ✅ STEP dibuka ulang dengan volume sama; STL berisi segitiga | `export_step_reopens_and_stl_is_written` |
| T6 Drawing → PDF/DXF | ✅ ≥ 3 tampak, dimensi otomatis, catatan lubang, PDF valid, DXF berisi garis | `drawing_exports_pdf_and_dxf` |

## 4. Baseline jalur GUI manual (dari kode, perlu dikonfirmasi sesi uji)

Belum ada sesi uji designer. Dugaan berikut berasal dari membaca
`crates/ducad-app/src/modeling/parametric_engine.rs` dan
`crates/ducad-core/src/parametric.rs`; konfirmasi dengan Bagian P.

| Tugas | Dugaan hasil | Sebab di kode |
|---|---|---|
| T1 | Mungkin berhasil, tetapi fitur GUI tidak masuk oplog | GUI menulis `FeaturePayload` ke `parametric_dag`, bukan ke oplog engine |
| T2 | **Kemungkinan besar gagal** | `Fillet` tidak menyimpan referensi tepi; regenerasi memanggil `fillet_all` sehingga semua tepi ikut di-fillet. `Hole` (juga `Helix`, `Boolean`, …) berstatus `Unsupported` saat regenerasi → lubang hilang dengan pesan "regenerasi fitur ini belum didukung". `Extrude`/`Revolve` merujuk `plane_index: usize` |
| T3 | Sebagian | Status error per fitur ada (`FeatureStatus::Error`), tetapi tanpa saran perbaikan seperti oplog |
| T4–T6 | Belum dinilai | Perlu sesi uji |

## 5. Bug dan celah yang ditemukan

| ID | Temuan | Dampak designer | Step |
|---|---|---|---|
| B0-1 ✅ | *(Diperbaiki Step 1a, 2026-10-21: riwayat undo sesi mencakup batch op dan edit; GUI membawa oplog di transaksi undo — ADR 0010.)* `Session::undo` hanya mencabut batch op terakhir. `set_params`, `replace_op`, dan `remove_op` tidak tercatat di riwayat undo; undo setelah edit parameter malah mencabut op. | Undo setelah mengubah dimensi tidak mengembalikan dimensi | 1a |
| B0-2 | Saran perbaikan (`fixes[].verified = true`) hanya diuji pada op yang gagal, tanpa me-replay op hilir. Contoh: fillet r=40 → fix r=26.9 "terverifikasi", tetapi `base_holes` lalu gagal `hole_outside_face`. | Menerima saran resmi bisa merusak fitur lain | 3 |
| B0-3 | Filter selector tidak menerima parameter (`+Z[z=$t]` → `selector_syntax`). Selector berbasis koordinat (`[z=5]`) putus saat dimensi berubah. | Penulis oplog/agent mudah membuat model yang rapuh; perlu pola selector relasional (bagian 2) atau filter ber-ekspresi | 1b |
| B0-4 | Hint `hole_outside_face` selalu menyebut "koordinat 'at' relatif terhadap pusat face", juga saat yang dipakai `at_world`; tidak ada `fixes`. | Pesan menyesatkan saat pulih dari error | 3 |
| B0-5 | GUI: `Fillet` diregenerasi sebagai `fillet_all`; `Hole`, `Helix`, `Boolean` tidak diregenerasi. | Edit dimensi awal di GUI merusak/menghilangkan fitur hilir | 1a (migrasi ke oplog) |
| B0-7 | Op `hole` lambat: ±0,5–0,6 s per op (2 lubang tembus) pada part kecil; regenerasi dan buka berkas proyek contoh ±1,1–1,3 s. Analisis Step 1a: biaya ada di boolean kernel, satu boolean per titik lubang (1 titik 112 ms, 2 titik 215 ms pada bracket tanpa fillet), dan naik ±3× bila body punya face lengkung (2 lubang setelah fillet: 703 ms). Lubang buta dengan kedalaman tepat = tebal (bidang koplanar) ±1 s. | Edit dimensi terasa tersendat; buka berkas lambat | 1b: satu boolean untuk semua titik lubang; replay inkremental dari op pertama yang terdampak; profil boolean kernel untuk face lengkung/koplanar |
| B0-8 🟡 | *(Untuk fitur yang sudah dimigrasi ke oplog — sketsa, extrude, fillet/chamfer, hole — diperbaiki: oplog dan ikatan sketsa dipulihkan saat berkas dibuka, tes `gui_oplog_survives_save_and_reopen`. DAG fitur lama tetap tidak disimpan.)* GUI: DAG fitur tidak disimpan ke `.ducad` dan direset saat buka/baru; riwayat aktivitas (dan cabangnya) dikosongkan setiap aplikasi start (`app.rs:569`). | Setelah buka ulang, riwayat fitur GUI hilang — T4 gagal di GUI | 1a/1c |
| B0-9 | Oplog belum punya op assembly/mate/coupling, datum bersudut/3 titik, teks sketsa, atau edit sketsa (trim/extend/offset/mirror). | Penghalang keputusan "oplog = sumber kebenaran"; harus masuk daftar ADR Step 1a | 1a |
| B0-10 | Platform utama persona adalah desktop macOS, tetapi CI tidak punya lane workspace macOS dan tes Windows `continue-on-error`. | Regresi di platform utama tidak tertangkap CI | 5 (gate rilis) |
| B1-1 | `diff` geometris (CLI/MCP `diff`, ghost proposal `preview_edit`) macet >60 s pada dua versi bracket dengan fillet sudut dalam + lubang (alas 40 vs 55). Stack: `truck_geometry::intersection_curve::hinted_sub_division`. Union silinder koaksial biasa tidak terdampak. Jalur edit GUI sudah menghindari diff. | Proposal agent dengan ghost preview pada part seperti ini membuat aplikasi/MCP macet | 3 (audit kernel: gagal jadi error, bukan macet); tes `diff.rs::diff_of_filleted_bracket_versions_finishes_in_time` (ignored), fixture `gui_bracket_hole.ops.json` |
| B0-6 | `docs/PLAN_DESIGNER_UX.md` merujuk `README.md` di `ducad-editor/`, padahal README ada di root repo. | Dokumentasi | 0 (diperbaiki di rencana) |

## 6. Baseline waktu (jalur oplog, build release)

Perintah:

```bash
cargo test --release -p ducad-engine --test sample_project -- --ignored baseline_timings --nocapture
```

Apple M2, release, commit `53ecb880`, dua kali jalan:

| Langkah | Jalan 1 | Jalan 2 |
|---|---|---|
| Bangun 5 op dari kosong | 1064 ms | 1052 ms |
| Regenerasi setelah `t` 5 → 8 | 1266 ms | 1202 ms |
| Simpan `.ducad` | 6.6 ms | 10.1 ms |
| Buka `.ducad` (replay) | 1135 ms | 1130 ms |
| Ekspor STEP | 5.4 ms | 5.1 ms |
| Drawing otomatis + PDF | 71 ms | 70 ms |

Rincian kumulatif per op (CLI profil dev ter-optimasi, satu kali jalan):
sketch 54 ms → +extrude 47 ms → +fillet 90 ms → +`base_holes` 698 ms →
+`wall_holes` 1365 ms. **Dua op `hole` memakan ±90 % waktu regenerasi** (B0-7).
Untuk edit dimensi interaktif, ±1,2 s per perubahan terlalu lambat; target
awal yang diusulkan: regenerasi proyek contoh ≤ 200 ms.

Metrik render/viewport ada di `docs/PERF_BASELINE.md`; baseline ini hanya
mencakup alur dokumen.

## 7. Matriks status kemampuan

Lihat `docs/STATUS_KEMAMPUAN.md`: ±70 baris kemampuan dengan status dan
bukti, plus 19 kontradiksi README vs implementasi (mis. extrude "Up to Face",
radial menu `Space`, `./test_api.sh` tidak ada, `docs/STATUS_FASE_A_B.md` basi
terhadap kernel truck).

## 8. Yang belum dikerjakan di Step 0

- **Sesi uji designer (Bagian P)** belum dijalankan; butuh penguji manusia.
  Hasilnya mengisi tabel P.3/P.4 dan target angka §6 rencana.
- **Tanggal berlaku rencana** (bagian 1) menunggu keputusan pemilik produk.

### Kegagalan tes yang sudah ada sebelum pekerjaan ini (bukan regresi)

- `ducad-core` `hole::tests::iso_table_in_vault_template_matches_engine`:
  membaca `scripts/memory-vault/Standards/ISO-Metric-Fasteners.md` di repo
  luar, yang dihapus di commit `cd57db0f`.
- `ducad-core/tests/layering.rs` (2 tes): crate `ducad-update` belum ada di
  tabel layering dan peta CLAUDE.md.

## 8a. Kemajuan Step 1 (2026-10-21)

| Butir | Hasil | Bukti |
|---|---|---|
| ADR mesin tunggal | ✅ `docs/adr/0010-oplog-mesin-tunggal.md` (keputusan, urutan migrasi 10 langkah, alternatif yang ditolak) | — |
| B0-1 undo edit (headless) | ✅ batch op, `set_params`, `replace_op`, `remove_op` → satu langkah undo/redo masing-masing; edit gagal tidak menambah langkah; bertahan setelah buka ulang | `sample_project.rs::{param_edit_is_one_undo_step, failed_param_edit_adds_no_undo_step, mixed_edits_undo_and_redo_in_order, undo_after_reopen_covers_ops_and_param_edits}`, `edit.rs` |
| Undo GUI membawa oplog | ✅ undo/redo geometri memulihkan oplog yang menyertainya; tidak lagi `oplog_stale` setelah undo | `ducad-app/src/oplog_undo.rs` (4 tes), `ducad-core` `UndoStack::attach_to_top` (2 tes) |
| Pembuat selector dari klik GUI | ✅ `select::suggest_faces/suggest_edges`: pada bracket contoh 46 face/tepi tunggal → 30 semantik, 16 numerik, 0 `idx:`; selector tepi sudut dalam & face atas alas tetap benar setelah `t` 5→8 | `tests/select_suggest.rs` (6 tes) |
| Migrasi fitur GUI ke op | 🟡 urutan 1–4 selesai (sketsa terikat, extrude, fillet/chamfer, hole); T1–T2 lewat GUI lulus tes otomatis. Sebelumnya: urutan 3 selesai: fillet/chamfer GUI (termasuk variabel) menjadi op oplog dengan selector semantik dari tepi yang diklik; menutup B0-5 untuk fillet/chamfer. Urutan 1–2 (sketsa, extrude) dirancang di ADR 0010 | `ducad-app/src/oplog_gui.rs` (4 tes), tes lama `error_card_ui::failed_fillet_offers_verified_fix_that_reapplies` kini lewat op |

## 8b. Kemajuan Step 2 (2026-10-21)

Butir rencana Step 2 dan hasilnya. Kriteria "pengguna uji dapat membuat dan
merevisi proyek contoh tanpa petunjuk lisan" hanya bisa dinilai lewat sesi
uji Bagian P — **belum dilakukan**.

| Butir Step 2 | Hasil | Bukti |
|---|---|---|
| 1. Urutan kerja sketsa → constraint → solid → revisi | 🟡 alur T1–T2 lewat GUI berjalan ujung-ke-ujung dan teruji otomatis; urutan tampilan menu tidak diubah | `oplog_gui::gui_sketch_extrude_fillet_then_edit_sketch_updates_downstream` |
| 2. Status sketsa yang bisa ditindaklanjuti | ✅ `ducad_sketch::constraint::sketch_status` (semua entitas dihitung, entitas bebas lewat ruang nol Jacobian, constraint redundan/putus per indeks); GUI: entitas terkunci hijau, glyph constraint bermasalah merah, baris status "N DOF tersisa, M entitas belum terkunci" | `constraint/tests.rs::sketch_status_tests` (3), `sketch_status_ui` (2) |
| 3. Dimensi bisa diedit langsung + validasi | 🟡 nilai utama fitur (jarak extrude, radius fillet, …) bisa diedit dari Feature Tree; popup dimensi menerima satuan eksplisit dan ekspresi (`1 in + 3 mm`, `25.4/2`), input tidak valid tidak lagi ditutup diam-diam. **Belum:** mengikat dimensi sketsa GUI ke parameter `$param` | `units_input` (4 tes), `feature_tree_edits_value_suppresses_and_refuses_unsafe_delete` |
| 4. Dependensi fitur terbaca + edit/suppress/rollback | ✅ seksi "Riwayat parametrik (oplog)" di Feature Tree: nama, jenis, ringkasan, dependensi (bergantung pada / dipakai oleh), edit nilai, suppress (`DesignDoc.suppressed`), hapus (ditolak dengan pesan bila masih dipakai); tiap aksi satu langkah undo. Roll-to marker untuk oplog belum ada | `ducad-engine/src/oplog_view.rs` (2 tes), tes app di atas |
| 5. Validasi profil sebelum solid | ✅ celah profil ditunjuk lokasinya (ujung terbuka disorot merah, ukuran celah di pesan) + tombol aman "Tutup celah N mm" (satu langkah undo, lalu extrude diulang) | `open_profile_offers_close_gap_fix_that_extrudes` |
| 6. Satuan dan presisi | ✅ satuan aktif sudah ada di UI (`LengthUnit`); input kini eksplisit: angka polos = satuan aktif (popup dimensi) atau mm (input fitur, seperti sebelumnya), satuan tertulis selalu menang, ekspresi didukung, input salah ditolak dengan alasan | `ducad-core/src/units_input.rs` |

Kriteria selesai Step 2:

| Kriteria | Status |
|---|---|
| Pengguna uji membuat dan merevisi proyek contoh tanpa petunjuk lisan | ☐ butuh sesi uji Bagian P |
| Perubahan parameter awal memperbarui model secara benar dan terprediksi | ✅ otomatis (engine + GUI) |
| Status constraint dan profil membantu menemukan masalah tanpa membaca log | ✅ (perlu dikonfirmasi sesi uji) |
| T4 lewat GUI: simpan → buka → edit sketsa tetap memperbarui hilir | ✅ `gui_oplog_survives_save_and_reopen` |
| Satu perintah = satu undo step, termasuk edit dimensi | ✅ untuk aksi oplog; edit dimensi sketsa = satu langkah sketsa + satu langkah pembaruan model saat keluar dari mode sketsa |

## 9. Berkas yang ditambahkan/diubah oleh Step 0

- `crates/ducad-engine/tests/fixtures/sample_bracket.ops.json` (baru)
- `crates/ducad-engine/tests/sample_project.rs` (baru)
- `docs/BASELINE_DESIGNER_UX.md` (baru, dokumen ini)
- `docs/STATUS_KEMAMPUAN.md` (baru)
- `docs/CEKLIS_UJI_GUI.md` (tambah Bagian P)
- `docs/PERF_BASELINE.md` (tambah baseline alur dokumen)
- `docs/PLAN_DESIGNER_UX.md` (status Step 0)
