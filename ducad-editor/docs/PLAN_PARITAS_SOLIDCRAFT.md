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
| KRN | `crates/ducad-kernel/src/` (wrapper OCCT, satu-satunya yang boleh memanggil OCCT) |
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
| 0.1 | Primitive Box/Cylinder/Sphere/Cone | `Op::Primitive` SPEC:105 | `ToolKind::Primitive{kind}` di T; grup "Primitif" di LT; popup dimensi (w/h/d, r/h) memakai pola `helix_popup.rs`; `OPS::apply_primitive` | [ ] |
| 0.2 | Mirror body 3D | `Op::Mirror` SPEC:298 | `CB::MirrorBody`; pilih plane dari HUD `DatumPlaneMode` yang sudah ada atau face planar; preview ghost seperti `propose_ops` | [ ] |
| 0.3 | Rotate body (gizmo) | `Op::Transform` punya rotasi | Handle rotasi di gizmo, snap 15°, field sudut di HUD; `SK::rotate_selected_body` paralel dengan `translate_selected_body` SK:841 | [ ] |
| 0.4 | Dimensi Radius/Diameter/Angle di sketch | `ConstraintSpec::Radius`/`Angle` | Pill dimensi di `ducad-app/src/overlay/dimensions.rs` untuk lingkaran/busur (R/Ø toggle) dan dua garis (sudut); hotkey `D` memilih tipe dari seleksi | [ ] |
| 0.5 | Constraint Midpoint/Concentric/Collinear/PointOnCurve | SKC `constraint/types.rs:62` | Tombol di CS dan FI `InspectorConstraintAction`; aturan pemilihan (2 lingkaran → concentric, titik+garis → midpoint/point-on-curve, 2 garis → collinear) di `app.rs:4280-4321` | [ ] |
| 0.6 | Center Point Arc, Center Rectangle | `EntitySpec::Arc{center}`, `Rect{center}` | Mode di HUD Arc/Rect (pola `SlotMode`/`PolygonMode`) | [ ] |
| 0.7 | Two-Distance Chamfer 2D | `compute_chamfer_2d(d1,d2)` SKC `ops.rs:548` | Field kedua di popup chamfer; GUI saat ini mengirim `dist,dist` (SK:90) | [ ] |
| 0.8 | Edit Text | command `UpdateText` di SKC | Double-click entitas teks → popup; `CB::EditText` | [ ] |
| 0.9 | Panel Parameters | `set_params`, ekspresi `$w/2-3` (`ops/num.rs:40`) | Drawer baru `parameters_drawer.rs`: tabel nama/ekspresi/nilai/komentar, tambah/hapus/rename, highlight op pemakai; field numerik di popup fitur menerima ekspresi | [ ] |
| 0.10 | Compare Bodies (diff visual) | `render_diff_svg` `engine/src/render.rs:99` | Menu Inspect → "Bandingkan dengan snapshot…" memakai history snapshot FT; overlay merah/hijau di viewport | [ ] |
| 0.11 | Export Command Script, Inspect Feature | `get_oplog`, `inspect` | Tombol di FT: "Salin oplog JSON", "Inspect" (bbox, volume, face count) | [ ] |
| 0.12 | Joint Limits | `MateLimits` `ducad-core/src/assembly.rs:162` | Field min/max di `ASM::UpdateMateParam`; solver clamp di `OPS::solve_assembly` (OPS:2038) | [ ] |
| 0.13 | Rigid Group | `JointKind::Rigid` `assembly.rs:91` | `ASM::MakeRigidGroup` dari multi-seleksi instance; solver memperlakukan grup sebagai satu body | [ ] |
| 0.14 | AutoConstrain umum | SKC `infer.rs` (hanya dari Freehand) | Tombol "Auto-constrain" di CS yang memanggil `infer` pada seleksi/seluruh sketsa; tampilkan jumlah constraint yang ditambah, satu langkah undo | [ ] |
| 0.15 | Render PNG dari GUI | `render_view` (MCP/CLI) | `FileOp::ExportPng` dengan pilihan resolusi dan preset studio | [ ] |
| 0.16 | Loose Ends | SKC `gaps.rs` | Highlight ujung terbuka saat sketsa gagal membentuk region; tombol "Tampilkan ujung terbuka" di error card | [ ] |

Kriteria selesai Fase 0: setiap item punya entri FTL en/id, ada di command palette, masuk
`docs/CEKLIS_UJI_GUI.md`, dan `cargo test --workspace` hijau.

---

## Fase 1 — Fondasi sketch & construction (±4 minggu)

Tanpa ini, Revolve, Circular Pattern, dan assembly sering mentok karena tidak ada sumbu/titik
referensi.

### 1A. Point entity dan construction geometry

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 1.1 | `Entity::Point` di SKC `entity.rs` | Entity baru + snapping + constraint Coincident/Fixed; `ToolKind::Point`; render sebagai marker | [ ] |
| 1.2 | `Op::ConstructAxis` | Varian: `TwoPoints`, `Edge`, `NormalToFace`, `ThroughCylinder`, `TwoPlanes`. Disimpan di document sebagai datum (pola `PlaneSpec`). Selector `axis:<id>` di `select/parse.rs` | [ ] |
| 1.3 | `Op::ConstructPoint` | Varian: `Vertex`, `Center` (circle/sphere/torus), `EdgePlane`, `TwoEdges`, `ThreePlanes`, `AlongPath` | [ ] |
| 1.4 | `PlaneSpec` tambahan | `Midplane`, `Perpendicular` (ke edge di titik), `Tangent` (ke silinder), `AlongPath`, `TwoEdges`; mode baru di HUD `DatumPlaneMode` | [ ] |
| 1.5 | Datum drawer | Perluas drawer datum plane menjadi "Datum" (plane/axis/point), show/hide, rename | [ ] |
| 1.6 | Revolve/Circular Pattern/Mate memakai axis datum | Opsi "sumbu: datum" di popup Revolve (OPS:251), Pattern (OPS:1553), Mate Concentric | [ ] |

### 1B. Entitas dan alat sketch yang belum ada

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 1.7 | Circle 2-point, 3-point, 2-tangent, 3-tangent | Mode di HUD Circle; 3-tangent memakai solver (constraint Tangent ×3 lalu solve) | [ ] |
| 1.8 | Tangent Arc | Mode Arc: titik awal menempel ujung entitas → constraint Tangent otomatis | [ ] |
| 1.9 | 3-Point Rectangle, Rounded Rectangle | Mode Rect; rounded = rect + 4 arc + Tangent + EqualRadius | [ ] |
| 1.10 | Center Point Slot, Arc Slot (center/3-point) | `SlotMode` tambahan; arc slot memakai 2 arc konsentris + 2 arc ujung | [ ] |
| 1.11 | Edge Polygon | `PolygonMode::Edge` | [ ] |
| 1.12 | Midpoint Line, Infinite Line (construction) | Mode Line; infinite = construction line dengan render tak-hingga | [ ] |
| 1.13 | Break | `ToolKind::Break`: pecah entitas di titik klik, pertahankan constraint di ujung | [ ] |
| 1.14 | Project (P) umum + Intersect | `Op::SketchProject{refs}` menghasilkan entitas *linked* (read-only, ikut berubah). Generalisasi `face_sketch.rs:76 project_boundary`. "Break Link" mengubahnya menjadi entitas biasa | [ ] |
| 1.15 | Copy/Cut/Paste entitas sketch | Clipboard internal (`ducad-app/src/clipboard.rs`): serialisasi `EntitySpec` + constraint internal; paste dengan offset; Cmd/Ctrl+C/X/V | [ ] |
| 1.16 | Toggle Driven dimension | Flag `driven` di `Constraint::Distance/Radius/Angle`; driven tidak masuk solver, hanya tampil | [ ] |
| 1.17 | Move dimension text | Offset teks dimensi disimpan di sketch, drag di overlay | [ ] |
| 1.18 | Constraint glyphs | Ikon kecil di kanvas per constraint (ICON_*), klik → pilih/hapus; toggle di CS | [ ] |
| 1.19 | Insert SVG | IO `svg_import.rs` (path → Bézier/line/arc) ke sketsa aktif, pakai parser yang sama dengan Vector mode | [ ] |
| 1.20 | Curvature (G2) constraint | Untuk spline–spline / spline–arc; tambah ke solver LM | [ ] |

---

## Fase 2 — Solid & direct editing (±4 minggu)

DUCAD mengklaim "Shapr3D-style direct modeling"; tiga op face di bawah adalah intinya.

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 2.1 | Offset Face | `Op::OffsetFace{faces, distance}`; KRN: `BRepOffsetAPI_MakeOffsetShape` per face / `BRepFeat`; gizmo tarik face = press-pull untuk face non-planar | [ ] |
| 2.2 | Delete Face | `Op::DeleteFace{faces}`; KRN: `BRepAlgoAPI_Defeaturing`; `CB::DeleteFace` | [ ] |
| 2.3 | Replace Face | `Op::ReplaceFace{face, with: surface_ref}`; KRN: `BRepFeat` / rebuild lewat defeaturing + extend | [ ] |
| 2.4 | Pattern on Path | `PatternSpec::Path{edge/sketch, count, align}`; sample kurva di KRN (`GCPnts_UniformAbscissa`) | [ ] |
| 2.5 | Pipe | Gula di atas `Op::Sweep` dengan profil lingkaran (r, wall) | [ ] |
| 2.6 | Web | Seperti Rib (OPS:958) tetapi multi-garis dan tebal simetris | [ ] |
| 2.7 | Torus | Tambah ke `Op::Primitive` | [ ] |
| 2.8 | Emboss dari sketsa | Generalisasi `apply_emboss_deboss` OPS:1842 agar menerima region sketsa, bukan hanya teks | [ ] |
| 2.9 | Align | `Op::Align{body, from_face, to_face}` = transform yang membuat dua face coplanar/konsentris | [ ] |
| 2.10 | Boundary Fill, Bounding Solid | Boundary fill: `BOPAlgo_CellsBuilder`; bounding solid: box/cylinder dari bbox/OBB | [ ] |
| 2.11 | Split Face by sketch/plane | Perluas `CB::SplitFace` agar menerima plane datum dan kurva sketsa | [ ] |

---

## Fase 3 — Timeline, file, dan keselamatan data (±3 minggu)

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 3.1 | Reorder feature (drag) | Drag-and-drop di FT; validasi dependensi (op yang merujuk id op lain tidak boleh melompatinya); replay oplog | [ ] |
| 3.2 | Rename feature | Field `label` opsional di setiap `Op`; `FT::RenameFeature`; tampil di tree dan drawing BOM | [ ] |
| 3.3 | Folder/group feature | `Op::Group{label, children}` atau metadata terpisah di document; collapse/expand di FT | [ ] |
| 3.4 | Copy/Paste feature | Clipboard op JSON (dari 1.15) + remap id dan selector saat paste | [ ] |
| 3.5 | Feature Dependents | Klik op → highlight op yang bergantung (graf sudah ada untuk replay) | [ ] |
| 3.6 | Roll-to marker sejati | Replay oplog sampai indeks N tanpa snapshot; fitur setelah marker tampil abu-abu; op baru disisipkan di posisi marker | [ ] |
| 3.7 | Recover Design: daftar + discard | `ducad-app/src/recovery.rs`: direktori autosave per dokumen, dialog "Pulihkan / Buang" saat start (sekarang otomatis: `mobile.rs:198`), menu File → Recoverable Designs | [ ] |
| 3.8 | Backup `.bak` + atomic save | Tulis ke temp lalu rename; simpan `.ducad.bak` sebelumnya; tes kill-mid-save | [ ] |
| 3.9 | Export 3MF | IO `threemf.rs` (zip + XML model, unit mm, warna material) dari mesh yang sama dengan STL | [ ] |
| 3.10 | Import 3MF / OBJ / glTF sebagai mesh | Perluas `import_worker.rs:94` (sekarang STL saja) | [ ] |
| 3.11 | Export/Import IGES | KRN: `IGESControl_Writer/Reader` sudah ada di OCCT; `FileOp::ExportIges`/`ImportIges` | [ ] |
| 3.12 | Installed Slicers | Deteksi PrusaSlicer/Bambu/Cura/OrcaSlicer di path standar per OS; "Buka di slicer" = export 3MF ke temp lalu `open` | [ ] |
| 3.13 | Named Views | Simpan kamera (pos/target/up/zoom) di document; save/apply/rename/delete di HUD ViewCube | [ ] |
| 3.14 | Crash report + replay | `catch_unwind` per aksi GUI → tulis `crash-<ts>.json` (oplog + aksi terakhir) ke app-data; menu Help → Crash Reports; `ducad-cli replay` sudah ada | [ ] |
| 3.15 | Export view as DXF (hidden-line) | Pakai HLR yang sudah dipakai drawing (`ducad-io/src/drawing/`) untuk view aktif → DXF | [ ] |

---

## Fase 4 — Assembly & motion (±4 minggu)

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 4.1 | Joint Revolute/Slider/Cylindrical/Planar/Ball | `JointKind` sudah ada di core (`assembly.rs:91`); tambah ke mate spec level `Op`, solver (DoF per joint), UI `HUD MateHudAction` | [ ] |
| 4.2 | Pin-slot, Cam | Cam = kontak kurva–titik; pakai `BRepExtrema` di KRN | [ ] |
| 4.3 | As-Built Joint | Joint dari posisi sekarang tanpa memindahkan part | [ ] |
| 4.4 | Joint Origin | Datum point/axis (Fase 1) sebagai acuan joint | [ ] |
| 4.5 | Rename / Activate component | `ASM::RenameInstance`, mode "aktif" yang membatasi edit ke satu komponen | [ ] |
| 4.6 | Move bodies/sketches ke component, Create components from bodies (manual) | Perluas `sync_assembly_instances` OPS:1927 menjadi aksi eksplisit | [ ] |
| 4.7 | Capture / Revert position | Simpan transform instance ke oplog (`Op::Transform` per instance) atau buang | [ ] |
| 4.8 | Contact set | `ContactSet{instances}`; solver menolak penetrasi memakai clash detection yang sudah ada (OPS:2070) | [ ] |
| 4.9 | Motion: plot, trace, export animasi, motion interference | Plot posisi/kecepatan joint (egui_plot); trace = polyline titik terpilih; animasi = PNG sequence/GIF; interference = clash per frame | [ ] |
| 4.10 | Occurrence copy (instance) | Duplikat instance dengan part yang sama | [ ] |
| 4.11 | Configuration rules & preview | Rule `if param then param`; preview tanpa activate | [ ] |

---

## Fase 5 — Inspect & analisis (±2 minggu)

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 5.1 | Clearance | Jarak minimum antar dua body (`BRepExtrema_DistShapeShape`); check `Clearance` sudah ada, tinggal UI | [ ] |
| 5.2 | Minimum Radius, Curvature map/comb | Shader warna kurvatur (`shader.wgsl` sudah punya zebra/draft); comb untuk kurva sketsa | [ ] |
| 5.3 | Continuity (G0–G2) antar face | `GeomLProp` / `BRepLProp` di KRN; label di edge | [ ] |
| 5.4 | Silhouette, Isocurve, Accessibility | Silhouette dari HLR; isocurve dari `BRepAdaptor_Surface`; accessibility = ray cast arah tool | [ ] |
| 5.5 | List Threads | Daftar `Op::Thread`/hole tapped di dokumen | [ ] |

---

## Fase 6 — Area baru (pilih sesuai target pengguna)

### 6A. Surfaces (±6 minggu) — prioritas jika target product/enclosure designer

Semua tersedia di OCCT; ini keunggulan DUCAD dibanding SolidCraft yang harus menulis sendiri.

| # | Fungsi | KRN (OCCT) | Status |
|---|---|---|---|
| 6.1 | Surface Extrude/Revolve/Sweep/Loft (open profile) | `BRepPrimAPI_MakePrism/MakeRevol`, `BRepOffsetAPI_MakePipeShell`, `ThruSections(false)` | [ ] |
| 6.2 | Patch (fill boundary), Boundary (Coons/Gordon) | `BRepFill_Filling`, `GeomFill_ConstrainedFilling` | [ ] |
| 6.3 | Ruled, Offset Surface, Extend | `BRepFill::Face`, `BRepOffsetAPI_MakeOffsetShape`, `GeomLib::ExtendSurfByLength` | [ ] |
| 6.4 | Trim / Untrim / Stitch / Unstitch | `BRepAlgoAPI_Splitter`, `BRep_Tool::Surface` + rebuild face, `BRepBuilderAPI_Sewing` | [ ] |
| 6.5 | Thicken (surface → solid) | `BRepOffsetAPI_MakeThickSolid` | [ ] |
| 6.6 | Blend / Match / Fair | `GeomFill`, `GeomPlate`, `FairCurve` | [ ] |
| 6.7 | Reverse Normal | `Reverse()` + flag di document | [ ] |
| 6.8 | Workspace "Surface" di LT | Grup tool terpisah, body tipe `Shell` di document dan selector `[kind=shell]` | [ ] |

### 6B. Plastic workspace (±3 minggu) — DUCAD sudah punya rule pack `dfm-injection`

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 6.9 | Plastic rules (tebal dinding, draft, radius) | Reuse `rules/dfm-injection`; rule aktif menjadi default parameter fitur | [ ] |
| 6.10 | Boss | Silinder + lubang + fillet dasar + draft, parameter dari rule | [ ] |
| 6.11 | Lip/Groove | Offset profil tepi shell ± clearance | [ ] |
| 6.12 | Snap Fit (cantilever) | Rib + hook; check tegangan via `simulate_static` (keunggulan DUCAD) | [ ] |
| 6.13 | Rest | Pad datar pada permukaan lengkung | [ ] |

### 6C. Sheet metal lanjutan (±3 minggu) — prioritas jika target fabrikasi

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 6.14 | Base flange dari sketsa bebas | Hapus batasan "persegi di XY" di `Op::BaseFlange` SPEC:353 | [ ] |
| 6.15 | Flange di atas flange | Perbaiki `Op::EdgeFlange` agar menerima edge hasil flange sebelumnya | [ ] |
| 6.16 | Convert solid → sheet metal | Deteksi tebal konstan + bend radius dari solid | [ ] |
| 6.17 | Corner relief (potong), Perforate | Relief sekarang tidak dipotong; perforate = pattern lubang pada flat | [ ] |
| 6.18 | Sheet rules manager + Export bend table | UI untuk K-factor/radius per material; export `BendTable` (`ducad-core`) ke CSV | [ ] |
| 6.19 | Flat view otomatis di drawing | Tambah view "Flat pattern" di `DrawingSpec` | [ ] |

### 6D. Canvas/Decal, Mesh convert, Library (±3 minggu)

| # | Fungsi | Pekerjaan | Status |
|---|---|---|---|
| 6.20 | Canvas (gambar referensi di sketsa) + kalibrasi | Tekstur di plane sketsa; dua titik → skala; opacity; generalisasi `import_pdf` | [ ] |
| 6.21 | Decal | Tekstur pada face (render saja, masuk GLB) | [ ] |
| 6.22 | Mesh → B-rep | Deteksi face planar/silinder dari mesh (`ShapeFix`, `BRepBuilderAPI_MakePolygon`); batasi ke part prismatis | [ ] |
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

- Samakan lisensi: `LICENSE` (AGPL-3.0) vs `Cargo.toml` (`MIT OR Apache-2.0`).
- README: jumlah tool MCP 27 → 37; bahasa "18+" → 2; Windows sudah di CI.
- Landing page: banner "New in 0.2" → versi sekarang.
