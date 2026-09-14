//! Proyektor 3D → 2D bersama untuk dua pemakai yang berbeda sifatnya.
//!
//! [`crate::hlr`] memproyeksikan ORTOGONAL ke bidang gambar dalam milimeter
//! (gambar kerja teknik: skala 1:1, tidak ada perspektif, tidak ada layar).
//! [`crate::vector_snapshot`] memproyeksikan PERSPEKTIF ke piksel viewport
//! (tangkapan vektor dari kamera yang sedang dipakai pengguna).
//!
//! Dua-duanya butuh operasi yang sama persis di baliknya: proyeksikan titik,
//! tentukan segitiga mana yang menghadap kamera, dan bandingkan kedalaman
//! untuk uji oklusi. Modul ini menyatukan ketiganya supaya HLR berbasis
//! mesh di `hlr.rs` tidak perlu digandakan hanya untuk mengganti rumus
//! proyeksinya.
//!
//! # Konvensi kedalaman
//!
//! [`Projector::project`] mengembalikan `depth` dengan aturan **makin besar
//! makin dekat ke kamera** di kedua mode, supaya `hlr::test_occlusion` bisa
//! membandingkannya dengan satu perbandingan `>` tanpa tahu mode mana yang
//! dipakai.
//!
//! Untuk mode perspektif nilai yang disimpan adalah `1/z` (bukan `-z`), dan
//! itu BUKAN pilihan kosmetik: interpolasi kedalaman di `test_occlusion`
//! dilakukan secara barisentrik DI RUANG LAYAR, dan hanya `1/z` yang linier
//! di ruang layar pada proyeksi perspektif. Menyimpan `-z` akan membuat
//! kedalaman segitiga salah di bagian tengah segitiga yang miring terhadap
//! kamera — persis kasus yang paling sering menentukan garis tersembunyi.
//! `1/z` juga tetap memenuhi "makin besar makin dekat" karena `z > 0`.

use glam::{vec2, Vec2, Vec3};

/// Toleransi kedalaman bawaan untuk HLR gambar kerja (mm).
///
/// Nilai yang sama dengan yang dipakai `hlr.rs` sebelum proyektor ini ada;
/// dipertahankan supaya gambar teknik yang sudah diekspor tidak berubah.
pub const DEFAULT_DEPTH_TOLERANCE_MM: f32 = 0.50;

/// Proyeksi 3D → 2D beserta informasi kedalaman untuk uji oklusi.
#[derive(Debug, Clone)]
pub enum Projector {
    /// Proyeksi ortogonal (paralel). Dipakai gambar kerja teknik.
    Orthographic {
        /// Sumbu horizontal bidang gambar.
        right: Vec3,
        /// Sumbu vertikal bidang gambar.
        up: Vec3,
        /// Sumbu kedalaman; makin besar hasil dot-nya, makin dekat ke kamera.
        depth: Vec3,
        /// Arah pandang, dari kamera menuju objek.
        view_dir: Vec3,
        /// Skala (u, v) dari satuan dunia ke satuan keluaran. `(1, 1)` untuk
        /// gambar kerja dalam mm; `v` negatif membalik sumbu ke layar y-turun.
        scale: Vec2,
        /// Geseran (u, v) setelah penskalaan.
        bias: Vec2,
        /// Selisih kedalaman minimum agar sebuah segitiga dianggap benar-benar
        /// berada di depan titik uji (satuan dunia).
        depth_tolerance: f32,
    },
    /// Proyeksi perspektif pinhole ke piksel layar. Dipakai Vector Snapshot.
    Perspective {
        /// Posisi mata kamera.
        eye: Vec3,
        /// Arah pandang ternormalisasi, dari mata menuju target.
        forward: Vec3,
        /// Sumbu kanan layar ternormalisasi.
        right: Vec3,
        /// Sumbu atas layar ternormalisasi.
        up: Vec3,
        /// Jarak bidang dekat; titik dengan `z < near` tidak dapat diproyeksikan.
        near: f32,
        /// Panjang fokus dalam piksel. `y` negatif karena sumbu y layar turun.
        focal: Vec2,
        /// Titik pusat proyeksi dalam piksel (biasanya tengah viewport).
        center: Vec2,
        /// Toleransi oklusi dalam satuan dunia, dikonversi ke ruang `1/z`
        /// oleh [`Projector::depth_epsilon`].
        depth_tolerance: f32,
    },
}

impl Projector {
    /// Proyektor ortogonal gambar kerja: keluaran dalam milimeter, sumbu `v`
    /// mengarah ke atas, tanpa penskalaan.
    ///
    /// `view_dir` adalah arah dari kamera menuju objek.
    pub fn orthographic_mm(view_dir: Vec3, right: Vec3, up: Vec3) -> Self {
        Projector::Orthographic {
            right,
            up,
            depth: -view_dir,
            view_dir,
            scale: vec2(1.0, 1.0),
            bias: Vec2::ZERO,
            depth_tolerance: DEFAULT_DEPTH_TOLERANCE_MM,
        }
    }

    /// Proyektor ortogonal yang membingkai layar seperti kamera perspektif
    /// dengan `fov_y` pada jarak `eye`→`target` — dipakai Vector Snapshot mode
    /// ortogonal supaya hasilnya sebesar yang terlihat di viewport.
    pub fn orthographic_screen(
        eye: Vec3,
        target: Vec3,
        up_hint: Vec3,
        fov_y: f32,
        width_px: f32,
        height_px: f32,
        depth_tolerance: f32,
    ) -> Option<Self> {
        let forward = (target - eye).normalize_or_zero();
        if forward.length_squared() < 0.5 {
            return None;
        }
        let right = forward.cross(up_hint).normalize_or_zero();
        if right.length_squared() < 0.5 {
            return None;
        }
        let up = right.cross(forward).normalize_or_zero();

        let tan_half = (fov_y * 0.5).tan();
        if !(tan_half.is_finite() && tan_half > 1e-6) {
            return None;
        }
        let distance = (target - eye).length();
        let world_per_px = (2.0 * distance * tan_half / height_px.max(1.0)).max(1e-9);
        let s = 1.0 / world_per_px;

        // Titik `target` harus jatuh tepat di tengah layar.
        let bias = vec2(
            width_px * 0.5 - target.dot(right) * s,
            height_px * 0.5 + target.dot(up) * s,
        );

        Some(Projector::Orthographic {
            right,
            up,
            depth: -forward,
            view_dir: forward,
            scale: vec2(s, -s),
            bias,
            depth_tolerance,
        })
    }

    /// Proyektor perspektif dari parameter kamera viewport.
    ///
    /// Mengembalikan `None` bila `target` berimpit dengan `eye` atau `up_hint`
    /// sejajar arah pandang — dua kasus yang membuat basis kamera tidak
    /// terdefinisi.
    #[allow(clippy::too_many_arguments)]
    pub fn perspective(
        eye: Vec3,
        target: Vec3,
        up_hint: Vec3,
        fov_y: f32,
        width_px: f32,
        height_px: f32,
        near: f32,
        depth_tolerance: f32,
    ) -> Option<Self> {
        let forward = (target - eye).normalize_or_zero();
        if forward.length_squared() < 0.5 {
            return None;
        }
        let right = forward.cross(up_hint).normalize_or_zero();
        if right.length_squared() < 0.5 {
            return None;
        }
        let up = right.cross(forward).normalize_or_zero();

        let tan_half = (fov_y * 0.5).tan();
        if !(tan_half.is_finite() && tan_half > 1e-6) {
            return None;
        }
        // Piksel persegi: fokus x dan y sama besar, diturunkan dari tinggi
        // viewport. `y` negatif karena sumbu y layar menunjuk ke bawah.
        let f = (height_px.max(1.0) * 0.5) / tan_half;

        Some(Projector::Perspective {
            eye,
            forward,
            right,
            up,
            near: near.max(1e-4),
            focal: vec2(f, -f),
            center: vec2(width_px * 0.5, height_px * 0.5),
            depth_tolerance,
        })
    }

    /// Proyeksikan satu titik dunia.
    ///
    /// Mengembalikan `(posisi_2d, kedalaman)` dengan kedalaman **makin besar
    /// makin dekat**, atau `None` bila titik berada di belakang bidang dekat
    /// (hanya mungkin pada mode perspektif).
    #[inline]
    pub fn project(&self, p: Vec3) -> Option<(Vec2, f32)> {
        match *self {
            Projector::Orthographic {
                right,
                up,
                depth,
                scale,
                bias,
                ..
            } => Some((
                vec2(p.dot(right) * scale.x + bias.x, p.dot(up) * scale.y + bias.y),
                p.dot(depth),
            )),
            Projector::Perspective {
                eye,
                forward,
                right,
                up,
                near,
                focal,
                center,
                ..
            } => {
                let rel = p - eye;
                let z = rel.dot(forward);
                if !(z.is_finite() && z >= near) {
                    return None;
                }
                let inv_z = 1.0 / z;
                Some((
                    vec2(
                        center.x + focal.x * rel.dot(right) * inv_z,
                        center.y + focal.y * rel.dot(up) * inv_z,
                    ),
                    inv_z,
                ))
            }
        }
    }

    /// Hasil dot antara normal segitiga dan arah pandang di titik itu.
    /// **Negatif berarti menghadap kamera.**
    ///
    /// Pada mode ortogonal nilainya tidak dinormalisasi — sama persis dengan
    /// perhitungan `hlr.rs` sebelum proyektor ini ada, supaya ambang batas
    /// lama tetap berperilaku sama. Pada mode perspektif kedua vektor
    /// dinormalisasi karena arah pandang berbeda-beda per titik dan ambang
    /// batasnya harus berarti "kosinus", bukan skala luas segitiga.
    #[inline]
    pub fn facing(&self, normal: Vec3, point: Vec3) -> f32 {
        match *self {
            Projector::Orthographic { view_dir, .. } => normal.dot(view_dir),
            Projector::Perspective { eye, .. } => normal
                .normalize_or_zero()
                .dot((point - eye).normalize_or_zero()),
        }
    }

    /// Selisih kedalaman minimum yang dianggap "benar-benar di depan" pada
    /// titik uji berkedalaman `test_depth`.
    ///
    /// Pada mode perspektif kedalaman disimpan sebagai `1/z`, jadi toleransi
    /// jarak dunia `t` harus dikonversi: `1/z − 1/(z+t) ≈ t/z²`, dan `1/z²`
    /// tidak lain adalah `test_depth²`.
    #[inline]
    pub fn depth_epsilon(&self, test_depth: f32) -> f32 {
        match *self {
            Projector::Orthographic {
                depth_tolerance, ..
            } => depth_tolerance,
            Projector::Perspective {
                depth_tolerance, ..
            } => depth_tolerance * test_depth * test_depth,
        }
    }

    /// Potong segmen 3D pada bidang dekat kamera.
    ///
    /// Mengembalikan `None` bila seluruh segmen berada di belakang bidang
    /// dekat. Pada mode ortogonal segmen selalu diteruskan apa adanya.
    pub fn clip_to_near_plane(&self, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
        let Projector::Perspective {
            eye, forward, near, ..
        } = *self
        else {
            return Some((a, b));
        };

        let za = (a - eye).dot(forward);
        let zb = (b - eye).dot(forward);

        match (za >= near, zb >= near) {
            (true, true) => Some((a, b)),
            (false, false) => None,
            (true, false) => {
                let t = (near - za) / (zb - za);
                Some((a, a + (b - a) * t))
            }
            (false, true) => {
                let t = (near - za) / (zb - za);
                Some((a + (b - a) * t, b))
            }
        }
    }
}

/// Potong segmen 3D pada setengah-ruang yang terlihat dari sebuah bidang
/// potong, dengan konvensi yang sama dengan shader viewport: titik dibuang
/// bila `dot(normal, p) − offset > 0`.
///
/// Mengembalikan `None` bila seluruh segmen terpotong.
pub fn clip_segment_to_halfspace(
    a: Vec3,
    b: Vec3,
    normal: Vec3,
    offset: f32,
) -> Option<(Vec3, Vec3)> {
    let da = normal.dot(a) - offset;
    let db = normal.dot(b) - offset;
    match (da <= 0.0, db <= 0.0) {
        (true, true) => Some((a, b)),
        (false, false) => None,
        (true, false) => {
            let t = da / (da - db);
            Some((a, a + (b - a) * t))
        }
        (false, true) => {
            let t = da / (da - db);
            Some((a + (b - a) * t, b))
        }
    }
}

/// Potong segmen 2D ke dalam kotak `[0, width] × [0, height]` dengan
/// algoritma Liang–Barsky.
///
/// Mengembalikan `None` bila segmen sepenuhnya di luar kotak. Dipakai Vector
/// Snapshot supaya geometri di luar layar tidak ikut masuk ke berkas SVG —
/// tangkapan harus berisi apa yang terlihat, bukan seluruh model.
pub fn clip_segment_to_rect(a: Vec2, b: Vec2, width: f32, height: f32) -> Option<(Vec2, Vec2)> {
    let d = b - a;
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;

    // (p, q) untuk keempat sisi: kiri, kanan, atas, bawah.
    let checks = [
        (-d.x, a.x),
        (d.x, width - a.x),
        (-d.y, a.y),
        (d.y, height - a.y),
    ];

    for (p, q) in checks {
        if p.abs() < 1e-9 {
            // Sejajar sisi ini: di luar bila memang sudah di sisi luar.
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let r = q / p;
        if p < 0.0 {
            if r > t1 {
                return None;
            }
            if r > t0 {
                t0 = r;
            }
        } else {
            if r < t0 {
                return None;
            }
            if r < t1 {
                t1 = r;
            }
        }
    }

    if t1 < t0 {
        return None;
    }
    Some((a + d * t0, a + d * t1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::vec3;

    fn kamera_uji() -> Projector {
        Projector::perspective(
            vec3(0.0, -100.0, 0.0),
            Vec3::ZERO,
            Vec3::Z,
            45f32.to_radians(),
            800.0,
            600.0,
            0.1,
            0.5,
        )
        .unwrap()
    }

    #[test]
    fn orthographic_mm_matches_raw_dot_products() {
        let view_dir = vec3(0.0, 1.0, 0.0);
        let right = vec3(1.0, 0.0, 0.0);
        let up = vec3(0.0, 0.0, 1.0);
        let proj = Projector::orthographic_mm(view_dir, right, up);

        let p = vec3(3.0, 7.0, -2.0);
        let (uv, d) = proj.project(p).unwrap();
        assert!((uv.x - p.dot(right)).abs() < 1e-6);
        assert!((uv.y - p.dot(up)).abs() < 1e-6);
        assert!((d - p.dot(-view_dir)).abs() < 1e-6);
    }

    #[test]
    fn orthographic_never_clips_at_near_plane() {
        let proj = Projector::orthographic_mm(Vec3::Y, Vec3::X, Vec3::Z);
        let a = vec3(0.0, -1e6, 0.0);
        let b = vec3(0.0, 1e6, 0.0);
        assert!(proj.clip_to_near_plane(a, b).is_some());
    }

    #[test]
    fn perspective_puts_target_at_screen_center() {
        let (uv, _) = kamera_uji().project(Vec3::ZERO).unwrap();
        assert!((uv.x - 400.0).abs() < 1e-3, "u = {}", uv.x);
        assert!((uv.y - 300.0).abs() < 1e-3, "v = {}", uv.y);
    }

    #[test]
    fn perspective_screen_y_axis_points_down() {
        let proj = kamera_uji();
        // Titik yang lebih TINGGI di dunia (+Z) harus punya v LEBIH KECIL.
        let (atas, _) = proj.project(vec3(0.0, 0.0, 10.0)).unwrap();
        let (bawah, _) = proj.project(vec3(0.0, 0.0, -10.0)).unwrap();
        assert!(atas.y < bawah.y);
    }

    #[test]
    fn perspective_depth_is_larger_when_closer() {
        let proj = kamera_uji();
        let (_, dekat) = proj.project(vec3(0.0, -50.0, 0.0)).unwrap();
        let (_, jauh) = proj.project(vec3(0.0, 50.0, 0.0)).unwrap();
        assert!(dekat > jauh);
    }

    #[test]
    fn perspective_rejects_points_behind_the_camera() {
        assert!(kamera_uji().project(vec3(0.0, -200.0, 0.0)).is_none());
    }

    #[test]
    fn perspective_clips_segment_that_crosses_the_near_plane() {
        let proj = Projector::perspective(
            vec3(0.0, -100.0, 0.0),
            Vec3::ZERO,
            Vec3::Z,
            45f32.to_radians(),
            800.0,
            600.0,
            1.0,
            0.5,
        )
        .unwrap();
        let belakang = vec3(0.0, -150.0, 0.0);
        let depan = Vec3::ZERO;
        let (a, b) = proj.clip_to_near_plane(belakang, depan).unwrap();
        assert!(proj.project(a).is_some(), "ujung terpotong harus terproyeksi");
        assert!((b - depan).length() < 1e-4);
        assert!(proj
            .clip_to_near_plane(belakang, vec3(0.0, -140.0, 0.0))
            .is_none());
    }

    #[test]
    fn perspective_rejects_up_vector_parallel_to_view() {
        assert!(Projector::perspective(
            vec3(0.0, 0.0, -100.0),
            Vec3::ZERO,
            Vec3::Z,
            45f32.to_radians(),
            800.0,
            600.0,
            0.1,
            0.5,
        )
        .is_none());
    }

    #[test]
    fn orthographic_screen_frames_like_the_perspective_camera() {
        let eye = vec3(0.0, -100.0, 0.0);
        let ortho = Projector::orthographic_screen(
            eye,
            Vec3::ZERO,
            Vec3::Z,
            45f32.to_radians(),
            800.0,
            600.0,
            0.5,
        )
        .unwrap();
        // Pada bidang yang melewati target, kedua proyeksi harus sama.
        let p = vec3(12.0, 0.0, -8.0);
        let (a, _) = ortho.project(p).unwrap();
        let (b, _) = kamera_uji().project(p).unwrap();
        assert!((a - b).length() < 1e-2, "{a:?} vs {b:?}");
    }

    #[test]
    fn depth_epsilon_shrinks_with_distance_in_perspective() {
        let proj = Projector::perspective(
            Vec3::ZERO,
            Vec3::Y,
            Vec3::Z,
            45f32.to_radians(),
            800.0,
            600.0,
            0.1,
            1.0,
        )
        .unwrap();
        // Toleransi 1 mm pada jarak 10 vs jarak 100.
        let dekat = proj.depth_epsilon(1.0 / 10.0);
        let jauh = proj.depth_epsilon(1.0 / 100.0);
        assert!(dekat > jauh);
        assert!((dekat - 0.01).abs() < 1e-6);
    }

    #[test]
    fn halfspace_clip_keeps_the_visible_side() {
        let n = Vec3::Y;
        let a = vec3(0.0, -5.0, 0.0);
        let b = vec3(0.0, 5.0, 0.0);
        let (ca, cb) = clip_segment_to_halfspace(a, b, n, 0.0).unwrap();
        assert!((ca - a).length() < 1e-5);
        assert!(cb.y.abs() < 1e-5);
        assert!(clip_segment_to_halfspace(vec3(0.0, 1.0, 0.0), b, n, 0.0).is_none());
    }

    #[test]
    fn rect_clip_trims_to_the_viewport() {
        let (a, b) =
            clip_segment_to_rect(vec2(-50.0, 10.0), vec2(50.0, 10.0), 100.0, 100.0).unwrap();
        assert!(a.x.abs() < 1e-4);
        assert!((b.x - 50.0).abs() < 1e-4);
        assert!(clip_segment_to_rect(vec2(-50.0, -50.0), vec2(-10.0, -10.0), 100.0, 100.0).is_none());
        // Segmen yang seluruhnya di dalam tidak berubah.
        let (a, b) = clip_segment_to_rect(vec2(10.0, 10.0), vec2(20.0, 20.0), 100.0, 100.0).unwrap();
        assert!((a - vec2(10.0, 10.0)).length() < 1e-5);
        assert!((b - vec2(20.0, 20.0)).length() < 1e-5);
    }
}
