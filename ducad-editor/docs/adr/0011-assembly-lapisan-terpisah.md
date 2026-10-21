# ADR 0011 — Assembly sebagai lapisan dokumen terpisah dari oplog part

**Status:** diusulkan (2026-10-21). Butuh persetujuan pemilik produk;
penjaga identitas body (butir 2) sudah diterapkan karena dibutuhkan apa pun
keputusannya.

## Konteks

ADR 0010 menjadikan oplog `ducad-engine` sumber kebenaran **pemodelan part**,
dan urutan migrasi terakhirnya (10) adalah assembly, mate, coupling, dan
explode, dengan catatan "butuh desain tersendiri".

Keadaan saat ini (`docs/STATUS_KEMAMPUAN.md`):

- `AssemblyTree` (`ducad-core/src/assembly.rs`) menyimpan instance, mate,
  coupling, dan langkah explode. Solver mate ada di
  `ducad-kernel/src/assembly_solver.rs`.
- Assembly sudah tersimpan ke `.ducad` (`datums.assembly`) dan body dirujuk
  lewat **UUID** di berkas, lewat **`BodyId`** saat aplikasi berjalan.
- Tidak ada op assembly di oplog, dan MCP/agent tidak punya tool assembly.

Setelah migrasi GUI ke oplog, replay terjadi setiap kali dimensi, parameter,
sketsa, atau fitur diedit. Sebelum ADR ini, replay menukar seluruh `ModelDoc`
(`SwapModelCommand`): semua `BodyId` dan UUID body berganti, sehingga
instance assembly menunjuk body yang salah atau hilang setelah setiap edit.

## Keputusan

1. **Assembly tetap lapisan dokumen sendiri, bukan op oplog.** Alasannya:
   - Mate adalah kendala yang diselesaikan solver secara serentak, bukan
     langkah riwayat berurutan seperti fitur part. Memasukkannya ke oplog
     membuat setiap geser komponen menjadi op baru dan replay penuh part.
   - Konvensi CAD: riwayat part dan susunan assembly adalah dua dokumen/lapis
     yang berbeda. Part bisa diedit tanpa menyentuh assembly, dan sebaliknya.
   - Undo assembly sudah berjalan lewat jalurnya sendiri.
2. **Identitas body stabil terhadap replay (diterapkan).** Hasil replay
   diterapkan sebagai selisih, bukan penukaran model:
   `modeling::apply_model_preserving_ids` mengganti geometri body yang namanya
   sama di tempat (BodyId dan UUID tetap), menambah body baru, menghapus body
   yang hilang — satu transaksi undo. Karena nama body hasil op = id op yang
   unik, pencocokan lewat nama stabil.
3. **Rujukan assembly ke body tetap lewat UUID di berkas.** Body yang dihapus
   dari oplog (mis. op dihapus di Feature Tree) membuat instance-nya putus;
   GUI harus menandai instance itu, bukan diam-diam menunjuk body lain
   (pekerjaan lanjutan, lihat di bawah).

## Alternatif yang ditolak

- **Op `assembly_instance`/`mate` di oplog part.** Mencampur dua jenis
  dokumen, membuat replay part ikut menyelesaikan ulang assembly, dan
  mengubah setiap drag komponen menjadi riwayat.
- **Tetap menukar seluruh model saat replay.** Memutus assembly dan seleksi
  setiap kali dimensi diubah — tidak dapat diterima setelah migrasi.

## Konsekuensi dan pekerjaan lanjutan

- Edit dimensi part yang dipakai assembly tidak lagi memutus instance/mate
  (tes `oplog_gui::assembly_instances_survive_param_replay`,
  `swap_model::replay_keeps_body_ids_and_uuids_and_is_one_step`).
- Body baru dari replay mendapat `BodyId` baru; body yang dihapus lalu
  di-undo kembali dengan `BodyId` baru (perilaku `DeleteBodyCommand` yang
  sudah ada). Instance yang merujuk body semacam itu perlu ditandai "putus".
- Agent belum bisa membaca/mengubah assembly: tool MCP assembly (daftar
  instance, mate, solve) adalah pekerjaan terpisah bila dibutuhkan.

## Status penerapan

| Butir | Status |
|---|---|
| 2. Identitas body stabil terhadap replay | ✅ `ducad-app/src/modeling/swap_model.rs` (`apply_model_preserving_ids`), dipakai `adopt_agent_model` |
| 3. Penanda instance putus di GUI | ☐ |
| Tool MCP assembly | ☐ (bila dibutuhkan) |
