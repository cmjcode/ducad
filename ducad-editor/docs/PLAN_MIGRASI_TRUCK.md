# Plan Migrasi Kernel: OpenCASCADE → Truck (via SolidCraft kernel)

**Status**: DRAFT — menunggu persetujuan
**Tanggal**: 2026-10-19
**Sumber referensi**: [tangency-labs/solidcraft](https://github.com/tangency-labs/solidcraft) v0.16.2, commit `c66e332` (2026-10-18), lisensi MIT OR Apache-2.0
**Kompleksitas**: BESAR (6 fase, ±11–14 minggu kerja + 2 minggu soak untuk 1 engineer)

---

## 0. Ringkasan eksekutif

### Keputusan arsitektur

**Jangan pakai truck polos. Adopsi `solidcraft-kernel` + `solidcraft-geom` + `vendor/truck-*` sebagai backend kedua `ducad-kernel`, di belakang API DUCAD yang sudah ada (`KernelShape`, `KernelMesh`, 179 `pub fn`).**

Alasan, dari riset kode nyata:

| | Truck polos (crates.io) | SolidCraft kernel | Kebutuhan DUCAD |
|---|---|---|---|
| Boolean | `and`/`or` saja; gagal pada face berimpit; bug panic di rilis 0.4.0 | 11 lapis retry (prism-exact 2D, coplanar glue, BSP planar, tool-nudge, piecewise) + history face/edge | Add/Cut extrude, Hole, Pattern, sheet-metal |
| Fillet / Chamfer | **tidak ada** | `blend.rs` 2.8k baris — edit B-rep sungguhan (planar-planar, rolling-ball chain, variable, G2) | `Fillet`, `Chamfer`, rounding interaktif |
| Shell / Draft | tidak ada | `polyhedron.rs` + `offset.rs` (konveks eksak; non-konveks via offset topologi-tetap) | `Shell`, `Draft` |
| Sweep path 3D / Helix | tidak ada | `sweep_path.rs` (RMF, rails, twist), `helix.rs` | `Sweep`, `Helix` |
| STEP read | "further down the road" | reader Part 21 sendiri, AP203/214/242, assemblies, warna | Import STEP, `import_step` MCP, file `.ducad` legacy |
| STEP write hasil boolean | **tidak bisa** | bisa (intersection curve → cubic B-spline) | Export STEP, `.ducad` |
| Tessellasi per-face | ya | ya + display plan GPU + `tri_face` | `KernelMesh.face_ranges` untuk sim & picking |
| Thread-safe | ya | ya (`Arc<Solid>`, tanpa global lock, rayon) | hilangkan `KERNEL_LOCK` |
| WASM | ya | ya (dipakai `apps/solidcraft-web`) | target web yang hari ini "tidak mungkin" |

Dengan truck polos: **72 dari 354 perintah UI (20 %) gagal total**. Dengan SolidCraft kernel: yang benar-benar hilang tinggal **HLR eksak** (SolidCraft hanya punya HLR berbasis mesh) dan **thread eksak** (SolidCraft memodelkan ulir sebagai mesh facet). Keduanya punya jalur mitigasi di §4.

### Definisi "berhasil 100 %"

Migrasi dinyatakan selesai bila **semua gate di bawah hijau pada backend truck**, bukan bila "semua fungsi OCCT direplikasi bit-per-bit" — itu mustahil dan tidak perlu:

1. `cargo test --workspace --features kernel-truck` hijau, termasuk 191 tes `ducad-kernel`, `drawing_sheet_smoke`, `sheet_p21`, `gdt_pdf`, `ui_glyph_coverage`.
2. **Golden corpus** (§5 F0): setiap `.ducad` di `examples/` + setiap contoh `get_schema` di `evals/` menghasilkan volume ±0,1 %, jumlah face/edge sama (setelah merge periodik), bbox ±0,01 mm, dan `run_checks` verdict identik dengan baseline OCCT.
3. Setiap file `.ducad` yang dibuat versi ≤ 0.5.6 terbuka tanpa kehilangan body (§5 F1).
4. Tidak ada op yang `panic`; op yang tidak didukung mengembalikan `Err` dengan `hint` + `fixes` seperti kontrak MCP sekarang.
5. Build macOS/iPad/Android/Windows hijau tanpa OCCT; build `wasm32` minimal `cargo check`.

---

## 1. Peta kondisi sekarang (DUCAD)

| Item | Nilai | Sumber |
|---|---|---|
| Crate yang menyentuh OCCT | hanya `ducad-kernel` | `grep opencascade crates/*/Cargo.toml` |
| Wrapper kernel | 25 modul, 15.874 baris, 179 `pub fn`, 191 tes | `crates/ducad-kernel/src/` |
| Modul kernel yang **tidak** memakai OCCT (aman, tetap) | `assembly_solver`, `interference`, `projection`, `thickness`, `detail`, `mass`, `vector_snapshot`, `picking/ray`, `hole`, `thread` (dua terakhir memanggil modul lain) | `grep "use opencascade"` |
| Executor op | `Session::apply` [session.rs:762](../crates/ducad-engine/src/session.rs#L762), 31 varian `Op` [spec.rs:33](../crates/ducad-engine/src/ops/spec.rs#L33) | |
| Jalur bersama setiap op | `compute::finish` [mod.rs:45](../crates/ducad-engine/src/compute/mod.rs#L45) → `is_valid`, `volume`, `tessellate`, `edge_dimensions`, `extract_shape_edges`, `shape_vertices` | |
| Konsumen mesh saja | `ducad-sim` (`SurfaceMesh{positions,triangles,tri_face}` + `exact_volume_mm3`), `ducad-render` | [sim.rs:151](../crates/ducad-engine/src/sim.rs#L151) |
| Format `.ducad` | JSON + **teks STEP per body** via `to_step_string` | [shape.rs:113](../crates/ducad-kernel/src/shape.rs#L113), [native.rs:226](../crates/ducad-io/src/native.rs#L226) |
| Thread | `KERNEL_LOCK` global, `KernelShape: !Send` | [lib.rs](../crates/ducad-kernel/src/lib.rs) |
| Patch vendor OCCT | 713 baris di `vendors/opencascade-rs` | `git diff --stat` |
| Toolchain | stable (rustc 1.96.1) — SolidCraft butuh ≥ 1.95, edition 2024 ✔ | `rust-toolchain.toml` |

---

## 2. Apa yang diambil dari SolidCraft (dan apa yang tidak)

### 2.1 Diambil utuh (vendor, jangan tulis ulang)

Semua path relatif ke repo SolidCraft, disingkat `sc:`.

| Komponen | Path | Baris | Fungsi untuk DUCAD |
|---|---|---|---|
| Kernel | `sc:crates/kernel` | ±75k (88k termasuk tes) | semua op B-rep |
| Geometri dasar | `sc:crates/geom` | 12.4k, dep hanya `serde`+`thiserror` | `Vec3`, `Plane`, `Region2`, `Loop2`, `Mesh`, `Aabb3`, `bvh` (picking), `section` |
| truck ter-patch | `sc:vendor/{truck-shapeops,truck-geometry,truck-meshalgo,truck-stepio,matext4cgmath}` | ±28k | wajib — tanpa patch ini boolean non-deterministik & panic |
| Blok `[patch.crates-io]` | `sc:Cargo.toml:139-147` | | wajib ikut |
| Tes equivalence vendor | `sc:crates/kernel/tests/*_equivalence.rs`, `src/issue_*_tests.rs` | | menjaga patch tetap benar |
| Fuzz boolean | `sc:crates/kernel/src/fuzz.rs` | | gate robustness F3 |

**Jangan diambil**: `sc:crates/{doc,engine,ui-egui,render,gpu,io,mcp}` — DUCAD sudah punya padanannya. Tapi **pola** di dalamnya disalin (lihat §2.3).

### 2.2 Pemetaan API `ducad-kernel` → SolidCraft

Legenda: **ADA** = panggil langsung; **ADAPTASI** = ada, perlu bridge tipe/semantik; **KOMPOSISI** = dibangun dari primitif SolidCraft mengikuti pola `sc:crates/doc/src/eval.rs`; **TULIS** = tidak ada di SolidCraft, tulis sendiri; **TETAP** = kode DUCAD murni Rust, tidak disentuh.

| Modul DUCAD | Fungsi | Status | Padanan SolidCraft |
|---|---|---|---|
| `primitives.rs` | `make_box/cylinder/sphere/cone` | ADA | `build::box_solid`, `cylinder`, `sphere` (`sc:build.rs:548-614`); cone → `extrude_tapered` atau `revolve` profil segitiga |
| `csg.rs` | `extrude_profile(_on_plane/_extent)` | ADAPTASI | `build::extrude(&Plane,&[Region2],start,end)` (`sc:build.rs:192`); extent Blind/ThroughAll/UpToFace → pola `extent_sides` + `height_at` di `sc:doc/src/eval.rs:3180-3330` |
| | `revolve_profile` | ADA | `build::revolve` (`sc:build.rs:340`) |
| | `union/subtract/intersect` | ADA | `ops::boolean(a,b,BoolOp)` (`sc:ops.rs:564`) — mengembalikan `Option<Body>` (None = hasil kosong → petakan ke `EmptyIntersection`) |
| | `loft_profiles` | ADA | `build::loft(&[(Plane,Loop2)])` (`sc:build.rs:1723`); **lubang di section ditolak** → validasi di DUCAD |
| | `sweep_profile_along_path/_wire` | ADA | `build::sweep` (garis/busur, `sc:build.rs:2027`) atau `sweep_path::sweep_path` (polyline 3D, `sc:sweep_path.rs:517`) |
| | `emboss_profiles_on_plane` | KOMPOSISI | extrude tipis + boolean (`emboss.rs` SolidCraft khusus silinder) |
| `modify.rs` | `fillet_edges(_by_index/_variable/_all)`, `fillet_vertex` | ADAPTASI | `blend::fillet(&Body,&[Vec3],r)`, `fillet_each`, `fillet_variable`, `blend_vertex` — **pemilihan edge via titik tengah edge (Vec3), bukan index** → bridge: `EdgeInfo.mid` |
| | `chamfer_edges/_all/_vertex` | ADAPTASI | `blend::chamfer`, `chamfer_sides` (`sc:blend.rs:2076,2301`) |
| | `shell_open/_hollow/_hollow_faces/_variable_thickness` | ADAPTASI | `polyhedron::shell(&Body, open:&[Vec3], t)` (`sc:polyhedron.rs:194`); variable thickness → TULIS (atau tolak dengan `Err` + hint) |
| | `draft_angle` | ADA | `polyhedron::draft(&Body, faces, neutral, pull, angle)` (`sc:polyhedron.rs:248`) |
| | `split_body`, `split_body_with_tool`, `split_face` | ADA | `ops::split_by_plane`, `splitface::split_body/split_faces` dengan `SplitTool::{Plane,Face,Curves}` |
| | `linear_pattern_shape`, `circular_pattern_shape` | KOMPOSISI | `ops::transform_matrix` × N + `union_apart` / `boolean` — pola `sc:doc/src/eval.rs:4161,5068` |
| | `create_rib(_from_curve/_solid)` | KOMPOSISI | pola `rib_tool` `sc:doc/src/eval_more.rs:217,291` |
| | `extrude_face`, `revolve_face`, `resize_shape_along_edge` | ADAPTASI | `polyhedron::offset_faces` (push-pull planar), `extrude` dari `planar_face()` + boolean |
| | `make_filleted_box` | KOMPOSISI | `box_solid` + `fillet` |
| `advanced.rs` | `mirror_shape` | KOMPOSISI | `mirror_matrix` + `transform_matrix` (det<0 ditangani `sc:ops.rs:1723`) |
| | `loft_sections`, `draft_faces_by_index`, `fillet_edges_variable_by_index` | ADAPTASI | seperti di atas |
| `helix.rs` | `create_helix_solid(_with_custom_profile)`, `create_helix_wire`, `generate_helix_points` | ADA/TETAP | `helix::sweep_helix` (`sc:helix.rs:23`, maks 200 putaran); `generate_helix_points` DUCAD murni Rust → TETAP |
| `thread.rs` | `cut_iso_thread` | ADAPTASI ⚠ | SolidCraft: `sweep_helix_mesh` + `faceted_boolean` (`sc:polybool.rs:345`) → hasil **facet**, bukan B-rep halus. Lihat risiko R4 |
| `hole.rs` | `apply_hole`, `create_hole_cutter` | KOMPOSISI | pola `hole_tools_and_prisms` `sc:doc/src/eval.rs:2892` (counterbore = 2 silinder, countersink = `extrude_tapered`) |
| `shape.rs` | `KernelShape::{tessellate, volume, is_valid, bounding_box, …}` | ADAPTASI | `Body::tessellate(tol)`, `measure(&Body)` (volume/area/centroid **berbasis mesh, ±1e-4**), `validity()`, `Body::size()` |
| | `clone_shape`, `translate/rotate/scale/transform_shape`, `make_compound` | ADA | `Body: Clone` (Arc), `ops::transform(_matrix)`, `union_apart`/`join_apart` sebagai pengganti compound |
| | `read_step`, `write_step`, `to/from_step_string`, `to/from_brep_bytes` | ADA | `step_import` (`sc:step_in/mod.rs:1140`), `step_export` (`sc:step.rs:98`), `encode/decode_native_body` (`sc:native.rs`) |
| | `read_stl`, `write_stl` | TETAP | DUCAD sudah punya di `ducad-io/mesh_export.rs` |
| `step.rs` | `write_step_compound` | ADA | `step_export_bodies` (`sc:step_out.rs:1799`) |
| `mesh.rs` | `KernelMesh{face_ranges,…}` | ADAPTASI | `geom::Mesh.tri_face[]` tidak kontigu → **urutkan segitiga per face** saat konversi agar `face_ranges` tetap valid untuk `ducad-sim` |
| `topo.rs` | `enumerate_faces/edges`, `FaceInfo{kind,…}`, `EdgeKind` | ADAPTASI | `Body::faces(tol)`, `edges(tol)`, `face_surface()` → `FaceSurface::{Plane,Cylinder,Cone,Sphere,Torus}`, `edge_circle()` (`sc:surface_kind.rs:45,78`) |
| `picking/*` | `pick_face/_details`, `pick_edge`, `pick_vertex`, `*_outward_normal` | ADAPTASI | BVH mesh `sc:crates/geom/src/bvh.rs` + pola `sc:ui-egui/src/pick_index.rs`; normal dari `face_surface` |
| `section.rs` | `SectionExtractor` | ADAPTASI | `ops::half_space(&Body,&Plane,positive)` (`sc:ops.rs:1857`) lalu `Body::planar_edges(&Plane, faces)` untuk loop eksak; hatch (`generate_iso_hatch_pattern`) TETAP |
| `hlr.rs`, `hlr_exact.rs`, `hlr_sheet.rs` | `HlrExtractor`, `extract_exact_hlr` | **TULIS** | SolidCraft hanya HLR mesh (`sc:engine/src/cmd/view_dxf_projection.rs`, 365 baris, BVH + interval oklusi). Lihat §4.1 |
| `mass.rs` | `MassProperties`, `principal_axes` | TETAP | DUCAD sudah mesh-based; `exact_volume` dari `measure()` |
| `interference.rs`, `thickness.rs` | `detect_interference`, `check_clearance`, `min_wall_thickness` | TETAP | mesh-based; `intersect` di dalamnya ikut backend; opsi upgrade ke `distance::body_distance` eksak |
| `assembly_solver.rs` | semua | TETAP | murni Rust |
| `projection.rs`, `detail.rs`, `vector_snapshot.rs` | semua | TETAP | murni Rust |
| `profile.rs` | `Profile{Line,Arc,Bezier}` | ADAPTASI | → `geom::Region2/Loop2` (Bezier via `sc:spline_conv.rs` / `freeform.rs`) |

**Hitungan**: dari 179 fungsi publik — ADA ≈ 55, ADAPTASI ≈ 70, KOMPOSISI ≈ 20, TETAP ≈ 30, TULIS ≈ 4 (HLR eksak ×3, shell variable thickness). Tidak ada yang DROP.

### 2.3 Pola SolidCraft yang disalin ke DUCAD (bukan kode)

| Pola | Sumber | Tujuan di DUCAD |
|---|---|---|
| `guard()` — `catch_unwind` di setiap batas kernel, "tidak pernah panic", `Err("not supported yet: …")` | `sc:kernel/src/lib.rs:230` | selaras ADR 0009 DUCAD; ganti `lock_kernel()` dengan `guard()` |
| Boolean diagnostics (`BooleanStrategy` yang dipakai dilaporkan sebagai warning) | `sc:boolean_diagnostics.rs` | masuk ke `run_ops` response → field `warnings` |
| Feature gagal → **pass-through state**, bukan rollback total | `sc:doc/src/eval.rs:1343` | opsional untuk GUI parametric (`parametric_engine.rs`); MCP tetap atomik |
| Golden robustness seeds + `BLESS` | `sc:tests/robustness/goldens.json`, `docs/robustness.md` | `tests/goldens/` DUCAD (F0) |
| Oracle vs FreeCAD (`recipe.json` + `expected.json`) | `sc:tests/oracle/` | DUCAD: oracle = **build OCCT** selama transisi |
| Cache B-rep native non-otoritatif | `sc:docs/design-container.md` | format `.ducad` baru (F1) |

---

## 3. Arsitektur target

```
ducad-kernel/
├── src/lib.rs            # API publik TIDAK berubah (179 fn, KernelShape, KernelMesh)
├── src/backend/mod.rs    # trait internal + pemilihan cfg
├── src/backend/occt/     # kode sekarang dipindah ke sini (feature "kernel-occt")
├── src/backend/truck/    # baru (feature "kernel-truck")
│   ├── bridge.rs         # glam<->geom::Vec3, Profile<->Region2, Mesh<->KernelMesh
│   ├── shape.rs          # KernelShape = { body: solidcraft_kernel::Body, cache }
│   ├── csg.rs modify.rs advanced.rs helix.rs hole.rs thread.rs …   # satu file per modul lama
│   ├── hlr_mesh.rs       # HLR berbasis mesh (porting view_dxf_projection + exact features)
│   └── section.rs
├── Cargo.toml            # [features] default=["kernel-occt"]; kernel-truck = ["dep:solidcraft-kernel", …]
vendors/
├── opencascade-rs/       # dihapus di F6
└── solidcraft/           # git subtree: crates/kernel, crates/geom, vendor/*, LICENSE-*, NOTICE
```

Prinsip:
1. **Dua backend hidup berdampingan** sampai cutover; CI menjalankan keduanya. Ini satu-satunya cara mengukur regresi secara objektif.
2. **API publik beku**. `ducad-engine`, `ducad-app`, `ducad-io`, `ducad-mcp` tidak tahu backend mana yang aktif. Pengecualian terdaftar di §3.1.
3. `KernelShape` tetap tipe DUCAD; di backend truck ia `Send + Sync` sehingga `KERNEL_LOCK` dan `import_worker` sync-dance bisa hilang (F5).

### 3.1 Perubahan API yang tak terhindarkan

| Perubahan | Alasan | Dampak |
|---|---|---|
| `KernelShape::volume()` toleransi ±1e-4 relatif (bukan eksak) | `measure()` SolidCraft berbasis mesh | `ducad-sim` `exact_volume_mm3`, checks `volume`/`mass`: longgarkan toleransi pembanding ke 5e-4 |
| `fillet_edges_by_index` → internal pakai `EdgeInfo.mid` | SolidCraft memilih edge via titik | index tetap valid di API; bridge memetakan |
| `make_compound` → multi-shell `Body` | tidak ada compound di truck | `write_step_compound`, `Split` (`faces_centroid`, lump) — gunakan `Body::lumps()` |
| `shell_variable_thickness` → `Err(Unsupported)` | tidak ada di SolidCraft | UI sembunyikan opsi saat backend truck; op mengembalikan hint |
| HLR: garis tersembunyi dari mesh (akurasi chord 0,01 mm) | tidak ada HLR eksak | `HlrGeometricFeature::{Circle,Arc,Ellipse}` tetap eksak dari `face_surface`/`edge_circle` sehingga auto-dimension tidak rusak |

---

## 4. Gap yang harus ditulis sendiri

### 4.1 HLR (Hidden Line Removal) — pengganti `hlr.rs` / `hlr_exact.rs` / `hlr_sheet.rs` (2.257 baris)

Strategi: **hybrid mesh + fitur eksak**.

1. Edge kandidat = `Body::edges(tol)` (polyline B-rep, bukan edge segitiga) + siluet dari `smooth_edges`/`sharp_edges` (`geom::Mesh`) — port dari `sc:view_dxf_projection.rs`.
2. Oklusi: proyeksikan tiap segmen, klip terhadap half-plane segitiga + kedalaman, gabungkan interval tersembunyi (algoritma SolidCraft, BVH dari `geom::bvh`).
3. Fitur geometris: untuk setiap edge yang `edge_circle()` ≠ None → emit `HlrGeometricFeature::Circle/Arc` dengan pusat & radius eksak (ini yang dipakai `auto_dim.rs` dan `drawing_sheet_view.rs:471`).
4. Section view A-A: `half_space` → `planar_edges` → loop eksak → hatch (TETAP).
5. Tes regresi: `drawing_sheet_smoke`, `sheet_p21`, `gdt_pdf`, `exploded_drawing` harus hijau; tambah golden SVG hash per contoh di `examples/`.

Estimasi: 5–7 hari.

### 4.2 Thread (ulir) ISO

Pilihan A (default): ikuti SolidCraft — `sweep_helix_mesh` + `faceted_boolean`; hasil facet (48 stasiun/putaran), B-rep planar. Export STEP tetap bisa (semua planar). Render & sim OK.
Pilihan B: ulir kosmetik (anotasi + silinder minor) untuk > 190 putaran atau bila A gagal.
Keduanya dibungkus `cut_iso_thread` dengan `warnings: ["thread faceted"]`.

### 4.3 Shell ketebalan variabel

Tidak ada di SolidCraft. `shell_variable_thickness` mengembalikan `Err` dengan `hint: "gunakan shell seragam lalu offset_faces per face"`. UI: sembunyikan slider variable saat `cfg!(feature="kernel-truck")`.

### 4.4 Bridge `Profile` ↔ `Region2`

`Profile::Loop` (Line/Arc/Bezier) → `Loop2` (SolidCraft menerima garis, busur, B-spline via `spline_conv`). Lubang: DUCAD `extrude_selection_with_holes_on_plane` sudah mengurai outer/inner → `Region2{outer, holes}`. Teks (`extrude_single_entity` path) → tiap glyph satu `Region2`.

---

## 5. Fase implementasi

Setiap fase punya **Exit gate** yang dijalankan di CI dengan dua backend. Fase tidak boleh dimulai sebelum gate fase sebelumnya hijau.

### F0 — Baseline & golden corpus (1 minggu)

| Task | File | Validasi |
|---|---|---|
| 0.1 Tambah subcommand `ducad-cli golden capture --out tests/goldens/occt/` : untuk setiap `examples/*.ducad` + setiap `evals/**/ops.json` tulis `{volume, area, bbox, faces, edges, checks_verdict, mesh_sha}` | `crates/ducad-cli/src/cmd/golden.rs` (baru), mirror `sc:tests/robustness/goldens.json` | `ls tests/goldens/occt | wc -l` = jumlah contoh |
| 0.2 Tambah `golden check --against tests/goldens/occt --tol 1e-3` | sama | hijau di backend OCCT (sanity) |
| 0.3 Catat baseline waktu op per contoh (`criterion` sudah ada di workspace) | `crates/ducad-kernel/benches/ops.rs` | laporan tersimpan di `docs/perf/occt-baseline.md` |
| 0.4 Pin toolchain ke versi eksak (catatan di `rust-toolchain.toml` sudah menyarankan ini; sekarang saat yang tepat karena rebuild OCCT toh akan terjadi) | `rust-toolchain.toml` | `cargo build` |

**Exit gate F0**: `golden check` hijau 100 % di OCCT; CI job `golden` ada.

**Status F0 (2026-10-19): selesai.** Catatan pelaksanaan yang menyimpang dari tabel di atas:

- Korpus nyata ≠ asumsi plan: `examples/` hampir tidak berisi `.ducad`, dan `evals/tasks/*.json` hanya prompt + checks (tanpa ops). Korpus default `golden` = `crates/ducad-engine/tests/fixtures` (semua contoh `get_schema` + `valve_body`), `examples/`, `evals/fixtures/`, `../examples/` → **21 contoh** (`*.ops.json` + 2 `.ducad`). Tambah direktori lain dengan `--corpus`.
- Golden menyimpan `{volume, area, bbox, faces, edges, valid, mesh{vertices,triangles,sha}, checks[{label,status}], error}` per body/contoh. `check`: volume & luas relatif `--tol` (1e-3), bbox `--bbox-tol` 0,01 mm, face/tepi & vonis check eksak; **sidik jari mesh hanya dicatat** (beda antar-OS/backend) kecuali `--strict-mesh`. Contoh yang dulu error lalu kini berhasil = catatan, bukan gagal.
- Waktu op per contoh dari `golden capture --perf` → `docs/perf/occt-examples.{md,json}`; bench criterion kernel → `crates/ducad-kernel/benches/ops.rs`; ringkasan di `docs/perf/occt-baseline.md`.
- CI: step `golden` di job `full` (Linux), bukan job terpisah — menghindari build OCCT kedua. Toolchain dipaku `1.96.1` di `rust-toolchain.toml` + semua `dtolnay/rust-toolchain@1.96.1`.

### F1 — Format file lepas dari STEP (1 minggu)

Masalah: `.ducad` menyimpan body sebagai teks STEP hasil OCCT. SolidCraft reader **bisa membaca STEP AP214 keluaran OCCT** (diverifikasi: schema AP203/214/242, `MANIFOLD_SOLID_BREP`, B-spline, revolusi), jadi legacy file selamat — tetapi jangan jadikan STEP sumber kebenaran ke depan.

| Task | File | Validasi |
|---|---|---|
| 1.1 Format v+1: simpan **oplog `ducad-engine` Op[]** + params sebagai sumber kebenaran; body STEP dipertahankan hanya untuk body tanpa oplog (import/legacy) | `crates/ducad-io/src/native.rs`, `crates/ducad-engine/src/session.rs` (serialisasi `Op`) | roundtrip test: save→load→`golden check` |
| 1.2 Cache B-rep opsional `brep.bin` = `encode_native_body` SolidCraft (schema 2, produser `"ducad-truck/1"`); loader memperlakukan cache sebagai *miss* bila produser beda | `native.rs` | load tanpa cache == load dengan cache (volume sama) |
| 1.3 Alat migrasi `ducad-cli migrate-file` (dijalankan dengan backend OCCT): load legacy → tiap body `to_step_string` → simpan format baru | `crates/ducad-cli/src/cmd/migrate.rs` | semua `examples/*.ducad` termigrasi, `golden check` hijau |
| 1.4 Reader toleran: bila ada STEP body, backend truck memakai `step_import` SolidCraft; catat `warnings` bila ada face yang gagal dibaca | `crates/ducad-kernel/src/backend/truck/shape.rs` | tes: 10 file legacy terpilih terbuka tanpa body hilang |

**Exit gate F1**: semua `.ducad` lama terbuka di backend OCCT format baru; roundtrip hijau; `format_version` naik dan di-dokumentasikan di `docs/FORMAT.md`.

> **Status F1 (2026-10-19)**: 1.1–1.3 selesai; 1.4 selesai untuk bagian yang tidak bergantung truck (pembaca toleran STEP rusak → replay, cache produsen lain → miss). Format v4 di `docs/FORMAT.md`. (a) Sisa: jalur `step_import` SolidCraft di `backend/truck/shape.rs` ikut F2 — tinggal mengisi `KernelShape::{NATIVE_CACHE_PRODUCER,to/from_native_cache,from_step_string}`; (b) Gate: `golden check` (21 contoh, baseline F0) lulus 21/21 — juga dengan `--strict-mesh` — baik pada working tree F1 maupun setelah `examples/industrial-demo/*.ducad` dimigrasi ke v4 di tempat (berkas contoh kemudian dikembalikan ke v2 agar tetap jadi korpus legacy).

### F2 — Backend truck: kerangka + op dasar (2–3 minggu)

| Task | File | Validasi |
|---|---|---|
| 2.1 `git subtree add` SolidCraft `crates/kernel`, `crates/geom`, `vendor/*` → `vendors/solidcraft/`; salin `[patch.crates-io]`; tambah `LICENSE-APACHE`, `NOTICE`, bagian "Vendored" di `docs/ATTRIBUTION.md`; `deny.toml` izinkan Apache-2.0 | `Cargo.toml`, `vendors/solidcraft/` | `cargo check -p solidcraft-kernel` |
| 2.2 Pindahkan kode OCCT ke `backend/occt/`; feature `kernel-occt` (default) / `kernel-truck`; `compile_error!` bila keduanya atau tidak ada | `crates/ducad-kernel/Cargo.toml`, `src/backend/` | `cargo test -p ducad-kernel` tetap hijau (OCCT) |
| 2.3 `bridge.rs`: `glam::DVec3`↔`geom::Vec3`, `Profile`↔`Region2`, `geom::Mesh`→`KernelMesh` (sort by `tri_face` → `face_ranges`), `EdgeInfo`→`extract_shape_edges` | `backend/truck/bridge.rs` | unit test roundtrip |
| 2.4 `KernelShape` truck = `{ body: Option<Body>, mesh_cache: OnceLock<KernelMesh> }`; `empty()`, `is_valid`, `volume`, `bounding_box`, `tessellate`, `clone` | `backend/truck/shape.rs` | tes `primitives` modul lama |
| 2.5 `primitives.rs`, `csg.rs` (extrude semua extent, revolve, union/subtract/intersect, loft, sweep), `shape.rs` transform | `backend/truck/{primitives,csg,shape}.rs` | `cargo test -p ducad-kernel --no-default-features --features kernel-truck -- primitives csg shape` |
| 2.6 `topo.rs`, `picking/*` via `geom::bvh` | `backend/truck/{topo,picking}.rs` | tes `adapter_tests.rs` di `ducad-app` (pick_face_details ×11) |
| 2.7 `guard()` menggantikan `lock_kernel()` di backend truck; `KernelShape: Send + Sync` static assert | `lib.rs` | `cargo test` + clippy |

**Exit gate F2**: op `Sketch, Primitive, Extrude, Revolve, Boolean, Transform, Scale, Delete, SetMaterial` lolos `golden check` di backend truck; semua tes `ducad-engine` untuk op tersebut hijau.

**Status F2 (2026-10-19): selesai.** Hasil gate:

| Gate | Hasil |
|---|---|
| `golden check --ops sketch,primitive,extrude,revolve,boolean,transform,scale,delete,set_material,study` (truck vs golden OCCT) | **6/6 lulus**, 15 dilewati (memakai op F3). Termasuk `model.ducad` lama (body = teks STEP keluaran OCCT, dibaca `step_import`) dan logo vektor (extrude Bézier). `study` ikut karena hanya menyimpan data studi. |
| `golden check` penuh, backend OCCT | 21/21 (tidak berubah) |
| Tes kontrak `ducad-kernel` (`src/contract_tests.rs`, API publik saja, dijalankan apa adanya di kedua backend) | OCCT 21/21, truck 25/25 (+4 unit `bridge`); suite kernel truck lengkap 73/73 |
| `cargo test -p ducad-kernel` (OCCT) setelah restrukturisasi | 259 lulus (238 tes lama + 21 kontrak) |
| `cargo test -p ducad-engine` (OCCT) | 227/227 |
| `cargo test --workspace` (OCCT) | 658 lulus; 3 gagal di `ducad-core` yang **sudah gagal sebelum F2** (tabel layering belum memuat `ducad-update`; templat vault ISO basi) |
| `cargo test -p ducad-engine --features kernel-truck` | 143 lulus / 84 gagal — **semuanya di luar op F2**: 50 `unsupported` (fillet/chamfer/shell/split/draft/loft_sections/helix), 14 memuat `fixtures/plate.ops.json` (fillet + hole), 1 study dengan fillet, 2 menuntut volume identik 1e-6 (sheet metal, ulir kosmetik → toleransi F5.1), 1 standard part M3 meleset 1,5e-4 (lihat catatan volume). |
| `cargo clippy --workspace --all-targets -D warnings` (OCCT); clippy `ducad-kernel` truck; `cargo check --workspace --all-targets --features kernel-truck` | bersih |

Catatan pelaksanaan yang menyimpang dari tabel di atas:

- **2.1** Bukan `git subtree` (itu menarik seluruh repo SolidCraft, termasuk app & UI): `crates/{kernel,geom}`, `vendor/{truck-*,matext4cgmath}`, lisensi & `NOTICE` disalin ke `vendors/solidcraft/`; SHA `c66e332` + cara memperbarui dicatat di `vendors/solidcraft/README.md`. Manifest kernel/geom ditulis ulang (nilai `workspace = true` eksplisit, edition 2024) dan crate ini di-`exclude` dari workspace (tes/lint pihak ketiga tidak ikut `--workspace`). Satu-satunya tambahan kode: `ducad_ext.rs` (kueri wire/ujung edge/vertex yang tidak ada di API publik SolidCraft). `debug-assertions = false` untuk crate truck di profil dev (seperti profil tes SolidCraft). `docs/ATTRIBUTION.md` baru; `deny.toml` sudah mengizinkan Apache-2.0.
- **2.2** Fitur: `kernel-truck` **menang** bila keduanya aktif; `compile_error!` hanya bila tidak ada backend. Alasan: perintah plan sendiri `cargo test --workspace --features kernel-truck` ikut menyalakan fitur default (OCCT). Fitur `kernel-truck` diteruskan di `ducad-{io,engine,mcp,ui,cli,app,assist,android}`. Pola modul: berkas publik (`src/csg.rs`, …) menyimpan tipe & logika netral-backend lalu `pub use crate::backend::<modul>::*`; kode OCCT pindah apa adanya ke `src/backend/occt/`. HLR/section/mass/interference tetap bersama lewat hook kecil (`backend::query`, `backend::hlr`, `backend::section`). Perubahan API publik (tanpa pemakai di luar kernel): `sweep_profile_along_wire` & `create_helix_wire` (bertipe `Wire` OCCT) kini khusus `kernel-occt`; `ExactCurve2D.curve` bertipe `ExactCurveType` milik DUCAD. Tambahan: `ducad_kernel::{BACKEND, Unsupported}`, `KernelShape::bounding_box()`.
- **2.3–2.4 Topologi logis** (`backend/truck/topology.rs`): SolidCraft memecah face periodik (silinder = 2 face, lingkaran = 2 busur). Backend truck menggabungkan face bertetangga pada permukaan analitik yang sama, menyisakan satu seam per cincin, dan menyambung potongan kurva yang sama — jadi indeks `enumerate_faces/edges`, `face_ranges`, selector, dan hitungan golden setara OCCT setelah `clean()` (box 6/12, silinder 3/3, plat berlubang 7/15, cincin revolve 4/6, bola 1 face — semua ditegakkan tes kontrak). `KernelShape` truck = `{ body: Option<Body>, cache: Arc<…> }` (topologi, `measure`, validitas di-cache; salinan berbagi cache).
- **2.5** Kerucut = revolve profil trapesium. Extrude dengan `normal` miring terhadap bidang sketsa → `Unsupported` (SolidCraft hanya tegak lurus). Revolve parsial: tanda sudut dikompensasi karena SolidCraft membalik arah bila `axis × radial` berlawanan normal sketsa (OCCT: tangan-kanan apa adanya). STEP/STL/cache native, `mirror_shape`, `write_step_compound`, `mass_properties` (inersia dari tesselasi halus), `check_clearance` (`body_distance`) sudah jalan di truck.
- **2.7** `lock_kernel()` truck = no-op; panggilan SolidCraft lewat `backend::truck::kernel()` (`guard`, panic → `Err`); `KernelShape: Send + Sync` di-assert saat kompilasi. Op yang belum diporting → `Err(ducad_kernel::Unsupported)`; `OpError::kernel` memetakannya ke kode `unsupported` + hint (terbukti lewat tool MCP `run_ops`).
- **Golden**: `golden check --ops a,b,…` (contoh dengan op lain dilaporkan "skip"); field `backend` dari `ducad_kernel::BACKEND`. CI: job `truck` (clippy + tes kernel truck, tanpa build OCCT) dan langkah "golden (backend truck, op F2)" di job `full` Linux.

Catatan untuk fase berikutnya:

- **Volume/luas** truck dari `measure()` (tesselasi halus, chord minimum 1e-4 mm): part sangat kecil bisa meleset > 1e-4 (ISO 4762 M3: 1,5e-4). Tes engine yang menuntut 1e-6 (sheet metal, ulir kosmetik) → dilonggarkan per backend di F5.1 sesuai §3.1.
- **Bbox** `inspect` kini dari `KernelShape::bounding_box()` (OCCT: bbox mesh, identik dengan sebelumnya; truck: tesselasi halus) karena vertex mesh tampilan SolidCraft tidak selalu di titik ekstrem lingkaran (seam revolusi sengaja di sudut "ganjil"). `fingerprint`, render, dan diff tetap memakai bbox mesh.
- Mesh tampilan truck lebih rapat (box 116 vs 20 segitiga; *display plan* SolidCraft membagi face datar). Sidik jari mesh hanya dicatat golden; ukur dampak render di F5.
- Gambar kerja di truck sudah jalan lewat jalur **mesh** (HLR eksak & potongan B-rep → F4; hook mengembalikan `None` + peringatan `HLR_EXACT_FALLBACK`).
- Jumlah edge bola/torus belum dibandingkan dengan OCCT (seam + edge kutub OCCT); tambahkan ke tes kontrak saat F3 menyentuhnya.
- F3 memetakan indeks DUCAD → titik SolidCraft lewat `LFace::members` / `LEdge::members` (sudah disimpan).

### F3 — Fitur B-rep lanjutan (3–4 minggu)

| Task | File | Validasi |
|---|---|---|
| 3.1 Fillet/chamfer: semua varian `modify.rs` + `advanced.rs` + `rounding.rs` di app (vertex, variable, by_index) | `backend/truck/modify.rs` | tes modul `modify` + `rounding.rs` tests |
| 3.2 Shell (`shell_open` dengan `ShellOpening::{Farthest,Rays}`), draft, split body/face | sama | tes `modify`, `gizmo.rs` tests |
| 3.3 Pattern linear/circular, mirror (KOMPOSISI; pakai `union_apart` saat tidak bersentuhan — jauh lebih cepat) | `backend/truck/advanced.rs` | tes `pattern`, `lines.rs` tests |
| 3.4 Helix, sweep path 3D, `create_helix_path_segments` (TETAP) | `backend/truck/helix.rs` | tes `helix` |
| 3.5 Hole (KOMPOSISI dari `sc:eval.rs:2892`), thread (§4.2), standard parts, sheet-metal (extrude+union — sudah tercakup F2) | `backend/truck/{hole,thread}.rs` | tes `hole`, `thread`, `sheet_metal`, `standard` |
| 3.6 Rib, extrude_face/revolve_face/resize_shape_along_edge, emboss | `backend/truck/modify.rs` | tes `operations.rs` |
| 3.7 Fuzz boolean: port `sc:fuzz.rs` → `tests/fuzz_boolean.rs`, 96 seed, golden hasil | `crates/ducad-kernel/tests/` | ≥ 80 % identitas volume (angka SolidCraft: 80,8 %) — **catat seed gagal sebagai known_failures, bukan blocker** |

**Exit gate F3**: 31/31 op `golden check` hijau atau masuk `known_failures.txt` dengan alasan; `evals/` semua contoh `get_schema` lolos `run_checks`.

**Status F3 (2026-10-19): selesai, dengan 1 contoh dilewati karena performa (R7 → F5).** Hasil gate:

| Gate | Hasil |
|---|---|
| `golden check --known tests/goldens/known_failures.truck.txt` (truck vs golden OCCT, korpus penuh, semua op) | **20/21 lulus** (18 identik, 2 dengan perbedaan topologi yang diterima), 1 dilewati: `valve_body` (R7, lihat bawah). Semua vonis check — termasuk studi statik/frekuensi/termal `pillow_block` — sama dengan OCCT |
| `golden check` penuh, backend OCCT | 21/21 (tidak berubah) |
| `cargo test -p ducad-kernel --lib`, truck (tes modul `tests.rs` kini jalan di KEDUA backend) | 221 lulus, 0 gagal, 13 diabaikan beralasan: 10 F4 (HLR eksak / tampak potongan B-rep), R2 (chamfer sudut campuran cekung+cembung), klaim ukuran BREP OCCT, + 1 lama |
| idem, OCCT | 259 lulus (tes yang menyentuh tipe OCCT dipindah ke `tests::occt_only`) |
| `SEEDS=0..95 cargo test --test fuzz_boolean` (truck) | **96/96 identitas volume (100 %)**, target ≥ 80 %; tidak ada known failure |
| `ducad-app` (truck): `rounding`, `lines`, `adapter_tests` | 49/49 |
| `cargo test -p ducad-engine --features kernel-truck -- --skip valve --skip drawing_golden_hashes --skip drawing_tool_ --skip hlr_cache` | **223 lulus, 0 gagal** (akhir F2: 143/84). Yang dilewati = 6 tes yang membangun `valve_body` (R7) — semuanya tes gambar F4; termasuk 1 tes baru berkas lintas-backend di `session_file` |
| idem tes engine yang tersentuh (`session_file`, `dfm`, `checks`, `compute`), OCCT | lulus |
| clippy `ducad-kernel` truck `-D warnings` | bersih |

Yang dibangun (`crates/ducad-kernel/src/backend/truck/`):

- **3.1 `modify.rs`** fillet/chamfer semua varian (all, ray, by_index, vertex, variable). Indeks/ray DUCAD → face/edge logis → **satu titik per anggota SolidCraft** (`LEdge::member_mids`, `LFace::member_points` baru di `topology.rs`; titik face diproyeksikan ke permukaan analitiknya). Seam tidak ikut "semua tepi". Radius variabel: per tepi lurus (`fillet_variable`); tepi lengkung → `Unsupported`. Chamfer yang ditolak sekaligus dicoba per tepi.
- **3.2** shell (`ShellOpening::{Indices,Rays,Farthest}`; kedalaman rongga = hollow + `half_space` + union), draft (`polyhedron::draft`), split body (`split_by_plane`, dipecah per lump) / dengan tool / split face. **Shell tebal variabel diimplementasikan** (bukan `Unsupported` seperti §4.3) untuk body konveks berface planar: rongga = polihedron konveks dengan offset per face; body lain → `Unsupported`.
- **3.3** pattern linear/melingkar & mirror sudah jalan sejak F2 (murni transformasi); `advanced.rs`: loft lintas bidang, draft & fillet variabel by index.
- **3.4 `helix.rs`** `sweep_helix` (gerak sekrup, dinding B-spline; ulir kiri = sumbu −k + pitch negatif); helix kerucut → `sweep_path` di sepanjang titik helix.
- **3.5** hole: `hole.rs` netral-backend sudah jalan; tambahan `apply_holes` (di truck semua pemotong digabung lalu SATU boolean). Thread: **§4.2 pilihan A** — `thread.rs` (truck) = `sweep_helix_mesh` 48 stasiun/putaran + `faceted_boolean`; boolean B-rep alur B-spline tidak selesai dalam hitungan menit (R4). Standard part & sheet metal lulus (komposisi F2).
- **3.6** rib = polihedron konveks dari setengah-ruang (juga prisma miring; draft = dua sisi panjang condong masuk); `extrude_face` = `copy_faces` + `thicken` + boolean (face lengkung: radius berubah); `revolve_face` (face planar; region dari edge logis, lingkaran eksak, `bridge::face_region`); `resize_shape_along_edge` (face di ujung AKHIR rusuk didahulukan); `make_filleted_box`. Emboss sudah jalan sejak F2.
- **3.7** `crates/ducad-kernel/tests/fuzz_boolean.rs`: port `sc:fuzz.rs` lewat API publik DUCAD (RNG & generator identik → seed sama dengan SolidCraft); `BLESS=1` menulis `tests/fuzz_boolean_known_failures.<backend>.txt`; CI menjalankan 24 seed.

Perbaikan di luar tabel yang ternyata perlu:

- **Bug SolidCraft** `surfaces::nearest_face` (dipakai `copy_faces`) mengukur jarak ke pusat/vertex segitiga → titik di tengah face atas kotak memilih face samping. Diperbaiki di vendor (jarak titik–segitiga), bertanda `DUCAD:`, dicatat di `vendors/solidcraft/README.md` (perubahan 4).
- **Boolean singgungan persis** (R1, `pillow_block`: silinder menyinggung sisi kotak + alas): truck gagal "no result". `csg::boolean` kini mengulang sekali dengan pemotong diskalakan 1e-5 **hanya pada sumbu tanpa face datar yang sebidang dengan target** (face berimpit tetap berimpit); sisa: 2 face serpihan 0,4 µm (dicatat di known failures).
- **Topologi logis**: (a) seam tidak dihitung saat menyambung busur satu lingkaran (lingkaran bor ter-chamfer = 1 edge seperti OCCT); (b) dua face B-spline yang menyambung mulus di ≥ 2 edge = satu face cincin + seam (tabung pegas 3 face/3 edge seperti OCCT); (c) busur B-spline berbentuk lingkaran dikenali lewat lingkaran 3-titik; (d) `face_boundary` face periodik berjalan lingkaran A → seam → lingkaran B (rentang aksial lubang terukur → check DFM `hole_web`).
- **Berkas lintas-backend** (penting untuk gate F1 "semua `.ducad` lama terbuka"): sidik jari `design` (hash volume 6 desimal) TIDAK PERNAH cocok antar-backend, sehingga di truck setiap berkas lama diam-diam kehilangan oplog-nya. Kini v4 menyimpan `design.geometry` (ringkasan nama/volume/bbox per body, `docs/FORMAT.md`); sidik jari beda tetapi ringkasan cocok dalam toleransi → replay dipakai. Berkas tanpa ringkasan: replay dibandingkan dengan body STEP-nya. `migrate-file`: toleransi volume 1e-4 di truck.
- **Massa** truck: chord tesselasi relatif radius face terkecil (berlantai relatif ukuran body) dan di-cache per shape — momen inersia silinder ramping kini < 1e-4.
- Check DFM `hole_web`: sinar radial yang lolos tepat di sambungan segitiga dilewati, tidak menggugurkan lubang.
- `golden check`: progres per contoh ke stderr; `--known <berkas>` (`sumber<TAB>awalan-perbedaan<TAB>alasan`, `*skip*` = lewati); toleransi bbox + satu chord mesh bila membandingkan backend lain dengan golden OCCT (bbox golden OCCT diambil dari mesh, meleset ke dalam sampai satu chord). CI: langkah golden truck kini korpus penuh + `--known`.
- Tes yang mengunci angka mesh/volume eksak OCCT dibuat netral-backend (volume, bukan jumlah vertex) atau bertoleransi per backend (§3.1: 1e-4 di truck). Chamfer semua tepi kubus: sudut dimitre di SolidCraft (7568) vs segitiga sudut di OCCT (7562,67) — keduanya sah, dikunci per backend.

Known failures / catatan untuk fase berikutnya:

- **`valve_body` (R7 → F5)**: tiap boolean lubang pada body berfillet ±1 menit (debug); replay penuh > 30 menit bahkan di build rilis. Waktu habis di tesselasi face berbatas kurva irisan (`presearch`, `double_projection`, trimming) yang diulang di `boolean_whole_with_refinement`. `encode_native_body` juga gagal untuk body ini ("intersection fit did not converge") → cache `brep` v4 terlewati (oplog tetap membangunnya). Tes engine yang membangun valve (`drawing_golden`, …) karenanya sangat lambat di truck.
- Loft persegi→lingkaran: OCCT 7 face/15 edge, SolidCraft 6/12 (pembagian sisi berbeda; volume sama).
- R2: chamfer sudut campuran cekung+cembung ditolak SolidCraft.
- Extrude miring terhadap bidang sketsa tetap `Unsupported` (F2).

### F4 — I/O, HLR, drawing (2 minggu)

| Task | File | Validasi |
|---|---|---|
| 4.1 STEP in/out via `step_import`/`step_export_bodies`; `import_worker` tanpa `KERNEL_LOCK`; `tooling.rs:1263` MCP `import_step` | `backend/truck/step.rs`, `ducad-app/src/import_worker.rs` | roundtrip STEP 20 contoh: volume ±1e-4, warna/nama terjaga |
| 4.2 HLR hybrid (§4.1) | `backend/truck/hlr_mesh.rs` | `drawing_sheet_smoke`, `sheet_p21`, `gdt_pdf`, `exploded_drawing`, golden SVG |
| 4.3 Section view (`half_space` + `planar_edges`) | `backend/truck/section.rs` | tes `section` |
| 4.4 Vector snapshot, detail view, PDF/DXF/SVG (TETAP) — hanya verifikasi | — | tes `ducad-io` |

**Exit gate F4**: seluruh Drawing Sheet (48 perintah) berfungsi di backend truck; export PDF/SVG/DXF dari `examples/` identik secara visual (golden hash dengan toleransi).

### F5 — Sim, checks, MCP, assembly, paralel (1–2 minggu)

| Task | File | Validasi |
|---|---|---|
| 5.1 `sim.rs` `surface_mesh` dari `face_ranges` hasil sort; `exact_volume_mm3` dari `measure()`; toleransi checks `volume`/`mass` 5e-4 | `ducad-engine/src/sim.rs`, `check/run.rs` | tes `ducad-sim`, `evals/` simulate |
| 5.2 `interference`, `clearance`, `min_wall` — ganti `intersect` internal; opsi `body_distance` eksak | TETAP + 1 baris | tes `interference` |
| 5.3 MCP: `get_schema` examples, `run_ops` diagnostics tambah `warnings[]` (boolean strategy) | `ducad-mcp`, `ducad-engine/src/tooling.rs` | `evals/` penuh |
| 5.4 Hapus `KERNEL_LOCK`, `lock_kernel()` dari backend truck; paralelkan `place_solids_with_material` tool merge dengan `rayon` (pola `sc:par.rs`) | `lib.rs`, `session.rs` | tes konkurensi: import STEP + extrude bersamaan |
| 5.5 Proposal/diff/undo-redo (`diff.rs:458 clone_shape`) | verifikasi | tes `ducad-engine` |

**Exit gate F5**: `cargo test --workspace --features kernel-truck` hijau; `golden check` hijau; benchmark ≤ 2× baseline OCCT untuk 90 % op (SolidCraft dikenal lambat pada pattern besar — ukur, jangan asumsikan).

**Status F5 (2026-10-19): kode selesai, gate BELUM hijau** (dikerjakan paralel dengan F3/F4 yang belum selesai; gate bergantung keduanya).

| Task | Hasil |
|---|---|
| 5.1 | `ducad_kernel::VOLUME_REL_TOL` (OCCT 0, truck 5e-4). Check `volume` (expect ± `tol_pct` dan min/max) dan `mass` memberi kelonggaran relatif sebesar itu → perilaku OCCT tidak berubah. `sim.rs` sudah memakai `face_ranges` terurut (bridge F2) dan `exact_volume_mm3 = volume()`; tidak perlu diubah. Tes engine yang membandingkan volume dengan rumus analitik (sheet metal ×3, ulir kosmetik, standard part M3) memakai `max(1e-6, VOLUME_REL_TOL)`; perbandingan replay-vs-replay tetap ketat (truck deterministik). |
| 5.2 | Tanpa perubahan: `check_clearance` truck sudah `body_distance` eksak (F2), `detect_interference` memakai `intersect` backend. |
| 5.3 | Modul baru `ducad_kernel::diagnostics` (`capture_kernel_warnings`, `warn`; lingkup per thread, bersarang, maks 32, tanpa duplikat). Boolean truck membungkus `capture_boolean_diagnostics` SolidCraft dan meneruskan `BooleanDiagnostics::warning()` (hanya strategi yang bisa mengubah hasil: `ToolNudge`, `UnverifiedShortcut`) + percobaan ulang skala 1e-5 milik DUCAD sebagai `boolean_retry: …`. `Session::run` membungkus tiap op → `outcomes[i].warnings`. Deskripsi tool MCP `run_ops` menjelaskan `boolean_retry`. |
| 5.4 | `lock_kernel()` truck = no-op sejak F2 (tidak ada mutex); `ducad_kernel::THREAD_SAFE`. `ducad_kernel::union_all` (OCCT: lipatan berurutan; truck: `union_apart` tanpa boolean bila bbox terpisah, selain itu reduksi pohon berpasangan paralel lewat `map_with_local_pools` SolidCraft, peringatan pekerja dicatat ulang di thread pemanggil) dipakai `place_solids_with_material` mode add/cut. Tes `ducad-kernel/tests/truck_concurrency.rs`: 6 thread import STEP + boolean bersamaan pada shape bersama = hasil berurutan; `union_all` = lipatan biasa. Komentar `import_worker.rs` diperbarui. |
| 5.5 | Tes `diff`, `edit` (undo/redo), `replay`, `session` hijau di truck. |

Hasil gate saat ini:

- `cargo test -p ducad-engine --features kernel-truck`: semua berkas tes hijau **kecuali** `drawing_golden` (tidak selesai > 30 menit: valve_body, lihat di bawah) dan `session_file` (sedang diubah sesi F3, belum kompil). `--lib` 81/81.
- `cargo test -p ducad-kernel` (OCCT) 260/260; tes truck baru 2/2 + unit `diagnostics`.
- `cargo clippy -D warnings` OCCT (kernel, engine, mcp, app): bersih. Clippy truck terhalang satu galat di `backend/truck/hlr_mesh.rs` (pekerjaan F4 yang sedang berjalan).
- **Terbuka (R7, blocker gate F5)**: `valve_body` di truck — tiap boolean lubang pada body ber-fillet ±59 s (debug), replay golden > 30 menit (release). Waktu habis di tesselasi truck untuk face berbatas kurva irisan (`intersection_curve::double_projection`, `trimming_with_steps`) yang diulang di `boolean_whole_with_refinement` / `refined_with_cache`. Juga `encode_native_body` gagal pada body yang sama (cache v4 dilewati). Saat ini `*skip*` di `known_failures.truck.txt` (F3). Perlu perbaikan di vendor SolidCraft (cache tesselasi antar-percobaan atau toleransi kasar untuk klasifikasi) sebelum benchmark ≤ 2× bisa diukur.
- Benchmark ≤ 2× OCCT belum diukur (`cargo bench -p ducad-kernel --features kernel-truck`), menunggu R7 di atas dan F3/F4 selesai.

### F6 — Platform, cutover, pembersihan (1 minggu + soak 2 minggu)

| Task | File | Validasi |
|---|---|---|
| 6.1 Build matrix: macOS, iPad (`build_ipad.sh` tanpa langkah OCCT), Android (hapus `android-toolchain.cmake`), Windows (`build_windows.ps1`) | skrip build | artefak terbentuk; ukuran binary dicatat vs baseline |
| 6.2 `wasm32-unknown-unknown`: `cargo check -p ducad-kernel --features kernel-truck --target wasm32-unknown-unknown` (rayon di-cfg seperti `sc:par.rs`) | CI job manual | hijau |
| 6.3 Rilis beta `0.6.0-beta` dengan **feature flag runtime** (`DUCAD_KERNEL=occt|truck`, default truck) — soak 2 minggu, laporan crash via `ducad-update` | `ducad-app/main.rs` | tidak ada crash kernel di telemetri |
| 6.4 Flip default feature → `kernel-truck`; hapus `backend/occt`, `vendors/opencascade-rs`, `clean_no_occt.sh`, bagian OCCT di `docs/{PANDUAN,TABLET,PACKAGING,PLAN}.md`, `ANALISIS_KOMPARATIF_CAD.md:294` (WASM) | banyak | `grep -ri occt` hanya di CHANGELOG |
| 6.5 Rilis `0.6.0`; bump pointer submodule sebelum tag (lihat memori "release tag vs submodule pointer") | `scripts/release` | tag + build |

**Exit gate F6**: semua gate §0 hijau; `vendors/opencascade-rs` tidak ada; CI tanpa langkah cmake.

---

## 6. Validasi (perintah)

```bash
# Kedua backend selama transisi
cargo test -p ducad-kernel                                             # OCCT (default)
cargo test -p ducad-kernel --no-default-features --features kernel-truck
cargo test --workspace --features kernel-truck
cargo clippy --workspace --features kernel-truck -- -D warnings
cargo test -p ducad-ui --test ui_glyph_coverage                        # aturan global ikon

# Golden & fuzz
cargo run -p ducad-cli -- golden check --against tests/goldens/occt --tol 1e-3
SEEDS=0..95 cargo test -p ducad-kernel --features kernel-truck --test fuzz_boolean

# Drawing
cargo test -p ducad-ui --test drawing_sheet_smoke --features kernel-truck
cargo test -p ducad-io --test sheet_p21 --test gdt_pdf --test exploded_drawing --features kernel-truck

# Platform
./build_ipad.sh check && ./build_macos.sh
cargo check -p ducad-kernel --no-default-features --features kernel-truck --target wasm32-unknown-unknown

# Bench
cargo bench -p ducad-kernel --features kernel-truck -- --save-baseline truck
```

---

## 7. Risiko & mitigasi

| # | Risiko | Kemungkinan | Dampak | Mitigasi |
|---|---|---|---|---|
| R1 | **Boolean gagal pada konfigurasi lengkung berimpit** (SolidCraft fuzz: 53,8 % clean, 27,1 % gagal pada desain acak; silinder sama-radius bersilangan ditolak) | Tinggi | Tinggi | Golden corpus dari pekerjaan nyata (bukan fuzz) adalah ukuran sebenarnya; `known_failures.txt`; `Err` + `fixes` ke agent MCP; backend OCCT tetap tersedia sampai soak selesai |
| R2 | Fillet/chamfer antara dua face lengkung atau berakhir di face non-planar **ditolak** (`sc:blend_corner.rs` ±30 `unsupported`) | Tinggi | Sedang | UI tampilkan alasan; `fillet_max` SolidCraft memberi radius maksimum yang masih bisa; dokumentasikan di `get_schema` |
| R3 | Shell non-konveks / lebih tebal dari dinding ditolak | Sedang | Sedang | hint "kurangi ketebalan"; variable thickness tidak didukung (§4.3) |
| R4 | Thread jadi facet (bukan B-rep halus) | Pasti | Rendah | §4.2; STEP tetap valid; dimensi ulir untuk drawing dari parameter, bukan geometri |
| R5 | `volume()`/`centroid` ±1e-4 (mesh) vs eksak | Pasti | Rendah | toleransi checks 5e-4; `exact_volume` dari tiga tessellasi konvergen (`sc:measure.rs`) |
| R6 | HLR mesh: garis tersembunyi sedikit meleset pada siluet lengkung (chord 0,01 mm) | Pasti | Rendah–Sedang | fitur lingkaran/busur tetap eksak (§4.1); uji visual golden SVG |
| R7 | Performa: pattern besar & fillet rantai lambat (SolidCraft `seed_81` 20 menit di CI) | Sedang | Sedang | `union_apart` untuk pattern tak bersentuhan; benchmark gate F5 ≤ 2×; `watch` cancel kooperatif (`sc:watch.rs`) agar UI tidak beku |
| R8 | **SolidCraft sedang mengganti kernelnya dengan "solidcore"** (`sc:CONTRIBUTING.md:347`) — upstream bisa berhenti merawat `crates/kernel` | Sedang | Sedang | vendor via `git subtree` dengan SHA tercatat; kita pemilik fork; pantau upstream per kuartal |
| R9 | STEP export menolak beberapa revolusi berskala non-seragam (`Scale` op) | Rendah | Rendah | `scale_shape` non-seragam → warning; export fallback mesh-STEP (facet) |
| R10 | Panic = abort di wasm (tidak ada `catch_unwind`) | Pasti (wasm) | Sedang | wasm hanya `cargo check` di plan ini; produk web = fase terpisah |
| R11 | Lisensi: Apache-2.0 §4(b) wajib notice file yang diubah | Pasti | Rendah | salin `vendor/README.md`, marker `SolidCraft:`, `NOTICE`; tambah ke `docs/ATTRIBUTION.md`; `deny.toml` |
| R12 | Edition 2024 / rustc ≥ 1.95 | Rendah | Rendah | sudah terpenuhi (1.96.1); pin di F0 |

---

## 8. Yang jadi lebih baik setelah migrasi

- Build bersih: 8–15 menit kompilasi C++ → ~0; tanpa cmake/NDK/iOS toolchain khusus.
- `KernelShape: Send + Sync`; `KERNEL_LOCK` hilang; import STEP dan editing paralel.
- Target `wasm32` terbuka (DUCAD web + MCP di browser).
- Satu bahasa (Rust) di seluruh stack; `#![forbid(unsafe_code)]` di kernel.
- Boolean deterministik (patch `integrate/mod.rs` SolidCraft) → oplog replay reproducible → diff/undo lebih bisa dipercaya.
- History face/edge dari boolean (`and_with_history`) → fondasi **topological naming** untuk selector (`of(>Z)` tetap valid setelah regenerasi) — bonus yang tidak ada di jalur OCCT sekarang.

---

## 9. Estimasi

| Fase | Durasi | Catatan |
|---|---|---|
| F0 Baseline | 1 mg | |
| F1 Format file | 1 mg | bisa paralel dengan F2.1–2.3 |
| F2 Kerangka + op dasar | 2–3 mg | |
| F3 Fitur B-rep | 3–4 mg | terbesar; fillet/shell adaptasi semantik |
| F4 I/O + HLR | 2 mg | HLR = bagian "TULIS" terbesar |
| F5 Sim/MCP/paralel | 1–2 mg | |
| F6 Platform + soak + cutover | 1 mg + 2 mg soak | |
| **Total** | **11–14 mg kerja + 2 mg soak** | 1 engineer; 2 engineer → ±8 mg (F3 ∥ F4) |

---

## 10. Acceptance

- [ ] Gate F0–F6 hijau di CI (kedua backend sampai F6.4)
- [ ] `golden check` 100 % atau `known_failures.txt` beralasan dan disetujui
- [ ] Semua `.ducad` ≤ 0.5.6 terbuka (korpus `examples/` + sampel pengguna)
- [ ] 354 perintah UI: tidak ada yang hilang; yang `Unsupported` ditampilkan dengan alasan, bukan tombol mati diam-diam
- [ ] `docs/` diperbarui; `ATTRIBUTION`/`NOTICE` lengkap; `deny.toml` lolos
- [ ] Rilis 0.6.0 tanpa `vendors/opencascade-rs`

---

## Lampiran A — Angka referensi SolidCraft (untuk kalibrasi ekspektasi)

- Kernel: 779 tes unit + 32 integrasi + 30 `#[ignore]`; 1 blok `proptest` identitas boolean.
- Robustness: 96 seed golden; fuzz desain acak 53,8 % clean / 27,1 % gagal / 18 % > 90 s; identitas boolean 80,8 %.
- Oracle: 84 kasus vs FreeCAD (`tests/oracle/`).
- Refusal eksplisit `"not supported yet"` di kernel: 49 lokasi + ±30 di `blend_corner.rs`.
- Vendor patch: truck-shapeops 31 marker, truck-meshalgo 47, truck-geometry 7, truck-stepio 5, matext4cgmath 3.

## Lampiran B — Perbedaan semantik yang harus diingat saat porting

| OCCT (DUCAD sekarang) | SolidCraft/truck |
|---|---|
| Edge/face dipilih by index enumerasi `TopExp` | by titik (`Vec3` di tengah edge / di face) |
| `Compound` multi-body | `Body` multi-shell (`lumps()`) |
| `BRepGProp` eksak | `measure()` mesh konvergen ±1e-4 |
| `boolean` selalu `Result<Shape>` | `Result<Option<Body>>` — `None` = kosong, bukan error |
| Face periodik silinder = 1 face dengan seam | SolidCraft split jadi 2 face (`step_in/split.rs`) → `merged_topology()` untuk hitungan face yang setara |
| `ShapeFix` healing eksplisit | `heal` privat otomatis; `validity()` hanya lapor |
| Thread-safe: tidak | ya |
