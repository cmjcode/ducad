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

---

## Tahap 2 — Penataan Modul, Pembekuan Permukaan, serta Keamanan Akun & Pembaruan

**Status:** Selesai & Terverifikasi

### 1. Pembekuan Target Sekunder & Pelabelan Permukaan Eksperimental
- **Target Rilis Aktif**: Mengunci target rilis aktif pada **Linux (`x86_64`)**, **Windows (`x86_64`)**, dan **macOS (`arm64`)**. Menandai `crates/ducad-android` dan paket `aur/ducad` sebagai *dihentikan sementara / tidak dirilis aktif* di `README.md`, `ducad-editor/docs/PACKAGING.md`, `ducad-editor/docs/STATUS_KEMAMPUAN.md`, dan `ducad-editor/docs/TABLET.md`.
- **Label Eksperimental**: Memberi label eksplisit `(Experimental)` / `(Eksperimental)` pada permukaan UI dan dokumentasi untuk path-traced photo rendering (`ducad-photoreal`), studi FEA lanjutan (modal, tekuk, termal), sketsa bebas/tinta (`ducad-ink`), mode Vektor 2D (`AppMode::Vector`), dan sinkronisasi awan (`AccountDrawer` Cloud Sync) melalui `ducad-i18n/locales/{en-US,id-ID}/main.ftl`, `ducad-app/src/mode/mod.rs`, dan `ducad-ui/src/account_drawer.rs`.
- **Kejujuran Klaim Produk & Uji Emas Lintas Platform**:
  - Menghapus klaim `"CAD/CAM"` pada kepala gambar (`ducad-io/src/drawing.rs`, `ducad-ui/src/drawing_sheet_view.rs`) dan metadata `<desc>` SVG (`ducad-io/src/svg.rs`) menjadi `"DUCAD Studio"` / `"DuCAD"`.
  - Memperbarui `ducad-engine/tests/drawing_golden.rs` dan `drawing.hashes.truck.json` agar memverifikasi invarian struktural lembar gambar lintas platform (`StructuralSheetGolden`: dimensi mm, grup tampak, jumlah garis kontinu/putus-putus, dan teks penting) sekaligus hash byte per `{os}-{arch}`.
- **Pembersihan Kode UI Mati (~3.400 Baris) & Saklar Semu**:
  - Menghapus 12 berkas UI tak terpakai (`ducad-ui/src/feature_inspector/*`, `ducad-ui/src/radial_menu.rs`, dan 7 berkas popup lama di `ducad-ui/src/tool_popups/*`) serta memangkas `history_drawer.rs` menjadi tipe data murni yang benar-benar dipakai.
  - Menyembunyikan saklar `palm_rejection` desktop yang belum terhubung ke pemfilteran pointer dari `top_bar.rs`, `command_palette.rs`, `shortcuts.rs`, dan `app.rs`.

### 2. Penguatan Keamanan Akun (`ducad-cloud` & `DUCAD-SERVER`) & Hapus Akun Mandiri
- **Sisi Klien (`ducad-cloud`, `ducad-ui`, `ducad-app`)**:
  - Menghentikan pencatatan `login_url` dan `ticket` OAuth pada level `info!` di `ducad-cloud/src/auth.rs`.
  - Menetapkan izin direktori konfigurasi `0700` dan berkas `session.json` `0600` di sistem Unix (`save_account_to`).
  - Menambahkan metode `CloudSession::delete_account` (`DELETE /api/v1/users/me`), tombol **Hapus Akun Permanen** dengan konfirmasi eksplisit di `AccountDrawer`, serta pembersihan menyeluruh berkas sesi, avatar lokal, dan state sinkronisasi di `ducad-app/src/cloud_ui.rs`.
- **Sisi Server (`DUCAD-SERVER`)**:
  - **Hashing Refresh Token**: Menyimpan SHA-256 hex (`jwt::hash_refresh_token`) di kolom `sessions.refresh_token` pada penerbitan maupun rotasi token, dengan kompatibilitas transisi untuk sesi lama agar berputar mulus tanpa memutus login pengguna.
  - **Validasi Ticket & Pembatasan Laju (`RateLimiter`)**: Memvalidasi format `ticket` (32–64 karakter hex ASCII) di `/api/v1/auth/login/{provider}` dan `/api/v1/auth/ticket/poll`, serta memasang middleware pembatas laju per IP (`30 req/menit` untuk endpoint auth umum, `120 req/menit` untuk `/ticket/poll`) dengan balasan HTTP `429 Too Many Requests` + header `Retry-After`.
  - **Pencabutan Token Apple (Guideline 5.1.1(v))**: Menyimpan `apple_refresh_token` pada tabel `users` (lengkap dengan migrasi kolom idempoten untuk basis data MySQL yang sudah berjalan) dan memanggil `https://appleid.apple.com/auth/revoke` saat pengguna mengeksekusi `DELETE /api/v1/users/me`.

### 3. Verifikasi Tanda Tangan Manifest Pembaruan (`ducad-update`) & Dokumentasi Bahasa Inggris
- Menambahkan modul `crates/ducad-update/src/signature.rs` berbasis `ring` (Ed25519) + RFC 7693 BLAKE2b-512 murni Rust untuk memverifikasi berkas tanda tangan `<paket>.sha256.minisig` (Minisign `ED` maupun `Ed`, termasuk `keynum` dan tanda tangan global `trusted comment`) serta `<paket>.sha256.sig` (Ed25519 mentah Base64/hex) terhadap kunci publik `DEFAULT_UPDATE_PUBKEY` / `DUCAD_UPDATE_PUBKEY`.
- Menambahkan langkah opsional penandatanganan Minisign di `.github/workflows/release.yml` (aktif saat secret `DUCAD_UPDATE_MINISIGN_SECKEY` tersedia) dan mendokumentasikan prosedurnya di `ducad-editor/docs/PACKAGING.md`.
- Menambahkan panduan bahasa Inggris ringkas di `ducad-editor/docs/GUIDE_EN.md` dan catatan rilis terstruktur di `CHANGELOG.md`, serta menautkannya dari `README.md`.

### Verifikasi Tahap 2
- `DUCAD-SERVER`: `cargo clippy --all-targets -- -D warnings` & `cargo test` -> **LULUS** (70/70 tes).
- `DUCAD-STUDIO`: `cargo deny check` -> **LULUS** (`advisories ok, bans ok, licenses ok, sources ok`).
- `DUCAD-STUDIO`: `cargo clippy --workspace --all-targets -- -D warnings` -> **LULUS** tanpa peringatan.
- `DUCAD-STUDIO`: `cargo test -p ducad-core -p ducad-cloud -p ducad-update -p ducad-ui -p ducad-app -p ducad-i18n` -> **LULUS** (termasuk `signature::tests`, `account_drawer_render`, `ui_glyph_coverage`, dan `layering`).
- `DUCAD-STUDIO`: `cargo test -p ducad-engine --test drawing_golden` -> **LULUS** (5/5).
- `DUCAD-STUDIO`: `scripts/release-gate.sh` -> **LULUS**.


---

## Tahap 3 — Perbaikan UI/UX, Registri Perintah, Input Satuan, Token Desain & i18n

**Status:** Selesai & Terverifikasi

### 1. Pusat Notifikasi Bertingkat Tunggal (`NotificationCenter`)
- Menambahkan modul `crates/ducad-app/src/notifications.rs` dan widget `crates/ducad-ui/src/notification_toast.rs` yang menyatukan tiga saluran status yang sebelumnya terpisah (`model_status`, `file_status`, dan `constraint_status`) ke dalam antrean bertingkat (`Info`, `Success`, `Warning`, `Error`) dengan riwayat maks. 20 entri, lencana hitungan di bilah atas (`TopBar`), kartu roti panggang di pojok kanan bawah (maks. 3 tampak bersamaan, auto-dismiss untuk info/sukses, persisten dengan tombol tutup dan detail yang dapat dibentangkan untuk peringatan/galat).

### 2. Registri Perintah Tunggal (`CommandRegistry`) & Perlindungan Fokus Teks
- Menambahkan `crates/ducad-app/src/command_registry.rs` sebagai sumber kebenaran tunggal untuk seluruh ID perintah, cakupan mode (`Global`, `Solid`, `Sketch`, `SolidOrSketch`), dan pintasan bawaan beserta serialisasi pengikatan kustom di `prefs.json` (`shortcuts: BTreeMap<String, String>`).
- **Penyelarasan Pintasan & Pembebasan Tabrakan**:
  - `⌘D` diseragamkan ke **Lembar Gambar 2D** (`file.drawing_sheet`) sesuai lencana menu File; **Duplikat** memakai `Shift+D` di cakupan 3D.
  - Memisahkan tabrakan huruf tunggal dengan modifier eksplisit atau pemisahan cakupan (`T` = Potong/Trim di Sketsa vs `Shift+T` = Alat Teks; `P` = Titik di Sketsa vs `Shift+P` = Bidang Datum; `O` = Offset di Sketsa vs `Shift+O` = Bola di Solid; `S` = Kotak/Kubus di Solid & Segi Empat di Sketsa vs `Shift+K` = Cangkang/Shell).
  - Mencegah seluruh pintasan tanpa `⌘`/`Ctrl` bocor saat pengguna sedang mengetik di kotak pencarian, kolom angka, atau obrolan AI (`matches_input(..., text_focus)`).
  - Menambahkan antarmuka kustomisasi pintasan dengan tombol **Reset Default** di bagian Pengaturan `AccountDrawer` (`crates/ducad-ui/src/account_drawer.rs` & `crates/ducad-app/src/cloud_ui.rs`).
- **Command Palette Lengkap**: Memperluas `crates/ducad-ui/src/command_palette.rs` agar mencakup 100% alat pemodelan 3D (termasuk Silinder, Bola, Kerucut, Torus, Teks, Bidang Datum, Tarik Sisi, Cermin, Pisah Bodi, Susun Rata), seluruh alat & kendala Sketsa, serta seluruh format ekspor (`STEP`, `STL`, `OBJ`, `3MF`, `glTF/GLB`, `DXF`, `SVG`, `PDF`, `PNG`, `Paket Rilis`).

### 3. Input Angka Sadar-Satuan, Ekspresi & `$param` di Seluruh Operasi
- Memperluas `crates/ducad-core/src/units_input.rs` (`parse_input_mm_with_params`) dan `crates/ducad-ui/src/unit_input.rs` agar mengevaluasi rujukan parameter `$nama_param`, ekspresi aritmetika (`25.4/2`, `1/4 in`), akhiran satuan (`mm`, `cm`, `m`, `in`, `ft`), dan koma desimal (`12,5`) dengan konversi langsung ke milimeter kanonik beserta pratinjau nilai mm secara langsung.
- Menyambungkan evaluasi sadar-satuan & `$param` (`eval_length_mm`) ke seluruh operasi dimensi di `crates/ducad-app/src/modeling/operations.rs`, `crates/ducad-app/src/overlay/dimensions.rs`, dan `crates/ducad-app/src/oplog_gui.rs` — termasuk memperbaiki bug skala diam-diam pada **Fillet** dan **Chamfer** ketika satuan tampilan disetel ke inci/cm/m.

### 4. Sistem Token Desain, Tema Terang, Aksesibilitas & Target Sentuh
- **Token Tema & Kontras WCAG AA (`crates/ducad-ui/src/theme.rs` & `crates/ducad-glass/src/widget.rs`)**:
  - Memperkenalkan palet semantik `ThemeTokens` (Gelap & Terang) dengan rasio kontras teks sekunder ≥ 4.5:1 di atas panel kaca, ukuran font minimum `12.0 pt`, skala UI (`ui_scale`: `0.85x`–`1.50x`), dan penghormatan `reduce_motion`.
  - Memperbaiki `GlassFrame::preset` agar beradaptasi ke latar terang alih-alih selalu memakai kaca gelap pekat, serta merombak `TopBar`, `LeftToolbar`, `ContextBar`, `AccountDrawer`, `CommandPalette`, `ItemsDrawer`, `FeatureTreeDrawer`, `CmfDrawer`, `LightingDrawer`, `BriefStudio`, `Swatches`, dan `ToolPopups` agar memakai `ThemeTokens`.
- **Keterjangkauan & Target Sentuh**:
  - Menambahkan opsi tampilan label teks di bawah ikon toolbar kiri (`show_toolbar_labels` di Pengaturan).
  - Menegakkan tinggi target sentuh minimum `44 pt` (`TOUCH_TARGET_IPAD`) di mode Sentuh/Otomatis pada tombol toolbar kiri dan kontrol utama.
  - Menyuntikkan metadata AccessKit (`Response::widget_info`) pada tombol-tombol berbasis ikon agar terbaca oleh pembaca layar.

### 5. Internasionalisasi (i18n) Menyeluruh & Inggris Bawaan `ducad-cli`
- Menambahkan ~230 pasangan kunci terjemahan baru yang setara 1:1 di `crates/ducad-i18n/locales/en-US/main.ftl` dan `crates/ducad-i18n/locales/id-ID/main.ftl` (termasuk 7 kunci yang sebelumnya hilang di kode lama).
- Mengganti literal Bahasa Indonesia yang sebelumnya tertulis langsung di permukaan UI dengan makro `ducad_i18n::t!`, serta menambahkan tes penjaga otomatis `crates/ducad-ui/tests/i18n_literal_guard.rs` ke `scripts/release-gate.sh` untuk memblokir PR apabila ada kunci `t!` yang hilang di salah satu bahasa atau ada literal widget Bahasa Indonesia yang bocor kembali.
- Mengubah bahasa keluaran dan bantuan bawaan `ducad-cli` menjadi Bahasa Inggris dengan dukungan flag global `--lang <en|id>` dan variabel lingkungan `DUCAD_LANG` untuk mempertahankan keluaran Bahasa Indonesia saat diminta.

### Verifikasi Tahap 3
- `cargo clippy --workspace --all-targets -- -D warnings`: **LULUS** tanpa peringatan.
- `cargo test -p ducad-core -p ducad-glass -p ducad-i18n`: **LULUS** (`units_input`, 22/22 `ducad-glass`, 4/4 `ducad-i18n`).
- `cargo test -p ducad-ui`: **LULUS** (72/72 unit test + seluruh 11 target tes integrasi termasuk `ui_glyph_coverage` 4/4 dan `i18n_literal_guard` 2/2).
- `cargo test -p ducad-app --lib`: **LULUS** (446/446 tes, termasuk `command_registry`, `notifications`, dan konversi satuan/`$param` Fillet & Chamfer).
- `cargo test -p ducad-cli`: **LULUS** (9/9 unit + 1/1 `chat` + 18/18 `cli` integrasi termasuk `--lang id` & `DUCAD_LANG=id`).
- `scripts/release-gate.sh`: **LULUS** (102/102 tes lulus di seluruh 6 grup metrik rilis, 0 gagal).

