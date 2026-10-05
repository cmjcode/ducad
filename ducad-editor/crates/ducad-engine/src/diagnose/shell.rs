//! Shell: tebal ≥ setengah dimensi terkecil body, atau kedalaman rongga
//! menembus dinding dasar.

use std::collections::BTreeMap;

use serde_json::json;

use super::verified_fixes;
use crate::error::{OpError, OpErrorCode, OpPatch};
use crate::ops::{eval, Op};
use crate::session::SessionCore;

pub(super) fn diagnose(core: &mut SessionCore, op: &Op, err: OpError) -> OpError {
    let Op::Shell {
        id,
        body,
        thickness,
        ..
    } = op
    else {
        return err;
    };
    if err.code == OpErrorCode::ShellDepthTooDeep {
        return depth_fixes(core, op, id, err);
    }
    if err.code != OpErrorCode::KernelFailed {
        return err;
    }
    let Ok(t) = eval(thickness, &core.meta.design.effective_params()) else {
        return err;
    };
    let Ok((_, geo)) = core.body(body) else {
        return err;
    };
    let Some((lo, hi)) = geo.mesh.bounding_box() else {
        return err;
    };
    let min_dim = (0..3)
        .map(|i| (hi[i] - lo[i]) as f64)
        .fold(f64::MAX, f64::min);
    if t < 0.5 * min_dim {
        return err;
    }
    let mut out = OpError::new(
        OpErrorCode::ShellTooThick,
        format!("tebal shell {t} mm ≥ setengah dimensi terkecil body '{body}' ({min_dim:.3} mm)"),
    )
    .with_hint(format!("pakai tebal < {:.2} mm", 0.5 * min_dim))
    .with_context(json!({ "thickness": t, "min_dimension": min_dim }));
    out.op_index = err.op_index;
    out.op_id = err.op_id.clone();
    let candidates = [0.5 * t, 0.25 * t]
        .into_iter()
        .map(|v| {
            let v = (v * 100.0).round() / 100.0;
            let mut set = BTreeMap::new();
            set.insert("/thickness".to_string(), json!(v));
            (
                format!("Pakai tebal {v} mm"),
                OpPatch {
                    op_id: id.clone(),
                    set,
                    remove: vec![],
                },
            )
        })
        .collect();
    out.fixes = verified_fixes(op, candidates, |patched| core.try_op(patched));
    out
}

/// Kedalaman terlalu dalam: tawarkan rongga penuh (hapus `depth`) dan
/// setengah kedalaman maksimum.
fn depth_fixes(core: &mut SessionCore, op: &Op, id: &str, mut err: OpError) -> OpError {
    let mut candidates = vec![(
        "Pakai rongga penuh (tanpa depth)".to_string(),
        OpPatch {
            op_id: id.to_string(),
            set: BTreeMap::new(),
            remove: vec!["/depth".to_string()],
        },
    )];
    if let Some(max) = err.context.get("max_depth").and_then(|v| v.as_f64()) {
        let v = (0.5 * max * 100.0).round() / 100.0;
        if v > 0.0 {
            let mut set = BTreeMap::new();
            set.insert("/depth".to_string(), json!(v));
            candidates.push((
                format!("Pakai kedalaman {v} mm"),
                OpPatch {
                    op_id: id.to_string(),
                    set,
                    remove: vec![],
                },
            ));
        }
    }
    err.fixes = verified_fixes(op, candidates, |patched| core.try_op(patched));
    err
}
