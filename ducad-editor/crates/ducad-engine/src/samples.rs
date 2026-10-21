//! Proyek contoh bawaan (Step 5 `docs/PLAN_DESIGNER_UX.md`).
//!
//! Bracket L ini adalah workflow sasaran designer: sketsa berdimensi param,
//! extrude, fillet, dan lubang M5. Tutorial GUI, perintah "Buka proyek
//! contoh", dan regression test (`tests/sample_project.rs`) memakai berkas
//! yang sama, jadi yang diajarkan tutorial selalu yang diuji CI.

use crate::error::{OpError, OpResult};
use crate::ops::OpFile;
use crate::session::DesignDoc;

/// OpFile JSON bracket contoh (`params` + `ops`).
pub const BRACKET_OPS_JSON: &str = include_str!("../tests/fixtures/sample_bracket.ops.json");

/// Id op yang menandai dokumen berisi bracket contoh.
pub const BRACKET_MARKER_OPS: [&str; 2] = ["profile", "wall_holes"];

/// Bracket contoh sebagai `OpFile`.
pub fn bracket() -> OpResult<OpFile> {
    serde_json::from_str(BRACKET_OPS_JSON)
        .map_err(|e| OpError::invalid(format!("bracket contoh rusak: {e}")))
}

/// Bracket contoh sebagai `DesignDoc` siap di-replay.
pub fn bracket_design() -> OpResult<DesignDoc> {
    let f = bracket()?;
    Ok(DesignDoc {
        params: f.params,
        oplog: f.ops,
        ..DesignDoc::default()
    })
}

/// `design` berisi bracket contoh (mungkin sudah direvisi).
pub fn is_bracket(design: &DesignDoc) -> bool {
    BRACKET_MARKER_OPS
        .iter()
        .all(|id| design.oplog.iter().any(|op| op.id() == *id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bracket_replays_to_one_valid_body() {
        let design = bracket_design().unwrap();
        assert!(is_bracket(&design));
        let s = crate::Session::replay(design).expect("bracket contoh di-replay");
        let sum = s.summary();
        assert_eq!(sum.bodies.len(), 1);
        assert!(sum.bodies[0].valid);
        assert!(!is_bracket(&DesignDoc::default()));
    }
}
