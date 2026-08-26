# Panduan Pemakaian DuCAD

Panduan komprehensif ini menjelaskan cara penggunaan seluruh fitur **DuCAD** dari pemodelan dasar hingga fitur lanjutan (Fase 1 hingga Fase 12): sketsa 2D parametrik, *datum workplanes*, pemodelan solid 3D B-Rep OpenCASCADE, *fastener hole wizard*, kurva spiral/pegas, gambar kerja teknik 2D (*engineering drawing sheet* & potongan *section view*), perakitan (*assembly*) & deteksi tabrakan (*clash detection*), *parametric history timeline*, hingga ekspor berbagai format industri (STEP, GLB/GLTF, SVG, PDF, DXF, STL, OBJ).

---

## 🚀 Menjalankan Aplikasi

```bash
cargo run -p ducad-app
```

> **Catatan Kompilasi Pertama**: Build pertama kali akan mengompilasi pustaka OpenCASCADE (OCCT) dari *source* (~8-15 menit). Kompilasi selanjutnya di-cache di folder `target/` sehingga aplikasi akan langsung terbuka secara instan.

---

## 🧭 Navigasi Kamera & ViewCube

| Aksi | Kontrol Mouse / Trackpad |
|---|---|
| **Orbit 3D** | Drag Klik-Tengah, atau Drag Klik-Kiri **saat tool Select aktif** |
| **Pan (Geser)** | `Shift + Drag`, atau Drag Klik-Kanan |
| **Zoom In / Out** | Scroll Wheel, atau Gerakan Pinch dua jari di trackpad |
| **Orbit + Zoom Dua Jari** | Trackpad / Layar Sentuh dua jari (gaya Shapr3D: 1 jari menggambar, 2 jari mengarahkan sudut pandang) |
| **ViewCube (Pojok Kanan Atas)** | Klik bidang **Top**, **Front**, **Right**, atau sudut kubus untuk orientasi tampak isometrik presisi instan |

---

## 📐 Sketsa 2D Parametrik (2D Sketching)

Sketsa dapat digambar pada bidang standar (XY, XZ, YZ) atau pada bidang kustom (*Datum Planes*).

### Daftar Tool Sketsa & Shortcut

| Tool | Shortcut | Cara Penggunaan |
|---|---|---|
| **Pilih (Select)** | `Esc` | Klik entitas untuk memilih, `Shift + Klik` untuk multi-pilih, `Delete`/`Backspace` untuk menghapus |
| **Garis (Line)** | `L` | Klik titik awal, klik titik akhir (atau ketik angka panjang + `Enter`) |
| **Persegi (Rectangle)** | `R` | Klik sudut pertama, klik sudut berlawanan (atau ketik ukuran sisi + `Enter`) |
| **Lingkaran (Circle)** | `C` | Klik titik pusat, klik untuk radius (atau ketik radius + `Enter`) |
| **Ellips (Ellipse)** | `E` | Klik titik pusat, klik untuk sumbu mayor dan minor |
| **Busur (Arc)** | `A` | Klik titik awal, klik titik akhir, lalu klik titik di busur untuk menentukan kelengkungan |
| **Poligon (Regular Polygon)** | — | Tool segi-$N$ beraturan (Segi-3, 5, 6 heksagonal kepala baut, 8). Pilih mode *Inscribed* (dalam lingkaran) atau *Circumscribed* (luar lingkaran) |
| **Slot (Lubang Baut Lonjong)**| — | Buat slot lonjong otomatis dari 2 titik pusat (*Center-to-Center*) atau panjang total (*Overall Slot*) dan lebar diameter |
| **Teks 2D (Sketch Text)** | — | Masukkan teks string, pilih font (Sans, Serif, Mono), teks langsung dikonversi menjadi kurva vektor sketsa |
| **Garis Konstruksi** | `X` | Mengubah garis aktif/terpilih menjadi garis putus-putus oranye referensi (tidak dihitung sebagai profil ekstrusi) |
| **Extend (Perpanjang)** | — | Klik segmen garis untuk memperpanjang otomatis hingga menyentuh batas kurva terdekat |
| **Offset (Sejajar)** | `O` | Klik entitas sumber (Line, Arc, Circle, Spline/Ellipse bi-arc), lalu klik arah sisi dan jarak offset |
| **Mirror (Cermin)** | `M` | Pilih entitas yang ingin dicerminkan, tekan `M`, lalu klik 2 titik yang menjadi garis sumbu cermin |
| **Trim (Potong)** | `T` | Arahkan mouse (hover) untuk melihat highlight merah sub-segmen yang akan dipotong, lalu klik untuk memotong |

### Fitur Snapping & Dynamic Input
* **Snap Otomatis**: Endpoint > Midpoint > Center > Intersection > Grid dengan indikator visual *glyph*.
* **Dynamic Input (Heads-Up Display)**: Langsung ketik angka saat menggambar lalu tekan `Enter` untuk nilai ukuran presisi.

---

## 🔗 Kendala Geometris (Geometric Constraints)

Panel **Constraint** muncul di sebelah kanan saat tool **Pilih** aktif dan ada entitas yang dipilih:

| Kombinasi Entitas | Constraint yang Tersedia |
|---|---|
| **1 Garis** | Horizontal, Vertikal, Panjang Tetap (mm) |
| **1 Lingkaran / Busur** | Radius Tetap (mm) |
| **2 Garis** | Sejajar (*Parallel*), Tegak Lurus (*Perpendicular*), Sama Panjang (*Equal*), Sudut Derajat (*Angle*) |
| **2 Lingkaran / Busur** | Sama Radius (*Equal Radius*), Bersinggungan (*Tangent*) |
| **1 Garis + 1 Lingkaran/Busur**| Bersinggungan (*Tangent*) |
| **2 Titik** | Berimpit (*Coincident*), Titik Kunci (*Fixed*), Simetris (*Symmetric*) |

---

## 🌐 Datum Workplanes (Bidang Referensi 3D Bebas)

Memungkinkan pembuatan sketsa dan geometri pada sudut atau posisi sembarang di ruang 3D:

1. **Akses**: Buka drawer **Datum Planes** dari toolbar atau menu pop-up.
2. **Metode Pembuatan Bidang Baru**:
   - **Offset Plane**: Masukkan jarak offset $d$ mm dari face planar atau plane acuan.
   - **Angled Plane**: Putar sebesar sudut $\theta^\circ$ terhadap edge acuan.
   - **3-Point Plane**: Klik 3 titik vertex di ruang 3D untuk membentuk bidang datar.
3. **Mengaktifkan Bidang**: Klik tombol *Set Active* pada daftar bidang untuk mulai menggambar sketsa pada bidang tersebut.

---

## 🧊 Pemodelan Solid 3D (OpenCASCADE B-Rep)

### 1. Extrude (Ekstrusi)
* Pilih profil tertutup di sketsa $\rightarrow$ Klik **Extrude** di panel atau Bottom Context Bar $\rightarrow$ Tentukan jarak (mm) dan arah (Satu arah, Simetris dua arah).

### 2. Revolve (Putar Profil)
* Pilih profil tertutup $\rightarrow$ Tekan `V` atau klik **Revolve** $\rightarrow$ Pilih sumbu putar (Sumbu X, Sumbu Y, garis tepi profil, atau 2 titik manual) $\rightarrow$ Tentukan sudut putar (90°, 180°, 360° penuh) $\rightarrow$ Eksekusi.

### 3. Loft (Penyambung Antar Profil)
* Pilih profil bawah $\rightarrow$ Klik **Set Profil Bawah** $\rightarrow$ Pilih profil atas pada ketinggian $Z$ $\rightarrow$ Klik **Loft**.

### 4. Helix / Coil / Spring (Geometri Spiral 3D)
* Buka **Helix Tool** $\rightarrow$ Masukkan Radius $R$, Pitch (jarak antar ulir), Ketinggian/Jumlah Putaran (*Turns*), dan Radius Kawat Profil $\rightarrow$ Hasilkan solid pegas kawat spiral, sudu auger, atau ulir botol secara otomatis.

### 5. Fastener Hole Wizard (Standar ISO Baut)
* Klik face planar $\rightarrow$ Buka **Hole Wizard Dialog**:
  * **Simple Hole**: Lubang bor silindris standar (Tembus / *Through All* atau Kedalaman *Blind*).
  * **Counterbore**: Lubang bertingkat untuk kepala baut L (*Socket Head Cap Screw*).
  * **Countersink**: Lubang tirus 90° untuk baut kepala rata (*Flat Head*).
  * **Tapped**: Lubang ulir metrik standar (pilihan cepat M2, M2.5, M3, M4, M5, M6, M8, M10, M12).
* Posisi lubang ditempatkan otomatis pada titik koordinat yang ditentukan.

### 6. Operasi Boolean Solid
* Pilih 2 solid di daftar/viewport:
  * **Union**: Menggabungkan dua bodi menjadi satu kesatuan.
  * **Subtract (Cut)**: Memotong bodi utama dengan bodi pemotong.
  * **Intersect**: Menyisakan hanya bagian volume yang saling tumpang tindih.

### 7. Fillet, Variable Radius Fillet, & Chamfer
* **Fillet Konstan**: Membulatkan seluruh tepi atau tepi terpilih dengan radius $R$.
* **Variable Radius Fillet**: Membulatkan tepi dengan radius berbeda di awal dan akhir ($R_{\text{start}} \ne R_{\text{end}}$).
* **Chamfer**: Membuat pingulan miring dengan jarak $d$ mm.

### 8. Shelling (Pengosongan Rongga) & Draft Angle
* **Shell**: Pilih bodi, tentukan ketebalan dinding, dan pilih face yang ingin dibuang untuk membuat casing berongga.
* **Draft Angle**: Menambahkan sudut kemiringan cetakan (*injection molding draft*) pada dinding bodi solid.

### 9. 3D Text Emboss & Deboss
* Buat teks sketsa pada permukaan face $\rightarrow$ Pilih opsi **Emboss** (teks timbul keluar) atau **Deboss / Engrave** (ukiran teks tenggelam ke dalam solid).

---

## 📑 Lembar Gambar Kerja 2D (Drawing Sheet & Blueprint)

Beralih ke mode **Drawing Sheet** dari top bar untuk membuat cetak biru teknik berstandar industri:

1. **Tampak Proyeksi Otomatis**:
   - Menghasilkan tampak Atas (*Top*), Depan (*Front*), Samping (*Right*), dan Isometrik 3D lengkap dengan algoritma *Hidden Line Removal* (HLR).
2. **Section View A-A (Tampak Potongan)**:
   - Membuat irisan melintang pada bidang potong yang dilengkapi dengan pola arsir (*Hatch pattern*) garis miring 45° standar ISO/ANSI dan garis panah pemotong `A ─── A`.
3. **Detail View (Lingkaran Pembesar)**:
   - Menambahkan lingkaran viewport pembesar dengan skala khusus (2:1, 5:1, 10:1) untuk area detail mikro.
4. **Dimensi Otomatis & Dimensi Manual**:
   - Klik 2 titik pada kanvas untuk menambahkan dimensi linier kustom, dimensi diameter lingkaran, atau sudut derajat.
5. **Tabel BOM (Bill of Materials) & Part Balloons**:
   - Menampilkan tabel otomatis nomor item, nama part, jumlah kuantitas, dan bahan material yang terhubung langsung ke balon penunjuk pada gambar isometrik.
6. **Kepala Gambar (ISO Title Block)**:
   - Bingkai standar teknik lengkap dengan judul proyek, nama perancang, tanggal, unit, dan nomor revisi.

---

## ⚙️ Perakitan (Assembly) & Uji Tabrakan (Clash Detection)

Buka drawer **Assembly** di panel kiri:

1. **Pohon Perakitan (*Assembly Tree*)**: Mengelola hierarki part komponen dan instance mandiri.
2. **Mate Constraints 3D**:
   - **Concentric Mate**: Mengunci keselarasan sumbu silinder poros dengan lubang part pasangannya.
   - **Coincident Mate**: Menempelkan dua permukaan datar saling berimpit.
   - **Distance / Angle Mate**: Mengatur jarak offset atau sudut engsel mekanis.
3. **Clash & Interference Detection**:
   - Klik **Run Clash Test** untuk mendeteksi tabrakan fisik antar bodi solid di seluruh perakitan. Sistem akan menghitung volume interferensi dan menyorot bagian yang bertabrakan sebelum proses manufaktur.

---

## 🕒 Parametric History Timeline (Feature Tree)

1. Buka drawer **History / Feature Tree** untuk melihat urutan langkah perancangan berbasis graf dependensi (DAG).
2. Anda dapat memilih langkah fitur masa lalu (misal *Extrude 1* atau *Sketch 2*), mengubah parameternya (seperti dimensi atau ketebalan), dan sistem akan secara otomatis meregenerasi seluruh bodi solid turunan.

---

## 🎨 Studio Lingkungan Pencahayaan & Material (CMF)

1. Buka drawer **Lighting & CMF Studio**:
   - Pilih preset suasana pencahayaan: *Warm Studio*, *Cool Tech*, *High Contrast*, *Sunset Gold*, atau *Cyberpunk Neon*.
   - Atur intensitas lampu, rotasi sumber cahaya, dan efek bayangan realistis **SSAO** (*Screen Space Ambient Occlusion*).
   - Terapkan warna dan sifat material fisik PBR (*Metallic*, *Roughness*).

---

## 💾 Manajemen Berkas (File I/O)

Menu **📄 File** di pojok kiri atas atau via Command Palette (`Ctrl/Cmd+K`):

### Berkas Asli DuCAD
* **Simpan (`Ctrl/Cmd+S`)** / **Buka (`Ctrl/Cmd+O`)**: Berkas `.ducad` menyimpan seluruh data sketsa 2D, B-Rep solid 3D, riwayat parametrik, dan perakitan.

### Format Impor & Ekspor

| Format | Mode | Keterangan |
|---|---|---|
| **STEP (`.step`, `.stp`)** | Import & Export | Standar pertukaran solid CAD B-Rep untuk CAM/CNC |
| **GLTF / GLB (`.glb`)** | Export | Format 3D biner dengan PBR untuk penayangan Web & AR Quick Look di iOS/Android |
| **SVG (`.svg`)** | Export | Format vektor 2D untuk mesin Laser Cutting dan CNC Router |
| **PDF (`.pdf`)** | Export | Gambar teknik vektor resolusi tinggi lengkap dengan kop ISO, arsir potongan, dan tabel BOM |
| **DXF (`.dxf`)** | Import & Export | Format sketsa 2D AutoCAD |
| **STL (`.stl`)** | Export | Format biner mesh untuk 3D Printing / Slicer |
| **OBJ / PLY / 3MF** | Export | Format model 3D poligon standar |

---

## 💡 Alur Kerja Ergonomis Standar DuCAD

DuCAD menerapkan hierarki interaksi konsisten untuk kecepatan kerja maksimal:
1. **Sidebar Kiri**: Menu untuk membuat objek baru yang belum ada (Sketsa 2D, Bodi 3D, Assembly, Datum Plane).
2. **Bottom Context Bar**: Menu interaktif yang muncul otomatis di bagian bawah saat sebuah objek, face, atau edge dipilih dengan tool *Select*.
3. **Canvas HUD (Header Atas)**: Kotak masukan angka/parameter ringkas saat menggambar.
4. **Pop-up Window (Kanan Bawah)**: Kotak dialog konfigurasi mendalam untuk fitur kompleks (Hole Wizard, Helix, Draft, 3D Text).
5. **Command Palette (`Ctrl/Cmd+K`)**: Pencarian instan seluruh perintah aplikasi dengan keyboard.
