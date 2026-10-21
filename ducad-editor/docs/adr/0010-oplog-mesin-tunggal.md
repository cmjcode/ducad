# ADR 0010 — Oplog `ducad-engine` sebagai satu-satunya mesin pemodelan

**Status:** diterima (2026-10-21); penerapan bertahap di Step 1a–1c
`docs/PLAN_DESIGNER_UX.md`.

## Konteks

DUCAD punya dua jalur pemodelan yang hidup berdampingan
(`docs/BASELINE_DESIGNER_UX.md`, `docs/STATUS_KEMAMPUAN.md`):

| | GUI manual | Oplog engine |
|---|---|---|
| Kode | `ducad-app/src/modeling/operations.rs` + DAG `parametric_engine.rs` | `ducad-engine` (`ops/spec.rs`, `session.rs`, `select/`) |
| Referensi geometri | indeks (`plane_index`, urutan body), atau tidak ada (fillet → `fillet_all`) | selector semantik (`>Z`, `of(+Z except >Z) and of(+X except >X)`) |
| Regenerasi | sebagian fitur `Unsupported` (hole, boolean, helix…), loft/sweep/draft/pattern tidak tercatat | replay penuh, deterministik, atomik, rollback |
| Disimpan | DAG tidak ikut `.ducad`, direset saat buka/baru | `design.oplog` di `.ducad` v4 |
| Error | status per fitur | kode error + hint + fix terverifikasi |
| Dipakai oleh | alur manual designer | CLI, MCP, agent, proposal, panel Industrial/Sim |

Akibatnya, designer yang memakai GUI tidak mendapat jaminan yang sudah
dimiliki agent: edit dimensi awal merusak fitur hilir (B0-5), riwayat hilang
setelah buka ulang (B0-8), dan setiap edit manual membuat oplog agent dibuang
sebagai `oplog_stale`. Proyek contoh Step 0 membuktikan jalur oplog sudah
lulus T1–T6 secara otomatis, sedangkan jalur GUI diperkirakan gagal di T2.

## Keputusan

1. **Oplog `ducad-engine` adalah sumber kebenaran pemodelan.** Setiap aksi
   pemodelan GUI menghasilkan `Op` dan dijalankan lewat `SessionCore` di atas
   state aplikasi (jalur yang sama dengan jembatan agent P5). GUI tidak lagi
   memanggil `ducad_kernel::*` langsung untuk fitur yang punya padanan op.
2. **`parametric_engine` dipensiunkan per fitur.** Fitur yang sudah dialihkan
   ke op berhenti dicatat ke DAG. DAG dihapus setelah fitur terakhir pindah;
   sampai saat itu, DAG hanya menampung fitur yang belum punya op.
3. **Referensi dari klik pengguna menjadi selector semantik.** Engine
   menyediakan pembuat selector (`select::suggest_*`): dari face/tepi yang
   dipilih di viewport, cari selector yang (a) memilih himpunan yang sama
   persis, (b) tidak memakai indeks enumerasi, dan (c) diutamakan yang
   relasional (`+Z except >Z`, `of(A) and of(B)`) di atas filter koordinat.
   `idx:` hanya dipakai sebagai jalan terakhir dan ditandai rapuh di UI.
4. **Undo GUI membawa oplog.** Setiap aksi yang mengubah oplog menitipkan
   keadaan oplog sebelum/sesudah ke transaksi undo geometri yang sama
   (`ducad-app/src/oplog_undo.rs`, `UndoStack::attach_to_top`). Di sisi
   headless, `Session::undo/redo` mencakup batch op, `set_params`,
   `replace_op`, dan `remove_op` (B0-1).
5. **Kegagalan referensi selalu eksplisit.** Bila selector tidak lagi cocok
   setelah edit, op gagal dengan `selector_empty`; engine tidak pernah
   memilih face/tepi lain diam-diam.
6. **Topological naming penuh bukan prasyarat.** Riwayat kernel
   (generated/modified/deleted) hanya ditambahkan untuk kasus yang selectornya
   ambigu, setelah selector terbukti tidak cukup pada kasus nyata.

## Urutan migrasi

Diurutkan menurut kebutuhan proyek contoh (Bagian P, T1–T6), lalu frekuensi
pakai. Setiap baris selesai bila fitur GUI menghasilkan op, lulus tes
regenerasi setelah edit dimensi awal, dan tidak lagi dicatat ke DAG.

| Urutan | Fitur GUI | Op padanan | Celah yang harus ditutup dulu |
|---|---|---|---|
| 1 | Sketsa + constraint + dimensi | `sketch` | Penerjemah sketsa GUI → `EntitySpec`/`ConstraintSpec` (nama entitas stabil) |
| 2 | Extrude blind/symmetric, cut | `extrude` | Bidang sketsa GUI → `plane`/datum op, bukan `plane_index` |
| 3 | Fillet, chamfer | `fillet`, `chamfer` | `select::suggest_edges` dari tepi yang diklik |
| 4 | Hole wizard | `hole` | `select::suggest_faces` + posisi sebagai ekspresi; performa op `hole` (B0-7) |
| 5 | Boolean, pattern | `boolean`, `pattern` | — |
| 6 | Revolve, loft, sweep, shell, draft, emboss | op bernama sama | Sumbu revolve 3D di GUI |
| 7 | Datum bersudut / 3 titik | `construct_plane` (perluasan `PlaneSpec`) | Op belum ada (B0-9) |
| 8 | Edit sketsa (trim/extend/offset/mirror), teks, freehand | hasilnya masuk op `sketch` sebagai entitas akhir | Tidak butuh op baru bila sketsa disimpan sebagai keadaan akhirnya |
| 9 | Direct modeling | `offset_face`, `delete_face`, `replace_face`, … | Pilihan face lewat `suggest_faces` |
| 10 | Assembly, mate, coupling, explode | belum ada | Op assembly perlu desain tersendiri (ADR terpisah); sampai itu assembly tetap di luar oplog |

## Alternatif yang ditolak

- **Mempertahankan `parametric_engine` dan memperbaikinya.** Butuh membangun
  ulang semua yang sudah ada di oplog (rollback atomik, diagnosis, fix,
  selector, simpan/replay) untuk jalur kedua, lalu tetap harus
  menyinkronkannya dengan agent.
- **Sinkronisasi dua arah DAG ↔ oplog.** Dua sumber kebenaran dengan
  semantik referensi berbeda; konflik tidak terhindarkan dan sulit diuji.
- **Topological naming penuh lebih dulu.** Mahal dan belum terbukti perlu;
  proyek contoh lulus edit dimensi dengan selector relasional saja.

## Konsekuensi

- Satu undo, satu format, satu jalur validasi untuk designer dan agent.
- Kualitas GUI bergantung pada kualitas pembuat selector; selector yang buruk
  menjadi error eksplisit, bukan geometri salah.
- Regenerasi penuh pada setiap edit membuat performa op (terutama `hole`,
  B0-7) langsung terasa di GUI; target Step 1b: regenerasi proyek contoh
  ≤ 200 ms.
- Assembly tetap di luar oplog sampai ADR tersendiri; klaim "GUI part replays
  headlessly" di README hanya benar untuk fitur yang sudah dimigrasi.
- Dokumen lama yang hanya punya body GUI dibuka lewat mode adopsi yang sudah
  ada (body menjadi `base_bodies`); tidak ada konversi DAG lama ke oplog
  karena DAG memang tidak pernah disimpan.
- Edit manual GUI yang belum dimigrasi masih memicu adopsi `oplog_stale`
  pada aksi agent berikutnya; celah ini tertutup per fitur sesuai urutan
  migrasi.

## Status penerapan

| Butir | Status |
|---|---|
| 4. Undo headless mencakup edit (B0-1) | ✅ `ducad-engine/src/session.rs`; tes `sample_project.rs::{param_edit_is_one_undo_step, mixed_edits_undo_and_redo_in_order, undo_after_reopen_covers_ops_and_param_edits}` |
| 4. Undo GUI membawa oplog | ✅ `ducad-app/src/oplog_undo.rs` (jembatan agent, proposal, panel Sim, `adopt_agent_model`) |
| 3. Pembuat selector | ✅ engine: `ducad-engine/src/select/suggest.rs` + `tests/select_suggest.rs`; belum dipakai GUI |
| Urutan 3: fillet/chamfer GUI | ✅ `ducad-app/src/oplog_gui.rs` (`round_via_oplog`), dipakai `fillet_selected_body`/`chamfer_selected_body`; termasuk fillet variabel, nama body kembar dibuat unik dalam langkah undo yang sama, fix terverifikasi dari op tampil di kartu error. Jalur gizmo-drag di `modeling/rounding.rs` belum dialihkan. |
| Urutan 1–2: sketsa + extrude | ✅ `extrude_via_oplog` + `sync_bound_sketches` (`ducad-app/src/oplog_gui.rs`), penerjemah `ducad-engine/src/ops/from_sketch.rs`, ikatan slot `SessionMeta::live_sketches`. Bidang Top/Front/Right; datum/face dan mode Vektor masih jalur lama. Tes `gui_sketch_extrude_fillet_then_edit_sketch_updates_downstream` menjalankan T1–T2 lewat GUI. |
| Urutan 4: hole wizard | ✅ `hole_via_oplog`: lubang baru dan edit lubang hasil op; spec yang tidak bisa dinyatakan op (counterbore custom) dan lubang lama tetap jalur lama |
| Urutan 5: boolean, pattern 3D | ✅ `boolean_via_oplog`, `pattern_via_oplog` (satu batch per aksi); seleksi body berurutan (`IndexSet`) sehingga subtract = pertama − kedua |
| Dimensi sketsa terikat param | ✅ `Sketch::dim_exprs` → `Num::Expr` di op `sketch`; replay karena param memperbarui slot GUI terikat (`adopt_replayed_session`), snapshot slot ikut di command undo oplog |
| Urutan 6: revolve | ✅ `revolve_via_oplog` lewat `run_feature_on_sketch` (helper bersama extrude); bidang sebenarnya, bukan selalu XY |
| Urutan 6: shell | ✅ komputasi tetap di thread latar (`shell_worker`); op `shell` setara (`>Z`/`<Z`… atau `suggest_faces` dari face yang dibuka) dicatat ke oplog dalam langkah undo yang sama tanpa hitung ulang. Shell variabel dan body bernama kembar tetap jalur lama |
| Urutan 6: loft | ✅ dua op `sketch` turunan (bidang aktif + offset setinggi `height`) + op `loft`, di bidang sebenarnya. Sketsa turunan tidak terikat ke slot GUI (edit sketsa tidak mengalir ke loft); profil bawah tertunda dari bidang datum tetap jalur lama |
| Urutan 6: sweep | ✅ op `sketch` turunan untuk profil + op `sweep` dengan jalur `points`; jalur berbusur dan bidang datum tetap jalur lama (titik polyline akan menghilangkan lengkung) |
| Urutan 6: draft | ✅ op `draft` untuk arah tarik sejajar sumbu; **perubahan perilaku:** bidang netral = face terbawah searah tarik (`<Z` untuk tarik +Z), bukan bidang melalui titik asal dunia. Sama untuk part yang duduk di z = 0 |
| Urutan 6: emboss | ☐ |
| Urutan 7–10 | ☐ |
| Buka berkas memulihkan oplog + ikatan sketsa | ✅ `restore_oplog_from_design` (`oplog_gui.rs`), dipanggil `open_native_path`; hanya bila geometri replay cocok (toleransi `same_geometry`) |

## Rancangan urutan 1–2: slot sketsa GUI yang terikat ke op

Masalah: op `sketch` selalu membuat slot baru di `SketchSet`. Sketsa yang
digambar pengguna sudah punya slot sendiri (dengan riwayat undo per slot dan
id entitas yang dipakai seleksi). Menjalankan op `sketch` begitu saja
menghasilkan sketsa ganda di bidang yang sama.

Rancangan:

1. **Ikatan slot.** `SessionMeta` mendapat peta `live_sketches: id op → SketchId`
   untuk slot GUI yang menjadi sumber op `sketch`. Saat op `sketch` dengan id
   terikat dijalankan di sesi live, engine memakai slot itu (tidak membuat slot
   baru, tidak menimpa isinya); frame/bidang dan pemeriksaan region tetap
   dihitung dari spec. Replay dari berkas (sesi baru, peta kosong) membangun
   slot dari spec seperti biasa.
2. **Penerjemah sketsa GUI → spec.** Entitas GUI (line, circle, arc, ellipse,
   spline, path, point) dan constraint diterjemahkan ke `EntitySpec`/
   `ConstraintSpec` dengan nama stabil (`entity_names`, atau nama turunan
   `EntityId` yang tidak berubah selama entitas hidup). Teks sketsa masuk
   sebagai path hasil vektorisasi.
3. **Extrude pertama** dari sketsa yang belum terikat: satu batch
   `[sketch, extrude]` dengan `profile: {names}` dari entitas terpilih dan
   `mode: cut` + `target` untuk smart cut. Bidang: `XY/XZ/YZ`, datum offset,
   atau face body lewat `suggest_faces`; bidang bersudut/3 titik tetap jalur
   lama sampai urutan 7.
4. **Edit sketsa terikat** (keluar dari mode sketsa dengan isi berubah):
   `replace_op` op `sketch` dengan spec baru → replay penuh → body hilir
   diperbarui. Slot GUI yang terikat tidak ditimpa hasil replay (GUI adalah
   sumber isinya); peta `sketch_ids` hasil replay dipetakan ulang ke slot
   live lewat id op.
5. **Celah yang sudah ada dan ikut tertutup:** `replace_op` agent pada op
   `sketch` di sesi live saat ini me-replay di sesi salinan dan tidak
   memperbarui slot GUI; langkah 4 memakai pemetaan yang sama.

Kriteria selesai: T1–T2 proyek contoh lewat GUI menghasilkan oplog yang
sama strukturnya dengan `sample_bracket.ops.json` (sketsa terikat, extrude,
fillet, hole), dan mengubah dimensi sketsa di GUI memperbarui fillet dan
lubang hilir tanpa `oplog_stale`.
