# Format berkas `.ducad`

Satu berkas JSON (UTF-8) berversi. Kode: [`crates/ducad-io/src/native.rs`](../crates/ducad-io/src/native.rs)
(struktur `DuCADFile`), oplog parametrik: [`crates/ducad-engine/src/session.rs`](../crates/ducad-engine/src/session.rs)
(`DesignDoc`), migrasi: [`crates/ducad-engine/src/migrate.rs`](../crates/ducad-engine/src/migrate.rs).

## Versi

| `format_version` | Sejak | Isi baru | Ditulis bila |
|---|---|---|---|
| 1 | Fase 5 | `sketch`, `front_sketch`, `right_sketch`, `bodies[]` (STEP per body) | tidak ada fitur di bawah |
| 2 | P1.6 | `design` (oplog `ducad-engine`: `params`, `oplog`, `checks`, `base_bodies`, `fingerprint`, …) | `design` terisi |
| 3 | vektor | `Entity::Path`, `styles`, `layers`, `groups`, `texts`, `origin`; `ink` | dokumen memakai fitur vektor/tinta |
| **4** | **0.6 (migrasi kernel F1)** | `bodies[].step` **opsional**, `bodies[].brep` (cache) | ada body turunan oplog atau cache |

Penulis selalu memilih versi **terendah** yang cukup, supaya build lama tetap bisa membuka berkas
yang tidak memakai fitur baru. Pembaca menerima semua versi ≤ versinya dan **menolak** versi yang
lebih baru (lebih aman daripada salah baca diam-diam). DUCAD ≤ 0.5.6 mengenal sampai v3, jadi
menolak berkas v4 dengan pesan "versi format".

## Body (`bodies[]`)

```jsonc
{
  "name": "bracket",          // = id op pembuatnya untuk part parametrik
  "uuid": "…",                // identitas stabil (referensi part eksternal)
  "visible": true,
  "material": { … },          // PBR
  "mechanical": { … },        // opsional, material mekanik (P16)
  "step": "ISO-10303-21; …",  // opsional sejak v4 — STEP AP214 otoritatif
  "brep": {                    // opsional sejak v4 — cache non-otoritatif
    "producer": "ducad-occt/1",
    "data": "<base64(zlib(bytes))>"
  },
  "round_history": { … }      // opsional, riwayat fillet/chamfer GUI
}
```

### Sumber kebenaran geometri (v4)

Oplog `design` adalah sumber kebenaran. STEP hanya dipertahankan untuk body yang **tidak** punya
oplog. Ada dua jenis body:

| Jenis | `step` | `brep` | Siapa yang menulis |
|---|---|---|---|
| **Body STEP** (GUI, impor, berkas lama) | ada, otoritatif | tidak ada | GUI (`ducad-app`), autosave, snapshot riwayat |
| **Body turunan oplog** | tidak ada | ada (cache) | `Session::save` (MCP `save_part`, `ducad-cli run/replay --out`), `ducad-cli migrate-file` |

Body dasar parametrik (`design.base_bodies[]`, misalnya hasil `import_step`) **selalu** membawa STEP
sendiri. Cache backend tidak pernah jadi satu-satunya salinan geometri yang tidak bisa dibangun ulang.

### Cache `brep`

* `producer` mengidentifikasi backend kernel + versi encoding (`KernelShape::NATIVE_CACHE_PRODUCER`):
  `ducad-occt/1` = BREP biner OCCT (`BinTools`); `ducad-truck/1` = `encode_native_body` SolidCraft.
* `data` = base64 standar dari aliran zlib. Batas dekompresi 512 MB.
* Produsen beda = *miss* (bukan error): geometri dibangun ulang lewat replay. Cache rusak = peringatan,
  lalu jatuh ke STEP/replay.
* Cache boleh dihapus kapan saja; memuat tanpa cache memberi geometri yang sama (diuji:
  `v4_load_without_cache_equals_load_with_cache`).

### Ringkasan geometri `design.geometry` (v4, PLAN_MIGRASI_TRUCK F3)

`design.fingerprint` adalah hash FNV atas volume (6 desimal), bbox mesh (4 desimal) dan jumlah face
tiap body — tidak pernah cocok persis bila berkas dibuka backend kernel LAIN (volume eksak OCCT vs
tesselasi truck). `Session::save` karena itu juga menulis ringkasan per body:

```json
"geometry": [ { "name": "bracket", "volume": 18230.51, "bbox": [[0,0,0],[60,40,12]] } ]
```

Saat replay, sidik jari yang berbeda masih diterima bila ringkasan cocok dalam toleransi lintas-backend
(nama sama, volume relatif ±1e-3, bbox ±(0,05 mm + 1e-3·diagonal)). Berkas tanpa `geometry` (v1–v3,
v4 lama): replay dibandingkan dengan body STEP berkas memakai toleransi yang sama. Field opsional —
pembaca lama mengabaikannya.

## Aturan memuat

**Engine (`Session::from_file`, MCP, CLI)** — berkas dengan `design`:

1. Ada body tanpa `step` (v4) → replay oplog. Bila replay gagal / sidik jari beda:
   non-strict → body diambil dari cache (mode adopsi, peringatan `oplog_stale` + `replay_failed: …`,
   body jadi `base_bodies` dengan STEP baru); strict (`ducad-cli build`) → error.
   Tanpa cache dan replay gagal → error; body tidak pernah hilang diam-diam.
2. Semua body membawa STEP (v1–v3) → STEP dibandingkan dengan sidik jari `design`; sama → replay;
   beda → replay tetap dicoba dan dipakai bila body-nya cocok dengan body STEP dalam toleransi
   (berkas dari backend lain), selain itu adopsi (`oplog_stale`). **STEP rusak tidak fatal** selama oplog masih bisa
   di-replay (peringatan `step_unreadable: …`).

Berkas tanpa `design` → semua body diadopsi sebagai `base_bodies`.

**GUI (`ducad-app`, `ducad_io::native::load`)** — geometri diambil dari cache yang cocok, lalu STEP.
Bila masih ada body tanpa geometri (`LoadedDocument::needs_replay`), app me-replay `design` lewat
`ducad-engine` sebelum membuang dokumen yang sedang terbuka. STEP rusak di berkas ber-`design` jadi
peringatan + replay; di berkas tanpa `design` tetap error.

**Part eksternal** — `add_external_part` (app) sama dengan GUI. `ducad_io::external::load_external_part`
(muat ulang referensi) tidak bisa me-replay karena `ducad-io` tidak bergantung pada `ducad-engine`;
ia memakai cache/STEP dan memberi error jelas bila keduanya tidak tersedia untuk backend ini.

## Migrasi berkas lama

```bash
# Laporan saja (dry run)
cargo run -p ducad-cli -- migrate-file part.ducad
# Tulis ke berkas lain
cargo run -p ducad-cli -- migrate-file part.ducad --out part.v4.ducad
# Timpa di tempat, salinan lama → part.ducad.bak
cargo run -p ducad-cli -- migrate-file a.ducad b.ducad --in-place
```

Sebuah body dimigrasi (STEP → cache) hanya bila: berkas punya `design`, replay **strict** berhasil
tanpa peringatan (tidak basi), body itu dihasilkan oplog, dan volumenya sama dengan STEP berkas
(toleransi relatif 1e-6 di backend OCCT, 1e-4 di truck — volume tesselasi). Hasil diverifikasi sebelum ditulis: replay ulang harus memberi sidik jari
yang sama, dan pemuat GUI harus bisa membaca semua body dari cache. Berkas yang tidak memenuhi
syarat dibiarkan byte-demi-byte (`changed: false`, alasan di `skipped` / `kept_step[].reason`:
`no_oplog`, `replay_failed: …`, `not_replayable: oplog_stale`, `not_produced_by_oplog`,
`volume_mismatch: …`). Migrasi idempoten. Kode keluar 1 bila ada berkas yang gagal.

Untuk menyimpan part parametrik yang masih harus dibuka DUCAD ≤ 0.5.6, pakai
`Session::save_legacy` (v2, STEP di setiap body).

## Ukuran

Contoh `examples/industrial-demo/mounting_bracket.ducad` (6 body parametrik): v2 224 KB → v4
155 KB. Tanpa kompresi zlib, BREP biner OCCT dalam base64 justru 4,3× lebih besar dari STEP.
