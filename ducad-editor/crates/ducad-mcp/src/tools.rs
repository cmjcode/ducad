//! Definisi dan implementasi 15 tool MCP DUCAD (P3.2).

use base64::Engine as _;
use ducad_engine::check::CheckItem;
use ducad_engine::export::{export, ExportFormat};
use ducad_engine::inspect::{summarize, DEFAULT_TOPOLOGY_LIMIT};
use ducad_engine::ops::{op_schema, Op, Params, EXAMPLE_PLATE};
use ducad_engine::render::{render_svg, svg_to_png, RenderOptions, View};
use ducad_engine::select::{select_edges, select_faces, SELECTOR_CHEATSHEET};
use ducad_engine::{OpError, OpErrorCode, OpResult, Session};
use ducad_kernel::SurfaceKind;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::server::{Part, Server};

pub const TOOL_NAMES: &[&str] = &[
    "new_part",
    "open_part",
    "save_part",
    "close_part",
    "run_ops",
    "set_params",
    "inspect",
    "query_geometry",
    "measure",
    "render_view",
    "get_oplog",
    "undo",
    "redo",
    "export",
    "get_schema",
    "set_checks",
    "run_checks",
    "propose_ops",
    "accept_proposal",
    "reject_proposal",
];

/// Batas teks hasil tool sebelum daftar terpanjang dipotong.
pub const MAX_TEXT_BYTES: usize = 60 * 1024;
const MAX_QUERY_ITEMS: usize = 50;

fn session_prop() -> Value {
    json!({ "type": "string", "description": "Id sesi (mis. \"s1\"); boleh kosong bila hanya ada satu part terbuka." })
}

fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

fn tool(name: &str, description: &str, input: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input })
}

fn ref_schema() -> Value {
    json!({
        "description": "Rujukan: {\"point\":[x,y,z]}, {\"body\":B,\"face\":SEL}, atau {\"body\":B,\"edge\":SEL}; selector harus menghasilkan tepat 1 elemen.",
        "oneOf": [
            schema(json!({ "point": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 } }), &["point"]),
            schema(json!({ "body": { "type": "string" }, "face": { "type": "string" } }), &["body", "face"]),
            schema(json!({ "body": { "type": "string" }, "edge": { "type": "string" } }), &["body", "edge"]),
        ]
    })
}

/// Daftar tool untuk `tools/list`.
pub fn definitions() -> Vec<Value> {
    let op_schema = op_schema();
    let definitions = op_schema.get("definitions").cloned().unwrap_or(json!({}));
    let mut run_ops = schema(
        json!({
            "session": session_prop(),
            "ops": { "type": "array", "items": { "$ref": "#/definitions/Op" }, "description": "Daftar Op (lihat get_schema)." },
            "dry_run": { "type": "boolean", "description": "Validasi tanpa mengubah part." }
        }),
        &["ops"],
    );
    let mut propose_ops = schema(
        json!({
            "session": session_prop(),
            "ops": { "type": "array", "items": { "$ref": "#/definitions/Op" } }
        }),
        &["ops"],
    );
    propose_ops["definitions"] = definitions.clone();
    run_ops["definitions"] = definitions;
    let check_defs =
        serde_json::to_value(schemars::schema_for!(Vec<CheckItem>)).unwrap_or(json!({}));
    let check_items = check_defs
        .get("items")
        .cloned()
        .unwrap_or(json!({ "type": "object" }));
    let with_check_defs = |mut v: Value| {
        v["definitions"] = check_defs.get("definitions").cloned().unwrap_or(json!({}));
        v
    };
    let set_checks = with_check_defs(schema(
        json!({ "session": session_prop(), "checks": { "type": "array", "items": check_items.clone() } }),
        &["checks"],
    ));
    let run_checks = with_check_defs(schema(
        json!({ "session": session_prop(), "checks": { "type": "array", "items": check_items } }),
        &[],
    ));
    let view = json!({ "type": "string", "enum": ["iso", "front", "back", "left", "right", "top", "bottom"] });
    vec![
        tool(
            "new_part",
            "Buat part kosong baru. Panggil sebelum run_ops bila belum ada part terbuka.",
            schema(json!({ "name": { "type": "string" } }), &[]),
        ),
        tool(
            "open_part",
            "Buka berkas .ducad (relatif ke root). Mode adopsi/oplog basi dilaporkan di summary.warnings.",
            schema(json!({ "path": { "type": "string" } }), &["path"]),
        ),
        tool(
            "save_part",
            "Simpan part ke .ducad; tanpa path memakai path asal.",
            schema(json!({ "session": session_prop(), "path": { "type": "string" } }), &[]),
        ),
        tool("close_part", "Tutup sesi part.", schema(json!({ "session": session_prop() }), &[])),
        tool(
            "run_ops",
            "Jalankan batch Op secara atomik; pakai dry_run:true dulu untuk validasi. Hasil: BatchReport.",
            run_ops,
        ),
        tool(
            "set_params",
            "Ubah param (digabung ke param lama) lalu replay seluruh oplog. Cara yang benar untuk mengubah dimensi.",
            schema(
                json!({ "session": session_prop(), "params": { "type": "object", "additionalProperties": { "type": "number" } } }),
                &["params"],
            ),
        ),
        tool(
            "inspect",
            "Ringkasan part: volume, bbox, jumlah face/tepi per body, sketch, params. topology:true untuk daftar face/tepi.",
            schema(
                json!({ "session": session_prop(), "body": { "type": "string" }, "topology": { "type": "boolean" },
                        "limit": { "type": "integer", "minimum": 1 } }),
                &[],
            ),
        ),
        tool(
            "query_geometry",
            "Uji selector face atau tepi pada satu body sebelum dipakai di fillet/hole/sketch.",
            schema(
                json!({ "session": session_prop(), "body": { "type": "string" }, "faces": { "type": "string" },
                        "edges": { "type": "string" } }),
                &["body"],
            ),
        ),
        tool(
            "measure",
            "Ukur jarak (dan sudut / celah bidang bila berlaku) antara dua rujukan.",
            schema(json!({ "session": session_prop(), "a": ref_schema(), "b": ref_schema() }), &["a", "b"]),
        ),
        tool(
            "render_view",
            "Render tampak part ke PNG (dan opsional SVG) untuk verifikasi visual.",
            schema(
                json!({ "session": session_prop(), "view": view, "hidden_lines": { "type": "boolean" },
                        "width": { "type": "integer", "minimum": 16, "maximum": 4096 },
                        "height": { "type": "integer", "minimum": 16, "maximum": 4096 },
                        "bodies": { "type": "array", "items": { "type": "string" } },
                        "save_svg": { "type": "string" } }),
                &[],
            ),
        ),
        tool(
            "get_oplog",
            "Ambil params dan daftar Op part (sumber kebenaran desain).",
            schema(json!({ "session": session_prop() }), &[]),
        ),
        tool("undo", "Batalkan batch run_ops terakhir.", schema(json!({ "session": session_prop() }), &[])),
        tool("redo", "Ulangi batch yang terakhir di-undo.", schema(json!({ "session": session_prop() }), &[])),
        tool(
            "export",
            "Ekspor body terlihat ke STEP/STL/OBJ/GLB.",
            schema(
                json!({ "session": session_prop(), "format": { "type": "string", "enum": ["step", "stl", "obj", "glb"] },
                        "path": { "type": "string" } }),
                &["format", "path"],
            ),
        ),
        tool(
            "get_schema",
            "Skema Op, cheatsheet selector, dan contoh lengkap. Panggil sekali di awal.",
            schema(json!({}), &[]),
        ),
        tool(
            "set_checks",
            "Ganti seluruh daftar check desain (persyaratan user: volume, bbox_size, hole_count, min_wall, clearance, …) dan evaluasi sekarang. Tulis sebelum memodelkan.",
            set_checks,
        ),
        tool(
            "run_checks",
            "Evaluasi check desain (atau daftar 'checks' yang diberikan) terhadap geometri saat ini.",
            run_checks,
        ),
        tool(
            "propose_ops",
            "Pratinjau batch Op tanpa mengubah part: diff body (+/- volume) dan gambar diff berwarna. Terapkan dengan accept_proposal.",
            propose_ops,
        ),
        tool(
            "accept_proposal",
            "Terapkan proposal dari propose_ops (gagal proposal_stale bila part berubah sejak proposal dibuat).",
            schema(json!({ "session": session_prop(), "proposal_id": { "type": "string" } }), &["proposal_id"]),
        ),
        tool(
            "reject_proposal",
            "Buang proposal yang tidak dipakai.",
            schema(json!({ "session": session_prop(), "proposal_id": { "type": "string" } }), &["proposal_id"]),
        ),
    ]
}

/// Payload hasil tool (+ gambar opsional).
struct ToolOut {
    payload: Value,
    image_png: Option<Vec<u8>>,
    is_error: bool,
}

impl ToolOut {
    fn ok(payload: Value) -> Self {
        Self {
            payload,
            image_png: None,
            is_error: false,
        }
    }
}

fn args<T: for<'de> Deserialize<'de>>(v: Value) -> OpResult<T> {
    serde_json::from_value(v)
        .map_err(|e| OpError::invalid(format!("argumen tool tidak valid: {e}")))
}

/// Jalankan tool; error menjadi `isError: true` dengan payload `{"error": OpError}`.
pub fn call(server: &mut Server, name: &str, arguments: Value) -> Value {
    let out = match call_inner(server, name, arguments) {
        Ok(o) => o,
        Err(e) => ToolOut {
            payload: json!({ "error": e }),
            image_png: None,
            is_error: true,
        },
    };
    let text = compact_text(out.payload);
    let mut content = vec![json!({ "type": "text", "text": text })];
    if let Some(png) = out.image_png {
        content.push(json!({
            "type": "image",
            "data": base64::engine::general_purpose::STANDARD.encode(png),
            "mimeType": "image/png",
        }));
    }
    json!({ "content": content, "isError": out.is_error })
}

/// JSON kompak; bila > 60 KB, larik terpanjang dipangkas separuh berulang
/// dan `truncated: true` + cara mempersempit ditambahkan.
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionArg {
    #[serde(default)]
    session: Option<String>,
}

fn summary_json(s: &Session) -> OpResult<Value> {
    to_value(summarize(s, None, false, DEFAULT_TOPOLOGY_LIMIT)?)
}

fn to_value(v: impl serde::Serialize) -> OpResult<Value> {
    serde_json::to_value(v).map_err(|e| OpError::new(OpErrorCode::Io, e.to_string()))
}

fn call_inner(server: &mut Server, name: &str, a: Value) -> OpResult<ToolOut> {
    match name {
        "new_part" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                name: Option<String>,
            }
            let a: A = args(a)?;
            let session = Session::new();
            let summary = summary_json(&session)?;
            let id = server.insert(Part {
                session,
                path: None,
                name: a.name,
            });
            Ok(ToolOut::ok(json!({ "session": id, "summary": summary })))
        }
        "open_part" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                path: String,
            }
            let a: A = args(a)?;
            let path = server.resolve(&a.path)?;
            let session = Session::from_file(&path)?;
            let summary = summary_json(&session)?;
            let id = server.insert(Part {
                session,
                path: Some(path),
                name: None,
            });
            Ok(ToolOut::ok(json!({ "session": id, "summary": summary })))
        }
        "save_part" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                #[serde(default)]
                path: Option<String>,
            }
            let a: A = args(a)?;
            let target = a.path.as_deref().map(|p| server.resolve(p)).transpose()?;
            let (_, part) = server.pick(a.session.as_deref())?;
            let path = target
                .or_else(|| part.path.clone())
                .ok_or_else(|| OpError::invalid("part belum punya path; isi argumen 'path'"))?;
            part.session.save(&path)?;
            part.path = Some(path.clone());
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            Ok(ToolOut::ok(json!({ "path": path, "bytes": bytes })))
        }
        "close_part" => {
            let a: SessionArg = args(a)?;
            let (id, _) = server.pick(a.session.as_deref())?;
            server.sessions.remove(&id);
            Ok(ToolOut::ok(json!({ "closed": id })))
        }
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
            let (_, part) = server.pick(a.session.as_deref())?;
            let report = part.session.run(a.ops, a.dry_run);
            let is_error = report.error.is_some();
            Ok(ToolOut {
                payload: to_value(report)?,
                image_png: None,
                is_error,
            })
        }
        "set_params" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                params: Params,
            }
            let a: A = args(a)?;
            let (_, part) = server.pick(a.session.as_deref())?;
            let mut params = part.session.design().params.clone();
            params.extend(a.params);
            let report = part.session.set_params(params)?;
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
            let (_, part) = server.pick(a.session.as_deref())?;
            let limit = a.limit.unwrap_or(DEFAULT_TOPOLOGY_LIMIT).max(1);
            let summary = summarize(&part.session, a.body.as_deref(), a.topology, limit)?;
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
            let (_, part) = server.pick(a.session.as_deref())?;
            let (_, geo) = part.session.body(&a.body)?;
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
            let (_, part) = server.pick(a.session.as_deref())?;
            let ra = resolve_ref(&part.session, &a.a)?;
            let rb = resolve_ref(&part.session, &a.b)?;
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
            let svg_path = a
                .save_svg
                .as_deref()
                .map(|p| server.resolve(p))
                .transpose()?;
            let (_, part) = server.pick(a.session.as_deref())?;
            let (w, h) = (a.width.unwrap_or(800), a.height.unwrap_or(600));
            let options = RenderOptions {
                view: a.view,
                width: w,
                height: h,
                hidden_lines: a.hidden_lines,
                bodies: a.bodies,
            };
            let r = render_svg(&part.session, &options)?;
            if let Some(p) = &svg_path {
                std::fs::write(p, &r.svg).map_err(|e| {
                    OpError::new(
                        OpErrorCode::Io,
                        format!("gagal menulis {}: {e}", p.display()),
                    )
                })?;
            }
            let png = svg_to_png(&r.svg, w, h)?;
            Ok(ToolOut {
                payload: json!({
                    "visible_segments": r.visible_segments,
                    "hidden_segments": r.hidden_segments,
                    "svg_path": svg_path,
                }),
                image_png: Some(png),
                is_error: false,
            })
        }
        "get_oplog" => {
            let a: SessionArg = args(a)?;
            let (_, part) = server.pick(a.session.as_deref())?;
            let d = part.session.design();
            Ok(ToolOut::ok(json!({ "params": d.params, "ops": d.oplog })))
        }
        "undo" | "redo" => {
            let a: SessionArg = args(a)?;
            let (_, part) = server.pick(a.session.as_deref())?;
            let changed = if name == "undo" {
                part.session.undo()?
            } else {
                part.session.redo()?
            };
            Ok(ToolOut::ok(
                json!({ "changed": changed, "summary": summary_json(&part.session)? }),
            ))
        }
        "export" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                format: ExportFormat,
                path: String,
            }
            let a: A = args(a)?;
            let path = server.resolve(&a.path)?;
            let (_, part) = server.pick(a.session.as_deref())?;
            let bytes = export(&part.session, a.format, &path)?;
            Ok(ToolOut::ok(json!({ "path": path, "bytes": bytes })))
        }
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
        "set_checks" | "run_checks" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                #[serde(default)]
                checks: Option<Vec<CheckItem>>,
            }
            let a: A = args(a)?;
            let (_, part) = server.pick(a.session.as_deref())?;
            let summary = if name == "set_checks" {
                let checks = a
                    .checks
                    .ok_or_else(|| OpError::invalid("set_checks butuh 'checks'"))?;
                part.session.set_checks(checks);
                part.session.run_checks(None)
            } else {
                part.session.run_checks(a.checks.as_deref())
            };
            Ok(ToolOut::ok(to_value(summary)?))
        }
        "propose_ops" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                ops: Vec<Op>,
            }
            let a: A = args(a)?;
            let (_, part) = server.pick(a.session.as_deref())?;
            let (proposal, shapes) = part.session.propose(a.ops)?;
            let r =
                ducad_engine::render::render_diff_svg(&part.session, &shapes, View::Iso, 800, 600)?;
            let png = svg_to_png(&r.svg, 800, 600)?;
            Ok(ToolOut {
                payload: json!({
                    "proposal_id": proposal.id,
                    "report": proposal.report,
                    "diff": proposal.diff,
                }),
                image_png: Some(png),
                is_error: false,
            })
        }
        "accept_proposal" | "reject_proposal" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                proposal_id: String,
            }
            let a: A = args(a)?;
            let (_, part) = server.pick(a.session.as_deref())?;
            if name == "reject_proposal" {
                return Ok(ToolOut::ok(
                    json!({ "rejected": part.session.reject(&a.proposal_id) }),
                ));
            }
            let report = part.session.accept(&a.proposal_id);
            let is_error = report.error.is_some();
            Ok(ToolOut {
                payload: to_value(report)?,
                image_png: None,
                is_error,
            })
        }
        other => Err(OpError::invalid(format!("tool tidak dikenal: {other}"))),
    }
}

/// Rujukan terukur: titik wakil + arah opsional + info bidang.
struct Ref {
    point: [f64; 3],
    dir: Option<[f64; 3]>,
    /// (titik, normal) bila rujukan adalah face planar.
    plane: Option<([f64; 3], [f64; 3])>,
}

fn resolve_ref(s: &Session, v: &Value) -> OpResult<Ref> {
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
            let (_, geo) = s.body(&body)?;
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
            let (_, geo) = s.body(&body)?;
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

fn measure(a: &Ref, b: &Ref) -> Value {
    let sub = |p: [f64; 3], q: [f64; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    let dot = |p: [f64; 3], q: [f64; 3]| p[0] * q[0] + p[1] * q[1] + p[2] * q[2];
    let d = sub(b.point, a.point);
    let r4 = ducad_engine::inspect::round4;
    let mut out = json!({ "distance": r4(dot(d, d).sqrt()) });
    if let (Some(u), Some(v)) = (a.dir, b.dir) {
        let c = (dot(u, v) / (dot(u, u).sqrt() * dot(v, v).sqrt())).clamp(-1.0, 1.0);
        out["angle_deg"] = json!(r4(c.acos().to_degrees()));
    }
    if let (Some((pa, na)), Some((pb, nb))) = (a.plane, b.plane) {
        let parallel = dot(na, nb).abs() >= ducad_engine::select::ANG_TOL_DEG.to_radians().cos();
        if parallel {
            out["plane_gap"] = json!(r4(dot(sub(pb, pa), na).abs()));
        }
    }
    out
}
