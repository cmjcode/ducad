# ADR 0006 — Tema Liquid Glass (crate `ducad-glass`)

**Status**: Diterima · **Tanggal**: 2026-10-06 · **Cakupan**: `ducad-glass`,
`ducad-ui::theme`, jalur render viewport `ducad-app`

## Konteks

Panel mengambang DUCAD dulu transparan ~57%. Grid dan geometri viewport
tembus jelas di balik teks sehingga mengganggu, lalu panel dipekatkan menjadi
~84% (`BG_PANEL_DARK`). Hasilnya terbaca, tetapi kesan "kaca" hilang.

Liquid Glass (Apple, WWDC 2025) menyelesaikan masalah yang sama dengan cara
lain: permukaan tetap transparan, tetapi latar di baliknya **dibelokkan di
tepi (lensa), diburamkan, dan ditint**, dengan sorotan spekular di rim.
Detail latar yang mengganggu hilang; warna dan gerak latar tetap terasa.

## Keputusan

1. **Crate terpisah `ducad-glass`**, di bawah `ducad-ui` dan `ducad-app`.
   Hanya bergantung pada egui/egui_wgpu (tes `glass_has_no_app_or_ui_dependency`).
   Isi: `GlassMaterial` + preset (`Panel`, `Pill`, `Toolbar`, `Popup`),
   geometri CPU (`sdf`), `GlassBackdrop` (GPU), dan widget `GlassFrame`.
2. **Scene dirender ke tekstur offscreen.** Callback `egui_wgpu` tidak bisa
   membaca surface yang sedang digambar. `ViewportCallback::prepare` merender
   `SceneRenderer` ke tekstur milik `GlassBackdrop`, `paint` mem-blit-nya ke
   viewport, dan rantai blur dual-Kawase (1/2 … 1/16 resolusi) dibangun sekali
   per frame — hanya bila ada panel kaca di frame itu.
3. **Satu panel = satu callback** yang menyampel tekstur tajam (rim) dan buram
   (tengah). Parameter panel masuk satu uniform buffer ber-offset dinamis.
4. **`GlassFrame` meniru API `egui::Frame`.** `theme::glass_frame()`,
   `pill_frame()`, dan `toolbar_frame()` mengembalikan `GlassFrame`, jadi
   pemanggil `….show(ui, …)` tidak berubah. Tempat yang menuntut `egui::Frame`
   (`Window::frame`, sidebar chat) memakai `.flat()`.
5. **Jalur datar selalu ada.** `GlassRuntime` di `egui::Context` bawaan mati,
   sehingga tes headless dan app tanpa GPU memakai isian datar lama. Aplikasi
   mematikan kaca saat "Kurangi transparansi" aktif dan saat lembar gambar 2D
   menutupi viewport. Dengan kaca mati, viewport kembali digambar langsung ke
   surface (tanpa biaya offscreen).
6. **Keterbacaan dijamin material, bukan kepekatan.** Sebelum ditint, latar
   dibatasi kecerahannya (`luma_limit`; mode terang: dibalik). Tes
   `text_stays_legible_over_any_backdrop` menjaga kontras teks utama ≥ 4.5:1
   dan sekunder ≥ 3:1 untuk latar apa pun, lewat `GlassMaterial::composite`
   yang menirukan shader. Tes GPU `gpu_pipeline` memastikan shader dan tiruan
   CPU itu memang sepakat.

## Konsekuensi

- Backdrop kaca = scene 3D saja. Kaca di atas kaca lain, atau di atas panel
  egui opak, tidak "melihat" lapisan egui di bawahnya. Popup/menu egui
  (`window_fill`) tetap pekat seperti sebelumnya.
- Panel yang keluar dari rect viewport menyampel tepi tekstur (clamp).
- Biaya per frame: satu pass scene offscreen + 7 pass blur beresolusi rendah
  + 9 sampel tekstur per piksel panel.
- Rumus SDF/lensa/komposit ada di dua tempat (`sdf.rs`/`material.rs` dan
  `glass.wgsl`); mengubah satu wajib mengubah yang lain — tes GPU akan gagal
  bila keduanya menyimpang.

## Alternatif yang ditolak

- **Jendela transparan + blur sistem (NSVisualEffectView)**: hanya memburamkan
  apa yang ada di belakang jendela, bukan viewport di dalam jendela; tidak
  ada di iPadOS lewat eframe.
- **Menaikkan lagi kepekatan panel**: terbaca, tetapi bukan kaca.
- **Menyalin surface tiap panel**: tidak mungkin di tengah render pass egui.

## Referensi

- Apple, "Meet Liquid Glass" (WWDC25, sesi 219).
- Pola shader umum: SDF persegi-bersudut-bulat → normal dari gradien SDF →
  pergeseran sampel latar di pita rim → dispersi → tint → rim spekular.
