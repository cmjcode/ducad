//! Selector semantik face/tepi (P1.4): agent memilih geometri lewat
//! properti (`>Z`, `|Z`, `of(>Z)`, `all[kind=cylinder][r=2.75]`), engine
//! menerjemahkannya ke indeks `ducad_kernel::enumerate_*`.

pub mod eval;
pub mod parse;

/// Ringkasan tata bahasa & semantik selector untuk agent (≤ 40 baris).
pub const SELECTOR_CHEATSHEET: &str = "\
Face/edge selectors (case-insensitive; LIN_TOL 1e-4 mm, ANG_TOL 0.5 degrees)
selector := term (or|and|except term)*        -- left-associative, equal precedence
term     := base filter*
BASE                 FACE                               EDGE
all                  all faces                          all edges
>A / <A              max/min centroid position on A     max/min midpoint position on A
+A / -A              outward normal along +A/-A         (n/a)
|A                   normal parallel to A               line parallel to A
#A                   normal perpendicular to A          line perpendicular to A
largest / smallest   max/min area                       (n/a)
longest / shortest   (n/a)                              max/min length
idx:i,j              enumeration indices                enumeration indices
of(S)                (n/a)                              edges of the faces matched by S
(S)                  grouping                           grouping
A = X | Y | Z
FILTER [KEY CMP VALUE], CMP = = < > <= >=, '=' numeric with tolerance 1e-3
kind   face: plane|cylinder|cone|sphere|torus|other   edge: line|circle|other
area   face area (mm2)              len  edge length (mm)
r      radius (cylinder/cone/sphere face, circle edge)
x y z  component of face centroid / edge midpoint
Results sorted ascending. Empty -> selector_empty + context.available (count per kind).
EXAMPLES
>Z                          topmost face
+Z[z=5]                     face facing +Z at height 5
#Z                          side walls of a box
|Z                          vertical edges (for corner fillets)
of(>Z)                      perimeter edges of the top face
of(>Z) and |X               top edges parallel to X
all except |Z               all edges except vertical ones
all[kind=cylinder][r=2.75]  walls of M5 clearance holes
";

pub use eval::{eval_edges, eval_faces, ANG_TOL_DEG};
pub use parse::{parse, SelCtx, SelExpr};

use ducad_kernel::KernelShape;

use crate::error::{OpErrorCode, OpResult};

/// Ganti selector di pesan `SelectorEmpty` dengan teks aslinya (evaluator
/// hanya melihat AST).
fn with_source(mut e: crate::OpError, src: &str) -> crate::OpError {
    if e.code == OpErrorCode::SelectorEmpty {
        let available = e.context.get("available").cloned().unwrap_or_default();
        e = eval::empty_error(src, available);
    }
    e
}

/// Parse + enumerasi + evaluasi selector face pada `shape`.
pub fn select_faces(shape: &KernelShape, src: &str) -> OpResult<Vec<usize>> {
    let expr = parse(src, SelCtx::Face)?;
    let faces = ducad_kernel::enumerate_faces(shape);
    eval_faces(&expr, &faces).map_err(|e| {
        if e.code == OpErrorCode::SelectorEmpty {
            let edges = ducad_kernel::enumerate_edges(shape);
            return eval::empty_error(src, eval::available_summary(&faces, Some(&edges)));
        }
        e
    })
}

/// Parse + enumerasi + evaluasi selector tepi pada `shape`.
pub fn select_edges(shape: &KernelShape, src: &str) -> OpResult<Vec<usize>> {
    let expr = parse(src, SelCtx::Edge)?;
    let faces = ducad_kernel::enumerate_faces(shape);
    let edges = ducad_kernel::enumerate_edges(shape);
    eval_edges(&expr, &faces, &edges).map_err(|e| with_source(e, src))
}
