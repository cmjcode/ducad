//! Render berbayang CPU untuk lembar gambar (P21.5).
//!
//! Sengaja BUKAN wgpu: hasilnya harus identik byte demi byte di setiap mesin
//! supaya gold test `build` stabil. Rasterizer z-buffer ortografik sederhana
//! dengan shading Lambert dua-sisi, garis tepi dari diskontinuitas
//! kedalaman/normal, dan supersampling 2×2.

use ducad_kernel::KernelMesh;
use glam::{vec3, Vec3};

use super::spec::ShadedCamera;

/// Batas sisi terpanjang gambar (piksel) — menjaga ukuran PDF (risiko 3).
pub const MAX_SHADED_PX: u32 = 1600;

/// Gambar raster RGB 8-bit, baris teratas lebih dulu.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RasterImage {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

/// Satu body yang dirender beserta warna dasarnya (0..1).
pub struct ShadedBody<'a> {
    pub mesh: &'a KernelMesh,
    pub color: [f32; 3],
}

/// Render ortografik berbayang seluruh `bodies` dari `camera`, dipaskan ke
/// bingkai `width × height` piksel dengan latar putih.
pub fn render_shaded(
    bodies: &[ShadedBody<'_>],
    camera: ShadedCamera,
    width: u32,
    height: u32,
) -> RasterImage {
    let width = width.clamp(8, MAX_SHADED_PX);
    let height = height.clamp(8, MAX_SHADED_PX);
    const SS: usize = 2;
    let (w, h) = (width as usize * SS, height as usize * SS);
    let (right, up, dir) = camera.axes();

    // Kotak pembatas di ruang kamera.
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for body in bodies {
        for p in &body.mesh.positions {
            let p = Vec3::from_array(*p);
            let (u, v) = (p.dot(right), p.dot(up));
            lo = [lo[0].min(u), lo[1].min(v)];
            hi = [hi[0].max(u), hi[1].max(v)];
        }
    }
    let mut color = vec![1.0f32; w * h * 3];
    if lo[0] > hi[0] {
        return downsample(&color, w, h, SS);
    }
    let span_u = (hi[0] - lo[0]).max(1e-3);
    let span_v = (hi[1] - lo[1]).max(1e-3);
    let margin = 0.04;
    let scale = ((w as f32 * (1.0 - 2.0 * margin)) / span_u)
        .min((h as f32 * (1.0 - 2.0 * margin)) / span_v);
    let (cu, cv) = ((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5);
    let to_px = |p: Vec3| -> (f32, f32, f32) {
        (
            w as f32 * 0.5 + (p.dot(right) - cu) * scale,
            h as f32 * 0.5 - (p.dot(up) - cv) * scale,
            p.dot(dir),
        )
    };

    let light = (-dir * 0.75 + up * 0.55 - right * 0.35).normalize();
    let mut depth = vec![f32::MAX; w * h];
    let mut normal = vec![[0.0f32; 3]; w * h];

    for body in bodies {
        let mesh = body.mesh;
        let smooth = mesh.normals.len() == mesh.positions.len();
        for tri in mesh.indices.chunks_exact(3) {
            let idx = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
            if idx.iter().any(|i| *i >= mesh.positions.len()) {
                continue;
            }
            let p: [Vec3; 3] = [
                Vec3::from_array(mesh.positions[idx[0]]),
                Vec3::from_array(mesh.positions[idx[1]]),
                Vec3::from_array(mesh.positions[idx[2]]),
            ];
            let face_n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
            let n: [Vec3; 3] = if smooth {
                [
                    Vec3::from_array(mesh.normals[idx[0]]),
                    Vec3::from_array(mesh.normals[idx[1]]),
                    Vec3::from_array(mesh.normals[idx[2]]),
                ]
            } else {
                [face_n; 3]
            };
            let s = [to_px(p[0]), to_px(p[1]), to_px(p[2])];
            let area =
                (s[1].0 - s[0].0) * (s[2].1 - s[0].1) - (s[2].0 - s[0].0) * (s[1].1 - s[0].1);
            if area.abs() < 1e-6 {
                continue;
            }
            let min_x = s
                .iter()
                .map(|q| q.0)
                .fold(f32::MAX, f32::min)
                .floor()
                .max(0.0) as usize;
            let max_x = (s.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil() as isize)
                .min(w as isize - 1);
            let min_y = s
                .iter()
                .map(|q| q.1)
                .fold(f32::MAX, f32::min)
                .floor()
                .max(0.0) as usize;
            let max_y = (s.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil() as isize)
                .min(h as isize - 1);
            if max_x < 0 || max_y < 0 {
                continue;
            }
            for y in min_y..=max_y as usize {
                for x in min_x..=max_x as usize {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((s[1].0 - px) * (s[2].1 - py) - (s[2].0 - px) * (s[1].1 - py)) / area;
                    let w1 = ((s[2].0 - px) * (s[0].1 - py) - (s[0].0 - px) * (s[2].1 - py)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * s[0].2 + w1 * s[1].2 + w2 * s[2].2;
                    let i = y * w + x;
                    if z >= depth[i] {
                        continue;
                    }
                    depth[i] = z;
                    let mut nn = (n[0] * w0 + n[1] * w1 + n[2] * w2).normalize_or_zero();
                    if nn == Vec3::ZERO {
                        nn = face_n;
                    }
                    // Dua-sisi: normal selalu menghadap kamera.
                    if nn.dot(dir) > 0.0 {
                        nn = -nn;
                    }
                    normal[i] = nn.to_array();
                    let diffuse = nn.dot(light).max(0.0);
                    let rim = (1.0 - nn.dot(-dir).max(0.0)).powi(3) * 0.12;
                    let k = (0.38 + 0.62 * diffuse - rim).clamp(0.0, 1.0);
                    for c in 0..3 {
                        color[i * 3 + c] = (body.color[c] * k).clamp(0.0, 1.0);
                    }
                }
            }
        }
    }

    // Garis tepi: lompatan kedalaman atau lipatan normal antar piksel tetangga.
    let depth_step = 1.2 / scale.max(1e-6) + 0.004 * span_u.max(span_v);
    let mut edge = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            for (dx, dy) in [(1isize, 0isize), (0, 1)] {
                let (nx, ny) = (x as isize + dx, y as isize + dy);
                if nx >= w as isize || ny >= h as isize {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                let (a, b) = (depth[i], depth[j]);
                let boundary = (a == f32::MAX) != (b == f32::MAX);
                let jump = a != f32::MAX && b != f32::MAX && (a - b).abs() > depth_step * 6.0;
                let crease = a != f32::MAX
                    && b != f32::MAX
                    && Vec3::from_array(normal[i]).dot(Vec3::from_array(normal[j])) < 0.55;
                if boundary || jump || crease {
                    edge[i] = true;
                    edge[j] = true;
                }
            }
        }
    }
    let ink = vec3(0.10, 0.11, 0.13);
    for (i, is_edge) in edge.iter().enumerate() {
        if *is_edge {
            color[i * 3] = ink.x;
            color[i * 3 + 1] = ink.y;
            color[i * 3 + 2] = ink.z;
        }
    }

    downsample(&color, w, h, SS)
}

fn downsample(color: &[f32], w: usize, h: usize, ss: usize) -> RasterImage {
    let (ow, oh) = (w / ss, h / ss);
    let mut rgb = Vec::with_capacity(ow * oh * 3);
    let norm = 1.0 / (ss * ss) as f32;
    for y in 0..oh {
        for x in 0..ow {
            for c in 0..3 {
                let mut sum = 0.0;
                for sy in 0..ss {
                    for sx in 0..ss {
                        sum += color[((y * ss + sy) * w + x * ss + sx) * 3 + c];
                    }
                }
                rgb.push((sum * norm * 255.0).round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    RasterImage {
        width: ow as u32,
        height: oh as u32,
        rgb,
    }
}

fn crc32(bytes: &[u8], seed: u32) -> u32 {
    let mut crc = !seed;
    for b in bytes {
        crc ^= *b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(data, crc32(kind, 0));
    out.extend_from_slice(&crc.to_be_bytes());
}

impl RasterImage {
    pub fn is_empty(&self) -> bool {
        self.width == 0
            || self.height == 0
            || self.rgb.len() != (self.width * self.height * 3) as usize
    }

    /// Piksel RGB terkompresi zlib — isi stream `/FlateDecode` PDF.
    pub fn zlib_rgb(&self) -> Vec<u8> {
        miniz_oxide::deflate::compress_to_vec_zlib(&self.rgb, 6)
    }

    /// Berkas PNG RGB 8-bit (untuk `<image>` SVG dan tekstur GUI).
    pub fn png_bytes(&self) -> Vec<u8> {
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&self.width.to_be_bytes());
        ihdr.extend_from_slice(&self.height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, RGB, deflate, filter 0, tanpa interlace
        png_chunk(&mut out, b"IHDR", &ihdr);
        let stride = self.width as usize * 3;
        let mut raw = Vec::with_capacity((stride + 1) * self.height as usize);
        for row in self.rgb.chunks_exact(stride.max(1)) {
            raw.push(0); // filter None
            raw.extend_from_slice(row);
        }
        png_chunk(
            &mut out,
            b"IDAT",
            &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6),
        );
        png_chunk(&mut out, b"IEND", &[]);
        out
    }

    /// PNG sebagai `data:image/png;base64,…`.
    pub fn png_data_uri(&self) -> String {
        use base64::Engine as _;
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(self.png_bytes())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube_mesh() -> KernelMesh {
        // Kubus 10 mm dari 12 segitiga, tanpa normal per-vertex.
        let p = |x: f32, y: f32, z: f32| [x, y, z];
        let positions = vec![
            p(0.0, 0.0, 0.0),
            p(10.0, 0.0, 0.0),
            p(10.0, 10.0, 0.0),
            p(0.0, 10.0, 0.0),
            p(0.0, 0.0, 10.0),
            p(10.0, 0.0, 10.0),
            p(10.0, 10.0, 10.0),
            p(0.0, 10.0, 10.0),
        ];
        let indices = vec![
            0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7,
            6, 3, 0, 4, 3, 4, 7,
        ];
        KernelMesh {
            positions,
            normals: Vec::new(),
            indices,
            face_ranges: Vec::new(),
        }
    }

    #[test]
    fn shaded_render_is_deterministic_and_shows_three_faces() {
        let mesh = cube_mesh();
        let bodies = [ShadedBody {
            mesh: &mesh,
            color: [0.7, 0.7, 0.75],
        }];
        let a = render_shaded(&bodies, ShadedCamera::Iso, 120, 90);
        let b = render_shaded(&bodies, ShadedCamera::Iso, 120, 90);
        assert_eq!(a, b);
        assert_eq!((a.width, a.height), (120, 90));
        assert!(!a.is_empty());
        // Pojok = latar putih; tengah = body berwarna, bukan putih.
        assert_eq!(&a.rgb[0..3], &[255, 255, 255]);
        let mid = (45 * 120 + 60) * 3;
        assert!(a.rgb[mid] < 250, "tengah gambar harus tertutup body");
        // Tiga muka terlihat → minimal tiga tingkat terang yang berbeda.
        let mut tones: Vec<u8> = a
            .rgb
            .chunks_exact(3)
            .map(|c| c[0])
            .filter(|v| *v < 250 && *v > 60)
            .collect();
        tones.sort_unstable();
        tones.dedup_by(|x, y| x.abs_diff(*y) < 12);
        assert!(tones.len() >= 3, "{tones:?}");
        // Kamera lain menghasilkan gambar lain.
        let custom = ShadedCamera::Custom {
            yaw: 20.0,
            pitch: 10.0,
        };
        assert_ne!(a, render_shaded(&bodies, custom, 120, 90));
    }

    #[test]
    fn png_encoding_is_well_formed() {
        let img = RasterImage {
            width: 2,
            height: 2,
            rgb: vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        };
        let png = img.png_bytes();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
        // CRC IEND yang sudah dikenal luas.
        assert_eq!(&png[png.len() - 4..], &[0xAE, 0x42, 0x60, 0x82]);
        let back = miniz_oxide::inflate::decompress_to_vec_zlib(&img.zlib_rgb()).unwrap();
        assert_eq!(back, img.rgb);
        assert!(img
            .png_data_uri()
            .starts_with("data:image/png;base64,iVBORw0KGgo"));
    }
}
