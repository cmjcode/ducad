# DuCAD (Design Universe CAD)

[![Rust](https://img.shields.io/badge/rust-2021_edition-orange.svg)](https://www.rust-lang.org)
[![Kernel](https://img.shields.io/badge/B--Rep%20Kernel-OpenCASCADE%20(OCCT)-blue.svg)](https://dev.opencascade.org)
[![Graphics](https://img.shields.io/badge/Renderer-wgpu%20/%20WebGPU-green.svg)](https://wgpu.rs)
[![UI](https://img.shields.io/badge/UI-egui%20/%20eframe-purple.svg)](https://github.com/emilk/egui)
[![License](https://img.shields.io/badge/license-MIT%20/%20Apache--2.0-blue.svg)](LICENSE)

**DuCAD** adalah software Computer-Aided Design (CAD) 2D/3D modern, parametrik, dan berkinerja tinggi yang ditulis murni dalam **Rust**. DuCAD menggabungkan presisi penyusunan draf teknik 2D ala **AutoCAD** dengan kemudahan pemodelan langsung (*direct modeling*) intuitif ala **Shapr3D**, didukung oleh kekuatan *solid modeling kernel* kelas industri **OpenCASCADE (OCCT)** dan akselerasi grafis modern **WebGPU (wgpu)**.

---

## 🌟 Fitur Utama

### 1. 📐 Sketsa 2D Parametrik & Geometri Presisi
* **Entitas Lengkap**: Line, Rectangle, Circle, Center-Radius Arc, 3-Point Arc, Ellipse, Regular Polygon ($N$-sisi Inscribed/Circumscribed), dan Slot (Center-to-Center & Overall).
* **Garis Konstruksi (*Construction Line*)**: Beralih mode referensi (`X`) dengan rendering garis putus-putus oranye tanpa mengganggu deteksi profil solid tertutup (*closed region*).
* **Teks Sketsa 2D (*2D Text*)**: Vektorisasi tipografi font TrueType/OpenType ke kurva sketsa untuk ekstrusi teks.
* **Sistem Snapping Cerdas**: Prioritas bertingkat (*Endpoint* > *Midpoint* > *Center* > *Intersection* > *Grid*) dengan visual glyph interaktif.
* **Geometric & Dimensional Constraint Solver**: Coincident, Fixed, Horizontal, Vertical, Parallel, Perpendicular, Equal Length/Radius, Distance, Radius, Tangent, Angle, dan Symmetric.
* **Modifikasi Kurva Sketsa**: Trim interaktif dengan highlight merah, Extend kurva ke batas terdekat, Offset paralel (bi-arc multi-tangent), dan Mirror refleksi simetris.

### 2. 🧊 Pemodelan Solid B-Rep 3D Kelas Industri (OpenCASCADE)
* **Operasi Ekstrusi & Putar**: Extrude (Blind, Symmetric, Up to Face), Revolve dengan sumbu kustom 3D, Loft multi-profil, dan Sweep sepanjang kurva pemandu.
* **Geometri Spiral (*Helix / Spring / Coil*)**: Generator parametrik kurva 3D untuk pembuatan pegas, ulir baut, dan sudu *auger*.
* **Operasi Boolean Solid**: Boolean Union (Gabung), Cut (Potong/Kurang), dan Intersect (Irisan).
* **Fitur Tepi & Dinding**: Fillet konstan, **Variable Radius Fillet** ($R_{\text{start}} \ne R_{\text{end}}$), Chamfer tepi, Thin-Wall Shelling (pengosongan rongga dalam), dan Draft angle.
* **Teks 3D Emboss & Deboss**: Menempelkan teks timbul (*emboss*) atau ukiran tenggelam (*deboss/engrave*) pada permukaan planar part.
* **Fastener Hole Wizard (Standar ISO)**:
  * *Simple Hole*: Lubang silinder lurus (tembus atau berkedalaman tertentu).
  * *Counterbore Hole*: Lubang bertingkat untuk baut kepala L (*Socket Head Cap Screw*).
  * *Countersink Hole*: Lubang tirus 90° untuk baut kepala rata (*Flat Head Screw*).
  * *Tapped Hole*: Lubang ulir metrik standar (M2, M2.5, M3, M4, M5, M6, M8, M10, M12).

### 3. 🌐 Datum Workplanes (Bidang Referensi 3D Bebas)
* Buat bidang sketsa dan pemodelan pada koordinat ruang 3D mana pun:
  * **Offset Plane**: Berjarak offset $d$ mm dari face/plane acuan.
  * **Angled Plane**: Diputar sudut $\theta^\circ$ terhadap tepi/garis linear acuan.
  * **3-Point Plane**: Dibentuk dari 3 titik vertex sembarang di ruang 3D.
* Visualisasi bidang transparan di viewport dan manajemen daftar bidang (*Planes Drawer*).

### 4. 📑 Gambar Kerja 2D (Engineering Drawing Sheet & ISO Blueprint)
* **Tampak Proyeksi Multi-View**: Tampak Atas (*Top*), Tampak Depan (*Front*), Tampak Samping (*Right*), dan Tampak Isometrik (*Isometric 3D*).
* **Hidden Line Removal (HLR)**: Ekstraksi garis tampak tajam dan garis tersembunyi berarsir/putus-putus.
* **Section View A-A (Tampak Potongan Melintang)**: Irisan solid 3D dengan pola arsir (*Hatch pattern*) 45° standar ISO/ANSI dan garis potong berpanah.
* **Detail View (Lingkaran Pembesar Skala)**: Viewport pembesar detail mikro independen (skala 2:1, 5:1, 10:1).
* **Dimensi Otomatis & Manual**: Garis dimensi linier, diameter lubang, radius busur, sudut derajat, dan teks anotasi bebas pada kanvas.
* **Tabel BOM (Bill of Materials) & Part Callout Balloons**: Tabel otomatis nomor komponen, kuantitas, material, terhubung dengan balon lingkaran penunjuk nomor part.
* **Kepala Gambar (ISO Title Block)**: Bingkai standar gambar teknik lengkap dengan informasi proyek, skala, perancang, dan tanggal.

### 5. ⚙️ Perakitan (Assembly) & Uji Tabrakan (Clash Detection)
* **Hierarki Pohon Perakitan (*Assembly Tree*)**: Manajemen multi-part dan multi-instance mandiri.
* **3D Mate Constraints**: Concentric Mate (sumbu silinder), Coincident Mate (permukaan rata berimpit), Distance & Angle Mate.
* **Clash & Interference Detection**: Uji tabrakan fisik otomatis antar bodi solid menggunakan operasi Boolean interseksi untuk mendeteksi interferensi part sebelum fabrikasi.

### 6. 🕒 Parametric History Timeline (Feature Tree)
* Perekaman langkah-langkah desain dalam struktur graf dependensi (*Directed Acyclic Graph* - DAG).
* Modifikasi parameter fitur masa lalu dengan kemampuan regenerasi otomatis seluruh geometri solid turunan.

### 7. 🔄 Interoperabilitas Format Berkas Luas
* **Import**:
  * `STEP` (`.step`, `.stp`) — Impor model CAD standar internasional B-Rep.
  * `DXF` (`.dxf`) — Impor sketsa vektor 2D AutoCAD R12/2000+.
  * Native `.ducad` — Format berkas dokumen berbasis JSON yang menyimpan geometri B-Rep, sketsa, dan riwayat.
* **Export**:
  * `STEP` (`.step`, `.stp`) — Ekspor solid B-Rep penuh untuk manufaktur CNC/CAM.
  * `GLTF / GLB` (`.glb`) — Format biner 3D Web & Augmented Reality (AR Quick Look di iOS/Android) dengan material PBR.
  * `SVG` (`.svg`) — Format vektor 2D untuk mesin Laser Cutting, CNC Router, dan software grafis.
  * `PDF` (`.pdf`) — Format gambar kerja teknik vektor ISO 1.4 resolusi tinggi dengan pola arsir potongan.
  * `STL` (`.stl` Binary), `OBJ`, `PLY`, `3MF` — Format mesh untuk 3D Printing / Slicer.

### 8. 🎨 UI/UX Alur Kerja Modern & Studio Rendering
* **Standar Alur Kerja Ergonomis DuCAD**:
  * *Sidebar Kiri*: Menu pembuatan objek baru (2D Sketch / 3D Solid / Assembly).
  * *Bottom Context Bar*: Menu pengeditan kontekstual untuk objek/face yang sedang dipilih dengan tool Select.
  * *Header Canvas HUD*: Input parameter cepat dan ringkas yang tidak mengganggu alur visual.
  * *Pop-up Dialog Kanan Bawah*: Konfigurasi mendalam untuk fitur kompleks (Hole Wizard, Helix, Draft, Text, Booleans).
* **Command Palette (`Ctrl/Cmd+K`)**: Akses instan ke seluruh tool dan perintah via pencarian teks cepat.
* **Radial Menu (`Space`)**: Menu melingkar di bawah kursor mouse untuk akses cepat tool esensial.
* **ViewCube 3D**: Kontrol orientasi kamera kubus interaktif (Top, Front, Right, Isometric, Orbit).
* **Studio Lighting & Material (SSAO & PBR)**: Pengaturan lingkungan pencahayaan (Warm Studio, Cool Tech, High Contrast, Sunset Gold, Cyberpunk Neon) dengan Screen Space Ambient Occlusion.
* **Dukungan Multi-Bahasa (i18n)**: 18+ bahasa dengan antarmuka default Bahasa Inggris dan catatan ramah pengembang.

---

## 🏗️ Struktur Arsitektur Workspace

DuCAD dibangun dengan arsitektur modular *multi-crate*:

```
DUCAD/
├── crates/
│   ├── ducad-core/      # Model data dokumen, riwayat undo/redo, pohon perakitan, mate, unit
│   ├── ducad-sketch/    # Mesin sketsa 2D, entitas geometri, constraint solver, snapping, region solver
│   ├── ducad-kernel/    # Pembungkus B-Rep OpenCASCADE (OCCT): boolean, fillet, hole, helix, section, mesh
│   ├── ducad-render/    # Engine rendering wgpu: kamera 3D, shader PBR, SSAO, grid, sketch overlay
│   ├── ducad-io/        # Modul import/export STEP, GLB/GLTF, SVG, PDF (drawing sheet), DXF, STL, OBJ
│   ├── ducad-ui/        # Komponen UI egui: toolbar, context bar, HUD, drawing sheet canvas, drawers, popups
│   ├── ducad-i18n/      # Sistem lokalisasi dan kamus terjemahan 18+ bahasa
│   └── ducad-app/       # Aplikasi utama, loop event winit/eframe, manajemen window, integrasi state
├── docs/                # Dokumentasi panduan operasional, perbandingan CAD, dan cetak biru arsitektur
└── Cargo.toml           # Workspace root manifest
```

---

## 🚀 Memulai (Getting Started)

### Prasyarat Sistem
* **Rust Toolchain**: Rust versi terbaru (1.75+ stabil disarankan) melalui `rustup`.
* **C/C++ Compiler & CMake**: CMake ≥ 3.16 dan C++17 compiler (Clang/GCC/MSVC) untuk mengompilasi kernel OpenCASCADE (OCCT).
* **Sistem Operasi**: macOS (Apple Silicon & Intel), Linux (X11 / Wayland), Windows 10/11.

### Menjalankan DuCAD

Kloning repositori dan jalankan melalui Cargo:

```bash
# Clone repositori
git clone https://github.com/cmjcode/ducad.git
cd DUCAD

# Jalankan aplikasi (proses kompilasi pertama akan membangun kernel OCCT ~8-15 menit)
cargo run -p ducad-app
```

> **Tips Kompilasi Pertama Kali**: Kompilasi awal dari *source* `occt-sys` memerlukan waktu beberapa menit untuk membangun seluruh pustaka OpenCASCADE C++. Hasil build akan di-cache secara permanen di direktori `target/` sehingga kompilasi berikutnya berjalan instan.

### Menjalankan Unit & Integrasi Test

```bash
cargo test --workspace
```

---

## ⌨️ Pintasan Keyboard Utama (Keyboard Shortcuts)

| Kategori | Pintasan | Fungsi |
|---|---|---|
| **Navigasi 3D** | `Klik Tengah Drag` / `Klik Kiri Drag` (Tool Select) | Orbit Kamera 3D |
| | `Shift + Drag` / `Klik Kanan Drag` | Pan (Geser) Kamera |
| | `Scroll Wheel` / `Pinch Trackpad` | Zoom In / Out |
| **Tool Sketsa** | `Esc` | Batal / Kembali ke Tool Select |
| | `L` | Tool Garis (*Line*) |
| | `R` | Tool Persegi (*Rectangle*) |
| | `C` | Tool Lingkaran (*Circle*) |
| | `A` | Tool Busur (*Arc*) |
| | `E` | Tool Ellips (*Ellipse*) |
| | `T` | Tool Potong (*Trim*) |
| | `O` | Tool Garis Sejajar (*Offset*) |
| | `M` | Tool Cermin (*Mirror*) |
| | `X` | Ubah Garis Konstruksi (*Toggle Construction*) |
| **Aplikasi** | `Ctrl/Cmd + K` | Buka Command Palette (Pencarian Perintah) |
| | `Space` | Buka Radial Menu di Kursor |
| | `Ctrl/Cmd + Z` | Undo Aksi |
| | `Ctrl/Cmd + Shift + Z` / `Ctrl + Y` | Redo Aksi |
| | `Ctrl/Cmd + S` | Simpan Dokumen (`.ducad`) |
| | `Ctrl/Cmd + O` | Buka Berkas Dokumen |
| | `Delete` / `Backspace` | Hapus Entitas / Objek Terpilih |

---

## 📚 Dokumentasi Terkait

* [Panduan Pemakaian Lengkap (User Manual)](file:///Users/jayuda/Documents/PROJECT/DUCAD/docs/PANDUAN.md) — Panduan mendalam cara penggunaan setiap tool dan fitur dari pemodelan hingga gambar kerja.
* [Analisis Komparatif CAD](file:///Users/jayuda/Documents/PROJECT/DUCAD/docs/ANALISIS_KOMPARATIF_CAD.md) — Studi perbandingan fitur teknis DuCAD terhadap AutoCAD, SolidWorks, Onshape, dan Shapr3D.
* [Peta Jalan & Pelacakan Fase](file:///Users/jayuda/Documents/PROJECT/DUCAD/implementation_plan.md) — Detail status implementasi teknis setiap fase dan modul.

---

## 📄 Lisensi

Proyek ini dilisensikan di bawah lisensi ganda [MIT License](LICENSE) atau [Apache License 2.0](LICENSE-APACHE).
