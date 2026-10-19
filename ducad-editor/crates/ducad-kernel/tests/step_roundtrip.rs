//! PLAN_MIGRASI_TRUCK F4.1 — roundtrip STEP: 20 bentuk khas DUCAD (primitif,
//! boolean, revolve, loft, fillet/chamfer, transformasi, multi-lump) ditulis
//! ke STEP lalu dibaca kembali. Volume harus sama dan jumlah face logis
//! tidak berubah. Hanya API publik.
//!
//! Toleransi volume: volume dihitung dari tesselasi halus, ±1e-4 per pengukuran (PLAN §3.1), jadi selisih dua
//! pengukuran dibatasi 2e-4. Bentuk dengan volume analitik juga dicek
//! terhadap nilai sebenarnya, supaya galat geometri tidak tersamar.

use ducad_kernel::csg::{loft_profiles, revolve_profile};
use ducad_kernel::modify::{chamfer_all, fillet_all, make_filleted_box};
use ducad_kernel::shape::{rotate_shape, translate_shape};
use ducad_kernel::{
    enumerate_faces, extrude_profile, intersect, make_box, make_cone, make_cylinder, make_sphere,
    subtract, union, write_step_compound, KernelShape, Profile, ProfileSegment,
};

fn rect_at(x0: f64, w: f64, h: f64) -> Profile {
    let p = [(x0, 0.0), (x0 + w, 0.0), (x0 + w, h), (x0, h)];
    Profile::Loop(
        (0..4)
            .map(|i| ProfileSegment::Line {
                start: p[i],
                end: p[(i + 1) % 4],
            })
            .collect(),
    )
}

fn at(s: KernelShape, x: f64, y: f64, z: f64) -> KernelShape {
    translate_shape(&s, x, y, z).unwrap()
}

fn plate_with_holes() -> KernelShape {
    let mut s = make_box(60.0, 40.0, 8.0, false).unwrap();
    for (x, y) in [(10.0, 10.0), (50.0, 10.0), (10.0, 30.0), (50.0, 30.0)] {
        s = subtract(&s, &at(make_cylinder(3.0, 20.0).unwrap(), x, y, -5.0)).unwrap();
    }
    s
}

fn cases() -> Vec<(&'static str, KernelShape)> {
    vec![
        ("box", make_box(40.0, 30.0, 20.0, false).unwrap()),
        ("box_centered", make_box(10.0, 10.0, 10.0, true).unwrap()),
        ("cylinder", make_cylinder(12.0, 30.0).unwrap()),
        ("sphere", make_sphere(15.0).unwrap()),
        ("cone", make_cone(10.0, 4.0, 20.0).unwrap()),
        (
            "tube",
            subtract(
                &make_cylinder(20.0, 40.0).unwrap(),
                &at(make_cylinder(15.0, 50.0).unwrap(), 0.0, 0.0, -5.0),
            )
            .unwrap(),
        ),
        ("plate_with_holes", plate_with_holes()),
        (
            "l_bracket",
            union(
                &make_box(50.0, 30.0, 6.0, false).unwrap(),
                &make_box(6.0, 30.0, 40.0, false).unwrap(),
            )
            .unwrap(),
        ),
        (
            "boss_on_plate",
            union(
                &make_box(40.0, 40.0, 5.0, false).unwrap(),
                &at(make_cylinder(8.0, 15.0).unwrap(), 20.0, 20.0, 5.0),
            )
            .unwrap(),
        ),
        (
            "intersect_lens",
            intersect(
                &make_sphere(10.0).unwrap(),
                &at(make_sphere(10.0).unwrap(), 8.0, 0.0, 0.0),
            )
            .unwrap(),
        ),
        (
            "revolve_ring",
            revolve_profile(&rect_at(10.0, 5.0, 10.0), (0.0, 0.0), (0.0, 1.0), None).unwrap(),
        ),
        (
            "revolve_partial",
            revolve_profile(&rect_at(10.0, 5.0, 10.0), (0.0, 0.0), (0.0, 1.0), Some(120.0)).unwrap(),
        ),
        (
            "extrude_circle",
            extrude_profile(
                &Profile::Circle {
                    center: (5.0, 5.0),
                    radius: 7.5,
                },
                12.0,
            )
            .unwrap(),
        ),
        (
            "loft_square_circle",
            loft_profiles(
                &rect_at(0.0, 20.0, 20.0),
                &Profile::Circle {
                    center: (10.0, 10.0),
                    radius: 6.0,
                },
                25.0,
            )
            .unwrap(),
        ),
        ("filleted_box", make_filleted_box(40.0, 30.0, 20.0, 3.0).unwrap()),
        ("fillet_all", fillet_all(&make_box(30.0, 30.0, 30.0, false).unwrap(), 2.0).unwrap()),
        ("chamfer_all", chamfer_all(&make_box(30.0, 20.0, 10.0, false).unwrap(), 1.5).unwrap()),
        (
            "rotated_box",
            rotate_shape(
                &make_box(20.0, 10.0, 5.0, false).unwrap(),
                (0.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                0.6,
            )
            .unwrap(),
        ),
        (
            "far_from_origin",
            at(make_cylinder(5.0, 10.0).unwrap(), 1000.0, -500.0, 250.0),
        ),
        (
            "two_lumps",
            union(
                &make_box(10.0, 10.0, 10.0, false).unwrap(),
                &at(make_box(10.0, 10.0, 10.0, false).unwrap(), 30.0, 0.0, 0.0),
            )
            .unwrap(),
        ),
    ]
}

const VOL_TOL: f64 = 2e-4;
// Truck: volume dari tesselasi (poligon tersirat), ±5e-4 seperti checks (PLAN §3.1, R5).
const EXACT_TOL: f64 = 5e-4;

/// Volume analitik bentuk yang punya rumus tertutup.
fn exact_volume(name: &str) -> Option<f64> {
    use std::f64::consts::PI;
    Some(match name {
        "box" => 40.0 * 30.0 * 20.0,
        "cylinder" => PI * 144.0 * 30.0,
        "sphere" => 4.0 / 3.0 * PI * 15f64.powi(3),
        "cone" => PI * 20.0 / 3.0 * (100.0 + 40.0 + 16.0),
        "tube" => PI * (400.0 - 225.0) * 40.0,
        "plate_with_holes" => 60.0 * 40.0 * 8.0 - 4.0 * PI * 9.0 * 8.0,
        "revolve_ring" => PI * (225.0 - 100.0) * 10.0,
        "revolve_partial" => PI * (225.0 - 100.0) * 10.0 / 3.0,
        "two_lumps" => 2000.0,
        _ => return None,
    })
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-12)
}

#[test]
fn step_roundtrip_preserves_volume_and_faces() {
    let cases = cases();
    assert_eq!(cases.len(), 20);
    let mut failures = Vec::new();
    for (name, shape) in &cases {
        let v0 = shape.volume();
        let f0 = enumerate_faces(shape).len();
        let text = match shape.to_step_string() {
            Ok(t) => t,
            Err(e) => {
                failures.push(format!("{name}: tulis STEP gagal: {e:#}"));
                continue;
            }
        };
        let back = match KernelShape::from_step_string(&text) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{name}: baca STEP gagal: {e:#}"));
                continue;
            }
        };
        let (v1, f1) = (back.volume(), enumerate_faces(&back).len());
        if rel(v1, v0) > VOL_TOL {
            failures.push(format!("{name}: volume {v0} → {v1}"));
        }
        if let Some(want) = exact_volume(name) {
            for (when, v) in [("sebelum", v0), ("sesudah", v1)] {
                if rel(v, want) > EXACT_TOL {
                    failures.push(format!("{name}: volume {when} {v}, analitik {want}"));
                }
            }
        }
        if f0 != f1 {
            failures.push(format!("{name}: face {f0} → {f1}"));
        }
        if !back.is_valid() {
            failures.push(format!("{name}: hasil baca tidak valid"));
        }
    }
    assert!(failures.is_empty(), "roundtrip STEP gagal:\n{}", failures.join("\n"));
}

#[test]
fn step_compound_file_roundtrip_keeps_total_volume() {
    let dir = std::env::temp_dir().join(format!("ducad-step-rt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("assembly.step");
    let a = plate_with_holes();
    let b = at(make_cylinder(5.0, 30.0).unwrap(), 100.0, 0.0, 0.0);
    write_step_compound(&[&a, &b], &path).unwrap();
    let back = KernelShape::read_step(&path).unwrap();
    let want = a.volume() + b.volume();
    assert!(rel(back.volume(), want) < VOL_TOL, "{} vs {want}", back.volume());
    let _ = std::fs::remove_dir_all(&dir);
}
