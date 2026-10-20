//! Op Fase 2 (direct editing & alat solid): error yang belum membawa `hint`
//! dilengkapi saran langkah berikutnya + contoh bentuk JSON yang sah.

use serde_json::json;

use crate::error::{OpError, OpErrorCode};
use crate::ops::Op;

/// (saran bila op gagal, contoh JSON) per jenis op.
fn advice(op: &Op) -> Option<(&'static str, &'static str)> {
    Some(match op {
        Op::OffsetFace { .. } => (
            "offset_face moves faces of bodies made of planes/cylinders/cones/spheres; for a face whose neighbours would change topology use a smaller distance, or extrude a sketch on the face instead",
            r#"{"op":"offset_face","id":"o","body":"b","faces":">Z","distance":5}"#,
        ),
        Op::DeleteFace { .. } => (
            "delete_face heals the gap by extending PLANAR neighbours; select whole features (all faces of a fillet / boss / hole), e.g. \"all[kind=cylinder][r=2]\"",
            r#"{"op":"delete_face","id":"d","body":"b","faces":"all[kind=cylinder]"}"#,
        ),
        Op::ReplaceFace { .. } => (
            "face must be 1 planar face; `with` is any sketch plane form and must not be perpendicular to the face",
            r#"{"op":"replace_face","id":"r","body":"b","face":">Z","with":{"base":"XY","offset":20}}"#,
        ),
        Op::SplitFace { .. } => (
            "the plane / sketch curves must cross the selected faces; omit `faces` to split every face the plane crosses",
            r#"{"op":"split_face","id":"s","body":"b","faces":">Z","tool":{"plane":{"base":"YZ","offset":10}}}"#,
        ),
        Op::Pipe { .. } => (
            "path = open sketch chain or ≥ 2 points; 0 < wall < r; tight bends need a bend radius ≥ r",
            r#"{"op":"pipe","id":"p","path":{"points":[[0,0,0],[0,0,50]]},"r":5,"wall":1}"#,
        ),
        Op::Web { .. } => (
            "the sketch needs line entities (one wall each); walls grow along the sketch normal (`reverse` flips) and must touch the body",
            r#"{"op":"web","id":"w","body":"b","sketch":"web_sk","thickness":2,"depth":10}"#,
        ),
        Op::Emboss { .. } => (
            "the sketch needs closed regions; deboss cuts against the sketch normal, so put the sketch ON the face (e.g. plane {\"body\":\"b\",\"face\":\">Z\"})",
            r#"{"op":"emboss","id":"e","sketch":"logo","body":"b","depth":1,"deboss":true}"#,
        ),
        Op::Align { .. } => (
            "`from` and `to.face` must each select 1 face of the same kind (planar–planar or cylinder–cylinder) on two different bodies",
            r#"{"op":"align","id":"a","body":"lid","from":"<Z","to":{"body":"box","face":">Z"}}"#,
        ),
        Op::BoundaryFill { .. } => (
            "each point must lie inside at least one bounding body or inside a closed cavity between them",
            r#"{"op":"boundary_fill","id":"f","bodies":["a","b"],"points":[[5,5,5]]}"#,
        ),
        Op::BoundingSolid { .. } => (
            "margin ≥ 0; for a cylinder give a non-zero axis",
            r#"{"op":"bounding_solid","id":"bb","body":"b","shape":"cylinder","axis":[0,0,1],"margin":2}"#,
        ),
        Op::Plastic { .. } => (
            "plastic features stand on a planar face (selector of exactly 1 face) except rest; sizes left out follow the dfm-injection rules",
            r#"{"op":"plastic","id":"b","body":"case","make":{"boss":{"face":"+Z[z=2]","at":[[0,0]],"height":8,"hole_d":2.5}}}"#,
        ),
        Op::Surface { .. } => (
            "surface ops work on sheets: sketch curves (open or closed) for extrude/revolve/sweep/loft, open boundary edges for patch/ruled/extend/blend; select edges with query_geometry first",
            r#"{"op":"surface","id":"s","make":{"extrude":{"sketch":"sk","end":20}}}"#,
        ),
        Op::Perforate { .. } => (
            "perforate works on a body made by base_flange / convert_to_sheet; pitch must exceed d and holes need room inside the plate after `margin` (default max(d, 2t+R))",
            r#"{"op":"perforate","id":"pf","body":"tray","d":4,"pitch":8,"margin":6}"#,
        ),
        Op::ConvertToSheet { .. } => (
            "convert_to_sheet takes a flat plate of uniform thickness with round through holes; bent parts must be rebuilt with base_flange + edge_flange",
            r#"{"op":"convert_to_sheet","id":"cv","body":"plate","bend_radius":2}"#,
        ),
        _ => return None,
    })
}

pub(super) fn diagnose(op: &Op, mut err: OpError) -> OpError {
    let Some((tip, example)) = advice(op) else {
        return err;
    };
    if err.hint.is_none() {
        let lead = match err.code {
            OpErrorCode::SelectorEmpty | OpErrorCode::SelectorSyntax => {
                "test the selector with query_geometry first; "
            }
            OpErrorCode::UnknownRef => "use a body/sketch name from inspect; ",
            _ => "",
        };
        err.hint = Some(format!("{lead}{tip}. Example: {example}"));
    }
    if err.context.is_null() {
        err.context = json!({ "op_kind": op.kind() });
    }
    err
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_direct_op_gets_a_hint() {
        let ops = [
            r#"{"op":"offset_face","id":"x","body":"b","faces":">Z","distance":1}"#,
            r#"{"op":"delete_face","id":"x","body":"b","faces":">Z"}"#,
            r#"{"op":"replace_face","id":"x","body":"b","face":">Z","with":"XY"}"#,
            r#"{"op":"split_face","id":"x","body":"b","tool":{"sketch":"s"}}"#,
            r#"{"op":"pipe","id":"x","path":"s","r":1}"#,
            r#"{"op":"web","id":"x","body":"b","sketch":"s","thickness":1,"depth":1}"#,
            r#"{"op":"emboss","id":"x","sketch":"s","body":"b","depth":1}"#,
            r#"{"op":"align","id":"x","body":"b","from":">Z","to":{"body":"c","face":"<Z"}}"#,
            r#"{"op":"boundary_fill","id":"x","bodies":["b"],"points":[[0,0,0]]}"#,
            r#"{"op":"bounding_solid","id":"x","body":"b"}"#,
            r#"{"op":"perforate","id":"x","body":"b","d":3,"pitch":6}"#,
            r#"{"op":"convert_to_sheet","id":"x","body":"b"}"#,
        ];
        for src in ops {
            let op: Op = serde_json::from_str(src).unwrap();
            let e = diagnose(&op, OpError::invalid("gagal"));
            assert!(
                e.hint.as_deref().is_some_and(|h| h.contains("Example")),
                "{src}"
            );
            let kept = diagnose(&op, OpError::invalid("x").with_hint("tetap"));
            assert_eq!(kept.hint.as_deref(), Some("tetap"));
        }
    }
}
