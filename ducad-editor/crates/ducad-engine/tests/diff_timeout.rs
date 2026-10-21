//! B1-1 (`docs/BASELINE_DESIGNER_UX.md`): diff geometris dua versi bracket
//! GUI (fillet sudut dalam + lubang, alas 40 vs 55) dulu macet tanpa batas di
//! `truck_geometry::intersection_curve::hinted_sub_division`. Sekarang boolean
//! pratinjau punya batas waktu: diff selesai dengan peringatan, bukan beku.
//!
//! Binary tersendiri: pekerjaan kernel yang ditinggalkan bisa masih berjalan
//! dan menahan boolean lain di proses yang sama.

use std::sync::mpsc;
use std::time::Duration;

#[test]
fn diff_of_filleted_bracket_versions_finishes_in_time() {
    let f: ducad_engine::ops::OpFile =
        serde_json::from_str(include_str!("fixtures/gui_bracket_hole.ops.json")).unwrap();
    let build = move |base: f64| {
        let mut p = f.params.clone();
        p.insert("base".into(), base);
        let mut s = ducad_engine::Session::new();
        assert!(s.set_params(p).unwrap().committed);
        assert!(s.run(f.ops.clone(), false).committed);
        s
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (a, b) = (build(40.0), build(55.0));
        let (d, _) = ducad_engine::diff::diff(&a, &b, true);
        let _ = tx.send(serde_json::to_string(&d).unwrap_or_default());
    });
    let out = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("diff geometris harus selesai (atau menyerah dengan peringatan) dalam batas waktu");
    assert!(!out.is_empty());
}
