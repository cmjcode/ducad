//! Pembuat selector dari face/tepi yang dipilih pengguna (ADR 0010 butir 3).
//!
//! GUI mengetahui indeks enumerasi yang diklik; oplog butuh selector yang
//! tetap benar setelah dimensi berubah. Fungsi di sini mencari selector yang
//! memilih himpunan yang SAMA PERSIS, dengan urutan preferensi:
//!
//! 1. [`Robustness::Semantic`] — tanpa angka: `>Z`, `+Z except >Z`,
//!    `of(+Z except >Z) and of(+X except >X)`, `all[kind=cylinder]`.
//! 2. [`Robustness::Numeric`] — memakai nilai koordinat/ukuran
//!    (`+Z[z=5]`, `[r=2.75]`): benar sekarang, putus bila dimensi itu
//!    berubah (B0-3) — gagal eksplisit, tidak memilih yang lain.
//! 3. [`Robustness::Index`] — `idx:` sebagai jalan terakhir; urutan
//!    enumerasi bisa berubah setelah edit apa pun.
//!
//! Setiap hasil diverifikasi ulang dengan evaluator yang sama dengan op.

use std::collections::BTreeSet;

use ducad_kernel::{EdgeInfo, FaceInfo, KernelShape};

use super::eval::{edge_kind_name, eval_edges, eval_faces, face_kind_name};
use super::parse::{parse, SelCtx};

/// Seberapa tahan selector terhadap edit dimensi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Robustness {
    Semantic,
    Numeric,
    Index,
}

/// Selector usulan untuk himpunan yang dipilih.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Suggestion {
    pub selector: String,
    pub robustness: Robustness,
}

type Set = BTreeSet<usize>;

const AXES: [&str; 3] = ["X", "Y", "Z"];

/// Atom face tanpa angka.
fn face_atoms(faces: &[FaceInfo]) -> Vec<String> {
    let mut out = Vec::new();
    for a in AXES {
        for p in [">", "<", "+", "-", "|", "#"] {
            out.push(format!("{p}{a}"));
        }
    }
    out.push("largest".into());
    out.push("smallest".into());
    let kinds: BTreeSet<&str> = faces.iter().map(|f| face_kind_name(f.kind)).collect();
    if kinds.len() > 1 {
        for k in kinds {
            out.push(format!("all[kind={k}]"));
        }
    }
    out
}

/// Atom tepi tanpa angka.
fn edge_atoms(edges: &[EdgeInfo]) -> Vec<String> {
    let mut out = Vec::new();
    for a in AXES {
        for p in [">", "<", "|", "#"] {
            out.push(format!("{p}{a}"));
        }
    }
    out.push("longest".into());
    out.push("shortest".into());
    let kinds: BTreeSet<&str> = edges.iter().map(|e| edge_kind_name(e.kind)).collect();
    if kinds.len() > 1 {
        for k in kinds {
            out.push(format!("all[kind={k}]"));
        }
    }
    out
}

fn eval_face_str(src: &str, faces: &[FaceInfo]) -> Option<Set> {
    let expr = parse(src, SelCtx::Face).ok()?;
    eval_faces(&expr, faces)
        .ok()
        .map(|v| v.into_iter().collect())
}

fn eval_edge_str(src: &str, faces: &[FaceInfo], edges: &[EdgeInfo]) -> Option<Set> {
    let expr = parse(src, SelCtx::Edge).ok()?;
    eval_edges(&expr, faces, edges)
        .ok()
        .map(|v| v.into_iter().collect())
}

/// Atom beserta himpunannya (atom kosong dibuang).
fn evaluated(atoms: Vec<String>, eval: impl Fn(&str) -> Option<Set>) -> Vec<(String, Set)> {
    atoms
        .into_iter()
        .filter_map(|a| eval(&a).map(|s| (a, s)))
        .filter(|(_, s)| !s.is_empty())
        .collect()
}

/// Cari `A`, lalu `A except B` / `A and B` yang himpunannya = `target`.
fn combine(atoms: &[(String, Set)], target: &Set) -> Option<String> {
    if let Some((a, _)) = atoms.iter().find(|(_, s)| s == target) {
        return Some(a.clone());
    }
    for (a, sa) in atoms {
        if !target.is_subset(sa) {
            continue;
        }
        for (b, sb) in atoms {
            if a == b {
                continue;
            }
            if sa.difference(sb).copied().collect::<Set>() == *target {
                return Some(format!("{a} except {b}"));
            }
            if sa.intersection(sb).copied().collect::<Set>() == *target {
                return Some(format!("{a} and {b}"));
            }
        }
    }
    None
}

fn index_selector(target: &Set) -> String {
    let list: Vec<String> = target.iter().map(|i| i.to_string()).collect();
    format!("idx:{}", list.join(","))
}

fn fmt_num(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let s = format!("{r:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

fn coord_filters(p: [f64; 3]) -> String {
    format!(
        "[x={}][y={}][z={}]",
        fmt_num(p[0]),
        fmt_num(p[1]),
        fmt_num(p[2])
    )
}

fn suggest_faces_in(faces: &[FaceInfo], target: &Set) -> Option<Suggestion> {
    if target.is_empty() || target.iter().any(|&i| i >= faces.len()) {
        return None;
    }
    let atoms = evaluated(face_atoms(faces), |s| eval_face_str(s, faces));
    if let Some(sel) = combine(&atoms, target) {
        return Some(Suggestion {
            selector: sel,
            robustness: Robustness::Semantic,
        });
    }
    // Numerik: atom arah + filter posisi/radius, untuk satu face.
    if target.len() == 1 {
        let f = &faces[*target.iter().next()?];
        let mut cands = Vec::new();
        if let Some(r) = f.radius {
            cands.push(format!(
                "all[kind={}][r={}]",
                face_kind_name(f.kind),
                fmt_num(r)
            ));
        }
        for (i, a) in AXES.iter().enumerate() {
            if f.normal[i].abs() > 0.999 {
                let sign = if f.normal[i] > 0.0 { "+" } else { "-" };
                let key = a.to_ascii_lowercase();
                cands.push(format!("{sign}{a}[{key}={}]", fmt_num(f.centroid[i])));
            }
        }
        cands.push(format!(
            "all[kind={}]{}",
            face_kind_name(f.kind),
            coord_filters(f.centroid)
        ));
        for c in cands {
            if eval_face_str(&c, faces).as_ref() == Some(target) {
                return Some(Suggestion {
                    selector: c,
                    robustness: Robustness::Numeric,
                });
            }
        }
    }
    Some(Suggestion {
        selector: index_selector(target),
        robustness: Robustness::Index,
    })
}

/// Selector face untuk `picked` (indeks `enumerate_faces`). `None` bila
/// kosong atau ada indeks di luar jangkauan.
pub fn suggest_faces(shape: &KernelShape, picked: &[usize]) -> Option<Suggestion> {
    let faces = ducad_kernel::enumerate_faces(shape);
    let target: Set = picked.iter().copied().collect();
    suggest_faces_in(&faces, &target)
}

/// Selector tepi untuk `picked` (indeks `enumerate_edges`).
pub fn suggest_edges(shape: &KernelShape, picked: &[usize]) -> Option<Suggestion> {
    let faces = ducad_kernel::enumerate_faces(shape);
    let edges = ducad_kernel::enumerate_edges(shape);
    let target: Set = picked.iter().copied().collect();
    if target.is_empty() || target.iter().any(|&i| i >= edges.len()) {
        return None;
    }
    let ev = |s: &str| eval_edge_str(s, &faces, &edges);
    let semantic = |sel: String| Suggestion {
        selector: sel,
        robustness: Robustness::Semantic,
    };

    // 1. Atom tepi dan kombinasinya.
    let atoms = evaluated(edge_atoms(&edges), ev);
    if let Some(sel) = combine(&atoms, &target) {
        return Some(semantic(sel));
    }

    // 2. Tepi milik face semantik: `of(F)`, `of(F) and <atom tepi>`.
    let face_sel: Vec<(String, Set)> = (0..faces.len())
        .filter_map(|i| {
            let s = suggest_faces_in(&faces, &Set::from([i]))?;
            (s.robustness == Robustness::Semantic).then_some(s.selector)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|f| {
            let src = format!("of({f})");
            ev(&src).map(|s| (src, s))
        })
        .collect();
    let mut pool = face_sel.clone();
    pool.extend(atoms.iter().cloned());
    if let Some(sel) = combine(&pool, &target) {
        return Some(semantic(sel));
    }

    // 3. Satu tepi = irisan tepi dua face yang mengapitnya.
    if target.len() == 1 {
        let e = *target.iter().next()?;
        let adjacent: Vec<&(String, Set)> =
            face_sel.iter().filter(|(_, s)| s.contains(&e)).collect();
        for (i, (a, sa)) in adjacent.iter().enumerate() {
            for (b, sb) in adjacent.iter().skip(i + 1) {
                if sa.intersection(sb).copied().collect::<Set>() == target {
                    let sel = format!("{a} and {b}");
                    if ev(&sel).as_ref() == Some(&target) {
                        return Some(semantic(sel));
                    }
                }
            }
        }
    }

    // 4. Numerik: arah + titik tengah, untuk satu tepi.
    if target.len() == 1 {
        let e = &edges[*target.iter().next()?];
        let mut cands = Vec::new();
        if let Some(d) = e.dir {
            for (i, a) in AXES.iter().enumerate() {
                if d[i].abs() > 0.999 {
                    let keys: Vec<String> = (0..3)
                        .filter(|&k| k != i)
                        .map(|k| format!("[{}={}]", ["x", "y", "z"][k], fmt_num(e.mid[k])))
                        .collect();
                    cands.push(format!("|{a}{}", keys.join("")));
                }
            }
        }
        cands.push(format!(
            "all[kind={}]{}",
            edge_kind_name(e.kind),
            coord_filters(e.mid)
        ));
        for c in cands {
            if ev(&c).as_ref() == Some(&target) {
                return Some(Suggestion {
                    selector: c,
                    robustness: Robustness::Numeric,
                });
            }
        }
    }

    Some(Suggestion {
        selector: index_selector(&target),
        robustness: Robustness::Index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_num_trims() {
        assert_eq!(fmt_num(5.0), "5");
        assert_eq!(fmt_num(2.75), "2.75");
        assert_eq!(fmt_num(-0.0000001), "0");
        assert_eq!(fmt_num(1.23456), "1.235");
    }
}
