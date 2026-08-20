//! Pengukuran non-destruktif di bidang sketch (Fase 7) — jarak & sudut.
//! Murni fungsi baca-saja: tidak membuat `Entity`, tidak menyentuh
//! `Sketch`/undo stack sama sekali (beda dari Line/Arc/dst yang selalu
//! jadi entitas nyata) — dipakai `ducad-app` untuk tool "Ukur" yang cuma
//! menampilkan angka, bukan menggambar apa pun ke dokumen.

use glam::DVec2;

/// Jarak lurus antara dua titik (mm, sama satuan dengan seluruh sketch).
pub fn distance(a: DVec2, b: DVec2) -> f64 {
    (b - a).length()
}

/// Sudut interior (derajat, 0–180) di titik `vertex` antara sinar ke `a`
/// dan sinar ke `b` — `atan2(det, dot)` supaya hasilnya independen dari
/// arah mana `a`/`b` diklik lebih dulu (beda dari `atan2` mentah yang bisa
/// negatif tergantung urutan). `None` kalau `a` atau `b` berimpit dengan
/// `vertex` (sinar tidak terdefinisi, sudut tidak bermakna).
pub fn angle_degrees(a: DVec2, vertex: DVec2, b: DVec2) -> Option<f64> {
    let v1 = a - vertex;
    let v2 = b - vertex;
    if v1.length() < 1e-9 || v2.length() < 1e-9 {
        return None;
    }
    let dot = v1.dot(v2);
    let det = v1.x * v2.y - v1.y * v2.x;
    Some(det.atan2(dot).abs().to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_3_4_5_triangle() {
        let a = DVec2::new(0.0, 0.0);
        let b = DVec2::new(3.0, 4.0);
        assert!((distance(a, b) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn distance_zero_for_coincident_points() {
        let p = DVec2::new(1.5, -2.5);
        assert!(distance(p, p) < 1e-9);
    }

    #[test]
    fn angle_right_angle_at_origin() {
        let vertex = DVec2::new(0.0, 0.0);
        let a = DVec2::new(5.0, 0.0);
        let b = DVec2::new(0.0, 3.0);
        let angle = angle_degrees(a, vertex, b).unwrap();
        assert!((angle - 90.0).abs() < 1e-9);
    }

    #[test]
    fn angle_straight_line_is_180() {
        let vertex = DVec2::new(1.0, 1.0);
        let a = DVec2::new(0.0, 1.0);
        let b = DVec2::new(3.0, 1.0);
        let angle = angle_degrees(a, vertex, b).unwrap();
        assert!((angle - 180.0).abs() < 1e-9);
    }

    #[test]
    fn angle_is_order_independent() {
        let vertex = DVec2::new(0.0, 0.0);
        let a = DVec2::new(2.0, 0.0);
        let b = DVec2::new(0.0, 2.0);
        // Menukar urutan a/b (klik titik kedua duluan) harus tetap hasil sama.
        assert_eq!(angle_degrees(a, vertex, b), angle_degrees(b, vertex, a));
    }

    #[test]
    fn angle_degenerate_when_ray_has_zero_length() {
        let vertex = DVec2::new(2.0, 2.0);
        assert!(angle_degrees(vertex, vertex, DVec2::new(5.0, 5.0)).is_none());
    }
}
