//! Tes kontrak permukaan Fase 6A (PLAN_PARITAS_SOLIDCRAFT §6A).

use ducad_kernel::surface::{self, RuledDir, SurfaceContinuity};
use ducad_kernel::*;
use glam::DVec3;

const XY: surface::SketchFrame = ([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);

fn line(a: (f64, f64), b: (f64, f64)) -> Vec<ProfileSegment> {
    vec![ProfileSegment::Line { start: a, end: b }]
}

fn open_l() -> Vec<ProfileSegment> {
    vec![
        ProfileSegment::Line { start: (0.0, 0.0), end: (10.0, 0.0) },
        ProfileSegment::Line { start: (10.0, 0.0), end: (10.0, 10.0) },
    ]
}

fn area(s: &KernelShape) -> f64 {
    s.surface_area()
}

#[test]
fn extrude_revolve_sweep_loft_sheets() {
    let s = surface::extrude_sheet(XY, &[open_l()], 0.0, 5.0).unwrap();
    assert!(s.is_sheet(), "permukaan terbuka");
    assert!((area(&s) - 100.0).abs() < 0.5, "{}", area(&s));
    let r = surface::revolve_sheet(XY, &[line((5.0, 0.0), (5.0, 10.0))], (0.0, 0.0), (0.0, 1.0), 360.0).unwrap();
    let want = std::f64::consts::TAU * 5.0 * 10.0;
    assert!((area(&r) - want).abs() / want < 0.01, "{}", area(&r));
    let w = surface::sweep_sheet(XY, &[line((0.0, 0.0), (10.0, 0.0))], &[[0.0, 0.0, 0.0], [0.0, 0.0, 20.0]]).unwrap();
    assert!((area(&w) - 200.0).abs() < 1.0, "{}", area(&w));
    let top = ([0.0, 0.0, 10.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let l = surface::loft_sheet(&[(XY, line((0.0, 0.0), (10.0, 0.0))), (top, line((0.0, 0.0), (10.0, 0.0)))]).unwrap();
    assert!((area(&l) - 100.0).abs() < 1.0, "{}", area(&l));
    assert!(surface::extrude_sheet(XY, &[], 0.0, 5.0).is_err());
    assert!(surface::extrude_sheet(XY, &[open_l()], 1.0, 1.0).is_err());
    assert!(surface::revolve_sheet(XY, &[open_l()], (0.0, 0.0), (0.0, 0.0), 90.0).is_err());
}

#[test]
fn thicken_and_reverse_and_copy() {
    let s = surface::extrude_sheet(XY, &[line((0.0, 0.0), (10.0, 0.0))], 0.0, 10.0).unwrap();
    let t = surface::thicken(&s, 2.0, false).unwrap();
    assert!((t.volume().abs() - 200.0).abs() < 1.0, "{}", t.volume());
    let rev = surface::reverse_normals(&s, &[0]).unwrap();
    assert!((area(&rev) - area(&s)).abs() < 1e-6);
    let cube = make_box(10.0, 10.0, 10.0, false).unwrap();
    let c = surface::copy_faces(&cube, &[0]).unwrap();
    assert!((area(&c) - 100.0).abs() < 1e-3);
    assert!(surface::thicken(&s, 0.0, false).is_err());
}

#[test]
fn offset_trim_untrim_stitch_unstitch() {
    let cube = make_box(10.0, 10.0, 10.0, false).unwrap();
    let top = enumerate_faces(&cube).iter().find(|f| f.normal[2] > 0.9).unwrap().index;
    let o = surface::offset_surface(&cube, &[top], 5.0).unwrap();
    let bb = o.bounding_box().unwrap();
    assert!((bb.0[2] - 15.0).abs() < 1e-6, "{bb:?}");
    let s = surface::extrude_sheet(XY, &[line((0.0, 0.0), (10.0, 0.0))], 0.0, 10.0).unwrap();
    let tr = surface::trim_by_plane(&s, DVec3::new(0.0, 0.0, 4.0), DVec3::Z, DVec3::new(5.0, 0.0, 1.0)).unwrap();
    assert!((area(&tr) - 40.0).abs() < 0.5, "{}", area(&tr));
    // Stitch dua pita yang bersebelahan.
    let a = surface::extrude_sheet(XY, &[line((0.0, 0.0), (10.0, 0.0))], 0.0, 10.0).unwrap();
    let b = surface::extrude_sheet(XY, &[line((10.0, 0.0), (10.0, 10.0))], 0.0, 10.0).unwrap();
    let st = surface::stitch(&[&a, &b], 1e-3).unwrap();
    assert!((area(&st) - 200.0).abs() < 1.0);
    let (pieces, rest) = surface::unstitch(&st, &[0]).unwrap();
    assert!(!pieces.is_empty() || rest.is_some());
    assert!(surface::stitch(&[&a], 1e-3).is_err());
    // Untrim: pelat berlubang → lubang hilang.
    let plate = subtract(&make_box(20.0, 20.0, 2.0, false).unwrap(), &translate_shape(&make_cylinder(3.0, 10.0).unwrap(), 10.0, 10.0, -1.0).unwrap()).unwrap();
    let top = enumerate_faces(&plate).iter().find(|f| f.normal[2] > 0.9).unwrap().index;
    let face = surface::copy_faces(&plate, &[top]).unwrap();
    let un = surface::untrim(&face, 0).unwrap();
    assert!((area(&un) - 400.0).abs() < 1.0, "{}", area(&un));
}

#[test]
fn patch_ruled_extend_boundary_blend() {
    // Kotak tanpa tutup atas → patch menutup.
    let cube = make_box(10.0, 10.0, 10.0, false).unwrap();
    let top = enumerate_faces(&cube).iter().find(|f| f.normal[2] > 0.9).unwrap().index;
    let side: Vec<usize> = enumerate_faces(&cube).iter().filter(|f| f.index != top).map(|f| f.index).collect();
    let open = surface::copy_faces(&cube, &side).unwrap();
    let rim: Vec<usize> = enumerate_edges(&open).iter().filter(|e| (e.start[2] - 10.0).abs() < 1e-6 && (e.end[2] - 10.0).abs() < 1e-6).map(|e| e.index).collect();
    assert!(!rim.is_empty());
    let p = surface::patch(&open, &rim).unwrap();
    assert!((area(&p) - 100.0).abs() < 1.0, "{}", area(&p));
    let r = surface::ruled(&open, &rim[..1], 5.0, RuledDir::Direction([0.0, 0.0, 1.0]), false).unwrap();
    assert!(area(&r) > 1.0);
    let s = surface::extrude_sheet(XY, &[line((0.0, 0.0), (10.0, 0.0))], 0.0, 10.0).unwrap();
    let top_edge: Vec<usize> = enumerate_edges(&s).iter().filter(|e| (e.start[2] - 10.0).abs() < 1e-6 && (e.end[2] - 10.0).abs() < 1e-6).map(|e| e.index).collect();
    let ex = surface::extend(&s, &top_edge, 5.0).unwrap();
    assert!((area(&ex) - 150.0).abs() < 1.0, "{}", area(&ex));
    let u = vec![vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]], vec![[0.0, 10.0, 0.0], [10.0, 10.0, 2.0]]];
    let bd = surface::boundary(&u, &[]).unwrap();
    assert!(area(&bd) > 99.0);
    assert!(surface::boundary(&u[..1], &[]).is_err());
    // Blend dua pita sejajar berjarak.
    let a = surface::extrude_sheet(XY, &[line((0.0, 0.0), (10.0, 0.0))], 0.0, 10.0).unwrap();
    let b = translate_shape(&a, 0.0, 10.0, 15.0).unwrap();
    let ea: Vec<usize> = enumerate_edges(&a).iter().filter(|e| (e.start[2] - 10.0).abs() < 1e-6 && (e.end[2] - 10.0).abs() < 1e-6).map(|e| e.index).collect();
    let eb: Vec<usize> = enumerate_edges(&b).iter().filter(|e| (e.start[2] - 15.0).abs() < 1e-6 && (e.end[2] - 15.0).abs() < 1e-6).map(|e| e.index).collect();
    let bl = surface::blend(&a, &ea, &b, &eb, SurfaceContinuity::G1);
    assert!(bl.is_ok(), "{:?}", bl.err());
}

#[test]
fn single_face_strip_is_sheet() {
    let s = surface::extrude_sheet(XY, &[line((0.0, 0.0), (10.0, 0.0))], 0.0, 10.0).unwrap();
    assert!(s.is_sheet(), "valid={} ", s.is_valid());
}
