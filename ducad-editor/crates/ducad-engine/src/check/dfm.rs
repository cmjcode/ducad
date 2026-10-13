//! Check DFM (design for manufacturing) dan standar (W1): pengukuran murni di
//! atas `FaceInfo`/`EdgeInfo`/mesh. Tidak ada yang menyentuh OCCT selain
//! pemanggil yang sudah meng-enumerasi face, jadi semuanya deterministik.
//!
//! Batas angka (rasio kedalaman bor, sudut draft, …) TIDAK ditanam di sini:
//! nilainya datang dari check, biasanya lewat paket aturan
//! (`crate::check::rules`) yang juga membawa sitasinya.

use ducad_kernel::{EdgeInfo, EdgeKind, FaceInfo, KernelMesh, MeshRaycaster, SurfaceKind};
use glam::DVec3;
use serde_json::json;

use super::run::Outcome;
use super::types::{FitKind, Process, ToleranceClass};
use crate::inspect::round4;
use crate::select::ANG_TOL_DEG;

type R = Result<Outcome, String>;

fn v(a: [f64; 3]) -> DVec3 {
    DVec3::from_array(a)
}

/// Kelompok face silinder cekung yang berbagi sumbu dan radius.
#[derive(Debug, Clone)]
pub struct ConcaveCylinder {
    /// Titik pada sumbu, di tengah rentang aksial.
    pub center: DVec3,
    /// Arah sumbu (satuan).
    pub dir: DVec3,
    pub radius: f64,
    /// Panjang sepanjang sumbu.
    pub depth: f64,
    /// Jumlah luas face anggotanya.
    pub area: f64,
}

impl ConcaveCylinder {
    /// Bagian keliling yang tertutup permukaan (1 = silinder penuh).
    pub fn coverage(&self) -> f64 {
        let full = 2.0 * std::f64::consts::PI * self.radius * self.depth;
        if full <= 1e-12 {
            0.0
        } else {
            self.area / full
        }
    }

    /// Lubang = dinding silinder (hampir) penuh; sisanya fillet cekung.
    pub fn is_hole(&self) -> bool {
        self.coverage() >= 0.85
    }
}

/// Kelompokkan face `Cylinder` cekung menurut sumbu + radius.
pub fn concave_cylinders(faces: &[FaceInfo]) -> Vec<ConcaveCylinder> {
    struct Acc {
        point: DVec3,
        dir: DVec3,
        radius: f64,
        lo: f64,
        hi: f64,
        area: f64,
    }
    let cos_tol = ANG_TOL_DEG.to_radians().cos();
    let mut groups: Vec<Acc> = Vec::new();
    for f in faces {
        let (SurfaceKind::Cylinder, Some(true), Some(r), Some((p, d))) =
            (f.kind, f.concave, f.radius, f.axis)
        else {
            continue;
        };
        let (p, mut d) = (v(p), v(d).normalize_or_zero());
        if d.length_squared() < 0.5 || r <= 1e-9 {
            continue;
        }
        let slot = groups.iter().position(|g| {
            g.dir.dot(d).abs() >= cos_tol
                && (p - g.point).cross(g.dir).length() <= 1e-3
                && (g.radius - r).abs() <= 1e-4 * r.max(1.0)
        });
        let g = match slot {
            Some(i) => &mut groups[i],
            None => {
                groups.push(Acc {
                    point: p,
                    dir: d,
                    radius: r,
                    lo: f64::MAX,
                    hi: f64::MIN,
                    area: 0.0,
                });
                let last = groups.len() - 1;
                &mut groups[last]
            }
        };
        if g.dir.dot(d) < 0.0 {
            d = -d;
        }
        let _ = d;
        // Rentang aksial dari batas face; kotak pembatas sebagai cadangan.
        let mut pts: Vec<DVec3> = f.boundary.iter().copied().map(v).collect();
        if pts.is_empty() {
            let (a, b) = (v(f.bbox.0), v(f.bbox.1));
            for i in 0..8 {
                pts.push(DVec3::new(
                    if i & 1 == 0 { a.x } else { b.x },
                    if i & 2 == 0 { a.y } else { b.y },
                    if i & 4 == 0 { a.z } else { b.z },
                ));
            }
        }
        for q in pts {
            let t = (q - g.point).dot(g.dir);
            g.lo = g.lo.min(t);
            g.hi = g.hi.max(t);
        }
        g.area += f.area;
    }
    groups
        .into_iter()
        .filter(|g| g.hi > g.lo)
        .map(|g| ConcaveCylinder {
            center: g.point + g.dir * ((g.lo + g.hi) * 0.5),
            dir: g.dir,
            radius: g.radius,
            depth: g.hi - g.lo,
            area: g.area,
        })
        .collect()
}

/// Lubang (silinder cekung penuh), terurut diameter lalu posisi agar
/// laporan stabil.
pub fn holes(faces: &[FaceInfo]) -> Vec<ConcaveCylinder> {
    let mut list: Vec<ConcaveCylinder> = concave_cylinders(faces)
        .into_iter()
        .filter(ConcaveCylinder::is_hole)
        .collect();
    list.sort_by(|a, b| {
        let key = |h: &ConcaveCylinder| [h.radius, h.center.x, h.center.y, h.center.z];
        key(a)
            .partial_cmp(&key(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    list
}

pub(super) fn hole_depth_ratio(faces: &[FaceInfo], max_ratio: f64, min_diameter: f64) -> R {
    if max_ratio <= 0.0 {
        return Err("max_ratio harus > 0".into());
    }
    let list: Vec<ConcaveCylinder> = holes(faces)
        .into_iter()
        .filter(|h| 2.0 * h.radius >= min_diameter)
        .collect();
    let worst = list
        .iter()
        .map(|h| (h.depth / (2.0 * h.radius), h))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let Some((ratio, h)) = worst else {
        return Ok(Outcome::new(
            true,
            serde_json::Value::Null,
            json!({ "max_ratio": max_ratio }),
            "tidak ada lubang silinder".into(),
        ));
    };
    let over = list
        .iter()
        .filter(|h| h.depth / (2.0 * h.radius) > max_ratio + 1e-9)
        .count();
    Ok(Outcome::new(
        over == 0,
        json!({ "max_ratio": round4(ratio), "diameter": round4(2.0 * h.radius),
                "depth": round4(h.depth), "holes": list.len(), "over_limit": over }),
        json!({ "max_ratio": max_ratio }),
        format!(
            "lubang terdalam relatif: Ø{:.3} × {:.3} mm = {ratio:.2} × diameter, batas {max_ratio} × ({over} dari {} lubang melewati batas)",
            2.0 * h.radius,
            h.depth,
            list.len()
        ),
    )
    .at(Some(h.center.to_array().map(round4))))
}

/// Material tersisa di sekitar lubang `h`: sinar radial dari dinding lubang
/// ke luar, jarak ke permukaan berikutnya.
fn web_of(ray: &MeshRaycaster, h: &ConcaveCylinder) -> Option<f64> {
    const ANGLES: usize = 24;
    let helper = if h.dir.x.abs() < 0.9 {
        DVec3::X
    } else {
        DVec3::Y
    };
    let u = h.dir.cross(helper).normalize_or_zero();
    let w = h.dir.cross(u);
    // Mesh lubang adalah poligon di DALAM lingkaran sejati, jadi titik di
    // radius sejati + ε sudah berada di dalam material mesh.
    let start = h.radius + (h.radius * 1e-3).max(1e-3);
    let mut best: Option<f64> = None;
    for frac in [0.15, 0.5, 0.85] {
        let base = h.center + h.dir * ((frac - 0.5) * h.depth);
        for k in 0..ANGLES {
            let a = k as f64 / ANGLES as f64 * std::f64::consts::TAU;
            let radial = u * a.cos() + w * a.sin();
            let origin = base + radial * start;
            let hit = ray.closest(
                origin.to_array().map(|x| x as f32),
                radial.to_array().map(|x| x as f32),
            )?;
            let web = hit as f64 + (start - h.radius);
            if best.is_none_or(|b| web < b) {
                best = Some(web);
            }
        }
    }
    best
}

pub(super) fn hole_web(
    faces: &[FaceInfo],
    mesh: &KernelMesh,
    min_ratio: Option<f64>,
    min: Option<f64>,
) -> R {
    if min_ratio.is_none() && min.is_none() {
        return Err("hole_web butuh min_ratio dan/atau min".into());
    }
    let list = holes(faces);
    if list.is_empty() {
        return Ok(Outcome::new(
            true,
            serde_json::Value::Null,
            json!({ "min_ratio": min_ratio, "min": min }),
            "tidak ada lubang silinder".into(),
        ));
    }
    let ray = MeshRaycaster::new(mesh).ok_or("body tanpa mesh")?;
    // Margin terkecil terhadap batasnya (negatif = melanggar).
    let mut worst: Option<(f64, f64, &ConcaveCylinder)> = None;
    let mut over = 0usize;
    for h in &list {
        let Some(web) = web_of(&ray, h) else {
            continue;
        };
        let need = min_ratio
            .map(|r| r * 2.0 * h.radius)
            .unwrap_or(0.0)
            .max(min.unwrap_or(0.0));
        let margin = web - need;
        // Toleransi 0,02 mm menyerap sag tesselasi.
        if margin < -0.02 {
            over += 1;
        }
        if worst.is_none_or(|(m, _, _)| margin < m) {
            worst = Some((margin, web, h));
        }
    }
    let Some((_, web, h)) = worst else {
        return Err("material di sekitar lubang tidak bisa diukur".into());
    };
    let need = min_ratio
        .map(|r| r * 2.0 * h.radius)
        .unwrap_or(0.0)
        .max(min.unwrap_or(0.0));
    Ok(Outcome::new(
        over == 0,
        json!({ "web": round4(web), "diameter": round4(2.0 * h.radius),
                "ratio": round4(web / (2.0 * h.radius)), "holes": list.len(), "over_limit": over }),
        json!({ "min_ratio": min_ratio, "min": min, "required": round4(need) }),
        format!(
            "material tersisa paling kritis {web:.3} mm di sekitar lubang Ø{:.3}, minimum {need:.3} mm ({over} dari {} lubang melanggar)",
            2.0 * h.radius,
            list.len()
        ),
    )
    .at(Some(h.center.to_array().map(round4))))
}

pub(super) fn min_draft(faces: &[FaceInfo], pull: [f64; 3], min_deg: f64) -> R {
    let pull = v(pull);
    let cos_tol = ANG_TOL_DEG.to_radians().cos();
    let mut worst: Option<(f64, &FaceInfo)> = None;
    let mut checked = 0usize;
    let mut failing = 0usize;
    for f in faces {
        let draft = match f.kind {
            SurfaceKind::Plane => {
                let s = v(f.normal).normalize_or_zero().dot(pull).abs().min(1.0);
                s.asin().to_degrees()
            }
            SurfaceKind::Cylinder | SurfaceKind::Cone => {
                let Some((_, d)) = f.axis else { continue };
                if v(d).normalize_or_zero().dot(pull).abs() < cos_tol {
                    // Sumbu tidak searah tarikan: bukan dinding cetakan lurus.
                    continue;
                }
                let s = v(f.normal).normalize_or_zero().dot(pull).abs().min(1.0);
                s.asin().to_degrees()
            }
            _ => continue,
        };
        // Face tegak lurus tarikan = bidang pisah/tutup.
        if draft >= 90.0 - ANG_TOL_DEG {
            continue;
        }
        checked += 1;
        if draft + 1e-6 < min_deg {
            failing += 1;
        }
        if worst.is_none_or(|(w, _)| draft < w) {
            worst = Some((draft, f));
        }
    }
    let Some((draft, f)) = worst else {
        return Ok(Outcome::new(
            true,
            serde_json::Value::Null,
            json!({ "min_deg": min_deg }),
            "tidak ada dinding searah tarikan".into(),
        ));
    };
    Ok(Outcome::new(
        failing == 0,
        json!({ "min_draft_deg": round4(draft), "face": f.index, "walls": checked, "below_limit": failing }),
        json!({ "min_deg": min_deg }),
        format!(
            "draft terkecil {draft:.2}° (face #{}), minimum {min_deg}° ({failing} dari {checked} dinding di bawah batas)",
            f.index
        ),
    )
    .at(Some(f.centroid.map(round4))))
}

pub(super) fn max_overhang(mesh: &KernelMesh, up: [f64; 3], max_deg: f64, max_area: f64) -> R {
    if !(0.0..90.0).contains(&max_deg) {
        return Err("max_deg harus di antara 0 dan 90".into());
    }
    let up = v(up);
    let pos = |i: u32| {
        mesh.positions
            .get(i as usize)
            .map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64))
    };
    let bed = mesh
        .positions
        .iter()
        .map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64).dot(up))
        .fold(f64::MAX, f64::min);
    if bed == f64::MAX {
        return Err("body tanpa mesh".into());
    }
    let limit = max_deg.to_radians().sin();
    let mut area = 0.0;
    let mut worst: Option<(f64, DVec3)> = None;
    for c in mesh.indices.chunks_exact(3) {
        let (Some(a), Some(b), Some(d)) = (pos(c[0]), pos(c[1]), pos(c[2])) else {
            return Err("mesh rusak (indeks di luar jangkauan)".into());
        };
        let n = (b - a).cross(d - a);
        let tri_area = n.length() * 0.5;
        if tri_area < 1e-9 {
            continue;
        }
        let down = -(n / (2.0 * tri_area)).dot(up);
        if down <= limit + 1e-6 {
            continue;
        }
        // Menempel di bed: bukan overhang.
        if [a, b, d].iter().all(|p| p.dot(up) - bed <= 1e-3) {
            continue;
        }
        area += tri_area;
        if worst.is_none_or(|(w, _)| tri_area > w) {
            worst = Some((tri_area, (a + b + d) / 3.0));
        }
    }
    Ok(Outcome::new(
        area <= max_area + 1e-6,
        json!({ "overhang_area": round4(area) }),
        json!({ "max_deg": max_deg, "max_area": max_area }),
        format!(
            "luas overhang lebih dari {max_deg}° dari vertikal: {area:.2} mm², batas {max_area} mm²"
        ),
    )
    .at(worst.map(|(_, p)| p.to_array().map(round4))))
}

pub(super) fn min_internal_radius(
    faces: &[FaceInfo],
    edges: &[EdgeInfo],
    axis: [f64; 3],
    min: f64,
) -> R {
    let axis = v(axis);
    let cos_tol = ANG_TOL_DEG.to_radians().cos();
    let sin_tol = ANG_TOL_DEG.to_radians().sin();
    // (a) Fillet cekung sejajar sumbu pahat dengan radius terlalu kecil.
    let mut smallest: Option<(f64, DVec3)> = None;
    let mut small = 0usize;
    for c in concave_cylinders(faces) {
        if c.is_hole() || c.dir.dot(axis).abs() < cos_tol {
            continue;
        }
        if c.radius + 1e-6 < min {
            small += 1;
        }
        if smallest.is_none_or(|(r, _)| c.radius < r) {
            smallest = Some((c.radius, c.center));
        }
    }
    // (b) Tepi cekung tajam antara dua dinding yang sejajar sumbu pahat.
    let is_wall = |f: &FaceInfo| {
        f.kind == SurfaceKind::Plane && v(f.normal).normalize_or_zero().dot(axis).abs() <= sin_tol
    };
    let mut sharp: Vec<DVec3> = Vec::new();
    for (i, a) in faces.iter().enumerate() {
        if !is_wall(a) {
            continue;
        }
        for b in faces.iter().skip(i + 1) {
            if !is_wall(b) {
                continue;
            }
            let (na, nb) = (
                v(a.normal).normalize_or_zero(),
                v(b.normal).normalize_or_zero(),
            );
            if na.dot(nb).abs() >= cos_tol {
                continue; // sejajar: bukan sudut
            }
            for ei in a.edge_indices.iter().filter(|e| b.edge_indices.contains(e)) {
                let Some(e) = edges.get(*ei) else { continue };
                if e.kind != EdgeKind::Line {
                    continue;
                }
                let mid = v(e.mid);
                // Cekung: face A menjulang di sisi luar bidang B (dan sebaliknya).
                let rises =
                    (v(a.centroid) - mid).dot(nb) > 1e-6 && (v(b.centroid) - mid).dot(na) > 1e-6;
                if rises {
                    sharp.push(mid);
                }
            }
        }
    }
    let pass = small == 0 && sharp.is_empty();
    let location = sharp
        .first()
        .copied()
        .or(smallest.filter(|(r, _)| *r + 1e-6 < min).map(|(_, p)| p));
    let msg = match (sharp.len(), smallest) {
        (0, None) => "tidak ada sudut dalam sejajar sumbu pahat".to_string(),
        (0, Some((r, _))) => format!(
            "radius sudut dalam terkecil {r:.3} mm, minimum {min} mm ({small} fillet di bawah batas)"
        ),
        (n, _) => format!(
            "{n} sudut dalam tajam (R0) sejajar sumbu pahat tidak bisa dibentuk end mill; minimum R{min} mm"
        ),
    };
    Ok(Outcome::new(
        pass,
        json!({ "sharp_corners": sharp.len(), "small_fillets": small,
                "min_radius": smallest.map(|(r, _)| round4(r)) }),
        json!({ "min": min }),
        msg,
    )
    .at(location.map(|p| p.to_array().map(round4))))
}

/// Satu ulir untuk `thread_engagement`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadLen {
    /// Mis. `"M6"` atau `"M10x1.5"`.
    pub designation: String,
    pub nominal: f64,
    pub length: f64,
}

/// Diameter nominal dari sebutan `M<d>[x<pitch>]`.
pub fn nominal_of(designation: &str) -> Option<f64> {
    let rest = designation.trim().strip_prefix(['M', 'm'])?;
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    rest[..end].parse().ok().filter(|d: &f64| *d > 0.0)
}

pub(super) fn thread_engagement(threads: &[ThreadLen], min_ratio: f64) -> R {
    let worst = threads
        .iter()
        .map(|t| (t.length / t.nominal, t))
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let Some((ratio, t)) = worst else {
        return Ok(Outcome::new(
            true,
            serde_json::Value::Null,
            json!({ "min_ratio": min_ratio }),
            "body tanpa ulir".into(),
        ));
    };
    let short = threads
        .iter()
        .filter(|t| t.length / t.nominal + 1e-9 < min_ratio)
        .count();
    Ok(Outcome::new(
        short == 0,
        json!({ "min_ratio": round4(ratio), "thread": t.designation, "length": round4(t.length),
                "threads": threads.len(), "below_limit": short }),
        json!({ "min_ratio": min_ratio }),
        format!(
            "panjang ulir terpendek relatif: {} sepanjang {:.3} mm = {ratio:.2} × d, minimum {min_ratio} × d ({short} dari {} ulir kurang)",
            t.designation,
            t.length,
            threads.len()
        ),
    ))
}

pub(super) fn fit(
    nominal: f64,
    hole: &str,
    shaft: &str,
    kind: Option<FitKind>,
    min_clearance: Option<f64>,
    max_clearance: Option<f64>,
) -> R {
    let h = ducad_core::IsoFit::parse(hole)?;
    let s = ducad_core::IsoFit::parse(shaft)?;
    let (lo, hi) = ducad_core::iso286::fit_clearance(nominal, &h, &s)?;
    let actual = if lo >= 0.0 {
        FitKind::Clearance
    } else if hi <= 0.0 {
        FitKind::Interference
    } else {
        FitKind::Transition
    };
    let pass = kind.is_none_or(|k| k == actual)
        && min_clearance.is_none_or(|m| lo + 1e-12 >= m)
        && max_clearance.is_none_or(|m| hi <= m + 1e-12);
    let name = |k: FitKind| match k {
        FitKind::Clearance => "longgar",
        FitKind::Transition => "transisi",
        FitKind::Interference => "sesak",
    };
    Ok(Outcome::new(
        pass,
        json!({ "kind": actual, "min_clearance": round4(lo), "max_clearance": round4(hi) }),
        json!({ "kind": kind, "min_clearance": min_clearance, "max_clearance": max_clearance }),
        format!(
            "suaian Ø{nominal} {hole}/{shaft}: {} — kelonggaran {:+.4} … {:+.4} mm",
            name(actual),
            lo,
            hi
        ),
    ))
}

/// Deviasi yang diizinkan (±mm) untuk dimensi linier `size` menurut
/// ISO 2768-1:1989 Tabel 1. `None` = kelas itu tidak didefinisikan untuk
/// rentang ukuran tersebut (atau ukuran di luar 0,5–4000 mm).
pub fn iso2768_linear(size_mm: f64, class: ToleranceClass) -> Option<f64> {
    // Batas atas rentang ukuran nominal (mm), lalu f / m / c / v.
    const TABLE: [(f64, [Option<f64>; 4]); 8] = [
        (3.0, [Some(0.05), Some(0.1), Some(0.2), None]),
        (6.0, [Some(0.05), Some(0.1), Some(0.3), Some(0.5)]),
        (30.0, [Some(0.1), Some(0.2), Some(0.5), Some(1.0)]),
        (120.0, [Some(0.15), Some(0.3), Some(0.8), Some(1.5)]),
        (400.0, [Some(0.2), Some(0.5), Some(1.2), Some(2.5)]),
        (1000.0, [Some(0.3), Some(0.8), Some(2.0), Some(4.0)]),
        (2000.0, [Some(0.5), Some(1.2), Some(3.0), Some(6.0)]),
        (4000.0, [None, Some(2.0), Some(4.0), Some(8.0)]),
    ];
    if !(0.5..=4000.0).contains(&size_mm) {
        return None;
    }
    let col = match class {
        ToleranceClass::F => 0,
        ToleranceClass::M => 1,
        ToleranceClass::C => 2,
        ToleranceClass::V => 3,
    };
    TABLE
        .iter()
        .find(|(upper, _)| size_mm <= *upper)
        .and_then(|(_, row)| row[col])
}

/// Kelas ISO 2768-1 terhalus yang LAZIM dipegang sebuah proses tanpa
/// pengerjaan lanjut. Ini pedoman DUCAD, bukan isi standar — sitasi paket
/// aturan menyebutnya demikian.
pub fn process_capability(process: Process) -> ToleranceClass {
    match process {
        Process::Cnc => ToleranceClass::F,
        Process::SheetMetal | Process::Sla | Process::Injection => ToleranceClass::M,
        Process::Fdm | Process::Sls => ToleranceClass::C,
        Process::Casting => ToleranceClass::V,
    }
}

pub(super) fn general_tolerance(size: [f64; 3], class: ToleranceClass, process: Process) -> R {
    let capable = process_capability(process);
    let dims: Vec<serde_json::Value> = size
        .iter()
        .map(|s| json!({ "size": round4(*s), "tol": iso2768_linear(*s, class) }))
        .collect();
    let label = |c: ToleranceClass| match c {
        ToleranceClass::F => "f",
        ToleranceClass::M => "m",
        ToleranceClass::C => "c",
        ToleranceClass::V => "v",
    };
    let fmt = |s: f64| match iso2768_linear(s, class) {
        Some(t) => format!("{s:.1} ±{t}"),
        None => format!("{s:.1} (di luar tabel)"),
    };
    Ok(Outcome::new(
        capable <= class,
        json!({ "process_class": capable, "dimensions": dims }),
        json!({ "class": class, "process": process }),
        format!(
            "toleransi umum ISO 2768-{}: {} × {} × {} mm; proses lazim memegang kelas {}",
            label(class),
            fmt(size[0]),
            fmt(size[1]),
            fmt(size[2]),
            label(capable)
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso2768_table_matches_the_standard() {
        use ToleranceClass::*;
        assert_eq!(iso2768_linear(2.0, F), Some(0.05));
        assert_eq!(iso2768_linear(6.0, M), Some(0.1));
        assert_eq!(iso2768_linear(25.0, M), Some(0.2));
        assert_eq!(iso2768_linear(100.0, M), Some(0.3));
        assert_eq!(iso2768_linear(100.0, F), Some(0.15));
        assert_eq!(iso2768_linear(300.0, C), Some(1.2));
        assert_eq!(iso2768_linear(2.0, V), None);
        assert_eq!(iso2768_linear(3000.0, F), None);
        assert_eq!(iso2768_linear(0.2, M), None);
    }

    #[test]
    fn thread_designation_parses() {
        assert_eq!(nominal_of("M6"), Some(6.0));
        assert_eq!(nominal_of("M10x1.5"), Some(10.0));
        assert_eq!(nominal_of("M2.5"), Some(2.5));
        assert_eq!(nominal_of("G1/4"), None);
    }

    #[test]
    fn fit_kinds() {
        let o = fit(25.0, "H7", "g6", Some(FitKind::Clearance), None, None).unwrap();
        assert_eq!(o.status, super::super::types::CheckStatus::Pass);
        let o = fit(25.0, "H7", "p6", Some(FitKind::Clearance), None, None).unwrap();
        assert_eq!(o.status, super::super::types::CheckStatus::Fail);
        assert!(fit(25.0, "h7", "g6", None, None, None).is_err());
    }
}
