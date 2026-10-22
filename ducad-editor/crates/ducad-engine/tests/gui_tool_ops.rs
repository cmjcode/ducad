//! Alat langsung GUI sebagai op oplog (ADR 0010): push_pull, revolve_face,
//! round_vertex, rib, resize_edge, shell_variable, adopt, dan `copy` pada
//! transform/scale. Tiap op: hasil sama dengan fungsi kernel berbasis ray
//! yang dipakai GUI dulu, dan replay design mereproduksi geometri yang sama.

use ducad_engine::ops::Op;
use ducad_engine::session::fingerprint;
use ducad_engine::{OpErrorCode, Session};
use ducad_kernel::{KernelShape, PickRay};
use glam::dvec3;
use serde_json::json;

fn op(v: serde_json::Value) -> Op {
    serde_json::from_value(v).unwrap()
}

/// Kotak 30×20×15, sudut minimum di origin.
fn boxed() -> Session {
    let mut s = Session::new();
    let r = s.run(
        vec![op(json!({"op":"primitive","id":"b","shape":{"box":{"size":[30,20,15]}}}))],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    s
}

fn shape(s: &Session, name: &str) -> KernelShape {
    ducad_kernel::clone_shape(&s.body(name).unwrap().1.shape).unwrap()
}

fn vol(s: &Session, name: &str) -> f64 {
    s.body(name).unwrap().1.shape.volume().abs()
}

fn bbox(s: &Session, name: &str) -> [[f64; 3]; 2] {
    s.summary()
        .bodies
        .iter()
        .find(|b| b.name == name)
        .map(|b| b.bbox)
        .unwrap_or_else(|| panic!("body {name} tidak ada"))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= b.abs() * 1e-6 + 1e-6
}

/// Jalankan `v` pada sesi, pastikan volume `body` sama dengan `expect`
/// (hasil kernel), lalu replay design harus memberi sidik jari yang sama.
fn run_and_replay(s: &mut Session, v: serde_json::Value, body: &str, expect: f64) {
    let r = s.run(vec![op(v.clone())], false);
    assert!(r.committed, "{v}: {:?}", r.error);
    assert!(close(vol(s, body), expect), "{v}: volume {} vs kernel {expect}", vol(s, body));
    let replayed = Session::replay(s.design().clone()).unwrap();
    assert_eq!(fingerprint(s.model()), fingerprint(replayed.model()), "{v}: replay beda");
}

const TOP: PickRay = PickRay {
    origin: (15.0, 10.0, 100.0),
    dir: (0.0, 0.0, -1.0),
};
const BOTTOM: PickRay = PickRay {
    origin: (15.0, 10.0, -100.0),
    dir: (0.0, 0.0, 1.0),
};
const CORNER: PickRay = PickRay {
    origin: (40.0, 30.0, 25.0),
    dir: (-10.0, -10.0, -10.0),
};

#[test]
fn push_pull_matches_kernel_extrude_face() {
    for d in [10.0, -5.0] {
        let mut s = boxed();
        let k = ducad_kernel::extrude_face(&shape(&s, "b"), TOP, d).unwrap();
        run_and_replay(
            &mut s,
            json!({"op":"push_pull","id":"pp1","body":"b","face":">Z","distance":d}),
            "b",
            k.volume().abs(),
        );
        assert!(close(vol(&s, "b"), 30.0 * 20.0 * (15.0 + d)));
    }
    // Selector harus tepat 1 face.
    let mut s = boxed();
    let r = s.run(
        vec![op(json!({"op":"push_pull","id":"pp","body":"b","face":"#Z","distance":1}))],
        false,
    );
    assert!(!r.committed);
    assert!(r.error.unwrap().hint.is_some());
}

#[test]
fn revolve_face_matches_kernel() {
    let mut s = boxed();
    let (o, d) = (dvec3(0.0, 0.0, 15.0), dvec3(0.0, 1.0, 0.0));
    let k = ducad_kernel::revolve_face(&shape(&s, "b"), TOP, o, d, Some(90.0)).unwrap();
    run_and_replay(
        &mut s,
        json!({"op":"revolve_face","id":"rf1","body":"b","face":">Z",
               "axis":{"origin":[0,0,15],"dir":[0,1,0]},"angle_deg":90}),
        "b",
        k.volume().abs(),
    );
    // Seperempat silinder r30 × 20 menempel di atas kotak.
    let quarter = std::f64::consts::PI * 900.0 / 4.0 * 20.0;
    assert!((vol(&s, "b") - (9000.0 + quarter)).abs() < quarter * 0.02, "{}", vol(&s, "b"));
}

#[test]
fn round_vertex_matches_kernel_vertex_tools() {
    let mut s = boxed();
    let k = ducad_kernel::fillet_vertex(&shape(&s, "b"), 3.0, CORNER, 1.0).unwrap();
    run_and_replay(
        &mut s,
        json!({"op":"round_vertex","id":"rv1","body":"b","near":[29,19,14],"radius":3,"chamfer":false}),
        "b",
        k.volume().abs(),
    );

    let mut s = boxed();
    let k = ducad_kernel::chamfer_vertex(&shape(&s, "b"), 2.0, CORNER, 1.0).unwrap();
    run_and_replay(
        &mut s,
        json!({"op":"round_vertex","id":"rv1","body":"b","near":[30,20,15],"radius":2,"chamfer":true}),
        "b",
        k.volume().abs(),
    );

    // Tidak ada sudut dalam 5 mm → error jelas, batch di-rollback.
    let mut s = boxed();
    let r = s.run(
        vec![op(json!({"op":"round_vertex","id":"rv","body":"b","near":[15,10,7.5],"radius":1}))],
        false,
    );
    assert!(!r.committed);
    let e = r.error.unwrap();
    assert_eq!(e.code, OpErrorCode::InvalidParam);
    assert!(e.message.contains("5 mm"), "{}", e.message);
}

#[test]
fn rib_matches_kernel_create_rib() {
    let mut s = boxed();
    let (a, b, n) = (dvec3(2.0, 10.0, 15.0), dvec3(28.0, 10.0, 15.0), dvec3(0.0, 0.0, 1.0));
    let k = ducad_kernel::create_rib(&shape(&s, "b"), a, b, n, 2.0, 5.0, Some(1.0)).unwrap();
    run_and_replay(
        &mut s,
        json!({"op":"rib","id":"rib1","body":"b","start":[2,10,15],"end":[28,10,15],
               "normal":[0,0,1],"thickness":2,"depth":5,"draft_deg":1.0}),
        "b",
        k.volume().abs(),
    );
    let bb = bbox(&s, "b");
    assert!((bb[1][2] - 20.0).abs() < 1e-6, "{bb:?}");
}

#[test]
fn resize_edge_matches_kernel() {
    let mut s = boxed();
    let k = ducad_kernel::resize_shape_along_edge(&shape(&s, "b"), (0.0, 0.0, 0.0), (0.0, 0.0, 15.0), 30.0)
        .unwrap();
    run_and_replay(
        &mut s,
        json!({"op":"resize_edge","id":"re1","body":"b","start":[0,0,0],"end":[0,0,15],"length":30}),
        "b",
        k.volume().abs(),
    );
    let bb = bbox(&s, "b");
    assert!((bb[1][2] - 30.0).abs() < 1e-6 && (bb[1][0] - 30.0).abs() < 1e-6, "{bb:?}");
}

#[test]
fn shell_variable_matches_kernel() {
    let mut s = boxed();
    let k = ducad_kernel::shell_variable_thickness(&shape(&s, "b"), 2.0, &[TOP], &[(BOTTOM, 4.0)])
        .unwrap();
    run_and_replay(
        &mut s,
        json!({"op":"shell_variable","id":"sv1","body":"b","thickness":2,"remove_faces":">Z",
               "custom":[{"faces":"<Z","thickness":4}]}),
        "b",
        k.volume().abs(),
    );
    // Rongga 26 × 16 × 11 (dinding 2, alas 4, atas terbuka).
    assert!(close(vol(&s, "b"), 9000.0 - 26.0 * 16.0 * 11.0));

    // Tanpa remove_faces: rongga tertutup.
    let mut s = boxed();
    let r = s.run(
        vec![op(json!({"op":"shell_variable","id":"sv","body":"b","thickness":2}))],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert!(close(vol(&s, "b"), 9000.0 - 26.0 * 16.0 * 11.0));
}

#[test]
fn transform_and_scale_copy_keep_the_original() {
    let mut s = boxed();
    let r = s.run(
        vec![
            op(json!({"op":"transform","id":"moved","body":"b","translate":[50,0,0],"copy":true})),
            op(json!({"op":"scale","id":"big","body":"b","factor":2,"copy":true})),
        ],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert_eq!(bbox(&s, "b"), [[0.0, 0.0, 0.0], [30.0, 20.0, 15.0]]);
    assert!(close(bbox(&s, "moved")[0][0], 50.0));
    assert!(close(vol(&s, "big"), 8.0 * 9000.0));
    let replayed = Session::replay(s.design().clone()).unwrap();
    assert_eq!(fingerprint(s.model()), fingerprint(replayed.model()));

    // Tanpa `copy` (oplog lama): tetap di tempat, dan JSON tidak berubah.
    let t = op(json!({"op":"transform","id":"t","body":"b","translate":[1,0,0]}));
    assert!(!serde_json::to_value(&t).unwrap().as_object().unwrap().contains_key("copy"));
    assert!(s.run(vec![t], false).committed);
    assert!(close(bbox(&s, "b")[0][0], 1.0));
}

#[test]
fn adopt_restores_frozen_geometry_on_replay() {
    // Geometri "GUI" yang tidak punya op: kotak berfillet sudut.
    let mut src = boxed();
    assert!(src
        .run(
            vec![op(json!({"op":"fillet","id":"f","body":"b","edges":"|Z","radius":2}))],
            false
        )
        .committed);
    let frozen = shape(&src, "b");

    // Body yang ada diganti, body yang belum ada dibuat.
    let mut s = boxed();
    let ops = vec![
        ducad_engine::adopt_op("adopt1", "b", &frozen).unwrap(),
        ducad_engine::adopt_op("adopt2", "fresh", &frozen).unwrap(),
    ];
    let r = s.run(ops, false);
    assert!(r.committed, "{:?}", r.error);
    for name in ["b", "fresh"] {
        assert!(close(vol(&s, name), frozen.volume().abs()), "{name}");
    }
    let replayed = Session::replay(s.design().clone()).unwrap();
    assert_eq!(fingerprint(s.model()), fingerprint(replayed.model()));
    assert!(close(vol(&replayed, "fresh"), frozen.volume().abs()));

    // Teks STEP juga diterima; brep rusak → error, bukan panic.
    let step = frozen.to_step_string().unwrap();
    let mut s = boxed();
    let r = s.run(vec![op(json!({"op":"adopt","id":"a","body":"c","brep":step}))], false);
    assert!(r.committed, "{:?}", r.error);
    assert!((vol(&s, "c") - frozen.volume().abs()).abs() < 1e-3 * frozen.volume().abs());
    for bad in ["garbage", "ducad-truck/1:!!!", "other-kernel/9:AAAA"] {
        let r = s.run(vec![op(json!({"op":"adopt","id":"x","body":"b","brep":bad}))], false);
        assert!(!r.committed, "{bad}");
        assert!(r.error.unwrap().hint.is_some(), "{bad}");
    }
}

#[test]
fn feature_tree_rows_and_display_json() {
    let frozen = shape(&boxed(), "b");
    let s = boxed();
    let ops = vec![
        op(json!({"op":"push_pull","id":"pp1","body":"b","face":">Z","distance":10})),
        op(json!({"op":"revolve_face","id":"rf1","body":"b","face":"<X",
                  "axis":{"origin":[0,0,0],"dir":[0,1,0]},"angle_deg":45})),
        op(json!({"op":"round_vertex","id":"rv1","body":"b","near":[30,20,0],"radius":1})),
        op(json!({"op":"resize_edge","id":"re1","body":"b","start":[30,0,0],"end":[30,20,0],"length":25})),
        op(json!({"op":"shell_variable","id":"sv1","body":"other","thickness":2})),
        ducad_engine::adopt_op("adopt1", "other", &frozen).unwrap(),
    ];
    let design = ducad_engine::DesignDoc {
        oplog: ops.clone(),
        ..s.design().clone()
    };
    let rows = ducad_engine::oplog_view::oplog_rows(&design);
    let main: Vec<Option<(String, f64)>> = rows.iter().map(|r| r.main_value.clone()).collect();
    assert_eq!(
        main,
        vec![
            Some(("distance".into(), 10.0)),
            Some(("angle_deg".into(), 45.0)),
            Some(("radius".into(), 1.0)),
            Some(("length".into(), 25.0)),
            Some(("thickness".into(), 2.0)),
            None,
        ]
    );
    assert_eq!(rows[0].summary, "10 mm, b");
    assert_eq!(rows[2].summary, "R1, b");
    assert_eq!(rows[5].summary, "Edit langsung (beku), other");
    assert!(ducad_engine::oplog_view::with_value(&ops[3], "length", 40.0).is_some());

    let shown = ducad_engine::oplog_view::oplog_display_json(&ops);
    let brep = shown[5]["brep"].as_str().unwrap();
    assert!(brep.starts_with("<brep ") && brep.len() < 40, "{brep}");
    assert_eq!(shown[0], serde_json::to_value(&ops[0]).unwrap());

    // Rib: nilai utama = thickness.
    let rib = op(json!({"op":"rib","id":"rib1","body":"b","start":[0,0,0],"end":[1,0,0],
                        "normal":[0,0,1],"thickness":2,"depth":10}));
    let d = ducad_engine::DesignDoc {
        oplog: vec![rib],
        ..s.design().clone()
    };
    let rows = ducad_engine::oplog_view::oplog_rows(&d);
    assert_eq!(rows[0].main_value, Some(("thickness".into(), 2.0)));
}
