//! Selector semantik face/tepi (P1.4): agent memilih geometri lewat
//! properti (`>Z`, `|Z`, `of(>Z)`, `all[kind=cylinder][r=2.75]`), engine
//! menerjemahkannya ke indeks `ducad_kernel::enumerate_*`.

pub mod eval;
pub mod parse;

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
