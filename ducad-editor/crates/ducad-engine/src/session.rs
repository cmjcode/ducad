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
    DeleteBodyCommand, ModelDoc, ReplaceGeometryCommand, SetBodyMechanicalCommand,
};
use crate::ops::sketch::{build_sketch, resolve_plane, ResolvedPlane};
use crate::ops::{
    eval, eval_arr, is_valid_op_id, AxisSpec, BodyMode, BoolKind, ExtrudeDir, HelixSection,
    HoleKindSpec, HoleSpecRef, MaterialSel, MirrorPlane, Num, Op, OutlineSpec, Params, PatternKind,
    PrimitiveSpec, ProfileSel, SplitKeep, SweepPath,
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
    /// Konfigurasi varian (P19). Kosong = hanya "Default".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configurations: Vec<ducad_core::Configuration>,
    /// Nama konfigurasi aktif; `None` = "Default" (tanpa penimpaan).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_configuration: Option<String>,
    /// Lembar gambar tersimpan (P21.6). Bukan op: anotasi/tata letak tidak
    /// mengubah geometri (lihat `docs/adr/0007-drawing-spec.md`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawings: Vec<ducad_io::drawing::DrawingSpec>,
}

impl DesignDoc {
    /// Lembar gambar tersimpan bernama `name`.
    pub fn drawing(&self, name: &str) -> Option<&ducad_io::drawing::DrawingSpec> {
        self.drawings.iter().find(|d| d.name == name)
    }

    /// Simpan/ganti lembar gambar menurut namanya.
    pub fn upsert_drawing(&mut self, spec: ducad_io::drawing::DrawingSpec) {
        match self.drawings.iter_mut().find(|d| d.name == spec.name) {
            Some(slot) => *slot = spec,
            None => self.drawings.push(spec),
        }
    }

    /// Konfigurasi aktif, bila ada dan dikenal.
    pub fn active(&self) -> Option<&ducad_core::Configuration> {
        let name = self.active_configuration.as_deref()?;
        self.configurations.iter().find(|c| c.name == name)
    }

    /// Parameter dasar ditimpa parameter konfigurasi aktif — inilah yang
    /// dipakai setiap evaluasi `$nama`.
    pub fn effective_params(&self) -> Params {
        let mut params = self.params.clone();
        if let Some(cfg) = self.active() {
            params.extend(cfg.params.iter().map(|(k, v)| (k.clone(), *v)));
        }
        params
    }

    /// Op `id` dinonaktifkan oleh konfigurasi aktif.
    pub fn is_suppressed(&self, id: &str) -> bool {
        self.active()
            .is_some_and(|c| c.suppressed_ops.iter().any(|s| s == id))
    }
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
            configurations: Vec::new(),
            active_configuration: None,
            drawings: Vec::new(),
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
    /// Id sketch → entitas yang namanya dibuat otomatis oleh `build_sketch`.
    /// Dipakai penamaan body per-objek: menebak dari pola `e<n>` salah untuk
    /// nama eksplisit seperti `"e5"`.
    pub auto_named: BTreeMap<String, std::collections::BTreeSet<EntityId>>,
    /// Id studi → hasil simulasi terakhir + tanda tangannya (P17). Tidak
    /// disimpan ke berkas.
    pub sim_results: BTreeMap<String, crate::sim::StudyResult>,
    /// Hasil studi frekuensi/buckling/termal (P18), dengan tanda tangan yang
    /// sama artinya dengan `sim_results`.
    pub analysis_results: BTreeMap<String, crate::sim::AnalysisResult>,
    /// Nama body → model sheet metal-nya (P19); dibangun ulang saat replay.
    pub sheet_metal: BTreeMap<String, SheetMetalState>,
    /// Nama body → sebutan part standar untuk BOM (P20).
    pub standard_parts: BTreeMap<String, String>,
    /// Nama body → ulir yang tercatat (kosmetik maupun fisik) (P20).
    pub threads: BTreeMap<String, Vec<ThreadNote>>,
    /// Nama lembar → sidik jari geometri saat terakhir dirender (P21):
    /// berbeda dari sidik jari kini = lembar kedaluwarsa. Tidak disimpan.
    pub drawing_rendered: BTreeMap<String, String>,
}

/// Catatan satu ulir pada body.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThreadNote {
    pub id: String,
    /// Mis. `"M10x1.5"`.
    pub designation: String,
    pub length: f64,
    pub left_handed: bool,
    pub cosmetic: bool,
}

/// Keadaan sheet metal satu body.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetMetalState {
    pub model: ducad_core::SheetMetalModel,
    /// Bidang sketsa pelat dasar.
    pub frame: PlaneFrame,
    /// Body sedang berbentuk pola datar (`unfold`).
    pub unfolded: bool,
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
    /// Bagian non-append (P11): params baru, op yang diganti, dan id op
    /// yang dihapus.
    edit: Option<(Option<Params>, Vec<ReplaceOp>, Vec<String>)>,
    base_fingerprint: String,
}

/// Penggantian satu op di oplog (id op tetap).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplaceOp {
    pub id: String,
    pub op: Op,
}

/// Proposal perubahan (ghost preview, P8.4): batch yang sudah dijalankan
/// lalu dibatalkan, beserta diff body-nya.
#[derive(Debug, Clone, Serialize)]
pub struct Proposal {
    pub id: String,
    pub ops: Vec<Op>,
    /// Params baru (proposal dari `propose_edit`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Params>,
    /// Op yang diganti (proposal dari `propose_edit`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub replace: Vec<ReplaceOp>,
    /// Id op yang dihapus dari oplog.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
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
// Edit oplog di tempat (P11, P16).
// ---------------------------------------------------------------------

/// Design hasil edit: params diganti, op diganti di tempat (id tetap), op
/// di `remove` dibuang, lalu `append` ditambahkan di akhir. Fungsi murni —
/// dipakai [`Session`] dan jembatan live GUI.
pub fn edit_design(
    base: &DesignDoc,
    params: Option<&Params>,
    replace: &[ReplaceOp],
    remove: &[String],
    append: &[Op],
) -> OpResult<DesignDoc> {
    let mut design = base.clone();
    let unknown = |id: &str, design: &DesignDoc| {
        let known: Vec<&str> = design.oplog.iter().map(|o| o.id()).collect();
        OpError::new(
            OpErrorCode::UnknownRef,
            format!("op '{id}' is not in the oplog"),
        )
        .with_hint("list the existing op ids with get_oplog")
        .with_context(serde_json::json!({ "ops": known }))
    };
    if let Some(p) = params {
        design.params = p.clone();
    }
    for r in replace {
        if remove.contains(&r.id) {
            return Err(OpError::invalid(format!(
                "op '{}' cannot be both replaced and removed",
                r.id
            )));
        }
        let Some(pos) = design.oplog.iter().position(|o| o.id() == r.id) else {
            return Err(unknown(&r.id, &design));
        };
        if r.op.id() != r.id {
            return Err(OpError::invalid(format!(
                "replacement op has id '{}' but must use id '{}'",
                r.op.id(),
                r.id
            )));
        }
        design.oplog[pos] = r.op.clone();
    }
    for id in remove {
        let before = design.oplog.len();
        design.oplog.retain(|o| o.id() != id);
        if design.oplog.len() == before {
            return Err(unknown(id, base));
        }
    }
    design.oplog.extend(append.iter().cloned());
    Ok(design)
}

/// Hasil [`preview_edit`]: sesi hasil replay + laporan + diff body.
pub struct EditPreview {
    /// Sesi baru hasil replay design yang diedit (siap diadopsi).
    pub session: Session,
    /// `committed = false`; `summary` = keadaan usulan.
    pub report: BatchReport,
    pub diff: Vec<crate::diff::BodyDiff>,
    pub shapes: crate::diff::DiffShapes,
}

/// Uji edit oplog (lihat [`edit_design`]) tanpa menyentuh model pemanggil:
/// replay pada sesi baru lalu diff terhadap `current`. Dipakai jembatan
/// live GUI, yang memegang model dan undo-nya sendiri.
pub fn preview_edit(
    current: &ModelDoc,
    base: &DesignDoc,
    params: Option<&Params>,
    replace: &[ReplaceOp],
    remove: &[String],
    append: &[Op],
) -> OpResult<EditPreview> {
    let design = edit_design(base, params, replace, remove, append)?;
    let before = crate::diff::snapshot_bodies(current);
    let (session, mut report) = Session::rebuild(DesignDoc {
        fingerprint: String::new(),
        ..design
    })
    .map_err(|e| with_remove_hint(e, remove))?;
    let after = crate::diff::snapshot_bodies(&session.model);
    let (diff, shapes, _) = crate::diff::diff_bodies(before, after, true);
    report.committed = false;
    report.summary = session.summary();
    Ok(EditPreview {
        session,
        report,
        diff,
        shapes,
    })
}

/// Riwayat batch undo setelah op di posisi `removed_at` (posisi oplog lama
/// sepanjang `old_len`) dihapus. Batch mencakup op TERAKHIR oplog; op di
/// depannya (mis. hasil muat berkas) tidak termasuk batch mana pun. Batch
/// yang menjadi kosong dibuang.
pub(crate) fn batches_after_remove(
    batches: &[usize],
    old_len: usize,
    removed_at: &[usize],
) -> Vec<usize> {
    let covered: usize = batches.iter().sum();
    let mut start = old_len.saturating_sub(covered);
    let mut out = Vec::with_capacity(batches.len());
    for &n in batches {
        let gone = removed_at
            .iter()
            .filter(|&&p| p >= start && p < start + n)
            .count();
        if n > gone {
            out.push(n - gone);
        }
        start += n;
    }
    out
}

/// Replay gagal setelah menghapus op: rujukan yang putus hampir selalu
/// berarti op lain masih memakai op yang dihapus.
fn with_remove_hint(e: OpError, remove: &[String]) -> OpError {
    if remove.is_empty() || e.code != OpErrorCode::UnknownRef {
        return e;
    }
    let by = e.op_id.clone().unwrap_or_else(|| "a later op".into());
    e.with_hint(format!(
        "op '{by}' still references the removed op(s) ({}); remove it as well or change its reference with replace_op",
        remove.join(", ")
    ))
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

    fn summary(&self) -> Summary {
        summarize_state(self.model, self.sketches, self.meta, None, false, 0)
    }

    /// Jalankan `ops` tanpa mengubah model (batch lalu rollback) dan
    /// kembalikan diff body + geometri selisihnya. Inti `Session::propose`
    /// (P8.4), dipakai juga jembatan live di GUI yang tidak memiliki
    /// `Session` sendiri.
    pub fn propose(
        &mut self,
        ops: Vec<Op>,
    ) -> OpResult<(
        BatchReport,
        Vec<crate::diff::BodyDiff>,
        crate::diff::DiffShapes,
        String,
    )> {
        let before = crate::diff::snapshot_bodies(self.model);
        let base_fingerprint = fingerprint(self.model);
        let (report, after) = self.run_with(ops, true, crate::diff::snapshot_bodies);
        if let Some(e) = report.error.clone() {
            return Err(e);
        }
        let (diff, shapes, _) = crate::diff::diff_bodies(before, after.unwrap_or_default(), true);
        Ok((report, diff, shapes, base_fingerprint))
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
            // Op yang di-suppress konfigurasi aktif tetap tercatat di oplog
            // (id-nya terpakai) tetapi tidak diterapkan.
            let result = self.check_id(op, &used).and_then(|_| {
                if self.meta.design.is_suppressed(op.id()) {
                    return Ok(OpOutcome {
                        op_index: i,
                        op_id: op.id().to_string(),
                        kind: op.kind(),
                        created: Vec::new(),
                        modified: Vec::new(),
                        removed: Vec::new(),
                        warnings: vec!["suppressed".to_string()],
                        detail: serde_json::Value::Null,
                    });
                }
                self.apply(op, i, &mut new_sketches)
            });
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

    /// Model sheet metal body `name`, atau error yang menjelaskan caranya.
    fn sheet_state(&self, name: &str) -> OpResult<&SheetMetalState> {
        self.meta.sheet_metal.get(name).ok_or_else(|| {
            let known: Vec<&str> = self.meta.sheet_metal.keys().map(String::as_str).collect();
            OpError::invalid(format!(
                "body '{name}' bukan part sheet metal (yang ada: {known:?})"
            ))
            .with_hint("sheet metal ops only work on a body created by op base_flange")
            .with_context(serde_json::json!({ "body": name, "sheet_metal_bodies": known }))
        })
    }

    /// Tambah flange (rantai `segments`) pada setiap sisi pelat dasar yang
    /// ditunjuk selector `edges`, lalu ganti geometri body. Beberapa sisi →
    /// id flange `id`, `id_2`, ….
    fn add_flanges(
        &mut self,
        id: &str,
        body: &str,
        edges: &str,
        relief: ducad_core::ReliefKind,
        label: &'static str,
        segments: impl Fn(&ducad_core::SheetMetalModel) -> OpResult<Vec<ducad_core::BendSegment>>,
    ) -> OpResult<()> {
        let (bid, geo) = self.body(body)?;
        let state = self.sheet_state(body)?.clone();
        if state.unfolded {
            return Err(OpError::invalid(format!(
                "body '{body}' sedang terbentang; jalankan op fold dulu"
            )));
        }
        let picked = crate::select::select_edges(&geo.shape, edges)?;
        let all = ducad_kernel::enumerate_edges(&geo.shape);
        let infos: Vec<ducad_kernel::EdgeInfo> = picked
            .iter()
            .filter_map(|i| all.iter().find(|e| e.index == *i).cloned())
            .collect();
        let sides = compute::sheet_metal::match_base_edges(&state.model, &state.frame, &infos)?;
        let mut model = state.model.clone();
        let chain = segments(&model)?;
        for (n, edge) in sides.iter().enumerate() {
            model.flanges.push(ducad_core::Flange {
                id: if n == 0 {
                    id.to_string()
                } else {
                    format!("{id}_{}", n + 1)
                },
                edge: *edge,
                segments: chain.clone(),
                relief,
            });
        }
        model.validate().map_err(OpError::invalid)?;
        let new_geo = compute::sheet_metal::folded(&model, &state.frame)?;
        self.exec(Box::new(ReplaceGeometryCommand::new(label, bid, new_geo)));
        if let Some(s) = self.meta.sheet_metal.get_mut(body) {
            s.model = model;
        }
        Ok(())
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
        let params = self.meta.design.effective_params();
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
                let entity_count = built.names.len();
                if let Some(slot) = self.sketches.sketch_mut(sid) {
                    *slot = built.sketch;
                }
                self.meta.sketch_ids.insert(id.clone(), sid);
                self.meta.frames.insert(id.clone(), frame);
                self.meta.auto_named.insert(id.clone(), built.auto_named);
                if built.closed_regions == 0 {
                    out.warnings
                        .push(format!("sketch '{id}' tidak punya region tertutup"));
                }
                out.detail = serde_json::json!({
                    "entities": entity_count,
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
                per_object,
                material,
                outline,
            } => {
                if *per_object && *mode != BodyMode::New {
                    return Err(OpError::invalid("per_object hanya untuk body baru"));
                }
                validate_material(material)?;
                let d = eval(distance, &params)?;
                let extent = extent_for(*direction, d);
                compute::validate_extent(extent)?;

                if *per_object {
                    let (sk, frame) = self.sketch(sketch)?;
                    let ids: HashSet<EntityId> = selection_entities(sk, profile, &params)?;
                    let draw_order = sk.draw_order();
                    let ordered_eids = crate::compute::solid::selection_in_draw_order(sk, &ids);
                    if ordered_eids.is_empty() {
                        return Err(OpError::new(
                            OpErrorCode::ProfileNotClosed,
                            "Tidak ada entitas yang dipilih untuk diekstrusi",
                        ));
                    }
                    let auto_named = self.meta.auto_named.get(sketch.as_str());
                    let mut named_bodies = Vec::with_capacity(ordered_eids.len());
                    for eid in ordered_eids {
                        let is_auto = auto_named.is_some_and(|set| set.contains(&eid));
                        let bname = entity_body_name(id, eid, sk, &draw_order, is_auto);
                        let style = sk
                            .styles
                            .get(eid)
                            .cloned()
                            .unwrap_or_else(ducad_sketch::Style::cad_default);
                        let mat = resolve_material(material, &style);
                        let geo = extrude_single_entity(
                            sk,
                            eid,
                            outline.as_ref(),
                            frame,
                            extent,
                            &params,
                        )?;
                        named_bodies.push((bname, geo, Some(mat)));
                    }
                    out.created
                        .extend(named_bodies.iter().map(|(n, _, _)| n.clone()));
                    if named_bodies.len() == 1 {
                        out.detail = volume_detail(&named_bodies[0].1);
                    } else {
                        let total_vol: f64 = named_bodies
                            .iter()
                            .map(|(_, g, _)| g.shape.volume().abs())
                            .sum();
                        out.detail = serde_json::json!({ "volume": total_vol });
                    }
                    self.exec(Box::new(AddMultipleSolidsCommand::with_materials(
                        "Extrude",
                        named_bodies,
                    )));
                } else if outline.is_some() {
                    let (sk, frame) = self.sketch(sketch)?;
                    let ids: HashSet<EntityId> = selection_entities(sk, profile, &params)?;
                    let ordered_eids = crate::compute::solid::selection_in_draw_order(sk, &ids);
                    let mut geos = Vec::new();
                    for eid in ordered_eids {
                        let geo = extrude_single_entity(
                            sk,
                            eid,
                            outline.as_ref(),
                            frame,
                            extent,
                            &params,
                        )?;
                        geos.push(geo);
                    }
                    let mat = resolve_material_for_selection(material, sk, &ids);
                    self.place_solids_with_material(
                        id,
                        "Extrude",
                        geos,
                        *mode,
                        target.as_deref(),
                        Some(mat),
                        &mut out,
                    )?;
                } else {
                    let (sk, frame) = self.sketch(sketch)?;
                    let ids = profile_entities(sk, profile)?;
                    let pick = profile_pick(profile, &ids, &params)?;
                    let solids = compute::extrude(sk, &pick, frame, extent)?;
                    let geos = solids.into_iter().map(|(_, g)| g).collect();
                    // Material dari entitas yang benar-benar membentuk profil
                    // (untuk `All`/`At`, `ids` di atas kosong).
                    let style_ids = selection_entities(sk, profile, &params)?;
                    let mat = resolve_material_for_selection(material, sk, &style_ids);
                    self.place_solids_with_material(
                        id,
                        "Extrude",
                        geos,
                        *mode,
                        target.as_deref(),
                        Some(mat),
                        &mut out,
                    )?;
                }
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
                let radius_end = match op {
                    Op::Fillet {
                        radius_end: Some(re),
                        ..
                    } => Some(eval(re, &params)?),
                    _ => None,
                };
                let new_geo = if let Some(r1) = radius_end {
                    compute::advanced::fillet_variable(&geo.shape, &idx, r, r1)?
                } else if is_fillet {
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
                depth,
                ..
            } => {
                let t = eval(thickness, &params)?;
                let d = match depth {
                    Some(d) => eval(d, &params)?,
                    None => 0.0,
                };
                let (bid, geo) = self.body(body)?;
                let idx = crate::select::select_faces(&geo.shape, remove_faces)?;
                let new_geo = compute::shell(&geo.shape, &FacePick::Indices(&idx), t, d)?;
                out.detail = serde_json::json!({ "faces": idx.len(), "depth": d, "volume": new_geo.shape.volume().abs() });
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
            Op::SetMaterial { id: _, body, material } => {
                let (bid, _) = self.body(body)?;
                let source = resolve_mechanical(material, &self.model.doc.material_library)?;
                self.exec(Box::new(SetBodyMechanicalCommand::new(
                    "Material",
                    bid,
                    Some(source),
                )));
                out.modified.push(body.clone());
            }
            Op::BaseFlange {
                id,
                sketch,
                thickness,
                bend_radius,
                k_factor,
            } => {
                let t = eval(thickness, &params)?;
                let radius = bend_radius
                    .as_ref()
                    .map(|r| eval(r, &params))
                    .transpose()?
                    .unwrap_or(t);
                let (sk, frame) = self.sketch(sketch)?;
                let frame = *frame;
                let mut profiles = compute::resolve_profiles(sk, &ProfilePick::AllRegions)?;
                if profiles.len() != 1 {
                    return Err(OpError::invalid(format!(
                        "sketch '{sketch}' untuk base_flange harus berisi tepat 1 region tertutup (ditemukan {})",
                        profiles.len()
                    )));
                }
                let outline = compute::sheet_metal::outline_from_profile(&profiles.remove(0))?;
                let model = ducad_core::SheetMetalModel {
                    outline,
                    thickness: t,
                    default_radius: radius,
                    k_factor: k_factor.unwrap_or(ducad_core::DEFAULT_K_FACTOR),
                    flanges: Vec::new(),
                    bend_table: ducad_core::BendTable::default(),
                };
                let geo = compute::sheet_metal::folded(&model, &frame)?;
                self.exec(Box::new(
                    AddSolidCommand::new("BaseFlange", geo).with_body_name(id.clone()),
                ));
                self.meta.sheet_metal.insert(
                    id.clone(),
                    SheetMetalState {
                        model,
                        frame,
                        unfolded: false,
                    },
                );
                out.created.push(id.clone());
            }
            Op::EdgeFlange {
                id,
                body,
                edges,
                length,
                angle,
                radius,
                relief,
            } => {
                let angle = angle
                    .as_ref()
                    .map(|a| eval(a, &params))
                    .transpose()?
                    .unwrap_or(90.0);
                let length = eval(length, &params)?;
                let radius = radius.as_ref().map(|r| eval(r, &params)).transpose()?;
                let relief = match relief {
                    crate::ops::ReliefSpec::None => ducad_core::ReliefKind::None,
                    crate::ops::ReliefSpec::Rect => ducad_core::ReliefKind::Rect,
                    crate::ops::ReliefSpec::Obround => ducad_core::ReliefKind::Obround,
                };
                self.add_flanges(id, body, edges, relief, "EdgeFlange", |model| {
                    Ok(vec![ducad_core::BendSegment {
                        angle_deg: angle,
                        radius: radius.unwrap_or(model.default_radius),
                        length,
                    }])
                })?;
                out.modified.push(body.clone());
            }
            Op::Hem {
                id,
                body,
                edges,
                length,
                gap,
                up,
            } => {
                let length = eval(length, &params)?;
                let gap = gap.as_ref().map(|g| eval(g, &params)).transpose()?;
                let sign = if *up { 1.0 } else { -1.0 };
                self.add_flanges(id, body, edges, ducad_core::ReliefKind::None, "Hem", |model| {
                    let gap = gap.unwrap_or(model.thickness);
                    if gap < 0.0 {
                        return Err(OpError::invalid(format!("gap hem harus >= 0 (dapat {gap})")));
                    }
                    Ok(vec![ducad_core::BendSegment {
                        angle_deg: 180.0 * sign,
                        radius: gap / 2.0,
                        length,
                    }])
                })?;
                out.modified.push(body.clone());
            }
            Op::Jog {
                id,
                body,
                edges,
                offset,
                length,
                angle,
                radius,
            } => {
                let offset = eval(offset, &params)?;
                let length = eval(length, &params)?;
                let angle = angle
                    .as_ref()
                    .map(|a| eval(a, &params))
                    .transpose()?
                    .unwrap_or(90.0);
                let radius = radius.as_ref().map(|r| eval(r, &params)).transpose()?;
                if !(angle > 0.0 && angle <= 90.0) {
                    return Err(OpError::invalid(format!(
                        "sudut jog harus 0 < sudut <= 90 (dapat {angle})"
                    )));
                }
                self.add_flanges(id, body, edges, ducad_core::ReliefKind::None, "Jog", |model| {
                    let r = radius.unwrap_or(model.default_radius);
                    let a = angle.to_radians();
                    // offset = (2R + t)(1 − cos a) + s·sin a → panjang miring s.
                    let from_bends = (2.0 * r + model.thickness) * (1.0 - a.cos());
                    let slant = (offset.abs() - from_bends) / a.sin();
                    if slant < -1e-9 {
                        return Err(OpError::invalid(format!(
                            "offset jog {offset} mm terlalu kecil untuk radius {r} dan sudut {angle}° (minimum {from_bends:.3} mm)"
                        ))
                        .with_context(serde_json::json!({ "min_offset": from_bends })));
                    }
                    let sign = if offset >= 0.0 { 1.0 } else { -1.0 };
                    Ok(vec![
                        ducad_core::BendSegment {
                            angle_deg: angle * sign,
                            radius: r,
                            length: slant.max(0.0),
                        },
                        ducad_core::BendSegment {
                            angle_deg: -angle * sign,
                            radius: r,
                            length,
                        },
                    ])
                })?;
                out.modified.push(body.clone());
            }
            Op::Unfold { id: _, body } | Op::Fold { id: _, body } => {
                let unfold = matches!(op, Op::Unfold { .. });
                let (bid, _) = self.body(body)?;
                let state = self.sheet_state(body)?.clone();
                if state.unfolded == unfold {
                    return Err(OpError::invalid(format!(
                        "body '{body}' sudah dalam keadaan {}",
                        if unfold { "terbentang" } else { "terlipat" }
                    )));
                }
                let geo = if unfold {
                    compute::sheet_metal::flat(&state.model, &state.frame)?
                } else {
                    compute::sheet_metal::folded(&state.model, &state.frame)?
                };
                self.exec(Box::new(ReplaceGeometryCommand::new(
                    if unfold { "Unfold" } else { "Fold" },
                    bid,
                    geo,
                )));
                if let Some(s) = self.meta.sheet_metal.get_mut(body) {
                    s.unfolded = unfold;
                }
                out.modified.push(body.clone());
            }
            Op::FlatPattern { id, body } => {
                self.body(body)?;
                let state = self.sheet_state(body)?.clone();
                let pattern = state.model.flat_pattern().map_err(OpError::invalid)?;
                let geo = compute::sheet_metal::flat(&state.model, &state.frame)?;
                self.exec(Box::new(
                    AddSolidCommand::new("FlatPattern", geo).with_body_name(id.clone()),
                ));
                out.created.push(id.clone());
                out.detail = serde_json::json!({
                    "flat_area_mm2": crate::inspect::round4(state.model.flat_area()),
                    "bend_lines": pattern.bend_lines.iter().map(|b| serde_json::json!({
                        "flange": b.flange,
                        "a": b.a.map(crate::inspect::round4),
                        "b": b.b.map(crate::inspect::round4),
                        "direction": if b.up { "up" } else { "down" },
                        "angle_deg": b.angle_deg,
                        "radius": b.radius,
                    })).collect::<Vec<_>>(),
                });
            }
            Op::StandardPart {
                id,
                standard,
                size,
                length,
                at,
            } => {
                let kind = ducad_core::StandardKind::from_key(standard).ok_or_else(|| {
                    let known: Vec<&str> =
                        ducad_core::StandardKind::ALL.iter().map(|s| s.key()).collect();
                    OpError::invalid(format!(
                        "standar '{standard}' tidak dikenal (pilihan: {})",
                        known.join(", ")
                    ))
                    .with_context(serde_json::json!({ "standards": known }))
                })?;
                let length = length.as_ref().map(|l| eval(l, &params)).transpose()?;
                let part = ducad_core::standard_part(kind, size, length).map_err(|e| {
                    OpError::invalid(e).with_context(serde_json::json!({ "sizes": kind.sizes() }))
                })?;
                let geo = compute::standard::standard_part(&part.shape, eval_arr(at, &params)?)?;
                self.exec(Box::new(
                    AddSolidCommand::new("StandardPart", geo).with_body_name(id.clone()),
                ));
                self.meta
                    .standard_parts
                    .insert(id.clone(), part.designation.clone());
                out.created.push(id.clone());
                out.detail = serde_json::json!({ "designation": part.designation });
            }
            Op::Thread {
                id,
                body,
                face,
                pitch,
                length,
                from_end,
                left_handed,
                cosmetic,
            } => {
                let (bid, geo) = self.body(body)?;
                let picked = crate::select::select_faces(&geo.shape, face)?;
                let faces = ducad_kernel::enumerate_faces(&geo.shape);
                let [index] = picked[..] else {
                    return Err(OpError::invalid(format!(
                        "selector face ulir harus menunjuk tepat 1 face (dapat {})",
                        picked.len()
                    )));
                };
                let info = faces
                    .iter()
                    .find(|f| f.index == index)
                    .ok_or_else(|| OpError::invalid("face ulir tidak ditemukan"))?;
                let (mut start, mut dir, span, major_d) = compute::standard::cylinder_span(info)?;
                let pitch = match pitch.as_ref().map(|p| eval(p, &params)).transpose()? {
                    Some(p) => p,
                    None => compute::standard::coarse_pitch(major_d).ok_or_else(|| {
                        OpError::invalid(format!(
                            "diameter {major_d:.3} mm bukan ukuran ulir kasar ISO M2–M12; isi `pitch`"
                        ))
                    })?,
                };
                let length = length
                    .as_ref()
                    .map(|l| eval(l, &params))
                    .transpose()?
                    .unwrap_or(span);
                if !(length > 0.0 && length <= span + 1e-6) {
                    return Err(OpError::invalid(format!(
                        "panjang ulir {length} mm harus di antara 0 dan panjang silinder {span:.3} mm"
                    ))
                    .with_context(serde_json::json!({ "cylinder_length": span })));
                }
                if *from_end {
                    for a in 0..3 {
                        start[a] += dir[a] * span;
                        dir[a] = -dir[a];
                    }
                }
                if !*cosmetic {
                    let new_geo = compute::standard::thread(
                        &geo.shape,
                        start,
                        dir,
                        major_d,
                        pitch,
                        length,
                        *left_handed,
                    )?;
                    self.exec(Box::new(ReplaceGeometryCommand::new("Thread", bid, new_geo)));
                    out.modified.push(body.clone());
                }
                let designation = format!("M{}x{}", crate::inspect::round4(major_d), pitch);
                self.meta.threads.entry(body.clone()).or_default().push(ThreadNote {
                    id: id.clone(),
                    designation: designation.clone(),
                    length: crate::inspect::round4(length),
                    left_handed: *left_handed,
                    cosmetic: *cosmetic,
                });
                out.detail = serde_json::json!({ "designation": designation, "cosmetic": cosmetic });
            }
            Op::Study {
                id: _,
                kind,
                setup,
                thermal,
                modes,
            } => {
                crate::sim::validate_study(
                    self.model,
                    self.meta,
                    &crate::sim::StudyDef {
                        kind: *kind,
                        setup: setup.clone(),
                        thermal: thermal.clone(),
                        modes: *modes,
                    },
                )?;
            }
            Op::Delete { id, body } => {
                let (bid, _) = self.body(body)?;
                self.exec(Box::new(DeleteBodyCommand::new(bid)));
                self.meta.consumed.insert(body.clone(), id.clone());
                out.removed.push(body.clone());
            }
            Op::Loft {
                id,
                sections,
                mode,
                target,
            } => {
                let mut secs = Vec::with_capacity(sections.len());
                for sid in sections {
                    let (sk, frame) = self.sketch(sid)?;
                    let mut profiles = compute::resolve_profiles(sk, &ProfilePick::AllRegions)?;
                    if profiles.len() != 1 {
                        return Err(OpError::invalid(format!(
                            "sketch penampang loft '{sid}' harus berisi tepat 1 region tertutup (ditemukan {})",
                            profiles.len()
                        )));
                    }
                    secs.push((profiles.remove(0), *frame));
                }
                let geo = compute::advanced::loft(&secs)?;
                out.detail = volume_detail(&geo);
                self.place_solids(id, "Loft", vec![geo], *mode, target.as_deref(), &mut out)?;
            }
            Op::Sweep {
                id,
                sketch,
                profile,
                path,
                mode,
                target,
            } => {
                let segments = match path {
                    SweepPath::Sketch(pid) => {
                        let (psk, pframe) = self.sketch(pid)?;
                        compute::advanced::sketch_path(psk, pframe)?
                    }
                    SweepPath::Points { points } => {
                        let pts = points
                            .iter()
                            .map(|p| eval_arr(p, &params))
                            .collect::<OpResult<Vec<_>>>()?;
                        compute::advanced::points_path(&pts)?
                    }
                };
                let geo = {
                    let (sk, frame) = self.sketch(sketch)?;
                    let ids = profile_entities(sk, profile)?;
                    let pick = profile_pick(profile, &ids, &params)?;
                    let mut profiles = compute::resolve_profiles(sk, &pick)?;
                    if profiles.len() != 1 {
                        return Err(OpError::invalid(format!(
                            "profil sweep harus tepat 1 region (ditemukan {}); pilih dengan 'profile'",
                            profiles.len()
                        )));
                    }
                    compute::advanced::sweep(&profiles.remove(0), frame, &segments)?
                };
                out.detail = volume_detail(&geo);
                self.place_solids(id, "Sweep", vec![geo], *mode, target.as_deref(), &mut out)?;
            }
            Op::Helix {
                id,
                r,
                pitch,
                turns,
                section,
                at,
                axis,
                end_r,
                left_hand,
                mode,
                target,
            } => {
                let shape = match section {
                    HelixSection::Circle { r } => {
                        compute::advanced::HelixShape::Circle(eval(r, &params)?)
                    }
                    HelixSection::Rect { w, h } => {
                        compute::advanced::HelixShape::Rect(eval(w, &params)?, eval(h, &params)?)
                    }
                    HelixSection::Triangle { w, h } => compute::advanced::HelixShape::Triangle(
                        eval(w, &params)?,
                        eval(h, &params)?,
                    ),
                };
                let end_r = end_r.as_ref().map(|e| eval(e, &params)).transpose()?;
                let geo = compute::advanced::helix(
                    eval(r, &params)?,
                    end_r,
                    eval(pitch, &params)?,
                    eval(turns, &params)?,
                    shape,
                    eval_arr(at, &params)?,
                    eval_arr(axis, &params)?,
                    *left_hand,
                )?;
                out.detail = volume_detail(&geo);
                self.place_solids(
                    id,
                    "Helix Thread",
                    vec![geo],
                    *mode,
                    target.as_deref(),
                    &mut out,
                )?;
            }
            Op::Draft {
                body,
                faces,
                angle_deg,
                neutral,
                pull,
                ..
            } => {
                let angle = eval(angle_deg, &params)?;
                let pull = DVec3::from(eval_arr(pull, &params)?);
                let (bid, geo) = self.body(body)?;
                let idx = crate::select::select_faces(&geo.shape, faces)?;
                let n_idx = crate::select::select_faces(&geo.shape, neutral)?;
                let all = ducad_kernel::enumerate_faces(&geo.shape);
                if n_idx.len() != 1 || all[n_idx[0]].kind != SurfaceKind::Plane {
                    return Err(OpError::invalid(format!(
                        "selector bidang netral \"{neutral}\" harus menghasilkan tepat 1 face planar (cocok {})",
                        n_idx.len()
                    )));
                }
                let np = DVec3::from(all[n_idx[0]].centroid);
                let new_geo = compute::advanced::draft(&geo.shape, &idx, np, pull, pull, angle)?;
                out.detail = serde_json::json!({ "faces": idx.len(), "volume": new_geo.shape.volume().abs() });
                self.exec(Box::new(ReplaceGeometryCommand::new(
                    "Draft Angle",
                    bid,
                    new_geo,
                )));
                out.modified.push(body.clone());
            }
            Op::Mirror {
                id,
                body,
                plane,
                copy,
                merge,
            } => {
                let (point, normal) = match plane {
                    MirrorPlane::Named(n) => {
                        let normal = match n.to_ascii_lowercase().as_str() {
                            "xy" | "top" => DVec3::Z,
                            "xz" | "front" => DVec3::Y,
                            "yz" | "right" => DVec3::X,
                            _ => {
                                return Err(OpError::invalid(format!(
                                    "bidang cermin '{n}' tidak dikenal (pakai XY/XZ/YZ atau {{point, normal}})"
                                )))
                            }
                        };
                        (DVec3::ZERO, normal)
                    }
                    MirrorPlane::Custom { point, normal } => (
                        DVec3::from(eval_arr(point, &params)?),
                        DVec3::from(eval_arr(normal, &params)?),
                    ),
                };
                let (bid, geo) = self.body(body)?;
                let mirrored = compute::advanced::mirror(&geo.shape, point, normal)?;
                if *copy && *merge {
                    let merged = compute::boolean(&geo.shape, &mirrored.shape, BooleanKind::Union)?;
                    out.detail = volume_detail(&merged);
                    self.exec(Box::new(ReplaceGeometryCommand::new("Mirror", bid, merged)));
                    out.modified.push(body.clone());
                } else if *copy {
                    out.detail = volume_detail(&mirrored);
                    self.exec(Box::new(
                        AddSolidCommand::new("Mirror", mirrored).with_body_name(id.clone()),
                    ));
                    out.created.push(id.clone());
                } else {
                    out.detail = volume_detail(&mirrored);
                    self.exec(Box::new(ReplaceGeometryCommand::new(
                        "Mirror", bid, mirrored,
                    )));
                    out.modified.push(body.clone());
                }
            }
            Op::Scale {
                body,
                factor,
                pivot,
                ..
            } => {
                let f = eval(factor, &params)?;
                let (bid, geo) = self.body(body)?;
                let new_geo = compute::advanced::scale(&geo.shape, eval_arr(pivot, &params)?, f)?;
                out.detail = volume_detail(&new_geo);
                self.exec(Box::new(ReplaceGeometryCommand::new("Scale", bid, new_geo)));
                out.modified.push(body.clone());
            }
            Op::Split {
                id,
                body,
                point,
                normal,
                keep,
            } => {
                let p = DVec3::from(eval_arr(point, &params)?);
                let n = DVec3::from(eval_arr(normal, &params)?);
                let (bid, geo) = self.body(body)?;
                let (pos, neg) = compute::advanced::split(&geo.shape, p, n)?;
                let miss = || {
                    OpError::invalid("bidang potong tidak membelah body (salah satu sisi kosong)")
                        .with_hint("periksa 'point' dan 'normal' terhadap bbox body (inspect)")
                };
                match keep {
                    SplitKeep::Both => {
                        let (Some(pos), Some(neg)) = (pos, neg) else {
                            return Err(miss());
                        };
                        out.detail = serde_json::json!({
                            "volume_positive": pos.shape.volume().abs(),
                            "volume_negative": neg.shape.volume().abs(),
                        });
                        self.exec(Box::new(ReplaceGeometryCommand::new(
                            "Split Body",
                            bid,
                            pos,
                        )));
                        self.exec(Box::new(
                            AddSolidCommand::new("Split Body", neg).with_body_name(id.clone()),
                        ));
                        out.modified.push(body.clone());
                        out.created.push(id.clone());
                    }
                    SplitKeep::Positive | SplitKeep::Negative => {
                        let side = if *keep == SplitKeep::Positive {
                            pos
                        } else {
                            neg
                        };
                        let side = side.ok_or_else(miss)?;
                        out.detail = volume_detail(&side);
                        self.exec(Box::new(ReplaceGeometryCommand::new(
                            "Split Body",
                            bid,
                            side,
                        )));
                        out.modified.push(body.clone());
                    }
                }
            }
        }
        Ok(out)
    }

    /// Pasang solid hasil extrude/revolve sesuai `mode`.
    fn place_solids(
        &mut self,
        id: &str,
        label: &'static str,
        geos: Vec<BodyGeometry>,
        mode: BodyMode,
        target: Option<&str>,
        out: &mut OpOutcome,
    ) -> OpResult<()> {
        self.place_solids_with_material(id, label, geos, mode, target, None, out)
    }

    #[allow(clippy::too_many_arguments)]
    fn place_solids_with_material(
        &mut self,
        id: &str,
        label: &'static str,
        mut geos: Vec<BodyGeometry>,
        mode: BodyMode,
        target: Option<&str>,
        material: Option<ducad_core::Material>,
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
                    let mut cmd = AddSolidCommand::new(label, geo).with_body_name(id);
                    if let Some(m) = material {
                        cmd = cmd.with_material(m);
                    }
                    self.exec(Box::new(cmd));
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
                    let named: Vec<(String, BodyGeometry, Option<ducad_core::Material>)> = geos
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
                                material,
                            )
                        })
                        .collect();
                    out.created.extend(named.iter().map(|(n, _, _)| n.clone()));
                    self.exec(Box::new(AddMultipleSolidsCommand::with_materials(
                        label, named,
                    )));
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

/// Ubah spesifikasi material mekanik op menjadi `MaterialSource`, menolak
/// kunci pustaka tak dikenal dan nilai kustom yang tidak fisik.
pub fn resolve_mechanical(
    spec: &crate::ops::MechMaterialSpec,
    custom: &[(String, ducad_core::MechanicalProperties)],
) -> OpResult<ducad_core::MaterialSource> {
    match spec {
        crate::ops::MechMaterialSpec::Key(key) => {
            let source = ducad_core::MaterialSource::Library(key.to_ascii_lowercase());
            if source.resolve(custom).is_some() {
                return Ok(source);
            }
            let mut keys: Vec<String> = custom.iter().map(|(k, _)| k.clone()).collect();
            keys.extend(ducad_core::material_library().iter().map(|m| m.key.to_string()));
            Err(OpError::invalid(format!(
                "material mekanik '{key}' tidak dikenal (pilihan: {})",
                keys.join(", ")
            ))
            .with_context(serde_json::json!({ "material": key, "library": keys })))
        }
        crate::ops::MechMaterialSpec::Custom { custom: props } => {
            let props = ducad_core::MechanicalProperties::from(*props);
            props
                .validate()
                .map_err(|e| OpError::invalid(format!("material kustom tidak valid: {e}")))?;
            Ok(ducad_core::MaterialSource::Custom(props))
        }
    }
}

/// Nama preset material yang dikenal [`resolve_material`].
pub const MATERIAL_PRESETS: &[&str] = &[
    "matte_plastic",
    "glossy_plastic",
    "anodized_aluminum",
    "polished_chrome",
    "translucent_glass",
];

/// Tolak preset tak dikenal — sebelumnya jatuh diam-diam ke material default.
fn validate_material(sel: &MaterialSel) -> OpResult<()> {
    match sel {
        MaterialSel::Preset(name)
            if !MATERIAL_PRESETS.contains(&name.to_ascii_lowercase().as_str()) =>
        {
            Err(OpError::invalid(format!(
                "preset material '{name}' tidak dikenal (pilihan: default, from_style, {})",
                MATERIAL_PRESETS.join(", ")
            ))
            .with_context(serde_json::json!({ "material": name, "presets": MATERIAL_PRESETS })))
        }
        _ => Ok(()),
    }
}

pub fn resolve_material(sel: &MaterialSel, style: &ducad_sketch::Style) -> ducad_core::Material {
    match sel {
        MaterialSel::Default => ducad_core::Material::default(),
        MaterialSel::Preset(name) => match name.to_ascii_lowercase().as_str() {
            "matte_plastic" => ducad_core::Material::matte_plastic(None),
            "glossy_plastic" => ducad_core::Material::glossy_plastic(None),
            "anodized_aluminum" => ducad_core::Material::anodized_aluminum(None),
            "polished_chrome" => ducad_core::Material::polished_chrome(None),
            "translucent_glass" => ducad_core::Material::translucent_glass(None),
            _ => ducad_core::Material::default(),
        },
        MaterialSel::FromStyle => {
            let raw_color = if let Some(ref fill) = style.fill {
                Some(fill.average_color().0)
            } else {
                style
                    .stroke
                    .as_ref()
                    .map(|stroke| stroke.paint.average_color().0)
            };
            match raw_color {
                None => ducad_core::Material::default(),
                Some(mut c) => {
                    c[3] *= style.opacity;
                    if c[3] < 0.99 {
                        ducad_core::Material::translucent_glass(Some(c))
                    } else {
                        ducad_core::Material::matte_plastic(Some(c))
                    }
                }
            }
        }
    }
}

fn resolve_material_for_selection(
    material: &MaterialSel,
    sk: &Sketch,
    ids: &HashSet<EntityId>,
) -> ducad_core::Material {
    match material {
        MaterialSel::Default => ducad_core::Material::default(),
        MaterialSel::Preset(_) => resolve_material(material, &ducad_sketch::Style::cad_default()),
        MaterialSel::FromStyle => {
            let style = crate::compute::solid::selection_style(sk, ids);
            resolve_material(material, &style)
        }
    }
}

/// Nama body per-objek: `<op>.<nama entitas>`, atau `<op>.p<k>` (posisi di
/// draw order) bila entitas tidak bernama atau namanya dibuat otomatis oleh
/// `build_sketch` (`auto_named`).
fn entity_body_name(
    id: &str,
    eid: EntityId,
    sketch: &Sketch,
    draw_order: &[EntityId],
    auto_named: bool,
) -> String {
    match sketch.entity_names.get(&eid) {
        Some(name) if !auto_named => format!("{id}.{name}"),
        _ => {
            let k = draw_order
                .iter()
                .position(|&x| x == eid)
                .map_or(1, |pos| pos + 1);
            format!("{id}.p{k}")
        }
    }
}

fn extrude_single_entity(
    sk: &Sketch,
    eid: EntityId,
    outline: Option<&OutlineSpec>,
    frame: &PlaneFrame,
    extent: ExtrudeExtent,
    params: &Params,
) -> OpResult<BodyGeometry> {
    let width_opt = if let Some(outline_spec) = outline {
        if let Some(ref w) = outline_spec.width {
            let width = eval(w, params)?;
            if width <= 0.0 {
                return Err(OpError::invalid(format!(
                    "lebar outline harus > 0, didapat {width}"
                )));
            }
            Some(width)
        } else {
            let stroke_style = sk
                .styles
                .get(eid)
                .and_then(|s| s.stroke.clone())
                .unwrap_or_else(|| ducad_sketch::Style::cad_default().stroke.unwrap());
            Some(stroke_style.width_mm)
        }
    } else {
        None
    };
    compute::extrude_single_entity(sk, eid, width_opt, frame, extent)
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

/// Entitas yang tercakup pilihan profil: nama → entitas bernama itu;
/// `All` → semua entitas non-konstruksi yang terlihat; `At` → batas region
/// di titik itu.
fn selection_entities(
    sk: &Sketch,
    profile: &ProfileSel,
    params: &Params,
) -> OpResult<HashSet<EntityId>> {
    Ok(match profile {
        ProfileSel::Names { .. } => profile_entities(sk, profile)?,
        ProfileSel::All(_) => sk
            .entities
            .iter()
            .filter(|(eid, e)| !e.is_construction() && !sk.is_hidden(*eid))
            .map(|(eid, _)| eid)
            .collect(),
        ProfileSel::At { at } => {
            let [u, v] = eval_arr(at, params)?;
            ducad_sketch::find_region_at_point(sk, DVec2::new(u, v))
                .map(|r| r.entity_ids.into_iter().collect())
                .unwrap_or_default()
        }
    })
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

pub(crate) fn hole_spec(
    spec: &HoleSpecRef,
    params: &Params,
) -> OpResult<ducad_core::hole::HoleSpec> {
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

    /// Pinjaman state sesi. `pub` supaya tool bersama (`tooling`) bisa
    /// dijalankan lewat jalur yang sama oleh `Session` maupun jembatan live.
    pub fn core(&mut self) -> SessionCore<'_> {
        SessionCore {
            model: &mut self.model,
            model_undo: &mut self.model_undo,
            sketches: &mut self.sketches,
            meta: &mut self.meta,
        }
    }

    /// Jalankan studi `id` dari oplog (hasil di-cache per tanda tangan).
    pub fn run_study(
        &mut self,
        id: &str,
        cancel: &ducad_sim::CancelToken,
    ) -> OpResult<std::sync::Arc<ducad_sim::SimReport>> {
        crate::sim::run_study(&mut self.core(), id, cancel)
    }

    /// Jalankan semua studi di oplog, urut kemunculan.
    pub fn run_all_studies(
        &mut self,
        cancel: &ducad_sim::CancelToken,
    ) -> Vec<(String, OpResult<crate::sim::StudyOutcome>)> {
        crate::sim::run_all_studies(&mut self.core(), cancel)
    }

    /// Jalankan studi `id` jenis apa pun (statik, frekuensi, buckling, termal).
    pub fn run_study_any(
        &mut self,
        id: &str,
        cancel: &ducad_sim::CancelToken,
    ) -> OpResult<crate::sim::StudyOutcome> {
        crate::sim::run_study_any(&mut self.core(), id, cancel)
    }

    pub fn model(&self) -> &ModelDoc {
        &self.model
    }

    /// Ambil alih model sesi (`KernelShape` tidak `Clone`) — dipakai GUI
    /// untuk mengadopsi hasil sesi tanpa menyalin geometri.
    pub fn into_model(self) -> ModelDoc {
        self.model
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
                b.mechanical = nb.mechanical.clone();
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
        // Penimpaan material konfigurasi aktif (setelah semua op).
        if let Some(cfg) = s.meta.design.active().cloned() {
            for (body, source) in &cfg.material_overrides {
                if let Some(id) = find_body(&s.model, body) {
                    if let Some(b) = s.model.doc.bodies.get_mut(id) {
                        b.mechanical = Some(source.clone());
                    }
                }
            }
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
        Self::load(path, false)
    }

    /// Seperti [`Session::from_file`], tetapi berkas yang TIDAK basi dan
    /// gagal direproduksi dari oplog → `OplogStale` (bukan adopsi diam-diam).
    /// Dipakai `ducad-cli build` (P10.2).
    pub fn from_file_strict(path: &std::path::Path) -> OpResult<Self> {
        Self::load(path, true)
    }

    fn load(path: &std::path::Path, strict: bool) -> OpResult<Self> {
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
                    configurations: design.configurations.clone(),
                    drawings: design.drawings.clone(),
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
                    Err(e) if e.code == OpErrorCode::OplogStale && !strict => Ok(adopted),
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
                    // Material mekanik yang dipilih di GUI tidak tercatat di
                    // oplog; berkas adalah keadaan terakhirnya.
                    if nb.mechanical.is_some() {
                        b.mechanical = nb.mechanical.clone();
                    }
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
                mechanical: b.mechanical.clone(),
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

    /// Daftar konfigurasi; berkas tanpa konfigurasi → satu "Default".
    /// "Default" (tanpa penimpaan) selalu ada di urutan pertama.
    pub fn configurations(&self) -> Vec<ducad_core::Configuration> {
        let mut out = vec![ducad_core::Configuration::named(
            ducad_core::DEFAULT_CONFIGURATION,
        )];
        out.extend(self.meta.design.configurations.iter().cloned());
        out
    }

    /// Nama konfigurasi aktif (`"Default"` bila tidak ada).
    pub fn active_configuration(&self) -> &str {
        self.meta
            .design
            .active()
            .map(|c| c.name.as_str())
            .unwrap_or(ducad_core::DEFAULT_CONFIGURATION)
    }

    fn check_configuration(&self, cfg: &ducad_core::Configuration) -> OpResult<()> {
        if !ducad_core::valid_configuration_name(&cfg.name)
            || cfg.name.eq_ignore_ascii_case(ducad_core::DEFAULT_CONFIGURATION)
        {
            return Err(OpError::invalid(format!(
                "configuration name '{}' is not allowed (1-48 chars of letters, digits, space, '_', '-', '.'; \"Default\" is reserved)",
                cfg.name
            )));
        }
        let known: Vec<&str> = self.meta.design.oplog.iter().map(|o| o.id()).collect();
        if let Some(bad) = cfg.suppressed_ops.iter().find(|s| !known.contains(&s.as_str())) {
            return Err(OpError::new(
                OpErrorCode::UnknownRef,
                format!("configuration '{}' suppresses unknown op '{bad}'", cfg.name),
            )
            .with_context(serde_json::json!({ "ops": known })));
        }
        if let Some((k, v)) = cfg.params.iter().find(|(_, v)| !v.is_finite()) {
            return Err(OpError::invalid(format!(
                "configuration '{}': param '{k}' = {v} is not a finite number",
                cfg.name
            )));
        }
        Ok(())
    }

    /// Ganti seluruh daftar konfigurasi (mis. impor design table). Bila
    /// konfigurasi aktif hilang dari daftar, sesi kembali ke "Default".
    pub fn set_configurations(
        &mut self,
        configs: Vec<ducad_core::Configuration>,
    ) -> OpResult<BatchReport> {
        let mut seen = std::collections::BTreeSet::new();
        for c in &configs {
            self.check_configuration(c)?;
            if !seen.insert(c.name.clone()) {
                return Err(OpError::new(
                    OpErrorCode::DuplicateId,
                    format!("configuration '{}' is listed twice", c.name),
                ));
            }
        }
        let active = self
            .meta
            .design
            .active_configuration
            .clone()
            .filter(|a| configs.iter().any(|c| &c.name == a));
        self.swap_rebuilt(DesignDoc {
            configurations: configs,
            active_configuration: active,
            ..self.meta.design.clone()
        })
    }

    /// Tambah/perbarui satu konfigurasi (param digabung ke yang lama) lalu
    /// aktifkan. Dipakai `set_params {configuration}`.
    pub fn set_configuration_params(&mut self, name: &str, params: Params) -> OpResult<BatchReport> {
        if name.eq_ignore_ascii_case(ducad_core::DEFAULT_CONFIGURATION) {
            let mut merged = self.meta.design.params.clone();
            merged.extend(params);
            return self.swap_rebuilt(DesignDoc {
                params: merged,
                active_configuration: None,
                ..self.meta.design.clone()
            });
        }
        let mut configs = self.meta.design.configurations.clone();
        match configs.iter_mut().find(|c| c.name == name) {
            Some(existing) => existing.params.extend(params),
            None => {
                let mut cfg = ducad_core::Configuration::named(name);
                cfg.params = params;
                self.check_configuration(&cfg)?;
                configs.push(cfg);
            }
        }
        self.swap_rebuilt(DesignDoc {
            configurations: configs,
            active_configuration: Some(name.to_string()),
            ..self.meta.design.clone()
        })
    }

    /// Aktifkan konfigurasi `name` (`"Default"` = tanpa penimpaan) dan
    /// replay penuh. Nama tak dikenal → `UnknownRef`.
    pub fn activate_configuration(&mut self, name: &str) -> OpResult<BatchReport> {
        let active = if name.eq_ignore_ascii_case(ducad_core::DEFAULT_CONFIGURATION) {
            None
        } else if self.meta.design.configurations.iter().any(|c| c.name == name) {
            Some(name.to_string())
        } else {
            let known: Vec<String> = self.configurations().into_iter().map(|c| c.name).collect();
            return Err(OpError::new(
                OpErrorCode::UnknownRef,
                format!("configuration '{name}' does not exist (available: {known:?})"),
            )
            .with_context(serde_json::json!({ "configuration": name, "available": known })));
        };
        self.swap_rebuilt(DesignDoc {
            active_configuration: active,
            ..self.meta.design.clone()
        })
    }

    /// Pasang konfigurasi dari berkas ops (`OpFile.configurations`) dan
    /// aktifkan `active` bila diisi. Tanpa konfigurasi → tidak berbuat apa-apa.
    pub fn apply_configuration_specs(
        &mut self,
        specs: &[crate::ops::ConfigurationSpec],
        active: Option<&str>,
    ) -> OpResult<Option<BatchReport>> {
        if specs.is_empty() && active.is_none() {
            return Ok(None);
        }
        let library = self.model.doc.material_library.clone();
        let mut configs = Vec::with_capacity(specs.len());
        for spec in specs {
            let mut cfg = ducad_core::Configuration::named(spec.name.clone());
            cfg.params = spec.params.clone();
            cfg.suppressed_ops = spec.suppressed_ops.clone();
            for (body, material) in &spec.material_overrides {
                cfg.material_overrides
                    .insert(body.clone(), resolve_mechanical(material, &library)?);
            }
            configs.push(cfg);
        }
        let report = self.set_configurations(configs)?;
        if !report.committed {
            return Ok(Some(report));
        }
        match active {
            Some(name) => self.activate_configuration(name).map(Some),
            None => Ok(Some(report)),
        }
    }

    /// Design table CSV semua konfigurasi (tanpa "Default").
    pub fn design_table_csv(&self) -> String {
        ducad_core::design_table_to_csv(&self.meta.design.configurations)
    }

    /// Ganti konfigurasi dari design table CSV. Penimpaan material
    /// konfigurasi bernama sama dipertahankan (CSV tidak memuatnya).
    pub fn import_design_table(&mut self, csv: &str) -> OpResult<BatchReport> {
        let mut configs = ducad_core::design_table_from_csv(csv)
            .map_err(|e| OpError::invalid(format!("design table tidak valid: {e}")))?;
        for cfg in &mut configs {
            if let Some(old) = self
                .meta
                .design
                .configurations
                .iter()
                .find(|c| c.name == cfg.name)
            {
                cfg.material_overrides = old.material_overrides.clone();
            }
        }
        self.set_configurations(configs)
    }

    /// Ganti params DASAR lalu replay penuh; gagal → sesi lama utuh.
    pub fn set_params(&mut self, p: Params) -> OpResult<BatchReport> {
        self.swap_rebuilt(DesignDoc {
            params: p,
            ..self.meta.design.clone()
        })
    }

    /// Replay `design` di sesi baru; sukses → gantikan sesi ini.
    fn swap_rebuilt(&mut self, design: DesignDoc) -> OpResult<BatchReport> {
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
        let (report, diff, shapes, base_fingerprint) = self.core().propose(ops.clone())?;
        let id = format!("p{}", self.next_proposal);
        self.next_proposal += 1;
        if self.proposals.len() >= MAX_PROPOSALS {
            self.proposals.pop_front();
        }
        self.proposals.push_back(StoredProposal {
            id: id.clone(),
            ops: ops.clone(),
            edit: None,
            base_fingerprint: base_fingerprint.clone(),
        });
        Ok((
            Proposal {
                id,
                ops,
                params: None,
                replace: Vec::new(),
                remove: Vec::new(),
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
        let report = match self.proposals[pos].edit.clone() {
            Some((params, replace, remove)) => self.apply_edit(params, &replace, &remove, ops),
            None => self.run(ops, false),
        };
        if report.committed {
            if let Some(pos) = self.proposals.iter().position(|p| p.id == proposal_id) {
                self.proposals.remove(pos);
            }
        }
        report
    }

    /// Design hasil edit atas design sesi ini (lihat [`edit_design`]).
    fn edited_design(
        &self,
        params: Option<&Params>,
        replace: &[ReplaceOp],
        remove: &[String],
        append: &[Op],
    ) -> OpResult<DesignDoc> {
        edit_design(&self.meta.design, params, replace, remove, append)
    }

    /// Ganti sesi dengan hasil rebuild, pertahankan riwayat batch, proposal,
    /// dan peringatan. `removed_at`: posisi oplog (lama) op yang dihapus.
    fn adopt_rebuilt(&mut self, mut s: Session, removed_at: &[usize], appended: usize) {
        let old_len = self.meta.design.oplog.len();
        s.meta.batches =
            batches_after_remove(&std::mem::take(&mut self.meta.batches), old_len, removed_at);
        if appended > 0 {
            s.meta.batches.push(appended);
        }
        s.meta.warnings = std::mem::take(&mut self.meta.warnings);
        s.proposals = std::mem::take(&mut self.proposals);
        s.next_proposal = self.next_proposal;
        *self = s;
    }

    /// Posisi oplog op-op yang akan dihapus (id tak dikenal diabaikan;
    /// sudah divalidasi [`edit_design`]).
    fn positions_of(&self, remove: &[String]) -> Vec<usize> {
        self.meta
            .design
            .oplog
            .iter()
            .enumerate()
            .filter(|(_, o)| remove.iter().any(|r| r == o.id()))
            .map(|(i, _)| i)
            .collect()
    }

    fn apply_edit(
        &mut self,
        params: Option<Params>,
        replace: &[ReplaceOp],
        remove: &[String],
        append: Vec<Op>,
    ) -> BatchReport {
        let design = match self.edited_design(params.as_ref(), replace, remove, &append) {
            Ok(d) => d,
            Err(e) => return self.failed_report(e),
        };
        match Self::rebuild(design) {
            Ok((s, report)) => {
                let removed_at = self.positions_of(remove);
                self.adopt_rebuilt(s, &removed_at, append.len());
                BatchReport {
                    summary: self.summary(),
                    ..report
                }
            }
            Err(e) => self.failed_report(with_remove_hint(e, remove)),
        }
    }

    /// Ganti satu op di oplog lalu replay penuh; gagal → sesi lama utuh
    /// (P11). Id op tidak dikenal → `UnknownRef`.
    pub fn replace_op(&mut self, id: &str, op: Op) -> OpResult<BatchReport> {
        let replace = vec![ReplaceOp {
            id: id.to_string(),
            op,
        }];
        self.edit_oplog(None, replace, Vec::new(), false)
    }

    /// Edit oplog di tempat: params baru, op diganti (id tetap), dan/atau op
    /// dihapus, lalu replay penuh. Gagal → sesi lama utuh dan `error` di
    /// laporan. `dry_run`: hanya uji replay, `summary` = keadaan usulan.
    /// Id op tidak dikenal → `Err(UnknownRef)`.
    pub fn edit_oplog(
        &mut self,
        params: Option<Params>,
        replace: Vec<ReplaceOp>,
        remove: Vec<String>,
        dry_run: bool,
    ) -> OpResult<BatchReport> {
        // Validasi id lebih dulu agar error referensi menjadi `Err`.
        let design = self.edited_design(params.as_ref(), &replace, &remove, &[])?;
        if !dry_run {
            return Ok(self.apply_edit(params, &replace, &remove, Vec::new()));
        }
        Ok(match Self::rebuild(design) {
            Ok((copy, report)) => BatchReport {
                committed: false,
                summary: copy.summary(),
                ..report
            },
            Err(e) => self.failed_report(with_remove_hint(e, &remove)),
        })
    }

    /// Proposal umum (P11): params baru, penggantian op, dan op tambahan,
    /// diuji dengan replay pada SALINAN sesi; `report.checks` berisi hasil
    /// `design.checks` pada keadaan usulan. Tanpa params/penggantian dan
    /// tanpa checks sama dengan [`Session::propose`].
    pub fn propose_edit(
        &mut self,
        params: Option<Params>,
        replace: Vec<ReplaceOp>,
        append: Vec<Op>,
    ) -> OpResult<(Proposal, crate::diff::DiffShapes)> {
        self.propose_oplog_edit(params, replace, Vec::new(), append)
    }

    /// Seperti [`Session::propose_edit`] plus penghapusan op (`remove`).
    pub fn propose_oplog_edit(
        &mut self,
        params: Option<Params>,
        replace: Vec<ReplaceOp>,
        remove: Vec<String>,
        append: Vec<Op>,
    ) -> OpResult<(Proposal, crate::diff::DiffShapes)> {
        if params.is_none()
            && replace.is_empty()
            && remove.is_empty()
            && self.meta.design.checks.is_empty()
        {
            return self.propose(append);
        }
        let design = self.edited_design(params.as_ref(), &replace, &remove, &append)?;
        let before = crate::diff::snapshot_bodies(&self.model);
        let base_fingerprint = fingerprint(&self.model);
        let (copy, mut report) = Self::rebuild(design).map_err(|e| with_remove_hint(e, &remove))?;
        let after = crate::diff::snapshot_bodies(&copy.model);
        let (diff, shapes, _) = crate::diff::diff_bodies(before, after, true);
        report.committed = false;
        report.summary = copy.summary();
        let id = format!("p{}", self.next_proposal);
        self.next_proposal += 1;
        if self.proposals.len() >= MAX_PROPOSALS {
            self.proposals.pop_front();
        }
        self.proposals.push_back(StoredProposal {
            id: id.clone(),
            ops: append.clone(),
            edit: Some((params.clone(), replace.clone(), remove.clone())),
            base_fingerprint: base_fingerprint.clone(),
        });
        Ok((
            Proposal {
                id,
                ops: append,
                params,
                replace,
                remove,
                report,
                diff,
                base_fingerprint,
            },
            shapes,
        ))
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

    /// Ganti seluruh lembar gambar tersimpan (mis. dari `OpFile.drawings`).
    pub fn set_drawings(&mut self, drawings: Vec<ducad_io::drawing::DrawingSpec>) {
        self.meta.design.drawings = drawings;
    }

    /// Simpan/ganti satu lembar gambar menurut namanya.
    pub fn save_drawing(&mut self, spec: ducad_io::drawing::DrawingSpec) {
        self.meta.design.upsert_drawing(spec);
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

#[cfg(test)]
mod edit_tests {
    use super::batches_after_remove;

    #[test]
    fn batches_after_remove_adjusts_only_covering_batch() {
        // oplog 6 op: 1 op dari berkas (tanpa batch), batch [2, 3].
        assert_eq!(batches_after_remove(&[2, 3], 6, &[2]), vec![1, 3]);
        assert_eq!(batches_after_remove(&[2, 3], 6, &[1, 2]), vec![3]);
        assert_eq!(batches_after_remove(&[2, 3], 6, &[0]), vec![2, 3]);
        assert_eq!(batches_after_remove(&[2, 3], 6, &[3, 4, 5]), vec![2]);
        assert_eq!(batches_after_remove(&[], 3, &[1]), Vec::<usize>::new());
    }
}
