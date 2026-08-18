use anyhow::{bail, Context, Result};
use glam::dvec3;
use opencascade::primitives::{Direction as OcctDirection, Edge, FaceOrientation, IntoShape};
use opencascade::workplane::Workplane;

use crate::lock_kernel;
use crate::picking::face::{
    compute_face_normal_and_centroid, resolve_face_along_ray, SurfaceKind,
};
use crate::picking::edge::resolve_edge_along_ray;
use crate::picking::ray::PickRay;
use crate::picking::vertex::resolve_vertex_along_ray;
use crate::shape::{deep_clone, KernelShape};

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

/// Fillet SEMUA tepi shape dengan `radius` yang sama. Pemilihan tepi
/// individual (mis. cuma tepi atas) butuh UI picking 3D yang belum ada —
/// lihat docs/PLAN.md.
pub fn fillet_all(shape: &KernelShape, radius: f64) -> Result<KernelShape> {
    if radius <= 0.0 {
        bail!("radius fillet harus > 0");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(shape.inner())?;
    cloned
        .fillet(radius)
        .context("radius fillet terlalu besar untuk salah satu tepi shape")?;
    Ok(KernelShape::from_inner(cloned))
}

/// Chamfer SEMUA tepi shape dengan `distance` yang sama (lihat batasan
/// yang sama seperti `fillet_all`).
pub fn chamfer_all(shape: &KernelShape, distance: f64) -> Result<KernelShape> {
    if distance <= 0.0 {
        bail!("jarak chamfer harus > 0");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(shape.inner())?;
    cloned
        .chamfer(distance)
        .context("jarak chamfer terlalu besar untuk salah satu tepi shape")?;
    Ok(KernelShape::from_inner(cloned))
}

/// Fillet HANYA tepi yang di-pick lewat `rays` (bukan semua tepi seperti
/// `fillet_all`) — tiap ray di-cast ULANG terhadap shape hasil
/// `deep_clone` (lihat desain di `PickRay`) buat resolusi Edge yang valid
/// dipakai `Shape::fillet_edges`.
pub fn fillet_edges(
    shape: &KernelShape,
    radius: f64,
    rays: &[PickRay],
    tolerance: f64,
) -> Result<KernelShape> {
    if radius <= 0.0 {
        bail!("radius fillet harus > 0");
    }
    if rays.is_empty() {
        bail!("pilih minimal 1 tepi (atau pakai fillet_all untuk semua tepi sekaligus)");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(shape.inner())?;
    let mut edges = Vec::with_capacity(rays.len());
    for ray in rays {
        let Some((edge, _, _)) = resolve_edge_along_ray(&cloned, *ray, tolerance) else {
            bail!("salah satu tepi terpilih tidak ditemukan lagi pada shape");
        };
        edges.push(edge);
    }
    cloned
        .fillet_edges(radius, &edges)
        .context("radius fillet terlalu besar untuk tepi terpilih (mis. melebihi batas ujung objek)")?;
    Ok(KernelShape::from_inner(cloned))
}

/// Fillet SEMUA tepi yang bertemu di 1 vertex (sudut) yang di-pick lewat
/// `ray` — beda dari `fillet_edges` yang fillet tepi spesifik hasil pick:
/// di sini user klik SUDUT, kernel yang mencari sendiri tepi-tepi yang
/// bertemu di situ.
pub fn fillet_vertex(
    shape: &KernelShape,
    radius: f64,
    ray: PickRay,
    tolerance: f64,
) -> Result<KernelShape> {
    if radius <= 0.0 {
        bail!("radius fillet harus > 0");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(shape.inner())?;
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

    cloned
        .fillet_edges(radius, &edges)
        .context("radius fillet terlalu besar untuk sudut terpilih (mis. melebihi batas ujung objek)")?;
    Ok(KernelShape::from_inner(cloned))
}

/// Chamfer SEMUA tepi yang bertemu di 1 vertex (sudut) yang di-pick lewat
/// `ray` — versi "potong lurus" dari `fillet_vertex`, dipakai saat gizmo
/// sudut di-DORONG (bukan ditarik): sudut dipangkas rata, bukan dibulatkan.
pub fn chamfer_vertex(
    shape: &KernelShape,
    distance: f64,
    ray: PickRay,
    tolerance: f64,
) -> Result<KernelShape> {
    if distance <= 0.0 {
        bail!("jarak chamfer harus > 0");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(shape.inner())?;
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

    cloned
        .chamfer_edges(distance, &edges)
        .context("jarak chamfer terlalu besar untuk sudut terpilih (mis. melebihi batas ujung objek)")?;
    Ok(KernelShape::from_inner(cloned))
}

/// Chamfer HANYA tepi yang di-pick lewat `rays` (lihat `fillet_edges`).
pub fn chamfer_edges(
    shape: &KernelShape,
    distance: f64,
    rays: &[PickRay],
    tolerance: f64,
) -> Result<KernelShape> {
    if distance <= 0.0 {
        bail!("jarak chamfer harus > 0");
    }
    if rays.is_empty() {
        bail!("pilih minimal 1 tepi (atau pakai chamfer_all untuk semua tepi sekaligus)");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(shape.inner())?;
    let mut edges = Vec::with_capacity(rays.len());
    for ray in rays {
        let Some((edge, _, _)) = resolve_edge_along_ray(&cloned, *ray, tolerance) else {
            bail!("salah satu tepi terpilih tidak ditemukan lagi pada shape");
        };
        edges.push(edge);
    }
    cloned
        .chamfer_edges(distance, &edges)
        .context("jarak chamfer terlalu besar untuk tepi terpilih (mis. melebihi batas ujung objek)")?;
    Ok(KernelShape::from_inner(cloned))
}

/// "Kosongkan" shape jadi cangkang setebal `thickness` mm, membuang face
/// terjauh ke arah `remove_face_dir`. Hanya solid tunggal watertight yang
/// didukung (bawaan `BRepOffsetAPI_MakeThickSolid` OCCT).
pub fn shell_hollow(
    shape: &KernelShape,
    thickness: f64,
    remove_face_dir: Direction,
) -> Result<KernelShape> {
    if thickness <= 0.0 {
        bail!("tebal shell harus > 0");
    }
    let _guard = lock_kernel();
    let cloned = deep_clone(shape.inner())?;
    let face = cloned
        .faces()
        .try_farthest(remove_face_dir.to_occt())
        .ok_or_else(|| anyhow::anyhow!("shape tidak punya face untuk dihilangkan"))?;
    let hollowed = cloned.hollow(-thickness.abs(), [face]);
    Ok(KernelShape::from_inner(hollowed))
}

/// Sama seperti `shell_hollow`, tapi wajah yang dibuang ditentukan lewat
/// picking (`rays`, bisa >1 — mis. buka 2 sisi sekaligus).
pub fn shell_hollow_faces(
    shape: &KernelShape,
    thickness: f64,
    rays: &[PickRay],
) -> Result<KernelShape> {
    if thickness <= 0.0 {
        bail!("tebal shell harus > 0");
    }
    if rays.is_empty() {
        bail!("pilih minimal 1 wajah (atau pakai shell_hollow untuk arah otomatis)");
    }
    let _guard = lock_kernel();
    let cloned = deep_clone(shape.inner())?;
    let mut faces = Vec::with_capacity(rays.len());
    for ray in rays {
        let Some((face, _)) = resolve_face_along_ray(&cloned, *ray) else {
            bail!("salah satu wajah terpilih tidak ditemukan lagi pada shape");
        };
        faces.push(face);
    }
    let hollowed = cloned.hollow(-thickness.abs(), faces);
    Ok(KernelShape::from_inner(hollowed))
}

/// Extrude satu sisi (face) solid sepanjang `distance` mm searah normal keluar.
/// Jika `distance > 0`, volume baru digabung (*Union*).
/// Jika `distance < 0`, volume dipotong (*Subtract* / Pocket Cut).
pub fn extrude_face(shape: &KernelShape, ray: PickRay, distance: f64) -> Result<KernelShape> {
    if distance.abs() < 1e-9 {
        bail!("jarak extrude face harus tidak nol");
    }
    let _guard = lock_kernel();
    let cloned = deep_clone(shape.inner())?;
    let Some((face, _)) = resolve_face_along_ray(&cloned, ray) else {
        bail!("wajah terpilih tidak ditemukan pada shape");
    };

    if SurfaceKind::from(face.surface_kind().as_str()) != SurfaceKind::Plane {
        if let Some(current_radius) = face.cylinder_or_cone_radius() {
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
        return Ok(KernelShape::from_inner(offset_shape));
    }

    let Some((normal, _)) = compute_face_normal_and_centroid(&face, ray.dir_vec()) else {
        bail!("tidak dapat menghitung normal untuk wajah terpilih");
    };
    let extrude_vec = normal * distance;
    let swept = face.extrude(extrude_vec);
    let swept_shape = swept.into_shape();

    if distance > 0.0 {
        let mut merged = cloned
            .union(&swept_shape)
            .context("gagal menggabungkan hasil extrude ke shape (mis. wajah bersinggungan dengan rounding di sebelahnya)")?
            .shape;
        merged.clean();
        Ok(KernelShape::from_inner(merged))
    } else {
        let mut result = cloned
            .subtract(&swept_shape)
            .context("gagal mengurangi hasil extrude dari shape (mis. wajah bersinggungan dengan rounding di sebelahnya)")?
            .shape;
        result.clean();
        Ok(KernelShape::from_inner(result))
    }
}

/// Smoke-test kemampuan kernel: kotak di-extrude dari sketch lalu difillet
pub fn make_filleted_box(
    width: f64,
    depth: f64,
    height: f64,
    fillet: f64,
) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let profile = Workplane::xy().rect(width, depth);
    let solid = profile.to_face().extrude(dvec3(0.0, 0.0, height));
    let mut shape = solid.into_shape();
    if fillet > 0.0 {
        shape
            .fillet(fillet)
            .context("radius fillet terlalu besar untuk kotak smoke-test ini")?;
    }
    Ok(KernelShape::from_inner(shape))
}
