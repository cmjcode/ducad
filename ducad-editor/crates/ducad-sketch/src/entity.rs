use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

use crate::constraint::PointRef;

slotmap::new_key_type! {
    /// Identitas stabil entitas sketch.
    pub struct EntityId;
}

/// Entitas sketch 2D (koordinat lokal bidang sketch, presisi f64).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Entity {
    Line {
        start: DVec2,
        end: DVec2,
        #[serde(default)]
        is_construction: bool,
    },
    Circle {
        center: DVec2,
        radius: f64,
        #[serde(default)]
        is_construction: bool,
    },
    Arc {
        center: DVec2,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        #[serde(default)]
        is_construction: bool,
    },
    /// Ellips axis-aligned (sumbu sejajar X/Y).
    Ellipse {
        center: DVec2,
        radius_x: f64,
        radius_y: f64,
        #[serde(default)]
        is_construction: bool,
    },
    /// Kurva Spline halus yang melalui deretan titik kontrol/fit (Catmull-Rom).
    Spline {
        points: Vec<DVec2>,
        /// Bentuk kurva ASLI, bila entitas ini lahir dari sumber yang memang
        /// punya definisi eksak — terutama outline glyph font.
        ///
        /// `points` selalu terisi sebagai hasil pencacahan (flatten) supaya
        /// tampilan, snap, region, dan berkas lama tidak berubah sedikit pun.
        /// `exact` cuma dipakai saat entitas ini jadi profil B-rep, agar
        /// kurvanya masuk ke kernel sebagai kurva sungguhan, bukan puluhan
        /// ruas lurus yang membuat dinding hasil extrude jadi patah-patah.
        ///
        /// `None` berarti kurva bebas biasa (mis. digambar tangan, atau hasil
        /// offset) — perilakunya persis seperti sebelum field ini ada.
        #[serde(default)]
        exact: Option<Vec<PathSeg>>,
        #[serde(default)]
        is_construction: bool,
    },
    /// Path vektor (M0.1). Koordinat mm, bidang sketch.
    Path {
        subpaths: Vec<Subpath>,
        #[serde(default)]
        is_construction: bool,
    },
}

/// Satu rangkaian segmen bersambung. `closed = true` berarti ada ruas
/// implisit dari titik akhir terakhir kembali ke `start`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subpath {
    pub start: DVec2,
    pub segs: Vec<PathSeg>, // V2: Line{end} | Cubic{c1,c2,end}
    #[serde(default)]
    pub closed: bool,
}

/// Satu langkah kurva eksak pada [`Entity::Spline`].
///
/// Tiap langkah menyambung dari titik akhir langkah sebelumnya; titik awal
/// langkah pertama adalah `points[0]`. Bentuk relatif ini dipilih supaya
/// rantainya tidak bisa "robek" — ujung yang dipakai bersama hanya disimpan
/// sekali, jadi mustahil ada celah antar segmen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PathSeg {
    /// Ruas lurus menuju `end`.
    Line { end: DVec2 },
    /// Bézier KUBIK dengan dua titik kontrol menuju `end`.
    ///
    /// Bézier kuadratik (yang dipakai TrueType) dinaikkan derajatnya ke kubik
    /// saat dibaca — eksak, tanpa kehilangan presisi — sehingga satu bentuk
    /// ini cukup untuk semua jenis font.
    Cubic { c1: DVec2, c2: DVec2, end: DVec2 },
}

impl PathSeg {
    /// Naikkan derajat Bézier KUADRATIK (satu titik kontrol, bentuk yang
    /// dipakai outline TrueType) menjadi kubik.
    ///
    /// Eksak, bukan aproksimasi — kurva hasilnya identik titik demi titik.
    /// Rumus bakunya `c1 = p0 + 2/3 (ctrl − p0)` dan `c2 = p2 + 2/3 (ctrl − p2)`.
    pub fn cubic_from_quadratic(start: DVec2, ctrl: DVec2, end: DVec2) -> Self {
        const TWO_THIRDS: f64 = 2.0 / 3.0;
        PathSeg::Cubic {
            c1: start + (ctrl - start) * TWO_THIRDS,
            c2: end + (ctrl - end) * TWO_THIRDS,
            end,
        }
    }

    /// Titik akhir langkah ini.
    pub fn end(&self) -> DVec2 {
        match self {
            PathSeg::Line { end } | PathSeg::Cubic { end, .. } => *end,
        }
    }

    /// Terapkan transformasi titik `f` ke seluruh titik langkah ini.
    ///
    /// Aman untuk translasi, rotasi, cermin, dan skala seragam: Bézier bersifat
    /// invarian-affine, jadi mentransformasi titik kontrolnya sama saja dengan
    /// mentransformasi kurvanya.
    pub fn map_points(self, f: impl Fn(DVec2) -> DVec2) -> Self {
        match self {
            PathSeg::Line { end } => PathSeg::Line { end: f(end) },
            PathSeg::Cubic { c1, c2, end } => PathSeg::Cubic {
                c1: f(c1),
                c2: f(c2),
                end: f(end),
            },
        }
    }
}

impl Subpath {
    /// Cacah ke poliline dengan toleransi `tol` mm (kurbo `flatten`).
    /// Untuk `closed`, titik terakhir == `start` (disalin, bukan implisit).
    pub fn flatten(&self, tol: f64) -> Vec<DVec2> {
        if self.segs.is_empty() {
            return if self.closed {
                vec![self.start, self.start]
            } else {
                vec![self.start]
            };
        }
        let kpath = self.to_kurbo();
        let mut pts = Vec::new();
        kurbo::flatten(kpath.iter(), tol, |el| match el {
            kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => {
                pts.push(DVec2::new(p.x, p.y));
            }
            kurbo::PathEl::ClosePath => {
                if let Some(&first) = pts.first() {
                    if pts.last() != Some(&first) {
                        pts.push(first);
                    }
                }
            }
            _ => {}
        });
        if self.closed {
            if let Some(&first) = pts.first() {
                if pts.last() != Some(&first) {
                    pts.push(first);
                }
            }
        }
        pts
    }

    /// Bounding box kontrol (konservatif).
    pub fn bbox(&self) -> (DVec2, DVec2) {
        let mut min = self.start;
        let mut max = self.start;
        for seg in &self.segs {
            match seg {
                PathSeg::Line { end } => {
                    min = min.min(*end);
                    max = max.max(*end);
                }
                PathSeg::Cubic { c1, c2, end } => {
                    min = min.min(*c1).min(*c2).min(*end);
                    max = max.max(*c1).max(*c2).max(*end);
                }
            }
        }
        (min, max)
    }

    /// Luas bertanda (signed area) dari subpath tertutup.
    /// Positif jika CCW (berlawanan jarum jam), negatif jika CW (searah jarum jam).
    pub fn signed_area(&self) -> f64 {
        let pts = self.flatten(0.01);
        if pts.len() < 3 {
            return 0.0;
        }
        let mut sum = 0.0;
        for i in 0..(pts.len() - 1) {
            sum += pts[i].x * pts[i + 1].y - pts[i + 1].x * pts[i].y;
        }
        sum * 0.5
    }

    /// Jumlah node kontrol: 1 + segs.len() (closed: tanpa duplikat).
    pub fn node_count(&self) -> usize {
        1 + self.segs.len()
    }

    /// Mengambil posisi node ke-`i` (0 = start).
    pub fn node(&self, i: usize) -> DVec2 {
        if i == 0 {
            self.start
        } else if i <= self.segs.len() {
            self.segs[i - 1].end()
        } else {
            panic!("Node index {i} out of bounds (node_count = {})", self.node_count());
        }
    }

    /// Geser node ke-`i`, handle ikut kaku (delta sama).
    pub fn set_node(&mut self, i: usize, p: DVec2) {
        let count = self.node_count();
        assert!(i < count, "Node index {i} out of bounds (node_count = {count})");
        let old_pos = self.node(i);
        let delta = p - old_pos;
        if delta.length_squared() == 0.0 {
            return;
        }

        if i == 0 {
            self.start = p;
            if let Some(PathSeg::Cubic { ref mut c1, .. }) = self.segs.first_mut() {
                *c1 += delta;
            }
            if self.closed && !self.segs.is_empty() {
                match self.segs.last_mut() {
                    Some(PathSeg::Cubic { ref mut c2, ref mut end, .. }) => {
                        *c2 += delta;
                        if (*end - old_pos).length_squared() < 1e-10 {
                            *end = p;
                        }
                    }
                    Some(PathSeg::Line { ref mut end })
                        if (*end - old_pos).length_squared() < 1e-10 =>
                    {
                        *end = p;
                    }
                    _ => {}
                }
            }
        } else {
            let seg_idx = i - 1;
            match &mut self.segs[seg_idx] {
                PathSeg::Line { ref mut end } => *end = p,
                PathSeg::Cubic { ref mut c2, ref mut end, .. } => {
                    *c2 += delta;
                    *end = p;
                }
            }
            if seg_idx + 1 < self.segs.len() {
                if let PathSeg::Cubic { ref mut c1, .. } = &mut self.segs[seg_idx + 1] {
                    *c1 += delta;
                }
            } else if self.closed && (self.start - old_pos).length_squared() < 1e-10 {
                self.start = p;
                if let Some(PathSeg::Cubic { ref mut c1, .. }) = self.segs.first_mut() {
                    *c1 += delta;
                }
            }
        }
    }

    /// Konversi ke kurbo::BezPath. Invarian: Cubic dengan c1 == start && c2 == end
    /// diperlakukan sebagai Line (tanpa NaN).
    pub fn to_kurbo(&self) -> kurbo::BezPath {
        let mut path = kurbo::BezPath::new();
        path.move_to(kurbo::Point::new(self.start.x, self.start.y));
        let mut cur = self.start;
        for seg in &self.segs {
            match seg {
                PathSeg::Line { end } => {
                    path.line_to(kurbo::Point::new(end.x, end.y));
                    cur = *end;
                }
                PathSeg::Cubic { c1, c2, end } => {
                    if (*c1 - cur).length_squared() < 1e-12 && (*c2 - *end).length_squared() < 1e-12 {
                        path.line_to(kurbo::Point::new(end.x, end.y));
                    } else {
                        path.curve_to(
                            kurbo::Point::new(c1.x, c1.y),
                            kurbo::Point::new(c2.x, c2.y),
                            kurbo::Point::new(end.x, end.y),
                        );
                    }
                    cur = *end;
                }
            }
        }
        if self.closed {
            path.close_path();
        }
        path
    }

    /// Konversi dari kurbo::BezPath: per MoveTo → satu Subpath; Quad dinaikkan ke Cubic.
    pub fn from_kurbo(path: &kurbo::BezPath) -> Vec<Subpath> {
        let mut subpaths = Vec::new();
        let mut current: Option<Subpath> = None;
        let mut cur = DVec2::ZERO;

        for el in path.elements() {
            match el {
                kurbo::PathEl::MoveTo(p) => {
                    if let Some(sub) = current.take() {
                        subpaths.push(sub);
                    }
                    cur = DVec2::new(p.x, p.y);
                    current = Some(Subpath {
                        start: cur,
                        segs: Vec::new(),
                        closed: false,
                    });
                }
                kurbo::PathEl::LineTo(p) => {
                    let end = DVec2::new(p.x, p.y);
                    if let Some(ref mut sub) = current {
                        sub.segs.push(PathSeg::Line { end });
                    }
                    cur = end;
                }
                kurbo::PathEl::QuadTo(p1, p2) => {
                    let ctrl = DVec2::new(p1.x, p1.y);
                    let end = DVec2::new(p2.x, p2.y);
                    if let Some(ref mut sub) = current {
                        sub.segs.push(PathSeg::cubic_from_quadratic(cur, ctrl, end));
                    }
                    cur = end;
                }
                kurbo::PathEl::CurveTo(p1, p2, p3) => {
                    let c1 = DVec2::new(p1.x, p1.y);
                    let c2 = DVec2::new(p2.x, p2.y);
                    let end = DVec2::new(p3.x, p3.y);
                    if let Some(ref mut sub) = current {
                        sub.segs.push(PathSeg::Cubic { c1, c2, end });
                    }
                    cur = end;
                }
                kurbo::PathEl::ClosePath => {
                    if let Some(ref mut sub) = current {
                        sub.closed = true;
                    }
                }
            }
        }
        if let Some(sub) = current {
            subpaths.push(sub);
        }
        subpaths
    }

    /// Terapkan transformasi titik `f` ke seluruh titik langkah subpath ini.
    pub fn map_points(&self, f: impl Fn(DVec2) -> DVec2 + Copy) -> Self {
        Self {
            start: f(self.start),
            segs: self.segs.iter().map(|s| s.map_points(f)).collect(),
            closed: self.closed,
        }
    }
}

/// Terapkan `f` ke seluruh langkah kurva eksak, mempertahankan `None`.
///
/// Dipakai operasi transformasi sketch supaya kurva eksak ikut bergerak
/// bersama `points`-nya; kalau tidak, keduanya akan saling bertentangan.
pub fn map_exact(
    exact: &Option<Vec<PathSeg>>,
    f: impl Fn(DVec2) -> DVec2 + Copy,
) -> Option<Vec<PathSeg>> {
    exact
        .as_ref()
        .map(|segs| segs.iter().map(|s| s.map_points(f)).collect())
}

impl Entity {
    /// Konstruktor helper untuk Line biasa.
    pub fn line(start: DVec2, end: DVec2) -> Self {
        Self::Line {
            start,
            end,
            is_construction: false,
        }
    }

    /// Konstruktor helper untuk Path biasa.
    pub fn path(subpaths: Vec<Subpath>) -> Self {
        Self::Path {
            subpaths,
            is_construction: false,
        }
    }

    /// Cek apakah entitas adalah Path vektor.
    pub fn is_path(&self) -> bool {
        matches!(self, Entity::Path { .. })
    }

    /// Untuk Path: gabungan flatten semua subpath. Entitas lain: seperti sebelumnya
    /// (fungsi pencacah yang sudah dipakai render/region). `FLATTEN_TOL = 0.01` mm.
    pub fn flatten_all(&self, tol: f64) -> Vec<Vec<DVec2>> {
        match self {
            Entity::Path { subpaths, .. } => {
                subpaths.iter().map(|sub| sub.flatten(tol)).collect()
            }
            Entity::Line { start, end, .. } => {
                vec![vec![*start, *end]]
            }
            Entity::Circle { center, radius, .. } => {
                let r = radius.abs();
                let n = if r > 0.0 && tol > 0.0 && tol < r {
                    let theta = 2.0 * (1.0 - tol / r).acos();
                    ((std::f64::consts::TAU / theta).ceil() as usize).max(16)
                } else {
                    32
                };
                let mut pts = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let a = std::f64::consts::TAU * (i as f64) / (n as f64);
                    pts.push(*center + DVec2::new(r * a.cos(), r * a.sin()));
                }
                vec![pts]
            }
            Entity::Arc { center, radius, start_angle, end_angle, .. } => {
                let r = radius.abs();
                let span = {
                    let s = end_angle - start_angle;
                    if s <= 0.0 { s + std::f64::consts::TAU } else { s }
                };
                let n = if r > 0.0 && tol > 0.0 && tol < r {
                    let theta = 2.0 * (1.0 - tol / r).acos();
                    ((span / theta).ceil() as usize).max(8)
                } else {
                    16
                };
                let mut pts = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let a = start_angle + span * (i as f64) / (n as f64);
                    pts.push(*center + DVec2::new(r * a.cos(), r * a.sin()));
                }
                vec![pts]
            }
            Entity::Ellipse { center, radius_x, radius_y, .. } => {
                let rx = radius_x.abs();
                let ry = radius_y.abs();
                let max_r = rx.max(ry);
                let n = if max_r > 0.0 && tol > 0.0 && tol < max_r {
                    let theta = 2.0 * (1.0 - tol / max_r).acos();
                    ((std::f64::consts::TAU / theta).ceil() as usize).max(32)
                } else {
                    64
                };
                let mut pts = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let a = std::f64::consts::TAU * (i as f64) / (n as f64);
                    pts.push(*center + DVec2::new(rx * a.cos(), ry * a.sin()));
                }
                vec![pts]
            }
            Entity::Spline { points, exact, .. } => {
                if let Some(segs) = exact {
                    if let Some(start) = points.first() {
                        let sub = Subpath {
                            start: *start,
                            segs: segs.clone(),
                            closed: false,
                        };
                        return vec![sub.flatten(tol)];
                    }
                }
                if points.is_empty() {
                    vec![]
                } else if points.len() == 1 {
                    vec![vec![points[0]]]
                } else {
                    vec![sample_catmull_rom(points, 16)]
                }
            }
        }
    }

    /// Konstruktor helper untuk Circle biasa.
    pub fn circle(center: DVec2, radius: f64) -> Self {
        Self::Circle {
            center,
            radius,
            is_construction: false,
        }
    }

    /// Konstruktor helper untuk Arc biasa.
    pub fn arc(center: DVec2, radius: f64, start_angle: f64, end_angle: f64) -> Self {
        Self::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            is_construction: false,
        }
    }

    /// Konstruktor helper untuk Ellipse biasa.
    pub fn ellipse(center: DVec2, radius_x: f64, radius_y: f64) -> Self {
        Self::Ellipse {
            center,
            radius_x,
            radius_y,
            is_construction: false,
        }
    }

    /// Konstruktor helper untuk Spline biasa (tanpa kurva eksak).
    pub fn spline(points: Vec<DVec2>) -> Self {
        Self::Spline {
            points,
            exact: None,
            is_construction: false,
        }
    }

    /// Konstruktor Spline yang membawa definisi kurva eksaknya.
    ///
    /// `points` tetap wajib diisi hasil pencacahan `exact`, karena seluruh
    /// bagian lain (snap, region, gambar layar, ekspor 2D) hanya membaca
    /// `points`. Lihat dokumentasi field `exact`.
    pub fn spline_exact(points: Vec<DVec2>, exact: Vec<PathSeg>) -> Self {
        Self::Spline {
            points,
            exact: Some(exact),
            is_construction: false,
        }
    }

    /// Kurva eksak entitas ini, bila ada — `None` untuk entitas non-Spline.
    pub fn exact_path(&self) -> Option<&[PathSeg]> {
        match self {
            Entity::Spline { exact, .. } => exact.as_deref(),
            _ => None,
        }
    }

    /// Mengecek apakah entitas adalah garis konstruksi / referensi.
    pub fn is_construction(&self) -> bool {
        match self {
            Entity::Line { is_construction, .. }
            | Entity::Circle { is_construction, .. }
            | Entity::Arc { is_construction, .. }
            | Entity::Ellipse { is_construction, .. }
            | Entity::Spline { is_construction, .. }
            | Entity::Path { is_construction, .. } => *is_construction,
        }
    }

    /// Mengatur status konstruksi entitas.
    pub fn set_construction(&mut self, construction: bool) {
        match self {
            Entity::Line { is_construction, .. }
            | Entity::Circle { is_construction, .. }
            | Entity::Arc { is_construction, .. }
            | Entity::Ellipse { is_construction, .. }
            | Entity::Spline { is_construction, .. }
            | Entity::Path { is_construction, .. } => *is_construction = construction,
        }
    }

    /// Builder pattern untuk mengubah status garis konstruksi.
    pub fn with_construction(mut self, construction: bool) -> Self {
        self.set_construction(construction);
        self
    }

    /// Titik-titik endpoint sebagai kandidat snap "endpoint".
    pub fn endpoints(&self) -> Vec<DVec2> {
        match self {
            Entity::Line { start, end, .. } => vec![*start, *end],
            Entity::Circle { center, radius, .. } => vec![
                *center + DVec2::new(*radius, 0.0),
                *center + DVec2::new(0.0, *radius),
                *center + DVec2::new(-*radius, 0.0),
                *center + DVec2::new(0.0, -*radius),
            ],
            Entity::Ellipse {
                center,
                radius_x,
                radius_y,
                ..
            } => vec![
                *center + DVec2::new(*radius_x, 0.0),
                *center + DVec2::new(0.0, *radius_y),
                *center + DVec2::new(-*radius_x, 0.0),
                *center + DVec2::new(0.0, -*radius_y),
            ],
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => vec![
                *center + DVec2::new(radius * start_angle.cos(), radius * start_angle.sin()),
                *center + DVec2::new(radius * end_angle.cos(), radius * end_angle.sin()),
            ],
            Entity::Spline { points, .. } => {
                if points.is_empty() {
                    vec![]
                } else if points.len() == 1 {
                    vec![points[0]]
                } else {
                    let mut pts = vec![points[0], *points.last().unwrap()];
                    for pt in &points[1..points.len() - 1] {
                        pts.push(*pt);
                    }
                    pts
                }
            }
            Entity::Path { subpaths, .. } => {
                let mut pts = Vec::new();
                for sub in subpaths {
                    for i in 0..sub.node_count() {
                        pts.push(sub.node(i));
                    }
                }
                pts
            }
        }
    }

    pub fn midpoint(&self) -> Option<DVec2> {
        match self {
            Entity::Line { start, end, .. } => Some((*start + *end) * 0.5),
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                let tau = std::f64::consts::TAU;
                let span = {
                    let s = end_angle - start_angle;
                    if s <= 0.0 {
                        s + tau
                    } else {
                        s
                    }
                };
                let mid_angle = start_angle + span * 0.5;
                Some(*center + DVec2::new(radius * mid_angle.cos(), radius * mid_angle.sin()))
            }
            Entity::Spline { points, .. } => {
                if points.len() >= 2 {
                    let sampled = sample_catmull_rom(points, 8);
                    if !sampled.is_empty() {
                        return Some(sampled[sampled.len() / 2]);
                    }
                }
                None
            }
            Entity::Path { subpaths, .. } => {
                if subpaths.len() == 1 {
                    let flat = subpaths[0].flatten(0.05);
                    if flat.len() >= 2 {
                        return Some(flat[flat.len() / 2]);
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn center(&self) -> Option<DVec2> {
        match self {
            Entity::Circle { center, .. }
            | Entity::Arc { center, .. }
            | Entity::Ellipse { center, .. } => Some(*center),
            Entity::Spline { points, .. } => {
                if points.is_empty() {
                    None
                } else {
                    let sum = points.iter().copied().sum::<DVec2>();
                    Some(sum / (points.len() as f64))
                }
            }
            Entity::Path { .. } => {
                self.bounding_box().map(|(min, max)| (min + max) * 0.5)
            }
            Entity::Line { .. } => None,
        }
    }

    /// Menghitung bounding box 2D (min, max) dari entitas ini.
    pub fn bounding_box(&self) -> Option<(DVec2, DVec2)> {
        match self {
            Entity::Line { start, end, .. } => {
                let min = DVec2::new(start.x.min(end.x), start.y.min(end.y));
                let max = DVec2::new(start.x.max(end.x), start.y.max(end.y));
                Some((min, max))
            }
            Entity::Circle { center, radius, .. } => {
                let r = radius.abs();
                let min = DVec2::new(center.x - r, center.y - r);
                let max = DVec2::new(center.x + r, center.y + r);
                Some((min, max))
            }
            Entity::Arc { center, radius, .. } => {
                let r = radius.abs();
                let min = DVec2::new(center.x - r, center.y - r);
                let max = DVec2::new(center.x + r, center.y + r);
                Some((min, max))
            }
            Entity::Ellipse {
                center,
                radius_x,
                radius_y,
                ..
            } => {
                let rx = radius_x.abs();
                let ry = radius_y.abs();
                let min = DVec2::new(center.x - rx, center.y - ry);
                let max = DVec2::new(center.x + rx, center.y + ry);
                Some((min, max))
            }
            Entity::Spline { points, .. } => {
                if points.is_empty() {
                    return None;
                }
                let mut min = points[0];
                let mut max = points[0];
                for p in &points[1..] {
                    min = min.min(*p);
                    max = max.max(*p);
                }
                Some((min, max))
            }
            Entity::Path { subpaths, .. } => {
                if subpaths.is_empty() {
                    return None;
                }
                let mut min = DVec2::splat(f64::INFINITY);
                let mut max = DVec2::splat(f64::NEG_INFINITY);
                for sub in subpaths {
                    let (b_min, b_max) = sub.bbox();
                    min = min.min(b_min);
                    max = max.max(b_max);
                }
                if min.x.is_infinite() {
                    None
                } else {
                    Some((min, max))
                }
            }
        }
    }

    /// Sama seperti `endpoints()`, tapi berpasangan dengan `PointRef` sumbernya.
    pub fn endpoint_refs(&self, id: EntityId) -> Vec<(PointRef, DVec2)> {
        match self {
            Entity::Line { start, end, .. } => vec![
                (PointRef::LineStart(id), *start),
                (PointRef::LineEnd(id), *end),
            ],
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => vec![
                (
                    PointRef::LineStart(id),
                    *center + DVec2::new(radius * start_angle.cos(), radius * start_angle.sin()),
                ),
                (
                    PointRef::LineEnd(id),
                    *center + DVec2::new(radius * end_angle.cos(), radius * end_angle.sin()),
                ),
            ],
            Entity::Circle { center, radius, .. } => vec![
                (PointRef::Center(id), *center + DVec2::new(*radius, 0.0)),
                (PointRef::Center(id), *center + DVec2::new(0.0, *radius)),
                (PointRef::Center(id), *center + DVec2::new(-*radius, 0.0)),
                (PointRef::Center(id), *center + DVec2::new(0.0, -*radius)),
            ],
            Entity::Ellipse {
                center,
                radius_x,
                radius_y,
                ..
            } => vec![
                (PointRef::Center(id), *center + DVec2::new(*radius_x, 0.0)),
                (PointRef::Center(id), *center + DVec2::new(0.0, *radius_y)),
                (PointRef::Center(id), *center + DVec2::new(-*radius_x, 0.0)),
                (PointRef::Center(id), *center + DVec2::new(0.0, -*radius_y)),
            ],
            Entity::Spline { points, .. } => {
                if points.len() >= 2 {
                    let mut refs = vec![
                        (PointRef::LineStart(id), points[0]),
                        (PointRef::LineEnd(id), *points.last().unwrap()),
                    ];
                    for pt in &points[1..points.len() - 1] {
                        refs.push((PointRef::Center(id), *pt));
                    }
                    refs
                } else if points.len() == 1 {
                    vec![(PointRef::LineStart(id), points[0])]
                } else {
                    vec![]
                }
            }
            Entity::Path { .. } => {
                // PointRef::PathNode diselesaikan di M0.4
                vec![]
            }
        }
    }

    /// Sama seperti `center()`, berpasangan dengan `PointRef::Center`.
    pub fn center_ref(&self, id: EntityId) -> Option<(PointRef, DVec2)> {
        self.center().map(|c| (PointRef::Center(id), c))
    }

    /// Jarak titik ke entitas — dipakai hit-testing seleksi & snap.
    pub fn distance_to(&self, p: DVec2) -> f64 {
        match self {
            Entity::Line { start, end, .. } => distance_point_segment(p, *start, *end),
            Entity::Circle { center, radius, .. } => ((p - *center).length() - radius).abs(),
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                let to_p = p - *center;
                let angle = to_p.y.atan2(to_p.x);
                if angle_in_range(angle, *start_angle, *end_angle) {
                    (to_p.length() - radius).abs()
                } else {
                    self.endpoints()
                        .into_iter()
                        .map(|e| (p - e).length())
                        .fold(f64::INFINITY, f64::min)
                }
            }
            Entity::Ellipse {
                center,
                radius_x,
                radius_y,
                ..
            } => {
                const SAMPLES: usize = 64;
                (0..SAMPLES)
                    .map(|i| {
                        let t = TAU * (i as f64) / (SAMPLES as f64);
                        let boundary =
                            *center + DVec2::new(radius_x * t.cos(), radius_y * t.sin());
                        (p - boundary).length()
                    })
                    .fold(f64::INFINITY, f64::min)
            }
            Entity::Spline { points, .. } => {
                if points.is_empty() {
                    return f64::INFINITY;
                }
                if points.len() == 1 {
                    return (p - points[0]).length();
                }
                let sampled = sample_catmull_rom(points, 16);
                let mut min_d = f64::INFINITY;
                for w in sampled.windows(2) {
                    let d = distance_point_segment(p, w[0], w[1]);
                    if d < min_d {
                        min_d = d;
                    }
                }
                min_d
            }
            Entity::Path { subpaths, .. } => {
                let mut min_d = f64::INFINITY;
                for sub in subpaths {
                    let pts = sub.flatten(0.05);
                    if pts.len() == 1 {
                        min_d = min_d.min((p - pts[0]).length());
                    } else {
                        for w in pts.windows(2) {
                            min_d = min_d.min(distance_point_segment(p, w[0], w[1]));
                        }
                    }
                }
                min_d
            }
        }
    }
}

/// Evaluasi titik pada kurva Catmull-Rom spline yang melewati `points`.
pub fn sample_catmull_rom(points: &[DVec2], samples_per_span: usize) -> Vec<DVec2> {
    let n = points.len();
    if n < 2 {
        return points.to_vec();
    }
    if n == 2 {
        return vec![points[0], points[1]];
    }

    let samples = samples_per_span.max(2);
    let mut result = Vec::with_capacity((n - 1) * samples + 1);

    for i in 0..(n - 1) {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n { points[i + 2] } else { points[n - 1] };

        for s in 0..samples {
            let t = s as f64 / samples as f64;
            let t2 = t * t;
            let t3 = t2 * t;

            let pt = 0.5
                * (2.0 * p1
                    + (-p0 + p2) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3);
            result.push(pt);
        }
    }
    result.push(*points.last().unwrap());
    result
}

pub(crate) fn distance_point_segment(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < f64::EPSILON {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

pub(crate) fn angle_in_range(angle: f64, start: f64, end: f64) -> bool {
    let norm = |a: f64| ((a % TAU) + TAU) % TAU;
    let (a, s, e) = (norm(angle), norm(start), norm(end));
    if s <= e {
        a >= s && a <= e
    } else {
        a >= s || a <= e
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sketch;

    fn approx_eq(a: DVec2, b: DVec2, tol: f64) -> bool {
        (a - b).length() <= tol
    }

    #[test]
    fn path_flatten_closed_ends_at_start() {
        let sub = Subpath {
            start: DVec2::new(0.0, 0.0),
            segs: vec![
                PathSeg::Line {
                    end: DVec2::new(10.0, 0.0),
                },
                PathSeg::Line {
                    end: DVec2::new(5.0, 10.0),
                },
            ],
            closed: true,
        };
        let pts = sub.flatten(0.01);
        assert!(pts.len() >= 3, "Harus menghasilkan minimal 3 titik poliline");
        assert_eq!(
            pts.first(),
            pts.last(),
            "Titik terakhir pada path closed harus sama persis dengan start"
        );
        assert_eq!(pts[0], DVec2::new(0.0, 0.0));
    }

    #[test]
    fn path_cubic_degenerate_flattens_like_line() {
        let start = DVec2::new(0.0, 0.0);
        let end = DVec2::new(10.0, 0.0);
        let sub = Subpath {
            start,
            segs: vec![PathSeg::Cubic {
                c1: start,
                c2: end,
                end,
            }],
            closed: false,
        };
        let pts = sub.flatten(0.01);
        assert_eq!(pts.len(), 2, "Cubic degenerate harus dicacah jadi 2 titik");
        assert!(!pts[0].x.is_nan() && !pts[0].y.is_nan());
        assert!(!pts[1].x.is_nan() && !pts[1].y.is_nan());
        assert_eq!(pts[0], start);
        assert_eq!(pts[1], end);
    }

    #[test]
    fn set_node_moves_both_adjacent_handles() {
        let mut sub = Subpath {
            start: DVec2::new(0.0, 0.0),
            segs: vec![
                PathSeg::Cubic {
                    c1: DVec2::new(1.0, 1.0),
                    c2: DVec2::new(4.0, 4.0),
                    end: DVec2::new(5.0, 5.0),
                },
                PathSeg::Cubic {
                    c1: DVec2::new(6.0, 6.0),
                    c2: DVec2::new(9.0, 9.0),
                    end: DVec2::new(10.0, 10.0),
                },
            ],
            closed: false,
        };

        // Node 1 adalah sambungan antara segmen 0 dan segmen 1:
        // Handle bersebelahan: segs[0].c2 dan segs[1].c1
        let new_pos = DVec2::new(7.0, 8.0);
        sub.set_node(1, new_pos);

        assert_eq!(sub.node(1), new_pos);
        match &sub.segs[0] {
            PathSeg::Cubic { c1, c2, end } => {
                assert_eq!(*c1, DVec2::new(1.0, 1.0), "c1 pertama tidak boleh berubah");
                assert_eq!(*c2, DVec2::new(6.0, 7.0), "c2 harus bergeser delta (2, 3)");
                assert_eq!(*end, new_pos);
            }
            _ => panic!("Bukan cubic"),
        }
        match &sub.segs[1] {
            PathSeg::Cubic { c1, c2, end } => {
                assert_eq!(*c1, DVec2::new(8.0, 9.0), "c1 harus bergeser delta (2, 3)");
                assert_eq!(*c2, DVec2::new(9.0, 9.0), "c2 kedua tidak boleh berubah");
                assert_eq!(*end, DVec2::new(10.0, 10.0));
            }
            _ => panic!("Bukan cubic"),
        }
    }

    #[test]
    fn path_kurbo_roundtrip_preserves_segments() {
        let mut kpath = kurbo::BezPath::new();
        kpath.move_to(kurbo::Point::new(0.0, 0.0));
        kpath.quad_to(kurbo::Point::new(5.0, 10.0), kurbo::Point::new(10.0, 0.0));
        kpath.curve_to(
            kurbo::Point::new(12.0, 2.0),
            kurbo::Point::new(14.0, 8.0),
            kurbo::Point::new(16.0, 0.0),
        );
        kpath.line_to(kurbo::Point::new(20.0, 0.0));
        kpath.close_path();

        let subpaths = Subpath::from_kurbo(&kpath);
        assert_eq!(subpaths.len(), 1);
        let sub = &subpaths[0];
        assert!(sub.closed);
        assert_eq!(sub.segs.len(), 3);

        // Quad dinaikkan ke Cubic eksak:
        // p0=(0,0), ctrl=(5,10), end=(10,0)
        // c1 = (0,0) + 2/3 * (5,10) = (10/3, 20/3)
        // c2 = (10,0) + 2/3 * ((5,10) - (10,0)) = (10,0) + 2/3 * (-5,10) = (20/3, 20/3)
        match &sub.segs[0] {
            PathSeg::Cubic { c1, c2, end } => {
                assert!(approx_eq(*c1, DVec2::new(10.0 / 3.0, 20.0 / 3.0), 1e-9));
                assert!(approx_eq(*c2, DVec2::new(20.0 / 3.0, 20.0 / 3.0), 1e-9));
                assert!(approx_eq(*end, DVec2::new(10.0, 0.0), 1e-9));
            }
            _ => panic!("Quad harus dinaikkan jadi Cubic"),
        }

        // Putar balik ke kurbo
        let back_kpath = sub.to_kurbo();
        let back_subpaths = Subpath::from_kurbo(&back_kpath);
        assert_eq!(subpaths, back_subpaths);
    }

    #[test]
    fn old_json_without_path_still_parses() {
        let legacy_line = r#"{"Line":{"start":[0.0,0.0],"end":[10.0,0.0],"is_construction":false}}"#;
        let line: Entity = serde_json::from_str(legacy_line).expect("Line lama harus terbaca");
        assert!(!line.is_path());

        let legacy_circle = r#"{"Circle":{"center":[5.0,5.0],"radius":3.0,"is_construction":false}}"#;
        let circle: Entity = serde_json::from_str(legacy_circle).expect("Circle lama harus terbaca");
        assert!(!circle.is_path());

        let legacy_spline = r#"{"Spline":{"points":[[0.0,0.0],[5.0,1.0]],"is_construction":false}}"#;
        let spline: Entity = serde_json::from_str(legacy_spline).expect("Spline lama harus terbaca");
        assert!(!spline.is_path());

        // Path baru harus bisa serialize & deserialize
        let path_entity = Entity::path(vec![Subpath {
            start: DVec2::new(1.0, 2.0),
            segs: vec![PathSeg::Line {
                end: DVec2::new(3.0, 4.0),
            }],
            closed: true,
        }]);
        let json = serde_json::to_string(&path_entity).unwrap();
        let back: Entity = serde_json::from_str(&json).unwrap();
        assert_eq!(path_entity, back);
    }

    #[test]
    fn hit_test_finds_path_within_tolerance() {
        let mut sketch = Sketch::default();
        let sub = Subpath {
            start: DVec2::new(0.0, 0.0),
            segs: vec![PathSeg::Cubic {
                c1: DVec2::new(0.0, 10.0),
                c2: DVec2::new(10.0, 10.0),
                end: DVec2::new(10.0, 0.0),
            }],
            closed: false,
        };
        let id = sketch.entities.insert(Entity::path(vec![sub]));

        // Titik dekat puncak kurva (5.0, 7.5) dalam toleransi 0.5 mm
        let hit = sketch.hit_test(DVec2::new(5.0, 7.5), 0.5);
        assert_eq!(hit, Some(id), "Hit-test harus mendeteksi path dalam toleransi");

        // Titik jauh
        let miss = sketch.hit_test(DVec2::new(5.0, 15.0), 0.5);
        assert_eq!(miss, None, "Hit-test tidak boleh mendeteksi titik jauh");
    }
}
