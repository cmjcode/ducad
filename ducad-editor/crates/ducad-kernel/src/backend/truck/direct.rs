//! Direct editing & alat solid Fase 2 (PLAN_PARITAS_SOLIDCRAFT §2): offset
//! face, delete face, replace face, split face (bidang/kurva), boundary fill,
//! bounding solid, torus. Semua memilih face lewat indeks logis
//! [`crate::topo::enumerate_faces`] lalu dipetakan ke face SolidCraft.

use anyhow::{anyhow, bail, Context, Result};
use glam::DVec3;
use solidcraft_geom::{Plane, Seg2, Vec2, Vec3};
use solidcraft_kernel::{Body, BoolOp, SplitTool};

use super::bridge::{arc_through, sketch_plane};
use super::kernel;
use super::modify::{checked, face_points, sc};
use crate::profile::ProfileSegment;
use crate::shape::KernelShape;

/// Face SolidCraft (urutan `face_iter`) dari face logis `faces`.
pub(crate) fn member_faces(shape: &KernelShape, faces: &[usize]) -> Result<Vec<usize>> {
    let topo = shape.topology();
    let mut out = Vec::new();
    for &i in faces {
        let Some(face) = topo.faces.get(i) else {
            bail!(
                "indeks face {i} di luar jangkauan (shape punya {} face)",
                topo.faces.len()
            );
        };
        for &m in &face.members {
            if !out.contains(&m) {
                out.push(m);
            }
        }
    }
    Ok(out)
}

fn bool_op(a: &Body, b: &Body, op: BoolOp, what: &str) -> Result<Option<Body>> {
    kernel(what, || solidcraft_kernel::boolean(a, b, op))
}

/// Ukuran (diagonal bbox) shape, minimal 1 mm.
fn size_of(shape: &KernelShape) -> f64 {
    shape
        .bounding_box()
        .map(|(lo, hi)| DVec3::from(hi).distance(DVec3::from(lo)))
        .filter(|d| d.is_finite())
        .unwrap_or(1.0)
        .max(1.0)
}

// ---------------------------------------------------------------------------
// Offset / delete face.
// ---------------------------------------------------------------------------

/// Geser face `faces` sejauh `distance` mm searah normal keluar (negatif:
/// masuk ke body). Face lain mengikuti (topologi dipertahankan): plane
/// ditranslasi, silinder/kerucut/bola/torus berubah radius. Bila kernel
/// menolak (topologi berubah), face dicoba sebagai press-pull: kulit face
/// ditebalkan lalu di-union/di-cut.
pub fn offset_faces_by_index(
    shape: &KernelShape,
    faces: &[usize],
    distance: f64,
) -> Result<KernelShape> {
    if faces.is_empty() {
        bail!("tidak ada face yang dipilih untuk di-offset");
    }
    if !distance.is_finite() || distance.abs() < 1e-9 {
        bail!("jarak offset face harus tidak nol (diberikan {distance})");
    }
    let body = shape.require_body("Offset face")?;
    let pts = face_points(shape, faces)?;
    let first = kernel("Offset face", || {
        solidcraft_kernel::offset_faces(body, &pts, distance)
    });
    match first {
        Ok(b) => checked(b, "Offset face"),
        Err(e) => {
            // Press-pull: kulit face → tebalkan → union / cut.
            let sheet = kernel("salin face", || solidcraft_kernel::copy_faces(body, &pts))
                .context("offset face gagal")?;
            let tool = kernel("Offset face", || {
                solidcraft_kernel::thicken(&sheet, distance, false)
            })
            .map_err(|_| e)
            .context("offset face gagal (face tidak bisa dipindah tanpa mengubah topologi)")?;
            let op = if distance > 0.0 { BoolOp::Union } else { BoolOp::Cut };
            match bool_op(body, &tool, op, "Offset face")? {
                Some(b) => checked(b, "Offset face"),
                None => Ok(KernelShape::empty()),
            }
        }
    }
}

/// Hapus face `faces` dan tutup celahnya dengan memperpanjang face planar
/// tetangga (menghapus fillet, chamfer, boss, lubang).
pub fn delete_faces_by_index(shape: &KernelShape, faces: &[usize]) -> Result<KernelShape> {
    if faces.is_empty() {
        bail!("tidak ada face yang dipilih untuk dihapus");
    }
    let body = shape.require_body("Delete face")?;
    let members = member_faces(shape, faces)?;
    if members.len() >= body.face_count() {
        bail!("tidak bisa menghapus semua face body");
    }
    let out = kernel("Delete face", || solidcraft_kernel::delete_faces(body, &members))?;
    checked(out, "Delete face")
}

// ---------------------------------------------------------------------------
// Replace face.
// ---------------------------------------------------------------------------

/// Slab setebal `depth` (negatif: berlawanan `normal`) dari bidang
/// (`point`, `normal`), selebar `2·extent` di sekitar proyeksi `center`.
fn half_slab(point: DVec3, normal: DVec3, center: DVec3, extent: f64, depth: f64) -> Result<Body> {
    let n = normal.normalize();
    let origin = center - n * (center - point).dot(n);
    let plane = Plane::from_normal(sc(origin), sc(n))
        .ok_or_else(|| anyhow!("normal bidang tidak valid"))?;
    let h = extent;
    let pts = [
        Vec2::new(-h, -h),
        Vec2::new(h, -h),
        Vec2::new(h, h),
        Vec2::new(-h, h),
    ];
    let region = solidcraft_geom::Region2 {
        outer: solidcraft_geom::Loop2::polygon(&pts),
        holes: Vec::new(),
    };
    let (start, end) = if depth >= 0.0 { (0.0, depth) } else { (depth, 0.0) };
    let mut v = kernel("slab", || {
        solidcraft_kernel::extrude(&plane, &[region], start, end)
    })?;
    v.pop().ok_or_else(|| anyhow!("gagal membentuk slab pemotong"))
}

/// Ganti face planar `face` dengan bidang (`plane_point`, `plane_normal`):
/// material di antara face dan bidang (di dalam jejak face) ditambah bila
/// bidang berada di luar, atau dibuang bila bidang masuk ke body. Bidang
/// boleh miring, asal tidak (hampir) tegak lurus face.
pub fn replace_face_with_plane(
    shape: &KernelShape,
    face: usize,
    plane_point: DVec3,
    plane_normal: DVec3,
) -> Result<KernelShape> {
    if !plane_point.is_finite() || !plane_normal.is_finite() || plane_normal.length() <= 1e-9 {
        bail!("bidang pengganti tidak valid");
    }
    let body = shape.require_body("Replace face")?;
    let Some((fp, n)) = super::modify::plane_of_face(shape, face) else {
        bail!("face {face} bukan face planar (Replace Face hanya untuk face datar)");
    };
    let mut m = plane_normal.normalize();
    if m.dot(n) < 0.0 {
        m = -m;
    }
    if m.dot(n) < 0.05 {
        bail!("bidang pengganti hampir tegak lurus face (sudut > 87°)");
    }
    let size = size_of(shape);
    let reach = size * 2.0 + (fp - plane_point).dot(m).abs() * 4.0;
    let pts = face_points(shape, &[face])?;
    let sheet = kernel("salin face", || solidcraft_kernel::copy_faces(body, &pts))?;
    let p_out = kernel("Replace face", || {
        solidcraft_kernel::thicken(&sheet, reach, false)
    })?;
    let p_in = kernel("Replace face", || {
        solidcraft_kernel::thicken(&sheet, -reach, false)
    })?;
    let extent = reach * 2.0;
    let h_in = half_slab(plane_point, m, fp, extent, -extent)?;
    let h_out = half_slab(plane_point, m, fp, extent, extent)?;
    let mut cur = body.clone();
    if let Some(add) = bool_op(&p_out, &h_in, BoolOp::Intersect, "Replace face")? {
        if let Some(b) = bool_op(&cur, &add, BoolOp::Union, "Replace face")? {
            cur = b;
        }
    }
    if let Some(cut) = bool_op(&p_in, &h_out, BoolOp::Intersect, "Replace face")? {
        match bool_op(&cur, &cut, BoolOp::Cut, "Replace face")? {
            Some(b) => cur = b,
            None => return Ok(KernelShape::empty()),
        }
    }
    checked(cur, "Replace face")
}

// ---------------------------------------------------------------------------
// Split face.
// ---------------------------------------------------------------------------

fn split_with(shape: &KernelShape, faces: &[usize], tool: &SplitTool) -> Result<KernelShape> {
    let body = shape.require_body("Split face")?;
    let members = if faces.is_empty() {
        (0..body.face_count()).collect()
    } else {
        member_faces(shape, faces)?
    };
    kernel("Split face", || {
        solidcraft_kernel::split_faces(body, &members, tool)
    })
    .context("split face gagal (alat pemotong tidak memotong face terpilih?)")
    .map(KernelShape::from_body)
}

/// Bagi face `faces` (kosong = semua) dengan bidang; solid tidak berubah.
pub fn split_faces_by_plane(
    shape: &KernelShape,
    faces: &[usize],
    plane_point: DVec3,
    plane_normal: DVec3,
) -> Result<KernelShape> {
    if !plane_normal.is_finite() || plane_normal.length() <= 1e-9 {
        bail!("normal bidang pemotong tidak boleh nol");
    }
    let plane = Plane::from_normal(sc(plane_point), sc(plane_normal.normalize()))
        .ok_or_else(|| anyhow!("normal bidang pemotong tidak valid"))?;
    if !faces.is_empty() {
        return split_with(shape, faces, &SplitTool::Plane(plane));
    }
    // "Semua face": hanya face yang mesh-nya melintasi bidang (kernel
    // menolak face yang tidak terpotong).
    let n = plane_normal.normalize();
    let topo = shape.topology();
    let mesh = &topo.mesh;
    let tol = size_of(shape) * 1e-6;
    let crossing: Vec<usize> = mesh
        .face_ranges
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for k in r.start as usize..r.end as usize {
                let Some(&i) = mesh.indices.get(k) else { continue };
                let Some(v) = mesh.positions.get(i as usize) else { continue };
                let d = (DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64) - plane_point).dot(n);
                lo = lo.min(d);
                hi = hi.max(d);
            }
            lo < -tol && hi > tol
        })
        .map(|(i, _)| i)
        .collect();
    if crossing.is_empty() {
        bail!("bidang pemotong tidak melintasi face mana pun");
    }
    split_with(shape, &crossing, &SplitTool::Plane(plane))
}

/// Bagi face `faces` (kosong = semua) dengan kurva sketsa: tiap rantai
/// terbuka di bidang (`origin`, `u_axis`, `v_axis`) disapu searah normal
/// bidang dan diperpanjang melewati ujungnya.
pub fn split_faces_by_curves(
    shape: &KernelShape,
    faces: &[usize],
    origin: [f64; 3],
    u_axis: [f64; 3],
    v_axis: [f64; 3],
    chains: &[Vec<ProfileSegment>],
) -> Result<KernelShape> {
    let (plane, scale) = sketch_plane(origin, u_axis, v_axis)?;
    let curves = chains_to_seg2(chains, scale);
    if curves.is_empty() {
        bail!("tidak ada kurva pemotong (sketsa kosong)");
    }
    split_with(shape, faces, &SplitTool::Curves { plane, curves })
}

/// Rantai segmen profil (koordinat 2D sketsa × `scale`) → segmen SolidCraft;
/// segmen degenerate dibuang, rantai kosong dilewati.
pub(crate) fn chains_to_seg2(chains: &[Vec<ProfileSegment>], scale: f64) -> Vec<Vec<Seg2>> {
    let p = |q: (f64, f64)| Vec2::new(q.0 * scale, q.1 * scale);
    let mut curves = Vec::new();
    for chain in chains {
        let mut segs = Vec::new();
        for s in chain {
            match *s {
                ProfileSegment::Line { start, end } => {
                    let (a, b) = (p(start), p(end));
                    if a.dist(b) > 1e-6 {
                        segs.push(Seg2::Line { a, b });
                    }
                }
                ProfileSegment::Arc { start, via, end } => {
                    let (a, m, b) = (p(start), p(via), p(end));
                    if a.dist(b) > 1e-6 {
                        segs.push(arc_through(a, m, b).unwrap_or(Seg2::Line { a, b }));
                    }
                }
                ProfileSegment::Bezier { start, c1, c2, end } => segs.push(Seg2::Cubic {
                    p0: p(start),
                    p1: p(c1),
                    p2: p(c2),
                    p3: p(end),
                }),
            }
        }
        if !segs.is_empty() {
            curves.push(segs);
        }
    }
    curves
}

// ---------------------------------------------------------------------------
// Titik di dalam solid, boundary fill, bounding solid.
// ---------------------------------------------------------------------------

/// Apakah `p` berada di dalam solid (paritas sinar terhadap mesh tampilan,
/// suara mayoritas 3 arah).
pub fn point_inside(shape: &KernelShape, p: DVec3) -> bool {
    let mesh = shape.tessellate();
    if mesh.indices.len() < 3 || !p.is_finite() {
        return false;
    }
    // Arah sedikit miring agar tidak menyusur edge/vertex mesh.
    let dirs = [
        DVec3::new(0.5773, 0.5774, 0.5775).normalize(),
        DVec3::new(-0.3127, 0.8123, -0.4925).normalize(),
        DVec3::new(0.6913, -0.1213, 0.7121).normalize(),
    ];
    let pos = |i: u32| -> DVec3 {
        let v = mesh.positions.get(i as usize).copied().unwrap_or([0.0; 3]);
        DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64)
    };
    let mut votes = 0;
    for d in dirs {
        let mut hits = 0usize;
        for t in mesh.indices.chunks_exact(3) {
            let (a, b, c) = (pos(t[0]), pos(t[1]), pos(t[2]));
            // Möller–Trumbore.
            let (e1, e2) = (b - a, c - a);
            let h = d.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-14 {
                continue;
            }
            let f = 1.0 / det;
            let s = p - a;
            let u = f * s.dot(h);
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = s.cross(e1);
            let v = f * d.dot(q);
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            if f * e2.dot(q) > 1e-9 {
                hits += 1;
            }
        }
        if hits % 2 == 1 {
            votes += 1;
        }
    }
    votes >= 2
}

/// Pecah body jadi solid-solid terpisahnya.
fn lumps(body: Body) -> Vec<Body> {
    match kernel("lumps", || body.lumps()) {
        Ok(l) if !l.is_empty() => l,
        _ => vec![body],
    }
}

/// Boundary Fill: isi sel yang dibatasi `tools` dan memuat `points`.
///
/// Titik di dalam ≥ 1 tool → sel = irisan tool yang memuatnya dikurangi
/// tool lain. Titik di luar semua tool → rongga tertutup di antara tool
/// (bagian ruang bebas yang tidak menyentuh batas luar). Hasil semua titik
/// di-union.
pub fn boundary_fill(tools: &[&KernelShape], points: &[DVec3]) -> Result<KernelShape> {
    if tools.is_empty() {
        bail!("boundary fill butuh minimal 1 body pembatas");
    }
    if points.is_empty() {
        bail!("boundary fill butuh minimal 1 titik sel");
    }
    let bodies: Vec<&Body> = tools
        .iter()
        .map(|t| t.require_body("Boundary fill"))
        .collect::<Result<_>>()?;
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for t in tools {
        if let Some((a, b)) = t.bounding_box() {
            lo = lo.min(DVec3::from(a));
            hi = hi.max(DVec3::from(b));
        }
    }
    if !lo.is_finite() || !hi.is_finite() {
        bail!("body pembatas tidak punya bounding box");
    }
    let mut cells: Vec<Body> = Vec::new();
    for (k, &p) in points.iter().enumerate() {
        let inside: Vec<bool> = tools.iter().map(|t| point_inside(t, p)).collect();
        let cell = if inside.iter().any(|b| *b) {
            let mut acc: Option<Body> = None;
            for (b, _) in bodies.iter().zip(&inside).filter(|(_, i)| **i) {
                acc = Some(match acc {
                    None => (*b).clone(),
                    Some(a) => bool_op(&a, b, BoolOp::Intersect, "Boundary fill")?
                        .ok_or_else(|| anyhow!("titik #{k}: sel kosong"))?,
                });
            }
            let mut acc = acc.ok_or_else(|| anyhow!("titik #{k}: sel kosong"))?;
            for (b, _) in bodies.iter().zip(&inside).filter(|(_, i)| !**i) {
                if let Some(r) = bool_op(&acc, b, BoolOp::Cut, "Boundary fill")? {
                    acc = r;
                }
            }
            let parts = lumps(acc);
            if parts.len() == 1 {
                parts.into_iter().next().ok_or_else(|| anyhow!("titik #{k}: sel kosong"))?
            } else {
                parts
                    .into_iter()
                    .find(|l| point_inside(&KernelShape::from_body(l.clone()), p))
                    .ok_or_else(|| anyhow!("titik #{k}: sel tidak ditemukan"))?
            }
        } else {
            let pad = (hi - lo).length() * 0.1 + 1.0;
            let (blo, bhi) = (lo - DVec3::splat(pad), hi + DVec3::splat(pad));
            let mut free = kernel("Boundary fill", || {
                solidcraft_kernel::box_solid(sc(blo), sc(bhi))
            })?;
            for b in &bodies {
                if let Some(r) = bool_op(&free, b, BoolOp::Cut, "Boundary fill")? {
                    free = r;
                }
            }
            let mut found = None;
            for l in lumps(free) {
                let s = KernelShape::from_body(l.clone());
                let touches = s.bounding_box().is_none_or(|(a, b)| {
                    let (a, b) = (DVec3::from(a), DVec3::from(b));
                    (a - blo).min_element() < 1e-6 || (bhi - b).min_element() < 1e-6
                });
                if !touches && point_inside(&s, p) {
                    found = Some(l);
                    break;
                }
            }
            found.ok_or_else(|| {
                anyhow!("titik #{k} tidak berada di dalam body maupun rongga tertutup")
            })?
        };
        cells.push(cell);
    }
    let mut it = cells.into_iter();
    let mut acc = it.next().ok_or_else(|| anyhow!("tidak ada sel"))?;
    for c in it {
        acc = bool_op(&acc, &c, BoolOp::Union, "Boundary fill")?
            .ok_or_else(|| anyhow!("union sel gagal"))?;
    }
    checked(acc, "Boundary fill")
}

/// Box sejajar sumbu dunia yang membungkus shape, ditambah `margin` tiap sisi.
pub fn bounding_box_solid(shape: &KernelShape, margin: f64) -> Result<KernelShape> {
    if !margin.is_finite() || margin < 0.0 {
        bail!("margin bounding solid harus ≥ 0");
    }
    let (lo, hi) = shape
        .bounding_box()
        .ok_or_else(|| anyhow!("shape kosong tidak punya bounding box"))?;
    let (lo, hi) = (DVec3::from(lo) - margin, DVec3::from(hi) + margin);
    if (hi - lo).min_element() < 1e-6 {
        bail!("bounding box pipih (tebal 0); beri margin > 0");
    }
    let b = kernel("Bounding solid", || {
        solidcraft_kernel::box_solid(sc(lo), sc(hi))
    })?;
    checked(b, "Bounding solid")
}

/// Silinder sejajar `axis` yang membungkus shape (+`margin` radial & ujung).
/// Sumbu melewati pusat bbox.
pub fn bounding_cylinder_solid(
    shape: &KernelShape,
    axis: DVec3,
    margin: f64,
) -> Result<KernelShape> {
    if !margin.is_finite() || margin < 0.0 {
        bail!("margin bounding solid harus ≥ 0");
    }
    if !axis.is_finite() || axis.length() <= 1e-9 {
        bail!("sumbu bounding cylinder tidak valid");
    }
    let a = axis.normalize();
    let (lo, hi) = shape
        .bounding_box()
        .ok_or_else(|| anyhow!("shape kosong tidak punya bounding box"))?;
    let c = (DVec3::from(lo) + DVec3::from(hi)) * 0.5;
    let mesh = shape.tessellate();
    let (mut hmin, mut hmax, mut r) = (f64::INFINITY, f64::NEG_INFINITY, 0.0f64);
    for v in &mesh.positions {
        let p = DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64) - c;
        let h = p.dot(a);
        hmin = hmin.min(h);
        hmax = hmax.max(h);
        r = r.max((p - a * h).length());
    }
    if !hmin.is_finite() {
        bail!("shape kosong");
    }
    // Mesh tampilan memotong busur (chord): beri kelonggaran 0,5 %.
    let (r, h0, h1) = (r * 1.005 + margin, hmin - margin, hmax + margin);
    if r < 1e-6 || h1 - h0 < 1e-6 {
        bail!("bounding cylinder pipih; beri margin > 0");
    }
    let base = c + a * h0;
    let b = kernel("Bounding solid", || {
        solidcraft_kernel::cylinder(sc(base), sc(a), r, h1 - h0)
    })?;
    checked(b, "Bounding solid")
}

/// Torus berpusat di origin, sumbu +Z: `major` = radius pusat tabung,
/// `minor` = radius tabung (`major > minor > 0`).
pub fn make_torus(major: f64, minor: f64) -> Result<KernelShape> {
    if !(minor.is_finite() && major.is_finite() && minor > 0.0 && major > minor) {
        bail!("torus butuh radius besar > radius kecil > 0 (diberikan R={major}, r={minor})");
    }
    let b = kernel("make_torus", || {
        solidcraft_kernel::torus(Vec3::ZERO, major, minor)
    })?;
    Ok(KernelShape::from_body(b))
}

// ---------------------------------------------------------------------------
// Kontinuitas antar face (Paritas 5.3).
// ---------------------------------------------------------------------------

/// Kelas kontinuitas sambungan dua face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Continuity {
    /// Tidak bertemu (celah).
    Gap,
    /// Bersentuhan tapi bersudut (G0).
    G0,
    /// Tangen, kurvatur melompat (G1).
    G1,
    /// Kurvatur kontinu (G2).
    G2,
}

impl Continuity {
    pub fn label(self) -> &'static str {
        match self {
            Continuity::Gap => "Gap",
            Continuity::G0 => "G0",
            Continuity::G1 => "G1",
            Continuity::G2 => "G2",
        }
    }
}

/// Hasil satu sambungan.
#[derive(Debug, Clone, PartialEq)]
pub struct SeamContinuity {
    pub class: Continuity,
    /// Titik di tengah sambungan (untuk label).
    pub mid: [f64; 3],
    /// Titik sampel sepanjang sambungan.
    pub points: Vec<[f64; 3]>,
    pub gap_mm: f64,
    pub angle_deg: f64,
    pub curvature_jump: f64,
}

/// Kontinuitas setiap sambungan dua-sisi body: toleransi celah `gap_mm`,
/// sudut normal `angle_deg`, lompatan kurvatur `curvature_per_mm`.
pub fn seam_continuity(
    shape: &KernelShape,
    gap_mm: f64,
    angle_deg: f64,
    curvature_per_mm: f64,
) -> Result<Vec<SeamContinuity>> {
    let body = shape.require_body("Continuity")?;
    let edges = kernel("Continuity", || solidcraft_kernel::pick_edges(body, &[]))?;
    if edges.is_empty() {
        return Ok(Vec::new());
    }
    let tol = solidcraft_kernel::ContinuityTolerances {
        gap_mm,
        angle_deg,
        curvature_per_mm,
    };
    let report = kernel("Continuity", || {
        solidcraft_kernel::seam_diagnostics(body, &edges, 9, tol)
    })?;
    let v = |p: Vec3| [p.x, p.y, p.z];
    Ok(report
        .seams
        .into_iter()
        .map(|s| {
            let class = match s.verdict {
                solidcraft_kernel::SeamVerdict::Smooth => Continuity::G2,
                solidcraft_kernel::SeamVerdict::CurvatureJump => Continuity::G1,
                solidcraft_kernel::SeamVerdict::Sharp => Continuity::G0,
                solidcraft_kernel::SeamVerdict::Gap => Continuity::Gap,
            };
            let points: Vec<[f64; 3]> = s.points.iter().map(|p| v(*p)).collect();
            let mid = points.get(points.len() / 2).copied().unwrap_or([0.0; 3]);
            SeamContinuity {
                class,
                mid,
                points,
                gap_mm: s.gap_mm.value,
                angle_deg: s.normal_angle_deg.value,
                curvature_jump: s.curvature_jump_per_mm.value,
            }
        })
        .collect())
}

/// 6.11 Lip: naikkan pita selebar `width` di sepanjang loop dalam
/// (`outside = false`) atau luar face planar `face` setinggi `height`.
pub fn raise_band(shape: &KernelShape, face: usize, width: f64, height: f64, outside: bool) -> Result<KernelShape> {
    if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
        bail!("lebar dan tinggi lip harus > 0");
    }
    let body = shape.require_body("Lip")?;
    let pts = face_points(shape, &[face])?;
    let at = *pts.first().ok_or_else(|| anyhow!("face lip tidak ditemukan"))?;
    let b = kernel("Lip", || solidcraft_kernel::raise_band(body, at, width, height, outside))?;
    checked(b, "Lip")
}
