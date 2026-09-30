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
use crate::ops::{op_schema, Op, EXAMPLE_PLATE};
use crate::render::{render_svg_core, RenderOptions, View};
use crate::select::{select_edges, select_faces, SELECTOR_CHEATSHEET};
use crate::session::SessionCore;
use ducad_kernel::SurfaceKind;

/// Tool yang bisa dijalankan hanya dengan [`SessionCore`].
pub const CORE_TOOLS: &[&str] = &[
    "run_ops",
    "inspect",
    "query_geometry",
    "measure",
    "render_view",
    "get_oplog",
    "get_schema",
    "run_checks",
    "drawing",
    "import_step",
];

/// Tool tingkat-core yang MENGUBAH model (pemanggil GUI menyinkronkan
/// `design` dan mencatat aktivitas setelahnya).
pub const MUTATING_CORE_TOOLS: &[&str] = &["run_ops", "import_step"];

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
            "menulis berkas tidak tersedia di konteks ini",
        ))
    }
}

pub fn args<T: for<'de> Deserialize<'de>>(v: Value) -> OpResult<T> {
    serde_json::from_value(v)
        .map_err(|e| OpError::invalid(format!("argumen tool tidak valid: {e}")))
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
                ops: Vec<Op>,
                #[serde(default)]
                dry_run: bool,
            }
            let a: A = args(a)?;
            let _ = a.session;
            let report = core.run(a.ops, a.dry_run);
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
                _ => return Err(OpError::invalid("isi tepat satu dari 'faces' atau 'edges'")),
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
            }
            let a: A = args(a)?;
            let _ = a.session;
            let svg_path = a
                .save_svg
                .as_deref()
                .map(|p| paths.resolve(p))
                .transpose()?;
            let (w, h) = (a.width.unwrap_or(800), a.height.unwrap_or(600));
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
                        format!("gagal menulis {}: {e}", p.display()),
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
        "get_oplog" => {
            let a: SessionArg = args(a)?;
            let _ = a.session;
            let d = &core.meta.design;
            Ok(ToolOut::ok(json!({ "params": d.params, "ops": d.oplog })))
        }
        "get_schema" => call_stateless_tool(name, a),
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
            "tool '{other}' bukan tool tingkat-core"
        ))),
    }
}

/// `drawing`: gambar kerja 4 tampak + dimensi otomatis → PDF/SVG/DXF.
fn drawing_tool(core: &mut SessionCore, a: Value, paths: &dyn ToolPaths) -> OpResult<ToolOut> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct A {
        #[serde(default)]
        session: Option<String>,
        format: String,
        path: String,
        #[serde(default)]
        paper: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        part_number: Option<String>,
        #[serde(default)]
        material: Option<String>,
        #[serde(default)]
        notes: Vec<String>,
    }
    let a: A = args(a)?;
    let _ = a.session;
    let paper = match a
        .paper
        .as_deref()
        .unwrap_or("a3")
        .to_ascii_lowercase()
        .as_str()
    {
        "a4" => ducad_io::drawing::PaperSize::A4Landscape,
        "a4-portrait" => ducad_io::drawing::PaperSize::A4Portrait,
        "a3" => ducad_io::drawing::PaperSize::A3Landscape,
        "a3-portrait" => ducad_io::drawing::PaperSize::A3Portrait,
        other => {
            return Err(OpError::invalid(format!(
                "kertas '{other}' tidak dikenal (a4, a4-portrait, a3, a3-portrait)"
            )))
        }
    };
    let path = paths.resolve(&a.path)?;
    let mut notes = crate::drawing_auto::hole_notes(&core.meta.design, &core.meta.design.params);
    notes.extend(a.notes);
    let info = crate::drawing_auto::TitleInfo {
        title: a.title.unwrap_or_default(),
        part_number: a.part_number.unwrap_or_default(),
        material: a.material.unwrap_or_default(),
        ..Default::default()
    };
    let sheet = crate::drawing_auto::auto_sheet(core, paper, &info, &notes)?;
    let io = |e: anyhow::Error| {
        OpError::new(
            OpErrorCode::Io,
            format!("gagal menulis {}: {e:#}", path.display()),
        )
    };
    match a.format.to_ascii_lowercase().as_str() {
        "pdf" => ducad_io::pdf::export_pdf(&sheet, &path).map_err(io)?,
        "svg" => ducad_io::svg::export_drawing_sheet_svg(&sheet, &path).map_err(io)?,
        "dxf" => ducad_io::dxf::export_drawing_sheet(&sheet, &path).map_err(io)?,
        other => {
            return Err(OpError::invalid(format!(
                "format gambar '{other}' tidak dikenal (pdf, svg, dxf)"
            )))
        }
    }
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(ToolOut::ok(
        json!({ "path": path, "bytes": bytes, "notes": notes }),
    ))
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
            "nama body '{}' tidak valid: harus cocok ^[a-z][a-z0-9_]{{0,31}}$",
            a.name
        )));
    }
    let taken = core.model.doc.bodies.values().any(|b| b.name == a.name)
        || core.meta.design.oplog.iter().any(|o| o.id() == a.name);
    if taken {
        return Err(OpError::new(
            OpErrorCode::DuplicateId,
            format!("nama '{}' sudah dipakai", a.name),
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
            struct A {}
            let _: A = args(a)?;
            let example: Value = serde_json::from_str(EXAMPLE_PLATE)
                .map_err(|e| OpError::new(OpErrorCode::Io, e.to_string()))?;
            Ok(ToolOut::ok(json!({
                "op_schema": op_schema(),
                "selector_cheatsheet": SELECTOR_CHEATSHEET,
                "example": example,
            })))
        }
        other => Err(OpError::invalid(format!(
            "tool '{other}' bukan tool tanpa state"
        ))),
    }
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
                "selector \"{sel}\" harus menghasilkan tepat 1 elemen (cocok {n})"
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
            "rujukan harus {point} atau {body, face} atau {body, edge}",
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
                json!("hasil dipotong: persempit dengan argumen 'body', 'limit', atau selector yang lebih spesifik"),
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
}
