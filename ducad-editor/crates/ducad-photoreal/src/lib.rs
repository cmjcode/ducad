//! # ducad-photoreal
//!
//! Mode Render DUCAD: gambar akhir yang dihitung dengan *path tracing*
//! (transport cahaya fisik: bayangan lembut, pantulan antarbenda, kaca yang
//! membiaskan), bukan shading perkiraan viewport interaktif.
//!
//! Path tracer, BVH, dan lingkungan studio berasal dari TextureCraft
//! (`github.com/storytold/texturecraft`, MIT OR Apache-2.0), dipaku ke satu
//! rev di `Cargo.toml` workspace. Crate ini hanya lapisan terjemahan:
//!
//! - **Mesh**: body DUCAD (sumbu Z ke atas, satuan mm) digabung jadi satu
//!   mesh dunia bersumbu Y ke atas — konvensi TextureCraft.
//! - **Material**: TextureCraft menggambar *texture set* (tile texel di ruang
//!   UV), bukan nilai seragam. Tiap material DUCAD yang unik menjadi satu
//!   set 64×64 berisi nilai konstan, sehingga UV tidak diperlukan.
//! - **Kamera**: pasangan `eye`/`target` DUCAD menjadi kamera orbit
//!   TextureCraft (yaw/pitch tanpa roll — sama dengan turntable DUCAD).
//!
//! Dua jalur hitung dengan hasil yang konvergen ke gambar yang sama:
//! [`render_cpu`] (selalu tersedia) dan [`GpuPhoto`] (compute shader; dipakai
//! bertahap oleh aplikasi dengan perangkat GPU milik egui, atau sekali jalan
//! lewat [`render_headless`]).

mod cpu;
pub mod engine;
mod gpu;
mod scene;

pub use cpu::CpuPhoto;
pub use engine::install;

pub use gpu::GpuPhoto;
pub use scene::{
    Environment, PhotoBody, PhotoCamera, PhotoMaterial, PhotoOptions, PhotoScene, SceneOptions,
};
pub use texturecraft_render::wgpu;

/// Kegagalan menyiapkan atau menjalankan render.
#[derive(Debug, thiserror::Error)]
pub enum PhotoError {
    #[error("tidak ada geometri untuk dirender")]
    Empty,
    #[error("mesh tidak valid: {0}")]
    InvalidMesh(String),
    #[error("terlalu banyak material berbeda ({0}); batas path tracer GPU adalah {1}")]
    TooManyMaterials(usize, usize),
    #[error("GPU: {0}")]
    Gpu(String),
}

/// Gambar hasil render: RGBA8, nilai ter-encode sRGB, baris dari atas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl PhotoImage {
    /// Encode sebagai PNG RGB 8-bit bertanda sRGB (alpha selalu penuh,
    /// jadi tidak ditulis).
    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        let rgb: Vec<u8> = self
            .rgba
            .chunks_exact(4)
            .flat_map(|px| [px[0], px[1], px[2]])
            .collect();
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
            let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
            writer.write_image_data(&rgb).map_err(|e| e.to_string())?;
            writer.finish().map_err(|e| e.to_string())?;
        }
        Ok(out)
    }
}

/// Jalur hitung yang dipakai sebuah render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Compute shader di GPU.
    Gpu,
    /// Semua inti CPU (lebih lambat, selalu tersedia).
    Cpu,
}

/// Render di CPU sampai `options.samples` sampel per piksel.
///
/// `progress(selesai, total)` dipanggil setelah tiap sampel; kembalikan
/// `false` untuk berhenti lebih awal (gambar sejauh itu tetap dikembalikan).
pub fn render_cpu(
    scene: &PhotoScene,
    camera: &PhotoCamera,
    options: &PhotoOptions,
    mut progress: impl FnMut(u32, u32) -> bool,
) -> PhotoImage {
    use texturecraft_render::trace::Tracer;
    let cam = scene.camera(camera);
    let settings = options.trace_settings();
    let lit = scene.trace_lit(camera, options);
    let mut tracer = Tracer::new(options.width, options.height);
    let total = options.samples.max(1);
    for done in 1..=total {
        tracer.pass(&lit, &cam, &settings);
        if !progress(done, total) {
            break;
        }
    }
    PhotoImage {
        width: tracer.width,
        height: tracer.height,
        rgba: tracer.rgba8(&options.display()),
    }
}

/// Render sekali jalan tanpa jendela: GPU bila ada adapter yang sanggup,
/// selain itu CPU. Mengembalikan jalur yang akhirnya dipakai.
pub fn render_headless(
    scene: &PhotoScene,
    camera: &PhotoCamera,
    options: &PhotoOptions,
) -> (PhotoImage, Backend) {
    match gpu::render_headless_gpu(scene, camera, options) {
        Ok(img) => (img, Backend::Gpu),
        Err(e) => {
            log::info!("render foto: GPU tidak dipakai ({e}); beralih ke CPU");
            (render_cpu(scene, camera, options, |_, _| true), Backend::Cpu)
        }
    }
}
