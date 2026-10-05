# P16–P20 — DUCAD menuju CAD skala industri: simulasi, sheet metal, GD&T, konfigurasi

Status: **rencana, belum dikerjakan**. Disusun 2026-10-05 dari audit kode
`ducad-editor/crates/*` (bukan dari dokumen pemasaran). Mengikuti konvensi
`.claude/plan/ducad-agent-harness/00-konvensi.md`: setiap fase punya
"apa yang dibuat", "detail teknis", dan "gate testing" yang harus hijau
sebelum fase berikutnya dimulai.

Konteks produk: pembanding utama adalah **SolidWorks Standard/Professional +
SolidWorks Simulation**. Dokumen pembanding lama (`docs/ANALISIS_KOMPARATIF_CAD.md`)
membandingkan dengan AutoCAD/Shapr3D dan tidak membahas simulasi; dokumen
ini melengkapinya, tidak menggantikannya.

---

## 0. Baseline DUCAD saat ini (yang sudah ada di kode)

| Area | Bukti di kode |
|---|---|
| 19 operasi solid | `ducad-engine/src/ops/spec.rs` — Sketch, Extrude, Revolve, Primitive, Boolean, Fillet, Chamfer, Shell, Hole, Pattern, Transform, Delete, Loft, Sweep, Helix, Draft, Mirror, Scale, Split |
| Hole wizard ISO | `ducad-core/src/hole.rs` — `HoleKind`, `IsoMetricThread`, `HoleSpec` |
| Volume / luas / centroid eksak | `ducad-kernel/src/shape.rs` — `KernelShape::volume` (BRepGProp), `surface_area`; `inspect.rs::BodyReport { volume, area, centroid, mass_g }` |
| Densitas material (4 nilai) | `ducad-core/src/lib.rs:143` — `MaterialPreset::density_g_cm3` |
| Checks sebagai unit test desain | `ducad-engine/src/check/types.rs` — `Valid, BodyCount, Volume, BboxSize, BboxMax, Mass, MinWall, HoleCount, Clearance, NoInterference` |
| Tebal dinding minimum (mesh) | `ducad-kernel/src/thickness.rs` — `min_wall_thickness`, `ray_hit_distance` |
| Assembly + mate + motion + clash | `ducad-core/src/assembly.rs` (`AssemblyTree`, `MateConstraint`, `JointKind`, `MotionStudy`, `ClashReport`, `BomRow`), `ducad-kernel/src/assembly_solver.rs`, `interference.rs` |
| Parametrik | `ducad-core/src/parametric.rs` (`ParametricDag`), `set_params`, oplog replay, `diff.rs` |
| Drawing | `hlr.rs`, `hlr_exact.rs`, `section.rs` (arsir ISO), `detail.rs`, `ducad-io/src/pdf.rs` |
| Analisis visual GPU | `ducad-render/src/shader.wgsl` — zebra, draft-angle heatmap, SSAO |
| Agent | MCP 26 tool, `ducad-cli build` deterministik dengan exit code 3 bila checks gagal |

Yang **belum ada** dan dibutuhkan industri: tensor inersia, pustaka material
mekanik (E, ν, σ_y), FEA (statik, frekuensi, buckling, termal), sheet metal,
GD&T/toleransi di drawing, konfigurasi varian/design table, thread fisik,
toolbox fastener, mate lanjutan (gear/screw/path), exploded view.

---

## 1. Perbandingan DUCAD vs SolidWorks

Legenda: ✅ setara · 🟡 sebagian · ❌ belum ada · ⭐ DUCAD lebih unggul

| Fitur | SolidWorks | DUCAD | Fase |
|---|---|---|---|
| Sketch parametrik + constraint | ✅ (D-Cubed) | ✅ solver LM mandiri, 13 constraint | — |
| Extrude/Revolve/Loft/Sweep/Helix | ✅ | ✅ | — |
| Fillet variabel, chamfer, shell, draft, rib, split | ✅ | ✅ | — |
| Hole wizard ISO (counterbore, countersink, tapped) | ✅ | ✅ | — |
| Thread fisik (cosmetic → geometri ulir) | ✅ | 🟡 cosmetic saja | P20 |
| Surfacing NURBS (knit, trim, thicken) | ✅ | ❌ | di luar P16–P20 |
| Sheet metal (flange, bend, flat pattern, k-factor) | ✅ | ❌ | P19 |
| Weldments / profil struktural | ✅ | ❌ | P20 |
| Feature tree editable asosiatif | ✅ | 🟡 `ParametricDag`, blocker P0.4 `BRepTools_History` | P0.4 (harness) |
| Konfigurasi / design table | ✅ | ❌ (`set_params` baru satu set) | P19 |
| Assembly mate dasar | ✅ | ✅ coincident/concentric/angle/limits | — |
| Mate lanjutan (gear, screw, cam, path) | ✅ | ❌ | P20 |
| Interference / clearance | ✅ | ✅ `Clearance`, `NoInterference` | — |
| Exploded view, BOM balloon | ✅ | 🟡 `BomRow` ada, exploded ❌ | P20 |
| Drawing: proyeksi, section, detail, PDF | ✅ | ✅ | — |
| GD&T ISO 1101, fit ISO 286, surface finish | ✅ | ❌ | P19 |
| **Mass properties lengkap (inersia, sumbu utama)** | ✅ | 🟡 volume/centroid/massa saja | **P16** |
| **Pustaka material mekanik** | ✅ | 🟡 4 densitas | **P16** |
| **FEA statik linier (gaya, tekanan, torsi, gravitasi)** | ✅ Simulation | ❌ | **P17–P18** |
| **Faktor keamanan, von Mises, deformasi** | ✅ | ❌ | **P17** |
| Frekuensi natural, buckling | ✅ Professional | ❌ | P18 |
| Termal steady-state | ✅ Professional | ❌ | P18 |
| Fatigue, nonlinier, CFD, motion dinamik penuh | ✅ Premium/Flow | ❌ | di luar cakupan |
| Checks sebagai unit test desain, CI build deterministik | ❌ | ⭐ | — |
| Kontrol agent penuh lewat MCP/oplog | ❌ | ⭐ | — |
| Offline, format terbuka, iPad native | 🟡 | ⭐ | — |

Fokus dokumen ini: baris tebal (simulasi) sebagai inti, lalu P19–P20
untuk menutup celah fitur industri non-simulasi.

---

## 2. Arsitektur yang ditambahkan

```
ducad-core ──► ducad-kernel ──► ducad-sim ──► ducad-engine ──► ducad-cli / ducad-mcp / ducad-app
   │  MechanicalProperties     │  mesh FE, assembling K,   │  Op::Fixture/Load,
   │  SimSetup (serde)          │  solver CG, post-proses   │  SimReport, Check::MaxStress…
   └─ tanpa kernel              └─ tanpa OCCT, tanpa GUI    └─ tanpa GUI (tes sudah ada)
```

Aturan tambahan (ikut "Aturan keras" CLAUDE.md):

- **`ducad-sim` tidak boleh `use opencascade`** dan tidak boleh bergantung pada
  egui/wgpu. Masukan dari kernel hanya `KernelMesh` dan hasil `select_faces`
  (indeks face). Dijaga tes `sim_has_no_kernel_or_gui_dependency` yang
  membaca `Cargo.toml` seperti `engine_has_no_gui_dependency`.
- Semua angka FEA `f64`; mesh render tetap `f32`.
- Tidak ada dependensi mesher luar: **Netgen (LGPL) dan TetGen (AGPL) tidak
  lolos `deny.toml`**. Mesher ditulis sendiri (hex voxel di P17, tet di P18).
- Solver linier: CG sparse dengan prekondisi Jacobi, implementasi sendiri
  (CSR). `faer` (MIT) boleh dipakai untuk eigen padat kecil di P18 bila
  perlu; wajib lolos `cargo deny check licenses`.
- Komputasi berat berjalan di thread latar dengan `CancelToken`, pola yang
  sama dengan `min_wall` di `ducad-app/src/checks_ui.rs`.
- Teks yang dibaca agent (deskripsi tool/Op/Check, `ERROR_GUIDE`) **bahasa
  Inggris**; pesan error inti engine dan UI bahasa Indonesia.

---

## P16 — Mass properties lengkap + pustaka material mekanik

### Apa yang dibuat
Panel "Properti Massa" setara SolidWorks *Mass Properties*: massa, volume,
luas, pusat massa, tensor inersia terhadap origin dan terhadap pusat massa,
momen & sumbu utama, radius girasi. Untuk assembly: agregat multi-material.
Pustaka material dengan sifat mekanik yang menjadi masukan FEA P17.

### Detail teknis

**Kernel (`ducad-kernel`)**
- Binding cxx baru di `vendors/opencascade-rs/crates/opencascade-sys`
  (pola P2.7 — tambah fungsi wrapper, **tanpa** mengubah versi/fitur OCCT):
  `BRepGProp::VolumeProperties(shape, props)` lalu `props.MatrixOfInertia()`,
  `props.CentreOfMass()`, `props.Mass()` (dengan densitas 1 → volume).
- `shape.rs`:
  ```rust
  pub struct MassProperties {
      pub volume_mm3: f64,
      pub centroid: [f64; 3],
      /// Tensor inersia geometris (densitas 1) terhadap origin, mm^5.
      pub inertia_origin: [[f64; 3]; 3],
  }
  pub fn mass_properties(&self) -> MassProperties  // let _guard = lock_kernel();
  ```
  Translasi ke pusat massa (teorema sumbu sejajar) dan eigen-dekomposisi
  3×3 (Jacobi iteratif, tanpa dependensi) dilakukan di luar lock, di helper
  murni `pub(crate) fn principal_axes(tensor) -> ([f64;3], [[f64;3];3])`.

**Core (`ducad-core/src/lib.rs` → pindah ke `material.rs`)**
```rust
pub struct MechanicalProperties {
    pub density_g_cm3: f64,
    pub young_modulus_gpa: f64,
    pub poisson_ratio: f64,
    pub yield_strength_mpa: f64,
    pub ultimate_strength_mpa: f64,
    pub thermal_expansion_per_k: f64,
    pub thermal_conductivity_w_mk: f64,
}
pub enum MaterialSource { Preset(MaterialPreset), Library(String), Custom(MechanicalProperties) }
```
- Tabel awal (nilai referensi umum, disebut sumbernya di doc comment):
  ABS, PA6, PC, Al 6061-T6, Al 7075-T6, S235, S355, AISI 304, AISI 1045,
  Ti-6Al-4V, kuningan, tembaga, kaca soda-lime.
- `Material` (visual, PBR) dan `MechanicalProperties` dipisah; satu body
  menyimpan keduanya. `density_g_cm3()` lama tetap ada sebagai jalur
  kompatibel; `Check::Mass` memakai `MechanicalProperties.density_g_cm3`
  bila ada.
- `Document` menyimpan `material_library: Vec<(String, MechanicalProperties)>`
  untuk material kustom per file; serde `#[serde(default)]` agar `.ducad`
  lama tetap terbaca.

**Engine (`ducad-engine`)**
- `inspect.rs::BodyReport` ditambah:
  `center_of_mass`, `inertia_com: [[f64;3];3]` (g·mm²), `principal_moments`,
  `principal_axes`, `radius_of_gyration`, `mechanical: Option<MechanicalReport>`.
- `inspect` untuk assembly: `AssemblyMassReport { total_mass_g, center_of_mass, inertia_com }`
  dijumlahkan per instance dengan transformasi mate.
- `Op::SetMaterial { body, material: MaterialSource }` — op baru (ikuti 7
  langkah "Menambah operasi baru"); menggantikan pengaturan material yang
  sekarang hanya di GUI sehingga oplog dan agent bisa memilih material.
- Checks baru di `check/types.rs`:
  `CenterOfMass { body, expect: [Num;3], tol: f64 }`,
  `MomentOfInertia { body, axis: "x"|"y"|"z"|"principal_min"|…, min/max }`.
- `tooling::ERROR_GUIDE` + skill `ducad-modeling/SKILL.md` + `schema/ops.schema.json`.

**GUI (`ducad-app`, `ducad-ui`)**
- `ducad-ui/src/mass_properties_panel.rs`: tabel nilai, pilihan satuan
  (g/kg, mm/cm/m), tombol salin. Penanda pusat massa di viewport (bola kecil
  + tiga sumbu utama, warna berbeda) lewat overlay gizmo yang ada.
- Pemilih material mekanik di panel properti body (dropdown pustaka + "kustom").
- Ikon: `egui_icons::icons::ICON_*`, jangan glyph Unicode (aturan global).

**MCP/CLI**
- `inspect` otomatis memuat field baru. `ducad-cli inspect --mass` mencetak
  tabel. Tool count tidak berubah.

### Gate testing P16
| Tes | Kriteria lulus |
|---|---|
| `cargo test -p ducad-kernel mass_properties` | Balok 10×20×30: I terhadap COM = ρV(b²+c²)/12 dsb., selisih < 1e-6 relatif; bola: tiga momen utama sama; silinder: sumbu utama sejajar sumbu silinder |
| `cargo test -p ducad-core material` | Setiap entri pustaka: E>0, 0<ν<0.5, σ_y ≤ σ_ult; `.ducad` lama tanpa `mechanical` tetap terbaca |
| `cargo test -p ducad-engine inspect_mass` | Fixture `tests/fixtures/mass_bracket.ops.json`: `inertia_com` simetris, positif-definit; assembly dua body berbeda material → COM gabungan sesuai rumus |
| `cargo test -p ducad-engine schema_file` | Skema diperbarui (`DUCAD_UPDATE_SCHEMA=1`) dan berisi `set_material`, `center_of_mass`, `moment_of_inertia` |
| `cargo test -p ducad-mcp` | Tes `instructions`/tool count tidak berubah; `inspect` membawa `inertia_com` |
| `cargo test -p ducad-ui --test ui_glyph_coverage` | Semua teks panel baru + terjemahan `en-US`/`id-ID` terender |
| `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check licenses` | Hijau |
| Manual GUI | Penanda COM bergerak saat body digeser; nilai sama dengan CLI untuk file yang sama |

---

## P17 — Simulasi statik linier: mesh hex voxel + solver + heatmap

### Apa yang dibuat
Studi statik ala *SolidWorks Simulation → Static*: pilih face tumpuan
(fixture), pasang beban (gaya, tekanan, torsi, gravitasi, bearing),
jalankan, lihat von Mises, deformasi, faktor keamanan, gaya reaksi. Hasil
masuk `inspect`, checks, `ducad-cli build`, dan MCP. Tahap ini memakai
**mesh heksahedral voxel** (akurasi "estimasi rekayasa", ±10 %) agar
pipeline ujung-ke-ujung ada dulu; mesh tetra konform menyusul di P18.

### Detail teknis

**Crate baru `ducad-sim`** (`crates/ducad-sim`, → core, kernel hanya untuk tipe `KernelMesh`):
```
src/
  lib.rs           // pub use; SimError
  setup.rs         // SimSetup, Fixture, Load, LoadKind (serde + JsonSchema)
  mesh/voxel.rs    // voxelisasi KernelMesh → HexMesh
  mesh/hex.rs      // HexMesh { nodes: Vec<[f64;3]>, elems: Vec<[u32;8]>, face_tags }
  element/hex8.rs  // matriks kekakuan 8-node trilinear, integrasi Gauss 2×2×2
  assemble.rs      // CSR K global, pemetaan DOF, penerapan BC (penalty / eliminasi)
  solver/cg.rs     // PCG Jacobi, toleransi relatif 1e-8, iterasi maks, CancelToken
  post.rs          // regangan/tegangan per titik Gauss → nodal (rata-rata), von Mises, SF
  report.rs        // SimReport
  benchmark/       // kasus analitik untuk tes
```

**Setup (serde, dibaca agent)**
```rust
pub struct SimSetup {
    pub body: String,                       // nama body (id op pembuat)
    pub fixtures: Vec<Fixture>,             // { id, faces: selector, kind: Fixed | Roller{normal} | Symmetry }
    pub loads: Vec<Load>,                   // { id, faces: selector, kind }
    pub mesh: MeshSettings,                 // { kind: Hex{cell_mm} | Tet{..}, target_elems: Option<usize> }
}
pub enum LoadKind {
    Force { newton: [f64;3] },              // total pada face, dibagi merata per luas
    Pressure { mpa: f64 },                  // normal face, positif = menekan
    Torque { axis_point: [f64;3], axis_dir: [f64;3], newton_mm: f64 },
    Gravity { g: f64, dir: [f64;3] },       // body load, memakai densitas P16
    Bearing { newton: [f64;3] },            // distribusi cosinus pada face silinder
    Remote { point: [f64;3], newton: [f64;3] }  // gaya di titik jauh → gaya + momen ekuivalen
}
```
Selector face memakai gramatika yang sudah ada (`>Z`, `all[kind=cylinder][r=5]`),
diselesaikan di engine lewat `select::select_faces`, lalu `KernelMesh.face_ranges`
memetakan face → segitiga → node voxel permukaan.

**Voxelisasi (`mesh/voxel.rs`)**
1. AABB body, ukuran sel `cell_mm` (default: bbox_max_dim/40, dibatasi 20k–200k sel aktif).
2. Klasifikasi pusat sel dalam/luar: parity ray cast sepanjang +X memakai
   `ducad_kernel::thickness::ray_hit_distance` yang sudah ada (dipanggil
   lewat trait agar `ducad-sim` tidak memegang OCCT), dengan jitter 3 sinar
   untuk kasus degenerasi.
3. Sel permukaan diberi `face_tag` = face B-rep terdekat (lewat segitiga `face_ranges`),
   sehingga fixture/load menempel pada face, bukan pada sel sembarang.
4. Koreksi volume: skala densitas elemen permukaan agar Σ volume voxel =
   `KernelShape::volume` (eksak), menjaga massa dan gravitasi benar.

**Elemen & assembling**
- Hex8 trilinear, matriks elastisitas isotropik dari `E, ν` (P16).
- DOF = 3 per node; K global CSR simetris, hanya segitiga atas disimpan.
- Fixture `Fixed`: eliminasi baris/kolom. `Roller`/`Symmetry`: penalty besar
  pada arah normal (dokumentasikan skalanya = 1e6·maks diag).
- Beban face: gaya dibagi per luas segitiga face ke node-node sel permukaan.

**Solver**
- PCG Jacobi, konvergensi `‖r‖/‖b‖ < 1e-8` atau iterasi maks 20·√n.
- Deteksi kekurangan tumpuan: bila K singular (CG diverge / tidak ada fixture),
  `SimError::Underconstrained { free_dofs_hint }` dengan `hint` berbahasa
  Inggris di tingkat tool (contoh: "add a Fixed fixture on at least one face").
- `CancelToken` dicek setiap 50 iterasi.

**Post-proses**
- Tegangan di titik Gauss → ekstrapolasi nodal rata-rata; von Mises;
  deformasi total; **faktor keamanan** `SF = σ_y / σ_vm_max`; gaya reaksi per fixture.
- `SimReport { max_von_mises_mpa, location, max_displacement_mm, safety_factor,
  reactions: Vec<{fixture_id, force_n: [f64;3]}>, mesh_stats, solver_stats,
  nodal_field: Option<Field> }`. `nodal_field` ditinggalkan dari JSON MCP
  secara default (besar), tersedia lewat `render_view`.

**Engine**
- `ducad-engine/src/sim.rs`: `run_static(session, SimSetup) -> Result<SimReport>`.
- `Op::Study { id, kind: "static", setup: SimSetup }` — disimpan di oplog agar
  studi ikut versi desain dan bisa di-replay; **tidak membuat body**, hasilnya
  cache per tanda tangan model (hash oplog + setup).
- Checks: `MaxStress { study, max_mpa }`, `MaxDisplacement { study, max_mm }`,
  `MinSafetyFactor { study, min }`. `Error` bila study belum dijalankan/gagal.
- `ducad-cli build`: menjalankan semua `Op::Study`, menulis `sim/<id>.json`
  + PNG heatmap ke `report.md`; check gagal → exit 3 (perilaku yang sudah ada).
- `ducad-cli sim PART.ducad --study ID [--json]` untuk pemakaian langsung.

**MCP (`ducad-mcp`)**
- Tool baru `simulate_static { session?, setup }` (readOnly: false karena
  menulis cache; bukan destructive) → `SimReport` ringkas + PNG heatmap.
- `render_view` mendapat `overlay: "stress" | "displacement" | "safety_factor"`
  dan `deform_scale`.
- Perbarui `instructions`, `ducad://guide`, tes jumlah tool (26 → 27; `--attach` ikut),
  `ERROR_GUIDE` (kode `SIM_UNDERCONSTRAINED`, `SIM_NO_MATERIAL`, `SIM_MESH_TOO_COARSE`,
  `SIM_DIVERGED`).

**Render & GUI**
- `ducad-render`: atribut vertex skalar + uniform `overlay_mode`; colormap
  viridis/turbo di `shader.wgsl` (pola draft-angle heatmap). Deformasi
  divisualkan dengan menggeser posisi vertex render (bukan mengubah B-rep).
- `ducad-app/src/sim_ui.rs`: panel "Simulasi" — daftar studi, pemilih face
  dengan klik (memakai picking ray yang ada → selector tersimpan), tabel beban,
  tombol Jalankan, progres, legenda colormap, slider skala deformasi.
  Studi berjalan di thread latar; hasil lama redup sampai yang baru tiba.
- Label aktivitas baru `"Study"` ditambahkan ke pemetaan `execute_model_command`
  (label lama tidak diubah).

### Gate testing P17
| Tes | Kriteria lulus |
|---|---|
| `cargo test -p ducad-sim element_hex8` | K elemen simetris, positif-semidefinit, 6 mode kaku (eigen ≈ 0), patch test regangan konstan eksak |
| `cargo test -p ducad-sim solver_cg` | Sistem SPD acak 1k DOF: residual < 1e-8; sistem singular → `Underconstrained` dalam ≤ iterasi maks |
| `cargo test -p ducad-sim bench_tension_bar` | Batang 10×10×100 mm, F=1000 N aksial: σ = 10 MPa ± 2 %, δ = FL/(EA) ± 3 % |
| `cargo test -p ducad-sim bench_cantilever` | Balok 10×10×100, F=100 N ujung: δ Euler-Bernoulli ± 10 % (hex voxel), σ_bending maks ± 10 % |
| `cargo test -p ducad-sim bench_pressure_plate` | Pelat tumpu 4 sisi, tekanan merata: defleksi tengah vs Timoshenko ± 10 % |
| `cargo test -p ducad-sim bench_hole_plate` | Pelat berlubang tarik: faktor konsentrasi 2.5–3.3 (teoretis 3.0) |
| `cargo test -p ducad-sim mesh_voxel` | Σ volume voxel = volume B-rep ± 0.1 % setelah koreksi; setiap face selector memetakan ≥ 1 node; sel 20k–200k dalam batas waktu 2 s (debug 10 s) |
| `cargo test -p ducad-sim sim_has_no_kernel_or_gui_dependency` | `Cargo.toml` tidak memuat opencascade/egui/eframe/wgpu |
| `cargo test -p ducad-engine study_op` | Fixture `tests/fixtures/sim_bracket.ops.json`: `Op::Study` di-replay, hasil deterministik (hash `SimReport` identik dua kali run), `MaxStress`/`MinSafetyFactor` Pass/Fail sesuai nilai sengaja |
| `cargo test -p ducad-engine schema_file`, `engine_has_no_gui_dependency` | Hijau |
| `cargo test -p ducad-mcp` | Tool 27, deskripsi Inggris, `simulate_static` dry-run mengembalikan error `SIM_UNDERCONSTRAINED` dengan `hint` saat tanpa fixture |
| `cargo test -p ducad-cli build_with_study` | `report.json` memuat `sim/*`; studi gagal check → exit 3 |
| `cargo test -p ducad-ui --test ui_glyph_coverage` | Hijau |
| Performa (`docs/PERF_BASELINE.md` ditambah baris) | Bracket 50k elemen: < 3 s release di M1; iPad: < 8 s atau turun otomatis ke 20k elemen |
| Manual GUI | Heatmap konsisten dengan nilai CLI; cancel di tengah tidak membekukan UI; undo satu langkah menghapus studi |

---

## P18 — Mesh tetrahedral konform + frekuensi, buckling, termal

### Apa yang dibuat
Akurasi kelas FEA sungguhan (±3 %) dengan mesh tetra yang mengikuti
permukaan B-rep (fillet, lubang, tirus), elemen Tet10, kontrol kehalusan
lokal. Di atas K dan M yang sama: analisis frekuensi natural, buckling
linier, dan termal konduksi steady-state dengan tegangan termal.

### Detail teknis
**Mesher tetra (`ducad-sim/src/mesh/tet.rs`)**
- Masukan: `KernelMesh` permukaan yang di-remesh (segitiga mendekati sama sisi)
  dengan ukuran target `h`; node permukaan dipertahankan sebagai constraint.
- Bowyer–Watson Delaunay 3D dengan predikat orientasi/insphere robust
  (aritmetika adaptif ala Shewchuk, implementasi sendiri atau crate
  `robust` (MIT) setelah `cargo deny`), lalu *boundary recovery* dan
  penghapusan tet luar (parity terhadap permukaan).
- Penyisipan titik interior (Steiner) sampai kualitas minimum (rasio
  radius 0.1) dan ukuran `h` tercapai; `sizing` lokal: lebih halus di face
  yang dibebani/dikunci dan di fillet kecil (berdasarkan kurvatur dari
  `face_kinds` + jari-jari).
- Konversi Tet4 → Tet10 (node tengah tepi, diproyeksikan ke permukaan
  untuk tepi batas).
- Fallback otomatis ke hex voxel P17 bila mesher gagal, dengan peringatan
  `SIM_MESH_FALLBACK_HEX` di laporan.

**Elemen & analisis**
- `element/tet10.rs` (integrasi Gauss 4 titik), matriks massa konsisten.
- **Frekuensi**: `K φ = λ M φ`, Lanczos dengan shift-invert memakai PCG;
  10 mode pertama; keluaran Hz + bentuk mode untuk animasi.
- **Buckling linier**: `K_σ` dari tegangan hasil statik, `(K + λ K_σ) φ = 0`,
  faktor beban kritis λ₁.
- **Termal**: konduksi `K_t T = q` dengan BC suhu/fluks/konveksi; regangan
  termal `α ΔT` → beban ekuivalen ke statik (coupling satu arah).
- `Op::Study.kind`: `"static" | "frequency" | "buckling" | "thermal" | "thermal_stress"`.
- Checks: `MinNaturalFrequency { study, min_hz }`, `MinBucklingFactor { study, min }`,
  `MaxTemperature { study, max_c }`.

**Engine/MCP/GUI**
- `simulate_static` diperluas menjadi `simulate { kind, setup }` (nama tool
  tetap `simulate_static` sebagai alias agar skill lama berjalan).
- Panel GUI: pilih jenis studi, slider kehalusan mesh, tampilan wireframe mesh,
  animasi mode getar.

### Gate testing P18
| Tes | Kriteria lulus |
|---|---|
| `cargo test -p ducad-sim delaunay_predicates` | Kasus degenerasi (kolinear, koplanar, kosferis) tidak panik; orientasi konsisten |
| `cargo test -p ducad-sim mesh_tet_quality` | Pada 12 fixture ops (bracket, flange, enclosure, roda gigi, dst.): 0 tet terbalik, rasio radius min ≥ 0.1, Σ volume tet = volume B-rep ± 0.05 %, semua face selector terpetakan |
| `cargo test -p ducad-sim bench_cantilever_tet` | δ dan σ ± 3 % analitik; konvergensi monoton saat `h` diperhalus (3 level) |
| `cargo test -p ducad-sim bench_hole_plate_tet` | Faktor konsentrasi 3.0 ± 5 % |
| `cargo test -p ducad-sim bench_frequency_beam` | Frekuensi 1–3 balok kantilever vs rumus Euler–Bernoulli ± 3 % |
| `cargo test -p ducad-sim bench_euler_buckling` | Kolom pinned-pinned: P_cr = π²EI/L² ± 5 % |
| `cargo test -p ducad-sim bench_thermal_rod` | Batang dengan suhu ujung berbeda: profil linier eksak ± 1e-6; pemuaian bebas tanpa tegangan (σ < 1e-6 MPa) |
| `cargo test -p ducad-sim fallback_hex` | Permukaan rusak sengaja → laporan berisi `SIM_MESH_FALLBACK_HEX`, bukan panik |
| `cargo test -p ducad-engine`, `-p ducad-mcp`, `-p ducad-cli` | Alias tool tetap; replay deterministik untuk semua jenis studi |
| Performa | Bracket 100k Tet10 statik < 10 s release M1; frekuensi 10 mode < 30 s |
| `cargo deny check licenses`, clippy, fmt berkas baru | Hijau |

---

## P19 — Sheet metal, GD&T/toleransi, konfigurasi varian

### Apa yang dibuat
Tiga fitur yang paling sering disebut saat industri menolak CAD "non-SolidWorks":
pembuatan part pelat lipat dengan pola bentangan, anotasi toleransi standar
di gambar kerja, dan satu file yang memuat banyak varian ukuran.

### Detail teknis
**Sheet metal (`ducad-engine/src/compute/sheet_metal.rs`, kernel `sheet.rs`)**
- Op baru: `BaseFlange { sketch, thickness, bend_radius, k_factor }`,
  `EdgeFlange { body, edges: selector, length, angle, relief: Rect|Obround }`,
  `Hem`, `Jog`, `Unfold { body }` / `Fold`, `FlatPattern { body }` (menghasilkan
  body datar + sketsa garis tekuk).
- Representasi: body solid biasa + metadata `SheetMetalInfo { thickness,
  default_radius, k_factor, bends: Vec<Bend { face_pair, angle, radius }> }`
  di `ducad-core`.
- Bend allowance `BA = θ·(R + k·t)`; tabel k-factor per material dan tabel
  bend deduction kustom (CSV di dokumen).
- Flat pattern → DXF (writer `ducad-io/src/dxf.rs` sudah ada) dengan layer
  `BEND_UP`/`BEND_DOWN`/`OUTLINE`, dan tampak datar otomatis di drawing.
- Checks: `MinBendRadius { body, min_ratio_to_t }`, `MinFlangeLength`.

**GD&T & toleransi (`ducad-io/src/drawing.rs`, `pdf.rs`, `ducad-core/src/drawing_annot.rs`)**
- Tipe anotasi: `DimensionTolerance { nominal, plus, minus | fit: IsoFit("H7") }`,
  `FeatureControlFrame { symbol: Flatness|Position|Perpendicularity|…, value,
  modifiers: [MMC|LMC], datums: ["A","B"] }`, `DatumFeature`, `SurfaceFinish { ra_um }`,
  `HoleTable`, `RevisionTable`.
- Tabel ISO 286 (toleransi IT dan deviasi fundamental untuk H, h, g, f, k, p, dst.)
  sebagai data statis teruji.
- Simbol GD&T digambar sebagai path vektor (bukan font) agar PDF dan layar
  konsisten; teks dari `ttf-parser` yang sudah dipakai.
- Op `Annotate` di drawing op-set (drawing sudah punya pipeline sendiri; ikuti
  `drawing_auto.rs`), MCP `drawing` menerima `annotations`.
- Check `ToleranceStackup { chain: [dim ids], max_total }` (worst-case & RSS)
  — pemeriksaan tumpukan toleransi sederhana yang tidak ada di SolidWorks Standard.

**Konfigurasi varian (`ducad-core/src/configuration.rs`)**
- `Configuration { name, params: BTreeMap<String, f64>, suppressed_ops: Vec<String>, material_overrides }`.
- `Document.configurations`, `active_configuration`. Replay oplog memakai
  param konfigurasi aktif; op di `suppressed_ops` dilewati (`FeatureStatus::Suppressed`
  sudah ada di `parametric.rs`).
- Design table: impor/ekspor CSV (baris = konfigurasi, kolom = param).
- `ducad-cli build --config NAME|--all-configs` → artefak per konfigurasi,
  BOM per konfigurasi. MCP: `set_params` menerima `configuration`, tool
  `list_configurations` (bisa digabung ke `inspect` agar tool count tidak membengkak).
- Checks berjalan per konfigurasi; `report.md` memuat matriks konfigurasi × check.

### Gate testing P19
| Tes | Kriteria lulus |
|---|---|
| `cargo test -p ducad-engine sheet_metal` | Kotak 4 flange: bentangan = Σ panjang datar + Σ BA ± 0.01 mm; `Unfold`→`Fold` mengembalikan volume ± 0.1 %; flat pattern tidak self-intersect |
| `cargo test -p ducad-io dxf_flat_pattern` | Layer tekuk ada; dibaca ulang oleh importer DXF sendiri tanpa hilang entitas |
| `cargo test -p ducad-core iso286` | Sampel tabel: 25H7 = +0.021/0, 25g6 = −0.007/−0.020, dst. (10 kombinasi) |
| `cargo test -p ducad-io gdt_pdf` | PDF memuat FCF; snapshot vektor (hash path) stabil; `ui_glyph_coverage` hijau |
| `cargo test -p ducad-engine configurations` | 3 konfigurasi bracket: volume berbeda sesuai param; op tersuppress tidak muncul di inspect; `.ducad` lama tanpa konfigurasi → satu konfigurasi "Default" |
| `cargo test -p ducad-cli build_all_configs` | Direktori per konfigurasi, exit 3 bila salah satu gagal check |
| clippy, deny, schema_file, mcp | Hijau |

---

## P20 — Assembly industri: thread fisik, toolbox, mate lanjutan, exploded view

### Apa yang dibuat
Melengkapi assembly agar BOM, gambar rakitan, dan simulasi beban antar part
bisa dipakai di produksi.

### Detail teknis
- **Thread fisik**: `Op::Thread { body, face: selector silinder, spec: IsoMetricThread, length, handed }`
  memakai `create_helix_solid_with_custom_profile` (ada) dengan profil ISO 68-1
  60°, lalu boolean. Opsi `cosmetic: true` tetap default untuk performa.
- **Toolbox fastener**: generator part standar (baut ISO 4762/4014, mur ISO 4032,
  ring ISO 7089, pin, bearing seri 6xxx) sebagai `Op::StandardPart { standard, size }`
  → body parametrik dari tabel yang sama dengan `HoleSpec`; BOM otomatis
  memuat nomor standar.
- **Mate lanjutan** di `assembly_solver.rs`: `Gear { ratio }`, `Screw { pitch }`,
  `RackPinion`, `Path`, `Cam` (kontak kurva), `Width`. Penyelesaian iteratif
  seperti `solve_assembly` yang ada; `evaluate_motion` memberi posisi semua
  instance per waktu.
- **Beban antar part untuk simulasi**: `Op::Study` pada assembly dengan
  kontak `Bonded` (node digabung pada face mate `Coincident`) dan `NoPenetration`
  linier sederhana (gap elements), membuka studi rakitan P17/P18.
- **Exploded view**: `ExplodeStep { instance, translation, rotation }` tersimpan
  di `AssemblyTree`; tampak exploded di drawing + balloon BOM otomatis.
- **Large assembly**: lazy-load `ExternalPartRef` dengan mesh LOD (decimasi
  quadric di `ducad-render`), instancing GPU yang sudah dicatat di `STATUS_FASE_A_B.md`.

### Gate testing P20
| Tes | Kriteria lulus |
|---|---|
| `cargo test -p ducad-kernel thread_iso` | M10×1.5 panjang 20: volume vs silinder polos berkurang sesuai luas profil ± 2 %; `is_valid` |
| `cargo test -p ducad-engine standard_parts` | Setiap entri toolbox valid, dimensi kunci sesuai tabel standar, BOM menampilkan nomor ISO |
| `cargo test -p ducad-kernel mate_gear_screw` | Gear rasio 2: sudut output = 2× input; screw pitch 1.5: translasi 1.5 mm/putaran |
| `cargo test -p ducad-sim bonded_assembly` | Dua balok bonded = satu balok utuh (δ ± 1 %) |
| `cargo test -p ducad-io exploded_drawing` | Balloon = jumlah baris BOM; PDF deterministik |
| Performa | Rakitan 500 instance: buka < 5 s, frame > 30 fps dengan LOD |
| clippy, deny, mcp, glyph coverage | Hijau |

---

## 3. Urutan, estimasi, dan ketergantungan

| Fase | Perkiraan | Prasyarat | Hasil terlihat user |
|---|---|---|---|
| P16 | 1–2 minggu | — | Panel massa + inersia, pustaka material, `set_material` |
| P17 | 4–6 minggu | P16 | Studi statik, heatmap, SF, checks, CLI/MCP |
| P18 | 4–6 minggu | P17 | Akurasi tet, frekuensi, buckling, termal |
| P19 | 4–5 minggu | P16 (material k-factor) | Sheet metal, GD&T, konfigurasi |
| P20 | 3–4 minggu | P17 (studi rakitan), P19 (BOM konfigurasi) | Thread, toolbox, mate lanjutan, exploded |

P19 dapat berjalan paralel dengan P17–P18 karena tidak menyentuh `ducad-sim`.
P0.4 (`BRepTools_History`, harness) tetap prioritas terpisah; fitur di sini
tidak bergantung padanya kecuali asosiativitas fixture/load saat topologi berubah
(sementara: selector string di-resolve ulang setiap run, sama seperti op lain).

## 4. Risiko dan mitigasi

| Risiko | Kemungkinan | Dampak | Mitigasi |
|---|---|---|---|
| Lisensi mesher (Netgen LGPL, TetGen AGPL, Gmsh GPL) | Pasti | Tidak bisa dipakai | Mesher sendiri; hex dulu (P17) agar nilai bisnis tidak menunggu tet |
| Akurasi FEA diremehkan pengguna industri | Tinggi | Reputasi | Benchmark analitik di CI, label "Estimasi (hex)" vs "Konform (tet)" di UI dan laporan, publikasi `docs/SIM_VALIDATION.md` |
| Binding cxx baru memicu rebuild OCCT | Sedang | 10–15 menit per rebuild | Hanya tambah fungsi wrapper, jangan sentuh versi/fitur/`[patch.crates-io]`; rebuild sekali, cache OCCT dijaga `clean-no-occt` |
| Memori/waktu di iPad | Sedang | Studi besar tak jalan | Batas elemen adaptif, PCG hemat memori, cancel token, hasil kasar dulu |
| Konflik `KERNEL_LOCK` saat voxelisasi memanggil kernel per sinar | Sedang | Deadlock/lambat | Ekstrak semua data mesh di satu panggilan terkunci, ray cast di `ducad-sim` murni tanpa kernel |
| Scope creep ke CFD/nonlinier/fatigue | Tinggi | Fase tak selesai | Dinyatakan di luar cakupan; export mesh/hasil ke tool spesialis |
| Oplog membengkak karena `nodal_field` | Rendah | File besar | Hasil tidak masuk `.ducad`; cache terpisah di `~/.ducad/sim-cache/<hash>` |
| Teks agent tercampur bahasa | Sedang | Agent salah paham | Tes yang memeriksa deskripsi tool/Op/Check baru hanya ASCII Inggris, seperti yang ada |

## 5. Definisi selesai untuk setiap fase

1. Semua gate testing fase hijau di CI (`cargo test --workspace`, clippy `-D warnings`,
   `cargo deny check`, `ui_glyph_coverage`).
2. `schema/ops.schema.json` diperbarui; setiap `Op`/`Check` baru punya doc comment
   `///` Inggris dan contoh di `tests/fixtures/*.ops.json` + `ops::EXAMPLES`.
3. `.claude/skills/ducad-modeling/SKILL.md` dan `tooling::ERROR_GUIDE` memuat
   selector, kode error, dan contoh baru; `ducad://guide` dan `instructions` MCP
   diperbarui bila ada tool baru.
4. `docs/PLAN.md` mendapat bagian "Status Fase P1x" dan `docs/PERF_BASELINE.md`
   mendapat baris pengukuran baru.
5. Tidak ada `unwrap()`/`expect()` pada input luar; `ducad-sim` dan `ducad-engine`
   tetap bebas GUI/OCCT sesuai tes penjaga.
6. Demo manual GUI dicatat di `docs/CEKLIS_UJI_GUI.md`.
