//! Vector Snapshot: "memipihkan" pandangan kamera viewport menjadi garis 2D.
//!
//! # Bedanya dengan [`crate::hlr`]
//!
//! [`crate::hlr`] menghasilkan GAMBAR KERJA: empat tampak ortogonal baku
//! (depan/atas/kanan/isometrik) dalam milimeter, untuk dicetak dan diberi
//! dimensi. Arah pandangnya ditentukan standar, bukan oleh pengguna.
//!
//! Modul ini menghasilkan TANGKAPAN VEKTOR dari sudut pandang yang sedang
//! dipakai pengguna: proyeksi perspektif, koordinat piksel viewport, dan
//! hanya berisi apa yang benar-benar terlihat di layar. Keluarannya untuk
//! ilustrasi, presentasi, dan manual — bukan untuk mendimensi.
//!
//! # Kenapa berbasis mesh, bukan [`crate::hlr_exact`]
//!
//! `hlr_exact` memakai `HLRBRep_Algo` milik OCCT dan menghasilkan kurva
//! analitik yang jauh lebih rapi — tapi bindingnya ortografis, memproses
//! SATU shape per panggilan, dan tidak menerima transformasi penempatan.
//! Tangkapan viewport harus perspektif, mencakup SEMUA body sekaligus, dan
//! menghormati pergeseran explode. Jalur berbasis mesh memenuhi ketiganya
//! dan — yang penting untuk tangkapan — memakai geometri yang sama persis
//! dengan yang dirender di layar, jadi hasilnya cocok dengan yang dilihat
//! pengguna. Konsekuensi yang diterima: busur keluar sebagai polyline.
//!
//! Seluruh modul ini murni aritmetika pada mesh dan rusuk yang SUDAH
//! diekstrak, jadi tidak ada panggilan OCCT sama sekali dan tidak perlu
//! memegang [`crate::lock_kernel`].

use glam::{Mat4, Vec3};

use crate::hlr::{
    append_projected_triangles, append_silhouette_edges, extract_mesh_feature_edges,
    simplify_and_merge_segments, test_occlusion, HlrLineKind, HlrSegment2D,
};
use crate::mesh::KernelMesh;
use crate::projection::{clip_segment_to_halfspace, clip_segment_to_rect, Projector};

/// Sudut pandang kamera viewport pada saat tangkapan diambil.
///
/// Sengaja memakai tipe polos, bukan `ducad_render::OrbitCamera`: crate kernel
/// tidak boleh bergantung pada crate render (yang menarik wgpu). Pemanggil di
/// `ducad-app` yang mengonversinya.
#[derive(Debug, Clone, Copy)]
pub struct SnapshotCamera {
    /// Posisi mata kamera.
    pub eye: Vec3,
    /// Titik yang dipandang; jatuh tepat di tengah tangkapan.
    pub target: Vec3,
    /// Acuan arah atas dunia (Z-up pada DUCAD).
    pub up: Vec3,
    /// Bidang pandang vertikal dalam radian.
    pub fov_y: f32,
    /// Lebar viewport dalam piksel — jadi lebar `viewBox` SVG.
    pub width_px: f32,
    /// Tinggi viewport dalam piksel — jadi tinggi `viewBox` SVG.
    pub height_px: f32,
    /// Jarak bidang dekat; geometri lebih dekat dari ini dipotong.
    pub near: f32,
    /// Pakai proyeksi ortogonal alih-alih perspektif, dengan pembingkaian
    /// yang sama. Berguna untuk tangkapan bergaya teknik dari sudut bebas.
    pub orthographic: bool,
}

impl Default for SnapshotCamera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(150.0, -150.0, 120.0),
            target: Vec3::ZERO,
            up: Vec3::Z,
            fov_y: 45f32.to_radians(),
            width_px: 1600.0,
            height_px: 900.0,
            near: 0.1,
            orthographic: false,
        }
    }
}

/// Satu body yang ikut dalam tangkapan.
#[derive(Clone, Copy)]
pub struct SnapshotBody<'a> {
    /// Rusuk B-rep yang sudah diekstrak (koordinat lokal body). Bila kosong,
    /// tepi lipatan tajam mesh dipakai sebagai gantinya.
    pub edges: &'a [([f32; 3], [f32; 3])],
    /// Mesh segitiga body — penghalang untuk uji garis tersembunyi dan sumber
    /// garis siluet.
    pub mesh: &'a KernelMesh,
    /// Penempatan body di dunia (pergeseran explode, penempatan rakitan).
    pub model: Mat4,
}

impl<'a> SnapshotBody<'a> {
    /// Body tanpa transformasi penempatan.
    pub fn new(edges: &'a [([f32; 3], [f32; 3])], mesh: &'a KernelMesh) -> Self {
        Self {
            edges,
            mesh,
            model: Mat4::IDENTITY,
        }
    }

    /// Body dengan transformasi penempatan.
    pub fn with_model(edges: &'a [([f32; 3], [f32; 3])], mesh: &'a KernelMesh, model: Mat4) -> Self {
        Self { edges, mesh, model }
    }
}

/// Pengaturan tangkapan.
#[derive(Debug, Clone)]
pub struct SnapshotOptions {
    /// Sertakan garis yang terhalang solid lain sebagai [`HlrLineKind::Hidden`].
    pub include_hidden: bool,
    /// Sertakan garis siluet permukaan lengkung. Tanpa ini, silinder yang
    /// dipandang dari samping kehilangan kedua sisinya.
    pub include_silhouette: bool,
    /// Bidang potong Section View yang sedang aktif, dengan konvensi shader
    /// viewport: titik dibuang bila `dot(normal, p) − offset > 0`.
    pub clip_plane: Option<(Vec3, f32)>,
    /// Panjang maksimum satu sub-segmen uji oklusi, dalam piksel. Makin kecil
    /// makin akurat batas antara bagian tampak dan tersembunyi, makin banyak
    /// pula garis yang dihasilkan.
    pub occlusion_step_px: f32,
    /// Segmen yang lebih pendek dari ini (piksel) dibuang.
    pub min_segment_px: f32,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self {
            include_hidden: true,
            include_silhouette: true,
            clip_plane: None,
            occlusion_step_px: 10.0,
            min_segment_px: 0.35,
        }
    }
}

/// Hasil tangkapan: garis 2D dalam koordinat piksel viewport, sumbu y ke bawah.
#[derive(Debug, Clone, Default)]
pub struct VectorSnapshot {
    pub segments: Vec<HlrSegment2D>,
    pub width_px: f32,
    pub height_px: f32,
}

impl VectorSnapshot {
    /// Jumlah segmen dengan jenis garis tertentu.
    pub fn count(&self, kind: HlrLineKind) -> usize {
        self.segments.iter().filter(|s| s.kind == kind).count()
    }

    /// Apakah tangkapan tidak menghasilkan satu garis pun.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

/// Batas atas jumlah sub-segmen per rusuk.
///
/// Rusuk yang membentang melintasi layar pada kamera perspektif bisa
/// menghasilkan panjang piksel yang sangat besar; tanpa batas ini satu rusuk
/// saja bisa meledak jadi puluhan ribu segmen dan membekukan ekspor.
const MAX_OCCLUSION_STEPS: usize = 512;

/// Ambil tangkapan vektor dari sudut pandang `camera`.
///
/// `extra_segments` adalah garis dunia tambahan di luar body — misalnya
/// entitas sketsa aktif — yang ikut diproyeksikan dan diuji oklusinya, tapi
/// tidak menyumbang penghalang.
pub fn extract_vector_snapshot(
    camera: &SnapshotCamera,
    bodies: &[SnapshotBody<'_>],
    extra_segments: &[(Vec3, Vec3)],
    options: &SnapshotOptions,
) -> VectorSnapshot {
    let width = camera.width_px.max(1.0);
    let height = camera.height_px.max(1.0);

    // Toleransi oklusi diskalakan ke ukuran adegan: 0,5 mm yang pas untuk
    // braket 100 mm terlalu kasar untuk komponen 2 mm dan terlalu halus untuk
    // rangka 20 m.
    let scene_extent = scene_extent(bodies, extra_segments);
    let depth_tolerance = (scene_extent * 1e-3).clamp(1e-4, 5.0);

    let Some(projector) = build_projector(camera, depth_tolerance) else {
        return VectorSnapshot {
            segments: Vec::new(),
            width_px: width,
            height_px: height,
        };
    };

    // 1. Kumpulkan penghalang dari SEMUA body sekaligus — sebuah body memang
    //    bisa menyembunyikan rusuk body lain.
    let mut occluders = Vec::new();
    for body in bodies {
        append_projected_triangles(body.mesh, body.model, &projector, &mut occluders);
    }

    // 2. Kumpulkan rusuk dunia yang akan digambar.
    let mut raw: Vec<(Vec3, Vec3, bool)> = Vec::new();
    for body in bodies {
        if body.edges.is_empty() {
            // Body hasil impor mesh (STL) tidak punya rusuk B-rep; pakai tepi
            // lipatan tajamnya supaya tetap ada garis yang digambar.
            for (a, b) in extract_mesh_feature_edges(body.mesh) {
                raw.push((
                    body.model.transform_point3(a),
                    body.model.transform_point3(b),
                    false,
                ));
            }
        } else {
            for (a, b) in body.edges {
                raw.push((
                    body.model.transform_point3(Vec3::from_array(*a)),
                    body.model.transform_point3(Vec3::from_array(*b)),
                    false,
                ));
            }
        }

        if options.include_silhouette {
            let mut sil = Vec::new();
            append_silhouette_edges(body.mesh, body.model, &projector, &mut sil);
            for (a, b) in sil {
                raw.push((a, b, true));
            }
        }
    }
    for (a, b) in extra_segments {
        raw.push((*a, *b, false));
    }

    // 3. Proyeksikan, uji oklusi, potong ke viewport.
    let mut segments = Vec::with_capacity(raw.len());
    for (mut p1, mut p2, is_silhouette) in raw {
        if (p1 - p2).length_squared() < 1e-12 {
            continue;
        }

        if let Some((normal, offset)) = options.clip_plane {
            let Some((c1, c2)) = clip_segment_to_halfspace(p1, p2, normal, offset) else {
                continue;
            };
            p1 = c1;
            p2 = c2;
        }

        let Some((p1, p2)) = projector.clip_to_near_plane(p1, p2) else {
            continue;
        };
        let (Some((a, da)), Some((b, db))) = (projector.project(p1), projector.project(p2)) else {
            continue;
        };

        let len_px = (b - a).length();
        if !len_px.is_finite() || len_px < options.min_segment_px {
            continue;
        }
        // Segmen yang seluruhnya di luar layar tidak perlu diuji oklusinya.
        if clip_segment_to_rect(a, b, width, height).is_none() {
            continue;
        }

        let steps = if len_px > options.occlusion_step_px * 1.5 {
            ((len_px / options.occlusion_step_px).ceil() as usize).clamp(1, MAX_OCCLUSION_STEPS)
        } else {
            1
        };

        for step in 0..steps {
            let t0 = step as f32 / steps as f32;
            let t1 = (step + 1) as f32 / steps as f32;

            let s0 = a.lerp(b, t0);
            let s1 = a.lerp(b, t1);
            // Kedalaman diinterpolasi dengan parameter layar yang sama seperti
            // posisinya — sah untuk kedua mode, lihat `crate::projection`.
            let d0 = da + (db - da) * t0;
            let d1 = da + (db - da) * t1;

            let mid = (s0 + s1) * 0.5;
            let mid_d = (d0 + d1) * 0.5;

            let occluded = test_occlusion(
                mid.x,
                mid.y,
                mid_d,
                &occluders,
                projector.depth_epsilon(mid_d),
            );

            let kind = if occluded {
                if !options.include_hidden {
                    continue;
                }
                HlrLineKind::Hidden
            } else if is_silhouette {
                HlrLineKind::Silhouette
            } else {
                HlrLineKind::Visible
            };

            let Some((c0, c1)) = clip_segment_to_rect(s0, s1, width, height) else {
                continue;
            };
            if (c1 - c0).length() < options.min_segment_px {
                continue;
            }
            segments.push(HlrSegment2D::new(c0, c1, kind));
        }
    }

    VectorSnapshot {
        segments: simplify_and_merge_segments(segments),
        width_px: width,
        height_px: height,
    }
}

fn build_projector(camera: &SnapshotCamera, depth_tolerance: f32) -> Option<Projector> {
    if camera.orthographic {
        Projector::orthographic_screen(
            camera.eye,
            camera.target,
            camera.up,
            camera.fov_y,
            camera.width_px.max(1.0),
            camera.height_px.max(1.0),
            depth_tolerance,
        )
    } else {
        Projector::perspective(
            camera.eye,
            camera.target,
            camera.up,
            camera.fov_y,
            camera.width_px.max(1.0),
            camera.height_px.max(1.0),
            camera.near,
            depth_tolerance,
        )
    }
}

/// Diagonal kotak pembatas seluruh geometri, dipakai menskalakan toleransi.
fn scene_extent(bodies: &[SnapshotBody<'_>], extra_segments: &[(Vec3, Vec3)]) -> f32 {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    let mut ada = false;

    for body in bodies {
        if let Some((bmin, bmax)) = body.mesh.bounding_box() {
            // Delapan sudut, karena `model` boleh memutar.
            for x in [bmin[0], bmax[0]] {
                for y in [bmin[1], bmax[1]] {
                    for z in [bmin[2], bmax[2]] {
                        let p = body.model.transform_point3(Vec3::new(x, y, z));
                        lo = lo.min(p);
                        hi = hi.max(p);
                        ada = true;
                    }
                }
            }
        }
    }
    for (a, b) in extra_segments {
        lo = lo.min(*a).min(*b);
        hi = hi.max(*a).max(*b);
        ada = true;
    }

    if !ada {
        return 100.0;
    }
    let d = (hi - lo).length();
    if d.is_finite() && d > 1e-6 {
        d
    } else {
        100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rusuk lurus dalam koordinat lokal body — bentuk yang sama dengan
    /// `BodyGeometry::edge_lines` di `ducad-app`, sumber nyata `SnapshotBody`.
    type RusukLokal = Vec<([f32; 3], [f32; 3])>;

    /// Kubus berpusat di titik asal dengan sisi `s`, lengkap 12 segitiga dan
    /// 12 rusuk.
    fn kubus(s: f32) -> (KernelMesh, RusukLokal) {
        let h = s * 0.5;
        let v = [
            [-h, -h, -h],
            [h, -h, -h],
            [h, h, -h],
            [-h, h, -h],
            [-h, -h, h],
            [h, -h, h],
            [h, h, h],
            [-h, h, h],
        ];
        // Lilitan berlawanan arah jarum jam dilihat dari LUAR kubus, supaya
        // normalnya menghadap keluar.
        let faces: [[usize; 4]; 6] = [
            [0, 3, 2, 1], // bawah (-Z)
            [4, 5, 6, 7], // atas (+Z)
            [0, 1, 5, 4], // depan (-Y)
            [2, 3, 7, 6], // belakang (+Y)
            [1, 2, 6, 5], // kanan (+X)
            [3, 0, 4, 7], // kiri (-X)
        ];
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();
        for f in faces {
            let base = positions.len() as u32;
            for i in f {
                positions.push(v[i]);
                normals.push([0.0, 0.0, 1.0]);
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let mesh = KernelMesh {
            positions,
            normals,
            indices,
            face_ranges: Vec::new(),
        };

        let rusuk_idx = [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ];
        let edges = rusuk_idx.iter().map(|(a, b)| (v[*a], v[*b])).collect();
        (mesh, edges)
    }

    fn kamera_depan(orthographic: bool) -> SnapshotCamera {
        SnapshotCamera {
            eye: Vec3::new(0.0, -200.0, 0.0),
            target: Vec3::ZERO,
            up: Vec3::Z,
            fov_y: 45f32.to_radians(),
            width_px: 800.0,
            height_px: 600.0,
            near: 0.1,
            orthographic,
        }
    }

    /// Panjang total (piksel) semua segmen tangkapan.
    fn panjang_total(snap: &VectorSnapshot) -> f32 {
        snap.segments.iter().map(|s| s.length()).sum()
    }

    /// Tangkapan berisi SATU garis dunia sepanjang 40 mm sejajar sumbu X pada
    /// jarak `y`. Tanpa body, jadi tidak ada oklusi yang mengaburkan ukuran.
    fn panjang_garis_pada_kedalaman(orthographic: bool, y: f32) -> f32 {
        let snap = extract_vector_snapshot(
            &kamera_depan(orthographic),
            &[],
            &[(Vec3::new(-20.0, y, 0.0), Vec3::new(20.0, y, 0.0))],
            &SnapshotOptions::default(),
        );
        panjang_total(&snap)
    }

    /// Bentang horizontal (piksel) dari semua segmen berjenis `kind`.
    fn bentang_x(snap: &VectorSnapshot, kind: HlrLineKind) -> f32 {
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for s in snap.segments.iter().filter(|s| s.kind == kind) {
            lo = lo.min(s.start[0]).min(s.end[0]);
            hi = hi.max(s.start[0]).max(s.end[0]);
        }
        hi - lo
    }

    #[test]
    fn snapshot_of_a_cube_has_visible_and_hidden_edges() {
        let (mesh, edges) = kubus(50.0);
        let bodies = [SnapshotBody::new(&edges, &mesh)];
        let snap = extract_vector_snapshot(
            &kamera_depan(false),
            &bodies,
            &[],
            &SnapshotOptions::default(),
        );

        assert!(!snap.is_empty(), "kubus harus menghasilkan garis");
        assert!(
            snap.count(HlrLineKind::Visible) > 0,
            "harus ada garis tampak"
        );
        assert!(
            snap.count(HlrLineKind::Hidden) > 0,
            "rusuk sisi jauh kubus harus tersembunyi"
        );
        assert_eq!(snap.width_px, 800.0);
        assert_eq!(snap.height_px, 600.0);
    }

    #[test]
    fn hidden_edges_are_dropped_when_not_requested() {
        let (mesh, edges) = kubus(50.0);
        let bodies = [SnapshotBody::new(&edges, &mesh)];
        let opts = SnapshotOptions {
            include_hidden: false,
            ..Default::default()
        };
        let snap = extract_vector_snapshot(&kamera_depan(false), &bodies, &[], &opts);

        assert_eq!(snap.count(HlrLineKind::Hidden), 0);
        assert!(snap.count(HlrLineKind::Visible) > 0);
    }

    #[test]
    fn all_segments_stay_inside_the_viewport() {
        let (mesh, edges) = kubus(400.0); // jauh lebih besar dari bingkai layar
        let bodies = [SnapshotBody::new(&edges, &mesh)];
        let snap = extract_vector_snapshot(
            &kamera_depan(false),
            &bodies,
            &[],
            &SnapshotOptions::default(),
        );

        assert!(!snap.is_empty());
        for s in &snap.segments {
            for p in [s.start, s.end] {
                assert!(
                    p[0] >= -0.01 && p[0] <= 800.01,
                    "x di luar viewport: {}",
                    p[0]
                );
                assert!(
                    p[1] >= -0.01 && p[1] <= 600.01,
                    "y di luar viewport: {}",
                    p[1]
                );
            }
        }
    }

    #[test]
    fn perspective_draws_nearer_geometry_larger() {
        let dekat = panjang_garis_pada_kedalaman(false, -60.0);
        let jauh = panjang_garis_pada_kedalaman(false, 60.0);
        assert!(dekat > 0.0 && jauh > 0.0);
        assert!(
            dekat > jauh * 1.5,
            "garis dekat {dekat} px harus jauh lebih panjang dari garis jauh {jauh} px"
        );
    }

    #[test]
    fn orthographic_mode_ignores_depth() {
        let dekat = panjang_garis_pada_kedalaman(true, -60.0);
        let jauh = panjang_garis_pada_kedalaman(true, 60.0);
        assert!(dekat > 0.0 && jauh > 0.0);
        assert!(
            (dekat - jauh).abs() < 0.5,
            "ortogonal harus mengabaikan kedalaman: {dekat} px vs {jauh} px"
        );
    }

    #[test]
    fn a_cube_seen_head_on_hides_everything_behind_its_near_face() {
        // Dipandang tegak lurus, HANYA rusuk sisi depan yang tampak: empat
        // rusuk penghubung dan empat rusuk sisi belakang semuanya jatuh di
        // dalam siluet sisi depan, jadi harus terklasifikasi tersembunyi.
        let (mesh, edges) = kubus(80.0);
        let opts = SnapshotOptions {
            include_silhouette: false,
            ..Default::default()
        };
        let snap = extract_vector_snapshot(
            &kamera_depan(false),
            &[SnapshotBody::new(&edges, &mesh)],
            &[],
            &opts,
        );

        // Fokus 724,3 px kali setengah-sisi 40 mm dibagi jarak 160 mm, dua sisi.
        let sisi_depan_px = 362.1;
        let tampak = bentang_x(&snap, HlrLineKind::Visible);
        let tersembunyi = bentang_x(&snap, HlrLineKind::Hidden);
        assert!(
            (tampak - sisi_depan_px).abs() < 2.0,
            "bentang tampak {tampak} px harus selebar sisi depan"
        );
        assert!(
            tersembunyi <= sisi_depan_px + 2.0,
            "garis tersembunyi {tersembunyi} px tidak boleh keluar dari siluet"
        );
        assert!(snap.count(HlrLineKind::Hidden) > 0);
    }

    #[test]
    fn body_transform_shifts_the_result() {
        let (mesh, edges) = kubus(40.0);
        let asli = extract_vector_snapshot(
            &kamera_depan(true),
            &[SnapshotBody::new(&edges, &mesh)],
            &[],
            &SnapshotOptions::default(),
        );
        let geser = extract_vector_snapshot(
            &kamera_depan(true),
            &[SnapshotBody::with_model(
                &edges,
                &mesh,
                Mat4::from_translation(Vec3::new(30.0, 0.0, 0.0)),
            )],
            &[],
            &SnapshotOptions::default(),
        );

        let pusat_x = |s: &VectorSnapshot| -> f32 {
            let n = s.segments.len() as f32;
            s.segments
                .iter()
                .map(|g| (g.start[0] + g.end[0]) * 0.5)
                .sum::<f32>()
                / n
        };
        assert!(!asli.is_empty() && !geser.is_empty());
        assert!(
            pusat_x(&geser) > pusat_x(&asli) + 5.0,
            "explode ke +X harus menggeser gambar ke kanan"
        );
    }

    #[test]
    fn geometry_behind_the_camera_is_dropped() {
        let (mesh, edges) = kubus(40.0);
        let mut cam = kamera_depan(false);
        // Kamera berada di belakang kubus dan memandang MENJAUH darinya.
        cam.eye = Vec3::new(0.0, 200.0, 0.0);
        cam.target = Vec3::new(0.0, 400.0, 0.0);
        let snap = extract_vector_snapshot(
            &cam,
            &[SnapshotBody::new(&edges, &mesh)],
            &[],
            &SnapshotOptions::default(),
        );
        assert!(
            snap.is_empty(),
            "kubus di belakang kamera tidak boleh tergambar"
        );
    }

    #[test]
    fn extra_segments_are_projected_too() {
        let (mesh, edges) = kubus(10.0);
        let tanpa = extract_vector_snapshot(
            &kamera_depan(true),
            &[SnapshotBody::new(&edges, &mesh)],
            &[],
            &SnapshotOptions::default(),
        );
        let dengan = extract_vector_snapshot(
            &kamera_depan(true),
            &[SnapshotBody::new(&edges, &mesh)],
            &[(Vec3::new(-60.0, 0.0, 40.0), Vec3::new(60.0, 0.0, 40.0))],
            &SnapshotOptions::default(),
        );
        assert!(dengan.segments.len() > tanpa.segments.len());
    }

    #[test]
    fn clip_plane_removes_the_cut_half() {
        let (mesh, edges) = kubus(60.0);
        let bodies = [SnapshotBody::new(&edges, &mesh)];
        let penuh = extract_vector_snapshot(
            &kamera_depan(true),
            &bodies,
            &[],
            &SnapshotOptions::default(),
        );
        let opts = SnapshotOptions {
            // Buang seluruh separuh +X.
            clip_plane: Some((Vec3::X, 0.0)),
            ..Default::default()
        };
        let terpotong = extract_vector_snapshot(&kamera_depan(true), &bodies, &[], &opts);

        assert!(!terpotong.is_empty());
        assert!(terpotong.segments.len() < penuh.segments.len());
        let kanan_max = terpotong
            .segments
            .iter()
            .map(|s| s.start[0].max(s.end[0]))
            .fold(f32::MIN, f32::max);
        // Bidang potong melewati pusat, jadi tidak boleh ada garis jauh di
        // kanan tengah layar (400 px).
        assert!(
            kanan_max < 405.0,
            "masih ada garis di sisi terpotong: {kanan_max}"
        );
    }

    #[test]
    fn empty_input_yields_an_empty_snapshot_with_viewport_size() {
        let snap =
            extract_vector_snapshot(&kamera_depan(false), &[], &[], &SnapshotOptions::default());
        assert!(snap.is_empty());
        assert_eq!(snap.width_px, 800.0);
        assert_eq!(snap.height_px, 600.0);
    }

    #[test]
    fn degenerate_camera_yields_empty_instead_of_panicking() {
        let (mesh, edges) = kubus(40.0);
        let mut cam = kamera_depan(false);
        cam.target = cam.eye; // arah pandang tidak terdefinisi
        let snap = extract_vector_snapshot(
            &cam,
            &[SnapshotBody::new(&edges, &mesh)],
            &[],
            &SnapshotOptions::default(),
        );
        assert!(snap.is_empty());
    }
}
