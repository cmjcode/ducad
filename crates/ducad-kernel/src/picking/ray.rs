use glam::{dvec3, DVec3};

/// Ray dunia (titik asal + arah, tidak harus dinormalisasi) dipakai untuk
/// picking edge/face 3D di viewport.
///
/// SENGAJA disimpan APA ADANYA oleh pemanggil (`ducad-app`) — BUKAN
/// index posisi di `shape.edges()`/`shape.faces()` atau handle OCCT
/// mentah. `fillet_edges`/`chamfer_edges`/`shell_hollow_faces` semua
/// harus `deep_clone` shape dulu sebelum memutasi (pola sama dengan
/// `fillet_all`/`chamfer_all`/`shell_hollow`, supaya shape asli pemanggil
/// tetap utuh untuk undo) — Face/Edge yang dipilih SEBELUM clone bukan
/// sub-shape yang valid dari shape HASIL clone, dan index posisi di
/// iterator `edges()`/`faces()` juga tidak terjamin stabil lintas
/// roundtrip STEP (belum pernah diverifikasi, jadi tidak boleh
/// diasumsikan). Ray dunia tidak kena masalah ini: `deep_clone` tidak
/// memindah/mengubah geometri di ruang dunia sama sekali, jadi ray yang
/// SAMA di-cast ULANG terhadap shape hasil clone akan selalu kena
/// permukaan/tepi yang SAMA secara geometris — robust lewat operasi
/// geometris nyata, bukan lewat asumsi index/handle yang tak teruji.
#[derive(Debug, Clone, Copy)]
pub struct PickRay {
    pub origin: (f64, f64, f64),
    pub dir: (f64, f64, f64),
}

impl PickRay {
    pub(crate) fn origin_vec(self) -> DVec3 {
        dvec3(self.origin.0, self.origin.1, self.origin.2)
    }

    pub(crate) fn dir_vec(self) -> DVec3 {
        dvec3(self.dir.0, self.dir.1, self.dir.2)
    }
}

/// Titik terdekat (ke `seg_a`/`seg_b`) di sepanjang segmen `seg_a..seg_b`
/// terhadap ray `ray_origin + t*ray_dir`, plus jaraknya ke ray. Bukan
/// solusi jarak-minimum-tersertifikasi antar 2 garis skew (itu perlu
/// clamp bersama di kedua parameter dalam loop iteratif) — pendekatan
/// dua-langkah standar yang cukup untuk hit-testing interaktif: cari `s`
/// (parameter di segmen) dari closest-point line-line lalu clamp ke
/// [0,1], baru proyeksikan balik ke ray. `opencascade-rs` tidak punya
/// primitif ray-vs-edge (beda dari `faces_along_ray` yang sudah ada untuk
/// face) — ditulis sendiri, konsisten dengan pola project (solver LM,
/// snap engine, DXF writer semua ditulis sendiri).
pub(crate) fn closest_point_ray_segment(
    ray_origin: DVec3,
    ray_dir: DVec3,
    seg_a: DVec3,
    seg_b: DVec3,
) -> (f64, DVec3) {
    let d1 = ray_dir;
    let d2 = seg_b - seg_a;
    let r = ray_origin - seg_a;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);

    if e < 1e-12 {
        // Segmen degenerate (dua titik approximation sama) — jarak ke
        // titik tunggal `seg_a`.
        let t = if a > 1e-12 {
            d1.dot(seg_a - ray_origin) / a
        } else {
            0.0
        };
        let closest_on_ray = ray_origin + d1 * t;
        return ((closest_on_ray - seg_a).length(), seg_a);
    }

    let s = if a < 1e-12 {
        (f / e).clamp(0.0, 1.0)
    } else {
        let c = d1.dot(r);
        let b = d1.dot(d2);
        let denom = a * e - b * b;
        let t_unclamped = if denom.abs() > 1e-12 {
            (b * f - c * e) / denom
        } else {
            0.0
        };
        ((b * t_unclamped + f) / e).clamp(0.0, 1.0)
    };

    let point_on_seg = seg_a + d2 * s;
    let t_ray = if a > 1e-12 {
        d1.dot(point_on_seg - ray_origin) / a
    } else {
        0.0
    };
    let closest_on_ray = ray_origin + d1 * t_ray;
    ((closest_on_ray - point_on_seg).length(), point_on_seg)
}

/// Jarak titik `point` ke garis `ray` (garis TAK TERBATAS di kedua arah,
/// bukan half-line dari `ray_origin` — sama persis dgn cabang segmen
/// degenerate di `closest_point_ray_segment` di atas, cuma dipisah jadi
/// helper sendiri karena vertex picking butuh jarak titik-ke-ray murni,
/// bukan titik-ke-segmen).
pub(crate) fn point_to_ray_distance(ray_origin: DVec3, ray_dir: DVec3, point: DVec3) -> f64 {
    let a = ray_dir.dot(ray_dir);
    let t = if a > 1e-12 {
        ray_dir.dot(point - ray_origin) / a
    } else {
        0.0
    };
    let closest_on_ray = ray_origin + ray_dir * t;
    (closest_on_ray - point).length()
}

/// Test titik 2D di dalam poligon (algoritma ray-casting/even-odd standar)
/// — dipakai [`resolve_planar_face_along_ray_fallback`] setelah proyeksi
/// titik hit 3D ke basis 2D bidang wajah.
pub(crate) fn point_in_polygon_2d(p: (f64, f64), poly: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > p.1) != (yj > p.1) && p.0 < (xj - xi) * (p.1 - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}
