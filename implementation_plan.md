# Rencana Implementasi: Peta Jalan Pengembangan Fitur Lanjutan DuCAD (Fase 9 – Fase 12)

## Pandungan SANGAT PENTING DuCAD WORKFLOW
- Sidebar menu ( baik 2D / Sketch ataupun 3D ) berisi menu untuk create object yang belum ada.
- Jika harus melibatkan object yang sudah ada, misal face, atau object nya. maka workflow object di klik dg tool select, lalu menu edit itu ditempatkan di bottom menu.
- Dari tool di bottom menu, jika perlu data lebih lanjut yang inputannya sedikit, maka ditaruh di HUD menu di header ( Wajib ikuti standar design hud menu yang sudah ada ) perhatikan jangan sampai tumpang tindih dg text notifikasi. 
- Jika terlalu banyak, buat window popup di pojok kanan bawah ( Wajib ikuti design window dari history, folder atau yang lain )
- Untuk penambahan menu di implementasi ini WAJIB mengikuti arahan diatas.
- Untuk setiap pembuatan menu baru WAJIB terapkan langsung 18in ya, default text button atau informasi di aplikasi pakai ENGLISH, catatan buat programmer pakai bahasa



## Roadmap Menuju CAD Kelas Industri Kompetitor AutoCAD & Shapr3D

Dokumen ini memuat rencana implementasi teknis penambahan fitur-fitur baru pada **DuCAD** (Fase 9 hingga Fase 12) yang melengkapi fungsionalitas pemodelan 2D/3D, standar lubang pengencang (*fastener hole wizard*), bidang referensi 3D bebas (*datum planes*), gambar kerja teknik potongan (*section view*), hingga format web/AR dan perakitan (*assembly*).

---

## 1. Peta Jalan Pengembangan Bertahap (Phased Roadmap)

### Fase 9: Pemurnian Sketsa 2D & Fastener Hole Wizard
**Tujuan**: Memberikan presisi pembuatan part mekanikal presisi dan pelengkap alur sketsa cepat.

- [x] **9.1 Mode Garis Konstruksi (*Construction / Reference Line*)**:
  - Penambahan field `is_construction: bool` pada `Entity` di `ducad-sketch`.
  - Renderer `ducad-render` menggambar garis putus-putus (*dashed line*) oranye.
  - Modifikasi `ClosedRegion` di `ducad-sketch` agar garis konstruksi diabaikan dari deteksi loop tertutup sehingga tidak ikut di-extrude.
  - Shortcut keyboard `X` untuk mengubah entitas aktif menjadi garis konstruksi.
- [x] **9.2 Hole Wizard & Standar Baut ISO (*Counterbore / Countersink / Tapped*)**:
  - Integrasi generator fitur lubang di `ducad-kernel` menggunakan operasi Boolean silinder bertingkat dan tirus:
    - *Simple Hole*: Lubang silinder lurus (tembus atau berkedalaman tertentu).
    - *Counterbore Hole*: Lubang bertingkat untuk kepala baut L (*Socket Head Cap Screw*).
    - *Countersink Hole*: Lubang tirus 90° untuk baut kepala rata (*Flat Head Screw*).
    - *Tapped Hole*: Lubang ulir standar metrik (M2, M2.5, M3, M4, M5, M6, M8, M10, M12).
  - Dialog pop-up UI terintegrasi dengan tabel dimensi ISO standar.
- [x] **9.3 Regular Polygon Tool (Segi-N Beraturan)**:
  - Tool pembuatan poligon $N$-sisi (segi-3, segi-5, segi-6 heksagonal untuk kepala baut/mur, segi-8).
  - Mode penentuan ukuran: *Inscribed* (di dalam lingkaran) atau *Circumscribed* (di luar lingkaran).
- [x] **9.4 Slot Tool (Lubang Pengait / Rel Baut)**:
  - Pembuatan slot lonjong otomatis dari 2 titik pusat (*Center-to-Center Slot*) atau panjang total (*Overall Slot*) dan diameter lebar.
- [x] **9.5 2D Text on Sketch & Emboss/Deboss**:
  - Rasterisasi/vektorisasi huruf font TTF/OTF ke kurva 2D sketsa.
  - Mendukung ekstrusi timbul (*Emboss*) dan ukiran tenggelam (*Deboss/Engrave*) pada permukaan bodi 3D.

---

### Fase 10: Datum Reference Planes 3D Bebas & Geometri Spiral
**Tujuan**: Memungkinkan pembuatan geometri miring di sembarang koordinat ruang 3D dan komponen spiral/pegas.

- [ ] **10.1 Arbitrary 3D Datum Workplanes (Bidang Referensi Bebas)**:
  - Perluasan struktur `SketchPlane` dari 3 bidang tetap menjadi bidang dinamis dengan parameter `(origin, u_axis, v_axis, normal)`.
  - Metode pembuatan bidang baru:
    1. *Offset Plane*: Berjarak $d$ mm dari face planar atau plane acuan.
    2. *Angled Plane*: Memutar $\theta^\circ$ terhadap edge linear acuan.
    3. *3-Point Plane*: Membentuk bidang datar yang melalui 3 titik vertex acuan.
  - Pemilih bidang sketsa aktif terintegrasi dengan penyorotan visual transparan di viewport.
- [ ] **10.2 Helix / Coil / Spring Tool (Kurva Spiral 3D)**:
  - Generator kurva parametrik 3D:
    $$x(t) = R \cos(2\pi t), \quad y(t) = R \sin(2\pi t), \quad z(t) = \text{pitch} \cdot t$$
  - Disambungkan ke fungsi `sweep_profile_along_path` untuk menghasilkan pegas kawat (*spring*), ulir botol, dan pisau ulir sekrup (*auger*).
- [ ] **10.3 Extend Tool & Bi-Arc Parallel Offset**:
  - Algoritma `extend_segment` pada `ducad-sketch` yang memperpanjang garis sampai menyentuh batas kurva terdekat.
  - Perluasan fungsi `offset_entity` agar mendukung kurva Ellipse dan Spline menggunakan pendekatan multi-arc tangensial.
- [ ] **10.4 Variable Radius Fillet (Fillet Radius Berubah)**:
  - Integrasi API OpenCASCADE `BRepFilletAPI_MakeFillet` dengan parameter radius variabel di titik awal dan titik akhir ($R_{\text{start}} \ne R_{\text{end}}$).

---

### Fase 11: Gambar Kerja 2D Lanjutan & Web/AR Interoperability
**Tujuan**: Menghasilkan gambar teknik potongan berstandar industri dan format presentasi 3D web/AR.

- [ ] **11.1 Section View A-A (Tampak Potongan Melintang)**:
  - Ekstraksi irisan solid 3D pada bidang potong menggunakan `BRepAlgoAPI_Section`.
  - Generator pola arsiran (*Hatch pattern*) garis miring 45° standar ISO/ANSI pada area potongan solid.
  - Indikator garis potong panah `A ─── A` pada tampak acuan.
- [ ] **11.2 Detail View (Lingkaran Pembesar Skala Detail)**:
  - Pembuatan viewport lingkaran dengan faktor skala pembesar independen (2:1, 5:1, 10:1) untuk area part dengan detail mikro.
- [ ] **11.3 Manual Dimensioning Canvas**:
  - Penambahan interaksi klik 2 titik sembarang di kanvas `DrawingSheetView` untuk menambahkan garis dimensi linier, sudut, atau diameter secara kustom di luar dimensi otomatis.
- [ ] **11.4 Ekspor Format GLTF / GLB (3D Web & Augmented Reality)**:
  - Modul eksportir binary `.glb` pada `ducad-io` lengkap dengan material PBR (albedo, roughness, metallic) untuk penayangan di browser web e-commerce atau AR Quick Look di iPhone/iPad/Android.
- [ ] **11.5 Ekspor Format SVG Vektor 2D**:
  - Ekspor gambar sketsa dan tampak proyeksi 2D ke format vektor `.svg` untuk mesin laser cutting dan software grafis ilustrasi.

---

### Fase 12: Parametrik, Rakitan (Assembly) & Shell CLI
**Tujuan**: Menghadirkan kemampuan manajemen riwayat parametrik penuh, perakitan multi-komponen, dan efisiensi perintah teks.

- [ ] **12.1 Parametric History Timeline (Feature Tree)**:
  - Perekaman langkah-langkah desain dalam struktur pohon dependensi (*Directed Acyclic Graph* - DAG).
  - Kemampuan mengedit parameter sketsa masa lalu yang secara otomatis memperbarui (*regenerate*) seluruh bodi solid 3D turunan.
- [ ] **12.2 Assembly Workspace & Mate Constraints**:
  - Manajemen pohon hierarki perakitan (*Assembly Tree* dengan part mandiri).
  - Solver kendala perakitan 3D:
    - *Concentric Mate*: Mengunci sumbu silinder poros dengan lubang.
    - *Coincident Mate*: Menempelkan dua permukaan datar.
    - *Distance / Angle Mate*: Mengatur jarak atau sudut rotasi engsel.
- [ ] **12.3 Clash & Interference Detection**:
  - Uji tabrakan fisik otomatis antar bodi solid menggunakan operasi Boolean interseksi untuk mencegah kesalahan perakitan di pabrik.
- [ ] **12.4 Persistent Bottom Command Prompt Bar (CLI)**:
  - Baris input teks perintah di bagian bawah layar ala AutoCAD yang menerima perintah singkat keyboard (`L`, `C`, `REC`, `TR`, `EX`, `DIM`, `EXT`) dengan pelengkapan otomatis (*autocomplete*).
- [ ] **12.5 Tabel BOM (Bill of Materials) & Part Callout Balloons**:
  - Tabel otomatis berisi nomor item, nama part, jumlah kuantitas, dan bahan material yang terhubung dengan lingkaran nomor penunjuk pada gambar isometrik.

---

## 2. Rencana Verifikasi & Uji Mutu (Verification Plan)

### Automated Tests (Unit & Integration)
- `cargo test --workspace` untuk memvalidasi:
  - `ducad-sketch`: Hit-testing garis konstruksi, kalkulasi poligon segi-N, slot solver, dan text contour sampling.
  - `ducad-kernel`: Hole wizard B-Rep boolean, helix spine wire generator, variable fillet, dan section view slicing.
  - `ducad-io`: Validasi integritas biner ekspor GLB/GLTF, SVG vector path, dan PDF Section View hatch rendering.
  - `ducad-ui`: Event routing toolbar, dialog hole wizard, dan drawing sheet interactive canvas.

### Manual Verification Workflow
1. **Pengujian Sketsa & Lubang Fastener**:
   - Menggambar profil persegi dengan garis konstruksi sumbu tengah $\rightarrow$ Extrude menjadi blok solid $\rightarrow$ Menerapkan **Hole Wizard** lubang baut L Counterbore M6 $\rightarrow$ Ekspor ke STEP dan verifikasi geometri di software CAD lain.
2. **Pengujian Datum Plane & Spiral**:
   - Membuat **Angled Datum Plane** 45° dari tepi kubus $\rightarrow$ Menggambar sketsa di atas bidang miring tersebut $\rightarrow$ Extrude solid miring $\rightarrow$ Membuat pegas spiral dengan **Helix Tool**.
3. **Pengujian Gambar Kerja Potongan**:
   - Membuka model casing bertingkat $\rightarrow$ Masuk ke mode **Drawing Sheet** $\rightarrow$ Menyalakan **Section View A-A** $\rightarrow$ Memvalidasi tampilan arsir miring 45° dan mengekspor ke **PDF Vektor 1.4**.
