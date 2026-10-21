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
| Teks sketsa TTF/OTF | GUI | stabil (GUI) | `ducad-sketch/src/tests.rs::{test_text_vectorization, text_regenerate_is_deterministic}` | Tidak ada entitas teks di oplog |
| Snapping berprioritas | GUI | stabil | `ducad-sketch/src/tests.rs::{snap_prefers_endpoint_over_grid, snap_finds_line_intersection}` | Glyph tidak diuji |
| Constraint (12 jenis) | keduanya | stabil | `ducad-sketch/src/constraint/tests.rs`; `ducad-engine/tests/sample_project.rs::builds_fully_constrained_bracket` | Belum ada auto-inference atau pewarnaan DOF di GUI |
| Trim/Extend/Offset/Mirror | GUI | stabil | `ducad-sketch/src/tests.rs::{trim_removes_middle_segment_between_two_cuts, test_biarc_and_multi_arc_offset}` | Tidak ada op edit sketsa di oplog |
| Freehand → constraint | GUI | stabil | `ducad-app/src/freehand.rs::stroke_becomes_lines_with_constraints_in_one_undo` | Tidak masuk oplog |
| **Datum** | | | | |
| Plane offset | keduanya | terbatas | `ducad-engine/tests/datum.rs::construction_planes_place_sketches` | Plane GUI berupa frame absolut berindeks, tidak asosiatif |
| Plane bersudut / 3 titik | GUI | terbatas | `ducad-app/src/document.rs`; `ducad-render/src/plane.rs::{test_angled_plane, test_three_point_plane}` | Tidak ada di oplog; normal acuan hardcode `Vec3::Z` |
| **3D** | | | | |
| Extrude blind | keduanya | stabil | `ducad-engine/tests/session.rs::plate_ops_build_expected_volume`; `sample_project.rs` | — |
| Extrude symmetric | oplog | terbatas | `ducad-kernel/src/tests.rs::symmetric_extrude_straddles_the_sketch_plane` | GUI selalu Blind (`operations.rs:108-113`) |
| Extrude up to face | – | **belum tersedia** | `ducad-kernel/src/csg.rs` `ExtrudeExtent` hanya Blind/Symmetric/TwoSided | Diklaim README |
| Revolve sumbu kustom | keduanya | terbatas | fixture `flange.ops.json`, `datum_axis.ops.json` | GUI: sumbu 2D di bidang sketsa |
| Loft / Sweep / Draft / Emboss | keduanya | terbatas | `ducad-engine/tests/advanced_ops.rs` | GUI memanggil kernel langsung, tidak masuk DAG, tanpa tes GUI |
| Helix | keduanya | terbatas | `advanced_ops.rs::helix_spring_volume` | Regenerasi GUI `Unsupported` |
| Boolean union/cut | keduanya | stabil (oplog) | fixture `bracket`, `flange`, `valve_body`; `ducad-kernel/tests/fuzz_boolean.rs` | Regenerasi GUI `Unsupported` |
| Boolean intersect | keduanya | terbatas | `ducad-kernel/src/tests.rs::intersect_overlapping_boxes_smaller_than_union` | Tanpa tes tingkat op |
| Fillet konstan / Chamfer | keduanya | stabil (oplog) / terbatas (GUI) | 6 fixture; `sample_project.rs::editing_initial_dimensions_keeps_downstream_features` | Regenerasi GUI = `fillet_all`/`chamfer_all` (B0-5) |
| Fillet radius variabel | keduanya | terbatas | `advanced_ops.rs::variable_fillet_and_replay` | Hanya tepi lurus; regenerasi GUI abaikan `radius_end` |
| Shell | keduanya | stabil | fixture `enclosure`, `plastic_case` | Tebal variabel hanya body konveks |
| Hole simple/cbore/csink | keduanya | stabil (oplog) / terbatas (GUI) | `ducad-kernel/src/tests.rs::test_hole_wizard_counterbore_iso4762_m6`; `sample_project.rs` | GUI tidak mencatat hole ke DAG; regenerasi `Unsupported`; op lambat (B0-7) |
| Hole tapped | keduanya | terbatas | `ducad-kernel/src/tests.rs::test_hole_wizard_tapped_m8` | Hanya bor tap, ulir tidak dimodelkan (kecuali op `thread`) |
| Pattern 3D | keduanya | stabil (oplog) / terbatas (GUI) | fixture `flange`; `adapter_tests.rs::adapter_apply_pattern_3d_volume` | GUI tidak masuk DAG |
| Direct modeling | keduanya | terbatas | `ducad-engine/tests/direct_edit.rs` | GUI hanya masuk undo model, tidak ke oplog/DAG |
| **Riwayat dan undo** | | | | |
| Regenerasi parametrik (oplog) | oplog | stabil | `ducad-engine/tests/replay.rs::{set_params_changes_thickness, replay_is_deterministic}`; `sample_project.rs` | Satu-satunya jalur dengan referensi semantik |
| Timeline DAG + regenerasi (GUI) | GUI | terbatas | `parametric_engine.rs::test_parametric_dag_recording_and_regeneration` | DAG tidak disimpan ke `.ducad` dan direset saat buka/baru; ref `plane_index`; fallback extrude 25 mm hardcode |
| Riwayat aktivitas + cabang | GUI | terbatas | `ducad-app/src/history_db.rs` | Dikosongkan setiap aplikasi start (`app.rs:569`), jadi cabang hilang |
| Undo | keduanya (terpisah) | terbatas | `ducad-app/src/document.rs::undo_order_across_modes_is_global`; `ducad-engine/tests/session.rs::undo_redo_reproduces_volume` | Tiga stack GUI + undo oplog terpisah; edit parameter oplog tidak bisa di-undo (B0-1) |
| **Berkas** | | | | |
| `.ducad` native | keduanya | terbatas | `ducad-io/tests/native_format.rs`; `ducad-engine/tests/session_file.rs`; `sample_project.rs::save_and_reopen_roundtrip` | Format kini v4; DAG GUI dan anotasi GD&T GUI tidak disimpan |
| Impor STEP | keduanya | terbatas | `ducad-kernel/tests/step_roundtrip.rs` | Hanya round-trip berkas sendiri; tanpa fixture CAD lain |
| Ekspor STEP | keduanya | stabil | `step_roundtrip.rs`; `sample_project.rs::export_step_reopens_and_stl_is_written` | Dibatasi tier lisensi lewat MCP (WIP belum di-commit) |
| Impor / ekspor DXF | GUI (+ flat oplog) | terbatas | `ducad-io/src/dxf.rs` (12 tes); `ducad-io/src/flat_dxf.rs` | Tanpa SPLINE/TEXT/INSERT/HATCH; R12; tanpa entitas DIMENSION |
| Impor STL, ekspor STL/OBJ | GUI + CLI | stabil | `ducad-io/src/mesh_export.rs`; `ducad-io/tests/hostile_stl.rs` | — |
| Ekspor GLB / SVG | GUI | stabil | `ducad-io/src/glb.rs`; `ducad-io/src/svg.rs` | AR Quick Look tidak terverifikasi |
| Ekspor PDF | keduanya | stabil | `ducad-io/tests/sheet_p21.rs`; `sample_project.rs::drawing_exports_pdf_and_dxf` | Dibatasi lisensi lewat MCP (WIP) |
| **Gambar kerja** | | | | |
| Multi-view + HLR | keduanya | stabil | `ducad-engine/tests/drawing_auto.rs`; `drawing_golden.rs` | — |
| Section + hatch | keduanya | stabil | `ducad-kernel/src/tests.rs::test_section_view_brep_slice_and_hatch_generation` | — |
| Detail view | keduanya | terbatas | `ducad-io/src/drawing.rs::test_detail_view_creation_and_layout` | Interaksi GUI tidak diuji |
| Dimensi otomatis / manual | keduanya | stabil (otomatis) | `sheet_p21.rs::editor_dimensions_are_associative_and_draggable` | Alat manual GUI tanpa tes end-to-end |
| BOM, balon, title block | keduanya | stabil | `ducad-io/tests/exploded_drawing.rs`; `ducad-engine/tests/package.rs` | — |
| Multi-sheet | oplog (paket) | terbatas | `ducad-io/src/drawing/pages.rs` | Editor GUI hanya satu sheet |
| **Assembly** | | | | |
| Pohon, instance, mate concentric/coincident/distance, clash | GUI | stabil | `ducad-core/src/assembly.rs`; `ducad-kernel/src/assembly_solver.rs`; `ducad-kernel/src/interference.rs` | **Tidak ada op assembly di oplog** — penghalang keputusan Step 0 |
| Mate angle | GUI | terbatas | `assembly_solver.rs` | Tanpa tes khusus |
| Coupling gear/screw/rack, explode | GUI | terbatas | `ducad-core/src/coupling.rs` | Bukan op oplog |
| **Agent / headless** | | | | |
| Oplog atomik + selector | oplog | stabil | `ducad-engine/tests/session.rs::failing_batch_rolls_back_everything`; `tests/select.rs` | Filter selector tanpa parameter (B0-3) |
| `ducad-cli` | oplog | stabil | `ducad-cli/tests/cli.rs` | `release`/`golden`/`migrate-file` tidak didokumentasikan |
| `ducad-mcp` (37 tool, 43 dengan `--attach`) | oplog | stabil | `ducad-mcp/src/tools.rs`; `ducad-mcp/src/server.rs` tes | README menyebut "20-tool" di bagian lain |
| Design checks, diff, build CI | oplog | stabil | `ducad-engine/tests/{checks,diff}.rs`; `cli.rs::build_*` | — |
| Kode error + fix terverifikasi | keduanya | terbatas | `ducad-engine/tests/diagnose.rs`; `sample_project.rs::verified_fixes_survive_full_replay` (ignored) | Fix "verified" tidak me-replay op hilir (B0-2); hanya untuk op oplog |
| Live bridge + proposal/ghost | keduanya | stabil (logika) | `ducad-app/src/proposal_ui.rs`; `ducad-mcp/src/attach.rs` | Unix saja; render ghost tidak diuji |
| `ducad-assist` loop proposal | oplog | stabil (backend tiruan) | `ducad-assist/tests/assist.rs` | — |
| Ask AI apple-fm / GGUF, MNEMONIC, eval | GUI / n.a. | eksperimental | `ducad-assist/src/{apple,gguf}.rs`; `ducad-app/src/memory.rs`; `evals/` | Tidak dibangun CI; GGUF tidak tersambung GUI; eval 11 tugas (README: 10) |
| **Simulasi dan industri** | | | | |
| Mass properties, FEA statik, konfigurasi, ISO 286, standard part | keduanya | stabil | `ducad-engine/tests/{inspect_mass,configurations,standard_parts}.rs`; `ducad-sim/tests/static_pipeline.rs` | FEA estimasi ±10 % |
| Frequency/buckling/thermal | keduanya | terbatas | `ducad-sim/tests/p18_gates.rs`; `ducad-engine/tests/study_kinds.rs` | Panel GUI hanya statik |
| Sheet metal + flat DXF | keduanya | stabil (oplog) / terbatas (GUI) | `ducad-engine/tests/sheet_metal.rs` | Base GUI hanya persegi di XY; batas di README sudah basi |
| GD&T di sheet | MCP + GUI | terbatas | `ducad-io/tests/gdt_pdf.rs` | Anotasi GUI hanya per sesi |
| **UI** | | | | |
| Command palette, ViewCube | GUI | stabil | `ducad-ui/src/command_palette.rs`; `ducad-ui/src/viewcube.rs` | — |
| Radial menu (`Space`) | GUI | **belum tersedia** | `ducad-ui/src/radial_menu.rs` `open_at` tanpa pemanggil | Kode mati, diklaim README |
| Lighting preset + SSAO | GUI | terbatas | `ducad-render/src/scene.rs` | Nama 4 preset beda dari README; bukan SSAO screen-space |
| i18n en-US/id-ID | GUI | terbatas | `ducad-i18n/src/lib.rs::test_key_parity` | Banyak string Indonesia hardcode |
| Mode sentuh iPad | GUI | terbatas | `ducad-ui/src/touch.rs` | Toleransi 24/12 px (README: 14 px); label mode memakai emoji |
| **Platform dan cloud** | | | | |
| Linux | n.a. | stabil | `.github/workflows/ci.yml` lane ubuntu | — |
| macOS (platform utama persona) | GUI | terbatas | `ducad-app/tests/ios_scene_manifest.rs` | **Tidak ada lane workspace macOS di CI**; diuji manual |
| Windows + MSI | n.a. | terbatas | `ci.yml`, `release.yml` | Tes/clippy Windows `continue-on-error`; tanpa live bridge |
| Android | n.a. | eksperimental | `ducad-android/src/lib.rs` | Tidak di CI |
| Akun/SSO | GUI | stabil (auth) | `ducad-cloud/src/auth.rs` | — |
| Cloud sync | GUI | eksperimental | `ducad-cloud/src/sync.rs` (belum di-commit) | Tes dengan respons tiruan |
| `./test_api.sh` | n.a. | **belum tersedia** | berkas tidak ada | Disebut di Getting Started |

## 2. Kontradiksi README vs implementasi

Untuk diselesaikan di Step 5 (atau lebih awal bila murah):

1. **"Both paths run the same modeling code… a part built in the GUI replays
   headlessly"** — tidak berlaku untuk alur manual GUI: tidak ada operasi GUI
   manual yang menghasilkan op oplog; assembly, datum bersudut/3 titik, teks
   sketsa, freehand, dan edit sketsa tidak punya op sama sekali.
2. **"Modifying past feature parameters with automatic regeneration"** — tidak
   berlaku di GUI (B0-5; DAG tidak disimpan).
3. **"Jump back to any recorded step"** — riwayat dikosongkan setiap start.
4. **Extrude "Up to Face"** tidak ada; **Symmetric** tidak terjangkau dari GUI.
5. **Revolve "custom 3D axis"** — di GUI sumbunya 2D.
6. **Tapped hole "threaded"** — hanya bor tap.
7. **Radial menu `Space`** tidak bisa dibuka.
8. **Preset lighting** README ≠ kode (`CleanStudio`, `WarmShowcase`, `CoolTech`,
   `DramaticDark`); "SSAO" bukan screen-space.
9. **Toleransi sentuh 14 px** ≠ 24 px jari / 12 px Pencil.
10. **Ask AI** — GUI hanya apple-fm; GGUF tidak tersambung.
11. **Batas sheet metal di README basi** — flange-on-flange dan bend relief
    sudah ada dan teruji.
12. **Jumlah tool MCP** — 37 vs "20-tool"; 43 dengan `--attach`.
13. **Eval "ten tasks"** — ada 11.
14. **`./test_api.sh`** disebut tetapi tidak ada.
15. **CI** — tidak ada lane workspace macOS; `release.yml` justru membangun dan
    menguji MSI; tes Windows tidak memblokir.
16. **Lisensi (WIP belum di-commit)** — ekspor STEP, PDF, `build_package`,
    `submit_for_review` ditolak untuk tier Free lewat MCP; README mengklaimnya
    tanpa syarat.
17. **`.ducad` "since format v2"** — kini v4; DAG GUI dan GD&T GUI tidak ikut.
18. **`docs/STATUS_FASE_A_B.md` basi** — masih berbingkai binding OCCT, padahal
    kernel kini truck/SolidCraft. P1.1, P2.4/P2.5, P3.4, P4.2, P4.3 sudah ada
    di kode; yang masih akurat: P0.5 (regenerasi GUI) dan P2.1 (UpTo*).
19. **Klaim kurang** (ada di kode, tidak di README): IGES, 3MF, impor
    glTF/SVG/PDF, op `perforate`/`convert_to_sheet`, CLI
    `release`/`golden`/`migrate-file`.

## 3. Belum terverifikasi

Perilaku visual/perangkat (palm rejection, ghost, glyph, layout iPad), "UI tidak
membeku" saat impor besar, AR Quick Look, STEP dari CAD lain, akurasi FEA, build
fitur opsional (apple-fm, GGUF, MNEMONIC, Android), dan cloud sync terhadap
server nyata. Butuh `docs/CEKLIS_UJI_GUI.md` atau perangkat.
