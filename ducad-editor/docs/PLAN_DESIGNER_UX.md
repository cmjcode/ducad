# Rencana Peningkatan DUCAD agar Dipercaya dan Disukai Designer

Dokumen ini menerjemahkan audit produk menjadi rencana kerja bertahap. Fokusnya
bukan menambah sebanyak mungkin fitur, melainkan membuat alur desain yang sudah
ada terasa dapat diandalkan, mudah dipahami, dan menghasilkan berkas yang bisa
dipakai di luar DUCAD.

**Status:** disetujui; Step 0 dikerjakan 2026-10-21 (hasil:
`docs/BASELINE_DESIGNER_UX.md`, `docs/STATUS_KEMAMPUAN.md`); sesi uji designer
dan tanggal berlaku masih terbuka  
**Cakupan:** fondasi dokumen dan pemodelan parametrik, alur sketch-ke-solid,
interaksi viewport, pemulihan error, pertukaran berkas, onboarding, dan validasi
produk.  
**Bukan tujuan rencana ini:** mengganti kernel geometri, mengejar paritas penuh
dengan semua CAD komersial, atau menambahkan fitur industri baru sebelum alur
utama teruji.

## 1. Tujuan

1. Designer dapat mengubah fitur awal tanpa kehilangan atau merusak fitur yang
   bergantung padanya.
2. Alur utama—sketsa, constraint, solid, revisi, simpan, dan ekspor—dapat
   diselesaikan tanpa bantuan atau tebakan tentang mode dan perintah aktif.
3. Kegagalan operasi menjelaskan sebab yang relevan dan menyediakan cara pulih
   tanpa kehilangan pekerjaan.
4. Dokumen native dan format pertukaran mempertahankan hasil yang dijanjikan
   setelah disimpan, dibuka kembali, atau dibawa ke aplikasi lain.
5. Dokumentasi dan UI menyebut status kemampuan secara akurat: stabil, terbatas,
   eksperimental, atau belum tersedia.

## 2. Prinsip pelaksanaan

- **Keandalan sebelum keluasan.** Jangan memperluas klaim fitur ketika perilaku
  edit, simpan, atau ekspornya belum lolos uji.
- **Satu model dokumen sebagai sumber kebenaran.** Sketch, fitur, parameter,
  datum, dan body perlu punya identitas/referensi yang stabil, bukan bergantung
  pada posisi indeks atau state UI.
- **UI mengikuti konteks kerja.** Tampilkan kontrol yang dibutuhkan untuk
  pilihan dan mode saat ini; jangan memaksa pengguna menghafal struktur internal.
- **Perubahan selalu dapat dibatalkan.** Operasi pengguna yang bermakna adalah
  satu transaksi undo yang konsisten, termasuk operasi multi-langkah.
- **Validasi dengan tugas nyata.** Unit test menjaga invarian; pengujian designer
  memastikan alur dan istilahnya masuk akal.

## 3. Keputusan awal: satu mesin pemodelan

Saat ini ada dua jalur pemodelan yang berjalan berdampingan:

| Jalur | Lokasi | Cara merujuk geometri | Dipakai oleh |
|---|---|---|---|
| Mesin parametrik GUI | `crates/ducad-app/src/modeling/parametric_engine.rs` | Indeks (`plane_index: usize`, urutan body) | Alur manual GUI, timeline |
| Oplog engine | `crates/ducad-engine` (`ops/spec.rs`, `session.rs`) | Selector semantik (`>Z`, `all[kind=cylinder][r=2.75]`, `FaceRef { body, face }`) | MCP, agent, proposal, checks, simulasi |

Selama keduanya hidup, "satu model dokumen sebagai sumber kebenaran" (Step 1)
tidak dapat tercapai: fitur yang dibuat lewat GUI dan lewat agent tidak berbagi
riwayat, undo, maupun aturan referensi yang sama.

**Keputusan (disetujui 2026-10-21; ADR menyusul di Step 1a):** oplog
`ducad-engine` menjadi satu-satunya sumber kebenaran
pemodelan. GUI menghasilkan op dan menampilkan hasil evaluasi oplog;
`parametric_engine` dipensiunkan secara bertahap. Alasannya:

- Oplog sudah atomik per batch, punya dry-run, rollback, error terstruktur
  dengan `hint`/`fixes`, dan tes — sebagian besar kebutuhan Step 1 dan Step 3.
- Selector semantik sudah memberi referensi yang tidak bergantung pada urutan
  indeks untuk kasus umum.
- Agent dan designer mengedit model yang sama, sehingga satu undo, satu format,
  dan satu jalur validasi cukup untuk keduanya.

**Kriteria selesai keputusan ini:** ADR ditulis (alternatif yang ditolak beserta
alasannya, rencana migrasi dokumen lama, dan daftar fitur GUI yang belum punya op
padanannya). Keputusan ini menjadi langkah pertama Step 1a.

## 4. Urutan dan ketergantungan

```text
Step 0: ukur kondisi sekarang dan tetapkan alur sasaran
   ↓
Step 1a: satu mesin pemodelan + satu dokumen + undo lintas domain
   ↓
Step 1b: referensi stabil + regenerasi deterministik
   ↓
Step 1c: format native, migrasi, autosave/recovery
   ↓
Step 2: alur sketch → solid → revisi yang dapat diandalkan
   ↓
Step 3: interaksi viewport, preview, dan pemulihan error
   ↓
Step 4: penyimpanan dan pertukaran berkas yang tervalidasi
   ↓
Step 5: onboarding, dokumentasi, dan rilis berbasis bukti
```

Step 0 dapat berjalan sebelum implementasi lain. Step 1 adalah prasyarat utama
untuk pekerjaan parametrik asosiatif pada Step 2. Step 1 sengaja dipecah menjadi
1a–1c, masing-masing dengan titik rilis sendiri, agar Step 2 bisa mulai setelah
1a dan 1b lulus tanpa menunggu seluruh pekerjaan format. Pengujian format pada
Step 4 boleh dimulai lebih awal, tetapi klaim stabilitas rilis ditentukan setelah
workflow Step 2–3 juga lulus.

## 5. Rencana per step

### Step 0 — Tetapkan baseline dan alur pengguna sasaran

**Tujuan**

Mengetahui bagian yang benar-benar bekerja saat ini dan menghindari pembangunan
berdasarkan asumsi. Pilih satu persona awal: designer produk yang membuat dan
merevisi part mekanik sederhana.

**Yang diubah**

- Tambah skenario uji produk ke `docs/CEKLIS_UJI_GUI.md`.
- Audit perbedaan daftar fitur publik di `README.md` (root repo, di luar
  `ducad-editor/`) dengan implementasi dan
  status di `docs/STATUS_FASE_A_B.md`.
- Catat baseline performa dan error untuk dokumen contoh; manfaatkan
  `docs/PERF_BASELINE.md` bila metrik yang dibutuhkan sudah tersedia.

**Perubahan teknis**

1. Definisikan satu proyek contoh yang representatif, misalnya bracket dengan
   sketsa berdimensi, ekstrusi, lubang, fillet, dan drawing sederhana.
2. Tuliskan tugas yang harus diselesaikan tanpa instruksi langkah demi langkah:
   membuat model, mengubah dimensi awal, memperbaiki operasi yang gagal,
   menyimpan, membuka ulang, dan mengekspor.
3. Rekam untuk tiap tugas: keberhasilan, durasi, kebingungan yang diamati,
   jumlah undo/pembatalan, dan kehilangan state/data.
4. Tandai kemampuan dalam matriks status: `stabil`, `terbatas`,
   `eksperimental`, atau `belum tersedia`. Sertakan bukti tes/manual untuk status
   `stabil`.
5. Tetapkan platform utama persona (desktop dengan mouse, atau iPad dengan
   sentuhan/pensil). Platform ini menentukan prioritas interaksi di Step 3.
6. Kalibrasi target angka di bagian 6 berdasarkan baseline yang terukur.
7. Tetapkan sejak kapan rencana ini berlaku dan bagaimana pekerjaan yang sedang
   berjalan di luar cakupan (mis. cloud sync, lisensi, akun) diselesaikan atau
   dibekukan.

**Target dan kriteria selesai**

- Ada satu workflow sasaran, dokumen uji, dan checklist yang dapat diulang.
- Platform utama, target angka, dan tanggal berlaku rencana sudah disepakati.
- Daftar fitur publik memiliki status yang dapat ditelusuri ke kode atau tes.
- Ada baseline tugas dan bug yang menjadi pembanding untuk step berikutnya.

### Step 1 — Selesaikan fondasi dokumen dan referensi parametrik

**Tujuan**

Membuat perubahan fitur stabil: fitur hilir merujuk objek yang sama meskipun
urutan geometri berubah setelah edit, undo, atau regenerasi.

**Yang diubah**

- Domain `ducad-core` dan model dokumen di `ducad-app`.
- Batas antara dokumen desain persisten dan state editor sementara.
- Referensi topologi yang digunakan oleh fitur asosiatif di `ducad-kernel` dan
  `ducad-engine`.
- Format native serta transaksi undo/redo.

Step 1 dipecah menjadi tiga sub-step dengan titik rilis masing-masing.

#### Step 1a — Satu mesin, satu dokumen, satu undo

> **Kemajuan 2026-10-21:** ADR 0010 ditulis; undo headless mencakup edit
> (B0-1); undo GUI membawa oplog; pembuat selector dari klik GUI; migrasi
> GUI urutan 1–4 (sketsa terikat, extrude, fillet/chamfer, hole) selesai —
> T1–T2 lewat GUI lulus tes otomatis. Urutan 5–10 (boolean, pattern,
> revolve/loft/sweep/shell/draft, datum bersudut, edit sketsa, direct,
> assembly) belum. Rincian: `docs/BASELINE_DESIGNER_UX.md` §8a.

1. Tulis dan setujui ADR "satu mesin pemodelan" (lihat bagian 3).
2. Audit semua referensi fitur yang memakai indeks face/edge/body atau indeks
   bidang; dokumentasikan pemilik data dan masa hidup referensinya, serta fitur
   GUI yang belum punya op padanan di oplog.
3. Arahkan perintah pemodelan GUI ke oplog `ducad-engine`; pensiunkan
   `parametric_engine` per fitur, dimulai dari fitur yang dipakai proyek contoh.
4. Selesaikan satu model `Document` yang menjadi sumber kebenaran untuk sketch,
   fitur (oplog), parameter, datum, dan body. Pindahkan state seleksi, kamera,
   hover, dan tool aktif ke `EditorState`, bukan ke dokumen desain.
5. Satukan operasi lintas domain ke transaksi dokumen. Satu aksi pengguna
   menghasilkan satu undo step; rollback mengembalikan dokumen ke keadaan
   sebelum operasi apabila validasi atau regenerasi gagal.

**Selesai bila:** proyek contoh dapat dibuat lewat GUI maupun agent ke dokumen
yang sama; undo/redo lintas sketch, fitur, dan parameter mengembalikan geometri
dan parameter dengan tepat; tidak ada operasi edit yang meninggalkan state
setengah diterapkan setelah error kernel.

#### Step 1b — Referensi stabil dan regenerasi deterministik

1. Jadikan selector semantik oplog (`>Z`, `|Z`, `of(...)`, filter jenis/radius)
   jalur utama referensi face/edge. Selector cukup untuk sebagian besar kasus
   designer dan tidak membutuhkan binding kernel baru.
2. Tambahkan riwayat hasil operasi kernel (`BRepTools_History`,
   `Generated`/`Modified`/`IsDeleted`) hanya sebagai pelengkap untuk kasus yang
   selectornya ambigu atau berubah arti setelah edit. Topological naming penuh
   bukan prasyarat Step 2.
3. Jika referensi benar-benar hilang atau selector menjadi ambigu, laporkan
   kegagalannya secara eksplisit, jangan diam-diam memilih face lain.
4. Bangun ulang regenerasi sebagai evaluasi dependency graph fitur yang
   deterministik. Setiap fitur menghasilkan output dan pemetaan referensi;
   kegagalan fitur menandai dependensi hilir dengan status yang jelas, bukan
   meninggalkan campuran geometri lama dan baru.

**Selesai bila:** tes regresi mengedit sketsa dasar dan memastikan fitur hilir
tetap mengacu pada face/edge yang benar, atau menjadi error terjelaskan bila
referensinya memang terhapus; regenerasi dokumen yang sama memberi hasil
deterministik.

#### Step 1c — Format native dan pemulihan

1. Lengkapi migrasi dan round-trip format native untuk dokumen lama, termasuk
   konversi dokumen berbasis `parametric_engine` ke oplog.
2. Autosave dan recovery harus menyimpan model dokumen yang konsisten, bukan
   hanya tampilan terakhir.

**Selesai bila:** simpan-buka dokumen mempertahankan oplog/dependency graph,
referensi, datum, dan hasil regenerasi; dokumen lama terbuka dengan hasil yang
sama atau laporan migrasi yang jelas.

### Step 2 — Poles alur utama sketch → solid → revisi

> **Kemajuan 2026-10-21:** butir 2–6 selesai (termasuk dimensi sketsa yang
> diikat ke `$param`); butir 1 sebagian (alur berjalan, urutan menu tidak
> diubah). Kriteria uji pengguna menunggu sesi Bagian P. Rincian:
> `docs/BASELINE_DESIGNER_UX.md` §8b.

**Tujuan**

Membuat workflow utama dapat dipelajari dan dipakai dari awal sampai revisi
tanpa harus memahami istilah internal engine.

**Yang diubah**

- UI sketch dan constraint pada `ducad-ui`/`ducad-app`.
- Perintah pembuatan dan pengeditan fitur di `ducad-engine`.
- Panel properti, feature tree/timeline, dan validasi dokumen.

**Perubahan teknis**

1. Susun alur tugas berdasarkan urutan kerja pengguna: pilih bidang, buat
   geometri, beri constraint/dimensi, validasi profil, buat fitur 3D, lalu
   revisi parameter.
2. Tampilkan status sketch yang dapat ditindaklanjuti: under-constrained,
   fully-constrained, atau konflik/redundan. Tautkan status ke entitas dan
   constraint yang relevan; jangan hanya menampilkan angka DOF.
3. Jadikan dimensi penting dapat diedit langsung dan terhubung ke parameter
   fitur. Validasi satuan, rentang, dan ekspresi sebelum commit.
4. Tampilkan dependency fitur secara terbaca: nama, parameter utama, status,
   dan fitur yang terdampak. Sediakan edit, suppress/unsuppress bila didukung,
   serta rollback yang konsisten.
5. Berikan validasi profil sebelum solid dibuat: loop terbuka, gap, region yang
   terpilih, dan kemungkinan hasil. Pesan harus menunjuk lokasi masalah dan
   menawarkan tindakan aman.
6. Definisikan satuan dan presisi sebagai bagian workflow. Perlihatkan satuan
   aktif, izinkan input yang eksplisit, dan hindari perubahan interpretasi angka
   yang tidak terlihat.

**Target dan kriteria selesai**

- Pengguna uji dapat membuat dan merevisi proyek contoh tanpa petunjuk lisan.
- Perubahan parameter awal memperbarui model secara benar dan terprediksi.
- Status constraint dan profil membantu pengguna menemukan masalah tanpa
  membaca log.
- Satu perintah pengguna tetap satu undo step, termasuk pengeditan dimensi.

### Step 3 — Perbaiki interaksi viewport dan pemulihan error

> **Kemajuan 2026-10-21:** butir 3–6 selesai (kernel berbatas waktu, fix
> diverifikasi dengan replay penuh, kategori error + langkah pemulihan,
> gizmo extrude lewat oplog). Butir 1–2 (audit picking/gizmo di perangkat)
> menunggu checklist manual. Rincian: `docs/BASELINE_DESIGNER_UX.md` §8c.

**Tujuan**

Membuat manipulasi langsung terasa presisi dan mencegah kegagalan operasi
menghentikan pekerjaan.

**Yang diubah**

- Picking, snapping, gizmo, dan preview di `ducad-app`/`ducad-ui`.
- Jalur error kernel/engine di `ducad-kernel` dan `ducad-engine`.
- Komponen preview proposal dan kartu error bila dipakai oleh alur manual.

**Perubahan teknis**

1. Audit pemilihan body/face/edge pada sudut kamera, zoom, geometri rapat, dan
   tablet. Prioritaskan platform utama yang ditetapkan di Step 0; bila iPad,
   uji sentuhan dan pensil sebagai jalur utama, bukan turunan dari mouse. Berikan highlight hover/selection yang konsisten dan affordance yang
   terlihat untuk pilihan yang ambigu.
2. Pastikan gizmo menggunakan transformasi world/screen yang konsisten, preview
   mengikuti pointer, dan angka parameter dapat diketik saat drag. Sediakan
   snap, axis lock, dan pembatalan yang dapat diprediksi.
3. Pisahkan preview sementara dari state model yang sudah di-commit. Jika preview
   gagal, jangan mengubah model; jika pengguna menerima, commit sebagai satu
   transaksi undo.
4. Normalisasi error operasi ke kategori yang dapat diterjemahkan UI: input
   tidak valid, tidak ada irisan, profil terbuka, radius tidak mungkin,
   referensi hilang, atau kegagalan kernel tak terduga. Mulai dari error
   terstruktur oplog yang sudah ada (`error`, `op_index`, `hint`, `fixes`) dan
   dry-run-nya, bukan membangun jalur error baru.
5. Untuk error umum, tampilkan sebab, konteks yang terukur jika tersedia,
   tindakan pemulihan, dan pilihan kembali ke keadaan sebelumnya. Tawarkan
   kandidat perbaikan hanya setelah kandidat lolos dry-run; jangan menerapkan
   perubahan tanpa persetujuan.
6. Audit jalur batas FFI/kernel agar kegagalan input menjadi error yang dapat
   dipulihkan, bukan panic atau penghentian aplikasi.

**Target dan kriteria selesai**

- Drag preview sama dengan hasil setelah commit; cancel tidak mengubah model.
- Pilihan dan gizmo lulus checklist desktop dan tablet yang didukung.
- Error umum tidak menyebabkan crash atau kehilangan pekerjaan.
- Setiap perbaikan otomatis yang disarankan tervalidasi sebelum ditampilkan.

### Step 4 — Validasi penyimpanan, impor/ekspor, dan drawing

**Tujuan**

Menjamin hasil dapat disimpan, dibuka kembali, dibagikan, dan digunakan dalam
workflow produksi yang dinyatakan didukung.

**Yang diubah**

- `ducad-io`, format native, dialog buka/simpan di `ducad-app`.
- Workflow drawing dan ekspor PDF/DXF/STEP/STL yang benar-benar ditargetkan.
- Fixture uji dan dokumentasi format.

**Perubahan teknis**

1. Tetapkan matriks format dan batas dukungan: entitas yang didukung saat impor,
   informasi yang dipertahankan, informasi yang hilang, serta batasan ekspor.
2. Tambahkan fixture nyata atau sintetis yang mencakup satuan, geometri analitik,
   spline/entitas yang didukung, multi-body, drawing, serta kasus malformed.
3. Uji round-trip native untuk geometri, parameter, dependency graph, datum,
   material, dan informasi drawing yang menjadi bagian dari format.
4. Uji STEP melalui pembukaan ulang dan pemeriksaan bentuk/geometri; uji DXF
   melalui jenis entitas, layer, dan ukuran; uji PDF melalui jumlah halaman,
   skala, teks, dimensi, dan pemeriksaan visual.
5. Untuk drawing, definisikan subset produksi yang dijamin terlebih dahulu.
   Tambahkan view, dimensi, anotasi, title block, atau multi-sheet hanya jika
   model dan ekspornya konsisten dalam subset tersebut.
6. Dialog ekspor harus menyatakan satuan/skala dan batasan yang relevan sebelum
   membuat berkas. Beri ringkasan hasil atau daftar item yang tidak diekspor.

**Target dan kriteria selesai**

- Round-trip native lulus untuk fixture dan versi format yang didukung.
- Format eksternal tidak diam-diam menghilangkan data yang diklaim didukung.
- Hasil PDF/DXF/STEP lolos validasi otomatis yang sesuai serta pemeriksaan
  visual/manual untuk output gambar.
- Dokumentasi membedakan format yang dapat dibuka, dibuat, dan round-trip.

### Step 5 — Onboarding, konsistensi produk, dan gerbang rilis

**Tujuan**

Membantu pengguna baru mencapai hasil pertama dengan cepat dan menjaga agar
kualitas yang diuji tercermin dalam klaim produk.

**Yang diubah**

- Onboarding dan contoh proyek di `ducad-ui`/`ducad-app`.
- `README.md`, `docs/PANDUAN.md`, `docs/CEKLIS_UJI_GUI.md`, dan catatan rilis.
- Gerbang validasi rilis.

**Perubahan teknis**

1. Buat proyek awal yang dapat diedit, bukan hanya tur tombol. Tutorial
   bertahap harus menghasilkan model yang sama dengan workflow sasaran dan
   mengajarkan edit/revisi, undo, simpan, dan ekspor.
2. Perlihatkan satu tindakan berikutnya yang relevan; izinkan pengguna
   melewati/mengulang tutorial tanpa mengubah dokumen kerja mereka.
3. Sinkronkan dokumentasi publik ke matriks status dari Step 0. Hapus atau
   beri label jelas pada klaim yang belum teruji pada workflow yang dijanjikan.
4. Jadikan workflow proyek contoh sebagai regression test otomatis sejauh
   memungkinkan (oplog/engine, round-trip, dan ekspor) dan jalankan di CI yang
   sudah ada (P0.6) sebagai lane yang memblokir; lengkapi bagian GUI dengan
   checklist manual pada platform yang didukung.
5. Catat metrik kualitas rilis: tingkat keberhasilan tugas, crash/error fatal,
   regresi penyimpanan, dan kegagalan ekspor.

**Target dan kriteria selesai**

- Pengguna baru dapat mencapai dan mengubah hasil tutorial tanpa panduan
  eksternal.
- README, panduan, UI, dan status implementasi tidak saling bertentangan.
- Gate rilis mencakup tes otomatis inti dan checklist GUI untuk workflow
  sasaran.

## 6. Ukuran keberhasilan produk

Ukur sebelum dan sesudah setiap perubahan besar. Target awal di bawah ini
dikalibrasi ulang setelah baseline Step 0.

| Ukuran | Cara mengukur | Arah | Target awal |
|---|---|---|---|
| Keberhasilan workflow utama | Sesi uji tugas tanpa bantuan | Naik | ≥ 4 dari 5 penguji menyelesaikan proyek contoh |
| Waktu sampai hasil pertama | Mulai aplikasi sampai model contoh berhasil dibuat | Turun | ≤ 20 menit untuk pengguna baru |
| Edit parametrik yang berhasil | Kasus edit awal dengan fitur hilir | Naik | 100% kasus regresi lulus atau error terjelaskan |
| Pemulihan setelah operasi gagal | Error → model tetap utuh → pengguna melanjutkan | Naik | 0 kehilangan pekerjaan dalam sesi uji |
| Regresi simpan/buka | Fixture native dan dokumen pengguna uji | Menuju nol | 0 regresi pada fixture |
| Ketepatan ekspor | Perbandingan geometri/drawing dan pembukaan di aplikasi target | Naik | Semua format berstatus `stabil` lulus validasi otomatis |
| Kebingungan UI | Salah mode, salah pilih, undo tidak sengaja dalam sesi observasi | Turun | Turun ≥ 50% dari baseline |

Jangan mengandalkan telemetry untuk data desain pribadi. Pengukuran dapat
dilakukan dengan dokumen uji dan sesi pengguna yang disetujui; kumpulkan hanya
data yang diperlukan.

## 7. Di luar cakupan sampai fondasi utama lulus

- Memperluas katalog simulasi, sheet metal, atau fitur industri.
- Menambah banyak format impor/ekspor tanpa matriks validasi.
- Menambah permukaan AI/agent baru sebelum alur manual dan undo aman.
- Menyebut fitur eksperimental sebagai kemampuan produksi.

Pekerjaan di luar cakupan dapat diprioritaskan ulang setelah data uji pengguna
menunjukkan kebutuhan dan Step 1–4 memenuhi kriteria selesai.

## 8. Dokumen dan area kode terkait

- `docs/STATUS_FASE_A_B.md` — status fondasi, solver, pemodelan, assembly, dan
  drawing; sinkronkan statusnya saat pekerjaan mendarat. Item P0.1–P0.5 di sana
  sesuai dengan Step 1a–1c.
- `crates/ducad-app/src/modeling/parametric_engine.rs` — mesin parametrik GUI
  berbasis indeks yang akan dipensiunkan (Step 1a).
- `crates/ducad-engine/src/ops/spec.rs`, `session.rs` — oplog dan selector yang
  diusulkan menjadi sumber kebenaran pemodelan.
- `docs/CEKLIS_UJI_GUI.md` — checklist GUI yang perlu ditambah dengan workflow
  sasaran lintas sketch, solid, revisi, dan berkas.
- `docs/PERF_BASELINE.md` — baseline performa yang dapat diperluas.
- `../README.md` (root repo) — daftar kemampuan publik yang perlu mengikuti
  bukti implementasi.
- `docs/BASELINE_DESIGNER_UX.md` — hasil Step 0: keputusan, baseline otomatis,
  temuan B0-1…B0-10, waktu.
- `docs/STATUS_KEMAMPUAN.md` — matriks status kemampuan Step 0.
- `crates/ducad-engine/tests/sample_project.rs` +
  `tests/fixtures/sample_bracket.ops.json` — proyek contoh sebagai tes CI.
- `crates/ducad-core`, `crates/ducad-engine`, `crates/ducad-kernel`,
  `crates/ducad-io`, `crates/ducad-ui`, dan aplikasi `ducad-app` — area teknis
  utama; lokasi modul rinci ditentukan setelah audit Step 0.

