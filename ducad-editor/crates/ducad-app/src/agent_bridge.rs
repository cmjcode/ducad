//! Jembatan live agent → GUI (P5.1): soket Unix di `$HOME/.ducad/agent.sock`
//! (folder yang sama dengan `ducad_history.db`) tempat `ducad-mcp --attach`
//! meneruskan tool ke aplikasi yang sedang terbuka, sehingga pengguna
//! melihat model terbentuk.
//!
//! Aturan main:
//! - **Mati secara default**; dinyalakan lewat command palette ("Agent
//!   Bridge") dan tidak tersedia saat privasi AI `OfflineOnly`.
//! - Thread latar hanya MEMBACA baris dan meneruskannya; seluruh eksekusi
//!   terjadi di UI thread (kernel sudah diserialkan `lock_kernel`, dan
//!   `KernelShape` tidak `Send`).
//! - Op dijalankan di atas state GUI lewat [`ducad_engine::SessionCore`],
//!   jadi satu batch agent = satu langkah undo GUI.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use serde_json::{json, Value};

/// Batas permintaan yang dilayani per frame agar UI tidak tersendat.
pub const MAX_REQUESTS_PER_FRAME: usize = 4;

/// Batas tunggu balasan (juga dipakai `ducad-mcp --attach`).
pub const REPLY_TIMEOUT_SECS: u64 = 120;

/// Satu permintaan dari klien soket yang menunggu dijawab UI thread.
pub struct BridgeRequest {
    pub id: u64,
    pub method: String,
    pub params: Value,
    pub reply: Sender<Value>,
}

/// Status jembatan yang dipegang aplikasi.
#[derive(Default)]
pub struct AgentBridge {
    pub enabled: bool,
    rx: Option<Receiver<BridgeRequest>>,
    stop: Option<Arc<AtomicBool>>,
    clients: Arc<AtomicUsize>,
    /// Pesan singkat untuk top bar / status.
    pub status: Option<String>,
    /// Kanal in-process untuk chat AI di aplikasi (P13.3): tidak butuh
    /// soket dan tidak tergantung sakelar Agent Bridge.
    local_rx: Option<Receiver<BridgeRequest>>,
    local_tx: Option<Sender<BridgeRequest>>,
    /// "Selalu minta persetujuan": `run_ops` (tanpa `dry_run`) dari agent
    /// mana pun dijalankan sebagai `propose_ops`.
    pub force_propose: bool,
    /// Tangkapan layar yang menunggu frame renderer (P15.2).
    pub pending_screenshot: Option<crate::live_tools::PendingScreenshot>,
}

/// `$HOME/.ducad/agent.sock` — satu folder dengan `ducad_history.db`.
pub fn socket_path() -> PathBuf {
    match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home).join(".ducad").join("agent.sock"),
        None => PathBuf::from("ducad-agent.sock"),
    }
}

impl AgentBridge {
    /// Pasang kanal permintaan tanpa membuka soket (dipakai tes).
    #[cfg(test)]
    pub(crate) fn test_channel(&mut self) -> Sender<BridgeRequest> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        self.enabled = true;
        tx
    }

    pub fn client_count(&self) -> usize {
        self.clients.load(Ordering::Relaxed)
    }

    /// Terima maksimal satu permintaan (soket dulu, lalu chat in-process);
    /// `None` bila tidak ada.
    pub fn try_recv(&self) -> Option<BridgeRequest> {
        if let Some(req) = self.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            return Some(req);
        }
        self.local_rx.as_ref()?.try_recv().ok()
    }

    /// Pengirim in-process untuk chat AI; kanal dibuat saat pertama dipakai.
    pub fn local_sender(&mut self) -> Sender<BridgeRequest> {
        if let Some(tx) = &self.local_tx {
            return tx.clone();
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.local_rx = Some(rx);
        self.local_tx = Some(tx.clone());
        tx
    }

    /// Ada sumber permintaan yang harus dilayani tiap frame.
    pub fn is_active(&self) -> bool {
        self.enabled || self.local_rx.is_some()
    }

    /// Balasan hasil tool.
    pub fn ok_reply(id: u64, payload: Value, image_png: Option<Vec<u8>>, is_error: bool) -> Value {
        use base64::Engine as _;
        let image = image_png
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
            .map(Value::String)
            .unwrap_or(Value::Null);
        json!({ "id": id, "payload": payload, "image_png": image, "is_error": is_error })
    }

    /// Balasan error (bentuknya sama, `payload.error` berisi `OpError`).
    pub fn err_reply(id: u64, e: &ducad_engine::OpError) -> Value {
        json!({ "id": id, "payload": { "error": e }, "image_png": Value::Null, "is_error": true })
    }
}

// ---------------------------------------------------------------------
// Server soket — desktop Unix saja.
// ---------------------------------------------------------------------

#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
mod unix_server {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::mpsc;

    impl AgentBridge {
        /// Nyalakan jembatan. Soket basi dihapus lebih dulu; izin 0600.
        pub fn start(&mut self, ctx: egui::Context) -> anyhow::Result<()> {
            if self.enabled {
                return Ok(());
            }
            let path = socket_path();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Soket basi (aplikasi sebelumnya berhenti mendadak) dibuang:
            // bind gagal bila berkasnya masih ada.
            let _ = std::fs::remove_file(&path);
            let listener = UnixListener::bind(&path)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            listener.set_nonblocking(true)?;

            let (tx, rx) = mpsc::channel::<BridgeRequest>();
            let stop = Arc::new(AtomicBool::new(false));
            let clients = self.clients.clone();
            let stop_thread = stop.clone();
            std::thread::spawn(move || {
                accept_loop(listener, tx, ctx, stop_thread, clients);
            });

            self.rx = Some(rx);
            self.stop = Some(stop);
            self.enabled = true;
            self.status = Some(path.display().to_string());
            Ok(())
        }

        /// Matikan jembatan dan hapus soketnya.
        pub fn shutdown(&mut self) {
            if let Some(stop) = self.stop.take() {
                stop.store(true, Ordering::Relaxed);
            }
            self.rx = None;
            self.enabled = false;
            self.clients.store(0, Ordering::Relaxed);
            self.status = None;
            let _ = std::fs::remove_file(socket_path());
        }
    }

    /// Terima koneksi sampai diminta berhenti. Listener non-blocking +
    /// tidur singkat supaya sakelar "matikan" tidak menunggu klien.
    fn accept_loop(
        listener: UnixListener,
        tx: mpsc::Sender<BridgeRequest>,
        ctx: egui::Context,
        stop: Arc<AtomicBool>,
        clients: Arc<AtomicUsize>,
    ) {
        while !stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let tx = tx.clone();
                    let ctx = ctx.clone();
                    let clients = clients.clone();
                    let stop = stop.clone();
                    std::thread::spawn(move || {
                        clients.fetch_add(1, Ordering::Relaxed);
                        serve_client(stream, tx, ctx, stop);
                        clients.fetch_sub(1, Ordering::Relaxed);
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    log::warn!("agent bridge: accept gagal: {e}");
                    break;
                }
            }
        }
        let _ = std::fs::remove_file(socket_path());
    }

    /// Satu klien: JSON per baris masuk, JSON per baris keluar.
    pub(crate) fn serve_client(
        stream: UnixStream,
        tx: mpsc::Sender<BridgeRequest>,
        ctx: egui::Context,
        stop: Arc<AtomicBool>,
    ) {
        let Ok(write_half) = stream.try_clone() else {
            return;
        };
        let _ = stream.set_nonblocking(false);
        let reader = BufReader::new(stream);
        let mut out = write_half;
        for line in reader.lines() {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let reply = handle_line(&line, &tx, &ctx);
            if serde_json::to_writer(&mut out, &reply).is_err() || out.write_all(b"\n").is_err() {
                break;
            }
            let _ = out.flush();
        }
    }

    /// Terjemahkan satu baris menjadi permintaan UI dan tunggu balasannya.
    fn handle_line(line: &str, tx: &mpsc::Sender<BridgeRequest>, ctx: &egui::Context) -> Value {
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return protocol_error(0, &format!("invalid JSON: {e}")),
        };
        let id = msg.get("id").and_then(Value::as_u64).unwrap_or(0);
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            return protocol_error(id, "permintaan butuh 'method'");
        };
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let (reply_tx, reply_rx) = mpsc::channel();
        let req = BridgeRequest {
            id,
            method: method.to_string(),
            params,
            reply: reply_tx,
        };
        if tx.send(req).is_err() {
            return protocol_error(id, "jembatan agent sudah dimatikan");
        }
        ctx.request_repaint();
        // Sedikit lebih longgar dari batas sisi klien supaya kartu proposal
        // yang kedaluwarsa sempat dijawab "rejected: timeout".
        match reply_rx.recv_timeout(std::time::Duration::from_secs(REPLY_TIMEOUT_SECS + 10)) {
            Ok(v) => v,
            Err(_) => protocol_error(id, "aplikasi tidak menjawab dalam batas waktu"),
        }
    }

    fn protocol_error(id: u64, message: &str) -> Value {
        let e = ducad_engine::OpError::new(ducad_engine::OpErrorCode::Io, message);
        AgentBridge::err_reply(id, &e)
    }
}

#[cfg(not(all(unix, not(any(target_os = "ios", target_os = "android")))))]
impl AgentBridge {
    /// Platform tanpa soket Unix (iPadOS): jembatan tidak tersedia.
    pub fn start(&mut self, _ctx: egui::Context) -> anyhow::Result<()> {
        anyhow::bail!("jembatan agent hanya tersedia di desktop")
    }

    pub fn shutdown(&mut self) {
        self.enabled = false;
    }
}

// ---------------------------------------------------------------------
// Sisi UI: eksekusi permintaan di atas state GUI.
// ---------------------------------------------------------------------

use ducad_engine::tooling::{call_core_tool, call_stateless_tool, ToolOut, ToolPaths};
use ducad_engine::{DesignDoc, OpError, OpErrorCode, OpResult, SessionCore};

use crate::app::DuCADApp;

/// Metode yang dijawab jembatan (selain tool tingkat-core).
const BRIDGE_ONLY: &[&str] = &[
    "set_params",
    "save_part",
    "propose_ops",
    "replace_op",
    "remove_op",
];

/// Metode yang sengaja TIDAK tersedia lewat jembatan; alasannya menyusul
/// di `hint` supaya agent tahu harus berbuat apa.
fn unsupported(method: &str) -> OpError {
    let hint = match method {
        "new_part" | "open_part" | "close_part" => {
            "do it in the app (File menu); the bridge works on the document that is currently open"
        }
        "accept_proposal" => "only the user can accept a proposal, with the buttons in the app",
        _ => "this method is not available in a live session",
    };
    OpError::new(
        OpErrorCode::InvalidParam,
        format!("method '{method}' is not available through the live bridge"),
    )
    .with_hint(hint)
}

/// Pagar path jembatan: relatif ke folder dokumen yang sedang terbuka,
/// atau `$HOME` bila dokumen belum pernah disimpan.
struct BridgePaths(PathBuf);

impl ToolPaths for BridgePaths {
    fn resolve(&self, p: &str) -> OpResult<PathBuf> {
        let raw = std::path::Path::new(p);
        if raw.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        }) {
            return Err(OpError::new(
                OpErrorCode::Io,
                format!("path must be relative to the document folder: {p}"),
            ));
        }
        Ok(self.0.join(raw))
    }
}

impl DuCADApp {
    /// Langkah pasca-perubahan model yang sama dengan `execute_model_command`:
    /// catat aktivitas, kosongkan seleksi, minta repaint.
    pub fn after_model_changed(&mut self, title: &str, details: &str) {
        self.record_activity(ducad_ui::ActivityKindUi::Solid3D, title, details);
        self.selected_bodies.clear();
        self.selected_edges.clear();
        self.selected_faces.clear();
        self.active_face = None;
        self.active_edge = None;
        self.active_vertex = None;
    }

    /// Nyalakan/matikan jembatan (command palette).
    pub fn toggle_agent_bridge(&mut self, ctx: &egui::Context) {
        if self.bridge.enabled {
            self.bridge.shutdown();
            self.model_status = Some(ducad_i18n::t!("bridge-off"));
            return;
        }
        // Kebijakan privasi disimpan di pengaturan chat (⚙ di panel Chat AI).
        self.chat.ensure_loaded();
        self.sync_ai_privacy();
        if self.ai.privacy == crate::assist_ui::AiPrivacy::OfflineOnly {
            self.model_status = Some(ducad_i18n::t!("bridge-blocked-offline"));
            return;
        }
        match self.bridge.start(ctx.clone()) {
            Ok(()) => self.model_status = Some(ducad_i18n::t!("bridge-on")),
            Err(e) => self.model_status = Some(format!("{}: {e}", ducad_i18n::t!("bridge-failed"))),
        }
    }

    /// Folder tempat `save_part`/`save_svg` jembatan boleh menulis.
    pub(crate) fn bridge_root(&self) -> PathBuf {
        self.current_file_path
            .as_ref()
            .and_then(|p| p.parent().map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Samakan `agent_meta` dengan geometri GUI saat ini. Sidik jari
    /// berbeda (pengguna mengedit manual atau menekan undo) → mode adopsi:
    /// body saat ini menjadi `base_bodies`, oplog dikosongkan, dan
    /// peringatan `"oplog_stale"` ditambahkan.
    pub(crate) fn sync_agent_meta(&mut self) {
        let fp = ducad_engine::session::fingerprint(&self.model);
        if self.agent_meta.design.fingerprint == fp {
            return;
        }
        let had_work = !self.agent_meta.design.oplog.is_empty()
            || !self.agent_meta.design.fingerprint.is_empty();
        let base_bodies = self
            .model
            .doc
            .bodies
            .iter()
            .filter_map(|(id, meta)| {
                let shape = &self.model.geometry.get(id)?.shape;
                Some(ducad_io::native::NativeBody {
                    name: meta.name.clone(),
                    uuid: ducad_core::new_part_uuid(),
                    visible: meta.visible,
                    material: meta.material,
                    mechanical: meta.mechanical.clone(),
                    step: shape.to_step_string().ok()?,
                    round_history: None,
                })
            })
            .collect();
        let design = DesignDoc {
            params: self.agent_meta.design.params.clone(),
            checks: self.agent_meta.design.checks.clone(),
            // Lembar gambar bukan op: tetap berlaku walau oplog diadopsi ulang.
            drawings: self.agent_meta.design.drawings.clone(),
            base_bodies,
            fingerprint: fp,
            ..DesignDoc::default()
        };
        self.agent_meta = ducad_engine::SessionMeta {
            design,
            ..Default::default()
        };
        if had_work {
            self.agent_meta.warnings.push("oplog_stale".to_string());
        }
    }

    /// Simpan `design` hasil agent ke dokumen GUI supaya ikut tersimpan ke
    /// berkas `.ducad`.
    pub(crate) fn sync_design_after_agent(&mut self) {
        self.agent_meta.design.fingerprint = ducad_engine::session::fingerprint(&self.model);
        self.design = serde_json::to_value(&self.agent_meta.design).ok();
    }

    /// Layani permintaan jembatan yang tertunda (awal `update`).
    pub fn poll_agent_bridge(&mut self, ctx: &egui::Context) {
        if !self.bridge.is_active() {
            return;
        }
        self.poll_screenshot(ctx);
        for _ in 0..MAX_REQUESTS_PER_FRAME {
            let Some(req) = self.bridge.try_recv() else {
                break;
            };
            let BridgeRequest {
                id,
                method,
                params,
                reply,
            } = req;
            // `None` = balasan menyusul (proposal menunggu keputusan
            // pengguna di kartu P8.4).
            if let Some(out) = self.agent_call(&method, params, id, &reply) {
                let _ = reply.send(AgentBridge::ok_reply(
                    id,
                    out.payload,
                    out.image_png,
                    out.is_error,
                ));
            }
            ctx.request_repaint();
        }
    }

    /// Jalankan satu metode jembatan langsung (dipakai tes proposal).
    /// Jalankan satu tool jembatan dari dalam proses tanpa klien (dipakai
    /// tutorial untuk membangun geometri awal bab). Balasan tertunda dibuang.
    pub(crate) fn agent_call_local(&mut self, method: &str, params: Value) -> Option<ToolOut> {
        let (tx, _rx) = std::sync::mpsc::channel();
        self.agent_call(method, params, 0, &tx)
    }

    #[cfg(test)]
    pub(crate) fn agent_call_for_test(
        &mut self,
        method: &str,
        params: Value,
        id: u64,
        reply: &Sender<Value>,
    ) -> Option<ToolOut> {
        self.agent_call(method, params, id, reply)
    }

    /// Jalankan satu metode jembatan. `None` = balasan ditunda.
    fn agent_call(
        &mut self,
        method: &str,
        params: Value,
        id: u64,
        reply: &Sender<Value>,
    ) -> Option<ToolOut> {
        if ducad_engine::tooling::STATELESS_TOOLS.contains(&method) {
            return Some(call_stateless_tool(method, params).unwrap_or_else(ToolOut::err));
        }
        if crate::live_tools::LIVE_TOOLS.contains(&method) {
            self.sync_agent_meta();
            return self.live_tool(method, params, id, reply);
        }
        if !ducad_engine::tooling::CORE_TOOLS.contains(&method) && !BRIDGE_ONLY.contains(&method) {
            return Some(ToolOut::err(unsupported(method)));
        }
        self.sync_agent_meta();
        let mut params = params;
        // Mode "selalu usulkan": edit oplog lama juga menunggu persetujuan.
        if matches!(method, "replace_op" | "remove_op")
            && self.bridge.force_propose
            && params.get("dry_run") != Some(&Value::Bool(true))
        {
            let proposed = match ducad_engine::tooling::parse_edit_args(method, params) {
                Ok(e) => self.agent_propose_edit(e, id, reply),
                Err(e) => Err(e),
            };
            return match proposed {
                Ok(()) => None,
                Err(e) => Some(ToolOut::err(e)),
            };
        }
        let method = if method == "run_ops"
            && self.bridge.force_propose
            && params.get("dry_run") != Some(&Value::Bool(true))
        {
            if let Some(o) = params.as_object_mut() {
                o.remove("dry_run");
            }
            "propose_ops"
        } else {
            method
        };
        match method {
            "propose_ops" => match self.agent_propose(params, id, reply) {
                Ok(()) => None,
                Err(e) => Some(ToolOut::err(e)),
            },
            "set_params" => Some(self.agent_set_params(params).unwrap_or_else(ToolOut::err)),
            "save_part" => Some(self.agent_save_part(params).unwrap_or_else(ToolOut::err)),
            "replace_op" | "remove_op" => Some(
                self.agent_edit_oplog(method, params)
                    .unwrap_or_else(ToolOut::err),
            ),
            _ => {
                let paths = BridgePaths(self.bridge_root());
                let out = {
                    let mut core = SessionCore {
                        model: &mut self.model,
                        model_undo: &mut self.model_undo,
                        sketches: &mut self.sketch_set,
                        meta: &mut self.agent_meta,
                    };
                    call_core_tool(&mut core, method, params, &paths)
                };
                let out = out.unwrap_or_else(ToolOut::err);
                if method == "set_checks" && !out.is_error {
                    // Checks bagian dari design → ikut tersimpan ke .ducad.
                    self.sync_design_after_agent();
                }
                if method == "import_step" && !out.is_error {
                    self.sync_design_after_agent();
                    self.after_model_changed(&ducad_i18n::t!("bridge-title"), "import_step");
                }
                if method == "run_ops" && !out.is_error {
                    let committed = out.payload["committed"] == Value::Bool(true);
                    if committed {
                        let n = out.payload["outcomes"]
                            .as_array()
                            .map(|a| a.len())
                            .unwrap_or(0);
                        self.sync_design_after_agent();
                        let count = n.to_string();
                        let title = ducad_i18n::t!("bridge-activity", count = count.as_str());
                        let detail = self.agent_meta.design.oplog.len().to_string();
                        self.after_model_changed(&title, &detail);
                    }
                }
                Some(out)
            }
        }
    }

    /// `set_params`: params diganti lalu seluruh oplog di-replay di sesi
    /// sementara; hasilnya diadopsi sebagai SATU langkah undo GUI.
    #[allow(clippy::result_large_err)]
    fn agent_set_params(&mut self, params: Value) -> OpResult<ToolOut> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct A {
            #[serde(default)]
            session: Option<String>,
            params: ducad_engine::ops::Params,
        }
        let a: A = ducad_engine::tooling::args(params)?;
        let _ = a.session;
        let mut merged = self.agent_meta.design.params.clone();
        merged.extend(a.params);
        let design = DesignDoc {
            params: merged,
            ..self.agent_meta.design.clone()
        };
        let session = ducad_engine::Session::replay(DesignDoc {
            fingerprint: String::new(),
            ..design
        })?;
        // `outcomes` kosong: params diganti lalu SELURUH oplog di-replay,
        // jadi tidak ada "op baru" yang bisa dilaporkan satu per satu —
        // keadaan akhir ada di `summary` dan `checks`.
        let checks = session.run_checks(None);
        let report = ducad_engine::tooling::to_value(ducad_engine::BatchReport {
            summary: session.summary(),
            committed: true,
            outcomes: Vec::new(),
            error: None,
            checks: Some(checks.results),
        })?;
        let meta = session.meta().clone();
        let model = session.into_model();
        self.adopt_agent_model(model, meta, "set_params");
        Ok(ToolOut::ok(report))
    }

    /// `replace_op` / `remove_op`: edit oplog, replay di sesi salinan, lalu
    /// adopsi sebagai SATU langkah undo GUI (`dry_run`: hanya laporan).
    #[allow(clippy::result_large_err)]
    pub(crate) fn agent_edit_oplog(&mut self, method: &str, params: Value) -> OpResult<ToolOut> {
        let e = ducad_engine::tooling::parse_edit_args(method, params)?;
        // Id op yang tidak dikenal → `Err` (sama dengan server MCP).
        ducad_engine::edit_design(&self.agent_meta.design, None, &e.replace, &e.remove, &[])?;
        let preview = match ducad_engine::preview_edit(
            &self.model,
            &self.agent_meta.design,
            None,
            &e.replace,
            &e.remove,
            &[],
        ) {
            Ok(p) => p,
            // Replay gagal → laporan gagal berbentuk BatchReport; model utuh.
            Err(err) => {
                let core = SessionCore {
                    model: &mut self.model,
                    model_undo: &mut self.model_undo,
                    sketches: &mut self.sketch_set,
                    meta: &mut self.agent_meta,
                };
                let summary = ducad_engine::inspect::summarize_core(
                    &core,
                    None,
                    false,
                    ducad_engine::inspect::DEFAULT_TOPOLOGY_LIMIT,
                )?;
                let report = ducad_engine::BatchReport {
                    committed: false,
                    outcomes: Vec::new(),
                    error: Some(err),
                    summary,
                    checks: None,
                };
                return Ok(ToolOut {
                    payload: ducad_engine::tooling::to_value(report)?,
                    image_png: None,
                    is_error: true,
                });
            }
        };
        let mut report = preview.report;
        if e.dry_run {
            return Ok(ToolOut::ok(ducad_engine::tooling::to_value(report)?));
        }
        report.committed = true;
        let meta = preview.session.meta().clone();
        self.adopt_agent_model(preview.session.into_model(), meta, method);
        Ok(ToolOut::ok(ducad_engine::tooling::to_value(report)?))
    }

    /// Ganti seluruh model GUI dengan hasil replay, satu langkah undo.
    pub(crate) fn adopt_agent_model(
        &mut self,
        model: ducad_engine::model::ModelDoc,
        meta: ducad_engine::SessionMeta,
        what: &str,
    ) {
        self.model_undo.execute(
            Box::new(crate::modeling::SwapModelCommand::new(model)),
            &mut self.model,
        );
        self.agent_meta = meta;
        self.sync_design_after_agent();
        self.after_model_changed(&ducad_i18n::t!("bridge-title"), what);
    }

    /// `save_part`: simpan dokumen GUI ke `.ducad` (path relatif terhadap
    /// folder dokumen; tanpa path memakai path dokumen saat ini).
    #[allow(clippy::result_large_err)]
    fn agent_save_part(&mut self, params: Value) -> OpResult<ToolOut> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct A {
            #[serde(default)]
            session: Option<String>,
            #[serde(default)]
            path: Option<String>,
        }
        let a: A = ducad_engine::tooling::args(params)?;
        let _ = a.session;
        let paths = BridgePaths(self.bridge_root());
        let path = match a.path.as_deref() {
            Some(p) => paths.resolve(p)?,
            None => self.current_file_path.clone().ok_or_else(|| {
                OpError::invalid("the document has no path yet; pass the 'path' argument")
            })?,
        };
        self.sync_design_after_agent();
        self.save_native_to(path.clone());
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Ok(ToolOut::ok(json!({ "path": path, "bytes": bytes })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_shapes() {
        let ok = AgentBridge::ok_reply(7, json!({ "a": 1 }), Some(vec![1, 2, 3]), false);
        assert_eq!(ok["id"], 7);
        assert_eq!(ok["payload"]["a"], 1);
        assert_eq!(ok["image_png"], "AQID");
        assert_eq!(ok["is_error"], false);

        let e = ducad_engine::OpError::new(ducad_engine::OpErrorCode::Io, "gagal");
        let err = AgentBridge::err_reply(8, &e);
        assert_eq!(err["is_error"], true);
        assert_eq!(err["payload"]["error"]["code"], "io");
    }

    #[test]
    fn socket_lives_next_to_history_db() {
        let p = socket_path();
        assert!(p.ends_with("agent.sock"), "{}", p.display());
    }

    /// Alur penuh lewat sepasang soket: agent mengirim `run_ops(plate)` →
    /// aplikasi punya 1 body; undo GUI sekali → 0 body; permintaan
    /// berikutnya melaporkan `oplog_stale` (mode adopsi).
    #[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
    #[test]
    fn run_ops_then_gui_undo_reports_stale() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixStream;

        let (client, app_side) = UnixStream::pair().expect("pasangan soket");
        let mut app = crate::app::DuCADApp::new_for_test();
        let ctx = egui::Context::default();
        let tx = app.bridge.test_channel();
        let stop = Arc::new(AtomicBool::new(false));
        let ctx_thread = ctx.clone();
        let stop_thread = stop.clone();
        let server = std::thread::spawn(move || {
            super::unix_server::serve_client(app_side, tx, ctx_thread, stop_thread);
        });

        let mut writer = client.try_clone().expect("klon soket");
        let mut reader = BufReader::new(client);
        let send = |writer: &mut UnixStream, line: String| {
            writer.write_all(line.as_bytes()).expect("kirim");
            writer.write_all(b"\n").expect("kirim");
            writer.flush().expect("flush");
        };
        // Pompa UI sampai satu balasan tiba (permintaan dijalankan di
        // "UI thread" = thread tes ini).
        let pump = |app: &mut crate::app::DuCADApp, reader: &mut BufReader<UnixStream>| {
            let mut line = String::new();
            // Batas waktu dinding, bukan jumlah iterasi: op kernel di build
            // debug bisa lambat saat tes lain berjalan paralel.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
            while std::time::Instant::now() < deadline {
                app.poll_agent_bridge(&ctx);
                let _ = reader.get_ref().set_nonblocking(true);
                // Soket non-blocking bisa memberi baris sepotong-sepotong:
                // kumpulkan sampai `\n` sebelum di-parse.
                match reader.read_line(&mut line) {
                    Ok(_) if line.ends_with('\n') => break,
                    _ => std::thread::sleep(std::time::Duration::from_millis(1)),
                }
            }
            let _ = reader.get_ref().set_nonblocking(false);
            serde_json::from_str::<Value>(&line).unwrap_or(Value::Null)
        };

        let plate: Value =
            serde_json::from_str(ducad_engine::ops::EXAMPLE_PLATE).expect("contoh plate");
        send(
            &mut writer,
            json!({ "id": 1, "method": "set_params", "params": { "params": plate["params"] } })
                .to_string(),
        );
        let r = pump(&mut app, &mut reader);
        assert_eq!(r["is_error"], false, "{r}");

        send(
            &mut writer,
            json!({ "id": 2, "method": "run_ops", "params": { "ops": plate["ops"] } }).to_string(),
        );
        let r = pump(&mut app, &mut reader);
        assert_eq!(r["is_error"], false, "{r}");
        assert_eq!(r["payload"]["committed"], true, "{r}");
        assert_eq!(app.model.doc.bodies.len(), 1);

        // Pengguna menekan undo di GUI.
        app.model_undo.undo(&mut app.model);
        assert_eq!(app.model.doc.bodies.len(), 0);

        send(
            &mut writer,
            json!({ "id": 3, "method": "inspect", "params": {} }).to_string(),
        );
        let r = pump(&mut app, &mut reader);
        let warnings = r["payload"]["warnings"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            warnings.iter().any(|w| w == "oplog_stale"),
            "permintaan setelah undo GUI harus melaporkan oplog_stale: {r}"
        );

        // Thread pelayan berhenti saat soket klien tertutup (EOF); kedua
        // klon harus dibuang, kalau tidak `join` menggantung.
        stop.store(true, Ordering::Relaxed);
        drop(writer);
        drop(reader);
        let _ = server.join();
    }
}
