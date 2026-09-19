//! Diagnosis error op + perbaikan terverifikasi (P9.2).
//!
//! Kebijakan (mengikat):
//! 1. Pre-check heuristik tidak pernah memblokir — kecuali yang pasti salah
//!    dan murah dicek (`HoleOutsideFace`, `BooleanNoOverlap` untuk
//!    subtract/intersect), yang dijalankan `Session` SEBELUM kernel.
//! 2. Setelah kernel gagal, diagnosis menentukan kode spesifik + `context`.
//! 3. Fix hanya ditawarkan bila sudah diverifikasi (op hasil patch dicoba di
//!    dalam transaksi batch yang sama lalu dibatalkan), maks 3 percobaan.
//! 4. Fix tidak pernah diterapkan otomatis.

mod boolean;
mod hole;
mod profile;
mod round;
mod shell;
mod sketch;

pub(crate) use boolean::precheck_boolean;
pub(crate) use hole::{hole_depth_warning, precheck_hole};

use crate::error::{apply_patch, OpError, OpPatch, SuggestedFix};
use crate::ops::Op;
use crate::session::SessionCore;

/// Maksimum percobaan verifikasi per error.
pub(crate) const MAX_VERIFY: usize = 3;

/// Perkaya `err` dari `op` yang gagal. `batch` = op di batch yang sedang
/// berjalan (untuk menemukan op sketch yang di-patch oleh `ProfileOpenGap`).
pub fn diagnose(core: &mut SessionCore, op: &Op, err: OpError, batch: &[Op]) -> OpError {
    match op {
        Op::Fillet { .. } | Op::Chamfer { .. } => round::diagnose(core, op, err),
        Op::Shell { .. } => shell::diagnose(core, op, err),
        Op::Extrude { .. } | Op::Revolve { .. } => profile::diagnose(core, op, err, batch),
        Op::Sketch { .. } => sketch::diagnose(core, op, err),
        _ => err,
    }
}

/// Kandidat patch → fix terverifikasi. `verify` menerima op hasil patch dan
/// mengembalikan `true` bila berhasil. Berhenti setelah `MAX_VERIFY` percobaan.
pub(crate) fn verified_fixes(
    op: &Op,
    candidates: Vec<(String, OpPatch)>,
    mut verify: impl FnMut(&Op) -> bool,
) -> Vec<SuggestedFix> {
    let mut out = Vec::new();
    for (label, patch) in candidates.into_iter().take(MAX_VERIFY) {
        let Ok(patched) = apply_patch(op, &patch) else {
            continue;
        };
        if verify(&patched) {
            out.push(SuggestedFix {
                label,
                patch,
                patched_op: patched,
                verified: true,
            });
        }
    }
    out
}
