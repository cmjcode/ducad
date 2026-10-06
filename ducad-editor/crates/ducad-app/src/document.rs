use ducad_core::Command;
use ducad_kernel::{KernelMesh, KernelShape};
use ducad_render::{PlaneKind, SketchPlane};
use ducad_sketch::{PlaneRef, Sketch, SketchId};

use crate::app::DuCADApp;
use crate::model::ModelDoc;
use crate::types::ToolKind;

/// Salin `SketchPlane` (f32, milik crate render) ke `PlaneFrame` (f64, milik
/// engine) apa adanya — normal TIDAK dihitung ulang, karena normal Front
/// sengaja −Y yang bukan `u × v`. Fungsi bebas, bukan `impl From`: kedua
/// tipe berasal dari crate lain sehingga aturan orphan melarangnya.
pub fn plane_frame_from(plane: &SketchPlane) -> ducad_engine::PlaneFrame {
    let a = |v: glam::Vec3| [v.x as f64, v.y as f64, v.z as f64];
    ducad_engine::PlaneFrame {
        origin: a(plane.origin),
        u_axis: a(plane.u_axis),
        v_axis: a(plane.v_axis),
        normal: a(plane.normal),
    }
}

impl DuCADApp {
    #[inline]
    pub fn static_plane_for_index(idx: usize) -> SketchPlane {
        match idx {
            0 => SketchPlane::top(),
            1 => SketchPlane::front(),
            2 => SketchPlane::right(),
            _ => SketchPlane::top(),
        }
    }

    /// Terjemahkan indeks bidang gaya lama (0=Top, 1=Front, 2=Right, 3+=datum)
    /// menjadi [`PlaneRef`].
    ///
    /// Jembatan sementara: sebagian pemanggil (mis. `parametric_engine`) masih
    /// menyimpan `plane_index: usize` di dalam payload fitur. Indeks itu
    /// BERGESER saat datum plane dihapus — persis cacat yang `PlaneRef`
    /// hilangkan — jadi payload tersebut perlu ikut dipindah ke `PlaneRef`
    /// (bagian dari P0.5). Sampai saat itu, konversinya terpusat di sini
    /// alih-alih aritmetika `+ 3` / `- 3` yang berserak.
    #[inline]
    pub fn plane_ref_for_index(&self, idx: usize) -> PlaneRef {
        match idx {
            0 => PlaneRef::Top,
            1 => PlaneRef::Front,
            2 => PlaneRef::Right,
            i => self
                .datum_planes
                .get(i - 3)
                .map(|dp| PlaneRef::Datum(dp.id))
                .unwrap_or(PlaneRef::Top),
        }
    }

    /// Kebalikan [`Self::plane_ref_for_index`].
    #[inline]
    pub fn index_for_plane_ref(&self, plane: PlaneRef) -> usize {
        match plane {
            PlaneRef::Top => 0,
            PlaneRef::Front => 1,
            PlaneRef::Right => 2,
            PlaneRef::Datum(id) => self
                .datum_planes
                .iter()
                .position(|dp| dp.id == id)
                .map(|pos| pos + 3)
                .unwrap_or(0),
        }
    }

    /// [`PlaneRef`] bidang yang sedang aktif.
    #[inline]
    pub fn active_plane_ref(&self) -> PlaneRef {
        match self.active_plane.kind {
            PlaneKind::Top => PlaneRef::Top,
            PlaneKind::Front => PlaneRef::Front,
            PlaneKind::Right => PlaneRef::Right,
            PlaneKind::Custom(id) => PlaneRef::Datum(id),
        }
    }

    /// Id sketsa yang sedang aktif — sketsa pertama pada bidang aktif.
    ///
    /// Berbeda dari kode lama yang jatuh ke `sketches[0]` saat indeks meleset
    /// (diam-diam menulis ke sketsa yang SALAH), di sini kegagalan resolusi
    /// jatuh ke sketsa aktif `SketchSet` yang dijamin valid.
    #[inline]
    pub fn active_sketch_id(&self) -> SketchId {
        self.sketch_set
            .first_on_plane(self.active_plane_ref())
            .unwrap_or_else(|| self.sketch_set.active_id())
    }

    /// Sketsa pada indeks bidang gaya lama.
    #[inline]
    pub fn sketch_at_index(&self, idx: usize) -> &Sketch {
        let plane = self.plane_ref_for_index(idx);
        self.sketch_set
            .first_on_plane(plane)
            .and_then(|id| self.sketch_set.sketch(id))
            .unwrap_or_else(|| self.sketch_set.active_sketch())
    }

    #[inline]
    pub fn sketch_at_index_mut(&mut self, idx: usize) -> Option<&mut Sketch> {
        let plane = self.plane_ref_for_index(idx);
        let id = self.sketch_set.first_on_plane(plane)?;
        self.sketch_set.sketch_mut(id)
    }

    /// Seluruh sketsa dalam urutan indeks bidang gaya lama.
    ///
    /// Dipakai jalur yang masih berpikir dalam indeks: hit-test lintas bidang
    /// dan serialisasi file native. Mengembalikan rujukan, bukan salinan —
    /// hit-test dipanggil tiap frame.
    pub fn plane_ordered_sketches(&self) -> Vec<&Sketch> {
        let mut out = Vec::with_capacity(3 + self.datum_planes.len());
        for idx in 0..(3 + self.datum_planes.len()) {
            out.push(self.sketch_at_index(idx));
        }
        out
    }

    #[inline]
    pub fn plane_for_index(&self, idx: usize) -> SketchPlane {
        match idx {
            0 => SketchPlane::top(),
            1 => SketchPlane::front(),
            2 => SketchPlane::right(),
            custom_idx if custom_idx >= 3 && custom_idx - 3 < self.datum_planes.len() => {
                self.datum_planes[custom_idx - 3].plane
            }
            _ => SketchPlane::top(),
        }
    }

    #[inline]
    pub fn active_plane_index(&self) -> usize {
        match self.active_plane.kind {
            PlaneKind::Top => 0,
            PlaneKind::Front => 1,
            PlaneKind::Right => 2,
            PlaneKind::Custom(id) => {
                self.datum_planes
                    .iter()
                    .position(|dp| dp.id == id)
                    .map(|pos| pos + 3)
                    .unwrap_or(0)
            }
        }
    }

    pub fn all_planes(&self) -> Vec<(usize, SketchPlane, String)> {
        let mut list = vec![
            (0, SketchPlane::top(), "Top Plane (XY)".to_string()),
            (1, SketchPlane::front(), "Front Plane (XZ)".to_string()),
            (2, SketchPlane::right(), "Right Plane (YZ)".to_string()),
        ];
        for (i, dp) in self.datum_planes.iter().enumerate() {
            list.push((i + 3, dp.plane, dp.name.clone()));
        }
        list
    }

    pub fn create_datum_plane(&mut self, name: String, mut plane: SketchPlane) -> u32 {
        self.datum_plane_counter += 1;
        let id = self.datum_plane_counter;
        plane.kind = PlaneKind::Custom(id);
        let datum_plane = ducad_render::plane::DatumPlane::new(id, name.clone(), plane);
        self.datum_planes.push(datum_plane);
        self.sketch_set.add(name.clone(), PlaneRef::Datum(id));
        self.record_activity(
            ducad_ui::ActivityKindUi::Solid3D,
            "Buat Bidang Referensi (Datum Plane)",
            &format!("Membuat bidang referensi '{}'", name),
        );
        self.record_datum_plane_feature(id, 0.0, 0.0, name);
        id
    }

    pub fn delete_datum_plane(&mut self, id: u32) {
        if let Some(pos) = self.datum_planes.iter().position(|dp| dp.id == id) {
            let is_active = match self.active_plane.kind {
                PlaneKind::Custom(active_id) => active_id == id,
                _ => false,
            };
            if is_active {
                self.set_sketch_plane(PlaneKind::Top);
            }
            let name = self.datum_planes[pos].name.clone();
            self.datum_planes.remove(pos);
            // Dicocokkan lewat PlaneRef, bukan posisi: menghapus datum di
            // TENGAH daftar tidak lagi menggeser sketsa datum sesudahnya.
            self.sketch_set.remove_plane(PlaneRef::Datum(id));
            self.record_activity(
                ducad_ui::ActivityKindUi::Solid3D,
                "Hapus Bidang Referensi",
                &format!("Menghapus bidang referensi '{}'", name),
            );
        }
    }

    pub fn set_sketch_plane_by_index(&mut self, idx: usize) {
        let plane = self.plane_for_index(idx);
        self.active_plane = plane;
        self.selected.clear();
        self.hovered = None;
        self.pending_points.clear();
        self.pending_point_refs.clear();
        self.offset_source = None;
        self.last_snap = None;
        self.is_sketching = true;
        self.left_toolbar.is_sketching = true;
        self.camera.orient_to_plane(&self.active_plane);
    }

    pub fn apply_create_datum_plane(&mut self) {
        let offset_val = self.datum_offset_input.trim().parse::<f64>().unwrap_or(20.0);
        let angle_val = self.datum_angle_input.trim().parse::<f64>().unwrap_or(45.0);

        let (name, plane) = match self.datum_mode {
            ducad_ui::DatumPlaneMode::Offset => {
                let dist = if self.datum_flip { -offset_val } else { offset_val };
                if let Some((_, _, hit)) = &self.active_face {
                    let origin = glam::vec3(hit.hit_point.0 as f32, hit.hit_point.1 as f32, hit.hit_point.2 as f32);
                    let norm = glam::vec3(hit.normal.0 as f32, hit.normal.1 as f32, hit.normal.2 as f32);
                    let plane = SketchPlane::from_face_offset(origin, norm, dist as f32);
                    let name = format!("Plane {} (Face {:+0.0}mm)", self.datum_plane_counter + 1, dist);
                    (name, plane)
                } else {
                    let base = self.plane_for_index(self.datum_base_plane_idx);
                    let plane = base.offset(dist as f32);
                    let name = format!("Plane {} (Offset {:+0.0}mm)", self.datum_plane_counter + 1, dist);
                    (name, plane)
                }
            }
            ducad_ui::DatumPlaneMode::Angled => {
                let ang = if self.datum_flip { -angle_val } else { angle_val };
                if let Some(edge) = self.selected_edges.first() {
                    let p1 = edge.polyline.first().map(|&(x, y, z)| glam::vec3(x as f32, y as f32, z as f32)).unwrap_or(glam::Vec3::ZERO);
                    let p2 = edge.polyline.last().map(|&(x, y, z)| glam::vec3(x as f32, y as f32, z as f32)).unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
                    let ref_norm = glam::Vec3::Z;
                    let plane = SketchPlane::from_angle_and_edge(p1, p2, ref_norm, ang as f32);
                    let name = format!("Plane {} (Angled {:0.0}°)", self.datum_plane_counter + 1, ang);
                    (name, plane)
                } else {
                    let plane = SketchPlane::from_angle_and_edge(
                        glam::Vec3::ZERO,
                        glam::Vec3::new(50.0, 0.0, 0.0),
                        glam::Vec3::Z,
                        ang as f32,
                    );
                    let name = format!("Plane {} (Angled {:0.0}°)", self.datum_plane_counter + 1, ang);
                    (name, plane)
                }
            }
            ducad_ui::DatumPlaneMode::ThreePoints => {
                if self.datum_selected_points.len() >= 3 {
                    let p1 = self.datum_selected_points[0];
                    let p2 = self.datum_selected_points[1];
                    let p3 = self.datum_selected_points[2];
                    if let Some(plane) = SketchPlane::from_3_points(p1, p2, p3) {
                        let name = format!("Plane {} (3-Point)", self.datum_plane_counter + 1);
                        (name, plane)
                    } else {
                        self.model_status = Some("Gagal membuat bidang: 3 titik kolinear".to_string());
                        return;
                    }
                } else {
                    self.model_status = Some("Pilih 3 titik non-kolinear terlebih dahulu".to_string());
                    return;
                }
            }
        };

        let new_id = self.create_datum_plane(name.clone(), plane);
        self.set_sketch_plane(PlaneKind::Custom(new_id));
        self.datum_selected_points.clear();
        self.planes_drawer_open = true;
        self.model_status = Some(format!("Bidang referensi '{}' berhasil dibuat", name));
        self.set_tool(ToolKind::Select);
    }

    #[inline]
    pub fn sketch(&self) -> &Sketch {
        let id = self.active_sketch_id();
        self.sketch_set
            .sketch(id)
            .unwrap_or_else(|| self.sketch_set.active_sketch())
    }

    #[inline]
    #[allow(dead_code)]
    pub fn sketch_mut(&mut self) -> &mut Sketch {
        let id = self.active_sketch_id();
        self.sketch_set.set_active(id);
        self.sketch_set.active_sketch_mut()
    }

    #[inline]
    pub fn execute_sketch_command(&mut self, cmd: Box<dyn Command<Sketch>>) {
        let name = cmd.name().to_string();
        let idx = self.active_plane_index();
        let plane_label = match self.active_plane.kind {
            PlaneKind::Custom(id) => self
                .datum_planes
                .iter()
                .find(|dp| dp.id == id)
                .map(|dp| dp.name.clone())
                .unwrap_or_else(|| self.active_plane.kind.display_label().to_string()),
            _ => self.active_plane.kind.display_label().to_string(),
        };
        let sketch_id = self.active_sketch_id();
        self.sketch_set.set_active(sketch_id);
        self.sketch_set.execute(cmd);
        self.clear_redo_except(crate::types::UndoTarget::Sketch);

        let (action_title, detail_desc) = match name.as_str() {
            "Line" => ("Sketsa Garis 2D", format!("Menggambar segmen garis di Bidang {}", plane_label)),
            "Circle" => ("Sketsa Lingkaran 2D", format!("Menggambar lingkaran di Bidang {}", plane_label)),
            "Arc" => ("Sketsa Busur 2D", format!("Menggambar busur 3-titik di Bidang {}", plane_label)),
            "Rectangle" => ("Sketsa Persegi 2D", format!("Menggambar kotak/persegi di Bidang {}", plane_label)),
            "Ellipse" => ("Sketsa Elips 2D", format!("Menggambar elips di Bidang {}", plane_label)),
            "Trim" => ("Potong Garis (Trim)", format!("Memotong segmen garis di Bidang {}", plane_label)),
            "Offset" => ("Offset Garis / Kurva", format!("Menduplikasi garis sejajar di Bidang {}", plane_label)),
            "Mirror" => ("Cermin Sketsa (Mirror)", format!("Mencerminkan entitas sketsa di Bidang {}", plane_label)),
            "Project Edges" => ("Proyeksi Tepi Face", format!("Memproyeksikan batas sisi objek sebagai garis konstruksi di Bidang {}", plane_label)),
            "Delete" => ("Hapus Entitas Sketsa", format!("Menghapus elemen 2D di Bidang {}", plane_label)),
            "Move" => ("Geser Sketsa 2D", format!("Memindahkan posisi elemen di Bidang {}", plane_label)),
            _ => ("Aktivitas Sketsa 2D", format!("{} di Bidang {}", name, plane_label)),
        };

        self.record_activity(
            ducad_ui::ActivityKindUi::Sketch2D,
            action_title,
            &detail_desc,
        );
        self.record_sketch_feature(idx, action_title);
        self.check_stale_features();
    }

    #[inline]
    pub fn execute_model_command(&mut self, cmd: Box<dyn Command<ModelDoc>>, details: &str) {
        let name = cmd.name().to_string();
        let action_title = match name.as_str() {
            "Extrude" => "Extrude Solid 3D",
            "Cut Extrude" => "Cut Extrude (Potong Solid)",
            "Extrude Face" => "Tarik Sisi Solid (Push-Pull)",
            "Revolve" => "Revolve Solid 3D",
            "Revolve Face" => "Putar Sisi Solid 3D",
            "Loft" => "Loft Solid 3D",
            "Fillet" => "Fillet Sudut Lengkung",
            "Chamfer" => "Chamfer Sudut Bevel",
            "Shell" => "Shell / Hollow Berongga",
            "Shell Face" => "Shell Berlubang Sisi",
            "Boolean Union" => "Boolean Gabung (Union)",
            "Boolean Subtract" => "Boolean Potong (Subtract)",
            "Boolean Intersect" => "Boolean Irisan (Intersect)",
            "Delete Body" => "Hapus Objek Solid 3D",
            "Translate Body" => "Geser Objek Solid 3D",
            "Rotate Body" => "Putar Objek Solid 3D",
            "Scale Body" => "Ubah Skala / Resize 3D",
            _ => &name,
        };

        self.model_undo.execute(cmd, &mut self.model);
        self.clear_redo_except(crate::types::UndoTarget::Model);
        self.onboarding.note_model_command(&name);
        self.record_activity(
            ducad_ui::ActivityKindUi::Solid3D,
            action_title,
            details,
        );
    }

    #[inline]
    pub fn execute_ink_command(&mut self, cmd: Box<dyn Command<ducad_ink::InkDoc>>, details: &str) {
        let name = cmd.name().to_string();
        self.ink_undo.execute(cmd, &mut self.ink);
        self.clear_redo_except(crate::types::UndoTarget::Ink);
        self.record_activity(
            ducad_ui::ActivityKindUi::Sketch2D,
            &name,
            details,
        );
    }

    #[inline]
    pub fn undo_active_ink(&mut self) {
        self.ink_undo.undo(&mut self.ink);
    }

    #[inline]
    pub fn redo_active_ink(&mut self) {
        self.ink_undo.redo(&mut self.ink);
    }

    #[inline]
    pub fn can_undo_active_ink(&self) -> bool {
        self.ink_undo.can_undo()
    }

    #[inline]
    pub fn can_redo_active_ink(&self) -> bool {
        self.ink_undo.can_redo()
    }

    /// Aksi baru di satu domain membatalkan redo domain lain — sama seperti
    /// satu tumpukan undo biasa.
    pub(crate) fn clear_redo_except(&mut self, keep: crate::types::UndoTarget) {
        use crate::types::UndoTarget;
        if keep != UndoTarget::Sketch {
            self.sketch_set.active_mut().undo.clear_redo();
        }
        if keep != UndoTarget::Model {
            self.model_undo.clear_redo();
        }
        if keep != UndoTarget::Ink {
            self.ink_undo.clear_redo();
        }
    }

    /// Tumpukan yang memegang langkah undo paling baru, diturunkan dari
    /// stempel jam logis tiap tumpukan (lihat `UndoStack::top_undo_stamp`).
    /// Tidak ada daftar urutan terpisah yang bisa tidak sinkron akibat
    /// coalescing, transaksi `begin/commit`, atau pengusiran `max_depth`.
    pub fn next_undo_target(&self) -> Option<crate::types::UndoTarget> {
        use crate::types::UndoTarget;
        [
            (UndoTarget::Sketch, self.sketch_set.active().undo.top_undo_stamp()),
            (UndoTarget::Model, self.model_undo.top_undo_stamp()),
            (UndoTarget::Ink, self.ink_undo.top_undo_stamp()),
        ]
        .into_iter()
        .filter_map(|(target, stamp)| stamp.map(|s| (s, target)))
        .max_by_key(|(s, _)| *s)
        .map(|(_, target)| target)
    }

    /// Pasangan redo dari [`Self::next_undo_target`]: tumpukan yang langkahnya
    /// paling akhir di-undo.
    pub fn next_redo_target(&self) -> Option<crate::types::UndoTarget> {
        use crate::types::UndoTarget;
        [
            (UndoTarget::Sketch, self.sketch_set.active().undo.top_redo_stamp()),
            (UndoTarget::Model, self.model_undo.top_redo_stamp()),
            (UndoTarget::Ink, self.ink_undo.top_redo_stamp()),
        ]
        .into_iter()
        .filter_map(|(target, stamp)| stamp.map(|s| (s, target)))
        .max_by_key(|(s, _)| *s)
        .map(|(_, target)| target)
    }

    #[inline]
    pub fn undo(&mut self) {
        match self.next_undo_target() {
            Some(crate::types::UndoTarget::Sketch) => self.undo_active_sketch(),
            Some(crate::types::UndoTarget::Model) => {
                self.model_undo.undo(&mut self.model);
                self.selected_bodies.clear();
            }
            Some(crate::types::UndoTarget::Ink) => self.undo_active_ink(),
            None => {}
        }
    }

    #[inline]
    pub fn redo(&mut self) {
        match self.next_redo_target() {
            Some(crate::types::UndoTarget::Sketch) => self.redo_active_sketch(),
            Some(crate::types::UndoTarget::Model) => {
                self.model_undo.redo(&mut self.model);
                self.selected_bodies.clear();
            }
            Some(crate::types::UndoTarget::Ink) => self.redo_active_ink(),
            None => {}
        }
    }

    #[inline]
    pub fn can_undo(&self) -> bool {
        self.next_undo_target().is_some()
    }

    #[inline]
    pub fn can_redo(&self) -> bool {
        self.next_redo_target().is_some()
    }

    #[inline]
    pub fn undo_active_sketch(&mut self) {
        let id = self.active_sketch_id();
        self.sketch_set.set_active(id);
        self.sketch_set.undo();
        self.check_stale_features();
    }

    #[inline]
    pub fn redo_active_sketch(&mut self) {
        let id = self.active_sketch_id();
        self.sketch_set.set_active(id);
        self.sketch_set.redo();
        self.check_stale_features();
    }

    #[inline]
    pub fn can_undo_active_sketch(&self) -> bool {
        let id = self.active_sketch_id();
        self.sketch_set.get(id).is_some_and(|slot| slot.undo.can_undo())
    }

    #[inline]
    pub fn can_redo_active_sketch(&self) -> bool {
        let id = self.active_sketch_id();
        self.sketch_set.get(id).is_some_and(|slot| slot.undo.can_redo())
    }

    #[inline]
    pub fn sketch_undo_count(&self) -> usize {
        let id = self.active_sketch_id();
        self.sketch_set.get(id).map_or(0, |slot| slot.undo.undo_count())
    }

    #[inline]
    pub fn sketch_redo_count(&self) -> usize {
        let id = self.active_sketch_id();
        self.sketch_set.get(id).map_or(0, |slot| slot.undo.redo_count())
    }

    /// Ubah bidang kerja sketsa aktif dan selaraskan kamera.
    pub fn set_sketch_plane(&mut self, kind: PlaneKind) {
        if self.active_plane.kind != kind {
            self.selected.clear();
            self.hovered = None;
            self.pending_points.clear();
            self.pending_point_refs.clear();
            self.offset_source = None;
            self.last_snap = None;
            self.active_plane = match kind {
                PlaneKind::Top => SketchPlane::top(),
                PlaneKind::Front => SketchPlane::front(),
                PlaneKind::Right => SketchPlane::right(),
                PlaneKind::Custom(id) => self
                    .datum_planes
                    .iter()
                    .find(|dp| dp.id == id)
                    .map(|dp| dp.plane)
                    .unwrap_or_else(SketchPlane::top),
            };
        }
        self.is_sketching = true;
        self.left_toolbar.is_sketching = true;
        self.camera.orient_to_plane(&self.active_plane);
    }

    /// Aktifkan `kind` sebagai bidang sketsa lewat gestur langsung di viewport 3D.
    pub fn activate_plane_from_viewport(&mut self, kind: PlaneKind) {
        self.set_sketch_plane(kind);
        self.model_status = Some(format!(
            "Bidang '{}' kini aktif untuk sketsa",
            kind.display_label()
        ));
    }

    pub fn new_document(&mut self) {
        self.sketch_set.reset();
        self.datum_planes.clear();
        self.datum_plane_counter = 0;
        self.selected.clear();
        self.hovered = None;
        self.pending_points.clear();
        self.pending_point_refs.clear();
        self.offset_source = None;
        self.line_chain_start = None;
        self.line_chain_segments = 0;
        self.model = ModelDoc::default();
        self.model_undo = ducad_core::UndoStack::default();
        self.ink = ducad_ink::InkDoc::default();
        self.ink_undo = ducad_core::UndoStack::default();
        self.selected_bodies.clear();
        self.current_file_path = None;
        self.design = None;
        self.history_db.clear();
        self.activity_cache.clear();
        self.parametric_dag.clear();
        self.file_status = Some("Dokumen baru".to_string());
        self.measurements.clear();
        self.set_tool(ToolKind::Select);
    }

    /// Muat tiga sketsa bidang standar dari berkas, membuang seluruh sketsa
    /// lain (datum) beserta riwayat undo-nya.
    ///
    /// Format native v1 hanya menyimpan tiga bidang standar — sketsa pada
    /// datum plane TIDAK ikut tersimpan. Itu kehilangan data yang diperbaiki
    /// P0.3 (format v2); di sini perilakunya dipertahankan apa adanya supaya
    /// perubahan model sketsa tidak bercampur dengan perubahan format berkas.
    pub fn load_standard_plane_sketches(&mut self, top: Sketch, front: Sketch, right: Sketch) {
        self.sketch_set.reset();
        for (plane, loaded) in [
            (PlaneRef::Top, top),
            (PlaneRef::Front, front),
            (PlaneRef::Right, right),
        ] {
            if let Some(id) = self.sketch_set.first_on_plane(plane) {
                if let Some(slot) = self.sketch_set.sketch_mut(id) {
                    *slot = loaded;
                }
            }
        }
    }

    pub fn native_body_refs(&self) -> Vec<(&str, bool, ducad_core::Material, &KernelShape)> {
        self.model
            .doc
            .bodies
            .iter()
            .map(|(id, meta)| {
                (
                    meta.name.as_str(),
                    meta.visible,
                    meta.material,
                    &self
                        .model
                        .geometry
                        .get(id)
                        .expect("body hilang dari storage")
                        .shape,
                )
            })
            .collect()
    }

    pub fn native_export_bodies(&self) -> Vec<ducad_io::native::ExportBody<'_>> {
        self.model
            .doc
            .bodies
            .iter()
            .map(|(id, meta)| {
                let shape = &self
                    .model
                    .geometry
                    .get(id)
                    .expect("body hilang dari storage")
                    .shape;
                let round_history = self.round_history.get(&id).map(|h| {
                    let feats: Vec<ducad_io::native::NativeRoundFeature> =
                        h.features.iter().map(ducad_io::native::NativeRoundFeature::from).collect();
                    (&h.base, feats)
                });
                ducad_io::native::ExportBody {
                    name: meta.name.as_str(),
                    uuid: Some(meta.uuid.clone()),
                    visible: meta.visible,
                    material: meta.material,
                    mechanical: meta.mechanical.clone(),
                    shape,
                    round_history,
                }
            })
            .collect()
    }

    pub fn all_body_shapes(&self) -> Vec<&KernelShape> {
        self.model
            .doc
            .bodies
            .iter()
            .map(|(id, _)| {
                &self
                    .model
                    .geometry
                    .get(id)
                    .expect("body hilang dari storage")
                    .shape
            })
            .collect()
    }

    pub fn visible_body_meshes(&self) -> Vec<(&str, &KernelMesh)> {
        self.model
            .doc
            .bodies
            .iter()
            .filter(|(_, meta)| meta.visible)
            .map(|(id, meta)| {
                (
                    meta.name.as_str(),
                    self.model
                        .geometry
                        .get(id)
                        .expect("body hilang dari storage")
                        .mesh
                        .as_ref(),
                )
            })
            .collect()
    }

    pub fn visible_bodies_with_material(&self) -> Vec<(&str, ducad_core::Material, &KernelMesh)> {
        self.model
            .doc
            .bodies
            .iter()
            .filter(|(_, meta)| meta.visible)
            .map(|(id, meta)| {
                (
                    meta.name.as_str(),
                    meta.material,
                    self.model
                        .geometry
                        .get(id)
                        .expect("body hilang dari storage")
                        .mesh
                        .as_ref(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datum_plane_creation_and_indexing() {
        let top_plane = SketchPlane::top();
        let offset_plane = top_plane.offset(25.0);
        assert_eq!(offset_plane.origin, glam::Vec3::new(0.0, 0.0, 25.0));

        let angled_plane = SketchPlane::from_angle_and_edge(
            glam::Vec3::ZERO,
            glam::Vec3::new(10.0, 0.0, 0.0),
            glam::Vec3::Z,
            45.0,
        );
        let normal = angled_plane.normal;
        assert!((normal.y.abs() - normal.z.abs()).abs() < 1e-4);

        let p1 = glam::Vec3::new(0.0, 0.0, 0.0);
        let p2 = glam::Vec3::new(10.0, 0.0, 0.0);
        let p3 = glam::Vec3::new(0.0, 10.0, 5.0);
        let three_pt_plane = SketchPlane::from_3_points(p1, p2, p3).unwrap();
        assert_eq!(three_pt_plane.origin, p1);

        let planes = vec![
            (0, SketchPlane::top(), "Top".to_string()),
            (1, SketchPlane::front(), "Front".to_string()),
            (2, SketchPlane::right(), "Right".to_string()),
            (3, offset_plane, "Offset Z+25".to_string()),
        ];
        // Ray from (0, 0, 100) pointing downwards
        let hit = crate::viewport::pick_plane_index_for_ray(
            glam::Vec3::new(0.0, 0.0, 100.0),
            glam::Vec3::new(0.0, 0.0, -1.0),
            &planes,
            0,
        );
        assert_eq!(hit, Some(3));
    }

    #[test]
    fn test_delete_datum_plane_logic() {
        let mut datum_planes = vec![
            ducad_render::plane::DatumPlane::new(1, "Plane 1".to_string(), SketchPlane::top().offset(10.0)),
            ducad_render::plane::DatumPlane::new(2, "Plane 2".to_string(), SketchPlane::top().offset(20.0)),
        ];
        let mut sketches = vec![Sketch::default(), Sketch::default(), Sketch::default(), Sketch::default(), Sketch::default()];

        // Delete Plane 1 (id: 1)
        if let Some(pos) = datum_planes.iter().position(|dp| dp.id == 1) {
            let idx = pos + 3;
            datum_planes.remove(pos);
            sketches.remove(idx);
        }

        assert_eq!(datum_planes.len(), 1);
        assert_eq!(datum_planes[0].id, 2);
        assert_eq!(sketches.len(), 4);
    }

    #[test]
    fn test_native_export_bodies_with_round_history() {
        let mut app = DuCADApp::new_for_test();
        let shape1 = ducad_kernel::extrude_profile(
            &ducad_kernel::Profile::Loop(vec![
                ducad_kernel::ProfileSegment::Line { start: (0.0, 0.0), end: (10.0, 0.0) },
                ducad_kernel::ProfileSegment::Line { start: (10.0, 0.0), end: (10.0, 10.0) },
                ducad_kernel::ProfileSegment::Line { start: (10.0, 10.0), end: (0.0, 10.0) },
                ducad_kernel::ProfileSegment::Line { start: (0.0, 10.0), end: (0.0, 0.0) },
            ]),
            10.0,
        ).unwrap();
        let shape2 = ducad_kernel::extrude_profile(
            &ducad_kernel::Profile::Loop(vec![
                ducad_kernel::ProfileSegment::Line { start: (0.0, 0.0), end: (10.0, 0.0) },
                ducad_kernel::ProfileSegment::Line { start: (10.0, 0.0), end: (10.0, 10.0) },
                ducad_kernel::ProfileSegment::Line { start: (10.0, 10.0), end: (0.0, 10.0) },
                ducad_kernel::ProfileSegment::Line { start: (0.0, 10.0), end: (0.0, 0.0) },
            ]),
            10.0,
        ).unwrap();
        let id = app.model.doc.add_body_with_material("Cube", ducad_core::Material::default());
        app.model.geometry.insert(id, crate::model::BodyGeometry::from_shape(shape1));

        let feature = crate::types::RoundFeature {
            kind: crate::types::RoundKind::Vertex,
            style: crate::types::RoundStyle::Fillet,
            ray: ducad_kernel::PickRay { origin: (0.0, 0.0, 10.0), dir: (0.0, 0.0, -1.0) },
            anchor: (10.0, 10.0, 10.0),
            radius: 10.0,
            radius_end: None,
            polyline: vec![],
        };
        app.round_history.insert(id, crate::types::RoundHistory {
            base: shape2,
            features: vec![feature],
        });

        let export_bodies = app.native_export_bodies();
        assert_eq!(export_bodies.len(), 1);
        assert_eq!(export_bodies[0].name, "Cube");
        let rh = export_bodies[0].round_history.as_ref().expect("round history must exist");
        assert_eq!(rh.1.len(), 1);
        assert_eq!(rh.1[0].radius, 10.0);
    }

    #[test]
    fn undo_order_across_modes_is_global() {
        let mut app = DuCADApp::new_for_test();

        // 1. Sketch command: insert a line
        let l = ducad_sketch::Entity::Line {
            start: glam::DVec2::new(0.0, 0.0),
            end: glam::DVec2::new(10.0, 0.0),
            is_construction: false,
        };
        app.execute_sketch_command(Box::new(ducad_sketch::commands::InsertEntities::new(
            "Line",
            vec![l],
        )));
        assert_eq!(app.sketch_set.active_sketch().entities.len(), 1);

        // 2. Model command: AddSolidCommand
        let shape = ducad_kernel::make_box(10.0, 10.0, 10.0, false).unwrap();
        let geo = ducad_engine::model::BodyGeometry::from_shape(shape);
        app.execute_model_command(
            Box::new(ducad_engine::model::AddSolidCommand::new("Dummy Cube", geo)),
            "add dummy cube",
        );
        assert_eq!(app.model.doc.bodies.len(), 1);

        // 3. Ink command: add a stroke
        let bid = app.ink.brushes.keys().next().unwrap();
        let mut lm: slotmap::SlotMap<ducad_sketch::layer::LayerId, ()> = slotmap::SlotMap::with_key();
        let lid = lm.insert(());
        let s = ducad_ink::Stroke::new(
            0,
            vec![ducad_ink::InkPoint::new(0.0, 0.0, 0.5, 0.0, 0)],
            bid,
            ducad_sketch::style::Rgba([0.0, 0.0, 0.0, 1.0]),
            lid,
        );
        app.execute_ink_command(
            Box::new(ducad_ink::commands::AddStroke::new(s)),
            "dummy ink test",
        );
        assert_eq!(app.ink.strokes.len(), 1);

        // Urutan global diturunkan dari stempel: aksi terakhir (Ink) di puncak.
        assert_eq!(app.next_undo_target(), Some(crate::types::UndoTarget::Ink));

        // Undo 1: pops Ink
        app.undo();
        assert_eq!(app.ink.strokes.len(), 0);
        assert_eq!(app.model.doc.bodies.len(), 1);
        assert_eq!(app.sketch_set.active_sketch().entities.len(), 1);

        // Undo 2: pops Model
        app.undo();
        assert_eq!(app.ink.strokes.len(), 0);
        assert_eq!(app.model.doc.bodies.len(), 0);
        assert_eq!(app.sketch_set.active_sketch().entities.len(), 1);

        // Undo 3: pops Sketch
        app.undo();
        assert_eq!(app.ink.strokes.len(), 0);
        assert_eq!(app.model.doc.bodies.len(), 0);
        assert_eq!(app.sketch_set.active_sketch().entities.len(), 0);

        // Redo 1: restores Sketch
        app.redo();
        assert_eq!(app.sketch_set.active_sketch().entities.len(), 1);
        assert_eq!(app.model.doc.bodies.len(), 0);
        assert_eq!(app.ink.strokes.len(), 0);

        // Redo 2: restores Model
        app.redo();
        assert_eq!(app.sketch_set.active_sketch().entities.len(), 1);
        assert_eq!(app.model.doc.bodies.len(), 1);
        assert_eq!(app.ink.strokes.len(), 0);

        // Redo 3: restores Ink
        app.redo();
        assert_eq!(app.sketch_set.active_sketch().entities.len(), 1);
        assert_eq!(app.model.doc.bodies.len(), 1);
        assert_eq!(app.ink.strokes.len(), 1);
    }

    fn test_stroke(app: &DuCADApp, x: f32) -> ducad_ink::Stroke {
        let bid = app.ink.brushes.keys().next().unwrap();
        ducad_ink::Stroke::new(
            0,
            vec![ducad_ink::InkPoint::new(x, 0.0, 0.5, 0.0, 0)],
            bid,
            ducad_sketch::style::Rgba([0.0, 0.0, 0.0, 1.0]),
            ducad_sketch::layer::LayerId::default(),
        )
    }

    fn insert_test_line(app: &mut DuCADApp) {
        app.execute_sketch_command(Box::new(ducad_sketch::commands::InsertEntities::new(
            "Line",
            vec![ducad_sketch::Entity::line(
                glam::DVec2::ZERO,
                glam::DVec2::new(10.0, 0.0),
            )],
        )));
    }

    /// Regresi REVIEW-2026-09-24 #5: lebih dari `max_depth` aksi satu domain
    /// tidak boleh membuat undo berikutnya jadi no-op senyap.
    #[test]
    fn global_undo_stays_consistent_after_eviction() {
        let mut app = DuCADApp::new_for_test();
        insert_test_line(&mut app);
        let n = ducad_core::undo::DEFAULT_MAX_DEPTH + 5;
        for i in 0..n {
            let s = test_stroke(&app, i as f32);
            app.execute_ink_command(Box::new(ducad_ink::commands::AddStroke::new(s)), "t");
        }
        for _ in 0..ducad_core::undo::DEFAULT_MAX_DEPTH {
            app.undo();
        }
        assert_eq!(app.ink.strokes.len(), 5, "lima langkah terlama sudah terusir");
        assert_eq!(app.next_undo_target(), Some(crate::types::UndoTarget::Sketch));
        app.undo();
        assert!(app.sketch_set.active_sketch().entities.is_empty());
        assert!(!app.can_undo());
    }

    /// Beberapa `execute_sketch_command` dalam satu transaksi `begin/commit`
    /// (pola Freehand/Pen) = satu langkah undo global.
    #[test]
    fn global_undo_treats_sketch_transaction_as_one_step() {
        let mut app = DuCADApp::new_for_test();
        let s = test_stroke(&app, 0.0);
        app.execute_ink_command(Box::new(ducad_ink::commands::AddStroke::new(s)), "t");
        app.sketch_set.active_mut().undo.begin("Freehand");
        insert_test_line(&mut app);
        insert_test_line(&mut app);
        app.sketch_set.active_mut().undo.commit();

        app.undo();
        assert!(app.sketch_set.active_sketch().entities.is_empty());
        assert_eq!(app.ink.strokes.len(), 1, "undo kedua baru menyentuh tinta");
        app.undo();
        assert!(app.ink.strokes.is_empty());
    }

    /// Regresi REVIEW-2026-09-24 #4: undo sesudah membuka berkas tidak boleh
    /// menjangkau riwayat dokumen sebelumnya.
    #[test]
    fn open_native_resets_global_undo() {
        let mut app = DuCADApp::new_for_test();
        insert_test_line(&mut app);
        let path = std::env::temp_dir().join(format!(
            "ducad-app-open-reset-{}.ducad",
            std::process::id()
        ));
        app.save_native_to(path.clone());
        let s = test_stroke(&app, 0.0);
        app.execute_ink_command(Box::new(ducad_ink::commands::AddStroke::new(s)), "t");
        assert!(app.can_undo());

        app.open_native_path(path.clone());
        let _ = std::fs::remove_file(&path);

        assert_eq!(app.sketch_set.active_sketch().entities.len(), 1);
        assert!(!app.can_undo(), "riwayat dokumen lama harus hilang");
        assert!(!app.can_redo());
        app.undo();
        assert_eq!(app.sketch_set.active_sketch().entities.len(), 1);
    }
}

#[cfg(test)]
mod plane_frame_tests {
    use super::plane_frame_from;
    use ducad_engine::PlaneFrame;
    use ducad_render::SketchPlane;

    #[test]
    fn plane_frame_from_matches_engine_standard_planes() {
        assert_eq!(plane_frame_from(&SketchPlane::top()), PlaneFrame::top());
        assert_eq!(plane_frame_from(&SketchPlane::front()), PlaneFrame::front());
        assert_eq!(plane_frame_from(&SketchPlane::right()), PlaneFrame::right());
    }
}
