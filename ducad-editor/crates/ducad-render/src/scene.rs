use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use glam::{Mat4, Vec3};

use crate::grid;

/// Bidang potong "tidak aktif" — normal nol vektor + offset sangat besar,
/// jadi `dot(0, world) - w` selalu sangat negatif dan tidak pernah lolos
/// syarat `> 0.0` di `fs_mesh` (lihat `shader.wgsl`), berapa pun posisi
/// mesh-nya. Dipakai `SceneRenderer::new`/`set_clip_plane(None)`.
const CLIP_PLANE_DISABLED: [f32; 4] = [0.0, 0.0, 0.0, 1.0e9];

/// Preset pencahayaan studio 3-titik (Fase 4.2 Studio Lighting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioPreset {
    /// Studio Bersih / Standar: Pencahayaan daylight seimbang, fill lembut, rim bersih.
    CleanStudio,
    /// Showcase Hangat: Key hangat, fill sejuk kontras, rim kuat untuk presentasi produk konsumen.
    WarmShowcase,
    /// High-Tech / Cool Lab: Key sejuk tajam, fill biru muda, rim perak tajam (bagus untuk logam & casing gadget).
    CoolTech,
    /// Sinematik / Gelap Dramatis: Kontras tinggi dengan siluet rim dominan.
    DramaticDark,
}

impl StudioPreset {
    pub fn all() -> &'static [StudioPreset] {
        &[
            StudioPreset::CleanStudio,
            StudioPreset::WarmShowcase,
            StudioPreset::CoolTech,
            StudioPreset::DramaticDark,
        ]
    }
}

/// Konfigurasi Studio Lighting, SSAO & Bayangan Kontak Lantai (Fase 4.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StudioConfig {
    pub enabled: bool,
    pub preset: StudioPreset,
    pub key_intensity: f32,
    pub fill_intensity: f32,
    pub rim_intensity: f32,
    pub ssao_intensity: f32,
    pub floor_shadow_enabled: bool,
    pub floor_shadow_intensity: f32,
    pub ground_z: f32,
    pub shadow_center: [f32; 2],
    pub shadow_radius: [f32; 2],
}

impl Default for StudioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            preset: StudioPreset::CleanStudio,
            key_intensity: 1.0,
            fill_intensity: 0.50,
            rim_intensity: 0.65,
            ssao_intensity: 0.85,
            floor_shadow_enabled: true,
            floor_shadow_intensity: 0.60,
            ground_z: 0.0,
            shadow_center: [0.0, 0.0],
            shadow_radius: [60.0, 60.0],
        }
    }
}

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
    fill_light: [f32; 4],
    rim_light: [f32; 4],
    studio_params: [f32; 4],
    shadow_bounds: [f32; 4],
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
    pub material_params: [f32; 4],
}

struct GpuMesh {
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    index_count: u32,
    data_hash: u64,
}

fn compute_mesh_fingerprint(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    colors: Option<&[[f32; 4]]>,
    material_params: Option<&[[f32; 4]]>,
    indices: &[u32],
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    positions.len().hash(&mut hasher);
    indices.len().hash(&mut hasher);
    if let Some(p) = positions.first() {
        bytemuck::cast_slice::<[f32; 3], u8>(std::slice::from_ref(p)).hash(&mut hasher);
    }
    if let Some(p) = positions.last() {
        bytemuck::cast_slice::<[f32; 3], u8>(std::slice::from_ref(p)).hash(&mut hasher);
    }
    if let Some(n) = normals.first() {
        bytemuck::cast_slice::<[f32; 3], u8>(std::slice::from_ref(n)).hash(&mut hasher);
    }
    if let Some(c) = colors.and_then(|c| c.first()) {
        bytemuck::cast_slice::<[f32; 4], u8>(std::slice::from_ref(c)).hash(&mut hasher);
    }
    if let Some(m) = material_params.and_then(|m| m.first()) {
        bytemuck::cast_slice::<[f32; 4], u8>(std::slice::from_ref(m)).hash(&mut hasher);
    }
    if let Some(i) = indices.first() {
        i.hash(&mut hasher);
    }
    if let Some(i) = indices.last() {
        i.hash(&mut hasher);
    }
    hasher.finish()
}

/// Renderer scene 3D yang hidup di dalam `CallbackResources` egui_wgpu.
/// `prepare()` dipanggil per-frame sebelum render pass, `paint()` di dalamnya.
pub struct SceneRenderer {
    globals_buf: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    grid_pipeline: wgpu::RenderPipeline,
    grid_vbuf: wgpu::Buffer,
    grid_vertex_count: u32,
    floor_pipeline: wgpu::RenderPipeline,
    floor_vbuf: wgpu::Buffer,
    floor_ibuf: wgpu::Buffer,
    floor_index_count: u32,
    mesh_pipeline: wgpu::RenderPipeline,
    body_edge_pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    gizmo_pipeline: wgpu::RenderPipeline,
    mesh: Option<GpuMesh>,
    gizmo_mesh: Option<GpuMesh>,
    body_edge_vbuf: Option<wgpu::Buffer>,
    body_edge_vertex_count: u32,
    body_edge_hash: u64,
    overlay_vbuf: Option<wgpu::Buffer>,
    overlay_vertex_count: u32,
    overlay_hash: u64,
    clip_plane: [f32; 4],
    studio_config: StudioConfig,
    zebra_config: ZebraConfig,
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

        let body_edge_depth_stencil = depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
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

        let floor_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("floor-shadow"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_floor"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<grid::LineVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_floor"),
                compilation_options: Default::default(),
                targets: &color_target,
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
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
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x4],
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

        let body_edge_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("body-edges"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_body_edge"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<grid::LineVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_body_edge"),
                compilation_options: Default::default(),
                targets: &color_target,
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: body_edge_depth_stencil,
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
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_gizmo"),
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

        // Inisialisasi quad lantai default (skala besar untuk bayangan kontak)
        let floor_extent = 1000.0;
        let floor_verts = [
            grid::LineVertex { position: [-floor_extent, -floor_extent, 0.0], color: [0.0, 0.0, 0.0, 1.0] },
            grid::LineVertex { position: [ floor_extent, -floor_extent, 0.0], color: [0.0, 0.0, 0.0, 1.0] },
            grid::LineVertex { position: [ floor_extent,  floor_extent, 0.0], color: [0.0, 0.0, 0.0, 1.0] },
            grid::LineVertex { position: [-floor_extent,  floor_extent, 0.0], color: [0.0, 0.0, 0.0, 1.0] },
        ];
        let floor_indices: [u32; 6] = [0, 1, 2, 0, 2, 3];

        let floor_vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("floor-vbuf"),
            contents: bytemuck::cast_slice(&floor_verts),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let floor_ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("floor-ibuf"),
            contents: bytemuck::cast_slice(&floor_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            globals_buf,
            globals_bind,
            grid_pipeline,
            grid_vbuf,
            grid_vertex_count: grid_verts.len() as u32,
            floor_pipeline,
            floor_vbuf,
            floor_ibuf,
            floor_index_count: 6,
            mesh_pipeline,
            body_edge_pipeline,
            overlay_pipeline,
            gizmo_pipeline,
            mesh: None,
            gizmo_mesh: None,
            body_edge_vbuf: None,
            body_edge_vertex_count: 0,
            body_edge_hash: 0,
            overlay_vbuf: None,
            overlay_vertex_count: 0,
            overlay_hash: 0,
            clip_plane: CLIP_PLANE_DISABLED,
            studio_config: StudioConfig::default(),
            zebra_config: ZebraConfig::default(),
            draft_config: DraftConfig::default(),
            current_grid_plane: Some(crate::plane::SketchPlane::top()),
            current_grid_extent: 500.0,
            current_grid_step: 10.0,
        }
    }

    /// Konfigurasi Studio Lighting & SSAO (Fase 4.2).
    pub fn set_studio_config(&mut self, config: StudioConfig) {
        self.studio_config = config;
    }

    /// Dapatkan konfigurasi Studio Lighting & SSAO yang sedang aktif.
    pub fn studio_config(&self) -> StudioConfig {
        self.studio_config
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

    /// Bidang potong section view (Fase 7).
    pub fn set_clip_plane(&mut self, plane: Option<(Vec3, f32)>) {
        self.clip_plane = match plane {
            Some((normal, offset)) => {
                let n = normal.normalize_or_zero();
                [n.x, n.y, n.z, offset]
            }
            None => CLIP_PLANE_DISABLED,
        };
    }

    /// Upload garis overlay 2D (sketch) untuk frame ini.
    pub fn set_overlay_lines(&mut self, device: &wgpu::Device, verts: &[grid::LineVertex]) {
        use std::hash::{Hash, Hasher};
        use wgpu::util::DeviceExt;
        if verts.is_empty() {
            self.overlay_vbuf = None;
            self.overlay_vertex_count = 0;
            self.overlay_hash = 0;
            return;
        }

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        verts.len().hash(&mut hasher);
        if let Some(v) = verts.first() {
            bytemuck::cast_slice::<grid::LineVertex, u8>(std::slice::from_ref(v)).hash(&mut hasher);
        }
        if let Some(v) = verts.last() {
            bytemuck::cast_slice::<grid::LineVertex, u8>(std::slice::from_ref(v)).hash(&mut hasher);
        }
        let hash = hasher.finish();

        if self.overlay_hash == hash && self.overlay_vertex_count == verts.len() as u32 {
            return;
        }

        let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("overlay"),
            contents: bytemuck::cast_slice(verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.overlay_vbuf = Some(buf);
        self.overlay_vertex_count = verts.len() as u32;
        self.overlay_hash = hash;
    }

    /// Upload garis tepi solid 3D (CAD feature edges) untuk frame ini dengan caching fingerprint.
    pub fn set_body_edges(&mut self, device: &wgpu::Device, verts: &[grid::LineVertex]) {
        use std::hash::{Hash, Hasher};
        use wgpu::util::DeviceExt;
        if verts.is_empty() {
            self.body_edge_vbuf = None;
            self.body_edge_vertex_count = 0;
            self.body_edge_hash = 0;
            return;
        }

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        verts.len().hash(&mut hasher);
        if let Some(v) = verts.first() {
            bytemuck::cast_slice::<grid::LineVertex, u8>(std::slice::from_ref(v)).hash(&mut hasher);
        }
        if let Some(v) = verts.last() {
            bytemuck::cast_slice::<grid::LineVertex, u8>(std::slice::from_ref(v)).hash(&mut hasher);
        }
        let hash = hasher.finish();

        if self.body_edge_hash == hash && self.body_edge_vertex_count == verts.len() as u32 {
            return;
        }

        let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("body-edges-vbuf"),
            contents: bytemuck::cast_slice(verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.body_edge_vbuf = Some(buf);
        self.body_edge_vertex_count = verts.len() as u32;
        self.body_edge_hash = hash;
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

    /// Upload mesh body (dari ducad-kernel) untuk ditampilkan dengan caching fingerprint.
    pub fn set_mesh(
        &mut self,
        device: &wgpu::Device,
        positions: &[[f32; 3]],
        normals: &[[f32; 3]],
        colors: Option<&[[f32; 4]]>,
        material_params: Option<&[[f32; 4]]>,
        indices: &[u32],
    ) {
        use wgpu::util::DeviceExt;
        if indices.is_empty() {
            self.mesh = None;
            return;
        }
        let hash = compute_mesh_fingerprint(positions, normals, colors, material_params, indices);
        if let Some(existing) = &self.mesh {
            if existing.data_hash == hash && existing.index_count == indices.len() as u32 {
                return; // Re-use GPU buffers without allocation
            }
        }

        const DEFAULT_CAD_GREY: [f32; 4] = [0.62, 0.68, 0.76, 1.0];
        const DEFAULT_MAT_PARAMS: [f32; 4] = [0.65, 0.0, 0.0, 0.0];
        let verts: Vec<MeshVertex> = positions
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let n = normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]);
                let color = colors.and_then(|c| c.get(i).copied()).unwrap_or(DEFAULT_CAD_GREY);
                let mat = material_params
                    .and_then(|m| m.get(i).copied())
                    .unwrap_or(DEFAULT_MAT_PARAMS);
                MeshVertex {
                    position: *p,
                    normal: n,
                    color,
                    material_params: mat,
                }
            })
            .collect();
        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-vb"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-ib"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });
        self.mesh = Some(GpuMesh {
            vertex_buf,
            index_buf,
            index_count: indices.len() as u32,
            data_hash: hash,
        });
    }

    /// Upload mesh solid gizmo (push/pull & rounding, Fase 9) dengan caching.
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
        let hash = compute_mesh_fingerprint(positions, normals, Some(colors), None, indices);
        if let Some(existing) = &self.gizmo_mesh {
            if existing.data_hash == hash && existing.index_count == indices.len() as u32 {
                return;
            }
        }
        const DEFAULT_GIZMO_COLOR: [f32; 4] = [0.0, 0.78, 1.0, 1.0];
        const DEFAULT_GIZMO_MAT_PARAMS: [f32; 4] = [0.25, 0.0, 0.60, 0.0];
        let verts: Vec<MeshVertex> = positions
            .iter()
            .enumerate()
            .map(|(i, p)| MeshVertex {
                position: *p,
                normal: normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]),
                color: colors.get(i).copied().unwrap_or(DEFAULT_GIZMO_COLOR),
                material_params: DEFAULT_GIZMO_MAT_PARAMS,
            })
            .collect();
        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gizmo-mesh-vb"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gizmo-mesh-ib"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });
        self.gizmo_mesh = Some(GpuMesh {
            vertex_buf,
            index_buf,
            index_count: indices.len() as u32,
            data_hash: hash,
        });
    }

    pub fn prepare(&mut self, queue: &wgpu::Queue, view_proj: Mat4, eye: Vec3) {
        // Hitung arah 3-point lighting berdasarkan StudioPreset
        let (key_dir, fill_dir, rim_dir) = match self.studio_config.preset {
            StudioPreset::CleanStudio => (
                Vec3::new(0.5, 0.4, 0.85).normalize(),
                Vec3::new(-0.6, -0.3, 0.5).normalize(),
                Vec3::new(-0.2, 0.8, -0.3).normalize(),
            ),
            StudioPreset::WarmShowcase => (
                Vec3::new(0.6, 0.3, 0.75).normalize(),
                Vec3::new(-0.5, -0.5, 0.4).normalize(),
                Vec3::new(-0.4, 0.7, 0.5).normalize(),
            ),
            StudioPreset::CoolTech => (
                Vec3::new(0.4, 0.6, 0.9).normalize(),
                Vec3::new(-0.7, 0.2, 0.3).normalize(),
                Vec3::new(0.1, -0.9, 0.4).normalize(),
            ),
            StudioPreset::DramaticDark => (
                Vec3::new(0.7, 0.1, 0.5).normalize(),
                Vec3::new(-0.4, -0.6, 0.2).normalize(),
                Vec3::new(-0.6, 0.8, 0.6).normalize(),
            ),
        };

        let key_light = [
            key_dir.x,
            key_dir.y,
            key_dir.z,
            self.studio_config.key_intensity.max(0.0),
        ];
        let fill_light = [
            fill_dir.x,
            fill_dir.y,
            fill_dir.z,
            self.studio_config.fill_intensity.max(0.0),
        ];
        let rim_light = [
            rim_dir.x,
            rim_dir.y,
            rim_dir.z,
            self.studio_config.rim_intensity.max(0.0),
        ];

        let studio_params = [
            if self.studio_config.enabled { 1.0 } else { 0.0 },
            self.studio_config.ssao_intensity.max(0.0),
            if self.studio_config.floor_shadow_enabled {
                self.studio_config.floor_shadow_intensity.clamp(0.0, 1.0)
            } else {
                0.0
            },
            self.studio_config.ground_z,
        ];

        let shadow_bounds = [
            self.studio_config.shadow_center[0],
            self.studio_config.shadow_center[1],
            self.studio_config.shadow_radius[0].max(1.0),
            self.studio_config.shadow_radius[1].max(1.0),
        ];

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
            light_dir: key_light,
            fill_light,
            rim_light,
            studio_params,
            shadow_bounds,
            clip_plane: self.clip_plane,
            zebra_params,
            draft_params,
            draft_dir: draft_dir_norm,
        };
        queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        // Update posisi quad lantai sesuai ground_z dan shadow_center
        if self.studio_config.floor_shadow_enabled {
            let cx = self.studio_config.shadow_center[0];
            let cy = self.studio_config.shadow_center[1];
            let gz = self.studio_config.ground_z;
            let rx = (self.studio_config.shadow_radius[0] * 3.0).max(300.0);
            let ry = (self.studio_config.shadow_radius[1] * 3.0).max(300.0);

            let floor_verts = [
                grid::LineVertex { position: [cx - rx, cy - ry, gz], color: [0.0, 0.0, 0.0, 1.0] },
                grid::LineVertex { position: [cx + rx, cy - ry, gz], color: [0.0, 0.0, 0.0, 1.0] },
                grid::LineVertex { position: [cx + rx, cy + ry, gz], color: [0.0, 0.0, 0.0, 1.0] },
                grid::LineVertex { position: [cx - rx, cy + ry, gz], color: [0.0, 0.0, 0.0, 1.0] },
            ];
            queue.write_buffer(&self.floor_vbuf, 0, bytemuck::cast_slice(&floor_verts));
        }
    }

    pub fn paint(&self, rpass: &mut wgpu::RenderPass<'_>) {
        rpass.set_bind_group(0, &self.globals_bind, &[]);

        // 1. Gambar Floor Contact Shadow di bawah objek & grid
        if self.studio_config.enabled && self.studio_config.floor_shadow_enabled {
            rpass.set_pipeline(&self.floor_pipeline);
            rpass.set_vertex_buffer(0, self.floor_vbuf.slice(..));
            rpass.set_index_buffer(self.floor_ibuf.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..self.floor_index_count, 0, 0..1);
        }

        // 2. Gambar Solid CAD Mesh
        if let Some(mesh) = &self.mesh {
            rpass.set_pipeline(&self.mesh_pipeline);
            rpass.set_vertex_buffer(0, mesh.vertex_buf.slice(..));
            rpass.set_index_buffer(mesh.index_buf.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }

        // 2b. Gambar Garis Tepi Solid 3D (CAD Feature Edges)
        if let Some(buf) = &self.body_edge_vbuf {
            rpass.set_pipeline(&self.body_edge_pipeline);
            rpass.set_vertex_buffer(0, buf.slice(..));
            rpass.draw(0..self.body_edge_vertex_count, 0..1);
        }

        // 3. Gambar Grid CAD
        rpass.set_pipeline(&self.grid_pipeline);
        rpass.set_vertex_buffer(0, self.grid_vbuf.slice(..));
        rpass.draw(0..self.grid_vertex_count, 0..1);

        // 4. Gambar Overlay Garis 2D (Sketch)
        if let Some(buf) = &self.overlay_vbuf {
            rpass.set_pipeline(&self.overlay_pipeline);
            rpass.set_vertex_buffer(0, buf.slice(..));
            rpass.draw(0..self.overlay_vertex_count, 0..1);
        }

        // 5. Gizmo solid (Fase 9) digambar TERAKHIR dengan depth test Always
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
    fn test_studio_config_defaults() {
        let cfg = StudioConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.preset, StudioPreset::CleanStudio);
        assert_eq!(cfg.key_intensity, 1.0);
        assert_eq!(cfg.fill_intensity, 0.50);
        assert_eq!(cfg.rim_intensity, 0.65);
        assert_eq!(cfg.ssao_intensity, 0.85);
        assert!(cfg.floor_shadow_enabled);
        assert_eq!(cfg.floor_shadow_intensity, 0.60);
    }

    #[test]
    fn test_studio_preset_all() {
        let presets = StudioPreset::all();
        assert_eq!(presets.len(), 4);
        assert!(presets.contains(&StudioPreset::CleanStudio));
        assert!(presets.contains(&StudioPreset::WarmShowcase));
        assert!(presets.contains(&StudioPreset::CoolTech));
        assert!(presets.contains(&StudioPreset::DramaticDark));
    }

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
        // fill_light: 16 bytes
        // rim_light: 16 bytes
        // studio_params: 16 bytes
        // shadow_bounds: 16 bytes
        // clip_plane: 16 bytes
        // zebra_params: 16 bytes
        // draft_params: 16 bytes
        // draft_dir: 16 bytes
        // Total = 224 bytes (multiple of 16)
        assert_eq!(std::mem::size_of::<Globals>(), 224);
        assert_eq!(std::mem::align_of::<Globals>(), 4);
    }

    #[test]
    fn test_shader_wgsl_validity() {
        let shader_str = include_str!("shader.wgsl");
        let module = egui_wgpu::wgpu::naga::front::wgsl::parse_str(shader_str);
        assert!(module.is_ok(), "WGSL parse error: {:?}", module.err());
    }
}

