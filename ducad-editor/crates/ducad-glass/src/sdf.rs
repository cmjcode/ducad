//! Geometri kaca di CPU: SDF persegi-bersudut-bulat, normal, dan profil lensa.
//!
//! Rumus di sini adalah acuan untuk `shaders/glass.wgsl` — shader memakai
//! rumus yang sama per piksel. Menaruhnya juga di CPU membuat perilakunya
//! bisa diuji tanpa GPU (tes di bawah) dan dipakai ulang oleh widget.

/// Jarak bertanda ke persegi bersudut bulat yang berpusat di titik asal.
///
/// Negatif di dalam bentuk, nol di tepi, positif di luar. `half` = setengah
/// ukuran, `radius` = jari-jari sudut (dijepit agar tidak melebihi sisi
/// terpendek).
pub fn rounded_rect_sdf(p: [f32; 2], half: [f32; 2], radius: f32) -> f32 {
    let r = clamp_radius(half, radius);
    let qx = p[0].abs() - half[0] + r;
    let qy = p[1].abs() - half[1] + r;
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let inside = qx.max(qy).min(0.0);
    outside + inside - r
}

/// Jari-jari sudut yang sah untuk ukuran tertentu (tidak negatif, tidak
/// melebihi setengah sisi terpendek).
pub fn clamp_radius(half: [f32; 2], radius: f32) -> f32 {
    radius.clamp(0.0, half[0].min(half[1]).max(0.0))
}

/// Normal satuan mengarah KE LUAR bentuk (gradien SDF, beda hingga terpusat).
///
/// Di pusat bentuk gradien bernilai nol; fungsi mengembalikan `[0, 0]` di
/// sana sehingga pemanggil tidak pernah menerima NaN.
pub fn rounded_rect_normal(p: [f32; 2], half: [f32; 2], radius: f32) -> [f32; 2] {
    const E: f32 = 0.5;
    let dx = rounded_rect_sdf([p[0] + E, p[1]], half, radius)
        - rounded_rect_sdf([p[0] - E, p[1]], half, radius);
    let dy = rounded_rect_sdf([p[0], p[1] + E], half, radius)
        - rounded_rect_sdf([p[0], p[1] - E], half, radius);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        [0.0, 0.0]
    } else {
        [dx / len, dy / len]
    }
}

/// Profil lensa: seberapa kuat latar dibelokkan pada kedalaman `depth`
/// (jarak dari tepi ke arah dalam, ≥ 0) untuk pita rim selebar `rim_width`.
///
/// Profil permukaan lensa cembung (busur lingkaran): bernilai 1 tepat di
/// tepi, turun curam lalu melandai ke 0 di batas dalam pita, dan tetap 0 di
/// bagian tengah — tengah kaca lurus, rim-nya "menarik" latar seperti tepi
/// lensa tebal (ciri Liquid Glass).
pub fn lens_profile(depth: f32, rim_width: f32) -> f32 {
    if rim_width <= 0.0 {
        return 0.0;
    }
    let t = (depth / rim_width).clamp(0.0, 1.0);
    let u = 1.0 - t;
    1.0 - (1.0 - u * u).max(0.0).sqrt()
}

/// Kedalaman halus dari tepi: jarak ke sisi terdekat, digabung smooth-min
/// selebar `k` agar gradiennya (arah tarikan lensa) tidak berbelok mendadak
/// di diagonal sudut. Dipakai shader untuk arah dan kekuatan lensa.
pub fn smooth_inner_depth(p: [f32; 2], half: [f32; 2], k: f32) -> f32 {
    let dx = half[0] - p[0].abs();
    let dy = half[1] - p[1].abs();
    let k = k.max(1e-3);
    let h = (0.5 + 0.5 * (dy - dx) / k).clamp(0.0, 1.0);
    dy + (dx - dy) * h - k * h * (1.0 - h)
}

/// Pergeseran sampel latar (piksel) untuk titik `p` di dalam kaca.
///
/// Arah pergeseran berlawanan dengan normal (ke dalam bentuk): tepi kaca
/// "menarik" gambar dari bagian yang lebih dalam, seperti tepi lensa cembung.
pub fn refraction_offset(
    p: [f32; 2],
    half: [f32; 2],
    radius: f32,
    rim_width: f32,
    refraction: f32,
) -> [f32; 2] {
    let d = rounded_rect_sdf(p, half, radius);
    if d > 0.0 {
        return [0.0, 0.0];
    }
    let n = rounded_rect_normal(p, half, radius);
    let k = lens_profile(-d, rim_width) * refraction;
    [-n[0] * k, -n[1] * k]
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: [f32; 2] = [100.0, 40.0];
    const R: f32 = 12.0;

    #[test]
    fn sdf_sign_matches_inside_edge_outside() {
        // Pusat: sedalam setengah sisi terpendek.
        assert!((rounded_rect_sdf([0.0, 0.0], HALF, R) + 40.0).abs() < 1e-4);
        // Tepat di sisi kanan dan sisi atas.
        assert!(rounded_rect_sdf([100.0, 0.0], HALF, R).abs() < 1e-4);
        assert!(rounded_rect_sdf([0.0, -40.0], HALF, R).abs() < 1e-4);
        // Di luar.
        assert!((rounded_rect_sdf([110.0, 0.0], HALF, R) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn sdf_rounds_the_corner() {
        // Titik sudut persegi tajam berada DI LUAR bentuk bersudut bulat,
        // sejauh r·(√2 − 1) dari busur.
        let d = rounded_rect_sdf(HALF, HALF, R);
        assert!((d - R * (2f32.sqrt() - 1.0)).abs() < 1e-3, "d = {d}");
        // Radius nol → sudut tajam tepat di tepi.
        assert!(rounded_rect_sdf(HALF, HALF, 0.0).abs() < 1e-4);
    }

    #[test]
    fn radius_is_clamped_to_shorter_side() {
        // Radius raksasa menjadi kapsul: ujung kanan tetap di tepi.
        assert_eq!(clamp_radius(HALF, 999.0), 40.0);
        assert!(rounded_rect_sdf([100.0, 0.0], HALF, 999.0).abs() < 1e-4);
        assert_eq!(clamp_radius(HALF, -5.0), 0.0);
    }

    #[test]
    fn normal_points_outward_and_is_unit() {
        let n = rounded_rect_normal([95.0, 0.0], HALF, R);
        assert!(n[0] > 0.99 && n[1].abs() < 1e-3, "kanan: {n:?}");
        let n = rounded_rect_normal([0.0, -38.0], HALF, R);
        assert!(n[1] < -0.99 && n[0].abs() < 1e-3, "atas: {n:?}");
        // Di sudut: diagonal, tetap satuan.
        let n = rounded_rect_normal([96.0, 36.0], HALF, R);
        assert!(n[0] > 0.3 && n[1] > 0.3);
        assert!(((n[0] * n[0] + n[1] * n[1]).sqrt() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn normal_is_finite_at_center() {
        // Kapsul sempurna: gradien tepat nol di pusat, tidak boleh NaN.
        let n = rounded_rect_normal([0.0, 0.0], [40.0, 40.0], 40.0);
        assert!(n[0].is_finite() && n[1].is_finite());
    }

    #[test]
    fn smooth_depth_matches_edge_distance_away_from_corners_and_is_continuous() {
        // Jauh dari sudut: sama dengan jarak ke sisi terdekat.
        assert!((smooth_inner_depth([0.0, 0.0], HALF, 20.0) - 40.0).abs() < 1e-4);
        assert!((smooth_inner_depth([90.0, 0.0], HALF, 20.0) - 10.0).abs() < 1e-4);
        // Di sudut: lebih kecil daripada keduanya (menyatu), tanpa lompatan
        // sepanjang diagonal.
        let mut prev = smooth_inner_depth([60.0, 0.0], HALF, 20.0);
        for i in 1..=40 {
            let t = i as f32;
            let v = smooth_inner_depth([60.0 + t, t], HALF, 20.0);
            assert!((v - prev).abs() < 2.0, "lompatan di langkah {i}: {prev} → {v}");
            prev = v;
        }
    }

    #[test]
    fn lens_is_strongest_at_edge_and_zero_inside() {
        assert_eq!(lens_profile(0.0, 16.0), 1.0);
        assert_eq!(lens_profile(16.0, 16.0), 0.0);
        assert_eq!(lens_profile(80.0, 16.0), 0.0);
        assert_eq!(lens_profile(4.0, 0.0), 0.0);
        // Monoton turun.
        let mut prev = f32::INFINITY;
        for i in 0..=16 {
            let v = lens_profile(i as f32, 16.0);
            assert!(v <= prev);
            prev = v;
        }
    }

    #[test]
    fn refraction_only_bends_the_rim() {
        // Tengah panel: tidak ada distorsi sama sekali.
        assert_eq!(
            refraction_offset([0.0, 0.0], HALF, R, 16.0, 20.0),
            [0.0, 0.0]
        );
        // Dekat tepi kanan: sampel ditarik ke kiri (ke dalam).
        let o = refraction_offset([98.0, 0.0], HALF, R, 16.0, 20.0);
        assert!(o[0] < -5.0 && o[1].abs() < 1e-2, "{o:?}");
        // Besarnya tidak pernah melebihi parameter refraksi.
        assert!(o[0].abs() <= 20.0);
        // Di luar bentuk: nol.
        assert_eq!(
            refraction_offset([150.0, 0.0], HALF, R, 16.0, 20.0),
            [0.0, 0.0]
        );
    }
}
