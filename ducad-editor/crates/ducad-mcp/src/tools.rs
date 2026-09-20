//! Definisi dan implementasi 15 tool MCP DUCAD (P3.2).

use base64::Engine as _;
use ducad_engine::check::CheckItem;
use ducad_engine::export::{export, ExportFormat};
use ducad_engine::inspect::summarize;
use ducad_engine::ops::{op_schema, Op, Params};
use ducad_engine::render::{svg_to_png, View};
use ducad_engine::tooling::{
    args, call_core_tool, call_stateless_tool, compact_text, to_value, SessionArg, ToolOut,
    CORE_TOOLS, STATELESS_TOOLS,
};
use ducad_engine::{OpError, OpResult, Session};
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

/// Jalankan tool; error menjadi `isError: true` dengan payload `{"error": OpError}`.
pub fn call(server: &mut Server, name: &str, arguments: Value) -> Value {
    if let Some(attach) = server.attach.as_mut() {
        return tool_result(attach.call(name, arguments));
    }
    tool_result(call_inner(server, name, arguments).unwrap_or_else(ToolOut::err))
}

/// Bungkus [`ToolOut`] menjadi hasil `tools/call` MCP (teks + gambar).
/// Dipakai juga mode `--attach` untuk hasil yang datang dari jembatan.
pub fn tool_result(out: ToolOut) -> Value {
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

fn summary_json(s: &Session) -> OpResult<Value> {
    to_value(summarize(
        s,
        None,
        false,
        ducad_engine::inspect::DEFAULT_TOPOLOGY_LIMIT,
    )?)
}

fn call_inner(server: &mut Server, name: &str, a: Value) -> OpResult<ToolOut> {
    // Tool yang hanya butuh state sesi dijalankan lewat implementasi
    // bersama di `ducad_engine::tooling` — sama persis dengan yang dipakai
    // jembatan live di GUI (P5).
    if STATELESS_TOOLS.contains(&name) {
        return call_stateless_tool(name, a);
    }
    if CORE_TOOLS.contains(&name) {
        #[derive(Deserialize, Default)]
        struct Pick {
            #[serde(default)]
            session: Option<String>,
        }
        let pick: Pick = serde_json::from_value(a.clone()).unwrap_or_default();
        let paths = server.paths();
        let (_, part) = server.pick(pick.session.as_deref())?;
        return call_core_tool(&mut part.session.core(), name, a, &paths);
    }
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
        "set_checks" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                checks: Vec<CheckItem>,
            }
            let a: A = args(a)?;
            let (_, part) = server.pick(a.session.as_deref())?;
            part.session.set_checks(a.checks);
            Ok(ToolOut::ok(to_value(part.session.run_checks(None))?))
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
