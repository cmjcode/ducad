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
            PickMode::Face => {
                if let Some(hit) = ducad_kernel::pick_face_details(&geo.shape, ray) {
                    self.selected_faces.push(ray);
                    self.active_face = Some((id, ray, hit));
                }
            }
        }
    }
}
