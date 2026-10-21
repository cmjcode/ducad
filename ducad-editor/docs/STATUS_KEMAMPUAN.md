# Matriks Status Kemampuan DUCAD

Hasil Step 0 `docs/PLAN_DESIGNER_UX.md` (2026-10-21, commit basis `53ecb880`).
Setiap klaim fitur di `README.md` root repo dipetakan ke status yang bisa
ditelusuri ke kode atau tes. Dokumen ini menjadi acuan saat README, panduan, dan
UI disinkronkan (Step 5). Perbarui baris saat pekerjaan mendarat.

**Definisi status**

- `stabil` — terimplementasi dan ada tes otomatis yang menguji perilaku yang
  terlihat pengguna.
- `terbatas` — bekerja dengan batas atau celah yang diketahui.
- `eksperimental` — kode ada tetapi tipis/tanpa tes, tidak dibangun CI, atau
  belum tersambung ke GUI.
- `belum tersedia` — diklaim tetapi tidak ditemukan atau tidak terjangkau.

**Jalur**: `GUI` = alur manual (`ducad-app/src/modeling/operations.rs` + DAG
`parametric_engine.rs`), `oplog` = `ducad-engine` (CLI/MCP/agent dan panel
Industrial/Sim), `keduanya` = ada di dua jalur dengan kode berbeda. Sesuai
keputusan Step 0, jalur oplog menjadi sumber kebenaran; baris berlabel `GUI`
saja adalah daftar kerja migrasi Step 1a.

**Catatan metode**: audit membaca kode dan tes; "stabil" berarti tes yang
menguji perilaku itu ada, bukan bukti tes lulus hari ini (gate-nya CI
`cargo test --workspace`). Klaim berat (riwayat dikosongkan saat start, DAG
direset, radial menu tanpa pemanggil, `test_api.sh`, preset lighting) dicek
ulang langsung. Path relatif dari `crates/` kecuali disebut lain.

## 1. Matriks

| Kemampuan | Jalur | Status | Bukti | Catatan/batas |
|---|---|---|---|---|
| **Sketsa** | | | | |
| Line/rect/circle/arc/arc 3 titik/polygon/slot | keduanya | stabil | `ducad-sketch/src/tests.rs::{arc_from_three_points_passes_through_all_three, test_regular_polygon_inscribed_and_circumscribed, test_slot_center_to_center_and_overall}`; `ducad-engine/src/ops/spec.rs` `EntitySpec` | — |
| Ellipse | keduanya | terbatas | `ducad-sketch/src/entity.rs`; `ducad-io/src/dxf.rs::rotated_ellipse_is_skipped_not_silently_flattened` | Hanya sejajar sumbu; dilewati saat ekspor DXF |
| Construction line | keduanya | stabil | `ducad-sketch/src/tests.rs::test_construction_entities_and_closed_regions` | Render visual tidak diuji |
| Teks sketsa TTF/OTF | GUI | stabil (GUI) | `ducad-sketch/src/tests.rs::{test_text_vectorization, text_regenerate_is_deterministic}`; `oplog_gui.rs::gui_text_extrude_round_trips_through_oplog` | Teks masuk op `sketch` sebagai kurva akhir (`Spline.exact`), bukan entitas teks yang bisa diedit di oplog (diperbarui Step 5) |
| Snapping berprioritas | GUI | stabil | `ducad-sketch/src/tests.rs::{snap_prefers_endpoint_over_grid, snap_finds_line_intersection}` | Glyph tidak diuji |
| Constraint (12 jenis) | keduanya | stabil | `ducad-sketch/src/constraint/tests.rs`; `ducad-engine/tests/sample_project.rs::builds_fully_constrained_bracket` | Belum ada auto-inference atau pewarnaan DOF di GUI |
| Trim/Extend/Offset/Mirror | GUI | stabil | `ducad-sketch/src/tests.rs::{trim_removes_middle_segment_between_two_cuts, test_biarc_and_multi_arc_offset}` | Tidak ada op edit sketsa; hasil akhirnya masuk op `sketch` saat keluar mode sketsa (ADR 0010 urutan 8; diperbarui Step 5) |
| Freehand → constraint | GUI | stabil | `ducad-app/src/freehand.rs::stroke_becomes_lines_with_constraints_in_one_undo` | Hasil akhir masuk op `sketch` lewat ikatan slot (ADR 0010 urutan 8; diperbarui Step 5) |
| **Datum** | | | | |
| Plane offset | keduanya | terbatas | `ducad-engine/tests/datum.rs::construction_planes_place_sketches` | Plane GUI berupa frame absolut berindeks, tidak asosiatif |
| Plane bersudut / 3 titik | GUI | terbatas | `ducad-app/src/document.rs`; `ducad-render/src/plane.rs::{test_angled_plane, test_three_point_plane}`; `oplog_gui.rs::gui_extrude_on_angled_and_three_point_datums_is_an_op` | Sketsa di datum GUI ditulis sebagai `PlaneSpec::Frame` di oplog (Step 1), tetapi bidangnya frame tetap, tidak asosiatif ke acuan; normal acuan hardcode `Vec3::Z` (diperbarui Step 5) |
| **3D** | | | | |
| Extrude blind | keduanya | stabil | `ducad-engine/tests/session.rs::plate_ops_build_expected_volume`; `sample_project.rs` | — |
| Extrude symmetric | oplog | terbatas | `ducad-kernel/src/tests.rs::symmetric_extrude_straddles_the_sketch_plane` | GUI selalu Blind (`operations.rs:108-113`) |
| Extrude up to face | – | **belum tersedia** | `ducad-kernel/src/csg.rs` `ExtrudeExtent` hanya Blind/Symmetric/TwoSided | README kini menyebutnya belum tersedia (Step 5) |
| Revolve sumbu kustom | keduanya | terbatas | fixture `flange.ops.json`, `datum_axis.ops.json` | GUI: sumbu 2D di bidang sketsa |
| Loft / Sweep / Draft / Emboss | keduanya | terbatas | `ducad-engine/tests/advanced_ops.rs`; `oplog_gui.rs::{gui_loft_records_ops_on_the_real_plane, gui_sweep_along_line_records_ops, gui_draft_records_op_with_bottom_neutral_plane, gui_emboss_and_deboss_are_ops}` | GUI kini menulis op (Step 1). Jalur lama tersisa: sweep berbusur/di datum, loft dari datum tertunda, emboss tanpa body sasaran (diperbarui Step 5) |
| Helix | keduanya | terbatas | `advanced_ops.rs::helix_spring_volume` | Helix GUI belum dimigrasi ke op; regenerasi GUI `Unsupported` |
| Boolean union/cut | keduanya | stabil (oplog) | fixture `bracket`, `flange`, `valve_body`; `ducad-kernel/tests/fuzz_boolean.rs`; `oplog_gui.rs::gui_subtract_respects_selection_order_and_is_an_op` | GUI kini op `boolean` (Step 1; diperbarui Step 5) |
| Boolean intersect | keduanya | terbatas | `ducad-kernel/src/tests.rs::intersect_overlapping_boxes_smaller_than_union` | Tanpa tes tingkat op |
| Fillet konstan / Chamfer | keduanya | stabil (oplog) / terbatas (GUI) | 6 fixture; `sample_project.rs::editing_initial_dimensions_keeps_downstream_features`; `oplog_gui.rs::gui_fillet_becomes_oplog_op_with_semantic_selector` | GUI kini op dengan selector semantik; B0-5 ditutup Step 1. Jalur gizmo-drag `modeling/rounding.rs` masih lama (diperbarui Step 5) |
| Fillet radius variabel | keduanya | terbatas | `advanced_ops.rs::variable_fillet_and_replay` | Hanya tepi lurus; regenerasi GUI abaikan `radius_end` |
| Shell | keduanya | stabil | fixture `enclosure`, `plastic_case` | Tebal variabel hanya body konveks |
| Hole simple/cbore/csink | keduanya | stabil (oplog) / terbatas (GUI) | `ducad-kernel/src/tests.rs::test_hole_wizard_counterbore_iso4762_m6`; `sample_project.rs`; `oplog_gui.rs::hole_spec_json_maps_iso_and_custom_and_rejects_custom_counterbore` | GUI kini op `hole` (Step 1); counterbore custom dan lubang lama tetap jalur lama; op lambat (B0-7) (diperbarui Step 5) |
| Hole tapped | keduanya | terbatas | `ducad-kernel/src/tests.rs::test_hole_wizard_tapped_m8` | Hanya bor tap, ulir tidak dimodelkan (kecuali op `thread`) |
| Pattern 3D | keduanya | stabil (oplog) / terbatas (GUI) | fixture `flange`; `adapter_tests.rs::adapter_apply_pattern_3d_volume`; `oplog_gui.rs::gui_linear_pattern_becomes_op_and_replays` | GUI kini op `pattern` (Step 1; diperbarui Step 5) |
| Direct modeling | keduanya | terbatas | `ducad-engine/tests/direct_edit.rs`; `oplog_gui.rs::gui_direct_offset_and_replace_face_are_ops` | Offset/delete/replace face GUI kini op; align, pipe, web, path pattern, boundary fill, bounding solid masih jalur lama (diperbarui Step 5) |
| **Riwayat dan undo** | | | | |
| Regenerasi parametrik (oplog) | oplog | stabil | `ducad-engine/tests/replay.rs::{set_params_changes_thickness, replay_is_deterministic}`; `sample_project.rs` | Satu-satunya jalur dengan referensi semantik |
| Timeline DAG + regenerasi (GUI) | GUI | terbatas | `parametric_engine.rs::test_parametric_dag_recording_and_regeneration`; `oplog_gui.rs::{feature_tree_edits_value_suppresses_and_refuses_unsafe_delete, gui_oplog_survives_save_and_reopen, gui_sketch_extrude_fillet_then_edit_sketch_updates_downstream}` | Fitur yang sudah dimigrasi kini tercatat di oplog (disimpan, diregenerasi, seksi "Riwayat parametrik" di Feature Tree); DAG hanya menampung fitur yang belum dimigrasi (helix, dll.) dan tetap tidak disimpan. Roll-to marker belum ada (diperbarui Step 5) |
| Riwayat aktivitas + cabang | GUI | terbatas | `ducad-app/src/history_db.rs` | Dikosongkan setiap aplikasi start (`app.rs:569`), jadi cabang hilang |
| Undo | keduanya (terpisah) | terbatas | `ducad-app/src/document.rs::undo_order_across_modes_is_global`; `ducad-engine/tests/session.rs::undo_redo_reproduces_volume`; `sample_project.rs::param_edit_is_one_undo_step`; `ducad-app/src/oplog_undo.rs` | B0-1 ditutup Step 1: edit param/op = satu langkah undo, undo GUI membawa oplog. Masih tiga stack GUI (diperbarui Step 5) |
| **Berkas** | | | | |
| `.ducad` native | keduanya | terbatas | `ducad-io/tests/native_format.rs`; `ducad-engine/tests/session_file.rs`; `sample_project.rs::save_and_reopen_roundtrip` | Format kini v4; oplog GUI ikut disimpan (`gui_oplog_survives_save_and_reopen`); anotasi GD&T GUI dan DAG fitur yang belum dimigrasi tidak disimpan (diperbarui Step 5) |
| Impor STEP | keduanya | terbatas | `ducad-kernel/tests/step_roundtrip.rs` | Hanya round-trip berkas sendiri; tanpa fixture CAD lain |
| Ekspor STEP | keduanya | stabil | `step_roundtrip.rs`; `sample_project.rs::export_step_reopens_and_stl_is_written`; `format_validation.rs::step_multibody_reimports_with_same_volume` | Dibatasi tier lisensi lewat `ducad-mcp` mandiri (`tooling::check_tool_license`, sudah di-commit); ekspor GUI dan `ducad-cli build` tidak dibatasi (diperbarui Step 5) |
| Impor / ekspor DXF | GUI (+ flat oplog) | terbatas | `ducad-io/src/dxf.rs` (17 tes); `ducad-io/src/flat_dxf.rs`; `format_validation.rs::drawing_dxf_declares_mm_and_layers` | Tanpa SPLINE/TEXT/INSERT/HATCH; R12; tanpa entitas DIMENSION. Satuan `$INSUNITS` ditulis dan dibaca (Step 4). Detail: `FORMAT_MATRIX.md` |
| Impor STL, ekspor STL/OBJ | GUI + CLI | stabil | `ducad-io/src/mesh_export.rs`; `ducad-io/tests/hostile_stl.rs` | — |
| Ekspor GLB / SVG | GUI | stabil | `ducad-io/src/glb.rs`; `ducad-io/src/svg.rs` | AR Quick Look tidak terverifikasi |
| Ekspor PDF | keduanya | stabil | `ducad-io/tests/sheet_p21.rs`; `sample_project.rs::drawing_exports_pdf_and_dxf` | Dibatasi lisensi lewat `ducad-mcp` mandiri (sudah di-commit) (diperbarui Step 5) |
| **Gambar kerja** | | | | |
| Multi-view + HLR | keduanya | stabil | `ducad-engine/tests/drawing_auto.rs`; `drawing_golden.rs` | — |
| Section + hatch | keduanya | stabil | `ducad-kernel/src/tests.rs::test_section_view_brep_slice_and_hatch_generation` | — |
| Detail view | keduanya | terbatas | `ducad-io/src/drawing.rs::test_detail_view_creation_and_layout` | Interaksi GUI tidak diuji |
| Dimensi otomatis / manual | keduanya | stabil (otomatis) | `sheet_p21.rs::editor_dimensions_are_associative_and_draggable` | Alat manual GUI tanpa tes end-to-end |
| BOM, balon, title block | keduanya | stabil | `ducad-io/tests/exploded_drawing.rs`; `ducad-engine/tests/package.rs` | — |
| Multi-sheet | oplog (paket) | terbatas | `ducad-io/src/drawing/pages.rs` | Editor GUI hanya satu sheet |
| **Assembly** | | | | |
| Pohon, instance, mate concentric/coincident/distance, clash | GUI | stabil | `ducad-core/src/assembly.rs`; `ducad-kernel/src/assembly_solver.rs`; `ducad-kernel/src/interference.rs` | Tidak ada op assembly di oplog — sengaja: lapisan dokumen terpisah (ADR 0011, diusulkan); instance bertahan saat replay (`assembly_instances_survive_param_replay`) (diperbarui Step 5) |
| Mate angle | GUI | terbatas | `assembly_solver.rs` | Tanpa tes khusus |
| Coupling gear/screw/rack, explode | GUI | terbatas | `ducad-core/src/coupling.rs` | Bukan op oplog |
| **Agent / headless** | | | | |
| Oplog atomik + selector | oplog | stabil | `ducad-engine/tests/session.rs::failing_batch_rolls_back_everything`; `tests/select.rs` | Filter selector tanpa parameter (B0-3) |
| `ducad-cli` | oplog | stabil | `ducad-cli/tests/cli.rs` | `release`/`golden`/`migrate-file` kini disebut di README (Step 5) |
| `ducad-mcp` (37 tool, 43 dengan `--attach`) | oplog | stabil | `ducad-mcp/src/tools.rs`; `ducad-mcp/src/server.rs` tes | README diselaraskan Step 5 (37 + 6 tool live) |
| Design checks, diff, build CI | oplog | stabil | `ducad-engine/tests/{checks,diff}.rs`; `cli.rs::build_*` | — |
| Kode error + fix terverifikasi | keduanya | terbatas | `ducad-engine/tests/diagnose.rs`; `sample_project.rs::verified_fixes_survive_full_replay` (ignored) | B0-2 ditutup Step 3: fix di-replay penuh sebelum ditawarkan (`verified_fixes_survive_full_replay`, tidak lagi ignored); hanya untuk op oplog (diperbarui Step 5) |
| Live bridge + proposal/ghost | keduanya | stabil (logika) | `ducad-app/src/proposal_ui.rs`; `ducad-mcp/src/attach.rs` | Unix saja; render ghost tidak diuji |
| `ducad-assist` loop proposal | oplog | stabil (backend tiruan) | `ducad-assist/tests/assist.rs` | — |
| Ask AI apple-fm / GGUF, MNEMONIC, eval | GUI / n.a. | eksperimental | `ducad-assist/src/{apple,gguf}.rs`; `ducad-app/src/memory.rs`; `evals/` | Tidak dibangun CI; GGUF tidak tersambung GUI; eval 11 tugas (README diselaraskan Step 5) |
| **Simulasi dan industri** | | | | |
| Mass properties, FEA statik, konfigurasi, ISO 286, standard part | keduanya | stabil | `ducad-engine/tests/{inspect_mass,configurations,standard_parts}.rs`; `ducad-sim/tests/static_pipeline.rs` | FEA estimasi ±10 % |
| Frequency/buckling/thermal | keduanya | terbatas | `ducad-sim/tests/p18_gates.rs`; `ducad-engine/tests/study_kinds.rs` | Panel GUI hanya statik |
| Sheet metal + flat DXF | keduanya | stabil (oplog) / terbatas (GUI) | `ducad-engine/tests/sheet_metal.rs` | Base GUI hanya persegi di XY; batas di README diselaraskan Step 5 |
| GD&T di sheet | MCP + GUI | terbatas | `ducad-io/tests/gdt_pdf.rs` | Anotasi GUI hanya per sesi |
| **UI** | | | | |
| Command palette, ViewCube | GUI | stabil | `ducad-ui/src/command_palette.rs`; `ducad-ui/src/viewcube.rs` | — |
| Radial menu (`Space`) | GUI | **belum tersedia** | `ducad-ui/src/radial_menu.rs` `open_at` tanpa pemanggil | Kode mati; klaim dihapus dari README (Step 5) |
| Lighting preset + SSAO | GUI | terbatas | `ducad-render/src/scene.rs` | Bukan SSAO screen-space (aproksimasi cavity per piksel); README diselaraskan Step 5 |
| i18n en-US/id-ID | GUI | terbatas | `ducad-i18n/src/lib.rs::test_key_parity` | Banyak string Indonesia hardcode |
| Mode sentuh iPad | GUI | terbatas | `ducad-ui/src/touch.rs` | Toleransi 24/12 px (README diselaraskan Step 5); label mode memakai emoji |
| **Platform dan cloud** | | | | |
| Linux | n.a. | stabil | `.github/workflows/ci.yml` lane ubuntu | — |
| macOS (platform utama persona) | GUI | terbatas | `ducad-app/tests/ios_scene_manifest.rs` | **Tidak ada lane workspace macOS di CI**; diuji manual |
| Windows + MSI | n.a. | terbatas | `ci.yml`, `release.yml` | Tes/clippy Windows `continue-on-error`; tanpa live bridge |
| Android | n.a. | eksperimental | `ducad-android/src/lib.rs` | Tidak di CI |
| Akun/SSO | GUI | stabil (auth) | `ducad-cloud/src/auth.rs` | — |
| Cloud sync | GUI | eksperimental | `ducad-cloud/src/sync.rs` | Tes dengan server lokal berskrip (respons tiruan), bukan server nyata |
| `./test_api.sh` | n.a. | **belum tersedia** | berkas tidak ada | Rujukan dihapus dari README (Step 5) |

## 2. Kontradiksi README vs implementasi

Ditemukan di Step 0; status per Step 5 (2026-10-21). ✅ = selesai (cara
penyelesaiannya disebut), 🟡 = README sudah diberi label tetapi celah di kode
masih ada, ☐ = belum selesai.

1. ✅ **"Both paths run the same modeling code… a part built in the GUI replays
   headlessly"** — *diperbaiki di kode oleh Step 1* (ADR 0010): sketsa,
   extrude, revolve, loft, sweep, fillet/chamfer, hole, boolean, pattern,
   shell, draft, emboss, datum `frame`, teks/freehand/edit sketsa (lewat op
   `sketch`), dan offset/delete/replace face kini menulis op
   (`oplog_gui.rs`, 24 tes lulus 2026-10-21). *README diperbaiki*: klaim
   diberi pengecualian — alat yang belum dimigrasi (helix, shell variabel,
   counterbore custom, sebagian direct modeling) dan assembly (ADR 0011).
2. ✅ **"Modifying past feature parameters with automatic regeneration"** —
   *diperbaiki di kode oleh Step 1–2*: seksi "Riwayat parametrik" Feature Tree
   (edit nilai, suppress, hapus), dimensi terikat `$param`
   (`feature_tree_edits_value_suppresses_and_refuses_unsafe_delete`,
   `gui_dimension_bound_to_param_drives_sketch_and_body`,
   `gui_sketch_extrude_fillet_then_edit_sketch_updates_downstream`). *README
   diperbaiki*: bagian 6 kini menyebut oplog (bukan DAG), batas fitur yang
   belum dimigrasi, dan roll-to marker yang belum ada.
3. 🟡 **"Jump back to any recorded step"** — kode tidak berubah: riwayat
   aktivitas masih dikosongkan setiap start (`app.rs:580`
   `history_db.clear()`). *README diberi label*: "recorded in the current
   session … cleared each time the application starts". Celah produk tetap
   terbuka.
4. 🟡 **Extrude "Up to Face" / Symmetric** — kode tidak berubah: tidak ada
   `UpTo*` di `ExtrudeExtent`; GUI hanya Blind maju/mundur
   (`extrude_via_oplog` hanya menulis `direction: reverse`). *README
   diperbaiki*: Symmetric "through the oplog only", Up to Face "not available
   yet".
5. ✅ **Revolve "custom 3D axis"** — *README diperbaiki*: "axis lying in the
   sketch plane" (sketsa di datum untuk posisi 3D; datum axis lewat oplog),
   sesuai `RevolveSpec.axis` dan `revolve_via_oplog`.
6. ✅ **Tapped hole "threaded"** — *README diperbaiki*: "tap-drill hole …
   thread is not modeled; add a `thread` op" (`ducad-kernel/src/hole.rs`
   `HoleKind::Tapped` hanya profil bor + chamfer masuk).
7. ✅ **Radial menu `Space`** — kode tetap mati (`RadialMenu::open_at` tanpa
   pemanggil, `radial_press` tidak pernah diisi). *README diperbaiki*: klaim
   di bagian 8 dan baris pintasan `Space` dihapus.
8. ✅ **Preset lighting / SSAO** — *README diperbaiki*: nama preset sesuai
   `StudioPreset` (Clean Studio, Warm Showcase, Cool Tech, Dramatic Dark) dan
   AO disebut aproksimasi cavity per piksel, bukan SSAO screen-space
   (`shader.wgsl` bagian 6).
9. ✅ **Toleransi sentuh 14 px** — *README diperbaiki*: 24 px jari / 12 px
   Pencil (`ducad-ui/src/touch.rs:107-108`).
10. 🟡 **Ask AI GGUF** — kode tidak berubah (`assist_ui.rs::on_device_backend`
    hanya `apple-fm`). *README diberi label*: `ducad-assist` "experimental",
    Ask AI hanya apple-fm, `local-gguf` "the app does not use it yet".
11. ✅ **Batas sheet metal basi** — *README diperbaiki*: flange-on-flange,
    `relief`, base berlubang/berbusur, `perforate`, `convert_to_sheet`
    (`sheet_metal.rs::{sheet_metal_flange_on_flange_extends_chain,
    sheet_metal_relief_is_cut_in_folded_and_flat,
    sheet_metal_base_with_holes_and_arcs}`).
12. ✅ **Jumlah tool MCP** — *README diperbaiki*: 37 tool + 6 tool live
    dengan `--attach` (`tools.rs` `TOOL_NAMES`, `LIVE_TOOL_NAMES`); baris
    riwayat P2–P3 diberi catatan "20 tools at the time; 37 today".
13. ✅ **Eval "ten tasks"** — *README diperbaiki*: eleven (11 berkas di
    `evals/tasks/`); baris riwayat P6 diberi catatan.
14. ✅ **`./test_api.sh`** — *README diperbaiki*: bagian dihapus, diganti
    catatan bahwa cloud sync eksperimental dan diuji dengan server berskrip.
15. ✅ **CI** — *README diperbaiki*: tanpa lane workspace macOS (hanya
    `ducad-update` di `macos-14`), DMG macOS dibangun lokal, tes/clippy semua
    target Windows tidak memblokir, `release.yml` membangun dan menguji MSI.
    Gerbang rilis ditautkan ke `GERBANG_RILIS.md` (ditulis paralel; lane CI
    designer workflow dikerjakan agen lain).
16. ✅ **Lisensi** — kini sudah di-commit (`ducad-engine/src/tooling.rs`
    `check_tool_license`, `ducad-mcp/src/tools.rs:654`). *README diberi
    label*: tier dibaca dari sesi login aplikasi; tier Free menolak `export`
    STEP/IGES/3MF/DXF, `drawing` PDF/DXF, `build_package`,
    `submit_for_review` dengan `license_required`. Catatan: GUI tidak
    menegakkan tier (`require_feature` hanya dipanggil tes) — perlu keputusan
    produk, bukan urusan README.
17. ✅ **`.ducad` "since format v2"** — *README diperbaiki*: "format v4",
    isi `design` (param, oplog, checks, suppressed, material, drawing), anotasi
    GD&T GUI belum disimpan. Oplog GUI kini ikut tersimpan (Step 1–2,
    `gui_oplog_survives_save_and_reopen`).
18. ✅ **`docs/STATUS_FASE_A_B.md` basi** — *diperbarui Step 5*: catatan
    bertanggal di atas (kernel truck/SolidCraft, matriks ini acuan terkini),
    P0.5 selesai untuk fitur yang lewat oplog, P1.1/P2.4/P2.5/P3.4/P4.x
    diberi status dan bukti; P2.1 (UpTo*) tetap terbuka. README menautkan
    matriks ini sebagai acuan yang menang bila berbeda.
19. ✅ **Klaim kurang** — *README diperbaiki*: impor IGES (eksperimental),
    3MF impor/ekspor, impor glTF/SVG/PDF, op `perforate`/`convert_to_sheet`,
    CLI `release`/`golden`/`migrate-file`; README menautkan
    `FORMAT_MATRIX.md`.

Temuan tambahan Step 5 yang juga diperbaiki di README: impor DXF kini
menyebut entitas yang didukung dan `$INSUNITS`; ekspor STEP hanya body
terlihat; GLB metre/Y-up dan AR Quick Look belum terverifikasi; klaim "UI
never freezes" saat impor STL dilunakkan (belum terverifikasi, §3); elips
hanya sejajar sumbu; datum GUI tidak asosiatif; i18n belum lengkap; daftar
crate menambah `ducad-sim` dan `ducad-update`; tutorial "Quick start: sample
project" dan petunjuk langkah berikutnya di kanvas disebut.

## 3. Belum terverifikasi

Perilaku visual/perangkat (palm rejection, ghost, glyph, layout iPad), "UI tidak
membeku" saat impor besar, AR Quick Look, STEP dari CAD lain, akurasi FEA, build
fitur opsional (apple-fm, GGUF, MNEMONIC, Android), dan cloud sync terhadap
server nyata. Butuh `docs/CEKLIS_UJI_GUI.md` atau perangkat.
