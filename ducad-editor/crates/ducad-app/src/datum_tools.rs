//! Geometri konstruksi datum di GUI (PLAN_PARITAS_SOLIDCRAFT Fase 1A):
//!
//! - 1.2 sumbu datum (dua titik, edge, normal face, sumbu silinder, potongan
//!   dua bidang) dan 1.3 titik datum (vertex, pusat lingkaran/bola,
//!   edge ∩ bidang, dua edge, tiga bidang, sepanjang lintasan);
//! - 1.4 mode bidang datum baru (Midplane, Perpendicular, Tangent, AlongPath,
//!   TwoEdges);
//! - 1.5 penyimpanan (berkas `.ducad`, field `datums`), tampil/sembunyi,
//!   ganti nama, hapus;
//! - 1.6 pemakaian sumbu datum di Revolve, Circular Pattern, dan Mate
//!   Concentric.
//!
//! Semua rumus memakai `ducad_engine::datum` — sumber yang sama dengan op
//! `construct_axis` / `construct_point` / `PlaneSpec` engine, jadi GUI, MCP,
//! dan CLI menghasilkan geometri identik. Masukan degenerat (bidang sejajar,
//! titik berimpit, NaN) menjadi pesan status, tidak pernah panik (ADR 0009).

use ducad_engine::datum::{self, Axis, EdgeCurve};
use ducad_engine::{OpError, PlaneFrame};
use ducad_kernel::{EdgeKind, SurfaceKind};
use ducad_render::{LineVertex, PlaneKind, SketchPlane};
use ducad_sketch::PlaneRef;
use ducad_ui::{DatumAxisMode, DatumKind, DatumPlaneMode, DatumPointMode};
use glam::{DVec3, Vec3};
use slotmap::Key;

use crate::app::DuCADApp;
use crate::types::ToolKind;

/// Sumbu datum GUI.
#[derive(Debug, Clone, PartialEq)]
pub struct DatumAxis {
    pub id: u32,
    pub name: String,
    pub axis: Axis,
    pub visible: bool,
}

/// Titik datum GUI.
#[derive(Debug, Clone, PartialEq)]
pub struct DatumPoint {
    pub id: u32,
    pub name: String,
    pub pos: DVec3,
    pub visible: bool,
}

/// Satu geometri yang dipilih pengguna di viewport untuk membangun datum.
#[derive(Debug, Clone, PartialEq)]
pub enum DatumPick {
    /// Titik bebas (vertex, titik di edge/face, titik datum).
    Point(DVec3),
    /// Edge body (atau sumbu datum sebagai garis) + titik klik di atasnya.
    Edge { curve: EdgeCurve, hit: DVec3 },
    /// Face datar atau bidang referensi (standar/datum).
    Plane(PlaneFrame),
    /// Face silinder/kerucut.
    Cylinder { axis: Axis, radius: f64 },
    /// Pusat face bola.
    Sphere(DVec3),
}

impl DatumPick {
    fn kind_label(&self) -> String {
        match self {
            DatumPick::Point(_) => ducad_i18n::t!("datum-pick-point"),
            DatumPick::Edge { .. } => ducad_i18n::t!("datum-pick-edge"),
            DatumPick::Plane(_) => ducad_i18n::t!("datum-pick-plane"),
            DatumPick::Cylinder { .. } => ducad_i18n::t!("datum-pick-cylinder"),
            DatumPick::Sphere(_) => ducad_i18n::t!("datum-pick-sphere"),
        }
    }
}

/// Jenis geometri yang dibutuhkan pilihan berikutnya.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickNeed {
    /// Titik apa saja (vertex/edge/face/titik datum).
    Point,
    /// Hanya vertex body atau titik datum.
    Vertex,
    Edge,
    Plane,
    Cylinder,
    /// Edge lingkaran/busur atau face bola.
    CenterSource,
}

impl PickNeed {
    fn label(self) -> String {
        match self {
            PickNeed::Point => ducad_i18n::t!("datum-pick-point"),
            PickNeed::Vertex => ducad_i18n::t!("datum-pick-vertex"),
            PickNeed::Edge => ducad_i18n::t!("datum-pick-edge"),
            PickNeed::Plane => ducad_i18n::t!("datum-pick-plane"),
            PickNeed::Cylinder => ducad_i18n::t!("datum-pick-cylinder"),
            PickNeed::CenterSource => ducad_i18n::t!("datum-pick-center-source"),
        }
    }
}

/// Apa yang sedang dibuat tool Datum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatumTarget {
    Plane(DatumPlaneMode),
    Axis(DatumAxisMode),
    Point(DatumPointMode),
}

impl DatumTarget {
    /// Urutan pilihan yang dibutuhkan. Kosong = mode bidang lama (tidak
    /// lewat pilihan geometri).
    pub fn needs(self) -> &'static [PickNeed] {
        use PickNeed::*;
        match self {
            DatumTarget::Plane(m) => match m {
                DatumPlaneMode::Midplane => &[Plane, Plane],
                DatumPlaneMode::Perpendicular | DatumPlaneMode::AlongPath => &[Edge],
                DatumPlaneMode::Tangent => &[Cylinder],
                DatumPlaneMode::TwoEdges => &[Edge, Edge],
                DatumPlaneMode::Offset | DatumPlaneMode::Angled | DatumPlaneMode::ThreePoints => &[],
            },
            DatumTarget::Axis(m) => match m {
                DatumAxisMode::TwoPoints => &[Point, Point],
                DatumAxisMode::Edge => &[Edge],
                DatumAxisMode::NormalToFace => &[Plane],
                DatumAxisMode::ThroughCylinder => &[Cylinder],
                DatumAxisMode::TwoPlanes => &[Plane, Plane],
            },
            DatumTarget::Point(m) => match m {
                DatumPointMode::Vertex => &[Vertex],
                DatumPointMode::Center => &[CenterSource],
                DatumPointMode::EdgePlane => &[Edge, Plane],
                DatumPointMode::TwoEdges => &[Edge, Edge],
                DatumPointMode::ThreePlanes => &[Plane, Plane, Plane],
                DatumPointMode::AlongPath => &[Edge],
            },
        }
    }

    pub fn label(self) -> String {
        match self {
            DatumTarget::Plane(m) => m.label(),
            DatumTarget::Axis(m) => m.label(),
            DatumTarget::Point(m) => m.label(),
        }
    }

    /// Mode ini memakai pecahan panjang edge.
    pub fn needs_fraction(self) -> bool {
        matches!(
            self,
            DatumTarget::Plane(DatumPlaneMode::AlongPath) | DatumTarget::Point(DatumPointMode::AlongPath)
        )
    }
}

/// State tool & dokumen datum (sumbu/titik; bidang tetap di
/// `DuCADApp::datum_planes` yang sudah ada).
pub struct DatumState {
    pub axes: Vec<DatumAxis>,
    pub points: Vec<DatumPoint>,
    /// Tampilan kamera bernama (Paritas 3.13), disimpan bersama datum.
    pub views: Vec<ducad_io::native::NativeNamedView>,
    pub axis_counter: u32,
    pub point_counter: u32,
    /// Jenis datum aktif di tool Datum.
    pub kind: DatumKind,
    pub axis_mode: DatumAxisMode,
    pub point_mode: DatumPointMode,
    /// Pilihan geometri yang sudah terkumpul untuk mode aktif.
    pub picks: Vec<DatumPick>,
    /// Pecahan panjang edge (0..1) untuk mode "sepanjang lintasan".
    pub fraction_input: String,
    /// Sumbu datum yang dipilih untuk Revolve / Pattern melingkar / Mate.
    pub revolve_axis: Option<u32>,
    pub pattern_axis: Option<u32>,
    pub mate_axis: Option<u32>,
}

impl Default for DatumState {
    fn default() -> Self {
        Self {
            axes: Vec::new(),
            points: Vec::new(),
            views: Vec::new(),
            axis_counter: 0,
            point_counter: 0,
            kind: DatumKind::Plane,
            axis_mode: DatumAxisMode::default(),
            point_mode: DatumPointMode::default(),
            picks: Vec::new(),
            fraction_input: "0.5".to_string(),
            revolve_axis: None,
            pattern_axis: None,
            mate_axis: None,
        }
    }
}

impl DatumState {
    pub fn axis(&self, id: u32) -> Option<&DatumAxis> {
        self.axes.iter().find(|a| a.id == id)
    }

    pub fn point(&self, id: u32) -> Option<&DatumPoint> {
        self.points.iter().find(|p| p.id == id)
    }

    /// Pecahan dari isian; `None` bila bukan angka.
    pub fn fraction(&self) -> Option<f64> {
        self.fraction_input.trim().parse::<f64>().ok()
    }

    /// Kosongkan seluruh datum sumbu/titik (dokumen baru / buka berkas).
    pub fn clear_document(&mut self) {
        self.views.clear();
        self.axes.clear();
        self.points.clear();
        self.axis_counter = 0;
        self.point_counter = 0;
        self.picks.clear();
        self.revolve_axis = None;
        self.pattern_axis = None;
        self.mate_axis = None;
    }
}

// ---------------------------------------------------------------------
// Rumus murni (teruji tanpa GUI).
// ---------------------------------------------------------------------

/// Pesan pengguna dari `OpError` (pesan + saran).
pub fn op_msg(e: &OpError) -> String {
    match &e.hint {
        Some(h) => format!("{} ({h})", e.message),
        None => e.message.clone(),
    }
}

fn need_picks(picks: &[DatumPick], n: usize) -> Result<(), String> {
    if picks.len() < n {
        Err(ducad_i18n::t!("datum-err-need-picks", have = picks.len(), need = n))
    } else {
        Ok(())
    }
}

fn as_plane(p: &DatumPick) -> Result<&PlaneFrame, String> {
    match p {
        DatumPick::Plane(f) => Ok(f),
        other => Err(ducad_i18n::t!("datum-err-wrong-pick", got = other.kind_label())),
    }
}

fn as_edge(p: &DatumPick) -> Result<(&EdgeCurve, DVec3), String> {
    match p {
        DatumPick::Edge { curve, hit } => Ok((curve, *hit)),
        other => Err(ducad_i18n::t!("datum-err-wrong-pick", got = other.kind_label())),
    }
}

fn as_point(p: &DatumPick) -> Result<DVec3, String> {
    match p {
        DatumPick::Point(v) => Ok(*v),
        other => Err(ducad_i18n::t!("datum-err-wrong-pick", got = other.kind_label())),
    }
}

fn line_ends(c: &EdgeCurve) -> Result<(DVec3, DVec3), String> {
    match *c {
        EdgeCurve::Line { start, end } => Ok((start, end)),
        EdgeCurve::Arc { .. } => Err(ducad_i18n::t!("datum-err-need-straight")),
    }
}

fn check_fraction(f: Option<f64>) -> Result<f64, String> {
    match f {
        Some(v) if v.is_finite() && (0.0..=1.0).contains(&v) => Ok(v),
        _ => Err(ducad_i18n::t!("datum-err-fraction")),
    }
}

/// Parameter (0..1) titik terdekat ke `p` pada kurva (sampling rapat).
pub fn param_on_curve(c: &EdgeCurve, p: DVec3) -> f64 {
    const N: usize = 256;
    let mut best = (0.0, f64::INFINITY);
    for i in 0..=N {
        let t = i as f64 / N as f64;
        let d = (c.point_at(t) - p).length_squared();
        if d < best.1 {
            best = (t, d);
        }
    }
    best.0
}

/// Sumbu datum dari pilihan (Paritas 1.2).
pub fn compute_axis(mode: DatumAxisMode, picks: &[DatumPick]) -> Result<Axis, String> {
    need_picks(picks, DatumTarget::Axis(mode).needs().len())?;
    let r = match mode {
        DatumAxisMode::TwoPoints => datum::axis_two_points(as_point(&picks[0])?, as_point(&picks[1])?),
        DatumAxisMode::Edge => as_edge(&picks[0])?.0.as_axis(),
        DatumAxisMode::NormalToFace => datum::axis_normal_to_plane(as_plane(&picks[0])?, None),
        DatumAxisMode::ThroughCylinder => match &picks[0] {
            DatumPick::Cylinder { axis, .. } => Axis::new(axis.origin_v(), axis.dir_v()),
            other => {
                return Err(ducad_i18n::t!("datum-err-wrong-pick", got = other.kind_label()))
            }
        },
        DatumAxisMode::TwoPlanes => datum::axis_two_planes(as_plane(&picks[0])?, as_plane(&picks[1])?),
    };
    r.map_err(|e| op_msg(&e))
}

/// Titik datum dari pilihan (Paritas 1.3).
pub fn compute_point(mode: DatumPointMode, picks: &[DatumPick], fraction: Option<f64>) -> Result<DVec3, String> {
    need_picks(picks, DatumTarget::Point(mode).needs().len())?;
    let p = match mode {
        DatumPointMode::Vertex => as_point(&picks[0])?,
        DatumPointMode::Center => match &picks[0] {
            DatumPick::Edge { curve: EdgeCurve::Arc { center, .. }, .. } => *center,
            DatumPick::Sphere(c) => *c,
            DatumPick::Edge { .. } => return Err(ducad_i18n::t!("datum-err-need-circle")),
            other => {
                return Err(ducad_i18n::t!("datum-err-wrong-pick", got = other.kind_label()))
            }
        },
        DatumPointMode::EdgePlane => {
            let (c, _) = as_edge(&picks[0])?;
            c.intersect_plane(as_plane(&picks[1])?).map_err(|e| op_msg(&e))?
        }
        DatumPointMode::TwoEdges => {
            let a = as_edge(&picks[0])?.0.as_axis().map_err(|e| op_msg(&e))?;
            let b = as_edge(&picks[1])?.0.as_axis().map_err(|e| op_msg(&e))?;
            datum::point_two_lines(&a, &b).map_err(|e| op_msg(&e))?.0
        }
        DatumPointMode::ThreePlanes => {
            datum::point_three_planes(as_plane(&picks[0])?, as_plane(&picks[1])?, as_plane(&picks[2])?)
                .map_err(|e| op_msg(&e))?
        }
        DatumPointMode::AlongPath => {
            let t = check_fraction(fraction)?;
            as_edge(&picks[0])?.0.point_at(t)
        }
    };
    if p.is_finite() {
        Ok(p)
    } else {
        Err(ducad_i18n::t!("datum-err-not-finite"))
    }
}

/// Bidang datum Paritas 1.4 dari pilihan. Mode lama → error.
pub fn compute_plane(
    mode: DatumPlaneMode,
    picks: &[DatumPick],
    angle_deg: f64,
    fraction: Option<f64>,
) -> Result<PlaneFrame, String> {
    need_picks(picks, DatumTarget::Plane(mode).needs().len())?;
    let r = match mode {
        DatumPlaneMode::Midplane => datum::plane_midplane(as_plane(&picks[0])?, as_plane(&picks[1])?),
        DatumPlaneMode::Perpendicular => {
            let (c, hit) = as_edge(&picks[0])?;
            let t = param_on_curve(c, hit);
            datum::plane_perpendicular(c.tangent_at(t), c.point_at(t))
        }
        DatumPlaneMode::Tangent => match &picks[0] {
            DatumPick::Cylinder { axis, radius } => datum::plane_tangent_cylinder(axis, *radius, angle_deg),
            other => {
                return Err(ducad_i18n::t!("datum-err-wrong-pick", got = other.kind_label()))
            }
        },
        DatumPlaneMode::AlongPath => {
            let t = check_fraction(fraction)?;
            datum::plane_along_curve(as_edge(&picks[0])?.0, t)
        }
        DatumPlaneMode::TwoEdges => {
            let (a0, a1) = line_ends(as_edge(&picks[0])?.0)?;
            let (b0, b1) = line_ends(as_edge(&picks[1])?.0)?;
            datum::plane_through_lines(a0, a1, b0, b1)
        }
        DatumPlaneMode::Offset | DatumPlaneMode::Angled | DatumPlaneMode::ThreePoints => {
            return Err(ducad_i18n::t!("datum-err-wrong-pick", got = mode.label()))
        }
    };
    r.map_err(|e| op_msg(&e))
}

/// `PlaneFrame` (f64, engine) → `SketchPlane` (f32, render). Jenisnya
/// diisi `create_datum_plane`.
pub fn sketch_plane_from_frame(f: &PlaneFrame) -> SketchPlane {
    let v = |a: [f64; 3]| Vec3::new(a[0] as f32, a[1] as f32, a[2] as f32);
    SketchPlane::custom(0, v(f.origin), v(f.u_axis), v(f.v_axis), v(f.normal))
}

/// Garis tak hingga sumbu sebagai `EdgeCurve` (untuk dipilih seperti edge).
fn axis_as_curve(a: &Axis, half: f64) -> EdgeCurve {
    EdgeCurve::Line {
        start: a.origin_v() - a.dir_v() * half,
        end: a.origin_v() + a.dir_v() * half,
    }
}

fn to_v3(p: DVec3) -> Vec3 {
    Vec3::new(p.x as f32, p.y as f32, p.z as f32)
}

fn tuple_v(p: (f64, f64, f64)) -> DVec3 {
    DVec3::new(p.0, p.1, p.2)
}

/// Jarak titik `p` ke ruas `a`–`b` di layar.
fn seg_dist(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-6 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

/// Setengah panjang garis sumbu datum yang digambar (mm).
const AXIS_HALF_LEN: f64 = 150.0;

/// Edge eksak `shape` yang paling dekat ke titik `hit` (titik pick dari
/// polyline tesselasi): dicocokkan ke `enumerate_edges` lewat jarak titik ke
/// kurva. `None` = tidak ada edge dalam `tol`; `Err` = yang terdekat
/// berjenis kurva bebas (tidak didukung datum).
pub fn edge_curve_near(shape: &ducad_kernel::KernelShape, hit: DVec3, tol: f64) -> Option<Result<EdgeCurve, String>> {
    if !hit.is_finite() {
        return None;
    }
    let edges = ducad_kernel::enumerate_edges(shape);
    let faces = ducad_kernel::enumerate_faces(shape);
    let mut best: Option<(EdgeCurve, f64)> = None;
    let mut unsupported = false;
    for e in &edges {
        match EdgeCurve::from_edge(e, &faces) {
            Ok(c) => {
                let d = (c.point_at(param_on_curve(&c, hit)) - hit).length();
                if best.as_ref().is_none_or(|b| d < b.1) {
                    best = Some((c, d));
                }
            }
            Err(_) => unsupported |= e.kind == EdgeKind::Other,
        }
    }
    match best {
        Some((c, d)) if d < tol => Some(Ok(c)),
        _ if unsupported => Some(Err(ducad_i18n::t!("datum-err-edge-unsupported"))),
        _ => None,
    }
}

/// Pilihan silinder dari hasil pick face (sumbu + radius). `None` bila face
/// bukan silinder/kerucut atau datanya tidak lengkap.
pub fn cylinder_pick(shape: &ducad_kernel::KernelShape, hit: &ducad_kernel::FaceHit) -> Option<DatumPick> {
    if !matches!(hit.surface_kind, SurfaceKind::Cylinder | SurfaceKind::Cone) {
        return None;
    }
    let face = hit
        .face_index
        .and_then(|i| ducad_kernel::enumerate_faces(shape).into_iter().nth(i));
    let (origin, dir) = match (hit.radial_axis, face.as_ref().and_then(|f| f.axis)) {
        (Some(ra), _) if tuple_v(ra.dir).length() > 1e-9 => (tuple_v(ra.origin), tuple_v(ra.dir)),
        (_, Some((o, d))) => (DVec3::from_array(o), DVec3::from_array(d)),
        _ => return None,
    };
    let radius = hit.surface_radius.or(face.and_then(|f| f.radius))?;
    let axis = Axis::new(origin, dir).ok()?;
    Some(DatumPick::Cylinder { axis, radius })
}

// ---------------------------------------------------------------------
// Integrasi DuCADApp.
// ---------------------------------------------------------------------

impl DuCADApp {
    /// Target datum aktif tool Datum.
    pub fn datum_target(&self) -> DatumTarget {
        match self.datums.kind {
            DatumKind::Plane => DatumTarget::Plane(self.datum_mode),
            DatumKind::Axis => DatumTarget::Axis(self.datums.axis_mode),
            DatumKind::Point => DatumTarget::Point(self.datums.point_mode),
        }
    }

    /// Buka tool Datum dengan jenis tertentu.
    pub fn open_datum_tool(&mut self, kind: DatumKind) {
        self.set_tool(ToolKind::DatumPlane);
        self.datums.kind = kind;
        self.datums.picks.clear();
    }

    /// Ringkasan status pilihan: "Edge 1/2 — berikutnya: Bidang".
    pub fn datum_pick_status(&self) -> (String, bool) {
        let needs = self.datum_target().needs();
        let have = self.datums.picks.len().min(needs.len());
        match needs.get(have) {
            Some(next) => (
                ducad_i18n::t!(
                    "datum-pick-status",
                    have = have,
                    need = needs.len(),
                    next = next.label()
                ),
                false,
            ),
            None => (ducad_i18n::t!("datum-pick-ready", need = needs.len()), true),
        }
    }

    /// Tangani klik viewport untuk tool Datum. `true` = klik dikonsumsi
    /// (mode berbasis pilihan geometri); `false` = biarkan mode bidang lama.
    pub fn datum_handle_click(&mut self, rect: egui::Rect, pos: egui::Pos2) -> bool {
        let target = self.datum_target();
        let needs = target.needs();
        if needs.is_empty() {
            return false;
        }
        // Sudah lengkap → klik berikutnya memulai pilihan baru.
        if self.datums.picks.len() >= needs.len() {
            self.datums.picks.clear();
        }
        let need = needs[self.datums.picks.len()];
        match self.datum_pick_at(rect, pos, need) {
            Some(pick) => {
                let label = pick.kind_label();
                self.datums.picks.push(pick);
                let (status, _) = self.datum_pick_status();
                self.model_status = Some(format!("{label}: {status}"));
            }
            None => {
                self.model_status = Some(ducad_i18n::t!("datum-pick-miss", need = need.label()));
            }
        }
        true
    }

    /// Titik datum terlihat yang dekat kursor (layar).
    fn datum_point_near(&self, rect: egui::Rect, pos: egui::Pos2) -> Option<DVec3> {
        self.datums
            .points
            .iter()
            .filter(|p| p.visible)
            .filter_map(|p| {
                let sp = crate::viewport::world_to_screen_pos(&self.camera, rect, to_v3(p.pos))?;
                let d = (sp - pos).length();
                (d <= 12.0).then_some((p.pos, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(p, _)| p)
    }

    /// Sumbu datum terlihat yang dekat kursor (layar).
    fn datum_axis_near(&self, rect: egui::Rect, pos: egui::Pos2) -> Option<(EdgeCurve, DVec3)> {
        let mut best: Option<(EdgeCurve, DVec3, f32)> = None;
        for a in self.datums.axes.iter().filter(|a| a.visible) {
            let curve = axis_as_curve(&a.axis, AXIS_HALF_LEN);
            let start = a.axis.origin_v() - a.axis.dir_v() * AXIS_HALF_LEN;
            let end = a.axis.origin_v() + a.axis.dir_v() * AXIS_HALF_LEN;
            let (Some(sa), Some(sb)) = (
                crate::viewport::world_to_screen_pos(&self.camera, rect, to_v3(start)),
                crate::viewport::world_to_screen_pos(&self.camera, rect, to_v3(end)),
            ) else {
                continue;
            };
            let d = seg_dist(pos, sa, sb);
            if d <= 8.0 && best.as_ref().is_none_or(|b| d < b.2) {
                // Titik klik: proyeksi ke garis di parameter layar.
                let ab = sb - sa;
                let t = if ab.length_sq() > 1e-6 {
                    ((pos - sa).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) as f64
                } else {
                    0.5
                };
                best = Some((curve, curve.point_at(t), d));
            }
        }
        best.map(|(c, h, _)| (c, h))
    }

    /// Edge body di bawah kursor sebagai `EdgeCurve` eksak (dicocokkan ke
    /// `enumerate_edges` lewat jarak titik klik ke kurva).
    fn body_edge_curve_at(&self, rect: egui::Rect, pos: egui::Pos2) -> Option<Result<(EdgeCurve, DVec3), String>> {
        let (body, _ray, hit) = self.pick_body_edge_at_cursor(rect, pos)?;
        let hit = tuple_v(hit);
        let geo = self.model.geometry.get(body)?;
        let tol = 0.5_f64.max(crate::viewport::pixel_tolerance_to_world(&self.camera, rect) * 20.0);
        edge_curve_near(&geo.shape, hit, tol).map(|r| r.map(|c| (c, hit)))
    }

    /// Ambil satu geometri sesuai kebutuhan `need` di posisi layar `pos`.
    pub fn datum_pick_at(&mut self, rect: egui::Rect, pos: egui::Pos2, need: PickNeed) -> Option<DatumPick> {
        match need {
            PickNeed::Point | PickNeed::Vertex => {
                if let Some(p) = self.datum_point_near(rect, pos) {
                    return Some(DatumPick::Point(p));
                }
                if let Some((_, _, v)) = self.pick_body_vertex_at_cursor(rect, pos) {
                    return Some(DatumPick::Point(tuple_v(v)));
                }
                if need == PickNeed::Vertex {
                    return None;
                }
                if let Some((_, _, e)) = self.pick_body_edge_at_cursor(rect, pos) {
                    return Some(DatumPick::Point(tuple_v(e)));
                }
                self.pick_body_face_at_cursor(rect, pos)
                    .map(|(_, _, hit)| DatumPick::Point(tuple_v(hit.hit_point)))
            }
            PickNeed::Edge | PickNeed::CenterSource => {
                if need == PickNeed::Edge {
                    if let Some((curve, hit)) = self.datum_axis_near(rect, pos) {
                        return Some(DatumPick::Edge { curve, hit });
                    }
                }
                match self.body_edge_curve_at(rect, pos) {
                    Some(Ok((curve, hit))) => return Some(DatumPick::Edge { curve, hit }),
                    Some(Err(msg)) => {
                        self.model_status = Some(msg);
                        return None;
                    }
                    None => {}
                }
                if need == PickNeed::CenterSource {
                    let (body, _, hit) = self.pick_body_face_at_cursor(rect, pos)?;
                    if hit.surface_kind == SurfaceKind::Sphere {
                        if let Some(ra) = hit.radial_axis {
                            return Some(DatumPick::Sphere(tuple_v(ra.origin)));
                        }
                        let geo = self.model.geometry.get(body)?;
                        let faces = ducad_kernel::enumerate_faces(&geo.shape);
                        let f = faces.get(hit.face_index?)?;
                        let samples: Vec<DVec3> = hit.boundary_points.iter().map(|p| tuple_v(*p)).collect();
                        return datum::sphere_center(&samples, f.radius?, DVec3::from_array(f.centroid))
                            .ok()
                            .map(DatumPick::Sphere);
                    }
                }
                None
            }
            PickNeed::Plane => {
                let (_, _, hit) = self.pick_body_face_at_cursor(rect, pos)?;
                if hit.surface_kind != SurfaceKind::Plane {
                    self.model_status = Some(ducad_i18n::t!("datum-err-need-planar"));
                    return None;
                }
                let n = tuple_v(hit.normal);
                let o = tuple_v(hit.hit_point);
                (n.is_finite() && o.is_finite() && n.length() > 1e-9)
                    .then(|| DatumPick::Plane(PlaneFrame::on_face(o.to_array(), n.to_array())))
            }
            PickNeed::Cylinder => {
                let (body, _, hit) = self.pick_body_face_at_cursor(rect, pos)?;
                let geo = self.model.geometry.get(body)?;
                let pick = cylinder_pick(&geo.shape, &hit);
                if pick.is_none() {
                    self.model_status = Some(ducad_i18n::t!("datum-err-need-cylinder"));
                }
                pick
            }
        }
    }

    /// Tambahkan bidang standar/datum (indeks gaya lama) sebagai pilihan.
    pub fn datum_add_plane_pick(&mut self, idx: usize) {
        let needs = self.datum_target().needs();
        if self.datums.picks.len() >= needs.len() {
            self.datums.picks.clear();
        }
        if needs.get(self.datums.picks.len()) != Some(&PickNeed::Plane) {
            let next = needs
                .get(self.datums.picks.len())
                .map(|n| n.label())
                .unwrap_or_default();
            self.model_status = Some(ducad_i18n::t!("datum-pick-miss", need = next.as_str()));
            return;
        }
        let frame = crate::document::plane_frame_from(&self.plane_for_index(idx));
        self.datums.picks.push(DatumPick::Plane(frame));
        self.model_status = Some(self.datum_pick_status().0);
    }

    // ---------------- Buat / hapus / ubah ----------------

    pub fn create_datum_axis(&mut self, name: Option<String>, axis: Axis) -> u32 {
        self.datums.axis_counter += 1;
        let id = self.datums.axis_counter;
        let name = name.unwrap_or_else(|| format!("{} {id}", ducad_i18n::t!("datum-kind-axis")));
        self.record_activity(
            ducad_ui::ActivityKindUi::Solid3D,
            &ducad_i18n::t!("datum-act-create-axis"),
            &name,
        );
        self.datums.axes.push(DatumAxis { id, name, axis, visible: true });
        self.model.doc.dirty = true;
        id
    }

    pub fn create_datum_point(&mut self, name: Option<String>, pos: DVec3) -> Option<u32> {
        if !pos.is_finite() {
            self.model_status = Some(ducad_i18n::t!("datum-err-not-finite"));
            return None;
        }
        self.datums.point_counter += 1;
        let id = self.datums.point_counter;
        let name = name.unwrap_or_else(|| format!("{} {id}", ducad_i18n::t!("datum-kind-point")));
        self.record_activity(
            ducad_ui::ActivityKindUi::Solid3D,
            &ducad_i18n::t!("datum-act-create-point"),
            &name,
        );
        self.datums.points.push(DatumPoint { id, name, pos, visible: true });
        self.model.doc.dirty = true;
        Some(id)
    }

    /// Hapus datum `(jenis, id)`. Bidang lewat `delete_datum_plane`.
    pub fn delete_datum(&mut self, kind: DatumKind, id: u32) {
        match kind {
            DatumKind::Plane => self.delete_datum_plane(id),
            DatumKind::Axis => {
                self.datums.axes.retain(|a| a.id != id);
                for slot in [
                    &mut self.datums.revolve_axis,
                    &mut self.datums.pattern_axis,
                    &mut self.datums.mate_axis,
                ] {
                    if *slot == Some(id) {
                        *slot = None;
                    }
                }
            }
            DatumKind::Point => self.datums.points.retain(|p| p.id != id),
        }
        self.model.doc.dirty = true;
    }

    /// Ganti nama datum. Nama kosong ditolak.
    pub fn rename_datum(&mut self, kind: DatumKind, id: u32, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        let slot = match kind {
            DatumKind::Plane => self.datum_planes.iter_mut().find(|d| d.id == id).map(|d| &mut d.name),
            DatumKind::Axis => self.datums.axes.iter_mut().find(|d| d.id == id).map(|d| &mut d.name),
            DatumKind::Point => self.datums.points.iter_mut().find(|d| d.id == id).map(|d| &mut d.name),
        };
        let Some(slot) = slot else {
            return false;
        };
        *slot = name.to_string();
        if kind == DatumKind::Plane {
            if let Some(slot) = self
                .sketch_set
                .first_on_plane(PlaneRef::Datum(id))
                .and_then(|sid| self.sketch_set.get_mut(sid))
            {
                slot.name = name.to_string();
            }
        }
        self.model.doc.dirty = true;
        true
    }

    /// Balik tampil/sembunyi datum. Mengembalikan keadaan baru.
    pub fn toggle_datum_visible(&mut self, kind: DatumKind, id: u32) -> Option<bool> {
        let slot = match kind {
            DatumKind::Plane => self.datum_planes.iter_mut().find(|d| d.id == id).map(|d| &mut d.visible),
            DatumKind::Axis => self.datums.axes.iter_mut().find(|d| d.id == id).map(|d| &mut d.visible),
            DatumKind::Point => self.datums.points.iter_mut().find(|d| d.id == id).map(|d| &mut d.visible),
        }?;
        *slot = !*slot;
        Some(*slot)
    }

    /// Buat datum dari pilihan yang terkumpul (tombol Buat / Enter).
    pub fn commit_datum_ref(&mut self) {
        let result = match self.datums.kind {
            DatumKind::Axis => compute_axis(self.datums.axis_mode, &self.datums.picks).map(|axis| {
                let id = self.create_datum_axis(None, axis);
                self.datums.axis(id).map(|a| a.name.clone()).unwrap_or_default()
            }),
            DatumKind::Point => {
                compute_point(self.datums.point_mode, &self.datums.picks, self.datums.fraction()).and_then(|p| {
                    let id = self.create_datum_point(None, p).ok_or_else(|| ducad_i18n::t!("datum-err-not-finite"))?;
                    Ok(self.datums.point(id).map(|a| a.name.clone()).unwrap_or_default())
                })
            }
            DatumKind::Plane => {
                self.apply_create_pick_datum_plane();
                return;
            }
        };
        match result {
            Ok(name) => {
                self.datums.picks.clear();
                self.planes_drawer_open = true;
                self.model_status = Some(ducad_i18n::t!("datum-created", name = name.as_str()));
            }
            Err(msg) => {
                self.model_status = Some(ducad_i18n::t!("datum-create-failed", error = msg.as_str()));
            }
        }
    }

    /// Bidang datum Paritas 1.4 dari pilihan.
    pub fn apply_create_pick_datum_plane(&mut self) {
        let angle = self.datum_angle_input.trim().parse::<f64>().unwrap_or(0.0);
        match compute_plane(self.datum_mode, &self.datums.picks, angle, self.datums.fraction()) {
            Ok(frame) => {
                let name = format!(
                    "Plane {} ({})",
                    self.datum_plane_counter + 1,
                    self.datum_mode.label()
                );
                let id = self.create_datum_plane(name.clone(), sketch_plane_from_frame(&frame));
                self.set_sketch_plane(PlaneKind::Custom(id));
                self.datums.picks.clear();
                self.planes_drawer_open = true;
                self.model_status = Some(ducad_i18n::t!("datum-created", name = name.as_str()));
                self.set_tool(ToolKind::Select);
            }
            Err(msg) => {
                self.model_status = Some(ducad_i18n::t!("datum-create-failed", error = msg.as_str()));
            }
        }
    }

    /// Pratinjau bidang dari pilihan (mode 1.4) — `None` bila belum lengkap
    /// atau degenerat.
    pub fn datum_pick_plane_preview(&self) -> Option<SketchPlane> {
        let angle = self.datum_angle_input.trim().parse::<f64>().unwrap_or(0.0);
        compute_plane(self.datum_mode, &self.datums.picks, angle, self.datums.fraction())
            .ok()
            .map(|f| sketch_plane_from_frame(&f))
    }

    // ---------------- Persistensi ----------------

    /// Datum dokumen untuk berkas `.ducad` (bidang + sketsanya, sumbu, titik).
    pub fn native_datums(&self) -> ducad_io::native::NativeDatums {
        let a = |v: Vec3| [v.x as f64, v.y as f64, v.z as f64];
        ducad_io::native::NativeDatums {
            planes: self
                .datum_planes
                .iter()
                .map(|dp| ducad_io::native::NativeDatumPlane {
                    id: dp.id,
                    name: dp.name.clone(),
                    origin: a(dp.plane.origin),
                    u_axis: a(dp.plane.u_axis),
                    v_axis: a(dp.plane.v_axis),
                    normal: a(dp.plane.normal),
                    visible: dp.visible,
                    sketch: self
                        .sketch_set
                        .first_on_plane(PlaneRef::Datum(dp.id))
                        .and_then(|id| self.sketch_set.sketch(id))
                        .filter(|s| !s.entities.is_empty())
                        .cloned(),
                })
                .collect(),
            axes: self
                .datums
                .axes
                .iter()
                .map(|x| ducad_io::native::NativeDatumAxis {
                    id: x.id,
                    name: x.name.clone(),
                    origin: x.axis.origin,
                    dir: x.axis.dir,
                    visible: x.visible,
                })
                .collect(),
            points: self
                .datums
                .points
                .iter()
                .map(|p| ducad_io::native::NativeDatumPoint {
                    id: p.id,
                    name: p.name.clone(),
                    pos: p.pos.to_array(),
                    visible: p.visible,
                })
                .collect(),
            views: self.datums.views.clone(),
            assembly: self.assembly_to_json(),
        }
    }

    /// Pasang datum dari berkas. Pemanggil sudah mengosongkan
    /// `datum_planes`. Entri rusak (NaN, arah nol) dilewati + dicatat.
    pub fn load_native_datums(&mut self, d: ducad_io::native::NativeDatums) {
        self.datums.clear_document();
        self.datums.views = d.views.iter().filter(|v| v.is_valid()).cloned().collect();
        let v = |a: [f64; 3]| Vec3::new(a[0] as f32, a[1] as f32, a[2] as f32);
        for p in d.planes {
            let all = [p.origin, p.u_axis, p.v_axis, p.normal];
            if all.iter().flatten().any(|c| !c.is_finite()) {
                log::warn!("datum plane '{}' rusak (non-finite); dilewati", p.name);
                continue;
            }
            let mut plane = SketchPlane::custom(p.id, v(p.origin), v(p.u_axis), v(p.v_axis), v(p.normal));
            plane.kind = PlaneKind::Custom(p.id);
            let mut dp = ducad_render::plane::DatumPlane::new(p.id, p.name.clone(), plane);
            dp.visible = p.visible;
            self.datum_planes.push(dp);
            self.datum_plane_counter = self.datum_plane_counter.max(p.id);
            let sid = self.sketch_set.add(p.name, PlaneRef::Datum(p.id));
            if let Some(sk) = p.sketch {
                if let Some(slot) = self.sketch_set.sketch_mut(sid) {
                    *slot = sk;
                }
            }
        }
        for x in d.axes {
            match Axis::new(DVec3::from_array(x.origin), DVec3::from_array(x.dir)) {
                Ok(axis) => {
                    self.datums.axis_counter = self.datums.axis_counter.max(x.id);
                    self.datums.axes.push(DatumAxis { id: x.id, name: x.name, axis, visible: x.visible });
                }
                Err(e) => log::warn!("sumbu datum '{}' rusak: {}; dilewati", x.name, e.message),
            }
        }
        for p in d.points {
            let pos = DVec3::from_array(p.pos);
            if !pos.is_finite() {
                log::warn!("titik datum '{}' rusak (non-finite); dilewati", p.name);
                continue;
            }
            self.datums.point_counter = self.datums.point_counter.max(p.id);
            self.datums.points.push(DatumPoint { id: p.id, name: p.name, pos, visible: p.visible });
        }
    }

    // ---------------- Render viewport ----------------

    /// Garis overlay datum: sumbu = garis putus-putus panjang, titik =
    /// penanda silang 3D, plus sorotan pilihan & pratinjau tool Datum.
    pub fn datum_overlay_lines(&self, world_scale: f64) -> Vec<LineVertex> {
        let mut verts = Vec::new();
        let ws = if world_scale.is_finite() && world_scale > 0.0 { world_scale } else { 0.1 };
        let axis_col = [0.95, 0.55, 0.15, 0.9];
        let point_col = [0.95, 0.80, 0.20, 1.0];
        let pick_col = [0.20, 0.90, 0.45, 1.0];
        let preview_col = [0.15, 0.80, 1.00, 1.0];
        let dash = (ws * 8.0) as f32;

        let axis_lines = |a: &Axis, col: [f32; 4], verts: &mut Vec<LineVertex>| {
            let o = a.origin_v();
            let d = a.dir_v();
            let p1 = o - d * AXIS_HALF_LEN;
            let p2 = o + d * AXIS_HALF_LEN;
            verts.extend(ducad_render::sketch::dashed_line_3d(
                to_v3(p1).to_array(),
                to_v3(p2).to_array(),
                dash.max(0.5),
                col,
            ));
        };
        let marker = |p: DVec3, size: f64, col: [f32; 4], verts: &mut Vec<LineVertex>| {
            for d in [DVec3::X, DVec3::Y, DVec3::Z] {
                verts.push(LineVertex { position: to_v3(p - d * size).to_array(), color: col });
                verts.push(LineVertex { position: to_v3(p + d * size).to_array(), color: col });
            }
        };

        for a in self.datums.axes.iter().filter(|a| a.visible) {
            axis_lines(&a.axis, axis_col, &mut verts);
        }
        for p in self.datums.points.iter().filter(|p| p.visible) {
            marker(p.pos, ws * 7.0, point_col, &mut verts);
        }

        if self.tool == ToolKind::DatumPlane {
            for pick in &self.datums.picks {
                match pick {
                    DatumPick::Point(p) | DatumPick::Sphere(p) => marker(*p, ws * 9.0, pick_col, &mut verts),
                    DatumPick::Edge { curve, hit } => {
                        const N: usize = 48;
                        for i in 0..N {
                            let a = curve.point_at(i as f64 / N as f64);
                            let b = curve.point_at((i + 1) as f64 / N as f64);
                            verts.push(LineVertex { position: to_v3(a).to_array(), color: pick_col });
                            verts.push(LineVertex { position: to_v3(b).to_array(), color: pick_col });
                        }
                        marker(*hit, ws * 5.0, pick_col, &mut verts);
                    }
                    DatumPick::Cylinder { axis, .. } => axis_lines(axis, pick_col, &mut verts),
                    DatumPick::Plane(f) => {
                        let o = f.origin_v();
                        marker(o, ws * 9.0, pick_col, &mut verts);
                        let n = f.normal_v() * (ws * 40.0);
                        verts.push(LineVertex { position: to_v3(o).to_array(), color: pick_col });
                        verts.push(LineVertex { position: to_v3(o + n).to_array(), color: pick_col });
                    }
                }
            }
            match self.datums.kind {
                DatumKind::Axis => {
                    if let Ok(a) = compute_axis(self.datums.axis_mode, &self.datums.picks) {
                        axis_lines(&a, preview_col, &mut verts);
                    }
                }
                DatumKind::Point => {
                    if let Ok(p) = compute_point(self.datums.point_mode, &self.datums.picks, self.datums.fraction()) {
                        marker(p, ws * 10.0, preview_col, &mut verts);
                    }
                }
                DatumKind::Plane => {}
            }
        }
        verts
    }

    /// Label nama sumbu/titik datum di viewport (painter egui).
    pub fn paint_datum_labels(&self, ui: &egui::Ui, rect: egui::Rect) {
        let painter = ui.painter_at(rect);
        let font = egui::FontId::proportional(10.0);
        let col = egui::Color32::from_rgb(240, 170, 70);
        for a in self.datums.axes.iter().filter(|a| a.visible) {
            let tip = a.axis.origin_v() + a.axis.dir_v() * (AXIS_HALF_LEN * 0.9);
            if let Some(sp) = crate::viewport::world_to_screen_pos(&self.camera, rect, to_v3(tip)) {
                painter.text(sp + egui::vec2(4.0, -4.0), egui::Align2::LEFT_BOTTOM, &a.name, font.clone(), col);
            }
        }
        for p in self.datums.points.iter().filter(|p| p.visible) {
            if let Some(sp) = crate::viewport::world_to_screen_pos(&self.camera, rect, to_v3(p.pos)) {
                painter.text(sp + egui::vec2(6.0, -6.0), egui::Align2::LEFT_BOTTOM, &p.name, font.clone(), col);
            }
        }
    }

    // ---------------- HUD tool Datum (sumbu/titik) ----------------

    /// HUD + keyboard untuk tool Datum saat jenisnya sumbu/titik.
    pub fn show_datum_ref_hud(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let plane_names: Vec<(usize, String)> = self.all_planes().into_iter().map(|(i, _, n)| (i, n)).collect();
        let target = self.datum_target();
        let (status, ready) = self.datum_pick_status();
        let wants_plane = target.needs().contains(&PickNeed::Plane);
        let input = ducad_ui::DatumRefHudInput {
            kind: self.datums.kind,
            axis_mode: self.datums.axis_mode,
            point_mode: self.datums.point_mode,
            plane_names: &plane_names,
            pick_status: &status,
            ready,
            wants_plane,
            needs_fraction: target.needs_fraction(),
        };
        let action = ducad_ui::datum_hud::render_datum_ref_hud(ui, rect, &input, &mut self.datums.fraction_input);
        if let Some(action) = action {
            self.apply_datum_ref_action(action);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.commit_datum_ref();
        } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.datums.picks.clear();
            self.set_tool(ToolKind::Select);
        }
        let step = status.clone();
        ducad_ui::ToolGuides::render_datum_text_guide(ui, rect, &target.label(), &step);
    }

    pub fn apply_datum_ref_action(&mut self, action: ducad_ui::DatumRefHudAction) {
        use ducad_ui::DatumRefHudAction as A;
        match action {
            A::SwitchKind(k) => {
                self.datums.kind = k;
                self.datums.picks.clear();
            }
            A::SetAxisMode(m) => {
                self.datums.axis_mode = m;
                self.datums.picks.clear();
            }
            A::SetPointMode(m) => {
                self.datums.point_mode = m;
                self.datums.picks.clear();
            }
            A::AddPlane(idx) => self.datum_add_plane_pick(idx),
            A::ClearPicks => self.datums.picks.clear(),
            A::SetFraction(f) => self.datums.fraction_input = format!("{f}"),
            A::Commit => self.commit_datum_ref(),
            A::Cancel => {
                self.datums.picks.clear();
                self.set_tool(ToolKind::Select);
            }
        }
    }

    // ---------------- 1.6: Revolve / Pattern / Mate ----------------

    /// Revolve profil sketsa terpilih mengelilingi sumbu datum `id`.
    ///
    /// `revolve_selected` selalu membangun revolve di bidang XY
    /// (`PlaneFrame::top()`), jadi sumbu diterjemahkan ke bidang yang SAMA
    /// lewat `datum::axis_in_plane`; sumbu yang tidak terletak di bidang itu
    /// ditolak dengan pesan engine.
    pub fn revolve_selected_about_datum_axis(&mut self, id: u32, angle_deg: f64) -> bool {
        let Some(axis) = self.datums.axis(id).map(|a| a.axis) else {
            self.model_status = Some(ducad_i18n::t!("datum-err-axis-missing"));
            return false;
        };
        if self.selected.is_empty() {
            self.model_status = Some(ducad_i18n::t!("datum-revolve-need-profile"));
            return false;
        }
        let frame = PlaneFrame::top();
        let (o, d) = match datum::axis_in_plane(&axis, &frame) {
            Ok(v) => v,
            Err(e) => {
                self.model_status = Some(ducad_i18n::t!("datum-create-failed", error = op_msg(&e)));
                return false;
            }
        };
        let d = if self.revolve_reverse { -d } else { d };
        let angle = if !angle_deg.is_finite() || (angle_deg - 360.0).abs() < 1e-4 {
            None
        } else {
            Some(angle_deg)
        };
        let ok = self.revolve_selected((o.x, o.y), (d.x, d.y), angle);
        if ok {
            self.revolve_staged_axis = None;
            self.set_tool(ToolKind::Select);
        }
        ok
    }

    /// Sumbu circular pattern 3D dari datum terpilih: `(pivot, arah)`.
    pub fn pattern_datum_axis(&self) -> Option<([f64; 3], [f64; 3])> {
        let a = self.datums.axis(self.datums.pattern_axis?)?;
        Some((a.axis.origin, a.axis.dir))
    }

    /// Mate Concentric antara face silinder yang sudah di-stage dan sumbu
    /// datum `id` (sumbu dunia tetap). Sumbu ditambatkan ke instance yang
    /// di-ground (selain body yang digerakkan) dalam kerangka lokalnya.
    pub fn apply_concentric_to_datum_axis(&mut self, id: u32) -> bool {
        let Some(axis) = self.datums.axis(id).map(|a| a.axis) else {
            self.model_status = Some(ducad_i18n::t!("datum-err-axis-missing"));
            return false;
        };
        let Some((body_a, _, hit_a)) = self.staged_mate_targets.first().cloned() else {
            self.model_status = Some(ducad_i18n::t!("datum-mate-need-cylinder"));
            return false;
        };
        if !matches!(hit_a.surface_kind, SurfaceKind::Cylinder | SurfaceKind::Cone) {
            self.model_status = Some(ducad_i18n::t!("datum-mate-need-cylinder"));
            return false;
        }
        self.sync_assembly_instances();
        let raw_a = body_a.data().as_ffi();
        let Some(inst_a) = self
            .assembly_tree
            .instances
            .values()
            .find(|i| i.body_id_raw == raw_a)
            .map(|i| i.id)
        else {
            self.model_status = Some(ducad_i18n::t!("datum-mate-no-anchor"));
            return false;
        };
        let Some(anchor) = self
            .assembly_tree
            .instances
            .values()
            .filter(|i| i.is_grounded && i.id != inst_a)
            .map(|i| i.id)
            .min()
        else {
            self.model_status = Some(ducad_i18n::t!("datum-mate-no-anchor"));
            return false;
        };
        let (cyl_o, cyl_d) = match hit_a.radial_axis {
            Some(ra) if tuple_v(ra.dir).length() > 1e-9 => (ra.origin, ra.dir),
            _ => (hit_a.centroid, hit_a.pull_dir),
        };
        let radius = hit_a.surface_radius.unwrap_or(5.0);
        // Sumbu datum (dunia) → kerangka lokal instance jangkar.
        let (t, q) = self
            .assembly_tree
            .instance_world_transform(anchor)
            .unwrap_or((DVec3::ZERO, glam::DQuat::IDENTITY));
        let inv = q.inverse();
        let lo = inv * (axis.origin_v() - t);
        let ld = inv * axis.dir_v();
        let kind = ducad_core::MateKind::Concentric {
            aligned: !self.mate_flip_alignment,
            lock_rotation: self.mate_lock_rotation,
        };
        let moving = ducad_core::MateTarget {
            instance_id: inst_a,
            kind: ducad_core::MateTargetKind::CylinderAxis { origin: cyl_o, direction: cyl_d, radius },
        };
        let reference = ducad_core::MateTarget {
            instance_id: anchor,
            kind: ducad_core::MateTargetKind::CylinderAxis {
                origin: (lo.x, lo.y, lo.z),
                direction: (ld.x, ld.y, ld.z),
                radius,
            },
        };
        let axis_name = self.datums.axis(id).map(|a| a.name.clone()).unwrap_or_default();
        let name = format!("{} Mate ({axis_name})", kind.type_name());
        // Konvensi solver: target A = referensi (jangkar grounded), target B
        // = part yang digerakkan.
        self.assembly_tree.add_mate(&name, kind, reference, moving);
        self.solve_and_apply_assembly();
        self.staged_mate_targets.clear();
        self.staged_mate_kind = None;
        self.active_face = None;
        self.selected_faces.clear();
        self.model.doc.dirty = true;
        self.model_status = Some(ducad_i18n::t!("datum-mate-applied", name = name.as_str()));
        true
    }

    /// Panel mengambang "sumbu: datum" untuk Revolve, Circular Pattern 3D,
    /// dan Mate Concentric (Paritas 1.6). Hanya tampil bila ada sumbu datum.
    pub fn show_datum_axis_usage(&mut self, ctx: &egui::Context, screen_rect: egui::Rect) {
        if self.datums.axes.is_empty() {
            return;
        }
        let mate_conc = matches!(self.staged_mate_kind, Some(ducad_core::MateKind::Concentric { .. }))
            && self.staged_mate_targets.len() == 1;
        let pattern_circ = self.tool == ToolKind::Pattern
            && !self.is_sketching
            && self.pattern_kind == ducad_ui::PatternKind::Circular;
        let revolve = self.tool == ToolKind::Revolve && !self.selected.is_empty();
        if !(mate_conc || pattern_circ || revolve) {
            return;
        }
        let axes: Vec<(u32, String)> = self.datums.axes.iter().map(|a| (a.id, a.name.clone())).collect();
        let pos = egui::pos2(screen_rect.center().x, screen_rect.bottom() - 130.0);
        let mut run_revolve = None;
        let mut run_mate = None;
        egui::Area::new(egui::Id::new("ducad-datum-axis-usage"))
            .fixed_pos(pos)
            .pivot(egui::Align2::CENTER_BOTTOM)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ducad_ui::glass_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(egui_icons::icons::ICON_TIMELINE.codepoint)
                                .color(ducad_ui::ACCENT_ORANGE),
                        );
                        ui.label(egui::RichText::new(ducad_i18n::t!("datum-axis-use-label")).size(11.0));
                        let slot = if mate_conc {
                            &mut self.datums.mate_axis
                        } else if pattern_circ {
                            &mut self.datums.pattern_axis
                        } else {
                            &mut self.datums.revolve_axis
                        };
                        let none_label = if pattern_circ {
                            ducad_i18n::t!("datum-axis-use-preset")
                        } else {
                            ducad_i18n::t!("datum-axis-use-none")
                        };
                        let current = slot
                            .and_then(|id| axes.iter().find(|(i, _)| *i == id))
                            .map(|(_, n)| n.clone())
                            .unwrap_or_else(|| none_label.clone());
                        egui::ComboBox::from_id_salt("ducad-datum-axis-use-combo")
                            .selected_text(current)
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(slot, None, none_label);
                                for (id, name) in &axes {
                                    ui.selectable_value(slot, Some(*id), name);
                                }
                            });
                        if let Some(id) = *slot {
                            if revolve && !mate_conc && !pattern_circ {
                                if ducad_ui::CanvasHud::hud_commit_btn(ui, ducad_i18n::t!("datum-axis-use-revolve")).clicked() {
                                    run_revolve = Some(id);
                                }
                            } else if mate_conc
                                && ducad_ui::CanvasHud::hud_commit_btn(ui, ducad_i18n::t!("datum-axis-use-mate")).clicked()
                            {
                                run_mate = Some(id);
                            }
                        }
                    });
                });
            });
        if let Some(id) = run_revolve {
            let angle = self.revolve_angle_setting;
            self.revolve_selected_about_datum_axis(id, angle);
        }
        if let Some(id) = run_mate {
            self.apply_concentric_to_datum_axis(id);
        }
    }

    /// Item drawer Datum untuk sumbu & titik.
    pub fn datum_drawer_items(&self) -> Vec<ducad_ui::DatumItemInfo> {
        self.datums
            .axes
            .iter()
            .map(|a| ducad_ui::DatumItemInfo { kind: DatumKind::Axis, id: a.id, name: a.name.clone(), visible: a.visible })
            .chain(self.datums.points.iter().map(|p| ducad_ui::DatumItemInfo {
                kind: DatumKind::Point,
                id: p.id,
                name: p.name.clone(),
                visible: p.visible,
            }))
            .collect()
    }

    /// Tangani event drawer Datum yang baru (1.5). `true` = sudah ditangani.
    pub fn handle_datum_drawer_event(&mut self, ev: &ducad_ui::PlanesDrawerEvent) -> bool {
        use ducad_ui::PlanesDrawerEvent as E;
        match ev {
            E::CreateDatum(k) => self.open_datum_tool(*k),
            E::ToggleVisible(k, id) => {
                if let Some(v) = self.toggle_datum_visible(*k, *id) {
                    self.model_status = Some(if v {
                        ducad_i18n::t!("datum-shown")
                    } else {
                        ducad_i18n::t!("datum-hidden")
                    });
                }
            }
            E::Rename(k, id, name) => {
                if self.rename_datum(*k, *id, name) {
                    self.model_status = Some(ducad_i18n::t!("datum-renamed", name = name.as_str()));
                }
            }
            E::DeleteDatum(k, id) => {
                self.delete_datum(*k, *id);
                self.model_status = Some(ducad_i18n::t!("datum-deleted"));
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(o: [f64; 3], n: [f64; 3]) -> DatumPick {
        DatumPick::Plane(PlaneFrame::on_face(o, n))
    }

    fn line(a: [f64; 3], b: [f64; 3]) -> DatumPick {
        DatumPick::Edge {
            curve: EdgeCurve::Line { start: DVec3::from_array(a), end: DVec3::from_array(b) },
            hit: (DVec3::from_array(a) + DVec3::from_array(b)) * 0.5,
        }
    }

    #[test]
    fn axis_modes_compute_and_reject_degenerate() {
        let a = compute_axis(
            DatumAxisMode::TwoPoints,
            &[DatumPick::Point(DVec3::ZERO), DatumPick::Point(DVec3::new(0.0, 0.0, 5.0))],
        )
        .unwrap();
        assert!((a.dir_v() - DVec3::Z).length() < 1e-9);
        // Titik berimpit → pesan, bukan panik.
        assert!(compute_axis(DatumAxisMode::TwoPoints, &[DatumPick::Point(DVec3::ZERO), DatumPick::Point(DVec3::ZERO)]).is_err());
        // NaN.
        assert!(compute_axis(
            DatumAxisMode::TwoPoints,
            &[DatumPick::Point(DVec3::NAN), DatumPick::Point(DVec3::ONE)]
        )
        .is_err());
        // Dua bidang sejajar → error.
        assert!(compute_axis(DatumAxisMode::TwoPlanes, &[plane([0.0; 3], [0.0, 0.0, 1.0]), plane([0.0, 0.0, 3.0], [0.0, 0.0, 1.0])]).is_err());
        let x = compute_axis(DatumAxisMode::TwoPlanes, &[plane([0.0; 3], [0.0, 0.0, 1.0]), plane([0.0; 3], [0.0, 1.0, 0.0])]).unwrap();
        assert!(x.dir_v().cross(DVec3::X).length() < 1e-9);
        // Pilihan kurang / salah jenis.
        assert!(compute_axis(DatumAxisMode::Edge, &[]).is_err());
        assert!(compute_axis(DatumAxisMode::Edge, &[DatumPick::Point(DVec3::ZERO)]).is_err());
        let e = compute_axis(DatumAxisMode::Edge, &[line([0.0; 3], [10.0, 0.0, 0.0])]).unwrap();
        assert!(e.dir_v().cross(DVec3::X).length() < 1e-9);
        let n = compute_axis(DatumAxisMode::NormalToFace, &[plane([1.0, 2.0, 3.0], [0.0, 0.0, 1.0])]).unwrap();
        assert!((n.origin_v() - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-9);
        let axis = Axis::new(DVec3::ZERO, DVec3::Y).unwrap();
        let c = compute_axis(DatumAxisMode::ThroughCylinder, &[DatumPick::Cylinder { axis, radius: 4.0 }]).unwrap();
        assert_eq!(c, axis);
    }

    #[test]
    fn point_modes_compute() {
        let p = compute_point(
            DatumPointMode::ThreePlanes,
            &[plane([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]), plane([0.0, 2.0, 0.0], [0.0, 1.0, 0.0]), plane([0.0, 0.0, 3.0], [0.0, 0.0, 1.0])],
            None,
        )
        .unwrap();
        assert!((p - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-9);
        let p = compute_point(DatumPointMode::AlongPath, &[line([0.0; 3], [10.0, 0.0, 0.0])], Some(0.25)).unwrap();
        assert!((p - DVec3::new(2.5, 0.0, 0.0)).length() < 1e-9);
        assert!(compute_point(DatumPointMode::AlongPath, &[line([0.0; 3], [10.0, 0.0, 0.0])], Some(1.5)).is_err());
        assert!(compute_point(DatumPointMode::AlongPath, &[line([0.0; 3], [10.0, 0.0, 0.0])], None).is_err());
        let p = compute_point(
            DatumPointMode::EdgePlane,
            &[line([0.0, 0.0, -5.0], [0.0, 0.0, 5.0]), plane([0.0, 0.0, 2.0], [0.0, 0.0, 1.0])],
            None,
        )
        .unwrap();
        assert!((p.z - 2.0).abs() < 1e-9);
        let p = compute_point(
            DatumPointMode::TwoEdges,
            &[line([-5.0, 0.0, 0.0], [5.0, 0.0, 0.0]), line([0.0, -5.0, 0.0], [0.0, 5.0, 0.0])],
            None,
        )
        .unwrap();
        assert!(p.length() < 1e-9);
        // Garis sejajar → error.
        assert!(compute_point(
            DatumPointMode::TwoEdges,
            &[line([0.0; 3], [5.0, 0.0, 0.0]), line([0.0, 1.0, 0.0], [5.0, 1.0, 0.0])],
            None
        )
        .is_err());
        let arc = EdgeCurve::Arc {
            center: DVec3::new(1.0, 1.0, 0.0),
            normal: DVec3::Z,
            radius: 2.0,
            start_dir: DVec3::X,
            sweep: std::f64::consts::TAU,
        };
        let p = compute_point(DatumPointMode::Center, &[DatumPick::Edge { curve: arc, hit: DVec3::ZERO }], None).unwrap();
        assert!((p - DVec3::new(1.0, 1.0, 0.0)).length() < 1e-9);
        assert!(compute_point(DatumPointMode::Center, &[line([0.0; 3], [1.0, 0.0, 0.0])], None).is_err());
        let p = compute_point(DatumPointMode::Center, &[DatumPick::Sphere(DVec3::ONE)], None).unwrap();
        assert_eq!(p, DVec3::ONE);
    }

    #[test]
    fn plane_modes_compute() {
        let m = compute_plane(
            DatumPlaneMode::Midplane,
            &[plane([0.0; 3], [0.0, 0.0, 1.0]), plane([0.0, 0.0, 10.0], [0.0, 0.0, -1.0])],
            0.0,
            None,
        )
        .unwrap();
        assert!((m.origin_v().z - 5.0).abs() < 1e-9);
        assert!(compute_plane(
            DatumPlaneMode::Midplane,
            &[plane([0.0; 3], [0.0, 0.0, 1.0]), plane([0.0; 3], [1.0, 0.0, 0.0])],
            0.0,
            None
        )
        .is_err());
        let p = compute_plane(DatumPlaneMode::Perpendicular, &[line([0.0; 3], [10.0, 0.0, 0.0])], 0.0, None).unwrap();
        assert!(p.normal_v().cross(DVec3::X).length() < 1e-9);
        assert!((p.origin_v().x - 5.0).abs() < 0.1);
        let axis = Axis::new(DVec3::ZERO, DVec3::Z).unwrap();
        let t = compute_plane(DatumPlaneMode::Tangent, &[DatumPick::Cylinder { axis, radius: 3.0 }], 0.0, None).unwrap();
        assert!((t.origin_v() - DVec3::new(3.0, 0.0, 0.0)).length() < 1e-9);
        let a = compute_plane(DatumPlaneMode::AlongPath, &[line([0.0; 3], [0.0, 0.0, 8.0])], 0.0, Some(0.5)).unwrap();
        assert!((a.origin_v().z - 4.0).abs() < 1e-9);
        let two = compute_plane(
            DatumPlaneMode::TwoEdges,
            &[line([0.0; 3], [5.0, 0.0, 0.0]), line([0.0; 3], [0.0, 5.0, 0.0])],
            0.0,
            None,
        )
        .unwrap();
        assert!(two.normal_v().cross(DVec3::Z).length() < 1e-9);
        // Edge bersilangan (tidak sebidang) → error.
        assert!(compute_plane(
            DatumPlaneMode::TwoEdges,
            &[line([0.0; 3], [5.0, 0.0, 0.0]), line([0.0, 0.0, 3.0], [0.0, 5.0, 3.0])],
            0.0,
            None
        )
        .is_err());
        // Mode lama tidak lewat pilihan.
        assert!(compute_plane(DatumPlaneMode::Offset, &[], 0.0, None).is_err());
    }

    // ------------------- Tes tingkat aplikasi -------------------

    use std::collections::HashSet;

    use ducad_sketch::Entity;
    use glam::DVec2;

    fn add_rect(app: &mut DuCADApp, x0: f64, y0: f64, x1: f64, y1: f64) -> HashSet<ducad_sketch::EntityId> {
        let c = [DVec2::new(x0, y0), DVec2::new(x1, y0), DVec2::new(x1, y1), DVec2::new(x0, y1)];
        (0..4)
            .map(|i| app.sketch_mut().entities.insert(Entity::line(c[i], c[(i + 1) % 4])))
            .collect()
    }

    fn add_cylinder(app: &mut DuCADApp, r: f64, h: f64) -> ducad_core::BodyId {
        let geo = ducad_engine::compute::primitive(&ducad_engine::compute::PrimitiveShape::Cylinder { r, h }, [0.0; 3])
            .unwrap();
        let id = app.model.doc.add_body("Cyl");
        app.model.geometry.insert(id, geo);
        id
    }

    #[test]
    fn create_rename_hide_delete_axis_and_point() {
        let mut app = DuCADApp::new_for_test();
        let ax = app.create_datum_axis(None, Axis::new(DVec3::ZERO, DVec3::Z).unwrap());
        let pt = app.create_datum_point(None, DVec3::new(1.0, 2.0, 3.0)).unwrap();
        assert_eq!((app.datums.axes.len(), app.datums.points.len()), (1, 1));
        // Titik NaN ditolak, tidak panik.
        assert!(app.create_datum_point(None, DVec3::NAN).is_none());
        assert_eq!(app.datums.points.len(), 1);

        assert!(app.rename_datum(DatumKind::Axis, ax, "Poros"));
        assert!(!app.rename_datum(DatumKind::Axis, ax, "   "), "nama kosong ditolak");
        assert!(!app.rename_datum(DatumKind::Point, 999, "x"), "id tak dikenal");
        assert_eq!(app.datums.axis(ax).unwrap().name, "Poros");
        assert!(app.rename_datum(DatumKind::Point, pt, "Pusat"));

        assert_eq!(app.toggle_datum_visible(DatumKind::Axis, ax), Some(false));
        assert_eq!(app.toggle_datum_visible(DatumKind::Point, pt), Some(false));
        assert_eq!(app.toggle_datum_visible(DatumKind::Point, 999), None);
        // Datum tersembunyi tidak menggambar apa pun.
        app.set_tool(ToolKind::Select);
        assert!(app.datum_overlay_lines(0.1).is_empty());
        app.toggle_datum_visible(DatumKind::Axis, ax);
        assert!(!app.datum_overlay_lines(0.1).is_empty());

        // Lewat event drawer.
        app.datums.pattern_axis = Some(ax);
        assert!(app.handle_datum_drawer_event(&ducad_ui::PlanesDrawerEvent::DeleteDatum(DatumKind::Axis, ax)));
        assert!(app.datums.axes.is_empty());
        assert_eq!(app.datums.pattern_axis, None, "rujukan sumbu ikut dilepas");
        app.delete_datum(DatumKind::Point, pt);
        assert!(app.datums.points.is_empty());
    }

    #[test]
    fn datum_plane_rename_and_hide_from_drawer() {
        let mut app = DuCADApp::new_for_test();
        let id = app.create_datum_plane("Plane 1".into(), SketchPlane::top().offset(5.0));
        assert!(app.handle_datum_drawer_event(&ducad_ui::PlanesDrawerEvent::Rename(DatumKind::Plane, id, "Atas".into())));
        assert_eq!(app.datum_planes[0].name, "Atas");
        assert!(app.handle_datum_drawer_event(&ducad_ui::PlanesDrawerEvent::ToggleVisible(DatumKind::Plane, id)));
        assert!(!app.datum_planes[0].visible);
    }

    #[test]
    fn commit_axis_and_point_from_picks_and_reject_degenerate() {
        let mut app = DuCADApp::new_for_test();
        app.open_datum_tool(DatumKind::Axis);
        app.datums.axis_mode = DatumAxisMode::TwoPlanes;
        app.datum_add_plane_pick(0); // Top
        app.datum_add_plane_pick(1); // Front
        app.commit_datum_ref();
        assert_eq!(app.datums.axes.len(), 1, "{:?}", app.model_status);
        assert!(app.datums.axes[0].axis.dir_v().cross(DVec3::X).length() < 1e-9);
        assert!(app.datums.picks.is_empty());

        // Dua bidang sejajar: tidak ada sumbu, ada pesan.
        app.datum_add_plane_pick(0);
        app.datum_add_plane_pick(0);
        app.commit_datum_ref();
        assert_eq!(app.datums.axes.len(), 1);
        assert!(app.model_status.is_some());

        // Titik dari tiga bidang standar = origin.
        app.datums.kind = DatumKind::Point;
        app.datums.point_mode = DatumPointMode::ThreePlanes;
        app.datums.picks.clear();
        for i in 0..3 {
            app.datum_add_plane_pick(i);
        }
        app.commit_datum_ref();
        assert_eq!(app.datums.points.len(), 1, "{:?}", app.model_status);
        assert!(app.datums.points[0].pos.length() < 1e-9);

        // Pilihan bidang di mode yang butuh edge ditolak.
        app.datums.point_mode = DatumPointMode::AlongPath;
        app.datums.picks.clear();
        app.datum_add_plane_pick(0);
        assert!(app.datums.picks.is_empty());
        // Commit tanpa pilihan → pesan, bukan panik.
        app.commit_datum_ref();
        assert_eq!(app.datums.points.len(), 1);
    }

    #[test]
    fn every_new_plane_mode_creates_a_datum_plane() {
        let axis = Axis::new(DVec3::ZERO, DVec3::Z).unwrap();
        let cases: Vec<(DatumPlaneMode, Vec<DatumPick>)> = vec![
            (DatumPlaneMode::Midplane, vec![plane([0.0; 3], [0.0, 0.0, 1.0]), plane([0.0, 0.0, 10.0], [0.0, 0.0, 1.0])]),
            (DatumPlaneMode::Perpendicular, vec![line([0.0; 3], [10.0, 0.0, 0.0])]),
            (DatumPlaneMode::Tangent, vec![DatumPick::Cylinder { axis, radius: 5.0 }]),
            (DatumPlaneMode::AlongPath, vec![line([0.0; 3], [0.0, 0.0, 10.0])]),
            (DatumPlaneMode::TwoEdges, vec![line([0.0; 3], [5.0, 0.0, 0.0]), line([0.0; 3], [0.0, 0.0, 5.0])]),
        ];
        for (mode, picks) in cases {
            let mut app = DuCADApp::new_for_test();
            app.set_tool(ToolKind::DatumPlane);
            app.datum_mode = mode;
            app.datums.picks = picks;
            app.datum_angle_input = "90".into();
            app.apply_create_datum_plane();
            assert_eq!(app.datum_planes.len(), 1, "{mode:?}: {:?}", app.model_status);
            let p = app.datum_planes[0].plane;
            assert!(matches!(p.kind, PlaneKind::Custom(_)));
            assert!(p.normal.is_finite() && (p.normal.length() - 1.0).abs() < 1e-4, "{mode:?}");
            assert!(app.datums.picks.is_empty());
        }
        // Tangent 90° di silinder r=5 sumbu Z: titik singgung (0, 5, 0).
        let mut app = DuCADApp::new_for_test();
        app.datum_mode = DatumPlaneMode::Tangent;
        app.datums.picks = vec![DatumPick::Cylinder { axis, radius: 5.0 }];
        app.datum_angle_input = "90".into();
        app.apply_create_datum_plane();
        let o = app.datum_planes[0].plane.origin;
        assert!((o - Vec3::new(0.0, 5.0, 0.0)).length() < 1e-4, "{o:?}");
        // Degenerat: midplane bidang tidak sejajar → tidak ada bidang.
        let mut app = DuCADApp::new_for_test();
        app.datum_mode = DatumPlaneMode::Midplane;
        app.datums.picks = vec![plane([0.0; 3], [0.0, 0.0, 1.0]), plane([0.0; 3], [1.0, 0.0, 0.0])];
        app.apply_create_datum_plane();
        assert!(app.datum_planes.is_empty());
        assert!(app.model_status.is_some());
    }

    #[test]
    fn revolve_around_datum_axis_makes_a_body() {
        let mut app = DuCADApp::new_for_test();
        app.selected = add_rect(&mut app, 10.0, 0.0, 15.0, 10.0);
        // Sumbu Y dunia (terletak di bidang XY tempat revolve dibangun).
        let ax = app.create_datum_axis(None, Axis::new(DVec3::ZERO, DVec3::Y).unwrap());
        assert!(app.revolve_selected_about_datum_axis(ax, 180.0), "{:?}", app.model_status);
        assert_eq!(app.model.doc.bodies.len(), 1);
        let vol: f64 = app.model.geometry.values().map(|g| g.shape.volume().abs()).sum();
        // Sama dengan revolve sumbu Y lewat preset (adapter_tests REVOLVE_VOLUME).
        assert!((vol - 1963.495408494).abs() / 1963.495408494 < 1e-3, "{vol}");

        // Sumbu tegak lurus bidang sketch ditolak dengan pesan engine.
        let mut app = DuCADApp::new_for_test();
        app.selected = add_rect(&mut app, 10.0, 0.0, 15.0, 10.0);
        let z = app.create_datum_axis(None, Axis::new(DVec3::ZERO, DVec3::Z).unwrap());
        assert!(!app.revolve_selected_about_datum_axis(z, 360.0));
        assert!(app.model.doc.bodies.is_empty());
        assert!(app.model_status.is_some());
        // Id tak dikenal.
        assert!(!app.revolve_selected_about_datum_axis(999, 360.0));
    }

    #[test]
    fn circular_pattern_uses_datum_axis() {
        let mut app = DuCADApp::new_for_test();
        let id = add_cylinder(&mut app, 2.0, 5.0);
        // Sumbu sejajar Z melalui (20, 0): salinan mengelilingi titik itu.
        let ax = app.create_datum_axis(None, Axis::new(DVec3::new(20.0, 0.0, 0.0), DVec3::Z).unwrap());
        app.datums.pattern_axis = Some(ax);
        app.pattern_kind = ducad_ui::PatternKind::Circular;
        app.pattern_circ_count = 2;
        app.pattern_circ_angle_deg = 360.0;
        app.selected_bodies = [id].into_iter().collect();
        app.apply_pattern_3d();
        assert!(app.model.doc.bodies.len() >= 2, "{:?}", app.model_status);
        // Salinan kedua (180°) berpusat di (40, 0).
        let far = app
            .model
            .geometry
            .iter()
            .filter(|(b, _)| *b != id)
            .any(|(b, _)| app.body_bbox_center(b).is_some_and(|c| (c.x - 40.0).abs() < 0.5));
        assert!(far, "salinan harus mengelilingi sumbu datum");
    }

    #[test]
    fn mate_concentric_moves_cylinder_onto_datum_axis() {
        let mut app = DuCADApp::new_for_test();
        let _anchor = add_cylinder(&mut app, 3.0, 5.0);
        let geo = ducad_engine::compute::primitive(
            &ducad_engine::compute::PrimitiveShape::Cylinder { r: 2.0, h: 5.0 },
            [10.0, 0.0, 0.0],
        )
        .unwrap();
        let moving = app.model.doc.add_body("Pin");
        app.model.geometry.insert(moving, geo);
        app.sync_assembly_instances();

        let ray = ducad_kernel::PickRay { origin: (50.0, 0.0, 2.5), dir: (-1.0, 0.0, 0.0) };
        let hit = ducad_kernel::pick_face_details(&app.model.geometry[moving].shape, ray).expect("face pin");
        app.staged_mate_kind = Some(ducad_core::MateKind::Concentric { aligned: true, lock_rotation: false });
        app.staged_mate_targets = vec![(moving, ray, hit)];
        let ax = app.create_datum_axis(None, Axis::new(DVec3::new(30.0, 0.0, 0.0), DVec3::Z).unwrap());
        assert!(app.apply_concentric_to_datum_axis(ax), "{:?}", app.model_status);
        let c = app.body_bbox_center(moving).unwrap();
        assert!((c.x - 30.0).abs() < 0.5 && c.y.abs() < 0.5, "pin harus pindah ke sumbu datum: {c:?}");
        assert!(app.staged_mate_kind.is_none());

        // Tanpa face silinder ter-stage: pesan, bukan panik.
        assert!(!app.apply_concentric_to_datum_axis(ax));
    }

    #[test]
    fn cylinder_and_edge_picks_from_real_geometry() {
        let mut app = DuCADApp::new_for_test();
        let id = add_cylinder(&mut app, 5.0, 10.0);
        let shape = &app.model.geometry[id].shape;
        // Sinar horizontal mengenai sisi silinder.
        let ray = ducad_kernel::PickRay { origin: (50.0, 0.0, 5.0), dir: (-1.0, 0.0, 0.0) };
        let hit = ducad_kernel::pick_face_details(shape, ray).expect("face silinder");
        let Some(DatumPick::Cylinder { axis, radius }) = cylinder_pick(shape, &hit) else {
            panic!("bukan silinder: {:?}", hit.surface_kind);
        };
        assert!((radius - 5.0).abs() < 1e-6);
        assert!(axis.dir_v().cross(DVec3::Z).length() < 1e-6);
        assert!(axis.distance_to(DVec3::ZERO) < 1e-6);

        // Titik di lingkaran atas → edge busur, pusat = (0,0,10).
        let c = edge_curve_near(shape, DVec3::new(5.0, 0.0, 10.0), 0.5).unwrap().unwrap();
        let p = compute_point(DatumPointMode::Center, &[DatumPick::Edge { curve: c, hit: DVec3::ZERO }], None).unwrap();
        assert!((p - DVec3::new(0.0, 0.0, 10.0)).length() < 1e-6, "{p:?}");
        // Jauh dari edge mana pun → None; NaN → None.
        assert!(edge_curve_near(shape, DVec3::new(100.0, 100.0, 100.0), 0.5).is_none());
        assert!(edge_curve_near(shape, DVec3::NAN, 0.5).is_none());
    }

    #[test]
    fn datums_survive_native_roundtrip() {
        let mut app = DuCADApp::new_for_test();
        let pid = app.create_datum_plane("Plane 1".into(), SketchPlane::top().offset(7.0));
        app.set_sketch_plane(PlaneKind::Custom(pid));
        add_rect(&mut app, 0.0, 0.0, 3.0, 3.0);
        app.toggle_datum_visible(DatumKind::Plane, pid);
        app.create_datum_axis(Some("A".into()), Axis::new(DVec3::ONE, DVec3::X).unwrap());
        app.create_datum_point(Some("P".into()), DVec3::new(4.0, 5.0, 6.0));
        app.toggle_datum_visible(DatumKind::Point, 1);

        let json = ducad_io::native::serialize_full_to_json(&[], &[], None, None, Some(&app.native_datums())).unwrap();
        let loaded = ducad_io::native::deserialize_from_json(&json).unwrap();

        let mut other = DuCADApp::new_for_test();
        other.datum_planes.clear();
        other.load_native_datums(loaded.datums);
        assert_eq!(other.datum_planes.len(), 1);
        assert!(!other.datum_planes[0].visible);
        assert!((other.datum_planes[0].plane.origin.z - 7.0).abs() < 1e-5);
        assert_eq!(other.sketch_at_index(3).entities.len(), 4, "sketsa bidang datum ikut tersimpan");
        assert_eq!(other.datums.axes[0].name, "A");
        assert!(!other.datums.points[0].visible);
        // Id baru tidak bertabrakan dengan yang dimuat.
        let next = other.create_datum_axis(None, Axis::new(DVec3::ZERO, DVec3::Y).unwrap());
        assert!(next > other.datums.axes[0].id);
        let next_plane = other.create_datum_plane("x".into(), SketchPlane::top());
        assert!(next_plane > pid);

        // Entri rusak dilewati, tidak panik.
        let mut bad = ducad_io::native::NativeDatums::default();
        bad.axes.push(ducad_io::native::NativeDatumAxis {
            id: 1,
            name: "nol".into(),
            origin: [0.0; 3],
            dir: [0.0; 3],
            visible: true,
        });
        bad.points.push(ducad_io::native::NativeDatumPoint { id: 1, name: "nan".into(), pos: [f64::NAN, 0.0, 0.0], visible: true });
        other.load_native_datums(bad);
        assert!(other.datums.axes.is_empty() && other.datums.points.is_empty());
    }

    #[test]
    fn palette_commands_open_datum_tool_and_drawer() {
        let mut app = DuCADApp::new_for_test();
        let ctx = egui::Context::default();
        app.run_parity_cmd(crate::types::ParityCmd::DatumAxis, &ctx);
        assert_eq!(app.tool, ToolKind::DatumPlane);
        assert_eq!(app.datums.kind, DatumKind::Axis);
        app.run_parity_cmd(crate::types::ParityCmd::DatumPoint, &ctx);
        assert_eq!(app.datums.kind, DatumKind::Point);
        app.planes_drawer_open = false;
        app.run_parity_cmd(crate::types::ParityCmd::DatumDrawer, &ctx);
        assert!(app.planes_drawer_open);
    }
}
