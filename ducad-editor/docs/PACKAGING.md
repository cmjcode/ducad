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
selain yang sudah di-static-link saat build (kernel Rust murni, wgpu).
`.AppImage`/`.deb` Linux belum dibuat — lihat di bawah.

## Windows: installer `.msi`

`release.yml` membangun `ducad-<versi>-windows-x86_64.msi` (+ `.sha256`)
dari `packaging/windows/ducad.wxs` dengan WiX Toolset v5, memvalidasinya
(ICE), lalu menguji pasang → jalankan `ducad-cli` → copot di runner.

- Instalasi **per-user** ke `%LOCALAPPDATA%\Programs\DUCAD`, tanpa hak admin.
  Disengaja: updater bawaan menimpa folder executable dari paket `.zip`, dan
  itu tidak mungkin di Program Files.
- Membuat shortcut Start Menu + Desktop, menambah folder instalasi ke `PATH`
  user (untuk `ducad-cli`/`ducad-mcp`), dan tercantum di "Apps & features".
- MSI versi baru menggantikan versi lama (MajorUpgrade, `UpgradeCode` tetap).
- Ikon: `assets/icon.ico` (dibuat dari `assets/icon.png`).
- Belum ditandatangani: Windows SmartScreen akan menampilkan peringatan
  "unknown publisher" sampai ada sertifikat code signing.

## Linux: paket AUR `ducad`

Metadata paket ada di `aur/ducad/` (root repo): `PKGBUILD`, `ducad.desktop`,
dan `.SRCINFO` hasil generate. PKGBUILD membangun dari tag `v<versi>` di
GitHub lewat sumber `git+`. Sejak 0.6.0 kernel geometri (SolidCraft/truck)
adalah Rust murni yang di-vendor di repo — tanpa submodule, tanpa pustaka
CAD sistem, tanpa cmake.
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
- **Code signing Windows** (`.exe`/`.msi`) — butuh sertifikat code signing
  berbayar; installer `.msi` sendiri sudah dibuat CI (lihat di atas).
- **AppImage/`.deb` Linux** — sama alasan dengan Windows, belum dicoba di
  putaran ini.
- **Ikon aplikasi** (`.icns`/`.ico`/PNG) — butuh aset visual, di luar
  lingkup kerja kode.
- **iOS packaging** (`.ipa`, TestFlight) — `./build_ipad.sh ipa|publish`
  (tanpa pre-build C++). Lihat `docs/TABLET.md`.
- **Android** — `make android-apk` menghasilkan APK debug (lihat
  `docs/TABLET.md`); signing rilis (keystore) di luar sandbox agent, sama
  alasannya dengan macOS.
