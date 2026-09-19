//! Sketch: constraint redundan/bertentangan → fix hapus constraint.

use serde_json::json;

use super::{verified_fixes, MAX_VERIFY};
use crate::error::{OpError, OpErrorCode, OpPatch};
use crate::ops::sketch::build_sketch;
use crate::ops::Op;
use crate::session::SessionCore;

pub(super) fn diagnose(core: &mut SessionCore, op: &Op, mut err: OpError) -> OpError {
    let Op::Sketch {
        id, constraints, ..
    } = op
    else {
        return err;
    };
    if err.code != OpErrorCode::OverConstrained || constraints.is_empty() {
        return err;
    }
    // Indeks redundan dari analyze_dof (sketch baru: indeks = urutan spec);
    // bila tidak ada (geometri runtuh), coba dari constraint terakhir.
    let mut order: Vec<usize> = err
        .context
        .get("redundant")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_u64().map(|i| i as usize))
                .collect()
        })
        .unwrap_or_default();
    if order.is_empty() {
        order = (0..constraints.len()).rev().collect();
    }
    order.retain(|i| *i < constraints.len());
    order.truncate(MAX_VERIFY);
    let listed: Vec<serde_json::Value> = order
        .iter()
        .map(|i| json!({ "index": i, "constraint": constraints[*i] }))
        .collect();
    if let Some(ctx) = err.context.as_object_mut() {
        ctx.insert("candidates".into(), json!(listed));
    } else {
        err.context = json!({ "candidates": listed });
    }
    let params = core.meta.design.params.clone();
    let candidates = order
        .iter()
        .map(|i| {
            let desc = serde_json::to_string(&constraints[*i]).unwrap_or_default();
            let patch = OpPatch {
                op_id: id.clone(),
                set: Default::default(),
                remove: vec![format!("/constraints/{i}")],
            };
            (format!("Hapus constraint #{i} {desc}"), patch)
        })
        .collect();
    err.fixes = verified_fixes(op, candidates, |patched| match patched {
        Op::Sketch {
            entities,
            constraints,
            ..
        } => build_sketch(entities, constraints, &params).is_ok(),
        _ => false,
    });
    err
}
