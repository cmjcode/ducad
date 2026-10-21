//! Shell/Hollow di thread latar. Shell pada body non-konveks (gabungan dua
//! objek, body berfillet) menempuh jalur kernel yang menjalankan belasan
//! boolean; bila dijalankan sinkron, UI membeku selama itu. Pekerjaan
//! (compute + verifikasi kandidat perbaikan bila gagal) dikirim ke thread
//! sendiri dan hasilnya diambil tiap frame lewat [`DuCADApp::poll_shell_job`].
//!
//! Hasil hanya diterapkan bila geometri body belum berubah sejak dikirim
//! (`mesh_ptr`); bila pengguna sempat mengubah body itu, hasilnya dibuang.
//! Dalam tes (`sync_kernel_ops`) pekerjaan dijalankan di tempat supaya
//! urutan panggilan tetap deterministik.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use ducad_core::BodyId;
use ducad_engine::compute::{self, FacePick};
use ducad_engine::OpError;
use ducad_kernel::KernelShape;

use crate::app::DuCADApp;
use crate::error_card_ui::{shell_fix_label, shell_fix_values, GuiFix};
use crate::model::{BodyGeometry, ReplaceGeometryCommand};
use crate::modeling::operations::ShellPick;
use crate::types::{PickMode, ToolKind};

/// Satu permintaan shell: parameter persis seperti yang diberikan pengguna.
pub(crate) struct ShellJob {
    pub id: BodyId,
    pub label: &'static str,
    pub pick: ShellPick,
    pub thickness: f64,
    pub depth: f64,
    /// Identitas mesh body saat job dikirim (`Arc::as_ptr`); bila berbeda
    /// saat hasil tiba, body sudah diubah dan hasil dibuang.
    pub mesh_ptr: usize,
    /// Status yang ditampilkan bila sukses (`None` = kosongkan status).
    pub done_status: Option<String>,
}

/// Hasil compute: geometri baru, atau galat beserta kandidat perbaikan
/// yang sudah diverifikasi (di thread yang sama, bukan di UI).
pub(crate) type ShellOutcome = Result<BodyGeometry, Box<(OpError, Vec<GuiFix>)>>;

/// Job yang sedang berjalan di thread latar.
pub struct PendingShell {
    receiver: Receiver<(ShellJob, ShellOutcome)>,
    started: Instant,
}

impl ShellPick {
    fn face_pick(&self) -> FacePick<'_> {
        match self {
            ShellPick::Rays(rays) => FacePick::Rays(rays),
            ShellPick::Direction(dir) => FacePick::Farthest(*dir),
        }
    }
}

/// Compute murni (boleh dari thread mana pun): shell, lalu bila gagal
/// verifikasi kandidat perbaikan pada shape yang sama.
pub(crate) fn compute_shell_job(shape: &KernelShape, job: &ShellJob) -> ShellOutcome {
    let pick = job.pick.face_pick();
    match compute::shell(shape, &pick, job.thickness, job.depth) {
        Ok(geo) => Ok(geo),
        Err(e) => {
            let fixes = shell_fix_values(shape, &e, job.thickness, job.depth, &pick);
            Err(Box::new((e, fixes)))
        }
    }
}

impl DuCADApp {
    /// Kirim job shell ke thread latar (atau jalankan di tempat bila
    /// `sync_kernel_ops` / thread gagal dibuat).
    pub(crate) fn submit_shell_job(&mut self, shape: KernelShape, job: ShellJob) {
        if self.sync_kernel_ops {
            let outcome = compute_shell_job(&shape, &job);
            self.apply_shell_result(job, outcome);
            return;
        }
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("ducad-shell".to_string())
            .spawn(move || {
                let outcome = compute_shell_job(&shape, &job);
                let _ = tx.send((job, outcome));
            });
        match spawned {
            Ok(_) => {
                self.shell_job = Some(PendingShell { receiver: rx, started: Instant::now() });
                self.model_status = Some(ducad_i18n::t!("shell-computing"));
            }
            Err(e) => {
                log::error!("thread shell gagal dibuat ({e}); shell berjalan di UI thread");
                // `job` sudah dipindah ke closure yang gagal dijalankan; closure
                // itu dibuang bersama galat, jadi job hilang. Beri tahu pengguna.
                self.model_status = Some(ducad_i18n::t!("shell-worker-lost"));
            }
        }
    }

    /// Dipanggil tiap frame: terapkan hasil shell yang sudah siap. Selama
    /// job masih berjalan frame berikutnya diminta supaya poll tetap jalan.
    pub(crate) fn poll_shell_job(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.shell_job else {
            return;
        };
        match pending.receiver.try_recv() {
            Ok((job, outcome)) => {
                self.shell_job = None;
                self.apply_shell_result(job, outcome);
                ctx.request_repaint();
            }
            Err(TryRecvError::Empty) => {
                let secs = pending.started.elapsed().as_secs();
                self.model_status = Some(if secs >= 2 {
                    ducad_i18n::t!("shell-computing-long", seconds = secs)
                } else {
                    ducad_i18n::t!("shell-computing")
                });
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Err(TryRecvError::Disconnected) => {
                // Thread panik sebelum mengirim hasil (kernel sudah dipagari
                // `guard`, jadi ini jarang): model tidak tersentuh.
                self.shell_job = None;
                self.model_status = Some(ducad_i18n::t!("shell-worker-lost"));
                ctx.request_repaint();
            }
        }
    }

    /// Terapkan hasil ke model — sama persis dengan jalur sinkron lama.
    pub(crate) fn apply_shell_result(&mut self, job: ShellJob, outcome: ShellOutcome) {
        let ShellJob { id, label, pick, thickness, depth, mesh_ptr, done_status } = job;
        let current = self
            .model
            .geometry
            .get(id)
            .map(|g| std::sync::Arc::as_ptr(&g.mesh) as usize);
        if current != Some(mesh_ptr) {
            self.model_status = Some(ducad_i18n::t!("shell-stale"));
            return;
        }
        match outcome {
            Ok(new_geo) => {
                let details = if depth > 0.0 {
                    format!(
                        "Membuat rongga hollow dinding tebal {:.1} mm, kedalaman {:.1} mm",
                        thickness, depth
                    )
                } else {
                    format!("Membuat rongga hollow dinding tebal {:.1} mm", thickness)
                };
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new(label, id, new_geo)),
                    &details,
                );
                let (open_rays, open_direction) = match pick {
                    ShellPick::Rays(rays) => (rays, None),
                    ShellPick::Direction(dir) => (Vec::new(), Some(dir)),
                };
                self.record_shell_feature(thickness, &open_rays, open_direction, depth);
                self.round_history.remove(&id);
                self.selected_faces.clear();
                self.active_face = None;
                // Tool Shell tetap dalam mode pilih-face supaya body berikutnya
                // bisa langsung diklik; jalur lain mengembalikan mode normal.
                if self.tool != ToolKind::Shell {
                    self.picking_mode = PickMode::None;
                }
                self.model_status = done_status;
            }
            Err(failure) => {
                let (e, fixes) = *failure;
                self.model_status = Some(crate::modeling::operations::op_status("Shell", &e));
                let fixes: Vec<(String, GuiFix)> =
                    fixes.into_iter().map(|f| (shell_fix_label(&f), f)).collect();
                self.show_op_error(&e, fixes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_engine::compute::PrimitiveShape;
    use ducad_kernel::PickRay;

    fn app_with_box() -> (DuCADApp, BodyId) {
        let mut app = DuCADApp::new_for_test();
        app.sync_kernel_ops = false;
        let geo = compute::primitive(
            &PrimitiveShape::Box { size: [20.0, 20.0, 20.0], centered: false },
            [0.0; 3],
        )
        .unwrap();
        let id = app.model.doc.add_body("Box");
        app.model.geometry.insert(id, geo);
        app.selected_bodies = [id].into_iter().collect();
        app.selected_faces = vec![PickRay { origin: (10.0, 10.0, 100.0), dir: (0.0, 0.0, -1.0) }];
        app.shell_thickness_input = "2".to_string();
        (app, id)
    }

    fn wait(app: &mut DuCADApp) {
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(30);
        while app.shell_job.is_some() {
            assert!(Instant::now() < deadline, "shell job tidak selesai");
            app.poll_shell_job(&ctx);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn background_shell_is_applied_when_it_arrives() {
        let (mut app, id) = app_with_box();
        app.commit_shell();
        assert!(app.shell_job.is_some(), "job harus berjalan di thread latar");
        assert!(app.model_status.as_deref().is_some_and(|s| s.contains("shell")));
        // Submit kedua selama sibuk ditolak, model tidak disentuh.
        app.commit_shell();
        wait(&mut app);
        let v = app.model.geometry[id].shape.volume();
        assert!((v - (8000.0 - 16.0 * 16.0 * 18.0)).abs() < 1e-3, "{v}");
        assert!(app.model_status.is_none(), "{:?}", app.model_status);
    }

    #[test]
    fn background_shell_result_is_discarded_when_body_changed() {
        let (mut app, id) = app_with_box();
        app.commit_shell();
        // Body diganti selama job berjalan: hasil shell tidak boleh menimpanya.
        let geo = compute::primitive(
            &PrimitiveShape::Box { size: [10.0, 10.0, 10.0], centered: false },
            [0.0; 3],
        )
        .unwrap();
        app.model.geometry.insert(id, geo);
        wait(&mut app);
        let v = app.model.geometry[id].shape.volume();
        assert!((v - 1000.0).abs() < 1e-6, "{v}");
        assert!(app.model_status.is_some());
    }

    #[test]
    fn background_shell_failure_shows_verified_fixes() {
        let (mut app, id) = app_with_box();
        app.shell_depth_input = "19".to_string();
        app.commit_shell();
        wait(&mut app);
        assert!(app.error_card.open);
        assert_eq!(app.error_fixes.len(), 2, "{:?}", app.error_fixes);
        let v = app.model.geometry[id].shape.volume();
        assert!((v - 8000.0).abs() < 1e-6, "{v}");
    }
}
