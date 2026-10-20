//! Geometri datum murni (PLAN_PARITAS_SOLIDCRAFT Fase 1, butir 1.2–1.4 dan
//! 1.6): sumbu, titik, dan bidang konstruksi dari masukan polos (`DVec3`,
//! [`PlaneFrame`], info topologi kernel). Tanpa sesi/oplog, sehingga GUI
//! (`ducad-app`) memakai rumus yang SAMA dengan op `construct_axis`,
//! `construct_point`, dan varian `PlaneSpec` baru.
//!
//! Semua fungsi menolak masukan non-finite (NaN/inf) dan kasus degenerat
//! (titik berimpit, bidang sejajar, …) dengan `OpError` + `hint`; tidak ada
//! yang panik.
//!
//! Fungsi:
//! - Sumbu ([`Axis`]): [`axis_two_points`], [`axis_two_planes`],
//!   [`axis_normal_to_plane`], [`Axis::new`].
//! - Titik: [`point_three_planes`], [`point_line_plane`], [`point_two_lines`],
//!   [`circle_center`], [`sphere_center`].
//! - Bidang ([`PlaneFrame`]): [`plane_midplane`], [`plane_perpendicular`],
//!   [`plane_tangent_cylinder`], [`plane_tangent_cylinder_through`],
//!   [`plane_through_lines`], [`plane_line_point`], [`plane_along_curve`].
//! - Kurva tepi ([`EdgeCurve`]): [`EdgeCurve::from_edge`],
//!   [`EdgeCurve::point_at`], [`EdgeCurve::tangent_at`],
//!   [`EdgeCurve::intersect_plane`], [`EdgeCurve::as_axis`].
//! - Revolve: [`axis_in_plane`] — sumbu 3D → garis (u, v) di bidang sketch.
//!
//! Orientasi `u`/`v` setiap bidang hasil mengikuti [`PlaneFrame::on_face`]
//! (u = +X dunia diproyeksikan, atau +Y bila bidang tegak lurus X).

use std::f64::consts::TAU;

use ducad_kernel::{EdgeInfo, EdgeKind, FaceInfo, SurfaceKind};
use glam::{DVec2, DVec3};

use crate::error::{OpError, OpErrorCode, OpResult};
use crate::plane::PlaneFrame;

/// Toleransi panjang (mm) untuk titik berimpit / "terletak di bidang".
pub const LIN_TOL: f64 = 1e-4;
/// Toleransi sejajar: panjang `a × b` dua vektor satuan (≈ sudut dalam rad).
pub const PAR_TOL: f64 = 1e-6;

/// Sumbu datum: titik + arah satuan. Larik `[f64; 3]` (bukan `DVec3`) agar
/// bentuk JSON-nya sama dengan [`PlaneFrame`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Axis {
    pub origin: [f64; 3],
    /// Satuan.
    pub dir: [f64; 3],
}

impl Axis {
    /// Sumbu dari titik + arah (dinormalisasi). Error bila arah nol atau
    /// ada komponen non-finite.
    pub fn new(origin: DVec3, dir: DVec3) -> OpResult<Self> {
        let origin = finite(origin, "titik sumbu")?;
        let dir = finite(dir, "arah sumbu")?;
        if dir.length() < 1e-12 {
            return Err(OpError::invalid("arah sumbu bernilai nol")
                .with_hint("berikan dua titik berbeda atau arah yang tidak nol"));
        }
        Ok(Self {
            origin: origin.to_array(),
            dir: dir.normalize().to_array(),
        })
    }

    pub fn origin_v(&self) -> DVec3 {
        DVec3::from_array(self.origin)
    }

    pub fn dir_v(&self) -> DVec3 {
        DVec3::from_array(self.dir)
    }

    /// Proyeksi tegak lurus `p` ke garis sumbu.
    pub fn closest_point(&self, p: DVec3) -> DVec3 {
        let (o, d) = (self.origin_v(), self.dir_v());
        o + d * (p - o).dot(d)
    }

    /// Jarak `p` ke garis sumbu.
    pub fn distance_to(&self, p: DVec3) -> f64 {
        (p - self.closest_point(p)).length()
    }
}

fn finite(v: DVec3, what: &str) -> OpResult<DVec3> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(OpError::invalid(format!("{what} tidak finite: {v:?}")))
    }
}

fn frame_ok(p: &PlaneFrame, what: &str) -> OpResult<(DVec3, DVec3)> {
    let o = finite(p.origin_v(), what)?;
    let n = finite(p.normal_v(), what)?;
    if n.length() < 1e-12 {
        return Err(OpError::invalid(format!("normal {what} bernilai nol")));
    }
    Ok((o, n.normalize()))
}

fn parallel_error(what: &str) -> OpError {
    OpError::invalid(format!("{what} sejajar sehingga tidak berpotongan"))
}

// ---------------------------------------------------------------------
// Sumbu.
// ---------------------------------------------------------------------

/// Sumbu melalui `a` menuju `b`.
pub fn axis_two_points(a: DVec3, b: DVec3) -> OpResult<Axis> {
    let (a, b) = (finite(a, "titik a")?, finite(b, "titik b")?);
    if (b - a).length() < LIN_TOL {
        return Err(OpError::invalid(format!(
            "dua titik sumbu berimpit (jarak {:.2e} mm)",
            (b - a).length()
        ))
        .with_hint("pilih dua titik yang berbeda"));
    }
    Axis::new(a, b - a)
}

/// Garis potong dua bidang. Arah = `n_a × n_b`; titik = proyeksi titik
/// tengah kedua origin ke garis tersebut.
pub fn axis_two_planes(a: &PlaneFrame, b: &PlaneFrame) -> OpResult<Axis> {
    let (oa, na) = frame_ok(a, "bidang a")?;
    let (ob, nb) = frame_ok(b, "bidang b")?;
    let d = na.cross(nb);
    if d.length() < PAR_TOL {
        return Err(parallel_error("kedua bidang")
            .with_hint("pilih dua bidang yang tidak sejajar (mis. XY dan XZ)"));
    }
    let (ha, hb) = (na.dot(oa), nb.dot(ob));
    let dd = d.length_squared();
    // Titik pada kedua bidang (rumus standar perpotongan dua bidang).
    let p0 = (nb.cross(d) * ha + d.cross(na) * hb) / dd;
    let line = Axis::new(p0, d)?;
    Axis::new(line.closest_point((oa + ob) * 0.5), d)
}

/// Sumbu tegak lurus `plane` melalui proyeksi `through` (default: origin
/// bidang) ke bidang itu.
pub fn axis_normal_to_plane(plane: &PlaneFrame, through: Option<DVec3>) -> OpResult<Axis> {
    let (o, n) = frame_ok(plane, "bidang")?;
    let p = finite(through.unwrap_or(o), "titik tembus")?;
    Axis::new(p - n * (p - o).dot(n), n)
}

// ---------------------------------------------------------------------
// Titik.
// ---------------------------------------------------------------------

/// Titik potong tiga bidang.
pub fn point_three_planes(a: &PlaneFrame, b: &PlaneFrame, c: &PlaneFrame) -> OpResult<DVec3> {
    let (oa, na) = frame_ok(a, "bidang a")?;
    let (ob, nb) = frame_ok(b, "bidang b")?;
    let (oc, nc) = frame_ok(c, "bidang c")?;
    let det = na.dot(nb.cross(nc));
    if det.abs() < PAR_TOL {
        return Err(OpError::invalid(
            "tiga bidang tidak berpotongan di satu titik (ada yang sejajar atau ketiganya berbagi garis)",
        )
        .with_hint("pilih tiga bidang yang normalnya saling bebas, mis. XY, XZ, YZ"));
    }
    let (ha, hb, hc) = (na.dot(oa), nb.dot(ob), nc.dot(oc));
    Ok((nb.cross(nc) * ha + nc.cross(na) * hb + na.cross(nb) * hc) / det)
}

/// Titik potong garis tak hingga `line` dengan `plane`.
pub fn point_line_plane(line: &Axis, plane: &PlaneFrame) -> OpResult<DVec3> {
    let (o, n) = frame_ok(plane, "bidang")?;
    let (lo, ld) = (finite(line.origin_v(), "garis")?, finite(line.dir_v(), "garis")?);
    let denom = ld.dot(n);
    if denom.abs() < PAR_TOL {
        return Err(parallel_error("garis dan bidang")
            .with_hint("pilih edge yang menembus bidang (tidak sejajar dengannya)"));
    }
    Ok(lo + ld * ((o - lo).dot(n) / denom))
}

/// Titik terdekat antara dua garis tak hingga: `(titik tengah ruas tegak
/// lurus bersama, jarak kedua garis)`. Jarak ≈ 0 berarti berpotongan.
pub fn point_two_lines(a: &Axis, b: &Axis) -> OpResult<(DVec3, f64)> {
    let (pa, da) = (finite(a.origin_v(), "garis a")?, finite(a.dir_v(), "garis a")?);
    let (pb, db) = (finite(b.origin_v(), "garis b")?, finite(b.dir_v(), "garis b")?);
    let n = da.cross(db);
    let nn = n.length_squared();
    if n.length() < PAR_TOL {
        return Err(parallel_error("kedua garis")
            .with_hint("pilih dua edge yang tidak sejajar"));
    }
    let w = pb - pa;
    let ta = w.cross(db).dot(n) / nn;
    let tb = w.cross(da).dot(n) / nn;
    let (ca, cb) = (pa + da * ta, pb + db * tb);
    Ok(((ca + cb) * 0.5, (ca - cb).length()))
}

/// Pusat lingkaran dari titik awal, tengah (setengah panjang busur), dan
/// akhir sebuah busur. Lingkaran penuh (`start == end`): titik tengah
/// berseberangan dengan titik awal, pusat = rata-ratanya. `None` bila
/// ketiga titik segaris/berimpit.
pub fn circle_center(start: DVec3, mid: DVec3, end: DVec3) -> Option<DVec3> {
    if !(start.is_finite() && mid.is_finite() && end.is_finite()) {
        return None;
    }
    if (start - mid).length() < 1e-9 {
        return None;
    }
    if (start - end).length() < 1e-9 {
        return Some((start + mid) * 0.5);
    }
    let u = mid - start;
    let w = end - start;
    let n = u.cross(w);
    let nn = n.length_squared();
    if nn < 1e-18 {
        return None;
    }
    Some(start + (w.cross(n) * u.length_squared() + n.cross(u) * w.length_squared()) / (2.0 * nn))
}

/// Pusat bola beradius `radius` dari titik-titik permukaannya (`samples`,
/// mis. titik batas + titik edge face) dan centroid permukaan face.
/// - Sampel tak sebidang → kuadrat terkecil linear (tanpa memakai radius).
/// - Sampel sebidang (satu lingkaran batas) → pusat lingkaran ± `h·n`,
///   `h = √(r² − ρ²)`; sisi dipilih dengan aturan Archimedes: centroid
///   permukaan tudung ada di setengah tinggi tudung.
/// - Tanpa sampel (bola penuh) → centroid.
pub fn sphere_center(samples: &[DVec3], radius: f64, face_centroid: DVec3) -> OpResult<DVec3> {
    let fc = finite(face_centroid, "centroid face")?;
    if !(radius.is_finite() && radius > 0.0) {
        return Err(OpError::invalid("radius bola tidak valid"));
    }
    let pts: Vec<DVec3> = samples.iter().copied().filter(|p| p.is_finite()).collect();
    // Tiga titik tak segaris pertama.
    let Some(p0) = pts.first().copied() else {
        return Ok(fc);
    };
    let Some(p1) = pts.iter().copied().find(|p| (*p - p0).length() > LIN_TOL) else {
        return Ok(fc);
    };
    let Some(p2) = pts
        .iter()
        .copied()
        .find(|p| (p1 - p0).cross(*p - p0).length() > LIN_TOL * radius)
    else {
        return Ok(fc);
    };
    let n = (p1 - p0).cross(p2 - p0).normalize();
    let off_plane = pts.iter().map(|p| (*p - p0).dot(n).abs()).fold(0.0, f64::max);
    if off_plane > LIN_TOL {
        // Kuadrat terkecil: 2(p_i − p_0)·c = |p_i|² − |p_0|².
        let mut ata = glam::DMat3::ZERO;
        let mut atb = DVec3::ZERO;
        for p in &pts[1..] {
            let row = (*p - p0) * 2.0;
            let rhs = p.length_squared() - p0.length_squared();
            ata += glam::DMat3::from_cols(row * row.x, row * row.y, row * row.z);
            atb += row * rhs;
        }
        if ata.determinant().abs() > 1e-12 {
            let c = ata.inverse() * atb;
            if c.is_finite() {
                return Ok(c);
            }
        }
    }
    let c0 = circle_center(p0, p1, p2)
        .ok_or_else(|| OpError::invalid("titik batas bola degenerat"))?;
    let rho = (p0 - c0).length();
    let h = (radius * radius - rho * rho).max(0.0).sqrt();
    if h < LIN_TOL {
        return Ok(c0);
    }
    // Sisi centroid terhadap bidang lingkaran (s = +1 searah n).
    let dc = (fc - c0).dot(n);
    let s = if dc >= 0.0 { 1.0 } else { -1.0 };
    // Pusat di sisi centroid → tudung besar, centroid di (r + h)/2; di sisi
    // lain → tudung kecil, centroid di (r − h)/2.
    let big = ((radius + h) / 2.0 - dc.abs()).abs();
    let small = ((radius - h) / 2.0 - dc.abs()).abs();
    Ok(if big <= small { c0 + n * (s * h) } else { c0 - n * (s * h) })
}

// ---------------------------------------------------------------------
// Bidang.
// ---------------------------------------------------------------------

/// Bidang tengah di antara dua bidang sejajar (searah atau berlawanan).
/// Normal = normal `a`; origin = titik tengah kedua origin.
pub fn plane_midplane(a: &PlaneFrame, b: &PlaneFrame) -> OpResult<PlaneFrame> {
    let (oa, na) = frame_ok(a, "bidang a")?;
    let (ob, nb) = frame_ok(b, "bidang b")?;
    if na.cross(nb).length() > PAR_TOL * 100.0 {
        return Err(OpError::invalid(format!(
            "bidang tengah butuh dua bidang sejajar (sudut antar normal {:.3}°)",
            na.angle_between(nb).to_degrees()
        ))
        .with_hint("pilih dua face datar yang saling sejajar, mis. \">Z\" dan \"<Z\""));
    }
    Ok(PlaneFrame::on_face(((oa + ob) * 0.5).to_array(), na.to_array()))
}

/// Bidang tegak lurus arah `dir` (mis. tangen edge) melalui `through`.
pub fn plane_perpendicular(dir: DVec3, through: DVec3) -> OpResult<PlaneFrame> {
    let d = finite(dir, "arah")?;
    let p = finite(through, "titik")?;
    if d.length() < 1e-12 {
        return Err(OpError::invalid("arah tegak lurus bernilai nol"));
    }
    Ok(PlaneFrame::on_face(p.to_array(), d.normalize().to_array()))
}

/// Arah radial acuan sudut 0 untuk sumbu `d`: +X dunia diproyeksikan tegak
/// lurus sumbu (+Y bila sumbu sejajar X) — konvensi yang sama dengan
/// `PlaneFrame::on_face`.
pub fn reference_radial(d: DVec3) -> DVec3 {
    let d = d.normalize_or_zero();
    let project = |w: DVec3| w - d * w.dot(d);
    let mut r = project(DVec3::X);
    if r.length() < 1e-6 {
        r = project(DVec3::Y);
    }
    r.normalize_or_zero()
}

/// Bidang singgung silinder (sumbu `axis`, radius `radius`) pada sudut
/// `angle_deg` di sekitar sumbu, diukur CCW (kaidah tangan kanan terhadap
/// `axis.dir`) dari [`reference_radial`]. Normal bidang = arah radial.
pub fn plane_tangent_cylinder(axis: &Axis, radius: f64, angle_deg: f64) -> OpResult<PlaneFrame> {
    if !(radius.is_finite() && radius > 0.0) {
        return Err(OpError::invalid(format!("radius silinder harus > 0 (diberikan {radius})")));
    }
    if !angle_deg.is_finite() {
        return Err(OpError::invalid("sudut singgung tidak finite"));
    }
    let (o, d) = (finite(axis.origin_v(), "sumbu")?, finite(axis.dir_v(), "sumbu")?);
    let r0 = reference_radial(d);
    let t = angle_deg.to_radians();
    let r = r0 * t.cos() + d.cross(r0) * t.sin();
    Ok(PlaneFrame::on_face((o + r * radius).to_array(), r.to_array()))
}

/// Bidang singgung silinder di titik permukaan yang paling dekat dengan
/// `through` (arah radial `through`). Error bila `through` di sumbu.
pub fn plane_tangent_cylinder_through(axis: &Axis, radius: f64, through: DVec3) -> OpResult<PlaneFrame> {
    let p = finite(through, "titik singgung")?;
    if !(radius.is_finite() && radius > 0.0) {
        return Err(OpError::invalid(format!("radius silinder harus > 0 (diberikan {radius})")));
    }
    let foot = axis.closest_point(p);
    let r = p - foot;
    if r.length() < LIN_TOL {
        return Err(OpError::invalid("titik singgung terletak di sumbu silinder")
            .with_hint("berikan titik di luar sumbu atau pakai angle_deg"));
    }
    let r = r.normalize();
    Ok(PlaneFrame::on_face((foot + r * radius).to_array(), r.to_array()))
}

/// Bidang melalui dua garis sebidang (`a0→a1`, `b0→b1`), berpotongan atau
/// sejajar. Origin = rata-rata keempat titik.
pub fn plane_through_lines(a0: DVec3, a1: DVec3, b0: DVec3, b1: DVec3) -> OpResult<PlaneFrame> {
    let (a0, a1) = (finite(a0, "edge a")?, finite(a1, "edge a")?);
    let (b0, b1) = (finite(b0, "edge b")?, finite(b1, "edge b")?);
    let (da, db) = ((a1 - a0).normalize_or_zero(), (b1 - b0).normalize_or_zero());
    if da == DVec3::ZERO || db == DVec3::ZERO {
        return Err(OpError::invalid("edge berpanjang nol"));
    }
    let mut n = da.cross(db);
    if n.length() < PAR_TOL {
        // Sejajar: normal dari arah dan vektor penghubung.
        n = da.cross(b0 - a0);
        if n.length() < LIN_TOL {
            return Err(OpError::invalid("kedua edge segaris sehingga bidangnya tidak tunggal")
                .with_hint("pilih edge kedua yang tidak segaris, atau pakai edge + titik"));
        }
    }
    let n = n.normalize();
    let off = (b0 - a0).dot(n);
    if off.abs() > LIN_TOL {
        return Err(OpError::invalid(format!(
            "kedua edge tidak sebidang (bersilangan, jarak {off:.4} mm)"
        ))
        .with_hint("pilih dua edge yang terletak pada satu bidang"));
    }
    let o = (a0 + a1 + b0 + b1) * 0.25;
    Ok(PlaneFrame::on_face(o.to_array(), n.to_array()))
}

/// Bidang melalui garis `a0→a1` dan titik `p`.
pub fn plane_line_point(a0: DVec3, a1: DVec3, p: DVec3) -> OpResult<PlaneFrame> {
    let (a0, a1, p) = (finite(a0, "edge")?, finite(a1, "edge")?, finite(p, "titik")?);
    let d = (a1 - a0).normalize_or_zero();
    if d == DVec3::ZERO {
        return Err(OpError::invalid("edge berpanjang nol"));
    }
    let n = d.cross(p - a0);
    if n.length() < LIN_TOL {
        return Err(OpError::invalid("titik terletak pada garis edge sehingga bidangnya tidak tunggal")
            .with_hint("pilih titik di luar garis edge"));
    }
    let o = (a0 + a1 + p) / 3.0;
    Ok(PlaneFrame::on_face(o.to_array(), n.normalize().to_array()))
}

/// Bidang normal terhadap kurva pada pecahan panjang `fraction` (0..=1).
pub fn plane_along_curve(curve: &EdgeCurve, fraction: f64) -> OpResult<PlaneFrame> {
    let t = check_fraction(fraction)?;
    plane_perpendicular(curve.tangent_at(t), curve.point_at(t))
}

fn check_fraction(f: f64) -> OpResult<f64> {
    if !(f.is_finite() && (0.0..=1.0).contains(&f)) {
        return Err(OpError::invalid(format!(
            "fraction harus di 0..1 (diberikan {f})"
        ))
        .with_hint("0 = awal edge, 0.5 = tengah, 1 = akhir"));
    }
    Ok(f)
}

// ---------------------------------------------------------------------
// Kurva tepi.
// ---------------------------------------------------------------------

/// Geometri eksak satu edge B-rep yang didukung datum: ruas garis atau
/// busur/lingkaran. Dibangun dari `EdgeInfo` (start/mid/end) karena kernel
/// tidak mengekspos titik sampel edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EdgeCurve {
    Line {
        start: DVec3,
        end: DVec3,
    },
    /// Titik pada parameter `t ∈ [0, 1]`: `center + radius·(cos θ·start_dir
    /// + sin θ·(normal × start_dir))`, `θ = t·sweep`.
    Arc {
        center: DVec3,
        /// Satuan.
        normal: DVec3,
        radius: f64,
        /// Satuan, dari pusat ke titik awal.
        start_dir: DVec3,
        /// Rad, (0, 2π].
        sweep: f64,
    },
}

impl EdgeCurve {
    /// Bangun dari info edge kernel. `faces` (hasil `enumerate_faces` shape
    /// yang sama) dipakai untuk normal lingkaran penuh, yang tidak bisa
    /// diturunkan dari titik start/mid/end saja. Edge jenis `other`
    /// (spline, dll.) → `Unsupported`.
    pub fn from_edge(edge: &EdgeInfo, faces: &[FaceInfo]) -> OpResult<Self> {
        let s = finite(DVec3::from_array(edge.start), "edge")?;
        let m = finite(DVec3::from_array(edge.mid), "edge")?;
        let e = finite(DVec3::from_array(edge.end), "edge")?;
        match edge.kind {
            EdgeKind::Line => {
                if (e - s).length() < 1e-9 {
                    return Err(OpError::invalid("edge garis berpanjang nol"));
                }
                Ok(EdgeCurve::Line { start: s, end: e })
            }
            EdgeKind::Circle => {
                let c = circle_center(s, m, e)
                    .ok_or_else(|| OpError::invalid("edge lingkaran degenerat (titiknya segaris)"))?;
                let r = (s - c).length();
                let (us, um) = ((s - c) / r, (m - c) / r);
                let phi = us.cross(um).length().atan2(us.dot(um));
                let mut normal = us.cross(um);
                if normal.length() < 1e-6 {
                    // Lingkaran penuh (titik tengah berseberangan): normal
                    // dari face tetangga (bidang → normalnya, silinder/
                    // kerucut → sumbunya).
                    normal = faces
                        .iter()
                        .filter(|f| f.edge_indices.contains(&edge.index))
                        .find_map(|f| match f.kind {
                            SurfaceKind::Plane => Some(DVec3::from_array(f.normal)),
                            SurfaceKind::Cylinder | SurfaceKind::Cone => {
                                f.axis.map(|(_, d)| DVec3::from_array(d))
                            }
                            _ => None,
                        })
                        .ok_or_else(|| {
                            OpError::new(
                                OpErrorCode::Unsupported,
                                "normal lingkaran penuh tidak bisa ditentukan (tidak ada face datar/silinder di sampingnya)",
                            )
                        })?;
                }
                let normal = normal.normalize_or_zero();
                if !(r.is_finite() && r > 1e-9) || normal == DVec3::ZERO {
                    return Err(OpError::invalid("edge lingkaran degenerat"));
                }
                // Lingkaran penuh: phi = π → sweep 2π.
                let sweep = if (s - e).length() < 1e-9 { TAU } else { (2.0 * phi).min(TAU) };
                Ok(EdgeCurve::Arc {
                    center: c,
                    normal,
                    radius: r,
                    start_dir: us,
                    sweep,
                })
            }
            EdgeKind::Other => Err(OpError::new(
                OpErrorCode::Unsupported,
                format!("edge #{} berjenis kurva bebas; datum hanya mendukung edge garis/lingkaran", edge.index),
            )
            .with_hint("pilih edge berjenis line atau circle (filter [kind=line] / [kind=circle])")),
        }
    }

    /// Titik pada pecahan panjang `t` (0..=1, dijepit).
    pub fn point_at(&self, t: f64) -> DVec3 {
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        match *self {
            EdgeCurve::Line { start, end } => start + (end - start) * t,
            EdgeCurve::Arc {
                center,
                normal,
                radius,
                start_dir,
                sweep,
            } => {
                let th = t * sweep;
                center + (start_dir * th.cos() + normal.cross(start_dir) * th.sin()) * radius
            }
        }
    }

    /// Tangen satuan pada pecahan `t` (searah pertambahan parameter).
    pub fn tangent_at(&self, t: f64) -> DVec3 {
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        match *self {
            EdgeCurve::Line { start, end } => (end - start).normalize_or_zero(),
            EdgeCurve::Arc {
                normal,
                start_dir,
                sweep,
                ..
            } => {
                let th = t * sweep;
                (normal.cross(start_dir) * th.cos() - start_dir * th.sin()).normalize_or_zero()
            }
        }
    }

    /// Garis pembawa ruas garis; busur → sumbu lingkarannya.
    pub fn as_axis(&self) -> OpResult<Axis> {
        match *self {
            EdgeCurve::Line { start, end } => axis_two_points(start, end),
            EdgeCurve::Arc { center, normal, .. } => Axis::new(center, normal),
        }
    }

    /// Titik potong dengan `plane`. Garis: diperpanjang tak hingga. Busur:
    /// hanya titik yang benar-benar di busur; bila dua, yang paling dekat
    /// ke awal busur.
    pub fn intersect_plane(&self, plane: &PlaneFrame) -> OpResult<DVec3> {
        match *self {
            EdgeCurve::Line { start, end } => point_line_plane(&axis_two_points(start, end)?, plane),
            EdgeCurve::Arc {
                center,
                normal,
                radius,
                start_dir,
                sweep,
            } => {
                let (o, n) = frame_ok(plane, "bidang")?;
                let v = normal.cross(start_dir);
                let a = radius * n.dot(start_dir);
                let b = radius * n.dot(v);
                let c = n.dot(o - center);
                let rr = (a * a + b * b).sqrt();
                if rr < LIN_TOL {
                    return Err(parallel_error("busur dan bidang"));
                }
                if c.abs() > rr + LIN_TOL {
                    return Err(OpError::invalid("bidang tidak memotong lingkaran edge")
                        .with_hint("pilih bidang yang memotong edge"));
                }
                let base = b.atan2(a);
                let delta = (c / rr).clamp(-1.0, 1.0).acos();
                let mut best: Option<f64> = None;
                for th in [base - delta, base + delta] {
                    let mut th = th.rem_euclid(TAU);
                    if th > TAU - 1e-9 {
                        th = 0.0;
                    }
                    if th <= sweep + 1e-9 && best.is_none_or(|b| th < b) {
                        best = Some(th);
                    }
                }
                let th = best.ok_or_else(|| {
                    OpError::invalid("bidang memotong lingkaran di luar rentang busur edge")
                        .with_hint("pilih bidang yang memotong bagian busur itu sendiri")
                })?;
                Ok(self.point_at(th / sweep))
            }
        }
    }
}

// ---------------------------------------------------------------------
// Revolve.
// ---------------------------------------------------------------------

/// Sumbu 3D → `(origin, dir)` dalam koordinat (u, v) `frame`. Sumbu harus
/// TERLETAK di bidang sketch: arahnya tegak lurus normal dan jaraknya ke
/// bidang ≤ [`LIN_TOL`].
pub fn axis_in_plane(axis: &Axis, frame: &PlaneFrame) -> OpResult<(DVec2, DVec2)> {
    let (fo, n) = frame_ok(frame, "bidang sketch")?;
    let (o, d) = (finite(axis.origin_v(), "sumbu")?, finite(axis.dir_v(), "sumbu")?);
    let tilt = d.dot(n);
    if tilt.abs() > PAR_TOL * 100.0 {
        return Err(OpError::invalid(format!(
            "sumbu datum tidak sebidang dengan bidang sketch (miring {:.3}° terhadap bidang)",
            tilt.clamp(-1.0, 1.0).asin().to_degrees().abs()
        ))
        .with_hint("sumbu revolve harus terletak di bidang sketch: pilih/buat sumbu datum yang sejajar bidang dan melalui bidang itu, atau pakai \"u\"/\"v\"/{\"origin\":[u,v],\"dir\":[du,dv]}"));
    }
    let dist = (o - fo).dot(n);
    if dist.abs() > LIN_TOL {
        return Err(OpError::invalid(format!(
            "sumbu datum sejajar bidang sketch tetapi berjarak {dist:.4} mm darinya"
        ))
        .with_hint("buat sketch pada bidang yang memuat sumbu (mis. plane {\"through_edges\":[...]}), atau pakai sumbu 2D {\"origin\":[u,v],\"dir\":[du,dv]}"));
    }
    let o2 = frame.to_local(o);
    let d2 = DVec2::new(d.dot(frame.u_v()), d.dot(frame.v_v()));
    if d2.length() < 1e-9 {
        return Err(OpError::invalid("arah sumbu datum nol di bidang sketch"));
    }
    Ok((o2, d2.normalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: DVec3, b: DVec3) -> bool {
        (a - b).length() < 1e-9
    }

    #[test]
    fn two_points_and_degenerate() {
        let a = axis_two_points(DVec3::ZERO, DVec3::new(0.0, 0.0, 5.0)).unwrap();
        assert!(close(a.dir_v(), DVec3::Z));
        assert!(axis_two_points(DVec3::ONE, DVec3::ONE).is_err());
        assert!(axis_two_points(DVec3::NAN, DVec3::ONE).is_err());
    }

    #[test]
    fn two_planes_is_shared_line() {
        let ax = axis_two_planes(&PlaneFrame::top(), &PlaneFrame::right()).unwrap();
        // XY ∩ YZ = sumbu Y.
        assert!(ax.dir_v().cross(DVec3::Y).length() < 1e-12);
        assert!(ax.distance_to(DVec3::ZERO) < 1e-12);
        assert!(axis_two_planes(&PlaneFrame::top(), &PlaneFrame::top()).is_err());
        let mut off = PlaneFrame::top();
        off.origin = [0.0, 0.0, 7.0];
        let ax = axis_two_planes(&off, &PlaneFrame::front()).unwrap();
        assert!((ax.origin[2] - 7.0).abs() < 1e-12 && ax.origin[1].abs() < 1e-12);
    }

    #[test]
    fn normal_axis_projects_point() {
        let ax = axis_normal_to_plane(&PlaneFrame::top(), Some(DVec3::new(3.0, 4.0, 9.0))).unwrap();
        assert!(close(ax.origin_v(), DVec3::new(3.0, 4.0, 0.0)));
        assert!(close(ax.dir_v(), DVec3::Z));
    }

    #[test]
    fn three_planes_and_line_plane() {
        let mut a = PlaneFrame::top();
        a.origin = [0.0, 0.0, 2.0];
        let mut b = PlaneFrame::front();
        b.origin = [0.0, 3.0, 0.0];
        let mut c = PlaneFrame::right();
        c.origin = [5.0, 0.0, 0.0];
        assert!(close(point_three_planes(&a, &b, &c).unwrap(), DVec3::new(5.0, 3.0, 2.0)));
        assert!(point_three_planes(&a, &a, &c).is_err());
        let line = axis_two_points(DVec3::new(1.0, 1.0, -4.0), DVec3::new(1.0, 1.0, 4.0)).unwrap();
        assert!(close(point_line_plane(&line, &a).unwrap(), DVec3::new(1.0, 1.0, 2.0)));
        let flat = axis_two_points(DVec3::ZERO, DVec3::X).unwrap();
        assert!(point_line_plane(&flat, &PlaneFrame::top()).is_err());
    }

    #[test]
    fn two_lines_closest_point() {
        let a = axis_two_points(DVec3::ZERO, DVec3::X).unwrap();
        let b = axis_two_points(DVec3::new(2.0, -1.0, 4.0), DVec3::new(2.0, 1.0, 4.0)).unwrap();
        let (p, d) = point_two_lines(&a, &b).unwrap();
        assert!(close(p, DVec3::new(2.0, 0.0, 2.0)));
        assert!((d - 4.0).abs() < 1e-12);
        assert!(point_two_lines(&a, &a).is_err());
    }

    #[test]
    fn circle_center_arc_and_full() {
        let c = circle_center(DVec3::new(5.0, 0.0, 1.0), DVec3::new(0.0, 5.0, 1.0), DVec3::new(-5.0, 0.0, 1.0)).unwrap();
        assert!(close(c, DVec3::new(0.0, 0.0, 1.0)));
        let full = circle_center(DVec3::new(5.0, 0.0, 0.0), DVec3::new(-5.0, 0.0, 0.0), DVec3::new(5.0, 0.0, 0.0)).unwrap();
        assert!(close(full, DVec3::ZERO));
        assert!(circle_center(DVec3::ZERO, DVec3::X, DVec3::X * 2.0).is_none());
    }

    #[test]
    fn midplane_requires_parallel() {
        let mut a = PlaneFrame::top();
        a.origin = [0.0, 0.0, 10.0];
        let mut b = PlaneFrame::top();
        b.normal = [0.0, 0.0, -1.0];
        let m = plane_midplane(&a, &b).unwrap();
        assert!((m.origin[2] - 5.0).abs() < 1e-12);
        assert!(plane_midplane(&a, &PlaneFrame::front()).is_err());
    }

    #[test]
    fn tangent_planes() {
        let ax = axis_two_points(DVec3::ZERO, DVec3::Z).unwrap();
        let p = plane_tangent_cylinder(&ax, 5.0, 90.0).unwrap();
        assert!(close(p.origin_v(), DVec3::new(0.0, 5.0, 0.0)));
        assert!(close(p.normal_v(), DVec3::Y));
        let q = plane_tangent_cylinder_through(&ax, 5.0, DVec3::new(-10.0, 0.0, 3.0)).unwrap();
        assert!(close(q.origin_v(), DVec3::new(-5.0, 0.0, 3.0)));
        assert!(plane_tangent_cylinder_through(&ax, 5.0, DVec3::new(0.0, 0.0, 3.0)).is_err());
        assert!(plane_tangent_cylinder(&ax, f64::NAN, 0.0).is_err());
    }

    #[test]
    fn planes_through_lines_and_point() {
        let p = plane_through_lines(DVec3::ZERO, DVec3::X, DVec3::ZERO, DVec3::Y).unwrap();
        assert!(p.normal_v().cross(DVec3::Z).length() < 1e-12);
        // Sejajar.
        let p = plane_through_lines(DVec3::ZERO, DVec3::X, DVec3::Z, DVec3::Z + DVec3::X).unwrap();
        assert!(p.normal_v().cross(DVec3::Y).length() < 1e-12);
        // Bersilangan.
        assert!(plane_through_lines(DVec3::ZERO, DVec3::X, DVec3::Z, DVec3::Z + DVec3::Y).is_err());
        // Segaris.
        assert!(plane_through_lines(DVec3::ZERO, DVec3::X, DVec3::X * 2.0, DVec3::X * 3.0).is_err());
        let q = plane_line_point(DVec3::ZERO, DVec3::X, DVec3::new(0.0, 0.0, 4.0)).unwrap();
        assert!(q.normal_v().cross(DVec3::Y).length() < 1e-12);
        assert!(plane_line_point(DVec3::ZERO, DVec3::X, DVec3::X * 5.0).is_err());
    }

    fn arc_edge() -> EdgeCurve {
        let info = EdgeInfo {
            index: 0,
            kind: EdgeKind::Circle,
            start: [5.0, 0.0, 0.0],
            end: [-5.0, 0.0, 0.0],
            mid: [0.0, 5.0, 0.0],
            length: 5.0 * std::f64::consts::PI,
            dir: None,
            radius: Some(5.0),
        };
        EdgeCurve::from_edge(&info, &[]).unwrap()
    }

    #[test]
    fn arc_curve_param_and_tangent() {
        let c = arc_edge();
        assert!(close(c.point_at(0.5), DVec3::new(0.0, 5.0, 0.0)));
        assert!(close(c.point_at(1.0), DVec3::new(-5.0, 0.0, 0.0)));
        assert!(close(c.tangent_at(0.0), DVec3::Y));
        let pl = plane_along_curve(&c, 0.5).unwrap();
        assert!(pl.normal_v().cross(DVec3::X).length() < 1e-9);
        assert!(plane_along_curve(&c, 1.5).is_err());
        assert!(plane_along_curve(&c, f64::NAN).is_err());
        // Bidang YZ memotong busur di (0, 5, 0).
        let p = c.intersect_plane(&PlaneFrame::right()).unwrap();
        assert!(close(p, DVec3::new(0.0, 5.0, 0.0)));
        // Bidang XZ hanya menyentuh ujung busur (θ = 0 dan π).
        let p = c.intersect_plane(&PlaneFrame::front()).unwrap();
        assert!(close(p, DVec3::new(5.0, 0.0, 0.0)));
    }

    #[test]
    fn full_circle_needs_face_normal() {
        let info = EdgeInfo {
            index: 3,
            kind: EdgeKind::Circle,
            start: [5.0, 0.0, 2.0],
            end: [5.0, 0.0, 2.0],
            mid: [-5.0, 0.0, 2.0],
            length: TAU * 5.0,
            dir: None,
            radius: Some(5.0),
        };
        assert!(EdgeCurve::from_edge(&info, &[]).is_err());
        let face = FaceInfo {
            index: 0,
            kind: SurfaceKind::Plane,
            centroid: [0.0, 0.0, 2.0],
            normal: [0.0, 0.0, 1.0],
            area: 1.0,
            radius: None,
            axis: None,
            bbox: ([0.0; 3], [0.0; 3]),
            edge_indices: vec![3],
            concave: None,
            boundary: Vec::new(),
        };
        let c = EdgeCurve::from_edge(&info, &[face]).unwrap();
        match c {
            EdgeCurve::Arc { center, sweep, .. } => {
                assert!(close(center, DVec3::new(0.0, 0.0, 2.0)));
                assert!((sweep - TAU).abs() < 1e-12);
            }
            _ => panic!("harus busur"),
        }
        assert!(close(c.as_axis().unwrap().dir_v().abs(), DVec3::Z));
    }

    #[test]
    fn axis_in_sketch_plane() {
        let ax = axis_two_points(DVec3::new(10.0, 0.0, 0.0), DVec3::new(10.0, 0.0, 5.0)).unwrap();
        let (o, d) = axis_in_plane(&ax, &PlaneFrame::front()).unwrap();
        assert!((o - DVec2::new(10.0, 0.0)).length() < 1e-12);
        assert!((d - DVec2::Y).length() < 1e-12);
        // Tegak lurus bidang XY → ditolak.
        assert!(axis_in_plane(&ax, &PlaneFrame::top()).is_err());
        // Sejajar tapi bergeser dari bidang XZ.
        let off = axis_two_points(DVec3::new(0.0, 3.0, 0.0), DVec3::new(0.0, 3.0, 5.0)).unwrap();
        let e = axis_in_plane(&off, &PlaneFrame::front()).unwrap_err();
        assert!(e.hint.is_some());
    }

    #[test]
    fn sphere_center_from_samples() {
        let c = DVec3::new(1.0, 2.0, 3.0);
        let r = 4.0;
        // Sampel tak sebidang.
        let pts = [c + DVec3::X * r, c + DVec3::Y * r, c - DVec3::Z * r, c - DVec3::X * r];
        assert!(close(sphere_center(&pts, r, c).unwrap(), c));
        // Lingkaran kecil di z = c.z + 2 (ρ = √12): tudung kecil di atas
        // (centroid z = c.z + 3) vs tudung besar di bawah (centroid z = c.z − 1).
        let rho = 12f64.sqrt();
        let ring: Vec<DVec3> = (0..6)
            .map(|k| {
                let t = k as f64;
                c + DVec3::new(rho * t.cos(), rho * t.sin(), 2.0)
            })
            .collect();
        assert!(close(sphere_center(&ring, r, c + DVec3::Z * 3.0).unwrap(), c));
        assert!(close(sphere_center(&ring, r, c - DVec3::Z).unwrap(), c));
        // Tanpa sampel → centroid.
        assert!(close(sphere_center(&[], r, c).unwrap(), c));
        assert!(sphere_center(&pts, f64::NAN, c).is_err());
    }
}
