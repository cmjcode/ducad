use eframe::egui;
use ducad_kernel::SurfaceKind;
use glam::{DVec2, DVec3, Vec3};

use crate::app::DuCADApp;
use crate::model::{
    AddSolidCommand, BodyGeometry, ReplaceGeometryCommand,
};
use crate::viewport::{pixel_tolerance_to_world, world_to_screen_pos};

impl DuCADApp {
    /// Hitung delta pergeseran dari pergeseran mouse layar diproyeksikan ke sumbu normal 3D.
    pub fn project_screen_drag_to_world_axis(
        &self,
        rect: egui::Rect,
        origin_3d: Vec3,
        normal_3d: Vec3,
        drag_delta: egui::Vec2,
    ) -> (f64, Option<egui::Vec2>) {
        let normal = normal_3d.normalize_or_zero();
        let p_base = origin_3d;
        let p_ref = p_base + normal * 10.0;
        let s_base = world_to_screen_pos(&self.camera, rect, p_base);
        let s_ref = world_to_screen_pos(&self.camera, rect, p_ref);

        let world_scale = pixel_tolerance_to_world(&self.camera, rect) as f32;

        if let (Some(sb), Some(sr)) = (s_base, s_ref) {
            let mut arrow_vec = sr - sb;
            let mut len = arrow_vec.length();

            // Jika normal_3d sejajar/hampir sejajar garis pandang kamera (depth foreshortening),
            // panjang proyeksi di layar akan mendekati 0 sehingga arah menjadi ambigu.
            // Gunakan proyeksi kamera orthogonal agar drag tetap responsif di semua sudut pandang.
            if len < 8.0 {
                let forward = (self.camera.target - self.camera.eye()).normalize_or_zero();
                let cam_right = forward.cross(Vec3::Z).normalize_or_zero();
                let cam_up = cam_right.cross(forward).normalize_or_zero();
                let right_proj = normal.dot(cam_right);
                let up_proj = normal.dot(cam_up);
                let screen_dir = egui::vec2(right_proj, -up_proj);
                if screen_dir.length_sq() > 0.01 {
                    arrow_vec = screen_dir.normalized() * 15.0;
                    len = 15.0;
                } else {
                    arrow_vec = egui::vec2(10.0, -10.0);
                    len = arrow_vec.length();
                }
            }

            let arrow_dir = arrow_vec / len;
            let dot = drag_delta.x * arrow_dir.x + drag_delta.y * arrow_dir.y;
            let mm_per_pixel = (10.0 / len).clamp(world_scale * 0.4, world_scale * 1.8);
            let delta_mm = dot * mm_per_pixel;
            return (delta_mm as f64, Some(arrow_vec));
        }

        let default_scale = (world_scale as f64) * 1.0;
        ((-drag_delta.y as f64) * default_scale, None)
    }

    /// Hitung delta pergeseran diproyeksikan ke sumbu normal bidang sketsa aktif.
    pub fn project_screen_drag_to_extrude_axis(
        &self,
        rect: egui::Rect,
        centroid: DVec2,
        drag_delta: egui::Vec2,
    ) -> (f64, Option<egui::Vec2>) {
        let p_base = self.active_plane.to_world(centroid, 0.0);
        self.project_screen_drag_to_world_axis(rect, p_base, self.active_plane.normal, drag_delta)
    }

    /// Cari body yang akan dipotong bila profil terpilih di-extrude sejauh
    /// `distance` (Smart Boolean Cut): body terlihat pertama yang irisannya
    /// dengan hasil sapuan punya volume (bukan sekadar bersentuhan di face).
    /// Murni: dipakai pratinjau gizmo maupun tombol Extrude bernilai ketik.
    pub fn detect_cut_target(&self, distance: f64) -> Option<ducad_core::BodyId> {
        let profile =
            crate::model::build_profile_from_selection(self.sketch(), &self.selected).ok()?;
        let swept = self.extrude_profile_active_plane(&profile, distance).ok()?;
        let swept_tess = swept.tessellate();
        if swept_tess.positions.is_empty() {
            return None;
        }

        let mut swept_min = glam::Vec3::splat(f32::INFINITY);
        let mut swept_max = glam::Vec3::splat(f32::NEG_INFINITY);
        for p in &swept_tess.positions {
            let v = glam::Vec3::from_slice(p);
            swept_min = swept_min.min(v);
            swept_max = swept_max.max(v);
        }

        for (b_id, b_geo) in self.model.geometry.iter() {
            let Some(body) = self.model.doc.bodies.get(b_id) else {
                continue;
            };
            if !body.visible || b_geo.mesh.positions.is_empty() {
                continue;
            }
            let mut b_min = glam::Vec3::splat(f32::INFINITY);
            let mut b_max = glam::Vec3::splat(f32::NEG_INFINITY);
            for p in &b_geo.mesh.positions {
                let v = glam::Vec3::from_slice(p);
                b_min = b_min.min(v);
                b_max = b_max.max(v);
            }
            let overlaps = swept_min.x <= b_max.x
                && swept_max.x >= b_min.x
                && swept_min.y <= b_max.y
                && swept_max.y >= b_min.y
                && swept_min.z <= b_max.z
                && swept_max.z >= b_min.z;
            if !overlaps {
                continue;
            }
            if let Ok(intersect_shape) = ducad_kernel::intersect(&b_geo.shape, &swept) {
                if intersect_shape.tessellate().triangle_count() > 0 {
                    return Some(b_id);
                }
            }
        }
        None
    }

    /// Deteksi live apakah extrude saat ini memotong solid yang ada (Smart Boolean Cut).
    pub fn update_gizmo_boolean_detection(&mut self) {
        let target = self.detect_cut_target(self.gizmo_distance);
        self.gizmo_is_cutting = target.is_some();
        self.gizmo_target_body = target;
        self.gizmo_cut_preview = target.and_then(|t| self.compute_cut_preview(t, self.gizmo_distance));
    }

    /// Hitung pratinjau boolean cut (lihat [`crate::app::CutPreview`]):
    /// `removed = target ∩ tool`, `remaining = target − tool`. Memakai cache
    /// bila target dan jarak tidak berubah. `None` bila kernel gagal — pemanggil
    /// lalu memakai fallback (tool merah tembus pandang) agar pratinjau tetap ada.
    pub fn compute_cut_preview(
        &self,
        target: ducad_core::BodyId,
        distance: f64,
    ) -> Option<crate::app::CutPreview> {
        if let Some(prev) = &self.gizmo_cut_preview {
            if prev.target == target && (prev.distance - distance).abs() < 1e-9 {
                return Some(crate::app::CutPreview {
                    target,
                    distance,
                    removed: prev.removed.clone(),
                    remaining: prev.remaining.clone(),
                    remaining_edges: prev.remaining_edges.clone(),
                });
            }
        }
        let profile =
            crate::model::build_profile_from_selection(self.sketch(), &self.selected).ok()?;
        let tool = self.extrude_profile_active_plane(&profile, distance).ok()?;
        let geo = self.model.geometry.get(target)?;
        let removed_shape = ducad_kernel::intersect(&geo.shape, &tool).ok()?;
        let remaining_shape = ducad_kernel::subtract(&geo.shape, &tool).ok()?;
        let removed = removed_shape.tessellate();
        let remaining = remaining_shape.tessellate();
        if removed.positions.is_empty() {
            return None;
        }
        let remaining_edges = ducad_kernel::extract_shape_edges(&remaining_shape, Some(&remaining));
        Some(crate::app::CutPreview {
            target,
            distance,
            removed,
            remaining,
            remaining_edges,
        })
    }

    /// Potong `solids` hasil extrude dari body `target_id` (Smart Boolean Cut),
    /// catat sebagai fitur Cut Extrude. `false` bila kernel gagal memotong.
    pub fn apply_cut_extrusion(
        &mut self,
        target_id: ducad_core::BodyId,
        solids: &[(String, BodyGeometry)],
        distance: f64,
    ) -> bool {
        let Some(target_geo) = self.model.geometry.get(target_id) else {
            return false;
        };
        let mut cur_target_shape: Option<ducad_kernel::KernelShape> = None;
        for (_, solid_geo) in solids {
            let base = cur_target_shape.as_ref().unwrap_or(&target_geo.shape);
            if let Ok(cut_res) = ducad_kernel::subtract(base, &solid_geo.shape) {
                cur_target_shape = Some(cut_res);
            }
        }
        let Some(final_shape) = cur_target_shape else {
            return false;
        };
        let new_geo = BodyGeometry::from_shape(final_shape);
        self.execute_model_command(
            Box::new(ReplaceGeometryCommand::new("Cut Extrude", target_id, new_geo)),
            &format!("Memotong solid 3D sedalam {:.1} mm", distance.abs()),
        );
        self.record_extrude_feature(distance, true);
        self.round_history.remove(&target_id);
        true
    }

    /// Eksekusi commit extrude/cut saat drag gizmo selesai atau nilai presisi di-enter.
    pub fn commit_gizmo_extrusion(&mut self) {
        if self.gizmo_distance.abs() > 0.1 {
            let result = crate::model::extrude_selection_with_holes_on_plane(
                self.sketch(),
                &self.selected,
                &crate::document::plane_frame_from(&self.active_plane),
                self.gizmo_distance,
            );
            if let Err(msg) = &result {
                // Dulu kegagalan di sini diabaikan diam-diam: pengguna hanya
                // melihat pratinjau kerangka tanpa tahu extrude gagal.
                self.model_status = Some(format!("Extrude gagal: {msg}"));
                self.show_op_error(
                    &ducad_engine::OpError::new(
                        ducad_engine::OpErrorCode::KernelFailed,
                        msg.clone(),
                    ),
                    Vec::new(),
                );
            }
            if let Ok(solids) = result {
                if self.gizmo_is_cutting {
                    if let Some(target_id) = self.gizmo_target_body {
                        let dist = self.gizmo_distance;
                        if !self.apply_cut_extrusion(target_id, &solids, dist) {
                            self.model_status =
                                Some("Cut Extrude gagal: kernel tidak bisa memotong body".to_string());
                        }
                    }
                } else if solids.len() == 1 {
                    let (name, geo) = solids.into_iter().next().unwrap();
                    let dist = self.gizmo_distance;
                    let cmd = AddSolidCommand::new("Extrude", geo);
                    self.execute_model_command(
                        Box::new(cmd),
                        &format!("Membuat solid {name} setinggi {:.1} mm", dist),
                    );
                    self.record_extrude_feature(dist, false);
                } else if !solids.is_empty() {
                    let count = solids.len();
                    let dist = self.gizmo_distance;
                    self.execute_model_command(
                        Box::new(crate::model::AddMultipleSolidsCommand::new("Teks 3D", solids)),
                        &format!("Membuat {} solid 3D setinggi {:.1} mm", count, dist),
                    );
                    self.record_extrude_feature(dist, false);
                }
                self.selected.clear();
            }
        }
        self.reset_gizmo_extrusion_state();
    }

    /// Mulai drag handle extrude profil. Bila sedang menunggu konfirmasi
    /// (`gizmo_staged`), drag melanjutkan dari tinggi yang ada; bila belum,
    /// mulai dari tinggi handle idle supaya handle tidak melompat dari pointer.
    pub fn begin_gizmo_drag(&mut self) {
        // Pindah ke tampilan 3D DULU: `set_tool(Select)` di dalamnya
        // membatalkan gizmo yang sedang aktif, jadi bendera di bawah harus
        // disetel sesudahnya — kalau tidak, drag dari mode sketsa langsung
        // dibatalkan dan tidak terjadi apa-apa.
        self.auto_enter_3d_mode_on_extrude_drag();
        // Idempoten: kanvas dan widget handle bisa sama-sama melaporkan
        // drag_started di frame yang sama; hanya inisialisasi saat belum aktif.
        if !self.extruding_from_gizmo {
            self.gizmo_distance = crate::overlay::GIZMO_IDLE_HEIGHT_MM;
        }
        self.extruding_from_gizmo = true;
        self.gizmo_staged = false;
        self.gizmo_dimension_editing = false;
    }

    /// Drag gizmo extrude profil dilepas: JANGAN langsung commit. Pratinjau,
    /// handle, dan label tinggi tetap tampil dan popup nilai dibuka dengan angka
    /// terseleksi, sehingga pengguna bisa langsung mengetik tinggi presisi.
    /// Enter/tombol centang meng-commit, Esc/tombol silang membatalkan, handle masih bisa digeser lagi.
    pub fn stage_gizmo_extrusion(&mut self) {
        if !self.extruding_from_gizmo {
            return;
        }
        if self.gizmo_distance.abs() <= 0.1 {
            // Drag tak berarti (klik saja): kembali ke keadaan idle.
            self.cancel_gizmo_extrusion();
            return;
        }
        self.gizmo_staged = true;
        self.gizmo_edit_input = Self::format_gizmo_input(self.unit, self.gizmo_distance);
        self.gizmo_dimension_editing = true;
        self.gizmo_edit_select_all = true;
        self.model_status = Some(format!(
            "Tinggi extrude {} — ketik nilai presisi lalu Enter untuk terapkan, atau Esc untuk batal",
            self.unit.format(self.gizmo_distance.abs())
        ));
    }

    /// Batalkan extrude profil yang sedang digeser/menunggu konfirmasi tanpa
    /// mengubah model. Seleksi profil dipertahankan.
    pub fn cancel_gizmo_extrusion(&mut self) {
        let was_active = self.extruding_from_gizmo;
        self.reset_gizmo_extrusion_state();
        if was_active {
            self.model_status = Some("Extrude dibatalkan".to_string());
        }
    }

    fn reset_gizmo_extrusion_state(&mut self) {
        self.extruding_from_gizmo = false;
        self.gizmo_staged = false;
        self.gizmo_dimension_editing = false;
        self.gizmo_edit_select_all = false;
        self.gizmo_is_cutting = false;
        self.gizmo_target_body = None;
        self.gizmo_cut_preview = None;
        self.gizmo_distance = 0.0;
        self.gizmo_edit_input = Self::format_gizmo_input(self.unit, self.gizmo_distance);
    }

    /// Format nilai jarak (mm internal) untuk kotak input gizmo: bulat bila
    /// bilangan bulat, selain itu satu desimal — agar angka hasil drag mudah
    /// dibaca dan ditimpa.
    pub fn format_gizmo_input(unit: ducad_core::LengthUnit, value_mm: f64) -> String {
        let v = unit.to_display_val(value_mm);
        if (v - v.round()).abs() < 1e-6 {
            format!("{:.0}", v)
        } else {
            format!("{:.1}", v)
        }
    }

    /// Ukuran DASAR face aktif yang diubah gizmo tarik-sisi (mm), supaya label
    /// dan input presisi memakai ukuran hasil, bukan hanya selisih:
    /// - face datar: jarak titik klik ke sisi terjauh body searah `pull_dir`
    ///   (= tinggi body untuk tutup extrude, lebar untuk sisi samping balok);
    /// - selimut silinder / bola: radius permukaan dari sumbu/pusat.
    ///
    /// `None` bila tidak terdefinisi tunggal (kerucut, permukaan bebas) —
    /// label kembali ke mode selisih (`Δ`).
    pub fn face_gizmo_base_dimension(&self) -> Option<f64> {
        let (body_id, _, hit) = self.active_face.as_ref()?;
        match hit.surface_kind {
            SurfaceKind::Plane => {
                let geo = self.model.geometry.get(*body_id)?;
                let d = DVec3::new(hit.pull_dir.0, hit.pull_dir.1, hit.pull_dir.2)
                    .normalize_or_zero();
                if d == DVec3::ZERO {
                    return None;
                }
                let hit_proj = DVec3::new(hit.hit_point.0, hit.hit_point.1, hit.hit_point.2).dot(d);
                let min_proj = geo
                    .mesh
                    .positions
                    .iter()
                    .map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64).dot(d))
                    .fold(f64::INFINITY, f64::min);
                min_proj.is_finite().then(|| (hit_proj - min_proj).max(0.0))
            }
            SurfaceKind::Cylinder | SurfaceKind::Sphere => hit.surface_radius,
            _ => None,
        }
    }

    /// Ukuran HASIL (mm) setelah tarik-sisi sejauh `face_gizmo_distance`:
    /// dasar + selisih. `None` bila dasar tidak diketahui.
    pub fn face_gizmo_result_dimension(&self) -> Option<f64> {
        self.face_gizmo_base_dimension()
            .map(|base| base + self.face_gizmo_distance)
    }

    /// Teks pill gizmo tarik-sisi. Bila ukuran dasar diketahui: ukuran hasil
    /// sebagai angka utama, selisih dalam kurung — `61.2 mm (+11.2)` atau
    /// `R 101.22 mm (+21.88)`. Bila tidak: selisih saja seperti semula.
    pub fn face_gizmo_dimension_text(&self) -> String {
        let Some((_, _, hit)) = self.active_face.as_ref() else {
            return self.unit.format(self.face_gizmo_distance);
        };
        let radial = matches!(
            hit.surface_kind,
            SurfaceKind::Cylinder | SurfaceKind::Cone | SurfaceKind::Sphere
        );
        match self.face_gizmo_result_dimension() {
            Some(result) => {
                let main = if radial {
                    format!("R {}", self.unit.format(result))
                } else {
                    self.unit.format(result)
                };
                let delta = self.unit.to_display_val(self.face_gizmo_distance);
                let sign = if delta < 0.0 { "-" } else { "+" };
                format!("{main} ({sign}{:.2})", delta.abs())
            }
            None => {
                let formatted = self.unit.format(self.face_gizmo_distance);
                if !radial {
                    formatted
                } else if self.face_gizmo_distance >= 0.0 {
                    format!("ΔR +{formatted}")
                } else {
                    format!("ΔR {formatted}")
                }
            }
        }
    }

    /// Teks kotak input presisi: ukuran hasil absolut bila dasar diketahui,
    /// selain itu selisih.
    pub fn face_gizmo_input_text(&self) -> String {
        let v = self
            .face_gizmo_result_dimension()
            .unwrap_or(self.face_gizmo_distance);
        Self::format_gizmo_input(self.unit, v)
    }

    /// Terjemahkan angka yang diketik di popup (satuan tampilan) menjadi
    /// `face_gizmo_distance`. Bila ukuran dasar diketahui, angka adalah
    /// ukuran HASIL (tinggi/radius akhir) sehingga selisih = target − dasar
    /// dan arah tarik/potong mengikuti tandanya. Bila tidak, angka adalah
    /// selisih: positif ikut arah drag saat ini, negatif eksplisit = potong.
    pub fn apply_face_gizmo_typed_value(&mut self, typed: f64) -> Result<(), String> {
        let v = self.unit.to_internal_mm(typed);
        match self.face_gizmo_base_dimension() {
            Some(base) => {
                if v <= 0.0 {
                    return Err("Ukuran hasil harus lebih besar dari 0".to_string());
                }
                self.face_gizmo_distance = v - base;
            }
            None => {
                self.face_gizmo_distance = if v < 0.0 {
                    v
                } else if self.face_gizmo_distance < 0.0 {
                    -v
                } else {
                    v
                };
            }
        }
        Ok(())
    }

    /// Mulai drag handle tarik-sisi (push/pull). Melanjutkan jarak yang ada
    /// bila sedang menunggu konfirmasi.
    pub fn begin_face_gizmo_drag(&mut self) {
        // Lihat `begin_gizmo_drag`: pindah mode dulu, baru setel bendera.
        self.auto_enter_3d_mode_on_extrude_drag();
        if !self.extruding_face_from_gizmo {
            self.face_gizmo_distance = 0.0;
        }
        self.extruding_face_from_gizmo = true;
        self.face_gizmo_staged = false;
        self.face_gizmo_dimension_editing = false;
    }

    /// Tombol Extrude di bilah konteks/palet saat sebuah sisi aktif: langsung
    /// buka input presisi (jarak 0) tanpa perlu drag.
    pub fn open_face_gizmo_precise_input(&mut self) {
        // Pindah mode dulu (lihat `begin_gizmo_drag`), baru setel bendera.
        self.auto_enter_3d_mode_on_extrude_drag();
        self.extruding_face_from_gizmo = true;
        self.face_gizmo_staged = true;
        self.face_gizmo_distance = 0.0;
        self.face_gizmo_dimension_editing = true;
        // Dasar diketahui → tampilkan ukuran sekarang (terseleksi penuh) agar
        // langsung ditimpa; tidak → kosong (selisih dari 0).
        self.face_gizmo_edit_input = if self.face_gizmo_base_dimension().is_some() {
            self.face_gizmo_input_text()
        } else {
            String::new()
        };
        self.gizmo_edit_select_all = true;
    }

    /// Drag gizmo tarik-sisi dilepas: tahan pratinjau + handle + label, buka
    /// popup nilai (lihat `stage_gizmo_extrusion`).
    pub fn stage_face_gizmo_extrusion(&mut self) {
        if !self.extruding_face_from_gizmo {
            return;
        }
        if self.face_gizmo_distance.abs() <= 0.1 {
            self.cancel_face_gizmo_extrusion();
            return;
        }
        self.face_gizmo_staged = true;
        self.face_gizmo_edit_input = self.face_gizmo_input_text();
        self.face_gizmo_dimension_editing = true;
        self.gizmo_edit_select_all = true;
        let verb = if self.face_gizmo_distance < 0.0 { "Potong sisi" } else { "Tarik sisi" };
        let result = self
            .face_gizmo_result_dimension()
            .map(|r| format!(" menjadi {}", self.unit.format(r)))
            .unwrap_or_default();
        self.model_status = Some(format!(
            "{verb} {}{result} — ketik ukuran hasil lalu Enter untuk terapkan, atau Esc untuk batal",
            self.unit.format(self.face_gizmo_distance.abs())
        ));
    }

    /// Commit tarik-sisi dengan `face_gizmo_distance` saat ini, lalu reset gizmo.
    pub fn commit_face_gizmo_extrusion(&mut self) {
        if self.face_gizmo_distance.abs() > 0.1 {
            self.extrude_active_face(self.face_gizmo_distance);
        }
        self.reset_face_gizmo_state();
    }

    /// Batalkan tarik-sisi yang sedang digeser/menunggu konfirmasi.
    pub fn cancel_face_gizmo_extrusion(&mut self) {
        let was_active = self.extruding_face_from_gizmo;
        self.reset_face_gizmo_state();
        if was_active {
            self.model_status = Some("Tarik sisi dibatalkan".to_string());
        }
    }

    /// Segarkan pratinjau potong sisi (push/pull ke arah DALAM): body tampil
    /// SUDAH terpotong (hasil `extrude_face` berjarak negatif) dan volume yang
    /// dibuang (`body − hasil`) merah tembus pandang. Tanpa ini prisma
    /// pemotong tersembunyi di dalam body yang pekat sehingga tampak tidak
    /// terjadi apa-apa. Arah keluar tetap memakai pratinjau tutup+dinding.
    pub fn refresh_face_cut_preview(&mut self) {
        let Some((target, ray)) = self.active_face.as_ref().map(|(id, r, _)| (*id, *r)) else {
            self.face_cut_preview = None;
            return;
        };
        let distance = self.face_gizmo_distance;
        if !self.extruding_face_from_gizmo || distance > -0.1 {
            self.face_cut_preview = None;
            return;
        }
        if let Some(prev) = &self.face_cut_preview {
            if prev.target == target && (prev.distance - distance).abs() < 1e-9 {
                return;
            }
        }
        self.face_cut_preview = self.compute_face_cut_preview(target, ray, distance);
    }

    fn compute_face_cut_preview(
        &self,
        target: ducad_core::BodyId,
        ray: ducad_kernel::PickRay,
        distance: f64,
    ) -> Option<crate::app::CutPreview> {
        let geo = self.model.geometry.get(target)?;
        let remaining_shape = ducad_kernel::extrude_face(&geo.shape, ray, distance).ok()?;
        let removed_shape = ducad_kernel::subtract(&geo.shape, &remaining_shape).ok()?;
        let removed = removed_shape.tessellate();
        if removed.positions.is_empty() {
            return None;
        }
        let remaining = remaining_shape.tessellate();
        let remaining_edges = ducad_kernel::extract_shape_edges(&remaining_shape, Some(&remaining));
        Some(crate::app::CutPreview {
            target,
            distance,
            removed,
            remaining,
            remaining_edges,
        })
    }

    fn reset_face_gizmo_state(&mut self) {
        self.face_cut_preview = None;
        self.extruding_face_from_gizmo = false;
        self.face_gizmo_staged = false;
        self.face_gizmo_dimension_editing = false;
        self.gizmo_edit_select_all = false;
        self.face_gizmo_distance = 0.0;
        self.face_gizmo_edit_input = "0".to_string();
    }

    /// Benar hanya saat pointer sedang MENGGESER sebuah gizmo (bukan saat
    /// menunggu konfirmasi). Dipakai untuk kursor resize dan memblokir orbit —
    /// saat staged, kanvas boleh di-orbit untuk memeriksa pratinjau.
    pub fn gizmo_pointer_dragging(&self) -> bool {
        (self.extruding_from_gizmo && !self.gizmo_staged)
            || (self.extruding_face_from_gizmo && !self.face_gizmo_staged)
            || self.filleting_vertex_from_gizmo
            || self.filleting_edge_from_gizmo
    }

    /// Cek apakah posisi mouse saat ini berada dekat dengan gizmo panah atau dasar profil.
    pub fn check_near_gizmo(&self, rect: egui::Rect, hover_pos: Option<egui::Pos2>) -> bool {
        let Some(pos) = hover_pos else {
            return false;
        };

        if let Some(c) = self.selected_closed_region_centroid() {
            let z_top = if self.extruding_from_gizmo {
                self.gizmo_distance as f32
            } else {
                16.0
            };
            let top_3d = self.active_plane.to_world(c, z_top);
            let bot_3d = self.active_plane.to_world(c, 0.0);
            let near_top = world_to_screen_pos(&self.camera, rect, top_3d)
                .is_some_and(|s| s.distance(pos) < 36.0);
            let near_bot = world_to_screen_pos(&self.camera, rect, bot_3d)
                .is_some_and(|s| s.distance(pos) < 36.0);
            if near_top || near_bot {
                return true;
            }
        }

        if let Some((_, _, hit)) = &self.active_face {
            let anchor = hit.gizmo_anchor();
            let c_base = Vec3::new(anchor.0 as f32, anchor.1 as f32, anchor.2 as f32);
            let pull_dir = Vec3::new(
                hit.pull_dir.0 as f32,
                hit.pull_dir.1 as f32,
                hit.pull_dir.2 as f32,
            );
            let dist = if self.extruding_face_from_gizmo {
                self.face_gizmo_distance as f32
            } else {
                18.0
            };
            let top_3d = c_base + pull_dir * dist;
            let mid_3d = (c_base + top_3d) * 0.5;
            let near_top = world_to_screen_pos(&self.camera, rect, top_3d)
                .is_some_and(|s| s.distance(pos) < 40.0);
            let near_bot = world_to_screen_pos(&self.camera, rect, c_base)
                .is_some_and(|s| s.distance(pos) < 40.0);
            let near_mid = world_to_screen_pos(&self.camera, rect, mid_3d)
                .is_some_and(|s| s.distance(pos) < 40.0);
            if near_top || near_bot || near_mid {
                return true;
            }
        }

        if let Some((c_base, pull_dir)) = self.active_vertex_gizmo_dir() {
            let z_pos = (14.0 + self.vertex_gizmo_radius.abs() as f32 * 0.35).clamp(14.0, 70.0);
            let top_3d = c_base + pull_dir * z_pos;
            let near_top = world_to_screen_pos(&self.camera, rect, top_3d)
                .is_some_and(|s| s.distance(pos) < 48.0);
            let near_base = world_to_screen_pos(&self.camera, rect, c_base)
                .is_some_and(|s| s.distance(pos) < 48.0);
            if near_top || near_base {
                return true;
            }
        }

        if let Some((c_base, pull_dir)) = self.active_edge_gizmo_dir() {
            let z_pos = (14.0 + self.edge_gizmo_radius.abs() as f32 * 0.35).clamp(14.0, 70.0);
            let top_3d = c_base + pull_dir * z_pos;
            let near_top = world_to_screen_pos(&self.camera, rect, top_3d)
                .is_some_and(|s| s.distance(pos) < 48.0);
            let near_base = world_to_screen_pos(&self.camera, rect, c_base)
                .is_some_and(|s| s.distance(pos) < 48.0);
            if near_top || near_base {
                return true;
            }
        }

        if !self.feature_pick_active() {
            if let Some((_, center)) = self.selected_single_body_center() {
                if let Some(s_center) = world_to_screen_pos(&self.camera, rect, center) {
                    if s_center.distance(pos) < 95.0 {
                        return true;
                    }
                }
            }
        }

        false
    }
}
