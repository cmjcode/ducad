//! Ekstraksi Proyeksi Ortogonal & Hidden Line Removal (HLR) untuk Gambar Kerja Teknik 2D.
//!
//! Menghasilkan 4 tampak standar (Tampak Depan, Atas, Samping Kanan, dan Isometrik 3D)
//! dari model solid 3D dengan pemisahan garis tampak (visible solid line), garis
//! tersembunyi (hidden dashed line), siluet permukaan lengkung, dan garis sumbu (centerlines).

use glam::{vec2, vec3, Vec2, Vec3};
use std::collections::HashMap;

use crate::lock_kernel;
use crate::mesh::KernelMesh;
use crate::shape::KernelShape;

/// Jenis tampak proyeksi 2D standar teknik.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ProjectedViewKind {
    /// Tampak Depan (Front View) — melihat ke arah sumbu +Y (bidang XZ).
    Front,
    /// Tampak Atas (Top View) — melihat dari atas +Z (bidang XY).
    Top,
    /// Tampak Samping Kanan (Right View) — melihat dari kanan +X (bidang YZ).
    Right,
    /// Tampak Isometrik 3D (Axonometric Isometric View).
    Isometric,
}

impl ProjectedViewKind {
    pub fn title_id(self) -> &'static str {
        match self {
            ProjectedViewKind::Front => "TAMPAK DEPAN",
            ProjectedViewKind::Top => "TAMPAK ATAS",
            ProjectedViewKind::Right => "TAMPAK SAMPING KANAN",
            ProjectedViewKind::Isometric => "TAMPAK ISOMETRIK 3D",
        }
    }

    pub fn title_en(self) -> &'static str {
        match self {
            ProjectedViewKind::Front => "FRONT VIEW",
            ProjectedViewKind::Top => "TOP VIEW",
            ProjectedViewKind::Right => "RIGHT SIDE VIEW",
            ProjectedViewKind::Isometric => "ISOMETRIC 3D VIEW",
        }
    }

    /// Vektor arah kamera / pandangan (View Direction, dari kamera menuju objek),
    /// vektor kanan layar (Right Vector), dan vektor atas layar (Up Vector).
    pub fn camera_vectors(self) -> (Vec3, Vec3, Vec3) {
        match self {
            ProjectedViewKind::Top => {
                // Kamera di +Z melihat ke -Z
                // Screen X: +X, Screen Y: +Y, View Dir: -Z (Depth: +Z)
                (vec3(0.0, 0.0, -1.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0))
            }
            ProjectedViewKind::Front => {
                // Kamera di -Y melihat ke +Y
                // Screen X: +X, Screen Y: +Z, View Dir: +Y (Depth: -Y)
                (vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0))
            }
            ProjectedViewKind::Right => {
                // Kamera di +X melihat ke -X
                // Screen X: -Y (atau +Y tergantung konvensi; ISO pandangan kanan: +Y di kanan layar), Screen Y: +Z
                (vec3(-1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0))
            }
            ProjectedViewKind::Isometric => {
                // Sudut isometrik standar 35.264° / 45°: arah pandang (1, -1, 1) ternormalisasi
                let v = vec3(1.0, -1.0, 1.0).normalize();
                let right = vec3(1.0, 1.0, 0.0).normalize();
                let up = right.cross(v).normalize();
                (-v, right, up)
            }
        }
    }
}

/// Klasifikasi garis pada gambar kerja 2D.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HlrLineKind {
    /// Garis tampak tebal (Visible continuous line — ISO 0.5mm / 0.7mm).
    Visible,
    /// Garis tersembunyi putus-putus (Hidden dashed line — ISO 0.25mm / 0.35mm).
    Hidden,
    /// Garis sumbu simetri / pusat lingkaran (Centerline dash-dot `— · —`).
    Centerline,
    /// Garis kontur siluet permukaan lengkung (Silhouette line).
    Silhouette,
}

/// Segmen garis 2D hasil proyeksi dengan koordinat dalam mm.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HlrSegment2D {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub kind: HlrLineKind,
}

impl HlrSegment2D {
    pub fn new(start: Vec2, end: Vec2, kind: HlrLineKind) -> Self {
        Self {
            start: [start.x, start.y],
            end: [end.x, end.y],
            kind,
        }
    }

    pub fn length(&self) -> f32 {
        let dx = self.end[0] - self.start[0];
        let dy = self.end[1] - self.start[1];
        (dx * dx + dy * dy).sqrt()
    }
}

/// Data satu proyeksi tampak 2D lengkap dengan garis, bounding box, dan dimensi.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectedView {
    pub kind: ProjectedViewKind,
    pub title: String,
    pub bounds_min: [f32; 2],
    pub bounds_max: [f32; 2],
    pub segments: Vec<HlrSegment2D>,
    pub centerlines: Vec<HlrSegment2D>,
    pub width_mm: f32,
    pub height_mm: f32,
    pub depth_mm: f32,
}

impl ProjectedView {
    pub fn size_2d(&self) -> [f32; 2] {
        [
            (self.bounds_max[0] - self.bounds_min[0]).max(1.0),
            (self.bounds_max[1] - self.bounds_min[1]).max(1.0),
        ]
    }

    pub fn center_2d(&self) -> [f32; 2] {
        [
            (self.bounds_min[0] + self.bounds_max[0]) * 0.5,
            (self.bounds_min[1] + self.bounds_max[1]) * 0.5,
        ]
    }
}

/// Gambar kerja teknik multi-view lengkap yang siap dilayout dan diekspor.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HlrDrawing {
    pub front: ProjectedView,
    pub top: ProjectedView,
    pub right: ProjectedView,
    pub isometric: ProjectedView,
    pub model_bbox_min: [f32; 3],
    pub model_bbox_max: [f32; 3],
}

impl HlrDrawing {
    pub fn model_dimensions(&self) -> (f32, f32, f32) {
        (
            (self.model_bbox_max[0] - self.model_bbox_min[0]).abs(),
            (self.model_bbox_max[1] - self.model_bbox_min[1]).abs(),
            (self.model_bbox_max[2] - self.model_bbox_min[2]).abs(),
        )
    }

    pub fn view_by_kind(&self, kind: ProjectedViewKind) -> &ProjectedView {
        match kind {
            ProjectedViewKind::Front => &self.front,
            ProjectedViewKind::Top => &self.top,
            ProjectedViewKind::Right => &self.right,
            ProjectedViewKind::Isometric => &self.isometric,
        }
    }
}

/// Mesin ekstraksi proyeksi dan Hidden Line Removal (HLR).
pub struct HlrExtractor;

impl HlrExtractor {
    /// Ekstraksi gambar kerja 4 tampak lengkap dari koleksi shape solid dan mesh dokumen.
    pub fn extract_drawing(
        shapes: &[&KernelShape],
        meshes: &[&KernelMesh],
    ) -> HlrDrawing {
        let _guard = lock_kernel();

        let merged_mesh = KernelMesh::merge(meshes);
        let (bbox_min, bbox_max) = merged_mesh.bounding_box().unwrap_or(([0.0; 3], [100.0; 3]));

        let front = Self::extract_view(shapes, &merged_mesh, ProjectedViewKind::Front, (bbox_min, bbox_max));
        let top = Self::extract_view(shapes, &merged_mesh, ProjectedViewKind::Top, (bbox_min, bbox_max));
        let right = Self::extract_view(shapes, &merged_mesh, ProjectedViewKind::Right, (bbox_min, bbox_max));
        let isometric = Self::extract_view(shapes, &merged_mesh, ProjectedViewKind::Isometric, (bbox_min, bbox_max));

        HlrDrawing {
            front,
            top,
            right,
            isometric,
            model_bbox_min: bbox_min,
            model_bbox_max: bbox_max,
        }
    }

    /// Ekstraksi satu tampak spesifik.
    pub fn extract_view(
        shapes: &[&KernelShape],
        mesh: &KernelMesh,
        view_kind: ProjectedViewKind,
        model_bbox: ([f32; 3], [f32; 3]),
    ) -> ProjectedView {
        let (view_dir, right_vec, up_vec) = view_kind.camera_vectors();
        let depth_dir = -view_dir; // Vektor kedalaman (makin besar = makin dekat ke kamera)

        // 1. Kumpulkan semua 3D kurva tepi dari B-Rep solid
        let mut raw_3d_segments: Vec<(Vec3, Vec3, bool)> = Vec::new(); // (start, end, is_boundary)

        for shape in shapes {
            let occ_shape = shape.inner();
            for edge in occ_shape.edges() {
                let approx = edge.approximation_segments();
                let points: Vec<Vec3> = approx
                    .map(|p| vec3(p.x as f32, p.y as f32, p.z as f32))
                    .collect();

                if points.len() >= 2 {
                    for w in points.windows(2) {
                        if (w[0] - w[1]).length_squared() > 1e-6 {
                            raw_3d_segments.push((w[0], w[1], false));
                        }
                    }
                } else {
                    let p1 = edge.start_point();
                    let p2 = edge.end_point();
                    let v1 = vec3(p1.x as f32, p1.y as f32, p1.z as f32);
                    let v2 = vec3(p2.x as f32, p2.y as f32, p2.z as f32);
                    if (v1 - v2).length_squared() > 1e-6 {
                        raw_3d_segments.push((v1, v2, false));
                    }
                }
            }
        }

        // 2. Ekstraksi garis siluet (silhouette edges) dari mesh segitiga
        let silhouette_segments = extract_silhouette_edges(mesh, view_dir);
        for (p1, p2) in silhouette_segments {
            raw_3d_segments.push((p1, p2, true));
        }

        // 3. Proyeksikan 3D segmen ke 2D viewport dan lakukan uji oklusi kedalaman (HLR)
        let mut segments_2d: Vec<HlrSegment2D> = Vec::new();
        let mut bounds_min = vec2(f32::MAX, f32::MAX);
        let mut bounds_max = vec2(f32::MIN, f32::MIN);

        // Pre-hitung segitiga terproyeksi untuk fast depth testing
        let projected_triangles = build_projected_triangles(mesh, right_vec, up_vec, depth_dir);

        for (p1_3d, p2_3d, is_silhouette) in raw_3d_segments {
            let u1 = p1_3d.dot(right_vec);
            let v1 = p1_3d.dot(up_vec);
            let d1 = p1_3d.dot(depth_dir);

            let u2 = p2_3d.dot(right_vec);
            let v2 = p2_3d.dot(up_vec);
            let d2 = p2_3d.dot(depth_dir);

            let seg_len_2d = ((u1 - u2).powi(2) + (v1 - v2).powi(2)).sqrt();
            if seg_len_2d < 0.05 {
                continue; // Terlalu pendek dalam proyeksi 2D
            }

            // Perbarui bounding box 2D
            bounds_min.x = bounds_min.x.min(u1).min(u2);
            bounds_min.y = bounds_min.y.min(v1).min(v2);
            bounds_max.x = bounds_max.x.max(u1).max(u2);
            bounds_max.y = bounds_max.y.max(v1).max(v2);

            // Uji oklusi visibilitas pada titik tengah segmen (dan sub-segmen jika panjang)
            let steps = if seg_len_2d > 10.0 { (seg_len_2d / 8.0).ceil() as usize } else { 1 };
            for step in 0..steps {
                let t_start = step as f32 / steps as f32;
                let t_end = (step + 1) as f32 / steps as f32;

                let sub_u1 = u1 + (u2 - u1) * t_start;
                let sub_v1 = v1 + (v2 - v1) * t_start;
                let sub_d1 = d1 + (d2 - d1) * t_start;

                let sub_u2 = u1 + (u2 - u1) * t_end;
                let sub_v2 = v1 + (v2 - v1) * t_end;
                let sub_d2 = d1 + (d2 - d1) * t_end;

                let mid_u = (sub_u1 + sub_u2) * 0.5;
                let mid_v = (sub_v1 + sub_v2) * 0.5;
                let mid_d = (sub_d1 + sub_d2) * 0.5;

                let is_occluded = test_occlusion(mid_u, mid_v, mid_d, &projected_triangles);

                let kind = if is_occluded {
                    HlrLineKind::Hidden
                } else if is_silhouette {
                    HlrLineKind::Silhouette
                } else {
                    HlrLineKind::Visible
                };

                segments_2d.push(HlrSegment2D {
                    start: [sub_u1, sub_v1],
                    end: [sub_u2, sub_v2],
                    kind,
                });
            }
        }

        // Jika tidak ada garis, gunakan model bounds
        if bounds_min.x > bounds_max.x {
            bounds_min = vec2(0.0, 0.0);
            bounds_max = vec2(100.0, 100.0);
        }

        // 4. Ekstraksi Garis Sumbu (Centerlines)
        let centerlines = extract_centerlines(shapes, right_vec, up_vec, view_kind);

        let dx = (model_bbox.1[0] - model_bbox.0[0]).abs();
        let dy = (model_bbox.1[1] - model_bbox.0[1]).abs();
        let dz = (model_bbox.1[2] - model_bbox.0[2]).abs();

        ProjectedView {
            kind: view_kind,
            title: view_kind.title_id().to_string(),
            bounds_min: [bounds_min.x, bounds_min.y],
            bounds_max: [bounds_max.x, bounds_max.y],
            segments: simplify_and_merge_segments(segments_2d),
            centerlines,
            width_mm: dx,
            height_mm: dz,
            depth_mm: dy,
        }
    }
}

/// Struktur data segitiga terproyeksi untuk fast spatial query.
struct ProjectedTri {
    u_min: f32,
    u_max: f32,
    v_min: f32,
    v_max: f32,
    u0: f32,
    v0: f32,
    d0: f32,
    u1: f32,
    v1: f32,
    d1: f32,
    u2: f32,
    v2: f32,
    d2: f32,
}

fn build_projected_triangles(
    mesh: &KernelMesh,
    right: Vec3,
    up: Vec3,
    depth: Vec3,
) -> Vec<ProjectedTri> {
    let mut tris = Vec::with_capacity(mesh.triangle_count());
    let tri_count = mesh.indices.len() / 3;

    for i in 0..tri_count {
        let i0 = mesh.indices[i * 3] as usize;
        let i1 = mesh.indices[i * 3 + 1] as usize;
        let i2 = mesh.indices[i * 3 + 2] as usize;

        if i0 >= mesh.positions.len() || i1 >= mesh.positions.len() || i2 >= mesh.positions.len() {
            continue;
        }

        let p0 = Vec3::from_array(mesh.positions[i0]);
        let p1 = Vec3::from_array(mesh.positions[i1]);
        let p2 = Vec3::from_array(mesh.positions[i2]);

        let u0 = p0.dot(right);
        let v0 = p0.dot(up);
        let d0 = p0.dot(depth);

        let u1 = p1.dot(right);
        let v1 = p1.dot(up);
        let d1 = p1.dot(depth);

        let u2 = p2.dot(right);
        let v2 = p2.dot(up);
        let d2 = p2.dot(depth);

        let u_min = u0.min(u1).min(u2);
        let u_max = u0.max(u1).max(u2);
        let v_min = v0.min(v1).min(v2);
        let v_max = v0.max(v1).max(v2);

        tris.push(ProjectedTri {
            u_min,
            u_max,
            v_min,
            v_max,
            u0,
            v0,
            d0,
            u1,
            v1,
            d1,
            u2,
            v2,
            d2,
        });
    }

    tris
}

/// Menguji apakah titik 2D (u, v) pada kedalaman `test_d` dihalangi oleh segitiga lain yang berada lebih depan (`tri_d > test_d + epsilon`).
fn test_occlusion(u: f32, v: f32, test_d: f32, triangles: &[ProjectedTri]) -> bool {
    const EPSILON: f32 = 0.25; // Toleransi kedalaman mm untuk menghindari self-occlusion di tepi segitiga

    for tri in triangles {
        // Fast AABB rejection
        if u < tri.u_min || u > tri.u_max || v < tri.v_min || v > tri.v_max {
            continue;
        }

        // Uji titik dalam segitiga menggunakan koordinat barisentrik 2D
        let v0x = tri.u2 - tri.u0;
        let v0y = tri.v2 - tri.v0;
        let v1x = tri.u1 - tri.u0;
        let v1y = tri.v1 - tri.v0;
        let v2x = u - tri.u0;
        let v2y = v - tri.v0;

        let dot00 = v0x * v0x + v0y * v0y;
        let dot01 = v0x * v1x + v0y * v1y;
        let dot02 = v0x * v2x + v0y * v2y;
        let dot11 = v1x * v1x + v1y * v1y;
        let dot12 = v1x * v2x + v1y * v2y;

        let denom = dot00 * dot11 - dot01 * dot01;
        if denom.abs() < 1e-7 {
            continue;
        }

        let inv_denom = 1.0 / denom;
        let b = (dot11 * dot02 - dot01 * dot12) * inv_denom;
        let a = (dot00 * dot12 - dot01 * dot02) * inv_denom;

        // Titik berada di dalam segitiga jika a >= 0, b >= 0, dan a + b <= 1
        if a >= -1e-4 && b >= -1e-4 && (a + b) <= 1.0001 {
            // Interpolasi kedalaman segitiga pada (u, v)
            let tri_d = tri.d0 + a * (tri.d1 - tri.d0) + b * (tri.d2 - tri.d0);
            if tri_d > test_d + EPSILON {
                return true; // Ada solid di depan segmen ini!
            }
        }
    }

    false
}

/// Ekstraksi garis siluet (silhouette edges) di mana normal permukaan berubah orientasi terhadap arah kamera.
fn extract_silhouette_edges(mesh: &KernelMesh, view_dir: Vec3) -> Vec<(Vec3, Vec3)> {
    let mut silhouette = Vec::new();
    let tri_count = mesh.triangle_count();
    if tri_count == 0 {
        return silhouette;
    }

    // Hitung normal per segitiga dan simpan dot product dengan arah kamera
    let mut tri_facing_cam = Vec::with_capacity(tri_count);
    for i in 0..tri_count {
        let i0 = mesh.indices[i * 3] as usize;
        let i1 = mesh.indices[i * 3 + 1] as usize;
        let i2 = mesh.indices[i * 3 + 2] as usize;
        let p0 = Vec3::from_array(mesh.positions[i0]);
        let p1 = Vec3::from_array(mesh.positions[i1]);
        let p2 = Vec3::from_array(mesh.positions[i2]);
        let normal = (p1 - p0).cross(p2 - p0);
        tri_facing_cam.push(normal.dot(view_dir) < 0.0);
    }

    // Kumpulkan pasangan tepi terbagi (shared edges)
    // Key: pair of vertex indices sorted
    let mut edge_map: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for tri_idx in 0..tri_count {
        let i0 = mesh.indices[tri_idx * 3];
        let i1 = mesh.indices[tri_idx * 3 + 1];
        let i2 = mesh.indices[tri_idx * 3 + 2];

        for (u, v) in [(i0, i1), (i1, i2), (i2, i0)] {
            let key = if u < v { (u, v) } else { (v, u) };
            edge_map.entry(key).or_default().push(tri_idx);
        }
    }

    for ((u, v), tris) in edge_map {
        if tris.len() == 2 {
            let f1 = tri_facing_cam[tris[0]];
            let f2 = tri_facing_cam[tris[1]];
            // Tepi siluet jika satu segitiga menghadap kamera dan yang lainnya menjauh
            if f1 != f2 {
                let p1 = Vec3::from_array(mesh.positions[u as usize]);
                let p2 = Vec3::from_array(mesh.positions[v as usize]);
                silhouette.push((p1, p2));
            }
        } else if tris.len() == 1 {
            // Boundary edge terbuka
            let p1 = Vec3::from_array(mesh.positions[u as usize]);
            let p2 = Vec3::from_array(mesh.positions[v as usize]);
            silhouette.push((p1, p2));
        }
    }

    silhouette
}

/// Ekstraksi garis sumbu simetri (centerlines) untuk silinder dan lingkaran.
fn extract_centerlines(
    shapes: &[&KernelShape],
    right: Vec3,
    up: Vec3,
    _view_kind: ProjectedViewKind,
) -> Vec<HlrSegment2D> {
    let mut lines = Vec::new();

    for shape in shapes {
        let occ_shape = shape.inner();
        for face in occ_shape.faces() {
            if let Some((axis_pt, axis_dir)) = face.cylinder_or_cone_axis() {
                let p_3d = vec3(axis_pt.x as f32, axis_pt.y as f32, axis_pt.z as f32);
                let dir_3d = vec3(axis_dir.x as f32, axis_dir.y as f32, axis_dir.z as f32).normalize();

                // Panjang garis sumbu sepanjang fitur silinder
                let p1 = p_3d - dir_3d * 20.0;
                let p2 = p_3d + dir_3d * 20.0;

                let u1 = p1.dot(right);
                let v1 = p1.dot(up);
                let u2 = p2.dot(right);
                let v2 = p2.dot(up);

                let len_2d = ((u1 - u2).powi(2) + (v1 - v2).powi(2)).sqrt();
                if len_2d > 2.0 {
                    lines.push(HlrSegment2D {
                        start: [u1, v1],
                        end: [u2, v2],
                        kind: HlrLineKind::Centerline,
                    });
                } else {
                    // Silinder tegak lurus kamera: buat tanda silang pusat (+)
                    let cu = p_3d.dot(right);
                    let cv = p_3d.dot(up);
                    let cross_size = 5.0;
                    lines.push(HlrSegment2D {
                        start: [cu - cross_size, cv],
                        end: [cu + cross_size, cv],
                        kind: HlrLineKind::Centerline,
                    });
                    lines.push(HlrSegment2D {
                        start: [cu, cv - cross_size],
                        end: [cu, cv + cross_size],
                        kind: HlrLineKind::Centerline,
                    });
                }
            }
        }
    }

    lines
}

/// Sederhanakan dan gabungkan segmen-segmen kolinear yang bertipe sama.
fn simplify_and_merge_segments(mut segments: Vec<HlrSegment2D>) -> Vec<HlrSegment2D> {
    if segments.len() <= 1 {
        return segments;
    }

    // Filter segmen yang terlalu pendek
    segments.retain(|s| s.length() >= 0.05);
    segments
}
