//! Modul Deteksi Tabrakan & Interferensi Fisik (Clash & Interference Detection) untuk DuCAD.
//!
//! Menguji tabrakan fisik otomatis antar bodi solid 3D menggunakan:
//! - **Broad-Phase Filtering**: Uji perpotongan Bounding Box AABB 3D yang cepat.
//! - **Narrow-Phase Exact B-Rep Boolean**: Operasi Boolean Intersect (`BRepAlgoAPI_Common`)
//!   presisi tinggi untuk mengekstrak bentuk solid tumpang tindih (*clash volume*).
//! - **Kalkulasi Volume & Centroid**: Menghitung volume interferensi eksak ($\text{mm}^3$)
//!   dan titik pusat benturan menggunakan teorema divergensi polihedron tertutup.

use anyhow::Result;

use crate::csg::intersect;
use crate::mesh::KernelMesh;
use crate::shape::KernelShape;

/// Rincian informasi satu benturan fisik (Interference Clash) antar dua bodi solid.
#[derive(Debug, Clone)]
pub struct BodyClash {
    /// ID unik untuk benturan ini dalam sesi deteksi.
    pub id: u32,
    /// ID Body A.
    pub body_a_id: u64,
    /// ID Body B.
    pub body_b_id: u64,
    /// Nama Body A.
    pub body_a_name: String,
    /// Nama Body B.
    pub body_b_name: String,
    /// Volume tabrakan / interferensi dalam satuan $\text{mm}^3$.
    pub volume: f64,
    /// Titik pusat tabrakan (Centroid X, Y, Z) dalam mm.
    pub center: [f64; 3],
    /// Batas minimum Bounding Box tabrakan [X, Y, Z].
    pub bbox_min: [f64; 3],
    /// Batas maksimum Bounding Box tabrakan [X, Y, Z].
    pub bbox_max: [f64; 3],
    /// Tessellated mesh dari volume tabrakan untuk visualisasi di 3D Viewport.
    pub clash_mesh: KernelMesh,
}

/// Hitung volume eksak dari mesh segitiga tertutup (watertight triangulated polyhedron)
/// menggunakan Teorema Divergensi / penjumlahan volume bertanda tetrahedron berakar di titik asal.
///
/// $$V = \frac{1}{6} \left| \sum_{i=0}^{N-1} \mathbf{v}_{i,0} \cdot (\mathbf{v}_{i,1} \times \mathbf{v}_{i,2}) \right|$$
pub fn compute_mesh_volume(mesh: &KernelMesh) -> f64 {
    if mesh.indices.len() < 3 || !mesh.indices.len().is_multiple_of(3) {
        return 0.0;
    }

    let mut total_signed_vol = 0.0f64;
    let n_triangles = mesh.indices.len() / 3;

    for t in 0..n_triangles {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;

        if i0 >= mesh.positions.len() || i1 >= mesh.positions.len() || i2 >= mesh.positions.len() {
            continue;
        }

        let p0 = mesh.positions[i0];
        let p1 = mesh.positions[i1];
        let p2 = mesh.positions[i2];

        let v0 = [p0[0] as f64, p0[1] as f64, p0[2] as f64];
        let v1 = [p1[0] as f64, p1[1] as f64, p1[2] as f64];
        let v2 = [p2[0] as f64, p2[1] as f64, p2[2] as f64];

        // Cross product v1 x v2
        let cross_x = v1[1] * v2[2] - v1[2] * v2[1];
        let cross_y = v1[2] * v2[0] - v1[0] * v2[2];
        let cross_z = v1[0] * v2[1] - v1[1] * v2[0];

        // Dot product v0 . (v1 x v2)
        let det = v0[0] * cross_x + v0[1] * cross_y + v0[2] * cross_z;
        total_signed_vol += det;
    }

    (total_signed_vol / 6.0).abs()
}

/// Hitung titik pusat gravitasi / centroid volume dari mesh segitiga tertutup.
pub fn compute_mesh_centroid(mesh: &KernelMesh) -> [f64; 3] {
    if mesh.indices.len() < 3 || !mesh.indices.len().is_multiple_of(3) {
        return [0.0, 0.0, 0.0];
    }

    let mut total_signed_vol = 0.0f64;
    let mut weighted_center = [0.0f64; 3];
    let n_triangles = mesh.indices.len() / 3;

    for t in 0..n_triangles {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;

        if i0 >= mesh.positions.len() || i1 >= mesh.positions.len() || i2 >= mesh.positions.len() {
            continue;
        }

        let p0 = mesh.positions[i0];
        let p1 = mesh.positions[i1];
        let p2 = mesh.positions[i2];

        let v0 = [p0[0] as f64, p0[1] as f64, p0[2] as f64];
        let v1 = [p1[0] as f64, p1[1] as f64, p1[2] as f64];
        let v2 = [p2[0] as f64, p2[1] as f64, p2[2] as f64];

        let cross_x = v1[1] * v2[2] - v1[2] * v2[1];
        let cross_y = v1[2] * v2[0] - v1[0] * v2[2];
        let cross_z = v1[0] * v2[1] - v1[1] * v2[0];

        let det = v0[0] * cross_x + v0[1] * cross_y + v0[2] * cross_z;
        let signed_vol = det / 6.0;
        total_signed_vol += signed_vol;

        // Centroid tetrahedron terhadap titik asal: (v0 + v1 + v2) / 4
        let tet_cx = (v0[0] + v1[0] + v2[0]) * 0.25;
        let tet_cy = (v0[1] + v1[1] + v2[1]) * 0.25;
        let tet_cz = (v0[2] + v1[2] + v2[2]) * 0.25;

        weighted_center[0] += tet_cx * signed_vol;
        weighted_center[1] += tet_cy * signed_vol;
        weighted_center[2] += tet_cz * signed_vol;
    }

    if total_signed_vol.abs() > 1e-9 {
        [
            weighted_center[0] / total_signed_vol,
            weighted_center[1] / total_signed_vol,
            weighted_center[2] / total_signed_vol,
        ]
    } else {
        // Fallback jika volume 0: gunakan rata-rata posisi bounding box
        if let Some((min, max)) = mesh.bounding_box() {
            [
                (min[0] + max[0]) as f64 * 0.5,
                (min[1] + max[1]) as f64 * 0.5,
                (min[2] + max[2]) as f64 * 0.5,
            ]
        } else {
            [0.0, 0.0, 0.0]
        }
    }
}

/// Uji cepat apakah dua Bounding Box AABB 3D saling beririsan (dengan toleransi kelonggaran).
pub fn aabb_intersects(
    min_a: [f32; 3],
    max_a: [f32; 3],
    min_b: [f32; 3],
    max_b: [f32; 3],
    tolerance: f32,
) -> bool {
    let overlap_x = (min_a[0] - tolerance) <= (max_b[0] + tolerance)
        && (max_a[0] + tolerance) >= (min_b[0] - tolerance);
    let overlap_y = (min_a[1] - tolerance) <= (max_b[1] + tolerance)
        && (max_a[1] + tolerance) >= (min_b[1] - tolerance);
    let overlap_z = (min_a[2] - tolerance) <= (max_b[2] + tolerance)
        && (max_a[2] + tolerance) >= (min_b[2] - tolerance);

    overlap_x && overlap_y && overlap_z
}

/// Simpul pohon AABB (BVH) di atas segitiga sebuah mesh.
struct BvhNode {
    min: [f32; 3],
    max: [f32; 3],
    /// Rentang indeks segitiga pada daftar terurut; kosong untuk simpul dalam.
    tri_range: Option<(usize, usize)>,
    children: Option<(usize, usize)>,
}

/// Pohon AABB sederhana (pembelahan median pada sumbu terpanjang) di atas
/// segitiga satu mesh.
///
/// Dipakai mid-phase deteksi tabrakan: menguji tiap pasang segitiga secara
/// langsung berbiaya O(Ta x Tb) dan jadi lebih mahal daripada operasi
/// boolean yang hendak dihindari.
struct TriBvh {
    nodes: Vec<BvhNode>,
    /// Segitiga dalam urutan pohon: tiap entri tiga titik.
    tris: Vec<[[f32; 3]; 3]>,
}

fn tri_bounds(t: &[[f32; 3]; 3]) -> ([f32; 3], [f32; 3]) {
    let mut min = t[0];
    let mut max = t[0];
    for p in &t[1..] {
        for k in 0..3 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    (min, max)
}

impl TriBvh {
    fn build(mesh: &KernelMesh) -> Option<Self> {
        let mut tris: Vec<[[f32; 3]; 3]> = Vec::with_capacity(mesh.indices.len() / 3);
        for chunk in mesh.indices.chunks_exact(3) {
            let (a, b, c) = (
                *mesh.positions.get(chunk[0] as usize)?,
                *mesh.positions.get(chunk[1] as usize)?,
                *mesh.positions.get(chunk[2] as usize)?,
            );
            tris.push([a, b, c]);
        }
        if tris.is_empty() {
            return None;
        }
        let mut bvh = TriBvh {
            nodes: Vec::new(),
            tris,
        };
        let n = bvh.tris.len();
        bvh.build_node(0, n);
        Some(bvh)
    }

    fn build_node(&mut self, start: usize, end: usize) -> usize {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for t in &self.tris[start..end] {
            let (tmin, tmax) = tri_bounds(t);
            for k in 0..3 {
                min[k] = min[k].min(tmin[k]);
                max[k] = max[k].max(tmax[k]);
            }
        }
        let idx = self.nodes.len();
        self.nodes.push(BvhNode {
            min,
            max,
            tri_range: Some((start, end)),
            children: None,
        });

        const LEAF_TRIS: usize = 8;
        if end - start <= LEAF_TRIS {
            return idx;
        }

        // Belah pada sumbu terpanjang, di median centroid.
        let axis = {
            let ext = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
            if ext[0] >= ext[1] && ext[0] >= ext[2] {
                0
            } else if ext[1] >= ext[2] {
                1
            } else {
                2
            }
        };
        let mid = (start + end) / 2;
        self.tris[start..end].select_nth_unstable_by(mid - start, |a, b| {
            let ca = (a[0][axis] + a[1][axis] + a[2][axis]) / 3.0;
            let cb = (b[0][axis] + b[1][axis] + b[2][axis]) / 3.0;
            ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
        });

        let left = self.build_node(start, mid);
        let right = self.build_node(mid, end);
        self.nodes[idx].tri_range = None;
        self.nodes[idx].children = Some((left, right));
        idx
    }

    /// Apakah ada sepasang segitiga dari kedua pohon yang benar-benar
    /// beririsan.
    fn intersects(&self, other: &TriBvh) -> bool {
        if self.nodes.is_empty() || other.nodes.is_empty() {
            return false;
        }
        let mut stack = vec![(0usize, 0usize)];
        while let Some((ia, ib)) = stack.pop() {
            let (na, nb) = (&self.nodes[ia], &other.nodes[ib]);
            if !aabb_intersects(na.min, na.max, nb.min, nb.max, 1e-4) {
                continue;
            }
            match (na.children, nb.children) {
                (None, None) => {
                    let (sa, ea) = na.tri_range.unwrap_or((0, 0));
                    let (sb, eb) = nb.tri_range.unwrap_or((0, 0));
                    for ta in &self.tris[sa..ea] {
                        for tb in &other.tris[sb..eb] {
                            if triangles_intersect(ta, tb) {
                                return true;
                            }
                        }
                    }
                }
                (Some((l, r)), None) => {
                    stack.push((l, ib));
                    stack.push((r, ib));
                }
                (None, Some((l, r))) => {
                    stack.push((ia, l));
                    stack.push((ia, r));
                }
                (Some((la, ra)), Some((lb, rb))) => {
                    stack.push((la, lb));
                    stack.push((la, rb));
                    stack.push((ra, lb));
                    stack.push((ra, rb));
                }
            }
        }
        false
    }
}

/// Uji irisan dua segitiga 3D lewat teorema sumbu pemisah (SAT).
///
/// Sumbu yang diperiksa: kedua normal bidang, ditambah 9 hasil cross antar
/// pasangan rusuk. Bila ADA satu sumbu yang memisahkan, kedua segitiga pasti
/// tidak beririsan.
fn triangles_intersect(a: &[[f32; 3]; 3], b: &[[f32; 3]; 3]) -> bool {
    let to_v = |p: [f32; 3]| [p[0] as f64, p[1] as f64, p[2] as f64];
    let a = [to_v(a[0]), to_v(a[1]), to_v(a[2])];
    let b = [to_v(b[0]), to_v(b[1]), to_v(b[2])];

    let sub = |p: [f64; 3], q: [f64; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    let cross = |p: [f64; 3], q: [f64; 3]| {
        [
            p[1] * q[2] - p[2] * q[1],
            p[2] * q[0] - p[0] * q[2],
            p[0] * q[1] - p[1] * q[0],
        ]
    };
    let dot = |p: [f64; 3], q: [f64; 3]| p[0] * q[0] + p[1] * q[1] + p[2] * q[2];

    let ea = [sub(a[1], a[0]), sub(a[2], a[1]), sub(a[0], a[2])];
    let eb = [sub(b[1], b[0]), sub(b[2], b[1]), sub(b[0], b[2])];

    let mut axes: Vec<[f64; 3]> = Vec::with_capacity(11);
    axes.push(cross(ea[0], ea[1]));
    axes.push(cross(eb[0], eb[1]));
    for x in &ea {
        for y in &eb {
            axes.push(cross(*x, *y));
        }
    }

    for axis in axes {
        let len2 = dot(axis, axis);
        // Sumbu degenerate (rusuk sejajar / segitiga pipih) tidak memberi
        // informasi pemisahan dan HARUS dilewati — bukan dianggap memisahkan.
        if len2 < 1e-18 {
            continue;
        }
        let proj = |t: &[[f64; 3]; 3]| {
            let v: [f64; 3] = [dot(t[0], axis), dot(t[1], axis), dot(t[2], axis)];
            (v[0].min(v[1]).min(v[2]), v[0].max(v[1]).max(v[2]))
        };
        let (amin, amax) = proj(&a);
        let (bmin, bmax) = proj(&b);
        // Toleransi relatif terhadap panjang sumbu supaya kontak tangensial
        // tidak dihitung sebagai tembusan.
        let eps = 1e-9 * len2.sqrt();
        if amax < bmin - eps || bmax < amin - eps {
            return false;
        }
    }
    true
}

/// Apakah kotak `inner` sepenuhnya berada di dalam `outer`.
fn aabb_contains(outer: ([f32; 3], [f32; 3]), inner: ([f32; 3], [f32; 3])) -> bool {
    (0..3).all(|k| inner.0[k] >= outer.0[k] && inner.1[k] <= outer.1[k])
}

/// Uji tabrakan fisik otomatis antar bodi solid (Clash & Interference Detection).
///
/// - `bodies`: Daftar bodi yang akan diuji dalam format `(body_id, name, KernelShape)`.
/// - `tolerance_mm3`: Ambang batas volume tabrakan minimum dalam $\text{mm}^3$ (default e.g. 0.001)
///   untuk mengabaikan kontak tangensial/face-touching ideal yang tidak saling menembus.
pub fn detect_interference(
    bodies: &[(u64, String, &KernelShape)],
    tolerance_mm3: f64,
) -> Vec<BodyClash> {
    if bodies.len() < 2 {
        return Vec::new();
    }

    let mut clashes = Vec::new();
    let mut next_clash_id = 1u32;

    type CachedBodyInfo<'a> = (
        u64,
        &'a String,
        &'a KernelShape,
        Option<([f32; 3], [f32; 3])>,
        Option<TriBvh>,
    );

    // Cache mesh, bounding box, dan pohon segitiga untuk broad + mid-phase.
    let cached_info: Vec<CachedBodyInfo> = bodies
        .iter()
        .map(|(id, name, shape)| {
            let mesh = shape.tessellate();
            let bbox = mesh.bounding_box();
            let bvh = TriBvh::build(&mesh);
            (*id, name, *shape, bbox, bvh)
        })
        .collect();

    // Evaluasi seluruh kombinasi pasangan unik (i, j) dengan i < j
    for i in 0..cached_info.len() {
        for j in (i + 1)..cached_info.len() {
            let (id_a, name_a, shape_a, bbox_a, bvh_a) = &cached_info[i];
            let (id_b, name_b, shape_b, bbox_b, bvh_b) = &cached_info[j];

            // 1. Broad-Phase AABB rejection test
            if let (Some((min_a, max_a)), Some((min_b, max_b))) = (bbox_a, bbox_b) {
                if !aabb_intersects(*min_a, *max_a, *min_b, *max_b, 0.01) {
                    // Bounding box sama sekali tidak beririsan, lewati Boolean yang mahal
                    continue;
                }
            }

            // 2. Mid-Phase: pasangan yang bounding box-nya tumpang tindih
            //    seringkali tetap tidak bersentuhan (dua profil L yang
            //    bersilangan tanpa menyentuh, misalnya). Sebelumnya pasangan
            //    seperti itu tetap membayar operasi boolean B-rep penuh.
            //
            //    PENTING — penjagaan containment: tidak adanya irisan
            //    SEGITIGA belum berarti tidak ada tabrakan. Bodi yang
            //    SEPENUHNYA berada di dalam bodi lain tidak punya satu pun
            //    segitiga yang beririsan, padahal itu interferensi total.
            //    Karena containment penuh PASTI menyiratkan containment
            //    bounding box, pasangan seperti itu diteruskan ke
            //    narrow-phase alih-alih ditolak.
            if let (Some(ba), Some(bb), Some(box_a), Some(box_b)) = (bvh_a, bvh_b, bbox_a, bbox_b) {
                let nested = aabb_contains(*box_a, *box_b) || aabb_contains(*box_b, *box_a);
                if !nested && !ba.intersects(bb) {
                    continue;
                }
            }

            // 3. Narrow-Phase Exact B-Rep Boolean Intersect
            if let Ok(clash_shape) = intersect(shape_a, shape_b) {
                let clash_mesh = clash_shape.tessellate();
                let vol = compute_mesh_volume(&clash_mesh);

                if vol >= tolerance_mm3 && clash_mesh.triangle_count() > 0 {
                    let center = compute_mesh_centroid(&clash_mesh);
                    let (bbox_min, bbox_max) = clash_mesh
                        .bounding_box()
                        .map(|(min, max)| {
                            (
                                [min[0] as f64, min[1] as f64, min[2] as f64],
                                [max[0] as f64, max[1] as f64, max[2] as f64],
                            )
                        })
                        .unwrap_or(([0.0; 3], [0.0; 3]));

                    clashes.push(BodyClash {
                        id: next_clash_id,
                        body_a_id: *id_a,
                        body_b_id: *id_b,
                        body_a_name: (*name_a).clone(),
                        body_b_name: (*name_b).clone(),
                        volume: vol,
                        center,
                        bbox_min,
                        bbox_max,
                        clash_mesh,
                    });
                    next_clash_id += 1;
                }
            }
        }
    }

    clashes
}

/// Hitung bentuk solid hasil tabrakan langsung antara dua shape (jika ada benturan).
pub fn compute_pair_interference(
    shape_a: &KernelShape,
    shape_b: &KernelShape,
    tolerance_mm3: f64,
) -> Result<Option<(KernelShape, f64)>> {
    let clash_shape = match intersect(shape_a, shape_b) {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };

    let mesh = clash_shape.tessellate();
    let vol = compute_mesh_volume(&mesh);
    if vol >= tolerance_mm3 && mesh.triangle_count() > 0 {
        Ok(Some((clash_shape, vol)))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::extrude_profile;
    use crate::profile::{Profile, ProfileSegment};

    fn make_box(width: f64, height: f64, depth: f64) -> KernelShape {
        let profile = Profile::Loop(vec![
            ProfileSegment::Line {
                start: (0.0, 0.0),
                end: (width, 0.0),
            },
            ProfileSegment::Line {
                start: (width, 0.0),
                end: (width, height),
            },
            ProfileSegment::Line {
                start: (width, height),
                end: (0.0, height),
            },
            ProfileSegment::Line {
                start: (0.0, height),
                end: (0.0, 0.0),
            },
        ]);
        extrude_profile(&profile, depth).expect("make_box failed")
    }

    #[test]
    fn test_compute_mesh_volume_box() {
        // Balok 10 x 20 x 30 = 6000 mm³
        let shape = make_box(10.0, 20.0, 30.0);
        let mesh = shape.tessellate();
        let vol = compute_mesh_volume(&mesh);
        assert!(
            (vol - 6000.0).abs() < 10.0,
            "Volume balok harus ~6000 mm³, didapat: {}",
            vol
        );
    }

    #[test]
    fn test_detect_interference_overlapping_boxes() {
        // Dua kubus 10 x 10 x 10 yang saling beririsan
        // Box A di origin: (0..10, 0..10, 0..10)
        let shape_a = make_box(10.0, 10.0, 10.0);

        // Box B digeser sehingga beririsan di (5..10, 0..10, 0..10) -> Volume irisan = 5 x 10 x 10 = 500 mm³
        let shape_b_orig = make_box(10.0, 10.0, 10.0);
        let shape_b = crate::shape::translate_shape(&shape_b_orig, 5.0, 0.0, 0.0).unwrap();

        let bodies = vec![
            (101u64, "Body A".to_string(), &shape_a),
            (102u64, "Body B".to_string(), &shape_b),
        ];

        let clashes = detect_interference(&bodies, 0.01);
        assert_eq!(clashes.len(), 1, "Harus mendeteksi tepat 1 benturan");
        let clash = &clashes[0];
        assert_eq!(clash.body_a_id, 101);
        assert_eq!(clash.body_b_id, 102);
        assert!(
            (clash.volume - 500.0).abs() < 10.0,
            "Volume benturan harus ~500 mm³, didapat: {}",
            clash.volume
        );
    }

    #[test]
    fn test_detect_interference_disjoint_boxes() {
        // Dua kubus yang terpisah jauh (tanpa kontak)
        let shape_a = make_box(10.0, 10.0, 10.0);
        let shape_b_orig = make_box(10.0, 10.0, 10.0);
        let shape_b = crate::shape::translate_shape(&shape_b_orig, 50.0, 50.0, 0.0).unwrap();

        let bodies = vec![
            (1u64, "Part 1".to_string(), &shape_a),
            (2u64, "Part 2".to_string(), &shape_b),
        ];

        let clashes = detect_interference(&bodies, 0.001);
        assert_eq!(clashes.len(), 0, "Bodi terpisah tidak boleh memiliki benturan");
    }
}
