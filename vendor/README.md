# Vendor patches

## `opencascade-0.2.0`

Salinan lokal dari crate [`opencascade`](https://crates.io/crates/opencascade)
0.2.0 (wrapper Rust untuk kernel geometri OpenCASCADE), di-patch lewat
`[patch.crates-io]` di root `Cargo.toml` workspace.

**Kenapa di-vendor** (bukan cukup pin versi dari crates.io): `Shape::faces_along_ray`
meng-hardcode toleransi geometris `BRepIntCurveSurface_Inter` ke `0.0001` —
terlalu ketat untuk ray *oblique* (miring) dari kamera 3D perspektif nyata,
menyebabkan `pick_face_details`/`extrude_face` CADRAW selalu `None`/gagal saat
ray mengenai sisi samping (side/swept face) dari sudut manapun selain tegak
lurus persis. Root cause terverifikasi lewat 5 test terisolasi di
`cadraw-kernel` (lihat riwayat commit) — bukan bug di CADRAW, tapi di
binding upstream, dan `Shape`/`Face` di `opencascade-rs` menyimpan field
`inner` (handle FFI mentah) sebagai `pub(crate)`, jadi TIDAK ADA cara
menembus toleransi itu dari luar crate tanpa vendor.

**Perubahan** (satu-satunya, di `src/primitives/shape.rs`): tambah method
publik baru `Shape::faces_along_ray_with_tolerance(ray_start, ray_dir,
tolerance)` — badan fungsi identik dengan `faces_along_ray` asli, cuma
`tolerance` jadi parameter. `faces_along_ray` yang lama TETAP ADA APA
ADANYA, sekarang jadi wrapper tipis yang manggil versi baru dengan `0.0001`
— nol perubahan perilaku untuk pemanggil manapun yang masih pakai method
lama. Tidak ada file lain yang disentuh.

**Cara upgrade** kalau upstream `opencascade` rilis versi baru: re-copy
`src/` dari versi baru itu, terapkan ulang patch yang sama di atas
(`faces_along_ray` → wrapper tipis + `faces_along_ray_with_tolerance` baru),
update nomor versi di `[patch.crates-io]` root `Cargo.toml` & di sini.
