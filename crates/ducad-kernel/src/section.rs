//! Ekstraksi Tampak Potongan Melintang (Section View A-A), Generator Arsiran 45° ISO/ANSI,
//! dan Indikator Garis Potong Teknik Berpanah (Fase 11.1).

use glam::{dvec3, vec2, vec3, Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::hlr::{HlrLineKind, HlrSegment2D, ProjectedView, ProjectedViewKind};
use crate::lock_kernel;
use crate::mesh::KernelMesh;
use crate::shape::KernelShape;

/// Konfigurasi bidang pemotong (cutting plane) untuk Section View.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SectionPlaneConfig {
    /// Titik acuan pada bidang potong (dalam koordinat 3D ruang model).
    pub origin: [f32; 3],
    /// Vektor normal bidang pemotong (arah potongan pandangan).
    pub normal: [f32; 3],
    /// Sumbu horizontal 2D pada bidang potong (Screen X pada Section View).
    pub u_axis: [f32; 3],
    /// Sumbu vertikal 2D pada bidang potong (Screen Y pada Section View).
    pub v_axis: [f32; 3],
    /// Jarak spasi antar garis arsir dalam mm (standar ISO 2.0 - 3.5 mm).
    pub hatch_spacing: f32,
    /// Sudut garis arsir dalam derajat (standar ISO 45.0°).
    pub hatch_angle_deg: f32,
}

impl Default for SectionPlaneConfig {
    fn default() -> Self {
        Self {
            // Default: Potongan melintang di tengah sumbu Y (melihat ke arah +Y, bidang XZ)
            origin: [0.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            u_axis: [1.0, 0.0, 0.0],
            v_axis: [0.0, 0.0, 1.0],
            hatch_spacing: 2.5,
            hatch_angle_deg: 45.0,
        }
    }
}

impl SectionPlaneConfig {
    /// Membuat bidang potong melalui titik tengah bounding box model pada sumbu Y (Front-facing section A-A).
    pub fn from_model_bbox_center_y(bbox_min: [f32; 3], bbox_max: [f32; 3]) -> Self {
        let center_y = (bbox_min[1] + bbox_max[1]) * 0.5;
        Self {
            origin: [0.0, center_y, 0.0],
            normal: [0.0, 1.0, 0.0],
            u_axis: [1.0, 0.0, 0.0],
            v_axis: [0.0, 0.0, 1.0],
            hatch_spacing: 2.5,
            hatch_angle_deg: 45.0,
        }
    }

    /// Proyeksikan titik 3D ke koordinat 2D (u, v) pada bidang potong.
    pub fn project_to_2d(&self, p: Vec3) -> Vec2 {
        let o = Vec3::from_array(self.origin);
        let u = Vec3::from_array(self.u_axis);
        let v = Vec3::from_array(self.v_axis);
        let rel = p - o;
        vec2(rel.dot(u), rel.dot(v))
    }

    /// Ubah koordinat 2D (u, v) pada bidang potong kembali ke titik 3D dunia.
    pub fn to_3d(&self, uv: Vec2) -> Vec3 {
        let o = Vec3::from_array(self.origin);
        let u = Vec3::from_array(self.u_axis);
        let v = Vec3::from_array(self.v_axis);
        o + u * uv.x + v * uv.y
    }
}

/// Indikator garis potong panah `A ─── A` pada tampak acuan (mis. Top View atau Front View).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuttingLineIndicator {
    /// Titik awal garis potong pada tampak 2D acuan (mm).
    pub start: [f32; 2],
    /// Titik akhir garis potong pada tampak 2D acuan (mm).
    pub end: [f32; 2],
    /// Arah vektor panah pandangan (tegak lurus garis potong).
    pub arrow_dir: [f32; 2],
    /// Posisi pangkal panah 1 (di ujung titik start).
    pub arrow1_pos: [f32; 2],
    /// Posisi pangkal panah 2 (di ujung titik end).
    pub arrow2_pos: [f32; 2],
    /// Posisi label huruf 'A' 1.
    pub label1_pos: [f32; 2],
    /// Posisi label huruf 'A' 2.
    pub label2_pos: [f32; 2],
    /// Huruf label potongan (mis. "A").
    pub label: String,
}

/// Mesin kalkulasi dan ekstraksi Tampak Potongan Melintang (Section View A-A).
pub struct SectionExtractor;

impl SectionExtractor {
    /// Ekstraksi Section View A-A lengkap dengan kurva batas potongan, pola arsir 45° ISO/ANSI,
    /// dan indikator garis potong pada tampak acuan (fungsi publik yang mengambil `lock_kernel()`).
    pub fn extract_section_view(
        shapes: &[&KernelShape],
        meshes: &[&KernelMesh],
        config: &SectionPlaneConfig,
        model_bbox: ([f32; 3], [f32; 3]),
    ) -> (ProjectedView, CuttingLineIndicator) {
        let _guard = lock_kernel();
        Self::extract_section_view_internal(shapes, meshes, config, model_bbox)
    }

    /// Helper privat/internal untuk pemanggil yang SUDAH memegang `lock_kernel()` (mis. `HlrExtractor::extract_drawing_with_sketch`).
    pub(crate) fn extract_section_view_internal(
        shapes: &[&KernelShape],
        meshes: &[&KernelMesh],
        config: &SectionPlaneConfig,
        model_bbox: ([f32; 3], [f32; 3]),
    ) -> (ProjectedView, CuttingLineIndicator) {
        let origin_3d = Vec3::from_array(config.origin);
        let normal_3d = Vec3::from_array(config.normal).normalize_or_zero();

        // 1. Ekstraksi kurva tepi irisan solid 3D menggunakan OpenCASCADE BRepAlgoAPI_Section
        let mut raw_3d_cut_segments: Vec<(Vec3, Vec3)> = Vec::new();

        let plane_point_d = dvec3(origin_3d.x as f64, origin_3d.y as f64, origin_3d.z as f64);
        let plane_normal_d = dvec3(normal_3d.x as f64, normal_3d.y as f64, normal_3d.z as f64);

        for shape in shapes {
            let occ_shape = shape.inner();
            if let Ok(section_edges) = occ_shape.section_with_plane(plane_point_d, plane_normal_d) {
                for edge_shape in &section_edges {
                    for edge in edge_shape.edges() {
                        let approx = edge.approximation_segments();
                        let pts: Vec<Vec3> = approx
                            .map(|p| vec3(p.x as f32, p.y as f32, p.z as f32))
                            .collect();

                        if pts.len() >= 2 {
                            for w in pts.windows(2) {
                                if (w[0] - w[1]).length_squared() > 1e-6 {
                                    raw_3d_cut_segments.push((w[0], w[1]));
                                }
                            }
                        } else {
                            let p1 = edge.start_point();
                            let p2 = edge.end_point();
                            let v1 = vec3(p1.x as f32, p1.y as f32, p1.z as f32);
                            let v2 = vec3(p2.x as f32, p2.y as f32, p2.z as f32);
                            if (v1 - v2).length_squared() > 1e-6 {
                                raw_3d_cut_segments.push((v1, v2));
                            }
                        }
                    }
                }
            }
        }

        // Fallback / tambahan jika ada mesh geometri tanpa B-Rep solid
        let merged_mesh = KernelMesh::merge(meshes);
        if raw_3d_cut_segments.is_empty() && !merged_mesh.positions.is_empty() {
            let mesh_cut_segs = slice_mesh_with_plane(&merged_mesh, origin_3d, normal_3d);
            raw_3d_cut_segments.extend(mesh_cut_segs);
        }

        // 2. Proyeksikan segmen 3D irisan ke koordinat 2D (u, v) pada bidang potong
        let mut cut_segments_2d: Vec<[Vec2; 2]> = Vec::new();
        let mut bounds_min = vec2(f32::MAX, f32::MAX);
        let mut bounds_max = vec2(f32::MIN, f32::MIN);

        for (p1_3d, p2_3d) in &raw_3d_cut_segments {
            let uv1 = config.project_to_2d(*p1_3d);
            let uv2 = config.project_to_2d(*p2_3d);

            if (uv1 - uv2).length_squared() < 1e-4 {
                continue;
            }

            bounds_min.x = bounds_min.x.min(uv1.x).min(uv2.x);
            bounds_min.y = bounds_min.y.min(uv1.y).min(uv2.y);
            bounds_max.x = bounds_max.x.max(uv1.x).max(uv2.x);
            bounds_max.y = bounds_max.y.max(uv1.y).max(uv2.y);

            cut_segments_2d.push([uv1, uv2]);
        }

        if bounds_min.x > bounds_max.x {
            bounds_min = vec2(0.0, 0.0);
            bounds_max = vec2(100.0, 100.0);
        }

        // 3. Generator Pola Arsiran (Hatch Pattern) 45° ISO/ANSI
        let hatch_segs_2d = generate_iso_hatch_pattern(
            &cut_segments_2d,
            bounds_min,
            bounds_max,
            config.hatch_spacing,
            config.hatch_angle_deg,
        );

        // 4. Bangun segmen HlrSegment2D lengkap
        let mut final_segments: Vec<HlrSegment2D> = Vec::new();

        // Garis batas irisan solid tebal (Section Outline)
        for seg in &cut_segments_2d {
            final_segments.push(HlrSegment2D {
                start: [seg[0].x, seg[0].y],
                end: [seg[1].x, seg[1].y],
                kind: HlrLineKind::Visible,
            });
        }

        // Garis arsir miring 45° halus
        for h_seg in hatch_segs_2d {
            final_segments.push(h_seg);
        }

        // Ekstraksi background geometry di belakang bidang potong
        let background_segs = extract_section_background(
            shapes,
            &merged_mesh,
            origin_3d,
            normal_3d,
            config,
        );
        for bg in background_segs {
            final_segments.push(bg);
        }

        let dx = (model_bbox.1[0] - model_bbox.0[0]).abs().max(1.0);
        let dy = (model_bbox.1[1] - model_bbox.0[1]).abs().max(1.0);
        let dz = (model_bbox.1[2] - model_bbox.0[2]).abs().max(1.0);

        let section_view = ProjectedView {
            kind: ProjectedViewKind::SectionAA,
            title: "TAMPAK POTONGAN A-A".to_string(),
            bounds_min: [bounds_min.x, bounds_min.y],
            bounds_max: [bounds_max.x, bounds_max.y],
            segments: final_segments,
            centerlines: Vec::new(),
            features: Vec::new(),
            width_mm: dx,
            height_mm: dz,
            depth_mm: dy,
        };

        // 5. Kalkulasi Indikator Garis Potong Berpanah A-A pada Tampak Acuan (Top View)
        let indicator = build_cutting_line_indicator(config, model_bbox);

        (section_view, indicator)
    }
}

/// Menghasilkan pola arsir 45° standar ISO/ANSI menggunakan algoritma scanline ray-casting
/// dengan uji paritas even-odd pada loop kontur tertutup hasil irisan solid.
pub fn generate_iso_hatch_pattern(
    segments: &[[Vec2; 2]],
    bounds_min: Vec2,
    bounds_max: Vec2,
    spacing_mm: f32,
    angle_deg: f32,
) -> Vec<HlrSegment2D> {
    if segments.is_empty() {
        return Vec::new();
    }

    let spacing = spacing_mm.max(1.0);
    let rad = angle_deg.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    // Vektor arah garis arsir D dan vektor normal tegak lurus N
    let dir = vec2(cos_a, sin_a);
    let norm = vec2(-sin_a, cos_a);

    // Hitung proyeksi rentang bounding box terhadap vektor normal
    let corners = [
        bounds_min,
        vec2(bounds_max.x, bounds_min.y),
        bounds_max,
        vec2(bounds_min.x, bounds_max.y),
    ];

    let mut n_min = f32::MAX;
    let mut n_max = f32::MIN;
    for c in &corners {
        let proj = c.dot(norm);
        n_min = n_min.min(proj);
        n_max = n_max.max(proj);
    }

    let mut hatch_segments = Vec::new();
    let mut offset = n_min + spacing * 0.5;

    while offset <= n_max {
        // Untuk garis dengan persamaan P . norm = offset:
        // Cari seluruh titik perpotongan dengan segmen batas irisan
        let mut t_intersections: Vec<f32> = Vec::new();

        for seg in segments {
            let p1 = seg[0];
            let p2 = seg[1];

            let d1 = p1.dot(norm) - offset;
            let d2 = p2.dot(norm) - offset;

            // Uji apakah garis memotong segmen
            if (d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0) {
                let u = d1 / (d1 - d2);
                let hit_pt = p1 + (p2 - p1) * u;
                let t = hit_pt.dot(dir);
                t_intersections.push(t);
            } else if d1.abs() < 1e-4 {
                let t = p1.dot(dir);
                t_intersections.push(t);
            }
        }

        // Urutkan titik perpotongan sepanjang arah garis arsir
        t_intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Hapus duplikat titik yang terlalu dekat (mis. perpotongan di sudut vertex)
        let mut unique_t: Vec<f32> = Vec::new();
        for t in t_intersections {
            if let Some(last) = unique_t.last() {
                if (t - *last).abs() > 0.05 {
                    unique_t.push(t);
                }
            } else {
                unique_t.push(t);
            }
        }

        // Pasangkan perpotongan secara even-odd (In-Solid Span: [t0, t1], [t2, t3], ...)
        for chunk in unique_t.chunks_exact(2) {
            let t_start = chunk[0];
            let t_end = chunk[1];

            if t_end - t_start > 0.1 {
                // Rekonstruksi titik 2D dari (offset, t)
                let pt1 = norm * offset + dir * t_start;
                let pt2 = norm * offset + dir * t_end;

                hatch_segments.push(HlrSegment2D {
                    start: [pt1.x, pt1.y],
                    end: [pt2.x, pt2.y],
                    kind: HlrLineKind::Hatch,
                });
            }
        }

        offset += spacing;
    }

    hatch_segments
}

/// Iris mesh segitiga dengan bidang potong 3D menghasilkan segmen garis irisan.
fn slice_mesh_with_plane(mesh: &KernelMesh, plane_orig: Vec3, plane_norm: Vec3) -> Vec<(Vec3, Vec3)> {
    let mut cut_segments = Vec::new();
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

        let d0 = (p0 - plane_orig).dot(plane_norm);
        let d1 = (p1 - plane_orig).dot(plane_norm);
        let d2 = (p2 - plane_orig).dot(plane_norm);

        let mut hits = Vec::new();

        // Edge 0-1
        if (d0 > 0.0 && d1 < 0.0) || (d0 < 0.0 && d1 > 0.0) {
            let t = d0 / (d0 - d1);
            hits.push(p0 + (p1 - p0) * t);
        }
        // Edge 1-2
        if (d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0) {
            let t = d1 / (d1 - d2);
            hits.push(p1 + (p2 - p1) * t);
        }
        // Edge 2-0
        if (d2 > 0.0 && d0 < 0.0) || (d2 < 0.0 && d0 > 0.0) {
            let t = d2 / (d2 - d0);
            hits.push(p2 + (p0 - p2) * t);
        }

        if hits.len() >= 2 && (hits[0] - hits[1]).length_squared() > 1e-6 {
            cut_segments.push((hits[0], hits[1]));
        }
    }

    cut_segments
}

/// Ekstraksi garis tampak dari geometri yang berada di belakang bidang potong (Background View).
fn extract_section_background(
    shapes: &[&KernelShape],
    _mesh: &KernelMesh,
    plane_orig: Vec3,
    plane_norm: Vec3,
    config: &SectionPlaneConfig,
) -> Vec<HlrSegment2D> {
    let mut bg_segments = Vec::new();

    for shape in shapes {
        let occ = shape.inner();
        for edge in occ.edges() {
            let p1 = edge.start_point();
            let p2 = edge.end_point();
            let v1 = vec3(p1.x as f32, p1.y as f32, p1.z as f32);
            let v2 = vec3(p2.x as f32, p2.y as f32, p2.z as f32);

            let d1 = (v1 - plane_orig).dot(plane_norm);
            let d2 = (v2 - plane_orig).dot(plane_norm);

            // Hanya ambil garis yang berada di belakang bidang potong (dalam arah pandang)
            if d1 > 0.01 && d2 > 0.01 {
                let uv1 = config.project_to_2d(v1);
                let uv2 = config.project_to_2d(v2);

                if (uv1 - uv2).length_squared() > 0.05 {
                    bg_segments.push(HlrSegment2D {
                        start: [uv1.x, uv1.y],
                        end: [uv2.x, uv2.y],
                        kind: HlrLineKind::Visible,
                    });
                }
            }
        }
    }

    bg_segments
}

/// Menghitung geometri indikator garis potong panah `A ─── A` pada tampak atas acuan (Top View).
pub fn build_cutting_line_indicator(
    config: &SectionPlaneConfig,
    model_bbox: ([f32; 3], [f32; 3]),
) -> CuttingLineIndicator {
    // Pada Top View (sumbu X = Screen X, sumbu Y = Screen Y):
    // Garis potong melintang X pada posisi Y = origin.y
    let x_min = model_bbox.0[0];
    let x_max = model_bbox.1[0];
    let cut_y = config.origin[1];

    let overhang = 8.0; // Perpanjangan garis melewati batas part dalam mm
    let start_x = x_min - overhang;
    let end_x = x_max + overhang;

    let arrow_len = 6.0;
    let arrow_dir = [0.0, 1.0]; // Menghadap ke arah +Y (arah pandangan Section A-A)

    let arrow1_pos = [start_x, cut_y + arrow_len];
    let arrow2_pos = [end_x, cut_y + arrow_len];

    let label1_pos = [start_x - 3.5, cut_y + arrow_len + 4.0];
    let label2_pos = [end_x + 1.0, cut_y + arrow_len + 4.0];

    CuttingLineIndicator {
        start: [start_x, cut_y],
        end: [end_x, cut_y],
        arrow_dir,
        arrow1_pos,
        arrow2_pos,
        label1_pos,
        label2_pos,
        label: "A".to_string(),
    }
}
