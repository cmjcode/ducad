use anyhow::{anyhow, bail, Context, Result};
use glam::{dvec3, DVec3};
use opencascade::angle::Angle;
use opencascade::primitives::{Direction as OcctDirection, Edge, FaceOrientation, IntoShape, Solid};
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
    let hollowed = cloned
        .try_hollow(-thickness.abs(), [face])
        .map_err(|e| anyhow::anyhow!("operasi shell/hollow gagal: {e}"))?;
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
    let hollowed = cloned
        .try_hollow(-thickness.abs(), faces)
        .map_err(|e| anyhow::anyhow!("operasi shell/hollow gagal: {e}"))?;
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

/// Revolve satu sisi (face) solid mengelilingi sumbu 3D (`axis_origin` + `axis_dir`).
/// `angle_degrees: None` = revolve 360°.
/// Menggabungkan hasil revolve (Union) dengan body asal jika bersentuhan, atau mengembalikan shape terpisah.
pub fn revolve_face(
    shape: &KernelShape,
    ray: PickRay,
    axis_origin: DVec3,
    axis_dir: DVec3,
    angle_degrees: Option<f64>,
) -> Result<KernelShape> {
    let dir_len = axis_dir.length();
    if dir_len < 1e-9 {
        bail!("sumbu revolve tidak valid (dua titik axis sama/terlalu dekat)");
    }
    let _guard = lock_kernel();
    let cloned = deep_clone(shape.inner())?;
    let Some((face, _)) = resolve_face_along_ray(&cloned, ray) else {
        bail!("wajah terpilih tidak ditemukan pada shape");
    };

    let angle = angle_degrees.map(Angle::Degrees);
    let solid: Solid = face
        .try_revolve(axis_origin, axis_dir, angle)
        .map_err(|e| anyhow!("Operasi Revolve Face gagal: pastikan sumbu putar tidak memotong bagian dalam face ({e})"))?;
    let swept_shape = solid.into_shape();

    match cloned.union(&swept_shape) {
        Ok(merged) => {
            let mut s = merged.shape;
            s.clean();
            Ok(KernelShape::from_inner(s))
        }
        Err(_) => Ok(KernelShape::from_inner(swept_shape)),
    }
}

/// Tambahkan kemiringan cetakan (draft angle) pada satu atau beberapa face planar
/// solid yang di-pick lewat `rays`. Digunakan untuk manufaktur produk cetakan
/// plastik (injection molding) agar produk bisa dilepas dari cetakan.
///
/// - `neutral_plane_point`: titik pada bidang netral (garis netral tidak bergerak).
/// - `neutral_plane_normal`: vektor normal dari bidang netral (biasanya sejajar pull_direction).
/// - `pull_direction`: arah bukaan cetakan (biasanya `DVec3::Z` = Z+).
/// - `angle_deg`: sudut kemiringan dalam derajat (1–5° umum untuk plastik).
/// - `rays`: satu atau lebih ray dari kamera untuk pick face yang akan di-draft.
pub fn draft_angle(
    shape: &KernelShape,
    neutral_plane_point: DVec3,
    neutral_plane_normal: DVec3,
    pull_direction: DVec3,
    angle_deg: f64,
    rays: &[PickRay],
) -> Result<KernelShape> {
    if angle_deg <= 0.0 || angle_deg >= 90.0 {
        bail!("sudut draft harus antara 0° dan 90° (eksklusif); diberikan {angle_deg:.3}°");
    }
    if rays.is_empty() {
        bail!("pilih minimal 1 face planar untuk diberi draft angle");
    }
    let pull_len = pull_direction.length();
    if pull_len < 1e-9 {
        bail!("arah pull tidak valid (vektor nol)");
    }
    let np_len = neutral_plane_normal.length();
    if np_len < 1e-9 {
        bail!("normal bidang netral tidak valid (vektor nol)");
    }

    // Normalisasi
    let pull_dir_norm = pull_direction / pull_len;
    let np_normal_norm = neutral_plane_normal / np_len;

    let _guard = lock_kernel();
    let cloned = deep_clone(shape.inner())?;

    // Resolve faces via picking rays
    let mut oc_faces = Vec::with_capacity(rays.len());
    for ray in rays {
        let Some((face, _)) = resolve_face_along_ray(&cloned, *ray) else {
            bail!("salah satu face terpilih tidak ditemukan pada shape");
        };
        if SurfaceKind::from(face.surface_kind().as_str()) != SurfaceKind::Plane {
            bail!("draft angle hanya mendukung face planar; pilih face datar");
        }
        oc_faces.push(face);
    }

    let face_refs: Vec<&opencascade::primitives::Face> = oc_faces.iter().collect();

    // Jalankan draft angle langsung pada cloned shape (OCCT call, lock tetap dipegang)
    let result = cloned
        .draft_angle(
            neutral_plane_point,
            np_normal_norm,
            pull_dir_norm,
            angle_deg,
            &face_refs,
        )
        .map_err(|e| anyhow!("Draft Angle gagal: {e}"))?;

    Ok(KernelShape::from_inner(result))
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

/// Ubah ukuran satu dimensi solid sepanjang rusuk (edge) yang dipilih.
/// Fungsi ini mencari face planar di ujung rusuk (`end` atau `start`) yang tegak lurus
/// terhadap rusuk tersebut (normal sejajar arah rusuk) lalu mengekstrusi/memotong face
/// tersebut sebesar `delta = new_length - old_length` via `extrude_face`.
/// Dengan cara ini, HANYA dimensi yang bersangkutan yang berubah (misal tinggi balok),
/// sedangkan dimensi sisi lainnya (panjang, lebar) tetap utuh.
pub fn resize_shape_along_edge(
    shape: &KernelShape,
    edge_start: (f64, f64, f64),
    edge_end: (f64, f64, f64),
    new_length: f64,
) -> Result<KernelShape> {
    if new_length <= 0.0 {
        bail!("ukuran baru harus positif");
    }
    let p_start = glam::DVec3::new(edge_start.0, edge_start.1, edge_start.2);
    let p_end = glam::DVec3::new(edge_end.0, edge_end.1, edge_end.2);
    let edge_vec = p_end - p_start;
    let old_length = edge_vec.length();
    if old_length < 1e-5 {
        bail!("panjang rusuk terlalu kecil");
    }
    let delta = new_length - old_length;
    if delta.abs() < 1e-6 {
        return crate::shape::clone_shape(shape);
    }
    let dir = edge_vec / old_length;

    let _guard = lock_kernel();
    let cloned = deep_clone(shape.inner())?;

    let mut best_target: Option<(PickRay, f64)> = None;

    for face in cloned.faces() {
        if SurfaceKind::from(face.surface_kind().as_str()) != SurfaceKind::Plane {
            continue;
        }
        if let Some((normal, centroid)) = compute_face_normal_and_centroid(&face, -dir) {
            // Cek face di ujung p_end (normal searah dir)
            let dot_end = normal.dot(dir);
            if dot_end > 0.95 {
                let plane_dist = (p_end - centroid).dot(normal).abs();
                if plane_dist < 1e-2 {
                    let ray = PickRay {
                        origin: (
                            centroid.x + dir.x * 20.0,
                            centroid.y + dir.y * 20.0,
                            centroid.z + dir.z * 20.0,
                        ),
                        dir: (-dir.x, -dir.y, -dir.z),
                    };
                    best_target = Some((ray, delta));
                    break;
                }
            }
            // Cek face di ujung p_start (normal searah -dir)
            let dot_start = normal.dot(-dir);
            if dot_start > 0.95 {
                let plane_dist = (p_start - centroid).dot(normal).abs();
                if plane_dist < 1e-2 {
                    let ray = PickRay {
                        origin: (
                            centroid.x - dir.x * 20.0,
                            centroid.y - dir.y * 20.0,
                            centroid.z - dir.z * 20.0,
                        ),
                        dir: (dir.x, dir.y, dir.z),
                    };
                    best_target = Some((ray, delta));
                    break;
                }
            }
        }
    }

    drop(_guard);

    if let Some((ray, d)) = best_target {
        if let Ok(result_shape) = extrude_face(shape, ray, d) {
            return Ok(result_shape);
        }
    }

    // Fallback: uniform scaling jika tidak ada face planar yang cocok di ujung rusuk
    let factor = new_length / old_length;
    let pivot = (
        (edge_start.0 + edge_end.0) * 0.5,
        (edge_start.1 + edge_end.1) * 0.5,
        (edge_start.2 + edge_end.2) * 0.5,
    );
    crate::shape::scale_shape(shape, pivot, factor)
}

/// Memotong solid 3D menggunakan bidang (plane) yang didefinisikan oleh sebuah titik
/// acuan dan vektor normal. Menghasilkan kumpulan solid terpisah (biasanya 2 solid).
pub fn split_body(
    shape: &KernelShape,
    plane_point: DVec3,
    plane_normal: DVec3,
) -> Result<Vec<KernelShape>> {
    if plane_normal.length_squared() < 1e-6 {
        bail!("normal bidang pemotong tidak boleh bernilai nol");
    }
    let normal = plane_normal.normalize();
    let _guard = lock_kernel();
    let cloned = deep_clone(shape.inner())?;
    let split_shapes = cloned
        .split_with_plane(plane_point, normal)
        .context("operasi split body gagal pada kernel OCCT")?;

    if split_shapes.is_empty() {
        bail!("bidang pemotong tidak membagi objek menjadi bagian valid");
    }

    Ok(split_shapes
        .into_iter()
        .map(KernelShape::from_inner)
        .collect())
}

/// Memotong solid 3D menggunakan shape pemotong (mis. face planar atau permukaan lembaran).
pub fn split_body_with_tool(
    shape: &KernelShape,
    tool: &KernelShape,
) -> Result<Vec<KernelShape>> {
    let _guard = lock_kernel();
    let cloned = deep_clone(shape.inner())?;
    let split_shapes = cloned
        .split_with_tool(tool.inner())
        .context("operasi split body dengan tool gagal pada kernel OCCT")?;

    if split_shapes.is_empty() {
        bail!("tool pemotong tidak membagi objek menjadi bagian valid");
    }

    Ok(split_shapes
        .into_iter()
        .map(KernelShape::from_inner)
        .collect())
}

/// Membagi face pada solid 3D dengan bidang pemotong tanpa memisahkan body menjadi dua bagian.
pub fn split_face(
    shape: &KernelShape,
    plane_point: DVec3,
    plane_normal: DVec3,
) -> Result<KernelShape> {
    if plane_normal.length_squared() < 1e-6 {
        bail!("normal bidang pemotong tidak boleh bernilai nol");
    }
    let normal = plane_normal.normalize();
    let _guard = lock_kernel();
    let cloned = deep_clone(shape.inner())?;
    let result_shape = cloned
        .split_faces_with_plane(plane_point, normal)
        .context("operasi split face gagal pada kernel OCCT")?;

    Ok(KernelShape::from_inner(result_shape))
}

/// Buat salinan solid 3D dalam susunan grid linier 3D sepanjang sumbu X, Y, dan Z.
/// `count_x`, `count_y`, `count_z`: jumlah item di tiap arah (minimal 1).
/// `pitch_x`, `pitch_y`, `pitch_z`: jarak pitch antar item (mm).
/// Mengembalikan HANYA solid salinan baru (tidak termasuk solid asli pada indeks (0, 0, 0)).
pub fn linear_pattern_shape(
    shape: &KernelShape,
    count_x: usize,
    pitch_x: f64,
    count_y: usize,
    pitch_y: f64,
    count_z: usize,
    pitch_z: f64,
) -> Result<Vec<KernelShape>> {
    let cx = count_x.max(1);
    let cy = count_y.max(1);
    let cz = count_z.max(1);

    let mut result = Vec::new();
    for iz in 0..cz {
        for iy in 0..cy {
            for ix in 0..cx {
                if ix == 0 && iy == 0 && iz == 0 {
                    continue;
                }
                let dx = ix as f64 * pitch_x;
                let dy = iy as f64 * pitch_y;
                let dz = iz as f64 * pitch_z;
                result.push(crate::shape::translate_shape(shape, dx, dy, dz)?);
            }
        }
    }
    Ok(result)
}

/// Buat salinan solid 3D dalam susunan melingkar (Circular Pattern 3D) mengelilingi sumbu putar poros 3D.
/// `pivot`: titik pada sumbu putar (x, y, z).
/// `axis`: vektor arah sumbu poros putar (x, y, z).
/// `count`: jumlah TOTAL item (minimal 2).
/// `total_angle_rad`: rentang sudut total (mis. 2*PI untuk 360° penuh).
/// Mengembalikan HANYA solid salinan baru (tidak termasuk solid asli pada k=0).
pub fn circular_pattern_shape(
    shape: &KernelShape,
    pivot: (f64, f64, f64),
    axis: (f64, f64, f64),
    count: usize,
    total_angle_rad: f64,
) -> Result<Vec<KernelShape>> {
    if count <= 1 {
        return Ok(Vec::new());
    }

    let is_full_circle = (total_angle_rad.abs() - std::f64::consts::TAU).abs() < 1e-4;
    let step_angle = if is_full_circle {
        total_angle_rad / count as f64
    } else {
        total_angle_rad / (count - 1) as f64
    };

    let mut result = Vec::new();
    for k in 1..count {
        let angle = k as f64 * step_angle;
        result.push(crate::shape::rotate_shape(shape, pivot, axis, angle)?);
    }
    Ok(result)
}


