# Status Fase A (P0), B (P1), C (P2), D (P4) & E (P3)

Diperbarui: 2026-09-12. Sumber rencana: `.claude/plans/ducad-pro-cad-roadmap.plan.md`.

> **Catatan 2026-10-21 (Step 5 `PLAN_DESIGNER_UX.md`):** dokumen ini disinkronkan
> dengan kode. Matriks kemampuan yang berlaku sekarang adalah
> [`STATUS_KEMAMPUAN.md`](STATUS_KEMAMPUAN.md); dokumen ini hanya catatan sejarah
> roadmap P0–P4. Kernel kini **truck/SolidCraft (Rust murni)**, tanpa kunci
> kernel global (`ducad-kernel/src/backend/truck/mod.rs`). Sejak ADR 0010
> oplog `ducad-engine` adalah sumber kebenaran pemodelan GUI juga, dan
> referensi geometri memakai selector semantik, bukan topological naming
> kernel. Baris yang berubah ditandai *(2026-10-21)*. Path relatif dari
> `crates/`.

Estimasi awal untuk Fase A + B adalah **6–8 bulan kerja penuh satu engineer
senior**. Dokumen ini mencatat apa yang sudah benar-benar mendarat dan
terverifikasi, versus apa yang belum — bukan rencananya.

Gate mutu di tiap baris "selesai": `cargo clippy --workspace --all-targets
-- -D warnings` exit 0 DAN `cargo test --workspace` hijau.

---

## Fase A — P0 Fondasi

| Item | Status | Catatan |
|---|---|---|
| **P0.6** CI & quality gate | ✅ **selesai** | Workflow 4 lane, clippy `-D warnings` memblokir, profil rilis. `fmt`/`deny` sengaja belum memblokir — lihat ADR 0001. |
| **P0.2** Undo lintas domain | 🟡 **sebagian** | `Transaction`, coalescing drag, batas kedalaman, `begin`/`commit`/`rollback` — selesai & teruji di `ducad-core`. *(2026-10-21)* Undo GUI kini membawa oplog (`ducad-app/src/oplog_undo.rs`, `UndoStack::attach_to_top`); urutan undo lintas mode global (`ducad-app/src/document.rs::undo_order_across_modes_is_global`). **Belum**: satu stack tunggal — GUI masih punya beberapa stack + undo oplog headless terpisah (`STATUS_KEMAMPUAN.md`, baris Undo). |
| **P0.1** Model dokumen tunggal | 🟡 **sebagian** | `SketchSet` berkunci `SketchId` + `PlaneRef` menggantikan tiga larik paralel; N sketsa per bidang kini mungkin. *(2026-10-21)* Dokumen desain headless ada: `DesignDoc` (`ducad-engine/src/session.rs`: param, oplog, checks, brief, `DrawingSpec`) dipakai CLI/MCP dan GUI (ADR 0010); assembly sengaja lapisan terpisah (ADR 0011). **Belum**: pemisahan `EditorState` dari `DuCADApp`. |
| **P0.3** Format native v2 | ✅ **selesai** *(2026-10-21; bentuk berbeda dari rencana)* | `.ducad` kini JSON berversi sampai **v4** (bukan container ZIP): oplog, param, datum, assembly (`datums.assembly`), drawing, cache B-rep opsional; migrasi `ducad-engine/src/migrate.rs`; pemulihan sesi `ducad-app/src/recovery.rs`. Bukti: `ducad-engine/tests/format_validation.rs::native_roundtrip_keeps_design_intent`, `ducad-io/tests/native_format.rs`, `docs/FORMAT.md`. Sisa: DAG GUI lama dan anotasi GD&T manual tidak disimpan. |
| **P0.4** Topological naming | 🔄 **diganti** *(2026-10-21)* | Rencana riwayat topologi dari kernel tidak dilanjutkan. ADR 0010 butir 3 & 6: referensi memakai **selector semantik** (`ducad-engine/src/select/`, pembuat selector dari klik `select::suggest_faces/suggest_edges`, `tests/select_suggest.rs`); kegagalan referensi eksplisit (`selector_empty`). Riwayat kernel hanya ditambah bila selector terbukti ambigu. Tidak lagi memblokir P0.5. |
| **P0.5** Regenerasi parametrik sungguhan | 🟢 **selesai untuk fitur yang dialihkan ke oplog** *(2026-10-21)* | Step 1–2 (ADR 0010 §Status penerapan): sketsa, extrude, revolve, fillet/chamfer, hole, boolean, pattern 3D, shell, loft, sweep, draft, emboss, direct edit dari GUI menjadi op oplog dan di-replay deterministik saat dimensi/param/fitur diedit. Bukti: `ducad-app/src/oplog_gui.rs::{gui_sketch_extrude_fillet_then_edit_sketch_updates_downstream, gui_dimension_bound_to_param_drives_sketch_and_body, gui_oplog_survives_save_and_reopen}`, `ducad-engine/tests/sample_project.rs::editing_initial_dimensions_keeps_downstream_features`. **Sisa**: fitur yang belum punya op dari GUI (helix GUI, sebagian kasus loft/sweep/shell/hole di bidang datum atau spec khusus) masih di DAG `parametric_engine` lama dengan keterbatasan semula; DAG itu tidak disimpan ke `.ducad`. |

### Hutang teknis yang diketahui dan sengaja dibiarkan

- `parametric_engine` masih menyimpan `plane_index: usize` di payload fitur.
  Indeks itu bergeser saat datum plane dihapus — persis cacat yang
  `PlaneRef` hilangkan. Konversinya sudah dipusatkan di
  `document.rs::plane_ref_for_index`. *(2026-10-21)* Sesuai ADR 0010, DAG ini
  dipensiunkan per fitur, bukan dipindah ke `PlaneRef`; sketsa di datum GUI
  kini ditulis ke oplog sebagai bidang `frame`.
- `DuCADApp` masih ~96 field.

---

## Fase B — P1 Sketch & Drafting 2D

| Item | Status | Catatan |
|---|---|---|
| **P1.4** Nested region → profil berlubang | ✅ **selesai** | `find_region_hierarchy` (aturan ganjil-genap), `Profile::WithHoles`, `build_face_on_plane`. Plus `KernelShape::volume()` (B-rep eksak) yang dibutuhkan untuk membuktikannya. |
| **P1.2** Solver | 🟡 **sebagian** | `analyze_dof` (rank, DOF, redundansi **per indeks kendala**), dekomposisi gugus union-find, 4 constraint baru (`PointOnCurve`, `Midpoint`, `Concentric`, `Collinear`). *(2026-10-21)* Status sketsa per entitas di kanvas (terkunci hijau, constraint bermasalah merah, "N DOF tersisa"): `ducad-sketch/src/constraint::sketch_status`, tes `constraint/tests.rs::sketch_status_tests`. **Belum**: Jacobian analitik, sparse solver, drag-with-solver, auto-constraint inference (selain freehand → constraint). |
| **P1.7** DXF | 🟡 **sebagian** | Impor LWPOLYLINE (terbuka/tertutup), POLYLINE gaya lama, segmen bulge→busur, ELLIPSE sejajar sumbu. *(2026-10-21)* `$INSUNITS` ditulis (mm) dan dibaca saat impor (dikonversi ke mm), entitas yang dilewati dihitung (`ducad-io/src/dxf.rs` `unit_tests`; `FORMAT_MATRIX.md`). **Belum**: SPLINE, TEXT/MTEXT, INSERT/BLOCK, HATCH, entitas DIMENSION, impor LAYER/LTYPE, DXF biner, ekspor elips/spline, DWG. |
| **P1.1** Entitas 2D baru | 🟡 **sebagian** *(2026-10-21)* | Ada di kode: `Entity::Point`, `Entity::Path` (path vektor, segmen garis/kubik), `Entity::Spline` dengan kurva eksak opsional (`ducad-sketch/src/entity.rs`); teks dan layer/gaya vektor tersimpan sejak format v3. **Belum**: `Ellipse` berotasi (masih sejajar sumbu; impor DXF-nya tetap dilewati, `ducad-io/src/dxf.rs::rotated_ellipse_is_skipped_not_silently_flattened`), NURBS sejati, entitas `Hatch` sketsa. |
| **P1.6** Command line ala AutoCAD | ❌ **belum** | Alias perintah, `@10<45`, Ortho/Polar/Otrack, Move/Copy/Rotate/Scale/Stretch/Array/Join/Explode/Break. |
| **P1.3** Driving dimensions + parameter | ✅ **selesai** *(2026-10-21)* | Parameter dokumen di `DesignDoc.params`; panel **Parameter** di GUI; dimensi sketsa menerima `$param`/ekspresi (`Sketch::dim_exprs` → `Num::Expr` di op `sketch`). Bukti: `oplog_gui::gui_dimension_bound_to_param_drives_sketch_and_body`, `ducad-engine/src/ops/from_sketch.rs::param_bound_dimension_becomes_expression_and_follows_params`, `ducad-engine/tests/replay.rs::set_params_changes_thickness`. |
| **P1.5** Layers, blocks, linetypes | 🟡 **sebagian** *(2026-10-21)* | Layer dan gaya vektor sketsa ada (format v3: `styles`, `layers`, `groups`; panel Layer di drawer). **Belum**: blocks dan linetype ala CAD 2D. |

---

## Fase C — P2 Pemodelan 3D

| Item | Status | Catatan |
|---|---|---|
| **P2.7** Robustness kernel | 🟡 **sebagian** | *(2026-10-21, kernel truck)* Panik kernel dipagari menjadi error `internal` (ADR 0009, log `~/.ducad/logs/panics.jsonl`), boolean/fillet/chamfer/hole berbatas waktu → `kernel_timeout` dengan rollback (`ducad-engine/tests/kernel_budget.rs`, `diff_timeout.rs`), fuzz boolean `ducad-kernel/tests/fuzz_boolean.rs`, kode error + fix terverifikasi (`ducad-engine/tests/diagnose.rs`). **Belum**: pelaporan edge yang menggagalkan fillet di semua kasus; `unwrap()` di `ducad-kernel/src` masih ada. |
| **P2.6** Mass properties | 🟢 **selesai (inti)** *(2026-10-21)* | Volume, luas, massa dari material, pusat massa, inersia: `ducad-engine/tests/inspect_mass.rs::{inspect_mass_bracket_fixture_reports_inertia, inspect_mass_assembly_of_two_materials_matches_formula}`. Sheet metal ada di oplog (`ducad-engine/tests/sheet_metal.rs`); pattern 3D kini op oplog (regen-aware). **Belum**: hole ANSI, cosmetic thread. |
| **P2.1** Mode extrude | 🟡 **sebagian** | `ExtrudeExtent::{Blind, Symmetric, TwoSided}` — semuanya diselesaikan sebagai SATU prisma dengan menggeser titik awal, tanpa boolean. *(2026-10-21, masih akurat)* **Belum**: `UpToNext`/`UpToBody`/`UpToFace` (`ducad-kernel/src/csg.rs`), draft, thin, opsi revolve; GUI hanya Blind (Symmetric/TwoSided lewat oplog). |
| **P2.2** Sketch on face asosiatif | 🟢 **selesai (oplog)** *(2026-10-21)* | Bidang sketsa `PlaneSpec::OnFace` dengan selector semantik (`ducad-engine/src/ops/spec.rs`), di-replay setelah edit; GUI menulis face yang diklik lewat `suggest_faces` (ADR 0010). Gagal eksplisit (`selector_empty`) bila face hilang. |
| **P2.4** Surface modeling | 🟡 **sebagian** *(2026-10-21)* | Thicken, extend, stitch di oplog: `ducad-engine/tests/surfaces.rs::{thicken_makes_solid_and_extend_grows, stitch_consumes_inputs, hostile_surface_inputs_rejected}`. Replace/delete face lewat direct modeling (P2.5). **Belum** dirinci: trim surface; tidak ada jalur GUI khusus surface yang teruji. |
| **P2.5** Direct modeling | 🟡 **sebagian** *(2026-10-21)* | Offset/delete/replace face, split face by sketch: `ducad-engine/tests/direct_edit.rs::{offset_and_delete_face, replace_face_with_face_of_other_body, split_face_by_sketch_then_offset_piece}`; dari GUI lewat `direct_via_oplog` (`ducad-app/src/direct_tools.rs`). **Belum** ke oplog: align, pipe, web, path pattern, boundary fill, bounding solid (ADR 0010 urutan 9). |
| **P2.8** Kernel paralel & tessellation inkremental | 🟡 **sebagian** *(2026-10-21)* | Tidak ada kunci kernel global: `KernelShape` `Send + Sync` (`ducad-kernel/src/backend/truck/mod.rs`). **Belum** diverifikasi: paralelisme operasi kernel dan tessellation inkremental. |

---

## Fase D — P4 Gambar Kerja Teknik

| Item | Status | Catatan |
|---|---|---|
| **P4.1** HLR eksak | 🟡 **sebagian** | `hlr_exact.rs` — *(2026-10-21)* kini di atas backend truck (`ducad_kernel::backend::hlr_exact`). Lingkaran tetap `Circle` (`ducad-kernel/src/tests.rs`, uji `extract_exact_hlr`). Gambar kerja multi-view + HLR stabil (`ducad-engine/tests/drawing_auto.rs`, `drawing_golden.rs`). **Belum**: auxiliary view, proyeksi 6 tampak. |
| **P4.2** Model sheet & view | 🟡 **sebagian** *(2026-10-21)* | `DrawingSpec` tersimpan di dokumen (ADR 0007, `ducad-io/src/drawing/spec.rs`); section + hatch (`ducad-kernel/src/tests.rs::test_section_view_brep_slice_and_hatch_generation`), detail view (`ducad-io/src/drawing.rs::test_detail_view_creation_and_layout`); multi-sheet lewat paket desain (`ducad-io/src/drawing/pages.rs`, `ducad-engine/tests/package.rs`). **Belum**: multi-sheet di editor GUI (satu lembar), auxiliary/broken/crop view. |
| **P4.3** Anotasi standar | 🟡 **sebagian** *(2026-10-21)* | Dimensi asosiatif di editor (`ducad-io/tests/sheet_p21.rs::editor_dimensions_are_associative_and_draggable`); GD&T (feature control frame, datum, toleransi/fit ISO 286, surface finish) di `ducad-io/src/drawing/gdt.rs`, tes `ducad-io/tests/gdt_pdf.rs`. **Belum**: simbol las; anotasi GD&T manual GUI belum disimpan ke `.ducad`. |
| **P4.4** Ekspor gambar | 🟡 **sebagian** *(2026-10-21)* | PDF satu lembar A4/A3 dan DXF R12 (`$INSUNITS 4`, layer standar, LINE/CIRCLE/ARC/TEXT) teruji: `ducad-engine/tests/format_validation.rs::{drawing_pdf_has_paper_size_and_title, drawing_dxf_declares_mm_and_layers}`; PDF multi-halaman lewat `build_package`. **Belum**: entitas DIMENSION asli di DXF, PDF/A dengan font tertanam. |

---

## Fase E — P3 Perakitan (Assembly)

| Item | Status | Catatan |
|---|---|---|
| **P3.2** Mate solver | ✅ **selesai** (inti) | Solver simultan, joints, limit mates, **drag dengan solver** (part hanya bergerak sepanjang DOF bebasnya; tersambung ke drag body di viewport dan menyinkronkan instance ↔ B-rep), **exploded view** (display-only, tidak pernah dilihat solver; slider + urai otomatis radial), **studi gerak** (mate numerik digerakkan `from`→`to`, playhead + putar bolak-balik). Sisa: DOF per instance di UI. |
| **P3.1** Model referensi part | 🟡 **sebagian** | Kerangka transform hierarkis, `PartSource::External` lengkap (resolusi path berlapis, UUID stabil, deteksi perubahan dengan penjagaan *racily clean*), DAN tersambung ke UI: seksi "Part eksternal" di drawer perakitan — sisipkan, periksa perubahan (lencana), muat ulang pada posisi terakit, lepas tautan. Instancing GPU **selesai**: jalur instanced (buffer per-mesh di-cache per fingerprint, buffer transform per-instance, satu `draw_indexed(.., first..first+n)` per mesh unik) berdampingan dengan jalur merged untuk body berwarna per-vertex. Mesh identik berbagi satu buffer GPU. |
| **P3.3** Interference & clearance | 🟡 **sebagian** | Mid-phase BVH segitiga + SAT (dengan penjagaan containment — bodi yang tertelan seluruhnya tidak punya irisan segitiga tapi TETAP interferensi). `check_clearance` menjawab "apakah ada celah minimal N mm" (`ducad-kernel/src/interference.rs`; *(2026-10-21)* keeksakan pada permukaan lengkung di backend truck belum diverifikasi). **Belum**: collision saat drag, paralelisasi. |
| **P3.4** BOM & properti | 🟡 **sebagian** | `part_number`/`material` per instance, `build_bom` dengan rollup kuantitas (digabung per nomor part, bukan per nama), mode rata & indented per sub-assembly tingkat pertama, ekspor CSV ber-escape RFC 4180. *(2026-10-21)* Balon penunjuk tertaut ke baris BOM di gambar: `ducad-io/tests/exploded_drawing.rs::exploded_drawing_has_one_balloon_per_bom_row_and_is_deterministic`, `ducad-io/src/pdf.rs::test_pdf_export_with_bom_and_balloons`. **Belum**: XLSX. |

### Bug laten yang ditemukan saat menyambungkan drag

`translate_shape`/`transform_shape` dulu MENGATUR posisi shape secara absolut — padahal dokumentasinya mengaku
"delta". Gizmo drag menyiasatinya dengan menghitung dari shape asli, tapi
`apply_mate_transform_to_shape` tidak: setiap solve perakitan sesudah yang
pertama menempatkan part di posisi yang salah, dan drag body dua kali
mereset drag pertama. Keduanya kini delta sejati (`translated()`), dengan test regresi kumulatif.

### Instancing GPU

Selesai. Body polos (kasus umum) lewat jalur instanced; body dengan warna
per-vertex (terpilih, highlight face, preview extrude/fillet, target potong)
tetap lewat jalur merged sehingga tidak ada fitur yang hilang. Test mengunci
invarian "tiap body tergambar tepat satu kali" dan "mesh identik berbagi
satu key". Yang belum diverifikasi otomatis: hasil piksel — perlu dilihat di
aplikasi berjalan.

---

## Tutorial selamat datang (onboarding)

Selesai 2026-10-06. Satu proyek berkelanjutan, rem cakram Ø240, dalam tiga
bab (Pemula: tur layar + piringan; Menengah: lubang baut + ventilasi; Mahir:
hub, verifikasi, ekspor, agent AI). Tiap langkah lulus hanya bila geometri
atau keadaan berubah sesuai tujuannya (`OnboardingGoal`: body bertambah,
volume berkurang, lingkaran/busur bertambah, nama command model, dsb.). Bab 2
dan 3 membangun geometri awalnya lewat oplog engine bila kanvas kosong. Widget
`ducad-ui/src/onboarding.rs`, adapter `ducad-app/src/onboarding_ui.rs`,
penanda first-run `~/.ducad/onboarding.json`. Dijaga tes
`ducad-ui/tests/onboarding_render.rs` dan tes unit `onboarding_ui`. Yang belum
diverifikasi otomatis: tata letak kartu pelajaran di aplikasi berjalan dan di
iPad (ceklis A14).

*(2026-10-21, Step 5)* Bab tambahan **Mulai cepat: proyek contoh**: membuka
bracket L contoh (`ducad-engine/src/samples.rs`), lalu ubah parameter, ubah
satu fitur di Riwayat Parametrik, undo, simpan, ekspor STEP. Dokumen yang
terbuka disisihkan dan dibuka kembali saat tutorial ditutup. Saran tindakan
berikutnya satu baris di bawah pil status (`ducad-app/src/next_action.rs`,
`ducad-ui/src/canvas_hud.rs::show_next_action`). Panduan pengguna:
`PANDUAN.md` §Mulai Cepat.

---

## Urutan yang disarankan berikutnya

*(2026-10-21)* Urutan lama (P0.4 riwayat topologi kernel → P0.1 →
P1.1) tidak berlaku lagi: P0.4 diganti selector semantik dan P0.5/P1.3/P2.2
sudah mendarat lewat oplog (ADR 0010). Prioritas kini mengikuti
`PLAN_DESIGNER_UX.md` dan sisa di `STATUS_KEMAMPUAN.md`, mis.:

1. Pindahkan sisa fitur GUI yang belum punya op (helix GUI, kasus khusus
   loft/sweep/shell/hole) lalu pensiunkan DAG `parametric_engine`.
2. **P2.1** `UpToFace`/`UpToNext`, dan Symmetric/TwoSided di GUI.
3. **P1.1** ellips berotasi (membuka sisa impor DXF).

## Tindakan yang perlu pemilik repo

- **Keputusan reformat rustfmt massal** (~1.628 selisih). Prosedurnya ada
  di `.github/workflows/ci.yml`.
