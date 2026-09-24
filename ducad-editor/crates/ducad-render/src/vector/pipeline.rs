//! Pipeline wgpu untuk merender elemen vektor dengan shader gradien & blending premultiplied.

use egui_wgpu::wgpu;
use wgpu::util::DeviceExt;

use ducad_sketch::LayerId;

use super::cache::{GradientStop, GradientUniform, LayerBatch, MAX_GRADIENTS_PER_BATCH};
use super::tessellate::VectorVertex;

/// Batch GPU ter-upload untuk satu layer vektor.
pub struct GpuVectorBatch {
    pub layer: LayerId,
    pub vertex_buf: wgpu::Buffer,
    pub index_buf: wgpu::Buffer,
    pub index_count: u32,
    pub gradient_buf: wgpu::Buffer,
    pub gradient_bind: wgpu::BindGroup,
}

/// Pipeline render vektor untuk mode 2D dan 3D.
pub struct VectorPipelines {
    pub pipeline_2d: wgpu::RenderPipeline,
    pub pipeline_3d: wgpu::RenderPipeline,
    pub gradient_layout: wgpu::BindGroupLayout,
}

/// Buat render pipeline vektor untuk mode 2D (depth test off) dan 3D (depth test on, occluded by solid bodies).
pub fn create_vector_pipelines(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    globals_layout: &wgpu::BindGroupLayout,
) -> VectorPipelines {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ducad-vector-shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader_vector.wgsl").into()),
    });

    let gradient_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("vector-gradient-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("vector-pipeline-layout"),
        bind_group_layouts: &[Some(globals_layout), Some(&gradient_layout)],
        immediate_size: 0,
    });

    let color_target = [Some(wgpu::ColorTargetState {
        format: color_format,
        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })];

    let vertex_buffer_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<VectorVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 28,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
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
        label: Some("vector-pipeline-2d"),
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
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: depth_stencil_2d,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });

    let pipeline_3d = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("vector-pipeline-3d"),
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
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: depth_stencil_3d,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });

    VectorPipelines {
        pipeline_2d,
        pipeline_3d,
        gradient_layout,
    }
}

/// Unggah batch satu layer vektor ke buffer GPU.
pub fn upload_layer_batch(
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    batch: &LayerBatch,
    gradient_layout: &wgpu::BindGroupLayout,
) -> Option<GpuVectorBatch> {
    if batch.vertices.is_empty() || batch.indices.is_empty() {
        return None;
    }

    let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vector-vertex-buf"),
        contents: bytemuck::cast_slice(&batch.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vector-index-buf"),
        contents: bytemuck::cast_slice(&batch.indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    let mut raw_gradients = [GradientUniform {
        kind: 0,
        count: 0,
        p0: [0.0; 2],
        p1: [0.0; 2],
        _pad: [0.0; 2],
        stops: [GradientStop {
            offset: 0.0,
            _pad: [0.0; 3],
            color: [0.0; 4],
        }; 8],
    }; MAX_GRADIENTS_PER_BATCH];

    for (i, g) in batch.gradients.iter().take(MAX_GRADIENTS_PER_BATCH).enumerate() {
        raw_gradients[i] = *g;
    }

    let gradient_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vector-gradient-buf"),
        contents: bytemuck::cast_slice(&raw_gradients),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let gradient_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vector-gradient-bind"),
        layout: gradient_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: gradient_buf.as_entire_binding(),
        }],
    });

    Some(GpuVectorBatch {
        layer: batch.layer,
        vertex_buf,
        index_buf,
        index_count: batch.indices.len() as u32,
        gradient_buf,
        gradient_bind,
    })
}
