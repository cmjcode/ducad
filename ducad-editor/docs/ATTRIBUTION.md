# Atribusi kode pihak ketiga

Kode pihak ketiga yang DISALIN ke repositori ini (bukan sekadar dependensi
crates.io di `Cargo.lock`). Dependensi biasa diaudit `cargo deny check`
(`deny.toml`).

## Vendored

| Path | Asal | Lisensi | Diubah DUCAD? |
|---|---|---|---|
| `vendors/solidcraft/crates/kernel`, `vendors/solidcraft/crates/geom` | SolidCraft 0.16.2 `c66e332`, https://github.com/tangency-labs/solidcraft | MIT OR Apache-2.0 | Ya — manifest ditulis ulang, `ducad_ext.rs` ditambah (lihat `vendors/solidcraft/README.md`, marker `DUCAD:`). |
| `vendors/solidcraft/vendor/truck-{geometry,meshalgo,shapeops,stepio}` | truck, © RICOS Co. Ltd., https://github.com/ricosjp/truck, dengan perbaikan SolidCraft | Apache-2.0 | Tidak oleh DUCAD; perubahan SolidCraft tercatat di `vendors/solidcraft/vendor/README.md` (marker `SolidCraft:`). |
| `vendors/solidcraft/vendor/matext4cgmath` | matext4cgmath 0.1.0, © RICOS Co. Ltd. | Apache-2.0 | Tidak oleh DUCAD (perbaikan SolidCraft: batas 100 langkah Newton). |

Teks lisensi dan berkas `NOTICE` SolidCraft ikut di `vendors/solidcraft/`
(`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`) sesuai Apache-2.0 §4.

## Saat mendistribusikan biner

Setiap biner DUCAD memuat kode SolidCraft dan truck: sertakan
`vendors/solidcraft/NOTICE` beserta teks Apache-2.0 di paket/“Tentang”
aplikasi.
