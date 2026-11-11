//! Path tracing di GPU (compute shader), bertahap.
//!
//! Satu [`GpuPhoto`] memegang pipeline TextureCraft di atas `wgpu::Device`
//! milik pemanggil: perangkat tanpa jendela untuk CLI/MCP, atau perangkat
//! egui untuk aplikasi (termasuk iPad). Tiap [`GpuPhoto::step`] menambah
//! sampel lalu menulis gambar ter-*tone-map* ke satu tekstur yang bisa
//! langsung ditampilkan — tanpa menyalin piksel kembali ke CPU.

use texturecraft_compose::{all_tiles, tile_bytes, BakedMaps, DisplayKind};
use texturecraft_render::gpu_trace::GpuTracer;
use texturecraft_render::readback::Readback;
use texturecraft_render::{wgpu, Renderer};

use crate::scene::{PhotoCamera, PhotoOptions, PhotoScene};
use crate::{PhotoError, PhotoImage};

/// Tekstur keluaran path tracer.
struct Output {
    texture: wgpu::Texture,
    /// `Rgba8Unorm` berisi nilai ter-encode sRGB: ditulis compute shader
    /// dan disampel apa adanya untuk ditampilkan. Itu persis yang diminta
    /// egui-wgpu dari tekstur pengguna (shader-nya memperlakukan sampel
    /// sebagai nilai gamma); tampilan `Rgba8UnormSrgb` akan men-decode dua
    /// kali dan membuat gambar terlalu gelap dan jenuh.
    view: wgpu::TextureView,
    size: [u32; 2],
}

impl Output {
    fn new(device: &wgpu::Device, size: [u32; 2]) -> Output {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ducad_photo_out"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Output {
            texture,
            view,
            size,
        }
    }
}

/// Sesi path tracing GPU yang bisa dilanjutkan bingkai demi bingkai.
pub struct GpuPhoto {
    /// Renderer raster TextureCraft: hanya dipakai sebagai pemilik tekstur
    /// material (path tracer menyalinnya dari sini).
    renderer: Renderer,
    tracer: GpuTracer,
    scene_id: u64,
    /// Putaran lingkungan yang terakhir dikirim ke path tracer.
    env_rotation: f32,
    output: Option<Output>,
    /// Akumulasi sudah di-reset untuk scene + ukuran sekarang.
    ready: bool,
}

impl GpuPhoto {
    /// Apakah `device` sanggup (compute shader dan batas binding yang cukup).
    pub fn supported(device: &wgpu::Device) -> bool {
        GpuTracer::supported(device)
    }

    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<GpuPhoto, PhotoError> {
        if !Self::supported(device) {
            return Err(PhotoError::Gpu(
                "perangkat ini tidak punya compute shader atau batas binding yang cukup".into(),
            ));
        }
        let tracer = GpuTracer::new(device).map_err(PhotoError::Gpu)?;
        let renderer = Renderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm);
        Ok(GpuPhoto {
            renderer,
            tracer,
            scene_id: 0,
            env_rotation: f32::NAN,
            output: None,
            ready: false,
        })
    }

    /// Unggah `scene` (mesh, BVH, lingkungan, material). Murah bila scene
    /// yang sama sudah terunggah.
    pub fn set_scene(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &PhotoScene,
    ) -> Result<(), PhotoError> {
        if self.scene_id == scene.id() {
            return Ok(());
        }
        let trace = scene.trace();
        let sets: Vec<((u32, u32), usize)> = trace
            .sets
            .iter()
            .map(|s| ((s.composite.resolution, s.composite.height), s.material))
            .collect();
        self.renderer
            .set_texture_sets(device, queue, &sets, scene.material_count().max(1));
        let baked = BakedMaps::default();
        for (ts, set) in trace.sets.iter().enumerate() {
            for (tx, ty) in all_tiles(set.composite.resolution, set.composite.height) {
                for kind in DisplayKind::ALL {
                    let bytes = tile_bytes(&set.composite, &baked, kind, tx, ty);
                    self.renderer
                        .upload_tile(device, queue, ts, kind, tx, ty, &bytes);
                }
            }
        }
        self.scene_id = scene.id();
        // Geometri dan lingkungan dikirim bersama putaran lampu di `step`.
        self.env_rotation = f32::NAN;
        self.ready = false;
        Ok(())
    }

    /// Buang sampel yang terkumpul (kamera atau pengaturan berubah).
    pub fn restart(&mut self) {
        self.ready = false;
    }

    /// Sampel per piksel yang sudah terkumpul.
    pub fn samples(&self) -> u32 {
        if self.ready {
            self.tracer.samples()
        } else {
            0
        }
    }

    /// Tambah `spp` sampel pada ukuran `options.width × options.height`
    /// lalu perbarui tekstur keluaran. Mengembalikan total sampel.
    ///
    /// Satu panggilan = satu pekerjaan GPU pendek; aplikasi memanggilnya
    /// sekali per bingkai supaya antarmuka tetap responsif.
    pub fn step(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &PhotoScene,
        camera: &PhotoCamera,
        options: &PhotoOptions,
        spp: u32,
    ) -> Result<u32, PhotoError> {
        self.set_scene(device, queue, scene)?;
        let rotation = scene.env_rotation(camera, options);
        if rotation.to_bits() != self.env_rotation.to_bits() {
            // Murah bila mesh dan lingkungan sama: hanya konstanta shading
            // yang ditulis ulang.
            self.tracer
                .set_scene(device, queue, &scene.trace_lit(camera, options))
                .map_err(PhotoError::Gpu)?;
            self.env_rotation = rotation;
            self.ready = false;
        }
        let size = [options.width.clamp(1, 8192), options.height.clamp(1, 8192)];
        let encoder = || {
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ducad_photo"),
            })
        };
        if self.output.as_ref().map(|o| o.size) != Some(size) {
            self.output = Some(Output::new(device, size));
            self.ready = false;
        }
        if !self.ready {
            let mut enc = encoder();
            self.tracer
                .sync_materials(device, queue, &mut enc, &self.renderer)
                .map_err(PhotoError::Gpu)?;
            self.tracer
                .reset(device, &mut enc, size[0], size[1])
                .map_err(PhotoError::Gpu)?;
            queue.submit([enc.finish()]);
            self.ready = true;
        }
        let cam = scene.camera(camera);
        let mut enc = encoder();
        if !self.tracer.pass(
            device,
            queue,
            &mut enc,
            &cam,
            &options.trace_settings(),
            spp.max(1),
        ) {
            return Err(PhotoError::Gpu("scene path tracer belum lengkap".into()));
        }
        if let Some(out) = &self.output {
            self.tracer
                .resolve(device, queue, &mut enc, &options.display(), &out.view);
        }
        queue.submit([enc.finish()]);
        Ok(self.tracer.samples())
    }

    /// Tampilan tekstur keluaran (`Rgba8Unorm`, nilai ter-encode sRGB),
    /// untuk didaftarkan ke egui (`register_native_texture`). Berubah bila
    /// ukuran render berubah.
    pub fn output_view(&self) -> Option<&wgpu::TextureView> {
        self.output.as_ref().map(|o| &o.view)
    }

    /// Ukuran tekstur keluaran sekarang.
    pub fn output_size(&self) -> Option<[u32; 2]> {
        self.output.as_ref().map(|o| o.size)
    }

    /// Salin gambar sekarang ke CPU (memblokir sampai GPU selesai).
    pub fn read_back(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Option<PhotoImage> {
        let out = self.output.as_ref()?;
        let (width, height, rgba) = Readback::blocking(device, queue, &out.texture)?;
        Some(PhotoImage {
            width,
            height,
            rgba,
        })
    }
}

/// Render sekali jalan di perangkat GPU tanpa jendela.
pub(crate) fn render_headless_gpu(
    scene: &PhotoScene,
    camera: &PhotoCamera,
    options: &PhotoOptions,
) -> Result<PhotoImage, PhotoError> {
    let instance = wgpu::Instance::default();
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| PhotoError::Gpu(format!("tidak ada adapter: {e}")))?;
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .map_err(|e| PhotoError::Gpu(format!("tidak ada device: {e}")))?;
    let mut gpu = GpuPhoto::new(&device, &queue)?;
    let total = options.samples.max(1);
    // Beberapa sampel per pekerjaan pada render kecil, kira-kira satu pada
    // 1080p — pekerjaan GPU tetap pendek.
    let pixels = (options.width.max(1) * options.height.max(1)).max(1);
    let chunk = (16_000_000 / pixels).clamp(1, 16);
    let mut done = 0;
    while done < total {
        done = gpu.step(&device, &queue, scene, camera, options, chunk.min(total - done))?;
    }
    gpu.read_back(&device, &queue)
        .ok_or_else(|| PhotoError::Gpu("gambar tidak terbaca balik dari GPU".into()))
}

/// Jalankan future wgpu yang di native selesai tanpa menunggu I/O.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Waker};
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = future.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}
