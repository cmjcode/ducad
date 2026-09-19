//! P0.9 — bracket L dibangun murni lewat `compute` + command model, tanpa
//! GUI dan tanpa `Session`. Menjadi fixture acuan P1: replay oplog harus
//! menghasilkan body yang sama.
//!
//! Volume analitik:
//! - alas 50×30×5                          = 7500
//! - dinding 50×5×30                       = 7500
//! - tumpang tindih 50×5×5                 = −1250   → union 13750
//! - fillet cekung r=2 sepanjang 50        = +(1 − π/4)·2²·50 ≈ +42.920
//! - 2 lubang M5 clearance Ø5.5 tembus 5mm = −2·π·2.75²·5   ≈ −237.583
//! total ≈ 13555.337 mm³

use std::f64::consts::PI;

use ducad_core::hole::{HoleKind, HoleSpec, IsoMetricThread};
use ducad_core::Command;
use ducad_engine::compute::{self, EdgePick, PrimitiveShape, ProfilePick};
use ducad_engine::model::{
    AddSolidCommand, BooleanCommand, BooleanKind, ModelDoc, ReplaceGeometryCommand,
};
use ducad_engine::PlaneFrame;
use ducad_kernel::{EdgeKind, ExtrudeExtent};
use ducad_sketch::{Entity, Sketch};
use glam::DVec2;

fn only_body(model: &ModelDoc) -> ducad_core::BodyId {
    assert_eq!(model.doc.bodies.len(), 1);
    model.doc.bodies.keys().next().unwrap()
}

#[test]
fn bracket_l_headless() {
    let mut model = ModelDoc::default();

    // 1. Alas: persegi 50×30 di bidang Top, extrude 5.
    let mut sketch = Sketch::default();
    let c = [
        DVec2::new(0.0, 0.0),
        DVec2::new(50.0, 0.0),
        DVec2::new(50.0, 30.0),
        DVec2::new(0.0, 30.0),
    ];
    for i in 0..4 {
        sketch.entities.insert(Entity::line(c[i], c[(i + 1) % 4]));
    }
    let mut solids = compute::extrude(
        &sketch,
        &ProfilePick::AllRegions,
        &PlaneFrame::top(),
        ExtrudeExtent::Blind(5.0),
    )
    .unwrap();
    assert_eq!(solids.len(), 1);
    let (_, base) = solids.pop().unwrap();
    let mut add = AddSolidCommand::new("Extrude", base);
    add.apply(&mut model);
    let base_id = only_body(&model);

    // 2. Dinding tegak + union.
    let wall = compute::primitive(
        &PrimitiveShape::Box {
            size: [50.0, 5.0, 30.0],
            centered: false,
        },
        [0.0, 0.0, 0.0],
    )
    .unwrap();
    let mut add = AddSolidCommand::new("Primitif", wall);
    add.apply(&mut model);
    let wall_id = model.doc.bodies.keys().find(|k| *k != base_id).unwrap();
    let mut union = BooleanCommand::try_new(
        &model,
        BooleanKind::Union,
        "Union",
        "Bracket",
        base_id,
        wall_id,
    )
    .unwrap();
    union.apply(&mut model);
    let id = only_body(&model);
    assert!((model.geometry[id].shape.volume().abs() - 13750.0).abs() < 1e-6 * 13750.0);

    // 3. Fillet tepi lipatan dalam (∥X, mid ≈ (25, 5, 5)) r=2.
    let edges = ducad_kernel::enumerate_edges(&model.geometry[id].shape);
    let fold: Vec<usize> = edges
        .iter()
        .filter(|e| {
            e.kind == EdgeKind::Line
                && e.dir.is_some_and(|d| (d[0] - 1.0).abs() < 1e-6)
                && (e.mid[0] - 25.0).abs() < 1e-3
                && (e.mid[1] - 5.0).abs() < 1e-3
                && (e.mid[2] - 5.0).abs() < 1e-3
        })
        .map(|e| e.index)
        .collect();
    assert_eq!(fold.len(), 1, "tepi lipatan dalam harus tepat satu");
    let filleted =
        compute::fillet(&model.geometry[id].shape, &EdgePick::Indices(&fold), 2.0).unwrap();
    let mut replace = ReplaceGeometryCommand::new("Fillet", id, filleted);
    replace.apply(&mut model);

    // 4. Dua lubang M5 clearance tembus di face +Z alas.
    let mut spec = HoleSpec::for_iso(IsoMetricThread::M5, HoleKind::Simple, 5.0);
    spec.is_through = true;
    let holed = compute::hole(
        &model.geometry[id].shape,
        &spec,
        &[[12.5, 20.0, 5.0], [37.5, 20.0, 5.0]],
        [0.0, 0.0, 1.0],
    )
    .unwrap();
    let mut replace = ReplaceGeometryCommand::new("Hole Wizard", id, holed);
    replace.apply(&mut model);

    // Assert akhir.
    let id = only_body(&model);
    let geo = &model.geometry[id];
    assert!(geo.shape.is_valid());
    let (min, max) = geo.mesh.bounding_box().unwrap();
    let want = ([0.0, 0.0, 0.0], [50.0, 30.0, 30.0]);
    for i in 0..3 {
        assert!((min[i] as f64 - want.0[i]).abs() < 1e-3, "min {min:?}");
        assert!((max[i] as f64 - want.1[i]).abs() < 1e-3, "max {max:?}");
    }
    let expected = 13750.0 + (1.0 - PI / 4.0) * 4.0 * 50.0 - 2.0 * PI * 2.75 * 2.75 * 5.0;
    let v = geo.shape.volume().abs();
    assert!((v - expected).abs() / expected < 0.01, "{v} vs {expected}");
}
