# Panduan Pemakaian CADRAW

Panduan ini menjelaskan CARA PAKAI semua yang sudah dibangun sampai Fase 8
(viewport 3D, sketching 2D, constraint solver, modeling 3D — termasuk
Revolve/Loft/Intersect/picking edge-wajah Fase 8, UX shell — command
palette/radial menu/tema, dan file I/O — buka/simpan/import/export). Untuk
status proyek/arsitektur/riwayat pengembangan, lihat `docs/PLAN.md` —
dokumen ini murni tentang mengoperasikan aplikasinya.

## Menjalankan

```bash
cargo run -p cadraw-app
```

Build pertama kali akan mengompilasi OCCT dari source (~8 menit, sekali
saja — di-cache di `target/` setelahnya). Kalau build gagal di langkah
CMake, cek `.cargo/config.toml` di root workspace sudah ada (berisi
`CMAKE_POLICY_VERSION_MINIMUM = "3.5"`, dibutuhkan di mesin dengan CMake
≥ 4.0 — sudah ditangani otomatis kalau file itu ada, tidak perlu diketik
manual).

## Navigasi kamera

Berlaku di tool apa pun (kecuali orbit klik-kiri, khusus tool Pilih):

| Aksi | Kontrol |
|---|---|
| Orbit | Drag klik-tengah, atau drag klik-kiri **saat tool Pilih aktif** |
| Pan | Shift+drag, atau drag klik-kanan |
| Zoom | Scroll, atau pinch trackpad/touch |
| Orbit + zoom dua jari | Trackpad/iPad dua jari (jalan di tool apa pun, gaya Shapr3D: satu jari menggambar, dua jari mengarahkan kamera) |

## Sketching 2D

Semua sketch digambar di bidang XY (Z=0). Toolbar di atas + shortcut
keyboard:

| Tool | Shortcut | Cara pakai |
|---|---|---|
| Pilih | — | Klik entitas untuk pilih, Shift+klik untuk multi-pilih, Delete/Backspace hapus seleksi |
| Garis | `L` | Klik titik awal, klik titik akhir (atau ketik panjang + Enter) |
| Persegi | `R` | Klik sudut pertama, klik sudut berlawanan (atau ketik sisi + Enter) |
| Lingkaran | `C` | Klik pusat, klik untuk radius (atau ketik radius + Enter) |
| Ellips | `E` | Klik pusat, klik sudut kotak pembatas |
| Arc | `A` | Klik titik awal, klik titik akhir, klik titik di busur (menentukan sisi/arah) |
| Offset | `O` | Klik entitas sumber, lalu klik untuk sisi + jarak hasil offset |
| Mirror | `M` | **Pilih dulu entitas via tool Pilih**, tekan M, klik 2 titik sumbu cermin |
| Trim | `T` | Hover menyorot merah sub-segmen yang akan hilang, klik untuk memotong |

Catatan:

- **Snap otomatis** aktif di semua tool titik: endpoint > midpoint > center
  > intersection > grid, ditandai glyph beda bentuk per jenis (lihat
  indikator "snap: ..." di status bar bawah).
- **Dynamic input** (ketik angka lalu Enter, gaya AutoCAD) baru tersedia
  untuk Garis/Persegi/Lingkaran — belum ada untuk Ellips/Arc/Offset/Mirror.
- **Esc** membatalkan titik yang sedang diklik (atau kembali ke tool Pilih
  kalau tidak ada titik pending).
- **Undo/Redo sketch**: `Ctrl/Cmd+Z` undo, `Ctrl/Cmd+Shift+Z` atau
  `Ctrl+Y` redo — juga ada tombol ↶/↷ di toolbar atas.
- Offset untuk Ellips, Trim vs Circle/Arc, dan spline belum didukung.

## Constraint (panel kanan "Constraint")

Muncul otomatis saat **tool Pilih aktif** dan ada 1-2 entitas terpilih.
Tombol yang tersedia bergantung kombinasi entitas:

| Seleksi | Constraint tersedia |
|---|---|
| 1 Line | Horizontal, Vertikal, Panjang (ketik nilai mm) |
| 1 Circle/Arc | Radius (ketik nilai mm) |
| 2 Line | Sejajar, Tegak Lurus, Sama Panjang, Sudut (ketik derajat) |
| 2 Circle/Arc | Sama Radius, Tangent |
| 1 Line + 1 Circle/Arc | Tangent |

Constraint dicoba dulu (dry-run) — kalau gagal konvergen, sketch TIDAK
berubah, cuma muncul pesan error dengan sisa residual di bawah panel.

Tiga tool titik tambahan ada di menu **"Titik ▾"** di toolbar utama (klik
untuk buka, label menu berubah menampilkan tool titik yang sedang aktif):

| Tool | Cara pakai |
|---|---|
| Coincident (titik) | Klik 2 titik (endpoint/center via snap) → dibuat berimpit |
| Fixed (titik) | Klik 1 titik → ditahan persis di posisi sekarang (tak perlu ketik target) |
| Symmetric (titik) | **Pilih 1 Line dulu via tool Pilih** (jadi sumbu), lalu klik 2 titik yang dibuat saling cermin |

Belum didukung: constraint pada titik ujung Arc, point-on-entity,
tangensial internal, Tangent Line-Line (memang tak masuk akal secara
geometris), browser/manajer constraint (lihat/hapus selain lewat Undo).

## Model 3D (panel kiri "Model 3D") — Fase 3

Ini yang mengubah sketch 2D jadi solid 3D nyata (lewat kernel OCCT).
Panel ini SELALU tampil (tidak bergantung tool sketch aktif), berdampingan
dengan panel Constraint di kanan.

### Extrude — bikin body pertama

1. Di tool **Pilih**, pilih entitas sketch yang membentuk profil tertutup:
   - **1 Lingkaran** sendirian → jadi silinder, ATAU
   - **≥3 Line/Arc** yang ujung-ujungnya nyambung membentuk satu loop
     tertutup (urutan klik/pilih bebas — CADRAW merangkainya sendiri).
     Contoh paling gampang: 4 garis dari tool Persegi.
2. Di panel Model 3D, isi **Jarak (mm)**, klik **Extrude**.
3. Kalau gagal (profil tidak tertutup, atau tercampur dengan Ellips/
   Lingkaran lain), pesan error muncul di bawah panel — sketch dan model
   tidak berubah, coba lagi setelah perbaiki seleksi.

### Daftar body

Body yang sudah ada muncul di daftar tengah panel:
- **Checkbox** kiri = tampil/sembunyikan di viewport (tidak menghapus).
- **Klik nama** = pilih body itu (ganti seleksi).
- **Ctrl/Cmd+klik atau Shift+klik** = tambah/kurangi dari seleksi
  (multi-pilih, dibutuhkan Union/Subtract).

### Revolve — putar profil 360° mengelilingi sumbu (Fase 8)

1. Di tool **Pilih**, pilih entitas sketch yang membentuk profil (sama
   aturan dengan Extrude — 1 Lingkaran, atau ≥3 Line/Arc loop tertutup).
2. Tekan **V** (atau pilih "Revolve" di toolbar/command palette).
3. Klik **2 titik** (snap aktif) menentukan garis sumbu — profil diputar
   360° penuh mengelilingi garis itu. Hasil langsung muncul sebagai body
   baru.
4. Kalau gagal (profil tidak valid, atau 2 titik sumbu sama/terlalu
   dekat), pesan error muncul di panel Model 3D.

⚠️ Baru revolve **360° penuh** — sudut parsial (mis. cuma 180°) belum ada
UI-nya di putaran pertama ini.

### Loft — solid antara 2 profil beda bentuk (Fase 8)

Bukan loft lintas-bidang sungguhan (sketch CADRAW masih satu bidang XY) —
profil ATAS diangkat lewat translasi Z murni, bukan digambar di bidang
lain.

1. Di tool **Pilih**, pilih profil BAWAH (sama aturan Extrude), klik **Set
   Profil Bawah dari Seleksi** di panel Model 3D — tersimpan sementara
   ("✓ ter-set").
2. Ganti seleksi ke profil ATAS (boleh bentuk beda, mis. persegi ke
   lingkaran), isi **Tinggi (mm)**, klik **Loft**. Profil bawah tetap di
   Z=0, profil atas ditempatkan di Z=tinggi, solid disambung di antaranya.

### Union / Subtract / Intersect — gabung/potong/irisan 2 body

Pilih **persis 2 body** di daftar, lalu klik **Union** (gabung jadi satu),
**Subtract (A-B)** (potong), atau **Intersect** (Fase 8 — cuma sisakan
volume yang tumpang-tindih; kalau 2 body tidak bersinggungan sama sekali,
gagal dengan pesan error). Hasilnya 1 body baru; 2 body asal lenyap (bisa
di-undo). ⚠️ Urutan A/B untuk Subtract belum bisa dipilih manual di
putaran pertama ini — kalau hasilnya kebalik, Undo lalu coba pilih body
satu-satu ulang (urutan seleksi internal kadang berubah).

### Fillet / Chamfer — semua tepi, ATAU tepi tertentu (Fase 8)

Pilih **persis 1 body**, isi Radius (Fillet) atau Jarak (Chamfer), klik
tombolnya — berlaku ke **semua tepi** body sekaligus (perilaku lama, tetap
jalan apa adanya).

Untuk memilih tepi TERTENTU saja (Fase 8): klik **☐ Pilih Tepi Manual**
(butuh persis 1 body terpilih dulu) — tombol berubah jadi **■ Pilih Tepi
Manual (aktif)**, lalu klik langsung tepi-tepi yang mau kena di viewport
3D (ter-highlight garis oranye, hitungan "N tepi dipilih" bertambah tiap
klik). Klik lagi tombol untuk keluar mode, atau langsung klik **Fillet**/
**Chamfer** — akan berlaku HANYA ke tepi yang dipilih. **Reset Pilihan**
mengosongkan seleksi tanpa keluar mode. ⚠️ Belum bisa klik ulang tepi yang
sama untuk membatalkannya (cuma bisa reset semua).

### Shell / Hollow — kosongkan jadi cangkang, 1 wajah ATAU beberapa (Fase 8)

Pilih **persis 1 body**, pilih arah di dropdown (mis. `PosZ` = buang face
paling atas), isi Tebal (mm), klik **Shell** — perilaku lama, buang 1 face
terjauh ke arah itu.

Untuk membuang BEBERAPA wajah sekaligus (mis. tabung terbuka 2 sisi):
klik **☐ Pilih Wajah Manual**, klik wajah-wajah yang mau dibuang di
viewport 3D (hitungan "N wajah dipilih" bertambah — ⚠️ belum ada
highlight visual 3D untuk wajah, baru angka di panel), lalu klik **Shell**
— akan membuang HANYA wajah yang dipilih, mengabaikan dropdown arah.

### Hapus & Undo/Redo Model

- **Hapus Body Terpilih**: hapus semua body yang sedang terpilih.
- **↶ Undo Model / ↷ Redo Model**: SENDIRI, terpisah dari undo sketch
  (`Ctrl+Z` di keyboard cuma memengaruhi sketch, bukan model — pakai
  tombol di panel untuk undo/redo operasi 3D).

### Belum didukung di Model 3D

Sweep sepanjang jalur (gap upstream binding OCCT, bukan CADRAW), Revolve
sudut parsial (baru 360°), loft lintas-bidang sungguhan & sketch-on-face
(sketch selalu di bidang XY), klik langsung di viewport 3D untuk GANTI
seleksi BODY (pakai daftar di panel — picking viewport baru untuk
edge/face pada body yang sudah terpilih), toggle-off klik ulang tepi/wajah
terpilih, highlight 3D wajah terpilih.

## UX shell — Fase 4

### Command palette

`Ctrl/Cmd+K` membuka kotak pencarian aksi mengambang di tengah atas layar
(atau lewat menu **"⚙ Pengaturan"** di toolbar → **"⌘K Buka Command
Palette"**). Ketik untuk menyaring (cocok substring, tak peduli besar/
kecil huruf), panah atas/bawah pindah sorotan, **Enter** atau klik untuk
eksekusi, **Esc** untuk tutup. Aksi yang tersedia: ganti tool apa pun
(termasuk tool titik), Undo/Redo sketch, Undo/Redo Model, Ganti Tema, dan
Hapus Seleksi (muncul cuma kalau ada entitas terpilih). Berguna untuk aksi
yang jarang dipakai atau kalau lupa shortcut hurufnya.

### Radial menu (khusus tool Pilih, cocok untuk sentuh/iPad)

Tekan-tahan (jangan digerakkan) di viewport selama ±0.4 detik saat tool
**Pilih** aktif → roda pilihan tool muncul persis di bawah titik tekan.
Sambil tetap menekan, geser jari/mouse ke salah satu slice (Garis,
Persegi, Lingkaran, Ellips, Arc, Offset, Mirror, Trim) lalu lepas untuk
pindah ke tool itu. Lepas di lingkaran kosong di tengah (atau tekan Esc)
untuk batal tanpa ganti tool. Ini jalur ganti-tool utama di layar sentuh
(tidak perlu menjangkau toolbar di tepi atas) — di mouse/trackpad, toolbar
dan shortcut huruf (L/R/C/E/A/O/M/T) tetap cara tercepat.

### Menu "⚙ Pengaturan" (ujung kanan toolbar)

Semua hal yang jarang disentuh lebih dari sekali per sesi dikumpulkan di
sini, bukan jadi tombol lepas di toolbar utama:

| Isi menu | Kegunaan |
|---|---|
| Tombol tema (mis. "☀ Terang" / "🌙 Gelap") | Label menampilkan tema TUJUAN — klik untuk pindah ke tema itu. Bisa juga lewat command palette ("Ganti Tema"). Default: gelap. |
| "⌘K Buka Command Palette" | Sama seperti menekan `Ctrl/Cmd+K` langsung. |
| "Pintasan Keyboard" (bisa dibuka/tutup) | Daftar referensi semua shortcut huruf tunggal & kombinasi Ctrl/Cmd — bukan pengaturan yang bisa diubah, cuma bantuan kalau lupa. |

### Target sentuh

Semua tombol/checkbox/combo box di seluruh aplikasi (toolbar, panel
Constraint, panel Model 3D, command palette) punya tinggi minimum 44pt
mengikuti rekomendasi Apple HIG untuk target sentuh — sengaja dibuat
lantai global, bukan disetel manual per tombol.

## File I/O — Fase 5

Menu **"📄 File"** di ujung kiri toolbar (sebelah nama "CADRAW"). Semua
aksi juga ada di command palette (`Ctrl/Cmd+K`, ketik nama aksinya).

### Dokumen native `.cadraw`

| Aksi | Shortcut | Perilaku |
|---|---|---|
| Baru | — | Kosongkan sketch+model+kedua undo stack. Kamera & tema TIDAK ikut direset. |
| Buka… | `Ctrl/Cmd+O` | Dialog pilih file `.cadraw`, mengganti SELURUH dokumen (undo stack ikut direset — undo lintas-dokumen tidak masuk akal). |
| Simpan | `Ctrl/Cmd+S` | Tulis ke file terakhir dibuka/disimpan; kalau dokumen belum pernah punya file (baru), jatuh ke Simpan Sebagai. |
| Simpan Sebagai… | `Ctrl/Cmd+Shift+S` | SELALU tampilkan dialog, walau dokumen sudah punya file aktif. |

File `.cadraw` adalah JSON manusiawi-dibaca (bisa dibuka teks editor untuk
diperiksa) — sketch (entitas+constraint) DAN semua body 3D (geometri
B-rep lengkap, bukan cuma mesh) tersimpan utuh, termasuk body yang lagi
disembunyikan (checkbox visible di panel Model 3D).

### Import

| Sumber | Hasil |
|---|---|
| STEP (`.step`/`.stp`) | 1 body baru (undo-able) — kalau file berisi beberapa solid, semuanya masuk sebagai SATU body gabungan (belum bisa dipisah otomatis). |
| DXF (`.dxf`, subset R12) | Entitas Line/Circle/Arc ditambahkan ke sketch aktif (undo-able, satu langkah). Jenis lain (TEXT/SPLINE/dst) dilewati — pesan status melaporkan berapa yang dilewati. |

### Export

| Format | Cakupan | Catatan |
|---|---|---|
| STEP | SEMUA body (arsip dokumen penuh) | >1 body digabung jadi satu file, masing-masing tetap solid terpisah (bukan di-union). |
| STL (biner) | Body **visible** saja | Digabung jadi satu mesh — mewakili hasil cetak/tampilan fisik, sama seperti yang tampak di viewport. |
| OBJ | Body **visible** saja | Satu blok objek per body (tetap terpisah di tool lain seperti Blender). |
| DXF | Entitas sketch Line/Circle/Arc | Ellips DILEWATI (DXF R12 tidak punya entitas ELLIPSE) — jumlah yang dilewati dilaporkan di status bar. |

Pesan hasil tiap aksi (sukses maupun gagal) muncul sebentar di status bar
bawah, di sebelah hint tool aktif.

### Belum didukung di File I/O

Import STL/OBJ (sudah berupa segitiga, tidak ada jalan balik ke B-rep),
Ellipse/spline/polyline di DXF, memisahkan file STEP multi-solid jadi
body terpisah saat import, autosave, daftar file terakhir dibuka,
indikator "belum disimpan" di title bar, drag-and-drop file ke jendela.

## Contoh alur kerja singkat: kotak dengan tepi membulat

1. Tool **Persegi** (`R`) → klik 2 sudut untuk bikin 4 garis persegi.
2. Tool **Pilih** → klik satu sisi, lalu Shift+klik tiga sisi lainnya
   (belum ada drag-select/marquee — pilih satu-satu).
3. Panel **Model 3D** → isi Jarak (mis. `20`) → **Extrude**. Body pertama
   muncul di daftar & di viewport.
4. Klik nama body itu di daftar (pilih), isi Radius Fillet (mis. `2`) →
   **Fillet**. Tepi body membulat.
5. Kalau salah langkah, klik **↶ Undo Model**.

## Status implementasi lengkap & keterbatasan

Ringkasan di atas cukup untuk pemakaian sehari-hari. Untuk daftar lengkap
apa yang sudah/belum dikerjakan per fase, keputusan arsitektur, dan bug
yang pernah ditemukan+diperbaiki, lihat `docs/PLAN.md`.
