//! Pembuat selector dari pilihan GUI (ADR 0010 butir 3): selector usulan
//! harus memilih himpunan yang sama, tanpa angka bila mungkin, dan tetap
//! menunjuk elemen yang benar setelah dimensi awal diubah.

use ducad_engine::ops::OpFile;
use ducad_engine::select::{select_edges, select_faces, suggest_edges, suggest_faces, Robustness};
use ducad_engine::Session;
use ducad_kernel::KernelShape;

fn session(params: &[(&str, f64)], upto: usize) -> Session {
    let mut f: OpFile =
        serde_json::from_str(include_str!("fixtures/sample_bracket.ops.json")).unwrap();
    for (k, v) in params {
        f.params.insert((*k).to_string(), *v);
    }
    f.ops.truncate(upto);
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    s
}

fn shape(s: &Session) -> &KernelShape {
    &s.body("bracket").unwrap().1.shape
}

#[test]
fn inner_corner_edge_gets_semantic_selector_that_survives_edit() {
    // Hanya profil + extrude: tepi sudut dalam belum di-fillet.
    let s = session(&[], 2);
    let sh = shape(&s);
    let picked = select_edges(sh, "|Y[x=5][z=5]").unwrap();
    assert_eq!(picked.len(), 1);

    let sug = suggest_edges(sh, &picked).unwrap();
    assert_eq!(sug.robustness, Robustness::Semantic, "{sug:?}");
    assert_eq!(select_edges(sh, &sug.selector).unwrap(), picked);

    // Tebal 5 → 8: selector usulan tetap menunjuk sudut dalam (x=t, z=t).
    let s8 = session(&[("t", 8.0)], 2);
    let sh8 = shape(&s8);
    assert_eq!(
        select_edges(sh8, &sug.selector).unwrap(),
        select_edges(sh8, "|Y[x=8][z=8]").unwrap(),
        "{}",
        sug.selector
    );
}

#[test]
fn base_top_face_gets_semantic_selector_that_survives_edit() {
    let s = session(&[], 2);
    let sh = shape(&s);
    let picked = select_faces(sh, "+Z[z=5]").unwrap();
    let sug = suggest_faces(sh, &picked).unwrap();
    assert_eq!(sug.robustness, Robustness::Semantic, "{sug:?}");
    assert_eq!(select_faces(sh, &sug.selector).unwrap(), picked);

    let s8 = session(&[("t", 8.0), ("height", 50.0)], 2);
    let sh8 = shape(&s8);
    assert_eq!(
        select_faces(sh8, &sug.selector).unwrap(),
        select_faces(sh8, "+Z[z=8]").unwrap(),
        "{}",
        sug.selector
    );
}

#[test]
fn simple_cases_prefer_single_atoms() {
    let s = session(&[], 2);
    let sh = shape(&s);
    let top = select_faces(sh, ">Z").unwrap();
    assert_eq!(suggest_faces(sh, &top).unwrap().selector, ">Z");
    let along_y = select_edges(sh, "|Y").unwrap();
    assert_eq!(suggest_edges(sh, &along_y).unwrap().selector, "|Y");
}

#[test]
fn hole_walls_get_kind_selector() {
    let s = session(&[], 5);
    let sh = shape(&s);
    let walls = select_faces(sh, "all[kind=cylinder][r=2.75]").unwrap();
    assert_eq!(walls.len(), 4);
    let sug = suggest_faces(sh, &walls).unwrap();
    assert_eq!(sug.robustness, Robustness::Semantic, "{sug:?}");
    assert_eq!(select_faces(sh, &sug.selector).unwrap(), walls);
}

#[test]
fn every_single_face_and_edge_round_trips() {
    // Apa pun yang diklik, usulan harus memilih tepat elemen itu.
    let s = session(&[], 5);
    let sh = shape(&s);
    let nf = ducad_kernel::enumerate_faces(sh).len();
    let ne = ducad_kernel::enumerate_edges(sh).len();
    let mut by_kind = std::collections::BTreeMap::<Robustness, usize>::new();
    for i in 0..nf {
        let sug = suggest_faces(sh, &[i]).unwrap();
        assert_eq!(select_faces(sh, &sug.selector).unwrap(), vec![i], "{sug:?}");
        *by_kind.entry(sug.robustness).or_default() += 1;
    }
    for i in 0..ne {
        let sug = suggest_edges(sh, &[i]).unwrap();
        assert_eq!(select_edges(sh, &sug.selector).unwrap(), vec![i], "{sug:?}");
        *by_kind.entry(sug.robustness).or_default() += 1;
    }
    println!("faces {nf}, edges {ne}: {by_kind:?}");
    assert_eq!(
        by_kind.get(&Robustness::Index),
        None,
        "tidak boleh jatuh ke idx: pada bracket contoh: {by_kind:?}"
    );
}

#[test]
fn invalid_picks_return_none() {
    let s = session(&[], 2);
    let sh = shape(&s);
    assert!(suggest_faces(sh, &[]).is_none());
    assert!(suggest_edges(sh, &[9999]).is_none());
}
