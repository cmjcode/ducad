//! Evaluasi `SelExpr` atas `FaceInfo`/`EdgeInfo` (P0.4).

use std::collections::{BTreeMap, BTreeSet};

use ducad_kernel::{EdgeInfo, EdgeKind, FaceInfo, SurfaceKind};

use super::parse::{Axis, Base, Cmp, Filter, Key, SelExpr, SetOp, Value};
use crate::compute::LIN_TOL;
use crate::error::{OpError, OpErrorCode, OpResult};

/// Toleransi sudut kesejajaran/ketegaklurusan (konvensi §4).
pub const ANG_TOL_DEG: f64 = 0.5;

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn face_kind_name(k: SurfaceKind) -> &'static str {
    match k {
        SurfaceKind::Plane => "plane",
        SurfaceKind::Cylinder => "cylinder",
        SurfaceKind::Cone => "cone",
        SurfaceKind::Sphere => "sphere",
        SurfaceKind::Torus => "torus",
        SurfaceKind::Other => "other",
    }
}

pub(crate) fn edge_kind_name(k: EdgeKind) -> &'static str {
    match k {
        EdgeKind::Line => "line",
        EdgeKind::Circle => "circle",
        EdgeKind::Other => "other",
    }
}

/// Ringkasan jumlah face/tepi per jenis — dimuat di `context.available`
/// saat selector kosong, agar agent bisa memperbaiki selector-nya.
pub(crate) fn available_summary(
    faces: &[FaceInfo],
    edges: Option<&[EdgeInfo]>,
) -> serde_json::Value {
    let mut fk: BTreeMap<&str, usize> = BTreeMap::new();
    for f in faces {
        *fk.entry(face_kind_name(f.kind)).or_default() += 1;
    }
    let mut out = serde_json::json!({ "faces": fk });
    if let Some(edges) = edges {
        let mut ek: BTreeMap<&str, usize> = BTreeMap::new();
        for e in edges {
            *ek.entry(edge_kind_name(e.kind)).or_default() += 1;
        }
        out["edges"] = serde_json::json!(ek);
    }
    out
}

/// Satu elemen yang bisa diseleksi — abstraksi bersama face/tepi.
trait Item {
    fn pos(&self) -> [f64; 3];
    fn kind_name(&self) -> &'static str;
    fn radius(&self) -> Option<f64>;
    fn area(&self) -> Option<f64>;
    fn len(&self) -> Option<f64>;
}

impl Item for FaceInfo {
    fn pos(&self) -> [f64; 3] {
        self.centroid
    }
    fn kind_name(&self) -> &'static str {
        face_kind_name(self.kind)
    }
    fn radius(&self) -> Option<f64> {
        self.radius
    }
    fn area(&self) -> Option<f64> {
        Some(self.area)
    }
    fn len(&self) -> Option<f64> {
        None
    }
}

impl Item for EdgeInfo {
    fn pos(&self) -> [f64; 3] {
        self.mid
    }
    fn kind_name(&self) -> &'static str {
        edge_kind_name(self.kind)
    }
    fn radius(&self) -> Option<f64> {
        self.radius
    }
    fn area(&self) -> Option<f64> {
        None
    }
    fn len(&self) -> Option<f64> {
        Some(self.length)
    }
}

fn passes<T: Item>(item: &T, f: &Filter) -> bool {
    let num = match f.key {
        Key::Kind => {
            return matches!(&f.value, Value::Ident(k) if k == item.kind_name());
        }
        Key::Area => item.area(),
        Key::Len => item.len(),
        Key::R => item.radius(),
        Key::X => Some(item.pos()[0]),
        Key::Y => Some(item.pos()[1]),
        Key::Z => Some(item.pos()[2]),
    };
    let (Some(a), Value::Num(b)) = (num, &f.value) else {
        return false;
    };
    match f.cmp {
        Cmp::Eq => (a - b).abs() <= 1e-3,
        Cmp::Lt => a < *b,
        Cmp::Gt => a > *b,
        Cmp::Le => a <= *b,
        Cmp::Ge => a >= *b,
    }
}

/// Indeks semua item yang nilai kuncinya ekstrem (maks/min) dengan
/// toleransi `tie(best)`.
fn extreme_by<T>(
    items: &[T],
    key: impl Fn(&T) -> f64,
    max: bool,
    tie: impl Fn(f64) -> f64,
) -> BTreeSet<usize> {
    let Some(best) = items.iter().map(&key).fold(None, |acc: Option<f64>, v| {
        Some(match acc {
            None => v,
            Some(a) if max => a.max(v),
            Some(a) => a.min(v),
        })
    }) else {
        return BTreeSet::new();
    };
    let tol = tie(best);
    (0..items.len())
        .filter(|&i| (key(&items[i]) - best).abs() <= tol)
        .collect()
}

struct Ctx<'a> {
    faces: &'a [FaceInfo],
    edges: Option<&'a [EdgeInfo]>,
}

fn cos_tol() -> f64 {
    ANG_TOL_DEG.to_radians().cos()
}

fn sin_tol() -> f64 {
    ANG_TOL_DEG.to_radians().sin()
}

fn eval_face_set(expr: &SelExpr, ctx: &Ctx) -> OpResult<BTreeSet<usize>> {
    match expr {
        SelExpr::Binary { op, lhs, rhs } => {
            let (a, b) = (eval_face_set(lhs, ctx)?, eval_face_set(rhs, ctx)?);
            Ok(set_op(*op, a, b))
        }
        SelExpr::Term { base, filters } => {
            let faces = ctx.faces;
            let set: BTreeSet<usize> = match base {
                Base::All => (0..faces.len()).collect(),
                Base::Largest | Base::Smallest => extreme_by(
                    faces,
                    |f| f.area,
                    matches!(base, Base::Largest),
                    |best| best.abs() * 1e-6,
                ),
                Base::Extreme(a, max) => {
                    extreme_by(faces, |f| f.centroid[a.index()], *max, |_| LIN_TOL)
                }
                Base::Facing(a, plus) => {
                    let mut u = a.unit();
                    if !plus {
                        u = u.map(|c| -c);
                    }
                    (0..faces.len())
                        .filter(|&i| dot(faces[i].normal, u) >= cos_tol())
                        .collect()
                }
                Base::Parallel(a) => (0..faces.len())
                    .filter(|&i| dot(faces[i].normal, a.unit()).abs() >= cos_tol())
                    .collect(),
                Base::Perpendicular(a) => (0..faces.len())
                    .filter(|&i| dot(faces[i].normal, a.unit()).abs() <= sin_tol())
                    .collect(),
                Base::Idx(list) => list.iter().copied().filter(|&i| i < faces.len()).collect(),
                Base::Group(inner) => eval_face_set(inner, ctx)?,
                Base::Longest | Base::Shortest | Base::Of(_) => {
                    return Err(OpError::new(
                        OpErrorCode::SelectorSyntax,
                        "bentuk selector ini hanya berlaku untuk tepi",
                    ))
                }
            };
            Ok(set
                .into_iter()
                .filter(|&i| filters.iter().all(|f| passes(&faces[i], f)))
                .collect())
        }
    }
}

fn eval_edge_set(expr: &SelExpr, ctx: &Ctx) -> OpResult<BTreeSet<usize>> {
    let edges = ctx.edges.unwrap_or(&[]);
    match expr {
        SelExpr::Binary { op, lhs, rhs } => {
            let (a, b) = (eval_edge_set(lhs, ctx)?, eval_edge_set(rhs, ctx)?);
            Ok(set_op(*op, a, b))
        }
        SelExpr::Term { base, filters } => {
            let line_dot = |e: &EdgeInfo, a: Axis| -> Option<f64> {
                (e.kind == EdgeKind::Line).then_some(())?;
                e.dir.map(|d| dot(d, a.unit()).abs())
            };
            let set: BTreeSet<usize> = match base {
                Base::All => (0..edges.len()).collect(),
                Base::Longest | Base::Shortest => extreme_by(
                    edges,
                    |e| e.length,
                    matches!(base, Base::Longest),
                    |_| LIN_TOL,
                ),
                Base::Extreme(a, max) => extreme_by(edges, |e| e.mid[a.index()], *max, |_| LIN_TOL),
                Base::Parallel(a) => (0..edges.len())
                    .filter(|&i| line_dot(&edges[i], *a).is_some_and(|d| d >= cos_tol()))
                    .collect(),
                Base::Perpendicular(a) => (0..edges.len())
                    .filter(|&i| line_dot(&edges[i], *a).is_some_and(|d| d <= sin_tol()))
                    .collect(),
                Base::Idx(list) => list.iter().copied().filter(|&i| i < edges.len()).collect(),
                Base::Of(inner) => {
                    let faces = eval_face_set(inner, ctx)?;
                    faces
                        .iter()
                        .flat_map(|&f| ctx.faces[f].edge_indices.iter().copied())
                        .filter(|&i| i < edges.len())
                        .collect()
                }
                Base::Group(inner) => eval_edge_set(inner, ctx)?,
                Base::Largest | Base::Smallest | Base::Facing(..) => {
                    return Err(OpError::new(
                        OpErrorCode::SelectorSyntax,
                        "bentuk selector ini hanya berlaku untuk face",
                    ))
                }
            };
            Ok(set
                .into_iter()
                .filter(|&i| filters.iter().all(|f| passes(&edges[i], f)))
                .collect())
        }
    }
}

fn set_op(op: SetOp, a: BTreeSet<usize>, b: BTreeSet<usize>) -> BTreeSet<usize> {
    match op {
        SetOp::Or => a.union(&b).copied().collect(),
        SetOp::And => a.intersection(&b).copied().collect(),
        SetOp::Except => a.difference(&b).copied().collect(),
    }
}

pub(crate) fn empty_error(src: &str, available: serde_json::Value) -> OpError {
    OpError::new(
        OpErrorCode::SelectorEmpty,
        format!("selector \"{src}\" tidak cocok dengan elemen mana pun"),
    )
    .with_hint(
        "periksa context.available untuk jenis face/tepi yang ada, atau uji dengan query_geometry",
    )
    .with_context(serde_json::json!({ "selector": src, "available": available }))
}

/// Evaluasi selector face. Hasil terurut naik; kosong → `SelectorEmpty`.
pub fn eval_faces(expr: &SelExpr, faces: &[FaceInfo]) -> OpResult<Vec<usize>> {
    let set = eval_face_set(expr, &Ctx { faces, edges: None })?;
    if set.is_empty() {
        return Err(empty_error(
            &format!("{expr:?}"),
            available_summary(faces, None),
        ));
    }
    Ok(set.into_iter().collect())
}

/// Evaluasi selector tepi (`of(...)` memakai `faces`).
pub fn eval_edges(expr: &SelExpr, faces: &[FaceInfo], edges: &[EdgeInfo]) -> OpResult<Vec<usize>> {
    let set = eval_edge_set(
        expr,
        &Ctx {
            faces,
            edges: Some(edges),
        },
    )?;
    if set.is_empty() {
        return Err(empty_error(
            &format!("{expr:?}"),
            available_summary(faces, Some(edges)),
        ));
    }
    Ok(set.into_iter().collect())
}
