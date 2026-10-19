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
