# DuCAD Landing Page

Landing page statis untuk **DuCAD** (Design Universe CAD) — CAD 2D/3D parametrik berbasis Rust
dengan simulasi dan agent AI. Dibangun tanpa framework atau build step: HTML/CSS/JS murni,
mudah di-deploy ke host statis mana pun.

## Struktur

```
ducad-landingpage/
├── index.html          # Seluruh markup halaman (satu halaman, section-based)
├── PrivacyPolicy.html  # Halaman Kebijakan Privasi (syarat Apple App Store Connect)
├── css/
│   └── styles.css      # Tema gelap + gradient brand DuCAD, layout responsif
├── js/
│   ├── i18n.js          # Dictionary Indonesia + logic toggle bahasa (default: English)
│   └── main.js          # Toggle nav mobile, scroll-reveal, salin kode, tombol ke-atas
└── images/             # Logo (SVG) & screenshot aplikasi (WebP, `shot-*.webp`)
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
- **Konten fitur**: copy diambil dari `DUCAD/README.md` — perbarui `index.html` dan kamus di
  `js/i18n.js` bila fitur produk berubah. Angka di hero (massa, tegangan, faktor keamanan)
  berasal dari model `examples/pillow_block.ops.json`.
- **Tautan GitHub**: saat ini menunjuk ke `https://github.com/cmjcode/ducad` — sesuaikan bila
  repositori publik menggunakan URL berbeda.
- **Screenshot**: `images/shot-assembly.webp`, `shot-simulation.webp`, `shot-ai-chat.webp`,
  `shot-drawing.webp` — tangkapan jendela aplikasi 1280×796 dari model
  `examples/pillow_block.ops.json`, dikonversi dengan `cwebp -q 70 -m 6 -sharp_yuv`.
- **Ikon**: sprite SVG inline di awal `<body>` (`<use href="#i-…">`); jangan memakai emoji/glyph.
- **Font**: font sistem (tanpa permintaan font eksternal).
- **Logo**: `images/logo.svg` untuk navbar header dan footer, serta `images/logocmj.svg` untuk kredit developer.
