use ducad_kernel::PickRay;
use eframe::egui;

use crate::app::DuCADApp;
use crate::types::{PickMode, PickedEdge};
use crate::viewport::{pixel_tolerance_to_world, screen_to_ray};

impl DuCADApp {
    /// Klik viewport saat `picking_mode` aktif (Fase 8)
    pub fn handle_3d_picking(&mut self, response: &egui::Response, rect: egui::Rect) {
        if !response.clicked() {
            return;
        }
        let Some(pos) = response.interact_pointer_pos() else {
            return;
        };
        if self.picking_mode == PickMode::Face {
            self.pick_face_for_tool(rect, pos);
            return;
        }
        let Some(&id) = self
            .selected_bodies
            .iter()
            .next()
            .filter(|_| self.selected_bodies.len() == 1)
        else {
            return;
        };
        let Some(body) = self.model.doc.bodies.get(id) else {
            return;
        };
        if !body.visible {
            return;
        }
        let Some(geo) = self.model.geometry.get(id) else {
            return;
        };
        let (origin, dir) = screen_to_ray(&self.camera, rect, pos);
        let ray = PickRay {
            origin: (origin.x as f64, origin.y as f64, origin.z as f64),
            dir: (dir.x as f64, dir.y as f64, dir.z as f64),
        };
        match self.picking_mode {
            PickMode::None => {}
            PickMode::Edge => {
                let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
                if let Some((_, polyline)) = ducad_kernel::pick_edge(&geo.shape, ray, tol) {
                    self.selected_edges.push(PickedEdge { ray, polyline });
                }
            }
            // Ditangani `pick_face_for_tool` di atas.
            PickMode::Face => {}
        }
    }

    /// Klik face saat mode pilih-face aktif (Shell, Draft, Split, Rib).
    ///
    /// Face boleh di sisi mana pun dan di body mana pun yang terlihat: body
    /// yang terkena otomatis menjadi body terpilih, jadi pengguna tidak perlu
    /// memilih body lebih dulu. Klik ulang face yang sama membatalkan
    /// pilihannya; klik face lain di body yang sama menambah pilihan.
    pub fn pick_face_for_tool(&mut self, rect: egui::Rect, pos: egui::Pos2) {
        let Some((id, ray, hit)) = self.pick_body_face_at_cursor(rect, pos) else {
            return;
        };
        let same_body = self.selected_bodies.len() == 1 && self.selected_bodies.contains(&id);
        if !same_body {
            self.selected_bodies.clear();
            self.selected_bodies.insert(id);
            self.selected_faces.clear();
            self.active_face = None;
        }
        let Some(geo) = self.model.geometry.get(id) else {
            return;
        };
        let already = hit.face_index.and_then(|idx| {
            self.selected_faces.iter().position(|r| {
                ducad_kernel::pick_face_details(&geo.shape, *r).and_then(|h| h.face_index)
                    == Some(idx)
            })
        });
        match already {
            Some(i) => {
                self.selected_faces.remove(i);
                self.active_face = self.selected_faces.last().and_then(|r| {
                    ducad_kernel::pick_face_details(&geo.shape, *r).map(|h| (id, *r, h))
                });
            }
            None => {
                self.selected_faces.push(ray);
                self.active_face = Some((id, ray, hit));
            }
        }
    }
}
