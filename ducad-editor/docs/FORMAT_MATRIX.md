# Matriks format berkas

Step 4 `docs/PLAN_DESIGNER_UX.md`. Dokumen ini membedakan format yang bisa
**dibuka**, **dibuat**, dan **round-trip** (dibuat lalu dibuka kembali tanpa
kehilangan data yang diklaim). Struktur `.ducad` sendiri dijelaskan di
[`FORMAT.md`](FORMAT.md). Status mengikuti istilah di
[`STATUS_KEMAMPUAN.md`](STATUS_KEMAMPUAN.md): stabil, terbatas, eksperimental.

Satuan internal DUCAD adalah **mm**. Setiap baris menyebut satuan yang
dinyatakan berkas dan bukti tes otomatisnya.

## Ringkasan

| Format | Buka | Buat | Round-trip | Satuan di berkas | Status |
|---|---|---|---|---|---|
| `.ducad` | ✅ | ✅ | ✅ desain lengkap | mm (implisit) | stabil |
| STEP AP242 (`.step`/`.stp`) | ✅ B-rep | ✅ B-rep | ✅ geometri | `SI_UNIT(.MILLI.,.METRE.)` | stabil (ekspor), terbatas (impor dari CAD lain) |
| STL biner/ASCII | ✅ mesh | ✅ biner | ✅ segitiga | tidak ada (dianggap mm) | stabil |
| OBJ | ✅ mesh | ✅ | 🟡 mesh tanpa material | tidak ada (dianggap mm) | stabil |
| GLB / glTF | ✅ mesh | ✅ GLB | 🟡 meter, Y ke atas | meter (glTF) | stabil (ekspor) |
| 3MF | ✅ mesh | ✅ (kirim ke slicer) | ✅ mesh | atribut `unit` (impor dikonversi ke mm) | terbatas |
| IGES (`.igs`/`.iges`) | ✅ | ✅ (palet: Ekspor IGES) | ✅ volume | mm (ekspor) | eksperimental (impor dari CAD lain) |
| DXF sketsa | ✅ | ✅ R12 | 🟡 garis/lingkaran/busur | `$INSUNITS` (impor dikonversi ke mm) | terbatas |
| DXF drawing | — | ✅ R12 | — | `$INSUNITS 4` (mm), skala 1:1 kertas | stabil (subset di bawah) |
| PDF drawing | ✅ garis vektor (referensi) | ✅ | — | ukuran kertas (pt) | stabil (subset di bawah) |
| SVG | ✅ path → sketsa | ✅ sketsa/drawing | 🟡 | px CSS 96 dpi → mm | terbatas |

## Per format

### `.ducad` (native)

- **Dipertahankan:** param, oplog (termasuk sketsa di bidang `frame` /
  `three_points`), op yang di-suppress, material body, lembar gambar
  (`DrawingSpec`), UUID body, checks, brief. Geometri dibangun ulang dari
  oplog saat dibuka; hasil replay sama dengan sebelum disimpan.
- **Hilang/terbatas:** anotasi GD&T yang ditempel manual di editor drawing GUI
  belum menjadi bagian format (lihat `STATUS_KEMAMPUAN.md`).
- **Input rusak:** JSON rusak, JSON bukan dokumen DUCAD, berkas terpotong, dan
  versi format lebih baru ditolak dengan pesan, bukan panic.
- **Bukti:** `ducad-engine/tests/format_validation.rs::native_roundtrip_keeps_design_intent`,
  `::corrupt_inputs_are_rejected_not_panicking`; `ducad-engine/tests/session_file.rs`;
  `ducad-io/tests/native_format.rs`; `sample_project.rs::save_and_reopen_roundtrip`.

### STEP

- **Ekspor:** semua body **terlihat** (GUI, CLI, dan MCP kini sama), satu
  berkas multi-body. Skema AP242, satuan mm. Body tersembunyi tidak ikut; GUI
  menyebut jumlahnya di ringkasan ekspor. (Sebelum Step 4, GUI ikut
  mengekspor body tersembunyi.)
- **Dipertahankan:** geometri B-rep (volume dibuka ulang sama, toleransi
  relatif 1e-4).
- **Hilang:** nama body, material, warna, param, dan riwayat fitur (STEP
  hanya geometri).
- **Impor:** satu shape (body majemuk). Belum ada fixture dari CAD lain.
- **Bukti:** `format_validation.rs::step_multibody_reimports_with_same_volume`;
  `ducad-kernel/tests/step_roundtrip.rs`; `sample_project.rs::export_step_reopens_and_stl_is_written`.

### STL / OBJ

- **Ekspor STL:** biner, semua body terlihat **digabung menjadi satu mesh**,
  tanpa material. Ukuran = 84 + 50·n byte. Koordinat dalam mm.
- **Ekspor OBJ:** satu objek per body (nama body), tanpa material, mm.
- **Impor:** STL biner/ASCII dan OBJ menjadi body mesh (bukan B-rep).
- **Bukti:** `format_validation.rs::stl_triangle_count_and_bbox_match_model`;
  `ducad-io/tests/hostile_stl.rs`; `ducad-io/src/mesh_formats.rs` (6 tes).

### GLB / glTF

- **Ekspor:** body terlihat, material PBR, **satuan meter dan sumbu Y ke atas**
  (konvensi glTF; mm × 0,001, Z-up → Y-up). Dinyatakan di label dialog simpan
  dan di ringkasan ekspor.
- **Impor:** glTF 2.0 (GLB atau buffer `data:`) menjadi mesh.
- **Bukti:** `ducad-io/src/glb.rs` (2 tes), `mesh_formats.rs`.

### DXF sketsa

- **Ekspor:** R12 ASCII, `$INSUNITS 4` (mm). LINE, CIRCLE, ARC, POINT.
  **Elips, spline, dan path tidak didukung R12 dan tidak ikut**; GUI menyebut
  jumlahnya setelah ekspor (sebelum Step 4 hilang diam-diam).
- **Impor:** LINE, CIRCLE, ARC, ELLIPSE (sejajar sumbu), LWPOLYLINE/POLYLINE
  (termasuk bulge). Entitas lain dilewati dan dihitung (`skipped`).
  `$INSUNITS` dibaca: berkas inci/cm/m/ft/… dikonversi ke mm dan GUI menyebut
  satuan asalnya. Tanpa `$INSUNITS` dianggap mm.
- **Hilang:** layer, warna, blok/INSERT, TEXT, HATCH, DIMENSION, SPLINE.
- **Bukti:** `ducad-io/src/dxf.rs` (17 tes, termasuk `unit_tests`).

### Drawing (PDF / DXF / SVG)

**Subset produksi yang dijamin** (dicakup tes otomatis; di luar ini berstatus
terbatas):

1. Satu lembar per ekspor, kertas A4 atau A3, tegak atau mendatar.
2. Tampak ortografis proyeksi sudut ketiga dengan garis tersembunyi (HLR) dan
   tampak isometri.
3. Dimensi otomatis linear/diameter, catatan lubang dari op `hole`.
4. Title block (judul, nomor part, material, revisi, tanggal).
5. Satuan gambar mm; DXF digambar 1:1 dalam mm kertas.

Di luar subset (terbatas): multi-sheet (hanya lewat paket desain), anotasi
GD&T manual, detail view interaktif, BOM/balon di part tunggal, kertas
selain A4/A3.

- **PDF:** satu halaman, `/MediaBox` sama dengan ukuran kertas, teks title
  block ada di stream, ditutup `%%EOF`. Diuji untuk A3 dan A4 mendatar.
- **DXF:** `$INSUNITS 4`, layer `BORDER`, `TITLEBLOCK`, `VISIBLE`, `HIDDEN`,
  `CENTERLINE`, `DIMENSIONS`, `HATCH`, `SECTION`, `BOM_TABLE`,
  `CALLOUT_BALLOONS`; entitas LINE, CIRCLE/ARC, TEXT. Gambar raster yang
  ditempel tidak ikut ke DXF. Berkas terbaca kembali oleh importer DXF DUCAD.
- **Bukti:** `format_validation.rs::drawing_pdf_has_paper_size_and_title`,
  `::drawing_dxf_declares_mm_and_layers`; `ducad-io/tests/sheet_p21.rs`,
  `gdt_pdf.rs`, `exploded_drawing.rs`; `ducad-engine/tests/drawing_golden.rs`.
- **Manual:** pemeriksaan visual hasil PDF/DXF di aplikasi target ada di
  `CEKLIS_UJI_GUI.md` Bagian P (tugas T6).

### Impor PDF / SVG / IGES

- **PDF:** garis vektor dari content stream (tak terkompresi atau Flate) jadi
  polyline mm untuk sketsa referensi. Teks, gambar, dan font tidak diimpor.
- **SVG:** path, bentuk dasar, dan busur menjadi `Entity::Path`; px CSS 96 dpi
  → mm; sumbu Y dibalik.
- **IGES:** pembaca kernel, dengan peringatan per entitas yang dilewati.
  Belum ada fixture dari CAD lain → eksperimental. Ekspor (palet "Ekspor
  IGES…") menulis body terlihat saja, dengan ringkasan yang sama seperti STEP;
  dibuka ulang dengan volume sama (`ducad-app` `interop::tests::export_3mf_and_iges_then_reimport`,
  `interop::tests::iges_skips_hidden_bodies`).

## Pesan di GUI (Step 4 butir 6)

- **Sebelum menyimpan:** label filter dialog simpan menyebut format dan
  satuan (mis. "STEP AP242 - satuan mm", "Model 3D GLTF Binary - satuan
  meter, Y ke atas").
- **Setelah menyimpan:** baris status menyebut jumlah body, satuan, batasan
  format (STL digabung, OBJ/STL tanpa material, GLB Y ke atas), body
  tersembunyi yang tidak ikut, dan entitas DXF yang dilewati.
- **Impor DXF:** menyebut satuan asal berkas bila bukan mm.
- Tes: `ducad-app` `document::tests::step_export_uses_visible_bodies_and_note_mentions_hidden`.

## Belum dikerjakan

- Fixture STEP/DXF/IGES dari CAD lain (SolidWorks, Fusion, FreeCAD, AutoCAD).
- Pembukaan hasil ekspor di aplikasi target (manual, `CEKLIS_UJI_GUI.md`).
- Ekspor DXF untuk elips/spline (perlu DXF R2000 atau aproksimasi polyline).
