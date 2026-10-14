//! Tool bersama MCP dan jembatan live (P5): implementasi tool yang hanya
//! butuh state sesi pinjaman ([`SessionCore`]), sehingga server MCP
//! (`ducad-mcp`) dan jembatan agent di GUI (`ducad-app`) memakai kode yang
//! SAMA — bentuk `params` dan hasilnya identik, tanpa `ducad-app` harus
//! bergantung pada `ducad-mcp`.
//!
//! Tool tingkat sesi yang mengubah identitas sesi (`set_params`, `undo`,
//! `redo`, proposal, buka/simpan berkas) tetap di pemanggilnya: keduanya
//! punya semantik berbeda (server MCP memiliki `Session`, GUI memiliki
//! dokumennya sendiri beserta tumpukan undo-nya).

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::check::CheckItem;
use crate::error::{OpError, OpErrorCode, OpResult};
use crate::inspect::{summarize_core, DEFAULT_TOPOLOGY_LIMIT};
use crate::ops::{op_schema, Op, EXAMPLES, EXAMPLE_PLATE};
use crate::render::{render_svg_core, RenderOptions, View};
use crate::select::{select_edges, select_faces, SELECTOR_CHEATSHEET};
use crate::session::SessionCore;
use ducad_kernel::SurfaceKind;

/// Tool yang bisa dijalankan hanya dengan [`SessionCore`].
pub const CORE_TOOLS: &[&str] = &[
    "simulate_static",
    "run_ops",
    "inspect",
    "query_geometry",
    "measure",
    "render_view",
    "get_oplog",
    "get_schema",
    "set_checks",
    "run_checks",
    "drawing",
    "import_step",
    "set_brief",
    "get_brief",
    "package_status",
    "submit_for_review",
    "build_package",
    "list_redlines",
    "add_redline",
    "resolve_redline",
    "import_pdf",
];

/// Tool tingkat-core yang MENGUBAH model (pemanggil GUI menyinkronkan
/// `design` dan mencatat aktivitas setelahnya).
pub const MUTATING_CORE_TOOLS: &[&str] = &["run_ops", "import_step", "import_pdf"];

/// Tool yang tidak mengubah part maupun berkas — aman dipanggil kapan saja
/// (anotasi MCP `readOnlyHint`).
pub const READ_ONLY_TOOLS: &[&str] = &[
    "inspect",
    "query_geometry",
    "measure",
    "get_oplog",
    "get_schema",
    "run_checks",
    "get_brief",
    "package_status",
    "list_redlines",
    "diff",
    "list_parts",
    "document_info",
    "get_view",
    "get_selection",
    "screenshot",
];

/// Tool yang tidak menyentuh sesi sama sekali (bisa dijawab tanpa part
/// terbuka).
pub const STATELESS_TOOLS: &[&str] = &["get_schema"];

/// Batas teks hasil tool sebelum daftar terpanjang dipangkas.
pub const MAX_TEXT_BYTES: usize = 60 * 1024;
const MAX_QUERY_ITEMS: usize = 50;

/// Payload hasil tool (+ gambar PNG opsional).
#[derive(Debug)]
pub struct ToolOut {
    pub payload: Value,
    pub image_png: Option<Vec<u8>>,
    pub is_error: bool,
}

impl ToolOut {
    pub fn ok(payload: Value) -> Self {
        Self {
            payload,
            image_png: None,
            is_error: false,
        }
    }

    pub fn err(e: OpError) -> Self {
        Self {
            payload: json!({ "error": e }),
            image_png: None,
            is_error: true,
        }
    }
}

/// Pagar path: setiap path yang datang dari tool diresolusi lewat sini.
pub trait ToolPaths {
    fn resolve(&self, p: &str) -> OpResult<PathBuf>;
}

/// Penolak semua path — dipakai konteks yang tidak boleh menulis berkas.
pub struct NoPaths;

impl ToolPaths for NoPaths {
    fn resolve(&self, _p: &str) -> OpResult<PathBuf> {
        Err(OpError::new(
            OpErrorCode::Io,
            "writing files is not available in this context",
        ))
    }
}

pub fn args<T: for<'de> Deserialize<'de>>(v: Value) -> OpResult<T> {
    serde_json::from_value(v).map_err(|e| OpError::invalid(format!("invalid tool arguments: {e}")))
}

pub fn to_value(v: impl serde::Serialize) -> OpResult<Value> {
    serde_json::to_value(v).map_err(|e| OpError::new(OpErrorCode::Io, e.to_string()))
}

/// Jalankan tool tingkat-core. Nama di luar [`CORE_TOOLS`] → `InvalidParam`,
/// supaya pemanggil bisa menanganinya sendiri lebih dulu.
fn call_core_tool_inner(
    core: &mut SessionCore,
    name: &str,
    a: Value,
    paths: &dyn ToolPaths,
) -> OpResult<ToolOut> {
    match name {
        "run_ops" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                ops: Vec<Value>,
                #[serde(default)]
                dry_run: bool,
            }
            let a: A = args(a)?;
            let _ = a.session;
            let report = core.run(parse_ops(a.ops)?, a.dry_run);
            let is_error = report.error.is_some();
            Ok(ToolOut {
                payload: to_value(report)?,
                image_png: None,
                is_error,
            })
        }
        "inspect" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                #[serde(default)]
                body: Option<String>,
                #[serde(default)]
                topology: bool,
                #[serde(default)]
                limit: Option<usize>,
            }
            let a: A = args(a)?;
            let _ = a.session;
            let limit = a.limit.unwrap_or(DEFAULT_TOPOLOGY_LIMIT).max(1);
            let summary = summarize_core(core, a.body.as_deref(), a.topology, limit)?;
            Ok(ToolOut::ok(to_value(summary)?))
        }
        "query_geometry" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                body: String,
                #[serde(default)]
                faces: Option<String>,
                #[serde(default)]
                edges: Option<String>,
            }
            let a: A = args(a)?;
            let _ = a.session;
            let (_, geo) = core.body(&a.body)?;
            let (idx, items) = match (&a.faces, &a.edges) {
                (Some(sel), None) => {
                    let idx = select_faces(&geo.shape, sel)?;
                    let all = ducad_kernel::enumerate_faces(&geo.shape);
                    let items = idx
                        .iter()
                        .take(MAX_QUERY_ITEMS)
                        .map(|&i| to_value(&all[i]))
                        .collect::<OpResult<Vec<_>>>()?;
                    (idx, items)
                }
                (None, Some(sel)) => {
                    let idx = select_edges(&geo.shape, sel)?;
                    let all = ducad_kernel::enumerate_edges(&geo.shape);
                    let items = idx
                        .iter()
                        .take(MAX_QUERY_ITEMS)
                        .map(|&i| to_value(&all[i]))
                        .collect::<OpResult<Vec<_>>>()?;
                    (idx, items)
                }
                _ => return Err(OpError::invalid("give exactly one of 'faces' or 'edges'")),
            };
            Ok(ToolOut::ok(
                json!({ "count": idx.len(), "indices": idx, "items": items }),
            ))
        }
        "measure" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                a: Value,
                b: Value,
            }
            let a: A = args(a)?;
            let _ = a.session;
            let ra = resolve_ref(core, &a.a)?;
            let rb = resolve_ref(core, &a.b)?;
            Ok(ToolOut::ok(measure(&ra, &rb)))
        }
        "render_view" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                #[serde(default)]
                view: View,
                #[serde(default)]
                hidden_lines: bool,
                #[serde(default)]
                width: Option<u32>,
                #[serde(default)]
                height: Option<u32>,
                #[serde(default)]
                bodies: Option<Vec<String>>,
                #[serde(default)]
                save_svg: Option<String>,
                #[serde(default)]
                overlay: Option<crate::sim::Overlay>,
                #[serde(default)]
                study: Option<String>,
                #[serde(default)]
                deform_scale: Option<f64>,
            }
            let a: A = args(a)?;
            let _ = a.session;
            let svg_path = a
                .save_svg
                .as_deref()
                .map(|p| paths.resolve(p))
                .transpose()?;
            let (w, h) = (a.width.unwrap_or(800), a.height.unwrap_or(600));
            if let Some(overlay) = a.overlay {
                let id = crate::sim::pick_study(core.meta, a.study.as_deref())?;
                let report = crate::sim::run_study(core, &id, &ducad_sim::CancelToken::new())?;
                let setup = crate::sim::setup_of(core.meta, &id)?;
                let (geo, yield_mpa) = crate::sim::study_body(core.model, core.meta, &setup)?;
                let svg = crate::sim::render_study_svg(
                    geo,
                    &report,
                    &crate::sim::StudyRenderOptions {
                        view: a.view,
                        width: w,
                        height: h,
                        overlay,
                        deform_scale: a.deform_scale,
                        yield_mpa,
                    },
                )?;
                if let Some(p) = &svg_path {
                    std::fs::write(p, &svg).map_err(|e| {
                        OpError::new(
                            OpErrorCode::Io,
                            format!("failed to write {}: {e}", p.display()),
                        )
                    })?;
                }
                return Ok(ToolOut {
                    payload: json!({ "study": id, "overlay": overlay, "svg_path": svg_path }),
                    image_png: png_of(&svg, w, h)?,
                    is_error: false,
                });
            }
            if a.study.is_some() || a.deform_scale.is_some() {
                return Err(OpError::invalid(
                    "`study` and `deform_scale` need `overlay` (stress, displacement or safety_factor)",
                ));
            }
            let options = RenderOptions {
                view: a.view,
                width: w,
                height: h,
                hidden_lines: a.hidden_lines,
                bodies: a.bodies,
            };
            let r = render_svg_core(core, &options)?;
            if let Some(p) = &svg_path {
                std::fs::write(p, &r.svg).map_err(|e| {
                    OpError::new(
                        OpErrorCode::Io,
                        format!("failed to write {}: {e}", p.display()),
                    )
                })?;
            }
            let png = png_of(&r.svg, w, h)?;
            Ok(ToolOut {
                payload: json!({
                    "visible_segments": r.visible_segments,
                    "hidden_segments": r.hidden_segments,
                    "svg_path": svg_path,
                }),
                image_png: png,
                is_error: false,
            })
        }
        "simulate_static" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                #[serde(default)]
                study: Option<String>,
                #[serde(default)]
                setup: Option<ducad_sim::SimSetup>,
                #[serde(default)]
                kind: Option<crate::ops::StudyKind>,
                #[serde(default)]
                thermal: Option<ducad_sim::ThermalSetup>,
                #[serde(default)]
                modes: Option<u32>,
                #[serde(default)]
                overlay: crate::sim::Overlay,
                #[serde(default)]
                view: View,
                #[serde(default)]
                deform_scale: Option<f64>,
                #[serde(default)]
                width: Option<u32>,
                #[serde(default)]
                height: Option<u32>,
            }
            let a: A = args(a)?;
            let _ = a.session;
            if a.study.is_some() && a.setup.is_some() {
                return Err(OpError::invalid(
                    "pass either `study` (id of a study op) or an inline `setup`, not both",
                ));
            }
            let cancel = ducad_sim::CancelToken::new();
            let (label, def, outcome) = match a.setup {
                Some(setup) => {
                    let def = crate::sim::StudyDef {
                        kind: a.kind.unwrap_or_default(),
                        setup,
                        thermal: a.thermal,
                        modes: a.modes,
                    };
                    let outcome = crate::sim::run_def(core.model, core.meta, &def, &cancel)?;
                    (None, def, outcome)
                }
                None => {
                    if a.kind.is_some() || a.thermal.is_some() || a.modes.is_some() {
                        return Err(OpError::invalid(
                            "`kind`, `thermal` and `modes` only apply to an inline `setup`; a stored study carries its own",
                        ));
                    }
                    let id = crate::sim::pick_study(core.meta, a.study.as_deref())?;
                    let outcome = crate::sim::run_study_any(core, &id, &cancel)?;
                    let def = crate::sim::def_of(core.meta, &id)?;
                    (Some(id), def, outcome)
                }
            };
            let setup = def.setup.clone();
            let mut payload = outcome.to_json()?;
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("study".into(), json!(label));
                obj.insert("body".into(), json!(setup.body));
                obj.insert("kind".into(), json!(def.kind.name()));
            }
            let mut out = ToolOut::ok(payload);
            // Hanya studi tegangan yang punya medan untuk diwarnai.
            if let Some(report) = outcome.stress() {
                let (geo, yield_mpa) = crate::sim::study_body(core.model, core.meta, &setup)?;
                let (w, h) = (a.width.unwrap_or(800), a.height.unwrap_or(600));
                let svg = crate::sim::render_study_svg(
                    geo,
                    report,
                    &crate::sim::StudyRenderOptions {
                        view: a.view,
                        width: w,
                        height: h,
                        overlay: a.overlay,
                        deform_scale: a.deform_scale,
                        yield_mpa,
                    },
                )?;
                if let Some(obj) = out.payload.as_object_mut() {
                    obj.insert("yield_mpa".into(), json!(yield_mpa));
                    obj.insert(
                        "accuracy".into(),
                        json!("engineering estimate (about +/-10 % on the default hex voxel mesh; a tet mesh follows curved faces better)"),
                    );
                }
                out.image_png = png_of(&svg, w, h)?;
            }
            // Check yang bergantung pada studi kini bisa dievaluasi.
            if let (Some(obj), false) = (
                out.payload.as_object_mut(),
                core.meta.design.checks.is_empty(),
            ) {
                let checks = core.meta.design.checks.clone();
                let results = crate::check::run_checks(core, &checks);
                obj.insert(
                    "checks".into(),
                    to_value(crate::check::CheckSummary::from_results(results))?,
                );
            }
            Ok(out)
        }
        "get_oplog" => {
            let a: SessionArg = args(a)?;
            let _ = a.session;
            let d = &core.meta.design;
            Ok(ToolOut::ok(json!({ "params": d.params, "ops": d.oplog })))
        }
        "get_schema" => call_stateless_tool(name, a),
        "set_checks" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                #[serde(default)]
                checks: Vec<CheckItem>,
                #[serde(default)]
                rule_packs: Vec<crate::check::RulePackRef>,
            }
            let a: A = args(a)?;
            let _ = a.session;
            let mut checks = a.checks;
            for pack in &a.rule_packs {
                checks.extend(crate::check::expand_rule_pack(pack)?);
            }
            core.meta.design.checks = checks.clone();
            let results = crate::check::run_checks(core, &checks);
            let mut out = checks_payload(core, results)?;
            if !a.rule_packs.is_empty() {
                out["disclaimer"] = json!(crate::check::RULE_DISCLAIMER);
            }
            Ok(ToolOut::ok(out))
        }
        "run_checks" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                #[serde(default)]
                checks: Option<Vec<CheckItem>>,
            }
            let a: A = args(a)?;
            let _ = a.session;
            let checks = a.checks.unwrap_or_else(|| core.meta.design.checks.clone());
            let results = crate::check::run_checks(core, &checks);
            Ok(ToolOut::ok(checks_payload(core, results)?))
        }
        "set_brief" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                brief: crate::brief::DesignBrief,
            }
            let a: A = args(a)?;
            let _ = a.session;
            a.brief.validate()?;
            core.meta.design.brief = Some(a.brief);
            brief_payload(core).map(ToolOut::ok)
        }
        "get_brief" => {
            let a: SessionArg = args(a)?;
            let _ = a.session;
            brief_payload(core).map(ToolOut::ok)
        }
        "package_status" => {
            let a: SessionArg = args(a)?;
            let _ = a.session;
            package_payload(core).map(ToolOut::ok)
        }
        "submit_for_review" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                /// Deskripsi perubahan revisi ini (masuk tabel revisi).
                #[serde(default)]
                description: String,
                #[serde(default)]
                by: String,
                #[serde(default)]
                date: Option<String>,
            }
            let a: A = args(a)?;
            let _ = a.session;
            let signature = crate::brief::release_signature(core.model, &core.meta.design);
            let mut release = core.meta.design.release.clone().unwrap_or_default();
            if release.effective_state(&signature) == crate::brief::ReleaseState::Approved {
                return Err(OpError::invalid(format!(
                    "revision {} is already approved and the design has not changed since",
                    release.revision
                ))
                .with_hint("change the design first; the next submit_for_review then opens the next revision"));
            }
            let date = a.date.unwrap_or_else(crate::brief::today);
            release.submit(&signature, &a.description, &a.by, &date);
            core.meta.design.release = Some(release);
            package_payload(core).map(ToolOut::ok)
        }
        "build_package" => build_package_tool(core, a, paths),
        "list_redlines" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                /// Sertakan redline yang sudah ditangani.
                #[serde(default)]
                all: bool,
            }
            let a: A = args(a)?;
            let _ = a.session;
            Ok(ToolOut::ok(redlines_payload(core, a.all)))
        }
        "add_redline" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                #[serde(default)]
                sheet: Option<String>,
                text: String,
                position: [f64; 2],
                #[serde(default)]
                target: Option<[f64; 2]>,
                #[serde(default)]
                author: String,
            }
            let a: A = args(a)?;
            let _ = a.session;
            let name = pick_sheet(core, a.sheet.as_deref(), true)?;
            let design = &mut core.meta.design;
            if design.drawing(&name).is_none() {
                design.upsert_drawing(ducad_io::drawing::DrawingSpec {
                    name: name.clone(),
                    ..Default::default()
                });
            }
            let Some(spec) = design.drawings.iter_mut().find(|d| d.name == name) else {
                return Err(OpError::invalid("sheet vanished while adding a redline"));
            };
            let id = ducad_io::drawing::next_redline_id(spec);
            let annotation = ducad_core::drawing_annot::Annotation::Redline {
                position: a.position,
                redline: ducad_core::drawing_annot::Redline {
                    id: id.clone(),
                    text: a.text,
                    target: a.target,
                    author: a.author,
                    resolved: false,
                    resolution: String::new(),
                },
            };
            annotation
                .validate()
                .map_err(|e| OpError::invalid(format!("invalid redline: {e}")))?;
            spec.layout.get_or_insert_with(Default::default).annotations.push(annotation);
            let mut out = redlines_payload(core, false);
            out["added"] = json!({ "sheet": name, "id": id });
            Ok(ToolOut::ok(out))
        }
        "resolve_redline" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                #[serde(default)]
                sheet: Option<String>,
                id: String,
                /// Apa yang dilakukan untuk memenuhinya.
                resolution: String,
            }
            let a: A = args(a)?;
            let _ = a.session;
            if a.resolution.trim().is_empty() {
                return Err(OpError::invalid(
                    "resolve_redline needs a `resolution` describing what was changed",
                ));
            }
            let mut hit: Option<String> = None;
            for spec in &mut core.meta.design.drawings {
                if a.sheet.as_deref().is_some_and(|s| s != spec.name) {
                    continue;
                }
                let Some(layout) = spec.layout.as_mut() else {
                    continue;
                };
                for annotation in &mut layout.annotations {
                    if let ducad_core::drawing_annot::Annotation::Redline { redline, .. } = annotation {
                        if redline.id == a.id && !redline.resolved && hit.is_none() {
                            redline.resolved = true;
                            redline.resolution = a.resolution.trim().to_string();
                            hit = Some(spec.name.clone());
                        }
                    }
                }
            }
            let Some(sheet) = hit else {
                let open = redlines_payload(core, false);
                return Err(OpError::new(
                    OpErrorCode::UnknownRef,
                    format!("no open redline '{}'", a.id),
                )
                .with_hint("list the open redlines with list_redlines")
                .with_context(open));
            };
            let mut out = redlines_payload(core, false);
            out["resolved"] = json!({ "sheet": sheet, "id": a.id });
            Ok(ToolOut::ok(out))
        }
        "drawing" => drawing_tool(core, a, paths),
        "import_step" => import_step_tool(core, a, paths),
        "import_pdf" => import_pdf_tool(core, a, paths),
        other => Err(OpError::invalid(format!(
            "tool '{other}' is not a core tool"
        ))),
    }
}

/// Lembar sasaran redline: `sheet` bila disebut, selain itu satu-satunya
/// lembar tersimpan (atau `sheet1` bila `create` dan belum ada lembar).
fn pick_sheet(core: &SessionCore, sheet: Option<&str>, create: bool) -> OpResult<String> {
    let drawings = &core.meta.design.drawings;
    match (sheet, drawings.as_slice()) {
        (Some(name), _) if drawings.iter().any(|d| d.name == name) || create => {
            Ok(name.to_string())
        }
        (Some(name), _) => Err(OpError::new(
            OpErrorCode::UnknownRef,
            format!("sheet '{name}' is not stored in the part"),
        )),
        (None, [only]) => Ok(only.name.clone()),
        (None, []) if create => Ok("sheet1".to_string()),
        (None, []) => Err(OpError::invalid("the part has no stored sheets")),
        (None, many) => Err(OpError::invalid(format!(
            "the part has {} sheets; name one with `sheet` ({:?})",
            many.len(),
            many.iter().map(|d| d.name.as_str()).collect::<Vec<_>>()
        ))),
    }
}

/// Redline semua lembar tersimpan.
fn redlines_payload(core: &SessionCore, include_resolved: bool) -> Value {
    let mut list = Vec::new();
    let mut open = 0usize;
    for spec in &core.meta.design.drawings {
        let Some(layout) = spec.layout.as_ref() else {
            continue;
        };
        for annotation in &layout.annotations {
            let ducad_core::drawing_annot::Annotation::Redline { position, redline } = annotation
            else {
                continue;
            };
            if !redline.resolved {
                open += 1;
            } else if !include_resolved {
                continue;
            }
            list.push(json!({
                "sheet": spec.name,
                "id": redline.id,
                "text": redline.text,
                "author": redline.author,
                "position": position,
                "target": redline.target,
                "resolved": redline.resolved,
                "resolution": redline.resolution,
            }));
        }
    }
    json!({ "open": open, "redlines": list })
}

fn build_package_tool(core: &mut SessionCore, a: Value, paths: &dyn ToolPaths) -> OpResult<ToolOut> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct A {
        #[serde(default)]
        session: Option<String>,
        /// PDF keluaran.
        path: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        part_number: Option<String>,
        #[serde(default)]
        author: Option<String>,
        #[serde(default)]
        date: Option<String>,
        #[serde(default)]
        sheets: Option<Vec<crate::package::SheetSpec>>,
        #[serde(default)]
        paper: Option<String>,
        /// Simpan susunan lembar ini di part.
        #[serde(default)]
        save: bool,
        /// Profil firma (JSON); bawaan: dicari otomatis.
        #[serde(default)]
        firm: Option<String>,
        /// Folder untuk laporan Markdown (traceability/compliance/cost).
        #[serde(default)]
        reports_dir: Option<String>,
    }
    let a: A = args(a)?;
    let _ = a.session;
    let path = paths.resolve(&a.path)?;
    let firm_path = match a.firm.as_deref() {
        Some(p) => Some(paths.resolve(p)?),
        None => crate::firm::find_firm_profile(None, path.parent()),
    };
    let firm = match &firm_path {
        Some(p) => crate::firm::FirmProfile::load(p)?,
        None => crate::firm::FirmProfile::default(),
    };
    let mut spec = core.meta.design.package.clone().unwrap_or_default();
    if let Some(sheets) = a.sheets {
        spec.sheets = sheets;
    }
    if a.paper.is_some() {
        spec.paper = a.paper;
    }
    let brief_title = core
        .meta
        .design
        .brief
        .as_ref()
        .map(|b| b.title.clone())
        .filter(|t| !t.trim().is_empty());
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("part")
        .to_string();
    let info = crate::package::PackageInfo {
        title: a.title.or(brief_title).unwrap_or_else(|| stem.clone()),
        part_number: a.part_number.unwrap_or(stem),
        author: a.author.unwrap_or_default(),
        date: a.date.unwrap_or_else(crate::brief::today),
    };
    let results = crate::check::run_checks(core, &core.meta.design.checks);
    let summary = crate::check::CheckSummary::from_results(results);
    let out = crate::package::build_package(core.model, core.meta, &spec, &info, &firm, Some(&summary))?;
    let io = |p: &std::path::Path, e: std::io::Error| {
        OpError::new(OpErrorCode::Io, format!("failed to write {}: {e}", p.display()))
    };
    let bytes = out.pdf_bytes();
    std::fs::write(&path, &bytes).map_err(|e| io(&path, e))?;
    let mut reports: Vec<String> = Vec::new();
    if let Some(dir) = a.reports_dir.as_deref() {
        let dir = paths.resolve(dir)?;
        std::fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;
        let mut write = |name: &str, text: String| -> OpResult<()> {
            let p = dir.join(name);
            std::fs::write(&p, text).map_err(|e| io(&p, e))?;
            reports.push(p.display().to_string());
            Ok(())
        };
        if let Some(t) = &out.traceability {
            write("traceability.md", crate::package::traceability_md(t))?;
        }
        if !summary.results.is_empty() {
            write("compliance.md", crate::package::compliance_md(&summary))?;
        }
        write("cost.md", crate::package::cost_md(&out.cost))?;
    }
    if a.save {
        core.meta.design.package = Some(spec);
    }
    let index: Vec<Value> = out
        .index()
        .into_iter()
        .map(|(n, title, kind)| json!({ "sheet": n, "title": title, "kind": kind }))
        .collect();
    Ok(ToolOut::ok(json!({
        "path": path.display().to_string(),
        "bytes": bytes.len(),
        "sheets": index,
        "state": out.state,
        "revision": out.release.revision,
        "checks": { "pass": summary.pass, "fail": summary.fail, "error": summary.error, "warn": summary.warn },
        "traceability": out.traceability.as_ref().map(|t| json!({ "verified": t.verified, "failed": t.failed, "unverified": t.unverified })),
        "unit_cost": out.cost.unit_total,
        "firm_profile": firm_path.map(|p| p.display().to_string()),
        "reports": reports,
        "warnings": out.warnings,
    })))
}

/// Hasil check + matriks keterlacakan (bila ada brief).
fn checks_payload(core: &SessionCore, results: Vec<crate::check::CheckResult>) -> OpResult<Value> {
    let trace = core
        .meta
        .design
        .brief
        .as_ref()
        .filter(|b| !b.requirements.is_empty())
        .map(|b| crate::brief::traceability(b, &results));
    let mut out = to_value(crate::check::CheckSummary::from_results(results))?;
    if let Some(t) = trace {
        out["traceability"] = to_value(t)?;
    }
    Ok(out)
}

/// Brief + keterlacakan terhadap `design.checks` + saran paket aturan.
fn brief_payload(core: &SessionCore) -> OpResult<Value> {
    let Some(brief) = core.meta.design.brief.clone() else {
        return Ok(json!({
            "brief": Value::Null,
            "hint": "no brief yet: write one with set_brief before modeling",
        }));
    };
    let results = crate::check::run_checks(core, &core.meta.design.checks);
    let trace = crate::brief::traceability(&brief, &results);
    let mut out = json!({ "brief": brief, "traceability": trace });
    if let Some(pack) = brief.suggested_rule_pack() {
        let applied = core
            .meta
            .design
            .checks
            .iter()
            .any(|c| c.rule.as_deref().is_some_and(|r| r.starts_with(pack)));
        if !applied {
            out["suggested_rule_pack"] = json!(pack);
        }
    }
    Ok(out)
}

/// Status rilis paket + ceklis review.
pub fn package_payload(core: &SessionCore) -> OpResult<Value> {
    let design = &core.meta.design;
    let signature = crate::brief::release_signature(core.model, design);
    let release = design.release.clone().unwrap_or_default();
    let results = crate::check::run_checks(core, &design.checks);
    let review = crate::brief::review_checklist(design, &results);
    Ok(json!({
        "state": release.effective_state(&signature),
        "revision": release.revision,
        "approval": release.approval,
        "approval_stale": release.approval_is_stale(&signature),
        "history": release.history,
        "signature": signature,
        "review": review,
    }))
}

/// Satu dimensi eksplisit dari tool `drawing`: selector geometri yang sama
/// dengan `query_geometry`, dipetakan ke rujukan fitur tampak.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DimensionSelector {
    /// `diameter`, `radius`, atau `hole_pattern`.
    #[serde(rename = "type")]
    kind: String,
    /// Selector face silinder, mis. `all[kind=cylinder][r=7]`.
    select: String,
    /// `front`, `top`, `right`, `section_a`, …
    view: String,
    /// Body yang dicari; default = semua body terlihat.
    #[serde(default)]
    body: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum DimensionsArg {
    Text(String),
    List(Vec<DimensionSelector>),
}

/// Ubah selector dimensi menjadi `DimensionRef` pada gambar `drawing`.
fn resolve_dimension_selectors(
    core: &SessionCore,
    drawing: &ducad_kernel::HlrDrawing,
    items: &[DimensionSelector],
) -> OpResult<Vec<ducad_core::drawing_annot::DimensionRef>> {
    use ducad_core::drawing_annot::{DimensionRef, FeatureRef};
    use ducad_kernel::ProjectedViewKind as K;
    let mut out = Vec::new();
    for item in items {
        let kind = ducad_io::drawing::parse_view_key(&item.view).ok_or_else(|| {
            OpError::invalid(format!(
                "unknown view '{}' in dimensions (front, top, right, section_<letter>)",
                item.view
            ))
        })?;
        // Sumbu gambar tampak di ruang model.
        let (right, up) = match kind {
            K::Section(c) => {
                let s = drawing.section(c).ok_or_else(|| {
                    OpError::invalid(format!("view '{}' is not on this sheet", item.view))
                })?;
                (
                    glam::Vec3::from_array(s.config.u_axis),
                    glam::Vec3::from_array(s.config.v_axis),
                )
            }
            K::Front | K::Top | K::Right => {
                let (_, r, u) = kind.camera_vectors();
                (r, u)
            }
            _ => {
                return Err(OpError::invalid(format!(
                    "dimensions cannot be placed on view '{}'",
                    item.view
                )))
            }
        };
        let toward = right.cross(up);
        let view = ducad_io::drawing::auto_dim::view_ref(kind);

        let names: Vec<String> = match &item.body {
            Some(b) => vec![b.clone()],
            None => {
                let mut v: Vec<String> = core
                    .model
                    .doc
                    .bodies
                    .values()
                    .filter(|b| b.visible)
                    .map(|b| b.name.clone())
                    .collect();
                v.sort();
                v
            }
        };
        let mut circles: Vec<FeatureRef> = Vec::new();
        let mut refs: Vec<DimensionRef> = Vec::new();
        for name in &names {
            let (_, geo) = core.body(name)?;
            let idx = match select_faces(&geo.shape, &item.select) {
                Ok(idx) => idx,
                Err(e) if e.code == OpErrorCode::SelectorEmpty && item.body.is_none() => continue,
                Err(e) => return Err(e),
            };
            let faces = ducad_kernel::enumerate_faces(&geo.shape);
            for i in idx {
                let face = &faces[i];
                let (Some(radius), Some((pt, dir))) = (face.radius, face.axis) else {
                    continue;
                };
                let p = glam::vec3(pt[0] as f32, pt[1] as f32, pt[2] as f32);
                let d = glam::vec3(dir[0] as f32, dir[1] as f32, dir[2] as f32);
                if d.dot(toward).abs() > 0.999 {
                    let fr = FeatureRef {
                        view,
                        edge: None,
                        center: [p.dot(right), p.dot(up)],
                        radius: radius as f32,
                    };
                    if !circles.iter().any(|c| {
                        (c.center[0] - fr.center[0]).hypot(c.center[1] - fr.center[1]) < 0.01
                            && (c.radius - fr.radius).abs() < 0.01
                    }) {
                        circles.push(fr);
                    }
                } else if d.dot(toward).abs() < 1e-3 {
                    // Tampak samping: rentang aksial dari kotak pembatas face.
                    let (lo, hi) = face.bbox;
                    let corners = [lo, hi].map(|c| glam::vec3(c[0] as f32, c[1] as f32, c[2] as f32));
                    let t: Vec<f32> = corners.iter().map(|c| (*c - p).dot(d)).collect();
                    let (a, b) = (p + d * t[0].min(t[1]), p + d * t[0].max(t[1]));
                    refs.push(DimensionRef::CylinderDiameter {
                        view,
                        axis_a: [a.dot(right), a.dot(up)],
                        axis_b: [b.dot(right), b.dot(up)],
                        radius: radius as f32,
                    });
                }
            }
        }
        match item.kind.as_str() {
            "hole_pattern" => {
                if circles.is_empty() {
                    return Err(OpError::new(
                        OpErrorCode::SelectorEmpty,
                        format!("dimension selector '{}' matches no circle facing view '{}'", item.select, item.view),
                    ));
                }
                out.push(DimensionRef::HolePattern { circles });
            }
            "diameter" => {
                if circles.is_empty() && refs.is_empty() {
                    return Err(OpError::new(
                        OpErrorCode::SelectorEmpty,
                        format!("dimension selector '{}' matches no cylinder in view '{}'", item.select, item.view),
                    ));
                }
                out.extend(circles.into_iter().map(|circle| DimensionRef::Diameter { circle }));
                out.extend(refs);
            }
            "radius" => {
                if circles.is_empty() {
                    return Err(OpError::new(
                        OpErrorCode::SelectorEmpty,
                        format!("dimension selector '{}' matches no arc facing view '{}'", item.select, item.view),
                    ));
                }
                out.extend(circles.into_iter().map(|arc| DimensionRef::Radius { arc }));
            }
            other => {
                return Err(OpError::invalid(format!(
                    "unknown dimension type '{other}' (diameter, radius, hole_pattern)"
                )))
            }
        }
    }
    Ok(out)
}

/// `drawing`: lembar gambar (tampak, potongan, dimensi, render berbayang) →
/// PDF/SVG/DXF. `name` merujuk lembar tersimpan; `save` menyimpannya.
fn drawing_tool(core: &mut SessionCore, a: Value, paths: &dyn ToolPaths) -> OpResult<ToolOut> {
    use ducad_io::drawing::{DimensionPolicy, DrawingSpec, ScaleSpec, SectionSpec, ShadedSpec, ViewSet};
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct A {
        #[serde(default)]
        session: Option<String>,
        format: String,
        path: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        save: bool,
        #[serde(default)]
        paper: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        part_number: Option<String>,
        #[serde(default)]
        material: Option<String>,
        #[serde(default)]
        notes: Option<Vec<String>>,
        #[serde(default)]
        annotations: Option<Vec<ducad_core::drawing_annot::Annotation>>,
        #[serde(default)]
        sections: Option<Vec<SectionSpec>>,
        #[serde(default)]
        views: Option<ViewSet>,
        #[serde(default)]
        scale: Option<ScaleSpec>,
        #[serde(default)]
        shaded: Option<Vec<ShadedSpec>>,
        #[serde(default)]
        dimensions: Option<DimensionsArg>,
        #[serde(default)]
        hidden_lines: Option<bool>,
    }
    let a: A = args(a)?;
    let _ = a.session;
    if let Some(annotations) = &a.annotations {
        ducad_core::drawing_annot::validate_annotations(annotations)
            .map_err(|e| OpError::invalid(format!("invalid drawing annotation: {e}")))?;
    }

    // Titik awal: lembar tersimpan bernama `name`, atau spec bawaan.
    let stored = a.name.as_deref().and_then(|n| core.meta.design.drawing(n).cloned());
    let from_store = stored.is_some();
    let mut spec = stored.unwrap_or_else(|| DrawingSpec {
        name: a.name.clone().unwrap_or_else(|| "sheet1".to_string()),
        ..DrawingSpec::default()
    });
    if spec.name.trim().is_empty() {
        return Err(OpError::invalid("drawing 'name' must not be empty"));
    }
    if let Some(paper) = &a.paper {
        spec.paper = ducad_io::drawing::parse_paper(paper).ok_or_else(|| {
            OpError::invalid(format!(
                "unknown paper '{paper}' (a4, a4-portrait, a3, a3-portrait)"
            ))
        })?;
    }
    if let Some(v) = a.title {
        spec.title.title = v;
    }
    if let Some(v) = a.part_number {
        spec.title.part_number = v;
    }
    if let Some(v) = a.material {
        spec.title.material = v;
    }
    if let Some(v) = a.sections {
        spec.sections = Some(v);
    }
    if let Some(v) = a.views {
        spec.views = v;
    }
    if let Some(v) = a.scale {
        spec.scale = v;
    }
    if let Some(v) = a.shaded {
        spec.shaded = v;
    }
    if let Some(v) = a.hidden_lines {
        spec.hidden_lines = v;
    }
    let mut selectors: Option<Vec<DimensionSelector>> = None;
    match a.dimensions {
        Some(DimensionsArg::Text(t)) if t.eq_ignore_ascii_case("auto") => spec.dimensions = DimensionPolicy::Auto,
        Some(DimensionsArg::Text(t)) if t.eq_ignore_ascii_case("none") => spec.dimensions = DimensionPolicy::None,
        Some(DimensionsArg::Text(t)) => {
            return Err(OpError::invalid(format!(
                "invalid dimensions '{t}' (\"auto\", \"none\", or a list of {{type, select, view}})"
            )))
        }
        Some(DimensionsArg::List(list)) => selectors = Some(list),
        None => {}
    }
    // Catatan lubang otomatis hanya untuk lembar baru; lembar tersimpan
    // sudah memuat catatannya sendiri.
    let mut notes = if from_store {
        spec.notes.clone()
    } else {
        crate::drawing_auto::hole_notes(&core.meta.design, &core.meta.design.effective_params())
    };
    if let Some(extra) = a.notes {
        if from_store {
            notes = extra;
        } else {
            notes.extend(extra);
        }
    }
    spec.notes = notes.clone();

    let path = paths.resolve(&a.path)?;
    // Sidik jari geometri KINI (bukan `design.fingerprint`, yang hanya
    // diperbarui saat simpan): kunci cache HLR dan penanda kedaluwarsa.
    let fingerprint = crate::session::fingerprint(core.model);
    let mut output = if let Some(items) = &selectors {
        // Dua langkah: bangun lembar tanpa dimensi untuk mendapat geometri
        // tampak, lalu petakan selector ke rujukan fitur.
        let mut probe = spec.clone();
        probe.dimensions = DimensionPolicy::None;
        let mut out = crate::drawing_auto::build_sheet(core.model, &probe, Some(&fingerprint))?;
        let refs = resolve_dimension_selectors(core, &out.sheet.drawing, items)?;
        spec.dimensions = DimensionPolicy::Only(refs.clone());
        out.sheet.dimension_policy = DimensionPolicy::Only(refs);
        out.sheet.generate_auto_dimensions();
        out
    } else {
        crate::drawing_auto::build_sheet(core.model, &spec, Some(&fingerprint))?
    };

    let annotation_count = match a.annotations {
        Some(list) => {
            output.sheet.annotations = list;
            output.sheet.annotations.len()
        }
        None => output.sheet.annotations.len(),
    };
    output
        .warnings
        .extend(crate::drawing_auto::write_sheet(&output.sheet, &a.format, &path)?);

    if a.save {
        let mut to_store = spec.clone();
        if !output.sheet.annotations.is_empty() {
            let mut layout = to_store.layout.take().unwrap_or_default();
            layout.annotations = output.sheet.annotations.clone();
            to_store.layout = Some(layout);
        }
        core.meta.design.upsert_drawing(to_store);
    }
    core.meta.drawing_rendered.insert(spec.name.clone(), fingerprint);

    let sheet = &output.sheet;
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let dims: Vec<&str> = sheet.auto_dimensions.iter().map(|d| d.text.as_str()).collect();
    Ok(ToolOut::ok(json!({
        "path": path,
        "bytes": bytes,
        "name": spec.name,
        "saved": a.save,
        "scale": ducad_io::drawing::format_scale_ratio(sheet.scale),
        "views": sheet.view_placements.iter().filter(|p| p.visible)
            .map(|p| ducad_io::drawing::view_key(p.kind)).collect::<Vec<_>>(),
        "sections": sheet.drawing.sections.iter().map(|s| s.label.clone()).collect::<Vec<_>>(),
        "shaded": sheet.shaded.len(),
        "dimensions": dims,
        "notes": notes,
        "annotations": annotation_count,
        "warnings": output.warnings,
    })))
}

/// `import_step`: body dari berkas STEP. Isi STEP disimpan di
/// `design.base_bodies` sehingga part tetap bisa di-replay tanpa berkas
/// aslinya; body bisa dirujuk op berikutnya dengan `name`.
fn import_step_tool(core: &mut SessionCore, a: Value, paths: &dyn ToolPaths) -> OpResult<ToolOut> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct A {
        #[serde(default)]
        session: Option<String>,
        path: String,
        name: String,
    }
    let a: A = args(a)?;
    let _ = a.session;
    if !crate::ops::is_valid_op_id(&a.name) {
        return Err(OpError::invalid(format!(
            "invalid body name '{}': must match ^[a-z][a-z0-9_]{{0,31}}$",
            a.name
        )));
    }
    let taken = core.model.doc.bodies.values().any(|b| b.name == a.name)
        || core.meta.design.oplog.iter().any(|o| o.id() == a.name);
    if taken {
        return Err(OpError::new(
            OpErrorCode::DuplicateId,
            format!("name '{}' is already used", a.name),
        ));
    }
    let path = paths.resolve(&a.path)?;
    let shape = ducad_kernel::KernelShape::read_step(&path)
        .map_err(|e| OpError::kernel("Import STEP", e))?;
    crate::compute::check_shape("Import STEP", &shape)?;
    let step = shape
        .to_step_string()
        .map_err(|e| OpError::kernel("Import STEP", e))?;
    let geo = crate::model::BodyGeometry::from_shape(shape);
    let volume = geo.shape.volume().abs();
    core.model_undo.execute(
        Box::new(
            crate::model::AddSolidCommand::new("Import STEP", geo).with_body_name(a.name.clone()),
        ),
        core.model,
    );
    core.meta
        .design
        .base_bodies
        .push(ducad_io::native::NativeBody {
            name: a.name.clone(),
            uuid: ducad_core::new_part_uuid(),
            visible: true,
            material: ducad_core::Material::default(),
            mechanical: None,
            step,
            round_history: None,
        });
    core.meta.design.fingerprint = crate::session::fingerprint(core.model);
    Ok(ToolOut::ok(
        json!({ "body": a.name, "volume": volume, "path": path }),
    ))
}

/// `import_pdf`: garis vektor dari PDF gambar lama → satu op `sketch`
/// berisi polyline konstruksi, sebagai referensi (bukan profil).
fn import_pdf_tool(core: &mut SessionCore, a: Value, paths: &dyn ToolPaths) -> OpResult<ToolOut> {
    fn default_page() -> usize {
        1
    }
    fn default_scale() -> f64 {
        1.0
    }
    fn default_points() -> usize {
        20_000
    }
    fn default_plane() -> String {
        "XY".to_string()
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct A {
        #[serde(default)]
        session: Option<String>,
        path: String,
        /// Id op sketch yang dibuat.
        sketch: String,
        #[serde(default = "default_plane")]
        plane: String,
        /// Halaman (content stream bergaris ke-n, mulai 1); 0 = semua.
        #[serde(default = "default_page")]
        page: usize,
        /// Pengali ukuran: gambar skala 1:2 → 2.
        #[serde(default = "default_scale")]
        scale: f64,
        /// Pertahankan koordinat halaman; bawaan: pojok kiri-bawah gambar ke (0, 0).
        #[serde(default)]
        keep_origin: bool,
        /// Garis biasa (bisa menjadi profil) alih-alih garis konstruksi.
        #[serde(default)]
        as_geometry: bool,
        #[serde(default = "default_points")]
        max_points: usize,
        #[serde(default)]
        dry_run: bool,
    }
    let a: A = args(a)?;
    let _ = a.session;
    if !(a.scale.is_finite() && a.scale > 0.0) {
        return Err(OpError::invalid("import_pdf: `scale` must be > 0"));
    }
    let path = paths.resolve(&a.path)?;
    let bytes = std::fs::read(&path).map_err(|e| {
        OpError::new(OpErrorCode::Io, format!("cannot read {}: {e}", path.display()))
    })?;
    let options = ducad_io::pdf_import::PdfImportOptions {
        page: (a.page > 0).then_some(a.page),
        ..Default::default()
    };
    let import = ducad_io::pdf_import::import_pdf_vectors(&bytes, &options).map_err(|e| {
        OpError::invalid(format!("import_pdf: {e}"))
            .with_hint("only vector PDFs (exported from CAD) can be imported; scans need to be redrawn")
    })?;
    let points = import.point_count();
    if points > a.max_points {
        return Err(OpError::invalid(format!(
            "import_pdf: the page has {points} points in {} polylines, more than max_points = {}",
            import.polylines.len(),
            a.max_points
        ))
        .with_hint("raise `max_points`, or import a simpler page")
        .with_context(json!({ "points": points, "polylines": import.polylines.len(), "pages": import.pages })));
    }
    let (lo, hi) = import.bbox.unwrap_or(([0.0; 2], [0.0; 2]));
    let origin = if a.keep_origin { [0.0, 0.0] } else { lo };
    let map = |p: &[f64; 2]| {
        json!([
            crate::inspect::round4((p[0] - origin[0]) * a.scale),
            crate::inspect::round4((p[1] - origin[1]) * a.scale)
        ])
    };
    let entities: Vec<Value> = import
        .polylines
        .iter()
        .map(|l| {
            json!({ "polyline": {
                "points": l.points.iter().map(map).collect::<Vec<_>>(),
                "closed": l.closed,
                "construction": !a.as_geometry,
            } })
        })
        .collect();
    let op = json!({ "op": "sketch", "id": a.sketch, "plane": a.plane, "entities": entities });
    let report = core.run(parse_ops(vec![op])?, a.dry_run);
    let is_error = report.error.is_some();
    let mut payload = json!({
        "sketch": a.sketch,
        "polylines": import.polylines.len(),
        "points": points,
        "pages": import.pages,
        "size_mm": [
            crate::inspect::round4((hi[0] - lo[0]) * a.scale),
            crate::inspect::round4((hi[1] - lo[1]) * a.scale)
        ],
        "warnings": import.warnings,
        "committed": report.committed,
    });
    if let Some(e) = report.error {
        payload["error"] = to_value(e)?;
    }
    Ok(ToolOut {
        payload,
        image_png: None,
        is_error,
    })
}

/// Tool tanpa state: dijawab tanpa sesi terbuka.
fn call_stateless_tool_inner(name: &str, a: Value) -> OpResult<ToolOut> {
    match name {
        "get_schema" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                op: Option<String>,
                #[serde(default)]
                example: Option<String>,
                #[serde(default)]
                full: bool,
                #[serde(default)]
                rule_packs: bool,
                #[serde(default)]
                checks: bool,
                #[serde(default)]
                check: Option<String>,
            }
            let a: A = args(a)?;
            if let Some(kind) = a.check.as_deref() {
                return check_detail(kind).map(ToolOut::ok);
            }
            if a.checks {
                return Ok(ToolOut::ok(check_catalog()));
            }
            if a.rule_packs {
                return Ok(ToolOut::ok(json!({
                    "rule_packs": crate::check::rule_pack_catalog(),
                    "usage": "set_checks {\"checks\": [...], \"rule_packs\": [\"dfm-cnc\", {\"pack\": \"dfm-sheet-metal\", \"body\": \"box\", \"vars\": {\"thickness\": \"$t\"}, \"requirement\": \"R3\"}]}",
                    "disclaimer": crate::check::RULE_DISCLAIMER,
                })));
            }
            let parse = |text: &str| -> OpResult<Value> {
                serde_json::from_str(text).map_err(|e| OpError::new(OpErrorCode::Io, e.to_string()))
            };
            if a.full {
                return Ok(ToolOut::ok(json!({
                    "op_schema": op_schema(),
                    "selector_cheatsheet": SELECTOR_CHEATSHEET,
                    "example": parse(EXAMPLE_PLATE)?,
                })));
            }
            if let Some(kind) = a.op.as_deref() {
                return op_detail(kind).map(ToolOut::ok);
            }
            if let Some(name) = a.example.as_deref() {
                let (_, text, covers) =
                    EXAMPLES
                        .iter()
                        .find(|(n, _, _)| *n == name)
                        .ok_or_else(|| {
                            let names: Vec<&str> = EXAMPLES.iter().map(|(n, _, _)| *n).collect();
                            OpError::new(
                                OpErrorCode::UnknownRef,
                                format!("unknown example '{name}' (available: {names:?})"),
                            )
                        })?;
                return Ok(ToolOut::ok(
                    json!({ "example": name, "covers": covers, "op_file": parse(text)? }),
                ));
            }
            Ok(ToolOut::ok(schema_overview()?))
        }
        other => Err(OpError::invalid(format!(
            "tool '{other}' is not a stateless tool"
        ))),
    }
}

// ---------------------------------------------------------------------
// Skema Op bertingkat untuk agent: ringkasan murah dulu, detail per op
// sesuai kebutuhan (skema penuh ±33 KB).
// ---------------------------------------------------------------------

/// Varian `Op` dari JSON Schema: `(jenis, skema varian)`.
fn op_variants(schema: &Value) -> Vec<(String, Value)> {
    let op = &schema["definitions"]["Op"];
    let list = op
        .get("oneOf")
        .or_else(|| op.get("anyOf"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    list.into_iter()
        .filter_map(|v| {
            let kind = v["properties"]["op"]["enum"][0].as_str()?.to_string();
            Some((kind, v))
        })
        .collect()
}

/// Semua jenis op yang sah (nilai field `"op"`).
pub fn op_kinds() -> Vec<String> {
    op_variants(&op_schema())
        .into_iter()
        .map(|(k, _)| k)
        .collect()
}

/// Kumpulkan nama definisi yang dirujuk `$ref` secara transitif.
fn collect_refs(v: &Value, defs: &Value, out: &mut std::collections::BTreeSet<String>) {
    match v {
        Value::Object(o) => {
            if let Some(name) = o
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|r| r.strip_prefix("#/definitions/"))
            {
                if out.insert(name.to_string()) {
                    collect_refs(&defs[name], defs, out);
                }
            }
            for x in o.values() {
                collect_refs(x, defs, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect_refs(x, defs, out)),
        _ => {}
    }
}

/// Skema lengkap SATU jenis op + definisi yang dirujuknya.
fn op_detail(kind: &str) -> OpResult<Value> {
    let schema = op_schema();
    let variants = op_variants(&schema);
    let Some((_, variant)) = variants.iter().find(|(k, _)| k == kind) else {
        let kinds: Vec<&str> = variants.iter().map(|(k, _)| k.as_str()).collect();
        return Err(OpError::new(
            OpErrorCode::UnknownRef,
            format!("unknown op kind '{kind}' (available: {kinds:?})"),
        ));
    };
    let defs = &schema["definitions"];
    let mut names = std::collections::BTreeSet::new();
    collect_refs(variant, defs, &mut names);
    let definitions: serde_json::Map<String, Value> = names
        .into_iter()
        .map(|n| {
            let d = defs[&n].clone();
            (n, d)
        })
        .collect();
    let examples: Vec<&str> = EXAMPLES
        .iter()
        .filter(|(_, text, _)| text.contains(&format!("\"op\":\"{kind}\"")))
        .map(|(n, _, _)| *n)
        .collect();
    Ok(json!({
        "op": kind,
        "schema": variant,
        "definitions": definitions,
        "examples": examples,
    }))
}

/// Skema `CheckItem` (JSON Schema).
fn check_schema() -> Value {
    serde_json::to_value(schemars::schema_for!(CheckItem)).unwrap_or(Value::Null)
}

/// Varian `Check`: `(jenis, skema varian)`.
fn check_variants(schema: &Value) -> Vec<(String, Value)> {
    schema
        .get("oneOf")
        .or_else(|| schema.get("anyOf"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| {
            let kind = v["properties"]["check"]["enum"][0].as_str()?.to_string();
            Some((kind, v))
        })
        .collect()
}

/// Semua jenis check yang sah (nilai field `"check"`).
pub fn check_kinds() -> Vec<String> {
    check_variants(&check_schema())
        .into_iter()
        .map(|(k, _)| k)
        .collect()
}

/// Katalog check ringkas: jenis, ringkasan, field wajib/opsional, field
/// bersama (id/note/requirement/citation/severity).
fn check_catalog() -> Value {
    let schema = check_schema();
    let checks: Vec<Value> = check_variants(&schema)
        .into_iter()
        .map(|(kind, v)| {
            let required: Vec<&str> = v["required"]
                .as_array()
                .map(|r| {
                    r.iter()
                        .filter_map(Value::as_str)
                        .filter(|f| *f != "check")
                        .collect()
                })
                .unwrap_or_default();
            let optional: Vec<&str> = v["properties"]
                .as_object()
                .map(|p| {
                    p.keys()
                        .map(String::as_str)
                        .filter(|f| *f != "check" && !required.contains(f))
                        .collect()
                })
                .unwrap_or_default();
            json!({
                "check": kind,
                "summary": v["description"].as_str().unwrap_or_default(),
                "required": required,
                "optional": optional,
            })
        })
        .collect();
    json!({
        "checks": checks,
        "common_fields": schema["properties"],
        "body_selector": "`body`: a body name or \"*\" (the only body); `bodies`: a name, a list of names, or \"*\" (all bodies). Numbers may be expressions such as \"$t\".",
        "more": "get_schema {\"check\":\"hole_web\"} = full schema of one check kind; {\"rule_packs\":true} = ready-made cited checks",
    })
}

/// Skema lengkap SATU jenis check + definisi yang dirujuknya.
fn check_detail(kind: &str) -> OpResult<Value> {
    let schema = check_schema();
    let variants = check_variants(&schema);
    let Some((_, variant)) = variants.iter().find(|(k, _)| k == kind) else {
        let kinds: Vec<&str> = variants.iter().map(|(k, _)| k.as_str()).collect();
        return Err(OpError::new(
            OpErrorCode::UnknownRef,
            format!("unknown check kind '{kind}' (available: {kinds:?})"),
        ));
    };
    let defs = &schema["definitions"];
    let mut names = std::collections::BTreeSet::new();
    collect_refs(variant, defs, &mut names);
    let definitions: serde_json::Map<String, Value> = names
        .into_iter()
        .map(|n| {
            let d = defs[&n].clone();
            (n, d)
        })
        .collect();
    Ok(json!({
        "check": kind,
        "schema": variant,
        "common_fields": schema["properties"],
        "definitions": definitions,
    }))
}

/// Ringkasan murah (±5 KB): tiap op dengan deskripsi + field wajib/opsional,
/// daftar contoh, cheatsheet selector, dan cara meminta detail.
fn schema_overview() -> OpResult<Value> {
    let schema = op_schema();
    let ops: Vec<Value> = op_variants(&schema)
        .into_iter()
        .map(|(kind, v)| {
            let required: Vec<&str> = v["required"]
                .as_array()
                .map(|r| {
                    r.iter()
                        .filter_map(Value::as_str)
                        .filter(|f| *f != "op")
                        .collect()
                })
                .unwrap_or_default();
            let optional: Vec<&str> = v["properties"]
                .as_object()
                .map(|p| {
                    p.keys()
                        .map(String::as_str)
                        .filter(|f| *f != "op" && !required.contains(f))
                        .collect()
                })
                .unwrap_or_default();
            json!({
                "op": kind,
                "summary": v["description"].as_str().unwrap_or_default(),
                "required": required,
                "optional": optional,
            })
        })
        .collect();
    let examples: Vec<Value> = EXAMPLES
        .iter()
        .map(|(n, _, covers)| json!({ "name": n, "covers": covers }))
        .collect();
    Ok(json!({
        "about": schema["definitions"]["Op"]["description"],
        "ops": ops,
        "selector_cheatsheet": SELECTOR_CHEATSHEET,
        "examples": examples,
        "check_kinds": check_kinds(),
        "more": "get_schema {\"op\":\"fillet\"} = full schema of one op kind; {\"example\":\"flange\"} = tested example OpFile; {\"full\":true} = entire JSON Schema (large); {\"checks\":true} = every check kind with its fields; {\"check\":\"mass\"} = one check kind in full; {\"rule_packs\":true} = cited DFM/standard rule packs",
    }))
}

/// Tabel kode error untuk agent (markdown, bahasa Inggris seperti seluruh
/// teks MCP). Satu sumber untuk resource `ducad://guide`.
pub const ERROR_GUIDE: &str = "\
| error.code | Meaning | Action |
|---|---|---|
| invalid_param | value out of domain / wrong tool argument or op field | read message (names op_index + valid fields) and fix it; lengths must lie in 1e-6..1e6 mm and every number in an op within +/-1e9 |
| unknown_ref | unknown body/sketch/entity/param/op name | use names from context (available/ops) or inspect/get_oplog |
| duplicate_id | op id already used | pick a new id ^[a-z][a-z0-9_]{0,31}$ or change the old op with replace_op |
| body_consumed | body was merged by a boolean or deleted | reference the resulting body in context.consumed_by |
| profile_not_closed | sketch has no closed loop | read hint (loose endpoints), close the contour |
| profile_ambiguous | profile.at point is not inside any region | pick a point inside a region / use names |
| profile_open_gap | two sketch endpoints almost meet | apply fixes (patches the sketch op) |
| selector_syntax | selector cannot be parsed | see context.pos |
| selector_empty | valid selector but 0 elements | see context.available, test with query_geometry |
| fillet_radius_too_large / chamfer_too_large | exceeds the neighboring edge (context.limit) | use fixes[i].patched_op |
| shell_too_thick | thickness >= half the smallest dimension | use fixes |
| shell_depth_too_deep | shell `depth` reaches the bottom wall (context.max_depth) | use fixes (full cavity or half the max depth) |
| hole_outside_face | hole point outside the face | `at` is relative to the face centroid; or use at_world |
| boolean_no_overlap | subtract/intersect without overlap | move a body (transform) |
| kernel_failed | OCCT failed | reduce radius/thickness, change op order |
| internal | DUCAD itself failed (a caught panic); the batch was rolled back | this is a bug, not a parameter error: try other values or another op order, and report the message with the op JSON |
| empty_result | result volume ~0 | check position/direction (direction reverse for a cut from a face) |
| constraint_unsolved / over_constrained | conflicting sketch constraints | remove one constraint |
| oplog_stale | model changed outside the oplog | bodies adopted as base_bodies; replan from inspect |
| proposal_stale | model changed since propose_ops | create a new proposal |
| sim_underconstrained | study has no fixture, or fixtures leave a rigid-body motion free | add a `fixed` fixture on at least one face |
| sim_no_material | study body has no mechanical material (E, nu) | run op set_material on the body first |
| sim_mesh_too_coarse | cells are larger than a wall or miss a loaded/fixed face | lower setup.mesh.cell_mm or raise mesh.target_elems |
| sim_diverged | solver did not converge | look for nearly disconnected or very thin regions; use a finer mesh |
| sim_cancelled | study was cancelled | run it again |
| drawing_section_empty | a `drawing` section plane/path does not cut any visible body | change the section `offset` or `path` (mm, relative to the bounding-box centre) so it passes through material |
| drawing_section_label_dup | two `drawing` sections use the same letter | give every section a different single-letter `label` |
| (warning) HLR_EXACT_FALLBACK | exact hidden-line removal failed for a view; the mesh fallback was used, so circles are polylines and that view has no associative dimensions | accept it, or simplify the offending fillets/blends |
| (warning) DRAWING_SECTION_EMPTY | the default section A-A does not cut the part | pass explicit `sections`, or `sections: []` to drop it |
| (warning) DRAWING_DXF_NO_RASTER | DXF cannot embed shaded renders | use pdf or svg for sheets with `shaded` |
| (warning) SIM_MESH_FALLBACK_HEX | the tet mesher could not mesh the body; the hex voxel mesh was used | accept the hex result, or change `mesh.cell_mm` so walls are at least two cells thick |
";

/// Anti-pola yang paling sering membuat agent berputar-putar.
pub const ANTI_PATTERNS: &str = "\
- Stacking corrective ops or repeated undo to fix an old op: use replace_op / remove_op / set_params.
- Raw coordinates although a param exists (\"w\": 60 while $w exists).
- Fillet/chamfer before a boolean: the edges change; round last.
- Picking edges with guessed idx: use semantic selectors, test with query_geometry.
- Referencing body a/b after a boolean: the result is named after the boolean op id.
- Extrude cut from a sketch on a face without direction \"reverse\": the face normal points outward, the cut misses the material.
- Skipping render_view: inspect alone does not show a feature on the wrong side.
- One giant batch: small batches (3-6 ops) are easier to diagnose.
";

/// Daftar jenis op satu baris per op (untuk panduan markdown).
pub fn op_catalog_markdown() -> String {
    op_variants(&op_schema())
        .into_iter()
        .map(|(kind, v)| {
            let summary = v["description"]
                .as_str()
                .unwrap_or_default()
                .replace('\n', " ");
            format!("- **{kind}** — {summary}\n")
        })
        .collect()
}

/// Argumen edit oplog yang sudah divalidasi, bersama untuk `replace_op`,
/// `remove_op`, dan `propose_ops` (server MCP dan jembatan live GUI).
#[derive(Debug, Default)]
pub struct EditArgs {
    pub session: Option<String>,
    /// Params BARU yang diminta (belum digabung ke params lama).
    pub params: Option<crate::ops::Params>,
    pub replace: Vec<crate::session::ReplaceOp>,
    pub remove: Vec<String>,
    /// Op yang ditambahkan di akhir (hanya `propose_ops`).
    pub append: Vec<Op>,
    pub dry_run: bool,
}

impl EditArgs {
    /// Hanya menambah op di akhir (tanpa params/penggantian/penghapusan).
    pub fn is_append_only(&self) -> bool {
        self.params.is_none() && self.replace.is_empty() && self.remove.is_empty()
    }
}

/// `{id, op}` mentah → `ReplaceOp`, dengan error parse yang menyebut id.
pub fn parse_replace(id: String, op: Value) -> OpResult<crate::session::ReplaceOp> {
    let mut ops = parse_ops(vec![op]).map_err(|mut e| {
        e.op_index = None;
        e.message = format!("replacement op for '{id}': {}", e.message);
        e
    })?;
    let op = ops
        .pop()
        .ok_or_else(|| OpError::invalid("empty replacement op"))?;
    Ok(crate::session::ReplaceOp { id, op })
}

/// Validasi argumen `replace_op` / `remove_op` / `propose_ops`.
pub fn parse_edit_args(tool: &str, a: Value) -> OpResult<EditArgs> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Replace {
        id: String,
        op: Value,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct A {
        #[serde(default)]
        session: Option<String>,
        // replace_op
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        op: Option<Value>,
        // remove_op
        #[serde(default)]
        ids: Vec<String>,
        #[serde(default)]
        dry_run: bool,
        // propose_ops
        #[serde(default)]
        ops: Vec<Value>,
        #[serde(default)]
        params: Option<crate::ops::Params>,
        #[serde(default)]
        replace: Vec<Replace>,
        #[serde(default)]
        remove: Vec<String>,
    }
    let a: A = args(a)?;
    let propose_only =
        !a.ops.is_empty() || a.params.is_some() || !a.replace.is_empty() || !a.remove.is_empty();
    let mut out = EditArgs {
        session: a.session,
        dry_run: a.dry_run,
        ..Default::default()
    };
    match tool {
        "replace_op" => {
            let (Some(id), Some(op), true, false) = (a.id, a.op, a.ids.is_empty(), propose_only)
            else {
                return Err(OpError::invalid(
                    "replace_op takes exactly 'id' and 'op' (+ optional 'dry_run')",
                ));
            };
            out.replace.push(parse_replace(id, op)?);
        }
        "remove_op" => {
            if a.id.is_some() || a.op.is_some() || a.ids.is_empty() || propose_only {
                return Err(OpError::invalid(
                    "remove_op takes 'ids' (list of op ids) (+ optional 'dry_run')",
                ));
            }
            out.remove = a.ids;
        }
        "propose_ops" => {
            if a.id.is_some() || a.op.is_some() || !a.ids.is_empty() || a.dry_run {
                return Err(OpError::invalid(
                    "propose_ops takes 'ops', 'params', 'replace', 'remove' (not id/op/ids/dry_run)",
                ));
            }
            if !propose_only {
                return Err(OpError::invalid(
                    "propose_ops needs at least one of 'ops', 'params', 'replace', 'remove'",
                ));
            }
            out.append = parse_ops(a.ops)?;
            out.params = a.params;
            out.replace = a
                .replace
                .into_iter()
                .map(|r| parse_replace(r.id, r.op))
                .collect::<OpResult<Vec<_>>>()?;
            out.remove = a.remove;
        }
        other => {
            return Err(OpError::invalid(format!(
                "'{other}' is not an oplog edit tool"
            )))
        }
    }
    Ok(out)
}

/// Parse larik op satu per satu supaya error menyebut `op_index`/`op_id`
/// op yang salah, bukan hanya pesan serde untuk seluruh batch.
pub fn parse_ops(raw: Vec<Value>) -> OpResult<Vec<Op>> {
    raw.into_iter()
        .enumerate()
        .map(|(i, v)| {
            let id = v.get("id").and_then(Value::as_str).map(str::to_string);
            let kind = v.get("op").and_then(Value::as_str).map(str::to_string);
            serde_json::from_value::<Op>(v).map_err(|e| {
                let mut err = OpError::invalid(format!(
                    "op #{i}{}: {e}",
                    id.as_deref()
                        .map(|s| format!(" ('{s}')"))
                        .unwrap_or_default()
                ))
                .with_hint(match &kind {
                    Some(k) => format!("see the valid fields with get_schema {{\"op\":\"{k}\"}}"),
                    None => "every op needs fields \"op\" and \"id\"; see get_schema".to_string(),
                });
                err.op_index = Some(i);
                err.op_id = id;
                err
            })
        })
        .collect()
}

#[cfg(feature = "raster")]
fn png_of(svg: &str, w: u32, h: u32) -> OpResult<Option<Vec<u8>>> {
    crate::render::svg_to_png(svg, w, h).map(Some)
}

#[cfg(not(feature = "raster"))]
fn png_of(_svg: &str, _w: u32, _h: u32) -> OpResult<Option<Vec<u8>>> {
    Ok(None)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionArg {
    #[serde(default)]
    pub session: Option<String>,
}

/// Rujukan terukur: titik wakil + arah opsional + info bidang.
pub struct Ref {
    point: [f64; 3],
    dir: Option<[f64; 3]>,
    /// (titik, normal) bila rujukan adalah face planar.
    plane: Option<([f64; 3], [f64; 3])>,
}

pub fn resolve_ref(core: &SessionCore, v: &Value) -> OpResult<Ref> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct R {
        #[serde(default)]
        point: Option<[f64; 3]>,
        #[serde(default)]
        body: Option<String>,
        #[serde(default)]
        face: Option<String>,
        #[serde(default)]
        edge: Option<String>,
    }
    let r: R = args(v.clone())?;
    let exactly_one = |n: usize, sel: &str| -> OpResult<()> {
        if n != 1 {
            return Err(OpError::invalid(format!(
                "selector \"{sel}\" must match exactly 1 element (matched {n})"
            ))
            .with_context(json!({ "matched": n })));
        }
        Ok(())
    };
    match (r.point, r.body, r.face, r.edge) {
        (Some(p), None, None, None) => Ok(Ref {
            point: p,
            dir: None,
            plane: None,
        }),
        (None, Some(body), Some(sel), None) => {
            let (_, geo) = core.body(&body)?;
            let idx = select_faces(&geo.shape, &sel)?;
            exactly_one(idx.len(), &sel)?;
            let f = ducad_kernel::enumerate_faces(&geo.shape).swap_remove(idx[0]);
            let planar = f.kind == SurfaceKind::Plane;
            Ok(Ref {
                point: f.centroid,
                dir: Some(f.normal),
                plane: planar.then_some((f.centroid, f.normal)),
            })
        }
        (None, Some(body), None, Some(sel)) => {
            let (_, geo) = core.body(&body)?;
            let idx = select_edges(&geo.shape, &sel)?;
            exactly_one(idx.len(), &sel)?;
            let e = ducad_kernel::enumerate_edges(&geo.shape).swap_remove(idx[0]);
            Ok(Ref {
                point: e.mid,
                dir: e.dir,
                plane: None,
            })
        }
        _ => Err(OpError::invalid(
            "a reference must be {point}, {body, face} or {body, edge}",
        )),
    }
}

pub fn measure(a: &Ref, b: &Ref) -> Value {
    let sub = |p: [f64; 3], q: [f64; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    let dot = |p: [f64; 3], q: [f64; 3]| p[0] * q[0] + p[1] * q[1] + p[2] * q[2];
    let d = sub(b.point, a.point);
    let r4 = crate::inspect::round4;
    let mut out = json!({ "distance": r4(dot(d, d).sqrt()) });
    if let (Some(u), Some(v)) = (a.dir, b.dir) {
        let c = (dot(u, v) / (dot(u, u).sqrt() * dot(v, v).sqrt())).clamp(-1.0, 1.0);
        out["angle_deg"] = json!(r4(c.acos().to_degrees()));
    }
    if let (Some((pa, na)), Some((pb, nb))) = (a.plane, b.plane) {
        let parallel = dot(na, nb).abs() >= crate::select::ANG_TOL_DEG.to_radians().cos();
        if parallel {
            out["plane_gap"] = json!(r4(dot(sub(pb, pa), na).abs()));
        }
    }
    out
}

// ---------------------------------------------------------------------
// Pemangkasan payload besar.
// ---------------------------------------------------------------------

/// JSON kompak; bila > [`MAX_TEXT_BYTES`], larik terpanjang dipangkas
/// separuh berulang dan `truncated: true` + cara mempersempit ditambahkan.
pub fn compact_text(mut payload: Value) -> String {
    let mut text = payload.to_string();
    let mut rounds = 0;
    while text.len() > MAX_TEXT_BYTES && rounds < 64 && halve_longest_array(&mut payload) {
        rounds += 1;
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("truncated".into(), json!(true));
            obj.insert(
                "truncation_hint".into(),
                json!(
                    "result truncated: narrow it with 'body', 'limit', or a more specific selector"
                ),
            );
        }
        text = payload.to_string();
    }
    text
}

/// Langkah jalur JSON: kunci objek atau indeks larik.
#[derive(Clone)]
enum Step {
    Key(String),
    Index(usize),
}

/// Cari jalur ke larik terpanjang.
fn longest_array_path(v: &Value, path: &mut Vec<Step>, best: &mut Option<(usize, Vec<Step>)>) {
    match v {
        Value::Array(a) => {
            if best.as_ref().is_none_or(|(n, _)| a.len() > *n) {
                *best = Some((a.len(), path.clone()));
            }
            for (i, x) in a.iter().enumerate() {
                path.push(Step::Index(i));
                longest_array_path(x, path, best);
                path.pop();
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                path.push(Step::Key(k.clone()));
                longest_array_path(x, path, best);
                path.pop();
            }
        }
        _ => {}
    }
}

fn halve_longest_array(v: &mut Value) -> bool {
    let mut best = None;
    longest_array_path(v, &mut Vec::new(), &mut best);
    let Some((n, path)) = best else {
        return false;
    };
    if n <= 1 {
        return false;
    }
    let mut cur = v;
    for step in &path {
        let next = match (step, cur) {
            (Step::Key(k), Value::Object(o)) => o.get_mut(k),
            (Step::Index(i), Value::Array(a)) => a.get_mut(*i),
            _ => None,
        };
        match next {
            Some(x) => cur = x,
            None => return false,
        }
    }
    match cur {
        Value::Array(a) => {
            a.truncate(n / 2);
            true
        }
        _ => false,
    }
}

/// Jalankan tool tingkat-core di balik pagar panik (`guard`): panik di
/// tool mana pun menjadi error `internal`, bukan proses yang mati. Nama di
/// luar [`CORE_TOOLS`] → `InvalidParam`, supaya pemanggil bisa
/// menanganinya sendiri lebih dulu.
pub fn call_core_tool(
    core: &mut SessionCore,
    name: &str,
    a: Value,
    paths: &dyn ToolPaths,
) -> OpResult<ToolOut> {
    crate::guard::catch_op_panic(&format!("tool {name}"), || {
        call_core_tool_inner(core, name, a, paths)
    })
}

/// Tool tanpa state sesi (`get_schema`), di balik pagar panik yang sama.
pub fn call_stateless_tool(name: &str, a: Value) -> OpResult<ToolOut> {
    crate::guard::catch_op_panic(&format!("tool {name}"), || {
        call_stateless_tool_inner(name, a)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_text_is_truncated() {
        let big = json!({ "items": (0..20000).map(|i| json!({ "i": i, "pad": "xxxxxxxxxx" })).collect::<Vec<_>>() });
        let t = compact_text(big);
        assert!(t.len() <= MAX_TEXT_BYTES);
        let v: Value = serde_json::from_str(&t).unwrap();
        assert_eq!(v["truncated"], true);
    }

    #[test]
    fn core_tools_run_on_borrowed_state() {
        let mut session = crate::Session::new();
        let plate: Value = serde_json::from_str(EXAMPLE_PLATE).unwrap();
        session
            .set_params(serde_json::from_value(plate["params"].clone()).unwrap())
            .unwrap();
        let out = call_core_tool(
            &mut session.core(),
            "run_ops",
            json!({ "ops": plate["ops"] }),
            &NoPaths,
        )
        .unwrap();
        assert!(!out.is_error, "{}", out.payload);

        let out = call_core_tool(&mut session.core(), "inspect", json!({}), &NoPaths).unwrap();
        assert_eq!(out.payload["bodies"].as_array().unwrap().len(), 1);

        let out = call_core_tool(
            &mut session.core(),
            "query_geometry",
            json!({ "body": "plate", "edges": "|Z" }),
            &NoPaths,
        )
        .unwrap();
        // Tepi sejajar Z pada plate: 4 sudut ter-fillet + jahitan silinder
        // lubang; yang diuji di sini selector jalan di atas state pinjaman.
        assert!(out.payload["count"].as_u64().unwrap() >= 4);

        // Path ditolak di konteks tanpa akses berkas.
        let err = call_core_tool(
            &mut session.core(),
            "render_view",
            json!({ "save_svg": "x.svg" }),
            &NoPaths,
        )
        .unwrap_err();
        assert_eq!(err.code, OpErrorCode::Io);
    }

    #[test]
    fn get_schema_is_tiered_and_cheap_by_default() {
        let overview = call_stateless_tool("get_schema", json!({}))
            .unwrap()
            .payload;
        let size = overview.to_string().len();
        assert!(size < 16 * 1024, "ringkasan harus murah: {size} byte");
        let ops = overview["ops"].as_array().unwrap();
        assert_eq!(ops.len(), op_kinds().len());
        for o in ops {
            assert!(
                !o["summary"].as_str().unwrap_or_default().is_empty(),
                "op tanpa deskripsi: {}",
                o["op"]
            );
        }
        let fillet = ops.iter().find(|o| o["op"] == "fillet").unwrap();
        assert!(fillet["required"]
            .as_array()
            .unwrap()
            .contains(&json!("radius")));
        assert!(overview["selector_cheatsheet"]
            .as_str()
            .unwrap()
            .contains("of(>Z)"));

        let d = call_stateless_tool("get_schema", json!({ "op": "hole" }))
            .unwrap()
            .payload;
        assert!(d["schema"]["properties"]["spec"].is_object());
        assert!(
            d["definitions"]["HoleSpecRef"].is_object(),
            "definisi yang dirujuk ikut"
        );
        assert!(
            d["definitions"].get("EntitySpec").is_none(),
            "definisi lain tidak ikut"
        );
        assert!(d["examples"].as_array().unwrap().contains(&json!("plate")));

        let e = call_stateless_tool("get_schema", json!({ "example": "flange" }))
            .unwrap()
            .payload;
        assert!(e["op_file"]["ops"].is_array());
        let err = call_stateless_tool("get_schema", json!({ "op": "filet" })).unwrap_err();
        assert_eq!(err.code, OpErrorCode::UnknownRef);
        assert!(err.message.contains("fillet"));

        let full = call_stateless_tool("get_schema", json!({ "full": true }))
            .unwrap()
            .payload;
        assert!(full["op_schema"]["definitions"]["Op"].is_object());
    }

    #[test]
    fn parse_ops_reports_index_and_id_of_bad_op() {
        let err = parse_ops(vec![
            json!({"op":"primitive","id":"b","shape":{"sphere":{"r":1}}}),
            json!({"op":"fillet","id":"f","body":"b","edges":"|Z","radious":2}),
        ])
        .unwrap_err();
        assert_eq!(err.op_index, Some(1));
        assert_eq!(err.op_id.as_deref(), Some("f"));
        assert!(err.message.contains("radious"), "{}", err.message);
        assert!(err.hint.unwrap_or_default().contains("\"op\":\"fillet\""));
    }

    #[test]
    fn set_checks_is_a_core_tool() {
        let mut session = crate::Session::new();
        let out = call_core_tool(
            &mut session.core(),
            "set_checks",
            json!({ "checks": [{"check": "body_count", "expect": 0}] }),
            &NoPaths,
        )
        .unwrap();
        assert_eq!(out.payload["pass"], 1, "{}", out.payload);
        assert_eq!(session.design().checks.len(), 1);
    }

    #[test]
    fn edit_args_validate_shape_per_tool() {
        let e = parse_edit_args(
            "replace_op",
            json!({"id":"f1","op":{"op":"fillet","id":"f1","body":"p","edges":"|Z","radius":1}}),
        )
        .unwrap();
        assert_eq!(e.replace.len(), 1);
        assert!(parse_edit_args("replace_op", json!({"id":"f1"})).is_err());
        assert!(parse_edit_args("remove_op", json!({"ids":[]})).is_err());
        assert!(parse_edit_args("propose_ops", json!({})).is_err());
        let e = parse_edit_args("propose_ops", json!({"remove":["h1"],"params":{"t":9}})).unwrap();
        assert!(!e.is_append_only());
        let err = parse_edit_args(
            "replace_op",
            json!({"id":"f1","op":{"op":"fillet","id":"f1","body":"p","edges":"|Z"}}),
        )
        .unwrap_err();
        assert!(err.message.contains("'f1'"), "{}", err.message);
    }
}

