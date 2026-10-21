//! Permukaan (sheet) Fase 6A (PLAN_PARITAS_SOLIDCRAFT §6A): extrude/revolve/
//! sweep/loft profil terbuka, patch, boundary (Coons/Gordon), ruled, offset,
//! extend, trim/untrim, stitch/unstitch, thicken, blend, match, fair,
//! reverse normal, salin face. Semua lewat operasi permukaan SolidCraft.
//!
//! Edge/face dipilih lewat indeks logis DUCAD lalu dipetakan ke titik /
//! indeks face SolidCraft.

use anyhow::{anyhow, bail, Result};
use glam::DVec3;
use solidcraft_geom::{Plane, Vec2, Vec3};
use solidcraft_kernel::Body;

use super::bridge::sketch_plane;
use super::direct::{chains_to_seg2, member_faces};
use super::kernel;
use super::modify::{face_points, sc};
use crate::profile::ProfileSegment;
use crate::shape::KernelShape;

/// Titik SolidCraft untuk edge logis `edges`, termasuk tepi batas terbuka
/// permukaan (yang ditolak `modify::edge_points` sebagai seam).
fn edge_points(shape: &KernelShape, edges: &[usize]) -> Result<Vec<Vec3>> {
    let topo = shape.topology();
    let mut out = Vec::new();
    for &i in edges {
        let Some(edge) = topo.edges.get(i) else {
            bail!(
                "indeks tepi {i} di luar jangkauan (shape punya {} tepi)",
                topo.edges.len()
            );
        };
        out.extend(edge.member_mids.iter().map(|p| sc(*p)));
    }
    Ok(out)
}

/// Bingkai bidang sketsa: (asal, sumbu u, sumbu v).
pub type SketchFrame = ([f64; 3], [f64; 3], [f64; 3]);

/// Kelas kontinuitas untuk blend/match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceContinuity {
    G0,
    G1,
    G2,
}

fn cont(c: SurfaceContinuity) -> solidcraft_kernel::Continuity {
    match c {
        SurfaceContinuity::G0 => solidcraft_kernel::Continuity::G0,
        SurfaceContinuity::G1 => solidcraft_kernel::Continuity::G1,
        SurfaceContinuity::G2 => solidcraft_kernel::Continuity::G2,
    }
}

fn sheet(b: Body, what: &str) -> Result<KernelShape> {
    let s = KernelShape::from_body(b);
    if s.tessellate().indices.is_empty() {
        bail!("{what} menghasilkan permukaan kosong");
    }
    Ok(s)
}

type Curves = Vec<Vec<solidcraft_geom::Seg2>>;

fn plane_and_curves(frame: SketchFrame, chains: &[Vec<ProfileSegment>]) -> Result<(Plane, Curves, f64)> {
    let (plane, scale) = sketch_plane(frame.0, frame.1, frame.2)?;
    let curves = chains_to_seg2(chains, scale);
    if curves.is_empty() {
        bail!("tidak ada kurva profil (sketsa kosong)");
    }
    Ok((plane, curves, scale))
}

fn finite(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(anyhow!("{name} tidak valid ({v})"))
    }
}

/// 6.1 Extrude kurva (terbuka/tertutup) jadi permukaan, dari `start` ke
/// `end` (mm sepanjang normal bidang).
pub fn extrude_sheet(
    frame: SketchFrame,
    chains: &[Vec<ProfileSegment>],
    start: f64,
    end: f64,
) -> Result<KernelShape> {
    let (start, end) = (finite("start", start)?, finite("end", end)?);
    if (end - start).abs() < 1e-6 {
        bail!("panjang extrude permukaan harus tidak nol");
    }
    let (plane, curves, _) = plane_and_curves(frame, chains)?;
    let b = kernel("Surface extrude", || {
        solidcraft_kernel::extrude_sheet(&plane, &curves, start, end)
    })?;
    sheet(b, "Surface extrude")
}

/// 6.1 Revolve kurva mengelilingi sumbu 2D di bidang sketsa (`sweep_deg`).
pub fn revolve_sheet(
    frame: SketchFrame,
    chains: &[Vec<ProfileSegment>],
    axis_origin: (f64, f64),
    axis_dir: (f64, f64),
    sweep_deg: f64,
) -> Result<KernelShape> {
    let sweep = finite("sudut", sweep_deg)?;
    if sweep.abs() < 1e-6 || sweep.abs() > 360.0 + 1e-9 {
        bail!("sudut revolve permukaan harus 0 < |sudut| ≤ 360");
    }
    let axis_len = axis_dir.0.hypot(axis_dir.1);
    if axis_len.is_nan() || axis_len <= 1e-9 {
        bail!("arah sumbu revolve tidak boleh nol");
    }
    let (plane, curves, scale) = plane_and_curves(frame, chains)?;
    let ao = Vec2::new(axis_origin.0 * scale, axis_origin.1 * scale);
    let ad = Vec2::new(axis_dir.0, axis_dir.1);
    let b = kernel("Surface revolve", || {
        solidcraft_kernel::revolve_sheet(&plane, &curves, ao, ad, 0.0, sweep.to_radians())
    })?;
    sheet(b, "Surface revolve")
}

/// 6.1 Sapu kurva sepanjang jalur titik 3D.
pub fn sweep_sheet(
    frame: SketchFrame,
    chains: &[Vec<ProfileSegment>],
    path: &[[f64; 3]],
) -> Result<KernelShape> {
    if path.len() < 2 || path.iter().flatten().any(|v| !v.is_finite()) {
        bail!("jalur sweep permukaan butuh ≥ 2 titik hingga");
    }
    let (plane, curves, _) = plane_and_curves(frame, chains)?;
    let pts: Vec<Vec3> = path.iter().map(|p| Vec3::new(p[0], p[1], p[2])).collect();
    let b = kernel("Surface sweep", || {
        solidcraft_kernel::sweep_sheet_points(&plane, &curves, &pts)
    })?;
    sheet(b, "Surface sweep")
}

/// 6.1 Loft permukaan melewati penampang (satu rantai per bidang).
pub fn loft_sheet(sections: &[(SketchFrame, Vec<ProfileSegment>)]) -> Result<KernelShape> {
    if sections.len() < 2 {
        bail!("loft permukaan butuh ≥ 2 penampang");
    }
    let mut secs = Vec::new();
    for (frame, chain) in sections {
        let (plane, mut curves, _) = plane_and_curves(*frame, std::slice::from_ref(chain))?;
        let segs = curves.pop().ok_or_else(|| anyhow!("penampang kosong"))?;
        secs.push((plane, segs));
    }
    let b = kernel("Surface loft", || solidcraft_kernel::loft_sheet(&secs))?;
    sheet(b, "Surface loft")
}

/// 6.2 Patch: tutup lubang yang dibatasi tepi terbuka `edges` body.
pub fn patch(shape: &KernelShape, edges: &[usize]) -> Result<KernelShape> {
    let body = shape.require_body("Patch")?;
    let pts = edge_points(shape, edges)?;
    if pts.is_empty() {
        bail!("pilih tepi batas yang akan ditutup");
    }
    let b = kernel("Patch", || solidcraft_kernel::patch_edges(body, &pts))?;
    sheet(b, "Patch")
}

/// 6.2 Boundary: permukaan melalui kurva `u` (dan `v`: Coons bila 2×2,
/// Gordon bila lebih). Kurva berupa titik 3D (spline kubik melewatinya).
pub fn boundary(u: &[Vec<[f64; 3]>], v: &[Vec<[f64; 3]>]) -> Result<KernelShape> {
    fn curves(
        set: &[Vec<[f64; 3]>],
        tag: &str,
    ) -> Result<Vec<solidcraft_kernel::BoundaryCurve<'static>>> {
        set.iter()
            .enumerate()
            .map(|(i, c)| {
                if c.len() < 2 || c.iter().flatten().any(|x| !x.is_finite()) {
                    bail!("kurva {tag}{} butuh ≥ 2 titik hingga", i + 1);
                }
                Ok(solidcraft_kernel::BoundaryCurve {
                    label: format!("{tag}{}", i + 1),
                    source: solidcraft_kernel::CurveSource::Points(
                        c.iter().map(|p| Vec3::new(p[0], p[1], p[2])).collect(),
                    ),
                    continuity: solidcraft_kernel::Continuity::G0,
                })
            })
            .collect()
    }
    let (cu, cv) = (curves(u, "u")?, curves(v, "v")?);
    if cu.len() < 2 {
        bail!("boundary butuh ≥ 2 kurva u");
    }
    let b = kernel("Boundary", || solidcraft_kernel::boundary_surface(&cu, &cv))?;
    sheet(b, "Boundary")
}

/// Arah lajur ruled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuledDir {
    Normal,
    Tangent,
    Direction([f64; 3]),
}

/// 6.3 Ruled: pita selebar `distance` dari tepi `edges`.
pub fn ruled(
    shape: &KernelShape,
    edges: &[usize],
    distance: f64,
    dir: RuledDir,
    flip: bool,
) -> Result<KernelShape> {
    let distance = finite("jarak", distance)?;
    if distance <= 0.0 {
        bail!("jarak ruled harus > 0");
    }
    let body = shape.require_body("Ruled")?;
    let pts = edge_points(shape, edges)?;
    let mode = match dir {
        RuledDir::Normal => solidcraft_kernel::RuledMode::Normal,
        RuledDir::Tangent => solidcraft_kernel::RuledMode::Tangent,
        RuledDir::Direction(d) => {
            let v = DVec3::from(d);
            if !v.is_finite() || v.length() <= 1e-9 {
                bail!("arah ruled tidak boleh nol");
            }
            solidcraft_kernel::RuledMode::Direction(sc(v.normalize()))
        }
    };
    let r = solidcraft_kernel::Ruled {
        distance,
        mode,
        flip,
        corners: solidcraft_kernel::RuledCorners::Mitre,
        face: None,
    };
    let b = kernel("Ruled", || solidcraft_kernel::ruled_surface(body, &pts, &r))?;
    sheet(b, "Ruled")
}

/// 6.3 Offset permukaan face `faces` sejauh `distance` (permukaan baru).
pub fn offset_surface(shape: &KernelShape, faces: &[usize], distance: f64) -> Result<KernelShape> {
    let distance = finite("jarak", distance)?;
    if distance.abs() < 1e-9 {
        bail!("jarak offset permukaan harus tidak nol");
    }
    let body = shape.require_body("Offset surface")?;
    let members = member_faces(shape, faces)?;
    let b = kernel("Offset surface", || {
        solidcraft_kernel::offset_surface(body, &members, distance, 1e-3)
    })?;
    sheet(b, "Offset surface")
}

/// 6.3 Extend: perpanjang tepi batas `edges` sejauh `distance`.
pub fn extend(shape: &KernelShape, edges: &[usize], distance: f64) -> Result<KernelShape> {
    let distance = finite("jarak", distance)?;
    if distance <= 0.0 {
        bail!("jarak extend harus > 0");
    }
    let body = shape.require_body("Extend")?;
    let pts = edge_points(shape, edges)?;
    let opts = solidcraft_kernel::ExtendOptions {
        mode: solidcraft_kernel::ExtendMode::Natural,
        limit: solidcraft_kernel::ExtendLimit::Distance(distance),
        neighbours: true,
    };
    let b = kernel("Extend", || {
        solidcraft_kernel::extend_surface(body, &pts, &opts)
    })?;
    sheet(b, "Extend")
}

/// 6.4 Trim permukaan dengan bidang; sisi yang memuat `keep` dipertahankan.
pub fn trim_by_plane(
    shape: &KernelShape,
    point: DVec3,
    normal: DVec3,
    keep: DVec3,
) -> Result<KernelShape> {
    if !normal.is_finite() || normal.length() <= 1e-9 || !point.is_finite() || !keep.is_finite() {
        bail!("bidang trim tidak valid");
    }
    let body = shape.require_body("Trim")?;
    let plane = Plane::from_normal(sc(point), sc(normal.normalize()))
        .ok_or_else(|| anyhow!("bidang trim tidak valid"))?;
    let tool = solidcraft_kernel::SplitTool::Plane(plane);
    let b = kernel("Trim", || solidcraft_kernel::trim(body, &tool, sc(keep)))?;
    sheet(b, "Trim")
}

/// 6.4 Untrim: buang semua lubang (loop dalam) face `face`.
pub fn untrim(shape: &KernelShape, face: usize) -> Result<KernelShape> {
    let body = shape.require_body("Untrim")?;
    let m = member_faces(shape, &[face])?;
    let f = *m.first().ok_or_else(|| anyhow!("face tidak ditemukan"))?;
    let b = kernel("Untrim", || {
        solidcraft_kernel::untrim(body, f, solidcraft_kernel::UntrimHoles::All, false, true)
    })?;
    sheet(b, "Untrim")
}

/// 6.4 Stitch: jahit beberapa permukaan (toleransi `tol` mm). Hasil yang
/// tertutup menjadi solid.
pub fn stitch(shapes: &[&KernelShape], tol: f64) -> Result<KernelShape> {
    if shapes.len() < 2 {
        bail!("stitch butuh ≥ 2 permukaan");
    }
    let tol = finite("toleransi", tol)?.max(1e-6);
    let bodies: Vec<&Body> = shapes
        .iter()
        .map(|s| s.require_body("Stitch"))
        .collect::<Result<_>>()?;
    let b = kernel("Stitch", || solidcraft_kernel::stitch(&bodies, tol))?;
    sheet(b, "Stitch")
}

/// 6.4 Unstitch: lepas face `faces` jadi permukaan sendiri. Mengembalikan
/// (potongan lepas, sisa).
pub fn unstitch(
    shape: &KernelShape,
    faces: &[usize],
) -> Result<(Vec<KernelShape>, Option<KernelShape>)> {
    let body = shape.require_body("Unstitch")?;
    let pts = face_points(shape, faces)?;
    let (pieces, rest) = kernel("Unstitch", || solidcraft_kernel::unstitch(body, &pts))?;
    Ok((
        pieces.into_iter().map(KernelShape::from_body).collect(),
        rest.map(KernelShape::from_body),
    ))
}

/// 6.5 Thicken permukaan jadi solid.
pub fn thicken(shape: &KernelShape, thickness: f64, symmetric: bool) -> Result<KernelShape> {
    let t = finite("tebal", thickness)?;
    if t.abs() < 1e-6 {
        bail!("tebal thicken harus tidak nol");
    }
    let body = shape.require_body("Thicken")?;
    let b = kernel("Thicken", || solidcraft_kernel::thicken(body, t, symmetric))?;
    Ok(KernelShape::from_body(b))
}

/// 6.6 Blend: permukaan penghubung tepi `a_edges` body A dan `b_edges`
/// body B dengan kontinuitas `c` di kedua sisi.
pub fn blend(
    a: &KernelShape,
    a_edges: &[usize],
    b: &KernelShape,
    b_edges: &[usize],
    c: SurfaceContinuity,
) -> Result<KernelShape> {
    let (ba, bb) = (a.require_body("Blend")?, b.require_body("Blend")?);
    let (pa, pb) = (edge_points(a, a_edges)?, edge_points(b, b_edges)?);
    let (out, _) = kernel("Blend", || {
        solidcraft_kernel::blend_between(ba, &pa, bb, &pb, [cont(c), cont(c)], [0.0, 0.0], None, 16, false)
    })?;
    sheet(out, "Blend")
}

/// 6.6 Match: ubah face `faces` agar menyambung ke tepi `target_edges`
/// body target dengan kontinuitas `c`.
pub fn match_to(
    shape: &KernelShape,
    faces: &[usize],
    target: &KernelShape,
    target_edges: &[usize],
    c: SurfaceContinuity,
) -> Result<KernelShape> {
    let body = shape.require_body("Match")?;
    let tb = target.require_body("Match")?;
    let fp = face_points(shape, faces)?;
    let tp = edge_points(target, target_edges)?;
    let b = kernel("Match", || {
        solidcraft_kernel::match_surface(body, &fp, tb, &tp, cont(c))
    })?;
    sheet(b, "Match")
}

/// 6.6 Fair: haluskan face `faces` (opsi bawaan SolidCraft).
pub fn fair(shape: &KernelShape, faces: &[usize]) -> Result<KernelShape> {
    let body = shape.require_body("Fair")?;
    let members = member_faces(shape, faces)?;
    let opts = solidcraft_kernel::FairOptions::default();
    let (b, _) = kernel("Fair", || solidcraft_kernel::fair_faces(body, &members, &opts))?;
    Ok(KernelShape::from_body(b))
}

/// 6.7 Balik normal face `faces`.
pub fn reverse_normals(shape: &KernelShape, faces: &[usize]) -> Result<KernelShape> {
    let body = shape.require_body("Reverse")?;
    let pts = face_points(shape, faces)?;
    let b = kernel("Reverse normal", || {
        solidcraft_kernel::reverse_faces(body, &pts)
    })?;
    Ok(KernelShape::from_body(b))
}

/// Salin face body jadi permukaan baru.
pub fn copy_faces(shape: &KernelShape, faces: &[usize]) -> Result<KernelShape> {
    let body = shape.require_body("Copy faces")?;
    let pts = face_points(shape, faces)?;
    let b = kernel("Copy faces", || solidcraft_kernel::copy_faces(body, &pts))?;
    sheet(b, "Copy faces")
}
