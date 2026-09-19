//! Tes tabel selector semantik (P1.4).

use ducad_engine::compute::{self, PrimitiveShape, ProfilePick};
use ducad_engine::select::{select_edges, select_faces};
use ducad_engine::{OpErrorCode, PlaneFrame};
use ducad_kernel::{ExtrudeExtent, KernelShape};
use ducad_sketch::{Entity, Sketch};
use glam::DVec2;

fn plate() -> KernelShape {
    compute::primitive(
        &PrimitiveShape::Box {
            size: [60.0, 40.0, 8.0],
            centered: false,
        },
        [0.0; 3],
    )
    .unwrap()
    .shape
}

fn plate_with_hole() -> KernelShape {
    let mut s = Sketch::default();
    let c = [
        DVec2::new(0.0, 0.0),
        DVec2::new(60.0, 0.0),
        DVec2::new(60.0, 40.0),
        DVec2::new(0.0, 40.0),
    ];
    for i in 0..4 {
        s.entities.insert(Entity::line(c[i], c[(i + 1) % 4]));
    }
    s.entities
        .insert(Entity::circle(DVec2::new(30.0, 20.0), 2.75));
    let mut solids = compute::extrude(
        &s,
        &ProfilePick::AllRegions,
        &PlaneFrame::top(),
        ExtrudeExtent::Blind(8.0),
    )
    .unwrap();
    solids.pop().unwrap().1.shape
}

#[test]
fn face_selectors_on_box() {
    let b = plate();
    let top = select_faces(&b, ">Z").unwrap();
    assert_eq!(top.len(), 1);
    let faces = ducad_kernel::enumerate_faces(&b);
    assert!(faces[top[0]].normal[2] > 0.99);
    assert_eq!(select_faces(&b, "+Z or -Z").unwrap().len(), 2);
    assert_eq!(select_faces(&b, "#Z").unwrap().len(), 4);
    assert_eq!(select_faces(&b, "largest").unwrap().len(), 2);
    assert_eq!(select_faces(&b, "ALL").unwrap().len(), 6);
    assert_eq!(select_faces(&b, "all[kind=plane][z=4]").unwrap().len(), 4);
}

#[test]
fn edge_selectors_on_box() {
    let b = plate();
    assert_eq!(select_edges(&b, "|Z").unwrap().len(), 4);
    assert_eq!(select_edges(&b, "of(>Z)").unwrap().len(), 4);
    assert_eq!(select_edges(&b, "of(>Z) and |X").unwrap().len(), 2);
    assert_eq!(select_edges(&b, "all except |Z").unwrap().len(), 8);
    assert_eq!(select_edges(&b, "longest").unwrap().len(), 4);
    assert_eq!(select_edges(&b, "(|X or |Y)[z>4]").unwrap().len(), 4);
}

#[test]
fn selectors_on_box_with_hole() {
    let b = plate_with_hole();
    assert!(!select_faces(&b, "all[kind=cylinder][r=2.75]")
        .unwrap()
        .is_empty());
    assert_eq!(select_edges(&b, "all[kind=circle]").unwrap().len(), 2);
}

#[test]
fn selector_errors() {
    let b = plate();
    let e = select_faces(&b, ">Q").unwrap_err();
    assert_eq!(e.code, OpErrorCode::SelectorSyntax);
    assert_eq!(e.context["pos"], 1);
    assert_eq!(
        select_edges(&b, "+Z").unwrap_err().code,
        OpErrorCode::SelectorSyntax
    );
    assert_eq!(
        select_faces(&b, "longest").unwrap_err().code,
        OpErrorCode::SelectorSyntax
    );
    assert_eq!(
        select_faces(&b, "of(>Z)").unwrap_err().code,
        OpErrorCode::SelectorSyntax
    );
    assert_eq!(
        select_faces(&b, "all[kind=line]").unwrap_err().code,
        OpErrorCode::SelectorSyntax
    );
    let e = select_faces(&b, "all[r>100]").unwrap_err();
    assert_eq!(e.code, OpErrorCode::SelectorEmpty);
    assert_eq!(e.context["selector"], "all[r>100]");
    assert_eq!(e.context["available"]["faces"]["plane"], 6);
    assert_eq!(e.context["available"]["edges"]["line"], 12);
}
