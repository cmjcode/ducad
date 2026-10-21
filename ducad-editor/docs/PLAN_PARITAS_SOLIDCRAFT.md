# Plan: Paritas Fungsi DUCAD terhadap SolidCraft

Tanggal: 2026-10-19. Basis: audit per-command terhadap `docs/features.md` SolidCraft v0.16.2
(±390 command) vs DUCAD v0.5.6. Hasil audit: ±40–45 % sudah ada di GUI DUCAD, ±10 % ada di
engine tapi belum ada menunya, sisanya belum ada.

Tujuan plan ini: **DUCAD tidak boleh gagal dipakai karena menu/fungsi dasar tidak ada.**
Fitur khas DUCAD (drawing sheet, FEA, design package, vector/ink, GD&T, AI card) tidak disentuh
di sini dan tetap menjadi pembeda.

Prinsip kerja (berlaku untuk semua fase):

- Semua fungsi geometri baru masuk sebagai `Op` di `ducad-engine/src/ops/spec.rs` dulu, baru
  GUI. Dengan begitu MCP, CLI, dan GUI mendapat fungsi yang sama (satu sumber, seperti
  *command registry* SolidCraft).
- Setiap op baru wajib: contoh di `get_schema`, fixture di `ducad-engine/tests/fixtures/`,
  pesan error + `hint` di `ducad-engine/src/diagnose/`.
- Setiap tombol/label baru wajib ada di `ducad-i18n/locales/{en-US,id-ID}/main.ftl` dan lolos
  `cargo test -p ducad-ui --test ui_glyph_coverage`. Ikon pakai `egui_icons::icons::ICON_*`,
  bukan glyph Unicode.
- Tidak ada `unwrap`/`panic` (ADR 0009). Uji hostile-input untuk setiap op.
- Tandai status di tabel: `[ ]` belum, `[~]` sedang, `[x]` selesai.

Singkatan file:

| Kode | Path |
|---|---|
| SPEC | `crates/ducad-engine/src/ops/spec.rs` (enum `Op`) |
| COMP | `crates/ducad-engine/src/compute/` (eksekusi op) |
| KRN | `crates/ducad-kernel/src/` (wrapper SolidCraft/truck, satu-satunya yang boleh memanggil kernel) |
| OPS | `crates/ducad-app/src/modeling/operations.rs` |
| T | `crates/ducad-app/src/types.rs` (`ToolKind`, `PaletteAction`, `FileOp`) |
| LT | `crates/ducad-ui/src/left_toolbar.rs` |
| CB | `crates/ducad-ui/src/context_bar.rs` (`ContextAction`) |
| CS | `crates/ducad-ui/src/constraint_strip.rs` |
| HUD | `crates/ducad-ui/src/canvas_hud.rs` |
| FT | `crates/ducad-ui/src/feature_tree_drawer.rs` |
| FI | `crates/ducad-ui/src/feature_inspector/` |
| ASM | `crates/ducad-ui/src/assembly_drawer.rs` |
| IND | `crates/ducad-ui/src/industry_panel.rs` + `crates/ducad-app/src/industry_ui.rs` |
| SK | `crates/ducad-app/src/input/sketch.rs` |
| SKC | `crates/ducad-sketch/src/` |
| IO | `crates/ducad-io/src/` |
| FTL | `crates/ducad-i18n/locales/*/main.ftl` |

---

## Fase 0 — Wiring engine ke GUI (±3 minggu)

Semua item di sini sudah ada di engine; pekerjaannya murni UI. Tidak ada perubahan kernel.

| # | Fungsi | Sudah ada | Pekerjaan | Status |
|---|---|---|---|---|
| 0.1 | Primitive Box/Cylinder/Sphere/Cone | `Op::Primitive` SPEC:105 | `ToolKind::Primitive{kind}` di T; grup "Primitif" di LT; popup dimensi (w/h/d, r/h) memakai pola `helix_popup.rs`; `OPS::apply_primitive` | [x] |
| 0.2 | Mirror body 3D | `Op::Mirror` SPEC:298 | `CB::MirrorBody`; pilih plane dari HUD `DatumPlaneMode` yang sudah ada atau face planar; preview ghost seperti `propose_ops` | [x] |
| 0.3 | Rotate body (gizmo) | `Op::Transform` punya rotasi | Handle rotasi di gizmo, snap 15°, field sudut di HUD; `SK::rotate_selected_body` paralel dengan `translate_selected_body` SK:841 | [x] |
| 0.4 | Dimensi Radius/Diameter/Angle di sketch | `ConstraintSpec::Radius`/`Angle` | Pill dimensi di `ducad-app/src/overlay/dimensions.rs` untuk lingkaran/busur (R/Ø toggle) dan dua garis (sudut); hotkey `D` memilih tipe dari seleksi | [x] |
| 0.5 | Constraint Midpoint/Concentric/Collinear/PointOnCurve | SKC `constraint/types.rs:62` | Tombol di CS dan FI `InspectorConstraintAction`; aturan pemilihan (2 lingkaran → concentric, titik+garis → midpoint/point-on-curve, 2 garis → collinear) di `app.rs:4280-4321` | [x] |
| 0.6 | Center Point Arc, Center Rectangle | `EntitySpec::Arc{center}`, `Rect{center}` | Mode di HUD Arc/Rect (pola `SlotMode`/`PolygonMode`) | [x] |
| 0.7 | Two-Distance Chamfer 2D | `compute_chamfer_2d(d1,d2)` SKC `ops.rs:548` | Field kedua di popup chamfer; GUI saat ini mengirim `dist,dist` (SK:90) | [x] |
| 0.8 | Edit Text | command `UpdateText` di SKC | Double-click entitas teks → popup; `CB::EditText` | [x] |
| 0.9 | Panel Parameters | `set_params`, ekspresi `$w/2-3` (`ops/num.rs:40`) | Drawer baru `parameters_drawer.rs`: tabel nama/ekspresi/nilai/komentar, tambah/hapus/rename, highlight op pemakai; field numerik di popup fitur menerima ekspresi | [~] |
| 0.10 | Compare Bodies (diff visual) | `render_diff_svg` `engine/src/render.rs:99` | Menu Inspect → "Bandingkan dengan snapshot…" memakai history snapshot FT; overlay merah/hijau di viewport | [x] |
| 0.11 | Export Command Script, Inspect Feature | `get_oplog`, `inspect` | Tombol di FT: "Salin oplog JSON", "Inspect" (bbox, volume, face count) | [x] |
| 0.12 | Joint Limits | `MateLimits` `ducad-core/src/assembly.rs:162` | Field min/max di `ASM::UpdateMateParam`; solver clamp di `OPS::solve_assembly` (OPS:2038) | [x] |
| 0.13 | Rigid Group | `JointKind::Rigid` `assembly.rs:91` | `ASM::MakeRigidGroup` dari multi-seleksi instance; solver memperlakukan grup sebagai satu body | [x] |
| 0.14 | AutoConstrain umum | SKC `infer.rs` (hanya dari Freehand) | Tombol "Auto-constrain" di CS yang memanggil `infer` pada seleksi/seluruh sketsa; tampilkan jumlah constraint yang ditambah, satu langkah undo | [x] |
| 0.15 | Render PNG dari GUI | `render_view` (MCP/CLI) | `FileOp::ExportPng` dengan pilihan resolusi dan preset studio | [~] |
| 0.16 | Loose Ends | SKC `gaps.rs` | Highlight ujung terbuka saat sketsa gagal membentuk region; tombol "Tampilkan ujung terbuka" di error card | [x] |

Catatan pengerjaan Fase 0 (2026-10-19):

- 0.1–0.3: panel `ducad-app/src/body_tools.rs`; gizmo rotasi ternyata sudah ada (snap 5°, Shift = bebas), jadi 0.3 menambah input sudut presisi + snap 15° di panel, bukan gizmo baru.
- 0.4 `sketch_dims.rs`; 0.5/0.14 `sketch_constraints.rs` (tombol di context bar sketsa); 0.6/0.7 `sketch_modes.rs`; 0.8 `text_edit.rs` (double-click glyph atau palette; teks dari tool Text kini bernama grup `Teks: …`); 0.16 `loose_ends.rs`.
- 0.9 `design_tools.rs`: tabel nama/nilai/dipakai-oleh, tambah/hapus/rename (menulis ulang `$lama` di oplog), replay satu langkah undo. **Belum**: kolom komentar (tidak ada tempat simpan di `DesignDoc`) dan input ekspresi di popup fitur GUI (fitur GUI belum berupa `Op`).
- 0.10 `view_tools.rs`: snapshot manual (bukan dari history FT) + API engine `diff::snapshot_model`/`diff_against_snapshot`; ghost hijau/merah memakai jalur ghost proposal.
- 0.11: Inspect dan Salin oplog lewat command palette (bukan tombol di FT).
- 0.12: solver sudah menjepit `MateLimits`; yang ditambah UI Min/Maks di drawer Assembly.
- 0.13: `AssemblyTree::rigid_groups` (serde default, berkas lama tetap terbaca) + solver menggerakkan anggota grup sebagai satu body; perintah lewat palette.
- 0.15: PNG dari renderer garis `render_view` (sama dengan MCP/CLI). **Belum**: preset studio berbayang — renderer saat ini hanya garis.
- Semua perintah baru masuk command palette lewat `ParityCmd` (`types.rs`); butir uji manual di `CEKLIS_UJI_GUI.md` bagian A18.

Kriteria selesai Fase 0: setiap item punya entri FTL en/id, ada di command palette, masuk
`docs/CEKLIS_UJI_GUI.md`, dan `cargo test --workspace` hijau.

---

## Fase 1 — Fondasi sketch & construction (±4 minggu)

Tanpa ini, Revolve, Circular Pattern, dan assembly sering mentok karena tidak ada sumbu/titik
referensi.

### 1A. Point entity dan construction geometry

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 1.1 | `Entity::Point` di SKC `entity.rs` | Entity baru + snapping + constraint Coincident/Fixed; `ToolKind::Point`; render sebagai marker | [x] |
| 1.2 | `Op::ConstructAxis` | Varian: `TwoPoints`, `Edge`, `NormalToFace`, `ThroughCylinder`, `TwoPlanes`. Disimpan di document sebagai datum (pola `PlaneSpec`). Selector `axis:<id>` di `select/parse.rs` | [x] |
| 1.3 | `Op::ConstructPoint` | Varian: `Vertex`, `Center` (circle/sphere/torus), `EdgePlane`, `TwoEdges`, `ThreePlanes`, `AlongPath` | [x] |
| 1.4 | `PlaneSpec` tambahan | `Midplane`, `Perpendicular` (ke edge di titik), `Tangent` (ke silinder), `AlongPath`, `TwoEdges`; mode baru di HUD `DatumPlaneMode` | [x] |
| 1.5 | Datum drawer | Perluas drawer datum plane menjadi "Datum" (plane/axis/point), show/hide, rename | [x] |
| 1.6 | Revolve/Circular Pattern/Mate memakai axis datum | Opsi "sumbu: datum" di popup Revolve (OPS:251), Pattern (OPS:1553), Mate Concentric | [x] |

### 1B. Entitas dan alat sketch yang belum ada

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 1.7 | Circle 2-point, 3-point, 2-tangent, 3-tangent | Mode di HUD Circle; 3-tangent memakai solver (constraint Tangent ×3 lalu solve) | [x] |
| 1.8 | Tangent Arc | Mode Arc: titik awal menempel ujung entitas → constraint Tangent otomatis | [x] |
| 1.9 | 3-Point Rectangle, Rounded Rectangle | Mode Rect; rounded = rect + 4 arc + Tangent + EqualRadius | [x] |
| 1.10 | Center Point Slot, Arc Slot (center/3-point) | `SlotMode` tambahan; arc slot memakai 2 arc konsentris + 2 arc ujung | [x] |
| 1.11 | Edge Polygon | `PolygonMode::Edge` | [x] |
| 1.12 | Midpoint Line, Infinite Line (construction) | Mode Line; infinite = construction line dengan render tak-hingga | [x] |
| 1.13 | Break | `ToolKind::Break`: pecah entitas di titik klik, pertahankan constraint di ujung | [x] |
| 1.14 | Project (P) umum + Intersect | `Op::SketchProject{refs}` menghasilkan entitas *linked* (read-only, ikut berubah). Generalisasi `face_sketch.rs:76 project_boundary`. "Break Link" mengubahnya menjadi entitas biasa | [~] |
| 1.15 | Copy/Cut/Paste entitas sketch | Clipboard internal (`ducad-app/src/clipboard.rs`): serialisasi `EntitySpec` + constraint internal; paste dengan offset; Cmd/Ctrl+C/X/V | [x] |
| 1.16 | Toggle Driven dimension | Flag `driven` di `Constraint::Distance/Radius/Angle`; driven tidak masuk solver, hanya tampil | [x] |
| 1.17 | Move dimension text | Offset teks dimensi disimpan di sketch, drag di overlay | [~] |
| 1.18 | Constraint glyphs | Ikon kecil di kanvas per constraint (ICON_*), klik → pilih/hapus; toggle di CS | [x] |
| 1.19 | Insert SVG | IO `svg_import.rs` (path → Bézier/line/arc) ke sketsa aktif, pakai parser yang sama dengan Vector mode | [x] |
| 1.20 | Curvature (G2) constraint | Untuk spline–spline / spline–arc; tambah ke solver LM | [x] |

Catatan pengerjaan 1A, 1.1–1.12, 1.16, 1.17, 1.20 (2026-10-19):

- 1.2–1.4 engine: `Op::ConstructAxis` / `Op::ConstructPoint` (semua varian di tabel), `PlaneSpec` baru (`midplane`, `perpendicular_to`, `tangent_to`, `along`, `through_edges`), rujukan `axis:<id>` / `point:<id>`; rumus murni di `ducad-engine/src/datum.rs` dipakai juga oleh GUI. Fixture `datum_axis.ops.json`. Kernel tidak mengekspos sampel edge, jadi edge non-garis/non-lingkaran ditolak (`unsupported`) dan pusat torus memakai centroid face (+ peringatan).
- 1.2–1.6 GUI: `ducad-app/src/datum_tools.rs` (sumbu/titik datum dari pick viewport, mode bidang baru di HUD), drawer "Datum" tiga bagian di `ducad-ui/src/planes_drawer.rs` (show/hide, rename, hapus; tersimpan di `.ducad`). Sumbu datum dipakai Revolve (harus terletak di bidang sketch), Pattern melingkar, dan Mate Concentric.
- 1.1 `Entity::Point` (dirujuk lewat `PointRef::Center`); tool "Titik" di LT; Coincident otomatis bila klik jatuh di titik yang ada. `PointRef::LineStart/LineEnd` kini juga menunjuk ujung busur.
- 1.7–1.12 `ducad-sketch/src/construct2d.rs` + `construct_cmd.rs`, GUI `sketch_modes.rs` + `sketch_modes_fase1.rs`. **Belum**: flag garis tak-hingga hilang setelah hapus → undo (id baru); hit-test hanya pada bagian berhingga.
- 1.16 `driven` di Distance/Radius/Angle (tidak ditulis bila false, berkas lama identik); klik kanan pill = toggle.
- 1.17 `ducad-sketch/src/dim_text.rs`. **Belum**: pill radius/sudut belum bisa digeser (baru panjang garis dan Distance).
- 1.20 `Constraint::Curvature` (ujung bertemu + tangen + kelengkungan sama) di solver LM.
- MCP: ringkasan `get_schema` kini hanya memuat kalimat pertama deskripsi op, dan enum jenis op di `tools/list` hanya ada di larik `ops` (bukan di `replace_op`/`replace`) — keduanya agar anggaran ukuran (16 KB / 40 KB) tetap terpenuhi saat op terus bertambah.

Catatan pengerjaan 1.13–1.15, 1.18, 1.19 (2026-10-19):

- 1.13 `ducad-sketch/src/break_at.rs` (`break_entity` + command `BreakEntity`): potongan pertama memakai ulang id asli, potongan kedua mengambil alih rujukan `LineEnd`; sambungan dikunci Coincident (+ Collinear untuk garis, Concentric + EqualRadius untuk busur). Lingkaran → dua busur setengah; spline melepas bentuk `exact`. Elips/Path belum didukung. GUI `ducad-app/src/sketch_break.rs`, tombol "Pecah" di LT.
- 1.14 Engine: `Op::SketchProject` (`compute/project.rs`, mode `project`/`intersect`, fixture `sketch_project.ops.json`). GUI `sketch_project.rs` memakai fungsi engine yang sama; asal entitas di `Sketch::linked` (`ducad-sketch/src/link.rs`, body dirujuk lewat NAMA karena `BodyId` tidak stabil lintas simpan). Turunan ulang tiap frame bila `mesh_fingerprint` body berubah. **Belum**: read-only hanya ditegakkan untuk Break (seret/edit langsung entitas linked masih bisa, tetapi ditimpa saat body berubah); irisan lengkung keluar sebagai banyak garis pendek.
- 1.15 `clipboard.rs`: payload JSON (`ducad-sketch-clip/1`) juga ditulis ke clipboard sistem; Cmd/Ctrl+C/X/V ditangani sebagai `Event::Copy/Cut/Paste` egui-winit.
- 1.18 `constraint_glyphs.rs`: ikon dari `ConstraintAction::icon` (ICON_*), klik → pilih, Delete → hapus; tombol toggle di CS + palette.
- 1.19 `ducad-io/src/svg_import.rs` memakai `usvg` (parser yang sama dengan render SVG aplikasi; Vector mode tidak punya parser SVG sendiri). Busur SVG tiba sebagai Bézier kubik, bukan `Entity::Arc`.

---

## Fase 2 — Solid & direct editing (±4 minggu)

DUCAD mengklaim "Shapr3D-style direct modeling"; tiga op face di bawah adalah intinya.

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 2.1 | Offset Face | `Op::OffsetFace{faces, distance}`; KRN: offset per face (`direct::offset_faces_by_index`); gizmo tarik face = press-pull untuk face non-planar | [x] |
| 2.2 | Delete Face | `Op::DeleteFace{faces}`; KRN: defeaturing (`direct::delete_faces_by_index`); `CB::DeleteFace` | [x] |
| 2.3 | Replace Face | `Op::ReplaceFace{face, with: surface_ref}`; KRN: rebuild lewat defeaturing + extend (`direct::replace_face_with_plane`) | [x] |
| 2.4 | Pattern on Path | `PatternSpec::Path{edge/sketch, count, align}`; sample kurva berjarak seragam di KRN | [x] |
| 2.5 | Pipe | Gula di atas `Op::Sweep` dengan profil lingkaran (r, wall) | [x] |
| 2.6 | Web | Seperti Rib (OPS:958) tetapi multi-garis dan tebal simetris | [x] |
| 2.7 | Torus | Tambah ke `Op::Primitive` | [x] |
| 2.8 | Emboss dari sketsa | Generalisasi `apply_emboss_deboss` OPS:1842 agar menerima region sketsa, bukan hanya teks | [x] |
| 2.9 | Align | `Op::Align{body, from_face, to_face}` = transform yang membuat dua face coplanar/konsentris | [x] |
| 2.10 | Boundary Fill, Bounding Solid | Boundary fill: sel dari boolean antar tool (`direct::boundary_fill`); bounding solid: box/cylinder dari bbox/OBB | [x] |
| 2.11 | Split Face by sketch/plane | Perluas `CB::SplitFace` agar menerima plane datum dan kurva sketsa | [x] |

Catatan pengerjaan Fase 2 (2026-10-19):

- Kernel: `ducad-kernel/src/backend/truck/direct.rs` (offset/delete/replace/split face, boundary
  fill, bounding solid, torus, `point_inside`) + `tests/direct_edit.rs`. Topologi logis tidak lagi
  menggabung face di sepanjang garis split, sehingga potongan Split Face bisa dipilih sendiri.
- Engine: op `offset_face`, `delete_face`, `replace_face`, `split_face`, `pipe`, `web`, `emboss`,
  `align`, `boundary_fill`, `bounding_solid` + `primitive.torus` + `pattern.path`
  (`compute/direct.rs`, `session/direct.rs`, `diagnose/direct.rs`); contoh `direct_edit` di
  `get_schema`; tes `ducad-engine/tests/direct_edit.rs` (termasuk input hostil). Fixture juga ikut
  sapuan hostil penuh (`--ignored`), tidak di sapuan cepat karena lambat.
- GUI: jendela **Edit Langsung** (`ducad-app/src/direct_tools.rs`) untuk 10 alat, entri palette
  `ParityCmd::Direct`, tombol Offset Face / Hapus Face di context bar face, Torus di popup Primitif,
  Split Face GUI kini hanya memotong face terpilih (atau face yang dilintasi bidang). Ceklis
  `CEKLIS_UJI_GUI.md` bagian A20.
- Batasan: 2.1 tanpa gizmo tarik khusus (pakai jendela / push-pull lama); 2.3 hanya mengganti face
  DATAR dengan BIDANG (face datar body lain, bidang datum/standar), belum permukaan lengkung;
  2.9 Align hanya memindah sepanjang normal (sebidang), tidak menyamakan titik tengah; Pipe & Pattern
  on Path di GUI memakai tepi terpilih sebagai jalur (MCP: sketsa atau titik); 2.10 Boundary Fill
  memakai body sebagai pembatas (bukan bidang/permukaan).

---

## Fase 3 — Timeline, file, dan keselamatan data (±3 minggu)

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 3.1 | Reorder feature (drag) | Drag-and-drop di FT; validasi dependensi (op yang merujuk id op lain tidak boleh melompatinya); replay oplog | [x] |
| 3.2 | Rename feature | Field `label` opsional di setiap `Op`; `FT::RenameFeature`; tampil di tree dan drawing BOM | [x] |
| 3.3 | Folder/group feature | `Op::Group{label, children}` atau metadata terpisah di document; collapse/expand di FT | [x] |
| 3.4 | Copy/Paste feature | Clipboard op JSON (dari 1.15) + remap id dan selector saat paste | [x] |
| 3.5 | Feature Dependents | Klik op → highlight op yang bergantung (graf sudah ada untuk replay) | [x] |
| 3.6 | Roll-to marker sejati | Replay oplog sampai indeks N tanpa snapshot; fitur setelah marker tampil abu-abu; op baru disisipkan di posisi marker | [x] |
| 3.7 | Recover Design: daftar + discard | `ducad-app/src/recovery.rs`: direktori autosave per dokumen, dialog "Pulihkan / Buang" saat start (sekarang otomatis: `mobile.rs:198`), menu File → Recoverable Designs | [x] |
| 3.8 | Backup `.bak` + atomic save | Tulis ke temp lalu rename; simpan `.ducad.bak` sebelumnya; tes kill-mid-save | [x] |
| 3.9 | Export 3MF | IO `threemf.rs` (zip + XML model, unit mm, warna material) dari mesh yang sama dengan STL | [x] |
| 3.10 | Import 3MF / OBJ / glTF sebagai mesh | Perluas `import_worker.rs:94` (sekarang STL saja) | [x] |
| 3.11 | Export/Import IGES | KRN: `iges::write_iges`/`read_iges`; `FileOp::ExportIges`/`ImportIges` | [x] |
| 3.12 | Installed Slicers | Deteksi PrusaSlicer/Bambu/Cura/OrcaSlicer di path standar per OS; "Buka di slicer" = export 3MF ke temp lalu `open` | [x] |
| 3.13 | Named Views | Simpan kamera (pos/target/up/zoom) di document; save/apply/rename/delete di HUD ViewCube | [x] |
| 3.14 | Crash report + replay | `catch_unwind` per aksi GUI → tulis `crash-<ts>.json` (oplog + aksi terakhir) ke app-data; menu Help → Crash Reports; `ducad-cli replay` sudah ada | [x] |
| 3.15 | Export view as DXF (hidden-line) | Pakai HLR yang sudah dipakai drawing (`ducad-io/src/drawing/`) untuk view aktif → DXF | [x] |

Catatan pengerjaan Fase 3 (2026-10-19):

- 3.1–3.6 dibangun di `ducad_core::ParametricDag` (pohon fitur GUI), bukan oplog engine:
  `move_feature` (tolak melompati induk/turunan), `rename_feature`, `groups` (folder),
  `paste_features` (remap dependensi), `dependency_highlight`, roll-to marker
  (`set_rollback_after`; fitur baru disisipkan di marker; regenerasi melewati fitur sesudahnya).
  UI di drawer Feature Tree: seret judul fitur untuk mengubah urutan, menu klik-kanan (naik/turun,
  ganti nama, salin/tempel, gulung ke sini, folder), klik dua kali = ganti nama, klik = highlight
  induk (ungu) & turunan (hijau). Handler: `ducad-app/src/timeline.rs`. **Belum**: pohon fitur GUI
  belum ikut disimpan di `.ducad` (sudah begitu sebelum Fase 3), jadi nama/folder/marker hilang saat
  berkas dibuka ulang; `label` di `Op` engine dan BOM gambar kerja belum.
- 3.7 `ducad-app/src/recovery.rs`: autosave per sesi di `data_dir/recovery/`, dialog Pulihkan/Buang
  saat start dan dari palette; autosave lama dimigrasikan. 3.8 `ducad_io::native::write_atomic_with_backup`
  (temp + fsync + `.bak` + rename) dipakai semua simpan `.ducad`.
- 3.9/3.10 `ducad-io/src/mesh_formats.rs` (3MF tulis/baca, OBJ, glTF/GLB, `read_mesh_file`);
  3.11 `ducad_kernel::iges` (penerjemah IGES SolidCraft); 3.12 `ducad-app/src/slicers.rs`;
  3.15 `ducad_io::dxf::export_vector_snapshot_dxf` (HLR sudut pandang aktif, satu layer per jenis
  garis, satuan piksel viewport). Semua lewat palette (grup Berkas) di `interop.rs`.
- 3.13 `named_views.rs` (tombol di bawah ViewCube; disimpan di `datums.views`). 3.14
  `crash_reports.rs`: pagar panik command model menulis `crash-<ts>.json` + `crash-<ts>.ducad`
  (bisa `ducad-cli replay`), jendela "Laporan crash" di palette.

---

## Fase 4 — Assembly & motion (±4 minggu)

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 4.1 | Joint Revolute/Slider/Cylindrical/Planar/Ball | `JointKind` sudah ada di core (`assembly.rs:91`); tambah ke mate spec level `Op`, solver (DoF per joint), UI `HUD MateHudAction` | [x] |
| 4.2 | Pin-slot, Cam | Cam = kontak kurva–titik; pakai jarak minimum B-rep di KRN | [x] |
| 4.3 | As-Built Joint | Joint dari posisi sekarang tanpa memindahkan part | [x] |
| 4.4 | Joint Origin | Datum point/axis (Fase 1) sebagai acuan joint | [x] |
| 4.5 | Rename / Activate component | `ASM::RenameInstance`, mode "aktif" yang membatasi edit ke satu komponen | [x] |
| 4.6 | Move bodies/sketches ke component, Create components from bodies (manual) | Perluas `sync_assembly_instances` OPS:1927 menjadi aksi eksplisit | [x] |
| 4.7 | Capture / Revert position | Simpan transform instance ke oplog (`Op::Transform` per instance) atau buang | [x] |
| 4.8 | Contact set | `ContactSet{instances}`; solver menolak penetrasi memakai clash detection yang sudah ada (OPS:2070) | [x] |
| 4.9 | Motion: plot, trace, export animasi, motion interference | Plot posisi/kecepatan joint (egui_plot); trace = polyline titik terpilih; animasi = PNG sequence/GIF; interference = clash per frame | [x] |
| 4.10 | Occurrence copy (instance) | Duplikat instance dengan part yang sama | [x] |
| 4.11 | Configuration rules & preview | Rule `if param then param`; preview tanpa activate | [x] |

Catatan pengerjaan Fase 4 (2026-10-20):

- Fondasi: perakitan GUI (instance, mate/joint, motion study, contact set, posisi tersimpan) kini
  **disimpan** di `.ducad` (`datums.assembly`, body dipetakan lewat UUID) — sebelumnya hilang saat
  berkas ditutup. Bug lama diperbaiki: scrub motion study tidak memindah geometri B-rep.
- 4.1–4.4 `ducad_core::joint`: joint = frame di ruang lokal A + pose relatif B + nilai joint;
  solver kernel memakai model ini (ikut relaksasi Jacobi, limit dijepit, motion study menggerakkan
  nilai utama). Jenis: Rigid, Revolute, Slider, Cylindrical, Planar, Ball, Pin-slot, Cam (follower
  dicari sampai menyentuh cam lewat jarak minimum/irisan). As-built = tanpa memindah part; asal joint
  bisa datum sumbu. UI: jendela "Alat Perakitan" + palette "Joint: …" (`assembly_tools.rs`).
- 4.5–4.8, 4.10: ganti nama/aktifkan komponen (part lain tidak bisa dipilih), kelompokkan seleksi
  jadi komponen (sub-assembly) dan pindahkan antar komponen, capture/revert posisi, contact set
  (gerakan yang membuat anggota menembus ditolak), salin occurrence.
- 4.9 `motion_tools.rs`: sampel di salinan pohon (model tidak berubah), plot nilai & kecepatan,
  jejak titik di viewport, interferensi per frame, ekspor PNG per frame + `motion.csv`.
  **Belum**: GIF (hanya urutan PNG).
- 4.11 `ducad_core::ConfigRule` (`if w > 50 then t = 4`, berantai, maks 16 putaran) diterapkan di
  `effective_params`; `ducad_engine::preview_configuration` + tombol Pratinjau di panel industri.

---

## Fase 5 — Inspect & analisis (±2 minggu)

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 5.1 | Clearance | Jarak minimum antar dua body (`interference::check_clearance`); check `Clearance` sudah ada, tinggal UI | [x] |
| 5.2 | Minimum Radius, Curvature map/comb | Shader warna kurvatur (`shader.wgsl` sudah punya zebra/draft); comb untuk kurva sketsa | [x] |
| 5.3 | Continuity (G0–G2) antar face | Properti lokal permukaan di KRN (`direct::seam_continuity`); label di edge | [x] |
| 5.4 | Silhouette, Isocurve, Accessibility | Silhouette dari HLR; isocurve dari parameterisasi face; accessibility = ray cast arah tool | [x] |
| 5.5 | List Threads | Daftar `Op::Thread`/hole tapped di dokumen | [x] |

Catatan pengerjaan Fase 5 (2026-10-20): jendela **Inspeksi** (`ducad-app/src/inspect_tools.rs`,
palette grup Analisis) + overlay viewport.

- 5.1 clearance dua body terpilih (`check_clearance`), garis titik terdekat + jarak.
- 5.2 radius minimum (face silinder/kerucut/bola); peta kurvatur di shader (`CurvatureConfig`,
  |dN|/|dP| per piksel ≈ 1/R, biru → merah pada nilai "Merah pada"); sisir kurvatur untuk
  spline/busur/lingkaran sketsa aktif.
- 5.3 `ducad_kernel::seam_continuity` (diagnostik seam SolidCraft): setiap sambungan diwarnai &
  dilabeli G0/G1/G2/Gap.
- 5.4 siluet dari HLR sudut pandang aktif; isocurve face datar (grid terpotong batas face) dan
  silinder (garis + lingkaran dalam rentang face); aksesibilitas = titik face yang menghadap alat
  dan tidak terhalang (raycast BVH), dengan persentase luas. **Belum**: isocurve kerucut/bola/NURBS.
- 5.5 daftar ulir dari op `thread` + lubang ISO `tapped`.

---

## Fase 6 — Area baru (pilih sesuai target pengguna)

### 6A. Surfaces (±6 minggu) — prioritas jika target product/enclosure designer

Implementasi kernel: `ducad-kernel/src/backend/truck/surface.rs` (`ducad_kernel::surface`).

| # | Fungsi | KRN (`surface::*`) | Status |
|---|---|---|---|
| 6.1 | Surface Extrude/Revolve/Sweep/Loft (open profile) | `extrude_sheet`, `revolve_sheet`, `sweep_sheet`, `loft_sheet` | [x] |
| 6.2 | Patch (fill boundary), Boundary (Coons/Gordon) | `patch`, `boundary` | [x] |
| 6.3 | Ruled, Offset Surface, Extend | `ruled`, `offset_surface`, `extend` | [x] |
| 6.4 | Trim / Untrim / Stitch / Unstitch | `trim_by_plane`, `untrim`, `stitch`, `unstitch` | [x] |
| 6.5 | Thicken (surface → solid) | `thicken` | [x] |
| 6.6 | Blend / Match / Fair | `blend`, `match_to`, `fair` | [x] |
| 6.7 | Reverse Normal | `reverse_normals` + flag di document | [x] |
| 6.8 | Workspace "Surface" di LT | Grup tool terpisah, body tipe `Shell` di document dan selector `[kind=shell]` | [~] |

Catatan pengerjaan 6A (2026-10-20): `ducad_kernel::surface` + op `surface` (MCP/CLI) dengan
`make` = extrude/revolve/sweep/loft (profil terbuka), patch, boundary, ruled, offset, copy_faces,
blend, extend, trim/untrim, thicken, reverse, fair, match, stitch/unstitch; body sheet terbuka
disimpan apa adanya. GUI: jendela **Permukaan** (`ducad-app/src/surface_tools.rs`) memakai
seleksi sketsa/tepi/face/body. Fixture `surfaces.ops.json`. **Belum (6.8)**: selector
`[kind=shell]` untuk memilih body sheet.

### 6B. Plastic workspace (±3 minggu) — DUCAD sudah punya rule pack `dfm-injection`

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 6.9 | Plastic rules (tebal dinding, draft, radius) | Reuse `rules/dfm-injection`; rule aktif menjadi default parameter fitur | [x] |
| 6.10 | Boss | Silinder + lubang + fillet dasar + draft, parameter dari rule | [x] |
| 6.11 | Lip/Groove | Offset profil tepi shell ± clearance | [x] |
| 6.12 | Snap Fit (cantilever) | Rib + hook; check tegangan via `simulate_static` (keunggulan DUCAD) | [x] |
| 6.13 | Rest | Pad datar pada permukaan lengkung | [x] |

Catatan pengerjaan 6B (2026-10-20): `compute/plastic.rs` + op `plastic` (`make` = boss,
lip/groove, snap_fit, rest). Ukuran yang tidak diisi diambil dari rule pack `dfm-injection` aktif
(`PlasticDefaults::from_checks`: draft, tebal dinding, radius dalam). Snap fit melaporkan regangan
vs batas izin. GUI: jendela **Plastik** (`ducad-app/src/plastic_tools.rs`) bekerja pada face yang
diklik. Fixture `plastic_case.ops.json`.

### 6C. Sheet metal lanjutan (±3 minggu) — prioritas jika target fabrikasi

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 6.14 | Base flange dari sketsa bebas | Hapus batasan "persegi di XY" di `Op::BaseFlange` SPEC:353 | [x] |
| 6.15 | Flange di atas flange | Perbaiki `Op::EdgeFlange` agar menerima edge hasil flange sebelumnya | [x] |
| 6.16 | Convert solid → sheet metal | Deteksi tebal konstan + bend radius dari solid | [x] |
| 6.17 | Corner relief (potong), Perforate | Relief sekarang tidak dipotong; perforate = pattern lubang pada flat | [x] |
| 6.18 | Sheet rules manager + Export bend table | UI untuk K-factor/radius per material; export `BendTable` (`ducad-core`) ke CSV | [ ] |
| 6.19 | Flat view otomatis di drawing | Tambah view "Flat pattern" di `DrawingSpec` | [ ] |

Catatan pengerjaan 6C (2026-10-20, 6.14–6.17 selesai; 6.18–6.19 berikutnya):

- 6.14 `base_flange` menerima region apa pun: busur/Bezier/elips dicacah jadi poligon, lubang di
  region jadi `SheetMetalModel::cutouts` (lingkaran tetap lingkaran sejati) dan ikut ke pola datar.
- 6.15 `edge_flange`/`hem`/`jog` pada tepi ujung flange memperpanjang rantai segmen flange itu
  (`EdgeTarget::Tip`, `SheetMetalModel::section_tip`).
- 6.16 op `convert_to_sheet`: solid pelat datar setebal seragam → body sheet metal (tebal dari
  dua face sejajar terbesar, lubang bulat → cutout, cek volume 1 %). Solid yang sudah bertekuk
  ditolak; konteks error memuat radius tekuk yang terdeteksi.
- 6.17 relief `rect`/`obround` kini dipotong (takik di pelat dasar lewat boolean 2D `i_overlay`,
  zona tekuk dipotong dari solid flange sebelum digabung; pola datar = satu daerah 2D yang
  di-extrude sekali). Op `perforate`: grid lubang lurus/selang-seling, margin dari tepi/lubang/
  relief, maks 2000 lubang.
- Fixture `sheet_vent.ops.json`; tes `tests/sheet_metal.rs` (lubang, relief, flange di flange,
  perforasi + input jahat, konversi).

### 6D. Canvas/Decal, Mesh convert, Library (±3 minggu)

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 6.20 | Canvas (gambar referensi di sketsa) + kalibrasi | Tekstur di plane sketsa; dua titik → skala; opacity; generalisasi `import_pdf` | [ ] |
| 6.21 | Decal | Tekstur pada face (render saja, masuk GLB) | [ ] |
| 6.22 | Mesh → B-rep | Deteksi face planar/silinder dari mesh lalu bangun face B-rep; batasi ke part prismatis | [ ] |
| 6.23 | Design library lokal, parts.resize | Folder `.ducad` template; resize = ganti ukuran `Op::StandardPart` | [ ] |

---

## Urutan eksekusi yang disarankan

1. **Fase 0** (wiring): nilai langsung, risiko rendah.
2. **Fase 1A** (point + axis + plane) → **1B** → **Fase 3** item 3.7–3.11 (recover, bak, 3MF, IGES).
3. **Fase 2** (direct editing) dan **Fase 3** sisanya (timeline).
4. **Fase 4**, **Fase 5**.
5. **Fase 6**: pilih 6A atau 6C dulu sesuai target pengguna; 6B setelah 6A.

Total perkiraan: Fase 0–5 ≈ 20 minggu; Fase 6 ≈ 15 minggu tambahan.

## Definisi selesai per item

- Op ada di `get_schema` dengan contoh dan fixture; `ducad-cli run` fixture hijau.
- Tombol/menu ada, label FTL en+id, ikon dari `egui_icons`, lolos `ui_glyph_coverage`.
- Tes hostile-input (input kosong, NaN, selector tidak cocok) tidak panic.
- Baris di `docs/CEKLIS_UJI_GUI.md` dan, bila mengubah format file, migrasi di IO + tes round-trip.
- README/landing page diperbarui bila fitur termasuk yang disebut "Not available yet".

## Perbaikan dokumentasi (kerjakan bersamaan dengan Fase 0)

- [ ] Samakan lisensi: `LICENSE` (AGPL-3.0) vs `Cargo.toml` (`MIT OR Apache-2.0`) — butuh keputusan pemilik proyek, belum diubah.
- [x] README: jumlah tool MCP 27 → 37; bahasa "18+" → 2; Windows sudah di CI (clippy + test).
- [ ] Landing page: banner "New in 0.2" → versi sekarang — sumber landing page tidak ada di repo ini.
