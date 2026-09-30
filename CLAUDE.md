# DUCAD — panduan untuk Claude Code

## Apa ini

DUCAD adalah aplikasi CAD 2D/3D: drafting sketch presisi dengan constraint,
lalu pemodelan solid B-rep (extrude, revolve, boolean, fillet, shell, lubang)
di atas kernel OpenCASCADE. Selain GUI, ada lapisan headless (`ducad-engine`)
yang dipakai CLI dan server MCP sehingga agent bisa memodelkan part lewat
oplog JSON yang bisa di-replay. Platform: macOS (desktop) dan iPad (iOS).

## Peta crate (`ducad-editor/crates/`)

Arah panah = "bergantung pada".

- `ducad-core` — dokumen, body, material, undo stack, spesifikasi lubang. Tanpa kernel.
- `ducad-sketch` — entitas 2D, constraint + solver, region tertutup, pengenal coretan (`recognize`) + inferensi constraint (`infer`). → core
- `ducad-kernel` — satu-satunya pembungkus OpenCASCADE (`KernelShape`, `KernelMesh`). → core
- `ducad-ink` — dokumen tinta bebas (`InkDoc`, coretan bertekanan, kuas, command, indeks spasial, eraser/lasso). Tanpa GUI/kernel. → core, sketch
- `ducad-io` — format `.ducad`, STEP/STL/OBJ/GLB, SVG/PDF/DXF. → core, sketch, kernel, ink
- `ducad-engine` — modeling headless: `compute`, `Op`/oplog, selector, `Session`, inspect, render. → core, sketch, kernel, io
- `ducad-cli` — binary `ducad-cli` (run/replay/inspect/check/oplog/diff/select/render/export/build/assist/chat/schema). → engine, assist, chat, mcp
- `ducad-mcp` — binary `ducad-mcp`, server MCP stdio: 24 tool + resources/prompts; mode `--attach` menambah 6 tool live. → engine
- `ducad-chat` — chat agent tanpa GUI/kernel: provider Anthropic/OpenAI-compatible (`ureq`, SSE), loop tool-use, pengaturan + kunci API (Keychain). Lihat `docs/adr/0005-chat-agent.md`.
- `ducad-assist` — asisten AI lokal/offline: backend di perangkat, loop yang selalu berujung proposal. Fitur `apple-fm`/`local-gguf` mati secara default (lihat `docs/adr/0002-ai-lokal.md`). → engine
- `ducad-render` — renderer wgpu viewport. → core
- `ducad-ui`, `ducad-i18n`, `ducad-cloud` — widget egui, terjemahan, akun.
- `ducad-app` — GUI (binary `ducad`); operasi modeling = adapter tipis di atas `ducad_engine::compute`. → semua di atas

Aturan: **`ducad-engine` tidak boleh bergantung pada egui/eframe/wgpu/
ducad-render/ducad-ui/rfd** (dijaga tes `engine_has_no_gui_dependency`).

## Perintah

```bash
cd ducad-editor
cargo build -p ducad-app                       # GUI
cargo test -p <crate>                          # selama mengerjakan satu crate
cargo fmt --all -- --check                     # gerbang CI (belum memblokir; format hanya berkas baru)
cargo clippy --workspace --all-targets -- -D warnings   # gerbang CI
cargo test --workspace                         # gerbang CI
make install-agent-tools                       # pasang ducad-cli, ducad-mcp (+ mnemonic-cli bila ada)
```

**Jembatan live (P5).** `ducad-mcp --attach [--socket PATH]` meneruskan
semua tool ke aplikasi yang sedang terbuka lewat `$HOME/.ducad/agent.sock`
(nyalakan di command palette → "Agent Bridge"; mati secara default, dan
diblokir saat privasi AI `OfflineOnly`). Satu batch agent = satu langkah
undo GUI; `propose_ops` menampilkan ghost hijau/merah dan baru dijawab
setelah pengguna menekan Terima/Tolak. `accept_proposal` sengaja TIDAK
tersedia bagi agent pada mode ini.

**Memori di perangkat (P11.5).** `cargo build -p ducad-app --features memory`
menautkan pustaka MNEMONIC headless (`default-features = false`) sehingga
`VaultService` dipanggil di dalam proses — satu-satunya jalan di iPadOS,
yang tidak bisa menjalankan server MCP. Mati secara default. Vault bawaan:
`$HOME/DUCAD-Memory` (iOS: Documents/DUCAD-Memory).

**Chat AI (P13).** Panel "Chat AI" (⌘⇧A / tombol ✨ AI) menjalankan
`ducad_chat::run_turn` di thread latar; setiap tool dikirim ke kanal
in-process jembatan agent (`agent_bridge.rs`), jadi tool dan pagarnya
identik dengan `--attach`. Privasi bawaan "hanya di perangkat": provider
jaringan dan Agent Bridge baru aktif setelah ⚙ → "Izinkan AI eksternal".
Headless: `ducad-cli chat [PART] --instruction "…" --provider anthropic --out OUT.ducad`.

`ducad-cli build PART --out DIR` membuat artefak manufaktur (STEP/STL/PDF/
PNG/BOM) + `report.json`/`report.md` secara deterministik; checks yang gagal
menghentikan build dengan kode 3. Lihat `docs/ci/README.md`.

Version control desain: commit `*.ops.json` sebagai sumber dan `.ducad`
sebagai artefak. `ducad-cli oplog PART.ducad` menulis oplog satu op per
baris; aktifkan diff git dengan `git config diff.ducad.textconv "ducad-cli oplog"`
(`.gitattributes` sudah memetakan `*.ducad diff=ducad`).

Skema `Op` tersimpan di `crates/ducad-engine/schema/ops.schema.json`;
perbarui dengan `DUCAD_UPDATE_SCHEMA=1 cargo test -p ducad-engine schema_file`.

## Aturan keras

- Hanya `ducad-kernel` yang boleh `use opencascade::…`. Crate lain melihat
  `KernelShape`/`KernelMesh`/tipe publik kernel saja.
- Setiap fungsi publik kernel yang menyentuh OCCT membuka dengan
  `let _guard = lock_kernel();` dan **tidak boleh** memanggil fungsi publik
  kernel lain selagi memegang guard (Mutex tidak reentrant → deadlock).
  Logika bersama ditaruh di helper `pub(crate)` tanpa lock.
- Jangan ubah `rust-toolchain.toml`, versi/fitur `opencascade`, atau blok
  `[patch.crates-io]` — memicu kompilasi ulang OCCT 10–15 menit.
- Komentar dan pesan untuk user dalam **bahasa Indonesia**; identifier Inggris.
- Jangan mengganti label command model (`"Extrude"`, `"Fillet"`, …):
  `execute_model_command` memetakan label ke judul aktivitas.
- Di engine, body dirujuk lewat **nama** (id op pembuatnya), bukan `BodyId`
  (berganti saat undo/redo boolean).
- Tidak ada `unwrap()`/`expect()` pada input dari luar (JSON, file, argumen tool).
- Dependency baru harus berlisensi yang diizinkan `ducad-editor/deny.toml`.

## Menambah operasi baru

1. Fungsi kernel + tes di `ducad-kernel` (pegang `TEST_LOCK` di tes kernel).
2. Fungsi murni di `ducad-engine/src/compute/` (validasi → `InvalidParam`,
   error kernel → `OpError::kernel`, cek `is_valid`/volume).
3. Varian `Op` di `ops/spec.rs` + perbarui `schema/ops.schema.json`.
4. Pemetaan op → command di `session.rs` (`SessionCore::apply`), plus tes.
5. Adapter GUI di `ducad-app/src/modeling/operations.rs` (label command tetap).
6. Dokumentasi skill `.claude/skills/ducad-modeling/SKILL.md` (selector,
   kode error, contoh).

## Dokumen

- Rencana harness agent: `.claude/plan/ducad-agent-harness/` (mulai dari `00-konvensi.md`).
- Rencana produk: `ducad-editor/docs/PLAN.md`; status: `ducad-editor/docs/STATUS_FASE_A_B.md`.
- Keputusan arsitektur: `ducad-editor/docs/adr/`.
- Skill agent: `.claude/skills/ducad-modeling/SKILL.md`; memori agent: `scripts/init_memory_vault.sh`.
- Rencana chat agent + op lanjutan: `.claude/plan/ducad-agent-harness/P13-P15-chat-agent.md`.
