//! Penyambungan mesin part eksternal, exploded view, studi gerak, dan drag
//! berbasis solver ke aplikasi (P3.1/P3.2).
//!
//! Semua logika inti ada di `ducad_core::external`, `ducad_io::external`,
//! dan `ducad_kernel::assembly_solver` — teruji di sana. Modul ini hanya
//! menjahitnya ke state aplikasi dan memastikan dua representasi posisi
//! tetap sinkron (lihat [`DuCADApp::after_body_moved`]).

use std::path::{Path, PathBuf};

use ducad_core::assembly::MotionStudy;
use ducad_core::external::{ExternalPartRef, PartSource, ResolveOutcome, SourceState};
use ducad_core::{AssemblyInstanceId, BodyId};
use ducad_io::external::{load_external_part, poll_external_source, PartMatch};
use glam::{DVec3, Vec3};
use slotmap::Key;

use crate::app::DuCADApp;
use crate::model::BodyGeometry;

impl DuCADApp {
    /// Folder dokumen aktif — dasar seluruh path relatif referensi
    /// eksternal. Dokumen yang belum pernah disimpan memakai folder kerja;
    /// referensinya akan diperbaiki otomatis (lewat path absolut cadangan)
    /// begitu dokumen disimpan di tempat lain.
    pub fn document_dir(&self) -> PathBuf {
        self.current_file_path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Folder pencarian cadangan untuk part bersama. Untuk saat ini hanya
    /// folder dokumen sendiri plus subfolder `parts/` di dalamnya —
    /// konvensi yang paling umum; pustaka part terkonfigurasi menyusul.
    fn external_search_dirs(&self) -> Vec<PathBuf> {
        let d = self.document_dir();
        vec![d.join("parts"), d]
    }

    /// Path `file` relatif terhadap folder dokumen bila memungkinkan.
    /// Bila berkasnya berada di luar pohon dokumen, hanya nama berkasnya
    /// yang disimpan sebagai path relatif dan path absolut menjadi jalur
    /// yang sebenarnya dipakai — resolusi tetap menemukannya lewat cadangan.
    fn relative_to_document(&self, file: &Path) -> String {
        let doc_dir = self.document_dir();
        file.strip_prefix(&doc_dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| {
                file.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file.to_string_lossy().to_string())
            })
    }

    /// `BodyId` lokal yang dipegang sebuah instance perakitan.
    fn body_of_instance(&self, id: AssemblyInstanceId) -> Option<BodyId> {
        let raw = self.assembly_tree.instances.get(&id)?.body_id_raw;
        self.model
            .doc
            .bodies
            .iter()
            .find(|(b, _)| b.data().as_ffi() == raw)
            .map(|(b, _)| b)
    }

    /// Instance perakitan yang memegang `body`, bila ada.
    fn instance_of_body(&self, body: BodyId) -> Option<AssemblyInstanceId> {
        let raw = body.data().as_ffi();
        self.assembly_tree
            .instances
            .iter()
            .find(|(_, i)| i.body_id_raw == raw)
            .map(|(id, _)| *id)
    }

    // ----------------------------------------------------------------
    // Part eksternal.
    // ----------------------------------------------------------------

    /// Sisipkan SEMUA body dari berkas `.ducad` lain sebagai instance
    /// eksternal. Tiap body jadi instance sendiri dengan referensi yang
    /// menunjuk UUID-nya, sehingga part multi-body tetap bisa dirujuk
    /// per-body.
    pub fn add_external_part(&mut self, file: &Path) -> Result<Vec<AssemblyInstanceId>, String> {
        let json = std::fs::read_to_string(file)
            .map_err(|e| format!("gagal membaca '{}': {e}", file.display()))?;
        let doc = ducad_io::native::deserialize_raw(&json).map_err(|e| format!("{e:#}"))?;
        if doc.bodies.is_empty() {
            return Err(format!("'{}' tidak memuat body apa pun", file.display()));
        }

        let relative = self.relative_to_document(file);
        let absolute = std::fs::canonicalize(file)
            .unwrap_or_else(|_| file.to_path_buf())
            .to_string_lossy()
            .to_string();
        let stamp = ducad_core::external::SourceStamp::of(file)
            .map_err(|e| format!("gagal mencatat cap '{}': {e}", file.display()))?;

        let mut created = Vec::with_capacity(doc.bodies.len());
        for body in &doc.bodies {
            let shape = ducad_kernel::KernelShape::from_step_string(&body.step)
                .map_err(|e| format!("geometri '{}' rusak: {e:#}", body.name))?;
            let body_id = self.model.doc.add_body_with_material(&body.name, body.material);
            self.model
                .geometry
                .insert(body_id, BodyGeometry::from_shape(shape));

            let inst_id = self
                .assembly_tree
                .add_instance(&body.name, body_id.data().as_ffi());
            let mut r = ExternalPartRef::new(&relative, &body.uuid, &body.name);
            r.last_absolute_path = Some(absolute.clone());
            r.last_seen = Some(stamp);
            if let Some(inst) = self.assembly_tree.instances.get_mut(&inst_id) {
                inst.source = Some(PartSource::External(Box::new(r)));
            }
            created.push(inst_id);
        }
        self.model.doc.dirty = true;
        self.model_status = Some(format!(
            "{} part eksternal disisipkan dari '{}'",
            created.len(),
            relative
        ));
        Ok(created)
    }

    /// Muat ulang geometri satu instance eksternal dari sumbernya, lalu
    /// pasang kembali pada posisi terakitnya dan selesaikan ulang mate.
    pub fn reload_external_part(&mut self, id: AssemblyInstanceId) -> Result<(), String> {
        let Some(r) = self
            .assembly_tree
            .instances
            .get(&id)
            .and_then(|i| i.source.as_ref())
            .and_then(|s| s.external())
            .cloned()
        else {
            return Err("instance ini bukan part eksternal".to_string());
        };
        let doc_dir = self.document_dir();
        let search = self.external_search_dirs();
        let (loaded, outcome) =
            load_external_part(&r, &doc_dir, &search).map_err(|e| format!("{e:#}"))?;

        // Geometri baru datang pada pose SUMBER; instance sudah punya
        // transform terakit hasil solver. Tanpa ini, part yang dimuat ulang
        // melompat kembali ke titik asal.
        let world = self
            .assembly_tree
            .instance_world_transform(id)
            .unwrap_or((DVec3::ZERO, glam::DQuat::IDENTITY));
        let (axis, angle) = world.1.to_axis_angle();
        let placed = ducad_kernel::apply_mate_transform_to_shape(
            &loaded.shape,
            &ducad_kernel::MateTransformResult {
                translation: (world.0.x, world.0.y, world.0.z),
                pivot: (0.0, 0.0, 0.0),
                axis: (axis.x, axis.y, axis.z),
                angle_rad: angle,
            },
        )
        .map_err(|e| format!("gagal menempatkan part: {e:#}"))?;

        let Some(body_id) = self.body_of_instance(id) else {
            return Err("body lokal instance ini hilang".to_string());
        };
        self.model
            .geometry
            .insert(body_id, BodyGeometry::from_shape(placed));
        if let Some(body) = self.model.doc.bodies.get_mut(body_id) {
            body.name = loaded.name.clone();
        }

        // Perbarui referensi: cap baru, dan — bila tadi jatuh ke pencocokan
        // nama — UUID yang sebenarnya, supaya pemuatan berikutnya kembali
        // lewat UUID. Jalur cadangan juga dinaikkan jadi jalur utama.
        if let Some(inst) = self.assembly_tree.instances.get_mut(&id) {
            inst.name = loaded.name.clone();
            if let Some(PartSource::External(r)) = inst.source.as_mut() {
                r.last_seen = Some(loaded.stamp);
                if loaded.matched_by == PartMatch::ByNameFallback {
                    r.part_uuid = loaded.resolved_uuid.clone();
                }
                if let ResolveOutcome::FoundByFallback { path, .. } = &outcome {
                    r.last_absolute_path = Some(path.to_string_lossy().to_string());
                }
            }
        }
        self.model.doc.dirty = true;
        self.solve_and_apply_assembly();

        let note = match (&outcome, loaded.matched_by) {
            (ResolveOutcome::FoundByFallback { .. }, _) => " (ditemukan lewat jalur cadangan)",
            (_, PartMatch::ByNameFallback) => " (dicocokkan lewat nama; UUID diperbarui)",
            _ => "",
        };
        self.model_status = Some(format!("'{}' dimuat ulang{note}", loaded.name));
        Ok(())
    }

    /// Periksa semua sumber eksternal TANPA memuat geometrinya. Mengembalikan
    /// yang berubah atau tidak terbaca, untuk ditampilkan sebagai lencana di
    /// panel perakitan. Dipanggil berkala, bukan tiap frame.
    pub fn poll_external_sources(&self) -> Vec<(AssemblyInstanceId, SourceState)> {
        let doc_dir = self.document_dir();
        let search = self.external_search_dirs();
        self.assembly_tree
            .external_instances()
            .map(|(id, r)| (id, poll_external_source(r, &doc_dir, &search)))
            .filter(|(_, st)| matches!(st, SourceState::Changed | SourceState::Unreadable))
            .collect()
    }

    /// Putus tautan ke berkas sumber: geometri yang sudah dimuat dipertahankan
    /// sebagai body lokal biasa.
    pub fn make_part_independent(&mut self, id: AssemblyInstanceId) {
        if let Some(inst) = self.assembly_tree.instances.get_mut(&id) {
            if inst.source.as_ref().is_some_and(|s| s.is_external()) {
                inst.source = Some(PartSource::Internal {
                    body_id_raw: inst.body_id_raw,
                });
                self.model.doc.dirty = true;
                self.model_status = Some(format!("'{}' kini part independen", inst.name));
            }
        }
    }

    // ----------------------------------------------------------------
    // Exploded view.
    // ----------------------------------------------------------------

    pub fn set_explode_factor(&mut self, f: f64) {
        self.assembly_tree.explode_factor = f.clamp(0.0, 1.0);
    }

    pub fn auto_explode(&mut self, distance: f64) {
        self.sync_assembly_instances();
        self.assembly_tree.auto_explode_radial(distance);
        if self.assembly_tree.explode_factor <= 0.0 {
            self.assembly_tree.explode_factor = 1.0;
        }
    }

    /// Pergeseran TAMPILAN sebuah body akibat exploded view — nol bila body
    /// bukan bagian perakitan atau tampilan sedang terakit. Dipakai saat
    /// membangun mesh; B-rep tidak pernah disentuh.
    pub fn explode_display_offset(&self, body: BodyId) -> Vec3 {
        if self.assembly_tree.explode_factor <= 0.0 {
            return Vec3::ZERO;
        }
        let Some(id) = self.instance_of_body(body) else {
            return Vec3::ZERO;
        };
        let (Some((shown, _)), Some((assembled, _))) = (
            self.assembly_tree.instance_display_transform(id),
            self.assembly_tree.instance_world_transform(id),
        ) else {
            return Vec3::ZERO;
        };
        (shown - assembled).as_vec3()
    }

    // ----------------------------------------------------------------
    // Studi gerak.
    // ----------------------------------------------------------------

    pub fn add_motion_study(
        &mut self,
        mate: ducad_core::MateConstraintId,
        from: f64,
        to: f64,
    ) -> Result<usize, String> {
        let Some(m) = self.assembly_tree.mates.get(&mate) else {
            return Err("mate tidak ditemukan".to_string());
        };
        if m.kind.driven_value().is_none() {
            return Err(format!(
                "mate '{}' ({}) tidak punya nilai numerik untuk digerakkan",
                m.name,
                m.kind.type_name()
            ));
        }
        self.assembly_tree.motion_studies.push(MotionStudy {
            name: format!("Gerak {}", m.name),
            driven_mate: mate,
            from,
            to,
            steps: 30,
        });
        self.model.doc.dirty = true;
        Ok(self.assembly_tree.motion_studies.len() - 1)
    }

    /// Geser playhead studi gerak ke `t` dalam [0, 1] dan terapkan hasilnya
    /// ke geometri.
    pub fn scrub_motion_study(&mut self, index: usize, t: f64) {
        let Some(study) = self.assembly_tree.motion_studies.get(index).cloned() else {
            return;
        };
        self.sync_assembly_instances();
        if ducad_kernel::evaluate_motion(&mut self.assembly_tree, &study, t).is_some() {
            // `evaluate_motion` sudah menyelesaikan tree; yang tersisa adalah
            // menerapkan pergeseran instance ke B-rep, yang dilakukan
            // `solve_and_apply_assembly` lewat solve kedua yang kini
            // konvergen seketika (residual sudah nol).
            self.solve_and_apply_assembly();
        }
    }

    // ----------------------------------------------------------------
    // Drag berbasis solver.
    // ----------------------------------------------------------------

    /// Dipanggil SETELAH sebuah body digeser langsung oleh pengguna.
    ///
    /// Menjaga dua representasi posisi tetap sinkron: B-rep di `ModelDoc`
    /// (yang dilihat render/picking/ekspor) dan `translation` instance di
    /// pohon perakitan (yang dilihat solver). Sebelum ini, menggeser body
    /// hanya memindahkan B-rep-nya; instance-nya tertinggal, dan solve
    /// berikutnya menghitung delta dari posisi yang salah.
    ///
    /// Bila body punya mate, perakitan diselesaikan ulang — sehingga part
    /// yang diseret menyamping pada mate silinder ditarik kembali ke
    /// sumbunya sambil mempertahankan pergeseran sepanjang sumbu, dan part
    /// lain yang bergantung padanya ikut bergerak.
    pub fn after_body_moved(&mut self, body: BodyId, delta: Vec3) {
        let Some(id) = self.instance_of_body(body) else {
            return;
        };
        let (_, parent_q) = self.assembly_tree.instance_parent_transform(id);
        let local = parent_q.inverse() * delta.as_dvec3();
        if let Some(inst) = self.assembly_tree.instances.get_mut(&id) {
            inst.translation.0 += local.x;
            inst.translation.1 += local.y;
            inst.translation.2 += local.z;
        }
        let has_mates = self
            .assembly_tree
            .mates
            .values()
            .any(|m| !m.suppressed && (m.target_a.instance_id == id || m.target_b.instance_id == id));
        if has_mates {
            self.solve_and_apply_assembly();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_core::assembly::{MateConstraint, MateKind, MateStatus, MateTarget, MateTargetKind};
    use ducad_io::native::{save_multi_plane_detailed, ExportBody};
    use ducad_kernel::{extrude_profile, Profile};
    use ducad_sketch::Sketch;

    fn cylinder(r: f64, h: f64) -> ducad_kernel::KernelShape {
        extrude_profile(&Profile::Circle { center: (0.0, 0.0), radius: r }, h).unwrap()
    }

    struct TempDir(PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_part_file(tag: &str, name: &str, height: f64) -> (TempDir, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "ducad-app-ext-{tag}-{}-{}",
            std::process::id(),
            ducad_core::new_part_uuid()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.ducad"));
        let shape = cylinder(5.0, height);
        let body = ExportBody {
            name,
            uuid: None,
            visible: true,
            material: ducad_core::Material::default(),
            mechanical: None,
            shape: &shape,
            round_history: None,
        };
        save_multi_plane_detailed(&path, &[&Sketch::default()], &[body]).unwrap();
        (TempDir(dir), path)
    }

    fn mesh_center(app: &DuCADApp, body: BodyId) -> [f32; 3] {
        app.model.geometry.get(body).unwrap().mesh.center()
    }

    #[test]
    fn external_part_lifecycle_add_poll_reload_detach() {
        let (_dir, path) = write_part_file("life", "Pin", 10.0);
        let mut app = DuCADApp::new_for_test();

        // Sisipkan.
        let ids = app.add_external_part(&path).expect("sisip harus berhasil");
        assert_eq!(ids.len(), 1);
        let id = ids[0];
        let inst = app.assembly_tree.instances.get(&id).unwrap();
        let r = inst.source.as_ref().and_then(|s| s.external()).expect("harus eksternal");
        // Dokumen uji belum pernah disimpan, jadi berkasnya berada di luar
        // folder dokumen: hanya nama berkas yang jadi path relatif dan path
        // absolut menjadi jalur yang sebenarnya dipakai.
        assert_eq!(r.relative_path, "Pin.ducad");
        assert!(r.last_absolute_path.is_some());
        assert!(r.last_seen.is_some(), "cap harus dicatat saat sisip");
        let body = app.body_of_instance(id).expect("body lokal harus ada");
        assert!(app.model.geometry.contains_key(body));

        // Belum ada perubahan -> tidak ada yang basi.
        assert!(app.poll_external_sources().is_empty());

        // Revisi sumber: tinggi 10 -> 30.
        let (_dir2, revised) = write_part_file("life2", "Pin", 30.0);
        std::fs::copy(&revised, &path).unwrap();
        let stale = app.poll_external_sources();
        assert_eq!(stale.len(), 1, "revisi harus terdeteksi tanpa memuat");
        assert_eq!(stale[0].0, id);

        // Muat ulang: geometri lokal harus jadi yang baru.
        app.reload_external_part(id).expect("muat ulang harus berhasil");
        let vol = app.model.geometry.get(body).unwrap().shape.volume().abs();
        let expected = std::f64::consts::PI * 25.0 * 30.0;
        assert!((vol - expected).abs() / expected < 1e-6, "volume {vol}");
        assert!(app.poll_external_sources().is_empty(), "setelah muat ulang tidak lagi basi");

        // Lepas tautan.
        app.make_part_independent(id);
        let inst = app.assembly_tree.instances.get(&id).unwrap();
        assert!(matches!(inst.source, Some(PartSource::Internal { .. })));
        assert!(app.model.geometry.contains_key(body), "geometri dipertahankan");
    }

    #[test]
    fn reload_keeps_the_part_at_its_assembled_position() {
        // Geometri muat ulang datang pada pose SUMBER; tanpa penempatan
        // ulang, part yang sudah dirakit melompat kembali ke titik asal.
        let (_dir, path) = write_part_file("pose", "Pin", 10.0);
        let mut app = DuCADApp::new_for_test();
        let id = app.add_external_part(&path).unwrap()[0];
        let body = app.body_of_instance(id).unwrap();

        // Tempatkan secara manual sejauh (40, 0, 0) lewat jalur drag.
        let moved = ducad_kernel::translate_shape(
            &app.model.geometry.get(body).unwrap().shape,
            40.0,
            0.0,
            0.0,
        )
        .unwrap();
        app.model.geometry.insert(body, BodyGeometry::from_shape(moved));
        app.after_body_moved(body, Vec3::new(40.0, 0.0, 0.0));
        let before = mesh_center(&app, body);
        assert!((before[0] - 40.0).abs() < 1e-3);

        app.reload_external_part(id).unwrap();
        let after = mesh_center(&app, body);
        assert!(
            (after[0] - 40.0).abs() < 1e-3,
            "posisi terakit harus dipertahankan setelah muat ulang: {after:?}"
        );
    }

    #[test]
    fn missing_source_file_is_reported_not_panicked() {
        let mut app = DuCADApp::new_for_test();
        let err = app
            .add_external_part(Path::new("/tmp/ducad-tidak-ada/x.ducad"))
            .unwrap_err();
        assert!(err.contains("gagal membaca"), "{err}");
    }

    #[test]
    fn dragging_a_mated_body_syncs_instance_and_snaps_back_along_constraint() {
        // Dua representasi posisi — B-rep di ModelDoc dan `translation`
        // instance — dulu lepas sinkron setelah drag. Sekarang keduanya
        // bergerak bersama, dan mate menarik balik komponen yang terkunci.
        let mut app = DuCADApp::new_for_test();
        let base_b = app.model.doc.add_body("Base");
        app.model.geometry.insert(base_b, BodyGeometry::from_shape(cylinder(5.0, 10.0)));
        let pin_b = app.model.doc.add_body("Pin");
        app.model.geometry.insert(pin_b, BodyGeometry::from_shape(cylinder(5.0, 10.0)));

        let base = app.assembly_tree.add_instance("Base", base_b.data().as_ffi());
        let pin = app.assembly_tree.add_instance("Pin", pin_b.data().as_ffi());
        app.assembly_tree.instances.get_mut(&base).unwrap().is_grounded = true;
        let axis = || MateTargetKind::CylinderAxis {
            origin: (0.0, 0.0, 0.0),
            direction: (0.0, 0.0, 1.0),
            radius: 5.0,
        };
        app.assembly_tree.mates.insert(
            1,
            MateConstraint {
                id: 1,
                name: "Sepusat".into(),
                kind: MateKind::Concentric { lock_rotation: false, aligned: true },
                target_a: MateTarget { instance_id: base, kind: axis() },
                target_b: MateTarget { instance_id: pin, kind: axis() },
                status: MateStatus::UnderConstrained,
                suppressed: false,
                limits: Default::default(),
                joint: None,
            },
        );

        // Pengguna menyeret pin menyamping DAN sepanjang sumbu.
        let delta = Vec3::new(5.0, 7.0, 30.0);
        let moved = ducad_kernel::translate_shape(
            &app.model.geometry.get(pin_b).unwrap().shape,
            delta.x as f64,
            delta.y as f64,
            delta.z as f64,
        )
        .unwrap();
        app.model.geometry.insert(pin_b, BodyGeometry::from_shape(moved));
        app.after_body_moved(pin_b, delta);

        // Instance tersinkron DAN ditarik balik ke sumbu; Z dipertahankan.
        let t = app.assembly_tree.instances.get(&pin).unwrap().translation;
        assert!(t.0.abs() < 1e-3 && t.1.abs() < 1e-3, "instance harus di sumbu: {t:?}");
        assert!((t.2 - 30.0).abs() < 1e-3, "instance harus di z=30: {t:?}");

        // Dan B-rep-nya ikut kembali ke sumbu — bukan tertinggal di (5,7).
        let c = mesh_center(&app, pin_b);
        assert!(c[0].abs() < 1e-2 && c[1].abs() < 1e-2, "B-rep harus di sumbu: {c:?}");
        assert!((c[2] - 35.0).abs() < 1e-2, "pusat silinder h=10 di z=30 -> 35: {c:?}");
        assert_eq!(app.assembly_tree.mates[&1].status, MateStatus::Satisfied);
    }

    #[test]
    fn explode_offset_is_display_only() {
        let mut app = DuCADApp::new_for_test();
        let b = app.model.doc.add_body("Part");
        app.model.geometry.insert(b, BodyGeometry::from_shape(cylinder(5.0, 10.0)));
        let id = app.assembly_tree.add_instance("Part", b.data().as_ffi());
        app.assembly_tree.instances.get_mut(&id).unwrap().is_grounded = false;
        app.assembly_tree.instances.get_mut(&id).unwrap().explode_offset = (0.0, 0.0, 80.0);

        assert_eq!(app.explode_display_offset(b), Vec3::ZERO, "faktor 0 = terakit");
        app.set_explode_factor(0.25);
        let off = app.explode_display_offset(b);
        assert!((off.z - 20.0).abs() < 1e-5, "{off:?}");
        // B-rep tidak pernah disentuh oleh explode.
        let c = mesh_center(&app, b);
        assert!((c[2] - 5.0).abs() < 1e-3, "B-rep tetap di tempat: {c:?}");
    }
}
