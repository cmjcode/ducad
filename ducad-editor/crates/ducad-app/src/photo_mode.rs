//! Mode Render: viewport diganti gambar path-traced yang makin bersih tiap
//! bingkai (lihat `ducad-photoreal`).
//!
//! Alur per bingkai, semuanya tanpa memblokir antarmuka:
//!
//! 1. **Scene** — sidik jari body terlihat (mesh, material, pergeseran
//!    explode) dan lingkungan dibandingkan dengan scene terakhir. Bila
//!    berbeda, scene baru dibangun di thread latar (gabung mesh + BVH).
//! 2. **Kamera** — kamera viewport diterjemahkan apa adanya; begitu ia atau
//!    ukuran viewport berubah, akumulasi sampel diulang dari nol.
//! 3. **Sampel** — satu pekerjaan GPU pendek per bingkai (compute shader, di
//!    perangkat wgpu milik egui — sama di Mac dan iPad). Bila GPU menolak,
//!    jalur CPU mengambil alih dengan anggaran waktu per bingkai.
//! 4. **Tampil** — tekstur hasil digambar menutupi viewport; overlay egui
//!    (HUD, popup) tetap di atasnya.

use std::hash::{Hash, Hasher};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ducad_kernel::KernelMesh;
use ducad_photoreal::{
    CpuPhoto, Environment, GpuPhoto, PhotoBody, PhotoCamera, PhotoImage, PhotoMaterial,
    PhotoOptions, PhotoScene, SceneOptions,
};

use crate::app::DuCADApp;

/// Sisi terpanjang gambar (piksel). Tablet lebih kecil: memori GPU terbatas
/// dan tiap pekerjaan harus tetap pendek supaya sentuhan terasa langsung.
const MAX_SIDE: u32 = if crate::platform::IS_MOBILE { 1024 } else { 1600 };
/// Sampel per piksel sebelum render berhenti sendiri (hemat baterai).
const TARGET_SAMPLES: u32 = if crate::platform::IS_MOBILE { 192 } else { 384 };
/// Lebar gambar lingkungan studio.
const ENV_WIDTH: u32 = if crate::platform::IS_MOBILE { 512 } else { 1024 };
/// Skala resolusi selama kamera bergerak. Satu sampel pada resolusi penuh
/// memakan puluhan milidetik GPU; pratinjau kasar menjaga orbit tetap
/// lancar, lalu gambar penuh menyusul begitu kamera diam.
const PREVIEW_SCALE: f32 = if crate::platform::IS_MOBILE { 0.33 } else { 0.4 };
/// Kamera dianggap diam setelah tidak berubah selama ini.
const SETTLE: Duration = Duration::from_millis(300);
/// Waktu kerja jalur CPU per bingkai.
const CPU_BUDGET: Duration = Duration::from_millis(14);

/// Masukan pembangun scene yang bisa dikirim ke thread latar.
struct BodyInput {
    mesh: Arc<KernelMesh>,
    material: PhotoMaterial,
    transform: Option<[[f32; 4]; 4]>,
}

type SceneResult = Result<PhotoScene, String>;

/// Kamera + ukuran yang menghasilkan akumulasi sekarang.
#[derive(Clone, Copy, PartialEq)]
struct ViewKey {
    camera: PhotoCamera,
    size: [u32; 2],
    scene: u64,
}

/// State Mode Render.
#[derive(Default)]
pub struct PhotoMode {
    pub active: bool,
    pub environment: Environment,
    render_state: Option<egui_wgpu::RenderState>,
    gpu: Option<GpuPhoto>,
    /// Alasan GPU tidak dipakai (jalur CPU aktif).
    gpu_error: Option<String>,
    cpu: Option<CpuPhoto>,
    cpu_texture: Option<egui::TextureHandle>,
    /// Sampel yang sudah ditampilkan jalur CPU.
    cpu_shown: u32,
    scene: Option<PhotoScene>,
    /// Sidik jari masukan `scene`.
    scene_key: u64,
    building: Option<(u64, mpsc::Receiver<SceneResult>)>,
    view: Option<ViewKey>,
    /// Kamera + scene yang terakhir terlihat, dan kapan terakhir berubah.
    pose: Option<(PhotoCamera, u64)>,
    moved_at: Option<Instant>,
    /// Awal akumulasi sekarang; dikosongkan setelah waktunya dicatat ke log.
    timing: Option<Instant>,
    /// Tekstur keluaran GPU yang terdaftar di egui, beserta ukurannya.
    texture: Option<(egui::TextureId, [u32; 2])>,
    samples: u32,
    /// Kegagalan membangun scene (ditampilkan di viewport).
    error: Option<String>,
}

impl PhotoMode {
    /// Simpan perangkat wgpu milik egui (dipanggil sekali saat app dibuat).
    pub fn attach(&mut self, render_state: egui_wgpu::RenderState) {
        self.render_state = Some(render_state);
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }

    pub fn target_samples(&self) -> u32 {
        TARGET_SAMPLES
    }

    /// `Some(true)` = GPU, `Some(false)` = CPU, `None` = belum mulai.
    pub fn on_gpu(&self) -> Option<bool> {
        if self.gpu.is_some() {
            Some(true)
        } else if self.cpu.is_some() {
            Some(false)
        } else {
            None
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.release();
        }
    }

    pub fn set_environment(&mut self, environment: Environment) {
        // Sidik jari scene memuat lingkungan, jadi scene dibangun ulang
        // sendiri di bingkai berikutnya.
        self.environment = environment;
    }

    /// Lepas semua sumber daya GPU/CPU (mode dimatikan).
    fn release(&mut self) {
        self.free_texture();
        self.gpu = None;
        self.cpu = None;
        self.cpu_texture = None;
        self.cpu_shown = 0;
        self.scene = None;
        self.scene_key = 0;
        self.building = None;
        self.view = None;
        self.pose = None;
        self.moved_at = None;
        self.timing = None;
        self.samples = 0;
        self.error = None;
        // `gpu_error` sengaja dipertahankan: perangkat yang menolak sekali
        // akan menolak lagi, tak perlu mencoba ulang tiap kali mode dibuka.
    }

    fn free_texture(&mut self) {
        if let (Some((id, _)), Some(rs)) = (self.texture.take(), &self.render_state) {
            rs.renderer.write().free_texture(&id);
        }
    }

    /// Gambar sekarang sebagai piksel (untuk disimpan).
    fn image(&self) -> Option<PhotoImage> {
        if let (Some(gpu), Some(rs)) = (&self.gpu, &self.render_state) {
            return gpu.read_back(&rs.device, &rs.queue);
        }
        self.cpu.as_ref().map(|c| c.image(&options([1, 1])))
    }

    fn start_build(&mut self, key: u64, bodies: Vec<BodyInput>, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        let scene_options = SceneOptions {
            environment: self.environment,
            env_width: ENV_WIDTH,
            ..SceneOptions::default()
        };
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("ducad-photo-scene".into())
            .spawn(move || {
                let photo_bodies: Vec<PhotoBody<'_>> = bodies
                    .iter()
                    .map(|b| PhotoBody {
                        positions: &b.mesh.positions,
                        normals: &b.mesh.normals,
                        indices: &b.mesh.indices,
                        transform: b.transform,
                        material: b.material,
                    })
                    .collect();
                let result =
                    PhotoScene::build(&photo_bodies, &scene_options).map_err(|e| e.to_string());
                let _ = tx.send(result);
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => self.building = Some((key, rx)),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Ambil scene yang selesai dibangun, bila ada.
    fn poll_build(&mut self) {
        let Some((key, rx)) = &self.building else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(scene)) => {
                self.scene = Some(scene);
                self.scene_key = *key;
                self.error = None;
                self.building = None;
            }
            Ok(Err(e)) => {
                self.scene = None;
                self.scene_key = *key;
                self.error = Some(e);
                self.building = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.error = Some("pembangun scene berhenti tak terduga".into());
                self.scene_key = *key;
                self.building = None;
            }
        }
    }

    /// Tambah sampel lewat GPU (bila `advance`); `false` bila GPU tidak bisa
    /// dipakai.
    fn step_gpu(
        &mut self,
        scene: &PhotoScene,
        camera: &PhotoCamera,
        size: [u32; 2],
        restart: bool,
        advance: bool,
    ) -> bool {
        if self.gpu_error.is_some() {
            return false;
        }
        let Some(rs) = self.render_state.clone() else {
            self.gpu_error = Some("tidak ada perangkat wgpu".into());
            return false;
        };
        if self.gpu.is_none() {
            match GpuPhoto::new(&rs.device, &rs.queue) {
                Ok(g) => self.gpu = Some(g),
                Err(e) => {
                    log::warn!("Mode Render: GPU tidak dipakai ({e}); beralih ke CPU");
                    self.gpu_error = Some(e.to_string());
                    return false;
                }
            }
        }
        let Some(gpu) = self.gpu.as_mut() else {
            return false;
        };
        if restart {
            gpu.restart();
        }
        if advance && gpu.samples() < TARGET_SAMPLES {
            if let Err(e) = gpu.step(&rs.device, &rs.queue, scene, camera, &options(size), 1) {
                log::warn!("Mode Render: langkah GPU gagal ({e}); beralih ke CPU");
                self.gpu_error = Some(e.to_string());
                self.free_texture();
                self.gpu = None;
                return false;
            }
        }
        self.samples = gpu.samples();
        let out_size = gpu.output_size();
        if self.texture.map(|(_, s)| s) != out_size {
            self.free_texture();
            let gpu = self.gpu.as_ref();
            if let (Some(view), Some(s)) = (gpu.and_then(|g| g.output_view()), out_size) {
                let id = rs.renderer.write().register_native_texture(
                    &rs.device,
                    view,
                    egui_wgpu::wgpu::FilterMode::Linear,
                );
                self.texture = Some((id, s));
            }
        }
        true
    }

    /// Tambah sampel lewat CPU dalam anggaran waktu satu bingkai.
    fn step_cpu(
        &mut self,
        ctx: &egui::Context,
        scene: &PhotoScene,
        camera: &PhotoCamera,
        size: [u32; 2],
        restart: bool,
        advance: bool,
    ) {
        // Jalur CPU jauh lebih lambat: setengah resolusi menjaga pratinjau
        // tetap muncul dalam hitungan detik.
        let size = [(size[0] / 2).max(16), (size[1] / 2).max(16)];
        let opts = options(size);
        let cpu = self.cpu.get_or_insert_with(|| CpuPhoto::new(size[0], size[1]));
        if restart || cpu.size() != size {
            cpu.restart(size[0], size[1]);
            self.cpu_shown = 0;
        }
        if advance && cpu.samples() < TARGET_SAMPLES {
            cpu.step(scene, camera, &opts, CPU_BUDGET);
        }
        self.samples = cpu.samples();
        if cpu.samples() > 0 && (cpu.samples() != self.cpu_shown || self.cpu_texture.is_none()) {
            let img = cpu.image(&opts);
            let color = egui::ColorImage::from_rgba_unmultiplied(
                [img.width as usize, img.height as usize],
                &img.rgba,
            );
            match &mut self.cpu_texture {
                Some(t) => t.set(color, egui::TextureOptions::LINEAR),
                None => {
                    self.cpu_texture =
                        Some(ctx.load_texture("ducad-photo-cpu", color, egui::TextureOptions::LINEAR))
                }
            }
            self.cpu_shown = cpu.samples();
        }
    }

    fn shown_texture(&self) -> Option<egui::TextureId> {
        if self.gpu.is_some() {
            self.texture.map(|(id, _)| id)
        } else {
            self.cpu_texture.as_ref().map(|t| t.id())
        }
    }
}

fn options(size: [u32; 2]) -> PhotoOptions {
    PhotoOptions {
        width: size[0],
        height: size[1],
        samples: TARGET_SAMPLES,
        ..PhotoOptions::default()
    }
}

/// Ukuran render untuk viewport `rect` (piksel fisik, dibatasi `MAX_SIDE`,
/// lalu dikali `scale` untuk pratinjau).
fn render_size(rect: egui::Rect, pixels_per_point: f32, scale: f32) -> [u32; 2] {
    let (w, h) = (
        (rect.width() * pixels_per_point).max(16.0),
        (rect.height() * pixels_per_point).max(16.0),
    );
    let scale = (MAX_SIDE as f32 / w.max(h)).min(1.0) * scale.clamp(0.05, 1.0);
    // Kelipatan 8: ukuran workgroup compute shader, dan mencegah akumulasi
    // diulang karena viewport bergeser satu piksel.
    let snap = |v: f32| (((v * scale) as u32) / 8 * 8).max(16);
    [snap(w), snap(h)]
}

impl DuCADApp {
    /// Body terlihat sebagai masukan scene, beserta sidik jarinya.
    fn photo_inputs(&self) -> (u64, Vec<BodyInput>) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.photo.environment.hash(&mut hasher);
        let mut bodies = Vec::new();
        for (id, geo) in self.model.geometry.iter() {
            let Some(body) = self.model.doc.bodies.get(id) else {
                continue;
            };
            if !body.visible || geo.mesh.indices.is_empty() {
                continue;
            }
            let material = ducad_photoreal::engine::material_of(&body.material);
            let offset = self.explode_display_offset(id);
            geo.mesh_fingerprint.hash(&mut hasher);
            for v in [
                material.base_color[0],
                material.base_color[1],
                material.base_color[2],
                material.roughness,
                material.metallic,
                material.clearcoat,
                material.transmission,
                material.specular_level,
                offset.x,
                offset.y,
                offset.z,
            ] {
                v.to_bits().hash(&mut hasher);
            }
            bodies.push(BodyInput {
                mesh: geo.mesh.clone(),
                material,
                transform: (offset != glam::Vec3::ZERO)
                    .then(|| glam::Mat4::from_translation(offset).to_cols_array_2d()),
            });
        }
        // 0 dicadangkan untuk "belum ada scene".
        (hasher.finish().max(1), bodies)
    }

    /// Jalankan satu bingkai Mode Render dan gambar hasilnya di `rect`.
    pub(crate) fn photo_frame(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        if !self.photo.active {
            return;
        }
        let (key, bodies) = self.photo_inputs();
        let camera = PhotoCamera {
            eye: self.camera.eye().to_array(),
            target: self.camera.target.to_array(),
            up: [0.0, 0.0, 1.0],
            fov_y: self.camera.fov_y,
            orthographic: matches!(self.camera.mode, ducad_render::CameraMode::Ortho2D { .. }),
        };
        let ctx = ui.ctx().clone();
        let pixels_per_point = ctx.pixels_per_point();

        let mut photo = std::mem::take(&mut self.photo);
        photo.poll_build();
        if bodies.is_empty() {
            photo.scene = None;
            photo.scene_key = 0;
            photo.building = None;
            photo.samples = 0;
            photo.error = Some(ducad_i18n::t!("png-no-bodies"));
        } else if key != photo.scene_key
            && photo.building.as_ref().map(|(k, _)| *k) != Some(key)
        {
            photo.start_build(key, bodies, &ctx);
        }

        if let Some(scene) = photo.scene.clone() {
            let pose = (camera, scene.id());
            if photo.pose != Some(pose) {
                photo.pose = Some(pose);
                photo.moved_at = Some(Instant::now());
            }
            let settling = photo.moved_at.is_some_and(|t| t.elapsed() < SETTLE);
            let size = render_size(
                rect,
                pixels_per_point,
                if settling { PREVIEW_SCALE } else { 1.0 },
            );
            let view = ViewKey {
                camera,
                size,
                scene: scene.id(),
            };
            let restart = photo.view != Some(view);
            photo.view = Some(view);
            if restart {
                photo.timing = Some(Instant::now());
            }
            // Satu sampel resolusi penuh memakan puluhan milidetik GPU dan
            // menahan bingkai. Selama pengguna berinteraksi dengan antarmuka
            // (penunjuk bergerak/ditekan) akumulasi ditunda supaya panel
            // tetap responsif; pratinjau kasar saat kamera bergerak cukup
            // murah untuk terus berjalan.
            let busy = ctx.input(|i| i.pointer.any_down() || i.pointer.is_moving());
            let advance = restart || settling || !busy;
            if !photo.step_gpu(&scene, &camera, size, restart, advance) {
                photo.step_cpu(&ctx, &scene, &camera, size, restart, advance);
            }
            if let Some(id) = photo.shown_texture() {
                ui.painter().image(
                    id,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            if photo.samples >= TARGET_SAMPLES {
                if let Some(start) = photo.timing.take() {
                    log::info!(
                        "Mode Render: {} sampel pada {}x{} dalam {:.1} dtk ({})",
                        photo.samples,
                        size[0],
                        size[1],
                        start.elapsed().as_secs_f32(),
                        if photo.gpu.is_some() { "GPU" } else { "CPU" }
                    );
                }
            }
            if settling || photo.samples < TARGET_SAMPLES {
                // Selama `settling`, bingkai berikutnya yang beralih ke
                // resolusi penuh; setelah itu sampai target tercapai.
                ctx.request_repaint();
            }
        } else if photo.building.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        let status = match (&photo.error, photo.scene.is_some()) {
            (Some(e), _) => e.clone(),
            (None, false) => ducad_i18n::t!("photo-preparing"),
            (None, true) => {
                let backend = if photo.gpu.is_some() { "GPU" } else { "CPU" };
                ducad_i18n::t!(
                    "photo-status",
                    samples = photo.samples,
                    target = TARGET_SAMPLES,
                    backend = backend
                )
            }
        };
        paint_status(ui, rect, &status);
        self.photo = photo;
    }

    /// Simpan gambar Mode Render sekarang sebagai PNG.
    pub(crate) fn photo_save_png(&mut self) {
        let Some(image) = self.photo.image() else {
            self.file_status = Some(ducad_i18n::t!("photo-nothing-to-save"));
            return;
        };
        let bytes = match image.to_png() {
            Ok(b) => b,
            Err(e) => {
                self.file_status = Some(ducad_i18n::t!("png-failed", error = e.as_str()));
                return;
            }
        };
        let filter = ducad_i18n::t!("png-filter");
        let Some(path) = self.pick_save_path(&filter, &["png"], "render.png") else {
            return;
        };
        self.file_status = Some(match std::fs::write(&path, bytes) {
            Ok(()) => {
                let name = path.display().to_string();
                ducad_i18n::t!("png-saved", name = name.as_str())
            }
            Err(e) => {
                let err = e.to_string();
                ducad_i18n::t!("png-failed", error = err.as_str())
            }
        });
    }
}

/// Pil status kecil di pojok kiri bawah viewport.
fn paint_status(ui: &egui::Ui, rect: egui::Rect, text: &str) {
    let font = egui::FontId::proportional(11.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, egui::Color32::from_gray(235));
    let pad = egui::vec2(8.0, 4.0);
    let min = egui::pos2(rect.min.x + 12.0, rect.max.y - 12.0 - galley.size().y - pad.y * 2.0);
    let bg = egui::Rect::from_min_size(min, galley.size() + pad * 2.0);
    ui.painter().rect_filled(
        bg,
        egui::CornerRadius::same(6),
        egui::Color32::from_rgba_unmultiplied(12, 14, 20, 190),
    );
    ui.painter().galley(min + pad, galley, egui::Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ukuran_render_dibatasi_dan_menjaga_aspek() {
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(2000.0, 1000.0));
        let [w, h] = render_size(rect, 2.0, 1.0);
        assert!(w <= MAX_SIDE && h <= MAX_SIDE);
        assert_eq!(w % 8, 0);
        assert_eq!(h % 8, 0);
        let aspect = w as f32 / h as f32;
        assert!((aspect - 2.0).abs() < 0.05, "aspek {aspect}");
    }

    #[test]
    fn viewport_kecil_tidak_menghasilkan_ukuran_nol() {
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(3.0, 1.0));
        let [w, h] = render_size(rect, 1.0, 1.0);
        assert!(w >= 16 && h >= 16);
    }

    #[test]
    fn pratinjau_lebih_kecil_dengan_aspek_sama() {
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0));
        let full = render_size(rect, 1.0, 1.0);
        let preview = render_size(rect, 1.0, PREVIEW_SCALE);
        assert!(preview[0] < full[0] && preview[1] < full[1]);
        let (a, b) = (
            full[0] as f32 / full[1] as f32,
            preview[0] as f32 / preview[1] as f32,
        );
        assert!((a - b).abs() < 0.05, "aspek {a} vs {b}");
    }
}
