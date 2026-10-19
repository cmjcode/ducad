//! Jembatan tipe DUCAD ↔ SolidCraft (PLAN_MIGRASI_TRUCK 2.3): vektor,
//! bingkai sketsa, `Profile` → `Region2`, dan jalur sweep.
//!
//! Aturan konversi profil meniru backend lama (≤ 0.5.x) sejauh yang
//! berpengaruh ke geometri: titik 2D dipetakan `origin + u·x + v·y` (jadi
//! ikut panjang `u`/`v`), radius lingkaran/elips TIDAK diskalakan, dan
//! segmen yang lebih pendek dari 1e-4 mm dibuang dengan pesan galat yang
//! sama. Arah loop tidak penting: SolidCraft menormalkan orientasinya.

use anyhow::{bail, Result};
use glam::DVec3;
use solidcraft_geom::{Loop2, Plane, Region2, Seg2, Vec2, Vec3};
use solidcraft_kernel::PathSeg;

use crate::profile::{bezier_is_degenerate, PathSegment, Profile, ProfileSegment};

/// Segmen lebih pendek dari ini dibuang (sama dengan backend lama (≤ 0.5.x)).
const MIN_SEGMENT_MM: f64 = 1e-4;
/// Celah antar-segmen yang disambung otomatis setelah segmen degenerate
/// dibuang. SolidCraft menuntut tiap sambungan loop rapat dalam 1e-6 mm.
const MAX_GAP_MM: f64 = 1e-3;

pub(crate) fn v3(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

pub(crate) fn t3(p: (f64, f64, f64)) -> Vec3 {
    Vec3::new(p.0, p.1, p.2)
}

pub(crate) fn to_dv(v: Vec3) -> DVec3 {
    DVec3::new(v.x, v.y, v.z)
}

/// Bidang sketsa SolidCraft dari sumbu DUCAD, plus faktor skala koordinat 2D.
///
/// SolidCraft hanya menerima bingkai ortonormal. Sumbu DUCAD boleh
/// berpanjang sama tapi bukan satu (koordinat 2D lalu diskalakan, persis
/// seperti `origin + u·x + v·y` di backend lama (≤ 0.5.x)); sumbu yang tidak tegak
/// lurus atau berbeda panjang (pemetaan afin umum) ditolak.
pub(crate) fn sketch_plane(origin: [f64; 3], u: [f64; 3], v: [f64; 3]) -> Result<(Plane, f64)> {
    let (u, v) = (v3(u), v3(v));
    let (lu, lv) = (u.len(), v.len());
    if !(lu > 1e-12 && lv > 1e-12 && lu.is_finite() && lv.is_finite()) {
        bail!("sumbu bidang sketsa degenerate");
    }
    if u.dot(v).abs() > 1e-9 * lu * lv || (lu - lv).abs() > 1e-9 * lu.max(lv) {
        bail!("sumbu bidang sketsa harus tegak lurus dan sama panjang");
    }
    let plane =
        Plane::new(v3(origin), u, v).ok_or_else(|| anyhow::anyhow!("bidang sketsa tidak valid"))?;
    Ok((plane, lu))
}

/// Region (batas luar + lubang) dari profil; koordinat 2D dikali `scale`.
pub(crate) fn region(profile: &Profile, scale: f64) -> Result<Region2> {
    match profile {
        Profile::WithHoles { outer, holes } => {
            if outer.has_holes() || holes.iter().any(|h| h.has_holes()) {
                bail!("profil berlubang tidak boleh bersarang — pecah jadi beberapa profil");
            }
            Ok(Region2 {
                outer: loop2(outer, scale)?,
                holes: holes
                    .iter()
                    .map(|h| loop2(h, scale))
                    .collect::<Result<_>>()?,
            })
        }
        simple => Ok(Region2 {
            outer: loop2(simple, scale)?,
            holes: Vec::new(),
        }),
    }
}

/// Satu loop tertutup dari profil tanpa lubang.
pub(crate) fn loop2(profile: &Profile, scale: f64) -> Result<Loop2> {
    let p = |q: (f64, f64)| Vec2::new(q.0 * scale, q.1 * scale);
    match profile {
        Profile::Circle { center, radius } => {
            if *radius <= 0.0 {
                bail!("radius lingkaran profil harus > 0");
            }
            Ok(Loop2::circle(p(*center), *radius))
        }
        Profile::Ellipse {
            center,
            radius_x,
            radius_y,
        } => {
            if *radius_x <= 0.0 || *radius_y <= 0.0 {
                bail!("radius ellips profil harus > 0");
            }
            Ok(ellipse(p(*center), *radius_x, *radius_y))
        }
        Profile::WithHoles { .. } => {
            bail!("profil berlubang tidak bisa jadi wire tunggal — pakai build_face_on_plane")
        }
        Profile::Loop(segments) => {
            if segments.is_empty() {
                bail!("profil loop kosong");
            }
            let mut segs = Vec::with_capacity(segments.len());
            for s in segments {
                match *s {
                    ProfileSegment::Line { start, end } => {
                        let (a, b) = (p(start), p(end));
                        if a.dist(b) > MIN_SEGMENT_MM {
                            segs.push(Seg2::Line { a, b });
                        }
                    }
                    ProfileSegment::Arc { start, via, end } => {
                        let (a, m, b) = (p(start), p(via), p(end));
                        if a.dist(b) > MIN_SEGMENT_MM {
                            segs.push(arc_through(a, m, b).unwrap_or(Seg2::Line { a, b }));
                        }
                    }
                    ProfileSegment::Bezier { start, c1, c2, end } => {
                        let (p0, p1, p2, p3) = (p(start), p(c1), p(c2), p(end));
                        let d = |q: Vec2| DVec3::new(q.x, q.y, 0.0);
                        if !bezier_is_degenerate(d(p0), d(p1), d(p2), d(p3)) {
                            segs.push(Seg2::Cubic { p0, p1, p2, p3 });
                        }
                    }
                }
            }
            if segs.is_empty() {
                bail!("semua segmen loop degenerate");
            }
            Ok(Loop2 {
                segs: close_gaps(segs),
            })
        }
    }
}

/// Elips eksak: empat busur konik (Bézier kuadratik rasional, bobot √2/2),
/// sumbu sejajar x/y sketsa — sama dengan elips backend lama (≤ 0.5.x).
fn ellipse(c: Vec2, rx: f64, ry: f64) -> Loop2 {
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let at = |x: f64, y: f64| Vec2::new(c.x + x, c.y + y);
    let quarter = |a: Vec2, apex: Vec2, b: Vec2| Seg2::Conic { a, apex, b, w };
    Loop2 {
        segs: vec![
            quarter(at(rx, 0.0), at(rx, ry), at(0.0, ry)),
            quarter(at(0.0, ry), at(-rx, ry), at(-rx, 0.0)),
            quarter(at(-rx, 0.0), at(-rx, -ry), at(0.0, -ry)),
            quarter(at(0.0, -ry), at(rx, -ry), at(rx, 0.0)),
        ],
    }
}

/// Busur lingkaran dari `a` lewat `m` ke `b`; `None` bila ketiganya segaris.
pub(crate) fn arc_through(a: Vec2, m: Vec2, b: Vec2) -> Option<Seg2> {
    let (ab, am) = (b - a, m - a);
    let d = 2.0 * ab.cross(am);
    if d.abs() < 1e-12 * ab.len2().max(am.len2()) || !d.is_finite() {
        return None;
    }
    // Pusat lingkaran luar segitiga (a, m, b), relatif terhadap `a`.
    let (lb, lm) = (ab.len2(), am.len2());
    let off = Vec2::new(am.y * lb - ab.y * lm, ab.x * lm - am.x * lb) * (1.0 / d);
    let center = a + off;
    let radius = off.len();
    let ang = |q: Vec2| (q - center).angle();
    let tau = std::f64::consts::TAU;
    let a0 = ang(a);
    let ccw = (ang(b) - a0).rem_euclid(tau);
    let via = (ang(m) - a0).rem_euclid(tau);
    let sweep = if via < ccw { ccw } else { ccw - tau };
    (radius.is_finite() && radius > 1e-9).then_some(Seg2::Arc {
        center,
        radius,
        start: a0,
        sweep,
    })
}

fn set_start(s: &mut Seg2, p: Vec2) -> bool {
    match s {
        Seg2::Line { a, .. } | Seg2::Conic { a, .. } => *a = p,
        Seg2::Cubic { p0, .. } => *p0 = p,
        _ => return false,
    }
    true
}

fn set_end(s: &mut Seg2, p: Vec2) -> bool {
    match s {
        Seg2::Line { b, .. } | Seg2::Conic { b, .. } => *b = p,
        Seg2::Cubic { p3, .. } => *p3 = p,
        _ => return false,
    }
    true
}

/// Sambung celah kecil antar-segmen berurutan (sisa segmen degenerate yang
/// dibuang): ujung garis/Bézier digeser ke titik sambung; di antara dua
/// busur disisipkan garis pendek.
fn close_gaps(mut segs: Vec<Seg2>) -> Vec<Seg2> {
    let n = segs.len();
    if n < 2 {
        return segs;
    }
    let mut inserts: Vec<(usize, Seg2)> = Vec::new();
    for i in 0..n {
        let j = (i + n - 1) % n;
        let (prev_end, start) = (segs[j].end(), segs[i].start());
        let gap = prev_end.dist(start);
        if !(gap > 1e-9 && gap <= MAX_GAP_MM) {
            continue;
        }
        if set_start(&mut segs[i], prev_end) || set_end(&mut segs[j], start) {
            continue;
        }
        inserts.push((
            i,
            Seg2::Line {
                a: prev_end,
                b: start,
            },
        ));
    }
    for (i, s) in inserts.into_iter().rev() {
        segs.insert(i, s);
    }
    segs
}

/// Jalur sweep SolidCraft dari segmen DUCAD (polyline jadi garis-garis).
pub(crate) fn path(segments: &[PathSegment]) -> Result<Vec<PathSeg>> {
    if segments.is_empty() {
        bail!("jalur sweep kosong");
    }
    let mut out = Vec::new();
    for seg in segments {
        match seg {
            PathSegment::Line { start, end } => {
                let (a, b) = (v3(*start), v3(*end));
                if a.dist(b) > 1e-9 {
                    out.push(PathSeg::Line { a, b });
                }
            }
            PathSegment::Arc { start, via, end } => {
                let (a, m, b) = (v3(*start), v3(*via), v3(*end));
                match arc3d(a, m, b) {
                    Some(arc) => out.push(arc),
                    None if a.dist(b) > 1e-9 => out.push(PathSeg::Line { a, b }),
                    None => {}
                }
            }
            PathSegment::Polyline(pts) => {
                for w in pts.windows(2) {
                    let (a, b) = (v3(w[0]), v3(w[1]));
                    if a.dist(b) > 1e-9 {
                        out.push(PathSeg::Line { a, b });
                    }
                }
            }
        }
    }
    if out.is_empty() {
        bail!("tidak ada edge valid pada jalur sweep");
    }
    Ok(out)
}

/// Busur 3D dari `a` lewat `m` ke `b` sebagai `PathSeg::Arc` (putaran
/// tangan-kanan terhadap normal bidang a→m→b).
fn arc3d(a: Vec3, m: Vec3, b: Vec3) -> Option<PathSeg> {
    let (u, v) = (m - a, b - a);
    let w = u.cross(v);
    let w2 = w.len2();
    if w2 < 1e-18 {
        return None;
    }
    let center = a + (w.cross(u) * v.len2() + v.cross(w) * u.len2()) * (1.0 / (2.0 * w2));
    let axis = w.normalized()?;
    let (ra, rb) = (a - center, b - center);
    let angle = axis
        .dot(ra.cross(rb))
        .atan2(ra.dot(rb))
        .rem_euclid(std::f64::consts::TAU);
    let angle = if angle < 1e-12 {
        std::f64::consts::TAU
    } else {
        angle
    };
    Some(PathSeg::Arc {
        a,
        center,
        axis,
        angle,
    })
}

/// Region 2D (di `plane`) yang dibatasi edge logis face planar `f`: rantai
/// edge disusun jadi loop; loop dengan luas terbesar = batas luar, sisanya
/// lubang. Edge lingkaran jadi busur eksak, edge lain polyline.
pub(crate) fn face_region(
    shape: &crate::shape::KernelShape,
    f: usize,
    plane: &Plane,
) -> Result<Region2> {
    let topo = shape.topology();
    let Some(face) = topo.faces.get(f) else {
        bail!("face {f} tidak ada");
    };
    let to2 = |p: DVec3| plane.to_local(Vec3::new(p.x, p.y, p.z));
    // Potongan: (titik awal, titik akhir, segmen 2D searah awal→akhir).
    let mut pieces: Vec<(Vec2, Vec2, Vec<Seg2>)> = Vec::new();
    let mut loops: Vec<Vec<Seg2>> = Vec::new();
    for &e in &face.edges {
        let Some(edge) = topo.edges.get(e) else { continue };
        let pts: Vec<Vec2> = edge.points.iter().map(|p| to2(*p)).collect();
        if pts.len() < 2 {
            continue;
        }
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        let closed = a.dist(b) < 1e-6;
        if edge.kind == crate::topo::EdgeKind::Circle && pts.len() >= 3 {
            let n = pts.len() - 1;
            if closed {
                if let Some(Seg2::Arc { center, radius, .. }) =
                    arc_through(pts[0], pts[n / 3], pts[2 * n / 3])
                {
                    loops.push(Loop2::circle(center, radius).segs);
                    continue;
                }
            } else if let Some(arc) = arc_through(a, to2(edge.mid()), b) {
                pieces.push((a, b, vec![arc]));
                continue;
            }
        }
        let segs: Vec<Seg2> = pts
            .windows(2)
            .filter(|w| w[0].dist(w[1]) > 1e-9)
            .map(|w| Seg2::Line { a: w[0], b: w[1] })
            .collect();
        if closed {
            loops.push(segs);
        } else {
            pieces.push((a, b, segs));
        }
    }
    // Rangkai potongan terbuka jadi loop.
    const JOIN: f64 = 1e-4;
    while let Some((start, mut end, mut segs)) = pieces.pop() {
        loop {
            if end.dist(start) < JOIN {
                break;
            }
            let Some(k) = pieces
                .iter()
                .position(|(a, b, _)| a.dist(end) < JOIN || b.dist(end) < JOIN)
            else {
                bail!("batas face tidak membentuk loop tertutup");
            };
            let (a, b, more) = pieces.swap_remove(k);
            if a.dist(end) < JOIN {
                segs.extend(more);
                end = b;
            } else {
                segs.extend(more.into_iter().rev().map(|s| s.reversed()));
                end = a;
            }
        }
        loops.push(close_gaps(segs));
    }
    let mut loops: Vec<Loop2> = loops.into_iter().map(|segs| Loop2 { segs }).collect();
    if loops.is_empty() {
        bail!("face tidak punya batas");
    }
    let outer_at = loops
        .iter()
        .enumerate()
        .max_by(|x, y| x.1.signed_area().abs().total_cmp(&y.1.signed_area().abs()))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let outer = loops.swap_remove(outer_at);
    Ok(Region2 {
        outer,
        holes: loops,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_through_three_points_keeps_side() {
        // Setengah lingkaran atas r=1 dari (1,0) lewat (0,1) ke (-1,0): CCW +π.
        let Some(Seg2::Arc {
            center,
            radius,
            sweep,
            ..
        }) = arc_through(
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(-1.0, 0.0),
        )
        else {
            panic!("harus busur");
        };
        assert!(center.dist(Vec2::ZERO) < 1e-12);
        assert!((radius - 1.0).abs() < 1e-12);
        assert!((sweep - std::f64::consts::PI).abs() < 1e-12);
        // Lewat bawah: CW −π.
        let Some(Seg2::Arc { sweep, .. }) = arc_through(
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, -1.0),
            Vec2::new(-1.0, 0.0),
        ) else {
            panic!("harus busur");
        };
        assert!((sweep + std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn region_keeps_area() {
        let square = Profile::Loop(vec![
            ProfileSegment::Line {
                start: (0.0, 0.0),
                end: (10.0, 0.0),
            },
            ProfileSegment::Line {
                start: (10.0, 0.0),
                end: (10.0, 10.0),
            },
            ProfileSegment::Line {
                start: (10.0, 10.0),
                end: (0.0, 10.0),
            },
            ProfileSegment::Line {
                start: (0.0, 10.0),
                end: (0.0, 0.0),
            },
        ]);
        let holed = square.with_holes(vec![Profile::Circle {
            center: (5.0, 5.0),
            radius: 2.0,
        }]);
        let r = region(&holed, 1.0).unwrap();
        let want = 100.0 - std::f64::consts::PI * 4.0;
        assert!((r.area() - want).abs() < 1e-9, "luas {}", r.area());
        // Elips eksak: πab.
        let e = loop2(
            &Profile::Ellipse {
                center: (0.0, 0.0),
                radius_x: 3.0,
                radius_y: 2.0,
            },
            1.0,
        )
        .unwrap();
        assert!((e.signed_area().abs() - std::f64::consts::PI * 6.0).abs() < 1e-6);
    }

    #[test]
    fn degenerate_segment_gap_is_closed() {
        // Segmen 5e-5 mm dibuang; sisanya harus tetap tertutup.
        let tri = Profile::Loop(vec![
            ProfileSegment::Line {
                start: (0.0, 0.0),
                end: (10.0, 0.0),
            },
            ProfileSegment::Line {
                start: (10.0, 0.0),
                end: (10.0, 0.00005),
            },
            ProfileSegment::Line {
                start: (10.0, 0.00005),
                end: (0.0, 10.0),
            },
            ProfileSegment::Line {
                start: (0.0, 10.0),
                end: (0.0, 0.0),
            },
        ]);
        let l = loop2(&tri, 1.0).unwrap();
        assert_eq!(l.segs.len(), 3);
        assert!(l.is_closed(1e-12));
    }

    #[test]
    fn arc3d_quarter_turn() {
        let h = std::f64::consts::FRAC_1_SQRT_2;
        let Some(PathSeg::Arc { center, angle, .. }) = arc3d(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(h, h, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ) else {
            panic!("harus busur");
        };
        assert!(center.dist(Vec3::ZERO) < 1e-12);
        assert!((angle - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }
}
