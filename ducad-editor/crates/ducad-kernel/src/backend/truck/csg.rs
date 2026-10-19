//! Bagian backend truck dari `crate::csg` (PLAN_MIGRASI_TRUCK 2.5): extrude,
//! revolve, loft, sweep, dan boolean lewat SolidCraft.

use anyhow::{anyhow, bail, Context, Result};
use solidcraft_geom::{Plane, Vec2, Vec3};
use solidcraft_kernel::{Body, BoolOp};

use super::bridge::{loop2, path, region, sketch_plane, v3};
use super::{kernel, unsupported};
use crate::csg::EmptyIntersection;
use crate::profile::{PathSegment, Profile};
use crate::shape::{clone_shape, KernelShape};

/// Satu body dari hasil builder SolidCraft (satu per region); beberapa
/// potongan terpisah digabung tanpa boolean.
pub(crate) fn one_body(mut bodies: Vec<Body>, what: &str) -> Result<KernelShape> {
    if bodies.len() <= 1 {
        return bodies
            .pop()
            .map(KernelShape::from_body)
            .ok_or_else(|| anyhow!("{what}: tidak menghasilkan solid"));
    }
    kernel(what, || solidcraft_kernel::join_apart(&bodies))?
        .map(KernelShape::from_body)
        .ok_or_else(|| anyhow!("{what}: potongan hasil tidak bisa digabung"))
}

/// Extrude profil pada bidang 3D sembarang (origin, u_axis, v_axis, normal)
/// sepanjang `distance` mm searah `normal`.
///
/// SolidCraft hanya meng-extrude tegak lurus bidang sketsa; `normal` yang
/// miring terhadap bidang `u×v` ditolak (`Unsupported`), sedangkan `normal`
/// yang berlawanan arah `u×v` membalik arah extrude seperti di OCCT.
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
    let (plane, scale) = sketch_plane(origin, u_axis, v_axis)?;
    let region = region(profile, scale)?;
    let n = v3(normal);
    let dir = if n.len() > 1e-6 {
        n * (1.0 / n.len())
    } else {
        Vec3::Z
    };
    let along = dir.dot(plane.normal());
    if (along.abs() - 1.0).abs() > 1e-6 {
        return unsupported("extrude miring (arah tidak tegak lurus bidang sketsa)");
    }
    let d = distance * along.signum();
    let (start, end) = if d >= 0.0 { (0.0, d) } else { (d, 0.0) };
    let bodies = kernel("Extrude", || {
        solidcraft_kernel::extrude(&plane, &[region], start, end)
    })?;
    one_body(bodies, "Extrude")
}

/// Revolve profil di bidang XY mengelilingi sumbu 2D di bidang yang sama.
/// `angle_degrees: None` = revolve penuh 360°.
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
    let region = region(profile, 1.0)?;
    let origin = Vec2::new(axis_origin.0, axis_origin.1);
    let dir = Vec2::new(axis_dir.0, axis_dir.1);
    // OCCT memutar tangan-kanan terhadap `axis_dir` apa adanya. SolidCraft
    // membalik tanda sudut parsial bila `axis × radial` berlawanan dengan
    // normal sketsa (radial = dari sumbu ke centroid profil) — kompensasi di
    // sini dengan aturan yang sama supaya hasil kedua backend berimpit.
    let angle = match angle_degrees {
        None => std::f64::consts::TAU,
        Some(deg) => {
            let axis = Vec3::new(dir.x, dir.y, 0.0) * (1.0 / dir_len);
            let rel = region.centroid() - origin;
            let rel = Vec3::new(rel.x, rel.y, 0.0);
            let radial = rel - axis * rel.dot(axis);
            let toward = axis.cross(radial).dot(Vec3::Z);
            if toward < 0.0 {
                -deg.to_radians()
            } else {
                deg.to_radians()
            }
        }
    };
    let bodies = kernel("Revolve", || {
        solidcraft_kernel::revolve_whole(&Plane::XY, &[region], origin, dir, angle)
    })
    .map_err(|e| {
        anyhow!(
            "Operasi Revolve gagal: pastikan sumbu putar tidak memotong bagian dalam profil dan profil membentuk bidang tertutup ({e:#})"
        )
    })?;
    one_body(bodies, "Revolve")
}

/// Loft antara 2 profil: `bottom` di `z = 0`, `top` di `z = height`.
pub fn loft_profiles(bottom: &Profile, top: &Profile, height: f64) -> Result<KernelShape> {
    if height.abs() < 1e-9 {
        bail!("tinggi loft harus tidak nol");
    }
    let sections = [
        (Plane::XY, loop2(bottom, 1.0)?),
        (Plane::XY.offset(height), loop2(top, 1.0)?),
    ];
    kernel("Loft", || solidcraft_kernel::loft(&sections)).map(KernelShape::from_body)
}

fn sweep(
    plane: &Plane,
    profile: &Profile,
    scale: f64,
    spine: &[PathSegment],
) -> Result<KernelShape> {
    let path = path(spine)?;
    let region = region(profile, scale)?;
    kernel("Sweep", || solidcraft_kernel::sweep(plane, &region, &path))
        .map(KernelShape::from_body)
        .map_err(|e| {
            anyhow!(
                "Operasi 3D Sweep gagal: pastikan profil tertutup dan jalur kurva valid ({e:#})"
            )
        })
}

/// Sapu profil 2D tertutup (bidang XY) di sepanjang jalur 3D.
pub fn sweep_profile_along_path(
    profile: &Profile,
    spine_segments: &[PathSegment],
) -> Result<KernelShape> {
    sweep(&Plane::XY, profile, 1.0, spine_segments)
}

/// Sapu profil 2D tertutup pada bidang (workplane) di sepanjang jalur 3D.
pub fn sweep_profile_on_plane_along_path(
    profile: &Profile,
    origin: [f64; 3],
    u_axis: [f64; 3],
    v_axis: [f64; 3],
    _normal: [f64; 3],
    spine_segments: &[PathSegment],
) -> Result<KernelShape> {
    let (plane, scale) = sketch_plane(origin, u_axis, v_axis)?;
    sweep(&plane, profile, scale, spine_segments)
}

/// Skala relatif pemotong untuk percobaan ulang boolean yang gagal.
const RETRY_SCALE: f64 = 1e-5;

/// Faktor skala per sumbu dunia untuk percobaan ulang: sumbu tempat `tool`
/// punya face DATAR yang sebidang dengan face datar `target` dikunci (1.0),
/// supaya face yang berimpit tetap berimpit — kalau tidak, sisa celah
/// sub-mikron jadi face/tepi serpihan. `None` bila semua sumbu terkunci,
/// atau skala tak-seragam akan membuat permukaan lengkung `tool` jadi lonjong.
fn retry_factors(target: &KernelShape, tool: &KernelShape, grow: bool) -> Option<[f64; 3]> {
    use solidcraft_kernel::FaceSurface;
    let f = if grow { 1.0 + RETRY_SCALE } else { 1.0 - RETRY_SCALE };
    let size = tool.body()?.size().max(1e-9);
    let tol = size * 1e-7 + 1e-9;
    let planes = |s: &KernelShape| -> Vec<(glam::DVec3, f64)> {
        s.topology()
            .faces
            .iter()
            .filter_map(|lf| match lf.surface? {
                FaceSurface::Plane { normal, point } => {
                    let n = super::bridge::to_dv(*normal);
                    Some((n, n.dot(super::bridge::to_dv(point))))
                }
                _ => None,
            })
            .collect()
    };
    let (pt, pb) = (planes(target), planes(tool));
    let mut k = [f; 3];
    for (axis, slot) in k.iter_mut().enumerate() {
        let e = glam::DVec3::from_array({
            let mut v = [0.0; 3];
            v[axis] = 1.0;
            v
        });
        let pinned = pb.iter().any(|(n, d)| {
            n.dot(e).abs() > 1.0 - 1e-9
                && pt
                    .iter()
                    .any(|(m, c)| m.dot(*n).abs() > 1.0 - 1e-9 && (c * m.dot(*n) - d).abs() < tol)
        });
        if pinned {
            *slot = 1.0;
        }
    }
    if k.iter().all(|x| *x == 1.0) {
        return None;
    }
    // Permukaan lengkung harus tetap bundar: skala sama di bidang tegak
    // lurus sumbunya (bola: ketiga sumbu sama).
    let uniform = k[0] == k[1] && k[1] == k[2];
    if !uniform {
        for lf in &tool.topology().faces {
            let axis = match lf.surface {
                None | Some(FaceSurface::Plane { .. }) => continue,
                Some(FaceSurface::Cylinder { axis, .. })
                | Some(FaceSurface::Cone { axis, .. })
                | Some(FaceSurface::Torus { axis, .. }) => super::bridge::to_dv(*axis),
                Some(FaceSurface::Sphere { .. }) => return None,
            };
            let along = (0..3).find(|&i| axis[i].abs() > 1.0 - 1e-9)?;
            let others: Vec<f64> = (0..3).filter(|&i| i != along).map(|i| k[i]).collect();
            if others[0] != others[1] {
                return None;
            }
        }
    }
    Some(k)
}

fn boolean(a: &KernelShape, b: &KernelShape, op: BoolOp, what: &str) -> Result<Option<Body>> {
    let (ba, bb) = (a.require_body(what)?, b.require_body(what)?);
    let first = match kernel(what, || solidcraft_kernel::boolean(ba, bb, op)) {
        Ok(r) => return Ok(r),
        Err(e) => e,
    };
    // Singgungan persis (silinder menyinggung bidang, alas menyentuh face
    // sepanjang garis) kadang ditolak truck (PLAN_MIGRASI_TRUCK R1). Ulangi
    // sekali dengan `b` diskalakan 1e-5 terhadap pusat bbox-nya — mengecil
    // untuk union/intersect, membesar untuk cut (tanpa kulit tipis) — yang
    // memutus singgungan dengan geser permukaan < 1 µm per 100 mm.
    let Some(k) = retry_factors(a, b, op == BoolOp::Cut) else {
        return Err(first);
    };
    let Some((lo, hi)) = b.bounding_box() else {
        return Err(first);
    };
    let c = [0, 1, 2].map(|i| (lo[i] + hi[i]) * 0.5);
    let m = [
        [k[0], 0.0, 0.0, 0.0],
        [0.0, k[1], 0.0, 0.0],
        [0.0, 0.0, k[2], 0.0],
        [c[0] * (1.0 - k[0]), c[1] * (1.0 - k[1]), c[2] * (1.0 - k[2]), 1.0],
    ];
    let retried = kernel(what, || {
        let b2 = solidcraft_kernel::transform_matrix(bb, m)?;
        solidcraft_kernel::boolean(ba, &b2, op)
    });
    match retried {
        Ok(r) => {
            log::warn!("{what}: boolean diulang dengan pemotong diskalakan {k:?} (singgungan persis)");
            Ok(r)
        }
        Err(_) => Err(first),
    }
}

/// Union (gabung material) dua shape. Face sebidang di sambungan tetap
/// tampil sebagai satu face lewat topologi logis (padanan `clean()` OCCT).
pub fn union(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    match (a.body(), b.body()) {
        (Some(_), Some(_)) => {}
        (None, _) => return clone_shape(b),
        (_, None) => return clone_shape(a),
    }
    boolean(a, b, BoolOp::Union, "Boolean Union")
        .context("gagal menggabungkan (union) dua shape")?
        .map(KernelShape::from_body)
        .ok_or_else(|| anyhow!("gagal menggabungkan (union) dua shape: hasil kosong"))
}

/// Subtract (`a` dikurangi `b`). Hasil kosong (b menelan a) = shape kosong.
pub fn subtract(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    match (a.body(), b.body()) {
        (Some(_), Some(_)) => {}
        (None, _) => return Ok(KernelShape::empty()),
        (_, None) => return clone_shape(a),
    }
    Ok(boolean(a, b, BoolOp::Cut, "Boolean Subtract")
        .context("gagal mengurangi (subtract) dua shape")?
        .map(KernelShape::from_body)
        .unwrap_or_else(KernelShape::empty))
}

/// Boolean intersect (irisan) dua shape. Irisan kosong = [`EmptyIntersection`].
pub fn intersect(a: &KernelShape, b: &KernelShape) -> Result<KernelShape> {
    if a.body().is_none() || b.body().is_none() {
        return Err(EmptyIntersection.into());
    }
    boolean(a, b, BoolOp::Intersect, "Boolean Intersect")
        .context("gagal menghitung irisan (intersect) dua shape")?
        .map(KernelShape::from_body)
        .ok_or_else(|| EmptyIntersection.into())
}
