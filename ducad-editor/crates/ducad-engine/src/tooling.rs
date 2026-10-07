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
];

/// Tool tingkat-core yang MENGUBAH model (pemanggil GUI menyinkronkan
/// `design` dan mencatat aktivitas setelahnya).
pub const MUTATING_CORE_TOOLS: &[&str] = &["run_ops", "import_step"];

/// Tool yang tidak mengubah part maupun berkas — aman dipanggil kapan saja
/// (anotasi MCP `readOnlyHint`).
pub const READ_ONLY_TOOLS: &[&str] = &[
    "inspect",
    "query_geometry",
    "measure",
    "get_oplog",
    "get_schema",
    "run_checks",
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
pub fn call_core_tool(
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
                checks: Vec<CheckItem>,
            }
            let a: A = args(a)?;
            let _ = a.session;
            core.meta.design.checks = a.checks.clone();
            let results = crate::check::run_checks(core, &a.checks);
            Ok(ToolOut::ok(to_value(
                crate::check::CheckSummary::from_results(results),
            )?))
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
            Ok(ToolOut::ok(to_value(
                crate::check::CheckSummary::from_results(results),
            )?))
        }
        "drawing" => drawing_tool(core, a, paths),
        "import_step" => import_step_tool(core, a, paths),
        other => Err(OpError::invalid(format!(
            "tool '{other}' is not a core tool"
        ))),
    }
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

/// Tool tanpa state: dijawab tanpa sesi terbuka.
pub fn call_stateless_tool(name: &str, a: Value) -> OpResult<ToolOut> {
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
            }
            let a: A = args(a)?;
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
        "more": "get_schema {\"op\":\"fillet\"} = full schema of one op kind; {\"example\":\"flange\"} = tested example OpFile; {\"full\":true} = entire JSON Schema (large)",
    }))
}

/// Tabel kode error untuk agent (markdown, bahasa Inggris seperti seluruh
/// teks MCP). Satu sumber untuk resource `ducad://guide`.
pub const ERROR_GUIDE: &str = "\
| error.code | Meaning | Action |
|---|---|---|
| invalid_param | value out of domain / wrong tool argument or op field | read message (names op_index + valid fields) and fix it |
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
