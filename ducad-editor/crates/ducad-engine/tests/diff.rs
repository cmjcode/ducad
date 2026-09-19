//! Diff desain (P8.2).

use std::f64::consts::PI;

use ducad_engine::diff::{diff, OpChange};
use ducad_engine::ops::OpFile;
use ducad_engine::Session;

fn session(json: &str) -> Session {
    let f: OpFile = serde_json::from_str(json).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    s
}

fn plate_with_holes(at: &str) -> String {
    ducad_engine::ops::EXAMPLE_PLATE.replace(r#""at":[[-20,-10],[20,-10],[-20,10],[20,10]]"#, at)
}

#[test]
fn one_extra_hole_is_a_changed_field_and_removed_volume() {
    let three = plate_with_holes(r#""at":[[-20,-10],[20,-10],[-20,10]]"#);
    let a = session(&three);
    let b = session(ducad_engine::ops::EXAMPLE_PLATE);
    let (d, shapes) = diff(&a, &b, true);
    assert!(d.params.is_empty());
    assert_eq!(d.ops.len(), 1, "{:?}", d.ops);
    match &d.ops[0] {
        OpChange::Changed { id, fields } => {
            assert_eq!(id, "h1");
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].path, "/at/3");
            assert!(fields[0].old.is_null());
        }
        other => panic!("{other:?}"),
    }
    let body = &d.bodies[0];
    assert_eq!(body.status, "changed");
    let expected = PI * 2.75 * 2.75 * 8.0;
    let removed = body.removed_volume.unwrap();
    assert!(
        (removed - expected).abs() / expected < 0.01,
        "{removed} vs {expected}"
    );
    assert!(body.added_volume.unwrap() < 1e-6);
    assert_eq!(shapes.removed.len(), 1);
    assert!(!d.is_empty());
}

#[test]
fn identical_sessions_have_no_diff_and_no_geometry_skips_kernel() {
    let a = session(ducad_engine::ops::EXAMPLE_PLATE);
    let b = session(ducad_engine::ops::EXAMPLE_PLATE);
    let (d, _) = diff(&a, &b, true);
    assert!(d.is_empty(), "{d:?}");

    let mut c = session(ducad_engine::ops::EXAMPLE_PLATE);
    let mut p = c.design().params.clone();
    p.insert("t".into(), 10.0);
    assert!(c.set_params(p).unwrap().committed);
    let (d, shapes) = diff(&a, &c, false);
    assert_eq!(d.params.len(), 1);
    assert_eq!(d.params[0].name, "t");
    assert_eq!(d.bodies[0].status, "changed");
    assert!(
        d.bodies[0].added_volume.is_none(),
        "--no-geometry tidak menghitung volume boolean"
    );
    assert!(shapes.added.is_empty() && shapes.removed.is_empty());
}

#[test]
fn added_removed_and_reordered_ops() {
    let a = session(
        r#"{"ops":[{"op":"primitive","id":"x","shape":{"box":{"size":[1,1,1]}}},
                   {"op":"primitive","id":"y","shape":{"box":{"size":[2,2,2]}},"at":[5,0,0]},
                   {"op":"primitive","id":"z","shape":{"sphere":{"r":1}},"at":[20,0,0]}]}"#,
    );
    let b = session(
        r#"{"ops":[{"op":"primitive","id":"y","shape":{"box":{"size":[2,2,2]}},"at":[5,0,0]},
                   {"op":"primitive","id":"x","shape":{"box":{"size":[1,1,1]}}},
                   {"op":"primitive","id":"w","shape":{"sphere":{"r":1}},"at":[40,0,0]}]}"#,
    );
    let (d, _) = diff(&a, &b, false);
    let kinds: Vec<&str> = d
        .ops
        .iter()
        .map(|c| match c {
            OpChange::Added { .. } => "added",
            OpChange::Removed { .. } => "removed",
            OpChange::Changed { .. } => "changed",
            OpChange::Reordered { .. } => "reordered",
        })
        .collect();
    assert!(
        kinds.contains(&"added") && kinds.contains(&"removed") && kinds.contains(&"reordered"),
        "{kinds:?}"
    );
    let statuses: Vec<(&str, &str)> = d
        .bodies
        .iter()
        .map(|b| (b.name.as_str(), b.status))
        .collect();
    assert!(
        statuses.contains(&("z", "removed")) && statuses.contains(&("w", "added")),
        "{statuses:?}"
    );
}

#[test]
fn diff_render_has_colored_layers() {
    let three = plate_with_holes(r#""at":[[-20,-10],[20,-10],[-20,10]]"#);
    let a = session(&three);
    let b = session(ducad_engine::ops::EXAMPLE_PLATE);
    let (_, shapes) = diff(&a, &b, true);
    let r = ducad_engine::render::render_diff_svg(
        &b,
        &shapes,
        ducad_engine::render::View::Iso,
        800,
        600,
    )
    .unwrap();
    assert!(r.svg.starts_with("<svg"));
    assert!(
        r.svg.contains("#dc2626"),
        "lapisan volume hilang berwarna merah"
    );
    assert!(r.svg.contains(r#"id="layer_0""#) && r.svg.contains(r#"id="layer_2""#));
    assert!(r.visible_segments > 0);
}
