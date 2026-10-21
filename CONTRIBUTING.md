# Berkontribusi ke DUCAD

Aturan keras dan peta crate untuk siapa pun (manusia atau agent) yang
mengubah kode. Menggantikan `CLAUDE.md` lama (dihapus di `cd57db0f`).
Diperbarui 2026-10-21 (Step 5
`ducad-editor/docs/PLAN_DESIGNER_UX.md`).

## Peta crate (`ducad-editor/crates/`)

Arah panah = "bergantung pada". Tabel ini **kontrak** yang dijaga
`ducad-core/tests/layering.rs`; menambah panah = ubah tabel itu DAN daftar ini.

- `ducad-core` — dokumen, body, material, undo stack, spesifikasi lubang, konfigurasi, model sheet metal, toleransi/GD&T, tier lisensi. Tanpa kernel.
- `ducad-i18n`, `ducad-chat`, `ducad-glass`, `ducad-sim`, `ducad-update` — tanpa dependensi DUCAD.
- `ducad-cloud` — akun dan sinkron cloud. → core (tier lisensi)
- `ducad-sketch` — entitas 2D, constraint + solver, status DOF. → core
- `ducad-kernel` — satu-satunya pembungkus kernel geometri (SolidCraft/truck): `KernelShape`, `KernelMesh`, STEP/IGES, HLR, potongan. → core
- `ducad-ink` — tinta bebas. → core, sketch
- `ducad-render` — renderer wgpu. → core, sketch
- `ducad-io` — `.ducad`, STL/OBJ/GLB/3MF, SVG/PDF/DXF, tata letak gambar kerja. → core, sketch, kernel, ink
- `ducad-engine` — modeling headless: `Op`/oplog, `compute`, selector, `Session`, checks, paket desain, proyek contoh (`samples`). **Sumber kebenaran pemodelan** (ADR 0010). → core, sketch, kernel, io, sim
- `ducad-assist` → engine; `ducad-mcp` → engine, kernel
- `ducad-ui` — widget egui. → core, sketch, kernel, io, glass, i18n, cloud
- `ducad-cli` → core, sketch, kernel, io, sim, engine, assist, chat, mcp
- `ducad-app` — GUI (`ducad`). → semua di atas kecuali cli
- `ducad-android` → app

## Aturan keras

1. **Kernel terkurung.** Hanya `ducad-kernel` yang boleh memakai crate
   `solidcraft-*`/`truck-*`. Dependensi kernel C++ dilarang (dijaga
   `ducad-core/tests/layering.rs`).
2. **Engine tanpa GUI.** `ducad-engine` tidak boleh bergantung pada
   egui/eframe/wgpu/ducad-render/ducad-ui/rfd (`engine_has_no_gui_dependency`).
   Crate UI hanya di glass/render/ui/app/android.
3. **GUI lewat oplog (ADR 0010).** Operasi pemodelan GUI menulis op ke oplog
   engine (`ducad-app/src/oplog_gui.rs`), bukan mengubah model langsung.
   Edit parameter/fitur = replay oplog, satu langkah undo. Assembly adalah
   lapisan terpisah (ADR 0011).
4. **Jangan pernah crash (ADR 0009).** Semua crate kecuali `ducad-android`
   memasang `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, …)]`
   (modul tes dikecualikan). Pakai `?`, `let … else`, `ok_or_else`, dan
   `total_cmp` untuk `f64`. Panik yang lolos ditangkap pagar
   `ducad_engine::guard` menjadi error `internal`; tetap perbaiki akarnya dan
   tambahkan kasus ke `ducad-engine/tests/hostile.rs`. Operasi kernel yang
   bisa lama memakai `compute::budget` (berbatas waktu, `kernel_timeout`).
5. **Bahasa.** Komentar, dokumen, dan teks untuk pengguna dalam bahasa
   Indonesia (lewat `ducad-i18n`, id-ID dan en-US); identifier bahasa Inggris.
   Pengecualian: semua teks yang dibaca agent lewat MCP (deskripsi tool,
   `ducad://guide`, doc `///` pada tipe kontrak `Op`/`Check`) dalam bahasa
   Inggris.
6. **Ikon UI.** Jangan menulis glyph Unicode/emoji mentah sebagai ikon; pakai
   `egui_icons::icons::ICON_*`. Setelah menambah teks UI jalankan
   `cargo test -p ducad-ui --test ui_glyph_coverage`.
7. **Body di engine dirujuk lewat nama** (id op pembuatnya), bukan `BodyId`.
8. Jangan ubah `rust-toolchain.toml` atau blok `[patch.crates-io]` (crate
   truck yang di-patch SolidCraft, `vendors/solidcraft/vendor/README.md`).
   Dependensi baru harus berlisensi yang diizinkan `ducad-editor/deny.toml`.
9. Jangan mengganti label command model (`"Extrude"`, `"Fillet"`, …):
   label dipetakan ke judul aktivitas dan deteksi tutorial.

## Perintah

```bash
cd ducad-editor
cargo build -p ducad-app                                # GUI
cargo test -p <crate>                                   # selama mengerjakan satu crate
cargo clippy --workspace --all-targets -- -D warnings   # gerbang CI
cargo test --workspace                                  # gerbang CI
cargo test -p ducad-core --test layering                # penjaga peta crate di atas
cargo clean-ws                                          # bersihkan hanya crate workspace
make install-agent-tools                                # pasang ducad-cli, ducad-mcp
```

Skema `Op` di `crates/ducad-engine/schema/ops.schema.json`; perbarui dengan
`DUCAD_UPDATE_SCHEMA=1 cargo test -p ducad-engine schema_file`. Hash emas
gambar kerja: `DUCAD_UPDATE_GOLDEN=1 cargo test -p ducad-engine --test drawing_golden`.

## Menambah operasi baru

1. Fungsi kernel + tes di `ducad-kernel`.
2. Fungsi murni di `ducad-engine/src/compute/` (validasi → `InvalidParam`,
   error kernel → `OpError::kernel`; operasi berat lewat `compute::budget`).
3. Varian `Op` di `ops/spec.rs` dengan doc `///` bahasa Inggris pada varian
   dan setiap field, lalu perbarui skema.
4. Pemetaan di `session.rs` (`SessionCore::apply`) + tes, dan contoh teruji
   di `tests/fixtures/*.ops.json` bila op belum diperagakan.
5. Jalur GUI di `ducad-app/src/oplog_gui.rs` (menulis op, bukan memanggil
   kernel langsung) + tes.
6. Skill agent `.claude/skills/ducad-modeling/SKILL.md`; kode error baru ke
   `tooling::ERROR_GUIDE`.

## Menambah check atau aturan

1. Varian `Check` + doc `///` bahasa Inggris di `check/types.rs`, evaluasi di
   `check/run.rs`; tes di `ducad-engine/tests/dfm.rs`.
2. Aturan baru = entri di `rules/<paket>.json` dengan `citation` dan `basis`
   (`standard` hanya bila memang isi standar; selain itu `guideline`).

## Sebelum PR dan rilis

- `cargo clippy … -D warnings` dan `cargo test --workspace` hijau.
- `scripts/release-gate.sh` hijau (lane CI `designer`, memblokir).
- Perubahan visual: bagian yang relevan di
  `ducad-editor/docs/CEKLIS_UJI_GUI.md`.
- Klaim fitur di README/panduan sesuai `ducad-editor/docs/STATUS_KEMAMPUAN.md`
  dan `ducad-editor/docs/FORMAT_MATRIX.md`.
- Rilis: `ducad-editor/docs/GERBANG_RILIS.md`.

## Dokumen

- Rencana aktif: `ducad-editor/docs/PLAN_DESIGNER_UX.md`; baseline dan
  kemajuan: `BASELINE_DESIGNER_UX.md`.
- Status kemampuan: `STATUS_KEMAMPUAN.md` (acuan utama), `STATUS_FASE_A_B.md`.
- Keputusan arsitektur: `ducad-editor/docs/adr/`.
