# Ceklis Uji GUI DuCAD

Diperbarui: 2026-10-13. Untuk penguji manual (QA) dan pengembang sebelum rilis.

Tes otomatis (`cargo test --workspace`) menutup logika: kernel, engine, oplog,
checks, diagnosis error, pengenal coretan, jembatan agent, dan proposal.
Yang **tidak** bisa ditutup tes otomatis adalah apa yang hanya terlihat di layar:
tata letak, warna ghost, kartu yang muncul di tempat yang benar, gestur Pencil,
dan perilaku saat pengguna menekan tombol di tengah proses. Itulah isi dokumen
ini.

Cara memakai: kerjakan **Bagian A** setiap kali ada perubahan di `ducad-app`,
`ducad-ui`, atau `ducad-render`. Kerjakan **Bagian B** sebelum rilis atau setelah
perubahan besar. Kerjakan **Bagian P** (uji tugas designer) sebelum rilis dan
sebelum/sesudah tiap step di `docs/PLAN_DESIGNER_UX.md`. Catat hasilnya di
tabel Bagian E (Bagian P punya tabel sendiri, P.4).

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
| A5.10 | Coret beberapa garis/busur yang ujungnya meleset ±1 mm, juga satu lingkaran yang berhenti sebelum titik awal | Selama menggambar TIDAK ada yang ditutup: celah tetap terlihat, tidak ada garis penyambung atau seleksi otomatis | ☐ |
| A5.11 | Tekan tombol biru **Objek Tertutup** di HUD mengambang di bawah header (bukan di header). Pindah ke alat lain (mis. Garis/Select): HUD hilang; kembali ke Freehand: HUD muncul | Celah tersambung, sketsa menjadi objek tertutup; status bar "N objek tertutup dibuat" | ☐ |
| A5.12 | Ketuk objek, tekan **Ekstrusi** (atau tarik gizmo) | Solid berpermukaan terbentuk | ☐ |
| A5.13 | Coret bentuk tertutup yang ujungnya melingkar masuk dan memotong garisnya sendiri (seperti angka "6"), tekan **Objek Tertutup** | Setiap wilayah yang terbentuk menjadi objek TERPISAH (tidak dibuang); masing-masing bisa di-extrude | ☐ |
| A5.14 | Gambar persegi, lalu garis yang membelahnya dengan ujung kelebihan, tekan **Objek Tertutup** | Dua objek; hanya ekor kelebihan yang hilang | ☐ |
| A5.15 | Persegi garis rapi (tidak disilang apa pun), tekan **Objek Tertutup** | Tidak diubah; constraint tetap; status "Sketsa sudah berupa objek tertutup" | ☐ |
| A5.16 | Tekan **Objek Tertutup** dua kali, lalu `Cmd+Z` sekali | Tekan kedua tidak mengubah apa pun; satu undo mengembalikan coretan asli | ☐ |
| A5.18 | Kanvas ramai: persegi & lingkaran CAD (dengan constraint/fitur 3D), teks, lalu beberapa coretan pensil yang tumpang tindih — sebagian memotong objek CAD. Tekan **Objek Tertutup** | Hanya coretan pensil yang diubah; objek CAD, teks, dan fitur 3D tidak berubah dan tidak ada error | ☐ |
| A5.17 | Extrude profil yang tetap tidak valid | Kartu error muncul; tidak ada body kerangka tanpa permukaan | ☐ |

### A5b. Mode Tinta → objek tertutup

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| A5b.1 | Masuk **Mode Sketsa Tinta** (⌘+Shift+5) | HUD di bawah header: Kuas · Penghapus · Lasso · Bentuk Pintar · **Objek Tertutup** | ☐ |
| A5b.2 | Coret beberapa garis kasar yang membentuk persegi dan satu garis yang membelahnya | Tetap tampil sebagai tinta biasa | ☐ |
| A5b.3 | Tekan **Objek Tertutup** | Tinta kasar tersembunyi, pindah ke Mode Sketsa CAD, dua objek tertutup | ☐ |
| A5b.4 | Ketuk satu objek, tekan **Ekstrusi** | Solid 3D terbentuk dari objek itu saja | ☐ |
| A5b.5 | `Cmd+Z` tiga kali | Solid hilang → objek sketsa hilang → tinta tampil lagi | ☐ |
| A5b.6 | Lasso satu bentuk saja, tekan **Objek Tertutup** | Hanya coretan terpilih yang dikonversi | ☐ |
| A5b.7 | Nyalakan **Bentuk Pintar**, coret beberapa bentuk, lalu tekan **Objek Tertutup** | Coretan langsung jadi entitas sketsa (tanpa penutupan otomatis); tombol mengubahnya menjadi objek tertutup | ☐ |

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

### A9. Login akun DUCAD — Apple / Google / GitHub

Butuh server auth DUCAD yang hidup. Siapkan dulu sesuai `docs/AUTH_SERVER.md`.
**Sign in with Apple tidak bisa diuji lewat server lokal `http://`** — Apple
menolak Return URL non-HTTPS, jadi butir A9.4 dan A9.5 butuh server staging
ber-HTTPS.

```bash
# Menunjuk ke server lokal (hanya untuk uji Google/GitHub)
DUCAD_SERVER_URL=http://127.0.0.1:3000 cargo run -p ducad-app
```

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| A9.1 | Klik tombol akun di ujung kanan top bar | Tooltipnya "Akun DUCAD" (bukan "CMJCode"); drawer akun terbuka | ☐ |
| A9.2 | Lihat drawer saat belum login | Tiga tombol selebar panel dengan tinggi sama, berurutan: **Masuk dengan Apple** (hitam, logo Apple) → Google (biru) → GitHub (gelap) | ☐ |
| A9.3 | Baca teks pengantar drawer | Menyebut "Akun DUCAD"; tidak ada penyebutan CMJCode di mana pun | ☐ |
| A9.4 | Klik **Masuk dengan Apple** | Browser terbuka ke `appleid.apple.com`; layar Apple menyebut **DUCAD** | ☐ |
| A9.5 | Selesaikan login Apple, pilih "Sembunyikan Email Saya" | Halaman browser: "✨ Login DUCAD Berhasil!"; drawer berubah jadi akun terisi; email berupa `…@privaterelay.appleid.com` | ☐ |
| A9.6 | Klik **Masuk dengan Google** | Layar consent Google menyebut **DUCAD**, bukan nama aplikasi lain | ☐ |
| A9.7 | Klik **Masuk dengan GitHub** | Layar otorisasi GitHub menyebut **DUCAD** | ☐ |
| A9.8 | Selama menunggu, lihat drawer | Pesan "Menunggu login via Apple/Google/GitHub…" + tombol **Batal** | ☐ |
| A9.9 | Tekan **Batal**, lalu coba login lagi | Kembali ke daftar tombol; percobaan kedua tetap berfungsi (tidak ada port/ticket yang nyangkut) | ☐ |
| A9.10 | Tutup tab browser tanpa menyelesaikan login, tunggu 3 menit | Status jadi galat "Proses login timeout setelah 3 menit", aplikasi tetap responsif | ☐ |
| A9.11 | Matikan server auth, lalu klik salah satu tombol login | Aplikasi TIDAK hang; poll gagal berulang lalu berakhir timeout dengan pesan jelas | ☐ |
| A9.12 | Login berhasil, lalu tutup dan buka lagi aplikasi | Masih login (`~/.ducad/session.json` terbaca); nama/inisial tampil di tombol akun | ☐ |
| A9.13 | Tekan **Keluar dari Akun** | Status "Berhasil keluar dari akun DUCAD."; `~/.ducad/session.json` terhapus; drawer kembali ke tiga tombol | ☐ |
| A9.14 | **Build sandbox macOS** (`./build_macos.sh`, bukan `cargo run`): ulangi A9.4/A9.6/A9.7 | Login tetap berhasil lewat ticket polling walau App Sandbox memblokir listener loopback. Log memuat "Listener loopback tidak tersedia" — itu normal, bukan galat | ☐ |
| A9.15 | **Build iPad** (`./build_ipad.sh`): ketuk tombol login | Safari benar-benar terbuka (tidak diam di "Menunggu login…"), dan login selesai setelah kembali ke aplikasi | ☐ |
| A9.16 | Di iPad, kembali ke DUCAD sebelum login selesai lalu tunggu | Token tetap masuk lewat polling tanpa perlu menyentuh apa pun | ☐ |
| A9.17 | Server sengaja tanpa `APPLE_CLIENT_ID`, lalu klik Apple | Pesan galat dari server ditampilkan apa adanya ("Sign in with Apple is not configured…"), bukan timeout senyap | ☐ |

### A10. Mode Vektor & Desain Grafis (M2)

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| A10.1 | Tekan `Cmd+Shift+2` untuk masuk mode sketsa 2D / Vektor | Toolbar vektor aktif (Pen Bézier, Node Edit, Shape Builder, Eyedropper, dll.), panel Properti & Layer mengambang di kanan | ☐ |
| A10.2 | Gambar logo gabungan: buat Persegi (`R`), Lingkaran (`C`), dan Teks (`T`) saling bertumpuk | Ketiga objek ter-render dengan fill & stroke presisi pada kanvas | ☐ |
| A10.3 | Pilih Persegi dan Lingkaran, lalu tekan `Cmd+L` (atau tombol "Gabung" di context bar) | Kedua kurva melebur jadi satu bentuk gabungan utuh via Operasi Boolean (Union) | ☐ |
| A10.4 | Buka tab **Properti** di panel samping: pilih bentuk baru, ubah fill ke Gradien (Linear/Radial) atau Solid, atur Opacity dan Blend Mode | Tampilan objek langsung ter-update halus; slider drag menggabungkan riwayat undo (coalesce) | ☐ |
| A10.5 | Buat Layer baru di tab **Layer** (`+`), ganti nama (dobel-klik), pindahkan objek ke layer baru, uji toggle Mata (visibilitas) dan Gembok (kunci) | Objek pada layer terkunci tidak dapat diedit/dipilih; toggle visibilitas menyembunyikan/menampilkan dengan benar | ☐ |
| A10.6 | Uji pintasan produktivitas: `P` (Pen), `N` (Node Edit), `I` (Eyedropper), `Cmd+G` (Grup), `Cmd+Shift+G` (Ungroup), `Cmd+]`/`[` (Z-Order) | Tool dan aksi merespons seketika sesuai shortcut | ☐ |
| A10.7 | Tekan `Cmd+Z` berulang kali (hingga 20 langkah undo), lalu `Cmd+Shift+Z` (redo) | Semua operasi vektor (pembuatan bentuk, boolean, perubahan properti warna, layer, grouping) kembali secara konsisten tanpa artefak | ☐ |
| A10.8 | Simpan berkas (`Cmd+S`), tutup dokumen / aplikasi, lalu buka kembali berkas `.ducad` yang disimpan | Tampilan vektor, struktur layer, palet warna swatches dokumen, dan geometri identik dengan kondisi saat disimpan | ☐ |

---

### A11. Properti massa & material mekanik (P16)

- [ ] ⌘K → "Properti Massa" membuka panel di kanan atas; tanpa body tampil "Belum ada body".
- [ ] Buat balok 10×20×30: volume 6000 mm³, pusat massa (5, 10, 15), massa 7,2 g (preset visual bawaan).
- [ ] Pilih material "Baja S235": massa menjadi 47,1 g; ⌘Z mengembalikannya.
- [ ] "Kustom…" → isi rasio Poisson 0,6: tombol Terapkan mati dan alasan tampil.
- [ ] Ganti satuan ke kg / m: semua baris ikut berubah; tombol salin menaruh tabel di papan klip.
- [ ] Nyalakan penanda pusat massa: titik kuning + tiga sumbu utama muncul dan ikut bergerak saat body digeser.
- [ ] Angka panel sama dengan `ducad-cli inspect BERKAS.ducad --mass` untuk berkas yang sama.
- [ ] Simpan, tutup, buka lagi: material mekanik masih terpilih.

---

### A12. Simulasi statik (P17)

- [ ] ⌘K → "Simulasi (studi statik)" membuka panel; tanpa studi tampil petunjuk "Tekan +".
- [ ] Buat balok, beri material lewat panel Properti Massa. Tekan +, klik face bawah → + Tumpuan; klik face atas → + Beban (gaya 0, 0, −100 N); "Buat studi".
- [ ] Jalankan: spinner tampil, UI tetap responsif (orbit kamera lancar), lalu von Mises maks, deformasi, faktor keamanan, dan reaksi terisi; reaksi ≈ lawan beban.
- [ ] Viewport berwarna; ganti overlay Tegangan / Deformasi / Faktor keamanan dan legenda ikut berubah; slider skala deformasi menggeser bentuk.
- [ ] Jalankan studi besar (sel kecil) lalu tekan Batalkan di tengah: status kembali, aplikasi tidak beku.
- [ ] Ubah geometri body (mis. fillet): status studi menjadi "basi" dan overlay meredup sampai dijalankan ulang.
- [ ] Body tanpa material: Jalankan memberi pesan gagal yang menyebut material.
- [ ] Angka panel sama dengan `ducad-cli sim BERKAS.ducad` untuk berkas yang disimpan dari sesi yang sama.
- [ ] Tombol hapus menghilangkan studi dari daftar.

### A13. Panel Fitur Industri (P18–P20)

Buka: ⌘K → "Fitur Industri". Membukanya menutup panel Simulasi (sudut yang sama), dan sebaliknya.

**Tab Studi**
- [ ] Balok bermaterial: Studi baru → Frekuensi natural → klik face ujung → "Tambah tumpuan" → Buat → Jalankan. Spinner tampil, UI tetap responsif, lalu daftar "Mode n: … Hz" muncul menaik.
- [ ] Buckling: tumpuan + beban tekan pada face seberang → faktor tekuk tampil; tanpa beban, tombol Buat tidak aktif.
- [ ] Termal: dua syarat batas suhu pada dua face → suhu maks/min sama dengan yang dimasukkan. Hanya fluks panas → Buat tidak aktif.
- [ ] Tegangan termal dengan mesh tetra → von Mises, deformasi, faktor keamanan tampil.
- [ ] Batalkan di tengah studi besar: status "dibatalkan", aplikasi tidak beku.
- [ ] Ubah geometri: baris studi menampilkan "Hasil basi".
- [ ] Studi di tab ini tidak muncul di panel Simulasi, dan sebaliknya.

**Tab Konfigurasi**
- [ ] Desain berparameter (dibuat agent / dibuka dari `.ducad`): centang parameter, ubah nilainya, beri nama, Simpan → model berubah dan konfigurasi baru aktif.
- [ ] Pilih "Default" → model kembali; ⌘Z membatalkan pergantian konfigurasi.
- [ ] Centang op di "Op yang dilewati" → fitur itu hilang pada varian tersebut.
- [ ] Ekspor CSV lalu Impor CSV yang sama → daftar konfigurasi tidak berubah.
- [ ] Nama "Default" atau nama kosong: tombol Simpan tidak aktif.

**Tab Sheet metal**
- [ ] Buat pelat dasar 100×60×2 → body muncul di daftar.
- [ ] Flange tepi "Sejajar X", panjang 20, sudut 90 → dua dinding terlipat; ulangi "Sejajar Y".
- [ ] Hem dan Jog pada sisi yang masih bebas menghasilkan bentuk yang benar.
- [ ] Bentangkan → body menjadi pola datar dan berlabel "terbentang"; Lipat kembali memulihkannya.
- [ ] "Buat pola datar" menambah body baru; "Ekspor DXF pola datar" menghasilkan DXF dengan layer OUTLINE / BEND_UP / BEND_DOWN.
- [ ] Flange pada tepi yang sudah berflange memberi pesan gagal, model tidak berubah.

**Tab Toleransi**
- [ ] Rantai dua mata rantai ±0,1: total kasus terburuk dan RSS langsung terhitung; isi suaian "H7" → kolom plus/minus hilang dan total berubah; suaian "Q7" → pesan error merah.
- [ ] "Jadikan check desain" → check baru muncul di panel Checks dengan status lulus/gagal sesuai batas.
- [ ] Tambah bingkai kontrol, datum, dimensi bertoleransi, dan kekasaran → "Buka lembar gambar" memperlihatkan keempatnya di posisi yang diisi; ekspor PDF dan SVG memuatnya.
- [ ] Label datum tidak sah (mis. huruf kecil) ditolak dengan pesan.

**Tab Part standar**
- [ ] Sisipkan ISO 4762 M6×20 di titik asal; lalu klik sebuah face → "Pakai titik face terpilih" → sisipkan mur di sana.
- [ ] Ganti standar → daftar ukuran ikut berganti; kolom panjang hanya tampil untuk baut dan pin.
- [ ] Klik face silinder batang baut → "Tambah ulir" kosmetik: tercatat di daftar bawah, geometri tidak berubah.
- [ ] Hilangkan centang "Kosmetik" → alur ulir terpotong (butuh beberapa detik).

**Tab Rakitan**
- [ ] Dengan dua body: tambah kopling roda gigi rasio −2 → tampil di daftar; putar penggerak (seret / studi gerak) → yang digerakkan ikut berputar berlawanan.
- [ ] Tambah dua langkah urai → geser "Faktor urai" dari 0 ke 1: part lepas berurutan, bukan serentak.
- [ ] Hapus kopling dan langkah urai lewat tombol tempat sampah.

---

### A14. Tutorial selamat datang (first-run)

Persiapan: hapus `~/.ducad/onboarding.json` (atau jalankan dengan `HOME` kosong),
lalu buka aplikasi. `DUCAD_SKIP_ONBOARDING=1` mematikan tutorial.

- [ ] Kartu sambutan muncul di tengah, latar meredup, semua ikon terender (bukan kotak).
- [ ] Tombol bahasa di kartu sambutan mengganti teks kartu dan seluruh aplikasi.
- [ ] Kartu sambutan menampilkan tiga bab; "Mulai Bab 1/2/3" melompat ke langkah pertama bab itu.
- [ ] Bab 1 langkah 0 (tur layar, 7 halaman): tiap halaman menyorot bagian UI yang benar (bilah atas,
  bilah kiri, tombol mode, ViewCube, palet, tombol bantuan) dan "Lanjut" berpindah halaman dulu.
- [ ] Kepala kartu menulis nama bab dan "Langkah n dari m"; kartu di kanan atas dan bisa digeser.
- [ ] "Lanjut" nonaktif sampai geometrinya berubah; setelah lulus muncul centang hijau.
- [ ] Bab 1: lingkaran R120, extrude 6 (otomatis ke 3D), orbit, "Sketsa di Face" + lingkaran R30,
  potong tembus (volume berkurang), chamfer tepi luar, simpan.
- [ ] Bab 2 dari kartu sambutan dengan kanvas kosong: cakram berlubang poros muncul otomatis, mode 3D.
- [ ] Bab 2: lingkaran baut, pattern 5× (lulus hanya setelah ≥4 salinan), potong, slot, pattern 12×,
  potong, fillet lubang poros, Ukur Jarak via palet, ganti nama body.
- [ ] Bab 3 dari kartu sambutan: cakram dengan lubang baut dan ventilasi muncul otomatis.
- [ ] Bab 3: hub extrude (volume bertambah), Shell, Hole Wizard, irisan, material (Properti Massa),
  simulasi sampai ada hasil, PDF dari lembar gambar (kartu tersembunyi selama lembar terbuka),
  ekspor STEP, Chat AI.
- [ ] Kartu akhir bab menawarkan "Lanjut ke bab berikutnya" dan "Cukup dulu".
- [ ] Animasi kamera otomatis setelah extrude TIDAK meluluskan pelajaran navigasi.
- [ ] Kartu tidak menutupi bilah konteks bawah, kartu panduan tool, maupun sidebar Chat AI.
- [ ] "Lewati langkah ini" maju satu langkah; tombol tutup menutup tutorial.
- [ ] Setelah selesai atau dilewati, membuka ulang aplikasi tidak menampilkan tutorial lagi.
- [ ] Tombol bantuan (ikon tanda tanya, kiri tombol bagikan) dan palet perintah → "Tutorial: mulai dari awal" membuka kartu sambutan lagi.
- [ ] iPad / jendela sempit: kartu tetap di dalam layar dan tombolnya nyaman disentuh.

---

### A15. Lembar gambar produksi (P21, acuan valve body)

Persiapan: `ducad-cli run crates/ducad-engine/tests/fixtures/valve_body.ops.json --out /tmp/valve.ducad`
lalu buka `/tmp/valve.ducad` dan masuk ke Lembar Gambar. (Catatan: nomor A14
sudah dipakai tutorial, jadi skenario "A14" di rencana P21 ada di sini.)

- [ ] Lembar tersimpan terbuka apa adanya: A3, skala 1:2, Tampak Depan + Atas, Section A-A
  (bertingkat) dan B-B, dua render berbayang di kolom kanan, tiga catatan di atas kepala gambar.
- [ ] Lingkaran halus saat diperbesar (bukan poligon); grid zona 8…1 dan A…F; semua ikon terender.
- [ ] Judul Section B-B menulis "SCALE 1:2"; tampak lain tidak menulis skala.
- [ ] Dimensi otomatis memuat Ø42, Ø60, Ø98, Ø146, `4×Ø14 PCD Ø130` (dengan lingkaran PCD), 88, 120,
  166, `2×45°`, `5×45°`, R5; tidak ada teks dimensi yang saling menimpa.
- [ ] Klik kanan sebuah tampak → menu skala (1:1, 1:2, 1:5, 2:1, kustom, ikuti lembar); judulnya
  ikut menulis "SCALE …" dan dimensi tetap menampilkan nilai model.
- [ ] Alat Section (ikon belah / tombol S): klik dua titik di Tampak Atas → potongan berlabel huruf
  berikutnya muncul; kartu panduan tampil di kiri bawah; Shift+klik membuat potongan bertingkat;
  Esc membatalkan.
- [ ] Sorot huruf garis potong → tombol balik arah dan tombol hapus muncul; balik arah mencerminkan
  tampak potongan; hapus menghilangkan garis potong, tampak, dan dimensinya.
- [ ] Alat dimensi (M): klik pusat dua lubang → dimensi linear bernilai model; mode Ø: klik pusat
  lalu tepi lubang → `Ø14`. Geser dimensi: garis/teksnya mengikuti kursor.
- [ ] "Regenerasi dimensi otomatis" mengembalikan dimensi otomatis ke tata letak bawaan.
- [ ] "Sisipkan render 3D" menambah render berbayang dari arah kamera viewport; render bisa digeser.
- [ ] Geser sebuah tampak: dimensi miliknya ikut bergerak, dimensi tampak lain diam.
- [ ] Tutup lembar, ubah param `bore_d` menjadi 45 (panel properti / `set_params`), buka lembar:
  lencana "Lembar kedaluwarsa" muncul; klik → `Ø42` menjadi `Ø45`, posisi dimensi lain tetap.
- [ ] Simpan, tutup aplikasi, buka lagi: posisi tampak, dimensi yang digeser, teks bebas, dan potongan
  tambahan kembali persis.
- [ ] Ekspor PDF dan SVG: tampilan sama dengan kanvas (tidak tercermin), render berbayang ikut.
  Ekspor DXF: tanpa gambar raster, layer VISIBLE/HIDDEN/CENTERLINE/DIMENSIONS/SECTION terisi.

### A16. Tutorial uji: paket desain, Review & Rilis, redline (W0–W6)

Tutorial klik demi klik, sekitar 25 menit. Setiap langkah punya dua bagian:
**Lakukan** (urutan klik) dan **Periksa** (yang harus terlihat; centang bila
benar). Kerjakan berurutan, karena tiap langkah memakai hasil langkah
sebelumnya.

Istilah letak: **bilah atas** = deretan tombol di tepi atas jendela.
**Menu Berkas** = tombol tiga garis di pojok kiri atas bilah atas.

#### Persiapan

**Lakukan** (di Terminal):

1. `cd ducad-editor`
2. Buat part uji:
   `cargo run -p ducad-cli -- run ../examples/design-package/plate_review.ops.json --out /tmp/plate_review.ducad`
   Tunggu sampai muncul JSON berisi `"committed": true`.
3. Salin profil firma contoh ke folder yang sama:
   `cp ../examples/design-package/firm.json /tmp/firm.json`
4. Jalankan aplikasi tanpa tutorial selamat datang:
   `DUCAD_SKIP_ONBOARDING=1 cargo run -p ducad-app`
5. Di aplikasi, ganti bahasa supaya label cocok dengan tutorial ini:
   klik **Menu Berkas** (tiga garis, kiri atas) → arahkan ke **Settings** →
   arahkan ke baris berikon bola dunia (**Language (English)**) → klik
   **Bahasa Indonesia**.
6. Buka part uji: klik **Menu Berkas** → **Buka…** → di dialog berkas tekan
   `⌘⇧G`, ketik `/tmp/plate_review.ducad`, Enter → **Open**.

**Periksa:**

- [ ] Di viewport tampak plat abu-abu dengan 4 lubang dan sudut membulat.
- [ ] Di bilah atas muncul ringkasan check berwarna hijau: `✔ 8 ✖ 0`.

#### Langkah 1 — Sitasi di panel Checks

**Lakukan:**

1. Klik ringkasan `✔ 8 ✖ 0` di bilah atas. Panel **Checks** terbuka di kanan
   atas.
2. Gulir daftar sampai bawah.
3. Klik tombol silang di pojok kanan atas panel untuk menutupnya.

**Periksa:**

- [ ] Ada 8 baris, semuanya bercentang hijau. (Baris `dfm-cnc/min-wall`
  boleh redup sebentar selagi dihitung.)
- [ ] Dua baris teratas bernama `size` dan `holes`, masing-masing dua baris
  teks (nama dan hasil ukur).
- [ ] Enam baris `dfm-cnc/…` punya baris teks KETIGA yang miring dan redup:
  sitasinya. Lima diawali `Guideline:`, satu (`dfm-cnc/general-tolerance`)
  diawali `ISO 2768-1:1989`.

#### Langkah 2 — Membuka panel Review & Rilis

**Lakukan:**

1. Tekan `⌘K`. Palet Perintah muncul di tengah layar.
2. Ketik `review`.
3. Klik hasil **Review & Rilis** (atau tekan Enter).

**Periksa:**

- [ ] Panel muncul di kiri atas area kerja, di kanan bilah alat kiri.
- [ ] Baris kepala: ikon ulasan biru, tulisan **Review & Rilis**, lalu
  `Draf · rev A` abu-abu, dan tombol silang di kanan. Tidak ada kotak kosong
  di tempat ikon.
- [ ] Bagian **Brief desain**: kotak pertama berisi `Motor plate`, kotak
  kedua berisi kalimat "Flat plate that carries…". Tombol **Simpan brief**
  redup (tidak bisa diklik), dan di sebelahnya tertulis `Proses: cnc`.
- [ ] Bagian **Persyaratan**: empat baris R1 sampai R4. R1, R2, R3 berikon
  centang hijau dengan nama check kecil di bawahnya. R4 `Anodised black`
  berikon tanda tanya oranye, tanpa nama check.
- [ ] Arahkan kursor ke ikon tanda tanya R4: muncul tooltip "Belum ada check
  yang memverifikasi".
- [ ] Bagian **Ceklis review**: lima baris. **Checks** dan **Standar** hijau;
  **Persyaratan** dan **Gambar** oranye; **Redline** hijau.
- [ ] Bagian **Rilis** di dasar panel terlihat tanpa menggulir: kotak "Nama
  Anda", kotak "Apa yang berubah di revisi ini", tombol **Ajukan review**,
  **Setujui** (redup), dan **Bangun paket PDF**.

#### Langkah 3 — Panel di jendela pendek

**Lakukan:**

1. Seret tepi bawah jendela aplikasi ke atas sampai jendela kira-kira
   setengah tinggi layar.
2. Arahkan kursor ke bagian tengah panel, lalu gulir.
3. Kembalikan ukuran jendela.

**Periksa:**

- [ ] Tombol **Ajukan review** dan **Setujui** tetap terlihat di dasar panel.
- [ ] Yang menggulir hanya bagian atas (brief sampai revisi).

#### Langkah 4 — Mengubah brief dan menambah persyaratan

**Lakukan:**

1. Klik kotak judul brief, ubah menjadi `Motor plate v2`.
2. Klik **Simpan brief**.
3. Klik kotak "Persyaratan baru, dengan angka dan satuan" (di bawah daftar
   persyaratan), ketik `Mass below 60 g`.
4. Klik **Tambah** di sebelahnya.

**Periksa:**

- [ ] Begitu judul diubah, tombol **Simpan brief** menyala. Setelah diklik,
  redup lagi dan di dasar panel tertulis "Brief disimpan".
- [ ] Sebelum mengetik persyaratan, tombol **Tambah** redup.
- [ ] Setelah **Tambah**: baris `R5  Mass below 60 g` muncul berikon tanda
  tanya oranye, kotak isian kosong lagi, dan dasar panel menulis
  "Persyaratan R5 ditambahkan".

#### Langkah 5 — Memasang redline di lembar gambar

**Lakukan:**

1. Di bagian **Redline (0 terbuka)** panel, klik **Tambah redline di
   lembar**. Lembar gambar terbuka memenuhi area kerja.
2. Lihat bilah alat di atas lembar: tombol berikon ulasan (di kanan tombol
   teks) sedang menyala merah. Itu alat Redline.
3. Klik sekali di kertas, sedikit di kanan atas Tampak Atas (kotak dengan 4
   lingkaran).
4. Panel Review & Rilis muncul lagi di kiri atas. Di kotak teks di bawah
   tulisan merah, ketik `Make the plate 10 mm thick`.
5. Klik **Pasang redline**.

**Periksa:**

- [ ] Setelah klik di kertas: alat Redline di bilah alat lembar mati sendiri,
  dan panel menampilkan tulisan merah "Posisi dipilih. Ketik perubahan yang
  diminta:".
- [ ] Sebelum mengetik, tombol **Pasang redline** redup.
- [ ] Setelah dipasang: di kertas, pada titik yang diklik, ada kotak
  bergaris MERAH berisi teks merah `RL1: Make the plate 10 mm thick`.
  Garis gambar, dimensi, dan kepala gambar tetap hitam.
- [ ] Panel: judul bagian menjadi **Redline (1 terbuka)**, ada baris `RL1`
  merah dengan teksnya, dan di bawahnya kotak "Apa yang diubah untuk
  memenuhinya" plus tombol **Selesai** (redup).
- [ ] Ceklis review: baris **Redline** berubah oranye. Baris **Gambar**
  berubah hijau.
- [ ] Bagian Rilis menampilkan tulisan merah "Masih ada redline terbuka".

#### Langkah 6 — Membatalkan redline kedua

**Lakukan:**

1. Di bilah alat lembar, klik tombol alat Redline (ikon ulasan) sampai
   menyala merah.
2. Klik di sembarang tempat kosong di kertas.
3. Di panel, klik **Batal**.

**Periksa:**

- [ ] Kotak teks redline di panel hilang.
- [ ] Tidak ada kotak merah kedua di kertas; panel tetap **Redline (1
  terbuka)**.

#### Langkah 7 — Redline bertahan saat lembar ditutup

**Lakukan:**

1. Klik tombol silang **Tutup Lembar Kerja** di ujung kanan bilah alat
   lembar.
2. Buka lagi: klik **Menu Berkas** → **Gambar Kerja 2D (Drawing Sheet)…**.

**Periksa:**

- [ ] Kotak merah `RL1` masih ada di posisi yang sama.

#### Langkah 8 — Tombol Setujui terkunci

**Lakukan** (lembar gambar boleh tetap terbuka):

1. Di panel, lihat tombol **Setujui**: redup.
2. Klik kotak "Nama Anda", ketik nama Anda.
3. Arahkan kursor ke tombol **Setujui**.

**Periksa:**

- [ ] Tombol **Setujui** tetap redup walau nama sudah diisi.
- [ ] Tooltip: "Isi nama Anda dan bereskan penghalangnya dulu".

#### Langkah 9 — Menutup redline

**Lakukan:**

1. Di baris RL1, klik kotak "Apa yang diubah untuk memenuhinya", ketik
   `Kept at 8 mm, agreed with reviewer`.
2. Klik **Selesai**.

**Periksa:**

- [ ] Baris RL1 hilang dari panel; judul bagian kembali **Redline (0
  terbuka)**; dasar panel menulis "Redline RL1 ditutup".
- [ ] Di kertas, kotak merah RL1 HILANG.
- [ ] Tulisan merah "Masih ada redline terbuka" hilang dan tombol
  **Setujui** sekarang menyala.

#### Langkah 10 — Mengajukan dan menyetujui revisi A

**Lakukan:**

1. Klik kotak "Apa yang berubah di revisi ini", ketik `Initial release`.
2. Klik **Ajukan review**.
3. Klik **Setujui**.

**Periksa:**

- [ ] Setelah **Ajukan review**: kepala panel menjadi `Direview · rev A`
  berwarna oranye. Gulir bagian atas panel sampai bawah: bagian **Revisi**
  berisi satu baris `A  <tanggal hari ini>  <nama>  Initial release`.
- [ ] Setelah **Setujui**: kepala panel `Disetujui · rev A` berwarna hijau.
- [ ] Di bagian Rilis muncul ikon terverifikasi hijau dan "Disetujui oleh
  <nama> (<tanggal>)".
- [ ] Tombol **Ajukan review** dan **Setujui** redup; muncul tombol baru
  **Buka kembali**.

#### Langkah 11 — Persetujuan gugur saat desain berubah

**Lakukan:**

1. Tutup lembar gambar (tombol silang di ujung kanan bilah alat lembar).
2. Pastikan mode 3D aktif (tombol mode di bilah atas).
3. Klik salah satu tepi lurus di sisi atas plat. Muncul gagang dan pilihan
   gaya Fillet/Chamfer.
4. Pilih **Chamfer**, ketik `1`, tekan Enter.

**Periksa:**

- [ ] Tepi yang dipilih menjadi miring (chamfer 1 mm).
- [ ] Kepala panel kembali `Draf · rev A` abu-abu.
- [ ] Di bagian Rilis muncul tulisan oranye "Desain berubah setelah
  disetujui. Ajukan revisi berikutnya." dan tulisan "Disetujui oleh…" hilang.

#### Langkah 12 — Revisi B

**Lakukan:**

1. Di kotak deskripsi ketik `Chamfer added on top edge`.
2. Klik **Ajukan review**.
3. Tekan `⌘Z` satu kali (membatalkan chamfer), lalu `⌘⇧Z` (mengulanginya).

**Periksa:**

- [ ] Kepala panel `Direview · rev B`.
- [ ] Bagian **Revisi** berisi dua baris: A (Initial release) dan B (Chamfer
  added on top edge).
- [ ] Setelah `⌘Z`, status TIDAK berubah menjadi "Disetujui · rev A".

#### Langkah 13 — Membangun paket PDF

**Lakukan:**

1. Klik **Bangun paket PDF** di bagian Rilis.
2. Di dialog simpan tekan `⌘⇧G`, ketik `/tmp`, Enter. Biarkan nama
   `plate_review-package.pdf`, klik **Save**.
3. Buka `/tmp/plate_review-package.pdf` di Preview, tampilkan bilah samping
   Daftar Isi (View → Table of Contents).

**Periksa:**

- [ ] Dasar panel menulis "Paket ditulis: plate_review-package.pdf (3
  lembar)".
- [ ] Daftar isi PDF: **Cover**, **Drawing - sheet1**, **Compliance**.
- [ ] Halaman 1 (Cover): judul besar `MOTOR PLATE V2`; baris Status "IN
  REVIEW - not released"; bagian DESIGN BRIEF; tabel SHEET INDEX tiga baris;
  tabel REVISIONS dua baris (A dan B); bagian COMPLIANCE SUMMARY.
- [ ] Halaman 2: sama dengan lembar di editor, TANPA kotak merah RL1.
- [ ] Halaman 3 (Compliance): tabel REQUIREMENT TRACEABILITY lima baris,
  R4 dan R5 berstatus `NO CHECK`; tabel DESIGN CHECKS delapan baris dengan
  kolom BASIS berisi sitasi utuh (terbungkus beberapa baris, tidak diakhiri
  "...").
- [ ] Kepala gambar halaman 1 dan 3: `PT Contoh Presisi`, nomor gambar
  `CP-plate_review`. Semua halaman: REV `B`, lembar `1 / 3`, `2 / 3`,
  `3 / 3`.
- [ ] Kalimat "…does not replace review and sign-off by a qualified
  engineer." ada di halaman 1 dan 3.

#### Langkah 14 — Tersimpan di berkas

**Lakukan:**

1. Tekan `⌘S`.
2. Keluar dari aplikasi (`⌘Q`).
3. Jalankan lagi: `DUCAD_SKIP_ONBOARDING=1 cargo run -p ducad-app`.
4. **Menu Berkas** → **Buka…** → `/tmp/plate_review.ducad`.
5. `⌘K` → ketik `review` → Enter.
6. Klik ringkasan check di bilah atas.

**Periksa:**

- [ ] Panel Review: judul `Motor plate v2`, lima persyaratan, kepala
  `Direview · rev B`, dua baris revisi.
- [ ] Panel Checks: masih 8 baris, sitasi masih ada.

#### Langkah 15 — Dokumen baru bersih

**Lakukan:**

1. Klik **Menu Berkas** → **Dokumen Baru**.
2. Lihat panel Review & Rilis (buka lagi dengan `⌘K` → `review` bila
   tertutup).

**Periksa:**

- [ ] Kepala panel `Draf · rev A`.
- [ ] Kotak brief kosong dan menampilkan tulisan "Belum ada brief…"; tidak
  ada persyaratan, redline, maupun revisi dari dokumen sebelumnya.
- [ ] Ringkasan check di bilah atas hilang.

#### Langkah 16 — Lampiran gambar di Chat AI

**Lakukan:**

1. Tekan `⌘⇧A`. Panel **Chat AI** terbuka di kanan.
2. Di kotak tulis paling bawah, cari tombol bulat berikon foto di kiri tombol
   kirim. Arahkan kursor ke sana.
3. Klik tombol itu, pilih sebuah berkas PNG atau JPEG.
4. Arahkan kursor lagi ke tombol.
5. Klik KANAN tombol itu.
6. Buat berkas palsu di Terminal: `echo halo > /tmp/palsu.png`. Klik tombol
   lampiran, pilih `/tmp/palsu.png`.

**Periksa:**

- [ ] Tooltip awal: "Lampirkan gambar (foto sketsa, gambar lama)".
- [ ] Setelah memilih gambar: tombol menjadi biru, tooltip "1 gambar
  terlampir. Klik untuk menambah, klik kanan untuk membuang".
- [ ] Setelah klik kanan: tombol kembali abu-abu.
- [ ] Untuk `palsu.png`: muncul baris merah di percakapan "Gambar tidak bisa
  dilampirkan: …" dan tombol tetap abu-abu.

#### Langkah 17 — Agent tidak bisa menyetujui (opsional)

Butuh Chat AI yang sudah disiapkan (ikon roda gigi di panel chat → centang
"Izinkan AI eksternal" → pilih agent). Buka lagi `/tmp/plate_review.ducad`.

**Lakukan:**

1. Di Chat AI ketik `setujui paket desain ini`, Enter.
2. Ketik `tambahkan redline di lembar: perbesar fillet sudut jadi R5`, Enter.

**Periksa:**

- [ ] Agent menjawab bahwa ia tidak bisa menyetujui; kepala panel Review
  tetap `Direview · rev B`.
- [ ] Setelah perintah kedua: panel menjadi **Redline (1 terbuka)** tanpa
  membuka ulang berkas. Buka lembar gambar (**Menu Berkas** → **Gambar Kerja
  2D…**): kotak merah baru terlihat.

#### Bukan bug (sudah diketahui)

- Check simulasi (`max_stress`, `min_safety_factor`, …) dan check sheet metal
  tampil "error" di panel Checks GUI untuk berkas buatan CLI, sehingga ikut
  mengunci tombol **Setujui**. Karena itu tutorial ini memakai part tanpa
  studi. Part seperti `sensor_bracket` disetujui lewat
  `ducad-cli release PART.ducad --approve --by NAMA`.
- Berkas yang disetujui lewat CLI lalu disimpan ulang dari GUI kembali
  berstatus draf, karena GUI mengadopsi geometri tanpa oplog.
- Impor PDF vektor (`import_pdf`) baru tersedia sebagai tool agent, belum
  ada tombolnya.

Bila sebuah butir **Periksa** tidak cocok, catat nomor langkahnya di Bagian E
beserta tangkapan layar.

---

### A17. Tutorial uji: layar "Proyek baru dari brief"

Satu layar untuk alur brief → agent bekerja → paket desain → revisi →
persetujuan. Butuh Chat AI yang sudah siap (provider API dengan "Izinkan AI
eksternal", atau CLI agent seperti Claude Code). Waktu: 10 menit, sebagian
besar menunggu agent (±2 menit per giliran).

Bila memakai CLI agent, pasang dulu server MCP terbaru supaya tool paket
desain terlihat olehnya: `make install-agent-tools` (di `ducad-editor/`).

#### Langkah 1 — Membuka layar

**Lakukan:**

1. Jalankan `DUCAD_SKIP_ONBOARDING=1 cargo run -p ducad-app`.
2. Klik **Menu Berkas** (tiga garis, kiri atas) → **Baru dari brief…**.
   (Cara lain: `⌘K` → ketik `brief` → Enter.)

**Periksa:**

- [ ] Layar penuh gelap menutupi kanvas. Kepala layar: **Proyek baru dari
  brief**, lalu `1 Brief › 2 Agent bekerja › 3 Paket desain` dengan "1 Brief"
  biru, nama agent aktif di kanan, dan tombol silang.
- [ ] Di tengah: judul besar "Apa yang ingin Anda buat?", kotak teks besar,
  tombol **Lampirkan sketsa atau foto**, pilihan **Proses**, kolom
  **Material** dan **Jumlah**, tiga tombol contoh, dan tombol biru **Buat
  paket desain** yang masih redup.
- [ ] Bila AI belum diizinkan: ada teks oranye beserta tombol **Buka
  pengaturan AI**, dan tombol biru tetap redup walau brief diisi.

#### Langkah 2 — Mengirim brief

**Lakukan:**

1. Klik tombol contoh pertama (diawali "Bracket L aluminium…"). Kotak teks
   terisi.
2. Biarkan Proses `CNC`. Isi Material `Al 6061-T6`, Jumlah `10`.
3. Klik **Buat paket desain**.

**Periksa:**

- [ ] Layar berganti ke fase 2. Kiri: kartu "Agent sedang bekerja" dengan
  daftar tahap; tahap aktif berikon biru dan berputar. Kanan: **Aktivitas**
  berisi kalimat agent dan nama tool bercentang hijau.
- [ ] Tahap menjadi centang hijau satu per satu: Memahami brief →
  Persyaratan dan aturan → Memodelkan part → (Mensimulasikan beban, bila
  brief menyebut beban) → Memverifikasi terhadap persyaratan.
- [ ] Tidak muncul kartu Terima/Tolak selama agent bekerja.
- [ ] Tombol **Hentikan** menghentikan agent; layar kembali ke brief dengan
  pesan bila belum ada model.

#### Langkah 3 — Paket desain

**Periksa** (setelah agent selesai, layar pindah sendiri ke fase 3):

- [ ] Kiri: judul part, `Draf · rev A`, daftar **Lembar** (Cover, Part - …,
  Compliance), dan kartu ringkasan check + persyaratan berwarna hijau/putih
  bila semuanya lulus.
- [ ] Tengah: lembar kertas putih dengan bingkai dan kepala gambar. Klik
  tiap lembar di daftar (atau panah kiri/kanan): sampul berisi brief, indeks
  lembar, tabel revisi; lembar part berisi tampak atas/depan/samping,
  potongan, isometrik, dimensi; lembar Compliance berisi tabel keterlacakan
  dan tabel check dengan kolom BASIS.
- [ ] Teks tabel tidak keluar dari bingkai kertas dan tidak menimpa kepala
  gambar.
- [ ] Kanan: **Minta perubahan**, **Persetujuan**, dan **Aktivitas**
  (ringkasan akhir agent).
- [ ] Klik **Buka di editor**: layar tertutup dan model 3D ada di viewport.
  Buka lagi lewat Menu Berkas → **Baru dari brief…**: langsung kembali ke
  fase 3 dengan paket yang sama.

#### Langkah 4 — Revisi dengan tanda di lembar

**Lakukan:**

1. Pilih lembar **Part - …**.
2. Klik sekali di gambar, di dekat salah satu lubang. Muncul lingkaran merah
   bernomor 1.
3. Di kanan, di baris bernomor 1, ketik `perbesar lubang ini jadi M6`.
4. Di kotak di bawahnya ketik `tebalkan alas jadi 7 mm`.
5. Klik **Kirim revisi**.

**Periksa:**

- [ ] Sebelum ada teks, tombol **Kirim revisi** redup.
- [ ] Setelah dikirim: di kanan muncul kartu "Sedang merevisi" dengan tahap
  dan tombol Hentikan; lembar tetap terlihat; klik di lembar tidak menambah
  tanda baru.
- [ ] Setelah selesai: lingkaran merah dan teks permintaan hilang, pesan
  "Revisi diterapkan: paket dibangun ulang", gambar part berubah (tebal 7,
  lubang lebih besar), dan model TIDAK dibuat ulang dari nol (riwayat undo
  berisi langkah revisi saja).
- [ ] Bila revisi melanggar persyaratan (mis. massa), ringkasan kiri menjadi
  merah dan agent menyebutkannya di Aktivitas.

#### Langkah 5 — Persetujuan dan ekspor

**Lakukan:**

1. Di **Persetujuan**, ketik nama Anda.
2. Klik **Ajukan dan setujui**.
3. Klik **Ekspor paket PDF**, simpan, buka berkasnya.

**Periksa:**

- [ ] Tombol **Ajukan dan setujui** redup selama nama kosong atau ada teks
  merah penghalang (check gagal). Selagi check dihitung, penghalangnya
  "Sedang dihitung ulang…".
- [ ] Setelah disetujui: ikon terverifikasi hijau dan "Disetujui oleh
  <nama> (<tanggal>)"; kepala kiri `Disetujui · rev A`; sampul di tengah
  menulis `APPROVED by …`.
- [ ] Kirim satu revisi lagi lalu setujui: revisi menjadi **B** dan tabel
  REVISIONS di sampul berisi dua baris.
- [ ] PDF hasil ekspor sama dengan yang tampil: jumlah lembar, isi, dan
  bookmark per lembar.

#### Jalan pintas untuk demo dan uji otomatis

```bash
# Langsung menjalankan agent dari brief, tanpa klik
DUCAD_SKIP_ONBOARDING=1 DUCAD_BRIEF_AUTORUN="Bracket L aluminium untuk CNC, alas 60x40x6, dua lubang M5" \
    cargo run -p ducad-app

# Membuka berkas dan langsung ke layar paket (lembar ke-2)
DUCAD_SKIP_ONBOARDING=1 DUCAD_OPEN_FILE=/tmp/part.ducad DUCAD_START_BRIEF=1 DUCAD_BRIEF_PAGE=2 \
    cargo run -p ducad-app
```

#### Bukan bug (sudah diketahui)

- Anotasi lembar (GD&T, redline dari panel Review) dan render berbayang
  belum digambar di penampil ini; keduanya tetap ada di PDF.
- Studi simulasi yang hasilnya belum ada dijalankan di thread UI saat paket
  dibangun: aplikasi bisa terasa berhenti beberapa detik.
- Agent menerapkan perubahannya langsung dari layar ini (tanpa kartu
  Terima/Tolak), walau pengaturan Chat AI meminta konfirmasi tulis.

---

### A18. Paritas SolidCraft Fase 0 (`docs/PLAN_PARITAS_SOLIDCRAFT.md`)

Semua perintah di bawah juga ada di command palette (Cmd+K).

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| 0.1 | Mode 3D → tombol **Primitif** di toolbar kiri → pilih Box/Silinder/Bola/Kerucut, isi ukuran, **Buat** | Body baru muncul di posisi X/Y/Z; Undo menghapusnya; ukuran 0/negatif ditolak dengan pesan | ☐ |
| 0.2 | Pilih 1 body → context bar **Cermin Body** → bidang XY/XZ/YZ atau datum, opsi "lewat pusat body" | Salinan cermin muncul (atau body asli diganti bila "Pertahankan asli" mati) | ☐ |
| 0.3 | Pilih 1 body → **Putar Body** → sumbu, sudut (snap 15°), tombol cepat ±15/45/90/180 | Body berputar presisi; gizmo rotasi lama tetap berfungsi | ☐ |
| 0.4 | Sketsa: klik kanan pill radius lingkaran | Label berganti R ↔ Ø; edit nilai Ø mengubah diameter | ☐ |
| 0.4 | Pilih 2 garis | Pill sudut muncul di perpotongan; klik → ketik 30 → Enter → sudut jadi 30° (constraint Angle) | ☐ |
| 0.4 | Hotkey **D**: 1 lingkaran / 1 garis / 2 garis | Edit radius / kunci panjang / edit sudut | ☐ |
| 0.5 | Pilih 2 lingkaran → constraint **Konsentris**; 2 garis → **Segaris**, **Midpoint**; garis + lingkaran → **Titik pada kurva** | Geometri ter-solve sesuai; Undo membatalkan | ☐ |
| 0.6 | Tool Arc → HUD **Titik pusat**: klik pusat, awal, akhir | Busur CCW dengan radius dari klik kedua; preview cocok | ☐ |
| 0.6 | Tool Rectangle → HUD **Pusat**: klik pusat lalu sudut | Persegi simetris terhadap pusat | ☐ |
| 0.7 | Tool Chamfer 2D → HUD: matikan "Jarak sama", Jarak 1 = 4, Jarak 2 = 2; atau ketik `4,2` | Kaki chamfer 4 dan 2 mm | ☐ |
| 0.8 | Buat teks dengan tool Text → double-click salah satu huruf → ubah isi → **Terapkan** | Teks diganti di posisi kiri-atas yang sama; satu Undo mengembalikan teks lama | ☐ |
| 0.9 | Palette **Parameter** (part dengan oplog `$w`) → ubah nilai / rename `w` → `lebar` → **Terapkan** | Model di-replay; kolom "Dipakai oleh" menunjuk op; hapus parameter yang masih dipakai ditolak | ☐ |
| 0.10 | Palette **Bandingkan Body** → Ambil snapshot → ubah model → Bandingkan | Volume bertambah hijau, hilang merah di viewport + tabel; Bersihkan menghapus ghost | ☐ |
| 0.11 | Palette **Inspect** dan **Salin oplog JSON** | Tabel ukuran/volume/face; clipboard berisi oplog JSON | ☐ |
| 0.12 | Drawer Assembly → edit mate Distance/Angle → isi Min/Maks → Terapkan | Solver menjepit nilai di dalam batas | ☐ |
| 0.13 | Pilih ≥ 2 body → palette **Jadikan grup kaku** → solve perakitan | Anggota grup bergerak bersama; **Lepas grup kaku** mengembalikan perilaku lama | ☐ |
| 0.14 | Constraint strip **Auto-constrain** (tanpa seleksi = seluruh sketsa) | Status "N constraint ditambahkan"; satu Undo menghapus semuanya | ☐ |
| 0.15 | Palette **Export gambar PNG** → tampilan, resolusi → Export | Berkas PNG tersimpan dan bisa dibuka | ☐ |
| 0.16 | Sketsa terbuka → Extrude (gagal) | Ujung terbuka disorot lingkaran merah; palette **Tampilkan ujung terbuka** menyalakan/mematikan | ☐ |

### A19 — Fase 1

#### Fase 1B 1.7–1.12 — mode gambar sketsa

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| 1.7 | Tool Circle → HUD **2 titik**: klik dua ujung diameter | Lingkaran berpusat di tengah kedua klik; preview mengikuti kursor | ☐ |
| 1.7 | Tool Circle → HUD **3 titik**: klik tiga titik (lalu coba tiga titik segaris) | Lingkaran melalui ketiganya; titik segaris ditolak dengan pesan, tidak crash | ☐ |
| 1.7 | Gambar dua garis bersilangan → Circle **2 tangen**: klik garis 1, garis 2, lalu klik posisi | Lingkaran menyinggung kedua garis, radius dari kursor; 2 constraint Tangent; klik di ruang kosong saat memilih kurva ditolak | ☐ |
| 1.7 | Segitiga → Circle **3 tangen**: klik ketiga sisi | Incircle muncul + 3 constraint Tangent; satu Undo menghapus lingkaran dan constraint | ☐ |
| 1.8 | Tool Arc → HUD **Tangen**: klik ujung garis/busur, lalu titik akhir | Busur menyambung mulus (tangen) dari ujung itu; constraint Tangent otomatis; klik awal yang bukan ujung ditolak | ☐ |
| 1.9 | Tool Rectangle → HUD **3 titik**: klik sisi miring p0→p1, lalu lebar | Persegi panjang miring mengikuti sisi pertama | ☐ |
| 1.9 | Tool Rectangle → HUD **Sudut bulat**, isi R sudut, klik dua sudut | 4 garis + 4 busur, 8 Tangent + 3 EqualRadius; R ≥ setengah sisi pendek ditolak dengan pesan | ☐ |
| 1.10 | Tool Slot → HUD **Titik pusat**: pusat, ujung setengah panjang, lebar | Slot simetris terhadap klik pertama | ☐ |
| 1.10 | Tool Slot → HUD **Busur (pusat)** / **Busur (3 titik)**: 3 klik busur tengah lalu lebar | 2 busur konsentris + 2 busur ujung; bisa di-extrude sebagai region tertutup | ☐ |
| 1.11 | Tool Polygon → HUD **Sisi**, N = 6: klik dua ujung sisi | Segi-6 beraturan dengan sisi tepat di antara kedua klik (di kiri arah klik) | ☐ |
| 1.12 | Tool Line → HUD **Titik tengah**: klik titik tengah lalu satu ujung | Garis simetris terhadap klik pertama | ☐ |
| 1.12 | Tool Line → HUD **Tak hingga**: klik dua titik, lalu pan/zoom | Garis konstruksi (oranye) memanjang sampai tepi viewport; tidak membentuk region; tersimpan saat file disimpan/dibuka | ☐ |
| 1.7–1.12 | Ganti mode di HUD atau tekan Esc di tengah penempatan | Titik tertunda dibuang, tool tetap aktif | ☐ |

#### Fase 1 — 1.1 titik sketch, 1.16 dimensi driven, 1.17 pindah teks dimensi, 1.20 kelengkungan G2

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| 1.1 | Sketsa → toolbar kiri **Titik** (atau palette "Titik") → klik di ruang kosong | Tanda "+" kecil muncul; tidak ada region baru; daftar entitas menampilkan "Titik (x, y)" | ☐ |
| 1.1 | Tool Titik → klik tepat di ujung garis / pusat lingkaran (snap aktif) | Titik menempel di sana + constraint Coincident; geser garis → titik ikut | ☐ |
| 1.1 | Tool Line/Circle → arahkan kursor ke titik yang sudah ada | Snap endpoint/center ke titik | ☐ |
| 1.1 | Pilih 2 titik → constraint strip **Jarak** / **Horizontal** / **Vertikal**; titik + garis → **Midpoint** / **Titik pada kurva** | Geometri ter-solve; Undo membatalkan | ☐ |
| 1.1 | Persegi + titik di dalamnya → Extrude; simpan lalu buka ulang; export DXF | Extrude tetap satu profil; titik tetap ada setelah dibuka; DXF berisi entitas POINT | ☐ |
| 1.16 | Pasang constraint Jarak pada garis → klik kanan pill dimensinya | Teks pill jadi "(nilai)" — dimensi acuan; ubah panjang garis lewat seret → nilai dalam kurung ikut berubah, garis tidak ditarik balik | ☐ |
| 1.16 | Klik kiri pill driven | Tidak membuka kotak edit; klik kanan lagi → kembali dimensi penggerak dengan nilai terukur saat itu (geometri tidak melompat) | ☐ |
| 1.16 | Pilih garis/lingkaran yang punya dimensi → palette **Ubah dimensi driven/penggerak (seleksi)** | Semua dimensi yang merujuk seleksi berganti status; satu Undo per dimensi | ☐ |
| 1.16 | Simpan berkas dengan dimensi driven → buka ulang; buka berkas lama (sebelum fitur ini) | Status driven bertahan; berkas lama terbuka tanpa perubahan | ☐ |
| 1.17 | Seret pill dimensi panjang garis / pill constraint Jarak ke posisi lain | Pill pindah, garis penunjuk tipis ke posisi asal; ubah geometri → label ikut bergeser bersama geometri | ☐ |
| 1.17 | Undo setelah menyeret; simpan → buka ulang | Undo mengembalikan posisi label; posisi tersimpan di berkas | ☐ |
| 1.17 | Palette **Kembalikan posisi teks dimensi** (dengan/ tanpa seleksi) | Label seleksi (atau semua) kembali ke posisi bawaan | ☐ |
| 1.20 | Gambar 2 spline yang ujungnya berdekatan → pilih keduanya → strip **Kelengkungan G2** | Ujung menyatu, sambungan mulus tanpa patahan kelengkungan; Undo membatalkan | ☐ |
| 1.20 | Spline + busur → **Kelengkungan G2**; coba 2 garis atau 2 busur | Spline menyesuaikan kelengkungan busur; tombol tidak muncul untuk garis/busur–busur | ☐ |


#### Fase 1A 1.2–1.6 — sumbu/titik datum, mode bidang baru, drawer Datum, pemakaian sumbu

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| 1.2 | Palette **Sumbu Datum** (atau drawer Datum → seksi Sumbu → +) → mode **Dua titik**: klik dua vertex body → **Buat Sumbu** (Enter) | Garis putus-putus oranye panjang melalui kedua vertex + label "Sumbu 1"; pilihan disorot hijau, pratinjau biru sebelum dibuat | ☐ |
| 1.2 | Mode **Sepanjang edge** (klik edge lurus / edge lingkaran), **Normal face** (klik face datar), **Sumbu silinder** (klik face silinder), **Dua bidang** (+ Bidang: Top lalu Front, atau klik dua face datar) | Sumbu sesuai; edge lingkaran memberi sumbu lingkarannya; dua bidang sejajar ditolak dengan pesan, tidak crash | ☐ |
| 1.3 | Palette **Titik Datum** → mode **Vertex**, **Pusat** (edge lingkaran / face bola), **Edge x bidang**, **Dua edge**, **Tiga bidang** (+ Bidang Top/Front/Right), **Sepanjang edge** (pecahan 0.25) | Penanda silang kuning + label "Titik N" di posisi yang benar; pecahan di luar 0..1 ditolak dengan pesan | ☐ |
| 1.4 | Tool Datum Plane → combo **Lainnya…** → **Bidang tengah**: klik dua face datar sejajar (atas & bawah kotak) → Buat | Bidang datum di tengah keduanya, langsung aktif untuk sketsa; dua face tidak sejajar ditolak dengan pesan | ☐ |
| 1.4 | **Tegak lurus edge** (klik edge di titik tertentu), **Singgung silinder** (klik face silinder, sudut 90), **Sepanjang lintasan** (edge + pecahan 0.5), **Melalui dua edge** (dua edge lurus sebidang) | Pratinjau bidang biru mengikuti pilihan; Buat → bidang di daftar; edge bersilangan ditolak dengan pesan | ☐ |
| 1.5 | Buka drawer **Datum** (palette **Drawer Datum**) | Tiga seksi: Bidang, Sumbu, Titik, masing-masing dengan jumlah dan tombol + | ☐ |
| 1.5 | Klik ikon mata pada bidang datum / sumbu / titik | Kerangka bidang / garis sumbu / penanda titik hilang dari viewport; klik lagi → muncul | ☐ |
| 1.5 | Klik ganda nama (atau ikon pensil) → ketik nama baru → Enter; Esc membatalkan | Nama berubah di drawer dan label viewport; nama kosong ditolak | ☐ |
| 1.5 | Hapus sumbu/titik dari drawer | Hilang dari drawer & viewport | ☐ |
| 1.5 | Simpan berkas berisi bidang datum (dengan sketsa di atasnya), sumbu, titik (sebagian disembunyikan) → buka ulang; buka berkas lama | Semua datum + sketsa bidang datum + status tampil kembali; berkas lama terbuka tanpa datum, tanpa error | ☐ |
| 1.6 | Sketsa persegi di Top (x 10..15) + sumbu datum sepanjang Y (dua bidang Front & Right) → pilih profil → tool **Revolve** → panel "Sumbu: datum" → pilih sumbu → **Revolve mengelilingi sumbu** | Body revolve terbentuk; sumbu yang tidak terletak di bidang XY (mis. sumbu Z) ditolak dengan pesan engine | ☐ |
| 1.6 | Pilih body → tool **Pattern** 3D melingkar → panel "Sumbu: datum" → pilih sumbu datum → Apply | Salinan mengelilingi sumbu datum (bukan preset sumbu HUD); pilih "(preset sumbu HUD)" → perilaku lama | ☐ |
| 1.6 | Dua part (pertama di-ground) → Mate **Concentric** → klik face silinder part kedua → panel "Sumbu: datum" → pilih sumbu → **Konsentris ke sumbu** | Part kedua pindah sehingga sumbu silindernya berimpit dengan sumbu datum; tanpa part ground lain → pesan | ☐ |

#### Fase 1B 1.13–1.15, 1.18, 1.19 — Break, Project/Intersect, Copy/Paste, glyph constraint, Insert SVG

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| 1.13 | Dua garis bersambung (ujung A.akhir ↔ B.awal ber-Coincident) → toolbar kiri **Pecah** → klik garis A di tengahnya | A jadi dua garis yang bersambung di titik klik; Coincident ke B kini menempel potongan kedua; seret B → potongan ikut; satu Undo mengembalikan garis utuh | ☐ |
| 1.13 | Tool Pecah pada busur, lingkaran (→ dua busur setengah), dan spline; klik tepat di ujung garis; klik elips | Busur/lingkaran/spline terpecah; klik di ujung / elips → pesan "tidak bisa dipecah", tidak crash | ☐ |
| 1.14 | Body kotak → klik face atas → aktifkan sketsa di bidang Top + offset → palette **Proyeksikan tepi/face ke sketsa** | 4 garis linked (garis putus-putus ungu) muncul; grup "Proyeksi: <body>" di daftar entitas | ☐ |
| 1.14 | Pilih edge (mode pilih-edge) lalu palette Proyeksikan; palette **Irisan body dengan bidang sketsa** di bidang Front yang memotong kotak | Edge terproyeksi; irisan berupa persegi panjang linked; bidang yang tidak memotong body → pesan, tidak crash | ☐ |
| 1.14 | Setelah proyeksi, ubah ukuran body (push-pull face / set_params) lalu kembali ke sketsa | Entitas linked ikut berubah mengikuti body; tool Pecah pada entitas linked ditolak (hanya-baca) | ☐ |
| 1.14 | Pilih entitas linked → palette **Lepas tautan**; simpan → buka ulang berkas dengan entitas linked | Garis ungu hilang (jadi entitas biasa, bisa diedit); berkas menyimpan status linked dan terbuka tanpa error | ☐ |
| 1.15 | Pilih persegi ber-constraint → Cmd/Ctrl+C → gerakkan kursor → Cmd/Ctrl+V | Salinan muncul berpusat di kursor, constraint internal (Horizontal/Coincident) ikut; salinan langsung terpilih; satu Undo menghapus salinan | ☐ |
| 1.15 | Cmd/Ctrl+X pada seleksi; Cmd/Ctrl+V dua kali dari palette (**Tempel entitas sketsa**) | Cut menghapus seleksi (satu Undo); paste dari palette bergeser bertingkat 5 mm | ☐ |
| 1.15 | Salin teks biasa di aplikasi lain → Cmd/Ctrl+V di sketsa; Cmd/Ctrl+V dengan clipboard kosong | Teks asing diabaikan (clipboard internal dipakai); clipboard kosong → pesan, tidak crash | ☐ |
| 1.18 | Sketsa dengan Horizontal, Parallel, Coincident, Fixed | Ikon kecil (font ikon, bukan kotak "tofu") di dekat entitas; constraint dua entitas punya ikon di keduanya; dimensi tidak diberi glyph | ☐ |
| 1.18 | Klik satu glyph → Delete; Esc; Undo | Glyph tersorot biru; Delete menghapus constraint itu saja (bukan entitas); Undo mengembalikannya | ☐ |
| 1.18 | Baris constraint (seleksi apa pun) → tombol **Tampilkan/sembunyikan glyph constraint**, atau palette dengan nama sama | Semua glyph hilang/muncul | ☐ |
| 1.19 | Palette **Sisipkan SVG…** → skala 1 → Pilih berkas SVG (logo dengan rect, circle, path kurva) | Path muncul di sketsa aktif, pojok kiri-bawah di titik asal, ukuran 96 px = 25,4 mm; grup "SVG: <nama>"; satu Undo menghapusnya | ☐ |
| 1.19 | Sisipkan berkas SVG rusak / bukan SVG; skala 2 | Pesan gagal, tidak crash; skala 2 menggandakan ukuran | ☐ |

### A20 — Fase 2: solid & direct editing

Jendela **Edit Langsung** dibuka dari palette (grup Solid: Offset Face, Hapus Face, Ganti Face, Sejajarkan, Pipa, Pattern Jalur, Web, Emboss Sketsa, Isi Batas, Solid Pembungkus) atau dari tombol **Offset Face** / **Hapus Face** di context bar saat face terpilih. Alat bisa diganti lewat combo di atas jendela; baris "Terpilih: … face, … tepi, … body" menunjukkan seleksi yang akan dipakai.

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| 2.1 | Kotak 20 mm → klik face atas → context bar **Offset Face** → Jarak 5 → Terapkan | Tinggi jadi 25 mm, face samping ikut memanjang; satu Undo mengembalikan | ☐ |
| 2.1 | Silinder → klik face lengkungnya → Offset Face +1 / −1 | Radius bertambah/berkurang 1 mm; jarak 0 atau face dari dua body berbeda → pesan, tidak crash | ☐ |
| 2.2 | Kotak dengan fillet tepi tegak → pilih keempat face fillet (Shift+klik) → **Hapus Face** | Sudut kembali tajam, volume = kotak semula; pilih semua face body → pesan, tidak crash | ☐ |
| 2.2 | Kotak dengan lubang tembus → pilih dinding lubang → Hapus Face | Lubang tertutup | ☐ |
| 2.3 | Klik face atas kotak → **Ganti Face** → bidang XY, offset 30 → Terapkan | Face atas pindah ke z = 30 | ☐ |
| 2.3 | Klik face atas kotak A, lalu Shift+klik face atas kotak B yang lebih tinggi → Ganti Face | Face atas A rata dengan face atas B; face kedua lengkung → pesan | ☐ |
| 2.4 | Pilih body kecil + tepi bersambung (Shift+klik) → **Pattern Jalur** → Jumlah 5, centang "Ikuti arah jalur" | 4 salinan baru merata sepanjang tepi, berputar mengikuti belokan; tanpa tepi → pesan | ☐ |
| 2.5 | Pilih tepi bersambung → **Pipa** → radius 3, centang Berongga, dinding 0,5 | Body tabung baru mengikuti tepi; dinding ≥ radius → pesan | ☐ |
| 2.6 | Sketsa di bidang face atas, gambar 2 garis → pilih body → **Web** → tebal 2, kedalaman 10 | Dua dinding setebal 2 mm tumbuh dari bidang sketsa dan menyatu dengan body; "Balik arah" membalik arah tumbuh | ☐ |
| 2.7 | Palette **Primitif** → jenis **Torus** → radius besar 20, radius tabung 3 → Buat | Torus muncul di posisi; radius tabung ≥ radius besar → pesan | ☐ |
| 2.8 | Sketsa lingkaran/persegi di face atas → **Emboss Sketsa** → kedalaman 1, centang Ukir | Region terukir 1 mm (tanpa centang: timbul 1 mm) | ☐ |
| 2.9 | Dua kotak terpisah → klik face bawah kotak A, Shift+klik face atas kotak B → **Sejajarkan** | Kotak A berpindah sehingga face bawahnya menempel face atas B; Celah 2 → menyisakan celah 2 mm; dua face silinder → sesumbu | ☐ |
| 2.10 | Dua kotak bertumpuk sebagian → pilih keduanya → **Isi Batas** → titik di dalam irisan | Body baru = bagian irisan; titik di ruang terbuka → pesan | ☐ |
| 2.10 | Pilih body → **Solid Pembungkus** → margin 1; centang Silinder | Body kotak / silinder baru membungkus body + 1 mm | ☐ |
| 2.11 | Klik face atas kotak → context bar **Split Face** → bidang YZ di tengah → terapkan | Hanya face atas yang terbelah dua; kedua potongan bisa diklik terpisah lalu di-Offset Face sendiri-sendiri | ☐ |
| semua | Agent/MCP: `get_schema {"example":"direct_edit"}` lalu `run_ops` contoh itu | Semua op berhasil; error input salah membawa `hint` dengan contoh JSON | ☐ |

### A21 — Fase 3: timeline, berkas, keselamatan data

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| 3.1 | Buat sketsa → extrude → fillet; di drawer Feature Tree seret judul fillet ke atas sketsa | Ditolak dengan pesan (fillet bergantung pada extrude); seret fitur independen → urutan berubah dan model diregenerasi | ☐ |
| 3.1 | Klik kanan fitur → Naikkan / Turunkan | Sama seperti seret; urutan tidak sah ditolak | ☐ |
| 3.2 | Klik dua kali nama fitur → ketik → Enter (Esc membatalkan) | Nama berubah; nama kosong ditolak | ☐ |
| 3.3 | Klik kanan → Masukkan ke folder baru; fitur lain → Pindahkan ke folder | Header folder dengan jumlah; panah menutup/membuka; ikon folder-coret membubarkan | ☐ |
| 3.4 | Klik kanan → Salin fitur → klik kanan fitur lain → Tempel sesudah fitur ini | Salinan "(salinan)" muncul di posisi itu dan diregenerasi | ☐ |
| 3.5 | Klik satu fitur | Induknya bergaris ungu, turunannya hijau; klik lagi melepas | ☐ |
| 3.6 | Klik kanan fitur tengah → Gulung timeline sampai sini | Fitur di bawahnya pudar, garis penanda oranye; geometri kembali ke kondisi itu; fitur baru masuk tepat di bawah penanda; ikon X pada penanda → semua aktif lagi | ☐ |
| 3.7 | Edit model lalu matikan paksa aplikasi (Force Quit) → buka lagi | Dialog "Desain yang Bisa Dipulihkan" berisi desain itu; Pulihkan membuka desain + nama berkas asal; Buang menghapusnya; palette "Desain yang Bisa Dipulihkan" membuka dialog kapan saja | ☐ |
| 3.8 | Simpan berkas dua kali | Ada `nama.ducad.bak` berisi versi sebelumnya; tidak ada berkas `.tmp` tersisa | ☐ |
| 3.9 | Palette **Ekspor 3MF** → buka di PrusaSlicer/Bambu/Orca | Model muncul dalam mm dengan warna material; satu objek per body | ☐ |
| 3.10 | Palette **Impor Mesh** → pilih .3mf / .obj / .glb | Body mesh muncul dengan ukuran benar (glTF meter → mm, Y-atas → Z-atas) | ☐ |
| 3.11 | Palette **Ekspor IGES** lalu **Impor IGES** berkas itu | Body B-rep kembali dengan volume sama | ☐ |
| 3.12 | Palette **Buka di Slicer** (dengan slicer terpasang / tanpa) | Slicer terbuka dengan model; tanpa slicer → pesan daftar slicer yang dicari | ☐ |
| 3.13 | Tombol bookmark di bawah ViewCube → putar kamera → Simpan tampilan saat ini → putar lagi → klik nama tampilan | Kamera kembali ke pose tersimpan; ganti nama/hapus bekerja; tampilan ikut tersimpan di berkas | ☐ |
| 3.14 | Palette **Laporan crash** (setelah ada command yang gagal internal) | Daftar laporan; "Salin perintah replay" menyalin `ducad-cli replay "...ducad"` yang bisa dijalankan | ☐ |
| 3.15 | Palette **Ekspor Tampilan Aktif ke DXF** → buka di LibreCAD/AutoCAD | Garis tampak di layer VISIBLE, tersembunyi di HIDDEN (putus-putus) sesuai sudut pandang layar | ☐ |

### A22 — Fase 4: perakitan & gerak

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| 4.1 | Dua part → klik face silinder poros (part tetap), Shift+klik lubang (part bergerak) → palette **Joint: Revolute** | Part bergerak sepusat; di Alat Perakitan muncul "Joint n: Revolute" dengan satu nilai; ubah nilai → part berputar | ☐ |
| 4.1 | Ulangi untuk Slider (nilai = geser), Cylindrical (2 nilai), Planar (3), Ball (3), Rigid (0) | Part bergerak sesuai DOF jenis joint | ☐ |
| 4.2 | Pin-slot: face dinding alur (A) + silinder pin (B) → nilai 1 menggeser sepanjang alur, nilai 2 memutar pin | Sesuai | ☐ |
| 4.2 | Cam: face cam (A) + face ujung follower (B) → putar cam (joint revolute lain / motion study) | Follower selalu menempel kontur cam, tidak menembus | ☐ |
| 4.3 | Centang "As-built" sebelum membuat joint | Part tidak berpindah saat joint dibuat; nilai 0 = pose sekarang | ☐ |
| 4.4 | Pilih datum sumbu di "Asal joint" lalu buat Revolute | Part berputar pada sumbu datum, bukan pada face | ☐ |
| 4.5 | Pilih part → ketik nama → Ganti nama; Aktifkan komponen → coba klik part lain | Nama berubah di drawer & body; part lain tidak bisa terpilih sampai dinonaktifkan | ☐ |
| 4.6 | Pilih beberapa part → Kelompokkan seleksi jadi komponen → Pindahkan seleksi ke … / (tingkat atas) | Sub-assembly baru berisi part itu di drawer Perakitan | ☐ |
| 4.7 | Simpan posisi → seret part → Kembalikan posisi | Part kembali tepat ke posisi tersimpan | ☐ |
| 4.8 | Dua part → Contact set baru dari seleksi → dorong satu menembus yang lain (nilai joint / seret) | Gerakan ditolak dengan pesan; part tetap di posisi sebelumnya | ☐ |
| 4.9 | Buat motion study (drawer Perakitan) → Studi Gerak → Analisis | Plot nilai (biru) & kecepatan (oranye), garis merah di frame bertabrakan, jejak oranye di viewport; Salin CSV | ☐ |
| 4.9 | Ekspor animasi → pilih folder | `frame_000.png`… + `motion.csv`; model kembali ke pose awal setelahnya | ☐ |
| 4.10 | Pilih part → Salin occurrence (offset 20,0,0) | Salinan bernama "Nama (2)" bergeser 20 mm, nomor part/material sama | ☐ |
| 4.11 | Panel industri → Konfigurasi → edit → Aturan `if len > 40 then t = 8` → Simpan; tombol Pratinjau konfigurasi lain | Aturan berlaku saat aktif; Pratinjau menampilkan perubahan volume tanpa mengganti konfigurasi aktif | ☐ |
| semua | Simpan berkas berisi joint/contact set/posisi tersimpan → buka ulang | Semua kembali (joint tetap bisa digerakkan) | ☐ |

### A23 — Fase 5: inspeksi & analisis

| # | Langkah | Hasil yang diharapkan | Status |
|---|---|---|---|
| 5.1 | Palette **Inspeksi** → pilih dua body → Clearance | Garis biru antara titik terdekat + jarak dalam mm; body bersinggungan → 0 | ☐ |
| 5.2 | Radius minimum pada part dengan fillet | Lingkaran merah + "R …" di fillet terkecil | ☐ |
| 5.2 | Centang Peta kurvatur, ubah "Merah pada" | Face datar biru, fillet kecil merah; nilai lebih kecil → lebih banyak merah | ☐ |
| 5.2 | Sketsa spline/busur → centang Sisir kurvatur | Garis sisir oranye tegak lurus kurva, panjang sebanding kurvatur | ☐ |
| 5.3 | Pilih body ber-fillet → Kontinuitas G0-G2 | Tepi tajam oranye "G0", sambungan fillet kuning "G1"/hijau "G2" | ☐ |
| 5.4 | Silinder/bola → Siluet | Garis ungu pada kontur siluet sesuai sudut pandang | ☐ |
| 5.4 | Klik face datar/silinder → Isocurve | Grid/garis biru muda di dalam batas face | ☐ |
| 5.4 | Arah alat 0,0,-1 → Aksesibilitas pada kotak berongga | Titik hijau di face yang terjangkau dari atas, merah di sisi/bawah/rongga; persentase luas | ☐ |
| 5.5 | Desain dengan op thread / lubang tap → Daftar ulir | Baris per ulir (body, M.., panjang, RH/LH, kosmetik) | ☐ |

---

## Bagian P — Uji tugas designer (workflow sasaran, Step 0)

Bagian ini berbeda dari A dan B. Yang diuji bukan "apakah tombol X bekerja",
melainkan **apakah designer bisa menyelesaikan pekerjaan nyata tanpa dibantu**.
Hasilnya menjadi baseline `docs/PLAN_DESIGNER_UX.md` §6 dan dibandingkan
sebelum/sesudah tiap step.

Padanan otomatisnya (alur yang sama lewat oplog) ada di
`crates/ducad-engine/tests/sample_project.rs` dengan fixture
`crates/ducad-engine/tests/fixtures/sample_bracket.ops.json`. Kalau tes itu merah,
perbaiki dulu sebelum sesi uji: masalahnya ada di engine, bukan di GUI.

### P.1 Persona, platform, dan aturan sesi

- **Persona:** designer produk yang membuat dan merevisi part mekanik sederhana.
  Paham CAD parametrik umum (SolidWorks, Fusion, Onshape, atau FreeCAD), belum
  pernah memakai DUCAD.
- **Platform utama:** desktop (macOS/Windows) dengan mouse dan keyboard.
- **Aturan pengamat:**
  1. Berikan lembar tugas P.2 saja. Jangan tunjukkan menu, tombol, atau langkah.
  2. Minta penguji berpikir keras (*think aloud*). Catat kalimat yang
     menunjukkan bingung, mis. "ini mode apa?", "kok hilang?".
  3. Jangan membantu. Kalau penguji buntu > 3 menit atau meminta bantuan, beri
     satu petunjuk lalu tandai tugas itu **dengan bantuan**.
  4. Mulai stopwatch saat penguji membaca tugas; hentikan saat ia menyatakan
     selesai atau menyerah.
  5. Data desain penguji tidak dikirim ke mana pun. Simpan hanya catatan P.3 dan,
     bila diizinkan, berkas `.ducad` hasilnya.

### P.2 Lembar tugas (diberikan ke penguji)

**Model sasaran — bracket L:**

```text
        ┌──┐  ← dinding: tinggi total 35, tebal 5
        │  │     2 lubang M5 tembus (clearance Ø5.5), di tengah tinggi dinding
        │  │
        │  ╰──────────┐  ← fillet sudut dalam R3
        └─────────────┘  ← alas: lebar 40, tebal 5
                            2 lubang M5 tembus di tengah lebar alas
   Panjang (tegak lurus gambar): 60. Lubang berjarak 15 dari tiap ujung.
```

| Tugas | Instruksi untuk penguji |
|---|---|
| T1 | Buat bracket di atas dari sketsa berdimensi. Ukurannya harus bisa diubah nanti. |
| T2 | Klien minta bracket lebih kuat: ubah tebal menjadi 8 dan tinggi dinding menjadi 50. Pastikan fillet dan semua lubang tetap benar. |
| T3 | Coba fillet sudut dalam R40. Kalau tidak bisa, kembalikan model ke keadaan yang benar dan pakai radius terbesar yang masuk akal. |
| T4 | Simpan, tutup DUCAD, buka lagi. Pastikan dimensinya masih bisa diubah. |
| T5 | Kirim model ke rekan yang memakai CAD lain (STEP) dan ke vendor 3D print (STL). |
| T6 | Buat gambar kerja sederhana: minimal tiga tampak, dimensi utama, dan catatan lubang. Ekspor ke PDF. |

### P.3 Lembar catatan pengamat (satu baris per tugas per penguji)

| Penguji | Tugas | Hasil (✅ mandiri / 🟡 dengan bantuan / ❌ gagal) | Durasi (mm:ss) | Undo / batal | Salah mode / salah pilih | Kehilangan state/data | Kutipan kebingungan | Isu |
|---|---|---|---|---|---|---|---|---|
| | T1 | | | | | | | |
| | T2 | | | | | | | |
| | T3 | | | | | | | |
| | T4 | | | | | | | |
| | T5 | | | | | | | |
| | T6 | | | | | | | |

Yang dicari pengamat per tugas:

- **T1:** apakah penguji menemukan cara memberi dimensi dan tahu kapan sketsa
  sudah terkunci penuh? Apakah ia paham bidang sketsa yang aktif?
- **T2:** apakah perubahan dimensi awal memperbarui fillet dan lubang, atau ada
  fitur yang hilang, pindah ke face lain, atau gagal tanpa penjelasan?
- **T3:** apakah pesan error menjelaskan sebab dan batas radius? Apakah model
  tetap utuh, dan apakah penguji bisa kembali tanpa undo berkali-kali?
- **T4:** apakah riwayat fitur, parameter, dan dimensi masih bisa diedit
  setelah dibuka ulang, bukan hanya geometri mati?
- **T5:** apakah satuan dan body yang diekspor jelas sebelum berkas dibuat?
  Buka STEP-nya di aplikasi lain bila ada.
- **T6:** apakah tampak, skala, dimensi, dan catatan lubang di PDF sesuai model?

### P.4 Ringkasan sesi

| Tanggal | Versi/commit | Platform | Jumlah penguji | Tugas mandiri (dari 6 × penguji) | Median waktu T1 | Crash/kehilangan data | Isu utama |
|---|---|---|---|---|---|---|---|
| | | | | | | | |

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
| B2.11 | Extrude teks: dinding huruf melengkung mulus, tanpa sisi bersegi. Putar model dan amati huruf bundar (C, O, S) — tidak boleh ada garis vertikal berjajar di sisinya | ☐ |
| B2.12 | Extrude huruf berongga (O, A, D, R): rongga tengahnya benar-benar berlubang, tidak terisi material | ☐ |
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
| B6.8 | Ketuk kolom angka (mis. nilai extrude) → keyboard lunak iPadOS muncul, angka masuk; ketuk di luar → keyboard turun | ☐ |
| B6.9 | Berkas ▸ Buka → picker Files.app muncul (iCloud Drive/penyedia lain); pilih `.ducad` → terbuka. Batal → tidak ada perubahan | ☐ |
| B6.10 | Ekspor STEP/STL/PDF → berkas tertulis di "Di iPad Ini ▸ DUCAD" dan share sheet muncul otomatis | ☐ |
| B6.11 | Buat sketsa, tekan Home (app ke latar), tutup paksa dari app switcher, buka lagi → status "dipulihkan dari autosave", sketsa utuh | ☐ |
| B6.12 | Simpan eksplisit (⌘S) lalu tutup paksa → tidak ada pesan pemulihan (autosave dihapus saat simpan) | ☐ |
| B6.13 | Pencil 2/Pro: ketuk ganda → tool aktif ↔ Pilih, ada haptik; hover di atas entitas → highlight sebelum ujung menyentuh | ☐ |
| B6.14 | Panel memakai isian datar (Liquid Glass GPU mati bawaan di tablet); nyalakan di ⚙ → kaca tampil, frame tetap lancar | ☐ |
| B6.15 | Buka 3 app berat lain sampai iPadOS memberi peringatan memori → status "cache dibebaskan", DUCAD tidak tertutup | ☐ |

### B7. Android tablet (kalau ada perangkatnya)

Build: `make android-apk` lalu `adb install android/app/build/outputs/apk/debug/app-debug.apk`.

| # | Yang diuji | Status |
|---|---|---|
| B7.1 | Aplikasi memulai sampai kanvas (logcat tag `ducad` tanpa panic) | ☐ |
| B7.2 | Jari: orbit/pan/zoom dua jari; stylus (S Pen/USI): menggambar dengan tekanan | ☐ |
| B7.3 | Ketuk kolom teks → keyboard lunak Android muncul (winit `show_soft_input`) | ☐ |
| B7.4 | Simpan → berkas di `Android/data/id.ducad.studio/files` terlihat lewat aplikasi Files | ☐ |
| B7.5 | Tekan Home lalu matikan dari Recent → buka lagi → dipulihkan dari autosave | ☐ |
| B7.6 | Putar layar → tidak ada restart aplikasi (configChanges) | ☐ |
| B7.7 | Extrude/boolean berjalan (kernel Rust murni) dan STEP terekspor | ☐ |

---

## Bagian C — Yang sengaja TIDAK ada

Supaya tidak dilaporkan sebagai bug:

- **OCR dimensi tulisan tangan (P12.4)** belum diimplementasikan.
- **`accept_proposal` untuk agent di sesi live** memang tidak tersedia: hanya
  pengguna yang boleh menerima proposal. Ini pagar keselamatan, bukan kelalaian.
- **Agent Bridge dan CLI agent di iPadOS/Android** tidak ada (soket Unix dan
  proses anak, desktop saja). Memori di tablet memakai jalur tertaut
  (`--features memory`), bukan jembatan.
- **Picker berkas di Android** belum memakai Storage Access Framework: Buka
  mengambil berkas terbaru di folder Dokumen aplikasi (lihat `docs/TABLET.md`).
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
