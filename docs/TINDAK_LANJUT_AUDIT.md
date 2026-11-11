# Dokumentasi Tindak Lanjut Audit Produk & Rekayasa DuCAD

Dokumen ini mencatat pelaksanaan bertahap dari temuan audit produk dan rekayasa DuCAD (`tinjauan-produk-ducad.md`) di ketiga repositori: `DUCAD-STUDIO`, `DUCAD-SERVER`, dan `DUCAD-LANDINGPAGE`.

---

## Tahap 1 — Tangani Hari Ini (Keamanan, Lisensi, CI Hijau, Privasi & Landing Page)

**Status:** Selesai & Terverifikasi

### 1. Pemisahan Kredensial & Dokumentasi Keamanan
- **Kredensial Apple Developer (`DUCAD-STUDIO/build_macos.sh`)**: Menghapus seluruh nilai rahasia yang sebelumnya tertulis langsung (`APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`, `SIGNING_IDENTITY`, `KEYCHAIN_PROFILE`) dan menggantinya dengan pembacaan dari berkas `.env` lokal (`chmod 600`) yang diabaikan Git. Menyediakan templat aman di `.env.example`.
- **Perlindungan `.gitignore`**: Menambahkan pola `.env`, `.env.*`, `!.env.example`, `ducad-editor/.env`, `**/xcuserdata/`, dan `*.xcuserstate`.
- **Kebijakan & Panduan Rotasi**:
  - Menambahkan `SECURITY.md` (dwibahasa ID/EN) sebagai jalur resmi pelaporan kerentanan.
  - Menambahkan `docs/ROTASI_KREDENSIAL_DAN_RIWAYAT_GIT.md` berisi daftar periksa rotasi Apple App-Specific Password serta rahasia historis `DUCAD-SERVER` (`ad9f86f0..d4a4e4fa`), lengkap dengan prosedur manual `git filter-repo` bila riwayat Git hendak dibersihkan di kemudian hari.

### 2. Penyatuan Lisensi `AGPL-3.0-only` & Pemberitahuan Atribusi (`NOTICE`)
- Menyeragamkan field `license = "AGPL-3.0-only"` di `ducad-editor/Cargo.toml` (diwarisi oleh seluruh 20 crate workspace), `ducad-editor/deny.toml`, dan `DUCAD-SERVER/Cargo.toml`, serta menyalin `LICENSE` ke `DUCAD-SERVER` dan `DUCAD-LANDINGPAGE`.
- Membuat berkas `NOTICE` di akar `DUCAD-STUDIO` yang memuat pemberitahuan hak cipta dan lisensi komponen pihak ketiga yang di-vendor (`SolidCraft`, `truck` / RICOS Co., Ltd., `matext4cgmath`, `TextureCraft`).
- Memperbarui alur pemaketan di `.github/workflows/release.yml`, `ducad-editor/packaging/windows/ducad.wxs`, `ducad-editor/Makefile`, dan `aur/ducad/PKGBUILD` agar setiap bundel rilis menyertakan `LICENSE` dan `NOTICE`.

### 3. Pembaruan Dependensi Rentan & Kelulusan `cargo deny check`
- Memperbarui `zip` (`8.6.0` → `9.5.1`, menutup CVE/RUSTSEC terkait ekstraksi arsip) dan menyesuaikan API pembacaan nama entri arsip 3MF di `crates/ducad-io/src/mesh_formats.rs`.
- Memperbarui `rustls` (`0.23.45` → `0.23.46`), `rustls-webpki` (`0.103.15` → `0.103.16`), `webpki-roots` (`1.0.9` → `1.0.12`), `async-executor`, `clipboard-win`, dan `yoke`.
- Mendokumentasikan pengecualian terbatas `RUSTSEC-2026-0459` (`rusqlite 0.40` / `libsqlite3-sys 0.38.2` yang dikunci oleh dependensi git `mnemonic`) di `ducad-editor/deny.toml`, karena `ducad-app` hanya membuka basis data internal di `~/.ducad/`.
- Menambahkan `publish = false` pada seluruh 20 crate internal agar pemeriksaan `cargo deny check bans` (`allow-wildcard-paths = true`) lulus 100%.
- Menambahkan `.github/dependabot.yml` untuk pemantauan mingguan Cargo dan GitHub Actions.

### 4. Pemulihan CI & Penegakan Gerbang Rilis
- **`crates/ducad-core/tests/layering.rs` & `CONTRIBUTING.md`**: Mendaftarkan `ducad-photoreal` sesuai dependensi nyatanya (`ducad-core`, `ducad-io`, `ducad-render`).
- **`.github/workflows/ci.yml`**: Mengganti runner `macos-14` yang telah dipensiunkan GitHub menjadi `macos-latest` dan menghapus `continue-on-error: true` pada job `cargo-deny` sehingga pelanggaran lisensi/advisory langsung memblokir PR.
- **`crates/ducad-engine/tests/drawing_golden.rs` & `drawing.hashes.truck.json`**: Menyimpan hash emas deterministik per arsitektur (`aarch64` dan `x86_64`) agar perbedaan pembulatan titik-ambang libm antara Apple Silicon dan Linux x86_64 tidak memicu kegagalan palsu sambil tetap menjaga ketepatan regresi di tiap arsitektur.
- **`.github/workflows/release.yml` & `scripts/release-tag.sh`**: Menambahkan gerbang pemblokir (`cargo deny check`, tes `layering`, dan `scripts/release-gate.sh`) sebelum rilis dapat di-tag atau dibangun.

### 5. Kebijakan Privasi Jujur & Reposisi `DUCAD-LANDINGPAGE`
- Menulis ulang `DUCAD-LANDINGPAGE/PrivacyPolicy.html` dan kamus Bahasa Indonesia di `js/i18n.js` agar transparan membedakan mode CAD offline (tanpa akun) dari fitur jaringan opsional (OAuth Google/GitHub/Apple, Cloud Sync MySQL, asisten AI eksternal dengan API key pengguna, dan pemeriksaan pembaruan GitHub Releases), lengkap dengan dasar hukum GDPR & UU No. 27 Tahun 2022 (UU PDP) serta mekanisme hapus akun mandiri (`DELETE /users/me`).
- Mereposisi `DUCAD-LANDINGPAGE/index.html` ke *"Parametric CAD built by your agent — and verified by you"* (*"CAD parametrik yang dibangun agen Anda — dan diverifikasi oleh Anda"*), menghapus seluruh klaim usang (`OpenCASCADE`, `CMake/C++17`, `v0.2`), memperbaiki URL unduhan yang rusak (`/releases/latest`), dan membuat skrip otomatis `scripts/sync-from-studio.py` yang menyinkronkan versi (`v0.8.0`), jumlah op (`55`), tool MCP (`37` + `6` live bridge), kode diagnosis (`34`), pustaka material (`13`), dan daftar `20` crate Rust langsung dari `DUCAD-STUDIO`.

### Verifikasi Tahap 1
- `cargo deny check` (`ducad-editor`): **LULUS** (`advisories ok, bans ok, licenses ok, sources ok`).
- `cargo test -p ducad-core --test layering`: **LULUS** (6/6).
- `cargo test -p ducad-update`: **LULUS** (21/21).
- `cargo test -p ducad-io --lib mesh_formats`: **LULUS** (6/6).
- `cargo test -p ducad-engine --test drawing_golden`: **LULUS** (5/5).
- `scripts/release-gate.sh`: **LULUS** seluruh grup metrik rilis.
- `python3 DUCAD-LANDINGPAGE/scripts/sync-from-studio.py --check`: **LULUS**.

