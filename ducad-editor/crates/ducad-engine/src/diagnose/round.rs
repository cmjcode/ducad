//! Fillet/chamfer: radius melebihi tepi tetangga terpendek.

use std::collections::BTreeMap;

use serde_json::json;

use super::verified_fixes;
use crate::compute::LIN_TOL;
use crate::error::{OpError, OpErrorCode, OpPatch};
use crate::ops::{eval, Op};
use crate::session::SessionCore;

fn close(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= LIN_TOL)
}

pub(super) fn diagnose(core: &mut SessionCore, op: &Op, err: OpError) -> OpError {
    let (is_fillet, id, body, edges, value, field) = match op {
        Op::Fillet {
            id,
            body,
            edges,
            radius,
            ..
        } => (true, id, body, edges, radius, "/radius"),
        Op::Chamfer {
            id,
            body,
            edges,
            distance,
            ..
        } => (false, id, body, edges, distance, "/distance"),
        _ => return err,
    };
    if err.code != OpErrorCode::KernelFailed {
        return err;
    }
    let Ok(r) = eval(value, &core.meta.design.effective_params()) else {
        return err;
    };
    let Ok((_, geo)) = core.body(body) else {
        return err;
    };
    let Ok(selected) = crate::select::select_edges(&geo.shape, edges) else {
        return err;
    };
    let all = ducad_kernel::enumerate_edges(&geo.shape);
    let mut limit = f64::MAX;
    let mut limiting: Option<usize> = None;
    let mut shortest = f64::MAX;
    for &e in &selected {
        let ends = [all[e].start, all[e].end];
        for n in all.iter().filter(|n| n.index != e) {
            let touches = ends.iter().any(|p| close(*p, n.start) || close(*p, n.end));
            if !touches {
                continue;
            }
            shortest = shortest.min(n.length);
            let l = if selected.contains(&n.index) {
                n.length * 0.5
            } else {
                n.length
            };
            if l < limit {
                limit = l;
                limiting = Some(n.index);
            }
        }
    }
    if limit == f64::MAX || r <= limit {
        return err;
    }
    let (code, what, noun) = if is_fillet {
        (OpErrorCode::FilletRadiusTooLarge, "radius", "radius fillet")
    } else {
        (OpErrorCode::ChamferTooLarge, "distance", "jarak chamfer")
    };
    let mut out = OpError::new(
        code,
        format!("{noun} {r} mm melebihi batas tepi tetangga {limit:.3} mm pada body '{body}'"),
    )
    .with_hint(format!(
        "kecilkan {noun} ≤ {limit:.2} mm atau pilih tepi lain"
    ))
    .with_context({
        let mut ctx = json!({
            "limit": limit,
            "shortest_edge": shortest,
            "limiting_edge": limiting,
            "edges": selected,
        });
        ctx[what] = json!(r);
        ctx
    });
    out.op_index = err.op_index;
    out.op_id = err.op_id.clone();
    let first = (0.9 * limit * 10.0).floor() / 10.0;
    let candidates: Vec<(String, OpPatch)> = [first, first / 2.0, first / 4.0]
        .into_iter()
        .filter(|v| *v > 0.0)
        .map(|v| {
            let mut set = BTreeMap::new();
            set.insert(field.to_string(), json!(v));
            (
                format!("Pakai {noun} {v} mm"),
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
