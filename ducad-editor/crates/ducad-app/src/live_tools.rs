//! Tool khusus sesi live (P15.2): hanya bermakna saat agent terhubung ke
//! aplikasi yang sedang terbuka (jembatan soket atau chat AI di aplikasi).
//! Agent bisa melihat apa yang dilihat pengguna (kamera, seleksi,
//! tangkapan layar) dan mengarahkan tampilan.

// `OpError` sengaja kaya konteks (kontrak JSON, sama dengan ducad-engine).
#![allow(clippy::result_large_err)]

use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use ducad_engine::tooling::{args, ToolOut};
use ducad_engine::{OpError, OpErrorCode, OpResult};
use ducad_kernel::PickRay;
use ducad_render::ViewPreset;
use glam::{DVec3, Vec3};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::agent_bridge::AgentBridge;
use crate::app::DuCADApp;

/// Nama tool live (dipakai jembatan untuk dispatch).
pub const LIVE_TOOLS: &[&str] = &[
    "document_info",
    "get_view",
    "set_view",
    "get_selection",
    "select",
    "screenshot",
];

/// Batas tunggu tangkapan layar dari renderer.
const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(10);
/// Lebar maksimum PNG tangkapan layar (hemat token model).
const SCREENSHOT_MAX_W: u32 = 1280;

/// Permintaan tangkapan layar yang menunggu frame berikutnya.
pub struct PendingScreenshot {
    pub id: u64,
    pub reply: Sender<Value>,
    pub deadline: Instant,
    pub requested: bool,
}

fn preset(name: &str) -> OpResult<ViewPreset> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "iso" | "isometric" => ViewPreset::Isometric,
        "front" => ViewPreset::Front,
        "back" => ViewPreset::Back,
        "left" => ViewPreset::Left,
        "right" => ViewPreset::Right,
        "top" => ViewPreset::Top,
        "bottom" => ViewPreset::Bottom,
        other => {
            return Err(OpError::invalid(format!(
                "unknown view '{other}' (iso, front, back, left, right, top, bottom)"
            )))
        }
    })
}

fn png_from_color_image(img: &egui::ColorImage) -> Option<Vec<u8>> {
    let [w, h] = img.size;
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    let mut rgba = image::RgbaImage::from_raw(w as u32, h as u32, bytes)?;
    if rgba.width() > SCREENSHOT_MAX_W {
        let nh =
            (rgba.height() as f64 * SCREENSHOT_MAX_W as f64 / rgba.width() as f64).round() as u32;
        rgba = image::imageops::resize(
            &rgba,
            SCREENSHOT_MAX_W,
            nh.max(1),
            image::imageops::FilterType::Triangle,
        );
    }
    let mut out = std::io::Cursor::new(Vec::new());
    rgba.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

impl DuCADApp {
    /// Nama body dari id-nya.
    fn body_name_of(&self, id: ducad_core::BodyId) -> Option<String> {
        self.model.doc.bodies.get(id).map(|b| b.name.clone())
    }

    fn body_id_by_name(&self, name: &str) -> OpResult<ducad_core::BodyId> {
        self.model
            .doc
            .bodies
            .iter()
            .find(|(_, b)| b.name == name)
            .map(|(id, _)| id)
            .ok_or_else(|| {
                let known: Vec<String> = self
                    .model
                    .doc
                    .bodies
                    .values()
                    .map(|b| b.name.clone())
                    .collect();
                OpError::new(
                    OpErrorCode::UnknownRef,
                    format!("body '{name}' does not exist (available: {known:?})"),
                )
            })
    }

    /// Bbox gabungan body terlihat (atau `names`) dari mesh.
    fn bodies_bbox(&self, names: Option<&[String]>) -> Option<(Vec3, Vec3)> {
        let mut acc: Option<(Vec3, Vec3)> = None;
        for (id, b) in self.model.doc.bodies.iter() {
            let wanted = match names {
                Some(n) => n.contains(&b.name),
                None => b.visible,
            };
            if !wanted {
                continue;
            }
            let Some(g) = self.model.geometry.get(id) else {
                continue;
            };
            let Some((mn, mx)) = g.mesh.bounding_box() else {
                continue;
            };
            let (mn, mx) = (Vec3::from(mn), Vec3::from(mx));
            acc = Some(match acc {
                Some((a, b)) => (a.min(mn), b.max(mx)),
                None => (mn, mx),
            });
        }
        acc
    }

    fn view_json(&self) -> Value {
        json!({
            "yaw_deg": self.camera.yaw.to_degrees(),
            "pitch_deg": self.camera.pitch.to_degrees(),
            "distance": self.camera.distance,
            "target": self.camera.target.to_array(),
            "mode": self.app_mode.label(),
        })
    }

    /// Jalankan satu tool live. `None` = balasan ditunda (screenshot).
    pub(crate) fn live_tool(
        &mut self,
        method: &str,
        params: Value,
        id: u64,
        reply: &Sender<Value>,
    ) -> Option<ToolOut> {
        let r = match method {
            "document_info" => self.live_document_info(),
            "get_view" => Ok(ToolOut::ok(self.view_json())),
            "set_view" => self.live_set_view(params),
            "get_selection" => self.live_get_selection(),
            "select" => self.live_select(params),
            "screenshot" => {
                if self.bridge.pending_screenshot.is_some() {
                    Err(OpError::invalid(
                        "another screenshot is still in progress; try again",
                    ))
                } else {
                    self.bridge.pending_screenshot = Some(PendingScreenshot {
                        id,
                        reply: reply.clone(),
                        deadline: Instant::now() + SCREENSHOT_TIMEOUT,
                        requested: false,
                    });
                    return None;
                }
            }
            other => Err(OpError::invalid(format!("unknown live tool: {other}"))),
        };
        Some(r.unwrap_or_else(ToolOut::err))
    }

    /// Minta/terima tangkapan layar (dipanggil tiap frame dari
    /// `poll_agent_bridge`).
    pub(crate) fn poll_screenshot(&mut self, ctx: &egui::Context) {
        let Some(p) = self.bridge.pending_screenshot.as_mut() else {
            return;
        };
        if !p.requested {
            p.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            ctx.request_repaint();
            return;
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let p = self.bridge.pending_screenshot.take().expect("ada");
            let out = match png_from_color_image(&img) {
                Some(png) => AgentBridge::ok_reply(
                    p.id,
                    json!({ "width": img.size[0], "height": img.size[1], "view": self.view_json() }),
                    Some(png),
                    false,
                ),
                None => AgentBridge::err_reply(
                    p.id,
                    &OpError::new(OpErrorCode::Io, "failed to encode PNG"),
                ),
            };
            let _ = p.reply.send(out);
        } else if Instant::now() >= p.deadline {
            let p = self.bridge.pending_screenshot.take().expect("ada");
            let e = OpError::new(OpErrorCode::Io, "the renderer did not deliver a screenshot")
                .with_hint("use render_view for a view image without the app window");
            let _ = p.reply.send(AgentBridge::err_reply(p.id, &e));
        } else {
            ctx.request_repaint();
        }
    }

    fn live_document_info(&mut self) -> OpResult<ToolOut> {
        let bodies: Vec<Value> = self
            .model
            .doc
            .bodies
            .iter()
            .map(|(id, b)| {
                let volume = self.model.geometry.get(id).map(|g| g.shape.volume().abs());
                json!({ "name": b.name, "visible": b.visible, "volume": volume,
                        "selected": self.selected_bodies.contains(&id) })
            })
            .collect();
        Ok(ToolOut::ok(json!({
            "file": self.current_file_path,
            "mode": self.app_mode.label(),
            "units": "mm",
            "bodies": bodies,
            "oplog_len": self.agent_meta.design.oplog.len(),
            "params": self.agent_meta.design.params,
            "selection": {
                "bodies": self.selected_bodies.len(),
                "faces": self.selected_faces.len(),
                "edges": self.selected_edges.len(),
            },
            "view": self.view_json(),
        })))
    }

    fn live_set_view(&mut self, params: Value) -> OpResult<ToolOut> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct A {
            #[serde(default)]
            session: Option<String>,
            #[serde(default)]
            view: Option<String>,
            #[serde(default = "yes")]
            fit: bool,
            #[serde(default)]
            bodies: Option<Vec<String>>,
        }
        fn yes() -> bool {
            true
        }
        let a: A = args(params)?;
        let _ = a.session;
        if let Some(names) = &a.bodies {
            for n in names {
                self.body_id_by_name(n)?;
            }
        }
        self.camera_animation = None;
        if let Some(v) = &a.view {
            self.camera.set_preset(preset(v)?);
        }
        if a.fit {
            if let Some((mn, mx)) = self.bodies_bbox(a.bodies.as_deref()) {
                let center = (mn + mx) * 0.5;
                let radius = ((mx - mn).length() * 0.5).max(1.0);
                self.camera.target = center;
                self.camera.distance = radius / (self.camera.fov_y * 0.5).sin() * 1.15;
            }
        }
        Ok(ToolOut::ok(self.view_json()))
    }

    fn live_get_selection(&mut self) -> OpResult<ToolOut> {
        let bodies: Vec<String> = self
            .selected_bodies
            .iter()
            .filter_map(|id| self.body_name_of(*id))
            .collect();
        let mut faces = Vec::new();
        for ray in &self.selected_faces {
            // Face terdekat di sepanjang ray, dari semua body.
            let mut best: Option<(f64, String, ducad_kernel::FaceHit)> = None;
            for (id, g) in self.model.geometry.iter() {
                if let Some(hit) = ducad_kernel::pick_face_details(&g.shape, *ray) {
                    let d = DVec3::from(hit.hit_point).distance(DVec3::from(ray.origin));
                    if best.as_ref().is_none_or(|b| d < b.0) {
                        let name = self.body_name_of(id).unwrap_or_default();
                        best = Some((d, name, hit));
                    }
                }
            }
            if let Some((_, body, hit)) = best {
                faces.push(json!({
                    "body": body,
                    "point": [hit.hit_point.0, hit.hit_point.1, hit.hit_point.2],
                    "normal": [hit.normal.0, hit.normal.1, hit.normal.2],
                    "kind": format!("{:?}", hit.surface_kind),
                }));
            }
        }
        let edges: Vec<Value> = self
            .selected_edges
            .iter()
            .filter_map(|e| {
                let (a, b) = (e.polyline.first()?, e.polyline.last()?);
                Some(json!({ "start": [a.0, a.1, a.2], "end": [b.0, b.1, b.2] }))
            })
            .collect();
        Ok(ToolOut::ok(
            json!({ "bodies": bodies, "faces": faces, "edges": edges }),
        ))
    }

    fn live_select(&mut self, params: Value) -> OpResult<ToolOut> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct A {
            #[serde(default)]
            session: Option<String>,
            #[serde(default)]
            body: Option<String>,
            #[serde(default)]
            faces: Option<String>,
            #[serde(default)]
            edges: Option<String>,
            /// Tambahkan ke seleksi yang ada (bawaan: ganti).
            #[serde(default)]
            add: bool,
        }
        let a: A = args(params)?;
        let _ = a.session;
        if !a.add {
            self.selected_bodies.clear();
            self.selected_faces.clear();
            self.selected_edges.clear();
            self.active_face = None;
            self.active_edge = None;
        }
        let Some(name) = a.body else {
            if a.faces.is_some() || a.edges.is_some() {
                return Err(OpError::invalid("'faces'/'edges' require 'body'"));
            }
            return self.live_get_selection();
        };
        let bid = self.body_id_by_name(&name)?;
        let shape = match self.model.geometry.get(bid) {
            Some(g) => {
                ducad_kernel::clone_shape(&g.shape).map_err(|e| OpError::kernel("select", e))?
            }
            None => {
                return Err(OpError::new(
                    OpErrorCode::UnknownRef,
                    format!("body '{name}' has no geometry"),
                ))
            }
        };
        let center = ducad_kernel::advanced::faces_centroid(&shape);
        let mut skipped = 0usize;
        match (&a.faces, &a.edges) {
            (None, None) => {
                self.selected_bodies.insert(bid);
            }
            (Some(sel), _) => {
                let idx = ducad_engine::select::select_faces(&shape, sel)?;
                let all = ducad_kernel::enumerate_faces(&shape);
                for i in idx {
                    let f = &all[i];
                    if f.kind != ducad_kernel::SurfaceKind::Plane {
                        skipped += 1;
                        continue;
                    }
                    let n = DVec3::from(f.normal);
                    let c = DVec3::from(f.centroid);
                    let o = c + n * 0.5;
                    self.selected_faces.push(PickRay {
                        origin: o.into(),
                        dir: (-n).into(),
                    });
                }
            }
            (None, Some(sel)) => {
                let idx = ducad_engine::select::select_edges(&shape, sel)?;
                let all = ducad_kernel::enumerate_edges(&shape);
                for i in idx {
                    let e = &all[i];
                    let mid = DVec3::from(e.mid);
                    let out = (mid - center).try_normalize().unwrap_or(DVec3::Z);
                    let o = mid + out;
                    self.selected_edges.push(crate::types::PickedEdge {
                        ray: PickRay {
                            origin: o.into(),
                            dir: (-out).into(),
                        },
                        polyline: vec![e.start.into(), e.mid.into(), e.end.into()],
                    });
                }
            }
        }
        let mut out = self.live_get_selection()?;
        if skipped > 0 {
            out.payload["warning"] = json!(format!(
                "{skipped} curved face(s) skipped: live selection supports planar faces only"
            ));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(app: &mut DuCADApp, method: &str, params: Value) -> ToolOut {
        let (tx, _rx) = std::sync::mpsc::channel();
        app.agent_call_for_test(method, params, 1, &tx)
            .expect("balasan langsung")
    }

    fn app_with_block() -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        let out = call(
            &mut app,
            "run_ops",
            json!({ "ops": [{ "op": "primitive", "id": "blok", "shape": { "box": { "size": [40, 20, 10] } } }] }),
        );
        assert!(!out.is_error, "{}", out.payload);
        app
    }

    #[test]
    fn document_info_and_set_view_fit() {
        let mut app = app_with_block();
        let info = call(&mut app, "document_info", json!({}));
        assert_eq!(info.payload["bodies"][0]["name"], "blok");
        assert!((info.payload["bodies"][0]["volume"].as_f64().unwrap() - 8000.0).abs() < 1e-6);
        let v = call(&mut app, "set_view", json!({ "view": "top" }));
        assert!(!v.is_error, "{}", v.payload);
        assert!((v.payload["pitch_deg"].as_f64().unwrap() - 89.0).abs() < 0.01);
        let t = v.payload["target"].as_array().unwrap();
        assert!(
            (t[0].as_f64().unwrap() - 20.0).abs() < 1e-3,
            "fit ke pusat bbox: {t:?}"
        );
        assert!(call(&mut app, "set_view", json!({ "view": "atas" })).is_error);
        assert!(call(&mut app, "set_view", json!({ "bodies": ["tidak_ada"] })).is_error);
    }

    #[test]
    fn select_by_selector_and_read_back() {
        let mut app = app_with_block();
        let r = call(&mut app, "select", json!({ "body": "blok", "faces": ">Z" }));
        assert!(!r.is_error, "{}", r.payload);
        let face = &r.payload["faces"][0];
        assert_eq!(face["body"], "blok");
        assert!(
            (face["normal"][2].as_f64().unwrap() - 1.0).abs() < 1e-6,
            "{face}"
        );
        assert!(
            (face["point"][2].as_f64().unwrap() - 10.0).abs() < 1e-3,
            "{face}"
        );

        let r = call(
            &mut app,
            "select",
            json!({ "body": "blok", "edges": "|Z", "add": true }),
        );
        assert_eq!(r.payload["edges"].as_array().unwrap().len(), 4);
        assert_eq!(
            r.payload["faces"].as_array().unwrap().len(),
            1,
            "add mempertahankan face"
        );

        let r = call(&mut app, "select", json!({ "body": "blok" }));
        assert_eq!(r.payload["bodies"], json!(["blok"]));
        assert!(r.payload["faces"].as_array().unwrap().is_empty());
        let r = call(&mut app, "select", json!({}));
        assert!(r.payload["bodies"].as_array().unwrap().is_empty());
    }

    #[test]
    fn screenshot_times_out_without_renderer() {
        let mut app = app_with_block();
        let ctx = egui::Context::default();
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(
            app.agent_call_for_test("screenshot", json!({}), 9, &tx)
                .is_none(),
            "balasan ditunda"
        );
        app.poll_screenshot(&ctx);
        app.bridge.pending_screenshot.as_mut().unwrap().deadline = Instant::now();
        app.poll_screenshot(&ctx);
        let reply = rx.try_recv().expect("balasan error");
        assert_eq!(reply["is_error"], true);
        assert!(app.bridge.pending_screenshot.is_none());
    }
}
