//! Studi gerak lanjutan (Paritas 4.9): plot nilai joint & kecepatan titik,
//! jejak (trace) titik terpilih di viewport, ekspor animasi (urutan PNG +
//! CSV), dan interferensi per frame.
//!
//! Sampel dihitung pada salinan pohon perakitan, jadi model tidak berubah
//! saat dianalisis. Interferensi per frame memakai geometri yang dipindah
//! sesuai pose tiap frame.

use std::collections::HashMap;

use ducad_core::assembly::{AssemblyInstanceId, AssemblyTree};
use glam::{DQuat, DVec3};

use crate::app::DuCADApp;

/// Satu sampel studi gerak.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionSample {
    pub t: f64,
    /// Nilai joint/mate yang digerakkan.
    pub value: f64,
    /// Posisi titik yang dilacak (dunia, mm).
    pub point: DVec3,
    /// Kecepatan titik (mm per satuan t).
    pub speed: f64,
    /// Jumlah pasangan body yang saling menembus pada frame ini.
    pub clashes: usize,
}

/// Status jendela studi gerak.
pub struct MotionToolsState {
    pub open: bool,
    pub study: usize,
    pub samples_n: u32,
    pub tracked: Option<AssemblyInstanceId>,
    pub check_interference: bool,
    pub show_trace: bool,
    pub samples: Vec<MotionSample>,
}

impl Default for MotionToolsState {
    fn default() -> Self {
        Self {
            open: false,
            study: 0,
            samples_n: 30,
            tracked: None,
            check_interference: true,
            show_trace: true,
            samples: Vec::new(),
        }
    }
}

fn pose_of(tree: &AssemblyTree, id: AssemblyInstanceId) -> Option<(DVec3, DQuat)> {
    tree.instance_world_transform(id)
}

/// Pindah shape dari pose `from` ke `to` (konvensi solver).
fn moved_shape(
    shape: &ducad_kernel::KernelShape,
    from: (DVec3, DQuat),
    to: (DVec3, DQuat),
) -> Option<ducad_kernel::KernelShape> {
    let dq = to.1 * from.1.inverse();
    let (axis, angle) = dq.to_axis_angle();
    let dt = to.0 - from.0;
    if dt.length() < 1e-12 && angle.abs() < 1e-12 {
        return ducad_kernel::clone_shape(shape).ok();
    }
    let tf = ducad_kernel::MateTransformResult {
        translation: (dt.x, dt.y, dt.z),
        pivot: (from.0.x, from.0.y, from.0.z),
        axis: (axis.x, axis.y, axis.z),
        angle_rad: angle,
    };
    ducad_kernel::apply_mate_transform_to_shape(shape, &tf).ok()
}

impl DuCADApp {
    /// Hitung sampel studi gerak ke-`study` (N+1 frame).
    pub fn compute_motion_samples(
        &mut self,
        study: usize,
        n: u32,
        tracked: Option<AssemblyInstanceId>,
        interference: bool,
    ) -> Result<Vec<MotionSample>, String> {
        self.sync_assembly_instances();
        let Some(st) = self.assembly_tree.motion_studies.get(study).cloned() else {
            return Err(ducad_i18n::t!("motion-no-study"));
        };
        let n = n.clamp(2, 500);
        let base_tree = self.assembly_tree.clone();
        let start: HashMap<AssemblyInstanceId, (DVec3, DQuat)> = base_tree
            .instances
            .keys()
            .filter_map(|id| pose_of(&base_tree, *id).map(|p| (*id, p)))
            .collect();
        // Titik dilacak: pusat bbox body instance (relatif pose awalnya).
        let tracked = tracked.or_else(|| {
            base_tree
                .mates
                .get(&st.driven_mate)
                .map(|m| m.target_b.instance_id)
        });
        let local_pt = tracked.and_then(|id| {
            let body = self.instance_body(id)?;
            let (lo, hi) = self.model.geometry.get(body)?.shape.bounding_box()?;
            let c = (DVec3::from(lo) + DVec3::from(hi)) * 0.5;
            let p0 = *start.get(&id)?;
            Some(p0.1.inverse() * (c - p0.0))
        });
        let shapes: Vec<(AssemblyInstanceId, u64, String, ducad_kernel::KernelShape)> =
            if interference {
                base_tree
                    .instances
                    .values()
                    .filter(|i| i.visible)
                    .filter_map(|i| {
                        let body = self.instance_body(i.id)?;
                        let g = self.model.geometry.get(body)?;
                        let sh = ducad_kernel::clone_shape(&g.shape).ok()?;
                        Some((i.id, i.body_id_raw, i.name.clone(), sh))
                    })
                    .collect()
            } else {
                Vec::new()
            };
        let mut out: Vec<MotionSample> = Vec::with_capacity(n as usize + 1);
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let mut tree = base_tree.clone();
            if ducad_kernel::evaluate_motion(&mut tree, &st, t).is_none() {
                return Err(ducad_i18n::t!("motion-not-numeric"));
            }
            let point = match (tracked, local_pt) {
                (Some(id), Some(lp)) => pose_of(&tree, id)
                    .map(|(pt, q)| pt + q * lp)
                    .unwrap_or(DVec3::ZERO),
                _ => DVec3::ZERO,
            };
            let speed = out
                .last()
                .map(|prev| (point - prev.point).length() * n as f64)
                .unwrap_or(0.0);
            let clashes = if interference {
                let moved: Vec<(u64, String, ducad_kernel::KernelShape)> = shapes
                    .iter()
                    .filter_map(|(id, raw, name, sh)| {
                        let from = *start.get(id)?;
                        let to = pose_of(&tree, *id)?;
                        Some((*raw, name.clone(), moved_shape(sh, from, to)?))
                    })
                    .collect();
                let refs: Vec<(u64, String, &ducad_kernel::KernelShape)> =
                    moved.iter().map(|(r, n, s)| (*r, n.clone(), s)).collect();
                ducad_kernel::detect_interference(&refs, 1e-3).len()
            } else {
                0
            };
            out.push(MotionSample {
                t,
                value: st.value_at(t),
                point,
                speed,
                clashes,
            });
        }
        if let Some(first) = out.get(1).map(|s| s.speed) {
            if let Some(s0) = out.first_mut() {
                s0.speed = first;
            }
        }
        Ok(out)
    }

    /// CSV sampel: t, nilai, x, y, z, kecepatan, jumlah tabrakan.
    pub fn motion_csv(samples: &[MotionSample]) -> String {
        let mut s = String::from("t,value,x,y,z,speed,clashes\n");
        for m in samples {
            s.push_str(&format!(
                "{:.4},{:.6},{:.6},{:.6},{:.6},{:.6},{}\n",
                m.t, m.value, m.point.x, m.point.y, m.point.z, m.speed, m.clashes
            ));
        }
        s
    }

    /// Ekspor animasi: `frame_000.png` … ke folder `dir` + `motion.csv`.
    /// Model digerakkan per frame lalu dikembalikan ke keadaan awal.
    pub fn export_motion_frames(
        &mut self,
        study: usize,
        n: u32,
        dir: &std::path::Path,
    ) -> Result<usize, String> {
        let Some(st) = self.assembly_tree.motion_studies.get(study).cloned() else {
            return Err(ducad_i18n::t!("motion-no-study"));
        };
        if ducad_kernel::evaluate_motion(&mut self.assembly_tree.clone(), &st, 0.0).is_none() {
            return Err(ducad_i18n::t!("motion-not-numeric"));
        }
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let n = n.clamp(2, 500);
        let samples = self
            .compute_motion_samples(study, n, self.motion_tools.tracked, false)
            .ok();
        let saved_tree = self.assembly_tree.clone();
        let saved_poses = self.instance_poses();
        let mut written = 0usize;
        let mut err = None;
        for k in 0..=n {
            self.scrub_motion_study(study, k as f64 / n as f64);
            match self.render_png_bytes(ducad_engine::render::View::Iso, 800, 600, false) {
                Ok(bytes) => match std::fs::write(dir.join(format!("frame_{k:03}.png")), bytes) {
                    Ok(()) => written += 1,
                    Err(e) => {
                        err = Some(e.to_string());
                        break;
                    }
                },
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
        }
        // Geometri kembali ke pose awal, lalu nilai mate/joint dipulihkan.
        self.restore_instance_poses(&saved_poses);
        self.assembly_tree = saved_tree;
        if let Some(e) = err {
            return Err(e);
        }
        if let Some(samples) = samples {
            let _ = std::fs::write(dir.join("motion.csv"), Self::motion_csv(&samples));
        }
        Ok(written)
    }

    /// Jejak titik terlacak di viewport (frame bertabrakan bertitik merah).
    pub fn paint_motion_trace(&self, ui: &egui::Ui, rect: egui::Rect) {
        if !self.motion_tools.show_trace || self.motion_tools.samples.len() < 2 {
            return;
        }
        let painter = ui.painter_at(rect);
        let projected: Vec<(egui::Pos2, usize)> = self
            .motion_tools
            .samples
            .iter()
            .filter_map(|s| {
                crate::viewport::world_to_screen_pos(&self.camera, rect, s.point.as_vec3())
                    .map(|p| (p, s.clashes))
            })
            .collect();
        let color = egui::Color32::from_rgb(255, 149, 0);
        let line: Vec<egui::Pos2> = projected.iter().map(|(p, _)| *p).collect();
        painter.add(egui::Shape::line(line, egui::Stroke::new(2.0, color)));
        for (p, clashes) in projected {
            if clashes > 0 {
                painter.circle_filled(p, 4.0, egui::Color32::from_rgb(255, 69, 58));
            }
        }
    }

    /// Jendela studi gerak.
    pub fn show_motion_tools(&mut self, ctx: &egui::Context) {
        if !self.motion_tools.open {
            return;
        }
        let studies: Vec<String> = self
            .assembly_tree
            .motion_studies
            .iter()
            .map(|s| s.name.clone())
            .collect();
        let mut instances: Vec<(AssemblyInstanceId, String)> = self
            .assembly_tree
            .instances
            .values()
            .map(|i| (i.id, i.name.clone()))
            .collect();
        instances.sort();
        let mut open = true;
        let mut run = false;
        let mut export = false;
        let mut copy_csv = false;
        let st = &mut self.motion_tools;
        egui::Window::new(ducad_i18n::t!("motion-title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(460.0)
            .show(ctx, |ui| {
                if studies.is_empty() {
                    ui.label(egui::RichText::new(ducad_i18n::t!("motion-empty")).small());
                    return;
                }
                st.study = st.study.min(studies.len() - 1);
                egui::Grid::new("motion-grid").num_columns(2).show(ui, |ui| {
                    ui.label(ducad_i18n::t!("motion-study"));
                    egui::ComboBox::from_id_salt("motion-study")
                        .selected_text(&studies[st.study])
                        .show_ui(ui, |ui| {
                            for (i, n) in studies.iter().enumerate() {
                                ui.selectable_value(&mut st.study, i, n);
                            }
                        });
                    ui.end_row();
                    ui.label(ducad_i18n::t!("motion-tracked"));
                    let cur = st
                        .tracked
                        .and_then(|t| instances.iter().find(|(i, _)| *i == t))
                        .map(|(_, n)| n.clone())
                        .unwrap_or_else(|| ducad_i18n::t!("motion-tracked-auto"));
                    egui::ComboBox::from_id_salt("motion-tracked")
                        .selected_text(cur)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut st.tracked,
                                None,
                                ducad_i18n::t!("motion-tracked-auto"),
                            );
                            for (id, n) in &instances {
                                ui.selectable_value(&mut st.tracked, Some(*id), n);
                            }
                        });
                    ui.end_row();
                    ui.label(ducad_i18n::t!("motion-samples"));
                    ui.add(egui::DragValue::new(&mut st.samples_n).range(2..=500));
                    ui.end_row();
                    ui.label(ducad_i18n::t!("motion-interference"));
                    ui.checkbox(&mut st.check_interference, "");
                    ui.end_row();
                    ui.label(ducad_i18n::t!("motion-trace"));
                    ui.checkbox(&mut st.show_trace, "");
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    run = ui.button(ducad_i18n::t!("motion-run")).clicked();
                    export = ui.button(ducad_i18n::t!("motion-export")).clicked();
                    copy_csv = ui
                        .add_enabled(
                            !st.samples.is_empty(),
                            egui::Button::new(ducad_i18n::t!("motion-copy-csv")),
                        )
                        .clicked();
                });
                if !st.samples.is_empty() {
                    plot(ui, &st.samples);
                    let clash_frames = st.samples.iter().filter(|s| s.clashes > 0).count();
                    let max_speed = st.samples.iter().map(|s| s.speed).fold(0.0, f64::max);
                    ui.label(ducad_i18n::t!(
                        "motion-summary",
                        frames = st.samples.len(),
                        clash = clash_frames,
                        speed = format!("{max_speed:.2}")
                    ));
                }
            });
        if run {
            let st = &self.motion_tools;
            let (s, n, tr, it) = (st.study, st.samples_n, st.tracked, st.check_interference);
            match self.compute_motion_samples(s, n, tr, it) {
                Ok(v) => self.motion_tools.samples = v,
                Err(e) => self.model_status = Some(e),
            }
        }
        if copy_csv {
            ctx.copy_text(Self::motion_csv(&self.motion_tools.samples));
            self.model_status = Some(ducad_i18n::t!("crash-copied"));
        }
        if export {
            if let Some(dir) = pick_folder() {
                let (s, n) = (self.motion_tools.study, self.motion_tools.samples_n);
                self.model_status = Some(match self.export_motion_frames(s, n, &dir) {
                    Ok(k) => ducad_i18n::t!(
                        "motion-exported",
                        count = k,
                        dir = dir.display().to_string()
                    ),
                    Err(e) => e,
                });
            }
        }
        if !open {
            self.motion_tools.open = false;
        }
    }
}

/// Folder tujuan ekspor: dialog di desktop, folder data app di tablet.
fn pick_folder() -> Option<std::path::PathBuf> {
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        rfd::FileDialog::new().pick_folder()
    }
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        Some(crate::platform::data_dir().join("motion"))
    }
}

/// Plot nilai (biru) dan kecepatan (oranye) terhadap t; frame tabrakan merah.
fn plot(ui: &mut egui::Ui, samples: &[MotionSample]) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(200.0), 140.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(80)),
        egui::StrokeKind::Inside,
    );
    let x_of = |t: f64| rect.left() + 4.0 + (t as f32) * (rect.width() - 8.0);
    let series = |f: &dyn Fn(&MotionSample) -> f64, color: egui::Color32| {
        let (lo, hi) = samples
            .iter()
            .map(f)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
        let span = (hi - lo).max(1e-9);
        let pts: Vec<egui::Pos2> = samples
            .iter()
            .map(|s| {
                let y = rect.bottom() - 4.0 - (((f(s) - lo) / span) as f32) * (rect.height() - 8.0);
                egui::pos2(x_of(s.t), y)
            })
            .collect();
        painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, color)));
    };
    series(&|s| s.value, egui::Color32::from_rgb(10, 132, 255));
    series(&|s| s.speed, egui::Color32::from_rgb(255, 149, 0));
    for s in samples.iter().filter(|s| s.clashes > 0) {
        painter.vline(
            x_of(s.t),
            rect.y_range(),
            egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 69, 58)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_core::assembly::{JointKind, MotionStudy};

    fn hit(o: (f64, f64, f64)) -> ducad_kernel::FaceHit {
        ducad_kernel::FaceHit {
            hit_point: o,
            centroid: o,
            normal: (1.0, 0.0, 0.0),
            surface_kind: ducad_kernel::SurfaceKind::Plane,
            pull_dir: (1.0, 0.0, 0.0),
            boundary_points: Vec::new(),
            face_index: Some(0),
            surface_radius: None,
            radial_axis: None,
        }
    }

    #[test]
    fn samples_value_trace_speed_clash_and_scrub_moves_geometry() {
        let mut app = DuCADApp::new_for_test();
        app.create_primitive(ducad_ui::PrimitiveKind::Box, [10.0; 3], [0.0; 3], false);
        app.create_primitive(ducad_ui::PrimitiveKind::Box, [4.0; 3], [20.0, 3.0, 3.0], false);
        let ids: Vec<_> = app.model.doc.bodies.iter().map(|(id, _)| id).collect();
        app.sync_assembly_instances();
        let ray = ducad_kernel::PickRay {
            origin: (0.0, 0.0, 0.0),
            dir: (1.0, 0.0, 0.0),
        };
        app.staged_mate_targets = vec![
            (ids[0], ray, hit((10.0, 5.0, 5.0))),
            (ids[1], ray, hit((20.0, 5.0, 5.0))),
        ];
        assert!(app.create_joint(JointKind::Slider, true), "{:?}", app.model_status);
        let mate = *app.assembly_tree.mates.keys().next().unwrap();
        app.assembly_tree.motion_studies.push(MotionStudy {
            name: "dorong".into(),
            driven_mate: mate,
            from: 0.0,
            to: -15.0,
            steps: 10,
        });
        let s = app.compute_motion_samples(0, 10, None, true).unwrap();
        assert_eq!(s.len(), 11);
        assert!((s[10].value + 15.0).abs() < 1e-9);
        // Kotak kecil bergeser −15 mm di X: pusat x 22 → 7 (menembus kotak besar).
        assert!((s[10].point.x - 7.0).abs() < 1e-3, "{:?}", s[10].point);
        assert!(s[0].clashes == 0 && s[10].clashes > 0);
        assert!(s[5].speed > 0.0);
        assert_eq!(DuCADApp::motion_csv(&s).lines().count(), 12);
        // Analisis tidak mengubah model.
        let min_x = |app: &DuCADApp| app.model.geometry.get(ids[1]).unwrap().shape.bounding_box().unwrap().0[0];
        assert!((min_x(&app) - 20.0).abs() < 1e-6);
        // Scrub benar-benar memindah geometri.
        app.scrub_motion_study(0, 1.0);
        assert!((min_x(&app) - 5.0).abs() < 1e-3, "{}", min_x(&app));
        assert!(app.compute_motion_samples(9, 10, None, false).is_err());
    }
}
