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
| B0-2 ✅ | *(Diperbaiki Step 3: `reverify_fixes` menguji ulang setiap fix dengan replay seluruh oplog; tes `verified_fixes_survive_full_replay` aktif.)* Saran perbaikan (`fixes[].verified = true`) hanya diuji pada op yang gagal, tanpa me-replay op hilir. Contoh: fillet r=40 → fix r=26.9 "terverifikasi", tetapi `base_holes` lalu gagal `hole_outside_face`. | Menerima saran resmi bisa merusak fitur lain | 3 |
| B0-3 | Filter selector tidak menerima parameter (`+Z[z=$t]` → `selector_syntax`). Selector berbasis koordinat (`[z=5]`) putus saat dimensi berubah. | Penulis oplog/agent mudah membuat model yang rapuh; perlu pola selector relasional (bagian 2) atau filter ber-ekspresi | 1b |
| B0-4 ✅ | *(Diperbaiki Step 3: hint menyesuaikan `at`/`at_world`.)* Hint `hole_outside_face` selalu menyebut "koordinat 'at' relatif terhadap pusat face", juga saat yang dipakai `at_world`; tidak ada `fixes`. | Pesan menyesatkan saat pulih dari error | 3 |
| B0-5 | GUI: `Fillet` diregenerasi sebagai `fillet_all`; `Hole`, `Helix`, `Boolean` tidak diregenerasi. | Edit dimensi awal di GUI merusak/menghilangkan fitur hilir | 1a (migrasi ke oplog) |
| B0-7 | Op `hole` lambat: ±0,5–0,6 s per op (2 lubang tembus) pada part kecil; regenerasi dan buka berkas proyek contoh ±1,1–1,3 s. Analisis Step 1a: biaya ada di boolean kernel, satu boolean per titik lubang (1 titik 112 ms, 2 titik 215 ms pada bracket tanpa fillet), dan naik ±3× bila body punya face lengkung (2 lubang setelah fillet: 703 ms). Lubang buta dengan kedalaman tepat = tebal (bidang koplanar) ±1 s. | Edit dimensi terasa tersendat; buka berkas lambat | 1b: satu boolean untuk semua titik lubang; replay inkremental dari op pertama yang terdampak; profil boolean kernel untuk face lengkung/koplanar |
| B0-8 🟡 | *(Untuk fitur yang sudah dimigrasi ke oplog — sketsa, extrude, fillet/chamfer, hole — diperbaiki: oplog dan ikatan sketsa dipulihkan saat berkas dibuka, tes `gui_oplog_survives_save_and_reopen`. DAG fitur lama tetap tidak disimpan.)* GUI: DAG fitur tidak disimpan ke `.ducad` dan direset saat buka/baru; riwayat aktivitas (dan cabangnya) dikosongkan setiap aplikasi start (`app.rs:569`). | Setelah buka ulang, riwayat fitur GUI hilang — T4 gagal di GUI | 1a/1c |
| B0-9 | Oplog belum punya op assembly/mate/coupling, datum bersudut/3 titik, teks sketsa, atau edit sketsa (trim/extend/offset/mirror). | Penghalang keputusan "oplog = sumber kebenaran"; harus masuk daftar ADR Step 1a | 1a |
| B0-10 | Platform utama persona adalah desktop macOS, tetapi CI tidak punya lane workspace macOS dan tes Windows `continue-on-error`. | Regresi di platform utama tidak tertangkap CI | 5 (gate rilis) |
| B1-1 ✅ | *(Diperbaiki Step 3: boolean/fillet/chamfer/hole kernel berbatas waktu (`compute::budget`, 45 s op, 8 s pratinjau); lewat batas → `kernel_timeout`, model utuh. Diff bracket kini selesai ±9 s dengan peringatan. Tes `diff_timeout.rs`, `kernel_budget.rs`.)* `diff` geometris (CLI/MCP `diff`, ghost proposal `preview_edit`) macet >60 s pada dua versi bracket dengan fillet sudut dalam + lubang (alas 40 vs 55). Stack: `truck_geometry::intersection_curve::hinted_sub_division`. Union silinder koaksial biasa tidak terdampak. Jalur edit GUI sudah menghindari diff. | Proposal agent dengan ghost preview pada part seperti ini membuat aplikasi/MCP macet | 3 (audit kernel: gagal jadi error, bukan macet); tes `diff.rs::diff_of_filleted_bracket_versions_finishes_in_time` (ignored), fixture `gui_bracket_hole.ops.json` |
| B1-2 ✅ | Boolean GUI memakai `HashSet` seleksi body, sehingga peran sasaran/pemotong pada **subtract** acak. Diperbaiki: seleksi body kini berurutan (`IndexSet`), body yang dipilih pertama = sasaran. | Subtract bisa memotong body yang salah | 1a; tes `gui_subtract_respects_selection_order_and_is_an_op` |
| B1-3 ✅ | Revolve GUI selalu memakai bidang XY (`PlaneFrame::top()`) apa pun bidang sketsanya, sehingga sketsa di bidang Front/Right diputar di sumbu yang salah. Diperbaiki lewat jalur oplog (bidang dari slot sketsa). | Hasil revolve salah posisi/orientasi | 1a; tes `gui_revolve_on_front_plane_uses_real_plane` |
| B1-4 ✅ | Loft GUI selalu berjalan sepanjang Z (bidang XY) apa pun bidang sketsanya. Diperbaiki lewat jalur oplog (bidang aktif + offset). | Loft dari sketsa Front/Right salah orientasi | 1a; tes `gui_loft_records_ops_on_the_real_plane` |
| B1-5 ✅ | Replay oplog menukar seluruh `ModelDoc`: semua `BodyId`/UUID berganti, sehingga instance assembly dan seleksi putus setiap kali dimensi/parameter diubah. Diperbaiki: hasil replay diterapkan sebagai selisih yang mempertahankan id body bernama sama (ADR 0011). | Assembly rusak setelah edit part | 1a; tes `assembly_instances_survive_param_replay` |
| B1-6 ✅ | Teks sketsa memberi nama yang sama ("Teks: AO") ke semua entitas glyph, sehingga sketsa teks tidak bisa dijadikan op (`duplicate_id`). Diperbaiki di `entity_spec_name`. | Extrude teks tidak tersimpan di oplog | 1a; tes `gui_text_extrude_round_trips_through_oplog` |
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
| Migrasi fitur GUI ke op | ✅ urutan 1–10 tertangani (lihat ADR 0010 §Status; beberapa sub-kasus tetap jalur lama dan tercatat di sana; assembly lewat ADR 0011); sebelumnya urutan 1–4 selesai (sketsa terikat, extrude, fillet/chamfer, hole); T1–T2 lewat GUI lulus tes otomatis. Sebelumnya: urutan 3 selesai: fillet/chamfer GUI (termasuk variabel) menjadi op oplog dengan selector semantik dari tepi yang diklik; menutup B0-5 untuk fillet/chamfer. Urutan 1–2 (sketsa, extrude) dirancang di ADR 0010 | `ducad-app/src/oplog_gui.rs` (4 tes), tes lama `error_card_ui::failed_fillet_offers_verified_fix_that_reapplies` kini lewat op |

## 8b. Kemajuan Step 2 (2026-10-21)

Butir rencana Step 2 dan hasilnya. Kriteria "pengguna uji dapat membuat dan
merevisi proyek contoh tanpa petunjuk lisan" hanya bisa dinilai lewat sesi
uji Bagian P — **belum dilakukan**.

| Butir Step 2 | Hasil | Bukti |
|---|---|---|
| 1. Urutan kerja sketsa → constraint → solid → revisi | 🟡 alur T1–T2 lewat GUI berjalan ujung-ke-ujung dan teruji otomatis; urutan tampilan menu tidak diubah | `oplog_gui::gui_sketch_extrude_fillet_then_edit_sketch_updates_downstream` |
| 2. Status sketsa yang bisa ditindaklanjuti | ✅ `ducad_sketch::constraint::sketch_status` (semua entitas dihitung, entitas bebas lewat ruang nol Jacobian, constraint redundan/putus per indeks); GUI: entitas terkunci hijau, glyph constraint bermasalah merah, baris status "N DOF tersisa, M entitas belum terkunci" | `constraint/tests.rs::sketch_status_tests` (3), `sketch_status_ui` (2) |
| 3. Dimensi bisa diedit langsung + terhubung ke parameter + validasi | ✅ popup dimensi menerima `$param`/ekspresi (`$base + 5`): dimensi diikat ke parameter dokumen (`Sketch::dim_exprs`) dan op `sketch` memuat ekspresinya; mengubah parameter (panel Parameter atau agent `set_params`) menggerakkan body DAN garis sketsa GUI; undo memulihkan keduanya. Nilai utama fitur bisa diedit dari Feature Tree. Satuan eksplisit/ekspresi diterima, input salah ditolak dengan alasan | `gui_dimension_bound_to_param_drives_sketch_and_body`, `from_sketch::param_bound_dimension_becomes_expression_and_follows_params`, `units_input` (4 tes) |
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

## 8c. Kemajuan Step 3 (2026-10-21)

| Butir Step 3 | Hasil | Bukti |
|---|---|---|
| 1. Audit picking (sudut kamera, zoom, tablet) | ☐ butuh checklist manual di perangkat | — |
| 2. Gizmo konsisten, angka bisa diketik saat drag | 🟡 sudah ada (popup nilai saat staged, `apply_face_gizmo_typed_value`); commit gizmo extrude kini op oplog | `gizmo_preview_cancel_and_commit_match` |
| 3. Pratinjau terpisah dari model; cancel tidak mengubah model; commit = satu undo | ✅ untuk gizmo extrude | idem |
| 4. Error dinormalisasi ke kategori UI | ✅ `OpErrorCode::category()` (match lengkap, 9 kategori); kartu error menampilkan langkah pemulihan per kategori + hint engine | `error_card_ui::failed_fillet_offers_verified_fix_that_reapplies` |
| 5. Fix ditawarkan hanya setelah lolos dry-run | ✅ B0-2: replay seluruh oplog sebelum fix ditawarkan (engine + GUI) | `verified_fixes_survive_full_replay` |
| 6. Kegagalan kernel jadi error, bukan panic/beku | ✅ panik sudah dipagari (ADR 0009); boolean/fillet/chamfer/hole kini berbatas waktu, maksimal 2 pekerjaan ditinggalkan | `kernel_budget.rs`, `diff_timeout.rs` |

Kriteria selesai Step 3:

| Kriteria | Status |
|---|---|
| Drag preview sama dengan hasil commit; cancel tidak mengubah model | ✅ gizmo extrude (tes otomatis); tarik-sisi (face gizmo) masih jalur lama |
| Pilihan dan gizmo lulus checklist desktop | ☐ checklist manual |
| Error umum tidak menyebabkan crash atau kehilangan pekerjaan | ✅ panik → `internal`, kernel beku → `kernel_timeout`, semua dengan rollback |
| Perbaikan otomatis tervalidasi sebelum ditampilkan | ✅ |

## 8d. Kemajuan Step 4 (2026-10-21)

| Butir Step 4 | Hasil | Bukti |
|---|---|---|
| 1. Matriks format dan batas dukungan | ✅ `docs/FORMAT_MATRIX.md`: buka/buat/round-trip, satuan, yang hilang per format | — |
| 2. Fixture satuan, analitik, multi-body, drawing, malformed | ✅ sintetis (bracket contoh, dua kotak, DXF inci, berkas terpotong) | `format_validation.rs`, `dxf.rs::unit_tests` |
| 3. Round-trip native (param, oplog, datum `frame`, suppressed, material, drawing) | ✅ | `format_validation.rs::native_roundtrip_keeps_design_intent` |
| 4. STEP buka ulang; DXF entitas/layer/satuan; PDF halaman/ukuran/teks | ✅ otomatis; ☐ visual manual | `step_multibody_reimports_with_same_volume`, `drawing_dxf_declares_mm_and_layers`, `drawing_pdf_has_paper_size_and_title` |
| 5. Subset drawing produksi | ✅ didefinisikan di `FORMAT_MATRIX.md` (satu lembar A4/A3, HLR, dimensi otomatis, catatan lubang, title block, mm) | idem |
| 6. Dialog ekspor menyatakan satuan; ringkasan hasil | ✅ label filter dialog simpan berisi satuan; status setelah ekspor: jumlah body, satuan, batasan, body tersembunyi, entitas DXF yang dilewati; impor DXF menyebut satuan asal | `document::tests::step_export_uses_visible_bodies_and_note_mentions_hidden` |

Bug yang ditemukan dan ditutup di Step 4:

| ID | Masalah | Perbaikan |
|---|---|---|
| B4-1 | Ekspor STEP GUI ikut menyertakan body tersembunyi, berbeda dari STL/OBJ/GLB/CLI/MCP | `visible_body_shapes()` |
| B4-2 | DXF tidak menyatakan satuan; impor DXF berkas inci dibaca sebagai mm (25,4× terlalu kecil) | header `$INSUNITS 4` di tiga penulis DXF; impor membaca `$INSUNITS` dan menskalakan ke mm |
| B4-3 | Ekspor DXF sketsa membuang elips/spline/path tanpa pemberitahuan | jumlah yang dilewati disebut di status |

Kriteria selesai Step 4:

| Kriteria | Status |
|---|---|
| Round-trip native lulus untuk fixture dan versi format yang didukung | ✅ |
| Format eksternal tidak diam-diam menghilangkan data yang diklaim | ✅ setelah B4-1..B4-3; batasan tercantum di matriks |
| PDF/DXF/STEP lolos validasi otomatis + pemeriksaan visual | 🟡 otomatis ✅; visual ☐ (Bagian P, T6) |
| Dokumentasi membedakan dapat dibuka/dibuat/round-trip | ✅ `FORMAT_MATRIX.md` |

## 8e. Kemajuan Step 5 (2026-10-21)

| Butir Step 5 | Hasil | Bukti |
|---|---|---|
| 1. Proyek awal yang bisa diedit; tutorial mengajarkan revisi, undo, simpan, ekspor | ✅ bab "Mulai cepat: proyek contoh" (pilihan utama kartu sambutan) membuka bracket yang sama dengan tes CI (`ducad_engine::samples`), lalu: ubah param → ubah nilai op di Pohon Fitur → undo → simpan → ekspor STEP; tiap langkah dideteksi dari perubahan desain nyata | `onboarding_ui::sample_chapter_revises_the_tested_bracket`, `onboarding_render::welcome_card_offers_the_sample_project` |
| 2. Satu tindakan berikutnya; lewati/ulang tutorial tanpa mengubah dokumen kerja | ✅ saran di bawah pil status (kosong → mulai sketsa/proyek contoh; sketsa bebas → dimensi; terkunci → extrude; belum disimpan → simpan), bisa ditutup. Dokumen kerja disisihkan ke `tutorial-stash.ducad` saat bab dimulai dan dibuka kembali (path + status belum disimpan) saat tutorial ditutup atau setelah crash | `next_action::tests`, `onboarding_ui::{tutorial_sets_aside_and_restores_the_working_document, stash_left_by_a_crash_is_restored_on_next_start, empty_document_is_not_stashed}` |
| 3. Dokumentasi publik selaras dengan matriks status | ✅ README (19 kontradiksi: 16 diperbaiki/diselaraskan, 3 diberi label karena celah kode masih ada: riwayat aktivitas dikosongkan saat start, extrude Up to Face, GGUF), PANDUAN (bagian Mulai Cepat, label UI dicocokkan dengan `.ftl`, fitur terbatas diberi label), STATUS_FASE_A_B (diperbarui ke kernel truck) | `STATUS_KEMAMPUAN.md` §2 |
| 4. Workflow contoh jadi regression test di CI yang memblokir; GUI lewat checklist | ✅ `scripts/release-gate.sh` + lane CI `designer`; checklist `CEKLIS_UJI_GUI.md` T0 dan P.5 | `docs/GERBANG_RILIS.md` |
| 5. Metrik kualitas rilis | ✅ tabel per rilis di `GERBANG_RILIS.md` §3; baseline otomatis 94 tes, 0 gagal; metrik sesi pengguna belum diukur | idem |

Bug yang ditemukan dan ditutup di Step 5:

| ID | Masalah | Perbaikan |
|---|---|---|
| B5-1 | Tutorial bekerja di dokumen yang sedang terbuka (bab 2/3 bisa menimpa kanvas; langkah tutorial mengubah dokumen kerja) | dokumen kerja disisihkan dan dipulihkan |
| B5-2 | Ekspor IGES ikut menyertakan body tersembunyi (sama dengan B4-1) dan tidak tercantum di matriks format | hanya body terlihat + ringkasan; `FORMAT_MATRIX.md` diperbaiki |
| B5-3 | Label menu "STEP… (semua body)" salah sejak B4-1 | "(body terlihat)" |
| B5-4 | `cargo test --workspace` merah karena tes vault ISO (vault dihapus di cd57db0f) dan tabel layering tanpa `ducad-update`/`ducad-cloud → ducad-core` | tes vault dilewati bila vault tidak ada; tabel layering diperbarui |

Kriteria selesai Step 5:

| Kriteria | Status |
|---|---|
| Pengguna baru mencapai dan mengubah hasil tutorial tanpa panduan eksternal | 🟡 alur ada dan teruji otomatis; butuh sesi uji T0 |
| README, panduan, UI, status implementasi tidak bertentangan | ✅ (celah kode yang tersisa diberi label) |
| Gate rilis mencakup tes otomatis inti dan checklist GUI | ✅ |

## 9. Berkas yang ditambahkan/diubah oleh Step 0

- `crates/ducad-engine/tests/fixtures/sample_bracket.ops.json` (baru)
- `crates/ducad-engine/tests/sample_project.rs` (baru)
- `docs/BASELINE_DESIGNER_UX.md` (baru, dokumen ini)
- `docs/STATUS_KEMAMPUAN.md` (baru)
- `docs/CEKLIS_UJI_GUI.md` (tambah Bagian P)
- `docs/PERF_BASELINE.md` (tambah baseline alur dokumen)
- `docs/PLAN_DESIGNER_UX.md` (status Step 0)
