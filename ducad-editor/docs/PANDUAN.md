# Panduan Pemakaian DuCAD

Panduan komprehensif ini menjelaskan cara penggunaan seluruh fitur **DuCAD** dari pemodelan dasar hingga fitur lanjutan (Fase 1 hingga Fase 12): sketsa 2D parametrik, *datum workplanes*, pemodelan solid 3D B-Rep, *fastener hole wizard*, kurva spiral/pegas, gambar kerja teknik 2D (*engineering drawing sheet* & potongan *section view*), perakitan (*assembly*) & deteksi tabrakan (*clash detection*), *parametric history timeline*, hingga ekspor berbagai format industri (STEP, GLB/GLTF, SVG, PDF, DXF, STL, OBJ).

> **Status fitur**: label *(terbatas: …)* dan *(eksperimental)* di panduan ini mengikuti
> [`STATUS_KEMAMPUAN.md`](STATUS_KEMAMPUAN.md). Fitur tanpa label diuji otomatis pada alur yang
> dijelaskan. Batas tiap format berkas ada di [`FORMAT_MATRIX.md`](FORMAT_MATRIX.md).
> Disinkronkan dengan kode pada Step 5 `PLAN_DESIGNER_UX.md` (2026-10-21).

---

## 🚀 Menjalankan Aplikasi

```bash
cargo run -p ducad-app
```

> **Kernel geometri**: DuCAD memakai kernel B-rep SolidCraft/truck (Rust murni, tanpa kompilasi C++), sehingga build pertama hanya mengompilasi kode Rust. Setiap panik kernel dicatat di `~/.ducad/logs/panics.jsonl` (lokal saja, tidak dikirim ke mana pun); lampirkan berkas itu saat melaporkan masalah.

---

## ⚡ Mulai Cepat: Proyek Contoh

Cara tercepat mengenal alur revisi DuCAD. Saat aplikasi dibuka pertama kali (atau lewat tombol
bantuan di kanan atas / Command Palette → **Tutorial: mulai dari awal**), kartu sambutan
menawarkan bab **Mulai cepat: proyek contoh**. Klik **Buka proyek contoh**: DuCAD membuka
bracket L contoh yang sudah jadi, lalu memandu lima langkah:

1. **Ubah parameter** — buka palet perintah (`Ctrl/Cmd+Shift+P`), ketik "parameter", Enter.
   Di panel **Parameter**, ubah `len` dari 60 menjadi 80, lalu klik **Terapkan**. Extrude,
   fillet, dan keempat lubang ikut menyesuaikan.
2. **Ubah satu fitur** — klik tombol garis waktu di baris ikon kanan bawah (**Pohon Fitur &
   Garis Waktu** terbuka). Di seksi **Riwayat Parametrik (oplog)**, klik ikon pensil pada op
   `inner_fillet`, ketik 4, lalu **Terapkan**.
3. **Batalkan** — tekan `Ctrl/Cmd+Z` sekali. Satu undo membatalkan satu perubahan utuh,
   termasuk fitur yang ikut dihitung ulang.
4. **Simpan** — tekan `Ctrl/Cmd+S` (mis. `bracket.ducad`). Parameter dan riwayat fitur ikut
   tersimpan.
5. **Ekspor STEP** — menu utama (ikon tiga garis) → **Ekspor** → **STEP**. Dialog simpan
   menyebut satuannya (mm); baris status menyebut body yang ikut.

Dokumen yang sedang terbuka disisihkan selama tutorial dan dibuka kembali saat tutorial ditutup.

**Saran tindakan berikutnya**: satu baris petunjuk (ikon bohlam) muncul di bawah pil status di
kiri atas kanvas, mis. "Gambar profil tertutup di bidang ini" atau "Model ini belum pernah
disimpan". Tombol silang di ujungnya (*Sembunyikan saran untuk sesi ini*) menyembunyikannya
sampai aplikasi dibuka lagi.

---

## 🎓 Tutorial Selamat Datang

Selain bab proyek contoh di atas, tutorial interaktif mengerjakan satu part dari awal sampai
siap produksi: **rem cakram Ø240 mm**. Setiap langkah menambah sesuatu pada cakram dan hanya
terbuka setelah geometrinya benar-benar berubah. Tiga bab yang saling menyambung:

1. **Pemula** — langkah 0 "Kenali DUCAD" (tur layar: bilah atas, bilah kiri, mode, ViewCube,
   bilah aksi, palet, bantuan), lalu lingkaran Ø240, extrude 6 mm, navigasi, sketsa lubang poros
   di sisi atas, potong tembus, chamfer tepi, simpan.
2. **Menengah** — lingkaran baut di sisi atas, pattern sirkular 5×, potong, slot ventilasi,
   pattern 12×, potong, fillet tepi lubang poros, ukur jarak, ganti nama body.
3. **Mahir** — hub Ø120 di atas cakram, shell 4 mm, Hole Wizard M8, tampilan irisan, material
   dan massa, simulasi statik, gambar kerja ke PDF, ekspor STEP, penyiapan agent Chat AI.

Kartu sambutan bisa melompat ke bab mana pun (termasuk **Mulai cepat: proyek contoh**). Jika kanvas kosong saat Bab 2 atau 3 dimulai,
geometri hasil bab sebelumnya dibuat otomatis lewat oplog engine.

* Setiap langkah memutar animasi contoh dan menyorot tombol yang dimaksud.
* Tombol **Lanjut** baru aktif setelah Anda benar-benar mencoba aksinya. **Lewati langkah ini** tersedia bila ingin melompat.
* Kartu tutorial bisa digeser bila menutupi area kerja.
* Buka lagi kapan saja lewat tombol bantuan (ikon tanda tanya) di kanan atas, atau lewat Command Palette (`Ctrl/Cmd+K`) → **Tutorial: mulai dari awal**.
* Status tersimpan di `~/.ducad/onboarding.json`. Variabel lingkungan `DUCAD_SKIP_ONBOARDING=1` mematikan tutorial (CI, demo).

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
| **Ellips (Ellipse)** | `E` | Klik titik pusat, klik untuk sumbu mayor dan minor *(terbatas: hanya sejajar sumbu; tidak ikut ekspor DXF)* |
| **Busur (Arc)** | `A` | Klik titik awal, klik titik akhir, lalu klik titik di busur untuk menentukan kelengkungan |
| **Poligon (Regular Polygon)** | `Y` | Tool segi-$N$ beraturan (Segi-3, 5, 6 heksagonal kepala baut, 8). Pilih mode *Inscribed* (dalam lingkaran) atau *Circumscribed* (luar lingkaran) |
| **Slot (Lubang Baut Lonjong)**| — | Buat slot lonjong otomatis dari 2 titik pusat (*Center-to-Center*) atau panjang total (*Overall Slot*) dan lebar diameter |
| **Teks 2D (Sketch Text)** | — | Masukkan teks string, pilih font (Sans, Serif, Mono), teks langsung dikonversi menjadi kurva vektor sketsa (masuk oplog sebagai kurva saat keluar dari mode sketsa) |
| **Garis Konstruksi** | `X` | Mengubah garis aktif/terpilih menjadi garis putus-putus oranye referensi (tidak dihitung sebagai profil ekstrusi) |
| **Extend (Perpanjang)** | `Shift + E` | Klik segmen garis untuk memperpanjang otomatis hingga menyentuh batas kurva terdekat |
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

1. **Akses**: Buka drawer **Datum** (palette "Drawer Datum", atau tombol bidang di toolbar). Drawer berisi
   tiga seksi — **Bidang**, **Sumbu**, **Titik** — masing-masing dengan tombol **+** untuk membuat datum baru.
2. **Metode Pembuatan Bidang Baru** (tool Datum Plane, jenis **Bidang**):
   - **Offset Plane**: Masukkan jarak offset $d$ mm dari face planar atau plane acuan.
   - **Angled Plane**: Putar sebesar sudut $\theta^\circ$ terhadap edge acuan.
   - **3-Point Plane**: Klik 3 titik vertex di ruang 3D untuk membentuk bidang datar.
   - Combo **Lainnya…**: **Bidang tengah** (dua face datar sejajar / bidang dari "+ Bidang"),
     **Tegak lurus edge** (klik edge di titik yang dilalui), **Singgung silinder** (face silinder + sudut),
     **Sepanjang lintasan** (edge + pecahan 0..1), **Melalui dua edge** (dua edge lurus sebidang).
3. **Sumbu datum** (jenis **Sumbu** di HUD, atau palette "Sumbu Datum"): Dua titik, Sepanjang edge, Normal face,
   Sumbu silinder, Dua bidang. **Titik datum** (jenis **Titik**, palette "Titik Datum"): Vertex, Pusat
   (lingkaran/bola), Edge x bidang, Dua edge, Tiga bidang, Sepanjang edge. Klik geometri di viewport (pilihan
   disorot hijau, pratinjau biru), lalu **Buat** atau Enter. Masukan degenerat (bidang sejajar, titik berimpit)
   menghasilkan pesan, tidak membuat datum.
4. **Mengelola datum**: klik nama bidang untuk mengaktifkannya sebagai bidang sketsa; ikon mata = tampil/sembunyi;
   klik ganda nama atau ikon pensil = ganti nama; ikon tong sampah = hapus. Datum (termasuk sketsa di bidang datum)
   ikut tersimpan di berkas `.ducad` (field `datums`; berkas lama tetap terbuka).
5. **Memakai sumbu datum**: saat ada sumbu datum, panel "Sumbu: datum" muncul untuk **Revolve** (sumbu harus
   terletak di bidang XY tempat revolve GUI dibangun), **Pattern** 3D melingkar, dan **Mate Concentric**
   (setelah memilih satu face silinder; part ground lain menjadi jangkar sumbu).
6. **Lewat MCP/CLI (oplog)** — geometri konstruksi tersedia sebagai op (`get_schema {"op":"construct_axis"}`,
   contoh lengkap `{"example":"datum_axis"}`):
   - `construct_axis` (`by`: `two_points`, `edge`, `normal_to_face`, `through_cylinder`, `two_planes`) →
     dirujuk sebagai `"axis:<id>"` di `revolve.axis` (sumbu harus terletak di bidang sketch) dan
     `pattern` melingkar (`"axis":"axis:<id>"` tanpa `pivot`).
   - `construct_point` (`by`: `vertex`, `center`, `edge_plane`, `two_edges`, `three_planes`, `along_path`) →
     dirujuk sebagai `"point:<id>"`.
   - Bidang sketch konstruksi (`plane`): `{"midplane":[A,B]}`, `{"perpendicular_to":L,"through":P}`,
     `{"tangent_to":{"body":B,"face":F},"angle_deg":a}`, `{"along":{"body":B,"edge":E},"fraction":f}`,
     `{"through_edges":[L1,L2]}` / `{"through_edges":[L],"through":P}`.

---

## 🧊 Pemodelan Solid 3D (B-Rep)

### 1. Extrude (Ekstrusi)
* Pilih profil tertutup di sketsa $\rightarrow$ Klik **Extrude** di panel atau Bottom Context Bar $\rightarrow$ Tentukan jarak (mm). Bila sapuan masuk ke body yang ada, hasilnya memotong body itu.
* *(terbatas: GUI selalu satu arah (Blind). Simetris/dua sisi hanya lewat oplog (CLI/MCP/agent). "Up to face" belum tersedia.)*

### 2. Revolve (Putar Profil)
* Pilih profil tertutup $\rightarrow$ Tekan `V` atau klik **Revolve** $\rightarrow$ Pilih sumbu putar (Sumbu X, Sumbu Y, garis tepi profil, atau 2 titik manual) $\rightarrow$ Tentukan sudut putar (90°, 180°, 360° penuh) $\rightarrow$ **Terapkan** (Enter).
* *(terbatas: di GUI sumbu putar harus terletak di bidang sketsa; sumbu datum 3D bebas hanya lewat oplog.)*

### 3. Loft (Penyambung Antar Profil)
* Pilih profil bawah $\rightarrow$ Klik **Set Profil Bawah dari Seleksi** $\rightarrow$ Pilih profil atas dan atur tinggi $\rightarrow$ Klik **Eksekusi Loft**.
* *(terbatas: mengedit sketsa sumber setelahnya tidak memperbarui loft; tanpa tes GUI otomatis.)*

### 4. Helix / Coil / Spring (Geometri Spiral 3D)
* Buka **Helix Tool** $\rightarrow$ Masukkan Radius $R$, Pitch (jarak antar ulir), Ketinggian/Jumlah Putaran (*Turns*), dan Radius Kawat Profil $\rightarrow$ Hasilkan solid pegas kawat spiral, sudu auger, atau ulir botol secara otomatis.
* *(terbatas: helix dari GUI belum tercatat di Riwayat Parametrik, jadi tidak dihitung ulang saat parameter berubah; helix lewat oplog (op `helix`) bisa.)*

### 5. Fastener Hole Wizard (Standar ISO Baut)
* Klik face planar $\rightarrow$ Buka **Hole Wizard Dialog**:
  * **Simple Hole**: Lubang bor silindris standar (Tembus / *Through All* atau Kedalaman *Blind*).
  * **Counterbore**: Lubang bertingkat untuk kepala baut L (*Socket Head Cap Screw*).
  * **Countersink**: Lubang tirus 90° untuk baut kepala rata (*Flat Head*).
  * **Tapped**: Lubang bor tap metrik (pilihan cepat M2, M2.5, M3, M4, M5, M6, M8, M10, M12). *(terbatas: hanya diameter bor tap; ulir tidak dimodelkan kecuali lewat op `thread` di oplog.)*
* Klik atau drag titik target di viewport untuk menggeser posisi lubang, lalu **Terapkan Lubang**. Lubang tercatat di Riwayat Parametrik dan ikut dihitung ulang saat dimensi berubah (counterbore ukuran kustom tetap jalur lama).

### 6. Operasi Boolean Solid
* Pilih 2 solid di daftar/viewport:
  * **Union**: Menggabungkan dua bodi menjadi satu kesatuan.
  * **Subtract (Cut)**: Memotong bodi utama dengan bodi pemotong.
  * **Intersect**: Menyisakan hanya bagian volume yang saling tumpang tindih. *(terbatas: tanpa tes tingkat op.)*
* Urutan pilihan berarti: Subtract = body pertama − body kedua.

### 7. Fillet, Variable Radius Fillet, & Chamfer
* **Fillet Konstan**: Membulatkan seluruh tepi atau tepi terpilih dengan radius $R$.
* **Variable Radius Fillet**: Membulatkan tepi dengan radius berbeda di awal dan akhir ($R_{\text{start}} \ne R_{\text{end}}$). *(terbatas: hanya tepi lurus.)*
* **Chamfer**: Membuat pingulan miring dengan jarak $d$ mm.

### 8. Shelling (Pengosongan Rongga) & Draft Angle
* **Shell**: Pilih bodi, tentukan ketebalan dinding, dan pilih face yang ingin dibuang untuk membuat casing berongga.
* **Draft Angle**: Menambahkan sudut kemiringan cetakan (*injection molding draft*) pada dinding bodi solid. Bidang netral = face terbawah searah tarik. *(terbatas: hanya arah tarik sejajar sumbu yang tercatat di Riwayat Parametrik.)*

### 9. 3D Text Emboss & Deboss
* Buat teks sketsa pada permukaan face $\rightarrow$ Pilih opsi **Emboss** (teks timbul keluar) atau **Deboss / Engrave** (ukiran teks tenggelam ke dalam solid).

---

## 📑 Lembar Gambar Kerja 2D (Drawing Sheet & Blueprint)

Klik **Gambar Kerja 2D** di top bar untuk membuat cetak biru teknik. Subset yang dijamin tes otomatis (satu lembar A4/A3, tampak ortografis + HLR + isometri, dimensi otomatis, catatan lubang, title block, mm) dijelaskan di [`FORMAT_MATRIX.md`](FORMAT_MATRIX.md#drawing-pdf--dxf--svg); di luar itu berstatus terbatas.

1. **Tampak Proyeksi Otomatis**:
   - Menghasilkan tampak Atas (*Top*), Depan (*Front*), Samping (*Right*), dan Isometrik 3D lengkap dengan algoritma *Hidden Line Removal* (HLR).
2. **Section View A-A (Tampak Potongan)**:
   - Membuat irisan melintang pada bidang potong yang dilengkapi dengan pola arsir (*Hatch pattern*) garis miring 45° standar ISO/ANSI dan garis panah pemotong `A ─── A`.
3. **Detail View (Lingkaran Pembesar)**:
   - Menambahkan lingkaran viewport pembesar dengan skala khusus (2:1, 5:1, 10:1) untuk area detail mikro. *(terbatas: interaksi GUI belum diuji otomatis.)*
4. **Dimensi Otomatis & Dimensi Manual**:
   - Dimensi otomatis dibuat dari model dan ikut berubah bila model berubah.
   - Klik 2 titik pada kanvas untuk menambahkan dimensi linier kustom, dimensi diameter lingkaran, atau sudut derajat. *(terbatas: alat dimensi manual belum diuji ujung-ke-ujung.)*
5. **Tabel BOM (Bill of Materials) & Part Balloons**:
   - Menampilkan tabel otomatis nomor item, nama part, jumlah kuantitas, dan bahan material yang terhubung langsung ke balon penunjuk pada gambar isometrik.
6. **Kepala Gambar (ISO Title Block)**:
   - Bingkai standar teknik lengkap dengan judul proyek, nama perancang, tanggal, unit, dan nomor revisi.
7. **Ekspor**: **Ekspor PDF (Vektor)** atau **Ekspor DXF (2D CAD)** — satu lembar per ekspor, satuan mm, DXF skala 1:1 kertas.

*(terbatas: editor GUI hanya satu lembar; multi-sheet hanya lewat paket desain (MCP `build_package`). Anotasi GD&T yang ditempel manual di editor drawing belum ikut tersimpan ke `.ducad`.)*

---

## ⚙️ Perakitan (Assembly) & Uji Tabrakan (Clash Detection)

Buka **Pohon Perakitan & Hierarki** (tombol *Pohon Perakitan & Mate Constraints* di top bar):

1. **Pohon Perakitan (*Assembly Tree*)**: Mengelola hierarki part komponen dan instance mandiri.
2. **Mate Constraints 3D**:
   - **Concentric Mate**: Mengunci keselarasan sumbu silinder poros dengan lubang part pasangannya.
   - **Coincident Mate**: Menempelkan dua permukaan datar saling berimpit.
   - **Distance / Angle Mate**: Mengatur jarak offset atau sudut engsel mekanis. *(Angle Mate terbatas: tanpa tes khusus.)*
   - Klik **Terapkan Mate**, lalu **Selesaikan & Perbarui Perakitan**.
3. **Clash & Interference Detection**:
   - Di tab **Tabrakan**, klik **Periksa Tabrakan** untuk mendeteksi tabrakan fisik antar bodi solid di seluruh perakitan. Sistem akan menghitung volume interferensi dan menyorot bagian yang bertabrakan sebelum proses manufaktur.

Assembly tersimpan di `.ducad` sebagai lapisan terpisah dari riwayat part (ADR 0011, status diusulkan): tidak ada op assembly di oplog, jadi CLI/MCP/agent belum bisa membuat atau me-replay perakitan. Mengedit dimensi part tidak memutus instance perakitan (identitas body dipertahankan).

---

## 🕒 Parametric History Timeline (Feature Tree)

1. Klik tombol garis waktu di baris ikon kanan bawah untuk membuka **Pohon Fitur & Garis Waktu**.
2. Seksi **Riwayat Parametrik (oplog)** menampilkan satu baris per op: nama, jenis, ringkasan, dan dependensi (*Bergantung pada* / *Dipakai oleh*). Per op tersedia:
   - **Ubah nilai utama** (ikon pensil) → ketik nilai → **Terapkan**. Fitur sesudahnya dihitung ulang.
   - **Aktifkan / nonaktifkan op ini** (suppress).
   - **Hapus op ini** — ditolak dengan pesan bila op masih dipakai op lain.

   Setiap aksi adalah satu langkah undo (`Ctrl/Cmd+Z`), dan riwayat ini ikut tersimpan di `.ducad`.
3. **Parameter dokumen**: palet perintah → **Parameter**. Dimensi sketsa bisa diikat ke parameter dengan mengetik `$nama` atau ekspresi (mis. `$base + 5`) di popup dimensi; mengubah parameter lalu **Terapkan** memperbarui sketsa dan body sekaligus, sebagai satu langkah undo.
4. Edit sketsa sumber (keluar dari mode sketsa) juga memperbarui fitur hilir.

*(terbatas: fitur yang belum dialihkan ke oplog — mis. helix dari GUI dan beberapa kasus khusus loft/sweep/shell — masih tercatat di daftar fitur lama (**Terapkan & Regenerasi**), tidak ikut tersimpan ke `.ducad`, dan regenerasinya tidak menjamin fitur hilir. Penanda *roll-to* (kembali ke titik riwayat) untuk oplog belum ada.)*

---

## 🎨 Studio Lingkungan Pencahayaan & Material (CMF)

1. Klik **Studio Lighting** di top bar:
   - Pilih preset pencahayaan: *Clean Studio*, *Warm Showcase*, *Cool Tech*, atau *Dramatic Dark*.
   - Atur keseimbangan lampu 3-titik (Key, Fill, Rim), oklusi ambien "SSAO", dan bayangan kontak lantai. *(terbatas: efek berlabel SSAO bukan SSAO screen-space sungguhan.)*
2. Di panel properti body, seksi **CMF & Material Industri**: pilih preset material, warna, dan sifat PBR (*Metallic*, *Roughness*).

---

## 💾 Manajemen Berkas (File I/O)

Menu utama (ikon tiga garis, tooltip **Berkas**) di kiri atas — submenu **Impor** dan **Ekspor** — atau via Command Palette (`Ctrl/Cmd+K` / `Ctrl/Cmd+Shift+P`). Batas lengkap tiap format (dapat dibuka, dibuat, round-trip, satuan, data yang hilang) ada di [`FORMAT_MATRIX.md`](FORMAT_MATRIX.md).

### Berkas Asli DuCAD
* **Simpan (`Ctrl/Cmd+S`)** / **Buka (`Ctrl/Cmd+O`)**: Berkas `.ducad` menyimpan sketsa 2D, body 3D, parameter dan riwayat parametrik (oplog, termasuk op yang di-suppress), datum, material, lembar gambar, dan perakitan. *(terbatas: fitur di daftar fitur lama yang belum masuk oplog dan anotasi GD&T manual di editor drawing tidak ikut tersimpan.)*

### Ringkasan ekspor & satuan
* Dialog simpan menyebut format dan satuannya (mis. "STEP AP242 - satuan mm"; GLB "satuan meter, Y ke atas").
* Setelah ekspor, baris status menyebut jumlah body dan satuan. **Body tersembunyi tidak ikut** ekspor STEP/STL/OBJ/GLB, dan jumlahnya disebut. Ekspor DXF sketsa menyebut elips/spline/path yang dilewati (tidak didukung DXF R12).
* Impor DXF membaca satuan berkas (`$INSUNITS`) dan **mengonversinya ke mm**; satuan asal disebut di baris status. Berkas tanpa `$INSUNITS` dianggap mm.

### Format Impor & Ekspor

| Format | Mode | Keterangan |
|---|---|---|
| **STEP (`.step`, `.stp`)** | Import & Export | Solid B-Rep AP242, satuan mm, multi-body untuk CAM/CNC. Hanya geometri (tanpa nama, material, riwayat). *(Impor terbatas: belum diuji dengan berkas dari CAD lain.)* |
| **GLTF / GLB (`.glb`)** | Import & Export | Mesh dengan PBR untuk Web & AR, satuan meter, sumbu Y ke atas. *(AR Quick Look belum diverifikasi.)* |
| **SVG (`.svg`)** | Import & Export | Vektor 2D (sketsa/gambar kerja) untuk Laser Cutting dan CNC Router; impor path → sketsa. *(terbatas)* |
| **PDF (`.pdf`)** | Export (+ impor garis referensi) | Gambar teknik vektor satu lembar A4/A3 dengan title block, arsir potongan, dan tabel BOM. Impor hanya garis vektor sebagai sketsa referensi |
| **DXF (`.dxf`)** | Import & Export | Sketsa 2D dan gambar kerja, R12, `$INSUNITS` mm. *(terbatas: tanpa SPLINE/TEXT/INSERT/HATCH/DIMENSION saat impor; layer tidak diimpor.)* |
| **STL (`.stl`)** | Import & Export | Mesh biner untuk 3D Printing / Slicer; semua body terlihat digabung satu mesh |
| **OBJ** | Import & Export | Mesh, satu objek per body, tanpa material |
| **3MF** | Import & Export | Mesh untuk slicer (palet: **Ekspor 3MF…** / **Impor Mesh (3MF / OBJ / glTF)…**). *(terbatas)* |
| **IGES (`.igs`, `.iges`)** | Import & Export | Palet: **Impor IGES…** / **Ekspor IGES…**. *(eksperimental: belum ada fixture dari CAD lain.)* |

---

## 💡 Alur Kerja Ergonomis Standar DuCAD

DuCAD menerapkan hierarki interaksi konsisten untuk kecepatan kerja maksimal:
1. **Sidebar Kiri**: Menu untuk membuat objek baru yang belum ada (Sketsa 2D, Bodi 3D, Assembly, Datum Plane).
2. **Bottom Context Bar**: Menu interaktif yang muncul otomatis di bagian bawah saat sebuah objek, face, atau edge dipilih dengan tool *Select*.
3. **Canvas HUD (Header Atas)**: Kotak masukan angka/parameter ringkas saat menggambar.
4. **Pop-up Window (Kanan Bawah)**: Kotak dialog konfigurasi mendalam untuk fitur kompleks (Hole Wizard, Helix, Draft, 3D Text).
5. **Command Palette (`Ctrl/Cmd+K`)**: Pencarian instan seluruh perintah aplikasi dengan keyboard.
