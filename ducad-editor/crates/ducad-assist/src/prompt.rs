//! Perakitan prompt (P11.2).

use std::fmt::Write as _;

use ducad_engine::check::CheckResult;
use ducad_engine::{OpError, Session};

/// Prompt sistem tetap (≤ 900 token).
pub const SYSTEM_PROMPT: &str = include_str!("../prompts/system.md");

/// Batas perkiraan token (karakter / 4) pesan user.
pub const USER_TOKEN_BUDGET: usize = 3000;
/// Maksimum `lessons` yang disertakan.
pub const MAX_LESSONS: usize = 5;

/// Masukan pesan user selain sesi.
pub struct Context<'a> {
    pub instruction: &'a str,
    pub last_error: Option<&'a OpError>,
    pub failed_checks: &'a [CheckResult],
    pub lessons: &'a [String],
}

/// Perkiraan token kasar: karakter / 4.
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Rakit pesan user. Bila melebihi [`USER_TOKEN_BUDGET`]: buang `lessons`,
/// lalu ringkas oplog menjadi daftar `id: jenis`.
pub fn user_message(session: &Session, ctx: &Context) -> String {
    let full = build(session, ctx, true, false);
    if estimate_tokens(&full) <= USER_TOKEN_BUDGET {
        return full;
    }
    let no_lessons = build(session, ctx, false, false);
    if estimate_tokens(&no_lessons) <= USER_TOKEN_BUDGET {
        return no_lessons;
    }
    build(session, ctx, false, true)
}

fn build(session: &Session, ctx: &Context, lessons: bool, compact_oplog: bool) -> String {
    let mut m = String::new();
    let _ = writeln!(m, "INSTRUKSI\n{}\n", ctx.instruction.trim());

    let summary = session.summary();
    let _ = writeln!(m, "BODY");
    if summary.bodies.is_empty() {
        let _ = writeln!(m, "(belum ada body)");
    }
    for b in &summary.bodies {
        let kinds: Vec<String> = b
            .face_kinds
            .iter()
            .map(|(k, n)| format!("{k}:{n}"))
            .collect();
        let _ = writeln!(
            m,
            "- {}: volume {} mm3, ukuran {}x{}x{}, bbox min {:?} max {:?}, face {{{}}}",
            b.name,
            round2(b.volume),
            round2(b.size[0]),
            round2(b.size[1]),
            round2(b.size[2]),
            b.bbox[0].map(round2),
            b.bbox[1].map(round2),
            kinds.join(", ")
        );
    }
    for s in &summary.sketches {
        let _ = writeln!(
            m,
            "- sketch {} di {}: {} entitas, {} region",
            s.id, s.plane, s.entities, s.closed_regions
        );
    }

    let design = session.design();
    let params = serde_json::to_string(&design.params).unwrap_or_default();
    let _ = writeln!(m, "\nPARAMS\n{params}");

    let _ = writeln!(m, "\nOPLOG");
    if design.oplog.is_empty() {
        let _ = writeln!(m, "(kosong)");
    }
    for op in &design.oplog {
        if compact_oplog {
            let _ = writeln!(m, "{}: {}", op.id(), op.kind());
        } else {
            let _ = writeln!(m, "{}", serde_json::to_string(op).unwrap_or_default());
        }
    }

    if let Some(e) = ctx.last_error {
        let json = serde_json::to_string(e).unwrap_or_else(|_| e.message.clone());
        let _ = writeln!(m, "\nERROR TERAKHIR (usulanmu gagal, perbaiki)\n{json}");
    }
    if !ctx.failed_checks.is_empty() {
        let _ = writeln!(m, "\nCHECK GAGAL");
        for c in ctx.failed_checks {
            let id = c.id.clone().unwrap_or_else(|| c.kind.to_string());
            let _ = writeln!(m, "- {id}: {}", c.message);
        }
    }
    if lessons && !ctx.lessons.is_empty() {
        let _ = writeln!(m, "\nPELAJARAN");
        for l in ctx.lessons.iter().take(MAX_LESSONS) {
            let _ = writeln!(m, "- {l}");
        }
    }
    let _ = write!(m, "\nBalas HANYA satu objek JSON.");
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_fits_budget() {
        assert!(
            estimate_tokens(SYSTEM_PROMPT) <= 900,
            "{}",
            estimate_tokens(SYSTEM_PROMPT)
        );
    }
}
