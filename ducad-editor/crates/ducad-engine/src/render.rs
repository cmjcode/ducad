//! Render SVG/PNG deterministik untuk umpan balik visual agent (P2.3).

use ducad_kernel::{HlrLineKind, SnapshotBody, SnapshotCamera, SnapshotOptions};
use glam::Vec3;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{OpError, OpErrorCode, OpResult};
use crate::session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum View {
    #[default]
    Iso,
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

impl View {
    /// (arah `eye − target` belum dinormalisasi, `up`).
    fn direction_up(self) -> (Vec3, Vec3) {
        match self {
            View::Iso => (Vec3::new(1.0, -1.0, 1.0), Vec3::Z),
            View::Front => (Vec3::new(0.0, -1.0, 0.0), Vec3::Z),
            View::Back => (Vec3::new(0.0, 1.0, 0.0), Vec3::Z),
            View::Right => (Vec3::new(1.0, 0.0, 0.0), Vec3::Z),
            View::Left => (Vec3::new(-1.0, 0.0, 0.0), Vec3::Z),
            View::Top => (Vec3::new(0.0, 0.0, 1.0), Vec3::Y),
            View::Bottom => (Vec3::new(0.0, 0.0, -1.0), Vec3::Y),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub view: View,
    pub width: u32,
    pub height: u32,
    pub hidden_lines: bool,
    /// `None` = semua body terlihat.
    pub bodies: Option<Vec<String>>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            view: View::Iso,
            width: 800,
            height: 600,
            hidden_lines: false,
            bodies: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderResult {
    pub svg: String,
    pub visible_segments: usize,
    pub hidden_segments: usize,
}

const FOV_Y_DEG: f32 = 45.0;
const MAX_SVG_BYTES: usize = 300 * 1024;

/// Kamera ortografik deterministik yang membingkai bbox `lo..hi`.
fn camera_for_bbox(lo: Vec3, hi: Vec3, view: View, width: u32, height: u32) -> SnapshotCamera {
    let c = (lo + hi) * 0.5;
    let diag = (hi - lo).length().max(1.0);
    let fov = FOV_Y_DEG.to_radians();
    let dist = 0.6 * diag / (fov * 0.5).tan();
    let (dir, up) = view.direction_up();
    SnapshotCamera {
        eye: c + dir.normalize() * dist,
        target: c,
        up,
        fov_y: fov,
        width_px: width as f32,
        height_px: height as f32,
        near: 0.1,
        orthographic: true,
    }
}

/// Warna lapisan volume yang bertambah pada render diff.
pub const DIFF_ADDED_COLOR: &str = "#16a34a";
/// Warna lapisan volume yang hilang pada render diff.
pub const DIFF_REMOVED_COLOR: &str = "#dc2626";

/// Render diff berwarna (P8.3): lapisan 1 = body versi baru `b` (warna
/// default), lapisan 2 = volume bertambah (hijau), lapisan 3 = volume hilang
/// (merah, putus-putus). Oklusi antar-lapisan diabaikan; tanpa garis
/// tersembunyi.
pub fn render_diff_svg(
    b: &Session,
    shapes: &crate::diff::DiffShapes,
    view: View,
    width: u32,
    height: u32,
) -> OpResult<RenderResult> {
    use crate::model::BodyGeometry;
    let model = b.model();
    let base: Vec<&BodyGeometry> = model
        .doc
        .bodies
        .iter()
        .filter(|(_, body)| body.visible)
        .filter_map(|(id, _)| model.geometry.get(id))
        .collect();
    let clone_geo = |s: &ducad_kernel::KernelShape| -> OpResult<BodyGeometry> {
        let c = ducad_kernel::clone_shape(s).map_err(|e| OpError::kernel("Render diff", e))?;
        Ok(BodyGeometry::from_shape(c))
    };
    let added = shapes
        .added
        .iter()
        .map(clone_geo)
        .collect::<OpResult<Vec<_>>>()?;
    let removed = shapes
        .removed
        .iter()
        .map(clone_geo)
        .collect::<OpResult<Vec<_>>>()?;
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for g in base
        .iter()
        .copied()
        .chain(added.iter())
        .chain(removed.iter())
    {
        if let Some((a, z)) = g.mesh.bounding_box() {
            lo = lo.min(Vec3::from_array(a));
            hi = hi.max(Vec3::from_array(z));
        }
    }
    if lo.x > hi.x {
        return Err(OpError::invalid("tidak ada body untuk dirender"));
    }
    let camera = camera_for_bbox(lo, hi, view, width, height);
    let options = SnapshotOptions {
        include_hidden: false,
        ..SnapshotOptions::default()
    };
    let snap = |geos: &[&BodyGeometry]| {
        let bodies: Vec<SnapshotBody> = geos
            .iter()
            .map(|g| SnapshotBody::new(&g.edge_lines, &g.mesh))
            .collect();
        ducad_kernel::extract_vector_snapshot(&camera, &bodies, &[], &options)
    };
    let s_base = snap(&base);
    let s_added = snap(&added.iter().collect::<Vec<_>>());
    let s_removed = snap(&removed.iter().collect::<Vec<_>>());
    let o_base = ducad_io::svg::SvgSnapshotOptions {
        background: Some("#ffffff".into()),
        include_hidden: false,
        ..Default::default()
    };
    let colored = |color: &str, dash: Option<&str>| ducad_io::svg::SvgSnapshotOptions {
        visible_color: color.into(),
        silhouette_color: color.into(),
        visible_stroke_px: 2.0,
        silhouette_stroke_px: 1.6,
        include_hidden: false,
        visible_dasharray: dash.map(str::to_string),
        ..Default::default()
    };
    let (o_added, o_removed) = (
        colored(DIFF_ADDED_COLOR, None),
        colored(DIFF_REMOVED_COLOR, Some("5 3")),
    );
    let svg = ducad_io::svg::export_vector_snapshot_svg_layers(&[
        (&s_base, &o_base),
        (&s_added, &o_added),
        (&s_removed, &o_removed),
    ])
    .map_err(|e| OpError::new(OpErrorCode::Io, format!("gagal membuat SVG diff: {e:#}")))?;
    let svg = match svg.find("<svg") {
        Some(i) => svg[i..].to_string(),
        None => svg,
    };
    let count = |s: &ducad_kernel::VectorSnapshot| s.segments.len();
    Ok(RenderResult {
        svg,
        visible_segments: count(&s_base) + count(&s_added) + count(&s_removed),
        hidden_segments: 0,
    })
}

/// Render body sesi sebagai SVG garis (tampak + opsional tersembunyi).
pub fn render_svg(s: &Session, opt: &RenderOptions) -> OpResult<RenderResult> {
    if opt.width == 0 || opt.height == 0 || opt.width > 8192 || opt.height > 8192 {
        return Err(OpError::invalid(format!(
            "ukuran render harus 1..=8192 piksel (diberikan {}x{})",
            opt.width, opt.height
        )));
    }
    let model = s.model();
    let mut picked = Vec::new();
    match &opt.bodies {
        Some(names) => {
            for n in names {
                let (id, geo) = s.body(n)?;
                picked.push((id, geo));
            }
        }
        None => {
            for (id, b) in model.doc.bodies.iter() {
                if let (true, Some(geo)) = (b.visible, model.geometry.get(id)) {
                    picked.push((id, geo));
                }
            }
        }
    }
    if picked.is_empty() {
        return Err(OpError::invalid("tidak ada body untuk dirender"));
    }

    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for (_, g) in &picked {
        if let Some((a, b)) = g.mesh.bounding_box() {
            lo = lo.min(Vec3::from_array(a));
            hi = hi.max(Vec3::from_array(b));
        }
    }
    if lo.x > hi.x {
        return Err(OpError::new(
            OpErrorCode::EmptyResult,
            "body yang dipilih tidak punya mesh",
        ));
    }
    let camera = camera_for_bbox(lo, hi, opt.view, opt.width, opt.height);
    let snap_bodies: Vec<SnapshotBody> = picked
        .iter()
        .map(|(_, g)| SnapshotBody::new(&g.edge_lines, &g.mesh))
        .collect();

    let mut min_segment_px = SnapshotOptions::default().min_segment_px;
    let mut attempt = 0;
    loop {
        let options = SnapshotOptions {
            include_hidden: opt.hidden_lines,
            min_segment_px,
            ..SnapshotOptions::default()
        };
        let snap = ducad_kernel::extract_vector_snapshot(&camera, &snap_bodies, &[], &options);
        let svg_opts = ducad_io::svg::SvgSnapshotOptions {
            background: Some("#ffffff".into()),
            include_hidden: opt.hidden_lines,
            ..Default::default()
        };
        let svg = ducad_io::svg::export_vector_snapshot_svg_string(&snap, &svg_opts)
            .map_err(|e| OpError::new(OpErrorCode::Io, format!("gagal membuat SVG: {e:#}")))?;
        attempt += 1;
        if svg.len() > MAX_SVG_BYTES && attempt <= 3 {
            min_segment_px *= 2.0;
            continue;
        }
        let hidden = snap.count(HlrLineKind::Hidden);
        // Prolog XML dibuang: opsional di SVG, dan hasilnya jadi diawali `<svg`.
        let svg = match svg.find("<svg") {
            Some(i) => svg[i..].to_string(),
            None => svg,
        };
        return Ok(RenderResult {
            svg,
            visible_segments: snap.segments.len() - hidden,
            hidden_segments: hidden,
        });
    }
}

/// Rasterisasi SVG ke PNG berlatar putih.
#[cfg(feature = "raster")]
pub fn svg_to_png(svg: &str, width: u32, height: u32) -> OpResult<Vec<u8>> {
    use resvg::tiny_skia::{Color, Pixmap, Transform};
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .map_err(|e| OpError::new(OpErrorCode::Io, format!("SVG tidak valid: {e}")))?;
    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| OpError::invalid(format!("ukuran PNG tidak valid {width}x{height}")))?;
    pixmap.fill(Color::WHITE);
    let size = tree.size();
    let transform =
        Transform::from_scale(width as f32 / size.width(), height as f32 / size.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap
        .encode_png()
        .map_err(|e| OpError::new(OpErrorCode::Io, format!("gagal encode PNG: {e}")))
}
