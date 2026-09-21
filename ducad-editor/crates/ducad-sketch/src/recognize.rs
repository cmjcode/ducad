//! Pengenal bentuk deterministik untuk coretan Pencil/mouse (P12.1).
//!
//! Murni: tanpa RNG, tanpa state global. Masukan koordinat BIDANG SKETCH
//! (mm), keluaran entitas sketch biasa. Pelurusan ke sumbu dan constraint
//! bukan urusan modul ini — lihat `infer` (P12.2).

use glam::DVec2;

use crate::entity::Entity;

/// Satu coretan mentah.
#[derive(Debug, Clone, Default)]
pub struct Stroke {
    /// Koordinat bidang sketch, mm.
    pub points: Vec<DVec2>,
    /// Tekanan per titik; boleh kosong.
    pub pressure: Vec<f32>,
}

/// Bentuk yang dikenali.
#[derive(Debug, Clone, PartialEq)]
pub enum Recognized {
    Line {
        a: DVec2,
        b: DVec2,
    },
    Circle {
        center: DVec2,
        radius: f64,
    },
    /// Sudut radian, konvensi `Entity::Arc` (CCW dari `start` ke `end`).
    Arc {
        center: DVec2,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    /// Sejajar sumbu (`Entity::Ellipse` tidak punya rotasi).
    Ellipse {
        center: DVec2,
        radius_x: f64,
        radius_y: f64,
    },
    /// Urutan CCW.
    Rect {
        corners: [DVec2; 4],
    },
    Polyline {
        points: Vec<DVec2>,
        closed: bool,
    },
    Spline {
        points: Vec<DVec2>,
    },
}

/// Panjang lintasan minimum (mm) agar coretan dianggap gambar, bukan ketukan.
const MIN_PATH_LEN: f64 = 2.0;
/// Jumlah titik hasil resample.
const RESAMPLE_N: usize = 64;
/// Ambang "tertutup" relatif diagonal bbox.
const CLOSE_FRAC: f64 = 0.15;
/// Deviasi maksimum garis relatif panjang lintasan.
const LINE_DEV_FRAC: f64 = 0.03;
/// RMS galat radial relatif radius untuk lingkaran/busur.
const CIRCLE_RMS_FRAC: f64 = 0.05;
/// Cakupan sudut minimum lingkaran (derajat).
const CIRCLE_SPAN_DEG: f64 = 330.0;
/// RMS ternormalisasi maksimum elips.
const ELLIPSE_RMS: f64 = 0.06;
/// Rasio sumbu minimum agar disebut elips (di bawah ini = lingkaran).
const ELLIPSE_RATIO: f64 = 1.25;
/// Toleransi sudut siku persegi (derajat).
const RECT_ANGLE_TOL_DEG: f64 = 15.0;
/// Rotasi persegi yang diluruskan ke sumbu (derajat).
const RECT_SNAP_DEG: f64 = 7.0;
/// Epsilon Douglas–Peucker untuk sudut, relatif diagonal bbox.
const DP_CORNER_FRAC: f64 = 0.04;
/// Epsilon Douglas–Peucker untuk spline, relatif diagonal bbox.
const DP_SPLINE_FRAC: f64 = 0.01;
/// Cakupan sudut busur (derajat).
const ARC_SPAN_MIN_DEG: f64 = 30.0;
const ARC_SPAN_MAX_DEG: f64 = 330.0;

fn bbox(points: &[DVec2]) -> (DVec2, DVec2) {
    let mut min = DVec2::splat(f64::MAX);
    let mut max = DVec2::splat(f64::MIN);
    for p in points {
        min = min.min(*p);
        max = max.max(*p);
    }
    (min, max)
}

fn path_len(points: &[DVec2]) -> f64 {
    points.windows(2).map(|w| (w[1] - w[0]).length()).sum()
}

/// Buang titik berdempet lalu resample berjarak seragam.
fn preprocess(raw: &[DVec2]) -> Option<Vec<DVec2>> {
    if raw.len() < 4 {
        return None;
    }
    let (min, max) = bbox(raw);
    let d = (max - min).length();
    let min_gap = 0.002 * d;
    let mut pts: Vec<DVec2> = Vec::with_capacity(raw.len());
    for p in raw {
        if pts.last().is_none_or(|q: &DVec2| (*p - *q).length() > min_gap) {
            pts.push(*p);
        }
    }
    if pts.len() < 4 {
        return None;
    }
    let total = path_len(&pts);
    if total < MIN_PATH_LEN {
        return None;
    }
    let step = total / RESAMPLE_N as f64;
    let mut out = vec![pts[0]];
    let mut acc = 0.0;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = (b - a).length();
        if seg <= f64::EPSILON {
            continue;
        }
        let mut t = step - acc;
        while t <= seg {
            out.push(a + (b - a) * (t / seg));
            t += step;
        }
        acc = (acc + seg) % step;
    }
    if out
        .last()
        .is_some_and(|p| (*p - pts[pts.len() - 1]).length() > 1e-9)
    {
        out.push(pts[pts.len() - 1]);
    }
    (out.len() >= 4).then_some(out)
}

/// Total least squares: (titik pada garis, arah satuan).
fn fit_line(points: &[DVec2]) -> (DVec2, DVec2) {
    let n = points.len() as f64;
    let mean = points.iter().copied().sum::<DVec2>() / n;
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for p in points {
        let d = *p - mean;
        sxx += d.x * d.x;
        sxy += d.x * d.y;
        syy += d.y * d.y;
    }
    // Vektor eigen terbesar matriks kovarians 2×2.
    let theta = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    (mean, DVec2::new(theta.cos(), theta.sin()))
}

/// Fit lingkaran Kåsa + satu iterasi Gauss–Newton.
fn fit_circle(points: &[DVec2]) -> Option<(DVec2, f64)> {
    let n = points.len() as f64;
    if points.len() < 3 {
        return None;
    }
    let mean = points.iter().copied().sum::<DVec2>() / n;
    let (mut suu, mut suv, mut svv, mut suuu, mut svvv, mut suvv, mut svuu) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for p in points {
        let d = *p - mean;
        suu += d.x * d.x;
        suv += d.x * d.y;
        svv += d.y * d.y;
        suuu += d.x * d.x * d.x;
        svvv += d.y * d.y * d.y;
        suvv += d.x * d.y * d.y;
        svuu += d.y * d.x * d.x;
    }
    let det = suu * svv - suv * suv;
    if det.abs() < 1e-12 {
        return None;
    }
    let b1 = 0.5 * (suuu + suvv);
    let b2 = 0.5 * (svvv + svuu);
    let uc = (b1 * svv - b2 * suv) / det;
    let vc = (b2 * suu - b1 * suv) / det;
    let mut center = mean + DVec2::new(uc, vc);
    let mut radius = points.iter().map(|p| (*p - center).length()).sum::<f64>() / n;

    // Satu iterasi Gauss–Newton atas (cx, cy, r) dengan residual |p−c| − r.
    let (mut jtj, mut jtr) = ([[0.0f64; 3]; 3], [0.0f64; 3]);
    for p in points {
        let d = *p - center;
        let len = d.length();
        if len < 1e-12 {
            continue;
        }
        let j = [-d.x / len, -d.y / len, -1.0];
        let r = len - radius;
        for a in 0..3 {
            jtr[a] += j[a] * r;
            for b in 0..3 {
                jtj[a][b] += j[a] * j[b];
            }
        }
    }
    if let Some(delta) = solve3(jtj, jtr) {
        center -= DVec2::new(delta[0], delta[1]);
        radius -= delta[2];
    }
    (radius > 1e-9).then_some((center, radius.abs()))
}

/// Eliminasi Gauss 3×3; `None` bila singular.
fn solve3(mut a: [[f64; 3]; 3], mut b: [f64; 3]) -> Option<[f64; 3]> {
    for i in 0..3 {
        let mut pivot = i;
        for r in i + 1..3 {
            if a[r][i].abs() > a[pivot][i].abs() {
                pivot = r;
            }
        }
        if a[pivot][i].abs() < 1e-12 {
            return None;
        }
        a.swap(i, pivot);
        b.swap(i, pivot);
        for r in i + 1..3 {
            let f = a[r][i] / a[i][i];
            let row = a[i];
            for (c, v) in a[r].iter_mut().enumerate().skip(i) {
                *v -= f * row[c];
            }
            b[r] -= f * b[i];
        }
    }
    let mut x = [0.0; 3];
    for i in (0..3).rev() {
        let mut s = b[i];
        for c in i + 1..3 {
            s -= a[i][c] * x[c];
        }
        x[i] = s / a[i][i];
    }
    Some(x)
}

/// Sudut terurut kontinu (unwrapped) titik terhadap pusat.
fn unwrapped_angles(points: &[DVec2], center: DVec2) -> Vec<f64> {
    let tau = std::f64::consts::TAU;
    let mut out = Vec::with_capacity(points.len());
    let mut prev = 0.0;
    for (i, p) in points.iter().enumerate() {
        let d = *p - center;
        let mut a = d.y.atan2(d.x);
        if i > 0 {
            while a - prev > std::f64::consts::PI {
                a -= tau;
            }
            while prev - a > std::f64::consts::PI {
                a += tau;
            }
        }
        out.push(a);
        prev = a;
    }
    out
}

fn rms_radial(points: &[DVec2], center: DVec2, radius: f64) -> f64 {
    let n = points.len() as f64;
    (points
        .iter()
        .map(|p| {
            let e = (*p - center).length() - radius;
            e * e
        })
        .sum::<f64>()
        / n)
        .sqrt()
}

/// Douglas–Peucker.
fn simplify(points: &[DVec2], eps: f64) -> Vec<DVec2> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let (first, last) = (points[0], points[points.len() - 1]);
    let dir = last - first;
    let len = dir.length();
    let mut worst = (0usize, 0.0f64);
    for (i, p) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let d = if len < 1e-12 {
            (*p - first).length()
        } else {
            ((*p - first).perp_dot(dir) / len).abs()
        };
        if d > worst.1 {
            worst = (i, d);
        }
    }
    if worst.1 <= eps {
        return vec![first, last];
    }
    let mut left = simplify(&points[..=worst.0], eps);
    let right = simplify(&points[worst.0..], eps);
    left.pop();
    left.extend(right);
    left
}

/// Buang simpul yang nyaris lurus (belokan < `min_turn_deg`) dari poligon
/// tertutup — sisa noise di sisi panjang kadang lolos Douglas–Peucker.
fn drop_collinear(corners: &[DVec2], min_turn_deg: f64) -> Vec<DVec2> {
    let n = corners.len();
    if n < 4 {
        return corners.to_vec();
    }
    let mut out: Vec<DVec2> = Vec::with_capacity(n);
    for i in 0..n {
        let prev = *out.last().unwrap_or(&corners[(i + n - 1) % n]);
        let next = corners[(i + 1) % n];
        let a = (corners[i] - prev).normalize_or_zero();
        let b = (next - corners[i]).normalize_or_zero();
        let turn = a.dot(b).clamp(-1.0, 1.0).acos().to_degrees();
        if turn >= min_turn_deg {
            out.push(corners[i]);
        }
    }
    if out.len() < 3 {
        corners.to_vec()
    } else {
        out
    }
}

/// Empat simpul dengan belokan terkuat, tetap dalam urutan aslinya.
fn strongest_four(corners: &[DVec2]) -> Option<[DVec2; 4]> {
    let n = corners.len();
    if n < 4 {
        return None;
    }
    let angles = interior_angles(corners);
    let mut idx: Vec<usize> = (0..n).collect();
    // Belokan terkuat = sudut dalam terkecil.
    idx.sort_by(|a, b| angles[*a].total_cmp(&angles[*b]));
    idx.truncate(4);
    idx.sort_unstable();
    Some([
        corners[idx[0]],
        corners[idx[1]],
        corners[idx[2]],
        corners[idx[3]],
    ])
}

/// RMS jarak titik ke keliling poligon.
fn rms_to_polygon(points: &[DVec2], poly: &[DVec2]) -> f64 {
    let n = poly.len();
    let dist = |p: DVec2| {
        (0..n)
            .map(|i| {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                let ab = b - a;
                let t = if ab.length_squared() < 1e-12 {
                    0.0
                } else {
                    ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
                };
                (p - (a + ab * t)).length()
            })
            .fold(f64::MAX, f64::min)
    };
    (points.iter().map(|p| dist(*p).powi(2)).sum::<f64>() / points.len() as f64).sqrt()
}

/// RMS maksimum kecocokan persegi relatif diagonal bbox.
const RECT_FIT_FRAC: f64 = 0.05;

/// Belokan minimum agar sebuah simpul dianggap sudut sungguhan (derajat).
const MIN_TURN_DEG: f64 = 15.0;

/// Sudut dalam (derajat) di tiap simpul poligon tertutup.
fn interior_angles(corners: &[DVec2]) -> Vec<f64> {
    let n = corners.len();
    (0..n)
        .map(|i| {
            let prev = corners[(i + n - 1) % n];
            let next = corners[(i + 1) % n];
            let a = (prev - corners[i]).normalize_or_zero();
            let b = (next - corners[i]).normalize_or_zero();
            a.dot(b).clamp(-1.0, 1.0).acos().to_degrees()
        })
        .collect()
}

/// Persegi panjang berorientasi dari 4 sudut kasar: arah sisi rata-rata
/// (modulo 90°), lalu bentang pada sumbu lokal.
fn fit_rect(corners: &[DVec2; 4]) -> [DVec2; 4] {
    let (mut sin4, mut cos4) = (0.0, 0.0);
    for i in 0..4 {
        let e = corners[(i + 1) % 4] - corners[i];
        let a = e.y.atan2(e.x);
        sin4 += (4.0 * a).sin();
        cos4 += (4.0 * a).cos();
    }
    let mut angle = sin4.atan2(cos4) / 4.0;
    if angle.to_degrees().abs() <= RECT_SNAP_DEG {
        angle = 0.0;
    }
    let (ux, uy) = (
        DVec2::new(angle.cos(), angle.sin()),
        DVec2::new(-angle.sin(), angle.cos()),
    );
    let center = corners.iter().copied().sum::<DVec2>() / 4.0;
    let (mut su, mut sv) = ((f64::MAX, f64::MIN), (f64::MAX, f64::MIN));
    for c in corners {
        let d = *c - center;
        let (u, v) = (d.dot(ux), d.dot(uy));
        su = (su.0.min(u), su.1.max(u));
        sv = (sv.0.min(v), sv.1.max(v));
    }
    [
        center + ux * su.0 + uy * sv.0,
        center + ux * su.1 + uy * sv.0,
        center + ux * su.1 + uy * sv.1,
        center + ux * su.0 + uy * sv.1,
    ]
}

/// Kenali bentuk; `None` bila coretan terlalu pendek (ketukan).
pub fn recognize(stroke: &Stroke) -> Option<Recognized> {
    let pts = preprocess(&stroke.points)?;
    let (min, max) = bbox(&pts);
    let diag = (max - min).length();
    let total = path_len(&pts);
    let closed = (pts[0] - pts[pts.len() - 1]).length() < CLOSE_FRAC * diag;

    // 3. Garis.
    let (origin, dir) = fit_line(&pts);
    let max_dev = pts
        .iter()
        .map(|p| (*p - origin).perp_dot(dir).abs())
        .fold(0.0, f64::max);
    if max_dev < LINE_DEV_FRAC * total {
        let proj = |p: DVec2| origin + dir * (p - origin).dot(dir);
        return Some(Recognized::Line {
            a: proj(pts[0]),
            b: proj(pts[pts.len() - 1]),
        });
    }

    let circle = fit_circle(&pts);
    if closed {
        // 4a. Lingkaran.
        if let Some((center, radius)) = circle {
            let angles = unwrapped_angles(&pts, center);
            let span = (angles[angles.len() - 1] - angles[0]).abs().to_degrees();
            if rms_radial(&pts, center, radius) < CIRCLE_RMS_FRAC * radius && span > CIRCLE_SPAN_DEG
            {
                return Some(Recognized::Circle { center, radius });
            }
        }
        // 4b. Elips sejajar sumbu.
        let center = (min + max) * 0.5;
        let (rx, ry) = ((max.x - min.x) * 0.5, (max.y - min.y) * 0.5);
        if rx > 1e-9 && ry > 1e-9 {
            let rms = (pts
                .iter()
                .map(|p| {
                    let u = (p.x - center.x) / rx;
                    let v = (p.y - center.y) / ry;
                    let e = (u * u + v * v).sqrt() - 1.0;
                    e * e
                })
                .sum::<f64>()
                / pts.len() as f64)
                .sqrt();
            let ratio = (rx / ry).max(ry / rx);
            if rms < ELLIPSE_RMS && ratio > ELLIPSE_RATIO {
                return Some(Recognized::Ellipse {
                    center,
                    radius_x: rx,
                    radius_y: ry,
                });
            }
        }
        // 4c. Sudut.
        let mut corners = simplify(&pts, DP_CORNER_FRAC * diag);
        // Titik awal dan akhir coretan tertutup adalah simpul yang sama.
        if corners.len() >= 2
            && (corners[0] - corners[corners.len() - 1]).length() < CLOSE_FRAC * diag
        {
            corners.pop();
        }
        let corners = drop_collinear(&corners, MIN_TURN_DEG);
        // 4–6 simpul: ambil empat belokan terkuat (coretan bernoise kadang
        // menyisakan simpul tambahan di sisi panjang).
        if (4..=6).contains(&corners.len()) {
            if let Some(c) = strongest_four(&corners) {
                let angles = interior_angles(&c);
                if angles.iter().all(|a| (a - 90.0).abs() <= RECT_ANGLE_TOL_DEG) {
                    let mut rect = fit_rect(&c);
                    // Pastikan urutan CCW.
                    let area: f64 = (0..4).map(|i| rect[i].perp_dot(rect[(i + 1) % 4])).sum();
                    if area < 0.0 {
                        rect.reverse();
                    }
                    if rms_to_polygon(&pts, &rect) < RECT_FIT_FRAC * diag {
                        return Some(Recognized::Rect { corners: rect });
                    }
                }
            }
        }
        if (3..=8).contains(&corners.len()) {
            return Some(Recognized::Polyline {
                points: corners,
                closed: true,
            });
        }
    } else {
        // 5. Busur.
        if let Some((center, radius)) = circle {
            let angles = unwrapped_angles(&pts, center);
            let signed = angles[angles.len() - 1] - angles[0];
            let span = signed.abs().to_degrees();
            if rms_radial(&pts, center, radius) < CIRCLE_RMS_FRAC * radius
                && (ARC_SPAN_MIN_DEG..=ARC_SPAN_MAX_DEG).contains(&span)
            {
                // `Entity::Arc` berjalan CCW dari start ke end.
                let (start_angle, end_angle) = if signed >= 0.0 {
                    (angles[0], angles[angles.len() - 1])
                } else {
                    (angles[angles.len() - 1], angles[0])
                };
                let wrap = |a: f64| a.rem_euclid(std::f64::consts::TAU);
                return Some(Recognized::Arc {
                    center,
                    radius,
                    start_angle: wrap(start_angle),
                    end_angle: wrap(end_angle),
                });
            }
        }
        let corners = simplify(&pts, DP_CORNER_FRAC * diag);
        if (2..=9).contains(&corners.len()) {
            // Tiap segmen harus benar-benar lurus.
            let straight = corners.windows(2).all(|w| {
                let (a, b) = (w[0], w[1]);
                let seg_dir = (b - a).normalize_or_zero();
                let seg_len = (b - a).length();
                pts.iter()
                    .filter(|p| {
                        let t = (**p - a).dot(seg_dir);
                        t >= 0.0 && t <= seg_len
                    })
                    .all(|p| (*p - a).perp_dot(seg_dir).abs() < LINE_DEV_FRAC * total)
            });
            if straight {
                return Some(Recognized::Polyline {
                    points: corners,
                    closed: false,
                });
            }
        }
    }

    // 6. Sisanya spline.
    Some(Recognized::Spline {
        points: simplify(&pts, DP_SPLINE_FRAC * diag),
    })
}

/// Entitas sketch dari bentuk yang dikenali.
pub fn to_entities(r: &Recognized) -> Vec<Entity> {
    match r {
        Recognized::Line { a, b } => vec![Entity::line(*a, *b)],
        Recognized::Circle { center, radius } => vec![Entity::Circle {
            center: *center,
            radius: *radius,
            is_construction: false,
        }],
        Recognized::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => vec![Entity::arc(*center, *radius, *start_angle, *end_angle)],
        Recognized::Ellipse {
            center,
            radius_x,
            radius_y,
        } => vec![Entity::Ellipse {
            center: *center,
            radius_x: *radius_x,
            radius_y: *radius_y,
            is_construction: false,
        }],
        Recognized::Rect { corners } => (0..4)
            .map(|i| Entity::line(corners[i], corners[(i + 1) % 4]))
            .collect(),
        Recognized::Polyline { points, closed } => {
            let n = points.len();
            let last = if *closed { n } else { n.saturating_sub(1) };
            (0..last)
                .map(|i| Entity::line(points[i], points[(i + 1) % n]))
                .collect()
        }
        // Coretan tangan tidak punya definisi kurva asli — yang ada hanya
        // jejak titik yang ditangkap, jadi `exact` memang kosong.
        Recognized::Spline { points } => vec![Entity::spline(points.clone())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, TAU};

    /// LCG deterministik (konstanta Numerical Recipes).
    struct Lcg(u64);
    impl Lcg {
        fn next_f64(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
        }
        /// Noise seragam di [-amp, amp].
        fn noise(&mut self, amp: f64) -> DVec2 {
            DVec2::new(
                (self.next_f64() * 2.0 - 1.0) * amp,
                (self.next_f64() * 2.0 - 1.0) * amp,
            )
        }
    }

    enum Shape {
        Line,
        LineTilted3,
        Circle,
        Arc,
        Ellipse,
        Rect,
        RectTilted30,
        Triangle,
        Ess,
    }

    /// Coretan sintetis berukuran ±40 mm.
    fn synth(shape: &Shape, noise_mm: f64, n: usize, seed: u64) -> Stroke {
        let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let t = |i: usize| i as f64 / (n - 1) as f64;
        let mut pts: Vec<DVec2> = match shape {
            Shape::Line => (0..n)
                .map(|i| DVec2::new(-20.0 + 40.0 * t(i), 5.0))
                .collect(),
            Shape::LineTilted3 => (0..n)
                .map(|i| {
                    let a: f64 = 3f64.to_radians();
                    DVec2::new(-20.0, 0.0) + DVec2::new(a.cos(), a.sin()) * (40.0 * t(i))
                })
                .collect(),
            Shape::Circle => (0..n)
                .map(|i| {
                    let a = TAU * t(i);
                    DVec2::new(3.0, -2.0) + DVec2::new(a.cos(), a.sin()) * 18.0
                })
                .collect(),
            Shape::Arc => (0..n)
                .map(|i| {
                    let a = 0.3 + 2.2 * t(i);
                    DVec2::new(1.0, 1.0) + DVec2::new(a.cos(), a.sin()) * 16.0
                })
                .collect(),
            Shape::Ellipse => (0..n)
                .map(|i| {
                    let a = TAU * t(i);
                    DVec2::new(20.0 * a.cos(), 10.0 * a.sin())
                })
                .collect(),
            Shape::Rect | Shape::RectTilted30 => {
                let corners = [
                    DVec2::new(-18.0, -12.0),
                    DVec2::new(18.0, -12.0),
                    DVec2::new(18.0, 12.0),
                    DVec2::new(-18.0, 12.0),
                ];
                let rot: f64 = if matches!(shape, Shape::RectTilted30) {
                    30f64.to_radians()
                } else {
                    0.0
                };
                let m = |p: DVec2| {
                    DVec2::new(
                        p.x * rot.cos() - p.y * rot.sin(),
                        p.x * rot.sin() + p.y * rot.cos(),
                    )
                };
                let per = n / 4;
                let mut v = Vec::new();
                for e in 0..4 {
                    let (a, b) = (corners[e], corners[(e + 1) % 4]);
                    for i in 0..per {
                        v.push(m(a + (b - a) * (i as f64 / per as f64)));
                    }
                }
                v.push(m(corners[0]));
                v
            }
            Shape::Triangle => {
                let corners = [
                    DVec2::new(-18.0, -10.0),
                    DVec2::new(18.0, -10.0),
                    DVec2::new(0.0, 16.0),
                ];
                let per = n / 3;
                let mut v = Vec::new();
                for e in 0..3 {
                    let (a, b) = (corners[e], corners[(e + 1) % 3]);
                    for i in 0..per {
                        v.push(a + (b - a) * (i as f64 / per as f64));
                    }
                }
                v.push(corners[0]);
                v
            }
            // Huruf "S": dua setengah lingkaran berlawanan arah.
            Shape::Ess => (0..n)
                .map(|i| {
                    let s = t(i);
                    if s < 0.5 {
                        let a = PI * 1.5 - TAU * 0.5 * (s * 2.0);
                        DVec2::new(0.0, 9.0) + DVec2::new(a.cos(), a.sin()) * 9.0
                    } else {
                        let a = PI * 0.5 + TAU * 0.5 * ((s - 0.5) * 2.0);
                        DVec2::new(0.0, -9.0) + DVec2::new(a.cos(), a.sin()) * 9.0
                    }
                })
                .collect(),
        };
        for p in &mut pts {
            *p += rng.noise(noise_mm);
        }
        Stroke {
            points: pts,
            pressure: Vec::new(),
        }
    }

    fn rate(shape: Shape, noise: f64, ok: impl Fn(&Recognized) -> bool) -> f64 {
        let mut hits = 0;
        for seed in 1..=5u64 {
            let s = synth(&shape, noise, 120, seed);
            if recognize(&s).as_ref().is_some_and(&ok) {
                hits += 1;
            }
        }
        hits as f64 / 5.0
    }

    #[test]
    fn shapes_are_recognized_across_seeds_and_noise() {
        for noise in [0.2, 0.8] {
            assert!(
                rate(Shape::Line, noise, |r| matches!(r, Recognized::Line { .. })) >= 0.95,
                "garis, noise {noise}"
            );
            assert!(
                rate(Shape::Circle, noise, |r| matches!(
                    r,
                    Recognized::Circle { .. }
                )) >= 0.95,
                "lingkaran, noise {noise}"
            );
            assert!(
                rate(Shape::Arc, noise, |r| matches!(r, Recognized::Arc { .. })) >= 0.95,
                "busur, noise {noise}"
            );
            assert!(
                rate(Shape::Ellipse, noise, |r| matches!(
                    r,
                    Recognized::Ellipse { .. }
                )) >= 0.95,
                "elips, noise {noise}"
            );
            assert!(
                rate(Shape::Rect, noise, |r| matches!(r, Recognized::Rect { .. })) >= 0.95,
                "persegi, noise {noise}"
            );
            assert!(
                rate(Shape::Triangle, noise, |r| matches!(
                    r,
                    Recognized::Polyline { closed: true, .. }
                )) >= 0.95,
                "segitiga, noise {noise}"
            );
        }
    }

    #[test]
    fn tilted_line_is_not_straightened() {
        let s = synth(&Shape::LineTilted3, 0.2, 120, 7);
        let Some(Recognized::Line { a, b }) = recognize(&s) else {
            panic!("harus Line");
        };
        let deg = (b - a).y.atan2((b - a).x).to_degrees();
        assert!((deg - 3.0).abs() < 1.0, "sudut {deg}");
    }

    #[test]
    fn tilted_rect_stays_tilted() {
        let s = synth(&Shape::RectTilted30, 0.2, 160, 3);
        let Some(Recognized::Rect { corners }) = recognize(&s) else {
            panic!("harus Rect: {:?}", recognize(&s));
        };
        let e = corners[1] - corners[0];
        let deg = e.y.atan2(e.x).to_degrees().rem_euclid(90.0);
        assert!((deg - 30.0).abs() < 5.0, "sudut {deg}");
    }

    #[test]
    fn ess_is_a_spline() {
        let s = synth(&Shape::Ess, 0.2, 140, 11);
        assert!(
            matches!(recognize(&s), Some(Recognized::Spline { .. })),
            "{:?}",
            recognize(&s)
        );
    }

    #[test]
    fn tap_is_none() {
        let tap = Stroke {
            points: vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(0.05, 0.02),
                DVec2::new(0.04, 0.05),
                DVec2::new(0.01, 0.01),
            ],
            pressure: Vec::new(),
        };
        assert!(recognize(&tap).is_none());
        assert!(recognize(&Stroke::default()).is_none());
    }

    #[test]
    fn rect_becomes_four_lines_ccw() {
        let s = synth(&Shape::Rect, 0.2, 160, 2);
        let r = recognize(&s).unwrap();
        let ents = to_entities(&r);
        assert_eq!(ents.len(), 4);
        let Recognized::Rect { corners } = r else {
            panic!("harus Rect");
        };
        let area: f64 = (0..4)
            .map(|i| corners[i].perp_dot(corners[(i + 1) % 4]))
            .sum();
        assert!(area > 0.0, "urutan CCW");
    }

    #[test]
    fn arc_direction_follows_entity_convention() {
        let s = synth(&Shape::Arc, 0.2, 120, 5);
        let Some(Recognized::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        }) = recognize(&s)
        else {
            panic!("harus Arc");
        };
        let span = (end_angle - start_angle).rem_euclid(TAU);
        assert!(span > 0.5 && span < 6.0, "span {span}");
        // Titik tengah busur CCW harus dekat titik tengah coretan.
        let mid = center + DVec2::from_angle(start_angle + span * 0.5) * radius;
        let drawn_mid = s.points[s.points.len() / 2];
        assert!((mid - drawn_mid).length() < 3.0, "{mid} vs {drawn_mid}");
    }

}
