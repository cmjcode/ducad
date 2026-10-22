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
        Op::PushPull { .. } => (
            "face must select exactly 1 face; distance > 0 adds material along the outward normal, < 0 cuts; a face next to a fillet may need a smaller distance",
            r#"{"op":"push_pull","id":"pp","body":"b","face":">Z","distance":10}"#,
        ),
        Op::RevolveFace { .. } => (
            "face must be 1 planar face and axis.dir must lie in its plane; the axis must not cross the inside of the face",
            r#"{"op":"revolve_face","id":"rf","body":"b","face":">Z","axis":{"origin":[0,0,10],"dir":[1,0,0]},"angle_deg":90}"#,
        ),
        Op::RoundVertex { .. } => (
            "`near` must be within 5 mm of a body corner (take it from the bbox in inspect); radius must be smaller than the shortest edge at that corner",
            r#"{"op":"round_vertex","id":"rv","body":"b","near":[20,10,5],"radius":3}"#,
        ),
        Op::Rib { .. } => (
            "the rib must touch the body so the union succeeds; thickness and depth > 0, normal non-zero and not parallel to start→end",
            r#"{"op":"rib","id":"r","body":"b","start":[2,25,30],"end":[48,25,30],"normal":[0,0,-1],"thickness":2,"depth":20}"#,
        ),
        Op::ResizeEdge { .. } => (
            "start/end are the edge end points (end moves); length > 0. Without a planar end face perpendicular to the edge the body scales uniformly",
            r#"{"op":"resize_edge","id":"re","body":"b","start":[0,0,0],"end":[0,0,15],"length":30}"#,
        ),
        Op::ShellVariable { .. } => (
            "shell_variable needs a convex body with only planar faces; every wall thickness must leave a cavity",
            r#"{"op":"shell_variable","id":"sv","body":"b","thickness":2,"remove_faces":">Z","custom":[{"faces":"<Z","thickness":4}]}"#,
        ),
        Op::Adopt { .. } => (
            "adopt stores frozen GUI geometry; a brep written by another kernel version cannot be read — redo the edit with a parametric op",
            r#"{"op":"adopt","id":"a","body":"b","brep":"ducad-truck/1:<base64>"}"#,
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
            r#"{"op":"push_pull","id":"x","body":"b","face":">Z","distance":1}"#,
            r#"{"op":"revolve_face","id":"x","body":"b","face":">Z","axis":{"origin":[0,0,0],"dir":[1,0,0]}}"#,
            r#"{"op":"round_vertex","id":"x","body":"b","near":[0,0,0],"radius":1,"chamfer":true}"#,
            r#"{"op":"rib","id":"x","body":"b","start":[0,0,0],"end":[1,0,0],"normal":[0,0,1],"thickness":1,"depth":1}"#,
            r#"{"op":"resize_edge","id":"x","body":"b","start":[0,0,0],"end":[1,0,0],"length":2}"#,
            r#"{"op":"shell_variable","id":"x","body":"b","thickness":1}"#,
            r#"{"op":"adopt","id":"x","body":"b","brep":"x"}"#,
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
