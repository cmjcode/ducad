//! Tampak Potongan (Section View), arsiran ISO, dan garis potong berpanah.
//!
//! Fase 11.1 hanya mengenal satu potongan A-A di tengah sumbu Y. P21.1
//! menjadikannya umum: bidang potong eksplisit ([`SectionPlaneConfig::from_axis`],
//! [`SectionPlaneConfig::through_points`]), beberapa potongan per gambar
//! ([`SectionView`]), dan potongan bertingkat ([`SectionPath`]).
//!
//! # Cara kerja
//!
//! Setiap potongan dinyatakan sebagai garis potong (polyline) di tampak
//! induk. Material di sisi pemirsa garis itu dibuang dengan SATU boolean
//! (prisma pemotong hasil extrude poligon garis potong), lalu sisa solidnya
//! diproyeksikan dengan HLR eksak searah panah. Face hasil potong diarsir.
//! Seluruhnya berjalan di bawah satu guard `lock_kernel()` lewat helper
//! `pub(crate)` — tidak ada fungsi publik kernel yang dipanggil di dalamnya.

use glam::{dvec3, vec2, vec3, DVec3, Vec2, Vec3};
use opencascade::primitives::{IntoShape, Shape};
use serde::{Deserialize, Serialize};

use crate::hlr::{HlrLineKind, HlrSegment2D, ProjectedView, ProjectedViewKind};
use crate::lock_kernel;
use crate::mesh::KernelMesh;
use crate::picking::face::SurfaceKind;
use crate::profile::{Profile, ProfileSegment};
use crate::shape::KernelShape;

/// Sumbu dunia untuk [`SectionPlaneConfig::from_axis`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionAxis {
    X,
    Y,
    Z,
}

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

    /// Bidang potong tegak lurus `axis`, digeser `offset_mm` dari pusat bbox.
    ///
    /// Arah pandang bawaan mengikuti tampak standar: X → seperti Tampak
    /// Kanan (memandang −X), Y → Tampak Depan (+Y), Z → Tampak Atas (−Z).
    /// `flip` membalik arah pandang.
    pub fn from_axis(
        axis: SectionAxis,
        offset_mm: f32,
        flip: bool,
        bbox: ([f32; 3], [f32; 3]),
    ) -> Self {
        let center = (Vec3::from_array(bbox.0) + Vec3::from_array(bbox.1)) * 0.5;
        let (unit, mut dir) = match axis {
            SectionAxis::X => (Vec3::X, -Vec3::X),
            SectionAxis::Y => (Vec3::Y, Vec3::Y),
            SectionAxis::Z => (Vec3::Z, -Vec3::Z),
        };
        if flip {
            dir = -dir;
        }
        let origin = center + unit * offset_mm;
        Self::with_view_dir(origin, dir)
    }

    /// Bidang melalui dua titik `p1`→`p2` pada tampak induk, tegak lurus
    /// tampak itu. Arah pandang = sisi KIRI arah `p1`→`p2`.
    pub fn through_points(p1: [f32; 2], p2: [f32; 2], parent: ProjectedViewKind) -> Self {
        let (_, right, up) = parent.camera_vectors();
        let a = right * p1[0] + up * p1[1];
        let t = vec2(p2[0] - p1[0], p2[1] - p1[1]).normalize_or_zero();
        let dir = right * -t.y + up * t.x;
        Self::with_view_dir(a, dir)
    }

    /// Bidang melalui `origin` dengan arah pandang `dir`; sumbu gambar
    /// mengikuti aturan tampak standar ([`section_axes`]).
    pub fn with_view_dir(origin: Vec3, dir: Vec3) -> Self {
        let dir = dir.normalize_or_zero();
        let (u, v) = section_axes(dir);
        Self {
            origin: origin.to_array(),
            normal: dir.to_array(),
            u_axis: u.to_array(),
            v_axis: v.to_array(),
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

/// Sumbu kanan/atas gambar untuk arah pandang `dir` (dari pemirsa ke objek),
/// mengikuti tampak ortografik standar: Z ke atas bila arah pandang mendatar;
/// untuk pandangan dari atas/bawah, X ke kanan.
pub(crate) fn section_axes(dir: Vec3) -> (Vec3, Vec3) {
    let d = dir.normalize_or_zero();
    let toward = -d;
    if d.z.abs() < 0.9 {
        let up = (Vec3::Z - d * d.z).normalize_or_zero();
        (up.cross(toward).normalize_or_zero(), up)
    } else {
        let right = Vec3::X;
        (right, toward.cross(right).normalize_or_zero())
    }
}

/// Garis potong (lurus atau bertingkat) di koordinat 2D tampak induk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionPath {
    /// ≥ 2 titik (mm model, koordinat tampak induk). Ruas pertama menentukan
    /// arah potong; ruas berikutnya harus sejajar atau tegak lurus terhadapnya.
    pub points: Vec<[f32; 2]>,
    pub parent: ProjectedViewKind,
}

/// Permintaan satu tampak potongan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionRequest {
    /// Huruf label, mis. "A" → "SECTION A-A".
    pub label: String,
    pub path: SectionPath,
    pub hatch_spacing: f32,
    pub hatch_angle_deg: f32,
}

impl SectionRequest {
    pub fn from_path(label: &str, path: SectionPath) -> Self {
        Self {
            label: label.to_string(),
            path,
            hatch_spacing: 2.5,
            hatch_angle_deg: 45.0,
        }
    }

    /// Lihat [`SectionPlaneConfig::from_axis`]. Tampak induk: Atas untuk
    /// sumbu X/Y, Depan untuk sumbu Z.
    pub fn from_axis(
        label: &str,
        axis: SectionAxis,
        offset_mm: f32,
        flip: bool,
        bbox: ([f32; 3], [f32; 3]),
    ) -> Self {
        let cfg = SectionPlaneConfig::from_axis(axis, offset_mm, flip, bbox);
        Self::from_plane(label, &cfg, bbox)
    }

    /// Lihat [`SectionPlaneConfig::through_points`].
    pub fn through_points(label: &str, p1: [f32; 2], p2: [f32; 2], parent: ProjectedViewKind) -> Self {
        Self::from_path(
            label,
            SectionPath {
                points: vec![p1, p2],
                parent,
            },
        )
    }

    /// Ubah bidang potong 3D menjadi garis potong pada tampak induk yang
    /// sesuai (Atas bila bidangnya tegak, Depan bila mendatar).
    pub fn from_plane(label: &str, cfg: &SectionPlaneConfig, bbox: ([f32; 3], [f32; 3])) -> Self {
        let dir = Vec3::from_array(cfg.normal).normalize_or_zero();
        let parent = if dir.z.abs() < 0.9 {
            ProjectedViewKind::Top
        } else {
            ProjectedViewKind::Front
        };
        Self::plane_on_parent(label, cfg, bbox, parent)
    }

    /// Seperti [`Self::from_plane`] dengan tampak induk pilihan. `Err` bila
    /// bidang potong tidak tegak lurus tampak induk itu.
    pub fn from_plane_on(
        label: &str,
        cfg: &SectionPlaneConfig,
        bbox: ([f32; 3], [f32; 3]),
        parent: Option<ProjectedViewKind>,
    ) -> Result<Self, String> {
        let dir = Vec3::from_array(cfg.normal).normalize_or_zero();
        let auto = Self::from_plane(label, cfg, bbox);
        let Some(parent) = parent else {
            return Ok(auto);
        };
        if !matches!(
            parent,
            ProjectedViewKind::Front | ProjectedViewKind::Top | ProjectedViewKind::Right
        ) {
            return Err("section parent view must be front, top or right".to_string());
        }
        let (view_dir, _, _) = parent.camera_vectors();
        if dir.dot(view_dir).abs() > 0.05 {
            // Bidang sejajar tampak induk yang diminta: pakai induk otomatis.
            return Ok(auto);
        }
        Ok(Self::plane_on_parent(label, cfg, bbox, parent))
    }

    fn plane_on_parent(
        label: &str,
        cfg: &SectionPlaneConfig,
        bbox: ([f32; 3], [f32; 3]),
        parent: ProjectedViewKind,
    ) -> Self {
        let dir = Vec3::from_array(cfg.normal).normalize_or_zero();
        let (_, right, up) = parent.camera_vectors();
        let d2 = vec2(dir.dot(right), dir.dot(up)).normalize_or_zero();
        // Arah jalan `t` sehingga sisi kirinya = arah pandang: kiri(t) = (−t.y, t.x).
        let t2 = vec2(d2.y, -d2.x);
        let origin = Vec3::from_array(cfg.origin);
        let o2 = vec2(origin.dot(right), origin.dot(up));
        // Rentang model sepanjang arah jalan.
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for corner in bbox_corners(bbox) {
            let c2 = vec2(corner.dot(right), corner.dot(up));
            let t = (c2 - o2).dot(t2);
            lo = lo.min(t);
            hi = hi.max(t);
        }
        let a = o2 + t2 * lo;
        let b = o2 + t2 * hi;
        let mut req = Self::from_path(
            label,
            SectionPath {
                points: vec![[a.x, a.y], [b.x, b.y]],
                parent,
            },
        );
        req.hatch_spacing = cfg.hatch_spacing;
        req.hatch_angle_deg = cfg.hatch_angle_deg;
        req
    }
}

fn bbox_corners(bbox: ([f32; 3], [f32; 3])) -> [Vec3; 8] {
    let (a, b) = bbox;
    [
        vec3(a[0], a[1], a[2]),
        vec3(b[0], a[1], a[2]),
        vec3(a[0], b[1], a[2]),
        vec3(b[0], b[1], a[2]),
        vec3(a[0], a[1], b[2]),
        vec3(b[0], a[1], b[2]),
        vec3(a[0], b[1], b[2]),
        vec3(b[0], b[1], b[2]),
    ]
}

/// Indikator garis potong berpanah pada tampak induk.
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
    /// Posisi label huruf 1.
    pub label1_pos: [f32; 2],
    /// Posisi label huruf 2.
    pub label2_pos: [f32; 2],
    /// Huruf label potongan (mis. "A").
    pub label: String,
    /// Polyline lengkap garis potong (bersiku untuk potongan bertingkat),
    /// sudah dilebihkan melewati tepi part. Kosong = garis lurus start→end.
    #[serde(default)]
    pub points: Vec<[f32; 2]>,
}

impl CuttingLineIndicator {
    /// Polyline garis potong (selalu ≥ 2 titik).
    pub fn polyline(&self) -> Vec<[f32; 2]> {
        if self.points.len() >= 2 {
            self.points.clone()
        } else {
            vec![self.start, self.end]
        }
    }
}

/// Satu tampak potongan lengkap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionView {
    pub label: String,
    pub view: ProjectedView,
    pub cutting_line: CuttingLineIndicator,
    /// Tampak tempat garis potong digambar.
    pub parent: ProjectedViewKind,
    /// Bidang potong ruas pertama (arah pandang = `normal`).
    pub config: SectionPlaneConfig,
    /// Garis potong asal (untuk mengulang ekstraksi setelah geometri berubah).
    pub path: SectionPath,
    /// Luas total penampang terpotong (mm²). 0 = bidang tidak memotong body.
    #[serde(default)]
    pub cut_area_mm2: f32,
    /// Panjang total kurva batas penampang (mm).
    #[serde(default)]
    pub cut_length_mm: f32,
}

impl SectionView {
    pub fn kind(&self) -> ProjectedViewKind {
        ProjectedViewKind::Section(self.label.chars().next().unwrap_or('A'))
    }
}

/// Kerangka potong 3D hasil penguraian [`SectionPath`].
struct CutFrame {
    origin: Vec3,
    /// Arah jalan garis potong.
    t: Vec3,
    /// Arah pandang (pemirsa → objek).
    d: Vec3,
    /// Titik garis potong dalam koordinat (t, d) relatif `origin`.
    pts: Vec<(f32, f32)>,
}

fn resolve_path(path: &SectionPath) -> Result<CutFrame, String> {
    if !matches!(
        path.parent,
        ProjectedViewKind::Front | ProjectedViewKind::Top | ProjectedViewKind::Right
    ) {
        return Err("section parent view must be front, top or right".to_string());
    }
    if path.points.len() < 2 {
        return Err("section path needs at least 2 points".to_string());
    }
    let (_, right, up) = path.parent.camera_vectors();
    let p0 = Vec2::from_array(path.points[0]);
    let t2 = (Vec2::from_array(path.points[1]) - p0).normalize_or_zero();
    if t2.length_squared() < 0.5 {
        return Err("section path: first two points coincide".to_string());
    }
    let d2 = vec2(-t2.y, t2.x);
    let mut pts = Vec::with_capacity(path.points.len());
    for (i, p) in path.points.iter().enumerate() {
        let rel = Vec2::from_array(*p) - p0;
        let cur = (rel.dot(t2), rel.dot(d2));
        if let Some(&(pt, pd)) = pts.last() {
            let (dt, dd): (f32, f32) = (cur.0 - pt, cur.1 - pd);
            let along = dd.abs() < 1e-3 * (1.0 + dt.abs());
            let step = dt.abs() < 1e-3 * (1.0 + dd.abs());
            if !along && !step {
                return Err(format!(
                    "section path segment {i} must be parallel or perpendicular to the first segment"
                ));
            }
            if along && dt <= 0.0 {
                return Err(format!("section path segment {i} doubles back"));
            }
        }
        pts.push(cur);
    }
    Ok(CutFrame {
        origin: right * p0.x + up * p0.y,
        t: right * t2.x + up * t2.y,
        d: right * d2.x + up * d2.y,
        pts,
    })
}

/// Mesin ekstraksi Tampak Potongan.
pub struct SectionExtractor;

impl SectionExtractor {
    /// Ekstraksi satu tampak potongan dari bidang potong 3D (API lama,
    /// dipertahankan untuk GUI). Mengambil `lock_kernel()`.
    pub fn extract_section_view(
        shapes: &[&KernelShape],
        meshes: &[&KernelMesh],
        config: &SectionPlaneConfig,
        model_bbox: ([f32; 3], [f32; 3]),
    ) -> (ProjectedView, CuttingLineIndicator) {
        let _guard = lock_kernel();
        let req = SectionRequest::from_plane("A", config, model_bbox);
        match Self::extract_internal(shapes, meshes, &req, model_bbox, true) {
            Ok((section, _)) => (section.view, section.cutting_line),
            Err(_) => (
                ProjectedView {
                    kind: ProjectedViewKind::Section('A'),
                    title: ProjectedViewKind::Section('A').title_id(),
                    ..ProjectedView::default()
                },
                indicator_for(&req, None, model_bbox),
            ),
        }
    }

    /// Ekstraksi satu tampak potongan dari [`SectionRequest`]. Mengambil
    /// `lock_kernel()`. `Err` berisi pesan berawalan kode (`DRAWING_SECTION_…`).
    pub fn extract(
        shapes: &[&KernelShape],
        meshes: &[&KernelMesh],
        request: &SectionRequest,
        model_bbox: ([f32; 3], [f32; 3]),
    ) -> Result<SectionView, String> {
        let _guard = lock_kernel();
        Self::extract_internal(shapes, meshes, request, model_bbox, true).map(|(s, _)| s)
    }

    /// Inti tanpa lock. Mengembalikan tampak + peringatan berkode.
    pub(crate) fn extract_internal(
        shapes: &[&KernelShape],
        meshes: &[&KernelMesh],
        request: &SectionRequest,
        model_bbox: ([f32; 3], [f32; 3]),
        exact: bool,
    ) -> Result<(SectionView, Vec<String>), String> {
        let label_char = request
            .label
            .chars()
            .next()
            .filter(|c| c.is_ascii_alphabetic())
            .ok_or_else(|| "DRAWING_SECTION_PATH: section label must start with a letter".to_string())?
            .to_ascii_uppercase();
        let label = label_char.to_string();
        let frame = resolve_path(&request.path)
            .map_err(|e| format!("DRAWING_SECTION_PATH: section {label}: {e}"))?;
        let kind = ProjectedViewKind::Section(label_char);
        let (right, up) = section_axes(frame.d);
        let mut warnings = Vec::new();

        let diag = (Vec3::from_array(model_bbox.1) - Vec3::from_array(model_bbox.0)).length();
        let big = (diag * 4.0 + 100.0) as f64;

        let mut segments: Vec<HlrSegment2D> = Vec::new();
        let mut arcs = Vec::new();
        let mut features = Vec::new();
        let mut centerlines = Vec::new();
        let mut cut_area = 0.0f64;
        let mut cut_length = 0.0f64;
        let mut is_exact = false;

        let occ: Vec<&Shape> = shapes.iter().map(|s| s.inner()).collect();
        let cutter = if occ.is_empty() { None } else { build_cutter(&frame, big).ok() };

        if let Some(cutter) = cutter {
            // Potong tiap body sendiri-sendiri supaya arsirnya bisa berselang.
            let mut remains: Vec<Shape> = Vec::new();
            let mut hatched_bodies = 0usize;
            for shape in &occ {
                let cut = match shape.subtract(&cutter) {
                    Ok(result) => result.shape,
                    Err(_) => {
                        warnings.push(format!(
                            "DRAWING_SECTION_FALLBACK: boolean cut failed for one body in section {label}; body drawn uncut"
                        ));
                        remains.push(crate::shape::deep_clone(shape).map_err(|e| e.to_string())?);
                        continue;
                    }
                };
                // Arsir dihitung PER FACE potong: pada potongan bertingkat dua
                // face bertemu di garis siku, dan menggabung konturnya akan
                // merusak paritas even-odd di sepanjang garis itu.
                // ISO 128-50: body bersebelahan diarsir berselang 45°/135°.
                let angle = request.hatch_angle_deg + 90.0 * (hatched_bodies % 2) as f32;
                let mut body_hatched = false;
                for face in cut.faces() {
                    if SurfaceKind::from(face.surface_kind().as_str()) != SurfaceKind::Plane {
                        continue;
                    }
                    let n = face.normal_at_center();
                    let n = vec3(n.x as f32, n.y as f32, n.z as f32).normalize_or_zero();
                    // Face potong menghadap pemirsa.
                    if n.dot(frame.d) > -0.999 {
                        continue;
                    }
                    let c = face.center_of_mass();
                    let rel = vec3(c.x as f32, c.y as f32, c.z as f32) - frame.origin;
                    let (ct, cd) = (rel.dot(frame.t), rel.dot(frame.d));
                    if !on_cut_plane(&frame, ct, cd) {
                        continue;
                    }
                    cut_area += face.surface_area();
                    let mut loops: Vec<[Vec2; 2]> = Vec::new();
                    for edge in face.edges() {
                        let pts: Vec<Vec2> = edge
                            .approximation_segments()
                            .map(|p| {
                                let q = vec3(p.x as f32, p.y as f32, p.z as f32);
                                vec2(q.dot(right), q.dot(up))
                            })
                            .collect();
                        for w in pts.windows(2) {
                            let len = (w[0] - w[1]).length();
                            if len > 1e-4 {
                                cut_length += len as f64;
                                loops.push([w[0], w[1]]);
                            }
                        }
                    }
                    if !loops.is_empty() {
                        body_hatched = true;
                        let (lo, hi) = loops_bounds(&loops);
                        segments.extend(generate_iso_hatch_pattern(
                            &loops,
                            lo,
                            hi,
                            request.hatch_spacing,
                            angle,
                        ));
                    }
                }
                if body_hatched {
                    hatched_bodies += 1;
                }
                if cut.faces().next().is_some() {
                    remains.push(cut);
                }
            }

            let remain_refs: Vec<&Shape> = remains.iter().collect();
            let lines = if exact {
                // Garis tersembunyi ikut dihitung dulu: fitur lingkaran yang
                // sebagian tertutup butuh busur tersembunyinya. Dibuang lagi
                // sebelum digambar (tampak potongan tidak memuat garis putus).
                crate::hlr_sheet::exact_lines(&remain_refs, right, up, true, None)
            } else {
                None
            };
            match lines {
                Some(mut lines) => {
                    is_exact = true;
                    // Siku potongan bertingkat tidak digambar di tampak potongan.
                    let t2 = vec2(frame.t.dot(right), frame.t.dot(up));
                    let base = vec2(frame.origin.dot(right), frame.origin.dot(up)).dot(t2);
                    let steps: Vec<f32> = frame
                        .pts
                        .windows(2)
                        .filter(|w| (w[0].0 - w[1].0).abs() < 1e-3 && (w[0].1 - w[1].1).abs() > 1e-3)
                        .map(|w| base + w[0].0)
                        .collect();
                    if !steps.is_empty() && t2.length_squared() > 0.5 {
                        lines.segments.retain(|s| {
                            let a = Vec2::from_array(s.start).dot(t2);
                            let b = Vec2::from_array(s.end).dot(t2);
                            !steps
                                .iter()
                                .any(|st| (a - st).abs() < 2e-3 && (b - st).abs() < 2e-3)
                        });
                    }
                    let (f, c) = crate::hlr_sheet::exact_features(&remain_refs, &lines, right, up, diag);
                    features = f;
                    centerlines = c;
                    lines.segments.retain(|s| s.kind != HlrLineKind::Hidden);
                    lines.arcs.retain(|a| a.kind != HlrLineKind::Hidden);
                    segments.extend(lines.segments);
                    arcs = lines.arcs;
                }
                None => {
                    if exact && !remain_refs.is_empty() {
                        warnings.push(format!(
                            "HLR_EXACT_FALLBACK: exact hidden-line removal failed for SECTION {label}-{label}; only the cut outline is drawn"
                        ));
                    }
                }
            }
        } else {
            // Tanpa B-rep: iris mesh dengan bidang ruas pertama.
            let merged = KernelMesh::merge(meshes);
            let cut = slice_mesh_with_plane(&merged, frame.origin, frame.d);
            let loops: Vec<[Vec2; 2]> = cut
                .iter()
                .map(|(a, b)| [vec2(a.dot(right), a.dot(up)), vec2(b.dot(right), b.dot(up))])
                .filter(|l| (l[0] - l[1]).length_squared() > 1e-4)
                .collect();
            for l in &loops {
                cut_length += (l[0] - l[1]).length() as f64;
                segments.push(HlrSegment2D::new(l[0], l[1], HlrLineKind::Visible));
            }
            if !loops.is_empty() {
                let (lo, hi) = loops_bounds(&loops);
                segments.extend(generate_iso_hatch_pattern(
                    &loops,
                    lo,
                    hi,
                    request.hatch_spacing,
                    request.hatch_angle_deg,
                ));
                // Luas poligon tidak dihitung untuk mesh; cukup penanda non-nol.
                cut_area = cut_length;
            }
        }

        if cut_area <= 1e-6 {
            warnings.push(format!(
                "DRAWING_SECTION_EMPTY: section {label}-{label} does not cut any body"
            ));
        }

        let mut lo = vec2(f32::MAX, f32::MAX);
        let mut hi = vec2(f32::MIN, f32::MIN);
        for s in &segments {
            for p in [s.start, s.end] {
                lo = lo.min(Vec2::from_array(p));
                hi = hi.max(Vec2::from_array(p));
            }
        }
        for a in &arcs {
            let a: &crate::hlr::HlrArc2D = a;
            let (b0, b1) = a.bounds();
            lo = lo.min(Vec2::from_array(b0));
            hi = hi.max(Vec2::from_array(b1));
        }
        if lo.x > hi.x {
            lo = vec2(0.0, 0.0);
            hi = vec2(100.0, 100.0);
        }

        let view = ProjectedView {
            kind,
            title: kind.title_id(),
            bounds_min: lo.to_array(),
            bounds_max: hi.to_array(),
            segments,
            centerlines,
            features,
            width_mm: (model_bbox.1[0] - model_bbox.0[0]).abs().max(1.0),
            height_mm: (model_bbox.1[2] - model_bbox.0[2]).abs().max(1.0),
            depth_mm: (model_bbox.1[1] - model_bbox.0[1]).abs().max(1.0),
            arcs,
            edge_refs: Vec::new(),
            exact: is_exact,
        };

        let mut labelled = request.clone();
        labelled.label = label.clone();
        let cutting_line = indicator_for(&labelled, Some(&frame), model_bbox);
        let mut config = SectionPlaneConfig::with_view_dir(frame.origin, frame.d);
        config.hatch_spacing = request.hatch_spacing;
        config.hatch_angle_deg = request.hatch_angle_deg;

        Ok((
            SectionView {
                label,
                view,
                cutting_line,
                parent: request.path.parent,
                config,
                path: request.path.clone(),
                cut_area_mm2: cut_area as f32,
                cut_length_mm: cut_length as f32,
            },
            warnings,
        ))
    }
}

/// Apakah titik (t, d) berada pada salah satu ruas potong (bukan ruas siku).
fn on_cut_plane(frame: &CutFrame, t: f32, d: f32) -> bool {
    let n = frame.pts.len();
    let mut i = 0;
    while i + 1 < n {
        let (a, b) = (frame.pts[i], frame.pts[i + 1]);
        if (a.1 - b.1).abs() < 1e-3 {
            // Ruas pertama/terakhir memanjang tak hingga ke luar.
            let lo = if i == 0 { f32::MIN } else { a.0 - 1e-2 };
            let hi = if i + 2 >= n { f32::MAX } else { b.0 + 1e-2 };
            if (d - a.1).abs() < 2e-3 && t >= lo && t <= hi {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn loops_bounds(loops: &[[Vec2; 2]]) -> (Vec2, Vec2) {
    let mut lo = vec2(f32::MAX, f32::MAX);
    let mut hi = vec2(f32::MIN, f32::MIN);
    for l in loops {
        for p in l {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
    }
    (lo, hi)
}

/// Prisma pemotong: seluruh ruang di sisi pemirsa garis potong.
fn build_cutter(frame: &CutFrame, big: f64) -> anyhow::Result<Shape> {
    let first = frame.pts[0];
    let last = frame.pts[frame.pts.len() - 1];
    let d_min = frame.pts.iter().map(|p| p.1 as f64).fold(f64::MAX, f64::min) - big;
    let mut poly: Vec<(f64, f64)> = Vec::new();
    poly.push((first.0 as f64 - big, first.1 as f64));
    for p in &frame.pts {
        poly.push((p.0 as f64, p.1 as f64));
    }
    poly.push((last.0 as f64 + big, last.1 as f64));
    poly.push((last.0 as f64 + big, d_min));
    poly.push((first.0 as f64 - big, d_min));
    // Buang titik berimpit berurutan.
    poly.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);

    let mut segs = Vec::with_capacity(poly.len());
    for i in 0..poly.len() {
        segs.push(ProfileSegment::Line {
            start: poly[i],
            end: poly[(i + 1) % poly.len()],
        });
    }
    let to_d = |v: Vec3| dvec3(v.x as f64, v.y as f64, v.z as f64);
    let (t, d) = (to_d(frame.t), to_d(frame.d));
    let n = t.cross(d).normalize();
    let origin: DVec3 = to_d(frame.origin) - n * big;
    let face = crate::profile::build_face_on_plane(
        &Profile::Loop(segs),
        origin.to_array(),
        t.to_array(),
        d.to_array(),
        n.to_array(),
    )?;
    Ok(face.extrude(n * (2.0 * big)).into_shape())
}

/// Garis potong berpanah di tampak induk. `frame = None` → garis lurus dari
/// dua titik pertama permintaan.
fn indicator_for(
    request: &SectionRequest,
    frame: Option<&CutFrame>,
    model_bbox: ([f32; 3], [f32; 3]),
) -> CuttingLineIndicator {
    let (_, right, up) = request.path.parent.camera_vectors();
    let overhang = 8.0;
    let mut points: Vec<[f32; 2]> = request.path.points.clone();
    let mut arrow = vec2(0.0, 1.0);
    if let Some(frame) = frame {
        let o2 = vec2(frame.origin.dot(right), frame.origin.dot(up));
        let t2 = vec2(frame.t.dot(right), frame.t.dot(up));
        let d2 = vec2(frame.d.dot(right), frame.d.dot(up));
        arrow = d2;
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for corner in bbox_corners(model_bbox) {
            let t = (vec2(corner.dot(right), corner.dot(up)) - o2).dot(t2);
            lo = lo.min(t);
            hi = hi.max(t);
        }
        let mut pts = frame.pts.clone();
        let n = pts.len();
        pts[0].0 = pts[0].0.min(lo - overhang);
        pts[n - 1].0 = pts[n - 1].0.max(hi + overhang);
        points = pts
            .iter()
            .map(|(t, d)| (o2 + t2 * *t + d2 * *d).to_array())
            .collect();
    }
    let start = points[0];
    let end = points[points.len() - 1];
    let s = Vec2::from_array(start);
    let e = Vec2::from_array(end);
    let along = (e - s).normalize_or_zero();
    let arrow_len = 6.0;
    CuttingLineIndicator {
        start,
        end,
        arrow_dir: arrow.to_array(),
        arrow1_pos: (s + arrow * arrow_len).to_array(),
        arrow2_pos: (e + arrow * arrow_len).to_array(),
        label1_pos: (s + arrow * (arrow_len + 4.0) - along * 3.5).to_array(),
        label2_pos: (e + arrow * (arrow_len + 4.0) + along * 1.0).to_array(),
        label: request.label.clone(),
        points,
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

