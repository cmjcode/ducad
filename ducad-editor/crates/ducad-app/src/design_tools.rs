//! Panel Parameters (Paritas 0.9), Salin oplog JSON dan Inspect (0.11).
//!
//! Parameter adalah `DesignDoc::params` (nama → nilai mm/derajat) yang dirujuk
//! op lewat ekspresi `$nama`. Menerapkan perubahan = replay oplog di sesi
//! salinan lalu adopsi sebagai SATU langkah undo — jalur yang sama dengan
//! `set_params` milik agent. Rename juga menulis ulang `$lama` → `$baru` di
//! setiap op yang memakainya.

use ducad_engine::ops::{Op, Params};
use ducad_engine::DesignDoc;
use serde_json::Value;

use crate::app::DuCADApp;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParamRow {
    pub name: String,
    /// Nama saat panel dibuka (`None` = baris baru).
    pub orig_name: Option<String>,
    pub value: String,
}

#[derive(Default)]
pub struct DesignToolsState {
    pub params_open: bool,
    pub rows: Vec<ParamRow>,
    pub error: Option<String>,
    pub inspect_open: bool,
}

fn is_ident(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_')
        && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// Ganti `$old` (utuh, bukan awalan nama lain) menjadi `$new` di `s`.
pub fn rename_in_expr(s: &str, old: &str, new: &str) -> String {
    let pat = format!("${old}");
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(&pat) {
        let after = &rest[i + pat.len()..];
        let boundary = !after.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_');
        out.push_str(&rest[..i]);
        if boundary {
            out.push('$');
            out.push_str(new);
        } else {
            out.push_str(&pat);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

fn rename_value(v: &mut Value, old: &str, new: &str) {
    match v {
        Value::String(s) if s.contains('$') => *s = rename_in_expr(s, old, new),
        Value::Array(a) => a.iter_mut().for_each(|x| rename_value(x, old, new)),
        Value::Object(o) => o.values_mut().for_each(|x| rename_value(x, old, new)),
        _ => {}
    }
}

fn uses_param(op: &Op, name: &str) -> bool {
    let Ok(json) = serde_json::to_string(op) else { return false };
    rename_in_expr(&json, name, "\u{1}") != json
}

/// Id op yang memakai parameter `name`.
pub fn param_users(design: &DesignDoc, name: &str) -> Vec<String> {
    design.oplog.iter().filter(|o| uses_param(o, name)).map(|o| o.id().to_string()).collect()
}

/// Validasi baris panel → (params baru, daftar rename). Error berupa teks
/// siap tampil.
pub fn rows_to_params(design: &DesignDoc, rows: &[ParamRow]) -> Result<(Params, Vec<(String, String)>), String> {
    let mut params = Params::new();
    let mut renames = Vec::new();
    for r in rows {
        let name = r.name.trim();
        if !is_ident(name) {
            return Err(ducad_i18n::t!("params-bad-name", name = name));
        }
        let value: f64 = r
            .value
            .trim()
            .parse()
            .ok()
            .filter(|v: &f64| v.is_finite())
            .ok_or_else(|| ducad_i18n::t!("params-bad-value", name = name))?;
        if params.insert(name.to_string(), value).is_some() {
            return Err(ducad_i18n::t!("params-duplicate", name = name));
        }
        if let Some(old) = &r.orig_name {
            if old != name {
                renames.push((old.clone(), name.to_string()));
            }
        }
    }
    // Parameter yang dihapus tidak boleh masih dipakai op.
    let kept: Vec<&String> = rows.iter().filter_map(|r| r.orig_name.as_ref()).collect();
    for old in design.params.keys().filter(|k| !kept.contains(k)) {
        let users = param_users(design, old);
        if !users.is_empty() {
            let ops = users.join(", ");
            return Err(ducad_i18n::t!("params-in-use", name = old.as_str(), ops = ops.as_str()));
        }
    }
    Ok((params, renames))
}

/// Terapkan rename ke seluruh oplog.
pub fn rename_ops(oplog: &[Op], renames: &[(String, String)]) -> Result<Vec<Op>, String> {
    oplog
        .iter()
        .map(|op| {
            let mut v = serde_json::to_value(op).map_err(|e| e.to_string())?;
            for (old, new) in renames {
                rename_value(&mut v, old, new);
            }
            serde_json::from_value(v).map_err(|e| e.to_string())
        })
        .collect()
}

impl DuCADApp {
    pub fn open_params_panel(&mut self) {
        self.design_tools.rows = self
            .agent_meta
            .design
            .params
            .iter()
            .map(|(k, v)| ParamRow { name: k.clone(), orig_name: Some(k.clone()), value: format!("{v}") })
            .collect();
        self.design_tools.error = None;
        self.design_tools.params_open = true;
    }

    /// Terapkan baris panel. `Ok` bila tersimpan.
    pub fn apply_param_rows(&mut self, rows: &[ParamRow]) -> Result<(), String> {
        let (params, renames) = rows_to_params(&self.agent_meta.design, rows)?;
        if self.agent_meta.design.oplog.is_empty() {
            // Belum ada op parametrik: cukup simpan nilainya.
            self.agent_meta.design.params = params;
            self.model.doc.dirty = true;
            return Ok(());
        }
        let oplog = rename_ops(&self.agent_meta.design.oplog, &renames)?;
        let design = DesignDoc { params, oplog, fingerprint: String::new(), ..self.agent_meta.design.clone() };
        let session = ducad_engine::Session::replay(design).map_err(|e| e.message.clone())?;
        let meta = session.meta().clone();
        self.adopt_agent_model(session.into_model(), meta, "parameters");
        Ok(())
    }

    /// Salin oplog sebagai JSON ke clipboard (Export Command Script).
    pub fn copy_oplog_json(&mut self, ctx: &egui::Context) {
        match serde_json::to_string_pretty(&self.agent_meta.design.oplog) {
            Ok(json) => {
                let n = self.agent_meta.design.oplog.len();
                ctx.copy_text(json);
                self.model_status = Some(ducad_i18n::t!("oplog-copied", count = n));
            }
            Err(e) => self.model_status = Some(e.to_string()),
        }
    }

    /// Ringkasan inspect untuk body terpilih (atau semua bila tak ada pilihan).
    pub fn inspect_rows(&mut self) -> Vec<ducad_engine::inspect::BodyReport> {
        let selected: Vec<String> = self
            .selected_bodies
            .iter()
            .filter_map(|id| self.model.doc.bodies.get(*id).map(|b| b.name.clone()))
            .collect();
        let core = ducad_engine::SessionCore {
            model: &mut self.model,
            model_undo: &mut self.model_undo,
            sketches: &mut self.sketch_set,
            meta: &mut self.agent_meta,
        };
        match ducad_engine::inspect::summarize_core(&core, None, false, 0) {
            Ok(s) => s
                .bodies
                .into_iter()
                .filter(|b| selected.is_empty() || selected.contains(&b.name))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn show_design_tools(&mut self, ctx: &egui::Context) {
        self.show_params_window(ctx);
        self.show_inspect_window(ctx);
    }

    fn show_params_window(&mut self, ctx: &egui::Context) {
        if !self.design_tools.params_open {
            return;
        }
        let mut open = true;
        let mut apply = false;
        let design = self.agent_meta.design.clone();
        let state = &mut self.design_tools;
        egui::Window::new(ducad_i18n::t!("params-title"))
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(ducad_i18n::t!("params-help")).small());
                let mut remove = None;
                egui::Grid::new("params-grid").num_columns(4).striped(true).show(ui, |ui| {
                    ui.strong(ducad_i18n::t!("params-col-name"));
                    ui.strong(ducad_i18n::t!("params-col-value"));
                    ui.strong(ducad_i18n::t!("params-col-used-by"));
                    ui.label("");
                    ui.end_row();
                    for (i, row) in state.rows.iter_mut().enumerate() {
                        ui.add(egui::TextEdit::singleline(&mut row.name).desired_width(110.0));
                        ui.add(egui::TextEdit::singleline(&mut row.value).desired_width(80.0));
                        let users = row.orig_name.as_deref().map(|n| param_users(&design, n)).unwrap_or_default();
                        ui.label(if users.is_empty() { "-".to_string() } else { users.join(", ") });
                        if ui
                            .button(egui_icons::icons::ICON_DELETE.codepoint)
                            .on_hover_text(ducad_i18n::t!("params-delete"))
                            .clicked()
                        {
                            remove = Some(i);
                        }
                        ui.end_row();
                    }
                });
                if let Some(i) = remove {
                    state.rows.remove(i);
                }
                ui.horizontal(|ui| {
                    if ui.button(ducad_i18n::t!("params-add")).clicked() {
                        let n = state.rows.len() + 1;
                        state.rows.push(ParamRow { name: format!("p{n}"), orig_name: None, value: "10".into() });
                    }
                    apply = ui.button(ducad_i18n::t!("params-apply")).clicked();
                });
                if let Some(err) = &state.error {
                    ui.colored_label(egui::Color32::from_rgb(255, 110, 110), err);
                }
            });
        if apply {
            let rows = self.design_tools.rows.clone();
            match self.apply_param_rows(&rows) {
                Ok(()) => {
                    self.model_status = Some(ducad_i18n::t!("params-applied"));
                    self.open_params_panel();
                }
                Err(e) => self.design_tools.error = Some(e),
            }
        }
        if !open {
            self.design_tools.params_open = false;
        }
    }

    fn show_inspect_window(&mut self, ctx: &egui::Context) {
        if !self.design_tools.inspect_open {
            return;
        }
        let rows = self.inspect_rows();
        let mut open = true;
        let mut copy = false;
        egui::Window::new(ducad_i18n::t!("inspect-title"))
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                if rows.is_empty() {
                    ui.label(ducad_i18n::t!("inspect-empty"));
                }
                egui::Grid::new("inspect-grid").num_columns(5).striped(true).show(ui, |ui| {
                    for h in ["inspect-col-body", "inspect-col-size", "inspect-col-volume", "inspect-col-faces", "inspect-col-valid"] {
                        ui.strong(ducad_i18n::t!(h));
                    }
                    ui.end_row();
                    for b in &rows {
                        ui.label(&b.name);
                        ui.label(format!("{:.2} x {:.2} x {:.2}", b.size[0], b.size[1], b.size[2]));
                        ui.label(format!("{:.2}", b.volume));
                        ui.label(format!("{} / {}", b.faces, b.edges));
                        ui.label(if b.valid { "OK" } else { "!" });
                        ui.end_row();
                    }
                });
                copy = ui.button(ducad_i18n::t!("oplog-copy")).clicked();
            });
        if copy {
            self.copy_oplog_json(ctx);
        }
        if !open {
            self.design_tools.inspect_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plate_design() -> DesignDoc {
        let ops: Vec<Op> = serde_json::from_value(serde_json::json!([
            {"op":"primitive","id":"b","shape":{"box":{"size":["$w", "$w/2", 5]}}}
        ]))
        .expect("op valid");
        DesignDoc { params: [("w".to_string(), 20.0)].into_iter().collect(), oplog: ops, ..Default::default() }
    }

    #[test]
    fn rename_only_touches_whole_names() {
        assert_eq!(rename_in_expr("$w/2+$wall", "w", "width"), "$width/2+$wall");
        assert_eq!(rename_in_expr("$w", "w", "x"), "$x");
        assert_eq!(rename_in_expr("w+1", "w", "x"), "w+1");
    }

    #[test]
    fn param_users_and_delete_guard() {
        let d = plate_design();
        assert_eq!(param_users(&d, "w"), vec!["b".to_string()]);
        let err = rows_to_params(&d, &[]).unwrap_err();
        assert!(err.contains('b'), "{err}");
    }

    #[test]
    fn rows_validate_names_values_and_duplicates() {
        let d = DesignDoc::default();
        let row = |n: &str, v: &str| ParamRow { name: n.into(), orig_name: None, value: v.into() };
        assert!(rows_to_params(&d, &[row("1x", "3")]).is_err());
        assert!(rows_to_params(&d, &[row("a", "abc")]).is_err());
        assert!(rows_to_params(&d, &[row("a", "NaN")]).is_err());
        assert!(rows_to_params(&d, &[row("a", "1"), row("a", "2")]).is_err());
        let (p, r) = rows_to_params(&d, &[row("a", " 2.5 ")]).unwrap();
        assert_eq!(p["a"], 2.5);
        assert!(r.is_empty());
    }

    #[test]
    fn rename_rewrites_ops_and_replay_succeeds() {
        let d = plate_design();
        let rows = vec![ParamRow { name: "width".into(), orig_name: Some("w".into()), value: "30".into() }];
        let (params, renames) = rows_to_params(&d, &rows).unwrap();
        let ops = rename_ops(&d.oplog, &renames).unwrap();
        let json = serde_json::to_string(&ops).unwrap();
        assert!(json.contains("$width") && !json.contains("\"$w\""), "{json}");
        let session = ducad_engine::Session::replay(DesignDoc { params, oplog: ops, ..d }).expect("replay");
        let vol: f64 = session.model().geometry.values().map(|g| g.shape.volume()).sum();
        assert!((vol - 30.0 * 15.0 * 5.0).abs() < 1.0, "{vol}");
    }

    #[test]
    fn apply_without_oplog_just_stores_params() {
        let mut app = DuCADApp::new_for_test();
        let rows = vec![ParamRow { name: "t".into(), orig_name: None, value: "3".into() }];
        app.apply_param_rows(&rows).unwrap();
        assert_eq!(app.agent_meta.design.params["t"], 3.0);
    }
}
