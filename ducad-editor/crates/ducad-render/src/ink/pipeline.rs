//! Pipeline wgpu untuk render tinta bertekanan dengan blending premultiplied dan topology TriangleStrip.

use egui_wgpu::wgpu;
use wgpu::util::DeviceExt;

use ducad_sketch::LayerId;

use super::stroke::{InkLayerBatch, InkVertex};

/// Layer tinta yang diunggah ke GPU dalam buffer vertex.
pub struct GpuInkLayer {
    pub layer: LayerId,
    pub vertex_buf: wgpu::Buffer,
    pub vertex_count: u32,
}

/// Pipeline render tinta untuk mode 2D dan 3D.
pub struct InkPipelines {
    pub pipeline_2d: wgpu::RenderPipeline,
    pub pipeline_3d: wgpu::RenderPipeline,
}

/// Buat render pipeline tinta untuk mode 2D (depth test off) dan 3D (depth test on).
pub fn create_ink_pipelines(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    globals_layout: &wgpu::BindGroupLayout,
) -> InkPipelines {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ducad-ink-shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader_ink.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ink-pipeline-layout"),
        bind_group_layouts: &[Some(globals_layout)],
        immediate_size: 0,
    });

    let color_target = [Some(wgpu::ColorTargetState {
        format: color_format,
        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })];

    let vertex_buffer_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<InkVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 32,
                shader_location: 3,
            },
        ],
    };

    let depth_stencil_2d = depth_format.map(|format| wgpu::DepthStencilState {
        format,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::Always),
        stencil: Default::default(),
        bias: Default::default(),
    });

    let depth_stencil_3d = depth_format.map(|format| wgpu::DepthStencilState {
        format,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: Default::default(),
        bias: Default::default(),
    });

    let pipeline_2d = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("ink-pipeline-2d"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_buffer_layout.clone())],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &color_target,
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: depth_stencil_2d,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });

    let pipeline_3d = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("ink-pipeline-3d"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_buffer_layout)],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &color_target,
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: depth_stencil_3d,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });

    InkPipelines {
        pipeline_2d,
        pipeline_3d,
    }
}

/// Unggah batch satu layer tinta ke buffer GPU.
pub fn upload_ink_layer_batch(
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    batch: &InkLayerBatch,
) -> Option<GpuInkLayer> {
    if batch.vertices.is_empty() {
        return None;
    }

    let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ink-layer-vbuf"),
        contents: bytemuck::cast_slice(&batch.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    Some(GpuInkLayer {
        layer: batch.layer,
        vertex_buf,
        vertex_count: batch.vertices.len() as u32,
    })
}
