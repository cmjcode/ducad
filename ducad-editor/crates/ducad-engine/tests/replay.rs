//! Golden test replay (P1.8).

use std::f64::consts::PI;
use std::path::PathBuf;

use ducad_engine::ops::OpFile;
use ducad_engine::{DesignDoc, Session};

/// Volume fixture P0.9 (`tests/bracket.rs`, bracket via compute langsung),
/// diukur 13555.337423 mm³ — analitik ≈ 13555.337.
const P09_BRACKET_VOLUME: f64 = 13555.337423;

fn load(json: &str) -> Session {
    let f: OpFile = serde_json::from_str(json).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    s
}

fn plate_volume(t: f64) -> f64 {
    60.0 * 40.0 * t - 4.0 * (1.0 - PI / 4.0) * 9.0 * t - 4.0 * PI * 2.75 * 2.75 * t
}

fn temp(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ducad-replay-{tag}-{}.ducad", std::process::id()))
}

fn stats(s: &Session) -> Vec<(String, f64, [f32; 3], [f32; 3])> {
    let mut v: Vec<_> = s
        .model()
        .doc
        .bodies
        .iter()
        .map(|(id, b)| {
            let g = &s.model().geometry[id];
            let (min, max) = g.mesh.bounding_box().unwrap();
            (b.name.clone(), g.shape.volume().abs(), min, max)
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

#[test]
fn plate_save_load_is_identical() {
    let mut s = load(include_str!("fixtures/plate.ops.json"));
    let v = s.body("plate").unwrap().1.shape.volume().abs();
    assert!(
        (v - plate_volume(8.0)).abs() / plate_volume(8.0) < 1e-3,
        "{v}"
    );

    let path = temp("plate");
    s.save(&path).unwrap();
    let loaded = Session::from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(
        loaded.summary().warnings.is_empty(),
        "{:?}",
        loaded.summary().warnings
    );
    let (a, b) = (stats(&s), stats(&loaded));
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.0, y.0);
        assert!((x.1 - y.1).abs() / x.1 < 1e-6, "volume {} vs {}", x.1, y.1);
        assert_eq!((x.2, x.3), (y.2, y.3), "bbox");
    }
}

#[test]
fn replay_is_deterministic() {
    let s = load(include_str!("fixtures/plate.ops.json"));
    let design: DesignDoc = s.design().clone();
    let a = Session::replay(design.clone()).unwrap();
    let b = Session::replay(design.clone()).unwrap();
    assert_eq!(a.design().fingerprint, b.design().fingerprint);
    assert_eq!(a.design().fingerprint, design.fingerprint);
}

#[test]
fn set_params_changes_thickness() {
    let mut s = load(include_str!("fixtures/plate.ops.json"));
    let mut p = s.design().params.clone();
    p.insert("t".into(), 10.0);
    assert!(s.set_params(p).unwrap().committed);
    let v = s.body("plate").unwrap().1.shape.volume().abs();
    assert!(
        (v - plate_volume(10.0)).abs() / plate_volume(10.0) < 1e-3,
        "{v}"
    );
}

#[test]
fn bracket_ops_reproduce_p09_fixture() {
    let s = load(include_str!("fixtures/bracket.ops.json"));
    assert_eq!(s.summary().bodies, vec!["bracket".to_string()]);
    let (_, geo) = s.body("bracket").unwrap();
    assert!(geo.shape.is_valid());
    let v = geo.shape.volume().abs();
    assert!(
        (v - P09_BRACKET_VOLUME).abs() / P09_BRACKET_VOLUME < 1e-3,
        "{v}"
    );
}
