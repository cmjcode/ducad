//! Fitur plastik injeksi (Paritas 6B): boss, lip/groove, snap-fit
//! kantilever, rest. Nilai bawaan diambil dari aturan DFM injeksi yang aktif
//! (`dfm-injection`: tebal dinding, draft, radius dalam) — lihat
//! [`PlasticDefaults::from_checks`].

use ducad_kernel::{KernelShape, Profile, ProfileSegment};
use glam::{DQuat, DVec3};

use super::{finish, require_positive};
use crate::error::{OpError, OpResult};
use crate::model::BodyGeometry;

fn k(op: &'static str) -> impl Fn(anyhow::Error) -> OpError {
    move |e| OpError::kernel(op, e)
}

/// Nilai bawaan fitur plastik (Paritas 6.9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlasticDefaults {
    /// Draft per sisi (derajat).
    pub draft_deg: f64,
    /// Tebal dinding minimum (mm).
    pub min_wall: f64,
    /// Radius dalam minimum (mm).
    pub min_radius: f64,
}

impl Default for PlasticDefaults {
    /// Nilai bawaan paket `dfm-injection`.
    fn default() -> Self {
        let pack = crate::check::rule_pack("dfm-injection");
        let var = |name: &str, d: f64| {
            pack.as_ref()
                .and_then(|p| p.vars.get(name))
                .and_then(|v| v.value.as_f64())
                .unwrap_or(d)
        };
        Self {
            draft_deg: var("min_draft_deg", 1.0),
            min_wall: var("min_wall", 1.0),
            min_radius: var("min_internal_radius", 0.5),
        }
    }
}

impl PlasticDefaults {
    /// Dari check desain yang aktif (hasil ekspansi rule pack, termasuk
    /// penimpaan variabelnya); yang tidak ada memakai nilai paket.
    pub fn from_checks(checks: &[crate::check::CheckItem]) -> Self {
        let mut d = Self::default();
        for c in checks {
            let Ok(v) = serde_json::to_value(c) else {
                continue;
            };
            let num = |k: &str| v.get(k).and_then(|x| x.as_f64());
            match v.get("check").and_then(|x| x.as_str()) {
                Some("min_draft_angle") => {
                    if let Some(x) = num("min_deg") {
                        d.draft_deg = x;
                    }
                }
                Some("min_wall") => {
                    if let Some(x) = num("min") {
                        d.min_wall = x;
                    }
                }
                Some("min_internal_radius") => {
                    if let Some(x) = num("min") {
                        d.min_radius = x;
                    }
                }
                _ => {}
            }
        }
        d
    }

    /// Diameter luar boss bawaan untuk lubang `hole_d` (dinding 2× tebal min).
    pub fn boss_outer(&self, hole_d: f64) -> f64 {
        hole_d + 4.0 * self.min_wall.max(0.5)
    }
}

/// Basis ortonormal (u, v) tegak lurus `n`; u = +X dunia diproyeksikan bila bisa.
fn basis(n: DVec3) -> (DVec3, DVec3) {
    let n = n.normalize_or_zero();
    let u = (DVec3::X - n * DVec3::X.dot(n))
        .try_normalize()
        .unwrap_or_else(|| (DVec3::Y - n * DVec3::Y.dot(n)).normalize_or_zero());
    (u, n.cross(u))
}

/// Shape di sumbu +Z (alas di origin) diputar ke `n` dan digeser ke `at`.
fn place_z(s: &KernelShape, at: DVec3, n: DVec3, what: &'static str) -> OpResult<KernelShape> {
    let q = DQuat::from_rotation_arc(DVec3::Z, n);
    let (axis, ang) = q.to_axis_angle();
    let s = if ang.abs() > 1e-12 {
        ducad_kernel::rotate_shape(s, (0.0, 0.0, 0.0), (axis.x, axis.y, axis.z), ang)
            .map_err(k(what))?
    } else {
        ducad_kernel::clone_shape(s).map_err(k(what))?
    };
    ducad_kernel::translate_shape(&s, at.x, at.y, at.z).map_err(k(what))
}

/// 6.10 Boss: silinder ber-draft dari `at` sepanjang `normal` setinggi
/// `height`, lubang `hole_d` (0 = pejal) sedalam boss, fillet dasar
/// `fillet` bila bisa (kegagalan fillet hanya peringatan).
#[allow(clippy::too_many_arguments)]
pub fn boss(
    base: &KernelShape,
    at: DVec3,
    normal: DVec3,
    height: f64,
    outer_d: f64,
    hole_d: f64,
    draft_deg: f64,
    fillet: f64,
) -> OpResult<(BodyGeometry, Vec<String>)> {
    require_positive("height", height)?;
    require_positive("outer_d", outer_d)?;
    if !(0.0..outer_d).contains(&hole_d) {
        return Err(OpError::invalid(format!(
            "hole_d ({hole_d}) harus 0 ≤ hole_d < outer_d ({outer_d})"
        )));
    }
    if !(0.0..45.0).contains(&draft_deg) {
        return Err(OpError::invalid("draft boss harus 0 ≤ draft < 45°"));
    }
    let n = normal.normalize_or_zero();
    if n == DVec3::ZERO || !at.is_finite() {
        return Err(OpError::invalid("posisi/arah boss tidak valid"));
    }
    let r0 = outer_d * 0.5;
    let r1 = r0 - height * draft_deg.to_radians().tan();
    if r1 <= hole_d * 0.5 + 0.1 {
        return Err(OpError::invalid("draft terlalu besar: dinding atas boss habis")
            .with_hint("kurangi draft_deg / height atau perbesar outer_d"));
    }
    let cone = ducad_kernel::make_cone(r0, r1, height).map_err(k("Boss"))?;
    let cone = place_z(&cone, at, n, "Boss")?;
    let mut acc = ducad_kernel::union(base, &cone).map_err(k("Boss"))?;
    let mut warnings = Vec::new();
    if fillet > 0.0 {
        // Tepi lingkaran dasar boss (radius r0, di bidang `at`).
        let base_edges: Vec<usize> = ducad_kernel::enumerate_edges(&acc)
            .iter()
            .filter(|e| {
                e.radius.is_some_and(|r| (r - r0).abs() < 1e-3)
                    && (DVec3::from(e.start) - at).dot(n).abs() < 1e-3
            })
            .map(|e| e.index)
            .collect();
        match ducad_kernel::fillet_edges_by_index(&acc, fillet, &base_edges) {
            Ok(f) if !base_edges.is_empty() => acc = f,
            _ => warnings.push(format!(
                "fillet dasar boss {fillet} mm dilewati (kernel menolak)"
            )),
        }
    }
    if hole_d > 0.0 {
        let hole = ducad_kernel::make_cylinder(hole_d * 0.5, height + 1.0).map_err(k("Boss"))?;
        let hole = place_z(&hole, at + n * 0.5, n, "Boss")?;
        acc = ducad_kernel::subtract(&acc, &hole).map_err(k("Boss"))?;
    }
    Ok((finish("Boss", acc)?, warnings))
}

/// 6.11 Lip: pita selebar `width` di sepanjang loop dalam face planar
/// `face` dinaikkan setinggi `height`. Groove (`groove = true`): pita yang
/// sama, diperbesar `clearance`, dipotong ke dalam body.
pub fn lip(
    base: &KernelShape,
    face: usize,
    width: f64,
    height: f64,
    groove: bool,
    clearance: f64,
) -> OpResult<BodyGeometry> {
    require_positive("width", width)?;
    require_positive("height", height)?;
    if !clearance.is_finite() || clearance < 0.0 {
        return Err(OpError::invalid("clearance groove harus ≥ 0"));
    }
    let raised =
        ducad_kernel::direct::raise_band(base, face, width, height, false).map_err(k("Lip"))?;
    if !groove {
        return finish("Lip", raised);
    }
    // Pita = hasil naik − body; geser masuk setinggi pita, perbesar clearance.
    let band = ducad_kernel::subtract(&raised, base).map_err(k("Groove"))?;
    let n = ducad_kernel::enumerate_faces(base)
        .get(face)
        .map(|f| DVec3::from(f.normal))
        .ok_or_else(|| OpError::invalid("face groove tidak ada"))?;
    let mut tool =
        ducad_kernel::translate_shape(&band, -n.x * height, -n.y * height, -n.z * height)
            .map_err(k("Groove"))?;
    if clearance > 0.0 {
        let all: Vec<usize> = (0..ducad_kernel::enumerate_faces(&tool).len()).collect();
        tool = ducad_kernel::offset_faces_by_index(&tool, &all, clearance).map_err(k("Groove"))?;
    }
    let out = ducad_kernel::subtract(base, &tool).map_err(k("Groove"))?;
    finish("Groove", out)
}

/// Hasil snap-fit: geometri + regangan maksimum perkiraan (%).
pub struct SnapFit {
    pub geo: BodyGeometry,
    pub strain_pct: f64,
}

/// Regangan izin bawaan snap-fit (ABS, %).
pub const SNAP_FIT_ALLOWABLE_STRAIN_PCT: f64 = 2.5;

/// 6.12 Snap-fit kantilever: balok setebal `thickness`, panjang `length`,
/// lebar `width` tegak dari `at` sepanjang `normal`, dengan kait sedalam
/// `hook` (ramp miring `angle_deg`) di ujung, menonjol ke arah `dir`.
/// Regangan = 1,5·t·y/L² (kantilever penampang persegi).
#[allow(clippy::too_many_arguments)]
pub fn snap_fit(
    base: &KernelShape,
    at: DVec3,
    normal: DVec3,
    dir: DVec3,
    length: f64,
    thickness: f64,
    width: f64,
    hook: f64,
    angle_deg: f64,
) -> OpResult<SnapFit> {
    for (n, v) in [
        ("length", length),
        ("thickness", thickness),
        ("width", width),
        ("hook", hook),
    ] {
        require_positive(n, v)?;
    }
    if !(5.0..85.0).contains(&angle_deg) {
        return Err(OpError::invalid("sudut kait harus 5..85°"));
    }
    let n = normal.normalize_or_zero();
    if n == DVec3::ZERO || !at.is_finite() || !dir.is_finite() {
        return Err(OpError::invalid("posisi/arah snap-fit tidak valid"));
    }
    let u = (dir - n * dir.dot(n))
        .try_normalize()
        .unwrap_or_else(|| basis(n).0);
    let v = u.cross(n);
    // Profil di bidang (u, n): balok + kait (ramp miring ke ujung).
    let t = thickness;
    let ramp = hook / angle_deg.to_radians().tan();
    let top = length;
    let y0 = (top - ramp - t).max(t);
    let pts = [
        (0.0, -1.0),
        (t, -1.0),
        (t, y0),
        (t + hook, y0),
        (t, (y0 + ramp).min(top)),
        (t, top),
        (0.0, top),
    ];
    let loop_ = Profile::Loop(
        pts.iter()
            .zip(pts.iter().cycle().skip(1))
            .map(|(a, b)| ProfileSegment::Line { start: *a, end: *b })
            .collect(),
    );
    let origin = at - v * (width * 0.5);
    let solid = ducad_kernel::extrude_profile_on_plane(
        &loop_,
        origin.to_array(),
        u.to_array(),
        n.to_array(),
        v.to_array(),
        width,
    )
    .map_err(k("Snap fit"))?;
    let out = ducad_kernel::union(base, &solid).map_err(k("Snap fit"))?;
    let strain_pct = 1.5 * t * hook / (length * length) * 100.0;
    Ok(SnapFit {
        geo: finish("Snap fit", out)?,
        strain_pct,
    })
}

/// 6.13 Rest: bantalan datar Ø`diameter`, permukaan atasnya `height` di
/// atas `at` searah `dir`, menembus ke dalam body sedalam `diameter`.
pub fn rest(
    base: &KernelShape,
    at: DVec3,
    dir: DVec3,
    diameter: f64,
    height: f64,
) -> OpResult<BodyGeometry> {
    require_positive("diameter", diameter)?;
    if !height.is_finite() || height < 0.0 {
        return Err(OpError::invalid("tinggi rest harus ≥ 0"));
    }
    let d = dir.normalize_or_zero();
    if d == DVec3::ZERO || !at.is_finite() {
        return Err(OpError::invalid("posisi/arah rest tidak valid"));
    }
    // Tertanam sampai sedalam diameter, tapi tidak menembus sisi seberang
    // body (jarak ke permukaan berikutnya di arah −d).
    let inner = at - d * 1e-3;
    let to_back = ducad_kernel::ray_hit_distance(
        &base.tessellate(),
        inner.as_vec3().to_array(),
        (-d).as_vec3().to_array(),
    )
    .map(|x| x as f64 * 0.98)
    .unwrap_or(diameter);
    let depth = to_back.clamp(0.01, diameter);
    let cyl = ducad_kernel::make_cylinder(diameter * 0.5, depth + height).map_err(k("Rest"))?;
    let cyl = place_z(&cyl, at - d * depth, d, "Rest")?;
    let out = ducad_kernel::union(base, &cyl).map_err(k("Rest"))?;
    finish("Rest", out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plate() -> KernelShape {
        ducad_kernel::make_box(40.0, 40.0, 3.0, false).unwrap()
    }

    #[test]
    fn defaults_follow_pack_and_checks() {
        let d = PlasticDefaults::default();
        assert_eq!(d.draft_deg, 1.0);
        let checks: Vec<crate::check::CheckItem> = serde_json::from_value(serde_json::json!([
            {"check":"min_draft_angle","body":"*","pull":"z","min_deg":2.5},
            {"check":"min_wall","body":"*","min":1.8}
        ]))
        .unwrap();
        let d = PlasticDefaults::from_checks(&checks);
        assert_eq!((d.draft_deg, d.min_wall, d.min_radius), (2.5, 1.8, 0.5));
        assert!((d.boss_outer(3.0) - 10.2).abs() < 1e-9);
    }

    #[test]
    fn boss_adds_hollow_drafted_post() {
        let p = plate();
        let v0 = p.volume();
        let (g, _) = boss(&p, DVec3::new(20.0, 20.0, 3.0), DVec3::Z, 10.0, 8.0, 3.0, 1.0, 0.5).unwrap();
        let added = g.shape.volume() - v0;
        assert!(added > 300.0 && added < 480.0, "{added}");
        assert!(boss(&p, DVec3::new(20.0, 20.0, 3.0), DVec3::Z, 10.0, 4.0, 5.0, 1.0, 0.0).is_err());
        assert!(boss(&p, DVec3::new(20.0, 20.0, 3.0), DVec3::Z, 100.0, 4.0, 2.0, 20.0, 0.0).is_err());
    }

    #[test]
    fn snap_fit_strain_and_rest() {
        let p = plate();
        let s = snap_fit(&p, DVec3::new(20.0, 20.0, 3.0), DVec3::Z, DVec3::X, 15.0, 1.5, 6.0, 1.0, 30.0).unwrap();
        assert!(s.geo.shape.volume() > p.volume());
        assert!((s.strain_pct - 1.5 * 1.5 * 1.0 / 225.0 * 100.0).abs() < 1e-9);
        let r = rest(&p, DVec3::new(10.0, 10.0, 3.0), DVec3::Z, 6.0, 2.0).unwrap();
        let added = r.shape.volume() - p.volume();
        assert!((added - std::f64::consts::PI * 9.0 * 2.0).abs() < 1.0, "{added}");
        assert!(rest(&p, DVec3::ZERO, DVec3::ZERO, 6.0, 1.0).is_err());
    }

    #[test]
    fn lip_and_groove_on_shelled_box() {
        let b = ducad_kernel::make_box(30.0, 20.0, 15.0, false).unwrap();
        let top = ducad_kernel::enumerate_faces(&b)
            .iter()
            .find(|f| f.normal[2] > 0.9)
            .unwrap()
            .index;
        let shell = ducad_kernel::shell_faces_by_index(&b, 2.0, &[top]).unwrap();
        let rim = ducad_kernel::enumerate_faces(&shell)
            .iter()
            .filter(|f| f.normal[2] > 0.9)
            .max_by(|a, c| a.centroid[2].total_cmp(&c.centroid[2]))
            .unwrap()
            .index;
        let l = lip(&shell, rim, 1.0, 2.0, false, 0.0).unwrap();
        assert!(l.shape.volume() > shell.volume());
        let g = lip(&shell, rim, 1.0, 2.0, true, 0.1).unwrap();
        assert!(g.shape.volume() < shell.volume());
    }
}
