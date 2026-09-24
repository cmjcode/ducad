//! Pemotongan coretan tinta menggunakan garis pemotong (Slice).

use glam::Vec2;

use crate::stroke::InkPoint;

/// Memotong sekumpulan titik coretan menggunakan garis potong `line: (p_start, p_end)`.
///
/// Setiap kali segmen coretan bersilangan dengan garis pemotong, titik potong baru diinterpolasi
/// dan menjadi titik akhir pecahan sebelumnya sekaligus titik awal pecahan berikutnya.
/// Bila tidak ada perpotongan, mengembalikan `vec![points.to_vec()]`.
pub fn slice(points: &[InkPoint], line: (Vec2, Vec2)) -> Vec<Vec<InkPoint>> {
    if points.len() < 2 {
        return vec![points.to_vec()];
    }

    let (la, lb) = line;
    let mut pieces: Vec<Vec<InkPoint>> = Vec::new();
    let mut current_piece: Vec<InkPoint> = vec![points[0]];

    for i in 1..points.len() {
        let p0 = points[i - 1];
        let p1 = points[i];

        if let Some((t, cut_pt)) = segment_intersection(p0, p1, la, lb) {
            if t > 1e-4 && t < 1.0 - 1e-4 {
                // Perpotongan di tengah segmen p0..p1
                current_piece.push(cut_pt);
                pieces.push(std::mem::take(&mut current_piece));
                current_piece.push(cut_pt);
                current_piece.push(p1);
            } else if t >= 1.0 - 1e-4 {
                // Perpotongan tepat di p1
                current_piece.push(p1);
                pieces.push(std::mem::take(&mut current_piece));
                current_piece.push(p1);
            } else {
                // Perpotongan tepat di p0
                if current_piece.len() > 1 {
                    pieces.push(std::mem::take(&mut current_piece));
                    current_piece.push(p0);
                }
                current_piece.push(p1);
            }
        } else {
            current_piece.push(p1);
        }
    }

    if !current_piece.is_empty() {
        pieces.push(current_piece);
    }

    // Filter pecahan yang memiliki setidaknya 2 titik bila memungkinkan
    if pieces.len() > 1 {
        pieces.retain(|p| p.len() >= 2);
    }

    pieces
}

/// Menghitung titik potong antara segmen `p0..p1` dan segmen garis `la..lb`.
/// Mengembalikan parameter `t` pada segmen `p0..p1` (0..=1) dan titik `InkPoint` terinterpolasi.
fn segment_intersection(
    p0: InkPoint,
    p1: InkPoint,
    la: Vec2,
    lb: Vec2,
) -> Option<(f32, InkPoint)> {
    let p = p0.pos();
    let r = p1.pos() - p;
    let q = la;
    let s = lb - q;

    let cross_rs = cross_2d(r, s);
    if cross_rs.abs() < 1e-6 {
        return None;
    }

    let qp = q - p;
    let t = cross_2d(qp, s) / cross_rs;
    let u = cross_2d(qp, r) / cross_rs;

    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        let cut_pos = p + r * t;
        let pressure = p0.pressure + (p1.pressure - p0.pressure) * t;
        let tilt = p0.tilt + (p1.tilt - p0.tilt) * t;
        let dt_ms = (p1.t_ms.saturating_sub(p0.t_ms)) as f32 * t;
        let t_ms = p0.t_ms + dt_ms.round() as u32;

        Some((t, InkPoint::new(cut_pos.x, cut_pos.y, pressure, tilt, t_ms)))
    } else {
        None
    }
}

fn cross_2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}
