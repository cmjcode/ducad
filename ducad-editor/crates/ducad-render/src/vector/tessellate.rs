//! Tesselasi CPU untuk entitas grafis vektor 2D ke mesh wgpu (`VectorVertex`).
//!
//! Menggunakan `lyon_tessellation` untuk fill (non-zero / even-odd) dan stroke (cap/join/dash).
//! Semua komputasi deterministik dan tanpa dependensi GPU.

use ducad_sketch::{
    Entity, FillRule, LineCap, LineJoin, Paint, PathSeg, StrokeStyle, Style, Subpath,
};
use glam::DVec2;
use lyon_path::Path;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule as LyonFillRule, FillTessellator, FillVertex,
    LineCap as LyonLineCap, LineJoin as LyonLineJoin, StrokeOptions, StrokeTessellator,
    StrokeVertex, VertexBuffers,
};

use crate::plane::SketchPlane;

/// Langkah offset bidang Z per layer agar tidak terjadi z-fighting.
pub const Z_LAYER_STEP: f32 = 0.005;
/// Offset dasar di atas bidang sketsa (0.02 mm).
pub const Z_BASE_OFFSET: f32 = 0.02;

/// Format vertex untuk rendering vektor GPU di `SceneRenderer`.
/// Ukuran tepat 48 byte (dengan padding 8 byte untuk 16-byte alignment).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VectorVertex {
    /// Koordinat DUNIA 3D (posisi 2D di bidang sketsa dipetakan lewat `SketchPlane::to_world`).
    pub pos: [f32; 3],
    /// Warna solid [r, g, b, a] non-premultiplied.
    pub color: [f32; 4],
    /// Tipe cat: 0 = solid; n > 0 = indeks gradien pada batch uniform.
    pub paint: u32,
    /// Koordinat 2D sketsa lokal (mm) untuk evaluasi gradien di fragment shader.
    pub uv: [f32; 2],
    /// Padding agar ukuran struct tepat 48 byte sesuai layout shader.
    pub _pad: [f32; 2],
}

impl VectorVertex {
    #[inline]
    pub fn new(pos: [f32; 3], color: [f32; 4], paint: u32, uv: [f32; 2]) -> Self {
        Self {
            pos,
            color,
            paint,
            uv,
            _pad: [0.0, 0.0],
        }
    }
}

/// Hasil tesselasi CPU: kumpulan vertex dan indeks segitiga.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tessellated {
    pub vertices: Vec<VectorVertex>,
    pub indices: Vec<u32>,
}

impl Tessellated {
    #[inline]
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.indices.is_empty()
    }

    /// Gabungkan hasil tesselasi lain ke dalam buffer ini dengan offset indeks.
    pub fn append(&mut self, other: Tessellated) {
        if other.is_empty() {
            return;
        }
        let base_index = self.vertices.len() as u32;
        self.vertices.extend(other.vertices);
        self.indices
            .extend(other.indices.into_iter().map(|idx| idx + base_index));
    }
}

/// Opsi tesselasi CPU.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TessOptions {
    /// Toleransi aproksimasi kurva ke segmen linier (mm), default 0.02 mm.
    pub tol_mm: f64,
    /// Rasio piksel per milimeter viewport (untuk skala hairline dan dash).
    pub px_per_mm: f32,
    /// Indeks lapisan (layer) untuk menentukan tinggi Z_OFFSET relatif.
    pub layer_index: u32,
    /// Opasitas global entitas (0.0..=1.0).
    pub opacity: f32,
}

impl Default for TessOptions {
    fn default() -> Self {
        Self {
            tol_mm: 0.02,
            px_per_mm: 1.0,
            layer_index: 0,
            opacity: 1.0,
        }
    }
}

/// Error saat tesselasi CPU.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TessError {
    #[error("koordinat memuat nilai tidak hingga (NaN atau tak hingga)")]
    NonFiniteCoordinate,
    #[error("geometri degenerate: {0}")]
    Degenerate(String),
    #[error("tesselasi lyon gagal: {0}")]
    LyonError(String),
    #[error("entitas tidak didukung atau tidak valid")]
    InvalidEntity,
}

/// Validasi semua titik dalam deretan subpath apakah bernilai hingga (bukan NaN/Inf).
pub fn validate_subpaths_finite(subpaths: &[Subpath]) -> Result<(), TessError> {
    for sub in subpaths {
        if !sub.start.is_finite() {
            return Err(TessError::NonFiniteCoordinate);
        }
        for seg in &sub.segs {
            match seg {
                PathSeg::Line { end } => {
                    if !end.is_finite() {
                        return Err(TessError::NonFiniteCoordinate);
                    }
                }
                PathSeg::Cubic { c1, c2, end } => {
                    if !c1.is_finite() || !c2.is_finite() || !end.is_finite() {
                        return Err(TessError::NonFiniteCoordinate);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Ekstrak deretan `Subpath` representatif dari suatu `Entity`.
pub fn entity_to_subpaths(entity: &Entity) -> Result<Vec<Subpath>, TessError> {
    match entity {
        Entity::Path { subpaths, .. } => {
            validate_subpaths_finite(subpaths)?;
            Ok(subpaths.clone())
        }
        Entity::Circle { center, radius, .. } => {
            if !center.is_finite() || !radius.is_finite() {
                return Err(TessError::NonFiniteCoordinate);
            }
            if *radius <= 0.0 {
                return Err(TessError::Degenerate("radius lingkaran <= 0".into()));
            }
            let (cx, cy, r) = (center.x, center.y, *radius);
            let k = 0.5522847498307935 * r;
            Ok(vec![Subpath {
                start: DVec2::new(cx + r, cy),
                segs: vec![
                    PathSeg::Cubic {
                        c1: DVec2::new(cx + r, cy + k),
                        c2: DVec2::new(cx + k, cy + r),
                        end: DVec2::new(cx, cy + r),
                    },
                    PathSeg::Cubic {
                        c1: DVec2::new(cx - k, cy + r),
                        c2: DVec2::new(cx - r, cy + k),
                        end: DVec2::new(cx - r, cy),
                    },
                    PathSeg::Cubic {
                        c1: DVec2::new(cx - r, cy - k),
                        c2: DVec2::new(cx - k, cy - r),
                        end: DVec2::new(cx, cy - r),
                    },
                    PathSeg::Cubic {
                        c1: DVec2::new(cx + k, cy - r),
                        c2: DVec2::new(cx + r, cy - k),
                        end: DVec2::new(cx + r, cy),
                    },
                ],
                closed: true,
            }])
        }
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        } => {
            if !center.is_finite() || !radius_x.is_finite() || !radius_y.is_finite() {
                return Err(TessError::NonFiniteCoordinate);
            }
            if *radius_x <= 0.0 || *radius_y <= 0.0 {
                return Err(TessError::Degenerate("radius elips <= 0".into()));
            }
            let (cx, cy, rx, ry) = (center.x, center.y, *radius_x, *radius_y);
            let kx = 0.5522847498307935 * rx;
            let ky = 0.5522847498307935 * ry;
            Ok(vec![Subpath {
                start: DVec2::new(cx + rx, cy),
                segs: vec![
                    PathSeg::Cubic {
                        c1: DVec2::new(cx + rx, cy + ky),
                        c2: DVec2::new(cx + kx, cy + ry),
                        end: DVec2::new(cx, cy + ry),
                    },
                    PathSeg::Cubic {
                        c1: DVec2::new(cx - kx, cy + ry),
                        c2: DVec2::new(cx - rx, cy + ky),
                        end: DVec2::new(cx - rx, cy),
                    },
                    PathSeg::Cubic {
                        c1: DVec2::new(cx - rx, cy - ky),
                        c2: DVec2::new(cx - kx, cy - ry),
                        end: DVec2::new(cx, cy - ry),
                    },
                    PathSeg::Cubic {
                        c1: DVec2::new(cx + kx, cy - ry),
                        c2: DVec2::new(cx + rx, cy - ky),
                        end: DVec2::new(cx + rx, cy),
                    },
                ],
                closed: true,
            }])
        }
        Entity::Line { start, end, .. } => {
            if !start.is_finite() || !end.is_finite() {
                return Err(TessError::NonFiniteCoordinate);
            }
            Ok(vec![Subpath {
                start: *start,
                segs: vec![PathSeg::Line { end: *end }],
                closed: false,
            }])
        }
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => {
            if !center.is_finite()
                || !radius.is_finite()
                || !start_angle.is_finite()
                || !end_angle.is_finite()
            {
                return Err(TessError::NonFiniteCoordinate);
            }
            // Cacah busur sudut ke segmen kurbo
            let sweep = end_angle - start_angle;
            let n_steps = (sweep.abs() / (std::f64::consts::PI / 16.0))
                .ceil()
                .max(4.0) as usize;
            let mut segs = Vec::with_capacity(n_steps);
            let start = DVec2::new(
                center.x + radius * start_angle.cos(),
                center.y + radius * start_angle.sin(),
            );
            for i in 1..=n_steps {
                let frac = i as f64 / n_steps as f64;
                let ang = start_angle + sweep * frac;
                let end = DVec2::new(center.x + radius * ang.cos(), center.y + radius * ang.sin());
                segs.push(PathSeg::Line { end });
            }
            Ok(vec![Subpath {
                start,
                segs,
                closed: false,
            }])
        }
        Entity::Spline { points, exact, .. } => {
            for p in points {
                if !p.is_finite() {
                    return Err(TessError::NonFiniteCoordinate);
                }
            }
            if points.is_empty() {
                return Err(TessError::Degenerate("spline kosong".into()));
            }
            if let Some(segs) = exact {
                for seg in segs {
                    match seg {
                        PathSeg::Line { end } => {
                            if !end.is_finite() {
                                return Err(TessError::NonFiniteCoordinate);
                            }
                        }
                        PathSeg::Cubic { c1, c2, end } => {
                            if !c1.is_finite() || !c2.is_finite() || !end.is_finite() {
                                return Err(TessError::NonFiniteCoordinate);
                            }
                        }
                    }
                }
                let is_closed = points.first() == points.last() && points.len() > 1;
                Ok(vec![Subpath {
                    start: points[0],
                    segs: segs.clone(),
                    closed: is_closed,
                }])
            } else {
                let mut segs = Vec::with_capacity(points.len().saturating_sub(1));
                for p in &points[1..] {
                    segs.push(PathSeg::Line { end: *p });
                }
                let is_closed = points.first() == points.last() && points.len() > 2;
                Ok(vec![Subpath {
                    start: points[0],
                    segs,
                    closed: is_closed,
                }])
            }
        }
    }
}

/// Tesselasi fill untuk deretan subpath tertutup sesuai `FillRule`.
pub fn tessellate_fill(
    subpaths: &[Subpath],
    fill: &Paint,
    rule: FillRule,
    plane: &SketchPlane,
    opts: &TessOptions,
) -> Result<Tessellated, TessError> {
    validate_subpaths_finite(subpaths)?;

    if subpaths.is_empty() {
        return Err(TessError::Degenerate("path kosong".into()));
    }

    let mut total_points = 0;
    for sub in subpaths {
        total_points += 1 + sub.segs.len();
    }
    if total_points < 3 {
        return Err(TessError::Degenerate(format!(
            "path memiliki {total_points} titik, membutuhkan minimal 3 titik untuk fill"
        )));
    }

    let mut builder = Path::builder();
    let mut has_segments = false;
    for sub in subpaths {
        if sub.segs.is_empty() && !sub.closed {
            continue;
        }
        builder.begin(lyon_path::geom::point(
            sub.start.x as f32,
            sub.start.y as f32,
        ));
        for seg in &sub.segs {
            has_segments = true;
            match seg {
                PathSeg::Line { end } => {
                    builder.line_to(lyon_path::geom::point(end.x as f32, end.y as f32));
                }
                PathSeg::Cubic { c1, c2, end } => {
                    builder.cubic_bezier_to(
                        lyon_path::geom::point(c1.x as f32, c1.y as f32),
                        lyon_path::geom::point(c2.x as f32, c2.y as f32),
                        lyon_path::geom::point(end.x as f32, end.y as f32),
                    );
                }
            }
        }
        builder.end(true); // Fill selalu mengasumsikan loop tertutup
    }

    if !has_segments {
        return Err(TessError::Degenerate(
            "tidak ada segmen untuk di-fill".into(),
        ));
    }

    let path = builder.build();

    let lyon_rule = match rule {
        FillRule::NonZero => LyonFillRule::NonZero,
        FillRule::EvenOdd => LyonFillRule::EvenOdd,
    };
    let fill_options =
        FillOptions::tolerance(opts.tol_mm.max(0.001) as f32).with_fill_rule(lyon_rule);

    let mut tessellator = FillTessellator::new();
    let mut geometry: VertexBuffers<VectorVertex, u32> = VertexBuffers::new();

    let (color, paint) = match fill {
        Paint::Solid(rgba) => {
            let mut c = rgba.0;
            c[3] = (c[3] * opts.opacity).clamp(0.0, 1.0);
            (c, 0u32)
        }
        Paint::Linear { .. } | Paint::Radial { .. } => {
            let mut c = fill.average_color().0;
            c[3] = (c[3] * opts.opacity).clamp(0.0, 1.0);
            (c, 1u32)
        }
    };

    let z_offset = Z_BASE_OFFSET + (opts.layer_index as f32 * Z_LAYER_STEP);

    tessellator
        .tessellate_path(
            &path,
            &fill_options,
            &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
                let p = vertex.position();
                let p2d = DVec2::new(p.x as f64, p.y as f64);
                let world = plane.to_world(p2d, z_offset);
                VectorVertex::new([world.x, world.y, world.z], color, paint, [p.x, p.y])
            }),
        )
        .map_err(|e| TessError::LyonError(format!("{e:?}")))?;

    Ok(Tessellated {
        vertices: geometry.vertices,
        indices: geometry.indices,
    })
}

/// Konversi subpath ke deretan poliline bersegmen, dengan penanganan pola garis putus-putus (`dash`).
pub fn subpaths_to_stroked_polylines(
    subpaths: &[Subpath],
    dash: &[f64],
    tol_mm: f64,
) -> Vec<Vec<DVec2>> {
    let mut polylines = Vec::new();
    let has_valid_dash = !dash.is_empty() && dash.iter().all(|&d| d > 0.0 && d.is_finite());

    for sub in subpaths {
        let pts = sub.flatten(tol_mm);
        if pts.len() < 2 {
            continue;
        }

        if !has_valid_dash {
            polylines.push(pts);
        } else {
            // Gunakan kurbo::dash pada kurva yang telah diflatten
            let mut bez = kurbo::BezPath::new();
            bez.move_to(kurbo::Point::new(pts[0].x, pts[0].y));
            for p in &pts[1..] {
                bez.line_to(kurbo::Point::new(p.x, p.y));
            }

            // Normalisasi array dash jika ganjil (Corel/SVG convention: repeat once)
            let mut norm_dash = dash.to_vec();
            if norm_dash.len() % 2 == 1 {
                norm_dash.extend_from_slice(dash);
            }

            let dashed = kurbo::dash(bez.into_iter(), 0.0, &norm_dash);
            let mut cur_poly = Vec::new();
            for el in dashed {
                match el {
                    kurbo::PathEl::MoveTo(p) => {
                        if cur_poly.len() >= 2 {
                            polylines.push(std::mem::take(&mut cur_poly));
                        } else {
                            cur_poly.clear();
                        }
                        cur_poly.push(DVec2::new(p.x, p.y));
                    }
                    kurbo::PathEl::LineTo(p) => {
                        cur_poly.push(DVec2::new(p.x, p.y));
                    }
                    kurbo::PathEl::QuadTo(p1, p2) => {
                        cur_poly.push(DVec2::new(p1.x, p1.y));
                        cur_poly.push(DVec2::new(p2.x, p2.y));
                    }
                    kurbo::PathEl::CurveTo(p1, p2, p3) => {
                        cur_poly.push(DVec2::new(p1.x, p1.y));
                        cur_poly.push(DVec2::new(p2.x, p2.y));
                        cur_poly.push(DVec2::new(p3.x, p3.y));
                    }
                    kurbo::PathEl::ClosePath => {
                        if let Some(&first) = cur_poly.first() {
                            if cur_poly.last() != Some(&first) {
                                cur_poly.push(first);
                            }
                        }
                        if cur_poly.len() >= 2 {
                            polylines.push(std::mem::take(&mut cur_poly));
                        }
                    }
                }
            }
            if cur_poly.len() >= 2 {
                polylines.push(cur_poly);
            }
        }
    }

    polylines
}

/// Tesselasi stroke (outline) dari deretan poliline dengan lebar dan gaya tertentu.
pub fn tessellate_stroke(
    polylines: &[Vec<DVec2>],
    stroke: &StrokeStyle,
    plane: &SketchPlane,
    opts: &TessOptions,
) -> Tessellated {
    if stroke.width_mm <= 0.0 || polylines.is_empty() {
        return Tessellated::empty();
    }

    let line_cap = match stroke.cap {
        LineCap::Butt => LyonLineCap::Butt,
        LineCap::Round => LyonLineCap::Round,
        LineCap::Square => LyonLineCap::Square,
    };
    let line_join = match stroke.join {
        LineJoin::Miter => LyonLineJoin::Miter,
        LineJoin::Round => LyonLineJoin::Round,
        LineJoin::Bevel => LyonLineJoin::Bevel,
    };
    let stroke_options = StrokeOptions::tolerance(opts.tol_mm.max(0.001) as f32)
        .with_line_width(stroke.width_mm as f32)
        .with_line_cap(line_cap)
        .with_line_join(line_join);

    let mut builder = Path::builder();
    let mut has_lines = false;

    for poly in polylines {
        if poly.len() < 2 {
            continue;
        }
        let any_non_finite = poly.iter().any(|p| !p.is_finite());
        if any_non_finite {
            continue;
        }

        has_lines = true;
        builder.begin(lyon_path::geom::point(poly[0].x as f32, poly[0].y as f32));
        for p in &poly[1..] {
            builder.line_to(lyon_path::geom::point(p.x as f32, p.y as f32));
        }
        let is_closed = (poly.first().unwrap() - poly.last().unwrap()).length_squared() < 1e-12;
        builder.end(is_closed);
    }

    if !has_lines {
        return Tessellated::empty();
    }

    let path = builder.build();

    let mut tessellator = StrokeTessellator::new();
    let mut geometry: VertexBuffers<VectorVertex, u32> = VertexBuffers::new();

    let (color, paint) = match &stroke.paint {
        Paint::Solid(rgba) => {
            let mut c = rgba.0;
            c[3] = (c[3] * opts.opacity).clamp(0.0, 1.0);
            (c, 0u32)
        }
        Paint::Linear { .. } | Paint::Radial { .. } => {
            let mut c = stroke.paint.average_color().0;
            c[3] = (c[3] * opts.opacity).clamp(0.0, 1.0);
            (c, 1u32)
        }
    };

    // Stroke digambar sedikit di atas fill layer yang sama untuk menghindari z-fighting
    let z_offset = Z_BASE_OFFSET + (opts.layer_index as f32 * Z_LAYER_STEP) + 0.001;

    let _ = tessellator.tessellate_path(
        &path,
        &stroke_options,
        &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
            let p = vertex.position();
            let p2d = DVec2::new(p.x as f64, p.y as f64);
            let world = plane.to_world(p2d, z_offset);
            VectorVertex::new([world.x, world.y, world.z], color, paint, [p.x, p.y])
        }),
    );

    Tessellated {
        vertices: geometry.vertices,
        indices: geometry.indices,
    }
}

/// Fill + stroke satu entitas sesuai `Style`. Deterministik.
///
/// Hairline (`width_mm == 0` atau `Style::cad_default`) tidak ditesselasi:
/// tetap lewat `LineVertex` lama (jalur render sketch CAD yang ada).
pub fn tessellate_entity(
    entity: &Entity,
    style: &Style,
    plane: &SketchPlane,
    opts: &TessOptions,
) -> Result<Tessellated, TessError> {
    // Hairline / CAD default: tidak ditesselasi, dibiarkan kosong
    if style.is_cad_default() {
        return Ok(Tessellated::empty());
    }
    if style.fill.is_none() && style.stroke.as_ref().is_none_or(|s| s.width_mm <= 0.0) {
        return Ok(Tessellated::empty());
    }

    let subpaths = entity_to_subpaths(entity)?;

    let mut result = Tessellated::empty();

    // 1. Tesselasi Fill jika ada
    if let Some(ref fill) = style.fill {
        let fill_opts = TessOptions {
            opacity: opts.opacity * style.opacity,
            ..*opts
        };
        match tessellate_fill(&subpaths, fill, style.fill_rule, plane, &fill_opts) {
            Ok(fill_tess) => result.append(fill_tess),
            Err(e) => {
                if matches!(e, TessError::NonFiniteCoordinate) {
                    return Err(e);
                }
                // Jika tidak ada stroke yang bisa digambar, teruskan error
                if style.stroke.as_ref().is_none_or(|s| s.width_mm <= 0.0) {
                    return Err(e);
                }
                log::warn!("Fill tesselation gagal, hanya menggambar stroke: {e}");
            }
        }
    }

    // 2. Tesselasi Stroke jika ada lebar > 0
    if let Some(ref stroke) = style.stroke {
        if stroke.width_mm > 0.0 {
            let stroke_opts = TessOptions {
                opacity: opts.opacity * style.opacity,
                ..*opts
            };
            let polylines = subpaths_to_stroked_polylines(&subpaths, &stroke.dash, opts.tol_mm);
            let mut stroke_tess = tessellate_stroke(&polylines, stroke, plane, &stroke_opts);
            // Indeks gradien per entitas mengikuti urutan cache: gradien fill
            // (bila ada) di slot 1, gradien stroke sesudahnya. Tanpa geseran
            // ini stroke bergradien memakai gradien fill.
            let fill_gradients = u32::from(
                style
                    .fill
                    .as_ref()
                    .is_some_and(|f| !matches!(f, Paint::Solid(_))),
            );
            if fill_gradients > 0 {
                for v in stroke_tess.vertices.iter_mut().filter(|v| v.paint > 0) {
                    v.paint += fill_gradients;
                }
            }
            result.append(stroke_tess);
        }
    }

    Ok(result)
}
