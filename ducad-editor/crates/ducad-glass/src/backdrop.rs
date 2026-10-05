//! Backdrop GPU untuk Liquid Glass.
//!
//! Callback `egui_wgpu` menggambar langsung ke surface dan tidak bisa membaca
//! piksel yang sudah ada di sana. Jadi scene 3D dirender dulu ke tekstur
//! offscreen ([`GlassBackdrop::render_scene`]), di-blit ke viewport
//! ([`GlassBackdrop::blit`]), lalu diburamkan sekali per frame
//! ([`GlassBackdrop::finish`]). Setiap panel kaca kemudian menyampel tekstur
//! tajam + buram itu ([`GlassBackdrop::paint_panel`]).
//!
//! Hidup di `egui_wgpu::CallbackResources`, sama seperti `SceneRenderer`.

use std::num::NonZeroU64;

use egui_wgpu::wgpu;

/// Jumlah tingkat turun blur dual-Kawase (1/2, 1/4, 1/8, 1/16 resolusi).
const BLUR_LEVELS: usize = 4;

/// Parameter satu panel kaca, persis tata letak `Panel` di `glass.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PanelUniform {
    /// min.xy, size.xy (piksel framebuffer).
    pub rect: [f32; 4],
    /// radius, rim_width, refraction, dispersion.
    pub shape: [f32; 4],
    /// frost, tint_strength, specular, saturation.
    pub look: [f32; 4],
    /// tint rgb, luma_limit.
    pub tint: [f32; 4],
    /// light_dir.xy (satuan), press.xy (piksel).
    pub light: [f32; 4],
    /// rim_shade, 0, 0, 0.
    pub fx: [f32; 4],
    /// pixels_per_point, press_strength, opacity, mode (1 = terang).
    pub extra: [f32; 4],
    /// Asal + ukuran tekstur scene di framebuffer; diisi saat unggah.
    pub scene: [f32; 4],
}

impl PanelUniform {
    /// Susun parameter shader dari material dan geometri panel.
    ///
    /// `rect` dan `corner_radius` dalam point egui; dikonversi ke piksel
    /// framebuffer dengan `pixels_per_point`.
    pub fn new(
        material: &crate::GlassMaterial,
        rect: egui::Rect,
        corner_radius: f32,
        pixels_per_point: f32,
    ) -> Self {
        let ppp = pixels_per_point.max(0.01);
        let [lx, ly] = material.light_dir;
        let len = (lx * lx + ly * ly).sqrt();
        let light = if len > 1e-6 {
            [lx / len, ly / len]
        } else {
            [0.0, -1.0]
        };
        let tint = material.tint_f32();
        Self {
            rect: [
                rect.min.x * ppp,
                rect.min.y * ppp,
                rect.width() * ppp,
                rect.height() * ppp,
            ],
            shape: [
                corner_radius.max(0.0) * ppp,
                material.rim_width.max(0.0) * ppp,
                material.refraction.max(0.0) * ppp,
                material.dispersion.clamp(0.0, 1.0),
            ],
            look: [
                material.frost.clamp(0.0, 1.0),
                material.tint_strength.clamp(0.0, 1.0),
                material.specular.clamp(0.0, 1.0),
                material.saturation.max(0.0),
            ],
            tint: [tint[0], tint[1], tint[2], material.luma_limit],
            light: [light[0], light[1], 0.0, 0.0],
            fx: [material.rim_shade.clamp(0.0, 1.0), 0.0, 0.0, 0.0],
            extra: [
                ppp,
                0.0,
                1.0,
                if material.mode == crate::GlassMode::Light {
                    1.0
                } else {
                    0.0
                },
            ],
            scene: [0.0, 0.0, 1.0, 1.0],
        }
    }

    /// Titik tekan/kursor (point egui) dan kekuatannya (0 … 1).
    pub fn with_press(mut self, pos: egui::Pos2, strength: f32) -> Self {
        let ppp = self.extra[0];
        self.light[2] = pos.x * ppp;
        self.light[3] = pos.y * ppp;
        self.extra[1] = strength.clamp(0.0, 1.0);
        self
    }

    /// Opasitas keseluruhan panel (mengikuti fade-in `Area` egui).
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.extra[2] = opacity.clamp(0.0, 1.0);
        self
    }
}

const PANEL_SIZE: u64 = std::mem::size_of::<PanelUniform>() as u64;

/// Tekstur yang bergantung pada ukuran viewport.
struct Targets {
    size: [u32; 2],
    scene_view: wgpu::TextureView,
    depth_view: Option<wgpu::TextureView>,
    /// `down[i]` = resolusi 1/2^(i+1).
    down_views: Vec<wgpu::TextureView>,
    /// `up[i]` = resolusi 1/2^(i+1), hasil naik dari tingkat i+1.
    up_views: Vec<wgpu::TextureView>,
    scene_bind: wgpu::BindGroup,
    down_binds: Vec<wgpu::BindGroup>,
    up_binds: Vec<wgpu::BindGroup>,
    glass_bind: wgpu::BindGroup,
}

/// Sumber daya GPU bersama semua panel kaca.
pub struct GlassBackdrop {
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    sampler: wgpu::Sampler,
    tex_layout: wgpu::BindGroupLayout,
    glass_tex_layout: wgpu::BindGroupLayout,
    panel_layout: wgpu::BindGroupLayout,
    blit_pipeline: wgpu::RenderPipeline,
    down_pipeline: wgpu::RenderPipeline,
    up_pipeline: wgpu::RenderPipeline,
    glass_pipeline: wgpu::RenderPipeline,
    panel_stride: u64,
    panel_buf: wgpu::Buffer,
    panel_bind: wgpu::BindGroup,
    panel_capacity: usize,
    targets: Option<Targets>,
    /// Asal + ukuran scene di framebuffer (piksel), dari frame terakhir.
    scene_rect_px: [f32; 4],
    staged: Vec<PanelUniform>,
    blur_dirty: bool,
}

impl GlassBackdrop {
    /// `color_format`/`depth_format` harus sama dengan target render egui
    /// (lihat `SceneRenderer::new`): pipeline blit dan kaca digambar di dalam
    /// render pass egui, yang membawa attachment kedalaman bila diminta.
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
    ) -> Self {
        let blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ducad-glass-blur"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blur.wgsl").into()),
        });
        let glass_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ducad-glass"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/glass.wgsl").into()),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ducad-glass"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };

        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ducad-glass-tex"),
            entries: &[texture_entry(0), sampler_entry(1)],
        });
        let glass_tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ducad-glass-backdrop"),
            entries: &[
                texture_entry(0),
                texture_entry(1),
                texture_entry(2),
                sampler_entry(3),
            ],
        });
        let panel_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ducad-glass-panel"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(PANEL_SIZE),
                },
                count: None,
            }],
        });

        let blur_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ducad-glass-blur"),
            bind_group_layouts: &[Some(&tex_layout)],
            immediate_size: 0,
        });
        let glass_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ducad-glass"),
            bind_group_layouts: &[Some(&glass_tex_layout), Some(&panel_layout)],
            immediate_size: 0,
        });

        // Di dalam render pass egui: tidak menulis kedalaman, selalu lolos.
        let overlay_depth = depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: Default::default(),
            bias: Default::default(),
        });

        let fullscreen = |label: &str,
                          layout: &wgpu::PipelineLayout,
                          module: &wgpu::ShaderModule,
                          vs: &str,
                          fs: &str,
                          blend: Option<wgpu::BlendState>,
                          depth_stencil: Option<wgpu::DepthStencilState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        let blit_pipeline = fullscreen(
            "ducad-glass-blit",
            &blur_layout,
            &blur_shader,
            "vs_fullscreen",
            "fs_blit",
            None,
            overlay_depth.clone(),
        );
        let down_pipeline = fullscreen(
            "ducad-glass-down",
            &blur_layout,
            &blur_shader,
            "vs_fullscreen",
            "fs_down",
            None,
            None,
        );
        let up_pipeline = fullscreen(
            "ducad-glass-up",
            &blur_layout,
            &blur_shader,
            "vs_fullscreen",
            "fs_up",
            None,
            None,
        );
        let glass_pipeline = fullscreen(
            "ducad-glass-panel",
            &glass_layout,
            &glass_shader,
            "vs_glass",
            "fs_glass",
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            overlay_depth,
        );

        // Offset dinamis harus kelipatan batas perangkat (umumnya 256).
        let align = u64::from(device.limits().min_uniform_buffer_offset_alignment).max(1);
        let panel_stride = PANEL_SIZE.div_ceil(align) * align;
        let panel_capacity = 32;
        let (panel_buf, panel_bind) =
            create_panel_buffer(device, &panel_layout, panel_stride, panel_capacity);

        Self {
            color_format,
            depth_format,
            sampler,
            tex_layout,
            glass_tex_layout,
            panel_layout,
            blit_pipeline,
            down_pipeline,
            up_pipeline,
            glass_pipeline,
            panel_stride,
            panel_buf,
            panel_bind,
            panel_capacity,
            targets: None,
            scene_rect_px: [0.0, 0.0, 1.0, 1.0],
            staged: Vec::new(),
            blur_dirty: false,
        }
    }

    /// Apakah sudah ada scene yang bisa disampel panel kaca.
    pub fn has_scene(&self) -> bool {
        self.targets.is_some()
    }

    /// Ukuran tekstur scene saat ini (piksel), bila ada.
    pub fn scene_size(&self) -> Option<[u32; 2]> {
        self.targets.as_ref().map(|t| t.size)
    }

    /// Render scene ke tekstur offscreen seukuran `rect` (point egui).
    ///
    /// `paint` menerima render pass dengan attachment warna (+ kedalaman bila
    /// `depth_format` diisi) yang sudah dibersihkan dengan `clear`. Dipanggil
    /// dari `CallbackTrait::prepare` viewport, sebelum render pass egui.
    pub fn render_scene(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        rect: egui::Rect,
        screen: &egui_wgpu::ScreenDescriptor,
        clear: [f32; 4],
        paint: impl FnOnce(&mut wgpu::RenderPass<'_>),
    ) {
        // Pembulatan yang sama dengan viewport yang dipasang egui untuk
        // callback, supaya blit jatuh 1:1 pada piksel.
        let vp = egui::PaintCallbackInfo {
            viewport: rect,
            clip_rect: rect,
            pixels_per_point: screen.pixels_per_point,
            screen_size_px: screen.size_in_pixels,
        }
        .viewport_in_pixels();
        if vp.width_px <= 0 || vp.height_px <= 0 {
            return;
        }
        let max_dim = device.limits().max_texture_dimension_2d;
        let size = [
            (vp.width_px as u32).min(max_dim),
            (vp.height_px as u32).min(max_dim),
        ];
        self.ensure_targets(device, size);
        self.scene_rect_px = [
            vp.left_px as f32,
            vp.top_px as f32,
            vp.width_px as f32,
            vp.height_px as f32,
        ];
        let Some(targets) = &self.targets else {
            return;
        };

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ducad-glass-scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.scene_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: f64::from(clear[0]),
                        g: f64::from(clear[1]),
                        b: f64::from(clear[2]),
                        a: f64::from(clear[3]),
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: targets.depth_view.as_ref().map(|view| {
                wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        paint(&mut rpass);
        drop(rpass);
        self.blur_dirty = true;
    }

    /// Salin tekstur scene ke viewport aktif render pass (dipanggil dari
    /// `CallbackTrait::paint` viewport).
    pub fn blit(&self, rpass: &mut wgpu::RenderPass<'_>) {
        let Some(targets) = &self.targets else {
            return;
        };
        rpass.set_pipeline(&self.blit_pipeline);
        rpass.set_bind_group(0, &targets.scene_bind, &[]);
        rpass.draw(0..3, 0..1);
    }

    /// Daftarkan satu panel untuk frame ini; mengembalikan slot yang dipakai
    /// [`Self::paint_panel`]. Dipanggil dari `CallbackTrait::prepare` panel.
    pub fn stage_panel(&mut self, panel: PanelUniform) -> u32 {
        self.staged.push(panel);
        (self.staged.len() - 1) as u32
    }

    /// Unggah parameter panel dan bangun rantai blur. Dipanggil dari
    /// `CallbackTrait::finish_prepare` setiap panel; hanya panggilan pertama
    /// dalam satu frame yang bekerja, sisanya tidak melakukan apa-apa.
    pub fn finish(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.staged.is_empty() {
            return;
        }
        if self.staged.len() > self.panel_capacity {
            self.panel_capacity = self.staged.len().next_power_of_two();
            let (buf, bind) = create_panel_buffer(
                device,
                &self.panel_layout,
                self.panel_stride,
                self.panel_capacity,
            );
            self.panel_buf = buf;
            self.panel_bind = bind;
        }
        let stride = self.panel_stride as usize;
        let mut bytes = vec![0u8; self.staged.len() * stride];
        for (i, panel) in self.staged.iter().enumerate() {
            let mut panel = *panel;
            panel.scene = self.scene_rect_px;
            let src = bytemuck::bytes_of(&panel);
            bytes[i * stride..i * stride + src.len()].copy_from_slice(src);
        }
        queue.write_buffer(&self.panel_buf, 0, &bytes);
        self.staged.clear();

        if self.blur_dirty {
            self.blur_dirty = false;
            self.build_blur(encoder);
        }
    }

    /// Gambar panel kaca di viewport aktif render pass.
    pub fn paint_panel(&self, rpass: &mut wgpu::RenderPass<'_>, slot: u32) {
        let Some(targets) = &self.targets else {
            return;
        };
        if slot as usize >= self.panel_capacity {
            return;
        }
        let offset = u64::from(slot) * self.panel_stride;
        rpass.set_pipeline(&self.glass_pipeline);
        rpass.set_bind_group(0, &targets.glass_bind, &[]);
        rpass.set_bind_group(1, &self.panel_bind, &[offset as u32]);
        rpass.draw(0..3, 0..1);
    }

    fn build_blur(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(t) = &self.targets else {
            return;
        };
        // Turun: scene → 1/2 → 1/4 → …
        blur_pass(
            encoder,
            &self.down_pipeline,
            &t.scene_bind,
            &t.down_views[0],
        );
        for i in 1..BLUR_LEVELS {
            blur_pass(
                encoder,
                &self.down_pipeline,
                &t.down_binds[i - 1],
                &t.down_views[i],
            );
        }
        // Naik: tingkat terdalam → … → 1/2.
        let last = BLUR_LEVELS - 1;
        blur_pass(
            encoder,
            &self.up_pipeline,
            &t.down_binds[last],
            &t.up_views[last - 1],
        );
        for i in (0..last - 1).rev() {
            blur_pass(
                encoder,
                &self.up_pipeline,
                &t.up_binds[i + 1],
                &t.up_views[i],
            );
        }
    }

    fn ensure_targets(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let make = |label: &str, w: u32, h: u32, format: wgpu::TextureFormat| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: w.max(1),
                        height: h.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let bind = |label: &str, view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &self.tex_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        };

        let scene_view = make("ducad-glass-scene", size[0], size[1], self.color_format);
        let depth_view = self
            .depth_format
            .map(|format| make("ducad-glass-depth", size[0], size[1], format));
        let level = |i: usize| [size[0] >> (i + 1), size[1] >> (i + 1)];
        let down_views: Vec<_> = (0..BLUR_LEVELS)
            .map(|i| {
                make(
                    "ducad-glass-down",
                    level(i)[0],
                    level(i)[1],
                    self.color_format,
                )
            })
            .collect();
        let up_views: Vec<_> = (0..BLUR_LEVELS - 1)
            .map(|i| {
                make(
                    "ducad-glass-up",
                    level(i)[0],
                    level(i)[1],
                    self.color_format,
                )
            })
            .collect();

        let scene_bind = bind("ducad-glass-scene", &scene_view);
        let down_binds: Vec<_> = down_views
            .iter()
            .map(|v| bind("ducad-glass-down", v))
            .collect();
        let up_binds: Vec<_> = up_views.iter().map(|v| bind("ducad-glass-up", v)).collect();
        let glass_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ducad-glass-backdrop"),
            layout: &self.glass_tex_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene_view),
                },
                // Buram ringan = satu langkah turun; buram penuh = ujung rantai naik.
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&down_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&up_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        self.targets = Some(Targets {
            size,
            scene_view,
            depth_view,
            down_views,
            up_views,
            scene_bind,
            down_binds,
            up_binds,
            glass_bind,
        });
    }
}

fn create_panel_buffer(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    stride: u64,
    capacity: usize,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ducad-glass-panels"),
        size: stride * capacity as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ducad-glass-panels"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: NonZeroU64::new(PANEL_SIZE),
            }),
        }],
    });
    (buffer, bind)
}

fn blur_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    source: &wgpu::BindGroup,
    target: &wgpu::TextureView,
) {
    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("ducad-glass-blur"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    rpass.set_pipeline(pipeline);
    rpass.set_bind_group(0, source, &[]);
    rpass.draw(0..3, 0..1);
}
