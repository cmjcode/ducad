//! Studi simulasi (P17): jembatan antara model engine dan `ducad-sim`.
//!
//! `Op::Study` hanya MENYIMPAN setup di oplog (dan memvalidasi body +
//! selector); solver dijalankan terpisah lewat [`run_study`] /
//! [`run_all_studies`] karena mahal. Hasil di-cache di
//! `SessionMeta::sim_results` bersama tanda tangan model + setup, sehingga
//! check `max_stress` dkk. tahu kapan hasilnya basi.

use ducad_sim::{
    BucklingReport, CancelToken, ElasticMaterial, FrequencyReport, ResolvedFixture, ResolvedLoad,
    ResolvedSetup, ResolvedThermalBc, ResolvedThermalSetup, SimError, SimReport, SimSetup,
    SurfaceMesh, ThermalBcKind, ThermalMaterial, ThermalReport, ThermalSetup,
};

use crate::error::{OpError, OpErrorCode, OpResult};
use crate::model::{BodyGeometry, ModelDoc};
use crate::ops::{Op, StudyKind};
use crate::session::{SessionCore, SessionMeta};

/// Hasil satu studi yang tersimpan di sesi.
#[derive(Debug, Clone)]
pub struct StudyResult {
    /// Tanda tangan (geometri body + material + setup) saat dijalankan.
    pub signature: u64,
    /// `Arc`: `SessionMeta` diklon tiap batch, medan nodal bisa besar.
    pub report: std::sync::Arc<SimReport>,
}

/// Definisi satu studi: isi `Op::Study` tanpa id.
#[derive(Debug, Clone, PartialEq)]
pub struct StudyDef {
    pub kind: StudyKind,
    pub setup: SimSetup,
    pub thermal: Option<ThermalSetup>,
    pub modes: Option<u32>,
}

impl StudyDef {
    /// Studi statik dari setup saja (bentuk P17).
    pub fn statik(setup: SimSetup) -> Self {
        Self {
            kind: StudyKind::Static,
            setup,
            thermal: None,
            modes: None,
        }
    }
}

/// Hasil studi yang bukan tegangan statik (P18).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnalysisReport {
    Frequency(FrequencyReport),
    Buckling(BucklingReport),
    Thermal(ThermalReport),
}

/// Hasil analisis yang tersimpan di sesi.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub signature: u64,
    pub report: std::sync::Arc<AnalysisReport>,
}

/// Hasil menjalankan satu studi, apa pun jenisnya.
#[derive(Debug, Clone)]
pub enum StudyOutcome {
    /// `static` dan `thermal_stress`.
    Stress(std::sync::Arc<SimReport>),
    /// `frequency`, `buckling`, `thermal`.
    Analysis(std::sync::Arc<AnalysisReport>),
}

impl StudyOutcome {
    pub fn stress(&self) -> Option<&std::sync::Arc<SimReport>> {
        match self {
            StudyOutcome::Stress(r) => Some(r),
            StudyOutcome::Analysis(_) => None,
        }
    }

    pub fn to_json(&self) -> OpResult<serde_json::Value> {
        let v = match self {
            StudyOutcome::Stress(r) => serde_json::to_value(r.as_ref()),
            StudyOutcome::Analysis(r) => serde_json::to_value(r.as_ref()),
        };
        v.map_err(|e| OpError::new(OpErrorCode::Io, format!("failed to serialize result: {e}")))
    }
}

/// Suhu acuan tanpa regangan termal (°C) untuk studi `thermal_stress`.
pub const REFERENCE_TEMPERATURE_C: f64 = 20.0;
/// Batas jumlah mode studi frekuensi/buckling.
pub const MAX_MODES: u32 = 40;

/// Besaran yang diwarnai pada render hasil studi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Overlay {
    #[default]
    Stress,
    Displacement,
    SafetyFactor,
}

/// Ubah `SimError` menjadi `OpError` berkode `sim_*`. Pesan dan hint tetap
/// bahasa Inggris (dibaca agent).
pub fn sim_error(e: SimError) -> OpError {
    let code = match &e {
        SimError::Underconstrained { .. } => OpErrorCode::SimUnderconstrained,
        SimError::MeshTooCoarse { .. } => OpErrorCode::SimMeshTooCoarse,
        SimError::Diverged { .. } => OpErrorCode::SimDiverged,
        SimError::Cancelled => OpErrorCode::SimCancelled,
        SimError::InvalidSetup(_) => OpErrorCode::InvalidParam,
        SimError::Unsupported(_) => OpErrorCode::Unsupported,
    };
    let mut err = OpError::new(code, e.to_string());
    if let Some(h) = e.hint() {
        err = err.with_hint(h);
    }
    err
}

fn body_named<'m>(
    model: &'m ModelDoc,
    meta: &SessionMeta,
    name: &str,
) -> OpResult<(&'m ducad_core::Body, &'m BodyGeometry)> {
    let found = model.doc.bodies.iter().find(|(_, b)| b.name == name);
    if let Some((id, body)) = found {
        if let Some(geo) = model.geometry.get(id) {
            return Ok((body, geo));
        }
    }
    let mut names: Vec<&str> = model.doc.bodies.values().map(|b| b.name.as_str()).collect();
    names.sort_unstable();
    let code = if meta.consumed.contains_key(name) {
        OpErrorCode::BodyConsumed
    } else {
        OpErrorCode::UnknownRef
    };
    Err(OpError::new(
        code,
        format!("body studi '{name}' tidak ditemukan (body yang ada: {names:?})"),
    )
    .with_context(serde_json::json!({ "body": name, "available": names })))
}

/// Mesh permukaan `f64` dengan tag face per segitiga untuk `ducad-sim`.
pub fn surface_mesh(geo: &BodyGeometry) -> SurfaceMesh {
    let mesh = &geo.mesh;
    let triangles: Vec<[u32; 3]> = mesh
        .indices
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    let mut tri_face = vec![0u32; triangles.len()];
    for (face, range) in mesh.face_ranges.iter().enumerate() {
        let (start, end) = (range.start as usize / 3, range.end as usize / 3);
        for slot in tri_face.iter_mut().take(end).skip(start) {
            *slot = face as u32;
        }
    }
    SurfaceMesh {
        positions: mesh
            .positions
            .iter()
            .map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
            .collect(),
        triangles,
        tri_face,
    }
}

fn unique_ids<'a>(kind: &str, ids: impl Iterator<Item = &'a str>) -> OpResult<()> {
    let mut seen = std::collections::BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(OpError::invalid(format!(
                "id {kind} '{id}' dipakai lebih dari sekali dalam satu studi"
            )));
        }
    }
    Ok(())
}

/// Selesaikan selector face setup menjadi indeks face body.
pub fn resolve_setup(geo: &BodyGeometry, setup: &SimSetup) -> OpResult<ResolvedSetup> {
    unique_ids("fixture", setup.fixtures.iter().map(|f| f.id.as_str()))?;
    unique_ids("load", setup.loads.iter().map(|l| l.id.as_str()))?;
    let faces = |selector: &str| -> OpResult<Vec<u32>> {
        Ok(crate::select::select_faces(&geo.shape, selector)?
            .into_iter()
            .map(|i| i as u32)
            .collect())
    };
    let fixtures = setup
        .fixtures
        .iter()
        .map(|f| {
            Ok(ResolvedFixture {
                id: f.id.clone(),
                faces: faces(&f.faces)?,
                kind: f.kind,
            })
        })
        .collect::<OpResult<Vec<_>>>()?;
    let loads = setup
        .loads
        .iter()
        .map(|l| {
            let is_gravity = matches!(l.kind, ducad_sim::LoadKind::Gravity { .. });
            let resolved = match (&l.faces, is_gravity) {
                (Some(sel), _) => faces(sel)?,
                (None, true) => Vec::new(),
                (None, false) => {
                    return Err(OpError::invalid(format!(
                        "beban '{}' butuh field `faces` (hanya gravity yang boleh tanpa face)",
                        l.id
                    )))
                }
            };
            Ok(ResolvedLoad {
                id: l.id.clone(),
                faces: resolved,
                kind: l.kind.clone(),
            })
        })
        .collect::<OpResult<Vec<_>>>()?;
    Ok(ResolvedSetup {
        fixtures,
        loads,
        mesh: setup.mesh.clone(),
        exact_volume_mm3: Some(geo.shape.volume().abs()),
    })
}

/// Selesaikan selector syarat batas termal. Jenis termal wajib punya
/// `thermal` dengan minimal satu suhu tetap atau konveksi; jenis lain tidak
/// boleh membawanya.
pub fn resolve_thermal(geo: &BodyGeometry, def: &StudyDef) -> OpResult<ResolvedThermalSetup> {
    let kind = def.kind.name();
    let Some(thermal) = &def.thermal else {
        if def.kind.is_thermal() {
            return Err(OpError::invalid(format!(
                "study kind `{kind}` needs field `thermal` with boundary conditions"
            ))
            .with_hint(
                "add e.g. \"thermal\":{\"boundary\":[{\"id\":\"hot\",\"faces\":\"<X\",\"kind\":\"temperature\",\"celsius\":120}]}",
            ));
        }
        return Ok(ResolvedThermalSetup::default());
    };
    if !def.kind.is_thermal() {
        return Err(OpError::invalid(format!(
            "field `thermal` is only valid for study kinds `thermal` and `thermal_stress` (got `{kind}`)"
        )));
    }
    unique_ids("thermal", thermal.boundary.iter().map(|b| b.id.as_str()))?;
    if !thermal.boundary.iter().any(|b| {
        matches!(
            b.kind,
            ThermalBcKind::Temperature { .. } | ThermalBcKind::Convection { .. }
        )
    }) {
        return Err(OpError::invalid(
            "a thermal study needs at least one `temperature` or `convection` boundary; heat flux alone has no steady state",
        ));
    }
    let boundary = thermal
        .boundary
        .iter()
        .map(|b| {
            Ok(ResolvedThermalBc {
                id: b.id.clone(),
                faces: crate::select::select_faces(&geo.shape, &b.faces)?
                    .into_iter()
                    .map(|i| i as u32)
                    .collect(),
                kind: b.kind,
            })
        })
        .collect::<OpResult<Vec<_>>>()?;
    Ok(ResolvedThermalSetup { boundary })
}

fn mode_count(def: &StudyDef) -> OpResult<usize> {
    match def.modes {
        None => Ok(0),
        Some(_) if !matches!(def.kind, StudyKind::Frequency | StudyKind::Buckling) => {
            Err(OpError::invalid(format!(
                "field `modes` is only valid for study kinds `frequency` and `buckling` (got `{}`)",
                def.kind.name()
            )))
        }
        Some(n) if (1..=MAX_MODES).contains(&n) => Ok(n as usize),
        Some(n) => Err(OpError::invalid(format!(
            "`modes` must be between 1 and {MAX_MODES} (got {n})"
        ))),
    }
}

/// Validasi murah saat `Op::Study` diterapkan: body ada, setiap selector
/// menghasilkan face, field cocok dengan jenis studi. Material diperiksa
/// saat studi dijalankan.
pub fn validate_study(model: &ModelDoc, meta: &SessionMeta, def: &StudyDef) -> OpResult<()> {
    let (_, geo) = body_named(model, meta, &def.setup.body)?;
    resolve_setup(geo, &def.setup)?;
    resolve_thermal(geo, def)?;
    mode_count(def).map(|_| ())
}

fn thermal_material(
    doc: &ducad_core::Document,
    body: &ducad_core::Body,
) -> OpResult<ThermalMaterial> {
    let p = doc.mechanical_of(body).ok_or_else(|| {
        OpError::new(
            OpErrorCode::SimNoMaterial,
            format!("body '{}' has no mechanical material", body.name),
        )
        .with_hint("run op set_material on the body first")
    })?;
    Ok(ThermalMaterial {
        conductivity_w_mk: p.thermal_conductivity_w_mk,
        expansion_per_k: p.thermal_expansion_per_k,
        reference_temperature_c: REFERENCE_TEMPERATURE_C,
    })
}

fn elastic_material(
    doc: &ducad_core::Document,
    body: &ducad_core::Body,
) -> OpResult<ElasticMaterial> {
    let Some(p) = doc.mechanical_of(body) else {
        return Err(OpError::new(
            OpErrorCode::SimNoMaterial,
            format!(
                "body '{}' has no mechanical material with a Young's modulus",
                body.name
            ),
        )
        .with_hint(
            "run op set_material on the body first, e.g. {\"op\":\"set_material\",\"id\":\"mat\",\"body\":\"<body>\",\"material\":\"al_6061_t6\"}",
        ));
    };
    Ok(ElasticMaterial {
        young_mpa: p.young_modulus_gpa * 1000.0,
        poisson: p.poisson_ratio,
        density_g_cm3: p.density_g_cm3,
        yield_mpa: p.yield_strength_mpa,
    })
}

/// FNV-1a 64-bit — stabil lintas jalan program.
fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= *b as u64;
        *h = h.wrapping_mul(0x100_0000_01b3);
    }
}

/// Tanda tangan studi statik (bentuk P17); sama dengan [`def_signature`]
/// untuk `StudyDef::statik`.
pub fn study_signature(model: &ModelDoc, setup: &SimSetup) -> Option<u64> {
    def_signature(model, &StudyDef::statik(setup.clone()))
}

/// Tanda tangan studi: sidik jari mesh body + material mekanik + definisi.
/// `None` bila body tidak ada.
pub fn def_signature(model: &ModelDoc, def: &StudyDef) -> Option<u64> {
    let setup = &def.setup;
    let (id, body) = model
        .doc
        .bodies
        .iter()
        .find(|(_, b)| b.name == setup.body)?;
    let geo = model.geometry.get(id)?;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    fnv(&mut h, &geo.mesh_fingerprint.to_le_bytes());
    if let Some(p) = model.doc.mechanical_of(body) {
        for v in [
            p.young_modulus_gpa,
            p.poisson_ratio,
            p.density_g_cm3,
            p.yield_strength_mpa,
            p.thermal_conductivity_w_mk,
            p.thermal_expansion_per_k,
        ] {
            fnv(&mut h, &v.to_bits().to_le_bytes());
        }
    }
    fnv(&mut h, serde_json::to_string(setup).ok()?.as_bytes());
    fnv(&mut h, def.kind.name().as_bytes());
    if let Some(t) = &def.thermal {
        fnv(&mut h, serde_json::to_string(t).ok()?.as_bytes());
    }
    fnv(&mut h, &def.modes.unwrap_or(0).to_le_bytes());
    Some(h)
}

/// Jalankan satu definisi studi di atas model (tanpa menyentuh cache).
pub fn run_def(
    model: &ModelDoc,
    meta: &SessionMeta,
    def: &StudyDef,
    cancel: &CancelToken,
) -> OpResult<StudyOutcome> {
    use std::sync::Arc;
    let (body, geo) = body_named(model, meta, &def.setup.body)?;
    let resolved = resolve_setup(geo, &def.setup)?;
    let thermal = resolve_thermal(geo, def)?;
    let modes = mode_count(def)?;
    let surface = surface_mesh(geo);
    let analysis = |r: AnalysisReport| StudyOutcome::Analysis(Arc::new(r));
    Ok(match def.kind {
        StudyKind::Static => {
            let material = elastic_material(&model.doc, body)?;
            StudyOutcome::Stress(Arc::new(
                ducad_sim::run_static(&surface, &material, &resolved, cancel).map_err(sim_error)?,
            ))
        }
        StudyKind::Frequency => {
            let material = elastic_material(&model.doc, body)?;
            analysis(AnalysisReport::Frequency(
                ducad_sim::run_frequency(&surface, &material, &resolved, modes, cancel)
                    .map_err(sim_error)?,
            ))
        }
        StudyKind::Buckling => {
            let material = elastic_material(&model.doc, body)?;
            analysis(AnalysisReport::Buckling(
                ducad_sim::run_buckling(&surface, &material, &resolved, modes, cancel)
                    .map_err(sim_error)?,
            ))
        }
        StudyKind::Thermal => {
            let material = thermal_material(&model.doc, body)?;
            analysis(AnalysisReport::Thermal(
                ducad_sim::run_thermal(&surface, &material, &resolved, &thermal, cancel)
                    .map_err(sim_error)?,
            ))
        }
        StudyKind::ThermalStress => {
            let material = elastic_material(&model.doc, body)?;
            let tm = thermal_material(&model.doc, body)?;
            StudyOutcome::Stress(Arc::new(
                ducad_sim::run_thermal_stress(&surface, &material, &tm, &resolved, &thermal, cancel)
                    .map_err(sim_error)?,
            ))
        }
    })
}

/// Jalankan satu setup statik di atas model (tanpa menyentuh cache).
pub fn run_static(
    model: &ModelDoc,
    meta: &SessionMeta,
    setup: &SimSetup,
    cancel: &CancelToken,
) -> OpResult<SimReport> {
    let (body, geo) = body_named(model, meta, &setup.body)?;
    let material = elastic_material(&model.doc, body)?;
    let resolved = resolve_setup(geo, setup)?;
    ducad_sim::run_static(&surface_mesh(geo), &material, &resolved, cancel).map_err(sim_error)
}

/// Masukan solver yang sudah siap dijalankan di thread lain (tidak ada lagi
/// panggilan kernel setelah ini): mesh permukaan, material, setup resolved.
pub fn prepare_static(
    model: &ModelDoc,
    meta: &SessionMeta,
    setup: &SimSetup,
) -> OpResult<(SurfaceMesh, ElasticMaterial, ResolvedSetup)> {
    let (body, geo) = body_named(model, meta, &setup.body)?;
    let material = elastic_material(&model.doc, body)?;
    let resolved = resolve_setup(geo, setup)?;
    Ok((surface_mesh(geo), material, resolved))
}

/// Studi STATIK di oplog, urut kemunculan (panel Simulasi GUI hanya
/// mengenal jenis ini). Semua jenis: [`study_defs`].
pub fn studies(meta: &SessionMeta) -> Vec<(&str, &SimSetup)> {
    meta.design
        .oplog
        .iter()
        .filter_map(|op| match op {
            Op::Study {
                id,
                setup,
                kind: StudyKind::Static,
                ..
            } => Some((id.as_str(), setup)),
            _ => None,
        })
        .collect()
}

/// Semua `Op::Study` di oplog (jenis apa pun), urut kemunculan.
pub fn study_defs(meta: &SessionMeta) -> Vec<(&str, StudyDef)> {
    meta.design
        .oplog
        .iter()
        .filter_map(|op| match op {
            Op::Study {
                id,
                kind,
                setup,
                thermal,
                modes,
            } => Some((
                id.as_str(),
                StudyDef {
                    kind: *kind,
                    setup: setup.clone(),
                    thermal: thermal.clone(),
                    modes: *modes,
                },
            )),
            _ => None,
        })
        .collect()
}

/// Definisi studi `id` dari oplog.
pub fn def_of(meta: &SessionMeta, id: &str) -> OpResult<StudyDef> {
    let mut all = study_defs(meta);
    match all.iter().position(|(sid, _)| *sid == id) {
        Some(i) => Ok(all.swap_remove(i).1),
        None => {
            let ids: Vec<&str> = all.iter().map(|(i, _)| *i).collect();
            Err(OpError::new(
                OpErrorCode::UnknownRef,
                format!("studi '{id}' tidak ada di oplog (studi yang ada: {ids:?})"),
            )
            .with_context(serde_json::json!({ "study": id, "available": ids })))
        }
    }
}

fn study_setup(meta: &SessionMeta, id: &str) -> OpResult<SimSetup> {
    def_of(meta, id).map(|d| d.setup)
}

/// Jalankan studi `id` (jenis apa pun) dari oplog dan simpan hasilnya di
/// cache sesi. Hasil yang tanda tangannya masih cocok dipakai ulang.
pub fn run_study_any(
    core: &mut SessionCore,
    id: &str,
    cancel: &CancelToken,
) -> OpResult<StudyOutcome> {
    let def = def_of(core.meta, id)?;
    let signature = def_signature(core.model, &def);
    if let Some(sig) = signature {
        if def.kind.has_stress() {
            if let Some(c) = core.meta.sim_results.get(id).filter(|c| c.signature == sig) {
                return Ok(StudyOutcome::Stress(c.report.clone()));
            }
        } else if let Some(c) = core
            .meta
            .analysis_results
            .get(id)
            .filter(|c| c.signature == sig)
        {
            return Ok(StudyOutcome::Analysis(c.report.clone()));
        }
    }
    let outcome = run_def(core.model, core.meta, &def, cancel)?;
    if let Some(signature) = signature {
        match &outcome {
            StudyOutcome::Stress(report) => {
                core.meta.sim_results.insert(
                    id.to_string(),
                    StudyResult {
                        signature,
                        report: report.clone(),
                    },
                );
            }
            StudyOutcome::Analysis(report) => {
                core.meta.analysis_results.insert(
                    id.to_string(),
                    AnalysisResult {
                        signature,
                        report: report.clone(),
                    },
                );
            }
        }
    }
    Ok(outcome)
}

/// Jalankan studi `id` yang menghasilkan tegangan (`static`,
/// `thermal_stress`); jenis lain ditolak dengan petunjuk.
pub fn run_study(
    core: &mut SessionCore,
    id: &str,
    cancel: &CancelToken,
) -> OpResult<std::sync::Arc<SimReport>> {
    let kind = def_of(core.meta, id)?.kind;
    if !kind.has_stress() {
        return Err(OpError::invalid(format!(
            "study '{id}' is a `{}` study and has no stress/displacement field",
            kind.name()
        ))
        .with_hint("stress overlays need a `static` or `thermal_stress` study"));
    }
    match run_study_any(core, id, cancel)? {
        StudyOutcome::Stress(r) => Ok(r),
        StudyOutcome::Analysis(_) => Err(OpError::invalid(format!(
            "study '{id}' returned no stress result"
        ))),
    }
}

/// Id studi yang dimaksud: `Some(id)` apa adanya, `None` = satu-satunya
/// studi di oplog.
pub fn pick_study(meta: &SessionMeta, id: Option<&str>) -> OpResult<String> {
    if let Some(id) = id {
        return study_setup(meta, id).map(|_| id.to_string());
    }
    let all = study_defs(meta);
    match all.as_slice() {
        [(only, _)] => Ok(only.to_string()),
        _ => {
            let ids: Vec<&str> = all.iter().map(|(i, _)| *i).collect();
            Err(OpError::invalid(format!(
                "pass `study` (the design has {} study ops: {ids:?}) or an inline `setup`",
                ids.len()
            ))
            .with_context(serde_json::json!({ "available": ids })))
        }
    }
}

/// Body + tegangan luluh sebuah setup, untuk render hasil.
pub fn study_body<'m>(
    model: &'m ModelDoc,
    meta: &SessionMeta,
    setup: &SimSetup,
) -> OpResult<(&'m BodyGeometry, f64)> {
    let (body, geo) = body_named(model, meta, &setup.body)?;
    Ok((geo, elastic_material(&model.doc, body)?.yield_mpa))
}

/// Setup studi `id` dari oplog.
pub fn setup_of(meta: &SessionMeta, id: &str) -> OpResult<SimSetup> {
    study_setup(meta, id)
}

/// Jalankan semua studi di oplog; `(id, hasil)` per studi, urut oplog.
pub fn run_all_studies(
    core: &mut SessionCore,
    cancel: &CancelToken,
) -> Vec<(String, OpResult<StudyOutcome>)> {
    let ids: Vec<String> = study_defs(core.meta)
        .into_iter()
        .map(|(id, _)| id.to_string())
        .collect();
    ids.into_iter()
        .map(|id| {
            let r = run_study_any(core, &id, cancel);
            (id, r)
        })
        .collect()
}

/// Hasil studi `id` yang masih segar (tanda tangan cocok dengan model
/// sekarang). Pesan error berbahasa Indonesia (tampil di panel checks).
pub fn fresh_result<'m>(
    model: &ModelDoc,
    meta: &'m SessionMeta,
    id: &str,
) -> Result<&'m SimReport, String> {
    let def = def_of(meta, id).map_err(|e| e.message)?;
    if !def.kind.has_stress() {
        return Err(format!(
            "studi '{id}' berjenis {} dan tidak punya hasil tegangan/deformasi",
            def.kind.name()
        ));
    }
    let cached = meta.sim_results.get(id).ok_or_else(|| {
        format!("studi '{id}' belum dijalankan (simulate_static / ducad-cli sim)")
    })?;
    if def_signature(model, &def) != Some(cached.signature) {
        return Err(format!(
            "hasil studi '{id}' sudah basi: model atau setup berubah; jalankan ulang"
        ));
    }
    Ok(cached.report.as_ref())
}

/// Hasil segar studi frekuensi/buckling/termal `id`.
pub fn fresh_analysis<'m>(
    model: &ModelDoc,
    meta: &'m SessionMeta,
    id: &str,
) -> Result<&'m AnalysisReport, String> {
    let def = def_of(meta, id).map_err(|e| e.message)?;
    let cached = meta.analysis_results.get(id).ok_or_else(|| {
        if def.kind.has_stress() {
            format!(
                "studi '{id}' berjenis {} dan tidak punya hasil frekuensi/buckling/suhu",
                def.kind.name()
            )
        } else {
            format!("studi '{id}' belum dijalankan (simulate_static / ducad-cli sim)")
        }
    })?;
    if def_signature(model, &def) != Some(cached.signature) {
        return Err(format!(
            "hasil studi '{id}' sudah basi: model atau setup berubah; jalankan ulang"
        ));
    }
    Ok(cached.report.as_ref())
}

// ---- Render heatmap -------------------------------------------------------

/// Colormap "turbo" (pendekatan polinomial Mikhailov), `t` di 0..1.
pub fn turbo(t: f64) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let r = 0.13572138
        + t * (4.61539260
            + t * (-42.66032258 + t * (132.13108234 + t * (-152.94239396 + t * 59.28637943))));
    let g = 0.09140261
        + t * (2.19418839
            + t * (4.84296658 + t * (-14.18503333 + t * (4.27729857 + t * 2.82956604))));
    let b = 0.10667330
        + t * (12.64194608
            + t * (-60.58204836 + t * (110.36276771 + t * (-89.90310912 + t * 27.34824973))));
    [r, g, b].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Opsi render hasil studi.
#[derive(Debug, Clone)]
pub struct StudyRenderOptions {
    pub view: crate::render::View,
    pub width: u32,
    pub height: u32,
    pub overlay: Overlay,
    /// Pengali deformasi yang digambar; `None` = otomatis (deformasi
    /// maksimum tampak ±5 % diagonal bbox), `Some(0.0)` = tanpa deformasi.
    pub deform_scale: Option<f64>,
    /// Tegangan luluh untuk overlay faktor keamanan.
    pub yield_mpa: f64,
}

const MAX_HEATMAP_TRIANGLES: usize = 24_000;

type P3 = [f64; 3];

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: P3) -> P3 {
    let n = dot(a, a).sqrt().max(1e-30);
    [a[0] / n, a[1] / n, a[2] / n]
}
fn mid(a: P3, b: P3) -> P3 {
    [
        (a[0] + b[0]) / 2.0,
        (a[1] + b[1]) / 2.0,
        (a[2] + b[2]) / 2.0,
    ]
}

/// Nilai overlay di satu titik. Faktor keamanan dibatasi 10 agar skala
/// warna tidak didominasi daerah nyaris tanpa tegangan.
pub fn overlay_value(
    overlay: Overlay,
    von_mises: f64,
    displacement: [f64; 3],
    yield_mpa: f64,
) -> f64 {
    match overlay {
        Overlay::Stress => von_mises,
        Overlay::Displacement => dot(displacement, displacement).sqrt(),
        Overlay::SafetyFactor => {
            if von_mises > 1e-12 {
                (yield_mpa / von_mises).min(10.0)
            } else {
                10.0
            }
        }
    }
}

/// Segitiga permukaan body dengan nilai medan di tiap sudutnya.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatTriangle {
    pub position: [[f64; 3]; 3],
    pub displacement: [[f64; 3]; 3],
    pub von_mises: [f64; 3],
}

/// Pecah permukaan body sampai seukuran sel mesh FE (dengan anggaran
/// `max_triangles`) dan sampel medan hasil di tiap sudut. Dipakai render
/// SVG/PNG dan overlay viewport GUI. Kosong bila laporan tanpa medan.
pub fn study_triangles(
    geo: &BodyGeometry,
    report: &SimReport,
    max_triangles: usize,
) -> Vec<HeatTriangle> {
    let Some(field) = report.nodal_field.as_ref() else {
        return Vec::new();
    };
    let surface = surface_mesh(geo);
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in &surface.positions {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    if surface.triangles.is_empty() || lo[0] > hi[0] {
        return Vec::new();
    }
    let diag = dot(sub(hi, lo), sub(hi, lo)).sqrt().max(1e-9);
    let budget = max_triangles.max(surface.triangles.len());
    // Pecah segitiga sampai sisi terpanjang <= target, dengan anggaran total.
    let mut target = report.mesh_stats.cell_mm.max(diag / 400.0);
    let mut tris: Vec<[P3; 3]> = Vec::new();
    for _ in 0..12 {
        tris.clear();
        let mut stack: Vec<[P3; 3]> = surface
            .triangles
            .iter()
            .filter_map(|t| {
                Some([
                    *surface.positions.get(t[0] as usize)?,
                    *surface.positions.get(t[1] as usize)?,
                    *surface.positions.get(t[2] as usize)?,
                ])
            })
            .collect();
        let mut over = false;
        while let Some(t) = stack.pop() {
            let len2 = |a: P3, b: P3| dot(sub(a, b), sub(a, b));
            let e = [len2(t[0], t[1]), len2(t[1], t[2]), len2(t[2], t[0])];
            if e.iter().all(|l| *l <= target * target) {
                tris.push(t);
            } else {
                let (m01, m12, m20) = (mid(t[0], t[1]), mid(t[1], t[2]), mid(t[2], t[0]));
                stack.push([t[0], m01, m20]);
                stack.push([m01, t[1], m12]);
                stack.push([m20, m12, t[2]]);
                stack.push([m01, m12, m20]);
            }
            if tris.len() + stack.len() > budget * 2 {
                over = true;
                break;
            }
        }
        if !over && tris.len() <= budget {
            break;
        }
        target *= 1.6;
    }
    tris.into_iter()
        .map(|position| {
            let samples = position.map(|p| field.sample(p));
            HeatTriangle {
                position,
                displacement: samples.map(|s| s.displacement),
                von_mises: samples.map(|s| s.von_mises_mpa),
            }
        })
        .collect()
}

/// Render hasil studi sebagai SVG: permukaan body dipecah sampai seukuran
/// sel mesh, tiap segitiga diwarnai nilai medan di titik beratnya, digambar
/// dari belakang ke depan (painter), dengan legenda nilai.
pub fn render_study_svg(
    geo: &BodyGeometry,
    report: &SimReport,
    opt: &StudyRenderOptions,
) -> OpResult<String> {
    if report.nodal_field.is_none() {
        return Err(OpError::new(
            OpErrorCode::Unsupported,
            "hasil studi tidak membawa medan nodal untuk dirender",
        ));
    }
    if opt.width == 0 || opt.height == 0 || opt.width > 8192 || opt.height > 8192 {
        return Err(OpError::invalid(format!(
            "ukuran render harus 1..=8192 piksel (diberikan {}x{})",
            opt.width, opt.height
        )));
    }
    let surface = surface_mesh(geo);
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in &surface.positions {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    if surface.triangles.is_empty() || lo[0] > hi[0] {
        return Err(OpError::new(
            OpErrorCode::EmptyResult,
            "body studi tidak punya mesh",
        ));
    }
    let diag = dot(sub(hi, lo), sub(hi, lo)).sqrt().max(1e-9);
    let deform = match opt.deform_scale {
        Some(s) if s.is_finite() && s >= 0.0 => s,
        Some(s) => {
            return Err(OpError::invalid(format!(
                "deform_scale harus >= 0 (diberikan {s})"
            )))
        }
        None if report.max_displacement_mm > 1e-12 => 0.05 * diag / report.max_displacement_mm,
        None => 0.0,
    };

    let tris = study_triangles(geo, report, MAX_HEATMAP_TRIANGLES);

    // Basis kamera ortografik.
    let (dir, up) = opt.view.direction_up();
    let forward = unit([-dir.x as f64, -dir.y as f64, -dir.z as f64]);
    let right = unit(cross(forward, [up.x as f64, up.y as f64, up.z as f64]));
    let cam_up = cross(right, forward);

    let value_of = |von_mises: f64, displacement: P3| -> f64 {
        overlay_value(opt.overlay, von_mises, displacement, opt.yield_mpa)
    };

    struct Tri {
        px: [[f64; 2]; 3],
        depth: f64,
        value: f64,
    }
    let mut drawn: Vec<Tri> = Vec::with_capacity(tris.len());
    let (mut min_xy, mut max_xy) = ([f64::MAX; 2], [f64::MIN; 2]);
    for t in &tris {
        let value = (0..3)
            .map(|k| value_of(t.von_mises[k], t.displacement[k]))
            .sum::<f64>()
            / 3.0;
        let moved = [0, 1, 2].map(|k| {
            let (p, d) = (t.position[k], t.displacement[k]);
            [
                p[0] + d[0] * deform,
                p[1] + d[1] * deform,
                p[2] + d[2] * deform,
            ]
        });
        // Buang sisi belakang (normal searah pandangan).
        let normal = cross(sub(moved[1], moved[0]), sub(moved[2], moved[0]));
        if dot(normal, forward) > 0.0 {
            continue;
        }
        let px = moved.map(|p| [dot(p, right), dot(p, cam_up)]);
        for q in &px {
            for a in 0..2 {
                min_xy[a] = min_xy[a].min(q[a]);
                max_xy[a] = max_xy[a].max(q[a]);
            }
        }
        let depth =
            (dot(moved[0], forward) + dot(moved[1], forward) + dot(moved[2], forward)) / 3.0;
        drawn.push(Tri { px, depth, value });
    }
    if drawn.is_empty() {
        return Err(OpError::new(
            OpErrorCode::EmptyResult,
            "tidak ada permukaan yang menghadap kamera",
        ));
    }
    // Jauh → dekat.
    drawn.sort_by(|a, b| b.depth.total_cmp(&a.depth));

    let (vmin, vmax) = drawn.iter().fold((f64::MAX, f64::MIN), |(lo, hi), t| {
        (lo.min(t.value), hi.max(t.value))
    });
    let span = (vmax - vmin).max(1e-12);
    // Faktor keamanan: merah = rendah, jadi skala dibalik.
    let color_t = |v: f64| {
        let t = (v - vmin) / span;
        if opt.overlay == Overlay::SafetyFactor {
            1.0 - t
        } else {
            t
        }
    };

    let (w, h) = (opt.width as f64, opt.height as f64);
    let legend_w = 90.0;
    let margin = 20.0;
    let avail = [
        (w - legend_w - 2.0 * margin).max(1.0),
        (h - 2.0 * margin).max(1.0),
    ];
    let extent = [
        (max_xy[0] - min_xy[0]).max(1e-9),
        (max_xy[1] - min_xy[1]).max(1e-9),
    ];
    let scale = (avail[0] / extent[0]).min(avail[1] / extent[1]);
    let offset = [
        margin + (avail[0] - extent[0] * scale) / 2.0,
        margin + (avail[1] - extent[1] * scale) / 2.0,
    ];
    let to_px = |q: [f64; 2]| {
        (
            offset[0] + (q[0] - min_xy[0]) * scale,
            // Sumbu y SVG ke bawah.
            offset[1] + (max_xy[1] - q[1]) * scale,
        )
    };

    use std::fmt::Write as _;
    let mut svg = String::with_capacity(drawn.len() * 96);
    let _ = write!(
        svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}" fill="#ffffff"/>"##
    );
    for t in &drawn {
        let [r, g, b] = turbo(color_t(t.value));
        let (a, bb, c) = (to_px(t.px[0]), to_px(t.px[1]), to_px(t.px[2]));
        let _ = write!(
            svg,
            r##"<path d="M{:.1} {:.1}L{:.1} {:.1}L{:.1} {:.1}Z" fill="#{r:02x}{g:02x}{b:02x}" stroke="#{r:02x}{g:02x}{b:02x}" stroke-width="0.6"/>"##,
            a.0, a.1, bb.0, bb.1, c.0, c.1
        );
    }
    // Legenda: 24 pita warna + nilai maksimum/minimum + satuan.
    let (lx, ly, lh) = (
        w - legend_w + 10.0,
        margin + 20.0,
        (h - 2.0 * margin - 40.0).max(24.0),
    );
    let bands = 24;
    for i in 0..bands {
        let t = 1.0 - (i as f64 + 0.5) / bands as f64;
        let [r, g, b] = turbo(if opt.overlay == Overlay::SafetyFactor {
            1.0 - t
        } else {
            t
        });
        let _ = write!(
            svg,
            r##"<rect x="{lx:.1}" y="{:.1}" width="16" height="{:.2}" fill="#{r:02x}{g:02x}{b:02x}"/>"##,
            ly + lh * i as f64 / bands as f64,
            lh / bands as f64 + 0.5
        );
    }
    let (title, unit_label) = match opt.overlay {
        Overlay::Stress => ("von Mises", "MPa"),
        Overlay::Displacement => ("displacement", "mm"),
        Overlay::SafetyFactor => ("safety factor", ""),
    };
    let text = |x: f64, y: f64, s: &str| {
        format!(
            r##"<text x="{x:.1}" y="{y:.1}" font-family="sans-serif" font-size="11" fill="#111827">{s}</text>"##
        )
    };
    svg.push_str(&text(lx, margin + 8.0, &format!("{title} {unit_label}")));
    svg.push_str(&text(lx + 20.0, ly + 9.0, &format!("{vmax:.3}")));
    svg.push_str(&text(lx + 20.0, ly + lh, &format!("{vmin:.3}")));
    svg.push_str(&text(lx, h - 6.0, &format!("x{deform:.3}")));
    svg.push_str("</svg>");
    Ok(svg)
}
