# Packaging DUCAD (Fase 7)

Status: putaran pertama — cukup untuk hasilkan `.app` macOS yang bisa
dijalankan lokal. Code signing, notarization, installer Windows, dan
AppImage Linux SENGAJA belum digarap (lihat "Di luar lingkup" di bawah).

## Build rilis

```bash
cargo build --release -p ducad-app
# Binary: target/release/ducad
```

## macOS: bundle `.app` lewat `cargo-bundle`

Metadata bundle sudah ada di `crates/ducad-app/Cargo.toml`
(`[package.metadata.bundle]`) — `cargo-bundle` membacanya otomatis, tidak
perlu config terpisah.

```bash
cargo install cargo-bundle   # sekali saja
cargo bundle --release -p ducad-app
# Hasil: target/release/bundle/osx/DUCAD.app
```

Buka lewat `open target/release/bundle/osx/DUCAD.app` atau drag ke
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
yang jalan langsung tanpa bundling — DUCAD tidak punya dependensi native
selain yang sudah di-static-link OCCT/wgpu saat build. Installer
(`.msi`/`.exe` installer Windows lewat `cargo-wix`, `.AppImage`/`.deb`
Linux) belum dibuat — di luar lingkup putaran ini, lihat di bawah.

## Linux: paket AUR `ducad`

Metadata paket ada di `aur/ducad/` (root repo): `PKGBUILD`, `ducad.desktop`,
dan `.SRCINFO` hasil generate. PKGBUILD membangun dari tag `v<versi>` di
GitHub lewat sumber `git+` — bukan tarball — karena binding OCCT ada di
submodule `ducad-editor/vendors/opencascade-rs` yang tidak ikut tarball.
OCCT dipakai dari paket sistem Arch `opencascade` (>= 7.8), dilink dinamis.
Terpasang: `ducad`, `ducad-cli`, `ducad-mcp`, entri desktop, ikon, lisensi.

Alur rilis:

```bash
# 1. bump versi (VERSION + Cargo.toml harus sama), commit, lalu tag + push
git tag v0.4.0 && git push origin v0.4.0
# 2. perbarui PKGBUILD/.SRCINFO dan push ke AUR
scripts/update-aur.sh            # atau: make publish-aur
scripts/update-aur.sh --no-push  # hanya commit lokal, untuk uji
```

Skrip membaca versi dari `VERSION`, menolak bila tag belum ada di `origin`,
menghitung ulang sha256 berkas sumber lokal, dan membuat `.SRCINFO` lewat
`makepkg` → container `archlinux:base-devel` → emitter bash bawaan (yang
terakhir dipakai di macOS tanpa docker). Repo AUR di-clone otomatis ke
`../ducad-aur` (ubah lewat `TARGET_REPO`); push memakai
`ssh://aur@aur.archlinux.org/ducad.git`, jadi kunci SSH harus terdaftar di
akun AUR. Belum ada varian `ducad-bin` karena rilis GitHub belum menyertakan
artefak Linux (hanya `.dmg`).

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
- **iOS packaging** (`.ipa`, TestFlight) — `./build_ipad.sh ipa|publish`
  (pre-build OCCT untuk iOS otomatis). Lihat `docs/TABLET.md`.
- **Android** — `make android-apk` menghasilkan APK debug (lihat
  `docs/TABLET.md`); signing rilis (keystore) di luar sandbox agent, sama
  alasannya dengan macOS.
