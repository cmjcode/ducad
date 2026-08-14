use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use glam::{Mat4, Vec3};

use crate::grid;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    light_dir: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct MeshVertex {
    position: [f32; 3],
    normal: [f32; 3],
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
    mesh: Option<GpuMesh>,
    /// Garis overlay 2D (entitas sketch, preview, glyph snap) — dibangun
    /// ulang tiap frame lewat `set_overlay_lines`, memakai pipeline garis
    /// yang sama dengan grid (topology & shader identik).
    overlay_vbuf: Option<wgpu::Buffer>,
    overlay_vertex_count: u32,
}

impl SceneRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
    ) -> Self {
        use wgpu::util::DeviceExt;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cadraw-scene"),
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
            label: Some("cadraw-scene"),
            bind_group_layouts: &[&globals_layout],
            push_constant_ranges: &[],
        });

        let depth_stencil = depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
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
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<grid::LineVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                }],
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
            multiview: None,
            cache: None,
        });

        let mesh_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_mesh"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_mesh"),
                compilation_options: Default::default(),
                targets: &color_target,
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil,
            multisample: Default::default(),
            multiview: None,
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
            mesh: None,
            overlay_vbuf: None,
            overlay_vertex_count: 0,
        }
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

    /// Upload mesh body (dari cadraw-kernel) untuk ditampilkan.
    pub fn set_mesh(
        &mut self,
        device: &wgpu::Device,
        positions: &[[f32; 3]],
        normals: &[[f32; 3]],
        indices: &[u32],
    ) {
        use wgpu::util::DeviceExt;
        let verts: Vec<MeshVertex> = positions
            .iter()
            .zip(normals)
            .map(|(p, n)| MeshVertex {
                position: *p,
                normal: *n,
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

    pub fn prepare(&mut self, queue: &wgpu::Queue, view_proj: Mat4, eye: Vec3) {
        let light = Vec3::new(0.4, 0.3, 0.85).normalize();
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            eye: [eye.x, eye.y, eye.z, 1.0],
            light_dir: [light.x, light.y, light.z, 0.0],
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
            rpass.set_vertex_buffer(0, buf.slice(..));
            rpass.draw(0..self.overlay_vertex_count, 0..1);
        }
    }
}
