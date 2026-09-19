//! Solver constraint dengan pola aman: klon → tambah → solve → commit.

use ducad_sketch::constraint::{self, Constraint, DofReport, SolveResult};
use ducad_sketch::{Entity, EntityId, Sketch};

use super::LIN_TOL;
use crate::error::{OpError, OpErrorCode, OpResult};

/// Ukuran karakteristik entitas: panjang garis, atau radius lingkaran/busur.
fn entity_size(e: &Entity) -> Option<f64> {
    match e {
        Entity::Line { start, end, .. } => Some((*end - *start).length()),
        Entity::Circle { radius, .. } | Entity::Arc { radius, .. } => Some(*radius),
        _ => None,
    }
}

/// Entitas yang sebelum solve berukuran nyata tetapi sesudahnya runtuh
/// (garis jadi titik, radius jadi nol). Solver kuadrat-terkecil bisa
/// "memenuhi" constraint yang saling bertentangan — mis. Horizontal +
/// Vertical pada garis yang sama — dengan meruntuhkan geometri, dan
/// `analyze_dof` tidak menandainya redundan karena Jacobian-nya tetap
/// berpangkat penuh. Ditemukan lewat tes, bukan teori.
fn collapsed_entities(before: &Sketch, after: &Sketch) -> Vec<EntityId> {
    after
        .entities
        .iter()
        .filter_map(|(id, e)| {
            let old = before.entities.get(id).and_then(entity_size)?;
            let new = entity_size(e)?;
            (old > LIN_TOL && new <= LIN_TOL).then_some(id)
        })
        .collect()
}

/// Klon `sketch`, tambahkan `extra` ke daftar constraint-nya, lalu solve.
/// Sketch hasil dikembalikan HANYA bila solver konvergen dan tidak ada
/// constraint baru yang redundan/konflik; sketch asli tidak pernah disentuh.
///
/// - Tidak konvergen → `ConstraintUnsolved` (context memuat residual).
/// - Geometri runtuh (garis jadi titik, radius nol) atau salah satu
///   constraint `extra` redundan menurut `analyze_dof` → `OverConstrained`.
pub fn solve_with(
    sketch: &Sketch,
    extra: &[Constraint],
) -> OpResult<(Sketch, SolveResult, DofReport)> {
    let mut trial = sketch.clone();
    let base = trial.constraints.len();
    trial.constraints.extend(extra.iter().cloned());
    let snapshot = trial.constraints.clone();
    let result = constraint::solve(&mut trial, &snapshot);
    if !result.converged {
        return Err(OpError::new(
            OpErrorCode::ConstraintUnsolved,
            format!(
                "Constraint gagal diselesaikan (sisa residual {:.4})",
                result.final_residual_norm
            ),
        )
        .with_hint("hapus constraint yang saling bertentangan atau ubah nilai dimensinya")
        .with_context(serde_json::json!({
            "residual": result.final_residual_norm,
            "iterations": result.iterations,
        })));
    }
    let collapsed = collapsed_entities(sketch, &trial);
    if !collapsed.is_empty() {
        return Err(OpError::new(
            OpErrorCode::OverConstrained,
            format!(
                "Constraint hanya terpenuhi dengan meruntuhkan {} entitas menjadi berukuran nol",
                collapsed.len()
            ),
        )
        .with_hint("constraint yang ditambahkan saling bertentangan; hapus salah satunya")
        .with_context(serde_json::json!({ "collapsed": collapsed.len() })));
    }
    let dof = constraint::analyze_dof(&trial, &snapshot);
    let new_redundant: Vec<usize> = dof
        .redundant
        .iter()
        .copied()
        .filter(|&i| i >= base)
        .collect();
    if !new_redundant.is_empty() {
        return Err(OpError::new(
            OpErrorCode::OverConstrained,
            format!(
                "Constraint baru redundan atau bertentangan (indeks {new_redundant:?}, DOF tersisa {})",
                dof.dof
            ),
        )
        .with_hint("constraint tersebut tidak menambah informasi; hapus salah satunya")
        .with_context(serde_json::json!({
            "redundant": new_redundant,
            "dof": dof.dof,
            "residual": result.final_residual_norm,
        })));
    }
    Ok((trial, result, dof))
}
