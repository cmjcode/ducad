//! Geometri dan kasus benchmark analitik (batang tarik, kantilever, pelat
//! bertekanan, pelat berlubang). Dipakai tes gate P17; engine boleh
//! memakainya untuk tes ujung-ke-ujung tanpa kernel.

pub mod advanced;
pub mod cases;

use crate::SurfaceMesh;

/// Tag face kotak: −X, +X, −Y, +Y, −Z, +Z.
pub const FACE_XMIN: u32 = 0;
pub const FACE_XMAX: u32 = 1;
pub const FACE_YMIN: u32 = 2;
pub const FACE_YMAX: u32 = 3;
pub const FACE_ZMIN: u32 = 4;
pub const FACE_ZMAX: u32 = 5;
/// Tag dinding lubang pada [`plate_with_hole`].
pub const FACE_HOLE: u32 = 6;

/// Kotak `[0, size]` dengan 12 segitiga berorientasi keluar.
pub fn box_surface(size: [f64; 3]) -> SurfaceMesh {
    box_surface_at([0.0; 3], size)
}

/// Kotak dengan sudut minimum `min`.
pub fn box_surface_at(min: [f64; 3], size: [f64; 3]) -> SurfaceMesh {
    let positions = (0..8usize)
        .map(|n| {
            [
                min[0] + (n & 1) as f64 * size[0],
                min[1] + ((n >> 1) & 1) as f64 * size[1],
                min[2] + (n >> 2) as f64 * size[2],
            ]
        })
        .collect();
    let triangles = vec![
        [0, 4, 6],
        [0, 6, 2],
        [1, 3, 7],
        [1, 7, 5],
        [0, 1, 5],
        [0, 5, 4],
        [2, 6, 7],
        [2, 7, 3],
        [0, 2, 3],
        [0, 3, 1],
        [4, 5, 7],
        [4, 7, 6],
    ];
    let tri_face = vec![
        FACE_XMIN, FACE_XMIN, FACE_XMAX, FACE_XMAX, FACE_YMIN, FACE_YMIN, FACE_YMAX, FACE_YMAX,
        FACE_ZMIN, FACE_ZMIN, FACE_ZMAX, FACE_ZMAX,
    ];
    SurfaceMesh {
        positions,
        triangles,
        tri_face,
    }
}

/// Pelat `[0, size]` dengan lubang silinder tembus (sumbu Z) di tengah.
/// Lubang didekati poligon `segments` sisi. Tag: enam sisi kotak + [`FACE_HOLE`].
pub fn plate_with_hole(size: [f64; 3], hole_diameter: f64, segments: usize) -> SurfaceMesh {
    let segments = segments.max(8);
    let r = hole_diameter / 2.0;
    let (cx, cy) = (size[0] / 2.0, size[1] / 2.0);
    let (hx, hy) = (size[0] / 2.0, size[1] / 2.0);
    // Sudut seragam + sudut pojok persegi agar pojok terwakili tepat.
    let mut angles: Vec<f64> = (0..segments)
        .map(|i| i as f64 / segments as f64 * std::f64::consts::TAU)
        .collect();
    let corner = hy.atan2(hx);
    for a in [
        corner,
        std::f64::consts::PI - corner,
        std::f64::consts::PI + corner,
        std::f64::consts::TAU - corner,
    ] {
        if angles.iter().all(|&b| (a - b).abs() > 1e-9) {
            angles.push(a);
        }
    }
    angles.sort_by(f64::total_cmp);
    let n = angles.len();
    // Urutan vertex: [lingkaran bawah, tepi bawah, lingkaran atas, tepi atas].
    let mut positions = Vec::with_capacity(4 * n + 8);
    for z in [0.0, size[2]] {
        for &a in &angles {
            positions.push([cx + r * a.cos(), cy + r * a.sin(), z]);
        }
        for &a in &angles {
            let (c, s) = (a.cos(), a.sin());
            let t = (hx / c.abs().max(1e-300)).min(hy / s.abs().max(1e-300));
            positions.push([
                (cx + t * c).clamp(0.0, size[0]),
                (cy + t * s).clamp(0.0, size[1]),
                z,
            ]);
        }
    }
    let cb = |i: usize| (i % n) as u32;
    let rb = |i: usize| (n + i % n) as u32;
    let ct = |i: usize| (2 * n + i % n) as u32;
    let rt = |i: usize| (3 * n + i % n) as u32;
    let mut triangles = Vec::new();
    let mut tri_face = Vec::new();
    for i in 0..n {
        // Atas (+Z): berlawanan jarum jam dilihat dari atas.
        triangles.push([ct(i), rt(i), rt(i + 1)]);
        triangles.push([ct(i), rt(i + 1), ct(i + 1)]);
        tri_face.extend([FACE_ZMAX, FACE_ZMAX]);
        // Bawah (−Z): orientasi dibalik.
        triangles.push([cb(i), rb(i + 1), rb(i)]);
        triangles.push([cb(i), cb(i + 1), rb(i + 1)]);
        tri_face.extend([FACE_ZMIN, FACE_ZMIN]);
        // Dinding lubang: normal keluar body mengarah ke sumbu lubang.
        triangles.push([cb(i), ct(i), ct(i + 1)]);
        triangles.push([cb(i), ct(i + 1), cb(i + 1)]);
        tri_face.extend([FACE_HOLE, FACE_HOLE]);
    }
    // Empat sisi luar memakai vertex sudut kotak tersendiri.
    let base = positions.len() as u32;
    let outer = box_surface(size);
    positions.extend(outer.positions.iter().copied());
    for (tri, &face) in outer.triangles.iter().zip(&outer.tri_face) {
        if face <= FACE_YMAX {
            triangles.push([tri[0] + base, tri[1] + base, tri[2] + base]);
            tri_face.push(face);
        }
    }
    SurfaceMesh {
        positions,
        triangles,
        tri_face,
    }
}

/// Profil poligon (berlawanan jarum jam di bidang XZ) diekstrusi sepanjang +Y
/// setebal `depth`. Tag: sisi ke-`i` (dari titik `i` ke `i+1`) = `i`, tutup
/// `y = 0` = `n`, tutup `y = depth` = `n + 1`. Tutup ditriangulasi dengan
/// pemotongan telinga sehingga profil cekung (mis. L) didukung.
pub fn extrude_profile(profile: &[[f64; 2]], depth: f64) -> SurfaceMesh {
    let n = profile.len();
    let mut positions = Vec::with_capacity(2 * n);
    for y in [0.0, depth] {
        for p in profile {
            positions.push([p[0], y, p[1]]);
        }
    }
    let mut triangles = Vec::new();
    let mut tri_face = Vec::new();
    for i in 0..n {
        let j = (i + 1) % n;
        let (a, b, c, d) = (i as u32, j as u32, (n + j) as u32, (n + i) as u32);
        // Profil CCW di XZ dilihat dari −Y; normal sisi mengarah keluar profil.
        triangles.push([a, c, b]);
        triangles.push([a, d, c]);
        tri_face.extend([i as u32, i as u32]);
    }
    // Pemotongan telinga pada profil.
    let cross2 = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut ring: Vec<usize> = (0..n).collect();
    let mut ears: Vec<[usize; 3]> = Vec::new();
    let mut guard = 0;
    while ring.len() > 3 && guard < 10 * n {
        guard += 1;
        let m = ring.len();
        let mut clipped = false;
        for k in 0..m {
            let (ia, ib, ic) = (ring[(k + m - 1) % m], ring[k], ring[(k + 1) % m]);
            let (a, b, c) = (profile[ia], profile[ib], profile[ic]);
            if cross2(a, b, c) <= 0.0 {
                continue;
            }
            let blocked = ring.iter().any(|&o| {
                o != ia && o != ib && o != ic && {
                    let p = profile[o];
                    cross2(a, b, p) >= 0.0 && cross2(b, c, p) >= 0.0 && cross2(c, a, p) >= 0.0
                }
            });
            if !blocked {
                ears.push([ia, ib, ic]);
                ring.remove(k);
                clipped = true;
                break;
            }
        }
        if !clipped {
            break;
        }
    }
    if ring.len() == 3 {
        ears.push([ring[0], ring[1], ring[2]]);
    }
    for e in ears {
        // Tutup y = 0: normal −Y; tutup y = depth: normal +Y.
        triangles.push([e[0] as u32, e[1] as u32, e[2] as u32]);
        tri_face.push(n as u32);
        triangles.push([(n + e[0]) as u32, (n + e[2]) as u32, (n + e[1]) as u32]);
        tri_face.push(n as u32 + 1);
    }
    SurfaceMesh {
        positions,
        triangles,
        tri_face,
    }
}

/// Braket L: kaki sepanjang X (`leg_x`) dan Z (`leg_z`), tebal `thickness`,
/// lebar `width` sepanjang Y. Tag sisi mengikuti [`extrude_profile`]:
/// 0 = bawah (z = 0), 1 = ujung kaki X, 2 = atas kaki X, 3 = dalam kaki Z,
/// 4 = ujung kaki Z, 5 = punggung (x = 0), 6 = y = 0, 7 = y = width.
pub fn l_bracket(leg_x: f64, leg_z: f64, thickness: f64, width: f64) -> SurfaceMesh {
    extrude_profile(
        &[
            [0.0, 0.0],
            [leg_x, 0.0],
            [leg_x, thickness],
            [thickness, thickness],
            [thickness, leg_z],
            [0.0, leg_z],
        ],
        width,
    )
}

/// Tag silinder dan cincin.
pub const FACE_SIDE: u32 = 0;
pub const FACE_BOTTOM: u32 = 1;
pub const FACE_TOP: u32 = 2;
/// Dinding dalam [`thick_ring`].
pub const FACE_BORE: u32 = 3;

/// Cincin tebal (silinder berongga) sumbu Z berpusat di titik asal XY.
/// `inner_radius = 0` menghasilkan silinder pejal. Tag: [`FACE_SIDE`] (luar),
/// [`FACE_BOTTOM`], [`FACE_TOP`], [`FACE_BORE`] (dalam).
pub fn thick_ring(
    outer_radius: f64,
    inner_radius: f64,
    height: f64,
    segments: usize,
) -> SurfaceMesh {
    let n = segments.max(8);
    let solid = inner_radius <= 0.0;
    let mut positions = Vec::new();
    for z in [0.0, height] {
        for i in 0..n {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            positions.push([outer_radius * a.cos(), outer_radius * a.sin(), z]);
        }
        for i in 0..n {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            if solid {
                positions.push([0.0, 0.0, z]);
            } else {
                positions.push([inner_radius * a.cos(), inner_radius * a.sin(), z]);
            }
        }
    }
    let ob = |i: usize| (i % n) as u32;
    let ib = |i: usize| (n + i % n) as u32;
    let ot = |i: usize| (2 * n + i % n) as u32;
    let it = |i: usize| (3 * n + i % n) as u32;
    let mut triangles = Vec::new();
    let mut tri_face = Vec::new();
    for i in 0..n {
        // Dinding luar: normal radial keluar.
        triangles.push([ob(i), ob(i + 1), ot(i + 1)]);
        triangles.push([ob(i), ot(i + 1), ot(i)]);
        tri_face.extend([FACE_SIDE, FACE_SIDE]);
        // Atas (+Z) dan bawah (−Z).
        triangles.push([it(i), ot(i), ot(i + 1)]);
        tri_face.push(FACE_TOP);
        triangles.push([ib(i), ob(i + 1), ob(i)]);
        tri_face.push(FACE_BOTTOM);
        if !solid {
            triangles.push([it(i), ot(i + 1), it(i + 1)]);
            tri_face.push(FACE_TOP);
            triangles.push([ib(i), ib(i + 1), ob(i + 1)]);
            tri_face.push(FACE_BOTTOM);
            // Dinding dalam: normal mengarah ke sumbu.
            triangles.push([ib(i), it(i), it(i + 1)]);
            triangles.push([ib(i), it(i + 1), ib(i + 1)]);
            tri_face.extend([FACE_BORE, FACE_BORE]);
        }
    }
    SurfaceMesh {
        positions,
        triangles,
        tri_face,
    }
}

/// Silinder pejal sumbu Z.
pub fn cylinder(radius: f64, height: f64, segments: usize) -> SurfaceMesh {
    thick_ring(radius, 0.0, height, segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linalg::{cross, dot, sub};

    fn face_area_vectors(mesh: &SurfaceMesh, faces: usize) -> (Vec<[f64; 3]>, f64) {
        let mut area = vec![[0.0; 3]; faces];
        let mut vol6 = 0.0;
        for (tri, &f) in mesh.triangles.iter().zip(&mesh.tri_face) {
            let [a, b, c] = [
                mesh.positions[tri[0] as usize],
                mesh.positions[tri[1] as usize],
                mesh.positions[tri[2] as usize],
            ];
            let n = cross(sub(b, a), sub(c, a));
            for k in 0..3 {
                area[f as usize][k] += 0.5 * n[k];
            }
            vol6 += dot(a, cross(b, c));
        }
        (area, vol6 / 6.0)
    }

    #[test]
    fn benchmark_box_is_closed_and_outward() {
        let mesh = box_surface([2.0, 3.0, 4.0]);
        let (area, volume) = face_area_vectors(&mesh, 6);
        assert!((volume - 24.0).abs() < 1e-12);
        let expect = [
            [-12.0, 0.0, 0.0],
            [12.0, 0.0, 0.0],
            [0.0, -8.0, 0.0],
            [0.0, 8.0, 0.0],
            [0.0, 0.0, -6.0],
            [0.0, 0.0, 6.0],
        ];
        for f in 0..6 {
            for k in 0..3 {
                assert!((area[f][k] - expect[f][k]).abs() < 1e-12, "face {f}");
            }
        }
    }

    #[test]
    fn benchmark_plate_with_hole_is_closed_and_outward() {
        let size = [60.0, 40.0, 5.0];
        let mesh = plate_with_hole(size, 10.0, 128);
        let (area, volume) = face_area_vectors(&mesh, 7);
        let exact = (60.0 * 40.0 - std::f64::consts::PI * 25.0) * 5.0;
        assert!((volume - exact).abs() / exact < 1e-3, "{volume} vs {exact}");
        let hole_area = 60.0 * 40.0 - exact / 5.0;
        assert!((area[FACE_ZMAX as usize][2] - (2400.0 - hole_area)).abs() < 0.1);
        assert!((area[FACE_ZMIN as usize][2] + (2400.0 - hole_area)).abs() < 0.1);
        assert!((area[FACE_XMAX as usize][0] - 200.0).abs() < 1e-9);
        // Permukaan tertutup: jumlah semua vektor luas nol.
        for k in 0..3 {
            let total: f64 = area.iter().map(|a| a[k]).sum();
            assert!(total.abs() < 1e-9);
        }
    }
    #[test]
    fn benchmark_extra_fixtures_are_closed_and_outward() {
        let bracket = l_bracket(40.0, 30.0, 5.0, 20.0);
        let (area, volume) = face_area_vectors(&bracket, 8);
        assert!(
            (volume - (40.0 * 5.0 + 25.0 * 5.0) * 20.0).abs() < 1e-9,
            "{volume}"
        );
        assert!((area[0][2] + 800.0).abs() < 1e-9 && (area[5][0] + 600.0).abs() < 1e-9);
        assert!((area[2][2] - 700.0).abs() < 1e-9 && (area[3][0] - 500.0).abs() < 1e-9);
        assert!((area[6][1] + 325.0).abs() < 1e-9 && (area[7][1] - 325.0).abs() < 1e-9);
        let ring = thick_ring(10.0, 6.0, 4.0, 256);
        let (area, volume) = face_area_vectors(&ring, 4);
        let exact = std::f64::consts::PI * (100.0 - 36.0) * 4.0;
        assert!((volume - exact).abs() / exact < 2e-4, "{volume} vs {exact}");
        assert!(area[FACE_TOP as usize][2] > 0.0 && area[FACE_BOTTOM as usize][2] < 0.0);
        let solid = cylinder(5.0, 12.0, 256);
        let (area, volume) = face_area_vectors(&solid, 4);
        let exact = std::f64::consts::PI * 25.0 * 12.0;
        assert!((volume - exact).abs() / exact < 2e-4);
        for k in 0..3 {
            assert!(area.iter().map(|a| a[k]).sum::<f64>().abs() < 1e-9);
        }
    }
}
