//! Bagian backend truck dari `crate::modify` (PLAN_MIGRASI_TRUCK F3.1–3.2,
//! 3.6): fillet, chamfer, shell, draft, split, extrude/revolve face, rib.
//!
//! SolidCraft memilih edge/face lewat TITIK (edge terdekat ke titik, face
//! yang mesh-nya lewat paling dekat), bukan indeks. Indeks/ray DUCAD
//! dipetakan ke face/edge logis (`super::topology`), lalu ke satu titik per
//! anggota SolidCraft-nya (`LEdge::member_mids`, `LFace::member_points`).

use anyhow::{anyhow, bail, Context, Result};
use glam::DVec3;
use solidcraft_geom::{Plane, Vec3};
use solidcraft_kernel::{Body, BoolOp, HalfSpace, SplitTool};

use super::bridge::to_dv;
use super::csg::one_body;
use super::topology::Topology;
use super::{kernel, unsupported};
use crate::modify::{check_shell_thickness, Direction, ShellDepthTooDeep, ShellOpening};
use crate::picking::face::SurfaceKind;
use crate::picking::ray::PickRay;
use crate::shape::{clone_shape, KernelShape};

pub(crate) fn sc(p: DVec3) -> Vec3 {
    Vec3::new(p.x, p.y, p.z)
}

// ---------------------------------------------------------------------------
// Pemilihan: indeks / ray DUCAD → titik SolidCraft.
// ---------------------------------------------------------------------------

/// Edge seam (antara dua potongan face periodik yang sama): bukan tepi
/// fisik, tidak bisa di-fillet. Backend lama (≤ 0.5.x) juga melewatinya.
fn is_seam(topo: &Topology, e: usize) -> bool {
    topo.edges.get(e).is_some_and(|edge| edge.faces.len() == 1)
}

/// Titik SolidCraft untuk edge logis `edges` (indeks `enumerate_edges`).
pub(crate) fn edge_points(shape: &KernelShape, edges: &[usize]) -> Result<Vec<Vec3>> {
    let topo = shape.topology();
    let mut out = Vec::new();
    for &i in edges {
        let Some(edge) = topo.edges.get(i) else {
            bail!(
                "indeks tepi {i} di luar jangkauan (shape punya {} tepi)",
                topo.edges.len()
            );
        };
        if is_seam(topo, i) {
            bail!("tepi #{i} adalah seam permukaan lengkung, bukan tepi fisik");
        }
        out.extend(edge.member_mids.iter().map(|p| sc(*p)));
    }
    Ok(out)
}

/// Edge logis yang ditunjuk ray (diresolusi ulang di shape ini).
/// Indeks tepi (`enumerate_edges`) yang ditunjuk pick ray GUI — dipakai GUI
/// untuk membuat selector semantik dari tepi yang diklik (ADR 0010).
pub fn edges_for_rays(shape: &KernelShape, rays: &[PickRay], tolerance: f64) -> Result<Vec<usize>> {
    ray_edges(shape, rays, tolerance)
}

fn ray_edges(shape: &KernelShape, rays: &[PickRay], tolerance: f64) -> Result<Vec<usize>> {
    let topo = shape.topology();
    let mut out = Vec::with_capacity(rays.len());
    for ray in rays {
        let Some((e, _)) = super::picking_edge::resolve_edge(topo, *ray, tolerance) else {
            bail!("salah satu tepi terpilih tidak ditemukan lagi pada shape");
        };
        if !out.contains(&e) {
            out.push(e);
        }
    }
    Ok(out)
}

/// Semua edge fisik (tanpa seam).
fn all_edges(shape: &KernelShape) -> Vec<usize> {
    let topo = shape.topology();
    (0..topo.edges.len()).filter(|&e| !is_seam(topo, e)).collect()
}

/// Edge logis yang berujung di vertex terdekat ray.
fn vertex_edges(shape: &KernelShape, ray: PickRay, tolerance: f64) -> Result<Vec<usize>> {
    let Some(v) = super::picking_vertex::pick_vertex(shape, ray, tolerance) else {
        bail!("sudut (vertex) terpilih tidak ditemukan lagi pada shape");
    };
    let v = DVec3::new(v.0, v.1, v.2);
    let topo = shape.topology();
    const COINCIDENT_EPS: f64 = 1e-6;
    let edges: Vec<usize> = (0..topo.edges.len())
        .filter(|&e| !is_seam(topo, e))
        .filter(|&e| {
            let edge = &topo.edges[e];
            (edge.start() - v).length() < COINCIDENT_EPS
                || (edge.end() - v).length() < COINCIDENT_EPS
        })
        .collect();
    if edges.is_empty() {
        bail!("tidak ada tepi yang bertemu di sudut terpilih");
    }
    Ok(edges)
}

/// Titik SolidCraft untuk face logis `faces` (indeks `enumerate_faces`).
pub(crate) fn face_points(shape: &KernelShape, faces: &[usize]) -> Result<Vec<Vec3>> {
    let topo = shape.topology();
    let mut out = Vec::new();
    for &i in faces {
        let Some(face) = topo.faces.get(i) else {
            bail!(
                "indeks face {i} di luar jangkauan (shape punya {} face)",
                topo.faces.len()
            );
        };
        out.extend(face.member_points.iter().map(|p| sc(*p)));
    }
    Ok(out)
}

/// Face logis yang ditunjuk ray (tanpa duplikat).
fn ray_faces(shape: &KernelShape, rays: &[PickRay], what: &str) -> Result<Vec<usize>> {
    let topo = shape.topology();
    let mut out = Vec::with_capacity(rays.len());
    for ray in rays {
        let Some((f, _, _)) = super::picking_face::resolve_face_hit(topo, *ray) else {
            bail!("salah satu {what} terpilih tidak ditemukan lagi pada shape");
        };
        if !out.contains(&f) {
            out.push(f);
        }
    }
    Ok(out)
}

/// Face logis terjauh ke arah `dir` (pusat massa, seperti `try_farthest` backend lama (≤ 0.5.x)).
fn farthest_face(shape: &KernelShape, dir: Direction) -> Result<usize> {
    let d = match dir {
        Direction::PosX => DVec3::X,
        Direction::NegX => DVec3::NEG_X,
        Direction::PosY => DVec3::Y,
        Direction::NegY => DVec3::NEG_Y,
        Direction::PosZ => DVec3::Z,
        Direction::NegZ => DVec3::NEG_Z,
    };
    let topo = shape.topology();
    let mut best: Option<(f64, usize)> = None;
    for (i, f) in topo.faces.iter().enumerate() {
        let s = f.centroid.dot(d);
        // `max_by`: seri → face terakhir (perilaku 0.5.x).
        if best.is_none_or(|(b, _)| s >= b) {
            best = Some((s, i));
        }
    }
    best.map(|(_, i)| i)
        .ok_or_else(|| anyhow!("shape tidak punya face untuk dihilangkan"))
}

fn is_planar_face(shape: &KernelShape, f: usize) -> bool {
    shape
        .topology()
        .faces
        .get(f)
        .is_some_and(|face| face.kind == SurfaceKind::Plane)
}

/// (titik di face, normal keluar) face planar logis `f`.
pub(crate) fn plane_of_face(shape: &KernelShape, f: usize) -> Option<(DVec3, DVec3)> {
    let face = shape.topology().faces.get(f)?;
    match face.surface? {
        solidcraft_kernel::FaceSurface::Plane { normal, .. } => {
            let p = face.member_points.first().copied().unwrap_or(face.centroid);
            Some((p, to_dv(*normal)))
        }
        _ => None,
    }
}

/// Hasil kernel → shape; body yang tidak valid ditolak (padanan
/// `validate_or_heal` backend lama (≤ 0.5.x) — SolidCraft sudah menyembuhkan sendiri).
pub(crate) fn checked(body: Body, what: &str) -> Result<KernelShape> {
    let shape = KernelShape::from_body(body);
    if !shape.is_valid() {
        bail!("{what} menghasilkan solid yang tidak valid");
    }
    Ok(shape)
}

// ---------------------------------------------------------------------------
// Fillet & chamfer.
// ---------------------------------------------------------------------------

fn fillet_points(shape: &KernelShape, radius: f64, points: &[Vec3]) -> Result<KernelShape> {
    if radius <= 0.0 {
        bail!("radius fillet harus > 0");
    }
    let body = shape.require_body("Fillet")?;
    let out = kernel("Fillet", || solidcraft_kernel::fillet(body, points, radius)).context(
        "radius fillet terlalu besar untuk tepi terpilih (mis. melebihi batas ujung objek)",
    )?;
    checked(out, "Fillet")
}

fn chamfer_points(shape: &KernelShape, distance: f64, points: &[Vec3]) -> Result<KernelShape> {
    if distance <= 0.0 {
        bail!("jarak chamfer harus > 0");
    }
    let body = shape.require_body("Chamfer")?;
    let all_at_once = kernel("Chamfer", || {
        solidcraft_kernel::chamfer(body, points, distance)
    });
    let out = match all_at_once {
        Ok(b) => b,
        // SolidCraft menolak beberapa sudut (mis. tiga tepi cekung bertemu):
        // chamfer satu per satu, titik tetap dari shape asli (tepi yang
        // sudah dipotong tetangganya bergeser, titik tengahnya tetap terdekat).
        Err(first) if points.len() > 1 => {
            let mut cur = body.clone();
            for p in points {
                cur = match kernel("Chamfer", || solidcraft_kernel::chamfer(&cur, &[*p], distance)) {
                    Ok(b) => b,
                    Err(_) => return Err(first),
                };
            }
            cur
        }
        Err(e) => return Err(e),
    };
    checked(out, "Chamfer")
        .context("jarak chamfer terlalu besar untuk tepi terpilih (mis. melebihi batas ujung objek)")
}

/// Fillet SEMUA tepi fisik shape dengan `radius` yang sama.
pub fn fillet_all(shape: &KernelShape, radius: f64) -> Result<KernelShape> {
    if radius <= 0.0 {
        bail!("radius fillet harus > 0");
    }
    let pts = edge_points(shape, &all_edges(shape))?;
    fillet_points(shape, radius, &pts)
        .context("radius fillet terlalu besar untuk salah satu tepi shape")
}

/// Chamfer SEMUA tepi fisik shape dengan `distance` yang sama.
pub fn chamfer_all(shape: &KernelShape, distance: f64) -> Result<KernelShape> {
    if distance <= 0.0 {
        bail!("jarak chamfer harus > 0");
    }
    let pts = edge_points(shape, &all_edges(shape))?;
    chamfer_points(shape, distance, &pts)
        .context("jarak chamfer terlalu besar untuk salah satu tepi shape")
}

/// Fillet tepi yang di-pick lewat `rays`.
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
    let edges = ray_edges(shape, rays, tolerance)?;
    fillet_points(shape, radius, &edge_points(shape, &edges)?)
}

/// Fillet radius variabel `radius_start` → `radius_end` pada tiap edge
/// logis `edges`, satu per satu (SolidCraft: radius variabel hanya untuk
/// tepi lurus).
pub(crate) fn fillet_variable_logical(
    shape: &KernelShape,
    radius_start: f64,
    radius_end: f64,
    edges: &[usize],
) -> Result<KernelShape> {
    if radius_start <= 0.0 || radius_end <= 0.0 {
        bail!("radius fillet variabel (awal dan akhir) harus > 0");
    }
    if edges.is_empty() {
        bail!("pilih minimal 1 tepi untuk fillet variabel");
    }
    if (radius_start - radius_end).abs() < 1e-12 {
        return fillet_points(shape, radius_start, &edge_points(shape, edges)?);
    }
    // Titik & ujung awal tiap edge diambil dari shape ASLI: setelah satu
    // fillet, tepi lain bergeser sedikit, titik tengahnya tetap terdekat.
    let topo = shape.topology();
    let mut picks = Vec::with_capacity(edges.len());
    for &i in edges {
        let Some(edge) = topo.edges.get(i) else {
            bail!(
                "indeks tepi {i} di luar jangkauan (shape punya {} tepi)",
                topo.edges.len()
            );
        };
        if edge.kind != crate::topo::EdgeKind::Line {
            return unsupported("fillet radius variabel pada tepi lengkung");
        }
        picks.push((sc(edge.mid()), sc(edge.start())));
    }
    let mut body = shape.require_body("Fillet")?.clone();
    for (at, start) in picks {
        body = kernel("Fillet variabel", || {
            solidcraft_kernel::fillet_variable(&body, at, start, radius_start, radius_end)
        })
        .context(
            "radius fillet variabel terlalu besar untuk tepi terpilih (mis. melebihi batas ujung objek)",
        )?;
    }
    checked(body, "Fillet")
}

/// Fillet tepi yang di-pick lewat `rays` dengan radius variabel.
pub fn fillet_edges_variable(
    shape: &KernelShape,
    radius_start: f64,
    radius_end: f64,
    rays: &[PickRay],
    tolerance: f64,
) -> Result<KernelShape> {
    if radius_start <= 0.0 || radius_end <= 0.0 {
        bail!("radius fillet variabel (start dan end) harus > 0");
    }
    if rays.is_empty() {
        bail!("pilih minimal 1 tepi untuk fillet variabel");
    }
    let edges = ray_edges(shape, rays, tolerance)?;
    fillet_variable_logical(shape, radius_start, radius_end, &edges)
}

/// Fillet SEMUA tepi yang bertemu di vertex yang di-pick lewat `ray`.
pub fn fillet_vertex(
    shape: &KernelShape,
    radius: f64,
    ray: PickRay,
    tolerance: f64,
) -> Result<KernelShape> {
    if radius <= 0.0 {
        bail!("radius fillet harus > 0");
    }
    let edges = vertex_edges(shape, ray, tolerance)?;
    fillet_points(shape, radius, &edge_points(shape, &edges)?).context(
        "radius fillet terlalu besar untuk sudut terpilih (mis. melebihi batas ujung objek)",
    )
}

/// Chamfer SEMUA tepi yang bertemu di vertex yang di-pick lewat `ray`.
pub fn chamfer_vertex(
    shape: &KernelShape,
    distance: f64,
    ray: PickRay,
    tolerance: f64,
) -> Result<KernelShape> {
    if distance <= 0.0 {
        bail!("jarak chamfer harus > 0");
    }
    let edges = vertex_edges(shape, ray, tolerance)?;
    chamfer_points(shape, distance, &edge_points(shape, &edges)?).context(
        "jarak chamfer terlalu besar untuk sudut terpilih (mis. melebihi batas ujung objek)",
    )
}

/// Chamfer tepi yang di-pick lewat `rays`.
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
    let edges = ray_edges(shape, rays, tolerance)?;
    chamfer_points(shape, distance, &edge_points(shape, &edges)?)
}

/// Fillet tepi berdasarkan indeks [`crate::topo::enumerate_edges`].
pub fn fillet_edges_by_index(
    shape: &KernelShape,
    radius: f64,
    edges: &[usize],
) -> Result<KernelShape> {
    if radius <= 0.0 {
        bail!("radius fillet harus > 0");
    }
    fillet_points(shape, radius, &edge_points(shape, edges)?)
}

/// Chamfer tepi berdasarkan indeks [`crate::topo::enumerate_edges`].
pub fn chamfer_edges_by_index(
    shape: &KernelShape,
    distance: f64,
    edges: &[usize],
) -> Result<KernelShape> {
    if distance <= 0.0 {
        bail!("jarak chamfer harus > 0");
    }
    chamfer_points(shape, distance, &edge_points(shape, edges)?)
}

// ---------------------------------------------------------------------------
// Shell.
// ---------------------------------------------------------------------------

fn resolve_shell_faces(shape: &KernelShape, opening: ShellOpening) -> Result<Vec<usize>> {
    match opening {
        ShellOpening::Indices(idx) => {
            let n = shape.topology().faces.len();
            if let Some(&bad) = idx.iter().find(|&&i| i >= n) {
                bail!("indeks face {bad} di luar jangkauan (shape punya {n} face)");
            }
            let mut v: Vec<usize> = Vec::with_capacity(idx.len());
            for &i in idx {
                if !v.contains(&i) {
                    v.push(i);
                }
            }
            Ok(v)
        }
        ShellOpening::Rays(rays) => {
            if rays.is_empty() {
                bail!("pilih minimal 1 wajah (atau pakai arah otomatis)");
            }
            ray_faces(shape, rays, "wajah")
        }
        ShellOpening::Farthest(dir) => Ok(vec![farthest_face(shape, dir)?]),
    }
}

/// Inti shell — `depth > 0` = hollow
/// penuh lalu bagian body asli di belakang bidang kedalaman digabung kembali.
fn hollow_open(
    shape: &KernelShape,
    thickness: f64,
    faces: &[usize],
    depth: f64,
) -> Result<KernelShape> {
    if !depth.is_finite() || depth < 0.0 {
        bail!("kedalaman rongga tidak boleh negatif (0 = rongga penuh)");
    }
    let body = shape.require_body("Shell")?;
    let open = face_points(shape, faces)?;
    let hollow = || -> Result<Body> {
        kernel("Shell", || {
            solidcraft_kernel::shell(body, &open, thickness.abs())
        })
        .map_err(|e| anyhow!("operasi shell/hollow gagal: {e:#}"))
    };
    if depth < 1e-9 {
        return checked(hollow()?, "Shell");
    }
    let [face] = faces else {
        bail!("kedalaman rongga hanya berlaku bila persis 1 sisi dibuka (isi 0 untuk rongga penuh)");
    };
    let Some((anchor, normal)) = plane_of_face(shape, *face) else {
        bail!("kedalaman rongga hanya berlaku untuk sisi datar (isi 0 untuk rongga penuh)");
    };
    let extent = shape
        .topology()
        .mesh
        .positions
        .iter()
        .map(|p| (anchor - DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).dot(normal))
        .fold(0.0_f64, f64::max);
    let max_depth = extent - thickness;
    if depth >= max_depth - 1e-6 {
        return Err(ShellDepthTooDeep {
            depth,
            max_depth: max_depth.max(0.0),
        }
        .into());
    }
    // Bagian body di BELAKANG bidang kedalaman (sisi yang menjauhi face terbuka).
    let plane = Plane::from_normal(sc(anchor - normal * depth), sc(normal))
        .ok_or_else(|| anyhow!("normal sisi terbuka tidak terdefinisi"))?;
    let (filler, _) = kernel("Shell", || solidcraft_kernel::half_space(body, &plane, false))
        .context("gagal membagi body pada kedalaman rongga")?;
    let filler = filler.ok_or_else(|| anyhow!("bidang kedalaman rongga tidak memotong body"))?;
    let hollowed = hollow()?;
    let out = kernel("Shell", || {
        solidcraft_kernel::boolean(&hollowed, &filler, BoolOp::Union)
    })
    .context("gagal menutup dasar rongga pada kedalaman yang diminta")?
    .ok_or_else(|| anyhow!("gagal menutup dasar rongga pada kedalaman yang diminta"))?;
    checked(out, "Shell")
}

/// Shell/hollow umum: buka face `opening`, dinding setebal `thickness`,
/// rongga sedalam `depth` mm dari face terbuka (`0` = rongga penuh).
pub fn shell_open(
    shape: &KernelShape,
    thickness: f64,
    opening: ShellOpening,
    depth: f64,
) -> Result<KernelShape> {
    check_shell_thickness(thickness)?;
    let faces = resolve_shell_faces(shape, opening)?;
    hollow_open(shape, thickness, &faces, depth)
}

/// Shell dengan face yang dibuang dipilih lewat indeks.
pub fn shell_faces_by_index(
    shape: &KernelShape,
    thickness: f64,
    remove_faces: &[usize],
) -> Result<KernelShape> {
    shell_open(shape, thickness, ShellOpening::Indices(remove_faces), 0.0)
}

/// Shell membuang face terjauh ke arah `remove_face_dir`.
pub fn shell_hollow(
    shape: &KernelShape,
    thickness: f64,
    remove_face_dir: Direction,
) -> Result<KernelShape> {
    shell_open(shape, thickness, ShellOpening::Farthest(remove_face_dir), 0.0)
}

/// Shell membuang face yang di-pick lewat `rays`.
pub fn shell_hollow_faces(
    shape: &KernelShape,
    thickness: f64,
    rays: &[PickRay],
) -> Result<KernelShape> {
    shell_open(shape, thickness, ShellOpening::Rays(rays), 0.0)
}

/// Shell dengan tebal per face. SolidCraft tidak punya padanannya
/// (PLAN_MIGRASI_TRUCK §4.3); untuk body KONVEKS berface planar (kotak,
/// enclosure, prisma) rongganya dibangun langsung sebagai polihedron
/// konveks: tiap bidang face digeser ke dalam sejauh tebalnya sendiri, face
/// yang dibuka digeser keluar. Body lain → `Unsupported`.
pub fn shell_variable_thickness(
    shape: &KernelShape,
    default_thickness: f64,
    remove_face_rays: &[PickRay],
    variable_thickness: &[(PickRay, f64)],
) -> Result<KernelShape> {
    if default_thickness <= 0.0 {
        bail!("tebal default shell harus > 0");
    }
    if let Some((_, t)) = variable_thickness.iter().find(|(_, t)| *t <= 0.0) {
        bail!("tebal khusus (variable thickness) harus > 0; diberikan {t:.3} mm");
    }
    let body = shape.require_body("Shell")?;
    let removed = ray_faces(shape, remove_face_rays, "wajah yang akan dihilangkan")?;
    let mut custom: Vec<(usize, f64)> = Vec::with_capacity(variable_thickness.len());
    for (ray, t) in variable_thickness {
        let f = ray_faces(shape, std::slice::from_ref(ray), "wajah dengan tebal khusus")?[0];
        custom.retain(|(g, _)| *g != f);
        custom.push((f, *t));
    }
    let topo = shape.topology();
    let size = body.size();
    let eps = size * 1e-7 + 1e-9;
    let mut planes = Vec::with_capacity(topo.faces.len());
    for f in 0..topo.faces.len() {
        let Some((on, n)) = plane_of_face(shape, f) else {
            return unsupported("shell_variable_thickness pada body dengan face lengkung");
        };
        planes.push((n, n.dot(on)));
    }
    let convex = planes.iter().all(|(n, d)| {
        topo.mesh
            .positions
            .iter()
            .all(|p| n.dot(DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)) <= d + eps * 1e3)
    });
    if !convex {
        return unsupported("shell_variable_thickness pada body tidak konveks");
    }
    let margin = default_thickness + size * 0.05;
    let inner: Vec<HalfSpace> = planes
        .iter()
        .enumerate()
        .map(|(f, (n, d))| {
            let shift = if removed.contains(&f) {
                -margin
            } else {
                custom
                    .iter()
                    .find(|(g, _)| *g == f)
                    .map_or(default_thickness, |(_, t)| *t)
            };
            HalfSpace { n: sc(*n), d: d - shift }
        })
        .collect();
    let cavity = kernel("Shell", || solidcraft_kernel::convex_polyhedron(&inner))
        .context("tebal dinding terlalu besar untuk body ini")?;
    let out = kernel("Shell", || solidcraft_kernel::boolean(body, &cavity, BoolOp::Cut))
        .context("operasi shell dengan variable thickness gagal")?
        .ok_or_else(|| anyhow!("shell menghabiskan seluruh body"))?;
    checked(out, "Shell")
}

// ---------------------------------------------------------------------------
// Extrude / revolve face, draft, resize.
// ---------------------------------------------------------------------------

/// Extrude satu sisi (face) sepanjang `distance` mm searah normal keluar:
/// `> 0` menambah material (union), `< 0` memotong (pocket). Face lengkung
/// (silinder) di-offset: radiusnya berubah sejauh `distance`.
pub fn extrude_face(shape: &KernelShape, ray: PickRay, distance: f64) -> Result<KernelShape> {
    if distance.abs() < 1e-9 {
        bail!("jarak extrude face harus tidak nol");
    }
    let body = shape.require_body("Extrude face")?;
    let Some((f, _, _)) = super::picking_face::resolve_face_hit(shape.topology(), ray) else {
        bail!("wajah terpilih tidak ditemukan pada shape");
    };
    let planar = is_planar_face(shape, f);
    if shape.topology().faces[f].kind == SurfaceKind::Cylinder {
        if let Some(r) = super::topo::face_radius(shape.topology(), f) {
            let concave = crate::topo::enumerate_faces(shape)
                .get(f)
                .and_then(|i| i.concave)
                .unwrap_or(false);
            let new_radius = if concave { r - distance } else { r + distance };
            if new_radius <= 0.0 {
                bail!(
                    "jarak drag ({distance:.3} mm) akan membuat radius permukaan ≤ 0 (radius saat ini {r:.3} mm)"
                );
            }
        }
    }
    // Prisma/kulit face: salin face lalu tebalkan searah normal keluar.
    let pts = face_points(shape, &[f])?;
    let sheet = kernel("salin face", || solidcraft_kernel::copy_faces(body, &pts))?;
    let tool = kernel("Extrude face", || {
        solidcraft_kernel::thicken(&sheet, distance, false)
    })
    .context(if planar {
        "gagal membentuk prisma dari wajah terpilih"
    } else {
        "gagal melakukan offset pada permukaan lengkung terpilih"
    })?;
    let op = if distance > 0.0 {
        BoolOp::Union
    } else {
        BoolOp::Cut
    };
    let out = kernel("Extrude face", || solidcraft_kernel::boolean(body, &tool, op)).context(
        if distance > 0.0 {
            "gagal menggabungkan hasil extrude ke shape (mis. wajah bersinggungan dengan rounding di sebelahnya)"
        } else {
            "gagal mengurangi hasil extrude dari shape (mis. wajah bersinggungan dengan rounding di sebelahnya)"
        },
    )?;
    match out {
        Some(b) => checked(b, "Extrude face").context(
            "offset permukaan menghasilkan geometri rusak (mis. menembus lubang/permukaan lain di body yang sama)",
        ),
        None => Ok(KernelShape::empty()),
    }
}

/// Revolve satu face planar mengelilingi sumbu 3D di bidangnya; digabung
/// (union) dengan body asal bila bisa, selain itu hasil revolve saja.
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
    let body = shape.require_body("Revolve face")?;
    let Some((f, _, _)) = super::picking_face::resolve_face_hit(shape.topology(), ray) else {
        bail!("wajah terpilih tidak ditemukan pada shape");
    };
    let Some((anchor, normal)) = plane_of_face(shape, f) else {
        return unsupported("revolve face lengkung (hanya face datar)");
    };
    let axis = axis_dir / dir_len;
    if axis.dot(normal).abs() > 1e-6 {
        bail!("Operasi Revolve Face gagal: sumbu putar harus terletak di bidang face");
    }
    // Bidang sketsa face: origin = titik sumbu di bidang face, x = sumbu.
    let origin = axis_origin - normal * (axis_origin - anchor).dot(normal);
    let plane = Plane::new(sc(origin), sc(axis), sc(normal.cross(axis)))
        .ok_or_else(|| anyhow!("bidang face tidak valid"))?;
    let region = super::bridge::face_region(shape, f, &plane)?;
    let angle = angle_degrees.map_or(std::f64::consts::TAU, f64::to_radians);
    let swept = kernel("Revolve face", || {
        solidcraft_kernel::revolve_whole(
            &plane,
            &[region],
            solidcraft_geom::Vec2::new(0.0, 0.0),
            solidcraft_geom::Vec2::new(1.0, 0.0),
            angle,
        )
    })
    .map_err(|e| {
        anyhow!(
            "Operasi Revolve Face gagal: pastikan sumbu putar tidak memotong bagian dalam face ({e:#})"
        )
    })?;
    let swept = one_body(swept, "Revolve face")?;
    let Some(sb) = swept.body() else {
        return Ok(swept);
    };
    match kernel("Revolve face", || {
        solidcraft_kernel::boolean(body, sb, BoolOp::Union)
    }) {
        Ok(Some(b)) => Ok(KernelShape::from_body(b)),
        _ => Ok(swept),
    }
}

/// Tambahkan draft angle pada face planar yang di-pick lewat `rays`.
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
    if pull_direction.length() < 1e-9 {
        bail!("arah pull tidak valid (vektor nol)");
    }
    if neutral_plane_normal.length() < 1e-9 {
        bail!("normal bidang netral tidak valid (vektor nol)");
    }
    let faces = ray_faces(shape, rays, "face")?;
    if faces.iter().any(|&f| !is_planar_face(shape, f)) {
        bail!("draft angle hanya mendukung face planar; pilih face datar");
    }
    draft_logical(
        shape,
        &faces,
        neutral_plane_point,
        neutral_plane_normal,
        pull_direction,
        angle_deg,
    )
    .map_err(|e| anyhow!("Draft Angle gagal: {e:#}"))
}

/// Draft face logis `faces` (dipakai juga `advanced::draft_faces_by_index`).
pub(crate) fn draft_logical(
    shape: &KernelShape,
    faces: &[usize],
    neutral_point: DVec3,
    neutral_normal: DVec3,
    pull_direction: DVec3,
    angle_deg: f64,
) -> Result<KernelShape> {
    let body = shape.require_body("Draft")?;
    let pts = face_points(shape, faces)?;
    let neutral = Plane::from_normal(sc(neutral_point), sc(neutral_normal.normalize()))
        .ok_or_else(|| anyhow!("normal bidang netral tidak valid"))?;
    let pull = sc(pull_direction.normalize());
    let out = kernel("Draft", || {
        solidcraft_kernel::draft(body, &pts, &neutral, pull, angle_deg.to_radians())
    })?;
    checked(out, "Draft")
}

/// Kotak berfillet (smoke-test kernel), alas di z = 0.
pub fn make_filleted_box(
    width: f64,
    depth: f64,
    height: f64,
    fillet: f64,
) -> Result<KernelShape> {
    let b = kernel("make_filleted_box", || {
        solidcraft_kernel::box_solid(
            Vec3::new(-width / 2.0, -depth / 2.0, 0.0),
            Vec3::new(width / 2.0, depth / 2.0, height),
        )
    })?;
    let shape = KernelShape::from_body(b);
    if fillet > 0.0 {
        return fillet_all(&shape, fillet)
            .context("radius fillet terlalu besar untuk kotak smoke-test ini");
    }
    Ok(shape)
}

/// Ubah ukuran solid sepanjang rusuk: face planar di ujung rusuk yang tegak
/// lurus terhadapnya di-extrude sejauh selisih panjang; selain itu skala
/// seragam (sama dengan backend lama (≤ 0.5.x)).
pub fn resize_shape_along_edge(
    shape: &KernelShape,
    edge_start: (f64, f64, f64),
    edge_end: (f64, f64, f64),
    new_length: f64,
) -> Result<KernelShape> {
    if new_length <= 0.0 {
        bail!("ukuran baru harus positif");
    }
    let p_start = DVec3::new(edge_start.0, edge_start.1, edge_start.2);
    let p_end = DVec3::new(edge_end.0, edge_end.1, edge_end.2);
    let edge_vec = p_end - p_start;
    let old_length = edge_vec.length();
    if old_length < 1e-5 {
        bail!("panjang rusuk terlalu kecil");
    }
    let delta = new_length - old_length;
    if delta.abs() < 1e-6 {
        return clone_shape(shape);
    }
    let dir = edge_vec / old_length;
    // Face di ujung AKHIR rusuk dulu, baru ujung awal: hasilnya tidak
    // bergantung urutan face (backend lama (≤ 0.5.x) kebetulan menemukan face atas dulu).
    let n_faces = shape.topology().faces.len();
    let find = |at: DVec3, outward: DVec3| -> Option<PickRay> {
        (0..n_faces).find_map(|f| {
            let (on, normal) = plane_of_face(shape, f)?;
            (normal.dot(outward) > 0.95 && (at - on).dot(normal).abs() < 1e-2).then(|| PickRay {
                // Ray dari luar face, menembak balik ke dalam.
                origin: (
                    on.x + outward.x * 20.0,
                    on.y + outward.y * 20.0,
                    on.z + outward.z * 20.0,
                ),
                dir: (-outward.x, -outward.y, -outward.z),
            })
        })
    };
    let target = find(p_end, dir).or_else(|| find(p_start, -dir));
    if let Some(ray) = target {
        if let Ok(out) = extrude_face(shape, ray, delta) {
            return Ok(out);
        }
    }
    let factor = new_length / old_length;
    let pivot = (
        (edge_start.0 + edge_end.0) * 0.5,
        (edge_start.1 + edge_end.1) * 0.5,
        (edge_start.2 + edge_end.2) * 0.5,
    );
    crate::shape::scale_shape(shape, pivot, factor)
}

// ---------------------------------------------------------------------------
// Split.
// ---------------------------------------------------------------------------

/// Pecah tiap body jadi solid-solid terpisahnya (lump).
fn per_lump(parts: Vec<Body>) -> Vec<KernelShape> {
    let mut out = Vec::new();
    for p in parts {
        match kernel("Split", || p.lumps()) {
            Ok(lumps) if lumps.len() > 1 => {
                out.extend(lumps.into_iter().map(KernelShape::from_body))
            }
            _ => out.push(KernelShape::from_body(p)),
        }
    }
    out
}

/// Potong solid dengan bidang (`plane_point`, `plane_normal`) jadi
/// potongan-potongan terpisah (biasanya 2).
pub fn split_body(
    shape: &KernelShape,
    plane_point: DVec3,
    plane_normal: DVec3,
) -> Result<Vec<KernelShape>> {
    if plane_normal.length_squared() < 1e-6 {
        bail!("normal bidang pemotong tidak boleh bernilai nol");
    }
    let body = shape.require_body("Split")?;
    let plane = Plane::from_normal(sc(plane_point), sc(plane_normal.normalize()))
        .ok_or_else(|| anyhow!("normal bidang pemotong tidak valid"))?;
    let parts = kernel("Split", || solidcraft_kernel::split_by_plane(body, &plane))
        .context("operasi split body gagal pada kernel truck")?;
    if parts.is_empty() {
        bail!("bidang pemotong tidak membagi objek menjadi bagian valid");
    }
    // Satu sisi bidang bisa berisi beberapa solid (bidang memotong dua kaki
    // huruf U): dipecah per solid, seperti hasil split backend lama (≤ 0.5.x).
    Ok(per_lump(parts))
}

/// Potong solid dengan shape pemotong (face pertamanya diperpanjang).
pub fn split_body_with_tool(
    shape: &KernelShape,
    tool: &KernelShape,
) -> Result<Vec<KernelShape>> {
    let body = shape.require_body("Split")?;
    let tb = tool.require_body("Split")?;
    let parts = kernel("Split", || {
        solidcraft_kernel::split_body(body, &SplitTool::Face { body: tb, face: 0 })
    })
    .context("operasi split body dengan tool gagal pada kernel truck")?;
    if parts.is_empty() {
        bail!("tool pemotong tidak membagi objek menjadi bagian valid");
    }
    Ok(per_lump(parts))
}

/// Bagi face yang dilewati bidang pemotong tanpa memisahkan body.
pub fn split_face(
    shape: &KernelShape,
    plane_point: DVec3,
    plane_normal: DVec3,
) -> Result<KernelShape> {
    if plane_normal.length_squared() < 1e-6 {
        bail!("normal bidang pemotong tidak boleh bernilai nol");
    }
    // Hanya face yang benar-benar dilintasi bidang (kernel menolak face
    // yang tidak terpotong) — lihat `direct::split_faces_by_plane`.
    super::direct::split_faces_by_plane(shape, &[], plane_point, plane_normal)
}

// ---------------------------------------------------------------------------
// Rib.
// ---------------------------------------------------------------------------

/// Solid tulang penguat antara `start_pt`–`end_pt`, setebal `thickness`
/// simetris ke samping, setinggi `depth` searah `normal_dir`. Dengan
/// `draft_angle_deg` kedua sisi panjangnya condong masuk ke arah puncak
/// (engsel di alas).
///
/// Dibangun sebagai polihedron konveks dari setengah-ruang — juga bila garis
/// rib tidak tegak lurus `normal_dir` (prisma miring seperti backend lama (≤ 0.5.x)).
pub fn create_rib_solid(
    start_pt: DVec3,
    end_pt: DVec3,
    normal_dir: DVec3,
    thickness: f64,
    depth: f64,
    draft_angle_deg: Option<f64>,
) -> Result<KernelShape> {
    if thickness <= 0.0 {
        bail!("tebal tulang penguat (rib thickness) harus > 0; diberikan {thickness:.3} mm");
    }
    if depth <= 0.0 {
        bail!("kedalaman tulang penguat (rib depth) harus > 0; diberikan {depth:.3} mm");
    }
    let edge_vec = end_pt - start_pt;
    let length = edge_vec.length();
    if length < 1e-4 {
        bail!("panjang garis tulang penguat terlalu pendek");
    }
    let norm_len = normal_dir.length();
    if norm_len < 1e-4 {
        bail!("arah kedalaman tulang penguat tidak valid (vektor nol)");
    }
    let dir = edge_vec / length;
    let mut up = normal_dir / norm_len;
    let mut side = dir.cross(up);
    if side.length() < 1e-4 {
        // Garis rib sejajar arah kedalaman: prisma tidak terdefinisi di
        // Backend lama (≤ 0.5.x) juga; pakai sumbu cadangan yang sama.
        let fallback = if dir.z.abs() < 0.9 { DVec3::Z } else { DVec3::Y };
        side = dir.cross(fallback);
        up = side.cross(dir).normalize();
    }
    let side = side.normalize();
    // Setengah-ruang `n·x <= d`, `n` keluar.
    let mut hs: Vec<(DVec3, f64)> = Vec::with_capacity(6);
    // Alas & puncak: bidang yang memuat `dir` dan `side`.
    let mut base_n = dir.cross(side).normalize();
    if base_n.dot(up) > 0.0 {
        base_n = -base_n;
    }
    hs.push((base_n, base_n.dot(start_pt)));
    hs.push((-base_n, (-base_n).dot(start_pt + up * depth)));
    // Ujung: bidang yang memuat `side` dan `up`.
    let mut end_n = side.cross(up).normalize();
    if end_n.dot(dir) < 0.0 {
        end_n = -end_n;
    }
    hs.push((end_n, end_n.dot(end_pt)));
    hs.push((-end_n, (-end_n).dot(start_pt)));
    // Sisi panjang: bidang yang memuat `dir` dan `up`, condong masuk bila draft.
    let tilt = match draft_angle_deg {
        Some(a) if a > 0.0 && a < 89.0 => a.to_radians(),
        _ => 0.0,
    };
    let rise = (up - dir * up.dot(dir) - side * up.dot(side)).normalize_or_zero();
    for s in [1.0, -1.0] {
        let wall = side * s;
        let foot = start_pt + wall * (thickness * 0.5);
        let n = (wall * tilt.cos() + rise * tilt.sin()).normalize();
        hs.push((n, n.dot(foot)));
    }
    let halfspaces: Vec<HalfSpace> = hs
        .into_iter()
        .map(|(n, d)| HalfSpace { n: sc(n), d })
        .collect();
    kernel("Rib", || solidcraft_kernel::convex_polyhedron(&halfspaces)).map(KernelShape::from_body)
}

/// Tambahkan tulang penguat ke `shape` (union).
pub fn create_rib(
    shape: &KernelShape,
    start_pt: DVec3,
    end_pt: DVec3,
    normal_dir: DVec3,
    thickness: f64,
    depth: f64,
    draft_angle_deg: Option<f64>,
) -> Result<KernelShape> {
    let rib = create_rib_solid(start_pt, end_pt, normal_dir, thickness, depth, draft_angle_deg)?;
    let body = shape.require_body("Rib")?;
    let rb = rib.require_body("Rib")?;
    match kernel("Rib", || solidcraft_kernel::boolean(body, rb, BoolOp::Union)) {
        Ok(Some(b)) => Ok(KernelShape::from_body(b)),
        _ => bail!(
            "Gagal menggabungkan tulang penguat ke bodi utama (pastikan posisi rib menyentuh dinding bodi)"
        ),
    }
}
