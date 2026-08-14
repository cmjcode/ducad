//! Wrapper CADRAW di atas kernel OpenCASCADE (via opencascade-rs).
//!
//! Seluruh aplikasi hanya boleh menyentuh tipe dari crate ini, bukan
//! `opencascade` langsung — agar detail FFI terisolasi dan kernel bisa
//! ditambal/diganti tanpa merombak app. `Shape` OCCT sengaja tidak pernah
//! `pub`: [`KernelShape`] membungkusnya sepenuhnya.

use anyhow::{bail, Context, Result};
use glam::dvec3;
use opencascade::primitives::{Direction as OcctDirection, Edge, Face, IntoShape, Shape, Wire};
use opencascade::workplane::Workplane;

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
        tessellate_shape(&self.0)
    }

    pub fn write_stl(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        self.0.write_stl(path)?;
        Ok(())
    }

    pub fn write_step(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        self.0.write_step(path)?;
        Ok(())
    }
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
    let path = std::env::temp_dir().join(format!(
        "cadraw-deep-clone-{}-{}.step",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
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
    match profile {
        Profile::Circle { center, radius } => {
            if *radius <= 0.0 {
                bail!("radius lingkaran profil harus > 0");
            }
            let edge = Edge::circle(dvec3(center.0, center.1, 0.0), dvec3(0.0, 0.0, 1.0), *radius);
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
                        Edge::segment(dvec3(start.0, start.1, 0.0), dvec3(end.0, end.1, 0.0))
                    }
                    ProfileSegment::Arc { start, via, end } => Edge::arc(
                        dvec3(start.0, start.1, 0.0),
                        dvec3(via.0, via.1, 0.0),
                        dvec3(end.0, end.1, 0.0),
                    ),
                })
                .collect();
            Ok(Wire::from_edges(edges.iter()))
        }
    }
}

/// Extrude profil di bidang XY sepanjang `distance` mm di sumbu Z (arah
/// negatif kalau `distance` negatif). Workplane lain (sketch-on-face)
/// belum didukung — sketch di CADRAW saat ini selalu di bidang XY, lihat
/// docs/PLAN.md.
pub fn extrude_profile(profile: &Profile, distance: f64) -> Result<KernelShape> {
    if distance.abs() < 1e-9 {
        bail!("jarak extrude harus tidak nol");
    }
    let wire = build_wire(profile)?;
    let face = Face::from_wire(&wire);
    let solid = face.extrude(dvec3(0.0, 0.0, distance));
    Ok(KernelShape(solid.into_shape()))
}

/// Union (gabung material) dua shape. Boolean intersect (irisan) tidak
/// tersedia di `opencascade-rs` 0.2.0 — cuma union & subtract, lihat
/// docs/PLAN.md.
pub fn union(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    Ok(KernelShape(a.0.union(&b.0).shape))
}

/// Subtract (`a` dikurangi `b`).
pub fn subtract(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    Ok(KernelShape(a.0.subtract(&b.0).shape))
}

/// Fillet SEMUA tepi shape dengan `radius` yang sama. Pemilihan tepi
/// individual (mis. cuma tepi atas) butuh UI picking 3D yang belum ada —
/// lihat docs/PLAN.md.
pub fn fillet_all(shape: &KernelShape, radius: f64) -> Result<KernelShape> {
    if radius <= 0.0 {
        bail!("radius fillet harus > 0");
    }
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
    let mut cloned = deep_clone(&shape.0)?;
    cloned.chamfer(distance);
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
    let cloned = deep_clone(&shape.0)?;
    let face = cloned
        .faces()
        .try_farthest(remove_face_dir.to_occt())
        .ok_or_else(|| anyhow::anyhow!("shape tidak punya face untuk dihilangkan"))?;
    // Offset negatif = dinding tumbuh ke DALAM (cangkang), bukan keluar.
    let hollowed = cloned.hollow(-thickness.abs(), [face]);
    Ok(KernelShape(hollowed))
}

/// Smoke-test kemampuan kernel: kotak di-extrude dari sketch lalu difillet
/// — persis alur "sketch → push/pull → fillet" yang jadi inti CADRAW.
pub fn make_filleted_box(width: f64, depth: f64, height: f64, fillet: f64) -> Result<KernelShape> {
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
}
