# Packaging CADRAW (Fase 7)

Status: putaran pertama — cukup untuk hasilkan `.app` macOS yang bisa
dijalankan lokal. Code signing, notarization, installer Windows, dan
AppImage Linux SENGAJA belum digarap (lihat "Di luar lingkup" di bawah).

## Build rilis

```bash
cargo build --release -p cadraw-app
# Binary: target/release/cadraw
```

## macOS: bundle `.app` lewat `cargo-bundle`

Metadata bundle sudah ada di `crates/cadraw-app/Cargo.toml`
(`[package.metadata.bundle]`) — `cargo-bundle` membacanya otomatis, tidak
perlu config terpisah.

```bash
cargo install cargo-bundle   # sekali saja
cargo bundle --release -p cadraw-app
# Hasil: target/release/bundle/osx/CADRAW.app
```

Buka lewat `open target/release/bundle/osx/CADRAW.app` atau drag ke
Applications. Ini `.app` valid (bisa dijalankan lewat Finder/Spotlight)
TAPI belum ditandatangani — macOS Gatekeeper akan memblokir peluncuran dari
mesin LAIN (bukan yang dipakai build) dengan pesan "tidak bisa dibuka
karena dari pengembang tak dikenal" sampai code signing beres (lihat di
bawah).

### Ikon

`icon = []` di manifest — belum ada ikon `.icns` dibuat (butuh aset visual
yang di luar lingkup kerja agent ini). `.app` akan pakai ikon generik
sampai file `.icns` disediakan dan didaftarkan lewat `icon = ["path/ke
/icon.icns"]`.

## Windows / Linux

`cargo build --release` menghasilkan `.exe` (Windows) / binary ELF (Linux)
yang jalan langsung tanpa bundling — CADRAW tidak punya dependensi native
selain yang sudah di-static-link OCCT/wgpu saat build. Installer
(`.msi`/`.exe` installer Windows lewat `cargo-wix`, `.AppImage`/`.deb`
Linux) belum dibuat — di luar lingkup putaran ini, lihat di bawah.

## Di luar lingkup (didokumentasikan, bukan lupa)

Semua butuh sertifikat berbayar dan/atau GUI interaktif yang tidak
tersedia di sandbox agent — sama alasan dengan TestFlight iOS di Fase 6:

- **Code signing + notarization macOS** — butuh Apple Developer ID
  (berbayar) + `xcrun notarytool`, yang butuh kredensial Apple ID
  interaktif.
- **Installer Windows** (`cargo-wix` → `.msi`) — belum dicoba; secara
  prinsip tidak butuh kredensial, jadi lebih mungkin dikerjakan agent di
  putaran berikutnya kalau dibutuhkan, tapi belum diverifikasi build-nya
  bersih di platform ini (agent jalan di macOS).
- **AppImage/`.deb` Linux** — sama alasan dengan Windows, belum dicoba di
  putaran ini.
- **Ikon aplikasi** (`.icns`/`.ico`/PNG) — butuh aset visual, di luar
  lingkup kerja kode.
- **iOS packaging** (`.ipa`, TestFlight) — sudah didokumentasikan
  terpisah di "Status Fase 6" `docs/PLAN.md`, blocker OCCT/iOS belum
  selesai jadi ini belum relevan sampai itu beres.
