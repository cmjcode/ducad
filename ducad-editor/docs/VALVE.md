
## Rencana Fase P21 — Gambar kerja setara lembar produksi (acuan: valve body)

### Tujuan

DUCAD harus bisa menghasilkan lembar gambar teknik yang setara dengan gambar
produksi valve body (A3, skala 1:2, AISI 316L) yang dipakai sebagai acuan:

- **Tampak:** Front, Top, Section A-A (bidang potong vertikal yang ditandai di
  Top), Section B-B (bidang potong horizontal yang ditandai di Top, dengan
  skala sendiri "SCALE 1:2"), dua render isometrik berbayang.
- **Dimensi:** diameter (Ø42, Ø98, Ø146, Ø160), radius (R65), jarak sumbu
  (120, 130, 166), pola lubang ("4×Ø14", PCD Ø130), chamfer ("2×45°",
  "5×45°"), tebal dinding, dimensi dengan garis bantu bertingkat.
- **Teks:** catatan umum bernomor, kepala gambar ISO (judul, material,
  skala, lembar, revisi, nomor gambar), grid zona A–F / 1–8.

Semua ini harus bisa dihasilkan **tanpa GUI** (tool `drawing`, `ducad-cli
build`) secara deterministik, **dan** bisa diatur ulang di editor lembar GUI.

### Kondisi awal (2026-10-06)

| Kemampuan | Status | Lokasi |
|---|---|---|
| Kertas A4/A3, bingkai + zona, title block, catatan | ada | `ducad-io/src/drawing.rs` |
| Front/Top/Right/Iso dengan HLR **berbasis mesh** | ada | `ducad-kernel/src/hlr.rs` |
| HLR eksak OCCT (`HLRBRep_Algo`, lingkaran tetap lingkaran) | ada tapi **hanya dipakai `vector_snapshot`**, belum untuk lembar | `ducad-kernel/src/hlr_exact.rs` |
| Section + arsir 45° | ada, **satu** bidang (A-A), selalu `from_model_bbox_center_y` | `ducad-kernel/src/section.rs`, `hlr.rs:271` |
| Detail view berskala | ada | `ducad-kernel/src/detail.rs` |
| Skala | satu skala per lembar; `SheetViewPlacement.scale` ada tetapi tidak dihormati ekspor/dimensi | `drawing.rs::layout_with_scale` |
| Dimensi otomatis | panjang+tinggi Front, fitur R/Ø per tampak; **tidak terikat geometri**, tidak ada pola lubang/PCD/chamfer | `drawing.rs::generate_auto_dimensions` |
| Anotasi GD&T/toleransi | ada, posisi mm bebas | `ducad-core/src/drawing_annot.rs`, `ducad-io/src/drawing/gdt.rs` |
| Ekspor | PDF (penulis PDF tangan sendiri), SVG, DXF (tanpa anotasi) | `pdf.rs`, `svg.rs`, `dxf.rs` |
| Headless | `auto_sheet_model` → tool `drawing` (`tooling.rs::drawing_tool`) dan `build` | `ducad-engine/src/drawing_auto.rs` |
| Editor GUI | drag tampak, dimensi manual (Linear/Ø/R/∠), teks, detail, balon, edit title block | `ducad-ui/src/drawing_sheet_view.rs` |
| Persistensi lembar | **tidak ada**: `drawing_sheet_doc` hidup hanya selama sesi, tidak masuk `.ducad` | `ducad-app/src/file_io.rs:835` |
| Render berbayang | hanya `render_view` → PNG terpisah, tidak masuk lembar | `ducad-engine/src/render.rs` |
| Bug tercatat P19 | SVG lembar tercermin sumbu-Y; title block PDF menulis operator `arc` tak sah | `svg.rs`, `pdf.rs` |

### Kesenjangan terhadap gambar acuan

1. Hanya satu section, posisi/arah tidak bisa dipilih; tidak ada B-B.
2. Tidak ada skala per tampak.
3. Lingkaran keluar sebagai poligon (HLR mesh) → dimensi Ø tidak presisi dan
   gambar cetak bergerigi.
4. Dimensi otomatis minim dan tidak asosiatif; `set_params` tidak
   memperbarui dimensi yang ditempatkan manual.
5. Render isometrik berbayang tidak bisa disematkan ke lembar.
6. Tata letak yang diatur pengguna hilang saat file ditutup.

### Prinsip

- Semua logika gambar tetap di `ducad-kernel` (OCCT) → `ducad-io/drawing`
  → `ducad-engine/drawing_auto`; GUI hanya memanggil dan menampilkan.
- Aturan `lock_kernel()`: fungsi publik kernel baru memakai pola
  `extract_*` (ambil guard) + `extract_*_internal` (`pub(crate)`, tanpa guard),
  seperti `SectionExtractor` sekarang.
- Hasil headless harus deterministik byte demi byte (gold file di
  `tests/golden/`), karena `build` dipakai CI.
- Teks yang dibaca agent (deskripsi tool, skema) bahasa Inggris; komentar
  dan pesan GUI bahasa Indonesia.
- Ikon UI tidak boleh emoji/glyph Unicode (lihat aturan global); tes
  `ui_glyph_coverage` wajib lolos.

### Status pelaksanaan (2026-10-07)

P21.0–P21.8 sudah diimplementasikan dan dijaga tes otomatis (kernel, io,
engine, uji asap editor). Penyimpangan dari rencana di bawah, semuanya disengaja:

- **Belum diuji manual di GUI.** Editor lembar (P21.7) terkompilasi dan lolos
  uji asap tanpa jendela (`ducad-ui/tests/drawing_sheet_smoke.rs`), tetapi
  ceklis manual **A15** (nomor A14 sudah dipakai tutorial) belum dijalankan.
- **Fixture valve body:** `min_wall ≥ 8` tidak mungkin dipenuhi bersama Ø146 /
  PCD Ø130 / Ø14 (sisa dinding flange 73 − 65 − 7 = 1 mm), jadi check-nya
  `min_wall ≥ 0.9`; `hole_count Ø14` = 4 karena check menghitung sumbu lubang
  (8 lubang fisik berada pada 4 sumbu yang sama). Chamfer 2×45° dipasang di
  mulut counterbore Ø60, bukan di rim flange, karena alasan dinding yang sama.
- **Render berbayang** disimpan sebagai `DrawingSheet.shaded` (bukan varian
  `SheetViewPlacement.kind`), dirender rasterizer CPU sendiri
  (`ducad-io/src/drawing/raster.rs`) dan disematkan `/FlateDecode`.
- **`HlrDrawing.section_a`/`cutting_plane`** dihapus langsung tanpa alias serde
  (risiko 4: lembar belum pernah dipersistenkan).
- **Batas waktu HLR 10 s:** OCCT tidak bisa dihentikan di tengah jalan; yang
  ada hanya peringatan `HLR_EXACT_SLOW` + fallback bila HLR gagal.
- **Cache HLR** global-proses di `ducad-engine/src/drawing_auto.rs` (kunci:
  sidik jari geometri + potongan), bukan field `SessionCore`.
- **Selector dimensi** dipetakan ke fitur lewat geometri face (sumbu/radius),
  bukan lewat `edge_refs`; `edge_refs` tetap diisi untuk segmen dan busur.
- **Validasi PDF:** `qpdf` tidak terpasang; tes memastikan operator `arc`
  hilang dan PDF valve dirender bersih oleh `pdftoppm`.
- Dimensi otomatis yang dihapus pengguna di editor tidak dipersistenkan
  (lihat ADR 0007).

### P21.0 — Dasar: perbaikan bug + jaring pengaman (≈ 2 hari)

- [x] Perbaiki cermin sumbu-Y SVG lembar supaya identik dengan PDF
  (`svg.rs::export_drawing_sheet_svg_string`), tambah tes yang membandingkan
  koordinat satu segmen Front di PDF vs SVG.
- [x] Perbaiki operator `arc` tak sah di title block PDF (`pdf.rs`): ganti
  dengan kurva Bézier 4 segmen; validasi dengan `qpdf --check` di tes bila
  terpasang, atau parser mini yang menolak operator di luar daftar PDF 1.4.
- [x] Gold test lembar: `ducad-engine/tests/drawing_golden.rs` merender
  fixture `flange.ops.json` ke PDF/SVG/DXF dan membandingkan hash; perbarui
  dengan `DUCAD_UPDATE_GOLDEN=1`.
- [x] Ganti ikon emoji `ManualDimensionMode::icon` ("📏") dengan
  `egui_icons::icons::ICON_*`; jalankan `cargo test -p ducad-ui --test ui_glyph_coverage`.

### P21.1 — Bidang potong eksplisit dan multi-section (≈ 4 hari)

**Kernel (`ducad-kernel/src/section.rs`, `hlr.rs`).**

- [x] `SectionPlaneConfig::from_axis(axis: Axis, offset_mm: f32, flip: bool,
  bbox)` dan `SectionPlaneConfig::through_points(p1, p2, view_kind)` (dua titik
  di tampak induk → bidang tegak lurus tampak itu).
- [x] Section bertingkat (offset/stepped section, seperti A-A pada valve
  body yang melewati dua lubang baut): `SectionPath { points: Vec<[f32;2]>,
  parent: ProjectedViewKind }`; tiap ruas memotong dengan bidangnya sendiri,
  hasil digabung ke satu tampak; garis potong bersiku di tampak induk.
- [x] `HlrDrawing.sections: Vec<SectionView>` dengan
  `SectionView { label: String, view: ProjectedView, cutting_line:
  CuttingLineIndicator, parent: ProjectedViewKind, config: SectionPlaneConfig }`.
  `section_a`/`cutting_plane` dipertahankan sebagai alias serde (`#[serde(default)]`)
  selama satu rilis, lalu dihapus. `ProjectedViewKind::SectionAA` → `Section(char)`.
- [x] Arsir per body: bila dua body bersentuhan di bidang potong, sudut
  arsir berselang 45°/135° (ISO 128-50); hatch ikut `material.visual`
  bila nanti ada.
- [x] Tes: kubus berlubang dipotong di `offset = 0` dan `offset = 10`,
  panjang total kurva potong dan luas arsir berbeda sesuai analitik;
  stepped section dua lubang memberi dua lingkaran di satu tampak.

**IO/engine.**

- [x] `DrawingSheet` menata section mengikuti aturan proyeksi sudut ketiga:
  section dari garis potong vertikal di Top diletakkan di sebelah Front;
  dari garis horizontal diletakkan di bawah/atas sesuai arah panah.
- [x] `auto_sheet_model(…, &DrawingSpec)` menggantikan daftar argumen
  lepas; `DrawingSpec { paper, title, notes, sections: Vec<SectionSpec>,
  views: ViewSet, scale: ScaleSpec, shaded: Vec<ShadedSpec>, dimensions:
  DimensionPolicy }` (serde, dipakai tool, CLI, dan GUI).
- [x] Tool `drawing` menerima
  `sections: [{ "label": "A", "parent": "top", "axis": "x", "offset": 0,
  "flip": false }, { "label": "B", "parent": "top", "path": [[-80,20],[80,20]] }]`
  (offset relatif pusat bbox, satuan mm); `ducad-cli build --section A:top:x:0`.
- [x] Skema tool + `SKILL.md` + `ERROR_GUIDE` (`DRAWING_SECTION_EMPTY`:
  bidang tidak memotong body; `DRAWING_SECTION_LABEL_DUP`).

### P21.2 — Skala per tampak (≈ 2 hari)

- [x] `layout_with_scale` menghormati `SheetViewPlacement.scale` masing-
  masing; skala lembar hanya nilai bawaan. Ukuran kotak tampak dihitung dari
  skala tampak itu sendiri.
- [x] Judul tampak menambahkan `SCALE 1:2` bila berbeda dari skala lembar
  (`format_scale_ratio`), seperti "SECTION B-B SCALE 1:2" di acuan.
- [x] Ekspor PDF/SVG/DXF, `generate_auto_dimensions`, dan editor GUI memakai
  skala per tampak (saat ini keempatnya mengalikan `sheet.scale`).
- [x] Tool `drawing.views: { "section_b": { "scale": 0.5 }, "right":
  { "visible": false } }`; GUI: klik kanan tampak → menu skala
  (1:1, 1:2, 1:5, 2:1, kustom).
- [x] Tes: dua tampak dengan skala berbeda menghasilkan panjang segmen PDF
  yang berbanding sesuai; dimensi tetap menulis nilai model (bukan nilai
  kertas).

### P21.3 — HLR eksak untuk lembar gambar (≈ 4 hari)

- [x] `ProjectedView` menambah `arcs: Vec<HlrArc2D { center, r, start_deg,
  end_deg, kind }>` dan `edge_refs: Vec<EdgeRef>` (indeks topologi edge asal
  per segmen/busur, dari `hlr_exact`). Segmen tetap ada untuk kompatibilitas
  renderer; arcs digambar sebagai kurva di PDF (Bézier) dan SVG (`<path A>`),
  DXF (`ARC`).
- [x] `HlrExtractor::extract_view` memakai `hlr_exact` sebagai jalur utama,
  fallback ke mesh HLR bila OCCT gagal (log peringatan `HLR_EXACT_FALLBACK`
  di `ToolOut.warnings`, sama pola dengan `SIM_MESH_FALLBACK_HEX`).
- [x] Garis sumbu otomatis: tiap busur penuh (lingkaran) dan permukaan
  silinder yang tampak dari samping mendapat centerline/center mark ISO 128.
- [x] Deteksi fitur dari B-rep, bukan dari mesh: `HlrGeometricFeature::{Circle
  {center, r, edge}, Arc{…}, Chamfer{len, angle_deg}, Cylinder side {axis
  line, r}}` — ini sumber data P21.4.
- [x] Performa: valve body (±60 face) < 2 s per tampak di M-series; cache
  hasil HLR per `(shape hash, view kind)` di `SessionCore` supaya `set_params`
  yang tidak mengubah geometri tidak menghitung ulang.
- [x] Tes: silinder Ø20 menghasilkan tepat satu `arcs` penuh pada Top; tidak
  ada poligon; jumlah `segments` turun dibanding mesh HLR.

### P21.4 — Dimensi asosiatif dan cerdas (≈ 6 hari)

**Model data (`ducad-core/src/drawing_annot.rs`).**

- [x] `DimensionRef` baru: `Linear { a: PointRef, b: PointRef, dir: Horizontal|Vertical|Aligned }`,
  `Diameter { circle: FeatureRef }`, `Radius { arc: FeatureRef }`,
  `Angle { e1: EdgeRef, e2: EdgeRef }`, `Chamfer { edge: EdgeRef }`,
  `HolePattern { circles: Vec<FeatureRef> }`. `PointRef` = ujung edge /
  pusat lingkaran / titik ekstrem bbox tampak.
- [x] `DimensionAnnotation` menambah `source: Option<DimensionRef>` dan
  `offset_mm: f32` (jarak garis dimensi dari objek) — posisi mutlak
  `start/end/line_pos` dihitung ulang dari `source` setiap kali geometri atau
  skala berubah; dimensi lama tanpa `source` tetap mutlak.

**Generator (`drawing.rs::generate_auto_dimensions`, modul baru
`drawing/auto_dim.rs`).**

- [x] Dimensi keseluruhan per tampak (bukan hanya Front), de-duplikasi
  antar tampak: ukuran yang sudah muncul di satu tampak tidak diulang.
- [x] Pola lubang: lingkaran ber-radius sama pada satu tampak → satu callout
  `4×Ø14` + PCD `Ø130` bila pusatnya sekonsentris; teks memakai Ø (U+00D8)
  yang sudah dicakup font (cek `ui_glyph_coverage`).
- [x] Chamfer: dari `HlrGeometricFeature::Chamfer` → `2×45°`; sama panjang →
  satu callout dengan `n×`.
- [x] Jarak antar sumbu lubang/bore (120, 130, 166 di acuan): pasangan
  pusat lingkaran yang sejajar sumbu diberi dimensi linear.
- [x] Penempatan: garis dimensi bertingkat 8/16/24 mm dari tepi tampak,
  urut dari pendek ke panjang (ISO 129-1); teks tidak boleh menimpa teks
  lain atau garis tampak (uji tabrak kotak teks; geser ke tingkat berikut).
- [x] `DimensionPolicy { Auto, None, Only(Vec<DimensionRef>) }` di
  `DrawingSpec`; tool `drawing.dimensions` menerima `"auto" | "none" |
  [ {"type":"diameter","select":"all[kind=cylinder][r=7]","view":"top"} ]`
  — selector geometri yang sama dengan `query_geometry`, dipetakan ke
  `FeatureRef` lewat `edge_refs` P21.3.
- [x] `set_params` → `SessionCore` memberi tanda lembar kedaluwarsa; dimensi
  dengan `source` dihitung ulang, `offset_mm` dan posisi teks manual
  dipertahankan.
- [x] Tes: flange 4 lubang menghasilkan `4×Ø…` + PCD; mengubah param
  `hole_d` memperbarui teks tanpa mengubah `offset_mm`; tidak ada dua kotak
  teks dimensi yang beririsan (tes geometri kotak).

### P21.5 — Render berbayang di lembar (≈ 3 hari)

- [x] `SheetViewPlacement.kind` menambah `Shaded(ShadedSpec { camera:
  Iso|IsoBack|Custom{yaw,pitch}, px_per_mm: u32 })`.
- [x] Headless: `render_svg_core` + `svg_to_png` menghasilkan PNG; PDF
  menyematkan sebagai XObject `Image` (DCT bila ada encoder JPEG di
  dependensi; kalau tidak, `FlateDecode` RGB — ukuran file ±1 MB untuk
  1200 px, masih di bawah batas `build`), SVG memakai `<image href="data:image/png;base64,…">`,
  DXF melewati (dicatat di warning).
- [x] Determinisme: PNG dirender CPU (jalur `render_svg`, bukan wgpu) supaya
  gold test stabil lintas mesin.
- [x] Tool `drawing.shaded: ["iso", "iso_back"]`; CLI
  `build --shaded iso,iso_back`; GUI: tombol "Sisipkan render 3D" di toolbar
  lembar, kamera mengikuti viewport saat ditekan.
- [x] Tes: PDF dengan `shaded` lebih besar dari tanpa; parser mini
  menemukan satu `/Subtype /Image` per spec.

### P21.6 — Persistensi lembar dan reproduksi headless (≈ 3 hari)

- [x] `DesignDoc.drawings: Vec<DrawingSpec>` (bukan op di oplog, konsisten
  dengan keputusan P19 bahwa anotasi bukan `Op`); disimpan di `.ducad`
  (`native.rs`), dimuat ulang oleh GUI ke `drawing_sheet_doc`, dan dipakai
  `build` tanpa argumen tambahan. Tata letak pengguna (posisi tampak,
  `offset_mm` dimensi, teks, balon) ikut di `DrawingSpec.layout`.
- [x] `inspect` melaporkan `drawings: [{ name, paper, views, sections,
  dimension_count }]`; `diff` membandingkan jumlah/label.
- [x] Tool baru? **Tidak**: `drawing` menerima `name` dan `save: true` untuk
  menyimpan spec ke part (jumlah tool tetap 27); `drawing { "name": "sheet1" }`
  tanpa field lain merender spec tersimpan.
- [x] Tes: round-trip `.ducad` → `DrawingSpec` identik; `build` dua kali
  menghasilkan hash PDF sama.

### P21.7 — Editor GUI (≈ 5 hari)

- [x] Alat **Section**: klik dua titik pada tampak induk → garis potong +
  label berikutnya (A, B, C…), panah bisa dibalik (klik panah), stepped
  section lewat klik tambahan dengan Shift; hapus lewat tombol × ikon
  `ICON_CLOSE`.
- [x] Skala per tampak (menu konteks) dan label "SCALE 1:2" ikut terender.
- [x] Dimensi asosiatif: alat dimensi memilih fitur (snap ke pusat
  lingkaran, ujung edge) dan menyimpan `source`; dimensi ikut berubah saat
  pengguna mengubah param di panel properti; tombol "Regenerasi dimensi otomatis".
- [x] Sisipkan render berbayang; drag seperti tampak lain.
- [x] Simpan/muat lembar bersama `.ducad` (P21.6); indikator "lembar
  kedaluwarsa" bila geometri berubah setelah lembar dibuat.
- [x] Tool guide (`tool_guides.rs`) untuk Section dan Dimensi asosiatif;
  i18n id-ID/en-US; `ui_glyph_coverage` lolos.
- [x] Ceklis uji GUI (`docs/CEKLIS_UJI_GUI.md`) bagian baru **A15** "Lembar
  gambar produksi": skenario valve body dari awal sampai PDF (ditulis; belum dijalankan).

### P21.8 — Validasi akhir dengan valve body (≈ 2 hari)

- [x] Fixture `tests/fixtures/valve_body.ops.json`: badan silinder Ø98
  tinggi 88, dua flange bulat Ø146 tebal 16 dengan 4×Ø14 pada PCD Ø130,
  flange atas oval 166×98 dengan 4 lubang, bore Ø42 tembus, bore Ø60
  kedalaman 25, chamfer 2×45° dan 5×45°, fillet R5 (sesuai catatan 2 di
  acuan), material AISI 316L dari pustaka P16. Dilengkapi checks: volume,
  `min_wall_thickness ≥ 8`, jumlah lubang Ø14 = 8.
- [x] `DrawingSpec` valve body: A3 landscape 1:2, Front, Top, Section A-A
  (stepped, induk Top, vertikal), Section B-B (induk Top, horizontal, skala
  1:2 ditulis), dua shaded iso, catatan 3 baris, dimensi `auto`.
- [x] Gold PDF/SVG + perbandingan visual manual dengan gambar acuan;
  kriteria lolos di bawah.
- [x] Dokumentasi: `SKILL.md` (bagian "Gambar kerja": sections, views,
  dimensions, shaded, name/save), `docs/ci/README.md` (flag build baru),
  ADR `0007-drawing-spec.md` (kenapa `DrawingSpec` di `DesignDoc`, bukan op).

### Kriteria penerimaan

1. `ducad-cli build valve_body.ops.json --out DIR --formats pdf,svg` tanpa
   argumen tambahan menghasilkan lembar A3 yang memuat: Front, Top, Section
   A-A, Section B-B "SCALE 1:2", dua render isometrik berbayang, title block
   berisi material AISI 316L dan skala 1:2, grid zona, tiga catatan.
2. Dimensi otomatis mencakup minimal: Ø42, Ø60, Ø98, Ø146, 4×Ø14, PCD Ø130,
   tinggi 88, jarak flange 120/130/166, chamfer 2×45°, R5 di detail/section;
   tidak ada teks dimensi yang beririsan.
3. Mengubah `set_params {"bore_d": 45}` lalu `drawing {"name":"sheet1"}`
   memperbarui Ø42 → Ø45 tanpa mengubah posisi dimensi lain.
4. Lingkaran di PDF/SVG adalah busur/kurva, bukan poligon (uji: SVG tidak
   memuat polyline > 12 titik untuk tepi silinder).
5. Hash PDF/SVG identik pada dua kali `build` berturut-turut dan di CI.
6. GUI: skenario A15 (dulu disebut A14) lolos di macOS — BELUM dijalankan manual; `cargo clippy --workspace --all-targets -D warnings`,
   `cargo test --workspace`, `ui_glyph_coverage` hijau.

### Urutan, estimasi, risiko

- Urutan: P21.0 → P21.1 → P21.3 → P21.2 → P21.4 → P21.5 → P21.6 → P21.7 →
  P21.8 (P21.3 sebelum P21.2 karena skala per tampak paling mudah diuji
  dengan busur eksak; P21.4 bergantung pada `edge_refs` P21.3).
- Estimasi total ≈ 31 hari kerja; P21.1, P21.3, P21.4 adalah jalur kritis.
- **Risiko 1 — HLR eksak OCCT lambat/gagal** pada fillet kompleks:
  fallback mesh dipertahankan; batas waktu per tampak 10 s lalu fallback.
- **Risiko 2 — deadlock `lock_kernel`**: section multi-bidang memanggil
  `section_with_plane` berulang; semua di satu guard lewat helper
  `pub(crate)`.
- **Risiko 3 — ukuran PDF** karena gambar raster: batas 1600 px per
  render, peringatan bila > 4 MB.
- **Risiko 4 — kompatibilitas serde** `HlrDrawing`/`DrawingSheet` yang
  disimpan user lama: tidak ada (lembar belum pernah dipersistenkan), jadi
  pemutusan format aman sebelum P21.6 dirilis.
- **Di luar cakupan P21:** tampak bantu (auxiliary view) miring, broken-out
  section, tampak datar sheet metal otomatis (tetap di catatan P19), ekspor
  anotasi ke DXF, drawing rakitan multi-lembar.

