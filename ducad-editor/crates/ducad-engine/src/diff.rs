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
    /// Perubahan lembar gambar tersimpan (P21.6): jumlah/label saja.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawings: Vec<DrawingChange>,
    /// Dampak revisi (W4): lembar paket, check, dan persyaratan yang
    /// terpengaruh. `None` bila tidak ada yang terdampak.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<Impact>,
}

/// Lembar paket yang isinya berubah akibat revisi.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SheetImpact {
    /// `cover`, `drawing`, `part`, `assembly`, `flat_pattern`, `bom`, `compliance`.
    pub kind: &'static str,
    /// Nama lembar/body yang dimaksud (kosong untuk lembar tunggal).
    pub name: String,
    pub reason: String,
}

/// Check yang hasilnya berubah.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckImpact {
    pub check: String,
    pub old: Option<crate::check::CheckStatus>,
    pub new: Option<crate::check::CheckStatus>,
    /// Pesan hasil baru (menyebut angka).
    pub message: String,
}

/// Persyaratan yang status keterlacakannya berubah.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RequirementImpact {
    pub id: String,
    pub old: Option<crate::brief::TraceStatus>,
    pub new: Option<crate::brief::TraceStatus>,
}

/// Dampak sebuah revisi terhadap paket desain.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Impact {
    /// Lembar paket (susunan sisi BARU) yang harus diterbitkan ulang.
    pub sheets: Vec<SheetImpact>,
    /// Lembar paket yang tidak terpengaruh.
    pub unchanged_sheets: usize,
    /// Check yang statusnya berubah (lulus ↔ gagal/error).
    pub checks: Vec<CheckImpact>,
    /// Check yang nilainya berubah tetapi statusnya tetap.
    pub checks_remeasured: usize,
    pub requirements: Vec<RequirementImpact>,
}

impl Impact {
    pub fn is_empty(&self) -> bool {
        self.sheets.is_empty()
            && self.checks.is_empty()
            && self.checks_remeasured == 0
            && self.requirements.is_empty()
    }
}

fn check_key(r: &crate::check::CheckResult) -> String {
    r.id.clone()
        .unwrap_or_else(|| format!("{}#{}", r.kind, r.index))
}

/// Hitung dampak revisi `a` → `b` dari diff body + hasil check kedua sisi.
fn impact_of(a: &Session, b: &Session, bodies: &[BodyDiff], drawings: &[DrawingChange]) -> Impact {
    use crate::package::SheetSpec;
    let changed: BTreeSet<&str> = bodies
        .iter()
        .filter(|d| d.status != "unchanged")
        .map(|d| d.name.as_str())
        .collect();
    let membership = bodies.iter().any(|d| d.status == "added" || d.status == "removed");

    let (ra, rb) = (a.run_checks(None).results, b.run_checks(None).results);
    let old: BTreeMap<String, &crate::check::CheckResult> =
        ra.iter().map(|r| (check_key(r), r)).collect();
    let new: BTreeMap<String, &crate::check::CheckResult> =
        rb.iter().map(|r| (check_key(r), r)).collect();
    let keys: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    let mut checks = Vec::new();
    let mut remeasured = 0usize;
    for key in keys {
        let (o, n) = (old.get(key), new.get(key));
        let (so, sn) = (o.map(|r| r.status), n.map(|r| r.status));
        if so != sn {
            checks.push(CheckImpact {
                check: key.clone(),
                old: so,
                new: sn,
                message: n.or(o).map(|r| r.message.clone()).unwrap_or_default(),
            });
        } else if o.map(|r| &r.measured) != n.map(|r| &r.measured) {
            remeasured += 1;
        }
    }

    let trace = |s: &Session, results: &[crate::check::CheckResult]| {
        s.design()
            .brief
            .as_ref()
            .map(|brief| crate::brief::traceability(brief, results))
    };
    let (ta, tb) = (trace(a, &ra), trace(b, &rb));
    let status_of = |t: &Option<crate::brief::Traceability>, id: &str| {
        t.as_ref()
            .and_then(|t| t.rows.iter().find(|r| r.id == id))
            .map(|r| r.status)
    };
    let ids: BTreeSet<String> = [&ta, &tb]
        .into_iter()
        .flatten()
        .flat_map(|t| t.rows.iter().map(|r| r.id.clone()))
        .collect();
    let requirements: Vec<RequirementImpact> = ids
        .into_iter()
        .filter_map(|id| {
            let (o, n) = (status_of(&ta, &id), status_of(&tb, &id));
            (o != n).then_some(RequirementImpact { id, old: o, new: n })
        })
        .collect();

    let compliance_changed = !checks.is_empty() || remeasured > 0 || !requirements.is_empty();
    let plan = match b.design().package.as_ref().filter(|p| !p.sheets.is_empty()) {
        Some(p) => p.sheets.clone(),
        None => crate::package::plan_sheets(b.model(), b.meta()),
    };
    let mut sheets = Vec::new();
    let mut unchanged = 0usize;
    for sheet in &plan {
        let hit: Option<(&'static str, String, String)> = match sheet {
            SheetSpec::Part { body } => changed
                .contains(body.as_str())
                .then(|| ("part", body.clone(), "body geometry changed".to_string())),
            SheetSpec::FlatPattern { body } => changed
                .contains(body.as_str())
                .then(|| ("flat_pattern", body.clone(), "body geometry changed".to_string())),
            SheetSpec::Assembly => (!changed.is_empty()).then(|| {
                ("assembly", String::new(), format!("{} body(ies) changed", changed.len()))
            }),
            SheetSpec::Drawing { name } => {
                let spec = b.design().drawing(name);
                let spec_changed = drawings.iter().any(|d| &d.name == name);
                let geometry = spec.is_some_and(|d| {
                    if d.bodies.is_empty() {
                        !changed.is_empty()
                    } else {
                        d.bodies.iter().any(|n| changed.contains(n.as_str()))
                    }
                });
                match (spec_changed, geometry) {
                    (true, _) => Some(("drawing", name.clone(), "sheet definition changed".to_string())),
                    (false, true) => Some(("drawing", name.clone(), "body geometry changed".to_string())),
                    _ => None,
                }
            }
            SheetSpec::Bom => (!changed.is_empty()).then(|| {
                let reason = if membership { "parts added or removed" } else { "part mass changed" };
                ("bom", String::new(), reason.to_string())
            }),
            SheetSpec::Compliance => compliance_changed
                .then(|| ("compliance", String::new(), "check results changed".to_string())),
            // Sampul memuat ringkasan kepatuhan, biaya, dan tabel revisi.
            SheetSpec::Cover => (compliance_changed || !changed.is_empty()).then(|| {
                ("cover", String::new(), "revision table and summary".to_string())
            }),
        };
        match hit {
            Some((kind, name, reason)) => sheets.push(SheetImpact { kind, name, reason }),
            None => unchanged += 1,
        }
    }
    Impact {
        sheets,
        unchanged_sheets: unchanged,
        checks,
        checks_remeasured: remeasured,
        requirements,
    }
}

/// Perubahan satu lembar gambar: `added`, `removed`, atau `changed`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DrawingChange {
    pub name: String,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<crate::drawing_auto::DrawingSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<crate::drawing_auto::DrawingSummary>,
}

fn diff_drawings(a: &crate::session::DesignDoc, b: &crate::session::DesignDoc) -> Vec<DrawingChange> {
    let names: BTreeSet<&String> = a
        .drawings
        .iter()
        .chain(b.drawings.iter())
        .map(|d| &d.name)
        .collect();
    names
        .into_iter()
        .filter_map(|name| {
            let old = a.drawing(name).map(crate::drawing_auto::summarize);
            let new = b.drawing(name).map(crate::drawing_auto::summarize);
            let status = match (&old, &new) {
                (None, Some(_)) => "added",
                (Some(_), None) => "removed",
                (Some(x), Some(y)) if x != y => "changed",
                _ => return None,
            };
            Some(DrawingChange {
                name: name.clone(),
                status,
                old,
                new,
            })
        })
        .collect()
}

impl DesignDiff {
    /// Tidak ada perbedaan sama sekali.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
            && self.ops.is_empty()
            && self.bodies.iter().all(|b| b.status == "unchanged")
            && self.drawings.is_empty()
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
    // Pratinjau: batas waktu pendek; lewat batas → peringatan, bukan beku (B1-1).
    match compute::solid::boolean_with_budget(
        a,
        b,
        BooleanKind::Subtract,
        compute::budget::PREVIEW_BUDGET,
    ) {
        Ok(g) => (Some(g.shape.volume().abs()), Some(g.shape)),
        Err(e) if e.code == OpErrorCode::EmptyResult => (Some(0.0), None),
        Err(e) => {
            warnings.push(format!("diff geometris body '{name}' gagal: {}", e.message));
            (None, None)
        }
    }
}

/// Salinan (nama, uuid, shape, bbox, volume) semua body model.
pub(crate) fn snapshot_bodies(m: &crate::model::ModelDoc) -> Vec<BodySnapshot> {
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
}

/// Snapshot geometri seluruh body untuk dibandingkan nanti (Compare
/// Bodies di GUI).
pub struct ModelSnapshot(Vec<BodySnapshot>);

impl ModelSnapshot {
    pub fn body_count(&self) -> usize {
        self.0.len()
    }
}

/// Ambil snapshot model saat ini.
pub fn snapshot_model(m: &crate::model::ModelDoc) -> ModelSnapshot {
    ModelSnapshot(snapshot_bodies(m))
}

/// Bandingkan snapshot lama dengan model sekarang (diff geometris).
/// Snapshot tetap utuh sehingga bisa dipakai berulang.
pub fn diff_against_snapshot(
    old: &ModelSnapshot,
    current: &crate::model::ModelDoc,
) -> (Vec<BodyDiff>, DiffShapes, Vec<String>) {
    let old_copy: Vec<BodySnapshot> = old
        .0
        .iter()
        .filter_map(|(n, u, s, b, v)| Some((n.clone(), u.clone(), ducad_kernel::clone_shape(s).ok()?, *b, *v)))
        .collect();
    diff_bodies(old_copy, snapshot_bodies(current), true)
}

/// Diff body lama vs baru (dicocokkan lewat nama, lalu uuid).
pub(crate) fn diff_bodies(
    old: Vec<BodySnapshot>,
    mut new: Vec<BodySnapshot>,
    geometric: bool,
) -> (Vec<BodyDiff>, DiffShapes, Vec<String>) {
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
    (bodies, shapes, warnings)
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

    let (bodies, shapes, warnings) = diff_bodies(
        snapshot_bodies(a.model()),
        snapshot_bodies(b.model()),
        geometric,
    );
    let drawings = diff_drawings(da, db);
    let impact = Some(impact_of(a, b, &bodies, &drawings)).filter(|i| !i.is_empty());
    (
        DesignDiff {
            params,
            ops,
            bodies,
            warnings,
            drawings,
            impact,
        },
        shapes,
    )
}
