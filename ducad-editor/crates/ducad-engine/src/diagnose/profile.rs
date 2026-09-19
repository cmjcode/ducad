//! Extrude/revolve: profil hampir tertutup (dua ujung menggantung < 0,5 mm).

use std::collections::BTreeMap;

use ducad_sketch::{Entity, EntityId, Sketch};
use glam::DVec2;
use serde_json::json;

use super::verified_fixes;
use crate::compute::LIN_TOL;
use crate::error::{OpError, OpErrorCode, OpPatch};
use crate::ops::sketch::build_sketch;
use crate::ops::{EntitySpec, Op, Params};
use crate::plane::PlaneFrame;
use crate::session::SessionCore;

/// Batas celah yang dianggap "hampir tertutup".
const GAP_MAX: f64 = 0.5;

#[derive(Clone, Copy, PartialEq)]
enum End {
    Start,
    End,
}

fn dangling_line_ends(sk: &Sketch) -> Vec<(EntityId, End, DVec2)> {
    let mut ends: Vec<(EntityId, End, DVec2)> = Vec::new();
    let mut others: Vec<DVec2> = Vec::new();
    for (id, e) in sk.entities.iter() {
        if e.is_construction() || sk.is_hidden(id) {
            continue;
        }
        match e {
            Entity::Line { start, end, .. } => {
                ends.push((id, End::Start, *start));
                ends.push((id, End::End, *end));
            }
            other => others.extend(other.endpoints()),
        }
    }
    let all: Vec<DVec2> = ends.iter().map(|e| e.2).chain(others).collect();
    ends.into_iter()
        .filter(|(_, _, p)| {
            all.iter()
                .filter(|q| (**q - *p).length() <= LIN_TOL)
                .count()
                <= 1
        })
        .collect()
}

/// Pointer JSON ke koordinat ujung entitas `name` di dalam op sketch.
fn endpoint_pointer(entities: &[EntitySpec], name: &str, end: End) -> Option<String> {
    for (i, spec) in entities.iter().enumerate() {
        let default_name = format!("e{}", i + 1);
        match spec {
            EntitySpec::Line { name: n, .. } if n.as_deref().unwrap_or(&default_name) == name => {
                let field = if end == End::Start { "from" } else { "to" };
                return Some(format!("/entities/{i}/line/{field}"));
            }
            EntitySpec::Polyline {
                name: n,
                points,
                closed,
                ..
            } => {
                let base = n.as_deref().unwrap_or(&default_name);
                let Some(k) = name
                    .strip_prefix(&format!("{base}."))
                    .and_then(|k| k.parse::<usize>().ok())
                else {
                    continue;
                };
                let idx = if end == End::Start { k } else { k + 1 };
                let idx = if *closed && idx == points.len() {
                    0
                } else {
                    idx
                };
                return (idx < points.len())
                    .then(|| format!("/entities/{i}/polyline/points/{idx}"));
            }
            _ => {}
        }
    }
    None
}

fn find_sketch_op<'a>(core: &'a SessionCore, batch: &'a [Op], id: &str) -> Option<&'a Op> {
    batch
        .iter()
        .chain(core.meta.design.oplog.iter())
        .find(|o| matches!(o, Op::Sketch { .. }) && o.id() == id)
}

/// Verifikasi murni: sketch hasil patch dibangun ulang lalu profil op
/// dijalankan lewat compute (tanpa menyentuh model).
fn verify(patched_sketch: &Op, op: &Op, frame: &PlaneFrame, params: &Params) -> bool {
    let Op::Sketch {
        entities,
        constraints,
        ..
    } = patched_sketch
    else {
        return false;
    };
    let Ok(built) = build_sketch(entities, constraints, params) else {
        return false;
    };
    let sk = built.sketch;
    match op {
        Op::Extrude {
            profile,
            distance,
            direction,
            ..
        } => {
            let Ok(d) = crate::ops::eval(distance, params) else {
                return false;
            };
            let extent = crate::session::extent_for(*direction, d);
            let Ok(ids) = crate::session::profile_entities(&sk, profile) else {
                return false;
            };
            let Ok(pick) = crate::session::profile_pick(profile, &ids, params) else {
                return false;
            };
            crate::compute::extrude(&sk, &pick, frame, extent).is_ok()
        }
        Op::Revolve { .. } => !ducad_sketch::find_region_hierarchy(&sk).is_empty(),
        _ => false,
    }
}

pub(super) fn diagnose(core: &mut SessionCore, op: &Op, err: OpError, batch: &[Op]) -> OpError {
    let sketch_id = match op {
        Op::Extrude { sketch, .. } | Op::Revolve { sketch, .. } => sketch.clone(),
        _ => return err,
    };
    if err.code != OpErrorCode::ProfileNotClosed {
        return err;
    }
    let Ok((sk, frame)) = core.sketch(&sketch_id).map(|(s, f)| (s.clone(), *f)) else {
        return err;
    };
    let dangling = dangling_line_ends(&sk);
    let [(ea, end_a, pa), (eb, end_b, pb)] = dangling.as_slice() else {
        return err;
    };
    let gap = (*pa - *pb).length();
    if gap >= GAP_MAX {
        return err;
    }
    let name = |id: &EntityId| {
        sk.entity_names
            .get(id)
            .cloned()
            .unwrap_or_else(|| "?".into())
    };
    let suffix = |e: End| if e == End::Start { "start" } else { "end" };
    let (na, nb) = (name(ea), name(eb));
    let mut out = OpError::new(
        OpErrorCode::ProfileOpenGap,
        format!(
            "profil sketch '{sketch_id}' hampir tertutup: celah {gap:.3} mm antara '{na}.{}' dan '{nb}.{}'",
            suffix(*end_a),
            suffix(*end_b)
        ),
    )
    .with_hint("samakan koordinat kedua ujung, atau tambahkan constraint coincident")
    .with_context(json!({
        "sketch": sketch_id,
        "distance": gap,
        "a": { "entity": na, "end": suffix(*end_a), "point": [pa.x, pa.y] },
        "b": { "entity": nb, "end": suffix(*end_b), "point": [pb.x, pb.y] },
    }));
    out.op_index = err.op_index;
    out.op_id = err.op_id.clone();

    let Some(sketch_op) = find_sketch_op(core, batch, &sketch_id).cloned() else {
        return out;
    };
    let Op::Sketch { entities, .. } = &sketch_op else {
        return out;
    };
    let Some(ptr) = endpoint_pointer(entities, &nb, *end_b) else {
        return out;
    };
    let mut set = BTreeMap::new();
    set.insert(ptr, json!([pa.x, pa.y]));
    let patch = OpPatch {
        op_id: sketch_id.clone(),
        set,
        remove: vec![],
    };
    let label = format!(
        "Tutup celah: pindahkan '{nb}.{}' ke ({:.3}, {:.3})",
        suffix(*end_b),
        pa.x,
        pa.y
    );
    let params = core.meta.design.params.clone();
    out.fixes = verified_fixes(&sketch_op, vec![(label, patch)], |patched| {
        verify(patched, op, &frame, &params)
    });
    out
}
