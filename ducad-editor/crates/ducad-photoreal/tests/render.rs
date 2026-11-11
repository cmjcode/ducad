//! Tes ujung-ke-ujung Mode Render: scene kecil benar-benar di-path-trace.
//!
//! Jalur CPU deterministik (aliran acak per piksel dan per sampel tetap),
//! jadi hasilnya bisa dibandingkan bit demi bit. Jalur GPU hanya diuji bila
//! mesin punya adapter yang sanggup; di CI tanpa GPU ia dilewati.

use ducad_engine::render::View;
use ducad_photoreal::engine::{camera_for_view, material_of};
use ducad_photoreal::{
    render_cpu, render_headless, Backend, CpuPhoto, Environment, PhotoBody, PhotoCamera,
    PhotoImage, PhotoMaterial, PhotoOptions, PhotoScene, SceneOptions,
};

/// Kubus 20 mm di atas lantai z = 0, 24 vertex (normal per sisi).
fn cube() -> (Vec<[f32; 3]>, Vec<u32>) {
    let h = 10.0;
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ];
    let (mut pos, mut idx) = (Vec::new(), Vec::new());
    for (n, u, v) in faces {
        let b = pos.len() as u32;
        for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            pos.push([
                (n[0] + su * u[0] + sv * v[0]) * h,
                (n[1] + su * u[1] + sv * v[1]) * h,
                (n[2] + su * u[2] + sv * v[2]) * h + h,
            ]);
        }
        idx.extend([b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    (pos, idx)
}

fn scene(material: PhotoMaterial) -> PhotoScene {
    let (pos, idx) = cube();
    PhotoScene::build(
        &[PhotoBody {
            positions: &pos,
            normals: &[],
            indices: &idx,
            transform: None,
            material,
        }],
        &SceneOptions {
            env_width: 128,
            ..SceneOptions::default()
        },
    )
    .expect("scene kubus")
}

fn iso_camera(scene: &PhotoScene, aspect: f32) -> PhotoCamera {
    let (lo, hi) = scene.bounds();
    camera_for_view(lo, hi, View::Iso, aspect)
}

fn small(samples: u32) -> PhotoOptions {
    PhotoOptions {
        width: 96,
        height: 72,
        samples,
        denoise: false,
        ..PhotoOptions::default()
    }
}

/// Kecerahan rata-rata (0..255) sebuah kotak piksel.
fn mean(img: &PhotoImage, x0: u32, y0: u32, x1: u32, y1: u32) -> f32 {
    let mut sum = 0.0;
    let mut n = 0.0;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * img.width + x) * 4) as usize;
            sum += (img.rgba[i] as f32 + img.rgba[i + 1] as f32 + img.rgba[i + 2] as f32) / 3.0;
            n += 1.0;
        }
    }
    sum / n
}

#[test]
fn render_cpu_deterministik_dan_memuat_benda() {
    let s = scene(PhotoMaterial {
        base_color: [0.9, 0.2, 0.1],
        ..PhotoMaterial::default()
    });
    let cam = iso_camera(&s, 96.0 / 72.0);
    let a = render_cpu(&s, &cam, &small(8), |_, _| true);
    let b = render_cpu(&s, &cam, &small(8), |_, _| true);
    assert_eq!((a.width, a.height), (96, 72));
    assert_eq!(a.rgba.len(), 96 * 72 * 4);
    assert_eq!(a, b, "dua render dengan masukan sama harus identik");

    // Kubus merah di tengah bingkai: kanal merah jauh di atas biru di sana,
    // sedangkan sudut gambar (lantai/latar abu-abu) netral.
    let center = ((36 * 96 + 48) * 4) as usize;
    assert!(
        a.rgba[center] as i32 > a.rgba[center + 2] as i32 + 40,
        "tengah bingkai bukan kubus merah: {:?}",
        &a.rgba[center..center + 3]
    );
    let corner = &a.rgba[0..3];
    assert!(
        (corner[0] as i32 - corner[2] as i32).abs() < 30,
        "sudut gambar seharusnya netral: {corner:?}"
    );
    assert!(a.rgba.chunks_exact(4).all(|px| px[3] == 255));
}

#[test]
fn kemajuan_dilaporkan_dan_bisa_dihentikan() {
    let s = scene(PhotoMaterial::default());
    let cam = iso_camera(&s, 96.0 / 72.0);
    let mut seen = Vec::new();
    render_cpu(&s, &cam, &small(6), |done, total| {
        seen.push((done, total));
        done < 3
    });
    assert_eq!(seen, vec![(1, 6), (2, 6), (3, 6)]);
}

#[test]
fn lampu_utama_di_kiri_kamera_menjatuhkan_bayangan_ke_kanan() {
    // Tampak depan: kamera di −Y memandang +Y, jadi kanan layar = +X.
    // Lampu utama di kiri kamera berarti lantai di kanan kubus lebih gelap
    // daripada lantai di kirinya.
    let s = scene(PhotoMaterial::default());
    let (lo, hi) = s.bounds();
    let mut cam = camera_for_view(lo, hi, View::Front, 2.0);
    // Naikkan kamera supaya lantai di kedua sisi terlihat jelas.
    cam.eye[2] += 60.0;
    let opts = PhotoOptions {
        width: 160,
        height: 80,
        samples: 48,
        denoise: false,
        ..PhotoOptions::default()
    };
    let lit_left = render_cpu(&s, &cam, &opts, |_, _| true);
    // Lantai tepat di kiri dan kanan kubus (kubus menempati kolom 60..100).
    let (left, right) = (
        mean(&lit_left, 40, 20, 60, 50),
        mean(&lit_left, 100, 20, 120, 50),
    );
    assert!(
        left > right + 8.0,
        "lampu kiri: lantai kiri {left} harus lebih terang dari kanan {right}"
    );

    let flipped = PhotoOptions {
        key_light_offset_deg: -opts.key_light_offset_deg,
        ..opts
    };
    let lit_right = render_cpu(&s, &cam, &flipped, |_, _| true);
    let (left, right) = (
        mean(&lit_right, 40, 20, 60, 50),
        mean(&lit_right, 100, 20, 120, 50),
    );
    assert!(
        right > left + 8.0,
        "lampu kanan: lantai kanan {right} harus lebih terang dari kiri {left}"
    );
}

#[test]
fn cpu_bertahap_menghasilkan_gambar_yang_sama_dengan_sekali_jalan() {
    let s = scene(PhotoMaterial::default());
    let cam = iso_camera(&s, 96.0 / 72.0);
    let opts = small(3);
    let whole = render_cpu(&s, &cam, &opts, |_, _| true);

    let mut cpu = CpuPhoto::new(opts.width, opts.height);
    assert_eq!(cpu.size(), [96, 72]);
    let mut guard = 0;
    while cpu.samples() < 3 {
        // Anggaran nol: tiap panggilan mengerjakan satu irisan baris saja.
        cpu.step(&s, &cam, &opts, std::time::Duration::ZERO);
        guard += 1;
        assert!(guard < 10_000, "jalur bertahap tidak pernah selesai");
    }
    assert!(guard > 3, "anggaran nol harus memecah sampel jadi beberapa langkah");
    assert_eq!(cpu.image(&opts), whole);

    cpu.restart(48, 36);
    assert_eq!((cpu.samples(), cpu.size()), (0, [48, 36]));
}

#[test]
fn gpu_mendekati_cpu_bila_tersedia() {
    let s = scene(PhotoMaterial {
        base_color: [0.2, 0.5, 0.9],
        roughness: 0.4,
        ..PhotoMaterial::default()
    });
    let cam = iso_camera(&s, 96.0 / 72.0);
    let opts = small(64);
    let (img, backend) = render_headless(&s, &cam, &opts);
    assert_eq!((img.width, img.height), (96, 72));
    if backend == Backend::Cpu {
        eprintln!("dilewati: tidak ada adapter GPU yang sanggup; jalur CPU dipakai");
        return;
    }
    // Kedua jalur memakai transport cahaya yang sama dan konvergen ke
    // gambar yang sama; pada 64 sampel selisih rata-ratanya kecil.
    let cpu = render_cpu(&s, &cam, &opts, |_, _| true);
    let diff: f64 = img
        .rgba
        .iter()
        .zip(&cpu.rgba)
        .map(|(a, b)| (*a as f64 - *b as f64).abs())
        .sum::<f64>()
        / img.rgba.len() as f64;
    assert!(diff < 6.0, "GPU dan CPU berbeda rata-rata {diff} tingkat per kanal");
}

#[test]
fn png_valid_dan_berukuran_benar() {
    let img = PhotoImage {
        width: 3,
        height: 2,
        rgba: vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 10, 20, 30, 255, 40, 50, 60, 255, 70,
            80, 90, 255,
        ],
    };
    let png = img.to_png().unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let decoder = png::Decoder::new(std::io::Cursor::new(&png));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!((info.width, info.height), (3, 2));
    assert_eq!(info.color_type, png::ColorType::Rgb);
    assert_eq!(&buf[..9], &[255, 0, 0, 0, 255, 0, 0, 0, 255]);
}

#[test]
fn kunci_lingkungan_bolak_balik() {
    for env in Environment::ALL {
        assert_eq!(Environment::from_key(env.key()), Some(env));
    }
    assert_eq!(
        Environment::from_key("Studio-Contrast"),
        Some(Environment::StudioContrast)
    );
    assert_eq!(Environment::from_key("disko"), None);
}

#[test]
fn preset_kaca_menjadi_transmisi_tanpa_clearcoat() {
    let glass = material_of(&ducad_core::Material::translucent_glass(None));
    assert!(glass.transmission > 0.9);
    assert_eq!(glass.clearcoat, 0.0);

    let chrome = material_of(&ducad_core::Material::polished_chrome(None));
    assert_eq!(chrome.transmission, 0.0);
    assert_eq!(chrome.metallic, 1.0);

    let gloss = material_of(&ducad_core::Material::glossy_plastic(None));
    assert!(gloss.clearcoat > 0.5 && gloss.transmission == 0.0);
}

#[test]
fn kamera_tampak_memuat_seluruh_kotak() {
    // Kotak pipih dan panjang: pembingkaian lewat sudut harus memuatnya
    // tanpa menyisakan bingkai yang nyaris kosong.
    let (lo, hi) = ([-60.0, -10.0, 0.0], [60.0, 10.0, 5.0]);
    for view in [View::Iso, View::Front, View::Top, View::Right, View::Bottom] {
        for aspect in [0.6_f32, 1.0, 1.8] {
            let cam = camera_for_view(lo, hi, view, aspect);
            let eye = glam::Vec3::from_array(cam.eye);
            let target = glam::Vec3::from_array(cam.target);
            let fwd = (target - eye).normalize();
            let (_, up) = view.direction_up();
            let right = fwd.cross(up).normalize();
            let screen_up = right.cross(fwd);
            let (tan_y, tan_x) = ((cam.fov_y * 0.5).tan(), (cam.fov_y * 0.5).tan() * aspect);
            let mut widest = 0.0_f32;
            for i in 0..8 {
                let p = glam::Vec3::new(
                    if i & 1 == 0 { lo[0] } else { hi[0] },
                    if i & 2 == 0 { lo[1] } else { hi[1] },
                    if i & 4 == 0 { lo[2] } else { hi[2] },
                ) - eye;
                let depth = p.dot(fwd);
                assert!(depth > 0.0, "{view:?}: sudut di belakang kamera");
                let (nx, ny) = (
                    p.dot(right) / (depth * tan_x),
                    p.dot(screen_up) / (depth * tan_y),
                );
                assert!(
                    nx.abs() <= 1.0 && ny.abs() <= 1.0,
                    "{view:?} aspek {aspect}: sudut di luar bingkai ({nx}, {ny})"
                );
                widest = widest.max(nx.abs()).max(ny.abs());
            }
            assert!(
                widest > 0.8,
                "{view:?} aspek {aspect}: benda hanya mengisi {widest} dari bingkai"
            );
        }
    }
}

#[test]
fn render_gpu_berulang_memakai_ulang_perangkat_dan_mulai_dari_nol() {
    // Perangkat GPU tanpa jendela disimpan per proses. Render kedua dengan
    // masukan sama harus identik dengan yang pertama: akumulasi sampel
    // render sebelumnya tidak boleh terbawa.
    let s = scene(PhotoMaterial::default());
    let cam = iso_camera(&s, 96.0 / 72.0);
    let opts = small(16);
    let (a, backend) = render_headless(&s, &cam, &opts);
    if backend == Backend::Cpu {
        eprintln!("dilewati: tidak ada adapter GPU yang sanggup");
        return;
    }
    let start = std::time::Instant::now();
    let (b, _) = render_headless(&s, &cam, &opts);
    let second = start.elapsed();
    assert_eq!(a, b, "render berulang harus identik");
    // Ukuran berbeda lewat perangkat yang sama tetap benar.
    let (c, _) = render_headless(&s, &cam, &PhotoOptions { width: 48, height: 36, ..opts });
    assert_eq!((c.width, c.height), (48, 36));
    eprintln!("render GPU kedua (perangkat dipakai ulang): {second:?}");
}
