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
    assert_eq!(s.summary().body_names(), vec!["bracket".to_string()]);
    let (_, geo) = s.body("bracket").unwrap();
    assert!(geo.shape.is_valid());
    let v = geo.shape.volume().abs();
    assert!(
        (v - P09_BRACKET_VOLUME).abs() / P09_BRACKET_VOLUME < 1e-3,
        "{v}"
    );
}

#[test]
fn replay_oplog_with_path_is_deterministic() {
    let json = r##"{
        "params": { "w": 40 },
        "ops": [
            {
                "op": "sketch",
                "id": "sk1",
                "plane": "XY",
                "entities": [
                    {
                        "path": {
                            "name": "profile",
                            "subpaths": [{
                                "start": [0, 0],
                                "closed": true,
                                "segs": [
                                    {"line": {"to": ["$w", 0]}},
                                    {"cubic": {"c1": ["$w + 10", 0], "c2": ["$w + 10", 20], "to": ["$w", 20]}},
                                    {"line": {"to": [0, 20]}}
                                ]
                            }],
                            "style": {
                                "fill": "#00ff88",
                                "stroke": "#112233",
                                "stroke_width": 1.5,
                                "opacity": 0.8
                            },
                            "layer": "Vectors"
                        }
                    }
                ],
                "constraints": []
            }
        ]
    }"##;
    let s1 = load(json);
    let s2 = load(json);

    let (sk1, _) = s1.sketch("sk1").unwrap();
    let (sk2, _) = s2.sketch("sk1").unwrap();

    assert_eq!(sk1.entities.len(), sk2.entities.len());
    assert_eq!(sk1.styles.len(), sk2.styles.len());
    assert_eq!(sk1.layers.len(), sk2.layers.len());

    let path1 = sk1.entities.values().next().unwrap();
    let path2 = sk2.entities.values().next().unwrap();
    assert_eq!(path1, path2);

    let style1 = sk1.styles.values().next().unwrap();
    let style2 = sk2.styles.values().next().unwrap();
    assert_eq!(style1, style2);
}

