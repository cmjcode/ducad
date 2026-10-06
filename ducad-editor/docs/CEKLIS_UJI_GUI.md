# Ceklis Uji GUI DuCAD

Diperbarui: 2026-09-22. Untuk penguji manual (QA) dan pengembang sebelum rilis.

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
- [ ] "Mulai tutorial" membuka pelajaran 1 (mode 2D/3D); kartu ada di kanan atas dan bisa digeser.
- [ ] Tombol mode disorot cincin oranye berdenyut dan tetap bisa diklik; pelajaran lulus setelah
  masuk 3D lalu kembali ke Sketsa.
- [ ] Pelajaran 3 (tool Pilih, setelah persegi) lulus setelah satu garis persegi diklik.
- [ ] "Lanjut" nonaktif sampai aksi dicoba; setelah lulus muncul tanda centang hijau.
- [ ] Persegi, fillet sudut persegi, lingkaran, Extrude, navigasi, tarik sisi, palet, simpan, dan
  Chat AI (langkah terakhir, berisi penyiapan agent) masing-masing lulus hanya setelah aksinya dilakukan.
- [ ] Animasi kamera otomatis setelah extrude TIDAK meluluskan pelajaran navigasi.
- [ ] Kartu tidak menutupi bilah konteks bawah, kartu panduan tool, maupun sidebar Chat AI.
- [ ] "Lewati langkah ini" maju satu langkah; tombol tutup menutup tutorial.
- [ ] Setelah selesai atau dilewati, membuka ulang aplikasi tidak menampilkan tutorial lagi.
- [ ] Tombol bantuan (ikon tanda tanya, kiri tombol bagikan) dan palet perintah → "Tutorial: mulai dari awal" membuka kartu sambutan lagi.
- [ ] iPad / jendela sempit: kartu tetap di dalam layar dan tombolnya nyaman disentuh.

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
