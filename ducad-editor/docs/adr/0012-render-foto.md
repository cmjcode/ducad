# ADR 0012 — Mode Render foto-realistis (crate `ducad-photoreal`)

**Status**: Diterima · **Tanggal**: 2026-11-11 · **Cakupan**: `ducad-photoreal`,
`ducad-engine::render`, `render_view` (MCP/CLI), drawer Studio Lighting `ducad-app`

## Konteks

Viewport DUCAD memakai shading perkiraan (Blinn-Phong, tiga lampu arah,
"pantulan" berupa gradien dua warna, bayangan berupa noda di lantai). Itu
tepat untuk memodelkan, tetapi tidak menghasilkan gambar presentasi: tidak ada
pantulan antarbenda, kaca tidak membiaskan, bayangan tidak mengikuti bentuk.
`render_view` dan Export PNG pun hanya garis HLR
(`PLAN_PARITAS_SOLIDCRAFT.md` 0.15).

TextureCraft (`github.com/storytold/texturecraft`, MIT OR Apache-2.0) memuat
path tracer progresif di CPU dan GPU (compute shader), BVH, lingkungan studio
prosedural, dan denoiser — Rust murni, `unsafe` dilarang, wgpu 30 seperti kita.

## Keputusan

1. **Viewport interaktif tidak diganti.** Ia membawa seleksi, section, zebra,
   gizmo, dan overlay yang tidak dimiliki TextureCraft. Path tracer hanya
   dipakai untuk gambar akhir: *Mode Render*.
2. **Crate terpisah `ducad-photoreal`.** Dependensi TextureCraft diambil lewat
   git dan **dipaku ke satu rev** di `Cargo.toml` workspace: proyeknya
   pra-rilis dan API-nya bergerak. Menaikkan rev = PR tersendiri.
3. **Engine tetap bebas wgpu.** `ducad-engine` hanya mendefinisikan
   `PhotoRequest` dan kait `install_photo_renderer`; binari (CLI, server MCP,
   aplikasi) memanggil `ducad_photoreal::install()`. Tes
   `engine_has_no_gui_dependency` tetap berlaku.
4. **Material seragam → texture set konstan.** Renderer TextureCraft
   menggambar texture set di ruang UV. Tiap material DUCAD yang unik menjadi
   satu set 64×64 bernilai konstan, jadi body tidak butuh UV. Preset kaca
   dipetakan ke transmisi 0,96 tanpa clearcoat (alpha 0,38 di viewport adalah
   trik rasterisasi, bukan sifat fisik).
5. **Lampu mengikuti kamera** (`key_light_offset_deg`, bawaan 55° di kiri),
   seperti meja putar studio. Softbox utama TextureCraft kebetulan tepat di
   belakang kamera isometrik DUCAD; cahaya frontal itu menghapus bayangan.
6. **Lantai + "cove tak hingga".** Lantai segi empat dengan opacity memudar
   radial; belahan bawah dan pita horizon lingkungan disamakan dengan
   radiansi lantai (`albedo · E / π`). Tidak ada garis horizon dari sudut mana
   pun, tanpa geometri dinding yang bergantung pada kamera.
7. **Dua jalur hitung, satu gambar.** GPU (`GpuPhoto`) bila perangkat punya
   compute shader dan batas binding cukup; selain itu CPU (`CpuPhoto`,
   setengah resolusi). Aplikasi memakai perangkat wgpu milik egui — sama di
   Mac dan iPad — dan menampilkan tekstur keluaran langsung. Tekstur itu
   `Rgba8Unorm` berisi nilai ter-encode sRGB, persis yang diharapkan
   egui-wgpu (tampilan `Rgba8UnormSrgb` men-decode dua kali → terlalu gelap).
8. **Tidak memblokir antarmuka.** Scene dibangun di thread latar; resolusi
   pratinjau saat kamera bergerak; akumulasi ditunda selama penunjuk aktif di
   antarmuka; render berhenti sendiri di jumlah sampel target.

## Konsekuensi

- `render_view` mendapat `style: "photo"`, `samples`, `environment`;
  `ducad-cli render` mendapat `--photo`, `--samples`, `--env`. Gaya bawaan
  tetap `lines` — itu yang tepat untuk memeriksa bentuk.
- Anggaran `tools/list` MCP dinaikkan 40 → 42 KiB (sebelumnya tinggal ±100
  byte dari batas).
- Dua versi `glam` (0.30 milik TextureCraft, 0.33 milik kita) hidup
  berdampingan; API publik `ducad-photoreal` memakai larik polos.
- `Cargo.lock` bertambah 10 paket.

## Batas yang diketahui

- **Material masih polos**: tanpa tekstur aus/kotor benda tampak seperti
  produk baru di studio. Tekstur/decal butuh UV (`auto_unwrap` TextureCraft
  atau UV parametrik truck yang kini dibuang).
- **iPad belum diuji di perangkat**: yang terbukti hanya kompilasi target iOS
  dan jalur GPU di Mac (±34 dtk untuk 384 sampel pada 1600×872). Lihat
  `CEKLIS_UJI_GUI.md` B6.16–B6.17.
- Panel Liquid Glass memburamkan scene raster, bukan gambar Mode Render.
- Bidang potong section view tidak ikut dirender (explode ikut).
- Simpan PNG dari aplikasi memakai ukuran viewport (maks 1600 px desktop,
  1024 px tablet); render resolusi tinggi lewat CLI.
