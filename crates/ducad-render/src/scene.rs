use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use glam::{Mat4, Vec3};

use crate::grid;

/// Bidang potong "tidak aktif" — normal nol vektor + offset sangat besar,
/// jadi `dot(0, world) - w` selalu sangat negatif dan tidak pernah lolos
/// syarat `> 0.0` di `fs_mesh` (lihat `shader.wgsl`), berapa pun posisi
/// mesh-nya. Dipakai `SceneRenderer::new`/`set_clip_plane(None)`.
const CLIP_PLANE_DISABLED: [f32; 4] = [0.0, 0.0, 0.0, 1.0e9];

/// Konfigurasi inspeksi garis zebra (Fase 3.1 Zebra Stripes Reflection Shader).
/// Memproyeksikan refleksi specular garis-garis berfrekuensi tinggi untuk
/// mengevaluasi kontinuitas tangensial (G1) dan kurvatur (G2) pada permukaan CAD.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZebraConfig {
    pub enabled: bool,
    /// Frekuensi atau kerapatan garis (default: 20.0).
    pub frequency: f32,
    /// Orientasi sudut garis dalam radian (0.0 = horizontal, PI/2 = vertikal).
    pub angle: f32,
    /// Faktor pencampuran antara warna shading standar dan zebra (0.0 = standar, 1.0 = zebra penuh).
    pub blend: f32,
}

impl Default for ZebraConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            frequency: 20.0,
            angle: 0.0,
            blend: 1.0,
        }
    }
}

/// Konfigurasi inspeksi sudut lepas cetakan (Fase 3.2 Draft Angle Heatmap Inspector).
/// Mewarnai permukaan 3D secara real-time berdasarkan sudut terhadap arah buka cetakan (*pull direction*):
/// - Hijau: Sudut aman (>= target_angle_deg, e.g. >= 1.0°)
/// - Kuning: Sudut kritis / butuh kemiringan draft (0° s/d target_angle_deg)
/// - Merah: Undercut (< 0°) yang tidak bisa lepas dari cetakan
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DraftConfig {
    pub enabled: bool,
    /// Arah buka cetakan (pull direction) ternormalisasi, default: [0.0, 0.0, 1.0] (+Z).
    pub pull_dir: [f32; 3],
    /// Sudut kemiringan aman target dalam derajat (default: 1.0°).
    pub target_angle_deg: f32,
    /// Faktor pencampuran antara warna shading standar dan heatmap (0.0 = standar, 1.0 = heatmap penuh).
    pub blend: f32,
}

impl Default for DraftConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            pull_dir: [0.0, 0.0, 1.0],
            target_angle_deg: 1.0,
            blend: 1.0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    light_dir: [f32; 4],
    /// Section view (Fase 7) — lihat komentar `clip_plane` di `shader.wgsl`.
    clip_plane: [f32; 4],
    /// Zebra stripes reflection (Fase 3.1) — [enabled (0.0/1.0), freq, angle, blend].
    zebra_params: [f32; 4],
    /// Draft angle heatmap (Fase 3.2) — [enabled (0.0/1.0), target_rad, blend, reserved].
    draft_params: [f32; 4],
    /// Pull direction vector — [x, y, z, 0.0].
    draft_dir: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

struct GpuMesh {
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    index_count: u32,
}

/// Renderer scene 3D yang hidup di dalam `CallbackResources` egui_wgpu.
/// `prepare()` dipanggil per-frame sebelum render pass, `paint()` di dalamnya.
pub struct SceneRenderer {
    globals_buf: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    grid_pipeline: wgpu::RenderPipeline,
    grid_vbuf: wgpu::Buffer,
    grid_vertex_count: u32,
    mesh_pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    gizmo_pipeline: wgpu::RenderPipeline,
    mesh: Option<GpuMesh>,
    /// Mesh solid gizmo push/pull & rounding (Fase 9 — Icon Gizmo Profesional):
    /// buffer TERPISAH dari `mesh` (body CAD) supaya upload-nya independen
    /// tiap frame (gizmo cuma ada saat ada seleksi/hover aktif) tanpa perlu
    /// menggabung-satukan index body + gizmo jadi satu draw call raksasa.
    /// Dipakai gizmo_pipeline dengan shading fs_mesh dan depth test Always
    /// sehingga gizmo selalu tampak di depan dan tidak terkubur di dalam body.
    gizmo_mesh: Option<GpuMesh>,
    /// Garis overlay 2D (entitas sketch, preview, glyph snap) — dibangun
    /// ulang tiap frame lewat `set_overlay_lines`, memakai overlay_pipeline.
    overlay_vbuf: Option<wgpu::Buffer>,
    overlay_vertex_count: u32,
    /// Section view (Fase 7) — lihat `set_clip_plane`.
    clip_plane: [f32; 4],
    /// Zebra stripes reflection config (Fase 3.1).
    zebra_config: ZebraConfig,
    /// Draft angle heatmap inspector config (Fase 3.2).
    draft_config: DraftConfig,
    current_grid_plane: Option<crate::plane::SketchPlane>,
    current_grid_extent: f32,
    current_grid_step: f32,
}

impl SceneRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
    ) -> Self {
        use wgpu::util::DeviceExt;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ducad-scene"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ducad-scene"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });

        let depth_stencil = depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        });

        let top_depth_stencil = depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: Default::default(),
            bias: Default::default(),
        });

        let color_target = [Some(wgpu::ColorTargetState {
            format: color_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];

        let grid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grid"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_line"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<grid::LineVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_line"),
                compilation_options: Default::default(),
                targets: &color_target,
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: depth_stencil.clone(),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_line"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<grid::LineVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_line"),
                compilation_options: Default::default(),
                targets: &color_target,
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: top_depth_stencil.clone(),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_mesh"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_mesh"),
                compilation_options: Default::default(),
                targets: &color_target,
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let gizmo_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gizmo-mesh"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_mesh"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_mesh"),
                compilation_options: Default::default(),
                targets: &color_target,
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: top_depth_stencil,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let grid_verts = grid::generate_grid(500.0, 10.0);
        let grid_vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid"),
            contents: bytemuck::cast_slice(&grid_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            globals_buf,
            globals_bind,
            grid_pipeline,
            grid_vbuf,
            grid_vertex_count: grid_verts.len() as u32,
            mesh_pipeline,
            overlay_pipeline,
            gizmo_pipeline,
            mesh: None,
            gizmo_mesh: None,
            overlay_vbuf: None,
            overlay_vertex_count: 0,
            clip_plane: CLIP_PLANE_DISABLED,
            zebra_config: ZebraConfig::default(),
            draft_config: DraftConfig::default(),
            current_grid_plane: Some(crate::plane::SketchPlane::top()),
            current_grid_extent: 500.0,
            current_grid_step: 10.0,
        }
    }

    /// Konfigurasi inspeksi garis zebra (Fase 3.1 Zebra Stripes Reflection Shader).
    pub fn set_zebra_config(&mut self, config: ZebraConfig) {
        self.zebra_config = config;
    }

    /// Dapatkan konfigurasi inspeksi garis zebra yang sedang aktif.
    pub fn zebra_config(&self) -> ZebraConfig {
        self.zebra_config
    }

    /// Konfigurasi inspeksi sudut lepas cetakan (Fase 3.2 Draft Angle Heatmap Inspector).
    pub fn set_draft_config(&mut self, config: DraftConfig) {
        self.draft_config = config;
    }

    /// Dapatkan konfigurasi inspeksi sudut lepas yang sedang aktif.
    pub fn draft_config(&self) -> DraftConfig {
        self.draft_config
    }

    /// Bidang potong section view (Fase 7): `Some((normal, offset))`
    /// membuang (di-`discard` di `fs_mesh`) fragment mesh di sisi `normal`
    /// yang JAUH dari origin sepanjang `offset` — cuma memotong tampilan,
    /// tidak pernah menyentuh geometri B-rep asli (beda dari operasi
    /// Boolean kernel), jadi aman digeser tiap frame tanpa memanggil OCCT
    /// sama sekali. `None` menonaktifkan.
    pub fn set_clip_plane(&mut self, plane: Option<(Vec3, f32)>) {
        self.clip_plane = match plane {
            Some((normal, offset)) => {
                let n = normal.normalize_or_zero();
                [n.x, n.y, n.z, offset]
            }
            None => CLIP_PLANE_DISABLED,
        };
    }

    /// Upload garis overlay 2D (sketch) untuk frame ini. Dipanggil dari
    /// `prepare()` callback, jadi `device` tersedia untuk buat buffer baru
    /// tiap frame — cukup murah untuk skala sketch Fase 1 (ratusan-ribuan
    /// vertex); dioptimalkan (buffer yang di-resize, bukan dibuat ulang)
    /// di Fase 7 kalau profiling menunjukkan perlu.
    pub fn set_overlay_lines(&mut self, device: &wgpu::Device, verts: &[grid::LineVertex]) {
        use wgpu::util::DeviceExt;
        if verts.is_empty() {
            self.overlay_vbuf = None;
            self.overlay_vertex_count = 0;
            return;
        }
        let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("overlay"),
            contents: bytemuck::cast_slice(verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.overlay_vbuf = Some(buf);
        self.overlay_vertex_count = verts.len() as u32;
    }

    /// Perbarui buffer grid untuk bidang sketsa tertentu dengan extent & step dinamis.
    pub fn set_grid_plane_with_extent(
        &mut self,
        device: &wgpu::Device,
        plane: &crate::plane::SketchPlane,
        half_extent: f32,
        step: f32,
    ) {
        if self.current_grid_plane == Some(*plane)
            && (self.current_grid_extent - half_extent).abs() < 1e-3
            && (self.current_grid_step - step).abs() < 1e-3
        {
            return;
        }

        use wgpu::util::DeviceExt;
        let grid_verts = grid::generate_grid_for_plane(plane, half_extent, step);
        self.grid_vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid"),
            contents: bytemuck::cast_slice(&grid_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.grid_vertex_count = grid_verts.len() as u32;
        self.current_grid_plane = Some(*plane);
        self.current_grid_extent = half_extent;
        self.current_grid_step = step;
    }

    /// Perbarui buffer grid untuk bidang sketsa tertentu (`Top`, `Front`, `Right`) dengan ukuran default.
    pub fn set_grid_plane(&mut self, device: &wgpu::Device, plane: &crate::plane::SketchPlane) {
        self.set_grid_plane_with_extent(device, plane, 500.0, 10.0);
    }

    /// Upload mesh body (dari ducad-kernel) untuk ditampilkan. Body kosong
    /// (tidak ada body, atau semua tersembunyi) membersihkan mesh yang
    /// sedang tampil — wgpu menolak buffer berukuran 0, jadi early-return
    /// ke `self.mesh = None` (sama pola dengan `set_overlay_lines`) alih-
    /// alih coba bikin buffer kosong.
    pub fn set_mesh(
        &mut self,
        device: &wgpu::Device,
        positions: &[[f32; 3]],
        normals: &[[f32; 3]],
        colors: Option<&[[f32; 4]]>,
        indices: &[u32],
    ) {
        use wgpu::util::DeviceExt;
        if indices.is_empty() {
            self.mesh = None;
            return;
        }
        const DEFAULT_CAD_GREY: [f32; 4] = [0.62, 0.68, 0.76, 1.0];
        let verts: Vec<MeshVertex> = positions
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let n = normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]);
                let color = colors.and_then(|c| c.get(i).copied()).unwrap_or(DEFAULT_CAD_GREY);
                MeshVertex {
                    position: *p,
                    normal: n,
                    color,
                }
            })
            .collect();
        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-vb"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-ib"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.mesh = Some(GpuMesh {
            vertex_buf,
            index_buf,
            index_count: indices.len() as u32,
        });
    }

    /// Upload mesh solid gizmo (push/pull & rounding, Fase 9) — mirror persis
    /// `set_mesh` di atas (SoA `positions`/`normals`/`colors`/`indices`, buffer
    /// terpisah kosong = `None` saat tidak ada gizmo aktif frame ini), TAPI
    /// disimpan di `self.gizmo_mesh` yang independen dari body supaya body
    /// tetap tampil walau gizmo kosong (dan sebaliknya).
    pub fn set_gizmo_mesh(
        &mut self,
        device: &wgpu::Device,
        positions: &[[f32; 3]],
        normals: &[[f32; 3]],
        colors: &[[f32; 4]],
        indices: &[u32],
    ) {
        use wgpu::util::DeviceExt;
        if indices.is_empty() {
            self.gizmo_mesh = None;
            return;
        }
        const DEFAULT_GIZMO_COLOR: [f32; 4] = [0.0, 0.78, 1.0, 1.0];
        let verts: Vec<MeshVertex> = positions
            .iter()
            .enumerate()
            .map(|(i, p)| MeshVertex {
                position: *p,
                normal: normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]),
                color: colors.get(i).copied().unwrap_or(DEFAULT_GIZMO_COLOR),
            })
            .collect();
        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gizmo-mesh-vb"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gizmo-mesh-ib"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.gizmo_mesh = Some(GpuMesh {
            vertex_buf,
            index_buf,
            index_count: indices.len() as u32,
        });
    }

    pub fn prepare(&mut self, queue: &wgpu::Queue, view_proj: Mat4, eye: Vec3) {
        let light = Vec3::new(0.4, 0.3, 0.85).normalize();
        let zebra_params = [
            if self.zebra_config.enabled { 1.0 } else { 0.0 },
            self.zebra_config.frequency.max(1.0),
            self.zebra_config.angle,
            self.zebra_config.blend.clamp(0.0, 1.0),
        ];
        let draft_dir_norm = {
            let v = Vec3::from_array(self.draft_config.pull_dir);
            let n = if v.length_squared() > 1e-6 {
                v.normalize()
            } else {
                Vec3::Z
            };
            [n.x, n.y, n.z, 0.0]
        };
        let target_rad = self.draft_config.target_angle_deg.to_radians().max(0.0001);
        let draft_params = [
            if self.draft_config.enabled { 1.0 } else { 0.0 },
            target_rad,
            self.draft_config.blend.clamp(0.0, 1.0),
            0.0,
        ];
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            eye: [eye.x, eye.y, eye.z, 1.0],
            light_dir: [light.x, light.y, light.z, 0.0],
            clip_plane: self.clip_plane,
            zebra_params,
            draft_params,
            draft_dir: draft_dir_norm,
        };
        queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));
    }

    pub fn paint(&self, rpass: &mut wgpu::RenderPass<'_>) {
        rpass.set_bind_group(0, &self.globals_bind, &[]);

        if let Some(mesh) = &self.mesh {
            rpass.set_pipeline(&self.mesh_pipeline);
            rpass.set_vertex_buffer(0, mesh.vertex_buf.slice(..));
            rpass.set_index_buffer(mesh.index_buf.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }

        rpass.set_pipeline(&self.grid_pipeline);
        rpass.set_vertex_buffer(0, self.grid_vbuf.slice(..));
        rpass.draw(0..self.grid_vertex_count, 0..1);

        if let Some(buf) = &self.overlay_vbuf {
            rpass.set_pipeline(&self.overlay_pipeline);
            rpass.set_vertex_buffer(0, buf.slice(..));
            rpass.draw(0..self.overlay_vertex_count, 0..1);
        }

        // Gizmo solid (Fase 9) digambar TERAKHIR dengan gizmo_pipeline (depth test Always)
        // supaya gizmo selalu tampak di depan objek 3D dan tidak terkubur di dalam body.
        if let Some(gizmo) = &self.gizmo_mesh {
            rpass.set_pipeline(&self.gizmo_pipeline);
            rpass.set_vertex_buffer(0, gizmo.vertex_buf.slice(..));
            rpass.set_index_buffer(gizmo.index_buf.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..gizmo.index_count, 0, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zebra_config_defaults() {
        let cfg = ZebraConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.frequency, 20.0);
        assert_eq!(cfg.angle, 0.0);
        assert_eq!(cfg.blend, 1.0);
    }

    #[test]
    fn test_draft_config_defaults() {
        let cfg = DraftConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.pull_dir, [0.0, 0.0, 1.0]);
        assert_eq!(cfg.target_angle_deg, 1.0);
        assert_eq!(cfg.blend, 1.0);
    }

    #[test]
    fn test_globals_uniform_buffer_layout() {
        // Uniform buffer alignment in WebGPU is 16 bytes:
        // mat4x4<f32>: 64 bytes
        // eye: 16 bytes
        // light_dir: 16 bytes
        // clip_plane: 16 bytes
        // zebra_params: 16 bytes
        // draft_params: 16 bytes
        // draft_dir: 16 bytes
        // Total = 160 bytes (multiple of 16)
        assert_eq!(std::mem::size_of::<Globals>(), 160);
        assert_eq!(std::mem::align_of::<Globals>(), 4);
    }
}
