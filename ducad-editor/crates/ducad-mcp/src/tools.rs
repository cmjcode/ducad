//! Definisi dan implementasi tool MCP DUCAD (P3.2, P15.1).

use base64::Engine as _;
use ducad_engine::export::{export, ExportFormat};
use ducad_engine::inspect::summarize;
use ducad_engine::ops::{Params, EXAMPLES};
use ducad_engine::render::{svg_to_png, View};
use ducad_engine::tooling::{
    args, call_core_tool, call_stateless_tool, compact_text, op_kinds, parse_edit_args, to_value,
    SessionArg, ToolOut, CORE_TOOLS, READ_ONLY_TOOLS, STATELESS_TOOLS,
};
use ducad_engine::{OpError, OpErrorCode, OpResult, Session};
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
    "replace_op",
    "remove_op",
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
    "drawing",
    "import_step",
    "diff",
    "list_parts",
    "simulate_static",
    "set_brief",
    "get_brief",
    "build_package",
    "package_status",
    "submit_for_review",
    "approve_package",
    "list_redlines",
    "add_redline",
    "resolve_redline",
    "import_pdf",
];

/// Tool yang bisa menimpa/membuang sesuatu (anotasi MCP `destructiveHint`).
const DESTRUCTIVE_TOOLS: &[&str] = &[
    "save_part",
    "close_part",
    "replace_op",
    "remove_op",
    "export",
    "set_checks",
    "drawing",
    "set_brief",
    "build_package",
    "approve_package",
];

/// Tool yang aman diulang dengan argumen sama (anotasi `idempotentHint`).
const IDEMPOTENT_TOOLS: &[&str] = &[
    "save_part",
    "set_params",
    "export",
    "set_checks",
    "drawing",
    "render_view",
    "set_view",
    "select",
    "simulate_static",
    "set_brief",
    "build_package",
];

fn session_prop() -> Value {
    json!({ "type": "string", "description": "Session id (e.g. \"s1\"); may be omitted when only one part is open." })
}

fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

/// Definisi tool MCP lengkap dengan `title` dan `annotations`
/// (readOnly/destructive/idempotent) supaya klien bisa mengizinkan tool
/// baca-saja tanpa bertanya.
fn tool(name: &str, title: &str, description: &str, input: Value) -> Value {
    let read_only = READ_ONLY_TOOLS.contains(&name);
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": input,
        "annotations": {
            "title": title,
            "readOnlyHint": read_only,
            "destructiveHint": !read_only && DESTRUCTIVE_TOOLS.contains(&name),
            "idempotentHint": read_only || IDEMPOTENT_TOOLS.contains(&name),
            "openWorldHint": false,
        },
    })
}

/// Skema satu Op yang RINGKAS: hanya `op` (enum jenis) dan `id`. Skema
/// penuh (±33 KB) sengaja tidak ditanam di sini — agent mengambil detail
/// per jenis lewat `get_schema {"op": …}`, dan engine memvalidasi setiap op
/// dengan error yang menyebut `op_index` serta field yang sah.
fn op_item() -> Value {
    json!({
        "type": "object",
        "description": "One Op: {\"op\":<kind>,\"id\":<name>, …fields of that kind}. Fields per kind: get_schema {\"op\":\"<kind>\"}.",
        "properties": {
            "op": { "type": "string", "enum": op_kinds() },
            "id": { "type": "string", "pattern": "^[a-z][a-z0-9_]{0,31}$" }
        },
        "required": ["op", "id"],
        "additionalProperties": true
    })
}

fn ops_prop(description: &str) -> Value {
    json!({ "type": "array", "items": op_item(), "description": description })
}

fn params_prop() -> Value {
    json!({ "type": "object", "additionalProperties": { "type": "number" },
            "description": "Params to change/add (merged into the existing params), e.g. {\"t\": 10}." })
}

fn ref_schema() -> Value {
    json!({
        "description": "Reference: {\"point\":[x,y,z]}, {\"body\":B,\"face\":SEL}, or {\"body\":B,\"edge\":SEL}; the selector must match exactly 1 element.",
        "oneOf": [
            schema(json!({ "point": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 } }), &["point"]),
            schema(json!({ "body": { "type": "string" }, "face": { "type": "string" } }), &["body", "face"]),
            schema(json!({ "body": { "type": "string" }, "edge": { "type": "string" } }), &["body", "edge"]),
        ]
    })
}

const BATCH_RESULT: &str = "Returns a BatchReport: committed, outcomes[] (created/modified/removed/warnings per op; a `boolean_retry:` warning means the kernel had to nudge or scale a tool to finish a boolean, so the result may differ slightly from the exact one: verify it with inspect/run_checks), summary (volume/bbox per body), checks. On failure → isError with error{code, message, hint, op_index, op_id, context, fixes[]}; when fixes is present, resend with patched_op.";

/// Daftar tool untuk `tools/list`.
pub fn definitions() -> Vec<Value> {
    // Skema check di `tools/list` RINGKAS (hanya `check` + field bersama),
    // sama seperti skema op: detail per jenis lewat
    // `get_schema {"check": …}`; engine tetap memvalidasi setiap item.
    let check_items = json!({
        "type": "object",
        "description": "One check: {\"check\":<kind>, …fields of that kind}. Kinds and fields: get_schema {\"checks\":true}; one kind in full: get_schema {\"check\":\"<kind>\"}.",
        "properties": {
            "check": { "type": "string", "enum": ducad_engine::tooling::check_kinds() },
            "id": { "type": "string" },
            "note": { "type": "string" },
            "requirement": { "type": "string", "description": "Requirement id from set_brief, e.g. \"R2\"." },
            "citation": { "type": "string", "description": "Standard clause or guideline this check enforces." },
            "severity": { "type": "string", "enum": ["error", "warning"] }
        },
        "required": ["check"],
        "additionalProperties": true
    });
    let set_checks = schema(
        json!({ "session": session_prop(), "checks": { "type": "array", "items": check_items },
                "rule_packs": { "type": "array",
                    "description": "Rule packs expanded into cited checks and appended to `checks`: a pack id (\"dfm-cnc\") or {\"pack\":id,\"body\":name,\"vars\":{…},\"requirement\":\"R3\"}. List packs and variables: get_schema {\"rule_packs\":true}.",
                    "items": { "oneOf": [ { "type": "string" }, { "type": "object" } ] } } }),
        &[],
    );
    // Skema check lengkap hanya dimuat sekali (di `set_checks`) supaya
    // `tools/list` tetap ringkas; engine tetap memvalidasi setiap item.
    let run_checks = schema(
        json!({ "session": session_prop(), "checks": { "type": "array", "items": { "type": "object" },
                "description": "Optional; same item format as `checks` of set_checks. Without it the design checks installed by set_checks are used." } }),
        &[],
    );
    let view = json!({ "type": "string", "enum": ["iso", "front", "back", "left", "right", "top", "bottom"] });
    let example_names: Vec<&str> = EXAMPLES.iter().map(|(n, _, _)| *n).collect();
    vec![
        tool(
            "new_part",
            "New part",
            "Create a new empty part; returns {session, summary}. Call before run_ops when no part is open.",
            schema(json!({ "name": { "type": "string" } }), &[]),
        ),
        tool(
            "open_part",
            "Open part",
            "Open a .ducad file (relative to the root); returns {session, summary}. Adoption mode / stale oplog is reported in summary.warnings.",
            schema(json!({ "path": { "type": "string" } }), &["path"]),
        ),
        tool(
            "save_part",
            "Save part",
            "Save the part to .ducad; without path, saves to its original path.",
            schema(json!({ "session": session_prop(), "path": { "type": "string" } }), &[]),
        ),
        tool("close_part", "Close part", "Close the part session (unsaved changes are lost).", schema(json!({ "session": session_prop() }), &[])),
        tool(
            "run_ops",
            "Run ops",
            &format!("Append a batch of Ops to the end of the oplog atomically (all succeed or nothing changes); use dry_run:true first to validate. {BATCH_RESULT} To CHANGE an existing op use replace_op/remove_op/set_params, do not stack new ops. Vector->3D example: extrude with profile:{{\"names\":[\"logo\"]}}, per_object:true, material:\"from_style\"."),
            schema(
                json!({
                    "session": session_prop(),
                    "ops": ops_prop("List of Ops, executed in order."),
                    "dry_run": { "type": "boolean", "description": "Validate without changing the part." }
                }),
                &["ops"],
            ),
        ),
        tool(
            "set_params",
            "Set params",
            &format!("Change params (merged into the existing params), then replay the whole oplog. The right way to change dimensions written as \"$name\". With `configuration`, the params are stored as overrides of that design variant (created if missing) and the variant becomes active; `configuration` alone just switches variant (\"Default\" = base design). `inspect` lists the variants in `configurations` and reports the effective params. {BATCH_RESULT}"),
            schema(json!({ "session": session_prop(), "params": params_prop(),
                           "configuration": { "type": "string", "description": "Design variant to write to and activate; \"Default\" is the base design." } }), &[]),
        ),
        tool(
            "replace_op",
            "Replace op",
            &format!("Replace ONE existing op in the oplog (same id, same position), then replay the whole oplog; later ops are recomputed. On failure the part is unchanged. Use it to fix an old op (selector, radius, profile, …) without undo. {BATCH_RESULT}"),
            schema(
                json!({
                    "session": session_prop(),
                    "id": { "type": "string", "description": "Id of the op to replace (see get_oplog)." },
                    "op": op_item(),
                    "dry_run": { "type": "boolean", "description": "Test the replay without changing the part; summary = proposed state." }
                }),
                &["id", "op"],
            ),
        ),
        tool(
            "remove_op",
            "Remove ops",
            &format!("Remove one or more ops from the oplog, then replay. An op still referenced by another op → unknown_ref error with a hint (remove the referencing op too, or change its reference with replace_op). {BATCH_RESULT}"),
            schema(
                json!({
                    "session": session_prop(),
                    "ids": { "type": "array", "items": { "type": "string" }, "minItems": 1, "description": "Ids of the ops to remove." },
                    "dry_run": { "type": "boolean", "description": "Test the replay without changing the part." }
                }),
                &["ids"],
            ),
        ),
        tool(
            "inspect",
            "Inspect part",
            "Part summary: volume, mass (mass_g, from the material density), center_of_mass, inertia tensor (inertia_com, principal_moments/axes, g*mm^2), area, bbox/size, centroid, face/edge counts per body, material, sketches (entities, closed regions, DOF), params. topology:true lists faces/edges (index, kind, point, normal).",
            schema(
                json!({ "session": session_prop(), "body": { "type": "string", "description": "Limit to one body." },
                        "topology": { "type": "boolean" },
                        "limit": { "type": "integer", "minimum": 1, "description": "Max topology items per body." } }),
                &[],
            ),
        ),
        tool(
            "query_geometry",
            "Query geometry",
            "Test a face OR edge selector on one body before using it in fillet/hole/sketch; returns {count, indices, items (max 50)}. Empty → selector_empty with context.available.",
            schema(
                json!({ "session": session_prop(), "body": { "type": "string" }, "faces": { "type": "string" },
                        "edges": { "type": "string" } }),
                &["body"],
            ),
        ),
        tool(
            "measure",
            "Measure",
            "Measure the distance between two references; plus angle_deg when both have a direction, and plane_gap when both are parallel planar faces.",
            schema(json!({ "session": session_prop(), "a": ref_schema(), "b": ref_schema() }), &["a", "b"]),
        ),
        tool(
            "render_view",
            "Render view",
            "Render a view of the part as a PNG image (optionally save the SVG) for visual verification. With `overlay`, render a study result as a color map instead (runs the study if its result is missing or stale).",
            schema(
                json!({ "session": session_prop(), "view": view.clone(), "hidden_lines": { "type": "boolean" },
                        "width": { "type": "integer", "minimum": 16, "maximum": 4096 },
                        "height": { "type": "integer", "minimum": 16, "maximum": 4096 },
                        "bodies": { "type": "array", "items": { "type": "string" } },
                        "save_svg": { "type": "string" },
                        "overlay": { "type": "string", "enum": ["stress", "displacement", "safety_factor"],
                                     "description": "Color the body by a study result: von Mises stress (MPa), displacement magnitude (mm) or safety factor." },
                        "study": { "type": "string", "description": "Id of the study op to show; may be omitted when the design has exactly one study. Needs `overlay`." },
                        "deform_scale": { "type": "number", "minimum": 0,
                                          "description": "Multiplier for the drawn deformation; 0 = undeformed, omitted = automatic (largest displacement drawn as 5 % of the model size). Needs `overlay`." } }),
                &[],
            ),
        ),
        tool(
            "simulate_static",
            "Simulate study",
            "Run a simulation study. Kinds `static` (default) and `thermal_stress` return max von Mises stress (MPa) + location, max displacement (mm), safety factor, reactions per fixture and a color-map PNG; `frequency` returns `frequencies_hz`, `buckling` returns `load_factors` (critical load = factor x applied load), `thermal` returns max/min temperature (no image for these three). Pass `study` (id of a stored `study` op; cached, feeds the study checks) or an inline `setup` (+ `kind`, `thermal`, `modes`) for a what-if run. The body needs op set_material. Results are engineering estimates (about +/-10 % on the default hex mesh; `mesh.kind: \"tet\"` follows curved faces but frequency runs above ~20,000 tets take minutes). Fields: get_schema {\"op\":\"study\"}.",
            schema(
                json!({ "session": session_prop(),
                        "study": { "type": "string", "description": "Id of a study op in the design; may be omitted when there is exactly one." },
                        "setup": { "type": "object", "description": "Inline study setup {body, fixtures:[{id,faces,kind}], loads:[{id,faces,kind,…}], mesh?:{kind?,cell_mm?|target_elems?}}; same shape as the `setup` of a study op." },
                        "kind": { "type": "string", "enum": ["static", "frequency", "buckling", "thermal", "thermal_stress"], "description": "Study kind for an inline `setup`; default static." },
                        "thermal": { "type": "object", "description": "Inline thermal boundaries {boundary:[{id,faces,kind,…}]}; same shape as `thermal` of a study op." },
                        "modes": { "type": "integer", "minimum": 1, "maximum": 40, "description": "Mode count for inline frequency (default 10) / buckling (default 3)." },
                        "overlay": { "type": "string", "enum": ["stress", "displacement", "safety_factor"], "description": "Quantity shown in the returned image; default stress." },
                        "view": view,
                        "deform_scale": { "type": "number", "minimum": 0, "description": "Multiplier for the drawn deformation; 0 = undeformed, omitted = automatic." },
                        "width": { "type": "integer", "minimum": 16, "maximum": 4096 },
                        "height": { "type": "integer", "minimum": 16, "maximum": 4096 } }),
                &[],
            ),
        ),
        tool(
            "get_oplog",
            "Get oplog",
            "Get the part params and Op list (the design source of truth; its ids are used by replace_op/remove_op).",
            schema(json!({ "session": session_prop() }), &[]),
        ),
        tool("undo", "Undo", "Undo the last run_ops batch (can be redone).", schema(json!({ "session": session_prop() }), &[])),
        tool("redo", "Redo", "Redo the last undone batch.", schema(json!({ "session": session_prop() }), &[])),
        tool(
            "export",
            "Export",
            "Export visible bodies to STEP/STL/OBJ/GLB.",
            schema(
                json!({ "session": session_prop(), "format": { "type": "string", "enum": ["step", "stl", "obj", "glb"] },
                        "path": { "type": "string" } }),
                &["format", "path"],
            ),
        ),
        tool(
            "get_schema",
            "Op schema",
            "No arguments: a cheap SUMMARY (every op kind + description + required/optional fields, selector cheatsheet, example list). {op:\"fillet\"} = full schema of one op kind; {example:\"flange\"} = a tested example OpFile; {full:true} = the entire JSON Schema (large). Call the summary once at the start.",
            schema(
                json!({
                    "op": { "type": "string", "enum": op_kinds() },
                    "example": { "type": "string", "enum": example_names },
                    "full": { "type": "boolean" },
                    "rule_packs": { "type": "boolean", "description": "List the rule packs: rules, citations, variables." },
                    "checks": { "type": "boolean", "description": "Every check kind with its required/optional fields." },
                    "check": { "type": "string", "description": "Full schema of one check kind, e.g. \"hole_web\"." }
                }),
                &[],
            ),
        ),
        tool(
            "set_checks",
            "Set checks",
            "Replace the whole list of design checks (user requirements: volume, bbox_size, hole_count, min_wall, clearance, mass, center_of_mass, moment_of_inertia, …) and evaluate them now. Write them before modeling; every later BatchReport includes their results. Per check: `requirement` (id from set_brief, for the traceability matrix), `citation` (standard clause or guideline), `severity` (`error` default blocks builds, `warning` only reports). `rule_packs` adds ready-made cited DFM/standard checks (dfm-cnc, dfm-fdm, dfm-injection, dfm-sheet-metal, iso-2768, fea-safety). Kinds and fields: get_schema {\"checks\":true}.",
            set_checks,
        ),
        tool(
            "run_checks",
            "Run checks",
            "Evaluate the design checks (or the given 'checks' list) against the current geometry; returns pass/fail per check with the measured value.",
            run_checks,
        ),
        tool(
            "propose_ops",
            "Propose changes",
            "Preview a change without modifying the part: body diff (+/- volume) and a colored diff image (green = added, red = removed). Give 'ops' (appended) and/or 'params', 'replace', 'remove' (oplog edits). Apply with accept_proposal.",
            schema(
                json!({
                    "session": session_prop(),
                    "ops": ops_prop("Ops appended to the end of the oplog."),
                    "params": params_prop(),
                    "replace": { "type": "array", "description": "Ops to replace: [{id, op}].",
                                 "items": schema(json!({ "id": { "type": "string" }, "op": op_item() }), &["id", "op"]) },
                    "remove": { "type": "array", "items": { "type": "string" }, "description": "Ids of the ops to remove." }
                }),
                &[],
            ),
        ),
        tool(
            "accept_proposal",
            "Accept proposal",
            "Apply a proposal from propose_ops (fails with proposal_stale if the part changed since it was made).",
            schema(json!({ "session": session_prop(), "proposal_id": { "type": "string" } }), &["proposal_id"]),
        ),
        tool(
            "reject_proposal",
            "Reject proposal",
            "Discard an unused proposal.",
            schema(json!({ "session": session_prop(), "proposal_id": { "type": "string" } }), &["proposal_id"]),
        ),
        tool(
            "drawing",
            "Drawing",
            "Engineering drawing sheet to PDF/SVG/DXF: orthographic views with exact hidden-line removal (circles stay circles), section views, per-view scale, automatic associative dimensions (overall sizes, Ø of shafts/bores, hole patterns `4×Ø14 PCD Ø130`, radii, chamfers, hole spacing), shaded 3D renders, notes and an ISO title block. All fields except `format` and `path` are optional; with none given you get front/top/right/isometric views, one section A-A through the model centre and automatic dimensions. `name` + `save: true` stores the sheet in the part (`inspect` lists it under `drawings`); afterwards `{\"name\":\"sheet1\",\"format\":\"pdf\",\"path\":…}` re-renders it, e.g. after `set_params` — dimension texts follow the new geometry while their positions stay. The result lists the dimension texts and `warnings`. `annotations` add toleranced dimensions, GD&T frames, datums, surface finish, hole and revision tables at sheet positions in mm (PDF/SVG only).",
            schema(
                json!({ "session": session_prop(),
                        "format": { "type": "string", "enum": ["pdf", "svg", "dxf"] },
                        "path": { "type": "string" },
                        "name": { "type": "string", "description": "Sheet name (default \"sheet1\"). If a sheet with this name is stored in the part it is the starting point and the other fields override it." },
                        "save": { "type": "boolean", "description": "Store the resulting sheet spec in the part under `name`." },
                        "paper": { "type": "string", "enum": ["a4", "a4-portrait", "a3", "a3-portrait"] },
                        "title": { "type": "string" }, "part_number": { "type": "string" },
                        "material": { "type": "string" },
                        "notes": { "type": "array", "items": { "type": "string" }, "description": "General notes, drawn numbered above the title block. For a new sheet they are appended to the automatic hole notes; for a stored sheet they replace its notes." },
                        "sections": { "type": "array", "items": { "type": "object" },
                            "description": "Section views; `[]` = none; omitted = one section A-A. Each item: `label` (one letter), `parent` (`top` default, `front`, `right` = view carrying the cutting line), then EITHER `axis` (`x`|`y`|`z` plane normal) + `offset` (mm from the bounding-box centre) + `flip`, OR `path` = cutting line points [[u,v],…] in parent-view mm relative to the bounding-box centre (2 points = straight cut, more = stepped section whose segments are parallel/perpendicular to the first; the view looks to the LEFT of the travel direction). Examples: {\"label\":\"A\",\"parent\":\"top\",\"axis\":\"x\",\"offset\":0}; {\"label\":\"B\",\"parent\":\"top\",\"path\":[[-80,20],[80,20]]}." },
                        "views": { "type": "object",
                            "description": "Per-view overrides keyed by `front`, `top`, `right`, `isometric`, `section_a`, …: {\"visible\":false} hides a view, {\"scale\":0.5} draws it at 1:2 and prints \"SCALE 1:2\" under it. Example: {\"right\":{\"visible\":false},\"section_b\":{\"scale\":0.5}}." },
                        "scale": { "description": "Sheet scale: \"auto\" (default, largest standard scale that fits), a ratio string \"1:2\", or a number 0.5." },
                        "shaded": { "type": "array",
                            "description": "Shaded 3D renders placed on the sheet (deterministic CPU raster). Items: \"iso\", \"iso_back\", or {\"camera\":{\"custom\":{\"yaw\":135,\"pitch\":-25}},\"px_per_mm\":8}. Skipped in DXF." },
                        "dimensions": { "description": "\"auto\" (default), \"none\", or an explicit list of {\"type\":\"diameter\"|\"radius\"|\"hole_pattern\",\"select\":<face selector as in query_geometry>,\"view\":\"top\"|\"front\"|\"right\"|\"section_a\"|…,\"body\"?:name}. Example: [{\"type\":\"hole_pattern\",\"select\":\"all[kind=cylinder][r=7]\",\"view\":\"top\"}]." },
                        "hidden_lines": { "type": "boolean", "description": "Draw hidden (dashed) lines; default true. Production sheets with section views usually set false." },
                        "annotations": { "type": "array", "items": { "type": "object" },
                            "description": "Each item has `type` and `position` [x,y] in sheet mm. Examples: {\"type\":\"feature_control_frame\",\"position\":[60,40],\"frame\":{\"symbol\":\"position\",\"value\":0.1,\"diameter_zone\":true,\"modifiers\":[\"mmc\"],\"datums\":[\"A\",\"B\"]}}; {\"type\":\"dimension_tolerance\",\"position\":[80,60],\"dimension\":{\"nominal\":25,\"tolerance\":{\"fit\":\"H7\"},\"diameter\":true}}; {\"type\":\"datum_feature\",\"position\":[30,30],\"datum\":{\"label\":\"A\"}}; {\"type\":\"surface_finish\",\"position\":[90,30],\"finish\":{\"ra_um\":1.6}}. GD&T symbols: straightness, flatness, circularity, cylindricity, profile_of_line, profile_of_surface, perpendicularity, angularity, parallelism, position, concentricity, symmetry, circular_runout, total_runout." } }),
                &["format", "path"],
            ),
        ),
        tool(
            "import_step",
            "Import STEP",
            "Import a STEP file as a body named `name` (the STEP content is stored in the part; later ops can use the body, e.g. boolean).",
            schema(
                json!({ "session": session_prop(), "path": { "type": "string" },
                        "name": { "type": "string", "pattern": "^[a-z][a-z0-9_]{0,31}$" } }),
                &["path", "name"],
            ),
        ),
        tool(
            "import_pdf",
            "Import PDF vectors",
            "Import the vector lines of a PDF drawing (exported from CAD) as ONE sketch of construction polylines, to trace or measure an old drawing. Text, raster images and scans are not imported. Coordinates are mm; `scale` multiplies them (a 1:2 drawing needs 2). Returns polyline/point counts and size_mm.",
            schema(
                json!({ "session": session_prop(), "path": { "type": "string" },
                        "sketch": { "type": "string", "pattern": "^[a-z][a-z0-9_]{0,31}$", "description": "Id of the sketch op to create." },
                        "plane": { "type": "string", "description": "Sketch plane, default \"XY\"." },
                        "page": { "type": "integer", "minimum": 0, "description": "Page with vector paths, from 1 (default); 0 = all pages." },
                        "scale": { "type": "number", "exclusiveMinimum": 0 },
                        "keep_origin": { "type": "boolean", "description": "Keep page coordinates; default moves the drawing's lower-left corner to (0,0)." },
                        "as_geometry": { "type": "boolean", "description": "Normal lines (usable as profiles) instead of construction lines." },
                        "max_points": { "type": "integer", "minimum": 1 },
                        "dry_run": { "type": "boolean" } }),
                &["path", "sketch"],
            ),
        ),
        tool(
            "diff",
            "Diff parts",
            "Compare the session part with another .ducad file or session: param, op, and per-body volume changes.",
            schema(
                json!({ "session": session_prop(), "against_path": { "type": "string" },
                        "against_session": { "type": "string" },
                        "geometric": { "type": "boolean", "description": "Compute per-body volume differences (slower)." } }),
                &[],
            ),
        ),
        tool(
            "list_parts",
            "List parts",
            "List open part sessions (id, name, path, body/op counts).",
            schema(json!({}), &[]),
        ),
        tool(
            "set_brief",
            "Set design brief",
            "Store the design brief in the part BEFORE modeling: what the part does, interfaces, loads, material, process, quantity, standards, cost target and numbered `requirements` [{id:\"R1\", text, source?}]. Then link every check to a requirement (`requirement` field of set_checks). Returns the brief, the traceability matrix and `suggested_rule_pack` for the process.",
            schema(
                json!({ "session": session_prop(),
                        "brief": { "type": "object", "description": "{title, function, interfaces[], loads[], material, process (cnc|sheet_metal|fdm|sla|sls|injection|casting), quantity, standards[], cost_target, notes[], requirements[{id,text,source?}]}; every field is optional." } }),
                &["brief"],
            ),
        ),
        tool(
            "get_brief",
            "Get design brief",
            "The stored design brief and its traceability matrix (requirement -> checks -> verified/failed/unverified).",
            schema(json!({ "session": session_prop() }), &[]),
        ),
        tool(
            "build_package",
            "Build design package",
            "Build the design package as ONE multi-page PDF: cover (brief, sheet index, revision table, compliance summary), part drawing per body, assembly drawing with BOM balloons, sheet metal flat patterns, BOM page and a compliance page (traceability + every check with its citation). `sheets` omitted = automatic. `reports_dir` also writes traceability.md, compliance.md and cost.md (indicative cost from the firm profile). Returns the sheet index, release state, check counts and warnings.",
            schema(
                json!({ "session": session_prop(), "path": { "type": "string", "description": "Output PDF." },
                        "title": { "type": "string" }, "part_number": { "type": "string" },
                        "author": { "type": "string" }, "date": { "type": "string", "description": "YYYY-MM-DD; default today." },
                        "sheets": { "type": "array", "items": { "type": "object" },
                            "description": "Sheets in order: {\"kind\":\"cover\"}, {\"kind\":\"drawing\",\"name\":\"sheet1\"} (stored sheet), {\"kind\":\"part\",\"body\":B}, {\"kind\":\"assembly\"}, {\"kind\":\"flat_pattern\",\"body\":B}, {\"kind\":\"bom\"}, {\"kind\":\"compliance\"}." },
                        "paper": { "type": "string", "enum": ["a4", "a4-portrait", "a3", "a3-portrait"] },
                        "save": { "type": "boolean", "description": "Store this sheet list in the part." },
                        "firm": { "type": "string", "description": "Firm profile JSON; default: firm.json next to the output, then the memory vault." },
                        "reports_dir": { "type": "string" } }),
                &["path"],
            ),
        ),
        tool(
            "package_status",
            "Package status",
            "Release state of the design package (draft | in_review | approved), revision, revision history and the review checklist (checks, requirements, standards, drawings, open redlines). An approval is void as soon as the design changes.",
            schema(json!({ "session": session_prop() }), &[]),
        ),
        tool(
            "submit_for_review",
            "Submit for review",
            "Mark the current revision as in review and record `description` in the revision table. After an approved revision changed, this opens the next revision (A -> B). Approval itself is done by the user.",
            schema(
                json!({ "session": session_prop(), "description": { "type": "string", "description": "What changed in this revision." },
                        "by": { "type": "string" }, "date": { "type": "string" } }),
                &[],
            ),
        ),
        tool(
            "approve_package",
            "Approve package",
            "Approve the current revision for release (headless sessions only; in the app only the user can approve). Refused while a blocking check fails or a redline is open unless `force` is true.",
            schema(
                json!({ "session": session_prop(), "by": { "type": "string", "description": "Name of the approver." },
                        "date": { "type": "string" }, "force": { "type": "boolean" } }),
                &["by"],
            ),
        ),
        tool(
            "list_redlines",
            "List redlines",
            "Review notes (redlines) the user drew on the stored drawing sheets: id, sheet, text, target point. Each open redline is a change request: make the change (set_params/replace_op/propose_ops), then close it with resolve_redline.",
            schema(json!({ "session": session_prop(), "all": { "type": "boolean", "description": "Include resolved redlines." } }), &[]),
        ),
        tool(
            "add_redline",
            "Add redline",
            "Add a review note to a stored sheet (created as \"sheet1\" if the part has none): red text at `position` [x,y] in sheet mm from the bottom-left corner, with an optional arrow to `target`.",
            schema(
                json!({ "session": session_prop(), "sheet": { "type": "string" }, "text": { "type": "string" },
                        "position": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 },
                        "target": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 },
                        "author": { "type": "string" } }),
                &["text", "position"],
            ),
        ),
        tool(
            "resolve_redline",
            "Resolve redline",
            "Close a redline after the requested change is made; `resolution` states what was changed. Resolved redlines are no longer drawn.",
            schema(
                json!({ "session": session_prop(), "sheet": { "type": "string" }, "id": { "type": "string" },
                        "resolution": { "type": "string" } }),
                &["id", "resolution"],
            ),
        ),
    ]
}

/// Tool yang hanya ada pada sesi live (aplikasi terbuka), P15.2.
pub const LIVE_TOOL_NAMES: &[&str] = &[
    "document_info",
    "get_view",
    "set_view",
    "get_selection",
    "select",
    "screenshot",
];

/// Definisi tool sesi live.
pub fn live_definitions() -> Vec<Value> {
    let view = json!({ "type": "string", "enum": ["iso", "front", "back", "left", "right", "top", "bottom"] });
    vec![
        tool(
            "document_info",
            "Document info",
            "State of the document open in the app: file, mode, bodies (name, visible, volume, selected), params, selection count, camera.",
            schema(json!({ "session": session_prop() }), &[]),
        ),
        tool("get_view", "Get view", "The user's viewport camera position.", schema(json!({ "session": session_prop() }), &[])),
        tool(
            "set_view",
            "Set view",
            "Point the user's viewport camera: preset view and/or fit to bodies (default: fit all visible bodies).",
            schema(
                json!({ "session": session_prop(), "view": view, "fit": { "type": "boolean" },
                        "bodies": { "type": "array", "items": { "type": "string" } } }),
                &[],
            ),
        ),
        tool(
            "get_selection",
            "Get selection",
            "What the user has selected: bodies, faces (point + normal), edges (endpoints). Use it to resolve \"this\"/\"that one\" in instructions.",
            schema(json!({ "session": session_prop() }), &[]),
        ),
        tool(
            "select",
            "Select",
            "Highlight bodies / planar faces / edges in the viewport via selectors (replaces the selection, or `add`). No arguments = clear the selection.",
            schema(
                json!({ "session": session_prop(), "body": { "type": "string" }, "faces": { "type": "string" },
                        "edges": { "type": "string" }, "add": { "type": "boolean" } }),
                &[],
            ),
        ),
        tool(
            "screenshot",
            "Screenshot",
            "Screenshot of the app window (what the user sees) as PNG.",
            schema(json!({ "session": session_prop() }), &[]),
        ),
    ]
}

/// Definisi tool untuk chat agent di dalam aplikasi (P13.2). `live`:
/// sesi tunggal dokumen yang sedang terbuka, jadi tool yang membuat/menutup
/// sesi dan `accept_proposal` (hanya pengguna yang boleh) disaring keluar,
/// dan tool live ditambahkan.
pub fn chat_tools(live: bool) -> Vec<Value> {
    let mut defs: Vec<Value> = definitions()
        .into_iter()
        .filter(|t| {
            let name = t["name"].as_str().unwrap_or_default();
            !live || !crate::attach::UNSUPPORTED.contains(&name)
        })
        .collect();
    if live {
        defs.extend(live_definitions());
    }
    defs
}

/// Jalankan tool; error menjadi `isError: true` dengan payload `{"error": OpError}`.
pub fn call(server: &mut Server, name: &str, arguments: Value) -> Value {
    tool_result(call_out(server, name, arguments))
}

/// Seperti [`call`] tetapi mengembalikan [`ToolOut`] mentah (dipakai
/// `ducad-cli chat`, yang menjalankan server in-process tanpa JSON-RPC).
pub fn call_out(server: &mut Server, name: &str, arguments: Value) -> ToolOut {
    let live_ok = server.attach.is_some() && LIVE_TOOL_NAMES.contains(&name);
    if !TOOL_NAMES.contains(&name) && !live_ok {
        return ToolOut::err(OpError::invalid(format!("unknown tool: {name}")));
    }
    if let Some(attach) = server.attach.as_mut() {
        return attach.call(name, arguments);
    }
    // Pagar panik (ADR 0009): panik di tool mana pun menjadi error `internal`,
    // bukan server yang mati di tengah sesi agent.
    ducad_engine::guard::catch_op_panic(&format!("tool {name}"), || {
        call_inner(server, name, arguments)
    })
    .unwrap_or_else(ToolOut::err)
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
            let path = target.or_else(|| part.path.clone()).ok_or_else(|| {
                OpError::invalid("the part has no path yet; pass the 'path' argument")
            })?;
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
                #[serde(default)]
                params: Params,
                #[serde(default)]
                configuration: Option<String>,
            }
            let a: A = args(a)?;
            let (_, part) = server.pick(a.session.as_deref())?;
            let report = match a.configuration.as_deref() {
                // Hanya berpindah varian.
                Some(name) if a.params.is_empty() => part.session.activate_configuration(name)?,
                Some(name) => part.session.set_configuration_params(name, a.params)?,
                None => {
                    let mut params = part.session.design().params.clone();
                    params.extend(a.params);
                    part.session.set_params(params)?
                }
            };
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
        "replace_op" | "remove_op" => {
            let e = parse_edit_args(name, a)?;
            let (_, part) = server.pick(e.session.as_deref())?;
            let report = part
                .session
                .edit_oplog(None, e.replace, e.remove, e.dry_run)?;
            let is_error = report.error.is_some();
            Ok(ToolOut {
                payload: to_value(report)?,
                image_png: None,
                is_error,
            })
        }
        "propose_ops" => {
            let e = parse_edit_args(name, a)?;
            let (_, part) = server.pick(e.session.as_deref())?;
            let params = e.params.map(|p| {
                let mut merged = part.session.design().params.clone();
                merged.extend(p);
                merged
            });
            let (proposal, shapes) = part
                .session
                .propose_oplog_edit(params, e.replace, e.remove, e.append)?;
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
        "approve_package" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                by: String,
                #[serde(default)]
                date: Option<String>,
                #[serde(default)]
                force: bool,
            }
            let a: A = args(a)?;
            let (_, part) = server.pick(a.session.as_deref())?;
            // Check studi butuh hasil segar sebelum ceklis review dinilai.
            part.session.refresh_studies();
            let results = part.session.run_checks(None).results;
            let review = ducad_engine::brief::review_checklist(part.session.design(), &results);
            let blockers = ducad_engine::brief::review_blockers(&review);
            if !blockers.is_empty() && !a.force {
                return Err(OpError::new(
                    OpErrorCode::InvalidParam,
                    format!("the package cannot be approved yet: {}", blockers.join("; ")),
                )
                .with_hint("fix the failing checks and resolve the open redlines, or pass force:true")
                .with_context(json!({ "review": review })));
            }
            let (mut release, signature) = part.session.release();
            let date = a.date.unwrap_or_else(ducad_engine::brief::today);
            release.approve(&signature, &a.by, &date)?;
            part.session.set_release(release);
            let mut out = ducad_engine::tooling::package_payload(&part.session.core())?;
            out["forced"] = json!(a.force && !blockers.is_empty());
            Ok(ToolOut::ok(out))
        }
        "list_parts" => {
            let _: Value = a;
            let parts: Vec<Value> = server
                .sessions
                .iter()
                .map(|(id, p)| {
                    json!({
                        "session": id,
                        "name": p.name,
                        "path": p.path,
                        "bodies": p.session.summary().bodies.len(),
                        "ops": p.session.design().oplog.len(),
                    })
                })
                .collect();
            Ok(ToolOut::ok(json!({ "parts": parts })))
        }
        "diff" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct A {
                #[serde(default)]
                session: Option<String>,
                #[serde(default)]
                against_path: Option<String>,
                #[serde(default)]
                against_session: Option<String>,
                #[serde(default)]
                geometric: bool,
            }
            let a: A = args(a)?;
            let (key, _) = server.pick(a.session.as_deref())?;
            let loaded;
            let that: &Session = match (&a.against_path, &a.against_session) {
                (Some(p), None) => {
                    loaded = Session::from_file(&server.resolve(p)?)?;
                    &loaded
                }
                (None, Some(k)) => {
                    &server
                        .sessions
                        .get(k)
                        .ok_or_else(|| {
                            OpError::new(OpErrorCode::UnknownRef, format!("unknown session '{k}'"))
                        })?
                        .session
                }
                _ => {
                    return Err(OpError::invalid(
                        "give exactly one of 'against_path' or 'against_session'",
                    ))
                }
            };
            let this = &server.sessions[&key].session;
            let (d, _) = ducad_engine::diff::diff(that, this, a.geometric);
            Ok(ToolOut::ok(to_value(d)?))
        }
        other => Err(OpError::invalid(format!("unknown tool: {other}"))),
    }
}

#[cfg(test)]
mod hostile_tests {
    use super::*;
    use serde_json::json;

    fn server() -> (Server, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "ducad-mcp-hostile-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        (Server::new(dir.clone()).unwrap(), dir)
    }

    /// Pola `hostile_params_never_panic` (CADCraft): setiap tool dengan
    /// argumen rusak harus mengembalikan nilai, bukan panik. Memanggil
    /// `call_inner` LANGSUNG — tanpa pagar `guard` — supaya panik benar-benar
    /// menggagalkan tes, bukan diubah menjadi error `internal`.
    #[test]
    fn hostile_arguments_never_panic() {
        let (mut s, dir) = server();
        let _ = call_inner(&mut s, "new_part", json!({"name": "p"}));
        let hostile = [
            json!(null),
            json!([]),
            json!("x"),
            json!({}),
            json!({"session": 5}),
            json!({"session": "nope"}),
            json!({"ops": "x"}),
            json!({"ops": [null]}),
            json!({"ops": [{"op": "extrude", "id": "x", "profile": null, "distance": -1e308}]}),
            json!({"path": "../../../etc/passwd"}),
            json!({"path": ""}),
            json!({"params": {"w": "$w/0", "": 1}}),
            json!({"op_id": "", "op": {"op": "nope"}, "ids": [""]}),
            json!({"selector": "all[", "body": "", "view": "zz", "format": "exe"}),
            json!({"width": -1, "height": 0, "checks": [null], "name": 7}),
            json!({"x".repeat(2000): "y".repeat(2000)}),
        ];
        for name in TOOL_NAMES {
            for args in &hostile {
                let out = call_inner(&mut s, name, args.clone());
                if let Ok(out) = &out {
                    assert!(out.payload.is_object() || out.payload.is_array(), "{name}");
                }
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_panicking_tool_becomes_an_internal_error_not_a_crash() {
        let e = ducad_engine::guard::catch_op_panic("tool x", || -> OpResult<ToolOut> {
            panic!("boom")
        })
        .unwrap_err();
        let out = tool_result(ToolOut::err(e));
        assert_eq!(out["isError"], true);
        assert!(out["content"][0]["text"].as_str().unwrap().contains("internal"));
    }
}
