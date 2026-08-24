# Rencana Implementasi: Fitur Desain Industri (Industrial Design) DUCAD
## Komparasi Menu DUCAD vs AutoCAD vs Shapr3D dan Peta Jalan Pengembangan

Dokumen ini memuat analisis perbandingan komprehensif antara **DUCAD**, **AutoCAD**, dan **Shapr3D**, identifikasi kesenjangan fitur (*feature gaps*) yang krusial untuk **Desain Industri (Industrial Design)**, serta peta jalan implementasi teknis berbasis kernel **OpenCASCADE (OCCT)**.

---

## 1. Analisis Komparatif: DUCAD vs AutoCAD vs Shapr3D

| Kategori Fitur | AutoCAD | Shapr3D (Parasolid Kernel) | DUCAD (OpenCASCADE Kernel) | Kebutuhan Desain Industri & Manufaktur |
| :--- | :--- | :--- | :--- | :--- |
| **2D Curves & Organic Sketching** | Line, Polyline, Spline (Fit/CV), Arc, Circle, Ellipse, Helix | Line, Arc, Circle, Ellipse, Spline (Control Point & Fit Point) | Line, Rectangle, Circle, Ellipse, Arc, **Spline (Catmull-Rom $C^1$)** *(Baru)* | **Kritis**: Produk konsumen (mouse, remote, botol, casing TWS) didominasi kurva organik bebas lipatan. |
| **2D Sketch Fillet & Chamfer** | Fillet, Chamfer (dengan live radius) | Fillet & Chamfer langsung di sudut vertex sketch | Belum ada di sketch (hanya ada di 3D body) | **Penting**: Membulatkan sudut profil sketsa sebelum diextrude. |
| **Sweep along Path (3D)** | Sweep Tool (Profil + Path 3D / 2D) | Sweep Tool (Otomatis deteksi path & guide curve) | Belum ada (hanya Linear Extrude & Loft) | **Kritis**: Handle pintu, perpipaan, kabel, frame kacamata, gasket karet. |
| **Draft Angle (Kemiringan Cetakan)**| Manual / Tapered Extrude | Draft Tool (Pilih Netral Plane + Face + Sudut) | Belum ada | **Wajib DFM (Design for Manufacturing)**: Injeksi plastik & die-cast wajib sudut kemiringan (0.5° - 3°) agar part bisa keluar cetakan. |
| **Split Body & Split Face** | Slice, SectionPlane | Split Body (dengan Plane/Face), Split Face | Belum ada | **Kritis**: Membagi casing menjadi part atas & bawah (*parting line*) dan membagi area warna/tekstur berbeda. |
| **Pattern / Array (2D & 3D)** | Rectangular, Polar, Path Array | Linear Pattern, Circular Pattern | Belum ada | **Kritis**: Kisi speaker (*speaker grille*), ventilasi heatsink, susunan lubang baut melingkar. |
| **Inspeksi Kontinuitas (Zebra)** | Zebra Analysis, Curvature Analysis | Zebra Stripes Shader, Curvature Map | Belum ada (hanya Section View & Dimensi) | **Kritis**: Evaluasi pantulan cahaya pada permukaan kelas A (*Class-A Surface*) untuk memastikan keluwesan G1/G2. |
| **Inspeksi Sudut Lepas (Draft Heatmap)**| Draft Analysis | Draft Analysis (Heatmap merah/hijau/kuning) | Belum ada | **Penting**: Menghindari *undercut* cetakan sebelum kirim file ke pabrik cetakan injeksi. |
| **Material & Rendering Visual** | Materials, Realistic Render | PBR Industrial Materials (Metal, Plastic, Glass) | Default Phong Shading (1 warna bodi) | **Penting**: Visualisasi estetika material sebelum masuk lini produksi. |
| **Drawing Sheets (Gambar Kerja 2D)**| 2D Layouts, Viewports, Title Blocks, GD&T | 2D Drawings Mode, Auto Projected Views, PDF | Belum ada (hanya ekspor sketsa DXF) | **Penting**: Gambar teknik ortogonal (Tampak Depan/Atas/Samping/Isometrik) untuk vendor fabrikasi. |

---

## 2. Peta Jalan Pengembangan Bertahap (Roadmap)

### Fase 1: Essential Curves & 3D Sweep (Sedang Berjalan)
Tujuan: Melengkapi kapabilitas pembuatan bentuk kurva organik 2D dan sweep 3D.
- [x] **1.1 2D Spline Entity (Catmull-Rom Interpolation)**:
  - Interpolasi $C^1$ kontinu yang melalui setiap titik kontrol input.
  - Universal snapping pada titik awal, akhir, kontrol, serta in-progress snapping.
  - Konversi halus Bi-Arc parametrik analitik ke kernel OpenCASCADE (`convert_spline_to_smooth_segments`) agar permukaan solid 3D mulus tanpa patahan.
- [ ] **1.2 2D Sketch Fillet & Chamfer Tool**:
  - Membulatkan sudut pertemuan 2 garis di mode sketsa dengan radius interaktif sebelum diextrude.
- [ ] **1.3 3D Sweep along Path**:
  - Implementasi `ducad_kernel::sweep_profile_along_wire` menggunakan `BRepOffsetAPI_MakePipe`.
  - UI pemilih profil dan jalur kurva di kanvas 3D.

### Fase 2: Fitur Manufaktur Produk Industri (DFM & Casing)
Tujuan: Memungkinkan perancangan casing produk injeksi plastik dan part mekanikal presisi.
- [ ] **2.1 Draft Angle (Taper Face Tool)**:
  - Integrasi `BRepOffsetAPI_DraftAngle` dari OpenCASCADE.
  - Parameter: Neutral Plane, Faces to draft, Angle (°).
- [ ] **2.2 Split Body & Split Face**:
  - Integrasi `BRepAlgoAPI_Splitter`.
  - Memotong bodi menjadi 2 solid terpisah menggunakan bidang (*cutting plane*) atau permukaan kurva (*parting surface*).
- [ ] **2.3 Linear & Circular Pattern (2D & 3D)**:
  - Array linier ($X, Y, Z$ dengan jarak *pitch* dan jumlah *count*).
  - Array sirkular (sumbu putar, sudut total / sudut per item, *count*).
- [ ] **2.4 Shell with Variable Thickness / Rib Support**:
  - Pengembangan fitur penipisan dinding casing (*hollow casing*) dan penambahan tulang penguat (*ribs*).

### Fase 3: Analisis & Inspeksi Kualitas Permukaan (Quality Inspection)
Tujuan: Memberikan alat verifikasi visual kelayakan desain industri secara *real-time*.
- [ ] **3.1 Zebra Stripes Reflection Shader**:
  - Fragment shader kustom pada pipeline `ducad-render` (wgpu) yang memproyeksikan garis zebra pantulan (*specular reflection stripes*) untuk memvalidasi kontinuitas tangensial (G1) dan kurvatur (G2).
- [ ] **3.2 Draft Angle Heatmap Inspector**:
  - Shader pewarnaan sudut permukaan terhadap arah buka cetakan (*pull direction*):
    - **Hijau**: Sudut aman $\ge 1.0^\circ$.
    - **Kuning**: Sudut kritis $0^\circ - 1.0^\circ$.
    - **Merah**: *Undercut* / kemiringan terbalik (tidak bisa lepas cetakan).

### Fase 4: Material Industri & Presentasi Visual (CMF Visualizer)
Tujuan: Menampilkan pratinjau warna, material, dan finishing (Color, Material, Finish).
- [ ] **4.1 Preset Material Desain Industri**:
  - *Matte Texture Plastic (ABS/PC)*
  - *Glossy Plastic*
  - *Anodized Brushed Aluminum*
  - *Polished Chrome / Stainless Steel*
  - *Translucent Glass / Clear Acrylic*
- [ ] **4.2 Studio Lighting & Screen Space Ambient Occlusion (SSAO)**:
  - Pencahayaan studio 3-titik dan bayangan lembut pada celah kontak lantai.

### Fase 5: Gambar Kerja Teknik 2D (Engineering Drawing Sheets)
Tujuan: Menghasilkan gambar kerja pabrikasi dari model solid 3D.
- [ ] **5.1 Ekstraksi Proyeksi Ortogonal (HLR - Hidden Line Removal)**:
  - Menggunakan modul OpenCASCADE `HLRBRep_Algo` untuk menghasilkan garis tampak dan garis putus-putus (*hidden lines*) dari tampak Atas, Depan, Samping, dan Isometrik.
- [ ] **5.2 Sheet Layout & Ekspor PDF / DXF**:
  - Kertas standar (A4/A3), kepala gambar (*title block*), dimensi otomatis, dan ekspor PDF vektor beresolusi tinggi.

---

## 3. Rencana Verifikasi (Verification Plan)

### Automated Tests
- `cargo test --workspace` untuk memvalidasi seluruh crate:
  - `ducad-sketch`: Hit-testing, snapping, sampling kurva, closed region solver.
  - `ducad-kernel`: B-Rep geometry construction, sweep, draft angle, split body, tessellation.
  - `ducad-render`: Shaders, tessellation mesh generation, lighting buffer.
  - `ducad-ui` & `ducad-i18n`: String translations & toolbar event propagation.

### Manual Verification
- Uji alur pembuatan produk industri lengkap di UI:
  1. Menggambar profil kurva organik menggunakan **Spline** dan **Arc**.
  2. Extrude bodi utama dengan panah **Gizmo Extrude**.
  3. Menerapkan **Draft Angle** pada sisi samping.
  4. Memotong casing atas & bawah dengan **Split Body**.
  5. Menyalakan **Zebra Stripe View** untuk mengecek kemulusan refleksi cahaya.
