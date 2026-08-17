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
use opencascade::primitives::{
    Compound, Direction as OcctDirection, Edge, Face, FaceOrientation, IntoShape, Shape, Solid, Wire,
};
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

/// Union (gabung material) dua shape. `.clean()` (`ShapeUpgrade_
/// UnifySameDomain` OCCT) di-panggil sesudahnya supaya face/edge yang
/// koplanar persis di sambungan boolean (mis. sisi kubus yang di-extrude
/// lurus keluar dari sisi lama) DIGABUNG jadi satu face, bukan tertinggal
/// sebagai dua face terpisah yang cuma bertemu di satu garis (terlihat
/// seperti "jahitan"/seam ganda di viewport walau geometrinya valid).
pub fn union(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let mut merged = a.0.union(&b.0).shape;
    merged.clean();
    Ok(KernelShape(merged))
}

/// Subtract (`a` dikurangi `b`) — lihat catatan `.clean()` di `union`.
pub fn subtract(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let mut result = a.0.subtract(&b.0).shape;
    result.clean();
    Ok(KernelShape(result))
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
    let all_edges: Vec<Edge> = cloned.edges().collect();
    let max_radius = max_fillet_radius(&cloned, &all_edges);
    if radius > max_radius {
        bail!("radius fillet ({radius:.2} mm) melebihi batas geometris salah satu tepi shape (maks {max_radius:.2} mm)");
    }
    cloned
        .fillet(radius)
        .context("radius fillet terlalu besar untuk salah satu tepi shape")?;
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
    let all_edges: Vec<Edge> = cloned.edges().collect();
    let max_distance = max_fillet_radius(&cloned, &all_edges);
    if distance > max_distance {
        bail!("jarak chamfer ({distance:.2} mm) melebihi batas geometris salah satu tepi shape (maks {max_distance:.2} mm)");
    }
    cloned
        .chamfer(distance)
        .context("jarak chamfer terlalu besar untuk salah satu tepi shape")?;
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

/// Jarak titik `point` ke garis `ray` (garis TAK TERBATAS di kedua arah,
/// bukan half-line dari `ray_origin` — sama persis dgn cabang segmen
/// degenerate di `closest_point_ray_segment` di atas, cuma dipisah jadi
/// helper sendiri karena vertex picking butuh jarak titik-ke-ray murni,
/// bukan titik-ke-segmen).
fn point_to_ray_distance(ray_origin: DVec3, ray_dir: DVec3, point: DVec3) -> f64 {
    let a = ray_dir.dot(ray_dir);
    let t = if a > 1e-12 { ray_dir.dot(point - ray_origin) / a } else { 0.0 };
    let closest_on_ray = ray_origin + ray_dir * t;
    (closest_on_ray - point).length()
}

/// Vertex (sudut/endpoint edge) terdekat ke `ray` (dalam `tolerance` mm)
/// — tidak ada primitif vertex-along-ray di `opencascade-rs` (sama
/// seperti face/edge di atas), jadi dikumpulkan sendiri: endpoint SEMUA
/// edge shape (banyak edge berbagi 1 vertex yang sama di topologi B-rep,
/// endpoint di-dedup lewat jarak epsilon supaya vertex yang sama tidak
/// dihitung berkali-kali lintas edge), lalu dipilih yang jaraknya ke ray
/// paling kecil dan masih dalam `tolerance`.
fn resolve_vertex_along_ray(shape: &Shape, ray: PickRay, tolerance: f64) -> Option<DVec3> {
    let origin = ray.origin_vec();
    let dir = ray.dir_vec();
    if dir.length_squared() < 1e-18 {
        return None;
    }

    let mut best: Option<(f64, DVec3)> = None;
    for v in collect_vertices(shape) {
        let dist = point_to_ray_distance(origin, dir, v);
        if dist <= tolerance && best.as_ref().is_none_or(|(d, _)| dist < *d) {
            best = Some((dist, v));
        }
    }
    best.map(|(_, v)| v)
}

/// Semua vertex (sudut) unik pada `shape` — endpoint SEMUA edge, di-dedup
/// lewat jarak epsilon (banyak edge berbagi 1 vertex topologi B-rep yang
/// sama). Diekstrak dari `resolve_vertex_along_ray` supaya logika dedup
/// yang sama dipakai juga oleh `shape_vertices` (marker vertex terlihat di
/// viewport, lihat pemanggilnya di cadraw-app) tanpa duplikasi.
fn collect_vertices(shape: &Shape) -> Vec<DVec3> {
    // Epsilon dedup endpoint — SAMA dengan epsilon "berimpit" yang dipakai
    // `fillet_vertex` di bawah buat mengumpulkan tepi yang bertemu di 1
    // vertex, supaya kriteria "vertex yang sama" konsisten di semua tempat.
    const DEDUP_EPS: f64 = 1e-6;
    let mut vertices: Vec<DVec3> = Vec::new();
    for edge in shape.edges() {
        for p in [edge.start_point(), edge.end_point()] {
            if !vertices.iter().any(|v| (*v - p).length() < DEDUP_EPS) {
                vertices.push(p);
            }
        }
    }
    vertices
}

/// Batas radius/jarak rounding (fillet/chamfer) yang MASUK AKAL secara
/// geometris. OCCT sendiri kadang tetap `IsDone()==true` (bukan
/// `StdFail_NotDone`, lihat catatan fix crash di `fillet_edges`/
/// `fillet_vertex` di bawah) walau radius jauh melebihi ukuran tepi yang
/// di-fillet — hasilnya bentuk yang "memakan" seluruh sisi objek (mis.
/// sudut box jadi baji/quarter-cylinder alih-alih sudut membulat wajar,
/// dilaporkan lewat screenshot user). Precheck manual ini dipanggil
/// SEBELUM manggil OCCT (pola sama dengan validasi radius silinder/
/// kerucut di `extrude_face`): radius tidak boleh melebihi (mendekati)
/// panjang tepi TERPENDEK yang terlibat — tiap tepi target sendiri, PLUS
/// semua tepi lain di `shape` yang salah satu endpoint-nya berimpit
/// (epsilon sama dengan `collect_vertices`/`fillet_vertex`) dengan salah
/// satu endpoint tepi target (tepi tetangga sudut).
///
/// BUKAN separuh panjang tepi — versi awal patch ini SALAH pakai `/2.0`
/// (kelewat konservatif, dilaporkan user lewat screenshot: gizmo berhenti
/// baru separuh jalan, padahal masih ada ruang sampai ujung tepinya).
/// Untuk sudut ORTOGONAL (semua sudut box/prism, kasus utama gizmo
/// rounding CADRAW), titik singgung fillet di sepanjang tepi tetangga
/// berjarak PERSIS `radius` dari titik sudut (`tan(45°) == 1` — sudut
/// antar 2 wajah tegak lurus dibagi dua = 45°), BUKAN `radius/2` — jadi
/// radius maksimum yang aman kira-kira SAMA DENGAN panjang tepi tetangga
/// terpendek (titik singgungnya baru menyentuh ujung SATU tepi tetangga,
/// bukan setengahnya), bukan setengahnya. Kalau radius sampai PERSIS
/// menyentuh sudut tetangga (kasus batas paling ekstrem), `fillet_edges`/
/// `fillet_vertex` masih bisa `Err` dari OCCT sendiri — itu SUDAH aman
/// (tidak crash, lihat fix `StdFail_NotDone` di atas), jadi tidak perlu
/// margin tambahan di sini.
///
/// Pakai jarak lurus (chord) endpoint-ke-endpoint sebagai "panjang tepi"
/// — akurat untuk tepi lurus (semua tepi box/prism, target utama gizmo
/// rounding CADRAW), aproksimasi (sedikit lebih pendek dari panjang
/// busur asli) untuk tepi melengkung — cukup konservatif untuk tujuan
/// batas atas di sini (BUKAN untuk pengukuran presisi).
fn max_fillet_radius(shape: &Shape, target_edges: &[Edge]) -> f64 {
    const COINCIDENT_EPS: f64 = 1e-6;
    let mut endpoints: Vec<DVec3> = Vec::new();
    let mut min_len = f64::INFINITY;
    for edge in target_edges {
        let len = (edge.end_point() - edge.start_point()).length();
        min_len = min_len.min(len);
        endpoints.push(edge.start_point());
        endpoints.push(edge.end_point());
    }
    for candidate in shape.edges() {
        let s = candidate.start_point();
        let e = candidate.end_point();
        let touches = endpoints
            .iter()
            .any(|p| (s - *p).length() < COINCIDENT_EPS || (e - *p).length() < COINCIDENT_EPS);
        if touches {
            min_len = min_len.min((e - s).length());
        }
    }
    if min_len.is_finite() {
        min_len
    } else {
        f64::INFINITY
    }
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
    /// Tipe permukaan geometris analitik di balik face yang terpilih.
    pub surface_kind: SurfaceKind,
    /// Arah satuan yang dipakai gizmo push/pull (CADRAW Fase 4) — TIDAK
    /// selalu sama dengan `normal`:
    /// - `Plane`: identik dengan `normal` (normal Newell, perilaku lama).
    /// - `Cylinder`/`Cone`: arah RADIAL di `hit_point`, yaitu
    ///   `(hit_point − proyeksi hit_point ke garis sumbu).normalize()` —
    ///   inilah arah geometris yang benar-benar mengubah radius saat
    ///   di-drag (lihat dispatch `extrude_face`/`offset_on_face`), bukan
    ///   normal permukaan lokal di titik itu (yang juga radial tapi
    ///   Newell's method di atas boundary loop TIDAK menghitungnya per
    ///   titik — nilainya konstan per-face, salah untuk radial drag).
    /// - `Sphere`: `(hit_point − centroid).normalize()` — `centroid` bola
    ///   penuh (`Face::center_of_mass`) SECARA MATEMATIS persis pusat bola
    ///   (centroid luas permukaan bola simetris = pusatnya).
    /// - `Torus`/`Other`: fallback ke `normal` (belum ada rumus arah
    ///   push/pull khusus utk tipe ini).
    pub pull_dir: (f64, f64, f64),
}

impl FaceHit {
    /// Titik anchor gizmo push/pull (CADRAW Fase 8 lanjutan) — TIDAK selalu
    /// `centroid`:
    /// - `Plane`: `centroid`, sama seperti perilaku lama (face datar
    ///   biasanya punya region kecil, centroid tetap dekat permukaan).
    /// - selain `Plane` (`Cylinder`/`Cone`/`Sphere`/`Torus`/`Other`):
    ///   `hit_point` — utk selimut silinder/bola penuh, `centroid` Newell
    ///   jatuh di SUMBU (silinder) atau PUSAT bola (bola), yaitu di DALAM
    ///   material. Handle gizmo yg diletakkan di `centroid + pull_dir·18`
    ///   pun ikut terkubur utk radius > 18 mm — tak terlihat & tak bisa
    ///   di-drag. `hit_point` selalu ada DI PERMUKAAN, jadi anchor aman
    ///   utk semua radius.
    pub fn gizmo_anchor(&self) -> (f64, f64, f64) {
        if self.surface_kind == SurfaceKind::Plane {
            self.centroid
        } else {
            self.hit_point
        }
    }
}

/// Tipe permukaan geometris analitik OCCT di balik sebuah `Face`, hasil
/// klasifikasi `Face::surface_kind()` (nama kelas dinamis C++ `Geom_*`).
/// Fase 1: sekadar deteksi/label — belum dipakai fitur apa pun, disiapkan
/// utk fitur mendatang yang berperilaku beda tergantung tipe face (mis.
/// deteksi smart boolean, hint UI khusus silinder/bola).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    /// Permukaan analitik lain (mis. torus non-standar) atau permukaan
    /// bebas/non-analitik (mis. B-Spline, hasil loft/sweep).
    Other,
}

impl From<&str> for SurfaceKind {
    /// Petakan nama kelas dinamis OCCT (keluaran `Face::surface_kind()`)
    /// ke varian enum. Nama yang tidak dikenali (termasuk permukaan
    /// bebas/non-analitik) jatuh ke `Other`, bukan error — klasifikasi ini
    /// sifatnya informasional, tidak boleh gagal.
    fn from(dynamic_type_name: &str) -> Self {
        match dynamic_type_name {
            "Geom_Plane" => SurfaceKind::Plane,
            "Geom_CylindricalSurface" => SurfaceKind::Cylinder,
            "Geom_ConicalSurface" => SurfaceKind::Cone,
            "Geom_SphericalSurface" => SurfaceKind::Sphere,
            "Geom_ToroidalSurface" => SurfaceKind::Torus,
            _ => SurfaceKind::Other,
        }
    }
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

/// Hitung arah satuan push/pull gizmo (`FaceHit::pull_dir`, CADRAW Fase 4)
/// utk `surface_kind` di titik `hit` — lihat dokumentasi field `pull_dir`
/// utk rumus per tipe. `normal` dipakai sbg fallback aman kalau rumus
/// radial tidak terdefinisi (mis. `hit` persis di garis sumbu, atau axis
/// tidak terbaca dari kernel). `ray_dir` dipakai utk koreksi tanda pada
/// kasus Sphere (lihat catatan di bawah).
fn compute_pull_dir(
    face: &Face,
    surface_kind: SurfaceKind,
    hit: DVec3,
    normal: DVec3,
    ray_dir: DVec3,
) -> DVec3 {
    const EPS: f64 = 1e-9;
    match surface_kind {
        SurfaceKind::Plane => normal,
        SurfaceKind::Cylinder | SurfaceKind::Cone => face
            .cylinder_or_cone_axis()
            .and_then(|(axis_location, axis_direction)| {
                let axis_direction = axis_direction.normalize();
                let proj_len = (hit - axis_location).dot(axis_direction);
                let proj_point = axis_location + axis_direction * proj_len;
                let radial = hit - proj_point;
                (radial.length() > EPS).then(|| radial.normalize())
            })
            .unwrap_or(normal),
        SurfaceKind::Sphere => {
            // `hit - centroid` HANYA benar utk bola PENUH, di mana centroid
            // Newell (rata-rata boundary loop tepi, lihat
            // `compute_face_normal_and_centroid`) kebetulan berimpit dgn
            // pusat bola. Utk face bola PARSIAL (sudut hasil fillet bola,
            // bola terpotong boolean) centroid adalah rata-rata boundary
            // loop FACE ITU — BUKAN pusat bola — sehingga `hit - centroid`
            // melenceng dari arah radial sebenarnya. Normal permukaan bola
            // SELALU radial dari pusat, baik penuh maupun parsial (properti
            // geometris bola), jadi pakai `face.normal_at(hit)` langsung
            // tanpa perlu binding gp_Sphere. Tanda dikoreksi spy mengarah
            // keluar dari kamera, sama seperti fallback GProp di
            // `pick_face_details`.
            let mut radial = face.normal_at(hit);
            if radial.length() > EPS {
                radial = radial.normalize();
                if radial.dot(ray_dir) > 0.0 {
                    radial = -radial;
                }
                radial
            } else {
                normal
            }
        }
        SurfaceKind::Torus | SurfaceKind::Other => normal,
    }
}

/// Cast `ray` ke `shape`, kembalikan detail face (hit point, centroid, normal keluar, arah push/pull) terdekat.
pub fn pick_face_details(shape: &KernelShape, ray: PickRay) -> Option<FaceHit> {
    let _guard = lock_kernel();
    let (face, hit) = resolve_face_along_ray(&shape.0, ray)?;
    let surface_kind = SurfaceKind::from(face.surface_kind().as_str());
    // Newell's method di atas boundary loop tepi (`compute_face_normal_and_
    // centroid`) SELALU gagal (`None`) utk permukaan tertutup tanpa loop
    // tepi sederhana — kasus nyata: bola PENUH (1 face, seam + 2 pole
    // degenerate, BUKAN loop tepi biasa; lihat catatan panjang di
    // `extrude_face_sphere_grows_radius_when_pulled_out`). Sebelum Fase 4,
    // ini artinya bola tidak bisa DIPILIH sama sekali di viewport (`None`
    // menembus sampai `pick_face_details`) — Fase 4 butuh FaceHit yang
    // valid utk bola (utk `pull_dir` radial), jadi fallback ke GProp-based
    // `center_of_mass`/`normal_at` yang robust utk SEMUA tipe permukaan
    // (tidak bergantung topologi loop tepi) dipakai kalau Newell gagal.
    let (normal, centroid) = compute_face_normal_and_centroid(&face, ray.dir_vec()).unwrap_or_else(|| {
        let centroid = face.center_of_mass();
        // PENTING: proyeksikan `hit` (titik yang TERBUKTI ada di permukaan,
        // hasil ray-face intersection), BUKAN `centroid` — utk bola penuh
        // `centroid` adalah PUSAT bola, dan memproyeksikan pusat bola ke
        // permukaannya sendiri adalah kasus degenerate (berjarak sama ke
        // SEMUA titik permukaan): `GeomAPI_ProjectPointOnSurf` gagal
        // menemukan solusi tunggal, dan `LowerDistanceParameters` OCCT
        // melempar `Standard_OutOfRange` yang TIDAK tertangkap cxx (abort
        // proses via `std::terminate`, dibuktikan lewat crash nyata saat
        // pengembangan Fase 4) — proyeksi di `hit` selalu well-defined
        // karena `hit` memang titik pada permukaan itu sendiri.
        let mut normal = face.normal_at(hit);
        if normal.dot(ray.dir_vec()) > 0.0 {
            normal = -normal;
        }
        (normal, centroid)
    });
    let pull_dir = compute_pull_dir(&face, surface_kind, hit, normal, ray.dir_vec());
    Some(FaceHit {
        hit_point: (hit.x, hit.y, hit.z),
        centroid: (centroid.x, centroid.y, centroid.z),
        normal: (normal.x, normal.y, normal.z),
        surface_kind,
        pull_dir: (pull_dir.x, pull_dir.y, pull_dir.z),
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
///
/// Dispatch per tipe permukaan (`SurfaceKind`, CADRAW Fase 3):
/// - **`Plane`**: jalur ASLI (extrude+union/subtract) — TIDAK disentuh sama
///   sekali, sudah teruji (termasuk catatan deadlock `KERNEL_LOCK` di
///   bawah).
/// - **`Cylinder`/`Cone`/`Sphere`/`Torus`/`Other`**: jalur baru,
///   `Shape::offset_on_face` (`BRepOffset_MakeOffset::SetOffsetOnFace`
///   langsung pada face terpilih). Extrude+boolean TIDAK cocok untuk wajah
///   lengkung — hasil extrude wajah lengkung adalah *swept surface* baru
///   (bukan silinder/kerucut/bola dengan radius berbeda), jadi geometri
///   hasil union/subtract-nya tidak akan tetap analitik/lengkung. Offset
///   per-face menghasilkan LANGSUNG solid baru (bukan dua langkah).
pub fn extrude_face(shape: &KernelShape, ray: PickRay, distance: f64) -> Result<KernelShape> {
    if distance.abs() < 1e-9 {
        bail!("jarak extrude face harus tidak nol");
    }
    let _guard = lock_kernel();
    let cloned = deep_clone(&shape.0)?;
    let Some((face, _)) = resolve_face_along_ray(&cloned, ray) else {
        bail!("wajah terpilih tidak ditemukan pada shape");
    };

    if SurfaceKind::from(face.surface_kind().as_str()) != SurfaceKind::Plane {
        // Validasi batas SEBELUM memanggil OCCT — cuma bisa presisi untuk
        // Cylinder/Cone (`cylinder_or_cone_radius`, lihat catatan di sana
        // soal Sphere/Torus/Other belum ada binding radius-nya). Untuk
        // tipe yang tidak bisa di-precheck, `offset_on_face` sendiri akan
        // `bail!` lewat `Result::Err`/`IsDone()==false` OCCT kalau offset
        // membuat geometri kolaps (mis. radius efektif ≤ 0).
        if let Some(current_radius) = face.cylinder_or_cone_radius() {
            // Arah pengaruh `distance` terhadap radius BALIK tergantung
            // orientasi face: pada wajah CEMBUNG normal-keluar (Forward,
            // mis. selimut luar silinder) `distance` positif MENAMBAH
            // radius (R+d). Pada wajah CEKUNG (Reversed, mis. dinding
            // lubang hasil subtract) normal-keluar-dari-material mengarah
            // ke sumbu, jadi `distance` positif MENGECILKAN radius (R-d)
            // — dibuktikan test `extrude_face_hollow_cylinder_inner_wall_
            // shrinks_hole_when_pushed_radially_inward` (R=8 → 6 saat
            // distance=+2). Pakai tanda yang salah (selalu R+d) membuat
            // precheck ini menolak operasi VALID (memperbesar lubang lewat
            // distance negatif) dan meloloskan operasi yang harusnya
            // ditolak (lubang menutup penuh), persis kebalikan dari
            // tujuannya.
            let new_radius = match face.orientation() {
                FaceOrientation::Reversed => current_radius - distance,
                _ => current_radius + distance,
            };
            if new_radius <= 0.0 {
                bail!(
                    "jarak drag ({distance:.3} mm) akan membuat radius permukaan ≤ 0 (radius saat ini {current_radius:.3} mm)"
                );
            }
        }
        let offset_shape = cloned
            .offset_on_face(&face, distance)
            .context("gagal melakukan offset pada permukaan lengkung terpilih")?;
        return Ok(KernelShape(offset_shape));
    }

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
    // di terminal). Replikasi langsung isi `union`/`subtract` (termasuk
    // `.clean()`-nya, lihat catatan di `union`) di sini, pola sama dengan
    // komentar `intersect` di atas.
    if distance > 0.0 {
        let mut merged = cloned.union(&swept_shape).shape;
        merged.clean();
        Ok(KernelShape(merged))
    } else {
        let mut result = cloned.subtract(&swept_shape).shape;
        result.clean();
        Ok(KernelShape(result))
    }
}

/// Deep-clone publik sebuah shape — dipakai app untuk menyimpan snapshot
/// B-rep (mis. shape dasar SEBELUM rounding parametrik pertama, supaya
/// radius bisa diubah/di-nol-kan lagi dengan rebuild dari dasar) tanpa
/// membuka akses ke `deep_clone` internal maupun detail locking kernel.
pub fn clone_shape(shape: &KernelShape) -> Result<KernelShape> {
    let _guard = lock_kernel();
    Ok(KernelShape(deep_clone(&shape.0)?))
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

/// (titik tengah dunia edge di arc-length setengah panjangnya, titik AWAL
/// dan AKHIR edge, panjang total edge) — satu entri per edge topologi
/// shape. `start`/`end` disertakan (bukan cuma titik tengah) supaya
/// pemanggil (cadraw-app) bisa menghitung sudut layar rusuknya SETELAH
/// diproyeksikan kamera — label dimensi dengan begitu bisa disejajarkan ke
/// arah rusuknya sendiri, ikut berubah sudut saat kamera diputar, sama
/// seperti pill dimensi entitas sketsa 2D.
pub type EdgeDimension = ((f64, f64, f64), (f64, f64, f64), (f64, f64, f64), f64);

/// Panjang + titik tengah SEMUA edge shape, dipakai fitur "Tampilkan
/// Semua Ukuran" (checkbox ruler properties, cadraw-app) untuk melabeli
/// tiap rusuk 3D tanpa perlu ray picking satu-satu. Dipanggil SEKALI saat
/// geometri body dibuat/berubah (lihat `BodyGeometry::from_shape` di
/// cadraw-app), bukan tiap frame render — traversal `shape.edges()` +
/// `approximation_segments()` di sini punya biaya sama dengan
/// `resolve_edge_along_ray`/`collect_vertices` di atas.
///
/// `shape.edges()` (seperti dicatat `collect_vertices` di atas) memuat
/// SETIAP edge per wajah yang memakainya, bukan sekali per topologi B-rep
/// — rusuk yang dipakai 2 wajah (kasus normal utk padatan tertutup) muncul
/// 2x dgn endpoint sama persis. Di-dedup pakai epsilon jarak endpoint yang
/// SAMA dgn `collect_vertices` (kedua urutan start/end dicek, karena arah
/// parametrisasi edge bisa terbalik antar wajah), supaya tiap rusuk cuma
/// dapat satu label, bukan dobel menumpuk.
///
/// Titik tengah dihitung berdasarkan ARC-LENGTH (jarak separuh panjang
/// polyline aproksimasi), bukan `(start+end)/2`, supaya label tetap jatuh
/// DI ATAS edge untuk rusuk melengkung (Arc/Circle hasil revolve/fillet),
/// bukan melayang di korda lurus antar endpoint-nya.
pub fn edge_dimensions(shape: &KernelShape) -> Vec<EdgeDimension> {
    let _guard = lock_kernel();
    const DEDUP_EPS: f64 = 1e-6;
    let mut seen_endpoints: Vec<(DVec3, DVec3)> = Vec::new();
    let mut out = Vec::new();

    for edge in shape.0.edges() {
        let start = edge.start_point();
        let end = edge.end_point();
        let is_duplicate = seen_endpoints.iter().any(|(a, b)| {
            ((start - *a).length() < DEDUP_EPS && (end - *b).length() < DEDUP_EPS)
                || ((start - *b).length() < DEDUP_EPS && (end - *a).length() < DEDUP_EPS)
        });
        if is_duplicate {
            continue;
        }
        seen_endpoints.push((start, end));

        let polyline: Vec<DVec3> = edge.approximation_segments().collect();
        if polyline.len() < 2 {
            continue;
        }
        let seg_lens: Vec<f64> = polyline.windows(2).map(|w| (w[1] - w[0]).length()).collect();
        let length: f64 = seg_lens.iter().sum();
        if length <= 1e-9 {
            continue;
        }
        let half = length * 0.5;
        let mut acc = 0.0;
        let mut mid = polyline[0];
        for (seg_len, w) in seg_lens.iter().zip(polyline.windows(2)) {
            if acc + seg_len >= half {
                let t = if *seg_len > 1e-9 { (half - acc) / seg_len } else { 0.0 };
                mid = w[0] + (w[1] - w[0]) * t;
                break;
            }
            acc += seg_len;
            mid = w[1];
        }
        out.push((
            (mid.x, mid.y, mid.z),
            (start.x, start.y, start.z),
            (end.x, end.y, end.z),
            length,
        ));
    }
    out
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
    let max_radius = max_fillet_radius(&cloned, &edges);
    if radius > max_radius {
        bail!("radius fillet ({radius:.2} mm) melebihi batas geometris tepi terpilih (maks {max_radius:.2} mm)");
    }
    cloned
        .fillet_edges(radius, &edges)
        .context("radius fillet terlalu besar untuk tepi terpilih (mis. melebihi batas ujung objek)")?;
    Ok(KernelShape(cloned))
}

/// Cast `ray` ke `shape`, kembalikan titik vertex (sudut) terdekat kalau
/// ada dalam `tolerance` mm dari ray. Dipakai UI utk hover/klik gizmo
/// vertex fillet (rounded sudut 3D, beda dari `pick_edge` yang menyasar
/// RUSUK).
pub fn pick_vertex(shape: &KernelShape, ray: PickRay, tolerance: f64) -> Option<(f64, f64, f64)> {
    let _guard = lock_kernel();
    resolve_vertex_along_ray(&shape.0, ray, tolerance).map(|p| (p.x, p.y, p.z))
}

/// Radius maksimum yang MASUK AKAL secara geometris utk fillet 1 sudut
/// (vertex) — cermin batas `max_fillet_radius` yang dipakai `fillet_vertex`
/// sebagai precheck, tapi diekspos publik SUPAYA UI (`cadraw-app`) bisa
/// mengunci gizmo rounding SELAGI drag (bukan cuma menolak SETELAH radius
/// terlanjur jauh melebihi batas). Tanpa ini, nilai radius internal gizmo
/// terus bertambah tanpa batas selagi mouse bergerak walau preview-nya
/// sudah berhenti berubah (`fillet_vertex` mulai `Err`) — begitu drag
/// dilepas, commit akan GAGAL total (sudut balik siku, bukan berhenti di
/// radius maksimum) karena radius yang dikirim sudah kadung di atas batas.
/// `None` kalau vertex tidak ditemukan lagi pada `shape` (ray usang) atau
/// tidak ada tepi yang bertemu di situ.
pub fn max_vertex_fillet_radius(shape: &KernelShape, ray: PickRay, tolerance: f64) -> Option<f64> {
    let _guard = lock_kernel();
    let vertex = resolve_vertex_along_ray(&shape.0, ray, tolerance)?;
    const COINCIDENT_EPS: f64 = 1e-6;
    let edges: Vec<Edge> = shape
        .0
        .edges()
        .filter(|edge| {
            (edge.start_point() - vertex).length() < COINCIDENT_EPS
                || (edge.end_point() - vertex).length() < COINCIDENT_EPS
        })
        .collect();
    if edges.is_empty() {
        return None;
    }
    Some(max_fillet_radius(&shape.0, &edges))
}

/// Cermin `max_vertex_fillet_radius`, tapi utk 1 tepi (edge) spesifik hasil
/// pick via `ray` (gizmo edge fillet, "klik rusuk pojok kubus") — dipakai
/// UI mengunci `edge_gizmo_radius` selagi drag, sama alasannya. `None`
/// kalau tepi tidak ditemukan lagi pada `shape`.
pub fn max_edge_fillet_radius(shape: &KernelShape, ray: PickRay, tolerance: f64) -> Option<f64> {
    let _guard = lock_kernel();
    let (edge, _, _) = resolve_edge_along_ray(&shape.0, ray, tolerance)?;
    Some(max_fillet_radius(&shape.0, &[edge]))
}

/// Semua vertex (sudut) unik dari `shape`, dedup sama seperti dipakai
/// `pick_vertex`/`fillet_vertex` (lihat `collect_vertices`). Dipakai UI utk
/// menggambar marker kecil di tiap sudut body saat mode 3D — tanpa marker
/// terlihat, target picking vertex yang kecil secara visual praktis tidak
/// bisa ditemukan user (lihat juga toleransi longgar di `pick_vertex`).
pub fn shape_vertices(shape: &KernelShape) -> Vec<(f64, f64, f64)> {
    let _guard = lock_kernel();
    collect_vertices(&shape.0).into_iter().map(|v| (v.x, v.y, v.z)).collect()
}

/// Fillet SEMUA tepi yang bertemu di 1 vertex (sudut) yang di-pick lewat
/// `ray` — beda dari `fillet_edges` yang fillet tepi spesifik hasil pick:
/// di sini user klik SUDUT, kernel yang mencari sendiri tepi-tepi yang
/// bertemu di situ. `ray` di-cast ULANG terhadap shape hasil `deep_clone`
/// (pola sama dgn `fillet_edges`, lihat desain di `PickRay`) buat
/// resolusi vertex yang valid pada clone, lalu tepi-tepi yang
/// `start_point`/`end_point`-nya berimpit (epsilon ~1e-6 mm, SAMA dengan
/// dedup di `resolve_vertex_along_ray`) dengan vertex itu dikumpulkan dan
/// di-fillet SEKALIGUS lewat `Shape::fillet_edges` — OCCT sendiri yang
/// menghasilkan sudut membulat (spherical corner) ketika >1 tepi yang
/// bertemu di 1 titik difillet bersamaan dengan radius sama.
pub fn fillet_vertex(shape: &KernelShape, radius: f64, ray: PickRay, tolerance: f64) -> Result<KernelShape> {
    if radius <= 0.0 {
        bail!("radius fillet harus > 0");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(&shape.0)?;
    let Some(vertex) = resolve_vertex_along_ray(&cloned, ray, tolerance) else {
        bail!("sudut (vertex) terpilih tidak ditemukan lagi pada shape");
    };

    const COINCIDENT_EPS: f64 = 1e-6;
    let edges: Vec<Edge> = cloned
        .edges()
        .filter(|edge| {
            (edge.start_point() - vertex).length() < COINCIDENT_EPS
                || (edge.end_point() - vertex).length() < COINCIDENT_EPS
        })
        .collect();
    if edges.is_empty() {
        bail!("tidak ada tepi yang bertemu di sudut terpilih");
    }

    let max_radius = max_fillet_radius(&cloned, &edges);
    if radius > max_radius {
        bail!("radius fillet ({radius:.2} mm) melebihi batas geometris sudut terpilih (maks {max_radius:.2} mm)");
    }
    cloned
        .fillet_edges(radius, &edges)
        .context("radius fillet terlalu besar untuk sudut terpilih (mis. melebihi batas ujung objek)")?;
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
    let max_distance = max_fillet_radius(&cloned, &edges);
    if distance > max_distance {
        bail!("jarak chamfer ({distance:.2} mm) melebihi batas geometris tepi terpilih (maks {max_distance:.2} mm)");
    }
    cloned
        .chamfer_edges(distance, &edges)
        .context("jarak chamfer terlalu besar untuk tepi terpilih (mis. melebihi batas ujung objek)")?;
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
        shape
            .fillet(fillet)
            .context("radius fillet terlalu besar untuk kotak smoke-test ini")?;
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
    fn clone_shape_independent_of_original() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(25.0, 15.0), 10.0).unwrap();
        let snapshot = clone_shape(&shape).unwrap();
        // Fillet hasil clone TIDAK boleh menyentuh snapshot maupun shape
        // asli — inti pemakaian `clone_shape` sbg base rounding parametrik.
        let filleted = fillet_all(&snapshot, 2.0).unwrap();
        assert!(filleted.tessellate().triangle_count() > 0);
        assert_eq!(
            shape.tessellate().positions.len(),
            snapshot.tessellate().positions.len()
        );
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

    /// Box hasil extrude rect 30x20 tinggi 15 punya 12 rusuk: 4 bawah
    /// (dua @30, dua @20), 4 atas (sama), 4 vertikal (@15) — dipakai fitur
    /// "Tampilkan Semua Ukuran" (checkbox ruler properties, cadraw-app).
    #[test]
    fn edge_dimensions_reports_all_box_edges() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let dims = edge_dimensions(&shape);
        assert_eq!(dims.len(), 12, "box punya 12 rusuk topologi");

        let mut lengths: Vec<f64> = dims.iter().map(|(_, _, _, len)| *len).collect();
        lengths.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let expected: [f64; 12] = [15.0, 15.0, 15.0, 15.0, 20.0, 20.0, 20.0, 20.0, 30.0, 30.0, 30.0, 30.0];
        for (got, want) in lengths.iter().zip(expected.iter()) {
            assert!((got - want).abs() < 1e-3, "panjang rusuk {} tidak cocok dgn {}", got, want);
        }

        for ((mx, my, mz), start, end, length) in &dims {
            // Titik tengah, start, dan end tiap rusuk harus jatuh di
            // dalam/pada bounding box shape (0..30, 0..20, 0..15) — bukan
            // di luar jangkauan geometri.
            assert!((-1e-3..=30.0 + 1e-3).contains(mx));
            assert!((-1e-3..=20.0 + 1e-3).contains(my));
            assert!((-1e-3..=15.0 + 1e-3).contains(mz));

            // Rusuk box selalu lurus — jarak start↔end (korda) harus sama
            // dgn `length` (arc-length polyline), dan `mid` harus persis
            // di tengah start↔end. Ini validasi utama field baru
            // `start`/`end` (dipakai app menghitung sudut layar rusuk).
            let chord = ((end.0 - start.0).powi(2) + (end.1 - start.1).powi(2) + (end.2 - start.2).powi(2)).sqrt();
            assert!((chord - length).abs() < 1e-3, "korda {} vs panjang {} beda jauh utk rusuk lurus", chord, length);
            assert!((mx - (start.0 + end.0) * 0.5).abs() < 1e-3);
            assert!((my - (start.1 + end.1) * 0.5).abs() < 1e-3);
            assert!((mz - (start.2 + end.2) * 0.5).abs() < 1e-3);
        }
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

    // ---- Vertex Fillet Gizmo: picking vertex (sudut) 3D ----

    #[test]
    fn pick_vertex_on_box_corner() {
        let _guard = TEST_LOCK.lock().unwrap();
        // Box x∈[0,30], y∈[0,20], z∈[0,15]; ray diagonal dari luar
        // menuju sudut box di titik asal (0,0,0).
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, -5.0),
            dir: (1.0, 1.0, 1.0),
        };
        let hit = pick_vertex(&shape, ray, 1.0).expect("harus kena sudut box di (0,0,0)");
        assert!(hit.0.abs() < 1e-3);
        assert!(hit.1.abs() < 1e-3);
        assert!(hit.2.abs() < 1e-3);
    }

    /// Validasi arsitektur WAJIB versi vertex (lihat catatan test
    /// `pick_face_consistent_across_deep_clone`/`pick_edge_consistent_across_deep_clone`
    /// di atas — dasar yang sama kenapa `PickRay` aman dipakai lintas
    /// roundtrip STEP juga berlaku utk vertex picking).
    #[test]
    fn pick_vertex_consistent_across_deep_clone() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, -5.0),
            dir: (1.0, 1.0, 1.0),
        };
        let tolerance = 1.0;
        let hit_original = pick_vertex(&shape, ray, tolerance).expect("harus kena sudut shape asli");
        let cloned = deep_clone(&shape.0).unwrap();
        let hit_cloned = resolve_vertex_along_ray(&cloned, ray, tolerance)
            .map(|p| (p.x, p.y, p.z))
            .expect("harus kena sudut shape hasil deep_clone");
        assert!((hit_original.0 - hit_cloned.0).abs() < 1e-6);
        assert!((hit_original.1 - hit_cloned.1).abs() < 1e-6);
        assert!((hit_original.2 - hit_cloned.2).abs() < 1e-6);
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

    // ---- Vertex Fillet Gizmo: fillet SEMUA tepi yang bertemu di 1 sudut ----

    #[test]
    fn fillet_vertex_rounds_box_corner() {
        let _guard = TEST_LOCK.lock().unwrap();
        // Box x∈[0,30], y∈[0,20], z∈[0,15]; ray diagonal menuju sudut
        // box di titik asal (0,0,0) — 3 tepi bertemu di sana.
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, -5.0),
            dir: (1.0, 1.0, 1.0),
        };
        let base_volume = shape.0.volume();
        let filleted = fillet_vertex(&shape, 2.0, ray, 1.0).unwrap();
        assert!(
            filleted.0.volume() < base_volume,
            "membulatkan sudut harus memotong material (volume berkurang)"
        );
        assert!(filleted.tessellate().triangle_count() > 0);
        // Shape asli tidak boleh ikut termutasi (pola sama dgn fillet_edges/fillet_all).
        assert!((shape.0.volume() - base_volume).abs() < 1e-6);
    }

    #[test]
    fn fillet_vertex_zero_radius_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, -5.0),
            dir: (1.0, 1.0, 1.0),
        };
        assert!(fillet_vertex(&shape, 0.0, ray, 1.0).is_err());
    }

    #[test]
    fn fillet_vertex_no_match_errors() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (1000.0, 1000.0, 1000.0),
            dir: (0.0, 0.0, 1.0),
        };
        assert!(fillet_vertex(&shape, 2.0, ray, 1.0).is_err());
    }

    // ---- Regresi: radius rounding melebihi batas geometri objek ----
    // User melaporkan drag gizmo rounding (fillet) sampai batas ujung
    // objek membuat SELURUH aplikasi langsung close, log `libc++abi:
    // terminating due to uncaught exception of type StdFail_NotDone`.
    // Root cause: `BRepFilletAPI_MakeFillet::Shape()` melempar
    // `StdFail_NotDone` (turunan `Standard_Failure`, BUKAN
    // `std::exception`) kalau build fillet gagal — sebelum patch
    // `Shape::fillet_edges`/`fillet_vertex` (lihat `vendor/README.md`)
    // exception itu TEMBUS lewat cxx dan `std::terminate` (abort proses),
    // bukan `Result::Err` yang bisa ditangani. Kalau patch ini regresi,
    // kedua test di bawah SIGABRT alih-alih gagal assert biasa.
    #[test]
    fn fillet_edges_oversized_radius_errors_not_crashes() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, 7.5),
            dir: (1.0, 1.0, 0.0),
        };
        // Radius 1000 mm jauh melebihi semua dimensi box (30×20×15).
        let result = fillet_edges(&shape, 1000.0, &[ray], 1.0);
        assert!(result.is_err(), "radius jauh melebihi ukuran box harus ditolak sbg Err, bukan sukses/crash");
    }

    #[test]
    fn fillet_vertex_oversized_radius_errors_not_crashes() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, -5.0),
            dir: (1.0, 1.0, 1.0),
        };
        let result = fillet_vertex(&shape, 1000.0, ray, 1.0);
        assert!(result.is_err(), "radius jauh melebihi ukuran box harus ditolak sbg Err, bukan sukses/crash");
    }

    #[test]
    fn chamfer_edges_oversized_distance_errors_not_crashes() {
        // Cermin test fillet di atas — jalur kode `Shape::chamfer_edges`
        // sama-sama lewat `BRepFilletAPI_MakeChamfer::Shape()`.
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, 7.5),
            dir: (1.0, 1.0, 0.0),
        };
        let result = chamfer_edges(&shape, 1000.0, &[ray], 1.0);
        assert!(result.is_err(), "jarak chamfer jauh melebihi ukuran box harus ditolak sbg Err, bukan sukses/crash");
    }

    // ---- Regresi: radius rounding "sukses" tapi melebihi batas wajar ----
    // Susulan laporan di atas: setelah fix crash `StdFail_NotDone`, user
    // melaporkan LAGI — drag gizmo rounding sampai "batas ujung objek"
    // masih tidak berhenti, hasilnya sudut box "memakan" seluruh sisi jadi
    // bentuk baji/quarter-cylinder (screenshot), BUKAN sudut membulat
    // wajar. OCCT sendiri masih `IsDone()==true` di radius segini (beda
    // dari test *_oversized_*_not_crashes di atas yang radiusnya 1000mm,
    // cukup ekstrem utk memicu StdFail_NotDone OCCT sendiri) — makanya
    // butuh precheck geometris manual `max_fillet_radius` SEBELUM manggil
    // OCCT, bukan cuma andalkan `IsDone()`/exception OCCT.
    #[test]
    fn fillet_vertex_radius_near_full_shortest_edge_succeeds() {
        // Regresi susulan: versi AWAL `max_fillet_radius` salah pakai
        // `/2.0` (kelewat konservatif — dilaporkan user lewat screenshot,
        // gizmo berhenti baru separuh jalan). Box 30×20×15; sudut atas: 2
        // tepi horizontal (30, 20) + 1 tepi vertikal (15) bertemu — batas
        // SEKARANG = panjang tepi terpendek itu sendiri (15mm, BUKAN
        // 7.5mm). Radius 14mm (dekat 15 tapi masih di bawahnya) HARUS
        // sukses — kalau formula `/2.0` regresi lagi, test ini gagal
        // (radius 14 > cap lama 7.5).
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, -5.0),
            dir: (1.0, 1.0, 1.0),
        };
        let result = fillet_vertex(&shape, 14.0, ray, 1.0);
        assert!(result.is_ok(), "radius 14mm mendekati tepi terpendek (15mm) harus tetap sukses: {}", result.as_ref().err().map(|e| e.to_string()).unwrap_or_default());
    }

    #[test]
    fn fillet_vertex_radius_exceeding_shortest_edge_errors_before_reaching_occt() {
        // Cermin test di atas — radius 20mm SENGAJA > 15 (batas BARU,
        // bukan lagi 7.5) tapi jauh < 1000 (radius yg sudah teruji bikin
        // OCCT sendiri gagal di test `*_oversized_*`) — murni menguji
        // precheck geometris, bukan crash fix lama.
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, -5.0),
            dir: (1.0, 1.0, 1.0),
        };
        let result = fillet_vertex(&shape, 20.0, ray, 1.0);
        assert!(
            result.is_err(),
            "radius 20mm pada sudut dgn tepi terpendek 15mm (batas 15mm) harus ditolak"
        );
    }

    #[test]
    fn fillet_edges_radius_near_full_shortest_touching_edge_succeeds() {
        // Cermin `fillet_vertex_radius_near_full_shortest_edge_succeeds`,
        // tapi lewat `fillet_edges` (pick 1 tepi spesifik via ray, bukan
        // gizmo sudut) — jalur kode berbeda, precheck geometris sama.
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, 7.5),
            dir: (1.0, 1.0, 0.0),
        };
        let result = fillet_edges(&shape, 14.0, &[ray], 1.0);
        assert!(result.is_ok(), "radius 14mm mendekati tepi terpendek (15mm) harus tetap sukses: {}", result.as_ref().err().map(|e| e.to_string()).unwrap_or_default());
    }

    #[test]
    fn fillet_edges_radius_exceeding_shortest_touching_edge_errors_before_reaching_occt() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
        let ray = PickRay {
            origin: (-5.0, -5.0, 7.5),
            dir: (1.0, 1.0, 0.0),
        };
        let result = fillet_edges(&shape, 20.0, &[ray], 1.0);
        assert!(
            result.is_err(),
            "radius 20mm pada tepi vertikal (15mm) yg bertemu tepi 30/20mm harus tetap ditolak (batas 15mm)"
        );
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

    #[test]
    fn surface_kind_detects_plane_faces_on_cube() {
        let _guard = TEST_LOCK.lock().unwrap();
        let cube = AdHocShape::make_box(10.0, 10.0, 10.0);
        let faces: Vec<_> = cube.faces().collect();
        assert_eq!(faces.len(), 6, "kubus harus punya 6 face");
        for face in &faces {
            assert_eq!(SurfaceKind::from(face.surface_kind().as_str()), SurfaceKind::Plane);
        }
    }

    #[test]
    fn surface_kind_detects_plane_and_cylinder_faces_on_cylinder() {
        let _guard = TEST_LOCK.lock().unwrap();
        let cylinder = AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), 5.0, 12.0);
        let mut plane_count = 0;
        let mut cylinder_count = 0;
        for face in cylinder.faces() {
            match SurfaceKind::from(face.surface_kind().as_str()) {
                SurfaceKind::Plane => plane_count += 1,
                SurfaceKind::Cylinder => cylinder_count += 1,
                other => panic!("face silinder tak terduga: {other:?}"),
            }
        }
        assert_eq!(plane_count, 2, "silinder harus punya 2 face Plane (tutup atas & bawah)");
        assert_eq!(cylinder_count, 1, "silinder harus punya 1 face Cylinder (selimut)");
    }

    #[test]
    fn surface_kind_detects_sphere_face() {
        let _guard = TEST_LOCK.lock().unwrap();
        let sphere = AdHocShape::make_sphere(7.0);
        let faces: Vec<_> = sphere.faces().collect();
        assert_eq!(faces.len(), 1, "bola harus punya 1 face");
        assert_eq!(SurfaceKind::from(faces[0].surface_kind().as_str()), SurfaceKind::Sphere);
    }

    // ---- CADRAW Fase 3: extrude_face dispatch per SurfaceKind ----
    //
    // Toleransi volume `1e-6` (relatif) dipakai di seluruh test volume di
    // bawah ini — bukan asal longgar, tapi hasil OCCT utk kasus Cylinder/
    // Sphere/dinding lubang TERBUKTI cocok exact (bit-level dekat) dengan
    // formula analitik saat diverifikasi manual, jadi toleransi ketat di
    // sini justru pembuktian jalur offset per-face benar-benar presisi,
    // bukan "kira-kira".

    fn assert_close(actual: f64, expected: f64, label: &str) {
        let rel_diff = (actual - expected).abs() / expected.abs().max(1e-9);
        assert!(
            rel_diff < 1e-6,
            "{label}: actual={actual}, expected={expected}, rel_diff={rel_diff}"
        );
    }

    #[test]
    fn extrude_face_cylinder_outer_wall_grows_radius_when_pulled_out() {
        let _guard = TEST_LOCK.lock().unwrap();
        const R: f64 = 10.0;
        const H: f64 = 20.0;
        let cylinder = KernelShape(AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R, H).0);
        // Ray dari luar, tegak lurus menuju selimut silinder (bukan tutup).
        let ray = PickRay { origin: (R + 50.0, 0.0, H / 2.0), dir: (-1.0, 0.0, 0.0) };
        let hit = pick_face_details(&cylinder, ray).expect("harus kena selimut silinder");
        assert_eq!(hit.surface_kind, SurfaceKind::Cylinder);

        let grown = extrude_face(&cylinder, ray, 2.0).expect("pull +2 pada selimut silinder harus berhasil");
        assert_close(grown.0.volume(), std::f64::consts::PI * 12.0 * 12.0 * H, "volume silinder R=12,h=20");
    }

    #[test]
    fn extrude_face_cylinder_outer_wall_shrinks_radius_when_pulled_in() {
        let _guard = TEST_LOCK.lock().unwrap();
        const R: f64 = 10.0;
        const H: f64 = 20.0;
        let cylinder = KernelShape(AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R, H).0);
        let ray = PickRay { origin: (R + 50.0, 0.0, H / 2.0), dir: (-1.0, 0.0, 0.0) };

        let shrunk = extrude_face(&cylinder, ray, -3.0).expect("push -3 pada selimut silinder harus berhasil");
        assert_close(shrunk.0.volume(), std::f64::consts::PI * 7.0 * 7.0 * H, "volume silinder R=7,h=20");
    }

    #[test]
    fn extrude_face_cylinder_outer_wall_rejects_offset_making_radius_non_positive() {
        let _guard = TEST_LOCK.lock().unwrap();
        const R: f64 = 10.0;
        const H: f64 = 20.0;
        let cylinder = KernelShape(AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R, H).0);
        let ray = PickRay { origin: (R + 50.0, 0.0, H / 2.0), dir: (-1.0, 0.0, 0.0) };

        // -10 persis membuat radius baru = 0 — harus ditolak jelas (bail!),
        // BUKAN diteruskan ke OCCT dan gagal telat/ambigu. `KernelShape`
        // sengaja tidak `Debug` (lihat dokumentasi struct), jadi pakai
        // `match` manual alih-alih `expect_err`.
        match extrude_face(&cylinder, ray, -10.0) {
            Ok(_) => panic!("radius jadi 0 harus ditolak"),
            Err(err) => assert!(err.to_string().contains("radius"), "pesan error harus jelas soal radius: {err}"),
        }
    }

    #[test]
    fn extrude_face_hollow_cylinder_inner_wall_shrinks_hole_when_pushed_radially_inward() {
        let _guard = TEST_LOCK.lock().unwrap();
        const R_OUT: f64 = 20.0;
        const R_IN: f64 = 8.0;
        const H: f64 = 20.0;
        // Tabung (outer minus inner, inner sedikit lebih tinggi utk potongan
        // bersih di kedua tutup) — dinding DALAM (lubang) adalah face
        // Cylinder terpisah dari dinding luar.
        let outer = AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R_OUT, H);
        let inner = AdHocShape::make_cylinder(dvec3(0.0, 0.0, -1.0), R_IN, H + 2.0);
        let mut tube_shape = outer.0.subtract(&inner.0).shape;
        tube_shape.clean();
        let tube = KernelShape(tube_shape);

        // Ray dari sumbu (di dalam lubang) menuju radial keluar — hit
        // terdekat HARUS dinding dalam, bukan dinding luar.
        let hole_ray = PickRay { origin: (0.0, 0.0, H / 2.0), dir: (1.0, 0.0, 0.0) };
        let hit = pick_face_details(&tube, hole_ray).expect("harus kena dinding lubang");
        assert_eq!(hit.surface_kind, SurfaceKind::Cylinder);

        // Dorong dinding lubang radial ke dalam (`distance` positif, sama
        // arah dengan "drag keluar" pada wajah cembung) — utk wajah CEKUNG
        // (dinding lubang), normal-keluar-dari-material OCCT mengarah ke
        // sumbu, jadi ini MENGECILKAN lubang (radius 8 → 6), menambah
        // volume material.
        let shrunk_hole = extrude_face(&tube, hole_ray, 2.0).expect("offset dinding lubang harus berhasil");
        let expect_vol = std::f64::consts::PI * (R_OUT * R_OUT - 6.0 * 6.0) * H;
        assert_close(shrunk_hole.0.volume(), expect_vol, "volume tabung dgn lubang R=6 (mengecil dari R=8)");
    }

    /// Regresi: precheck radius dinding lubang sebelum ini selalu memakai
    /// `current_radius + distance`, sama seperti wajah cembung. Untuk wajah
    /// CEKUNG (Reversed) rumus yang benar adalah `current_radius -
    /// distance` (lihat komentar di `extrude_face`) — jadi `distance`
    /// NEGATIF pada dinding lubang justru MEMBESARKAN lubang. Dengan tanda
    /// lama, kasus ini (radius baru 13 > radius awal 8, jelas valid)
    /// ditolak keliru dengan pesan "radius ≤ 0" yang menyesatkan.
    #[test]
    fn extrude_face_hollow_cylinder_inner_wall_enlarges_past_original_radius_when_pulled_radially_outward() {
        let _guard = TEST_LOCK.lock().unwrap();
        const R_OUT: f64 = 20.0;
        const R_IN: f64 = 8.0;
        const H: f64 = 20.0;
        let outer = AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R_OUT, H);
        let inner = AdHocShape::make_cylinder(dvec3(0.0, 0.0, -1.0), R_IN, H + 2.0);
        let mut tube_shape = outer.0.subtract(&inner.0).shape;
        tube_shape.clean();
        let tube = KernelShape(tube_shape);

        let hole_ray = PickRay { origin: (0.0, 0.0, H / 2.0), dir: (1.0, 0.0, 0.0) };
        let hit = pick_face_details(&tube, hole_ray).expect("harus kena dinding lubang");
        assert_eq!(hit.surface_kind, SurfaceKind::Cylinder);

        // `distance` NEGATIF pada dinding lubang menarik dinding menjauh
        // dari sumbu -> lubang membesar (radius 8 -> 13), MELEBIHI radius
        // awal. Operasi ini valid selama radius baru < R_OUT dan harus
        // berhasil, bukan ditolak precheck.
        let enlarged_hole =
            extrude_face(&tube, hole_ray, -5.0).expect("offset dinding lubang (memperbesar) harus berhasil");
        let expect_vol = std::f64::consts::PI * (R_OUT * R_OUT - 13.0 * 13.0) * H;
        assert_close(enlarged_hole.0.volume(), expect_vol, "volume tabung dgn lubang R=13 (membesar dari R=8)");
    }

    /// Regresi pasangan test di atas: dinding lubang didorong PERSIS
    /// sejauh radiusnya sendiri (`distance == current_radius`) sehingga
    /// radius baru = R - d = 0 -> lubang menutup penuh (geometri kolaps).
    /// Precheck harus menolak ini dengan pesan jelas soal radius, BUKAN
    /// meloloskannya ke OCCT (yang sebelum perbaikan ini terjadi, karena
    /// tanda lama `R + d` mengevaluasi 8 + 8 = 16 > 0, lolos precheck).
    #[test]
    fn extrude_face_hollow_cylinder_inner_wall_rejects_offset_that_closes_hole_completely() {
        let _guard = TEST_LOCK.lock().unwrap();
        const R_OUT: f64 = 20.0;
        const R_IN: f64 = 8.0;
        const H: f64 = 20.0;
        let outer = AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R_OUT, H);
        let inner = AdHocShape::make_cylinder(dvec3(0.0, 0.0, -1.0), R_IN, H + 2.0);
        let mut tube_shape = outer.0.subtract(&inner.0).shape;
        tube_shape.clean();
        let tube = KernelShape(tube_shape);

        let hole_ray = PickRay { origin: (0.0, 0.0, H / 2.0), dir: (1.0, 0.0, 0.0) };
        pick_face_details(&tube, hole_ray).expect("harus kena dinding lubang");

        match extrude_face(&tube, hole_ray, R_IN) {
            Ok(_) => panic!("lubang menutup penuh (radius jadi 0) harus ditolak"),
            Err(err) => assert!(err.to_string().contains("radius"), "pesan error harus jelas soal radius: {err}"),
        }
    }

    #[test]
    fn extrude_face_sphere_grows_radius_when_pulled_out() {
        let _guard = TEST_LOCK.lock().unwrap();
        const R: f64 = 7.0;
        let sphere = KernelShape(AdHocShape::make_sphere(R).0);
        let ray = PickRay { origin: (50.0, 0.0, 0.0), dir: (-1.0, 0.0, 0.0) };

        // Verifikasi tipe permukaan lewat resolver privat yang sama dgn
        // yang dipakai `extrude_face` sendiri — `extrude_face` jalur
        // non-planar (di bawah) tidak butuh `pick_face_details` sama
        // sekali. (Sebelum Fase 4, `pick_face_details` juga TIDAK BISA
        // dipakai di sini: `compute_face_normal_and_centroid`, Newell's
        // method di atas boundary face, gagal `None` khusus utk bola
        // PENUH — 1 face tertutup dgn seam+2 pole degenerate, bukan loop
        // tepi sederhana. Fase 4 menambah fallback GProp-based di
        // `pick_face_details`, lihat test `pick_face_details_works_on_full_sphere`
        // di bawah utk pembuktian jalur itu sekarang berhasil.)
        let (face, _) = resolve_face_along_ray(&sphere.0, ray).expect("harus kena permukaan bola");
        assert_eq!(SurfaceKind::from(face.surface_kind().as_str()), SurfaceKind::Sphere);

        let grown = extrude_face(&sphere, ray, 1.5).expect("pull +1.5 pada bola harus berhasil");
        let expect_vol = 4.0 / 3.0 * std::f64::consts::PI * (R + 1.5) * (R + 1.5) * (R + 1.5);
        assert_close(grown.0.volume(), expect_vol, "volume bola R=8.5");
    }

    #[test]
    fn extrude_face_cone_lateral_face_changes_volume_in_pull_direction() {
        let _guard = TEST_LOCK.lock().unwrap();
        // Kerucut lewat `revolve_profile`: segitiga siku (0,0)-(R,0)-(0,H)
        // di XY direvolve mengelilingi sumbu Y (pola sama dgn
        // `revolve_profile_produces_ring_solid`) — `BRepPrimAPI_MakeCone`
        // sendiri TIDAK ada binding-nya di `opencascade-sys` (di luar
        // cakupan CADRAW Fase 2), jadi kerucut fixture test dibangun dari
        // primitif yang SUDAH ADA, bukan nambah FFI baru cuma utk test.
        const CONE_R: f64 = 6.0;
        const CONE_H: f64 = 14.0;
        let cone_profile = Profile::Loop(vec![
            ProfileSegment::Line { start: (0.0, 0.0), end: (CONE_R, 0.0) },
            ProfileSegment::Line { start: (CONE_R, 0.0), end: (0.0, CONE_H) },
            ProfileSegment::Line { start: (0.0, CONE_H), end: (0.0, 0.0) },
        ]);
        let cone = revolve_profile(&cone_profile, (0.0, 0.0), (0.0, 1.0), None).unwrap();
        let base_vol = cone.0.volume();
        assert_close(base_vol, std::f64::consts::PI * CONE_R * CONE_R * CONE_H / 3.0, "volume kerucut awal");

        let ray = PickRay { origin: (50.0, CONE_H / 2.0, 0.0), dir: (-1.0, 0.0, 0.0) };
        let hit = pick_face_details(&cone, ray).expect("harus kena selimut kerucut");
        assert_eq!(hit.surface_kind, SurfaceKind::Cone);

        // Kerucut BUKAN silinder — offset selimut kerucut menggeser sudut
        // puncak sepanjang sumbu (properti "cone sejajar" standar), jadi
        // volume TIDAK berubah proporsional-sederhana seperti silinder.
        // Yang divalidasi di sini murni ARAH perubahan (tarik keluar =
        // volume naik, tekan masuk = volume turun) + hasil tetap solid
        // valid — bukan formula tertutup.
        let grown = extrude_face(&cone, ray, 1.0).expect("pull +1.0 pada selimut kerucut harus berhasil");
        assert!(grown.0.volume() > base_vol, "menarik selimut kerucut keluar harus menambah volume");
        assert!(grown.tessellate().triangle_count() > 0);

        let shrunk = extrude_face(&cone, ray, -1.0).expect("push -1.0 pada selimut kerucut harus berhasil");
        assert!(shrunk.0.volume() < base_vol, "menekan selimut kerucut masuk harus mengurangi volume");
        assert!(shrunk.tessellate().triangle_count() > 0);
    }

    #[test]
    fn extrude_face_planar_regression_still_uses_extrude_and_boolean_path() {
        // Regresi murni: wajah datar (Plane) tetap lewat jalur lama
        // extrude+union/subtract, tidak tersentuh dispatch tipe permukaan
        // baru — memakai skenario yang sama dgn `test_extrude_face_cylinder_top`
        // (tutup datar silinder), harus tetap identik perilakunya.
        let _guard = TEST_LOCK.lock().unwrap();
        let circle_profile = Profile::Circle { center: (0.0, 0.0), radius: 12.0 };
        let cylinder = extrude_profile(&circle_profile, 25.0).unwrap();
        let ray_top = PickRay { origin: (0.0, 0.0, 100.0), dir: (0.0, 0.0, -1.0) };
        let hit_top = pick_face_details(&cylinder, ray_top).expect("harus kena top cap silinder");
        assert_eq!(hit_top.surface_kind, SurfaceKind::Plane);

        let taller = extrude_face(&cylinder, ray_top, 15.0).expect("extrude top cap silinder berhasil");
        let expect_vol = std::f64::consts::PI * 12.0 * 12.0 * 40.0;
        assert_close(taller.0.volume(), expect_vol, "volume silinder tinggi 40 (25+15) hasil jalur planar lama");
    }

    // ---- CADRAW Fase 4: `FaceHit::pull_dir` per `SurfaceKind` ----

    #[test]
    fn pull_dir_equals_normal_on_planar_face() {
        let _guard = TEST_LOCK.lock().unwrap();
        let circle_profile = Profile::Circle { center: (0.0, 0.0), radius: 12.0 };
        let cylinder = extrude_profile(&circle_profile, 25.0).unwrap();
        let ray_top = PickRay { origin: (0.0, 0.0, 100.0), dir: (0.0, 0.0, -1.0) };
        let hit_top = pick_face_details(&cylinder, ray_top).expect("harus kena top cap silinder");
        assert_eq!(hit_top.surface_kind, SurfaceKind::Plane);
        assert_eq!(hit_top.pull_dir, hit_top.normal, "Plane: pull_dir harus identik dgn normal Newell (perilaku lama)");
    }

    #[test]
    fn pull_dir_is_radial_on_cylinder_wall() {
        let _guard = TEST_LOCK.lock().unwrap();
        const R: f64 = 10.0;
        const H: f64 = 20.0;
        let cylinder = KernelShape(AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R, H).0);
        // Ray radial murni (bidang simetri y=0) — titik hit persis (R, 0, H/2),
        // proyeksi ke sumbu Z persis (0, 0, H/2), jadi pull_dir radial harus
        // persis (1, 0, 0), BUKAN sekadar "condong ke +x".
        let ray = PickRay { origin: (R + 50.0, 0.0, H / 2.0), dir: (-1.0, 0.0, 0.0) };
        let hit = pick_face_details(&cylinder, ray).expect("harus kena selimut silinder");
        assert_eq!(hit.surface_kind, SurfaceKind::Cylinder);
        assert!((hit.pull_dir.0 - 1.0).abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
        assert!(hit.pull_dir.1.abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
        assert!(hit.pull_dir.2.abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
    }

    #[test]
    fn pull_dir_is_radial_on_cone_lateral_face() {
        let _guard = TEST_LOCK.lock().unwrap();
        // Fixture sama dgn `extrude_face_cone_lateral_face_changes_volume_in_pull_direction`
        // — kerucut lewat `revolve_profile` mengelilingi sumbu Y.
        const CONE_R: f64 = 6.0;
        const CONE_H: f64 = 14.0;
        let cone_profile = Profile::Loop(vec![
            ProfileSegment::Line { start: (0.0, 0.0), end: (CONE_R, 0.0) },
            ProfileSegment::Line { start: (CONE_R, 0.0), end: (0.0, CONE_H) },
            ProfileSegment::Line { start: (0.0, CONE_H), end: (0.0, 0.0) },
        ]);
        let cone = revolve_profile(&cone_profile, (0.0, 0.0), (0.0, 1.0), None).unwrap();
        // Ray radial murni pada bidang simetri z=0 — titik hit dan proyeksi
        // sumbu Y sama-sama punya z=0, jadi pull_dir radial harus persis
        // (1, 0, 0) walau radius kerucut menyempit sepanjang sumbu.
        let ray = PickRay { origin: (50.0, CONE_H / 2.0, 0.0), dir: (-1.0, 0.0, 0.0) };
        let hit = pick_face_details(&cone, ray).expect("harus kena selimut kerucut");
        assert_eq!(hit.surface_kind, SurfaceKind::Cone);
        assert!((hit.pull_dir.0 - 1.0).abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
        assert!(hit.pull_dir.1.abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
        assert!(hit.pull_dir.2.abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
    }

    #[test]
    fn pick_face_details_works_on_full_sphere_with_radial_pull_dir() {
        // Regresi Fase 4: sebelum fallback GProp di `pick_face_details`,
        // fungsi ini SELALU `None` utk bola penuh (lihat catatan panjang di
        // `extrude_face_sphere_grows_radius_when_pulled_out`) — artinya
        // bola tidak bisa dipilih sama sekali di viewport. Test ini
        // membuktikan jalur itu sekarang berhasil DAN `pull_dir`-nya benar
        // radial dari pusat bola.
        let _guard = TEST_LOCK.lock().unwrap();
        const R: f64 = 7.0;
        let sphere = KernelShape(AdHocShape::make_sphere(R).0);
        let ray = PickRay { origin: (50.0, 0.0, 0.0), dir: (-1.0, 0.0, 0.0) };
        let hit = pick_face_details(&sphere, ray)
            .expect("Fase 4: pick_face_details harus berhasil utk bola penuh (fallback GProp)");
        assert_eq!(hit.surface_kind, SurfaceKind::Sphere);
        assert!(
            hit.centroid.0.abs() < 1e-4 && hit.centroid.1.abs() < 1e-4 && hit.centroid.2.abs() < 1e-4,
            "centroid GProp bola penuh berpusat di origin, actual={:?}",
            hit.centroid
        );
        assert!((hit.pull_dir.0 - 1.0).abs() < 1e-4, "pull_dir salah: {:?}", hit.pull_dir);
        assert!(hit.pull_dir.1.abs() < 1e-4, "pull_dir salah: {:?}", hit.pull_dir);
        assert!(hit.pull_dir.2.abs() < 1e-4, "pull_dir salah: {:?}", hit.pull_dir);
    }

    #[test]
    fn pull_dir_is_radial_on_partial_sphere_octant_face() {
        // Regresi utk bug `compute_pull_dir` Sphere: face bola PARSIAL (mis.
        // sudut hasil fillet bola / bola terpotong boolean) di mana jalur
        // Newell (`compute_face_normal_and_centroid`) BERHASIL, dan
        // centroid-nya = rata-rata boundary loop FACE ITU (bukan pusat
        // bola). Fixture: irisan bola dgn box oktan (0,0,0)-(R+5,R+5,R+5)
        // menyisakan 1/8 permukaan bola dibatasi 3 busur seperempat
        // lingkaran — beda dgn test full-sphere di atas (yg lewat fallback
        // GProp krn Newell SELALU gagal utk bola penuh), test ini sengaja
        // menguji jalur Newell yg berhasil dgn centroid loop condong ke
        // oktan (+,+,+), BUKAN origin.
        let _guard = TEST_LOCK.lock().unwrap();
        const R: f64 = 10.0;
        let sphere = AdHocShape::make_sphere(R);
        let octant_box =
            AdHocShape::make_box_point_point(dvec3(0.0, 0.0, 0.0), dvec3(R + 5.0, R + 5.0, R + 5.0));
        let octant = intersect(&KernelShape(sphere.0), &KernelShape(octant_box.0))
            .expect("irisan bola dgn box oktan harus berhasil");

        // Ray radial murni menuju (R, ~0, ~0) — sedikit digeser dari sumbu
        // y/z supaya pasti kena permukaan bola melengkung, bukan salah satu
        // dari 3 face datar potongan box (yg terletak persis di bidang
        // x=0/y=0/z=0).
        let ray = PickRay { origin: (R + 50.0, 0.001, 0.001), dir: (-1.0, 0.0, 0.0) };
        let hit = pick_face_details(&octant, ray).expect("harus kena permukaan bola oktan");
        assert_eq!(hit.surface_kind, SurfaceKind::Sphere);
        // Buktikan test ini betul2 menguji jalur bug: centroid loop face
        // oktan HARUS condong ke (+,+,+), bukan pusat bola (0,0,0).
        assert!(
            hit.centroid.0 > 1.0 && hit.centroid.1 > 1.0 && hit.centroid.2 > 1.0,
            "fixture salah: centroid loop harus condong ke oktan (+,+,+), BUKAN pusat bola: {:?}",
            hit.centroid
        );
        // pull_dir harus tetap radial dari PUSAT bola (0,0,0) — bukan dari
        // centroid loop yg melenceng ke oktan. Titik hit ≈ (R,0,0) →
        // radial ≈ (1,0,0).
        assert!((hit.pull_dir.0 - 1.0).abs() < 1e-3, "pull_dir salah (bukan radial dari pusat bola): {:?}", hit.pull_dir);
        assert!(hit.pull_dir.1.abs() < 1e-3, "pull_dir salah (bukan radial dari pusat bola): {:?}", hit.pull_dir);
        assert!(hit.pull_dir.2.abs() < 1e-3, "pull_dir salah (bukan radial dari pusat bola): {:?}", hit.pull_dir);
    }
}
