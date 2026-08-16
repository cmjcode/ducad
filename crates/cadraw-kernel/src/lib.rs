//! Wrapper CADRAW di atas kernel OpenCASCADE (via opencascade-rs).
//!
//! Seluruh aplikasi hanya boleh menyentuh tipe dari crate ini, bukan
//! `opencascade` langsung — agar detail FFI terisolasi dan kernel bisa
//! ditambal/diganti tanpa merombak app. `Shape` OCCT sengaja tidak pernah
//! `pub`: [`KernelShape`] membungkusnya sepenuhnya.

use anyhow::{bail, Context, Result};
use glam::{dvec3, DVec3};
use opencascade::adhoc::AdHocShape;
use opencascade::angle::Angle;
use opencascade::primitives::{Compound, Direction as OcctDirection, Edge, Face, IntoShape, Shape, Solid, Wire};
use opencascade::workplane::Workplane;
use std::sync::Mutex;

/// Kunci global memaksa SEMUA pemanggilan ke kernel OCCT serial lintas
/// thread mana pun. OCCT (setidaknya jalur transfer STEP — dibuktikan
/// lewat crash `SIGABRT`/`Interface_InterfaceError` di test suite Fase 3,
/// lihat `tests::TEST_LOCK`) TIDAK thread-safe untuk dipanggil BERSAMAAN
/// dari >1 thread; juga dikonfirmasi lewat compile-time check bahwa
/// `KernelShape` (dan `opencascade::Shape` di baliknya) TIDAK `Send` —
/// `UniquePtr<TopoDS_Shape>` milik `cxx` tidak pernah diberi `unsafe impl
/// Send` di binding ini, konsisten dengan OCCT yang memang tidak
/// thread-safe. Sampai Fase 6, ini bukan masalah nyata karena
/// `cadraw-app` cuma pernah memanggil kernel dari UI thread tunggal.
/// Fase 7 menambah `import_worker` di `cadraw-app` (thread latar belakang
/// untuk Import STEP, supaya UI tidak beku selama tessellation shape
/// besar) — begitu ADA thread kedua yang bisa memanggil kernel, lock ini
/// jadi wajib di setiap fungsi publik supaya tidak pernah ada 2
/// panggilan OCCT jalan bersamaan, apa pun urutan aksi user (mis. klik
/// Extrude persis saat import STEP di latar belakang masih berjalan).
/// Dipegang HANYA di fungsi publik, bukan di helper privat seperti
/// `deep_clone`/`tessellate_shape` yang selalu dipanggil dari dalam
/// fungsi publik yang sudah memegang lock — `Mutex` std TIDAK reentrant,
/// mengunci dua kali dari thread yang sama akan deadlock.
static KERNEL_LOCK: Mutex<()> = Mutex::new(());

/// Kunci `KERNEL_LOCK`, pulih dari poisoning (panic sebelumnya sambil
/// memegang lock) alih-alih ikut panic — satu operasi kernel yang gagal
/// tidak seharusnya mengunci permanen SEMUA operasi kernel berikutnya di
/// aplikasi UI yang harus tetap responsif.
fn lock_kernel() -> std::sync::MutexGuard<'static, ()> {
    KERNEL_LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Mesh hasil tessellation, siap di-upload ke GPU (f32, indexed).
#[derive(Debug, Clone, Default)]
pub struct KernelMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl KernelMesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Gabungkan beberapa mesh jadi satu buffer, menggeser indeks per mesh
    /// supaya tetap valid. Dipakai render (satu draw call untuk semua body
    /// visible) dan export STL/OBJ multi-body (Fase 5, `cadraw-io`) — dua
    /// pemakai yang sebelumnya menduplikasi logika gabung-mesh ini sendiri.
    pub fn merge(meshes: &[&KernelMesh]) -> KernelMesh {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();
        for mesh in meshes {
            let offset = positions.len() as u32;
            positions.extend_from_slice(&mesh.positions);
            normals.extend_from_slice(&mesh.normals);
            indices.extend(mesh.indices.iter().map(|i| i + offset));
        }
        KernelMesh {
            positions,
            normals,
            indices,
        }
    }
}

fn tessellate_shape(shape: &Shape) -> KernelMesh {
    let mesh = shape.mesh();
    let positions = mesh
        .vertices
        .iter()
        .map(|v| [v.x as f32, v.y as f32, v.z as f32])
        .collect();
    let normals = mesh
        .normals
        .iter()
        .map(|n| [n.x as f32, n.y as f32, n.z as f32])
        .collect();
    let indices = mesh.indices.iter().map(|i| *i as u32).collect();
    KernelMesh {
        positions,
        normals,
        indices,
    }
}

/// Solid/shape B-rep OCCT. Field dalam sengaja privat — pemanggil di luar
/// crate ini hanya boleh membangunnya lewat fungsi di modul ini
/// (`extrude_profile`, `union`, `subtract`, dst) dan membacanya lewat
/// `tessellate`/`write_stl`, tidak pernah menyentuh tipe `opencascade`
/// langsung.
pub struct KernelShape(Shape);

impl KernelShape {
    pub fn tessellate(&self) -> KernelMesh {
        let _guard = lock_kernel();
        tessellate_shape(&self.0)
    }

    pub fn write_stl(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let _guard = lock_kernel();
        self.0.write_stl(path)?;
        Ok(())
    }

    pub fn write_step(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let _guard = lock_kernel();
        self.0.write_step(path)?;
        Ok(())
    }

    /// Baca shape B-rep dari file STEP — kebalikan `write_step`. Dipakai
    /// Import STEP (Fase 5, `cadraw-io`; Fase 7, `cadraw-app::import_worker`)
    /// dan test/`deep_clone` internal.
    pub fn read_step(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let _guard = lock_kernel();
        Ok(KernelShape(Shape::read_step(path).context("read_step: gagal membaca STEP")?))
    }

    /// Serialize B-rep ini jadi teks STEP AP214 (bukan mesh — topologi+
    /// geometri persis, sama presisi dengan file `.step` biasa). Dipakai
    /// `cadraw-io` (Fase 5) untuk menyematkan body ke dalam SATU file
    /// native `.cadraw` tanpa pernah menyentuh tipe `opencascade` — cuma
    /// String, sama seperti `KernelMesh` membungkus mesh sebagai `[f32;3]`
    /// mentah. Roundtrip lewat file sementara (sama trik dengan
    /// `deep_clone`) karena binding ini tidak expose serialisasi in-memory.
    pub fn to_step_string(&self) -> Result<String> {
        let _guard = lock_kernel();
        let path = temp_step_path("to-step-string");
        let result = (|| -> Result<String> {
            self.0
                .write_step(&path)
                .context("to_step_string: gagal menulis STEP sementara")?;
            std::fs::read_to_string(&path).context("to_step_string: gagal membaca balik STEP sementara")
        })();
        let _ = std::fs::remove_file(&path);
        result
    }

    /// Kebalikan `to_step_string`.
    pub fn from_step_string(step: &str) -> Result<Self> {
        let _guard = lock_kernel();
        let path = temp_step_path("from-step-string");
        let result = (|| -> Result<Shape> {
            std::fs::write(&path, step).context("from_step_string: gagal menulis STEP sementara")?;
            Shape::read_step(&path).context("from_step_string: gagal membaca balik STEP sementara")
        })();
        let _ = std::fs::remove_file(&path);
        result.map(KernelShape)
    }
}

/// Path file sementara unik (PID + timestamp nanosecond, sama pola dengan
/// yang dipakai `deep_clone` sebelumnya) — dipusatkan di sini supaya
/// `deep_clone`/`to_step_string`/`from_step_string` tidak menduplikasi
/// logika pembuatan nama file.
fn temp_step_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "cadraw-{tag}-{}-{}.step",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

/// `opencascade-rs` (0.2.0) tidak menyediakan `Clone` untuk `Shape` — objek
/// C++ di baliknya cuma dibungkus `UniquePtr` tanpa binding copy-constructor.
/// Satu-satunya cara publik untuk menyalin B-rep persis (bukan cuma
/// referensi) adalah roundtrip lewat file STEP (format yang menyimpan
/// topologi+geometri B-rep tepat, bukan mesh tessellation). Dipakai HANYA
/// oleh operasi yang secara internal memutasi shape di tempat (fillet/
/// chamfer) atau mengonsumsi kepemilikan (`hollow`), supaya shape ASLI
/// milik pemanggil tetap utuh untuk keperluan undo — bukan technical debt,
/// keputusan sadar mengingat batasan binding versi ini.
fn deep_clone(shape: &Shape) -> Result<Shape> {
    let path = temp_step_path("deep-clone");
    shape.write_step(&path).context("deep_clone: gagal menulis STEP sementara")?;
    let result = Shape::read_step(&path).context("deep_clone: gagal membaca balik STEP sementara");
    let _ = std::fs::remove_file(&path);
    result
}

/// Satu segmen loop profil 2D di bidang XY, dalam koordinat mentah (mm) —
/// bukan `glam::DVec2` supaya tidak membocorkan versi glam manapun ke
/// pemanggil (crate ini sengaja pin glam 0.23, lihat `Cargo.toml`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfileSegment {
    Line { start: (f64, f64), end: (f64, f64) },
    /// Busur 3 titik: awal, titik-di-busur (menentukan sisi), akhir — sama
    /// konvensi dengan `cadraw_sketch::arc_from_three_points`.
    Arc {
        start: (f64, f64),
        via: (f64, f64),
        end: (f64, f64),
    },
}

/// Profil 2D tertutup di bidang XY, siap di-extrude/revolve. Dibangun
/// pemanggil (biasanya `cadraw-app` dari seleksi entitas sketch).
#[derive(Debug, Clone)]
pub enum Profile {
    /// Lingkaran penuh — jadi silinder saat di-extrude.
    Circle { center: (f64, f64), radius: f64 },
    /// Loop tertutup segmen Line/Arc; segmen harus sudah berurutan
    /// end-to-end kembali ke titik awal (verifikasi kontinuitas jadi
    /// tanggung jawab pemanggil — lihat pembangun chain di `cadraw-app`).
    Loop(Vec<ProfileSegment>),
}

fn build_wire(profile: &Profile) -> Result<Wire> {
    build_wire_at_z(profile, 0.0)
}

fn build_wire_on_plane(
    profile: &Profile,
    origin: [f64; 3],
    u_axis: [f64; 3],
    v_axis: [f64; 3],
    normal: [f64; 3],
) -> Result<Wire> {
    let to_3d = |p: (f64, f64)| -> glam::DVec3 {
        dvec3(
            origin[0] + u_axis[0] * p.0 + v_axis[0] * p.1,
            origin[1] + u_axis[1] * p.0 + v_axis[1] * p.1,
            origin[2] + u_axis[2] * p.0 + v_axis[2] * p.1,
        )
    };
    let norm = dvec3(normal[0], normal[1], normal[2]).normalize();

    match profile {
        Profile::Circle { center, radius } => {
            if *radius <= 0.0 {
                bail!("radius lingkaran profil harus > 0");
            }
            let c3 = to_3d(*center);
            let edge = Edge::circle(c3, norm, *radius);
            Ok(Wire::from_edges([&edge]))
        }
        Profile::Loop(segments) => {
            if segments.is_empty() {
                bail!("profil loop kosong");
            }
            let edges: Vec<Edge> = segments
                .iter()
                .map(|s| match s {
                    ProfileSegment::Line { start, end } => {
                        Edge::segment(to_3d(*start), to_3d(*end))
                    }
                    ProfileSegment::Arc { start, via, end } => {
                        Edge::arc(to_3d(*start), to_3d(*via), to_3d(*end))
                    }
                })
                .collect();
            Ok(Wire::from_edges(edges.iter()))
        }
    }
}

/// Sama seperti `build_wire`, tapi diangkat ke ketinggian `z` — dipakai
/// `loft_profiles` untuk menempatkan profil ATAS di `z = height` sementara
/// profil BAWAH tetap di `z = 0` (sketch CADRAW cuma satu bidang XY, lihat
/// docs/PLAN.md — ini bukan workplane sungguhan, cuma translasi Z).
fn build_wire_at_z(profile: &Profile, z: f64) -> Result<Wire> {
    match profile {
        Profile::Circle { center, radius } => {
            if *radius <= 0.0 {
                bail!("radius lingkaran profil harus > 0");
            }
            let edge = Edge::circle(dvec3(center.0, center.1, z), dvec3(0.0, 0.0, 1.0), *radius);
            Ok(Wire::from_edges([&edge]))
        }
        Profile::Loop(segments) => {
            if segments.is_empty() {
                bail!("profil loop kosong");
            }
            let edges: Vec<Edge> = segments
                .iter()
                .map(|s| match s {
                    ProfileSegment::Line { start, end } => {
                        Edge::segment(dvec3(start.0, start.1, z), dvec3(end.0, end.1, z))
                    }
                    ProfileSegment::Arc { start, via, end } => Edge::arc(
                        dvec3(start.0, start.1, z),
                        dvec3(via.0, via.1, z),
                        dvec3(end.0, end.1, z),
                    ),
                })
                .collect();
            Ok(Wire::from_edges(edges.iter()))
        }
    }
}

/// Extrude profil pada bidang 3D sembarang (origin, u_axis, v_axis, normal) sepanjang `distance` mm
/// searah normal bidang.
pub fn extrude_profile_on_plane(
    profile: &Profile,
    origin: [f64; 3],
    u_axis: [f64; 3],
    v_axis: [f64; 3],
    normal: [f64; 3],
    distance: f64,
) -> Result<KernelShape> {
    if distance.abs() < 1e-9 {
        bail!("jarak extrude harus tidak nol");
    }
    let _guard = lock_kernel();
    let wire = build_wire_on_plane(profile, origin, u_axis, v_axis, normal)?;
    let face = Face::from_wire(&wire);
    let norm_len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    let extrude_dir = if norm_len > 1e-6 {
        dvec3(
            (normal[0] / norm_len) * distance,
            (normal[1] / norm_len) * distance,
            (normal[2] / norm_len) * distance,
        )
    } else {
        dvec3(0.0, 0.0, distance)
    };
    let solid = face.extrude(extrude_dir);
    Ok(KernelShape(solid.into_shape()))
}

/// Extrude profil di bidang XY sepanjang `distance` mm di sumbu Z.
pub fn extrude_profile(profile: &Profile, distance: f64) -> Result<KernelShape> {
    extrude_profile_on_plane(
        profile,
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        distance,
    )
}

/// Revolve profil di bidang XY mengelilingi sumbu 2D (`axis_origin`+
/// `axis_dir`, keduanya diangkat ke Z=0 — axis yang dipakai selalu di
/// bidang XY sama seperti sketch-nya, konsisten dengan `build_wire`).
/// `angle_degrees: None` = revolve penuh 360° (default binding
/// `Face::revolve`); revolve parsial (sudut kustom) DIDUKUNG lewat
/// `Some(derajat)` tapi belum ada UI-nya di Fase 8 putaran pertama —
/// lihat docs/PLAN.md.
pub fn revolve_profile(
    profile: &Profile,
    axis_origin: (f64, f64),
    axis_dir: (f64, f64),
    angle_degrees: Option<f64>,
) -> Result<KernelShape> {
    let dir_len = (axis_dir.0 * axis_dir.0 + axis_dir.1 * axis_dir.1).sqrt();
    if dir_len < 1e-9 {
        bail!("sumbu revolve tidak valid (dua titik axis sama/terlalu dekat)");
    }
    let _guard = lock_kernel();
    let wire = build_wire(profile)?;
    let face = Face::from_wire(&wire);
    let origin = dvec3(axis_origin.0, axis_origin.1, 0.0);
    let axis = dvec3(axis_dir.0, axis_dir.1, 0.0);
    let angle = angle_degrees.map(Angle::Degrees);
    let solid: Solid = face.revolve(origin, axis, angle);
    Ok(KernelShape(solid.into_shape()))
}

/// Loft antara 2 profil: `bottom` di `z = 0`, `top` diangkat ke
/// `z = height`. BUKAN loft lintas-workplane sungguhan (sketch CADRAW
/// cuma satu bidang XY) — cara paling jujur untuk "loft" tanpa
/// infrastruktur workplane yang belum ada, lihat docs/PLAN.md.
pub fn loft_profiles(bottom: &Profile, top: &Profile, height: f64) -> Result<KernelShape> {
    if height.abs() < 1e-9 {
        bail!("tinggi loft harus tidak nol");
    }
    let _guard = lock_kernel();
    let bottom_wire = build_wire_at_z(bottom, 0.0)?;
    let top_wire = build_wire_at_z(top, height)?;
    let solid = Solid::loft([&bottom_wire, &top_wire]);
    Ok(KernelShape(solid.into_shape()))
}

/// Union (gabung material) dua shape.
pub fn union(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    let _guard = lock_kernel();
    Ok(KernelShape(a.0.union(&b.0).shape))
}

/// Subtract (`a` dikurangi `b`).
pub fn subtract(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    let _guard = lock_kernel();
    Ok(KernelShape(a.0.subtract(&b.0).shape))
}

/// Boolean intersect (irisan) dua shape — cuma sisakan volume yang
/// tumpang-tindih. `opencascade-rs` 0.2.0 tidak expose `.intersect()` di
/// `Shape` publik seperti union/subtract (cuma di `AdHocShape`, wrapper
/// tipis di atas `BRepAlgoAPI_Common`) — di-deep_clone dulu (pola sama
/// dengan fillet/chamfer) supaya `a`/`b` asli pemanggil tidak tersentuh,
/// lalu dibungkus `AdHocShape` sekali pakai untuk akses `.intersect()`.
pub fn intersect(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let cloned = deep_clone(&a.0)?;
    let mut adhoc = AdHocShape(cloned);
    adhoc.intersect(&b.0);
    // Pakai `tessellate_shape` (helper privat, TIDAK mengunci sendiri) —
    // bukan `KernelShape::tessellate()` publik, yang akan mencoba
    // `lock_kernel()` lagi selagi `_guard` di atas masih dipegang (Mutex
    // std tidak reentrant, akan deadlock).
    if tessellate_shape(&adhoc.0).triangle_count() == 0 {
        bail!("intersect: kedua shape tidak bersinggungan (hasil kosong)");
    }
    Ok(KernelShape(adhoc.0))
}

/// Fillet SEMUA tepi shape dengan `radius` yang sama. Pemilihan tepi
/// individual (mis. cuma tepi atas) butuh UI picking 3D yang belum ada —
/// lihat docs/PLAN.md.
pub fn fillet_all(shape: &KernelShape, radius: f64) -> Result<KernelShape> {
    if radius <= 0.0 {
        bail!("radius fillet harus > 0");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(&shape.0)?;
    cloned.fillet(radius);
    Ok(KernelShape(cloned))
}

/// Chamfer SEMUA tepi shape dengan `distance` yang sama (lihat batasan
/// yang sama seperti `fillet_all`).
pub fn chamfer_all(shape: &KernelShape, distance: f64) -> Result<KernelShape> {
    if distance <= 0.0 {
        bail!("jarak chamfer harus > 0");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(&shape.0)?;
    cloned.chamfer(distance);
    Ok(KernelShape(cloned))
}

/// Ray dunia (titik asal + arah, tidak harus dinormalisasi) dipakai untuk
/// picking edge/face 3D di viewport.
///
/// SENGAJA disimpan APA ADANYA oleh pemanggil (`cadraw-app`) — BUKAN
/// index posisi di `shape.edges()`/`shape.faces()` atau handle OCCT
/// mentah. `fillet_edges`/`chamfer_edges`/`shell_hollow_faces` semua
/// harus `deep_clone` shape dulu sebelum memutasi (pola sama dengan
/// `fillet_all`/`chamfer_all`/`shell_hollow`, supaya shape asli pemanggil
/// tetap utuh untuk undo) — Face/Edge yang dipilih SEBELUM clone bukan
/// sub-shape yang valid dari shape HASIL clone, dan index posisi di
/// iterator `edges()`/`faces()` juga tidak terjamin stabil lintas
/// roundtrip STEP (belum pernah diverifikasi, jadi tidak boleh
/// diasumsikan). Ray dunia tidak kena masalah ini: `deep_clone` tidak
/// memindah/mengubah geometri di ruang dunia sama sekali, jadi ray yang
/// SAMA di-cast ULANG terhadap shape hasil clone akan selalu kena
/// permukaan/tepi yang SAMA secara geometris — robust lewat operasi
/// geometris nyata, bukan lewat asumsi index/handle yang tak teruji.
#[derive(Debug, Clone, Copy)]
pub struct PickRay {
    pub origin: (f64, f64, f64),
    pub dir: (f64, f64, f64),
}

impl PickRay {
    fn origin_vec(self) -> DVec3 {
        dvec3(self.origin.0, self.origin.1, self.origin.2)
    }

    fn dir_vec(self) -> DVec3 {
        dvec3(self.dir.0, self.dir.1, self.dir.2)
    }
}

/// Titik terdekat (ke `seg_a`/`seg_b`) di sepanjang segmen `seg_a..seg_b`
/// terhadap ray `ray_origin + t*ray_dir`, plus jaraknya ke ray. Bukan
/// solusi jarak-minimum-tersertifikasi antar 2 garis skew (itu perlu
/// clamp bersama di kedua parameter dalam loop iteratif) — pendekatan
/// dua-langkah standar yang cukup untuk hit-testing interaktif: cari `s`
/// (parameter di segmen) dari closest-point line-line lalu clamp ke
/// [0,1], baru proyeksikan balik ke ray. `opencascade-rs` tidak punya
/// primitif ray-vs-edge (beda dari `faces_along_ray` yang sudah ada untuk
/// face) — ditulis sendiri, konsisten dengan pola project (solver LM,
/// snap engine, DXF writer semua ditulis sendiri).
fn closest_point_ray_segment(ray_origin: DVec3, ray_dir: DVec3, seg_a: DVec3, seg_b: DVec3) -> (f64, DVec3) {
    let d1 = ray_dir;
    let d2 = seg_b - seg_a;
    let r = ray_origin - seg_a;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);

    if e < 1e-12 {
        // Segmen degenerate (dua titik approximation sama) — jarak ke
        // titik tunggal `seg_a`.
        let t = if a > 1e-12 { d1.dot(seg_a - ray_origin) / a } else { 0.0 };
        let closest_on_ray = ray_origin + d1 * t;
        return ((closest_on_ray - seg_a).length(), seg_a);
    }

    let s = if a < 1e-12 {
        (f / e).clamp(0.0, 1.0)
    } else {
        let c = d1.dot(r);
        let b = d1.dot(d2);
        let denom = a * e - b * b;
        let t_unclamped = if denom.abs() > 1e-12 { (b * f - c * e) / denom } else { 0.0 };
        ((b * t_unclamped + f) / e).clamp(0.0, 1.0)
    };

    let point_on_seg = seg_a + d2 * s;
    let t_ray = if a > 1e-12 { d1.dot(point_on_seg - ray_origin) / a } else { 0.0 };
    let closest_on_ray = ray_origin + d1 * t_ray;
    ((closest_on_ray - point_on_seg).length(), point_on_seg)
}

/// Toleransi geometris `BRepIntCurveSurface_Inter` untuk face-picking
/// interaktif — lebih longgar dari default upstream (`0.0001`, lihat
/// `vendor/README.md`). TERBUKTI (test terisolasi) TIDAK CUKUP sendirian
/// utk kasus ray oblique pada wajah sweep/samping (root cause BUKAN
/// toleransi — lihat `resolve_planar_face_along_ray_fallback` di bawah)
/// tapi tetap dipertahankan sebagai margin aman kecil, tidak merugikan.
const FACE_PICK_TOLERANCE_MM: f64 = 0.01;

/// Face terdekat (dari `ray.origin`) yang kena `ray`.
///
/// **Root cause OCCT yang ditemukan** (5+ test terisolasi di modul test
/// bawah, direproduksi 1:1 dari laporan user): `BRepIntCurveSurface_Inter`
/// upstream (`Shape::faces_along_ray`/`_with_tolerance`) SELALU gagal
/// mendeteksi ray OBLIQUE (sudut apa pun selain tegak lurus persis)
/// mengenai wajah SAMPING hasil sweep/extrude — di TOLERANSI BERAPA PUN
/// (dibuktikan: 0.0001 gagal, 0.01 gagal juga). Wajah CAP (atas/bawah,
/// dibangun dari wire) TIDAK terpengaruh — tetap benar walau oblique. Ray
/// tegak lurus persis pada wajah samping JUGA tetap benar (itulah kenapa
/// test kernel Fase 8 yang lama "lolos" — semua pakai ray axis-aligned,
/// tidak pernah menguji sudut kamera 3D sungguhan).
///
/// **Strategi**: coba jalur OCCT dulu (benar utk wajah melengkung/cap/hit
/// tegak lurus — jangan dibuang, cuma py punya SATU celah spesifik).
/// Kalau OCCT kosong (`faces_along_ray` gagal total), baru fallback ke
/// [`resolve_planar_face_along_ray_fallback`] — ray-vs-poligon planar
/// murni Rust yang ditulis sendiri (pola sama dgn `closest_point_ray_segment`
/// utk edge — tulis lapisan tipis sendiri saat OCCT/binding-nya punya gap,
/// bukan dependensi besar), HANYA berlaku utk wajah datar (titik tepi
/// koplanar) — wajah melengkung dilewati (sudah benar via OCCT di atas).
fn resolve_face_along_ray(shape: &Shape, ray: PickRay) -> Option<(Face, DVec3)> {
    let origin = ray.origin_vec();
    let dir = ray.dir_vec();
    let occt_hit = shape
        .faces_along_ray_with_tolerance(origin, dir, FACE_PICK_TOLERANCE_MM)
        .into_iter()
        .min_by(|(_, p1), (_, p2)| {
            (*p1 - origin)
                .length_squared()
                .partial_cmp(&(*p2 - origin).length_squared())
                .expect("titik hit faces_along_ray tidak boleh NaN")
        });
    if occt_hit.is_some() {
        return occt_hit;
    }
    resolve_planar_face_along_ray_fallback(shape, ray)
}

/// Test titik 2D di dalam poligon (algoritma ray-casting/even-odd standar)
/// — dipakai [`resolve_planar_face_along_ray_fallback`] setelah proyeksi
/// titik hit 3D ke basis 2D bidang wajah.
fn point_in_polygon_2d(p: (f64, f64), poly: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > p.1) != (yj > p.1) && p.0 < (xj - xi) * (p.1 - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Fallback ray-vs-poligon planar murni Rust — dipanggil HANYA saat jalur
/// OCCT (`faces_along_ray_with_tolerance`) di atas kosong (lihat dokumentasi
/// root-cause di `resolve_face_along_ray`). Iterasi SEMUA wajah shape;
/// untuk tiap wajah, kumpulkan titik tepi (`face.edges()` + Newell's method
/// — POLA SAMA dgn `compute_face_normal_and_centroid`, urutan loop tepi
/// sudah terbukti benar dari situ), cek KOPLANAR (wajah melengkung —
/// silinder/revolve — dilewati, deviasi dari bidang best-fit akan besar),
/// baru ray-plane intersection + point-in-polygon (proyeksi ke basis 2D
/// bidang wajah via `u_axis`/`v_axis` tegak lurus normal).
fn resolve_planar_face_along_ray_fallback(shape: &Shape, ray: PickRay) -> Option<(Face, DVec3)> {
    let origin = ray.origin_vec();
    let dir = ray.dir_vec();
    if dir.length_squared() < 1e-18 {
        return None;
    }

    let mut best: Option<(Face, DVec3, f64)> = None;
    for face in shape.faces() {
        let pts = chain_face_boundary_points(&face);
        if pts.len() < 3 {
            continue;
        }
        let centroid = pts.iter().fold(DVec3::ZERO, |acc, p| acc + *p) / (pts.len() as f64);

        // Newell's method — SAMA persis dgn `compute_face_normal_and_centroid`.
        let mut normal = DVec3::ZERO;
        for i in 0..pts.len() {
            let j = (i + 1) % pts.len();
            normal.x += (pts[i].y - pts[j].y) * (pts[i].z + pts[j].z);
            normal.y += (pts[i].z - pts[j].z) * (pts[i].x + pts[j].x);
            normal.z += (pts[i].x - pts[j].x) * (pts[i].y + pts[j].y);
        }
        let normal_len = normal.length();
        if normal_len < 1e-9 {
            continue;
        }
        let unit_normal = normal / normal_len;

        // Cek koplanar — wajah melengkung (silinder/revolve) deviasinya
        // BESAR dari bidang best-fit, dilewati (sudah benar via OCCT).
        let max_deviation = pts
            .iter()
            .map(|p| (*p - centroid).dot(unit_normal).abs())
            .fold(0.0_f64, f64::max);
        if max_deviation > 1e-3 {
            continue;
        }

        // Ray-plane intersection.
        let denom = dir.dot(unit_normal);
        if denom.abs() < 1e-9 {
            continue; // ray sejajar bidang wajah
        }
        let t = (centroid - origin).dot(unit_normal) / denom;
        if t <= 0.0 {
            continue; // perpotongan di belakang origin ray
        }
        let hit_point = origin + dir * t;

        // Point-in-polygon: proyeksi ke basis 2D bidang wajah.
        let u_axis = (pts[0] - centroid).normalize_or_zero();
        if u_axis == DVec3::ZERO {
            continue;
        }
        let v_axis = unit_normal.cross(u_axis).normalize_or_zero();
        let to_2d = |p: DVec3| -> (f64, f64) {
            let d = p - centroid;
            (d.dot(u_axis), d.dot(v_axis))
        };
        let poly2d: Vec<(f64, f64)> = pts.iter().map(|p| to_2d(*p)).collect();
        if !point_in_polygon_2d(to_2d(hit_point), &poly2d) {
            continue;
        }

        let dist_sq = (hit_point - origin).length_squared();
        if best.as_ref().is_none_or(|(_, _, d)| dist_sq < *d) {
            best = Some((face, hit_point, dist_sq));
        }
    }
    best.map(|(face, point, _)| (face, point))
}

/// Edge terdekat ke `ray` (dalam `tolerance` mm), plus titik terdekatnya
/// dan polyline approksimasi (buat highlight render) — tidak ada primitif
/// ray-vs-edge di `opencascade-rs`, iterasi manual tiap edge lewat
/// `approximation_segments()` + `closest_point_ray_segment` di atas.
fn resolve_edge_along_ray(shape: &Shape, ray: PickRay, tolerance: f64) -> Option<(Edge, DVec3, Vec<DVec3>)> {
    let origin = ray.origin_vec();
    let dir = ray.dir_vec();
    let mut best: Option<(f64, Edge, DVec3, Vec<DVec3>)> = None;
    for edge in shape.edges() {
        let polyline: Vec<DVec3> = edge.approximation_segments().collect();
        if polyline.len() < 2 {
            continue;
        }
        let mut edge_best: Option<(f64, DVec3)> = None;
        for pair in polyline.windows(2) {
            let (dist, point) = closest_point_ray_segment(origin, dir, pair[0], pair[1]);
            if edge_best.is_none_or(|(d, _)| dist < d) {
                edge_best = Some((dist, point));
            }
        }
        let Some((dist, point)) = edge_best else { continue };
        if dist <= tolerance && best.as_ref().is_none_or(|(d, ..)| dist < *d) {
            best = Some((dist, edge, point, polyline));
        }
    }
    best.map(|(_, edge, point, polyline)| (edge, point, polyline))
}

/// Informasi hit face hasil picking 3D (titik hit, titik pusat centroid, dan normal satuan keluar).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceHit {
    /// Titik hit langsung pada permukaan face (x, y, z)
    pub hit_point: (f64, f64, f64),
    /// Titik pusat / centroid aproksimasi face (x, y, z)
    pub centroid: (f64, f64, f64),
    /// Vektor normal satuan yang mengarah ke luar (outward normal) (x, y, z)
    pub normal: (f64, f64, f64),
}

/// Susun titik tepi sebuah `Face` jadi LOOP TERSAMBUNG berdasarkan
/// konektivitas titik ujung — **root cause SEBENARNYA** dari bug face-pick
/// yang awalnya diduga soal ray oblique/toleransi OCCT: `face.edges()`
/// TERBUKTI (debug trace langsung) tidak menjamin urutan keliling
/// berurutan sama sekali — untuk wajah samping hasil sweep/extrude,
/// urutannya bisa "vertikal-kiri, vertikal-kanan, horizontal-bawah,
/// horizontal-atas" (2 pasang tepi sejajar berurutan) alih-alih keliling
/// sebenarnya. Newell's method dijalankan di atas urutan "zig-zag" begini
/// menjumlahkan ke VEKTOR NOL persis (dibuktikan lewat kasus real:
/// box 194.468x77.195x51.933, wajah Y=0 — `normal_raw = (0,0,0)` exact).
/// Algoritma: mulai dari segmen tepi pertama, rantai tepi lain yang titik
/// ujungnya (toleransi kecil) cocok dengan ujung rantai saat ini, sampai
/// semua tepi terpakai. Tepi yang tidak nyambung (longgar dari asumsi
/// loop tertutup sederhana — semua wajah CADRAW yang didukung planar
/// SELALU begini) → fallback ke urutan mentah asli, lebih aman drpd
/// infinite loop.
fn chain_face_boundary_points(face: &Face) -> Vec<DVec3> {
    // SELURUH titik tiap tepi disimpan (bukan cuma titik awal/akhir) —
    // wajib utk tepi lengkung (mis. lingkaran penuh tutup silinder, HANYA
    // 1 tepi): ambil cuma 2 titik ujung akan runtuh jadi 2 titik nyaris
    // identik (lingkaran tertutup, awal≈akhir), menghilangkan seluruh
    // bentuk lingkaran (ditemukan lewat regresi test silinder — root fix
    // pertama SALAH ambil cuma first/last per tepi).
    let edge_pointlists: Vec<Vec<DVec3>> = face
        .edges()
        .map(|edge| edge.approximation_segments().collect::<Vec<DVec3>>())
        .filter(|pts| pts.len() >= 2)
        .collect();
    if edge_pointlists.is_empty() {
        return Vec::new();
    }
    if edge_pointlists.len() == 1 {
        // Cuma 1 tepi (mis. lingkaran penuh) — urutan internal tepi itu
        // sendiri SUDAH benar menyusuri kurva, tidak ada ambiguitas
        // urutan ANTAR-tepi sama sekali (itu satu-satunya yang diperbaiki
        // fungsi ini).
        return edge_pointlists.into_iter().next().expect("sudah dicek len()==1");
    }

    const EPS: f64 = 1e-6;
    let mut used = vec![false; edge_pointlists.len()];
    let mut chain = edge_pointlists[0].clone();
    used[0] = true;
    let mut remaining = edge_pointlists.len() - 1;
    while remaining > 0 {
        let tail = *chain.last().expect("chain tidak pernah kosong setelah inisialisasi");
        let mut found = false;
        for (i, pts) in edge_pointlists.iter().enumerate() {
            if used[i] {
                continue;
            }
            let start = pts[0];
            let end = *pts.last().expect("edge_pointlists sudah difilter len()>=2");
            if (start - tail).length_squared() < EPS {
                // Sambung searah — buang titik pertama (duplikat sambungan).
                chain.extend(pts.iter().skip(1).copied());
                used[i] = true;
                found = true;
                remaining -= 1;
                break;
            }
            if (end - tail).length_squared() < EPS {
                // Sambung terbalik — buang titik TERAKHIR (duplikat sambungan).
                chain.extend(pts.iter().rev().skip(1).copied());
                used[i] = true;
                found = true;
                remaining -= 1;
                break;
            }
        }
        if !found {
            // Tidak nyambung sbg loop sederhana — kembalikan urutan mentah
            // asli sbg fallback (lebih baik drpd infinite loop/panic).
            return edge_pointlists.into_iter().flatten().collect();
        }
    }
    // Buang titik penutup terakhir kalau sama dgn titik awal (loop tertutup)
    if chain.len() > 1 && (chain[0] - *chain.last().expect("chain tidak kosong")).length_squared() < EPS {
        chain.pop();
    }
    chain
}

/// Hitung vektor normal satuan ke arah luar (*outward normal*) dan titik pusat (*centroid*) dari sebuah `Face`.
fn compute_face_normal_and_centroid(face: &Face, ray_dir: DVec3) -> Option<(DVec3, DVec3)> {
    let pts = chain_face_boundary_points(face);
    if pts.is_empty() {
        return None;
    }
    let centroid = pts.iter().fold(DVec3::ZERO, |acc, p| acc + *p) / (pts.len() as f64);

    // Newell's method untuk kalkulasi normal poligon 3D sembarang
    let mut normal = DVec3::ZERO;
    for i in 0..pts.len() {
        let j = (i + 1) % pts.len();
        normal.x += (pts[i].y - pts[j].y) * (pts[i].z + pts[j].z);
        normal.y += (pts[i].z - pts[j].z) * (pts[i].x + pts[j].x);
        normal.z += (pts[i].x - pts[j].x) * (pts[i].y + pts[j].y);
    }
    let norm_len = normal.length();
    if norm_len < 1e-9 {
        return None;
    }
    let mut unit_normal = normal / norm_len;
    // Pastikan normal mengarah ke luar (berlawanan dengan arah ray datang dari kamera)
    if unit_normal.dot(ray_dir) > 0.0 {
        unit_normal = -unit_normal;
    }
    Some((unit_normal, centroid))
}

/// Cast `ray` ke `shape`, kembalikan detail face (hit point, centroid, normal keluar) terdekat.
pub fn pick_face_details(shape: &KernelShape, ray: PickRay) -> Option<FaceHit> {
    let _guard = lock_kernel();
    let (face, hit) = resolve_face_along_ray(&shape.0, ray)?;
    let (normal, centroid) = compute_face_normal_and_centroid(&face, ray.dir_vec())?;
    Some(FaceHit {
        hit_point: (hit.x, hit.y, hit.z),
        centroid: (centroid.x, centroid.y, centroid.z),
        normal: (normal.x, normal.y, normal.z),
    })
}

/// Cast `ray` ke `shape`, kembalikan titik hit face TERDEKAT (kalau ada).
/// Dipakai UI utk face-picking interaktif (Shell multi-face); resolusi
/// ULANG di apply-time (`shell_hollow_faces`) pakai `resolve_face_along_ray`
/// privat langsung, bukan fungsi publik ini (biar tidak lock_kernel dua
/// kali — lihat catatan reentrant di `KERNEL_LOCK`).
pub fn pick_face(shape: &KernelShape, ray: PickRay) -> Option<(f64, f64, f64)> {
    let _guard = lock_kernel();
    resolve_face_along_ray(&shape.0, ray).map(|(_, p)| (p.x, p.y, p.z))
}

/// Extrude satu sisi (face) solid sepanjang `distance` mm searah normal keluar.
/// Jika `distance > 0`, volume baru digabung (*Union*).
/// Jika `distance < 0`, volume dipotong (*Subtract* / Pocket Cut).
pub fn extrude_face(shape: &KernelShape, ray: PickRay, distance: f64) -> Result<KernelShape> {
    if distance.abs() < 1e-9 {
        bail!("jarak extrude face harus tidak nol");
    }
    let _guard = lock_kernel();
    let cloned = deep_clone(&shape.0)?;
    let Some((face, _)) = resolve_face_along_ray(&cloned, ray) else {
        bail!("wajah terpilih tidak ditemukan pada shape");
    };
    let Some((normal, _)) = compute_face_normal_and_centroid(&face, ray.dir_vec()) else {
        bail!("tidak dapat menghitung normal untuk wajah terpilih");
    };
    let extrude_vec = normal * distance;
    let swept = face.extrude(extrude_vec);
    let swept_shape = swept.into_shape();

    // TIDAK panggil `union`/`subtract` publik di sini — keduanya
    // `lock_kernel()` sendiri, dan `_guard` di atas masih dipegang selagi
    // thread yang sama (Mutex std TIDAK reentrant) -> deadlock permanen
    // begitu tombol drag gizmo dilepas (freeze total, tanpa panic/error
    // di terminal). Replikasi langsung isi `union`/`subtract` di sini,
    // pola sama dengan komentar `intersect` di atas.
    if distance > 0.0 {
        Ok(KernelShape(cloned.union(&swept_shape).shape))
    } else {
        Ok(KernelShape(cloned.subtract(&swept_shape).shape))
    }
}

/// (titik hit terdekat di edge, polyline approksimasi edge itu utk
/// highlight render) — hasil `pick_edge`.
pub type EdgePickHit = ((f64, f64, f64), Vec<(f64, f64, f64)>);

/// Cast `ray` ke `shape`, kembalikan (titik hit terdekat di edge, polyline
/// approksimasi edge itu utk highlight) kalau ada edge dalam `tolerance`
/// mm dari ray. Dipakai UI utk edge-picking interaktif (Fillet/Chamfer
/// per-tepi).
pub fn pick_edge(shape: &KernelShape, ray: PickRay, tolerance: f64) -> Option<EdgePickHit> {
    let _guard = lock_kernel();
    resolve_edge_along_ray(&shape.0, ray, tolerance).map(|(_, point, polyline)| {
        (
            (point.x, point.y, point.z),
            polyline.into_iter().map(|p| (p.x, p.y, p.z)).collect(),
        )
    })
}

/// Fillet HANYA tepi yang di-pick lewat `rays` (bukan semua tepi seperti
/// `fillet_all`) — tiap ray di-cast ULANG terhadap shape hasil
/// `deep_clone` (lihat desain di `PickRay`) buat resolusi Edge yang valid
/// dipakai `Shape::fillet_edges`.
pub fn fillet_edges(shape: &KernelShape, radius: f64, rays: &[PickRay], tolerance: f64) -> Result<KernelShape> {
    if radius <= 0.0 {
        bail!("radius fillet harus > 0");
    }
    if rays.is_empty() {
        bail!("pilih minimal 1 tepi (atau pakai fillet_all untuk semua tepi sekaligus)");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(&shape.0)?;
    let mut edges = Vec::with_capacity(rays.len());
    for ray in rays {
        let Some((edge, _, _)) = resolve_edge_along_ray(&cloned, *ray, tolerance) else {
            bail!("salah satu tepi terpilih tidak ditemukan lagi pada shape");
        };
        edges.push(edge);
    }
    cloned.fillet_edges(radius, &edges);
    Ok(KernelShape(cloned))
}

/// Chamfer HANYA tepi yang di-pick lewat `rays` (lihat `fillet_edges`).
pub fn chamfer_edges(shape: &KernelShape, distance: f64, rays: &[PickRay], tolerance: f64) -> Result<KernelShape> {
    if distance <= 0.0 {
        bail!("jarak chamfer harus > 0");
    }
    if rays.is_empty() {
        bail!("pilih minimal 1 tepi (atau pakai chamfer_all untuk semua tepi sekaligus)");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(&shape.0)?;
    let mut edges = Vec::with_capacity(rays.len());
    for ray in rays {
        let Some((edge, _, _)) = resolve_edge_along_ray(&cloned, *ray, tolerance) else {
            bail!("salah satu tepi terpilih tidak ditemukan lagi pada shape");
        };
        edges.push(edge);
    }
    cloned.chamfer_edges(distance, &edges);
    Ok(KernelShape(cloned))
}

/// Arah pemilihan face yang dihilangkan untuk `shell_hollow` — face
/// TERJAUH ke arah ini yang dibuang (mis. `PosZ` membuang face atas,
/// menyisakan wadah terbuka ke atas). Hanya 1 face; hollow multi-face
/// (mis. buka 2 sisi sekaligus) belum didukung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl Direction {
    fn to_occt(self) -> OcctDirection {
        match self {
            Direction::PosX => OcctDirection::PosX,
            Direction::NegX => OcctDirection::NegX,
            Direction::PosY => OcctDirection::PosY,
            Direction::NegY => OcctDirection::NegY,
            Direction::PosZ => OcctDirection::PosZ,
            Direction::NegZ => OcctDirection::NegZ,
        }
    }
}

/// "Kosongkan" shape jadi cangkang setebal `thickness` mm, membuang face
/// terjauh ke arah `remove_face_dir`. Hanya solid tunggal watertight yang
/// didukung (bawaan `BRepOffsetAPI_MakeThickSolid` OCCT).
pub fn shell_hollow(shape: &KernelShape, thickness: f64, remove_face_dir: Direction) -> Result<KernelShape> {
    if thickness <= 0.0 {
        bail!("tebal shell harus > 0");
    }
    let _guard = lock_kernel();
    let cloned = deep_clone(&shape.0)?;
    let face = cloned
        .faces()
        .try_farthest(remove_face_dir.to_occt())
        .ok_or_else(|| anyhow::anyhow!("shape tidak punya face untuk dihilangkan"))?;
    // Offset negatif = dinding tumbuh ke DALAM (cangkang), bukan keluar.
    let hollowed = cloned.hollow(-thickness.abs(), [face]);
    Ok(KernelShape(hollowed))
}

/// Sama seperti `shell_hollow`, tapi wajah yang dibuang ditentukan lewat
/// picking (`rays`, bisa >1 — mis. buka 2 sisi sekaligus) alih-alih
/// "face terjauh ke satu arah". `Shape::hollow` di `opencascade-rs` SUDAH
/// generic multi-face sejak awal — `shell_hollow` yang membatasi ke 1
/// face lewat `try_farthest` itu sendiri, fungsi ini terpisah supaya
/// perilaku `shell_hollow` lama tidak berubah.
pub fn shell_hollow_faces(shape: &KernelShape, thickness: f64, rays: &[PickRay]) -> Result<KernelShape> {
    if thickness <= 0.0 {
        bail!("tebal shell harus > 0");
    }
    if rays.is_empty() {
        bail!("pilih minimal 1 wajah (atau pakai shell_hollow untuk arah otomatis)");
    }
    let _guard = lock_kernel();
    let cloned = deep_clone(&shape.0)?;
    let mut faces = Vec::with_capacity(rays.len());
    for ray in rays {
        let Some((face, _)) = resolve_face_along_ray(&cloned, *ray) else {
            bail!("salah satu wajah terpilih tidak ditemukan lagi pada shape");
        };
        faces.push(face);
    }
    let hollowed = cloned.hollow(-thickness.abs(), faces);
    Ok(KernelShape(hollowed))
}

/// Tulis beberapa shape ke SATU file STEP, masing-masing tetap solid
/// terpisah (dibungkus `TopoDS_Compound`, BUKAN di-union jadi satu solid).
/// Dipakai export "semua body" (Fase 5, `cadraw-io`) — tool CAD lain yang
/// membuka file ini akan melihat N solid terpisah, sesuai isi dokumen
/// CADRAW aslinya.
pub fn write_step_compound(shapes: &[&KernelShape], path: impl AsRef<std::path::Path>) -> Result<()> {
    if shapes.is_empty() {
        bail!("tidak ada body untuk diekspor");
    }
    let _guard = lock_kernel();
    let refs: Vec<&Shape> = shapes.iter().map(|s| &s.0).collect();
    let compound = Compound::from_shapes(refs);
    let combined: Shape = compound.into();
    combined.write_step(path)?;
    Ok(())
}

/// Smoke-test kemampuan kernel: kotak di-extrude dari sketch lalu difillet
/// — persis alur "sketch → push/pull → fillet" yang jadi inti CADRAW.
pub fn make_filleted_box(width: f64, depth: f64, height: f64, fillet: f64) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let profile = Workplane::xy().rect(width, depth);
    let solid = profile.to_face().extrude(dvec3(0.0, 0.0, height));
    let mut shape = solid.into_shape();
    if fillet > 0.0 {
        shape.fillet(fillet);
    }
    Ok(KernelShape(shape))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// OCCT (setidaknya jalur transfer STEP yang dipakai `deep_clone`) TIDAK
    /// thread-safe di binding ini — ditemukan lewat test, bukan teori: jalan
    /// sendiri-sendiri semua lulus, tapi `cargo test` default (multi-thread)
    /// crash `SIGABRT`/`Interface_InterfaceError` karena beberapa test
    /// menyentuh working-session STEP OCCT yang sama secara bersamaan. Lock
    /// global ini memaksa seluruh test modul jalan serial. Tidak mempengaruhi
    /// `cadraw-app` (single-threaded, kernel selalu dipanggil dari UI thread).
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn rect_profile(w: f64, h: f64) -> Profile {
        Profile::Loop(vec![
            ProfileSegment::Line {
                start: (0.0, 0.0),
                end: (w, 0.0),
            },
            ProfileSegment::Line {
                start: (w, 0.0),
                end: (w, h),
            },
            ProfileSegment::Line {
                start: (w, h),
                end: (0.0, h),
            },
            ProfileSegment::Line {
                start: (0.0, h),
                end: (0.0, 0.0),
            },
        ])
    }

    /// Sama seperti `rect_profile`, tapi sudut kiri-bawah di `(x0,y0)`
    /// bukan `(0,0)` — dipakai test yang butuh profil TIDAK menyentuh
    /// origin/axis (mis. revolve, intersect dua box tidak overlap).
    fn offset_rect_profile(x0: f64, y0: f64, x1: f64, y1: f64) -> Profile {
        Profile::Loop(vec![
            ProfileSegment::Line {
                start: (x0, y0),
                end: (x1, y0),
            },
            ProfileSegment::Line {
                start: (x1, y0),
                end: (x1, y1),
            },
            ProfileSegment::Line {
                start: (x1, y1),
                end: (x0, y1),
            },
            ProfileSegment::Line {
                start: (x0, y1),
                end: (x0, y0),
            },
        ])
    }

    #[test]
    fn extrude_rectangle_produces_mesh() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(40.0, 30.0), 20.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
        assert!(!mesh.positions.is_empty());
    }

    #[test]
    fn extrude_circle_produces_cylinder_mesh() {
        let _guard = TEST_LOCK.lock().unwrap();
        let profile = Profile::Circle {
            center: (0.0, 0.0),
            radius: 10.0,
        };
        let shape = extrude_profile(&profile, 15.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn extrude_empty_loop_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert!(extrude_profile(&Profile::Loop(vec![]), 10.0).is_err());
    }

    #[test]
    fn extrude_zero_distance_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert!(extrude_profile(&rect_profile(10.0, 10.0), 0.0).is_err());
    }

    #[test]
    fn union_and_subtract_produce_valid_mesh() {
        let _guard = TEST_LOCK.lock().unwrap();
        let a = extrude_profile(&rect_profile(40.0, 40.0), 10.0).unwrap();
        let b = extrude_profile(&rect_profile(20.0, 20.0), 10.0).unwrap();
        let unioned = union(&a, &b).unwrap();
        assert!(unioned.tessellate().triangle_count() > 0);
        let subtracted = subtract(&a, &b).unwrap();
        assert!(subtracted.tessellate().triangle_count() > 0);
    }

    #[test]
    fn fillet_all_and_chamfer_all_smoke() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let filleted = fillet_all(&shape, 2.0).unwrap();
        assert!(filleted.tessellate().triangle_count() > 0);
        // Deep-clone di dalam fillet_all/chamfer_all TIDAK memutasi `shape`
        // asli — shape asli harus masih valid & bisa dipakai lagi setelah.
        let chamfered = chamfer_all(&shape, 2.0).unwrap();
        assert!(chamfered.tessellate().triangle_count() > 0);
        assert!(shape.tessellate().triangle_count() > 0);
    }

    #[test]
    fn shell_hollow_smoke() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 30.0), 20.0).unwrap();
        let hollowed = shell_hollow(&shape, 2.0, Direction::PosZ).unwrap();
        assert!(hollowed.tessellate().triangle_count() > 0);
    }

    #[test]
    fn deep_clone_preserves_mesh_vertex_count() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(25.0, 15.0), 10.0).unwrap();
        let cloned = deep_clone(&shape.0).unwrap();
        let original_mesh = shape.tessellate();
        let cloned_mesh = tessellate_shape(&cloned);
        assert_eq!(original_mesh.positions.len(), cloned_mesh.positions.len());
    }

    #[test]
    fn make_filleted_box_smoke() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = make_filleted_box(40.0, 30.0, 20.0, 3.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn step_string_roundtrip_preserves_mesh_vertex_count() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(25.0, 15.0), 10.0).unwrap();
        let step = shape.to_step_string().unwrap();
        assert!(step.contains("ISO-10303"), "STEP harus AP214 ISO-10303");
        let restored = KernelShape::from_step_string(&step).unwrap();
        assert_eq!(shape.tessellate().positions.len(), restored.tessellate().positions.len());
    }

    #[test]
    fn read_step_roundtrips_write_step() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(10.0, 10.0), 5.0).unwrap();
        let path = std::env::temp_dir().join(format!("cadraw-test-read-step-{}.step", std::process::id()));
        shape.write_step(&path).unwrap();
        let restored = KernelShape::read_step(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(shape.tessellate().positions.len(), restored.tessellate().positions.len());
    }

    #[test]
    fn write_step_compound_combines_two_bodies() {
        let _guard = TEST_LOCK.lock().unwrap();
        let a = extrude_profile(&rect_profile(10.0, 10.0), 5.0).unwrap();
        let b = extrude_profile(&rect_profile(20.0, 20.0), 5.0).unwrap();
        let path = std::env::temp_dir().join(format!("cadraw-test-compound-{}.step", std::process::id()));
        write_step_compound(&[&a, &b], &path).unwrap();
        let restored = KernelShape::read_step(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        // Compound gabungan dua box terpisah harus punya lebih banyak
        // vertex dari salah satu box sendirian (bukti keduanya masuk).
        assert!(restored.tessellate().positions.len() > a.tessellate().positions.len());
    }

    #[test]
    fn write_step_compound_empty_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        let path = std::env::temp_dir().join("cadraw-test-compound-empty.step");
        assert!(write_step_compound(&[], &path).is_err());
    }

    #[test]
    fn kernel_mesh_merge_shifts_indices() {
        let a = KernelMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
        };
        let b = KernelMesh {
            positions: vec![[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
        };
        let merged = KernelMesh::merge(&[&a, &b]);
        assert_eq!(merged.positions.len(), 6);
        assert_eq!(merged.indices, vec![0, 1, 2, 3, 4, 5]);
    }

    // ---- Fase 8: Revolve ----

    #[test]
    fn revolve_profile_produces_ring_solid() {
        let _guard = TEST_LOCK.lock().unwrap();
        // Rectangle x∈[10,20], y∈[0,5] direvolve mengelilingi sumbu Y
        // (garis x=0) — profil TIDAK menyentuh axis, jadi hasilnya solid
        // ring/tube berlubang (bukan cakram penuh dari radius 0).
        let profile = offset_rect_profile(10.0, 0.0, 20.0, 5.0);
        let shape = revolve_profile(&profile, (0.0, 0.0), (0.0, 1.0), None).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
        let mut max_radius: f32 = 0.0;
        let mut min_radius: f32 = f32::MAX;
        let mut min_y: f32 = f32::MAX;
        let mut max_y: f32 = f32::MIN;
        for p in &mesh.positions {
            let radius = (p[0] * p[0] + p[2] * p[2]).sqrt();
            max_radius = max_radius.max(radius);
            min_radius = min_radius.min(radius);
            min_y = min_y.min(p[1]);
            max_y = max_y.max(p[1]);
        }
        assert!(min_radius > 5.0, "radius dalam {min_radius} seharusnya mendekati 10 (profil tidak menyentuh axis)");
        assert!(max_radius > 15.0 && max_radius < 25.0, "radius luar {max_radius} seharusnya mendekati 20");
        assert!(min_y >= -0.5 && max_y <= 5.5, "tinggi hasil harus dalam rentang y profil asli [0,5], dapat [{min_y},{max_y}]");
    }

    #[test]
    fn revolve_profile_degenerate_axis_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        let profile = offset_rect_profile(10.0, 0.0, 20.0, 5.0);
        assert!(revolve_profile(&profile, (0.0, 0.0), (0.0, 0.0), None).is_err());
    }

    // ---- Fase 8: Loft ----

    #[test]
    fn loft_between_rectangles_spans_requested_height() {
        let _guard = TEST_LOCK.lock().unwrap();
        let bottom = rect_profile(20.0, 20.0);
        let top = rect_profile(10.0, 10.0);
        let shape = loft_profiles(&bottom, &top, 15.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
        let mut min_z: f32 = f32::MAX;
        let mut max_z: f32 = f32::MIN;
        for p in &mesh.positions {
            min_z = min_z.min(p[2]);
            max_z = max_z.max(p[2]);
        }
        assert!((-0.5..=0.5).contains(&min_z), "dasar loft harus di z=0, dapat {min_z}");
        assert!((14.5..=15.5).contains(&max_z), "puncak loft harus di z=15, dapat {max_z}");
    }

    #[test]
    fn loft_zero_height_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        let bottom = rect_profile(20.0, 20.0);
        let top = rect_profile(10.0, 10.0);
        assert!(loft_profiles(&bottom, &top, 0.0).is_err());
    }

    // ---- Fase 8: Boolean intersect ----

    #[test]
    fn intersect_overlapping_boxes_smaller_than_union() {
        let _guard = TEST_LOCK.lock().unwrap();
        let a = extrude_profile(&rect_profile(40.0, 40.0), 10.0).unwrap();
        let b = extrude_profile(&offset_rect_profile(20.0, 20.0, 60.0, 60.0), 10.0).unwrap();
        let intersected = intersect(&a, &b).unwrap();
        let unioned = union(&a, &b).unwrap();
        // Irisan HARUS lebih kecil dari union (bukti nyata "cuma yang
        // tumpang tindih", bukan cuma "tidak panic") — jumlah vertex
        // tessellation dipakai sbg proxy volume kasar.
        assert!(intersected.tessellate().positions.len() < unioned.tessellate().positions.len());
        assert!(intersected.tessellate().triangle_count() > 0);
    }

    #[test]
    fn intersect_non_overlapping_boxes_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        let a = extrude_profile(&rect_profile(10.0, 10.0), 10.0).unwrap();
        let b = extrude_profile(&offset_rect_profile(100.0, 100.0, 110.0, 110.0), 10.0).unwrap();
        assert!(intersect(&a, &b).is_err());
    }

    // ---- Fase 8: Picking 3D (edge/face) ----

    /// Validasi arsitektur WAJIB (lihat docs/PLAN.md desain kunci Fase 8):
    /// ray dunia yang SAMA harus kena face yang SAMA baik di shape asli
    /// maupun di hasil `deep_clone`-nya — dasar kenapa `PickRay` disimpan
    /// (bukan index/handle) aman dipakai lintas roundtrip STEP.
    #[test]
    fn pick_face_consistent_across_deep_clone() {
        let _guard = TEST_LOCK.lock().unwrap();
        // Box x∈[0,30], y∈[0,20], z∈[0,15]; ray lurus ke bawah dari atas
        // menuju tengah face TOP (z=15).
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (15.0, 10.0, 100.0),
            dir: (0.0, 0.0, -1.0),
        };
        let hit_original = pick_face(&shape, ray).expect("harus kena face top shape asli");
        let cloned = deep_clone(&shape.0).unwrap();
        let hit_cloned = resolve_face_along_ray(&cloned, ray)
            .map(|(_, p)| (p.x, p.y, p.z))
            .expect("harus kena face top shape hasil deep_clone");
        assert!((hit_original.0 - hit_cloned.0).abs() < 1e-6);
        assert!((hit_original.1 - hit_cloned.1).abs() < 1e-6);
        assert!((hit_original.2 - hit_cloned.2).abs() < 1e-6);
    }

    /// Validasi arsitektur WAJIB versi edge (lihat catatan test di atas).
    #[test]
    fn pick_edge_consistent_across_deep_clone() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        // Ray diagonal menuju rusuk vertikal box di (x=0,y=0).
        let ray = PickRay {
            origin: (-5.0, -5.0, 7.5),
            dir: (1.0, 1.0, 0.0),
        };
        let tolerance = 1.0;
        let (hit_original, _) = pick_edge(&shape, ray, tolerance).expect("harus kena rusuk shape asli");
        let cloned = deep_clone(&shape.0).unwrap();
        let (_, hit_cloned, _) =
            resolve_edge_along_ray(&cloned, ray, tolerance).expect("harus kena rusuk shape hasil deep_clone");
        assert!((hit_original.0 - hit_cloned.x).abs() < 1e-3);
        assert!((hit_original.1 - hit_cloned.y).abs() < 1e-3);
        assert!((hit_original.2 - hit_cloned.z).abs() < 1e-3);
    }

    #[test]
    fn pick_face_miss_returns_none() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (1000.0, 1000.0, 1000.0),
            dir: (0.0, 0.0, 1.0),
        };
        assert!(pick_face(&shape, ray).is_none());
    }

    // ---- Fase 8: Fillet/Chamfer per-tepi, Shell multi-face ----

    #[test]
    fn fillet_edges_affects_only_picked_edge() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, 7.5),
            dir: (1.0, 1.0, 0.0),
        };
        let filleted_one = fillet_edges(&shape, 2.0, &[ray], 1.0).unwrap();
        let filleted_all = fillet_all(&shape, 2.0).unwrap();
        let original_verts = shape.tessellate().positions.len();
        let one_verts = filleted_one.tessellate().positions.len();
        let all_verts = filleted_all.tessellate().positions.len();
        assert!(one_verts > original_verts, "fillet 1 tepi harus mengubah mesh (tambah vertex bulat)");
        assert!(
            one_verts < all_verts,
            "fillet 1 tepi HARUS lebih sedikit vertex baru dibanding fillet SEMUA 12 tepi box — bukti hanya 1 tepi yang kena"
        );
    }

    #[test]
    fn fillet_edges_empty_rays_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        assert!(fillet_edges(&shape, 2.0, &[], 1.0).is_err());
    }

    #[test]
    fn fillet_edges_no_match_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (1000.0, 1000.0, 1000.0),
            dir: (0.0, 0.0, 1.0),
        };
        assert!(fillet_edges(&shape, 2.0, &[ray], 1.0).is_err());
    }

    #[test]
    fn chamfer_edges_smoke() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, 7.5),
            dir: (1.0, 1.0, 0.0),
        };
        let chamfered = chamfer_edges(&shape, 2.0, &[ray], 1.0).unwrap();
        assert!(chamfered.tessellate().triangle_count() > 0);
        // Shape asli tidak boleh ikut termutasi (pola sama dgn fillet_all).
        assert!(shape.tessellate().triangle_count() > 0);
    }

    #[test]
    fn shell_hollow_faces_multi_face_differs_from_single() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 30.0), 20.0).unwrap();
        let ray_top = PickRay {
            origin: (15.0, 15.0, 100.0),
            dir: (0.0, 0.0, -1.0),
        };
        let ray_bottom = PickRay {
            origin: (15.0, 15.0, -100.0),
            dir: (0.0, 0.0, 1.0),
        };
        let hollow_two = shell_hollow_faces(&shape, 2.0, &[ray_top, ray_bottom]).unwrap();
        let hollow_one = shell_hollow(&shape, 2.0, Direction::PosZ).unwrap();
        assert!(hollow_two.tessellate().triangle_count() > 0);
        // 2 wajah dibuang (tabung terbuka 2 sisi, 4 dinding) HARUS beda
        // topologi dari 1 wajah dibuang (wadah terbuka 1 sisi, 5 dinding)
        // — dibuktikan lewat JUMLAH FACE B-rep asli (10 vs 11, dicek
        // langsung sekali ketika menulis test ini) dan jumlah triangle
        // (32 vs 28) — bukan jumlah vertex tessellation, yang KEBETULAN
        // sama (48==48) di box simetris ini walau topologinya beda nyata
        // (ditemukan lewat test yang gagal, bukan diasumsikan aman).
        assert_ne!(hollow_two.0.faces().count(), hollow_one.0.faces().count());
        assert_ne!(hollow_two.tessellate().triangle_count(), hollow_one.tessellate().triangle_count());
    }

    #[test]
    fn shell_hollow_faces_empty_rays_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 30.0), 20.0).unwrap();
        assert!(shell_hollow_faces(&shape, 2.0, &[]).is_err());
    }

    #[test]
    fn extrude_vertical_front_xz_produces_solid() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile_on_plane(
            &rect_profile(30.0, 20.0),
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, -1.0, 0.0],
            15.0,
        )
        .unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
        assert!(!mesh.positions.is_empty());
    }

    #[test]
    fn extrude_vertical_right_yz_produces_solid() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile_on_plane(
            &rect_profile(25.0, 35.0),
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            10.0,
        )
        .unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
        assert!(!mesh.positions.is_empty());
    }

    #[test]
    fn test_pick_face_huge_magnitude_direction_matches_unit_direction() {
        // DIAGNOSTIK: ray dunia-nyata dari `screen_to_ray` (unprojection kamera)
        // punya magnitude arah SANGAT besar (~17000-24700, dibanding origin yang
        // hanya berskala ~200) — jauh beda dari ray unit-length yang dipakai test
        // kernel lain. Tes ini isolasi murni: SAMA arah ternormalisasi, SAMA
        // origin, BEDA cuma magnitude `dir` — buktikan apakah OCCT `gp_Dir`
        // (yang seharusnya menormalisasi otomatis) benar-benar tidak peduli skala.
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(200.0, 200.0), 20.0).unwrap();

        let unit_dir = (0.0_f64, 0.0, -1.0);
        let huge_dir = (0.0_f64, 0.0, -20000.0); // arah sama (lurus -Z), magnitude 20000x

        let ray_unit = PickRay { origin: (100.0, 100.0, 500.0), dir: unit_dir };
        let ray_huge = PickRay { origin: (100.0, 100.0, 500.0), dir: huge_dir };

        let hit_unit = pick_face_details(&shape, ray_unit);
        let hit_huge = pick_face_details(&shape, ray_huge);

        eprintln!("[DIAGNOSTIK] hit_unit = {hit_unit:?}");
        eprintln!("[DIAGNOSTIK] hit_huge = {hit_huge:?}");

        assert!(hit_unit.is_some(), "ray unit-length harus kena top face");
        assert!(hit_huge.is_some(), "ray magnitude besar (20000x) HARUS tetap kena face yang sama kalau gp_Dir menormalisasi dengan benar");
    }

    #[test]
    fn test_pick_face_real_world_oblique_ray_reproduction() {
        // DIAGNOSTIK: reproduksi PERSIS 1:1 laporan user — ray nyata dari
        // `screen_to_ray` (origin + dir sungguhan disalin dari log terminal)
        // TERBUKTI secara analitik (ray-slab AABB test manual) menembus
        // bounding box mesh body real (min=(-120,-20,0) max=(74.47,57.19,
        // 51.93), masuk tepat di titik (36.42,-20.0,30.94) — pas di
        // permukaan Y=min) TAPI `pick_face_details` melaporkan MISS. Box
        // test di sini punya dimensi PERSIS SAMA (w=194.468 h=77.195
        // d=51.933, `extrude_profile` selalu mulai dari (0,0,0)) — ray
        // ditranslasi ikut supaya posisi RELATIF terhadap box identik
        // dengan kasus nyata (translasi origin box dari (-120,-20,0) ke
        // (0,0,0): geser ray.origin +120 di X, +20 di Y, +0 di Z; arah
        // TIDAK berubah karena translasi tidak mengubah vektor arah).
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

        let ray = PickRay {
            origin: (152.94723510742188 + 120.0, -152.9267120361328 + 20.0, 124.88241577148438),
            dir: (-14566.6611328125, 16616.1640625, -11743.1689453125),
        };
        let hit = pick_face_details(&shape, ray);
        eprintln!("[DIAGNOSTIK real-world ray] hit = {hit:?}");
        assert!(hit.is_some(), "ray nyata TERBUKTI (ray-slab AABB manual) menembus box, tapi pick_face_details melaporkan MISS — bug OCCT/binding pada ray oblique?");
    }

    #[test]
    fn test_pick_face_same_oblique_direction_unit_length_isolation() {
        // Isolasi lanjutan dari `test_pick_face_real_world_oblique_ray_reproduction`:
        // SAMA PERSIS arah oblique (dinormalisasi ke unit length), SAMA
        // origin & box — HANYA magnitude `dir` yang beda (unit vs ~25024).
        // Kalau tes ini LULUS sementara yang huge-magnitude GAGAL, terbukti
        // magnitude besar + oblique (BUKAN oblique sendirian) akar masalahnya.
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

        let ray = PickRay {
            origin: (152.94723510742188 + 120.0, -152.9267120361328 + 20.0, 124.88241577148438),
            dir: (-0.5821141452051053, 0.664016554764354, -0.4692815114097371),
        };
        let hit = pick_face_details(&shape, ray);
        eprintln!("[DIAGNOSTIK unit-length oblique] hit = {hit:?}");
        assert!(hit.is_some(), "arah oblique SAMA tapi unit-length harus tetap kena kalau masalahnya murni di magnitude");
    }

    #[test]
    fn test_pick_face_simple_clean_oblique_ray_baseline() {
        // KOREKSI dari percobaan pertama: (-1,-1,-1) dari (200,200,200)
        // ternyata mengenai KORNER box (100,100,100) persis — x=y=z
        // berkurang sama rata sepanjang ray, jadi itu kasus degenerate 3
        // wajah sekaligus, bukan tes oblique yang bersih. Di sini dipakai
        // arah oblique ASIMETRIS yang terverifikasi manual (ray-slab test)
        // masuk jelas di TENGAH wajah atas Z=100 (titik masuk (65.625,
        // 65.625, 100.0) — jarak ke tepi terdekat ~34 unit, jauh dari
        // ambigu).
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(100.0, 100.0), 100.0).unwrap();

        let origin = (300.0_f64, 150.0, 250.0);
        let target = (50.0_f64, 60.0, 90.0); // di dalam box, jauh dari semua tepi
        let dir = (target.0 - origin.0, target.1 - origin.1, target.2 - origin.2);

        let ray = PickRay { origin, dir };
        let hit = pick_face_details(&shape, ray);
        eprintln!("[DIAGNOSTIK baseline oblique bersih] hit = {hit:?}");
        assert!(hit.is_some(), "ray oblique asimetris menuju tengah wajah atas HARUS kena — kalau gagal, oblique ray rusak secara umum");
        if let Some(h) = hit {
            assert!((h.hit_point.2 - 100.0).abs() < 1e-3, "harus kena wajah Z=100 (atas), bukan wajah lain");
        }
    }

    #[test]
    fn test_pick_face_min_bound_face_same_box_dims_as_real_case() {
        // Isolasi lebih jauh: box PERSIS dimensi kasus nyata (194.468 x
        // 77.195 x 51.933), tapi ray BERSIH (angka bulat, bukan hasil
        // unprojection kamera) yang diverifikasi manual (ray-slab) masuk
        // pas di wajah Y=MIN (y=0) — sama axis/sisi dengan kasus nyata
        // (yang juga masuk di Y=min), titik masuk (115.38, 0.0, 27.31)
        // jauh dari semua tepi. Kalau ini GAGAL juga, berarti spesifik ke
        // wajah MIN-bound pada box dimensi ini — bukan noise angka kamera.
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

        let ray = PickRay { origin: (100.0, -100.0, 25.0), dir: (20.0, 130.0, 3.0) };
        let hit = pick_face_details(&shape, ray);
        eprintln!("[DIAGNOSTIK min-face box dims real] hit = {hit:?}");
        assert!(hit.is_some(), "ray bersih menuju wajah Y=min box dimensi real HARUS kena");
    }

    #[test]
    fn test_pick_face_max_bound_side_face_oblique() {
        // Konfirmasi terakhir: sisi X=MAX (bukan min), ray oblique bersih,
        // box dims sama. Kalau ini JUGA gagal, terbukti pola sebenarnya
        // "SISI SAMPING (swept side face) yang kena miring/oblique" — BUKAN
        // spesifik ke wajah min-bound. Cap face (atas/bawah, dari wire)
        // sudah terbukti OK walau oblique di test sebelumnya.
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

        let ray = PickRay { origin: (300.0, 20.0, 25.0), dir: (-150.0, 20.0, 5.0) };
        let hit = pick_face_details(&shape, ray);
        eprintln!("[DIAGNOSTIK max-face side oblique] hit = {hit:?}");
        assert!(hit.is_some(), "ray bersih menuju wajah X=max HARUS kena");
    }

    #[test]
    fn test_pick_face_cap_face_real_box_dims_isolation() {
        // Pemisah variabel terakhir: box dimensi PERSIS real (194.468 x
        // 77.195 x 51.933) TAPI kena CAP face (Z=max, dari wire) bukan sisi
        // samping. Kalau ini LULUS sementara Y=min/X=max GAGAL (box SAMA),
        // terbukti variabelnya JENIS WAJAH (cap vs sisi samping/swept),
        // BUKAN dimensi box.
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

        let ray = PickRay { origin: (100.0, 30.0, 300.0), dir: (20.0, 5.0, -255.0) };
        let hit = pick_face_details(&shape, ray);
        eprintln!("[DIAGNOSTIK cap-face box dims real] hit = {hit:?}");
        assert!(hit.is_some(), "ray oblique ke cap face Z=max HARUS kena walau box dims sama dgn kasus yg gagal");
    }

    #[test]
    fn test_pick_face_details_and_extrude_box_faces() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();

        // 1. Pick Top face (+Z)
        let ray_top = PickRay {
            origin: (15.0, 10.0, 100.0),
            dir: (0.0, 0.0, -1.0),
        };
        let hit_top = pick_face_details(&shape, ray_top).expect("harus kena top face");
        assert!((hit_top.normal.0 - 0.0).abs() < 1e-5);
        assert!((hit_top.normal.1 - 0.0).abs() < 1e-5);
        assert!((hit_top.normal.2 - 1.0).abs() < 1e-5);
        assert!((hit_top.centroid.2 - 15.0).abs() < 1e-5);

        // Extrude Top face +Z by 10mm
        let extruded_top = extrude_face(&shape, ray_top, 10.0).expect("extrude top face berhasil");
        assert!(extruded_top.tessellate().triangle_count() > 0);

        // 2. Pick Right/Side face (+X)
        let ray_right = PickRay {
            origin: (100.0, 10.0, 7.5),
            dir: (-1.0, 0.0, 0.0),
        };
        let hit_right = pick_face_details(&shape, ray_right).expect("harus kena side face");
        assert!((hit_right.normal.0 - 1.0).abs() < 1e-5);
        assert!((hit_right.normal.1 - 0.0).abs() < 1e-5);
        assert!((hit_right.normal.2 - 0.0).abs() < 1e-5);

        // Extrude Side face +X by 5mm
        let extruded_right = extrude_face(&shape, ray_right, 5.0).expect("extrude side face berhasil");
        assert!(extruded_right.tessellate().triangle_count() > 0);
    }

    #[test]
    fn test_extrude_face_cylinder_top() {
        let _guard = TEST_LOCK.lock().unwrap();
        let circle_profile = Profile::Circle {
            center: (0.0, 0.0),
            radius: 12.0,
        };
        let cylinder = extrude_profile(&circle_profile, 25.0).unwrap();
        let ray_top = PickRay {
            origin: (0.0, 0.0, 100.0),
            dir: (0.0, 0.0, -1.0),
        };
        let hit_top = pick_face_details(&cylinder, ray_top).expect("harus kena top cap silinder");
        assert!((hit_top.normal.2 - 1.0).abs() < 1e-5);
        assert!((hit_top.centroid.2 - 25.0).abs() < 1e-5);

        let taller_cylinder = extrude_face(&cylinder, ray_top, 15.0).expect("extrude top cap silinder berhasil");
        assert!(taller_cylinder.tessellate().triangle_count() > 0);
    }
}
