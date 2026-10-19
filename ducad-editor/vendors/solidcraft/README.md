# SolidCraft kernel (vendored)

Backend kernel `kernel-truck` DUCAD (PLAN_MIGRASI_TRUCK F2). Salinan sebagian
repositori SolidCraft — bukan submodule — supaya build tidak bergantung pada
upstream yang sedang berpindah ke kernel baru (risiko R8 di plan).

| | |
|---|---|
| Upstream | https://github.com/tangency-labs/solidcraft (sebelumnya `bherbruck/solidcraft`) |
| Versi | 0.16.2, commit `c66e33284bebf07a01cc81cef8323320de58834e` (2026-10-18) |
| Lisensi | MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`) |
| truck | crate `truck-*` RICOS Co. Ltd., Apache-2.0; yang ditambal SolidCraft ada di `vendor/` |

## Isi

| Path | Dari upstream | Catatan |
|---|---|---|
| `crates/kernel` | `crates/kernel` | `solidcraft-kernel`: B-rep, boolean, fillet, STEP, … |
| `crates/geom` | `crates/geom` | `solidcraft-geom`: Vec3, Plane, Region2, Mesh, BVH |
| `vendor/truck-{geometry,meshalgo,shapeops,stepio}`, `vendor/matext4cgmath` | `vendor/` | truck ber-patch SolidCraft; daftar perubahan di `vendor/README.md`, baris berubah bertanda `SolidCraft:` |
| `LICENSE-*`, `NOTICE` | akar repo | wajib ikut (Apache-2.0 §4) |

Tidak diambil: `crates/{doc,engine,ui-egui,render,gpu,io,mcp,sketch,…}`,
`apps/`, `vendor/egui-wgpu` — DUCAD punya padanannya sendiri.

## Perubahan DUCAD (Apache-2.0 §4(b))

1. `crates/kernel/Cargo.toml`, `crates/geom/Cargo.toml` — ditulis ulang:
   nilai `workspace = true` diganti nilai eksplisit dari `Cargo.toml` workspace
   SolidCraft (versi dependensi, edition 2024, `unsafe_code = "forbid"`),
   karena crate ini berada di luar workspace aslinya. Ditandai komentar `DUCAD:`.
2. `crates/kernel/src/ducad_ext.rs` — BERKAS BARU: kueri topologi read-only
   (`face_wires`, `edge_ends`, `vertex_points`) yang tidak ditawarkan API
   SolidCraft; dipakai lapisan topologi logis DUCAD
   (`ducad-kernel/src/backend/truck/topology.rs`).
3. `crates/kernel/src/lib.rs` — satu baris `pub mod ducad_ext;` (bertanda `DUCAD:`).

Kode SolidCraft lain tidak diubah.

## Integrasi

- `ducad-editor/Cargo.toml`: `exclude = ["vendors/solidcraft"]` (bukan anggota
  workspace — tes & lint pihak ketiga tidak ikut `--workspace`), blok
  `[patch.crates-io]` untuk `truck-*`/`matext4cgmath` (WAJIB: tanpa itu
  boolean tidak deterministik dan bisa panic), dan `debug-assertions = false`
  untuk crate truck/SolidCraft di profil dev (assert debug truck menolak
  geometri sah; SolidCraft melakukan hal yang sama di profil tesnya).
- `ducad-kernel` memakainya hanya dengan fitur `kernel-truck`.

## Memperbarui

```bash
git clone https://github.com/tangency-labs/solidcraft /tmp/sc && git -C /tmp/sc checkout <sha>
rsync -a --delete --exclude target /tmp/sc/crates/kernel/ vendors/solidcraft/crates/kernel/
rsync -a --delete --exclude target /tmp/sc/crates/geom/   vendors/solidcraft/crates/geom/
# vendor/truck-* dan matext4cgmath dengan cara yang sama, lalu:
#  - pasang lagi perubahan DUCAD 1–3 di atas (Cargo.toml eksplisit, ducad_ext.rs, baris lib.rs)
#  - samakan versi dependensi di Cargo.toml dengan workspace SolidCraft yang baru
#  - perbarui SHA di tabel atas
cargo test -p ducad-kernel --no-default-features --features kernel-truck
cargo run -p ducad-cli --features kernel-truck -- golden check --against tests/goldens/occt \
  --ops sketch,primitive,extrude,revolve,boolean,transform,scale,delete,set_material,study
```
