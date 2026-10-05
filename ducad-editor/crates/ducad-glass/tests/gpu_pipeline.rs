//! Tes pipeline GPU Liquid Glass tanpa jendela: scene → blur → panel kaca →
//! baca balik piksel. Menjaga dua hal yang tidak bisa dijaga tes CPU:
//! shader WGSL benar-benar lolos validasi wgpu, dan hasil di GPU cocok dengan
//! tiruan CPU (`GlassMaterial::composite`, `sdf`).
//!
//! Mesin tanpa adapter GPU (sebagian runner CI) melewati tes ini.
//!
//! `DUCAD_GLASS_DUMP=/path/out.ppm cargo test -p ducad-glass --test gpu_pipeline`
//! menulis gambar hasil untuk diperiksa mata.

use ducad_glass::{GlassBackdrop, GlassMaterial, GlassMode, GlassPreset, PanelUniform};
use egui_wgpu::wgpu;

const W: u32 = 512;
const H: u32 = 320;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

/// Pola uji: kisi garis terang + pita warna, mirip grid viewport CAD.
const PATTERN_WGSL: &str = r#"
@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(index & 1u) * 4 - 1);
    let y = f32(i32(index >> 1u) * 4 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}
@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    // Kisi tipis + bentuk lunak besar (seperti body CAD terang di viewport):
    // pembelokan di rim hanya terlihat pada tepi bentuk, bukan pada kisi seragam.
    let g = step(vec2<f32>(15.0), frag.xy % vec2<f32>(16.0));
    let line = max(g.x, g.y);
    var base = vec3<f32>(0.08, 0.08, 0.09);
    if (frag.x > 300.0) { base = vec3<f32>(0.85, 0.35, 0.10); }
    if (frag.y > 200.0) { base = vec3<f32>(0.10, 0.45, 0.90); }
    let c1 = 1.0 - smoothstep(60.0, 70.0, distance(frag.xy, vec2<f32>(140.0, 110.0)));
    let c2 = 1.0 - smoothstep(40.0, 48.0, distance(frag.xy, vec2<f32>(330.0, 260.0)));
    let c3 = 1.0 - smoothstep(30.0, 36.0, distance(frag.xy, vec2<f32>(420.0, 60.0)));
    base = mix(base, vec3<f32>(0.92, 0.92, 0.88), c1);
    base = mix(base, vec3<f32>(0.95, 0.95, 0.30), c2);
    base = mix(base, vec3<f32>(0.30, 0.95, 0.60), c3);
    return vec4<f32>(mix(base, vec3<f32>(0.9), line * 0.5), 1.0);
}
"#;

fn pattern_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pattern"),
        source: wgpu::ShaderSource::Wgsl(PATTERN_WGSL.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pattern"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(FORMAT.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Render satu frame: scene (warna polos `clear`, atau pola bila `pattern`),
/// lalu blit + panel-panel kaca. Mengembalikan RGBA8 baris demi baris.
fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    clear: [f32; 4],
    pattern: bool,
    panels: &[(egui::Rect, f32, GlassMaterial)],
) -> Vec<u8> {
    let mut backdrop = GlassBackdrop::new(device, FORMAT, None);
    let pipeline = pattern.then(|| pattern_pipeline(device));
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [W, H],
        pixels_per_point: 1.0,
    };
    let full = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(W as f32, H as f32));

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&Default::default());
    backdrop.render_scene(device, &mut encoder, full, &screen, clear, |rpass| {
        if let Some(p) = &pipeline {
            rpass.set_pipeline(p);
            rpass.draw(0..3, 0..1);
        }
    });
    assert!(backdrop.has_scene());
    assert_eq!(backdrop.scene_size(), Some([W, H]));

    let slots: Vec<u32> = panels
        .iter()
        .map(|(rect, radius, material)| {
            backdrop.stage_panel(PanelUniform::new(material, *rect, *radius, 1.0))
        })
        .collect();
    backdrop.finish(device, queue, &mut encoder);
    // Panggilan kedua dalam frame yang sama tidak boleh merusak apa pun.
    backdrop.finish(device, queue, &mut encoder);

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("target"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
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
        backdrop.blit(&mut rpass);
        for slot in slots {
            backdrop.paint_panel(&mut rpass, slot);
        }
    }

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(W * H * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(W * 4),
                rows_per_image: Some(H),
            },
        },
        wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.expect("map readback"));
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let data = buffer
        .slice(..)
        .get_mapped_range()
        .expect("mapped range")
        .to_vec();
    data
}

fn pixel(img: &[u8], x: u32, y: u32) -> [f32; 3] {
    let i = ((y * W + x) * 4) as usize;
    [
        img[i] as f32 / 255.0,
        img[i + 1] as f32 / 255.0,
        img[i + 2] as f32 / 255.0,
    ]
}

fn close(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= tol)
}

#[test]
fn glass_interior_matches_cpu_composite_and_rim_is_lit() {
    let Some((device, queue)) = device() else {
        eprintln!("lewati: tidak ada adapter GPU");
        return;
    };
    let rect = egui::Rect::from_min_size(egui::pos2(96.0, 60.0), egui::vec2(320.0, 200.0));
    for mode in [GlassMode::Dark, GlassMode::Light] {
        for bg in [[0.8, 0.2, 0.2], [0.95, 0.95, 0.95], [0.02, 0.02, 0.03]] {
            let material = GlassMaterial::preset(GlassPreset::Panel, mode);
            let img = render(
                &device,
                &queue,
                [bg[0], bg[1], bg[2], 1.0],
                false,
                &[(rect, 18.0, material)],
            );
            // Di luar panel: scene ter-blit apa adanya.
            assert!(
                close(pixel(&img, 20, 20), bg, 0.01),
                "{mode:?} {bg:?}: blit berubah"
            );
            // Tengah panel: sama dengan tiruan CPU (blur warna polos = warna polos).
            let want = material.composite(bg);
            let got = pixel(&img, 256, 160);
            assert!(
                close(got, want, 0.02),
                "{mode:?} {bg:?}: tengah {got:?}, CPU {want:?}"
            );
            // Rim kiri-atas (menghadap cahaya) lebih terang daripada tengah.
            let rim = pixel(&img, 97 + 60, 60);
            assert!(
                rim[1] > got[1] + 0.03 || got[1] > 0.9,
                "{mode:?} {bg:?}: rim {rim:?} tidak lebih terang dari tengah {got:?}"
            );
        }
    }
}

#[test]
fn glass_bends_the_backdrop_only_near_the_rim() {
    let Some((device, queue)) = device() else {
        eprintln!("lewati: tidak ada adapter GPU");
        return;
    };
    let rect = egui::Rect::from_min_size(egui::pos2(96.0, 60.0), egui::vec2(320.0, 200.0));
    // Material bening tanpa blur/tint/sorotan: yang tersisa hanya lensa.
    let clear = GlassMaterial {
        tint_strength: 0.0,
        frost: 0.0,
        specular: 0.0,
        dispersion: 0.0,
        saturation: 1.0,
        luma_limit: 1.0,
        ..GlassMaterial::preset(GlassPreset::Panel, GlassMode::Dark)
    };
    let plain = render(&device, &queue, [0.0, 0.0, 0.0, 1.0], true, &[]);
    let glass = render(
        &device,
        &queue,
        [0.0, 0.0, 0.0, 1.0],
        true,
        &[(rect, 18.0, clear)],
    );

    let diff = |x0: u32, y0: u32, x1: u32, y1: u32| -> f32 {
        let mut sum = 0.0;
        for y in y0..y1 {
            for x in x0..x1 {
                let (a, b) = (pixel(&plain, x, y), pixel(&glass, x, y));
                sum += (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs();
            }
        }
        sum / ((x1 - x0) * (y1 - y0)) as f32
    };
    // Tengah panel (jauh dari rim 14 px): tidak terdistorsi.
    let center = diff(200, 130, 300, 190);
    // Pita rim atas: garis kisi mendatar (y = 63) bergeser → berbeda jelas.
    let rim = diff(150, 61, 350, 74);
    assert!(center < 0.01, "tengah ikut terdistorsi: {center}");
    assert!(rim > 0.05, "rim tidak membelokkan latar: {rim}");
}

#[test]
fn dump_showcase_when_requested() {
    let Ok(path) = std::env::var("DUCAD_GLASS_DUMP") else {
        return;
    };
    let Some((device, queue)) = device() else {
        eprintln!("lewati: tidak ada adapter GPU");
        return;
    };
    let dark = |p| GlassMaterial::preset(p, GlassMode::Dark);
    let panels = [
        (
            egui::Rect::from_min_size(egui::pos2(24.0, 24.0), egui::vec2(200.0, 272.0)),
            10.0,
            dark(GlassPreset::Panel),
        ),
        (
            egui::Rect::from_min_size(egui::pos2(250.0, 24.0), egui::vec2(236.0, 40.0)),
            20.0,
            dark(GlassPreset::Pill),
        ),
        (
            egui::Rect::from_min_size(egui::pos2(250.0, 90.0), egui::vec2(48.0, 200.0)),
            10.0,
            dark(GlassPreset::Toolbar),
        ),
        (
            egui::Rect::from_min_size(egui::pos2(320.0, 90.0), egui::vec2(166.0, 200.0)),
            10.0,
            dark(GlassPreset::Popup),
        ),
    ];
    let img = render(&device, &queue, [0.0, 0.0, 0.0, 1.0], true, &panels);
    let mut out = format!("P6\n{W} {H}\n255\n").into_bytes();
    for px in img.chunks_exact(4) {
        out.extend_from_slice(&px[..3]);
    }
    std::fs::write(&path, out).expect("tulis dump");
    eprintln!("ditulis: {path}");
}
