//! Operasi boolean path, offset, simplify, dan kalkulasi area (M2.4).
//!
//! Logika murni berbasis i_overlay dan kurbo tanpa ketergantungan GUI.

use glam::DVec2;
use crate::entity::{PathSeg, Subpath};
use crate::style::{FillRule, LineCap, LineJoin, StrokeStyle};

use i_overlay::core::fill_rule::FillRule as OFillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;

/// Jenis operasi boolean pada dua rangkaian subpath vektor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Difference,
    Intersection,
    Xor,
}

/// Kesalahan yang dapat terjadi saat menjalankan operasi path.
#[derive(Debug, Clone, PartialEq)]
pub enum PathOpError {
    Empty,
    Degenerate(String),
    Overlay(String),
}

impl std::fmt::Display for PathOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "Hasil operasi path kosong"),
            Self::Degenerate(msg) => write!(f, "Geometri degenerasi: {msg}"),
            Self::Overlay(msg) => write!(f, "Kesalahan overlay boolean: {msg}"),
        }
    }
}

impl std::error::Error for PathOpError {}

fn to_contours(subs: &[Subpath], tol: f64) -> Vec<Vec<[f64; 2]>> {
    let mut contours = Vec::new();
    for sub in subs {
        let pts = sub.flatten(tol);
        if pts.len() < 3 {
            continue;
        }
        let mut contour: Vec<[f64; 2]> = pts.into_iter().map(|p| [p.x, p.y]).collect();
        // i_overlay auto-closes contours; if first and last are duplicate, drop last
        if contour.len() > 1 {
            let first = contour[0];
            let last = *contour.last().unwrap();
            if (first[0] - last[0]).abs() < 1e-9 && (first[1] - last[1]).abs() < 1e-9 {
                contour.pop();
            }
        }
        if contour.len() >= 3 {
            contours.push(contour);
        }
    }
    contours
}

fn shapes_to_subpaths(shapes: &[Vec<Vec<[f64; 2]>>]) -> Vec<Subpath> {
    let mut subpaths = Vec::new();
    for shape in shapes {
        for contour in shape {
            if contour.len() < 3 {
                continue;
            }
            let start = DVec2::new(contour[0][0], contour[0][1]);
            let mut segs = Vec::with_capacity(contour.len());
            for pt in &contour[1..] {
                segs.push(PathSeg::Line {
                    end: DVec2::new(pt[0], pt[1]),
                });
            }
            segs.push(PathSeg::Line { end: start });
            subpaths.push(Subpath {
                start,
                segs,
                closed: true,
            });
        }
    }
    subpaths
}

fn polygon_area(pts: &[[f64; 2]]) -> f64 {
    if pts.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    let n = pts.len();
    for i in 0..n {
        let j = (i + 1) % n;
        sum += pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1];
    }
    sum * 0.5
}

/// Menjalankan operasi boolean (Union, Difference, Intersection, Xor) pada subpath.
/// Orientasi kontur luar CCW (positif) dan lubang CW (negatif).
pub fn boolean(
    a: &[Subpath],
    b: &[Subpath],
    op: BoolOp,
    rule: FillRule,
    tol: f64,
) -> Result<Vec<Subpath>, PathOpError> {
    let tol = tol.max(1e-4);
    let subj = to_contours(a, tol);
    let clip = to_contours(b, tol);

    let overlay_rule = match op {
        BoolOp::Union => OverlayRule::Union,
        BoolOp::Difference => OverlayRule::Difference,
        BoolOp::Intersection => OverlayRule::Intersect,
        BoolOp::Xor => OverlayRule::Xor,
    };

    let ofill = match rule {
        FillRule::EvenOdd => OFillRule::EvenOdd,
        FillRule::NonZero => OFillRule::NonZero,
    };

    if subj.is_empty() && clip.is_empty() {
        return Err(PathOpError::Empty);
    }

    let shapes = subj.overlay(&clip, overlay_rule, ofill);
    let subpaths = shapes_to_subpaths(&shapes);

    if subpaths.is_empty() {
        return Err(PathOpError::Empty);
    }

    Ok(subpaths)
}

/// Offset path sebesar ± d (positif = membesar ke luar).
/// Untuk subpath terbuka berfungsi sebagai "stroke to path" jika `cap` diberikan.
pub fn offset(
    subs: &[Subpath],
    d: f64,
    join: LineJoin,
    cap: Option<LineCap>,
    tol: f64,
) -> Result<Vec<Subpath>, PathOpError> {
    if subs.is_empty() {
        return Err(PathOpError::Empty);
    }
    if d.abs() < 1e-9 {
        return Ok(subs.to_vec());
    }

    let tol = tol.max(1e-4);
    let mut out = Vec::new();

    let kjoin = match join {
        LineJoin::Miter => kurbo::Join::Miter,
        LineJoin::Round => kurbo::Join::Round,
        LineJoin::Bevel => kurbo::Join::Bevel,
    };

    for sub in subs {
        if !sub.closed {
            if let Some(c) = cap {
                let stroke_style = StrokeStyle {
                    paint: crate::style::Paint::Solid(crate::style::Rgba([0.0, 0.0, 0.0, 1.0])),
                    width_mm: 2.0 * d.abs(),
                    dash: Vec::new(),
                    cap: c,
                    join,
                };
                let stroked = stroke_to_path(std::slice::from_ref(sub), &stroke_style, tol)?;
                out.extend(stroked);
            }
        } else {
            let kpath = sub.to_kurbo();
            let bez = kurbo::expand_path(kpath, kurbo::Diagonal2::new(d, d), kjoin, 4.0, tol);
            let expanded_subs = Subpath::from_kurbo(&bez);
            out.extend(expanded_subs);
        }
    }

    if out.is_empty() {
        return Err(PathOpError::Empty);
    }
    Ok(out)
}

/// Mengonversi stroke menjadi objek path tertutup.
pub fn stroke_to_path(
    subs: &[Subpath],
    stroke: &StrokeStyle,
    tol: f64,
) -> Result<Vec<Subpath>, PathOpError> {
    if subs.is_empty() {
        return Err(PathOpError::Empty);
    }
    if stroke.width_mm <= 0.0 {
        return Err(PathOpError::Degenerate("Lebar stroke <= 0".into()));
    }

    let tol = tol.max(1e-4);
    let kjoin = match stroke.join {
        LineJoin::Miter => kurbo::Join::Miter,
        LineJoin::Round => kurbo::Join::Round,
        LineJoin::Bevel => kurbo::Join::Bevel,
    };
    let kcap = match stroke.cap {
        LineCap::Butt => kurbo::Cap::Butt,
        LineCap::Round => kurbo::Cap::Round,
        LineCap::Square => kurbo::Cap::Square,
    };

    let mut kstroke = kurbo::Stroke::new(stroke.width_mm);
    kstroke.join = kjoin;
    kstroke.start_cap = kcap;
    kstroke.end_cap = kcap;
    kstroke.miter_limit = 4.0;
    if !stroke.dash.is_empty() {
        kstroke.dash_pattern = stroke.dash.iter().copied().collect();
    }

    let mut out = Vec::new();
    for sub in subs {
        let kpath = sub.to_kurbo();
        let stroked_bez = kurbo::stroke(kpath, &kstroke, &kurbo::StrokeOpts::default(), tol);
        let res_subs = Subpath::from_kurbo(&stroked_bez);
        for mut s in res_subs {
            s.closed = true;
            out.push(s);
        }
    }

    if out.is_empty() {
        return Err(PathOpError::Empty);
    }
    Ok(out)
}

/// Menyederhanakan subpath untuk mengurangi jumlah node dengan galat maksimal ≤ tol.
pub fn simplify(sub: &Subpath, tol: f64) -> Subpath {
    let tol = tol.max(1e-4);
    let kpath = sub.to_kurbo();
    let simplified_bez = kurbo::simplify::simplify_bezpath(
        kpath,
        tol,
        &kurbo::simplify::SimplifyOptions::default(),
    );
    let mut subs = Subpath::from_kurbo(&simplified_bez);
    if let Some(mut s) = subs.pop() {
        s.closed = sub.closed;
        s
    } else {
        sub.clone()
    }
}

/// Menghitung luas bertanda dari rangkaian subpath berdasarkan `FillRule`.
pub fn area(subs: &[Subpath], rule: FillRule) -> f64 {
    if subs.is_empty() {
        return 0.0;
    }
    let tol = 0.01;
    let contours = to_contours(subs, tol);
    if contours.is_empty() {
        return 0.0;
    }
    let ofill = match rule {
        FillRule::EvenOdd => OFillRule::EvenOdd,
        FillRule::NonZero => OFillRule::NonZero,
    };

    let empty_clip: Vec<Vec<[f64; 2]>> = Vec::new();
    let shapes = contours.overlay(&empty_clip, OverlayRule::Union, ofill);
    let mut total = 0.0;
    for shape in &shapes {
        for contour in shape {
            total += polygon_area(contour);
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_edit::{shape_to_path_circle, shape_to_path_rect};

    #[test]
    fn union_two_overlapping_squares_area() {
        let s1 = shape_to_path_rect(DVec2::new(0.0, 0.0), DVec2::new(10.0, 10.0));
        let s2 = shape_to_path_rect(DVec2::new(5.0, 0.0), DVec2::new(15.0, 10.0));
        let u = boolean(&[s1], &[s2], BoolOp::Union, FillRule::NonZero, 0.01)
            .expect("union should succeed");
        let a = area(&u, FillRule::NonZero);
        assert!((a - 150.0).abs() < 1e-3, "Expected 150.0, got {a}");
    }

    #[test]
    fn difference_creates_hole_subpath_cw() {
        let big = shape_to_path_rect(DVec2::new(0.0, 0.0), DVec2::new(20.0, 20.0));
        let small = shape_to_path_rect(DVec2::new(5.0, 5.0), DVec2::new(15.0, 15.0));
        let diff = boolean(&[big], &[small], BoolOp::Difference, FillRule::NonZero, 0.01)
            .expect("difference should succeed");

        let pos_areas = diff.iter().filter(|s| s.signed_area() > 0.0).count();
        let neg_areas = diff.iter().filter(|s| s.signed_area() < 0.0).count();
        assert_eq!(pos_areas, 1, "Must have 1 CCW outer boundary");
        assert_eq!(neg_areas, 1, "Must have 1 CW hole boundary");

        let a = area(&diff, FillRule::NonZero);
        assert!((a - (400.0 - 100.0)).abs() < 1e-3, "Expected 300.0, got {a}");
    }

    #[test]
    fn intersection_disjoint_is_empty_err() {
        let s1 = shape_to_path_rect(DVec2::new(0.0, 0.0), DVec2::new(10.0, 10.0));
        let s2 = shape_to_path_rect(DVec2::new(20.0, 20.0), DVec2::new(30.0, 30.0));
        let res = boolean(&[s1], &[s2], BoolOp::Intersection, FillRule::NonZero, 0.01);
        assert_eq!(res, Err(PathOpError::Empty));
    }

    #[test]
    fn offset_circle_radius_grows_by_d() {
        let c = shape_to_path_circle(DVec2::ZERO, 50.0);
        let off = offset(&[c], 10.0, LineJoin::Round, None, 0.01).expect("offset should succeed");
        let a = area(&off, FillRule::NonZero);
        let expected = std::f64::consts::PI * 60.0 * 60.0;
        assert!(
            (a - expected).abs() < 50.0,
            "Expected area ~ {expected}, got {a}"
        );
    }

    #[test]
    fn stroke_to_path_area_equals_length_times_width() {
        let line = Subpath {
            start: DVec2::ZERO,
            segs: vec![PathSeg::Line {
                end: DVec2::new(100.0, 0.0),
            }],
            closed: false,
        };
        let style = StrokeStyle {
            paint: crate::style::Paint::Solid(crate::style::Rgba([0.0, 0.0, 0.0, 1.0])),
            width_mm: 10.0,
            dash: Vec::new(),
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        };
        let stroked = stroke_to_path(&[line], &style, 0.01).expect("stroke_to_path should succeed");
        let a = area(&stroked, FillRule::NonZero);
        assert!((a - 1000.0).abs() < 1e-2, "Expected area 1000.0, got {a}");
    }

    #[test]
    fn simplify_keeps_error_within_tol() {
        let c = shape_to_path_circle(DVec2::ZERO, 50.0);
        let tol = 0.5;
        let simplified = simplify(&c, tol);
        assert!(simplified.segs.len() <= c.segs.len());
        // Periksa galat titik-titik sampel
        let pts_orig = c.flatten(0.01);
        let (_, _, _, max_dist) = pts_orig.iter().fold((0, 0.0, DVec2::ZERO, 0.0f64), |acc, &pt| {
            let (_, _, _, dist) = crate::path_edit::closest_point(&simplified, pt);
            if dist > acc.3 {
                (0, 0.0, pt, dist)
            } else {
                acc
            }
        });
        assert!(
            max_dist <= tol * 2.0,
            "Max deviation {} exceeded tol {}",
            max_dist,
            tol
        );
    }

    #[test]
    fn boolean_is_deterministic() {
        let s1 = shape_to_path_rect(DVec2::new(0.0, 0.0), DVec2::new(10.0, 10.0));
        let s2 = shape_to_path_rect(DVec2::new(5.0, 5.0), DVec2::new(15.0, 15.0));
        let first = boolean(std::slice::from_ref(&s1), std::slice::from_ref(&s2), BoolOp::Union, FillRule::NonZero, 0.01)
            .expect("first run");
        for _ in 0..5 {
            let next = boolean(std::slice::from_ref(&s1), std::slice::from_ref(&s2), BoolOp::Union, FillRule::NonZero, 0.01)
                .expect("subsequent run");
            assert_eq!(first, next, "Boolean must produce identical subpaths across runs");
        }
    }

    proptest::proptest! {
        #[test]
        fn xor_equals_union_minus_intersection_area(
            dx in 2.0f64..8.0f64,
            dy in 2.0f64..8.0f64,
        ) {
            let s1 = shape_to_path_rect(DVec2::new(0.0, 0.0), DVec2::new(10.0, 10.0));
            let s2 = shape_to_path_rect(DVec2::new(dx, dy), DVec2::new(dx + 10.0, dy + 10.0));

            let u = boolean(std::slice::from_ref(&s1), std::slice::from_ref(&s2), BoolOp::Union, FillRule::NonZero, 0.01).unwrap();
            let inter = boolean(std::slice::from_ref(&s1), std::slice::from_ref(&s2), BoolOp::Intersection, FillRule::NonZero, 0.01).unwrap();
            let xor = boolean(std::slice::from_ref(&s1), std::slice::from_ref(&s2), BoolOp::Xor, FillRule::NonZero, 0.01).unwrap();

            let a_u = area(&u, FillRule::NonZero);
            let a_inter = area(&inter, FillRule::NonZero);
            let a_xor = area(&xor, FillRule::NonZero);

            let diff = (a_xor - (a_u - a_inter)).abs();
            let rel = diff / a_u;
            proptest::prop_assert!(rel <= 1e-4, "XOR area {} should equal Union {} - Inter {}, diff = {}, rel = {}", a_xor, a_u, a_inter, diff, rel);
        }
    }
}
