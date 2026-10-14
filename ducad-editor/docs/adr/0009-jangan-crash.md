# ADR 0009 — Jangan pernah crash: pagar panik, log berkas, autosave desktop, penjaga layering

**Status:** diterima dan diterapkan penuh (2026-10-14)

## Konteks

Perbandingan dengan [CADCraft](https://github.com/storytold/cadcraft) (drafting 2D
klon AutoCAD, Rust + egui, dikendalikan agent lewat MCP — lihat
`docs/ANALISIS_KOMPARATIF_CAD.md` §5) menunjukkan DUCAD sudah lebih jauh dalam
hal kontrak agent (oplog yang bisa di-replay, diagnosis + fix terverifikasi,
checks bersitasi, paket desain), tetapi tertinggal pada satu prinsip
operasional yang di CADCraft "mengalahkan pekerjaan fitur": **aplikasi tidak
boleh crash, dan pekerjaan pengguna tidak boleh hilang**.

Keadaan DUCAD sebelum ADR ini:

- Satu `panic!` di dalam op (OCCT, index di luar batas, `unwrap` yang
  terlewat) menjatuhkan GUI (dengan pekerjaan yang belum disimpan), server
  `ducad-mcp` (di tengah sesi agent), atau `ducad-cli` (di tengah build CI).
  `SessionCore::run_with` membuka transaksi undo dan tidak pernah menutupnya.
- Aturan "tidak ada `unwrap()`/`expect()` pada input dari luar" hanya tertulis
  di `CLAUDE.md`, tidak ditegakkan lint. Engine punya 23 `unwrap`/`expect` di
  kode produksi (semua terbukti aman, tetapi tanpa penjaga).
- Log aplikasi hanya ke stderr (`env_logger::init()`): aplikasi yang dibuka
  dari Dock tidak meninggalkan jejak apa pun saat crash.
- Autosave pemulihan hanya di tablet; di desktop crash = kehilangan semua
  perubahan sejak simpan terakhir.
- Pelapisan crate dijaga tiga tes ad-hoc (`engine_has_no_gui_dependency`,
  `glass_has_no_app_or_ui_dependency`, `sim_has_no_kernel_or_gui_dependency`);
  panah lain di peta `CLAUDE.md` tidak dijaga.

## Keputusan

1. **Pagar panik** (`ducad-engine/src/guard.rs`, `catch_op_panic`): setiap op
   di `SessionCore::run_with` (termasuk diagnosisnya), setiap tool di
   `tooling::call_core_tool`/`call_stateless_tool`, `ducad-mcp::tools::call_out`,
   `AgentBridge::agent_call` (UI thread), dan `DuCADApp::execute_model_command`
   (command GUI) dijalankan di bawah `catch_unwind`. Panik menjadi `OpError`
   berkode baru **`internal`** (hint: "ini bug DUCAD, bukan salah parameter"),
   jalur rollback batch yang biasa tetap berjalan, dan paniknya dicatat
   `log::error!`. `lock_kernel()` sudah memulihkan mutex yang ter-poison, jadi
   kernel tetap bisa dipakai setelahnya.
2. **Lint anti-panik di SEMUA crate workspace**:
   `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic,
   clippy::todo, clippy::unimplemented)]` di akar setiap crate (modul tes
   dikecualikan lewat `cfg_attr(test, allow(...))`). Pola pengganti:
   `let … else`, `ok_or_else`, `?` pada fungsi ber-`Option`,
   `(v.len() == 1).then(|| v.pop()).flatten()`, indeks langsung bila panjang
   sudah dicek, dan `total_cmp` untuk komparator `f64`. Pengecualian hanya
   untuk invarian tipe yang dijaga field privat, dengan `#[allow]` lokal dan
   komentar alasan; saat ini satu: `SketchSet::active()` (`ducad-sketch`).
   **Komparator sort:** sejak Rust 1.81 `sort_by` boleh panik bila
   komparatornya bukan urutan total, dan `partial_cmp(..).unwrap_or(Equal)`
   dengan NaN melanggar itu; semua komparator `f64` di kode produksi kini
   `total_cmp`.
3. **Tes input hostil** (pola `hostile_params_never_panic` CADCraft):
   `ducad-engine/tests/hostile.rs` memutasi setiap daun JSON op pada fixture
   teruji (angka → 0/−1/±1e308/1e-300/teks, string → kosong/selector
   rusak/ekspresi salah, larik → kosong) lalu menjalankannya `dry_run`; hasil
   `internal` = panik tertangkap = tes gagal. Sapuan default memakai empat
   fixture (±5 s); sapuan penuh atas semua fixture (±90 s) adalah tes
   `#[ignore]`: `cargo test -p ducad-engine --test hostile -- --ignored`
   (`DUCAD_HOSTILE_TRACE=1 … -- --nocapture` menampilkan mutasi terakhir bila
   biner tes abort). `ducad-mcp` menguji setiap tool dengan argumen rusak
   lewat `call_inner` (tanpa pagar), dan `ducad-io/tests/hostile_stl.rs`
   menguji STL rusak.
4. **Log berkas dengan rotasi** (`ducad-app/src/logging.rs`): `log::` ke
   stderr dan `~/.ducad/logs/ducad.log` (iPad: Application Support), rotasi
   `ducad.1.log`/`ducad.2.log` tiap start, berhenti di 16 MiB, filter
   `RUST_LOG` (bawaan `warn,ducad=info`), dan panic hook yang menulis setiap
   panik ke log sebelum hook bawaan.
5. **Autosave pemulihan di semua platform**: `App::save` menulis
   `~/.ducad/autosave.ducad` tiap 30 s bila ada perubahan; di desktop
   `on_exit` menghapusnya (keluar normal), sehingga berkas yang tersisa saat
   start = bekas crash dan dipulihkan dengan pesan
   `file-restored-autosave-desktop`.
6. **Penjaga layering tabel** (`ducad-core/tests/layering.rs`): tabel
   dependensi DUCAD yang diizinkan per crate (= peta `CLAUDE.md`), crate UI
   hanya di `glass/render/ui/app/android`, `opencascade` hanya di `kernel`.
   Menambah panah = ubah tabel + peta.
7. **Batas jangkauan sebelum kernel.** Exception C++ OCCT yang tidak
   dibungkus binding (`StdFail_NotDone`, `Standard_DomainError`) menembus FFI
   dan meng-ABORT proses; `catch_unwind` tidak bisa menangkapnya. Tes hostil
   menemukan dua pemicu (lubang Ø1e308, box 1e-300). Pertahanannya di engine:
   `compute::check_op_magnitudes` menolak setiap angka op di luar ±1e9 sebelum
   `apply`, dan `require_positive` (semua panjang/radius/tebal, kini juga
   dimensi primitif) membatasi ke 1e-6..1e6 mm. Keduanya `invalid_param`,
   tercantum di `ERROR_GUIDE`. Alokasi dari input juga dibatasi: header STL
   biner tidak lagi bisa meminta `Vec::with_capacity` miliaran elemen.

## Yang sengaja TIDAK diambil dari CADCraft

- **Everything-is-a-command / command line gaya AutoCAD.** DUCAD sudah punya
  kontrak yang lebih kuat untuk agent (oplog deklaratif, selector, diagnosis),
  dan GUI-nya direct-modeling, bukan drafting berbasis prompt.
- **Kanal kontrol TCP + `ui_click`/`screenshot` lewat protokol JSON.** DUCAD
  memakai soket Unix `agent.sock` dengan pagar persetujuan pengguna
  (`propose_ops`); menambah klik UI sintetis bertentangan dengan pagar itu.
- **Clean-room/ATTRIBUTION per aset.** Tidak relevan: DUCAD memakai OCCT
  (LGPL, dibungkus satu crate) dan `deny.toml` sudah menjaga lisensi.
- **Dokumen paritas** (`gaps.md`, `*-parity.md`): DUCAD bukan klon produk
  lain; `docs/PLAN.md` + `STATUS_FASE_A_B.md` sudah memegang peran itu.

## Konsekuensi

- Error `internal` adalah kode kontrak baru (`OpErrorCode::Internal`): ada di
  `ERROR_GUIDE` (`ducad://guide`) dan `SKILL.md`. Skema op
  (`ops.schema.json`) tidak memuat kode error, jadi tidak berubah. Agent
  diinstruksikan untuk tidak "memperbaiki parameter" pada error ini,
  melainkan mencoba jalur lain dan melaporkannya.
- Panik yang tertangkap di tengah `UndoStack::execute` (GUI) bisa meninggalkan
  model setengah berubah tanpa entri undo; itu masih lebih baik daripada
  kehilangan dokumen, dan autosave + log memberi jalan pulih. Panik di dalam
  `run_with` aman: rollback transaksi + `meta` lama dipulihkan.
- `DuCADApp::new` kini mengembalikan `Result` (tanpa backend wgpu, eframe
  melaporkan error alih-alih panik); `ImportWorker` menjalankan impor di
  thread pemanggil bila thread worker gagal dibuat.
- Exception C++ OCCT tetap menjadi jalur crash yang tersisa untuk input yang
  lolos validasi tetapi tetap ditolak OCCT. Perbaikan tuntasnya adalah
  membungkus pembuat shape/boolean di binding yang di-vendor
  (`vendors/opencascade-rs/crates/opencascade-sys/include/*.hxx`, pola
  `try { … } catch (const Standard_Failure&)` yang sudah dipakai
  `b_rep_adaptor.hxx`) — pekerjaan terpisah karena menyentuh blok
  `[patch.crates-io]`. Sampai itu selesai, setiap abort baru yang ditemukan
  sapuan penuh ditutup dengan validasi di engine plus kasus di
  `absurd_magnitudes_are_rejected_before_the_kernel`.
- `cargo test -p ducad-core --test layering` wajib hijau sebelum menambah
  dependensi antar-crate.
