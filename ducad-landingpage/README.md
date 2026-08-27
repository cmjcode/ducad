# DuCAD Landing Page

Landing page statis untuk **DuCAD** (Design Universe CAD) — CAD 2D/3D modern, parametrik, dan
berkinerja tinggi berbasis Rust. Dibangun tanpa framework atau build step: HTML/CSS/JS murni,
mudah di-deploy ke host statis mana pun.

## Struktur

```
ducad-landingpage/
├── index.html        # Seluruh markup halaman (satu halaman, section-based)
├── css/
│   └── styles.css    # Tema gelap + gradient brand DuCAD, layout responsif
├── js/
│   ├── i18n.js        # Dictionary Indonesia + logic toggle bahasa (default: English)
│   └── main.js        # Toggle nav mobile, scroll-reveal, salin kode, tombol ke-atas
└── images/           # Logo (SVG) & screenshot aplikasi (PNG) — sudah tersedia
```

## Bahasa (i18n)

Halaman ini berbahasa **Inggris secara default**, dengan toggle **EN / ID** di navbar (tersimpan
di `localStorage` sehingga pilihan bahasa diingat pada kunjungan berikutnya). HTML ditulis dalam
Bahasa Inggris sebagai sumber kebenaran; `js/i18n.js` menangkap teks asli tersebut lalu menukarnya
dengan kamus Bahasa Indonesia saat tombol "ID" ditekan. Untuk menambah bahasa baru, duplikasi pola
kamus di `js/i18n.js` dan tambahkan satu tombol lagi pada `.lang-switch` di `index.html`.

## Menjalankan Secara Lokal

Tidak ada dependensi maupun build step. Cukup jalankan server statis sederhana agar path
relatif (gambar, font, dsb.) dimuat dengan benar:

```bash
cd ducad-landingpage
python3 -m http.server 8080
# buka http://localhost:8080
```

Atau gunakan ekstensi "Live Server" di editor mana pun.

## Deploy

Karena murni statis, halaman ini bisa langsung di-deploy ke:
- **GitHub Pages** — push folder ini ke branch `gh-pages` atau aktifkan Pages dari root repo
- **Netlify / Vercel** — drag-and-drop folder atau hubungkan repo Git, tanpa build command
- Host statis lain (S3 + CloudFront, Cloudflare Pages, dll.)

## Kustomisasi

- **Palet warna & tipografi**: variabel CSS di `css/styles.css` (`:root`), mengikuti gradient
  brand asli DuCAD (`#2a4ced → #2065ed → #00b7ed`).
- **Konten fitur**: seluruh copy diambil dari `DUCAD/README.md` dan
  `DUCAD/docs/ANALISIS_KOMPARATIF_CAD.md` — perbarui `index.html` bila fitur produk berubah.
- **Tautan GitHub**: saat ini menunjuk ke `https://github.com/cmjcode/ducad` — sesuaikan bila
  repositori publik menggunakan URL berbeda.
- **Screenshot**: `images/image1.png` sampai `images/image5.png` — tangkapan layar UI aplikasi (Sketsa 2D, Solid Modeling & Hole Wizard, Direct Modeling/Shell, Parametric History DAG, Gambar Kerja 2D ISO).
- **Logo**: `images/logo.svg` untuk navbar header dan footer, serta `images/logocmj.svg` untuk kredit developer.
