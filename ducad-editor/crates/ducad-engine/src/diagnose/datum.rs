//! Datum (Fase 1 butir 1.2/1.3): `construct_axis` / `construct_point` yang
//! gagal selalu membawa `hint` — pesan dari `compute::datum` sudah spesifik;
//! di sini hanya dilengkapi bila hint-nya kosong (mis. selector kosong,
//! body tidak dikenal) dengan contoh bentuk JSON yang sah.

use serde_json::json;

use crate::error::{OpError, OpErrorCode};
use crate::ops::Op;

/// Contoh `by` yang sah per jenis op datum.
const AXIS_EXAMPLE: &str = "construct_axis by: {\"two_points\":{\"a\":[0,0,0],\"b\":[0,0,10]}}, {\"edge\":{\"body\":\"b\",\"edge\":\"|Z[x=0][y=0]\"}}, {\"normal_to_face\":{\"body\":\"b\",\"face\":\">Z\"}}, {\"through_cylinder\":{\"body\":\"b\",\"face\":\"all[kind=cylinder]\"}}, {\"two_planes\":{\"a\":\"XZ\",\"b\":\"YZ\"}}";
const POINT_EXAMPLE: &str = "construct_point by: {\"vertex\":{\"body\":\"b\",\"near\":[0,0,10]}}, {\"center\":{\"body\":\"b\",\"edge\":\"of(>Z)[kind=circle]\"}}, {\"edge_plane\":{\"body\":\"b\",\"edge\":\"|Z\",\"plane\":{\"base\":\"XY\",\"offset\":5}}}, {\"two_edges\":{\"a\":\"axis:a1\",\"b\":\"axis:a2\"}}, {\"three_planes\":{\"a\":\"XY\",\"b\":\"XZ\",\"c\":\"YZ\"}}, {\"along_path\":{\"body\":\"b\",\"edge\":\"...\",\"fraction\":0.5}}";

pub(super) fn diagnose(op: &Op, mut err: OpError) -> OpError {
    let example = match op {
        Op::ConstructAxis { .. } => AXIS_EXAMPLE,
        Op::ConstructPoint { .. } => POINT_EXAMPLE,
        _ => return err,
    };
    if err.hint.is_none() {
        let lead = match err.code {
            OpErrorCode::SelectorEmpty | OpErrorCode::SelectorSyntax => {
                "test the selector with query_geometry first; "
            }
            OpErrorCode::UnknownRef => "use a body name from inspect / an existing datum id; ",
            _ => "",
        };
        err.hint = Some(format!("{lead}valid forms: {example}"));
    }
    if err.context.is_null() {
        err.context = json!({ "datum_op": op.kind() });
    }
    err
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_missing_hint_only() {
        let op: Op = serde_json::from_str(
            r#"{"op":"construct_axis","id":"a","by":{"edge":{"body":"b","edge":"|Z"}}}"#,
        )
        .unwrap();
        let e = diagnose(&op, OpError::new(OpErrorCode::SelectorEmpty, "kosong"));
        assert!(e.hint.as_deref().unwrap_or_default().contains("query_geometry"));
        let e = diagnose(&op, OpError::invalid("x").with_hint("tetap"));
        assert_eq!(e.hint.as_deref(), Some("tetap"));
    }
}
