//! Diff desain (P8.2): perubahan params, op (dicocokkan lewat `id`), dan
//! geometri body (dicocokkan lewat nama, lalu `uuid`).

use std::collections::{BTreeMap, BTreeSet};

use ducad_kernel::KernelShape;
use serde::Serialize;

use crate::compute;
use crate::error::OpErrorCode;
use crate::inspect::round4;
use crate::model::{BodyGeometry, BooleanKind};
use crate::ops::Op;
use crate::session::Session;

#[derive(Debug, Clone, Serialize)]
pub struct ParamChange {
    pub name: String,
    pub old: Option<f64>,
    pub new: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldChange {
    /// JSON pointer, mis. `"/radius"`.
    pub path: String,
    pub old: serde_json::Value,
    pub new: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum OpChange {
    Added {
        index: usize,
        op: Op,
    },
    Removed {
        index: usize,
        op: Op,
    },
    Changed {
        id: String,
        fields: Vec<FieldChange>,
    },
    Reordered {
        id: String,
        from: usize,
        to: usize,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct BodyDiff {
    pub name: String,
    /// `"added" | "removed" | "changed" | "unchanged"`.
    pub status: &'static str,
    pub volume_old: Option<f64>,
    pub volume_new: Option<f64>,
    pub added_volume: Option<f64>,
    pub removed_volume: Option<f64>,
    pub bbox_old: Option<[[f64; 3]; 2]>,
    pub bbox_new: Option<[[f64; 3]; 2]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesignDiff {
    pub params: Vec<ParamChange>,
    pub ops: Vec<OpChange>,
    pub bodies: Vec<BodyDiff>,
    pub warnings: Vec<String>,
}

impl DesignDiff {
    /// Tidak ada perbedaan sama sekali.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
            && self.ops.is_empty()
            && self.bodies.iter().all(|b| b.status == "unchanged")
    }
}

/// Shape selisih untuk render diff berwarna.
#[derive(Default)]
pub struct DiffShapes {
    pub added: Vec<KernelShape>,
    pub removed: Vec<KernelShape>,
}

/// Kumpulkan daun JSON yang berbeda antara `a` dan `b` sebagai JSON pointer.
fn json_diff(path: &str, a: &serde_json::Value, b: &serde_json::Value, out: &mut Vec<FieldChange>) {
    use serde_json::Value;
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            for k in keys {
                let p = format!("{path}/{}", k.replace('~', "~0").replace('/', "~1"));
                json_diff(
                    &p,
                    x.get(k).unwrap_or(&Value::Null),
                    y.get(k).unwrap_or(&Value::Null),
                    out,
                );
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            for i in 0..x.len().max(y.len()) {
                json_diff(
                    &format!("{path}/{i}"),
                    x.get(i).unwrap_or(&Value::Null),
                    y.get(i).unwrap_or(&Value::Null),
                    out,
                );
            }
        }
        _ if a != b => out.push(FieldChange {
            path: path.to_string(),
            old: a.clone(),
            new: b.clone(),
        }),
        _ => {}
    }
}

/// Subsekuens umum terpanjang atas id (untuk mendeteksi op yang berpindah).
fn lcs(a: &[&str], b: &[&str]) -> BTreeSet<String> {
    let (n, m) = (a.len(), b.len());
    let mut t = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if a[i] == b[j] {
                t[i + 1][j + 1] + 1
            } else {
                t[i + 1][j].max(t[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, BTreeSet::new());
    while i < n && j < m {
        if a[i] == b[j] {
            out.insert(a[i].to_string());
            i += 1;
            j += 1;
        } else if t[i + 1][j] >= t[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

fn diff_ops(a: &[Op], b: &[Op]) -> Vec<OpChange> {
    let idx_a: BTreeMap<&str, usize> = a.iter().enumerate().map(|(i, o)| (o.id(), i)).collect();
    let idx_b: BTreeMap<&str, usize> = b.iter().enumerate().map(|(i, o)| (o.id(), i)).collect();
    let mut out = Vec::new();
    for (i, op) in a.iter().enumerate() {
        if !idx_b.contains_key(op.id()) {
            out.push(OpChange::Removed {
                index: i,
                op: op.clone(),
            });
        }
    }
    for (i, op) in b.iter().enumerate() {
        match idx_a.get(op.id()) {
            None => out.push(OpChange::Added {
                index: i,
                op: op.clone(),
            }),
            Some(&j) => {
                let (va, vb) = (serde_json::to_value(&a[j]), serde_json::to_value(op));
                if let (Ok(va), Ok(vb)) = (va, vb) {
                    let mut fields = Vec::new();
                    json_diff("", &va, &vb, &mut fields);
                    if !fields.is_empty() {
                        out.push(OpChange::Changed {
                            id: op.id().to_string(),
                            fields,
                        });
                    }
                }
            }
        }
    }
    let common_a: Vec<&str> = a
        .iter()
        .map(Op::id)
        .filter(|id| idx_b.contains_key(id))
        .collect();
    let common_b: Vec<&str> = b
        .iter()
        .map(Op::id)
        .filter(|id| idx_a.contains_key(id))
        .collect();
    let stable = lcs(&common_a, &common_b);
    for id in common_b.iter().filter(|id| !stable.contains(**id)) {
        out.push(OpChange::Reordered {
            id: id.to_string(),
            from: idx_a[id],
            to: idx_b[id],
        });
    }
    out
}

/// (nama, uuid, salinan shape, bbox, volume) satu body.
type BodySnapshot = (String, String, KernelShape, Option<[[f64; 3]; 2]>, f64);

fn bbox(g: &BodyGeometry) -> Option<[[f64; 3]; 2]> {
    let (min, max) = g.mesh.bounding_box()?;
    Some([min.map(|v| v as f64), max.map(|v| v as f64)])
}

fn volume_of_difference(
    a: &KernelShape,
    b: &KernelShape,
    warnings: &mut Vec<String>,
    name: &str,
) -> (Option<f64>, Option<KernelShape>) {
    match compute::boolean(a, b, BooleanKind::Subtract) {
        Ok(g) => (Some(g.shape.volume().abs()), Some(g.shape)),
        Err(e) if e.code == OpErrorCode::EmptyResult => (Some(0.0), None),
        Err(e) => {
            warnings.push(format!("diff geometris body '{name}' gagal: {}", e.message));
            (None, None)
        }
    }
}

/// Bandingkan dua sesi. `geometric = true` menghitung volume tambah/hilang
/// dengan boolean kernel (kegagalan kernel → `None` + peringatan, bukan error).
pub fn diff(a: &Session, b: &Session, geometric: bool) -> (DesignDiff, DiffShapes) {
    let (da, db) = (a.design(), b.design());
    let names: BTreeSet<&String> = da.params.keys().chain(db.params.keys()).collect();
    let params = names
        .into_iter()
        .filter_map(|n| {
            let (o, w) = (da.params.get(n).copied(), db.params.get(n).copied());
            (o != w).then(|| ParamChange {
                name: n.clone(),
                old: o,
                new: w,
            })
        })
        .collect();
    let ops = diff_ops(&da.oplog, &db.oplog);

    let bodies_of = |s: &Session| -> Vec<BodySnapshot> {
        let m = s.model();
        m.doc
            .bodies
            .iter()
            .filter_map(|(id, body)| {
                let g = m.geometry.get(id)?;
                let shape = ducad_kernel::clone_shape(&g.shape).ok()?;
                Some((
                    body.name.clone(),
                    body.uuid.clone(),
                    shape,
                    bbox(g),
                    g.shape.volume().abs(),
                ))
            })
            .collect()
    };
    let old = bodies_of(a);
    let mut new = bodies_of(b);
    let mut warnings = Vec::new();
    let mut shapes = DiffShapes::default();
    let mut bodies = Vec::new();

    for (name, uuid, shape, bb, vol) in old {
        let pos = new
            .iter()
            .position(|n| n.0 == name)
            .or_else(|| new.iter().position(|n| n.1 == uuid));
        let Some(pos) = pos else {
            bodies.push(BodyDiff {
                name,
                status: "removed",
                volume_old: Some(round4(vol)),
                volume_new: None,
                added_volume: None,
                removed_volume: Some(round4(vol)),
                bbox_old: bb,
                bbox_new: None,
            });
            if geometric {
                shapes.removed.push(shape);
            }
            continue;
        };
        let (_, _, nshape, nbb, nvol) = new.remove(pos);
        let same_bbox = match (bb, nbb) {
            (Some(x), Some(y)) => (0..2).all(|k| (0..3).all(|i| (x[k][i] - y[k][i]).abs() <= 1e-6)),
            (None, None) => true,
            _ => false,
        };
        let unchanged = (vol - nvol).abs() / vol.abs().max(1e-12) < 1e-9 && same_bbox;
        let (mut added, mut removed) = (None, None);
        if !unchanged && geometric {
            let (av, ashape) = volume_of_difference(&nshape, &shape, &mut warnings, &name);
            let (rv, rshape) = volume_of_difference(&shape, &nshape, &mut warnings, &name);
            if av.is_none() || rv.is_none() {
                added = None;
                removed = None;
            } else {
                added = av.map(round4);
                removed = rv.map(round4);
            }
            shapes.added.extend(ashape);
            shapes.removed.extend(rshape);
        }
        bodies.push(BodyDiff {
            name,
            status: if unchanged { "unchanged" } else { "changed" },
            volume_old: Some(round4(vol)),
            volume_new: Some(round4(nvol)),
            added_volume: added,
            removed_volume: removed,
            bbox_old: bb,
            bbox_new: nbb,
        });
    }
    for (name, _, shape, bb, vol) in new {
        bodies.push(BodyDiff {
            name,
            status: "added",
            volume_old: None,
            volume_new: Some(round4(vol)),
            added_volume: Some(round4(vol)),
            removed_volume: None,
            bbox_old: None,
            bbox_new: bb,
        });
        if geometric {
            shapes.added.push(shape);
        }
    }
    (
        DesignDiff {
            params,
            ops,
            bodies,
            warnings,
        },
        shapes,
    )
}
