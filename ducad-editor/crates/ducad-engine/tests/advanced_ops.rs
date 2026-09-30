//! Tes op lanjutan P14 lewat `Session` (loft, sweep, helix, draft, mirror,
//! scale, split, fillet variabel).

use std::f64::consts::PI;

use ducad_engine::ops::Op;
use ducad_engine::Session;

fn ops(json: &str) -> Vec<Op> {
    serde_json::from_str(json).unwrap()
}

fn run(s: &mut Session, json: &str) {
    let r = s.run(ops(json), false);
    assert!(r.committed, "{:?}", r.error);
}

fn volume(s: &Session, name: &str) -> f64 {
    s.body(name).unwrap().1.shape.volume().abs()
}

fn bbox(s: &Session, name: &str) -> [[f64; 3]; 2] {
    s.summary()
        .bodies
        .into_iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("body {name} tidak ada"))
        .bbox
}

fn near(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1e-9)
}

fn block(s: &mut Session, at: [f64; 3], size: [f64; 3]) {
    let j = format!(
        r#"[{{"op":"primitive","id":"b","shape":{{"box":{{"size":[{},{},{}]}}}},"at":[{},{},{}]}}]"#,
        size[0], size[1], size[2], at[0], at[1], at[2]
    );
    run(s, &j);
}

#[test]
fn loft_square_to_circle_on_offset_plane() {
    let mut s = Session::new();
    run(
        &mut s,
        r#"[
        {"op":"sketch","id":"s0","plane":"XY","entities":[{"rect":{"center":[0,0],"w":20,"h":20}}]},
        {"op":"sketch","id":"s1","plane":{"base":"XY","offset":30},"entities":[{"circle":{"center":[0,0],"r":8}}]},
        {"op":"loft","id":"l","sections":["s0","s1"]}
    ]"#,
    );
    let v = volume(&s, "l");
    // Di antara prisma lingkaran (π·64·30) dan prisma persegi (400·30).
    assert!(v > PI * 64.0 * 30.0 && v < 400.0 * 30.0, "{v}");
    let bb = bbox(&s, "l");
    assert!((bb[1][2] - 30.0).abs() < 1e-3, "{bb:?}");

    let r = s.run(ops(r#"[{"op":"loft","id":"l2","sections":["s0"]}]"#), true);
    assert!(r.error.is_some());
}

#[test]
fn sweep_along_points_and_sketch_path() {
    let mut s = Session::new();
    run(
        &mut s,
        r#"[
        {"op":"sketch","id":"prof","plane":"XZ","entities":[{"circle":{"center":[0,0],"r":2}}]},
        {"op":"sweep","id":"pipe","sketch":"prof","path":{"points":[[0,0,0],[0,-40,0]]}}
    ]"#,
    );
    assert!(
        near(volume(&s, "pipe"), PI * 4.0 * 40.0, 1e-3),
        "{}",
        volume(&s, "pipe")
    );

    run(
        &mut s,
        r#"[
        {"op":"sketch","id":"rail","plane":"XY","entities":[{"line":{"from":[0,-40],"to":[0,0]}}]},
        {"op":"sketch","id":"prof2","plane":"XZ","entities":[{"circle":{"center":[20,0],"r":1}}]},
        {"op":"sketch","id":"rail2","plane":"XY","entities":[{"line":{"from":[20,0],"to":[20,-30]}}]},
        {"op":"sweep","id":"rod","sketch":"prof2","path":"rail2"}
    ]"#,
    );
    assert!(
        near(volume(&s, "rod"), PI * 30.0, 1e-3),
        "{}",
        volume(&s, "rod")
    );
}

#[test]
fn helix_spring_volume() {
    let mut s = Session::new();
    run(
        &mut s,
        r#"[{"op":"helix","id":"spring","r":10,"pitch":4,"turns":3,"section":{"circle":{"r":1}}}]"#,
    );
    let length = 3.0 * ((2.0 * PI * 10.0f64).powi(2) + 16.0).sqrt();
    let v = volume(&s, "spring");
    assert!(near(v, PI * length, 0.03), "{v} vs {}", PI * length);
    let r = s.run(
        ops(r#"[{"op":"helix","id":"bad","r":10,"pitch":4,"turns":3,"section":{"circle":{"r":3}}}]"#),
        true,
    );
    assert!(
        r.error.is_some(),
        "kawat lebih tebal dari pitch harus ditolak"
    );
}

#[test]
fn draft_side_walls() {
    let mut s = Session::new();
    block(&mut s, [0.0, 0.0, 0.0], [20.0, 20.0, 10.0]);
    run(
        &mut s,
        r##"[{"op":"draft","id":"d","body":"b","faces":"#Z","angle_deg":5}]"##,
    );
    let v = volume(&s, "b");
    assert!(v < 4000.0 - 1.0 && v > 3000.0, "{v}");
}

#[test]
fn mirror_copy_merge_and_replace() {
    let mut s = Session::new();
    block(&mut s, [10.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    run(
        &mut s,
        r#"[{"op":"mirror","id":"m","body":"b","plane":"YZ"}]"#,
    );
    let bb = bbox(&s, "m");
    assert!(
        (bb[0][0] + 20.0).abs() < 1e-3 && (bb[1][0] + 10.0).abs() < 1e-3,
        "{bb:?}"
    );
    assert!(near(volume(&s, "m"), 1000.0, 1e-6));

    let mut s = Session::new();
    block(&mut s, [0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    run(
        &mut s,
        r#"[{"op":"mirror","id":"m","body":"b","plane":{"point":[10,0,0],"normal":[1,0,0]},"merge":true}]"#,
    );
    assert!(near(volume(&s, "b"), 2000.0, 1e-6));
    assert_eq!(s.summary().bodies.len(), 1);
}

#[test]
fn scale_and_split() {
    let mut s = Session::new();
    block(&mut s, [0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    run(
        &mut s,
        r#"[{"op":"scale","id":"sc","body":"b","factor":2}]"#,
    );
    assert!(near(volume(&s, "b"), 8000.0, 1e-6));
    run(
        &mut s,
        r#"[{"op":"split","id":"lower","body":"b","point":[0,0,5],"normal":[0,0,1]}]"#,
    );
    assert!(near(volume(&s, "b"), 6000.0, 1e-6), "{}", volume(&s, "b"));
    assert!(near(volume(&s, "lower"), 2000.0, 1e-6));
    assert!(bbox(&s, "b")[0][2] >= 5.0 - 1e-3);

    let r = s.run(
        ops(r#"[{"op":"split","id":"x","body":"b","point":[0,0,500],"normal":[0,0,1]}]"#),
        true,
    );
    assert!(r.error.is_some(), "bidang di luar body");
}

#[test]
fn variable_fillet_and_replay() {
    let mut s = Session::new();
    block(&mut s, [0.0, 0.0, 0.0], [20.0, 20.0, 10.0]);
    run(
        &mut s,
        r#"[{"op":"fillet","id":"f","body":"b","edges":"|Z","radius":1,"radius_end":3}]"#,
    );
    let v = volume(&s, "b");
    assert!(v < 4000.0 && v > 3900.0, "{v}");
    // Oplog dengan op baru bisa di-replay persis.
    let again = Session::replay(s.design().clone()).unwrap();
    assert!(near(volume(&again, "b"), v, 1e-9));
    let json = serde_json::to_string(&s.design().oplog).unwrap();
    assert!(json.contains("\"radius_end\":3"), "{json}");
}
