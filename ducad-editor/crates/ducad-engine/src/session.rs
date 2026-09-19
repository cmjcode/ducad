//! `Session` — menjalankan oplog (`Op`) di atas model engine secara atomik
//! (P1.5). Identitas body lewat NAMA (id op pembuatnya), bukan `BodyId`
//! yang berganti saat undo/redo boolean.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use ducad_core::{BodyId, UndoStack};
use ducad_io::native::NativeBody;
use ducad_kernel::{ExtrudeExtent, KernelShape, SurfaceKind};
use ducad_sketch::{EntityId, PlaneRef, Sketch, SketchId, SketchSet};
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::compute::{self, EdgePick, FacePick, PrimitiveShape, ProfilePick};
use crate::error::{OpError, OpErrorCode, OpResult};
use crate::inspect::{summarize_state, Summary};
use crate::model::{
    AddMultipleSolidsCommand, AddSolidCommand, BodyGeometry, BooleanCommand, BooleanKind,
    DeleteBodyCommand, ModelDoc, ReplaceGeometryCommand,
};
use crate::ops::sketch::{build_sketch, resolve_plane, ResolvedPlane};
use crate::ops::{
    eval, eval_arr, is_valid_op_id, AxisSpec, BodyMode, BoolKind, ExtrudeDir, HoleKindSpec,
    HoleSpecRef, Num, Op, Params, PatternKind, PrimitiveSpec, ProfileSel,
};
use crate::plane::PlaneFrame;

/// Isi field `design` di file `.ducad`: sumber kebenaran part parametrik.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignDoc {
    /// Versi skema `DesignDoc` (= 1).
    pub schema: u32,
    #[serde(default)]
    pub params: Params,
    #[serde(default)]
    pub oplog: Vec<Op>,
    /// Persyaratan desain yang diperiksa setelah tiap batch (P7).
    #[serde(default)]
    pub checks: Vec<crate::check::CheckItem>,
    /// Body bawaan (file buatan GUI/impor) yang menjadi titik awal replay.
    #[serde(default)]
    pub base_bodies: Vec<NativeBody>,
    /// Sidik jari geometri setelah op terakhir (P1.7).
    #[serde(default)]
    pub fingerprint: String,
}

impl Default for DesignDoc {
    fn default() -> Self {
        Self {
            schema: 1,
            params: Params::new(),
            oplog: Vec::new(),
            checks: Vec::new(),
            base_bodies: Vec::new(),
            fingerprint: String::new(),
        }
    }
}

/// Hasil satu op.
#[derive(Debug, Clone, Serialize)]
pub struct OpOutcome {
    pub op_index: usize,
    pub op_id: String,
    pub kind: &'static str,
    pub created: Vec<String>,
    pub modified: Vec<String>,
    pub removed: Vec<String>,
    pub warnings: Vec<String>,
    pub detail: serde_json::Value,
}

/// Hasil satu batch `run`.
#[derive(Debug, Clone, Serialize)]
pub struct BatchReport {
    pub committed: bool,
    pub outcomes: Vec<OpOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<OpError>,
    /// Keadaan SETELAH batch (atau setelah rollback).
    pub summary: Summary,
    /// Hasil `design.checks` setelah batch ter-commit (bila ada check).
    /// Check yang gagal TIDAK membatalkan batch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks: Option<Vec<crate::check::CheckResult>>,
}

/// Metadata sesi di luar model/sketch — bagian yang diklon saat batch dan
/// dikembalikan saat rollback.
#[derive(Debug, Clone, Default)]
pub struct SessionMeta {
    pub design: DesignDoc,
    /// Id sketch → bidangnya.
    pub frames: BTreeMap<String, PlaneFrame>,
    /// Id sketch → slot di `SketchSet`.
    pub sketch_ids: BTreeMap<String, SketchId>,
    /// Nama body → id op yang meleburnya/menghapusnya.
    pub consumed: BTreeMap<String, String>,
    /// Penghitung datum (`PlaneRef::Datum(n)`), mulai 1.
    pub datum_counter: u32,
    /// Jumlah op per batch yang di-commit (untuk undo tingkat sesi).
    pub batches: Vec<usize>,
    /// Peringatan tingkat sesi (mis. `"oplog_stale"`).
    pub warnings: Vec<String>,
}

/// Pinjaman state sesi — logika `run`/`body`/`sketch` ditulis di sini agar
/// `DuCADApp` kelak bisa menjalankan op di atas state miliknya sendiri (P5).
pub struct SessionCore<'a> {
    pub model: &'a mut ModelDoc,
    pub model_undo: &'a mut UndoStack<ModelDoc>,
    pub sketches: &'a mut SketchSet,
    pub meta: &'a mut SessionMeta,
}

pub struct Session {
    model: ModelDoc,
    model_undo: UndoStack<ModelDoc>,
    sketches: SketchSet,
    meta: SessionMeta,
    redo: Vec<Vec<Op>>,
    /// Proposal tertunda (maks [`MAX_PROPOSALS`], yang tertua dibuang).
    proposals: std::collections::VecDeque<StoredProposal>,
    next_proposal: u32,
}

/// Jumlah proposal tertunda yang disimpan per sesi.
pub const MAX_PROPOSALS: usize = 8;

struct StoredProposal {
    id: String,
    ops: Vec<Op>,
    base_fingerprint: String,
}

/// Proposal perubahan (ghost preview, P8.4): batch yang sudah dijalankan
/// lalu dibatalkan, beserta diff body-nya.
#[derive(Debug, Clone, Serialize)]
pub struct Proposal {
    pub id: String,
    pub ops: Vec<Op>,
    /// `committed = false`.
    pub report: BatchReport,
    pub diff: Vec<crate::diff::BodyDiff>,
    pub base_fingerprint: String,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------
// Pencarian nama.
// ---------------------------------------------------------------------

fn find_body(model: &ModelDoc, name: &str) -> Option<BodyId> {
    model
        .doc
        .bodies
        .iter()
        .find(|(_, b)| b.name == name)
        .map(|(id, _)| id)
}

fn body_lookup<'m>(
    model: &'m ModelDoc,
    meta: &SessionMeta,
    name: &str,
) -> OpResult<(BodyId, &'m BodyGeometry)> {
    if let Some(id) = find_body(model, name) {
        if let Some(geo) = model.geometry.get(id) {
            return Ok((id, geo));
        }
    }
    if let Some(by) = meta.consumed.get(name) {
        return Err(OpError::new(
            OpErrorCode::BodyConsumed,
            format!("body '{name}' sudah dilebur/dihapus oleh op '{by}'"),
        )
        .with_hint(format!("rujuk body hasil op '{by}'"))
        .with_context(serde_json::json!({ "body": name, "consumed_by": by })));
    }
    let mut known: Vec<&str> = model.doc.bodies.values().map(|b| b.name.as_str()).collect();
    known.sort();
    Err(OpError::new(
        OpErrorCode::UnknownRef,
        format!("body '{name}' tidak dikenal (body yang ada: {known:?})"),
    )
    .with_context(serde_json::json!({ "body": name, "available": known })))
}

fn clone_shape(shape: &KernelShape) -> OpResult<KernelShape> {
    ducad_kernel::clone_shape(shape).map_err(|e| OpError::kernel("Salin shape", e))
}

// ---------------------------------------------------------------------
// Sidik jari (P1.7).
// ---------------------------------------------------------------------

/// FNV-1a 64-bit heksadesimal atas string kanonik: untuk tiap body terurut
/// nama `name|volume(6 desimal)|bbox min/max (4 desimal)|jumlah face`.
pub fn fingerprint(model: &ModelDoc) -> String {
    let mut rows: Vec<String> = model
        .doc
        .bodies
        .iter()
        .filter_map(|(id, b)| {
            let geo = model.geometry.get(id)?;
            let (min, max) = geo.mesh.bounding_box().unwrap_or(([0.0; 3], [0.0; 3]));
            let f4 = |v: f32| format!("{:.4}", v as f64);
            let faces = ducad_kernel::enumerate_faces(&geo.shape).len();
            Some(format!(
                "{}|{:.6}|{},{},{}|{},{},{}|{}",
                b.name,
                geo.shape.volume().abs(),
                f4(min[0]),
                f4(min[1]),
                f4(min[2]),
                f4(max[0]),
                f4(max[1]),
                f4(max[2]),
                faces
            ))
        })
        .collect();
    rows.sort();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in rows.join("\n").bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    format!("{h:016x}")
}

// ---------------------------------------------------------------------
// SessionCore: eksekusi op.
// ---------------------------------------------------------------------

impl SessionCore<'_> {
    pub fn body(&self, name: &str) -> OpResult<(BodyId, &BodyGeometry)> {
        body_lookup(self.model, self.meta, name)
    }

    pub fn sketch(&self, id: &str) -> OpResult<(&Sketch, &PlaneFrame)> {
        let (Some(sid), Some(frame)) = (self.meta.sketch_ids.get(id), self.meta.frames.get(id))
        else {
            let known: Vec<&str> = self.meta.sketch_ids.keys().map(String::as_str).collect();
            return Err(OpError::new(
                OpErrorCode::UnknownRef,
                format!("sketch '{id}' tidak dikenal (sketch yang ada: {known:?})"),
            )
            .with_context(serde_json::json!({ "sketch": id, "available": known })));
        };
        let sketch = self.sketches.sketch(*sid).ok_or_else(|| {
            OpError::new(
                OpErrorCode::UnknownRef,
                format!("slot sketch '{id}' hilang"),
            )
        })?;
        Ok((sketch, frame))
    }

    fn params(&self) -> &Params {
        &self.meta.design.params
    }

    fn summary(&self) -> Summary {
        summarize_state(self.model, self.sketches, self.meta, None, false, 0)
    }

    /// Jalankan `ops` secara atomik (algoritma P1.5).
    pub fn run(&mut self, ops: Vec<Op>, dry_run: bool) -> BatchReport {
        self.run_with(ops, dry_run, |_| ()).0
    }

    /// Seperti [`SessionCore::run`], plus `inspect` yang dipanggil atas model
    /// SETELAH semua op berhasil dan SEBELUM commit/rollback (dipakai
    /// `propose` untuk mengambil geometri hasil lalu membatalkannya).
    pub fn run_with<T>(
        &mut self,
        ops: Vec<Op>,
        dry_run: bool,
        inspect: impl FnOnce(&ModelDoc) -> T,
    ) -> (BatchReport, Option<T>) {
        let saved = self.meta.clone();
        self.model_undo.begin("batch");
        let mut new_sketches: Vec<SketchId> = Vec::new();
        let mut outcomes = Vec::with_capacity(ops.len());
        let mut used: HashSet<String> = self
            .meta
            .design
            .oplog
            .iter()
            .map(|o| o.id().to_string())
            .chain(self.meta.sketch_ids.keys().cloned())
            .chain(self.model.doc.bodies.values().map(|b| b.name.clone()))
            .collect();

        let mut failure: Option<OpError> = None;
        for (i, op) in ops.iter().enumerate() {
            let result = self
                .check_id(op, &used)
                .and_then(|_| self.apply(op, i, &mut new_sketches));
            match result {
                Ok(outcome) => {
                    used.insert(op.id().to_string());
                    outcomes.push(outcome);
                }
                Err(mut e) => {
                    e.op_index = Some(i);
                    e.op_id = Some(op.id().to_string());
                    // Diagnosis + fix terverifikasi, SEBELUM rollback: model
                    // masih berisi hasil op 0..i-1 dari batch ini (P9.2).
                    let e = crate::diagnose::diagnose(self, op, e, &ops);
                    failure = Some(e);
                    break;
                }
            }
        }

        let inspected = failure.is_none().then(|| inspect(self.model));
        if failure.is_some() || dry_run {
            self.model_undo.rollback(self.model);
            for sid in new_sketches {
                self.sketches.remove(sid);
            }
            *self.meta = saved;
            let report = BatchReport {
                committed: false,
                outcomes,
                error: failure,
                summary: self.summary(),
                checks: None,
            };
            return (report, inspected);
        }

        self.model_undo.commit();
        let n = ops.len();
        self.meta.design.oplog.extend(ops);
        if n > 0 {
            self.meta.batches.push(n);
        }
        self.meta.design.fingerprint = fingerprint(self.model);
        let checks = (!self.meta.design.checks.is_empty())
            .then(|| crate::check::run_checks_on(self.model, self.meta, &self.meta.design.checks));
        let report = BatchReport {
            committed: true,
            outcomes,
            error: None,
            summary: self.summary(),
            checks,
        };
        (report, inspected)
    }

    fn check_id(&self, op: &Op, used: &HashSet<String>) -> OpResult<()> {
        let id = op.id();
        if !is_valid_op_id(id) {
            return Err(OpError::invalid(format!(
                "id op '{id}' tidak valid: harus cocok ^[a-z][a-z0-9_]{{0,31}}$"
            )));
        }
        if used.contains(id) {
            return Err(OpError::new(
                OpErrorCode::DuplicateId,
                format!("id '{id}' sudah dipakai op, sketch, atau body lain"),
            ));
        }
        Ok(())
    }

    /// Coba `op` di atas keadaan saat ini lalu batalkan seluruh efeknya
    /// (dipakai verifikasi fix P9). Undo stack asli ditukar sementara dengan
    /// stack kosong sehingga transaksi batch yang sedang terbuka tidak
    /// tersentuh.
    pub(crate) fn try_op(&mut self, op: &Op) -> bool {
        let saved_meta = self.meta.clone();
        let mut temp = UndoStack::default();
        std::mem::swap(self.model_undo, &mut temp);
        self.model_undo.begin("verifikasi");
        let mut new_sketches = Vec::new();
        let ok = self.apply(op, 0, &mut new_sketches).is_ok();
        self.model_undo.rollback(self.model);
        std::mem::swap(self.model_undo, &mut temp);
        for sid in new_sketches {
            self.sketches.remove(sid);
        }
        *self.meta = saved_meta;
        ok
    }

    fn exec(&mut self, cmd: Box<dyn ducad_core::Command<ModelDoc>>) {
        self.model_undo.execute(cmd, self.model);
    }

    fn body_shape_clone(&self, name: &str) -> OpResult<KernelShape> {
        clone_shape(&self.body(name)?.1.shape)
    }

    fn apply(
        &mut self,
        op: &Op,
        index: usize,
        new_sketches: &mut Vec<SketchId>,
    ) -> OpResult<OpOutcome> {
        let mut out = OpOutcome {
            op_index: index,
            op_id: op.id().to_string(),
            kind: op.kind(),
            created: Vec::new(),
            modified: Vec::new(),
            removed: Vec::new(),
            warnings: Vec::new(),
            detail: serde_json::Value::Null,
        };
        let params = self.params().clone();
        match op {
            Op::Sketch {
                id,
                plane,
                entities,
                constraints,
            } => {
                let resolved = {
                    let lookup = |name: &str| self.body_shape_clone(name);
                    resolve_plane(plane, &params, &lookup)?
                };
                let built = build_sketch(entities, constraints, &params)?;
                let (frame, plane_ref) = match resolved {
                    ResolvedPlane::Standard(f, r) => (f, r),
                    ResolvedPlane::Datum(f) => {
                        self.meta.datum_counter += 1;
                        (f, PlaneRef::Datum(self.meta.datum_counter))
                    }
                };
                let sid = self.sketches.add(id.clone(), plane_ref);
                new_sketches.push(sid);
                if let Some(slot) = self.sketches.sketch_mut(sid) {
                    *slot = built.sketch;
                }
                self.meta.sketch_ids.insert(id.clone(), sid);
                self.meta.frames.insert(id.clone(), frame);
                if built.closed_regions == 0 {
                    out.warnings
                        .push(format!("sketch '{id}' tidak punya region tertutup"));
                }
                out.detail = serde_json::json!({
                    "entities": built.names.len(),
                    "closed_regions": built.closed_regions,
                    "dof": built.dof.dof,
                });
            }
            Op::Extrude {
                id,
                sketch,
                profile,
                distance,
                direction,
                mode,
                target,
            } => {
                let d = eval(distance, &params)?;
                let extent = extent_for(*direction, d);
                let solids = {
                    let (sk, frame) = self.sketch(sketch)?;
                    let ids = profile_entities(sk, profile)?;
                    let pick = profile_pick(profile, &ids, &params)?;
                    compute::extrude(sk, &pick, frame, extent)?
                };
                let geos = solids.into_iter().map(|(_, g)| g).collect();
                self.place_solids(id, "Extrude", geos, *mode, target.as_deref(), &mut out)?;
            }
            Op::Revolve {
                id,
                sketch,
                profile,
                axis,
                angle_deg,
                mode,
                target,
            } => {
                let (origin, dir) = match axis {
                    AxisSpec::Named(n) => match n.to_ascii_lowercase().as_str() {
                        "u" => (DVec2::ZERO, DVec2::X),
                        "v" => (DVec2::ZERO, DVec2::Y),
                        _ => {
                            return Err(OpError::invalid(format!(
                                "sumbu revolve '{n}' tidak dikenal (pakai \"u\" atau \"v\")"
                            )))
                        }
                    },
                    AxisSpec::Line { origin, dir } => {
                        let [ox, oy] = eval_arr(origin, &params)?;
                        let [dx, dy] = eval_arr(dir, &params)?;
                        (DVec2::new(ox, oy), DVec2::new(dx, dy))
                    }
                };
                let angle = angle_deg.as_ref().map(|a| eval(a, &params)).transpose()?;
                let geo = {
                    let (sk, frame) = self.sketch(sketch)?;
                    let ids = profile_entities(sk, profile)?;
                    let pick = profile_pick(profile, &ids, &params)?;
                    // Hasil `revolve_profile` (bidang XY) dipetakan ke bidang
                    // sketch oleh `compute::revolve` — berbeda dari GUI lama
                    // yang selalu tinggal di XY.
                    compute::revolve(sk, &pick, frame, origin, dir, angle)?
                };
                self.place_solids(id, "Revolve", vec![geo], *mode, target.as_deref(), &mut out)?;
            }
            Op::Primitive { id, shape, at } => {
                let shape = match shape {
                    PrimitiveSpec::Box { size, centered } => PrimitiveShape::Box {
                        size: eval_arr(size, &params)?,
                        centered: *centered,
                    },
                    PrimitiveSpec::Cylinder { r, h } => PrimitiveShape::Cylinder {
                        r: eval(r, &params)?,
                        h: eval(h, &params)?,
                    },
                    PrimitiveSpec::Sphere { r } => PrimitiveShape::Sphere {
                        r: eval(r, &params)?,
                    },
                    PrimitiveSpec::Cone { r1, r2, h } => PrimitiveShape::Cone {
                        r1: eval(r1, &params)?,
                        r2: eval(r2, &params)?,
                        h: eval(h, &params)?,
                    },
                };
                let geo = compute::primitive(&shape, eval_arr(at, &params)?)?;
                out.detail = volume_detail(&geo);
                self.exec(Box::new(
                    AddSolidCommand::new("Primitif", geo).with_body_name(id.clone()),
                ));
                out.created.push(id.clone());
            }
            Op::Boolean { id, kind, a, b } => {
                let (kind, label) = match kind {
                    BoolKind::Union => (BooleanKind::Union, "Union"),
                    BoolKind::Subtract => (BooleanKind::Subtract, "Subtract"),
                    BoolKind::Intersect => (BooleanKind::Intersect, "Intersect"),
                };
                if a == b {
                    return Err(OpError::invalid(format!(
                        "boolean butuh dua body berbeda (keduanya '{a}')"
                    )));
                }
                let (a_id, a_geo) = self.body(a)?;
                let (b_id, b_geo) = self.body(b)?;
                // Pre-check memblokir (pasti salah, murah): subtract/intersect
                // tanpa irisan bbox. Union tanpa irisan hanya peringatan.
                let blocking = kind != BooleanKind::Union;
                if let Some(w) = crate::diagnose::precheck_boolean(a, b, a_geo, b_geo, blocking)? {
                    out.warnings.push(w);
                }
                let geo = compute::boolean(&a_geo.shape, &b_geo.shape, kind)?;
                out.detail = volume_detail(&geo);
                self.exec(Box::new(BooleanCommand::from_result(
                    label,
                    id.clone(),
                    a_id,
                    b_id,
                    geo,
                )));
                self.meta.consumed.insert(a.clone(), id.clone());
                self.meta.consumed.insert(b.clone(), id.clone());
                out.created.push(id.clone());
                out.removed.extend([a.clone(), b.clone()]);
            }
            Op::Fillet {
                body,
                edges,
                radius,
                ..
            }
            | Op::Chamfer {
                body,
                edges,
                distance: radius,
                ..
            } => {
                let is_fillet = matches!(op, Op::Fillet { .. });
                let r = eval(radius, &params)?;
                let (bid, geo) = self.body(body)?;
                let idx = crate::select::select_edges(&geo.shape, edges)?;
                let pick = EdgePick::Indices(&idx);
                let new_geo = if is_fillet {
                    compute::fillet(&geo.shape, &pick, r)?
                } else {
                    compute::chamfer(&geo.shape, &pick, r)?
                };
                out.detail = serde_json::json!({ "edges": idx.len(), "volume": new_geo.shape.volume().abs() });
                let label = if is_fillet { "Fillet" } else { "Chamfer" };
                self.exec(Box::new(ReplaceGeometryCommand::new(label, bid, new_geo)));
                out.modified.push(body.clone());
            }
            Op::Shell {
                body,
                remove_faces,
                thickness,
                ..
            } => {
                let t = eval(thickness, &params)?;
                let (bid, geo) = self.body(body)?;
                let idx = crate::select::select_faces(&geo.shape, remove_faces)?;
                let new_geo = compute::shell(&geo.shape, &FacePick::Indices(&idx), t)?;
                out.detail = serde_json::json!({ "faces": idx.len(), "volume": new_geo.shape.volume().abs() });
                self.exec(Box::new(ReplaceGeometryCommand::new("Shell", bid, new_geo)));
                out.modified.push(body.clone());
            }
            Op::Hole {
                body,
                face,
                at,
                at_world,
                spec,
                ..
            } => {
                let (bid, geo) = self.body(body)?;
                let idx = crate::select::select_faces(&geo.shape, face)?;
                let faces = ducad_kernel::enumerate_faces(&geo.shape);
                if idx.len() != 1 || faces[idx[0]].kind != SurfaceKind::Plane {
                    return Err(OpError::invalid(format!(
                        "selector face lubang \"{face}\" harus menghasilkan tepat 1 face planar (cocok {})",
                        idx.len()
                    ))
                    .with_context(serde_json::json!({ "matched": idx.len() })));
                }
                let f = &faces[idx[0]];
                let frame = PlaneFrame::on_face(f.centroid, f.normal);
                let positions = hole_positions(&frame, at, at_world, &params)?;
                let spec = hole_spec(spec, &params)?;
                // Pre-check memblokir: titik di luar face (tanpa apply_hole).
                crate::diagnose::precheck_hole(f, &frame, &positions)?;
                if !spec.is_through {
                    if let Some(w) =
                        crate::diagnose::hole_depth_warning(geo, &positions, f.normal, spec.depth)
                    {
                        out.warnings.push(w);
                    }
                }
                let new_geo = compute::hole(&geo.shape, &spec, &positions, f.normal)?;
                out.detail = serde_json::json!({
                    "holes": positions.len(),
                    "diameter": spec.diameter,
                    "through": spec.is_through,
                    "volume": new_geo.shape.volume().abs(),
                });
                self.exec(Box::new(ReplaceGeometryCommand::new(
                    "Hole Wizard",
                    bid,
                    new_geo,
                )));
                out.modified.push(body.clone());
            }
            Op::Pattern {
                id,
                body,
                kind,
                merge,
            } => {
                let (bid, geo) = self.body(body)?;
                let (copies, label) = match kind {
                    PatternKind::Linear { count, pitch } => (
                        compute::linear_pattern(
                            &geo.shape,
                            count.map(|c| c as usize),
                            eval_arr(pitch, &params)?,
                        )?,
                        "Linear Pattern 3D",
                    ),
                    PatternKind::Circular {
                        pivot,
                        axis,
                        count,
                        angle_deg,
                    } => (
                        compute::circular_pattern(
                            &geo.shape,
                            eval_arr(pivot, &params)?,
                            eval_arr(axis, &params)?,
                            *count as usize,
                            eval(angle_deg, &params)?,
                        )?,
                        "Circular Pattern 3D",
                    ),
                };
                if copies.is_empty() {
                    return Err(OpError::invalid(
                        "pattern tidak menghasilkan salinan (jumlah total minimal 2)",
                    ));
                }
                out.detail = serde_json::json!({ "instances": copies.len() + 1 });
                if *merge {
                    let mut acc = clone_shape(&geo.shape)?;
                    for c in &copies {
                        acc = compute::boolean(&acc, &c.shape, BooleanKind::Union)?.shape;
                    }
                    let merged = compute::finish("Pattern", acc)?;
                    self.exec(Box::new(ReplaceGeometryCommand::new(
                        "Pattern", bid, merged,
                    )));
                    out.modified.push(body.clone());
                } else {
                    let named: Vec<(String, BodyGeometry)> = copies
                        .into_iter()
                        .enumerate()
                        .map(|(k, g)| (format!("{id}.{}", k + 1), g))
                        .collect();
                    out.created.extend(named.iter().map(|(n, _)| n.clone()));
                    self.exec(Box::new(AddMultipleSolidsCommand::new(label, named)));
                }
            }
            Op::Transform {
                body,
                translate,
                rotate,
                ..
            } => {
                if translate.is_none() && rotate.is_none() {
                    return Err(OpError::invalid(
                        "transform butuh 'translate' dan/atau 'rotate'",
                    ));
                }
                let (bid, geo) = self.body(body)?;
                let mut shape = clone_shape(&geo.shape)?;
                if let Some(t) = translate {
                    let [x, y, z] = eval_arr(t, &params)?;
                    shape = ducad_kernel::translate_shape(&shape, x, y, z)
                        .map_err(|e| OpError::kernel("Transform", e))?;
                }
                if let Some(r) = rotate {
                    let [px, py, pz] = eval_arr(&r.pivot, &params)?;
                    let [ax, ay, az] = eval_arr(&r.axis, &params)?;
                    if DVec3::new(ax, ay, az).length() < 1e-9 {
                        return Err(OpError::invalid("sumbu rotasi tidak boleh vektor nol"));
                    }
                    let angle = eval(&r.angle_deg, &params)?.to_radians();
                    shape = ducad_kernel::rotate_shape(&shape, (px, py, pz), (ax, ay, az), angle)
                        .map_err(|e| OpError::kernel("Transform", e))?;
                }
                let new_geo = compute::finish("Transform", shape)?;
                self.exec(Box::new(ReplaceGeometryCommand::new(
                    "Transform",
                    bid,
                    new_geo,
                )));
                out.modified.push(body.clone());
            }
            Op::Delete { id, body } => {
                let (bid, _) = self.body(body)?;
                self.exec(Box::new(DeleteBodyCommand::new(bid)));
                self.meta.consumed.insert(body.clone(), id.clone());
                out.removed.push(body.clone());
            }
        }
        Ok(out)
    }

    /// Pasang solid hasil extrude/revolve sesuai `mode`.
    fn place_solids(
        &mut self,
        id: &str,
        label: &'static str,
        mut geos: Vec<BodyGeometry>,
        mode: BodyMode,
        target: Option<&str>,
        out: &mut OpOutcome,
    ) -> OpResult<()> {
        match mode {
            BodyMode::New => {
                if target.is_some() {
                    return Err(OpError::invalid(
                        "'target' hanya berlaku untuk mode add/cut",
                    ));
                }
                if geos.len() == 1 {
                    let geo = geos.pop().expect("len == 1");
                    out.detail = volume_detail(&geo);
                    self.exec(Box::new(
                        AddSolidCommand::new(label, geo).with_body_name(id),
                    ));
                    out.created.push(id.to_string());
                } else {
                    // Urutan deterministik: centroid mesh (x, lalu y, lalu z).
                    geos.sort_by(|a, b| {
                        let (ca, cb) = (
                            ducad_kernel::compute_mesh_centroid(&a.mesh),
                            ducad_kernel::compute_mesh_centroid(&b.mesh),
                        );
                        ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let named: Vec<(String, BodyGeometry)> = geos
                        .into_iter()
                        .enumerate()
                        .map(|(k, g)| {
                            (
                                if k == 0 {
                                    id.to_string()
                                } else {
                                    format!("{id}.{}", k + 1)
                                },
                                g,
                            )
                        })
                        .collect();
                    out.created.extend(named.iter().map(|(n, _)| n.clone()));
                    self.exec(Box::new(AddMultipleSolidsCommand::new(label, named)));
                }
            }
            BodyMode::Add | BodyMode::Cut => {
                let Some(target) = target else {
                    return Err(OpError::invalid("mode add/cut butuh 'target' (nama body)"));
                };
                let mut tool = geos
                    .pop()
                    .ok_or_else(|| {
                        OpError::new(OpErrorCode::EmptyResult, "tidak ada solid untuk digabung")
                    })?
                    .shape;
                for g in geos {
                    tool = compute::boolean(&tool, &g.shape, BooleanKind::Union)?.shape;
                }
                let (tid, tgeo) = self.body(target)?;
                let kind = if mode == BodyMode::Add {
                    BooleanKind::Union
                } else {
                    BooleanKind::Subtract
                };
                let geo = compute::boolean(&tgeo.shape, &tool, kind)?;
                out.detail = volume_detail(&geo);
                self.exec(Box::new(ReplaceGeometryCommand::new(label, tid, geo)));
                out.modified.push(target.to_string());
            }
        }
        Ok(())
    }
}

fn volume_detail(geo: &BodyGeometry) -> serde_json::Value {
    serde_json::json!({ "volume": geo.shape.volume().abs() })
}

/// Entitas untuk `ProfileSel::Names`: nama persis atau anaknya (`outline`
/// memilih `outline.top`, `outline.left`, …).
/// Arah extrude JSON → `ExtrudeExtent`. `Symmetric(len)`: len = tebal
/// TOTAL (csg.rs: offset −len/2, panjang len) — sama dengan kontrak JSON.
pub(crate) fn extent_for(direction: ExtrudeDir, d: f64) -> ExtrudeExtent {
    match direction {
        ExtrudeDir::Normal => ExtrudeExtent::Blind(d),
        ExtrudeDir::Reverse => ExtrudeExtent::Blind(-d),
        ExtrudeDir::Symmetric => ExtrudeExtent::Symmetric(d),
    }
}

pub(crate) fn profile_entities(sketch: &Sketch, sel: &ProfileSel) -> OpResult<HashSet<EntityId>> {
    let ProfileSel::Names { names } = sel else {
        return Ok(HashSet::new());
    };
    let mut ids = HashSet::new();
    for n in names {
        let prefix = format!("{n}.");
        let before = ids.len();
        ids.extend(
            sketch
                .entity_names
                .iter()
                .filter(|(_, name)| *name == n || name.starts_with(&prefix))
                .map(|(id, _)| *id),
        );
        if ids.len() == before {
            let known: BTreeSet<&str> = sketch.entity_names.values().map(String::as_str).collect();
            return Err(OpError::new(
                OpErrorCode::UnknownRef,
                format!("entitas profil '{n}' tidak dikenal (yang ada: {known:?})"),
            ));
        }
    }
    Ok(ids)
}

pub(crate) fn profile_pick<'a>(
    sel: &ProfileSel,
    ids: &'a HashSet<EntityId>,
    params: &Params,
) -> OpResult<ProfilePick<'a>> {
    Ok(match sel {
        ProfileSel::All(_) => ProfilePick::AllRegions,
        ProfileSel::Names { .. } => ProfilePick::Entities(ids),
        ProfileSel::At { at } => {
            let [u, v] = eval_arr(at, params)?;
            ProfilePick::AtPoint(DVec2::new(u, v))
        }
    })
}

/// Posisi lubang dunia: tepat satu dari `at` (lokal pada bidang face) atau
/// `at_world` (diproyeksikan tegak lurus ke bidang face).
fn hole_positions(
    frame: &PlaneFrame,
    at: &[[Num; 2]],
    at_world: &[[Num; 3]],
    params: &Params,
) -> OpResult<Vec<[f64; 3]>> {
    match (at.is_empty(), at_world.is_empty()) {
        (false, true) => at
            .iter()
            .map(|p| {
                let [u, v] = eval_arr(p, params)?;
                Ok(frame.to_world(DVec2::new(u, v)).to_array())
            })
            .collect(),
        (true, false) => at_world
            .iter()
            .map(|p| {
                let w = DVec3::from_array(eval_arr(p, params)?);
                let n = frame.normal_v();
                Ok((w - n * (w - frame.origin_v()).dot(n)).to_array())
            })
            .collect(),
        _ => Err(OpError::invalid(
            "lubang butuh tepat satu dari 'at' atau 'at_world' (tidak kosong)",
        )),
    }
}

fn hole_spec(spec: &HoleSpecRef, params: &Params) -> OpResult<ducad_core::hole::HoleSpec> {
    use ducad_core::hole::{HoleKind, HoleSpec, IsoMetricThread};
    const THROUGH_DEFAULT_DEPTH: f64 = 20.0;
    let depth = |d: &Option<Num>| -> OpResult<Option<f64>> {
        d.as_ref().map(|n| eval(n, params)).transpose()
    };
    match spec {
        HoleSpecRef::Iso {
            iso,
            kind,
            depth: d,
        } => {
            let thread = IsoMetricThread::all()
                .iter()
                .copied()
                .filter(|t| *t != IsoMetricThread::Custom)
                .find(|t| t.label().eq_ignore_ascii_case(iso))
                .ok_or_else(|| {
                    OpError::invalid(format!(
                        "ukuran ISO '{iso}' tidak dikenal (M2, M2.5, M3, M4, M5, M6, M8, M10, M12)"
                    ))
                })?;
            let kind = match kind {
                HoleKindSpec::Clearance => HoleKind::Simple,
                HoleKindSpec::Tapped => HoleKind::Tapped,
                HoleKindSpec::Counterbore => HoleKind::Counterbore,
                HoleKindSpec::Countersink => HoleKind::Countersink,
            };
            let d = depth(d)?;
            let mut s = HoleSpec::for_iso(thread, kind, d.unwrap_or(THROUGH_DEFAULT_DEPTH));
            // `for_iso` memakai kolom clearance_dia untuk Simple/Counterbore/
            // Countersink dan tap_drill untuk Tapped (ducad-core/src/hole.rs).
            s.is_through = d.is_none();
            Ok(s)
        }
        HoleSpecRef::Custom { diameter, depth: d } => {
            let dia = eval(diameter, params)?;
            let d = depth(d)?;
            let mut s = HoleSpec::for_iso(
                IsoMetricThread::Custom,
                HoleKind::Simple,
                d.unwrap_or(THROUGH_DEFAULT_DEPTH),
            );
            s.diameter = dia;
            s.is_through = d.is_none();
            Ok(s)
        }
    }
}

// ---------------------------------------------------------------------
// Session.
// ---------------------------------------------------------------------

impl Session {
    pub fn new() -> Self {
        Self {
            model: ModelDoc::default(),
            model_undo: UndoStack::default(),
            sketches: SketchSet::new(),
            meta: SessionMeta::default(),
            redo: Vec::new(),
            proposals: std::collections::VecDeque::new(),
            next_proposal: 1,
        }
    }

    fn core(&mut self) -> SessionCore<'_> {
        SessionCore {
            model: &mut self.model,
            model_undo: &mut self.model_undo,
            sketches: &mut self.sketches,
            meta: &mut self.meta,
        }
    }

    pub fn model(&self) -> &ModelDoc {
        &self.model
    }

    pub fn design(&self) -> &DesignDoc {
        &self.meta.design
    }

    pub fn meta(&self) -> &SessionMeta {
        &self.meta
    }

    pub fn sketches(&self) -> &SketchSet {
        &self.sketches
    }

    pub fn body(&self, name: &str) -> OpResult<(BodyId, &BodyGeometry)> {
        body_lookup(&self.model, &self.meta, name)
    }

    pub fn sketch(&self, id: &str) -> OpResult<(&Sketch, &PlaneFrame)> {
        let sid = self.meta.sketch_ids.get(id);
        let frame = self.meta.frames.get(id);
        match (sid.and_then(|s| self.sketches.sketch(*s)), frame) {
            (Some(sk), Some(f)) => Ok((sk, f)),
            _ => Err(OpError::new(
                OpErrorCode::UnknownRef,
                format!("sketch '{id}' tidak dikenal"),
            )),
        }
    }

    pub fn summary(&self) -> Summary {
        summarize_state(&self.model, &self.sketches, &self.meta, None, false, 0)
    }

    /// Jalankan batch op secara atomik.
    pub fn run(&mut self, ops: Vec<Op>, dry_run: bool) -> BatchReport {
        let report = self.core().run(ops, dry_run);
        if report.committed {
            self.redo.clear();
        }
        report
    }

    /// Bangun sesi dari `base_bodies` + seluruh oplog. Tidak memeriksa
    /// sidik jari.
    fn rebuild(design: DesignDoc) -> OpResult<(Self, BatchReport)> {
        let mut s = Self::new();
        for nb in &design.base_bodies {
            let shape = KernelShape::from_step_string(&nb.step).map_err(|e| {
                OpError::new(
                    OpErrorCode::Io,
                    format!("gagal membaca body dasar '{}': {e:#}", nb.name),
                )
            })?;
            let id = s
                .model
                .doc
                .add_body_with_material(nb.name.clone(), nb.material);
            if let Some(b) = s.model.doc.bodies.get_mut(id) {
                b.visible = nb.visible;
                b.uuid = nb.uuid.clone();
            }
            s.model.geometry.insert(id, BodyGeometry::from_shape(shape));
        }
        let ops = design.oplog.clone();
        s.meta.design = DesignDoc {
            oplog: Vec::new(),
            ..design
        };
        let report = s.run(ops, false);
        if let Some(e) = report.error.clone() {
            return Err(e);
        }
        Ok((s, report))
    }

    /// Replay penuh `design`. Bila `design.fingerprint` terisi dan hasil
    /// replay tidak menghasilkan sidik jari yang sama → `OplogStale`.
    pub fn replay(design: DesignDoc) -> OpResult<Self> {
        let expected = design.fingerprint.clone();
        let n = design.oplog.len();
        let (mut s, _) = Self::rebuild(design)?;
        if !expected.is_empty() && s.meta.design.fingerprint != expected {
            return Err(OpError::new(
                OpErrorCode::OplogStale,
                "oplog tidak lagi menghasilkan geometri yang tercatat (sidik jari berbeda)",
            )
            .with_context(
                serde_json::json!({ "expected": expected, "actual": s.meta.design.fingerprint }),
            ));
        }
        // Tanpa riwayat batch asli: tiap op menjadi satu langkah undo.
        s.meta.batches = vec![1; n];
        Ok(s)
    }

    /// Muat berkas `.ducad`. Berkas dengan `design` di-replay; berkas tanpa
    /// `design` (buatan GUI/impor) diadopsi: body-nya menjadi `base_bodies`
    /// dengan oplog kosong, nama ganda dibedakan dengan sufiks `#2`, `#3`.
    pub fn from_file(path: &std::path::Path) -> OpResult<Self> {
        let json = std::fs::read_to_string(path).map_err(|e| {
            OpError::new(
                OpErrorCode::Io,
                format!("gagal membaca {}: {e}", path.display()),
            )
        })?;
        let file = ducad_io::native::deserialize_raw(&json).map_err(|e| {
            OpError::new(
                OpErrorCode::Io,
                format!("berkas {} tidak valid: {e:#}", path.display()),
            )
        })?;
        let design = file
            .design
            .clone()
            .map(serde_json::from_value::<DesignDoc>)
            .transpose();
        match design {
            Ok(Some(design)) => {
                // Body di berkas dibandingkan dengan sidik jari yang tercatat
                // di `design` saat disimpan. Berbeda → berkas diubah (GUI)
                // setelah op terakhir: jangan replay, adopsi body berkas.
                let keep = DesignDoc {
                    params: design.params.clone(),
                    checks: design.checks.clone(),
                    ..DesignDoc::default()
                };
                let stale_warning = || vec!["oplog_stale".to_string()];
                let adopted = Self::adopt(file.bodies.clone(), keep.clone(), stale_warning())?;
                if adopted.meta.design.fingerprint != design.fingerprint {
                    log::warn!(
                        "oplog basi: sidik jari berkas {} != design {}",
                        adopted.meta.design.fingerprint,
                        design.fingerprint
                    );
                    return Ok(adopted);
                }
                match Self::replay(design) {
                    Ok(mut s) => {
                        s.adopt_uuids(&file.bodies);
                        Ok(s)
                    }
                    Err(e) if e.code == OpErrorCode::OplogStale => Ok(adopted),
                    Err(e) => Err(e),
                }
            }
            Ok(None) => Self::adopt(file.bodies, DesignDoc::default(), Vec::new()),
            Err(e) => Self::adopt(
                file.bodies,
                DesignDoc::default(),
                vec![format!("design_invalid: {e}")],
            ),
        }
    }

    /// Salin uuid body dari berkas ke body hasil replay (cocok lewat nama),
    /// supaya uuid stabil lintas simpan → muat → simpan.
    fn adopt_uuids(&mut self, bodies: &[NativeBody]) {
        for nb in bodies {
            if let Some(id) = find_body(&self.model, &nb.name) {
                if let Some(b) = self.model.doc.bodies.get_mut(id) {
                    b.uuid = nb.uuid.clone();
                }
            }
        }
    }

    /// Mode adopsi: body berkas menjadi `base_bodies`, oplog kosong.
    fn adopt(bodies: Vec<NativeBody>, base: DesignDoc, warnings: Vec<String>) -> OpResult<Self> {
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        let base_bodies = bodies
            .into_iter()
            .map(|mut nb| {
                let n = seen.entry(nb.name.clone()).or_insert(0);
                *n += 1;
                if *n > 1 {
                    nb.name = format!("{}#{n}", nb.name);
                }
                nb
            })
            .collect();
        let design = DesignDoc {
            oplog: Vec::new(),
            fingerprint: String::new(),
            base_bodies,
            ..base
        };
        let (mut s, _) = Self::rebuild(design)?;
        s.meta.design.fingerprint = fingerprint(&s.model);
        s.meta.warnings = warnings;
        Ok(s)
    }

    /// Simpan ke `.ducad` v2. Field `sketch`/`front_sketch`/`right_sketch`
    /// hanya untuk tampilan di GUI: gabungan entitas semua sketch pada
    /// bidang standar itu (sketch pada datum/face tidak ditulis). Sumber
    /// kebenaran tetap `design`.
    pub fn save(&mut self, path: &std::path::Path) -> OpResult<()> {
        self.meta.design.fingerprint = fingerprint(&self.model);
        let design = serde_json::to_value(&self.meta.design)
            .map_err(|e| OpError::new(OpErrorCode::Io, format!("gagal serialisasi design: {e}")))?;
        let planes = [PlaneRef::Top, PlaneRef::Front, PlaneRef::Right];
        let display: Vec<Sketch> = planes.iter().map(|p| self.display_sketch(*p)).collect();
        let display_refs: Vec<&Sketch> = display.iter().collect();
        let mut bodies: Vec<(&ducad_core::Body, &KernelShape)> = self
            .model
            .doc
            .bodies
            .iter()
            .filter_map(|(id, b)| Some((b, &self.model.geometry.get(id)?.shape)))
            .collect();
        bodies.sort_by(|a, b| a.0.name.cmp(&b.0.name));
        let exports: Vec<ducad_io::native::ExportBody> = bodies
            .iter()
            .map(|(b, shape)| ducad_io::native::ExportBody {
                name: &b.name,
                uuid: Some(b.uuid.clone()),
                visible: b.visible,
                material: b.material,
                shape,
                round_history: None,
            })
            .collect();
        ducad_io::native::save_multi_plane_detailed_with_design(
            path,
            &display_refs,
            &exports,
            Some(&design),
        )
        .map_err(|e| {
            OpError::new(
                OpErrorCode::Io,
                format!("gagal menyimpan {}: {e:#}", path.display()),
            )
        })
    }

    /// Gabungan entitas semua sketch engine pada `plane`. Constraint tidak
    /// ikut (EntityId berubah saat digabung), nama entitas juga tidak:
    /// `Sketch::entity_names` berkunci `EntityId` sehingga serde_json menolak
    /// menyerialisasinya ("key must be a string").
    fn display_sketch(&self, plane: PlaneRef) -> Sketch {
        let mut out = Sketch::default();
        for sid in self.meta.sketch_ids.values() {
            let Some(slot) = self.sketches.get(*sid) else {
                continue;
            };
            if slot.plane != plane {
                continue;
            }
            for e in slot.sketch.entities.values() {
                out.entities.insert(e.clone());
            }
        }
        out
    }

    /// Ganti params lalu replay penuh; gagal → sesi lama utuh.
    pub fn set_params(&mut self, p: Params) -> OpResult<BatchReport> {
        let design = DesignDoc {
            params: p,
            ..self.meta.design.clone()
        };
        match Self::rebuild(design) {
            Ok((mut s, report)) => {
                s.meta.batches = std::mem::take(&mut self.meta.batches);
                s.meta.warnings = std::mem::take(&mut self.meta.warnings);
                s.redo = std::mem::take(&mut self.redo);
                *self = s;
                Ok(BatchReport {
                    summary: self.summary(),
                    ..report
                })
            }
            Err(e) => Ok(BatchReport {
                committed: false,
                outcomes: Vec::new(),
                error: Some(e),
                summary: self.summary(),
                checks: None,
            }),
        }
    }

    /// Jalankan `ops` tanpa mengubah model (batch lalu rollback) dan
    /// kembalikan diff-nya sebagai proposal. Batch yang gagal → error-nya.
    pub fn propose(&mut self, ops: Vec<Op>) -> OpResult<(Proposal, crate::diff::DiffShapes)> {
        let before = crate::diff::snapshot_bodies(&self.model);
        let base_fingerprint = fingerprint(&self.model);
        let (report, after) = self
            .core()
            .run_with(ops.clone(), true, crate::diff::snapshot_bodies);
        if let Some(e) = report.error.clone() {
            return Err(e);
        }
        let (diff, shapes, _) = crate::diff::diff_bodies(before, after.unwrap_or_default(), true);
        let id = format!("p{}", self.next_proposal);
        self.next_proposal += 1;
        if self.proposals.len() >= MAX_PROPOSALS {
            self.proposals.pop_front();
        }
        self.proposals.push_back(StoredProposal {
            id: id.clone(),
            ops: ops.clone(),
            base_fingerprint: base_fingerprint.clone(),
        });
        Ok((
            Proposal {
                id,
                ops,
                report,
                diff,
                base_fingerprint,
            },
            shapes,
        ))
    }

    /// Terapkan proposal. Model berubah sejak proposal dibuat → error
    /// `ProposalStale` (proposal tetap disimpan agar bisa dibuang).
    pub fn accept(&mut self, proposal_id: &str) -> BatchReport {
        let Some(pos) = self.proposals.iter().position(|p| p.id == proposal_id) else {
            let known: Vec<String> = self.proposals.iter().map(|p| p.id.clone()).collect();
            return self.failed_report(OpError::new(
                OpErrorCode::UnknownRef,
                format!("proposal '{proposal_id}' tidak dikenal (yang ada: {known:?})"),
            ));
        };
        if self.proposals[pos].base_fingerprint != fingerprint(&self.model) {
            return self.failed_report(
                OpError::new(
                    OpErrorCode::ProposalStale,
                    format!("model berubah sejak proposal '{proposal_id}' dibuat"),
                )
                .with_hint("buat proposal baru dengan propose_ops"),
            );
        }
        let ops = self.proposals[pos].ops.clone();
        let report = self.run(ops, false);
        if report.committed {
            self.proposals.remove(pos);
        }
        report
    }

    /// Buang proposal; `false` bila id tidak dikenal.
    pub fn reject(&mut self, proposal_id: &str) -> bool {
        let before = self.proposals.len();
        self.proposals.retain(|p| p.id != proposal_id);
        self.proposals.len() != before
    }

    fn failed_report(&self, e: OpError) -> BatchReport {
        BatchReport {
            committed: false,
            outcomes: Vec::new(),
            error: Some(e),
            summary: self.summary(),
            checks: None,
        }
    }

    /// Ganti seluruh daftar check desain.
    pub fn set_checks(&mut self, checks: Vec<crate::check::CheckItem>) {
        self.meta.design.checks = checks;
    }

    /// Evaluasi `checks` (atau `design.checks` bila `None`).
    pub fn run_checks(
        &self,
        checks: Option<&[crate::check::CheckItem]>,
    ) -> crate::check::CheckSummary {
        let list = checks.unwrap_or(&self.meta.design.checks);
        crate::check::CheckSummary::from_results(crate::check::run_checks_on(
            &self.model,
            &self.meta,
            list,
        ))
    }

    /// Buang batch terakhir lalu replay. `false` bila tidak ada yang bisa di-undo.
    pub fn undo(&mut self) -> OpResult<bool> {
        let Some(&n) = self.meta.batches.last() else {
            return Ok(false);
        };
        let mut design = self.meta.design.clone();
        let cut = design.oplog.split_off(design.oplog.len() - n);
        let (mut s, _) = Self::rebuild(design)?;
        let mut batches = std::mem::take(&mut self.meta.batches);
        batches.pop();
        s.meta.batches = batches;
        s.meta.warnings = std::mem::take(&mut self.meta.warnings);
        s.redo = std::mem::take(&mut self.redo);
        s.redo.push(cut);
        *self = s;
        Ok(true)
    }

    /// Terapkan ulang batch yang terakhir di-undo.
    pub fn redo(&mut self) -> OpResult<bool> {
        let Some(ops) = self.redo.pop() else {
            return Ok(false);
        };
        let rest = std::mem::take(&mut self.redo);
        let report = self.core().run(ops.clone(), false);
        self.redo = rest;
        match report.error {
            Some(e) => {
                self.redo.push(ops);
                Err(e)
            }
            None => Ok(true),
        }
    }
}
