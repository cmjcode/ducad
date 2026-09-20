# Ceklis Uji GUI DuCAD

Diperbarui: 2026-09-20. Untuk penguji manual (QA) dan pengembang sebelum rilis.

Tes otomatis (`cargo test --workspace`) menutup logika: kernel, engine, oplog,
checks, diagnosis error, pengenal coretan, jembatan agent, dan proposal.
Yang **tidak** bisa ditutup tes otomatis adalah apa yang hanya terlihat di layar:
tata letak, warna ghost, kartu yang muncul di tempat yang benar, gestur Pencil,
dan perilaku saat pengguna menekan tombol di tengah proses. Itulah isi dokumen
ini.

Cara memakai: kerjakan **Bagian A** setiap kali ada perubahan di `ducad-app`,
`ducad-ui`, atau `ducad-render`. Kerjakan **Bagian B** sebelum rilis atau setelah
perubahan besar. Catat hasilnya di tabel Bagian E.

Konvensi: ☐ belum diuji · ✅ lulus · ❌ gagal (tulis nomor isu) · ⏭ dilewati
(tulis alasan).

---

## 0. Persiapan

```bash
cd ducad-editor

# Build biasa (tanpa fitur opsional) — dipakai sebagian besar butir di bawah
cargo run -p ducad-app

# Build dengan asisten AI di perangkat (butuh macOS 26+ dan Apple Intelligence
# aktif; lihat docs/adr/0002-ai-lokal.md)
cargo run -p ducad-app --features apple-fm

# Build dengan memori MNEMONIC tertaut langsung (P11.5)
cargo run -p ducad-app --features memory
```

**Berkas uji.** Beberapa butir butuh part yang punya oplog + checks — ini tidak
bisa dibuat dari GUI, jadi siapkan dulu dengan CLI:

```bash
# 1. Salin fixture dan tambahkan checks
cp evals/fixtures/plate_param.ops.json /tmp/plate_checks.ops.json
# lalu tambahkan di JSON-nya, sejajar dengan "params" dan "ops":
#   "checks": [
#     { "id": "tebal",   "check": "bbox_size",  "body": "*", "expect": [60,40,8], "tol": 0.05 },
#     { "id": "lubang",  "check": "hole_count", "body": "*", "diameter": 5.5, "expect": 4 },
#     { "id": "dinding", "check": "min_wall",   "body": "*", "min": 2 }
#   ]

# 2. Bangun part-nya
cargo run -p ducad-cli -- run /tmp/plate_checks.ops.json --out /tmp/plate_checks.ducad
```

Hasil yang diharapkan: `committed: true` dan tiga check berstatus `pass`.
Berkas `/tmp/plate_checks.ducad` inilah yang dibuka di GUI untuk butir A1 dan A2.

---

## Bagian A — Fitur baru (wajib diuji setiap perubahan GUI)

### A1. Panel Checks dan ringkasan top bar (P7.5)

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| A1.1 | Buka `/tmp/plate_checks.ducad` | Plat 60×40×8 dengan 4 lubang muncul di viewport | ☐ |
| A1.2 | Lihat top bar | Ada ringkasan `✓ 3 ✗ 0`; tooltipnya "Hasil check desain (klik untuk detail)" | ☐ |
| A1.3 | Klik ringkasan itu | Panel Checks terbuka, tiga baris: `tebal`, `lubang`, `dinding`, semuanya ikon lulus | ☐ |
| A1.4 | Klik satu baris check | Body terkait terpilih dan kamera mengarah ke lokasi check | ☐ |
| A1.5 | Ubah geometri (mis. potong satu lubang lagi, atau geser body) | Ringkasan dihitung ulang sendiri; check yang gagal berubah ikon dan angkanya menyebut nilai terukur | ☐ |
| A1.6 | Seret gizmo tanpa melepas tombol | Hasil lama ditampilkan redup, TIDAK dihitung ulang tiap frame (UI tetap mulus) | ☐ |
| A1.7 | Buka dokumen baru (tanpa checks) | Ringkasan hilang dari top bar; panel bilang "Part ini belum punya check desain." | ☐ |

### A2. Jembatan agent live (P5)

Butuh dua terminal: satu menjalankan GUI, satu lagi mengirim perintah.

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| A2.1 | Buka Command Palette (`Cmd+K`), ketik "Agent" | Ada entri **Agent Bridge (jembatan agent live)** | ☐ |
| A2.2 | Jalankan entri itu | Status bar: "Agent Bridge aktif…"; chip **Agent: 0** muncul di top bar; berkas `~/.ducad/agent.sock` ada | ☐ |
| A2.3 | Jalankan `ducad-mcp --attach` lalu panggil `inspect` dari agent/klien | Chip berubah jadi **Agent: 1** saat klien terhubung; hasil `inspect` menggambarkan dokumen yang sedang terbuka | ☐ |
| A2.4 | Suruh agent menjalankan `run_ops` (mis. tambah primitif box) | Body baru **langsung terlihat** di viewport tanpa menyentuh GUI; drawer Aktivitas mencatat "Agent: n operasi" | ☐ |
| A2.5 | Tekan `Cmd+Z` sekali | SELURUH batch agent hilang dalam satu langkah undo (bukan satu op per undo) | ☐ |
| A2.6 | Minta agent memanggil `inspect` lagi setelah undo itu | Hasilnya memuat peringatan `oplog_stale` | ☐ |
| A2.7 | Minta agent memanggil `new_part` / `open_part` / `accept_proposal` | Ditolak dengan pesan "tidak tersedia pada sesi live" + saran yang masuk akal | ☐ |
| A2.8 | Matikan Agent Bridge lewat palette | Chip hilang; `~/.ducad/agent.sock` terhapus; permintaan agent berikutnya gagal dengan saran menyalakan Agent Bridge | ☐ |
| A2.9 | Nyalakan lagi, lalu tutup aplikasi dari tombol jendela | `~/.ducad/agent.sock` tidak tertinggal | ☐ |
| A2.10 | Pastikan privasi AI pada mode "Hanya di perangkat", lalu coba nyalakan jembatan | Ditolak dengan pesan kebijakan privasi, jembatan tetap mati | ☐ |

### A3. Kartu proposal + ghost preview (P8.4)

Lanjutan dari A2 (jembatan menyala).

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| A3.1 | Minta agent `propose_ops` yang MENAMBAH material (mis. box baru) | Ghost **hijau tembus pandang** muncul di viewport; kartu "Usulan agent" di kanan bawah berisi `+x mm³ / −y mm³` dan daftar op | ☐ |
| A3.2 | Perhatikan sisi agent | Agent **menunggu** — belum ada balasan sampai pengguna memilih | ☐ |
| A3.3 | Tekan **Terima** | Ghost hilang, geometri asli berubah, status "Usulan agent diterapkan", agent menerima `committed: true` | ☐ |
| A3.4 | Ulangi proposal, lalu tekan **Tolak** | Model TIDAK berubah; agent menerima `rejected: "user"` | ☐ |
| A3.5 | Proposal yang MENGHAPUS material (mis. shell atau potong) | Bagian yang hilang digambar **merah tembus pandang** | ☐ |
| A3.6 | Buat proposal, tekan `Cmd+Z` sebelum memilih, lalu tekan Terima | Ditolak dengan `proposal_stale` — model yang sudah berubah tidak boleh ditimpa | ☐ |
| A3.7 | Buat proposal lalu diamkan >120 detik | Kartu hilang sendiri, status "Usulan agent kedaluwarsa", agent menerima `rejected: "timeout"` | ☐ |
| A3.8 | Kirim proposal kedua saat yang pertama masih menunggu | Yang lama dijawab `rejected: "superseded"`, kartu menampilkan yang baru | ☐ |

### A4. Kartu error dengan tombol perbaikan (P9.3)

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| A4.1 | Buat balok kecil, pilih satu tepi, minta fillet dengan radius jauh lebih besar dari tepinya (mis. R20 pada balok 10 mm) | Kartu error muncul dengan judul yang bisa dibaca manusia (bukan kode mentah) + satu kalimat penyebab | ☐ |
| A4.2 | Perhatikan tombol di kartu | Ada tombol saran (mis. "Pakai radius 1,8 mm") — nilainya sudah diverifikasi, bukan tebakan | ☐ |
| A4.3 | Tekan tombol saran | Field radius terisi nilai itu dan operasinya dijalankan ulang — berhasil | ☐ |
| A4.4 | Tekan ✕ | Kartu tertutup, model tidak berubah | ☐ |
| A4.5 | Shell dengan tebal ≥ setengah dimensi terkecil | Kartu error tebal shell berlebih dengan saran 0,5× dan 0,25× | ☐ |

### A5. Alat Freehand (P12.3)

Paling penting di iPad, tapi **harus jalan juga dengan mouse/trackpad** supaya
bisa diuji di macOS.

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| A5.1 | Masuk mode sketsa, pilih alat **Freehand** di toolbar kiri | Kursor siap mencoret; ikon alat aktif | ☐ |
| A5.2 | Coret persegi kasar dalam satu tarikan | Coretan mentah abu-abu tipis + bentuk hasil (4 garis) tebal, plus glyph constraint di dekat entitas | ☐ |
| A5.3 | Diamkan 600 ms (pengaturan terima-otomatis aktif) | Usulan diterima sendiri; sketsa berisi garis ber-constraint | ☐ |
| A5.4 | Tekan `Cmd+Z` sekali | SELURUH hasil coretan hilang dalam satu langkah undo | ☐ |
| A5.5 | Coret lingkaran, lalu tekan `Esc` sebelum 600 ms | Usulan dibuang, sketsa tidak berubah | ☐ |
| A5.6 | Coret bentuk baru sementara usulan sebelumnya masih tampil | Usulan lama diterima lebih dulu, coretan baru tidak menelan usulan lama | ☐ |
| A5.7 | Coret huruf "S" | Dikenali sebagai spline (bukan dipaksa jadi garis/busur) | ☐ |
| A5.8 | Coret garis miring ~3° | TIDAK diluruskan paksa ke horizontal | ☐ |
| A5.9 | Coret persegi lalu langsung Extrude | Berhasil tanpa harus menambal celah manual | ☐ |

### A6. Asisten AI lokal (P11.4) — build `--features apple-fm`

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| A6.1 | Build tanpa fitur AI, buka palette | Entri "Tanya AI…" tetap ada, tapi dialognya bilang build ini tanpa backend di perangkat | ☐ |
| A6.2 | Build `--features apple-fm` di mesin yang mendukung | Chip **AI: di perangkat** muncul di top bar | ☐ |
| A6.3 | Buka part parametrik (`/tmp/plate_checks.ducad`), jalankan "Tanya AI…", ketik "tebal jadi 12 mm" | Spinner jalan, UI TIDAK beku (bisa orbit kamera saat model bekerja) | ☐ |
| A6.4 | Tunggu hasil | Dialog menampilkan alasan + daftar perubahan (mis. `t: 8 → 12`) dengan tombol Terapkan/Tolak | ☐ |
| A6.5 | Tekan Terapkan | Geometri berubah, tercatat di drawer Aktivitas | ☐ |
| A6.6 | Jalankan pada dokumen non-parametrik (hasil gambar manual di GUI) | Pesan jelas bahwa dokumen ini bukan part parametrik — bukan crash atau diam saja | ☐ |
| A6.7 | Tekan Batal saat model masih bekerja | Dialog kembali normal, hasil yang telat datang diabaikan | ☐ |

### A7. Memori MNEMONIC tertaut (P11.5) — build `--features memory`

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| A7.1 | Jalankan `--features memory` pertama kali | Folder `~/DUCAD-Memory` dibuat sendiri bila belum ada; aplikasi tidak melambat saat start (vault dibuka saat pertama dipakai) | ☐ |
| A7.2 | Isi vault (mis. jalankan `scripts/init_memory_vault.sh`), lalu pakai "Tanya AI…" | Pelajaran/preferensi dari vault ikut jadi konteks model | ☐ |
| A7.3 | Terapkan usulan AI, lalu lihat `~/DUCAD-Memory/Sessions/<tanggal>.md` | Ada baris log sesi hari ini | ☐ |
| A7.4 | Hapus/rusak folder vault lalu jalankan lagi | Aplikasi tetap jalan; memori sekadar tidak aktif (dicatat di log), bukan crash | ☐ |

### A8. Cabang dari histori (P8.5)

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| A8.1 | Lakukan beberapa operasi, buka drawer Feature Tree / Aktivitas | Daftar aktivitas terisi berurutan | ☐ |
| A8.2 | Pilih entri lama → "Buat cabang dari sini" | Dokumen kembali ke snapshot itu; status "Cabang cabang-N dibuat" | ☐ |
| A8.3 | Lakukan operasi baru | Tercatat di cabang baru, entri lama tidak hilang | ☐ |
| A8.4 | Pakai filter cabang di drawer | Bisa berpindah antara "Semua cabang" dan satu cabang tertentu | ☐ |
| A8.5 | Tutup dan buka lagi aplikasi | Database histori lama tetap terbaca (migrasi kolom tidak menghapus data) | ☐ |

---

## Bagian B — Regresi inti (sebelum rilis)

### B1. Sketsa 2D

| # | Yang diuji | Status |
|---|---|---|
| B1.1 | Line, Rectangle, Circle, Arc (center-radius & 3 titik), Ellipse, Polygon, Slot — semua tergambar dengan dimensi yang benar | ☐ |
| B1.2 | Snapping berjenjang: endpoint > midpoint > center > intersection > grid, glyph-nya terlihat | ☐ |
| B1.3 | Constraint: coincident, horizontal, vertical, parallel, perpendicular, equal, tangent, symmetric, distance, radius, angle | ☐ |
| B1.4 | Trim (highlight merah), Extend, Offset, Mirror | ☐ |
| B1.5 | Construction line (`X`) tidak ikut membentuk region tertutup | ☐ |
| B1.6 | Edit dimensi lewat pill di kanvas ("Tampilkan Semua Ukuran") | ☐ |
| B1.7 | Undo/redo sketsa berurutan tanpa entitas tertinggal | ☐ |

### B2. Solid 3D

| # | Yang diuji | Status |
|---|---|---|
| B2.1 | Extrude (blind, simetris, up-to-face) dan Cut Extrude | ☐ |
| B2.2 | Revolve (termasuk sumbu kustom), Loft, Sweep | ☐ |
| B2.3 | Boolean Union / Subtract / Intersect | ☐ |
| B2.4 | Fillet konstan, Fillet radius variabel, Chamfer | ☐ |
| B2.5 | Shell / hollow, Draft angle, Split body/face | ☐ |
| B2.6 | Hole Wizard: simple, counterbore, countersink, tapped (M2–M12) | ☐ |
| B2.7 | Push-pull sisi lewat gizmo, termasuk pratinjau prisma saat diseret | ☐ |
| B2.8 | Pattern linier & sirkular (2D dan 3D) dengan ghost pratinjau | ☐ |
| B2.9 | Teks emboss/deboss di permukaan datar | ☐ |
| B2.10 | Undo/redo model: body yang dihapus muncul lagi utuh, seleksi tidak menunjuk body hantu | ☐ |

### B3. Bidang, gambar kerja, perakitan

| # | Yang diuji | Status |
|---|---|---|
| B3.1 | Datum plane: offset, bersudut, 3 titik — dan menggambar sketsa di atasnya | ☐ |
| B3.2 | Drawing sheet: empat tampak, HLR, section A-A, detail view | ☐ |
| B3.3 | Dimensi otomatis dan manual, tabel BOM, balon nomor part, kop gambar | ☐ |
| B3.4 | Assembly: instance, mate concentric/coincident/distance/angle | ☐ |
| B3.5 | Clash detection menandai tabrakan antar body | ☐ |
| B3.6 | Exploded view hanya menggeser tampilan, tidak mengubah geometri/ekspor | ☐ |

### B4. Berkas dan interoperabilitas

| # | Yang diuji | Status |
|---|---|---|
| B4.1 | Simpan & buka `.ducad`; buka berkas v1 lama masih berhasil | ☐ |
| B4.2 | Simpan part yang dibuat agent → buka lagi: oplog + checks ikut tersimpan (panel Checks tetap terisi) | ☐ |
| B4.3 | Import STEP, STL, DXF (import besar jalan di latar, UI tidak beku) | ☐ |
| B4.4 | Export STEP, STL, OBJ, GLB, SVG, PDF, DXF | ☐ |
| B4.5 | Vector Snapshot SVG memakai proporsi viewport yang benar-benar terlihat | ☐ |
| B4.6 | UUID body stabil: simpan → buka → simpan lagi tidak mengacak identitas | ☐ |

### B5. UI/UX umum

| # | Yang diuji | Status |
|---|---|---|
| B5.1 | Command Palette (`Cmd+K`) dan Radial Menu (`Space`) | ☐ |
| B5.2 | ViewCube: Top/Front/Right/Iso dan orbit | ☐ |
| B5.3 | Semua drawer: Items, Planes, CMF, Lighting, Feature Tree, Assembly, Akun | ☐ |
| B5.4 | Studio lighting, SSAO, zebra, draft heatmap, section view | ☐ |
| B5.5 | Ganti tema terang/gelap dan ganti bahasa — tidak ada teks yang hilang atau kepanjangan | ☐ |
| B5.6 | Ganti satuan (mm/cm/inci) mengubah tampilan angka secara konsisten | ☐ |
| B5.7 | Semua pintasan papan ketik di tabel README masih benar | ☐ |

### B6. iPad (kalau ada perangkatnya)

| # | Yang diuji | Status |
|---|---|---|
| B6.1 | Mode Pencil Only: jari hanya orbit/pan/zoom, Pencil menggambar (palm rejection) | ☐ |
| B6.2 | Mode Finger Design: target sentuh 44pt, toleransi 14px | ☐ |
| B6.3 | Mode Hybrid | ☐ |
| B6.4 | Tata letak tidak bertabrakan: TopBar vs ViewCube, menu "⋯" muncul di layar sempit | ☐ |
| B6.5 | Freehand dengan Pencil terasa < 100 ms per coretan | ☐ |
| B6.6 | Build `--features memory`: vault di folder Documents aplikasi bisa dibaca/ditulis | ☐ |
| B6.7 | Agent Bridge **tidak** tersedia di iPad (memang desktop saja) — pastikan tidak ada tombol yang menyesatkan | ☐ |

---

## Bagian C — Yang sengaja TIDAK ada

Supaya tidak dilaporkan sebagai bug:

- **OCR dimensi tulisan tangan (P12.4)** belum diimplementasikan.
- **`accept_proposal` untuk agent di sesi live** memang tidak tersedia: hanya
  pengguna yang boleh menerima proposal. Ini pagar keselamatan, bukan kelalaian.
- **Agent Bridge di iPadOS** tidak ada (soket Unix, desktop saja). Memori di iPad
  memakai jalur tertaut (`--features memory`), bukan jembatan.
- **Backend AI di perangkat** hanya muncul bila OS mendukungnya (macOS 26+/
  iOS 26+, Apple Intelligence aktif). Detail dan jebakan penautan ada di
  `docs/adr/0002-ai-lokal.md`. iPadOS **belum pernah diuji di perangkat nyata**.
- **Merge antar cabang histori** di luar lingkup versi ini.

---

## Bagian D — Kalau ada yang gagal

1. Catat nomor butirnya, langkah persisnya, dan apa yang terlihat vs yang
   diharapkan.
2. Sertakan `~/.ducad/ducad_history.db` (kalau relevan) dan berkas `.ducad`-nya.
3. Untuk kegagalan yang berkaitan dengan agent, sertakan juga hasil JSON dari
   sisi agent: `error.code`, `error.hint`, dan `error.context` biasanya sudah
   menunjuk penyebabnya.
4. Jalankan `cargo test --workspace` sekali — kalau ikut merah, bugnya ada di
   logika, bukan di GUI.

---

## Bagian E — Catatan hasil

| Tanggal | Versi/commit | Platform | Bagian yang dikerjakan | Lulus | Gagal | Catatan |
|---|---|---|---|---|---|---|
| | | | | | | |
