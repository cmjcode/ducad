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

**Perubahan #1** (di `src/primitives/shape.rs`): tambah method
publik baru `Shape::faces_along_ray_with_tolerance(ray_start, ray_dir,
tolerance)` — badan fungsi identik dengan `faces_along_ray` asli, cuma
`tolerance` jadi parameter. `faces_along_ray` yang lama TETAP ADA APA
ADANYA, sekarang jadi wrapper tipis yang manggil versi baru dengan `0.0001`
— nol perubahan perilaku untuk pemanggil manapun yang masih pakai method
lama.

**Perubahan #2** (CADRAW Fase 1 — deteksi tipe surface, di
`src/primitives/face.rs`): tambah method publik baru
`Face::surface_kind() -> String`, mengembalikan nama kelas dinamis C++/OCCT
dari permukaan geometris di balik face (mis. `"Geom_Plane"`,
`"Geom_CylindricalSurface"`, `"Geom_SphericalSurface"`, dst). Disusun
murni dari binding FFI yang SUDAH ADA di `opencascade-sys` 0.2.0
(`BRep_Tool_Surface`, `DynamicType`, `type_name`) — tidak ada perubahan di
`opencascade-sys` sama sekali. `cadraw-kernel::SurfaceKind` mem-parse
string ini jadi enum.

**Perubahan #3** (fixture test utk Perubahan #2, di `src/adhoc.rs`): tambah
method publik baru `AdHocShape::make_sphere(r) -> Self`, dipasangkan
dengan `BRepPrimAPI_MakeSphere_ctor`/`Shape` yang SUDAH ADA di FFI
upstream tapi belum ada wrapper Rust publiknya (beda dengan `make_box`/
`make_cylinder` yang sudah ada sebelumnya). Tidak ada perubahan di
`opencascade-sys`.

**Perubahan #4** (CADRAW Fase 3 — `extrude_face` dispatch per tipe
permukaan, di `src/primitives/shape.rs`): tambah dua method publik baru:
- `Shape::volume() -> f64` — `BRepGProp_VolumeProperties`/`Mass()`, FFI-nya
  SUDAH ADA (dipakai internal sebelumnya), cuma belum ada wrapper publik.
  Dipakai `cadraw-kernel` utk regresi volume `extrude_face` jalur offset.
- `Shape::offset_on_face(face, offset) -> Result<Self, Error>` — menyusun
  binding `BRepOffset_MakeOffset` yang ditambahkan ke `opencascade-sys` di
  Fase 2 (base offset `0.0` utk semua face, `SetOffsetOnFace(face, offset)`
  cuma utk face terpilih, mode `BRepOffset_Skin` + `intersection = true` +
  join `GeomAbs_Intersection` biar sambungan ke face tetangga yang tidak
  bergerak tetap rapi). TIDAK ada perubahan lagi di `opencascade-sys` di
  fase ini — murni pemakaian FFI Fase 2 yang sebelumnya belum dipakai
  siapa pun (lihat catatan "Sengaja belum ada" di `docs/PLAN.md` § Fase 8
  Lanjutan 3).

**Perubahan #5** (CADRAW Fase 3, di `src/primitives/face.rs`): tambah
method publik baru `Face::cylinder_or_cone_radius() -> Option<f64>` —
coba `BRepAdaptor_Surface_cylinder`/`_cone` (Fase 2) berantai, `None` kalau
bukan salah satu dari keduanya. Dipakai `cadraw-kernel::extrude_face` utk
validasi batas (`radius + distance > 0`) SEBELUM memanggil
`offset_on_face`, supaya offset yang membuat radius ≤ 0 ditolak jelas lebih
awal. Sengaja TIDAK menutupi Sphere/Torus — `opencascade-sys` belum punya
binding `gp_Sphere`/`gp_Torus` (di luar cakupan Fase 2); utk tipe itu
`cadraw-kernel` mengandalkan `IsDone()`/`Result::Err` dari OCCT sendiri di
`offset_on_face`.

**Perubahan #6** (CADRAW Fase 3, di `src/lib.rs`): tambah varian baru
`Error::OffsetOnFaceFailed(String)`, dipakai `offset_on_face` (Perubahan
#4) membungkus pesan `cxx::Exception` dari kegagalan
`Initialize`/`SetOffsetOnFace`/`MakeOffsetShape`/`Shape` milik
`BRepOffset_MakeOffset`.

**Perubahan #7** (CADRAW Fase 4 — arah gizmo `pull_dir` radial, di
`src/primitives/face.rs`): tambah method publik baru
`Face::cylinder_or_cone_axis() -> Option<(DVec3, DVec3)>` (titik acuan +
arah satuan sumbu) — pola identik Perubahan #5, cuma menyusun ulang binding
`gp_Cylinder_location`/`_direction`/`gp_Cone_location`/`_direction` yang
SUDAH ADA di `opencascade-sys` sejak Fase 2 (dulu cuma dipakai
`cylinder_or_cone_radius` utk radius, arah/lokasinya belum pernah dipakai
sampai fase ini). Dipakai `cadraw-kernel::compute_pull_dir` menghitung arah
radial gizmo di titik hit: proyeksikan `hit_point` ke garis sumbu, ambil
vektor `(hit − proyeksi)`. TIDAK ada perubahan di `opencascade-sys` di fase
ini — sphere pakai `Face::center_of_mass()` (SUDAH ADA sejak awal) sbg pusat
bola, bukan binding `gp_Sphere` baru (tetap belum ada, di luar cakupan Fase
4 juga — cukup krn `center_of_mass()` GProp-based SECARA MATEMATIS persis
pusat bola utk bola penuh/simetris).

Tidak ada file lain yang disentuh di tujuh perubahan di atas.

**Cara upgrade** kalau upstream `opencascade` rilis versi baru: re-copy
`src/` dari versi baru itu, terapkan ulang ketujuh patch di atas
(`faces_along_ray` → wrapper tipis + `faces_along_ray_with_tolerance` baru;
`Face::surface_kind` baru; `AdHocShape::make_sphere` baru;
`Shape::volume`/`Shape::offset_on_face` baru; `Face::cylinder_or_cone_radius`
baru; `Error::OffsetOnFaceFailed` baru; `Face::cylinder_or_cone_axis` baru),
update nomor versi di `[patch.crates-io]` root `Cargo.toml` & di sini.

## `opencascade-sys-0.2.0`

Salinan lokal dari crate [`opencascade-sys`](https://crates.io/crates/opencascade-sys)
0.2.0 (binding cxx mentah ke kernel OpenCASCADE — satu lapis DI BAWAH
`opencascade-0.2.0` di atas), di-patch lewat `[patch.crates-io]` yang sama
di root `Cargo.toml` workspace. `opencascade-0.2.0` (vendored di atas)
otomatis memakai copy lokal ini juga — dependensinya `opencascade-sys = "0.2"`
tidak diubah, resolusi `[patch.crates-io]` yang mengarahkannya.

**Kenapa di-vendor** (CADRAW Fase 2 — arah gizmo radial silinder/kerucut +
offset shell per-face): fitur ini butuh dua kelas OCCT yang belum ada
binding-nya sama sekali di `opencascade-sys` 0.2.0 upstream —
`BRepAdaptor_Surface` (deteksi tipe surface + ekstraksi `gp_Cylinder`/
`gp_Cone`) dan `BRepOffset_MakeOffset` (offset shell per-face). Beda dengan
patch `opencascade-0.2.0` di atas (yang cukup nambah method Rust baru di atas
FFI yang SUDAH ADA), ini butuh nambah binding cxx BARU — hanya bisa
dilakukan di `wrapper.hxx` + cxx bridge (`src/lib.rs`) milik
`opencascade-sys` sendiri, jadi crate ini juga harus di-vendor terpisah.

**Perubahan #1** (`include/wrapper.hxx` + `src/lib.rs`): tambah binding
`BRepAdaptor_Surface`:
- ctor dari `TopoDS_Face` (`BRepAdaptor_Surface_ctor`),
- `GetType()` → enum `GeomAbs_SurfaceType` (Plane/Cylinder/Cone/…, dipetakan
  1:1 ke `enum GeomAbs_SurfaceType` OCCT asli lewat `type` alias, BUKAN
  `enum class` baru — lihat catatan pola di bawah),
- `BRepAdaptor_Surface_cylinder`/`BRepAdaptor_Surface_cone` → `Result<UniquePtr<gp_Cylinder>>`/`Result<UniquePtr<gp_Cone>>`,
- accessor `gp_Cylinder`/`gp_Cone` baru (`gp_Cylinder_location`/`_direction`/`_radius`,
  `gp_Cone_location`/`_direction`/`_radius`/`_semi_angle`) — titik axis, arah
  axis, radius, semi-angle, dipakai utk arah gizmo radial & validasi tipe
  surface di CADRAW.

**Perubahan #2** (`include/wrapper.hxx` + `src/lib.rs`): tambah binding
`BRepOffset_MakeOffset` (offset shell per-face, mode default `BRepOffset_Skin`,
join `GeomAbs_Intersection`): ctor, `Initialize(shape, offset, tol, mode,
intersection, self_inter, join, thickening, remove_int_edges)`,
`SetOffsetOnFace(face, offset)`, `MakeOffsetShape()`, `IsDone()`, `Shape()`.

**Pola penting (dipakai di kedua perubahan di atas): Standard_Failure OCCT
BUKAN turunan `std::exception`** — dia turunan `Standard_Transient`
(diverifikasi langsung dari header OCCT vendored,
`occt-sys-0.2.0/OCCT/src/Standard/Standard_Failure.hxx`). Mekanisme cxx yang
otomatis menerjemahkan exception C++ jadi `Result::Err` cuma nangkep
`std::exception`, jadi kalau OCCT melempar `Standard_Failure` (mis.
`GeomAdaptor_Surface::Cylinder()` dipanggil di surface yang bukan silinder)
lewat fungsi `Result<>` biasa, exception itu TEMBUS dan memicu
`std::terminate` — abort seluruh proses, bukan error Rust yang rapi. Semua
fungsi baru yang bisa gagal secara geometris di sisi OCCT karena itu
dibungkus manual `try { ... } catch (const Standard_Failure &e) { throw
std::runtime_error(...); }` di `wrapper.hxx`
(`rethrow_standard_failure_as_runtime_error`, `[[noreturn]]`) SEBELUM
dideklarasikan `Result<>` di cxx bridge — pola yang sama dengan
`handle_try_deref` yang sudah ada di file ini sebelumnya. Dibuktikan lewat
test `vendor/opencascade-sys-0.2.0/tests/surface_and_offset.rs` (mis.
`plane_face_reports_plane_type_and_rejects_cylinder_accessor`): tanpa
wrapper ini, test itu akan meng-crash seluruh test binary, bukan cuma
mengembalikan `Err`.

**Pola kedua yang perlu diketahui**: enum `GeomAbs_SurfaceType`,
`BRepOffset_Mode`, `GeomAbs_JoinType` yang ditambahkan di `src/lib.rs`
SEKALIGUS didefinisikan `#[repr(u32)] pub enum ...` di luar blok `unsafe
extern "C++" { ... }` (biar dipakai jadi tipe Rust biasa) DAN
dideklarasikan ulang `type GeomAbs_SurfaceType;` dst DI DALAM blok itu.
Deklarasi `type` kedua ini WAJIB kalau header OCCT asli yang mendefinisikan
enum yang sama (mis. `<GeomAbs_SurfaceType.hxx>`) sudah di-`#include` di
`wrapper.hxx` — tanpanya, cxx mencoba mendefinisikan `enum class
GeomAbs_SurfaceType` versinya sendiri dan bentrok kompilasi C++
("enumeration previously declared as unscoped") dengan `enum
GeomAbs_SurfaceType` OCCT yang sudah ada. `TopAbs_ShapeEnum`/
`BOPAlgo_GlueEnum`/`IFSelect_ReturnStatus` yang sudah ada sebelumnya di file
ini memakai pola dobel-deklarasi yang sama — cuma sekarang didokumentasikan
eksplisit karena baru ketauan pas nambah enum baru pertama kalinya sejak
vendor ini dibuat.

Tidak ada perubahan lain di `opencascade-sys-0.2.0` selain dua binding baru
di atas — `build.rs`, `Cargo.toml`, `examples/`, `tests/triangulation.rs`
upstream tidak disentuh (`tests/surface_and_offset.rs` adalah file test
BARU, bukan modifikasi).

**Cara upgrade** kalau upstream `opencascade-sys` rilis versi baru: re-copy
`include/wrapper.hxx` + `src/lib.rs` dari versi baru itu, terapkan ulang
kedua perubahan di atas (binding `BRepAdaptor_Surface` +
`BRepOffset_MakeOffset`, termasuk pola `Standard_Failure`→`Result` dan pola
dobel-deklarasi enum), pastikan `tests/surface_and_offset.rs` masih lolos,
update nomor versi di `[patch.crates-io]` root `Cargo.toml` & di sini.
