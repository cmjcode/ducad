//! Konstruksi geometri 2D murni untuk mode gambar Fase 1B (Paritas 1.7–1.12):
//! lingkaran 2/3 titik dan tangen, busur tangen, persegi 3 titik dan
//! bersudut bulat, slot busur, poligon dari sisi, garis titik-tengah, dan
//! garis tak hingga.
//!
//! Semua fungsi di sini bebas efek samping dan mengembalikan `None` untuk
//! input degeneratif (titik kolinear, radius nol, NaN) — tidak pernah panic
//! (ADR 0009).

use glam::DVec2;
use std::f64::consts::{PI, TAU};

use crate::entity::Entity;
use crate::ops::circumcenter;

const EPS: f64 = 1e-9;

fn finite(p: DVec2) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// Bilangan hingga dan > EPS (NaN → false).
fn positive(v: f64) -> bool {
    v.is_finite() && v > EPS
}

/// Pasangan indeks (garis, busur) tangen dan (busur, busur) radius sama.
pub type RoundedRectPairs = ([(usize, usize); 8], [(usize, usize); 3]);

/// Kurva yang bisa disinggung lingkaran: garis (tak hingga, sama dengan
/// tafsiran solver) atau lingkaran/busur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TangentTarget {
    Line { a: DVec2, b: DVec2 },
    Circle { center: DVec2, radius: f64 },
}

impl TangentTarget {
    /// Target tangen dari entitas sketsa: Line, Circle, atau Arc.
    pub fn from_entity(entity: &Entity) -> Option<Self> {
        let t = match entity {
            Entity::Line { start, end, .. } => {
                if (*end - *start).length() < EPS {
                    return None;
                }
                TangentTarget::Line { a: *start, b: *end }
            }
            Entity::Circle { center, radius, .. } | Entity::Arc { center, radius, .. } => {
                if !positive(*radius) {
                    return None;
                }
                TangentTarget::Circle { center: *center, radius: *radius }
            }
            _ => return None,
        };
        t.is_finite().then_some(t)
    }

    fn is_finite(&self) -> bool {
        match self {
            TangentTarget::Line { a, b } => finite(*a) && finite(*b),
            TangentTarget::Circle { center, radius } => finite(*center) && radius.is_finite(),
        }
    }

    /// Radius lingkaran berpusat `c` yang tepat menyinggung target ini
    /// (lingkaran-lingkaran: singgung luar, sama dengan residual solver).
    pub fn radius_at(&self, c: DVec2) -> f64 {
        match self {
            TangentTarget::Line { a, b } => distance_to_infinite_line(c, *a, *b),
            TangentTarget::Circle { center, radius } => (c - *center).length() - radius,
        }
    }

    /// Residual tangen untuk lingkaran (`c`, `r`) — rumus yang sama dengan
    /// `Constraint::Tangent` di solver sehingga hasil konstruksi langsung
    /// memenuhi constraint-nya.
    pub fn residual(&self, c: DVec2, r: f64) -> f64 {
        self.radius_at(c) - r
    }
}

fn distance_to_infinite_line(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    let len = d.length();
    if len < EPS {
        return (p - a).length();
    }
    (d.x * (p.y - a.y) - d.y * (p.x - a.x)).abs() / len
}

/// Selesaikan sistem linear N×N (eliminasi Gauss, pivot parsial).
fn solve_linear<const N: usize>(mut m: [[f64; N]; N], mut v: [f64; N]) -> Option<[f64; N]> {
    for col in 0..N {
        let pivot = (col..N).max_by(|&i, &j| m[i][col].abs().total_cmp(&m[j][col].abs()))?;
        if m[pivot][col].abs() < 1e-14 {
            return None;
        }
        m.swap(col, pivot);
        v.swap(col, pivot);
        for row in (col + 1)..N {
            let f = m[row][col] / m[col][col];
            let pivot_row = m[col];
            for (k, cell) in m[row].iter_mut().enumerate().skip(col) {
                *cell -= f * pivot_row[k];
            }
            v[row] -= f * v[col];
        }
    }
    let mut x = [0.0; N];
    for row in (0..N).rev() {
        let mut s = v[row];
        for k in (row + 1)..N {
            s -= m[row][k] * x[k];
        }
        x[row] = s / m[row][row];
    }
    x.iter().all(|e| e.is_finite()).then_some(x)
}

/// Newton teredam dengan Jacobian numerik. `None` bila tidak konvergen.
fn newton<const N: usize>(f: impl Fn(&[f64; N]) -> [f64; N], mut x: [f64; N], scale: f64) -> Option<[f64; N]> {
    let norm = |r: &[f64; N]| r.iter().map(|e| e * e).sum::<f64>().sqrt();
    let tol = 1e-10 * scale.max(1.0);
    let mut r = f(&x);
    for _ in 0..100 {
        let rn = norm(&r);
        if !rn.is_finite() {
            return None;
        }
        if rn < tol {
            return Some(x);
        }
        let h = 1e-7 * scale.max(1.0);
        let mut jac = [[0.0; N]; N];
        for k in 0..N {
            let mut xp = x;
            xp[k] += h;
            let rp = f(&xp);
            for (i, row) in jac.iter_mut().enumerate() {
                row[k] = (rp[i] - r[i]) / h;
            }
        }
        let neg = r.map(|e| -e);
        let dx = solve_linear(jac, neg)?;
        // Redam: paruh langkah sampai residual turun.
        let mut step = 1.0;
        let mut improved = false;
        for _ in 0..30 {
            let mut xn = x;
            for k in 0..N {
                xn[k] += step * dx[k];
            }
            let rn_new = f(&xn);
            if norm(&rn_new) < rn {
                x = xn;
                r = rn_new;
                improved = true;
                break;
            }
            step *= 0.5;
        }
        if !improved {
            return None;
        }
    }
    (norm(&r) < tol).then_some(x)
}

/// Lingkaran 2 titik: `a`–`b` adalah diameter.
pub fn circle_two_point(a: DVec2, b: DVec2) -> Option<(DVec2, f64)> {
    let r = (b - a).length() * 0.5;
    (finite(a) && finite(b) && r > EPS && r.is_finite()).then(|| ((a + b) * 0.5, r))
}

/// Lingkaran melalui tiga titik. `None` bila kolinear/berimpit.
pub fn circle_three_point(a: DVec2, b: DVec2, c: DVec2) -> Option<(DVec2, f64)> {
    if !(finite(a) && finite(b) && finite(c)) {
        return None;
    }
    let center = circumcenter(a, b, c)?;
    let r = (a - center).length();
    (finite(center) && r > EPS && r.is_finite()).then_some((center, r))
}

/// Lingkaran menyinggung dua kurva, radius dari kursor: radius = rata-rata
/// radius singgung kedua kurva bila pusatnya di kursor, lalu pusat dicari
/// (Newton dari kursor) agar menyinggung keduanya tepat.
pub fn circle_tangent_two(t1: &TangentTarget, t2: &TangentTarget, cursor: DVec2) -> Option<(DVec2, f64)> {
    if !finite(cursor) || !t1.is_finite() || !t2.is_finite() {
        return None;
    }
    let (r1, r2) = (t1.radius_at(cursor), t2.radius_at(cursor));
    let r = (r1 + r2) * 0.5;
    if !positive(r) {
        return None;
    }
    let x = newton(
        |x: &[f64; 2]| {
            let c = DVec2::new(x[0], x[1]);
            [t1.residual(c, r), t2.residual(c, r)]
        },
        [cursor.x, cursor.y],
        r,
    )?;
    let c = DVec2::new(x[0], x[1]);
    finite(c).then_some((c, r))
}

/// Lingkaran menyinggung tiga kurva. `guess` = tebakan pusat (mis. rata-rata
/// titik pilih) — menentukan solusi mana yang diambil (incircle vs excircle).
pub fn circle_tangent_three(targets: [&TangentTarget; 3], guess: DVec2) -> Option<(DVec2, f64)> {
    if !finite(guess) || targets.iter().any(|t| !t.is_finite()) {
        return None;
    }
    let r0 = targets.iter().map(|t| t.radius_at(guess)).sum::<f64>() / 3.0;
    let r0 = if r0 > EPS { r0 } else { 1.0 };
    let x = newton(
        |x: &[f64; 3]| {
            let c = DVec2::new(x[0], x[1]);
            [targets[0].residual(c, x[2]), targets[1].residual(c, x[2]), targets[2].residual(c, x[2])]
        },
        [guess.x, guess.y, r0],
        r0,
    )?;
    let (c, r) = (DVec2::new(x[0], x[1]), x[2]);
    (finite(c) && r > EPS && r.is_finite()).then_some((c, r))
}

/// Ujung Line/Arc terdekat ke `p` (dalam `tol`) beserta arah tangen KELUAR
/// (arah lanjutan kurva melewati ujung itu, satuan).
pub fn endpoint_tangent(entity: &Entity, p: DVec2, tol: f64) -> Option<(DVec2, DVec2)> {
    let candidates: Vec<(DVec2, DVec2)> = match entity {
        Entity::Line { start, end, .. } => {
            let d = (*end - *start).normalize_or_zero();
            vec![(*end, d), (*start, -d)]
        }
        Entity::Arc { center, radius, start_angle, end_angle, .. } => {
            let at = |a: f64| *center + DVec2::new(a.cos(), a.sin()) * *radius;
            let ccw = |a: f64| DVec2::new(-a.sin(), a.cos());
            vec![(at(*end_angle), ccw(*end_angle)), (at(*start_angle), -ccw(*start_angle))]
        }
        _ => return None,
    };
    candidates
        .into_iter()
        .filter(|(q, d)| finite(*q) && d.length() > 0.5)
        .map(|(q, d)| ((q - p).length(), q, d))
        .filter(|(dist, _, _)| *dist <= tol)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, q, d)| (q, d))
}

/// Busur yang berawal di `start` dengan arah tangen `dir` dan berakhir di
/// `end`. `None` bila `end` segaris dengan arah tangen (hasilnya garis).
pub fn tangent_arc(start: DVec2, dir: DVec2, end: DVec2) -> Option<Entity> {
    if !(finite(start) && finite(dir) && finite(end)) {
        return None;
    }
    let u = dir.normalize_or_zero();
    if u == DVec2::ZERO {
        return None;
    }
    let n = DVec2::new(-u.y, u.x);
    let d = end - start;
    let nd = n.dot(d);
    if nd.abs() < EPS * d.length().max(1.0) {
        return None;
    }
    // Pusat di normal: |start + n t − end| = |t| → t = |d|² / (2 n·d).
    let t = d.length_squared() / (2.0 * nd);
    let center = start + n * t;
    let radius = t.abs();
    if !positive(radius) {
        return None;
    }
    let ang = |p: DVec2| (p - center).y.atan2((p - center).x);
    let (a_s, a_e) = (ang(start), ang(end));
    // Arah CCW di `start` searah `dir` → busur CCW start→end.
    let ccw_at_start = DVec2::new(-(start - center).y, (start - center).x);
    let (start_angle, end_angle) = if ccw_at_start.dot(u) > 0.0 { (a_s, a_e) } else { (a_e, a_s) };
    Some(Entity::Arc { center, radius, start_angle, end_angle, is_construction: false })
}

/// Residual tangen dua entitas menurut rumus solver; `None` bila pasangan
/// tidak didukung `Constraint::Tangent`. Dipakai untuk memastikan constraint
/// otomatis hanya dipasang bila geometrinya sudah memenuhinya (mis. busur
/// yang menyinggung DALAM busur lain tidak bisa dinyatakan solver).
pub fn tangent_residual(a: &Entity, b: &Entity) -> Option<f64> {
    let ta = TangentTarget::from_entity(a)?;
    let tb = TangentTarget::from_entity(b)?;
    match (ta, tb) {
        (TangentTarget::Line { .. }, TangentTarget::Line { .. }) => None,
        (t, TangentTarget::Circle { center, radius }) | (TangentTarget::Circle { center, radius }, t) => {
            Some(t.residual(center, radius))
        }
    }
}

/// Persegi panjang 3 titik (boleh miring): sisi `p0`→`p1`, lebar = jarak
/// tegak lurus `p2` ke sisi itu. Sudut berurutan p0, p1, p1+w, p0+w.
pub fn rect_three_point(p0: DVec2, p1: DVec2, p2: DVec2) -> Option<[DVec2; 4]> {
    if !(finite(p0) && finite(p1) && finite(p2)) {
        return None;
    }
    let d = p1 - p0;
    let len = d.length();
    if len < EPS {
        return None;
    }
    let n = DVec2::new(-d.y, d.x) / len;
    let w = (p2 - p0).dot(n);
    if w.abs() < EPS {
        return None;
    }
    Some([p0, p1, p1 + n * w, p0 + n * w])
}

/// Persegi bersudut bulat dari dua sudut berseberangan dan radius sudut.
/// Hasil: 4 Line (bawah, kanan, atas, kiri) lalu 4 Arc (kanan-bawah,
/// kanan-atas, kiri-atas, kiri-bawah). `None` bila radius ≤ 0 atau ≥
/// setengah sisi terpendek.
pub fn rounded_rect(a: DVec2, b: DVec2, radius: f64, is_construction: bool) -> Option<Vec<Entity>> {
    if !(finite(a) && finite(b) && positive(radius)) {
        return None;
    }
    let (min, max) = (a.min(b), a.max(b));
    let short = (max.x - min.x).min(max.y - min.y);
    if 2.0 * radius >= short - EPS {
        return None;
    }
    let r = radius;
    let line = |s: DVec2, e: DVec2| Entity::line(s, e).with_construction(is_construction);
    let arc = |c: DVec2, s: f64| Entity::Arc { center: c, radius: r, start_angle: s, end_angle: s + PI * 0.5, is_construction };
    Some(vec![
        line(DVec2::new(min.x + r, min.y), DVec2::new(max.x - r, min.y)),
        line(DVec2::new(max.x, min.y + r), DVec2::new(max.x, max.y - r)),
        line(DVec2::new(max.x - r, max.y), DVec2::new(min.x + r, max.y)),
        line(DVec2::new(min.x, max.y - r), DVec2::new(min.x, min.y + r)),
        arc(DVec2::new(max.x - r, min.y + r), -PI * 0.5),
        arc(DVec2::new(max.x - r, max.y - r), 0.0),
        arc(DVec2::new(min.x + r, max.y - r), PI * 0.5),
        arc(DVec2::new(min.x + r, min.y + r), -PI),
    ])
}

/// Pasangan indeks (garis, busur) yang saling tangen pada hasil
/// [`rounded_rect`], dan pasangan busur yang radiusnya sama.
pub fn rounded_rect_constraint_pairs() -> RoundedRectPairs {
    (
        [(0, 4), (1, 4), (1, 5), (2, 5), (2, 6), (3, 6), (3, 7), (0, 7)],
        [(4, 5), (4, 6), (4, 7)],
    )
}

/// Slot busur: busur tengah (`center`, `radius`, sudut CCW `start`→`end`)
/// dengan setengah lebar `half_width`. Hasil: busur luar, busur dalam, dan
/// dua busur ujung (setengah lingkaran). `None` bila lebar ≤ 0 atau ≥ radius.
pub fn arc_slot(
    center: DVec2,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    half_width: f64,
    is_construction: bool,
) -> Option<Vec<Entity>> {
    let ok = finite(center)
        && [radius, start_angle, end_angle, half_width].iter().all(|v| v.is_finite())
        && radius > EPS
        && half_width > EPS
        && half_width < radius - EPS;
    if !ok {
        return None;
    }
    let mut span = (end_angle - start_angle).rem_euclid(TAU);
    if span < 1e-9 {
        return None;
    }
    if span >= TAU - 1e-9 {
        span = TAU;
    }
    let (a0, a1) = (start_angle, start_angle + span);
    let at = |a: f64| center + DVec2::new(a.cos(), a.sin()) * radius;
    let arc = |c: DVec2, r: f64, s: f64, e: f64| Entity::Arc { center: c, radius: r, start_angle: s, end_angle: e, is_construction };
    Some(vec![
        arc(center, radius + half_width, a0, a1),
        arc(center, radius - half_width, a0, a1),
        // Ujung awal: dari sisi dalam memutar ke belakang menuju sisi luar.
        arc(at(a0), half_width, a0 + PI, a0 + TAU),
        // Ujung akhir: dari sisi luar memutar ke depan menuju sisi dalam.
        arc(at(a1), half_width, a1, a1 + PI),
    ])
}

/// Slot busur dari busur tengah `centerline` (Entity::Arc) dan titik lebar
/// `width_point`: setengah lebar = selisih jarak titik itu ke pusat dan radius.
pub fn arc_slot_from_arc(centerline: &Entity, width_point: DVec2, is_construction: bool) -> Option<Vec<Entity>> {
    let Entity::Arc { center, radius, start_angle, end_angle, .. } = centerline else {
        return None;
    };
    if !finite(width_point) {
        return None;
    }
    let hw = ((width_point - *center).length() - radius).abs();
    arc_slot(*center, *radius, *start_angle, *end_angle, hw, is_construction)
}

/// Titik sudut poligon beraturan N sisi yang didefinisikan satu sisinya
/// `p0`→`p1`; poligon berada di kiri sisi itu (urutan CCW).
pub fn edge_polygon_vertices(p0: DVec2, p1: DVec2, sides: usize) -> Option<Vec<DVec2>> {
    if !(finite(p0) && finite(p1)) {
        return None;
    }
    let sides = sides.clamp(3, 1024);
    let d = p1 - p0;
    let len = d.length();
    if len < EPS {
        return None;
    }
    let ext = TAU / sides as f64;
    let mut dir = d.y.atan2(d.x);
    let mut verts = Vec::with_capacity(sides);
    let mut p = p0;
    for _ in 0..sides {
        verts.push(p);
        p += DVec2::new(dir.cos(), dir.sin()) * len;
        dir += ext;
    }
    Some(verts)
}

/// Garis dari titik tengah `mid` ke satu ujung `end`: (ujung lawan, `end`).
pub fn line_from_midpoint(mid: DVec2, end: DVec2) -> Option<(DVec2, DVec2)> {
    if !(finite(mid) && finite(end)) || (end - mid).length() < EPS {
        return None;
    }
    Some((mid * 2.0 - end, end))
}

/// Ruas tampil untuk garis tak hingga lewat `a`–`b`: berpusat di proyeksi
/// `around` (mis. pusat pandangan) pada garis, sepanjang ±`half_extent`.
pub fn infinite_line_span(a: DVec2, b: DVec2, around: DVec2, half_extent: f64) -> Option<(DVec2, DVec2)> {
    if !(finite(a) && finite(b) && finite(around) && positive(half_extent)) {
        return None;
    }
    let u = (b - a).normalize_or_zero();
    if u == DVec2::ZERO {
        return None;
    }
    let foot = a + u * (around - a).dot(u);
    Some((foot - u * half_extent, foot + u * half_extent))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAN: DVec2 = DVec2::new(f64::NAN, 0.0);

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn two_point_circle_uses_diameter() {
        let (c, r) = circle_two_point(DVec2::ZERO, DVec2::new(4.0, 0.0)).unwrap();
        assert_eq!(c, DVec2::new(2.0, 0.0));
        assert!(near(r, 2.0));
        assert!(circle_two_point(DVec2::ONE, DVec2::ONE).is_none());
        assert!(circle_two_point(NAN, DVec2::ONE).is_none());
    }

    #[test]
    fn three_point_circle_and_collinear() {
        let (c, r) = circle_three_point(DVec2::new(1.0, 0.0), DVec2::new(0.0, 1.0), DVec2::new(-1.0, 0.0)).unwrap();
        assert!(c.length() < 1e-9 && near(r, 1.0));
        assert!(circle_three_point(DVec2::ZERO, DVec2::ONE, DVec2::splat(2.0)).is_none());
        assert!(circle_three_point(NAN, DVec2::ONE, DVec2::X).is_none());
    }

    #[test]
    fn two_tangent_circle_between_perpendicular_lines() {
        let x_axis = TangentTarget::Line { a: DVec2::ZERO, b: DVec2::X };
        let y_axis = TangentTarget::Line { a: DVec2::ZERO, b: DVec2::Y };
        let (c, r) = circle_tangent_two(&x_axis, &y_axis, DVec2::new(3.0, 5.0)).unwrap();
        assert!(near(r, 4.0));
        assert!(near(c.x, 4.0) && near(c.y, 4.0), "{c:?}");
        assert!(near(x_axis.residual(c, r), 0.0) && near(y_axis.residual(c, r), 0.0));
        // Kursor di garis → radius 0 → None.
        assert!(circle_tangent_two(&x_axis, &y_axis, DVec2::ZERO).is_none());
        assert!(circle_tangent_two(&x_axis, &y_axis, NAN).is_none());
    }

    #[test]
    fn two_tangent_circle_line_and_circle() {
        let ground = TangentTarget::Line { a: DVec2::ZERO, b: DVec2::X };
        let wheel = TangentTarget::Circle { center: DVec2::new(0.0, 5.0), radius: 5.0 };
        let (c, r) = circle_tangent_two(&ground, &wheel, DVec2::new(8.0, 2.0)).unwrap();
        assert!(near(ground.residual(c, r), 0.0) && near(wheel.residual(c, r), 0.0));
    }

    #[test]
    fn three_tangent_circle_is_incircle_of_triangle() {
        // Segitiga 3-4-5: inradius = (3+4-5)/2 = 1, pusat (1,1).
        let a = TangentTarget::Line { a: DVec2::ZERO, b: DVec2::new(4.0, 0.0) };
        let b = TangentTarget::Line { a: DVec2::ZERO, b: DVec2::new(0.0, 3.0) };
        let c = TangentTarget::Line { a: DVec2::new(4.0, 0.0), b: DVec2::new(0.0, 3.0) };
        let (center, r) = circle_tangent_three([&a, &b, &c], DVec2::new(1.3, 1.0)).unwrap();
        assert!(near(r, 1.0), "{r}");
        assert!(near(center.x, 1.0) && near(center.y, 1.0));
        // Tiga garis sejajar: tidak ada lingkaran yang menyinggung semua.
        let p1 = TangentTarget::Line { a: DVec2::ZERO, b: DVec2::X };
        let p2 = TangentTarget::Line { a: DVec2::Y, b: DVec2::new(1.0, 1.0) };
        let p3 = TangentTarget::Line { a: DVec2::new(0.0, 5.0), b: DVec2::new(1.0, 5.0) };
        assert!(circle_tangent_three([&p1, &p2, &p3], DVec2::new(0.0, 0.5)).is_none());
        assert!(circle_tangent_three([&a, &b, &c], NAN).is_none());
    }

    #[test]
    fn three_tangent_circle_with_circles() {
        let c1 = TangentTarget::Circle { center: DVec2::new(-5.0, 0.0), radius: 2.0 };
        let c2 = TangentTarget::Circle { center: DVec2::new(5.0, 0.0), radius: 2.0 };
        let l = TangentTarget::Line { a: DVec2::new(0.0, -6.0), b: DVec2::new(1.0, -6.0) };
        let (c, r) = circle_tangent_three([&c1, &c2, &l], DVec2::new(0.0, -2.0)).unwrap();
        for t in [c1, c2, l] {
            assert!(t.residual(c, r).abs() < 1e-6);
        }
    }

    #[test]
    fn tangent_target_rejects_degenerate_entities() {
        assert!(TangentTarget::from_entity(&Entity::line(DVec2::ONE, DVec2::ONE)).is_none());
        assert!(TangentTarget::from_entity(&Entity::circle(DVec2::ZERO, 0.0)).is_none());
        assert!(TangentTarget::from_entity(&Entity::circle(DVec2::ZERO, f64::NAN)).is_none());
        assert!(TangentTarget::from_entity(&Entity::ellipse(DVec2::ZERO, 1.0, 2.0)).is_none());
    }

    #[test]
    fn tangent_arc_continues_a_line_smoothly() {
        let line = Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0));
        let (start, dir) = endpoint_tangent(&line, DVec2::new(10.05, 0.0), 0.1).unwrap();
        assert_eq!(start, DVec2::new(10.0, 0.0));
        assert_eq!(dir, DVec2::X);
        let arc = tangent_arc(start, dir, DVec2::new(15.0, 5.0)).unwrap();
        let Entity::Arc { center, radius, .. } = arc else { panic!() };
        assert!(near(center.x, 10.0) && near(center.y, 5.0) && near(radius, 5.0));
        let ends = arc.endpoints();
        assert!(ends.iter().any(|p| (*p - start).length() < 1e-9));
        assert!(ends.iter().any(|p| (*p - DVec2::new(15.0, 5.0)).length() < 1e-9));
        assert!(tangent_residual(&line, &arc).unwrap().abs() < 1e-9);
        // Menurun (kanan bawah) → busur searah jarum jam, pusat di bawah.
        let down = tangent_arc(start, dir, DVec2::new(15.0, -5.0)).unwrap();
        let Entity::Arc { center, .. } = down else { panic!() };
        assert!(near(center.y, -5.0));
        // Segaris dengan tangen → bukan busur.
        assert!(tangent_arc(start, dir, DVec2::new(20.0, 0.0)).is_none());
        assert!(tangent_arc(start, DVec2::ZERO, DVec2::ONE).is_none());
        assert!(tangent_arc(NAN, dir, DVec2::ONE).is_none());
        assert!(endpoint_tangent(&line, DVec2::new(5.0, 0.0), 0.1).is_none());
    }

    #[test]
    fn endpoint_tangent_of_arc_points_outward() {
        let arc = Entity::Arc { center: DVec2::ZERO, radius: 1.0, start_angle: 0.0, end_angle: PI * 0.5, is_construction: false };
        let (p, d) = endpoint_tangent(&arc, DVec2::new(0.0, 1.0), 0.01).unwrap();
        assert!((p - DVec2::Y).length() < 1e-9 && (d - DVec2::new(-1.0, 0.0)).length() < 1e-9);
        let (p, d) = endpoint_tangent(&arc, DVec2::X, 0.01).unwrap();
        assert!((p - DVec2::X).length() < 1e-9 && (d - DVec2::new(0.0, -1.0)).length() < 1e-9);
    }

    #[test]
    fn three_point_rect_is_rotated() {
        let c = rect_three_point(DVec2::ZERO, DVec2::new(3.0, 3.0), DVec2::new(-1.0, 1.0)).unwrap();
        assert!(((c[2] - c[1]).length() - 2f64.sqrt()).abs() < 1e-9);
        assert!((c[1] - c[0]).dot(c[3] - c[0]).abs() < 1e-9);
        assert!(rect_three_point(DVec2::ZERO, DVec2::X, DVec2::new(5.0, 0.0)).is_none());
        assert!(rect_three_point(DVec2::ZERO, DVec2::ZERO, DVec2::ONE).is_none());
        assert!(rect_three_point(NAN, DVec2::X, DVec2::ONE).is_none());
    }

    #[test]
    fn rounded_rect_is_closed_and_tangent() {
        let ents = rounded_rect(DVec2::ZERO, DVec2::new(10.0, 6.0), 1.0, false).unwrap();
        assert_eq!(ents.len(), 8);
        // Setiap ujung dipakai tepat dua kali → loop tertutup.
        let ends: Vec<DVec2> = ents.iter().flat_map(|e| e.endpoints()).collect();
        for p in &ends {
            assert_eq!(ends.iter().filter(|q| (**q - *p).length() < 1e-9).count(), 2, "{p:?}");
        }
        let (tangents, _) = rounded_rect_constraint_pairs();
        for (l, a) in tangents {
            assert!(tangent_residual(&ents[l], &ents[a]).unwrap().abs() < 1e-9);
        }
        assert!(rounded_rect(DVec2::ZERO, DVec2::new(10.0, 6.0), 3.0, false).is_none());
        assert!(rounded_rect(DVec2::ZERO, DVec2::new(10.0, 6.0), 0.0, false).is_none());
        assert!(rounded_rect(DVec2::ZERO, DVec2::new(10.0, 6.0), f64::NAN, false).is_none());
    }

    #[test]
    fn arc_slot_has_two_concentric_arcs_and_closed_caps() {
        let ents = arc_slot(DVec2::ZERO, 10.0, 0.0, PI * 0.5, 1.0, false).unwrap();
        assert_eq!(ents.len(), 4);
        let ends: Vec<DVec2> = ents.iter().flat_map(|e| e.endpoints()).collect();
        for p in &ends {
            assert_eq!(ends.iter().filter(|q| (**q - *p).length() < 1e-9).count(), 2, "{p:?}");
        }
        // Ujung awal (0°) menonjol ke bawah (y < 0), ujung akhir ke kiri.
        let Some(Entity::Arc { center, radius, start_angle, end_angle, .. }) = ents.get(2).cloned() else { panic!() };
        let mid = (start_angle + end_angle) * 0.5;
        assert!((center + DVec2::new(mid.cos(), mid.sin()) * radius).y < -0.5);
        assert!(arc_slot(DVec2::ZERO, 10.0, 0.0, 1.0, 10.0, false).is_none());
        assert!(arc_slot(DVec2::ZERO, 10.0, 0.0, 1.0, 0.0, false).is_none());
        assert!(arc_slot(DVec2::ZERO, 10.0, 1.0, 1.0, 1.0, false).is_none());
        assert!(arc_slot(DVec2::ZERO, f64::NAN, 0.0, 1.0, 1.0, false).is_none());
        let centerline = Entity::Arc { center: DVec2::ZERO, radius: 10.0, start_angle: 0.0, end_angle: 1.0, is_construction: false };
        assert!(arc_slot_from_arc(&centerline, DVec2::new(12.0, 0.0), false).is_some());
        assert!(arc_slot_from_arc(&Entity::line(DVec2::ZERO, DVec2::X), DVec2::ONE, false).is_none());
    }

    #[test]
    fn edge_polygon_shares_the_clicked_edge() {
        let v = edge_polygon_vertices(DVec2::ZERO, DVec2::new(2.0, 0.0), 4).unwrap();
        assert_eq!(v.len(), 4);
        assert!((v[2] - DVec2::new(2.0, 2.0)).length() < 1e-9 && (v[3] - DVec2::new(0.0, 2.0)).length() < 1e-9);
        let hex = edge_polygon_vertices(DVec2::ZERO, DVec2::X, 6).unwrap();
        for i in 0..6 {
            assert!(near((hex[(i + 1) % 6] - hex[i]).length(), 1.0));
        }
        assert!(edge_polygon_vertices(DVec2::ONE, DVec2::ONE, 5).is_none());
        assert!(edge_polygon_vertices(NAN, DVec2::ONE, 5).is_none());
    }

    #[test]
    fn midpoint_and_infinite_line_helpers() {
        let (a, b) = line_from_midpoint(DVec2::new(1.0, 1.0), DVec2::new(3.0, 2.0)).unwrap();
        assert_eq!((a, b), (DVec2::new(-1.0, 0.0), DVec2::new(3.0, 2.0)));
        assert!(line_from_midpoint(DVec2::ONE, DVec2::ONE).is_none());
        assert!(line_from_midpoint(NAN, DVec2::ONE).is_none());
        let (s, e) = infinite_line_span(DVec2::ZERO, DVec2::X, DVec2::new(50.0, 7.0), 100.0).unwrap();
        assert_eq!((s, e), (DVec2::new(-50.0, 0.0), DVec2::new(150.0, 0.0)));
        assert!(infinite_line_span(DVec2::ZERO, DVec2::ZERO, DVec2::ONE, 10.0).is_none());
        assert!(infinite_line_span(DVec2::ZERO, DVec2::X, DVec2::ONE, f64::NAN).is_none());
    }
}
