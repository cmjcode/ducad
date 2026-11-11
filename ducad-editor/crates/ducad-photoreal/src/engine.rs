//! Sambungan ke `ducad-engine`: pelaksana `render_view` gaya foto.
//!
//! Engine tidak boleh bergantung pada wgpu, jadi ia hanya mendefinisikan
//! permintaannya (`ducad_engine::render::PhotoRequest`). Binari memanggil
//! [`install`] sekali saat mulai; sejak itu `render_view { "style": "photo" }`
//! (MCP, jembatan agent di GUI) dan `ducad-cli render --photo` bekerja.

// `OpError` besar, tetapi bentuk `OpResult` ditetapkan engine
// (`PhotoRenderer`); sama dengan `ducad-engine`/`ducad-mcp`.
#![allow(clippy::result_large_err)]

use ducad_engine::render::{PhotoOutput, PhotoRequest, View};
use ducad_engine::{OpError, OpErrorCode, OpResult};

use crate::{
    Backend, Environment, PhotoBody, PhotoCamera, PhotoMaterial, PhotoOptions, PhotoScene,
    SceneOptions,
};

/// Sudut pandang vertikal render tampak baku (±34°: lensa "normal", distorsi
/// perspektif kecil — lazim untuk foto produk).
pub const VIEW_FOV_Y: f32 = 0.6;

/// Ruang kosong di sekeliling benda (1 = menyentuh tepi bingkai): cukup
/// untuk bayangan di lantai tanpa mengecilkan benda.
const FRAME_MARGIN: f32 = 1.12;

/// Bagian cahaya yang diteruskan preset kaca. Alpha 0,38 di viewport adalah
/// trik rasterisasi (campur warna dengan latar), bukan sifat fisik: kaca
/// nyata meneruskan hampir semua cahaya dan terlihat lewat pantulan serta
/// pembiasannya.
const GLASS_TRANSMISSION: f32 = 0.96;

/// Daftarkan path tracer sebagai pelaksana render foto engine.
pub fn install() {
    ducad_engine::render::install_photo_renderer(render_request);
}

/// Material fisik untuk material tampilan DUCAD.
pub fn material_of(m: &ducad_core::Material) -> PhotoMaterial {
    let transmission = if m.preset == ducad_core::MaterialPreset::TranslucentGlass {
        GLASS_TRANSMISSION
    } else if m.is_translucent() {
        1.0 - m.base_color[3].clamp(0.0, 1.0)
    } else {
        0.0
    };
    PhotoMaterial {
        base_color: [m.base_color[0], m.base_color[1], m.base_color[2]],
        roughness: m.roughness,
        metallic: m.metallic,
        // Antarmuka kaca sudah punya pantulan Fresnel sendiri; clearcoat di
        // preset kaca hanya meniru pantulan itu untuk viewport.
        clearcoat: if transmission > 0.5 { 0.0 } else { m.clearcoat },
        transmission,
        specular_level: 0.5,
    }
}

/// Kamera perspektif yang membingkai kotak `lo..hi` dari arah `view`.
pub fn camera_for_view(lo: [f32; 3], hi: [f32; 3], view: View, aspect: f32) -> PhotoCamera {
    let center = [
        (lo[0] + hi[0]) * 0.5,
        (lo[1] + hi[1]) * 0.5,
        (lo[2] + hi[2]) * 0.5,
    ];
    let (dir, up) = view.direction_up();
    let d = dir.normalize_or_zero();
    // Basis layar: `right`/`screen_up` tegak lurus arah pandang.
    let right = up.cross(d).normalize_or_zero();
    let screen_up = d.cross(right);
    // Jarak terkecil yang memuat kedelapan sudut kotak. Membingkai sudut
    // (bukan bola pembatas) membuat benda pipih atau panjang tetap mengisi
    // gambar. Sudut berjarak `z` di depan pusat butuh mata sejauh
    // `z + max(|x| / tan(half_x), |y| / tan(half_y))`. Margin dikalikan ke
    // suku sudut, bukan ke jarak: sudut terluar lalu jatuh tepat di
    // `1 / FRAME_MARGIN` dari tepi, berapa pun kedalaman bendanya.
    let tan_y = (VIEW_FOV_Y * 0.5).tan();
    let tan_x = tan_y * aspect.max(0.05);
    let mut distance = 1e-3_f32;
    for i in 0..8 {
        let corner = glam::Vec3::new(
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ) - glam::Vec3::from_array(center);
        let (x, y, z) = (corner.dot(right), corner.dot(screen_up), corner.dot(d));
        distance =
            distance.max(z + FRAME_MARGIN * (x.abs() / tan_x).max(y.abs() / tan_y));
    }
    PhotoCamera {
        eye: [
            center[0] + d.x * distance,
            center[1] + d.y * distance,
            center[2] + d.z * distance,
        ],
        target: center,
        up: up.to_array(),
        fov_y: VIEW_FOV_Y,
        orthographic: false,
    }
}

fn render_request(req: &PhotoRequest<'_>) -> OpResult<PhotoOutput> {
    let environment = match req.environment.as_deref() {
        None => Environment::default(),
        Some(key) => Environment::from_key(key).ok_or_else(|| {
            OpError::invalid(format!("unknown environment '{key}'")).with_hint(format!(
                "one of: {}",
                Environment::ALL.map(Environment::key).join(", ")
            ))
        })?,
    };
    let bodies: Vec<PhotoBody<'_>> = req
        .bodies
        .iter()
        .map(|(body, geo)| PhotoBody {
            positions: &geo.mesh.positions,
            normals: &geo.mesh.normals,
            indices: &geo.mesh.indices,
            transform: None,
            material: material_of(&body.material),
        })
        .collect();
    let scene = PhotoScene::build(
        &bodies,
        &SceneOptions {
            environment,
            ..SceneOptions::default()
        },
    )
    .map_err(|e| OpError::new(OpErrorCode::EmptyResult, format!("render foto: {e}")))?;
    let (lo, hi) = scene.bounds();
    let camera = camera_for_view(lo, hi, req.view, req.width as f32 / req.height.max(1) as f32);
    let options = PhotoOptions {
        width: req.width,
        height: req.height,
        samples: req.samples,
        ..PhotoOptions::default()
    };
    let (image, backend) = crate::render_headless(&scene, &camera, &options);
    let png = image
        .to_png()
        .map_err(|e| OpError::new(OpErrorCode::Io, format!("gagal encode PNG: {e}")))?;
    Ok(PhotoOutput {
        png,
        backend: match backend {
            Backend::Gpu => "gpu",
            Backend::Cpu => "cpu",
        },
        samples: req.samples,
    })
}
