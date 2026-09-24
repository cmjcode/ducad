# ADR 0004 — Model Vektor dalam ducad-sketch

**Status**: Diterima · **Tanggal**: 2026-09-24 · **Cakupan**: M0 (M0.1–M0.8)

Dokumen ini mencatat keputusan arsitektur penambahan model grafis vektor 2D
ke dalam DUCAD untuk mendukung sketsa multi-alat (CAD, ilustrasi vektor, dan
persiapan ekstrusi 3D), beserta alasan dan alternatif yang ditolak.

---

## 1. Vektor memperluas `ducad-sketch`, bukan membuat crate `ducad-vector` terpisah

**Masalah.** DUCAD sebelumnya hanya mendukung entitas geometris analitis
dasar (`Line`, `Circle`, `Arc`, `Ellipse`, dan `Spline` titik kontrol).
Untuk kebutuhan desain produk modern, logo, ikon, serta profil ekstrusi bebas,
dibutuhkan representasi kurva komposit Bézier arbitrary (subpath terbuka/tertutup,
garis lurus, kurva kuadratik, dan kubik) lengkap dengan atribut visual
(fill, stroke, stroke width, opacity, fill rule, layer, dan grup).

Ada godaan untuk membuat crate baru bernama `ducad-vector` yang menangani semua
hal berbau ilustrasi grafis secara terisolasi.

**Keputusan.** Entitas vektor diintegrasikan langsung ke dalam `ducad-sketch`
melalui varian `Entity::Path` yang memuat daftar `Subpath` berbasis pustaka
`kurbo`, bersama sistem atribut visual `Style`, `Paint::Solid(Color)`, `Layer`,
`Group`, dan pelacakan `rev` (revisi entitas) + indeks spasial `rstar`.

**Alasan.**
1. **Satu kanvas, satu sistem koordinat**: Kanvas 2D DUCAD bekerja dalam
   satuan milimeter riil (`f64`). Memisahkan vektor ke crate lain akan
   menciptakan duplikasi struktur dokumen, kanvas ganda, dan friksi koordinat.
2. **Koleksi entitas terpadu**: `SlotMap<EntityId, Entity>` tunggal memudahkan
   undo/redo, seleksi multi-elemen, penamaan entitas, dan pengelompokan.
3. **Persiapan ekstrusi 3D (M3)**: Profil 3D nantinya dapat dibangun secara
   mulus dari kombinasi garis analitis CAD maupun kurva vektor tanpa perlu
   konversi bolak-balik antar-crate yang rawan kehilangan presisi numerik.

**Alternatif yang ditolak.**
- *Membuat crate `ducad-vector` mandiri.* Memerlukan jembatan sinkronisasi ID
  antar dua crate berbeda, mempersulit sistem undo/redo hierarkis, dan memicu
  konversi tipe yang berulang-ulang setiap kali user beralih antara alat CAD
  dan alat vektor.

---

## 2. Tinta bebas (`ducad-ink`) dipisahkan menjadi crate mandiri

**Masalah.** Input coretan tangan bebas (Apple Pencil, stylus sentuh, mouse)
menghasilkan ribuan titik berdensitas tinggi (laju sampling 120–240 Hz) dengan
informasi tekanan (`force`), sudut kemiringan, dan stempel waktu. Jika titik
mentah ini dimasukkan langsung ke dalam `ducad-sketch`, struktur data CAD
akan membengkak dan memperlambat evaluasi geometris.

**Keputusan.** `ducad-ink` dibangun sebagai crate terpisah yang menangani
titik coretan stylus, kompresi, penyederhanaan kurva, dan smoothing. Hanya
setelah coretan tangan disetujui atau dikonversi (melalui pengenal gestur
atau fit Bézier), kurva hasil konversi dimasukkan ke dalam `ducad-sketch`
sebagai `Entity::Path` dengan penanda `Provenance::Freehand`.

**Alasan.**
- Coretan tinta mentah tidak memerlukan solver geometric constraint dan
  tidak memerlukan representasi `f64` presisi ganda (cukup `f32`).
- Menjaga `ducad-sketch` tetap ringkas, deterministik, dan berfokus pada
  geometri parametrik.
- Mengisolasi dependensi dan alur data throughput tinggi dari kernel CAD.

**Alternatif yang ditolak.**
- *Menyimpan `InkStroke` langsung sebagai varian `Entity` di `ducad-sketch`.*
  Membengkakkan slotmap entitas, memperlambat spatial index AABB, dan mencemari
  file `.ducad` dengan jutaan koordinat mentah sementara yang belum difit.

---

## 3. Constraint solver hanya mengikat node utama path (node-only constraint)

**Masalah.** Sebuah `Subpath` Bézier kubik terdiri dari titik-titik simpul
utama (*anchor nodes*) dan titik kendali tangen (*Bézier control handles* `c1`
dan `c2`). Jika setiap titik kendali dimasukkan sebagai derajat kebebasan
(*degree of freedom* / DoF) ke dalam constraint solver numerik, jumlah variabel
akan melonjak drastis (6 variabel per segmen kubik). Hal ini dapat menyebabkan
singularitas matriks Jacobi ketika handle bernilai nol atau kolinear, serta
memperlambat konvergensi solver secara signifikan.

**Keputusan.** Solver hanya mengikat node simpul kurva melalui
`PointRef::PathNode { id, sub, node }`. Titik kendali Bézier tidak dijadikan
DoF solver independen.

**Alasan.**
1. **Stabilitas numerik solver**: Solver non-linear Levenberg-Marquardt / BFGS
   tetap cepat, stabil, dan terhindar dari singularitas geometri handle.
2. **Semantik relasi yang jelas**: Relasi geometris (mis. Coincident,
   Horizontal, Vertical, Distance, Fixed) secara intuitif menghubungkan
   ujung-ujung kurva atau persimpangan profil, bukan orientasi gagang tangen.
3. **Pengendalian bentuk kurva**: Kontrol kehalusan kurva (smooth, symmetric,
   corner) ditangani pada level editor kurva vektor melalui vektor diferensial
   relatif terhadap node, bukan melalui persamaan constraint numerik solver.

**Alternatif yang ditolak.**
- *Mendaftarkan semua control handle sebagai `PointRef::PathControl1` dan
  `PointRef::PathControl2` di solver.* Menimbulkan ledakan dimensi DoF,
  memperlambat perhitungan hingga ratusan milidetik, dan sering gagal
  konvergen saat dua pegangan saling bertindihan.

---

## 4. Konsekuensi

1. **Format berkas native v3**: `DuCADFile` menambahkan dukungan untuk
   layer, grup, style, dan entitas path, dengan mekanisme `needs_v3` sehingga
   berkas sederhana tetap tersimpan dalam format v1/v2 yang kompatibel.
2. **Performa per-frame O(1)**: Setiap entitas memiliki `rev: u64` dan
   metode `touch(id)` yang menginvalidasi cache spasial `rstar`, memastikan
   viewport tidak melakukan komputasi ulang saat tidak ada perubahan data.
3. **Oplog Engine & CLI**: `EntitySpec::Path` memungkinkan scripting deklaratif
   dan automasi headless melalui `ducad-cli run` dan `ducad-cli inspect`.
