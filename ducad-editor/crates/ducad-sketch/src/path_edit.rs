//! Operasi pengeditan path vektor dan konversi bentuk dasar ke path (M2).
//!
//! Logika murni tanpa ketergantungan GUI.

use glam::DVec2;
use crate::entity::{Entity, PathSeg, Subpath};

/// Konstanta kappa baku untuk aproksimasi lingkaran dengan 4 busur Bézier kubik:
/// kappa = 4/3 * (sqrt(2) - 1) ≈ 0.5522847498307936
pub const KAPPA: f64 = 0.5522847498307936;

/// Jenis bentuk dasar untuk dikonversi menjadi [`Subpath`].
#[derive(Debug, Clone, PartialEq)]
pub enum VectorShape {
    Rect { min: DVec2, max: DVec2 },
    Circle { center: DVec2, radius: f64 },
    Ellipse { center: DVec2, rx: f64, ry: f64 },
    Polygon { center: DVec2, radius: f64, sides: usize },
    Line { start: DVec2, end: DVec2 },
}

/// Mengubah persegi/persegi panjang (didefinisikan oleh dua titik berlawanan)
/// menjadi [`Subpath`] tertutup dengan 4 segmen garis lurus.
pub fn shape_to_path_rect(p0: DVec2, p1: DVec2) -> Subpath {
    let min = p0.min(p1);
    let max = p0.max(p1);
    Subpath {
        start: DVec2::new(min.x, min.y),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(max.x, min.y),
            },
            PathSeg::Line {
                end: DVec2::new(max.x, max.y),
            },
            PathSeg::Line {
                end: DVec2::new(min.x, max.y),
            },
            PathSeg::Line {
                end: DVec2::new(min.x, min.y),
            },
        ],
        closed: true,
    }
}

/// Mengubah lingkaran menjadi [`Subpath`] tertutup dengan 4 busur Bézier kubik.
pub fn shape_to_path_circle(center: DVec2, radius: f64) -> Subpath {
    shape_to_path_ellipse(center, radius, radius)
}

/// Mengubah elips menjadi [`Subpath`] tertutup dengan 4 busur Bézier kubik.
pub fn shape_to_path_ellipse(center: DVec2, rx: f64, ry: f64) -> Subpath {
    let kx = KAPPA * rx;
    let ky = KAPPA * ry;
    Subpath {
        start: center + DVec2::new(rx, 0.0),
        segs: vec![
            PathSeg::Cubic {
                c1: center + DVec2::new(rx, ky),
                c2: center + DVec2::new(kx, ry),
                end: center + DVec2::new(0.0, ry),
            },
            PathSeg::Cubic {
                c1: center + DVec2::new(-kx, ry),
                c2: center + DVec2::new(-rx, ky),
                end: center + DVec2::new(-rx, 0.0),
            },
            PathSeg::Cubic {
                c1: center + DVec2::new(-rx, -ky),
                c2: center + DVec2::new(-kx, -ry),
                end: center + DVec2::new(0.0, -ry),
            },
            PathSeg::Cubic {
                c1: center + DVec2::new(kx, -ry),
                c2: center + DVec2::new(rx, -ky),
                end: center + DVec2::new(rx, 0.0),
            },
        ],
        closed: true,
    }
}

/// Mengubah poligon n-sisi beraturan menjadi [`Subpath`] tertutup dengan n segmen garis.
pub fn shape_to_path_polygon(center: DVec2, radius: f64, sides: usize) -> Subpath {
    let n = sides.max(3);
    let step = std::f64::consts::TAU / n as f64;
    let start = center + DVec2::new(radius, 0.0);
    let mut segs = Vec::with_capacity(n);
    for i in 1..n {
        let angle = step * i as f64;
        let pt = center + DVec2::new(radius * angle.cos(), radius * angle.sin());
        segs.push(PathSeg::Line { end: pt });
    }
    segs.push(PathSeg::Line { end: start });
    Subpath {
        start,
        segs,
        closed: true,
    }
}

/// Helper umum untuk mengonversi [`VectorShape`] ke [`Subpath`].
pub fn shape_to_path(shape: &VectorShape) -> Subpath {
    match shape {
        VectorShape::Rect { min, max } => shape_to_path_rect(*min, *max),
        VectorShape::Circle { center, radius } => shape_to_path_circle(*center, *radius),
        VectorShape::Ellipse { center, rx, ry } => shape_to_path_ellipse(*center, *rx, *ry),
        VectorShape::Polygon {
            center,
            radius,
            sides,
        } => shape_to_path_polygon(*center, *radius, *sides),
        VectorShape::Line { start, end } => Subpath {
            start: *start,
            segs: vec![PathSeg::Line { end: *end }],
            closed: false,
        },
    }
}

/// Builder interaktif untuk menggambar path Bézier (M2.2).
#[derive(Debug, Clone, Default)]
pub struct PenBuilder {
    pub start: Option<DVec2>,
    pub segs: Vec<PathSeg>,
    pub last_handle: Option<DVec2>,
}

impl PenBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Titik terakhir yang tercatat dalam builder.
    pub fn last_point(&self) -> Option<DVec2> {
        self.segs.last().map(|s| s.end()).or(self.start)
    }

    /// Menambahkan node sudut (corner node).
    pub fn corner(&mut self, p: DVec2) {
        if self.start.is_none() {
            self.start = Some(p);
            self.last_handle = None;
        } else {
            if let Some(h_out) = self.last_handle.take() {
                self.segs.push(PathSeg::Cubic {
                    c1: h_out,
                    c2: p,
                    end: p,
                });
            } else {
                self.segs.push(PathSeg::Line { end: p });
            }
        }
    }

    /// Menambahkan node halus (smooth node) dengan handle simetris.
    pub fn smooth(&mut self, p: DVec2, handle_out: DVec2) {
        let h_in = p * 2.0 - handle_out; // cermin simetris
        self.smooth_broken(p, h_in, handle_out);
    }

    /// Menambahkan node halus dengan handle keluar dan masuk yang asimetris/patah.
    pub fn smooth_broken(&mut self, p: DVec2, h_in: DVec2, h_out: DVec2) {
        if self.start.is_none() {
            self.start = Some(p);
            self.last_handle = Some(h_out);
        } else {
            let prev_h = self.last_handle.take().unwrap_or_else(|| {
                self.last_point().unwrap_or(p)
            });
            self.segs.push(PathSeg::Cubic {
                c1: prev_h,
                c2: h_in,
                end: p,
            });
            self.last_handle = Some(h_out);
        }
    }

    /// Membatalkan (pop) node terakhir. Mengembalikan true jika ada node yang dihapus.
    pub fn pop(&mut self) -> bool {
        if self.segs.pop().is_some() || self.start.take().is_some() {
            self.last_handle = None;
            true
        } else {
            false
        }
    }

    /// Pratinjau segmen berikutnya dari posisi node terakhir ke posisi kursor saat ini.
    pub fn preview(&self, cursor: DVec2) -> Option<PathSeg> {
        let _last = self.last_point()?;
        if let Some(h_out) = self.last_handle {
            Some(PathSeg::Cubic {
                c1: h_out,
                c2: cursor,
                end: cursor,
            })
        } else {
            Some(PathSeg::Line { end: cursor })
        }
    }

    /// Menyelesaikan pembuatan subpath. Mengembalikan None jika jumlah node < 2.
    pub fn finish(mut self, close: bool) -> Option<Subpath> {
        let start = self.start?;
        if self.segs.is_empty() {
            return None;
        }

        if close {
            // Jika segmen terakhir berakhir persis di start, jatuhkan segmen duplikat tersebut
            // karena closed = true sudah menyiratkan ruas kembali ke start secara implisit.
            if let Some(last) = self.segs.last() {
                if (last.end() - start).length() <= 1e-6 {
                    self.segs.pop();
                }
            }
            if self.segs.is_empty() {
                return None;
            }
        }

        Some(Subpath {
            start,
            segs: self.segs,
            closed: close,
        })
    }
}

/// Kunci sudut ke kelipatan 45° terdekat terhadap titik acuan `origin` (saat Shift ditekan).
pub fn snap_angle_45(origin: DVec2, target: DVec2) -> DVec2 {
    let diff = target - origin;
    let dist = diff.length();
    if dist < 1e-6 {
        return target;
    }
    let angle = diff.y.atan2(diff.x);
    let step = std::f64::consts::FRAC_PI_4; // 45 derajat
    let snapped_angle = (angle / step).round() * step;
    origin + DVec2::new(snapped_angle.cos(), snapped_angle.sin()) * dist
}

/// Sisi handle Bézier (arah masuk atau keluar dari node).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleSide {
    In,
    Out,
}

/// Jenis kelengkungan node Bézier (M2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Corner,
    Smooth,
    Symmetric,
}

/// Helper untuk mengambil posisi handle masuk dan keluar dari node ke-`i`.
pub fn get_node_handles(sub: &Subpath, i: usize) -> (Option<DVec2>, Option<DVec2>) {
    if sub.segs.is_empty() {
        return (None, None);
    }
    let n = sub.segs.len();
    let h_in = if i == 0 {
        if sub.closed {
            if let PathSeg::Cubic { c2, .. } = sub.segs[n - 1] {
                Some(c2)
            } else {
                None
            }
        } else {
            None
        }
    } else if i <= n {
        if let PathSeg::Cubic { c2, .. } = sub.segs[i - 1] {
            Some(c2)
        } else {
            None
        }
    } else {
        None
    };

    let h_out = if i < n {
        if let PathSeg::Cubic { c1, .. } = sub.segs[i] {
            Some(c1)
        } else {
            None
        }
    } else if i == n && sub.closed {
        if let PathSeg::Cubic { c1, .. } = sub.segs[0] {
            Some(c1)
        } else {
            None
        }
    } else {
        None
    };

    (h_in, h_out)
}

fn set_incoming_handle(sub: &mut Subpath, i: usize, h: DVec2) {
    let n = sub.segs.len();
    if i == 0 && sub.closed && n > 0 {
        if let PathSeg::Cubic { ref mut c2, .. } = sub.segs[n - 1] {
            *c2 = h;
        }
    } else if i > 0 && i <= n {
        if let PathSeg::Cubic { ref mut c2, .. } = sub.segs[i - 1] {
            *c2 = h;
        }
    }
}

fn set_outgoing_handle(sub: &mut Subpath, i: usize, h: DVec2) {
    let n = sub.segs.len();
    if i < n {
        if let PathSeg::Cubic { ref mut c1, .. } = sub.segs[i] {
            *c1 = h;
        }
    } else if i == n && sub.closed && n > 0 {
        if let PathSeg::Cubic { ref mut c1, .. } = sub.segs[0] {
            *c1 = h;
        }
    }
}

/// Mengetahui jenis kelengkungan node ke-`i` (Corner, Smooth, Symmetric).
pub fn node_kind(sub: &Subpath, i: usize) -> NodeKind {
    if sub.node_count() == 0 || i >= sub.node_count() {
        return NodeKind::Corner;
    }
    let p = sub.node(i);
    let (h_in, h_out) = get_node_handles(sub, i);
    let (Some(hi), Some(ho)) = (h_in, h_out) else {
        return NodeKind::Corner;
    };
    let vi = hi - p;
    let vo = ho - p;
    let li = vi.length();
    let lo = vo.length();
    if li < 1e-6 || lo < 1e-6 {
        return NodeKind::Corner;
    }
    let di = vi / li;
    let do_dir = vo / lo;
    if di.dot(do_dir) < -0.999 {
        if (li - lo).abs() <= 1e-3 * (li + lo).max(1.0) {
            NodeKind::Symmetric
        } else {
            NodeKind::Smooth
        }
    } else {
        NodeKind::Corner
    }
}

/// Mengubah jenis kelengkungan node ke-`i` dan meratakan handle bila perlu.
pub fn set_node_kind(sub: &Subpath, i: usize, kind: NodeKind) -> Subpath {
    if sub.node_count() == 0 || i >= sub.node_count() {
        return sub.clone();
    }
    let mut res = sub.clone();
    let n = res.segs.len();
    if n == 0 {
        return res;
    }

    // Pastikan segmen terhubung berupa Cubic sehingga memiliki handle
    if i > 0 && i <= n {
        res = seg_to_curve(&res, i - 1);
    } else if i == 0 && res.closed {
        res = seg_to_curve(&res, n - 1);
    }

    if i < n {
        res = seg_to_curve(&res, i);
    } else if i == n && res.closed {
        res = seg_to_curve(&res, 0);
    }

    if kind == NodeKind::Corner {
        return res;
    }

    let p = res.node(i);
    let (h_in, h_out) = get_node_handles(&res, i);
    let (Some(hi), Some(ho)) = (h_in, h_out) else {
        return res;
    };

    let vi = hi - p;
    let vo = ho - p;
    let li = vi.length().max(1.0);
    let lo = vo.length().max(1.0);

    let di = if vi.length_squared() > 1e-9 { vi.normalize() } else { DVec2::new(-1.0, 0.0) };
    let do_dir = if vo.length_squared() > 1e-9 { vo.normalize() } else { DVec2::new(1.0, 0.0) };

    // Tangen terusan kurva: arah rata-rata dari vi menjauh menuju vo
    let avg_tangent = (do_dir - di).normalize_or_zero();
    let tangent = if avg_tangent.length_squared() > 1e-6 {
        avg_tangent
    } else {
        DVec2::new(1.0, 0.0)
    };

    let (len_in, len_out) = match kind {
        NodeKind::Symmetric => {
            let avg_l = (li + lo) * 0.5;
            (avg_l, avg_l)
        }
        NodeKind::Smooth | NodeKind::Corner => (li, lo),
    };

    let new_hi = p - tangent * len_in;
    let new_ho = p + tangent * len_out;

    set_incoming_handle(&mut res, i, new_hi);
    set_outgoing_handle(&mut res, i, new_ho);

    res
}

/// Menggeser posisi node ke-`i` secara murni, handle bergerak kaku bersamanya.
pub fn move_node(sub: &Subpath, i: usize, to: DVec2) -> Subpath {
    let mut res = sub.clone();
    res.set_node(i, to);
    res
}

/// Menggeser posisi handle kontrol Bézier pada node ke-`i`.
pub fn move_handle(sub: &Subpath, i: usize, side: HandleSide, to: DVec2, keep_smooth: bool) -> Subpath {
    if sub.node_count() == 0 || i >= sub.node_count() {
        return sub.clone();
    }
    let mut res = sub.clone();
    let n = res.segs.len();
    if n == 0 {
        return res;
    }
    let p = res.node(i);

    match side {
        HandleSide::In => {
            if i > 0 && i <= n {
                res = seg_to_curve(&res, i - 1);
            } else if i == 0 && res.closed {
                res = seg_to_curve(&res, n - 1);
            }
            set_incoming_handle(&mut res, i, to);

            if keep_smooth {
                let vi = to - p;
                let li = vi.length();
                if li > 1e-9 {
                    let opp_dir = -(vi / li);
                    let (_, h_out) = get_node_handles(&res, i);
                    let cur_lo = h_out.map(|h| (h - p).length()).unwrap_or(li);
                    let new_ho = p + opp_dir * cur_lo;
                    if i < n {
                        res = seg_to_curve(&res, i);
                    } else if i == n && res.closed {
                        res = seg_to_curve(&res, 0);
                    }
                    set_outgoing_handle(&mut res, i, new_ho);
                }
            }
        }
        HandleSide::Out => {
            if i < n {
                res = seg_to_curve(&res, i);
            } else if i == n && res.closed {
                res = seg_to_curve(&res, 0);
            }
            set_outgoing_handle(&mut res, i, to);

            if keep_smooth {
                let vo = to - p;
                let lo = vo.length();
                if lo > 1e-9 {
                    let opp_dir = -(vo / lo);
                    let (h_in, _) = get_node_handles(&res, i);
                    let cur_li = h_in.map(|h| (h - p).length()).unwrap_or(lo);
                    let new_hi = p + opp_dir * cur_li;
                    if i > 0 && i <= n {
                        res = seg_to_curve(&res, i - 1);
                    } else if i == 0 && res.closed {
                        res = seg_to_curve(&res, n - 1);
                    }
                    set_incoming_handle(&mut res, i, new_hi);
                }
            }
        }
    }

    res
}

/// Menyisipkan node baru pada segmen ke-`seg` di parameter `t` (de Casteljau split).
/// Bentuk kurva dipertahankan secara eksak.
pub fn insert_node_at(sub: &Subpath, seg: usize, t: f64) -> Subpath {
    if seg >= sub.segs.len() {
        return sub.clone();
    }
    let t = t.clamp(0.0, 1.0);
    let p0 = if seg == 0 {
        sub.start
    } else {
        sub.segs[seg - 1].end()
    };
    let mut res = sub.clone();

    match sub.segs[seg] {
        PathSeg::Line { end } => {
            let pt = p0.lerp(end, t);
            res.segs[seg] = PathSeg::Line { end: pt };
            res.segs.insert(seg + 1, PathSeg::Line { end });
        }
        PathSeg::Cubic { c1, c2, end } => {
            let p01 = p0.lerp(c1, t);
            let p12 = c1.lerp(c2, t);
            let p23 = c2.lerp(end, t);
            let p012 = p01.lerp(p12, t);
            let p123 = p12.lerp(p23, t);
            let pt = p012.lerp(p123, t);

            res.segs[seg] = PathSeg::Cubic {
                c1: p01,
                c2: p012,
                end: pt,
            };
            res.segs.insert(
                seg + 1,
                PathSeg::Cubic {
                    c1: p123,
                    c2: p23,
                    end,
                },
            );
        }
    }

    res
}

/// Menghapus node ke-`i` dan menggabungkan dua segmen yang bersebelahan dengan curve fitting kurbo.
pub fn delete_node(sub: &Subpath, i: usize) -> Option<Subpath> {
    if sub.segs.is_empty() || sub.node_count() <= 2 {
        return None;
    }
    let n = sub.segs.len();
    if !sub.closed {
        if i == 0 {
            let new_start = sub.segs[0].end();
            let new_segs = sub.segs[1..].to_vec();
            if new_segs.is_empty() {
                return None;
            }
            return Some(Subpath {
                start: new_start,
                segs: new_segs,
                closed: false,
            });
        }
        if i == n {
            let mut new_segs = sub.segs.clone();
            new_segs.pop();
            if new_segs.is_empty() {
                return None;
            }
            return Some(Subpath {
                start: sub.start,
                segs: new_segs,
                closed: false,
            });
        }
        if i > n {
            return None;
        }
        let seg_a = sub.segs[i - 1];
        let seg_b = sub.segs[i];
        let p0 = if i == 1 { sub.start } else { sub.segs[i - 2].end() };

        let merged = merge_two_segments(p0, seg_a, seg_b);
        let mut new_segs = sub.segs.clone();
        new_segs[i - 1] = merged;
        new_segs.remove(i);
        Some(Subpath {
            start: sub.start,
            segs: new_segs,
            closed: false,
        })
    } else {
        if n < 3 {
            return None;
        }
        if i == 0 || i == n {
            let seg_a = sub.segs[n - 1];
            let seg_b = sub.segs[0];
            let p0 = sub.node(n - 1);
            let merged = merge_two_segments(p0, seg_a, seg_b);
            let mut new_segs = sub.segs.clone();
            new_segs.remove(0);
            *new_segs.last_mut().unwrap() = merged;
            Some(Subpath {
                start: merged.end(),
                segs: new_segs,
                closed: true,
            })
        } else {
            let seg_a = sub.segs[i - 1];
            let seg_b = sub.segs[i];
            let p0 = if i == 1 { sub.start } else { sub.segs[i - 2].end() };
            let merged = merge_two_segments(p0, seg_a, seg_b);
            let mut new_segs = sub.segs.clone();
            new_segs[i - 1] = merged;
            new_segs.remove(i);
            Some(Subpath {
                start: sub.start,
                segs: new_segs,
                closed: true,
            })
        }
    }
}

fn merge_two_segments(p0: DVec2, seg_a: PathSeg, seg_b: PathSeg) -> PathSeg {
    let p3 = seg_b.end();
    if matches!(seg_a, PathSeg::Line { .. }) && matches!(seg_b, PathSeg::Line { .. }) {
        return PathSeg::Line { end: p3 };
    }

    let mut kpath = kurbo::BezPath::new();
    kpath.move_to(kurbo::Point::new(p0.x, p0.y));
    match seg_a {
        PathSeg::Line { end } => kpath.line_to(kurbo::Point::new(end.x, end.y)),
        PathSeg::Cubic { c1, c2, end } => kpath.curve_to(
            kurbo::Point::new(c1.x, c1.y),
            kurbo::Point::new(c2.x, c2.y),
            kurbo::Point::new(end.x, end.y),
        ),
    }
    match seg_b {
        PathSeg::Line { end } => kpath.line_to(kurbo::Point::new(end.x, end.y)),
        PathSeg::Cubic { c1, c2, end } => kpath.curve_to(
            kurbo::Point::new(c1.x, c1.y),
            kurbo::Point::new(c2.x, c2.y),
            kurbo::Point::new(end.x, end.y),
        ),
    }

    let chord = (p3 - p0).length();
    let accuracy = (chord * 0.05).clamp(1e-4, 1.0);
    let s = kurbo::simplify::SimplifyBezPath::new(&kpath);
    if let Some((cubic, _)) = kurbo::fit_to_cubic(&s, 0.0..1.0, accuracy) {
        PathSeg::Cubic {
            c1: DVec2::new(cubic.p1.x, cubic.p1.y),
            c2: DVec2::new(cubic.p2.x, cubic.p2.y),
            end: p3,
        }
    } else {
        // Fallback: estimasi dari posisi control points segmen lama
        let c1 = match seg_a {
            PathSeg::Cubic { c1, .. } => c1,
            PathSeg::Line { end } => p0 + (end - p0) * (2.0 / 3.0),
        };
        let c2 = match seg_b {
            PathSeg::Cubic { c2, .. } => c2,
            PathSeg::Line { end } => end - (end - seg_a.end()) * (2.0 / 3.0),
        };
        PathSeg::Cubic { c1, c2, end: p3 }
    }
}

/// Mengubah segmen ke-`seg` menjadi garis lurus [`PathSeg::Line`].
pub fn seg_to_line(sub: &Subpath, seg: usize) -> Subpath {
    if seg >= sub.segs.len() {
        return sub.clone();
    }
    let mut res = sub.clone();
    let end = res.segs[seg].end();
    res.segs[seg] = PathSeg::Line { end };
    res
}

/// Mengubah segmen ke-`seg` menjadi kurva Bézier kubik [`PathSeg::Cubic`] tanpa mengubah bentuknya.
pub fn seg_to_curve(sub: &Subpath, seg: usize) -> Subpath {
    if seg >= sub.segs.len() {
        return sub.clone();
    }
    let mut res = sub.clone();
    if let PathSeg::Line { end } = res.segs[seg] {
        let p0 = if seg == 0 {
            res.start
        } else {
            res.segs[seg - 1].end()
        };
        res.segs[seg] = PathSeg::Cubic {
            c1: p0 + (end - p0) * (1.0 / 3.0),
            c2: end - (end - p0) * (1.0 / 3.0),
            end,
        };
    }
    res
}

/// Memutus subpath di node ke-`i` menjadi dua subpath terpisah.
pub fn break_at_node(sub: &Subpath, i: usize) -> (Subpath, Subpath) {
    if sub.segs.is_empty() || i == 0 || i >= sub.segs.len() {
        return (
            sub.clone(),
            Subpath {
                start: sub.node(i.min(sub.segs.len())),
                segs: Vec::new(),
                closed: false,
            },
        );
    }
    let p_break = sub.segs[i - 1].end();
    let a = Subpath {
        start: sub.start,
        segs: sub.segs[..i].to_vec(),
        closed: false,
    };
    let b = Subpath {
        start: p_break,
        segs: sub.segs[i..].to_vec(),
        closed: false,
    };
    (a, b)
}

/// Menggabungkan dua subpath jika ujung `a` dan awal `b` (atau sebaliknya) berdekatan dalam batas `tol`.
pub fn join(a: &Subpath, b: &Subpath, tol: f64) -> Option<Subpath> {
    let end_a = a.segs.last().map(|s| s.end()).unwrap_or(a.start);
    let start_b = b.start;
    if (end_a - start_b).length() <= tol {
        let mut segs = a.segs.clone();
        segs.extend(b.segs.iter().cloned());
        let closed = a.closed || b.closed;
        return Some(Subpath {
            start: a.start,
            segs,
            closed,
        });
    }
    let end_b = b.segs.last().map(|s| s.end()).unwrap_or(b.start);
    let start_a = a.start;
    if (end_b - start_a).length() <= tol {
        let mut segs = b.segs.clone();
        segs.extend(a.segs.iter().cloned());
        let closed = a.closed || b.closed;
        return Some(Subpath {
            start: b.start,
            segs,
            closed,
        });
    }
    None
}

/// Membalik arah penelusuran subpath secara eksak.
pub fn reverse(sub: &Subpath) -> Subpath {
    if sub.segs.is_empty() {
        return sub.clone();
    }
    let new_start = sub.segs.last().unwrap().end();
    let mut new_segs = Vec::with_capacity(sub.segs.len());
    for i in (0..sub.segs.len()).rev() {
        let prev_pt = if i == 0 {
            sub.start
        } else {
            sub.segs[i - 1].end()
        };
        match sub.segs[i] {
            PathSeg::Line { .. } => {
                new_segs.push(PathSeg::Line { end: prev_pt });
            }
            PathSeg::Cubic { c1, c2, .. } => {
                new_segs.push(PathSeg::Cubic {
                    c1: c2,
                    c2: c1,
                    end: prev_pt,
                });
            }
        }
    }
    Subpath {
        start: new_start,
        segs: new_segs,
        closed: sub.closed,
    }
}

/// Mencari titik terdekat pada subpath terhadap titik `p`.
/// Mengembalikan `(seg_idx, t, nearest_point, distance)`.
pub fn closest_point(sub: &Subpath, p: DVec2) -> (usize, f64, DVec2, f64) {
    if sub.segs.is_empty() {
        let dist = (sub.start - p).length();
        return (0, 0.0, sub.start, dist);
    }
    let mut best_seg = 0;
    let mut best_t = 0.0;
    let mut best_pt = sub.start;
    let mut min_dist = f64::INFINITY;

    for (seg_idx, seg) in sub.segs.iter().enumerate() {
        let p0 = if seg_idx == 0 {
            sub.start
        } else {
            sub.segs[seg_idx - 1].end()
        };
        match seg {
            PathSeg::Line { end } => {
                let v = *end - p0;
                let len2 = v.length_squared();
                let t = if len2 < 1e-12 {
                    0.0
                } else {
                    ((p - p0).dot(v) / len2).clamp(0.0, 1.0)
                };
                let pt = p0 + v * t;
                let dist = (pt - p).length();
                if dist < min_dist {
                    min_dist = dist;
                    best_seg = seg_idx;
                    best_t = t;
                    best_pt = pt;
                }
            }
            PathSeg::Cubic { c1, c2, end } => {
                let k_cubic = kurbo::CubicBez::new(
                    kurbo::Point::new(p0.x, p0.y),
                    kurbo::Point::new(c1.x, c1.y),
                    kurbo::Point::new(c2.x, c2.y),
                    kurbo::Point::new(end.x, end.y),
                );
                let nearest = kurbo::ParamCurveNearest::nearest(
                    &k_cubic,
                    kurbo::Point::new(p.x, p.y),
                    1e-4,
                );
                let pt = kurbo::ParamCurve::eval(&k_cubic, nearest.t);
                let pt_dvec = DVec2::new(pt.x, pt.y);
                let dist = nearest.distance_sq.sqrt();
                if dist < min_dist {
                    min_dist = dist;
                    best_seg = seg_idx;
                    best_t = nearest.t;
                    best_pt = pt_dvec;
                }
            }
        }
    }

    (best_seg, best_t, best_pt, min_dist)
}

/// Mengaplikasikan transformasi affine kurbo pada DVec2.
pub fn apply_affine(affine: kurbo::Affine, p: DVec2) -> DVec2 {
    let kp = affine * kurbo::Point::new(p.x, p.y);
    DVec2::new(kp.x, kp.y)
}

/// Mentransformasikan seluruh node dan handle pada sebuah `Subpath`.
pub fn transform_subpath(sub: &Subpath, affine: kurbo::Affine) -> Subpath {
    let start = apply_affine(affine, sub.start);
    let segs = sub
        .segs
        .iter()
        .map(|seg| match seg {
            PathSeg::Line { end } => PathSeg::Line {
                end: apply_affine(affine, *end),
            },
            PathSeg::Cubic { c1, c2, end } => PathSeg::Cubic {
                c1: apply_affine(affine, *c1),
                c2: apply_affine(affine, *c2),
                end: apply_affine(affine, *end),
            },
        })
        .collect();
    Subpath {
        start,
        segs,
        closed: sub.closed,
    }
}

/// Mentransformasikan sembarang entitas dengan matriks `kurbo::Affine` (M2.5).
/// - `Path`: titik kontrol dipetakan langsung dengan matriks affine (`map_points`).
/// - `Circle`: jika skala non-seragam atau terdapat shear, dikonversi menjadi `Entity::Path` (4 kubik Bézier).
///   Jika skala seragam, tetap `Entity::Circle`.
/// - `Ellipse`: jika mengalami rotasi atau shear, dikonversi menjadi `Entity::Path` (4 kubik Bézier).
///   Jika axis-aligned, tetap `Entity::Ellipse`.
/// - `Line`, `Arc`, `Spline`: ditransformasikan sesuai geometri masing-masing.
pub fn transform_entity(entity: &Entity, affine: kurbo::Affine) -> Entity {
    let [a, b, c, d, _tx, _ty] = affine.as_coeffs();

    match entity {
        Entity::Path {
            subpaths,
            is_construction,
        } => {
            let new_subs = subpaths
                .iter()
                .map(|sub| transform_subpath(sub, affine))
                .collect();
            Entity::Path {
                subpaths: new_subs,
                is_construction: *is_construction,
            }
        }
        Entity::Line {
            start,
            end,
            is_construction,
        } => Entity::Line {
            start: apply_affine(affine, *start),
            end: apply_affine(affine, *end),
            is_construction: *is_construction,
        },
        Entity::Circle {
            center,
            radius,
            is_construction,
        } => {
            let sx2 = a * a + b * b;
            let sy2 = c * c + d * d;
            let is_orthogonal = (a * c + b * d).abs() < 1e-9;
            let is_uniform = is_orthogonal && (sx2 - sy2).abs() < 1e-9;

            if is_uniform {
                let s = sx2.sqrt();
                let new_center = apply_affine(affine, *center);
                Entity::Circle {
                    center: new_center,
                    radius: radius * s,
                    is_construction: *is_construction,
                }
            } else {
                let path_sub = shape_to_path_circle(*center, *radius);
                let transformed_sub = transform_subpath(&path_sub, affine);
                Entity::Path {
                    subpaths: vec![transformed_sub],
                    is_construction: *is_construction,
                }
            }
        }
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            is_construction,
        } => {
            let is_axis_aligned = b.abs() < 1e-9 && c.abs() < 1e-9;
            if is_axis_aligned {
                let new_center = apply_affine(affine, *center);
                Entity::Ellipse {
                    center: new_center,
                    radius_x: (radius_x * a).abs(),
                    radius_y: (radius_y * d).abs(),
                    is_construction: *is_construction,
                }
            } else {
                let path_sub = shape_to_path_ellipse(*center, *radius_x, *radius_y);
                let transformed_sub = transform_subpath(&path_sub, affine);
                Entity::Path {
                    subpaths: vec![transformed_sub],
                    is_construction: *is_construction,
                }
            }
        }
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            is_construction,
        } => {
            let sx2 = a * a + b * b;
            let sy2 = c * c + d * d;
            let is_orthogonal = (a * c + b * d).abs() < 1e-9;
            let is_uniform = is_orthogonal && (sx2 - sy2).abs() < 1e-9;

            if is_uniform {
                let s = sx2.sqrt();
                let new_center = apply_affine(affine, *center);
                let rot = b.atan2(a);
                let det = a * d - b * c;
                let (new_start, new_end) = if det >= 0.0 {
                    (start_angle + rot, end_angle + rot)
                } else {
                    (rot - end_angle, rot - start_angle)
                };
                Entity::Arc {
                    center: new_center,
                    radius: radius * s,
                    start_angle: new_start,
                    end_angle: new_end,
                    is_construction: *is_construction,
                }
            } else {
                let arc = kurbo::Arc {
                    center: kurbo::Point::new(center.x, center.y),
                    radii: kurbo::Vec2::new(*radius, *radius),
                    start_angle: *start_angle,
                    sweep_angle: end_angle - start_angle,
                    x_rotation: 0.0,
                };
                let bez = kurbo::Shape::to_path(&arc, 0.01);
                let subs = Subpath::from_kurbo(&bez);
                let transformed_subs = subs
                    .into_iter()
                    .map(|s| transform_subpath(&s, affine))
                    .collect();
                Entity::Path {
                    subpaths: transformed_subs,
                    is_construction: *is_construction,
                }
            }
        }
        Entity::Spline {
            points,
            exact,
            is_construction,
        } => {
            let new_points = points.iter().map(|p| apply_affine(affine, *p)).collect();
            let new_exact = exact.as_ref().map(|segs| {
                segs.iter()
                    .map(|seg| match *seg {
                        PathSeg::Line { end } => PathSeg::Line {
                            end: apply_affine(affine, end),
                        },
                        PathSeg::Cubic { c1, c2, end } => PathSeg::Cubic {
                            c1: apply_affine(affine, c1),
                            c2: apply_affine(affine, c2),
                            end: apply_affine(affine, end),
                        },
                    })
                    .collect()
            });
            Entity::Spline {
                points: new_points,
                exact: new_exact,
                is_construction: *is_construction,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_to_path_rect_has_four_line_segs_closed() {
        let p0 = DVec2::new(10.0, 20.0);
        let p1 = DVec2::new(60.0, 50.0);
        let sub = shape_to_path_rect(p0, p1);
        assert!(sub.closed, "Rectangle subpath must be closed");
        assert_eq!(sub.segs.len(), 4, "Rectangle must have exactly 4 segments");
        for seg in &sub.segs {
            assert!(matches!(seg, PathSeg::Line { .. }), "Each segment must be a Line");
        }
        assert_eq!(sub.start, DVec2::new(10.0, 20.0));
        assert_eq!(sub.segs[0].end(), DVec2::new(60.0, 20.0));
        assert_eq!(sub.segs[1].end(), DVec2::new(60.0, 50.0));
        assert_eq!(sub.segs[2].end(), DVec2::new(10.0, 50.0));
        assert_eq!(sub.segs[3].end(), DVec2::new(10.0, 20.0));
    }

    #[test]
    fn shape_to_path_circle_is_four_cubics_within_tol() {
        let center = DVec2::new(50.0, -30.0);
        let radius = 100.0;
        let sub = shape_to_path_circle(center, radius);
        assert!(sub.closed, "Circle subpath must be closed");
        assert_eq!(sub.segs.len(), 4, "Circle must have 4 cubic segments");

        let mut max_err: f64 = 0.0;
        let mut prev = sub.start;
        for seg in &sub.segs {
            match seg {
                PathSeg::Cubic { c1, c2, end } => {
                    for i in 0..=100 {
                        let t = i as f64 / 100.0;
                        let u = 1.0 - t;
                        let pt = prev * (u * u * u)
                            + *c1 * (3.0 * u * u * t)
                            + *c2 * (3.0 * u * t * t)
                            + *end * (t * t * t);
                        let dist = (pt - center).length();
                        let err = (dist - radius).abs();
                        if err > max_err {
                            max_err = err;
                        }
                    }
                    prev = *end;
                }
                _ => panic!("Expected Cubic segment"),
            }
        }
        // Galat maksimum terhadap lingkaran <= 0.03% r (konstanta kappa 0.5523)
        let tol = 0.0003 * radius;
        assert!(
            max_err <= tol,
            "Max radial error {} exceeded tolerance {} (0.03% of r = {})",
            max_err,
            tol,
            radius
        );
    }

    #[test]
    fn pen_corner_then_corner_gives_line_seg() {
        let mut b = PenBuilder::new();
        b.corner(DVec2::new(0.0, 0.0));
        b.corner(DVec2::new(10.0, 0.0));
        let sub = b.finish(false).expect("should produce subpath");
        assert_eq!(sub.start, DVec2::new(0.0, 0.0));
        assert_eq!(sub.segs.len(), 1);
        assert_eq!(sub.segs[0], PathSeg::Line { end: DVec2::new(10.0, 0.0) });
        assert!(!sub.closed);
    }

    #[test]
    fn pen_smooth_makes_symmetric_handles() {
        let mut b = PenBuilder::new();
        b.corner(DVec2::new(0.0, 0.0));
        let p = DVec2::new(10.0, 10.0);
        let h_out = DVec2::new(15.0, 12.0);
        b.smooth(p, h_out);
        let sub = b.finish(false).expect("should produce subpath");
        assert_eq!(sub.segs.len(), 1);
        match sub.segs[0] {
            PathSeg::Cubic { c2, end, .. } => {
                assert_eq!(end, p);
                // c2 adalah h_in yang simetris terhadap p dengan h_out
                let midpoint = (c2 + h_out) * 0.5;
                assert!((midpoint - p).length() < 1e-9);
            }
            _ => panic!("Expected Cubic segment"),
        }
    }

    #[test]
    fn pen_close_sets_closed_and_drops_duplicate_end() {
        let mut b = PenBuilder::new();
        b.corner(DVec2::new(0.0, 0.0));
        b.corner(DVec2::new(10.0, 0.0));
        b.corner(DVec2::new(10.0, 10.0));
        b.corner(DVec2::new(0.0, 0.0)); // Titik penutup yang sama dengan start
        let sub = b.finish(true).expect("should produce closed subpath");
        assert!(sub.closed, "Subpath must be marked closed");
        assert_eq!(
            sub.segs.len(),
            2,
            "Duplicate end segment must be dropped, leaving 2 segments for 3 nodes"
        );
        assert_eq!(sub.node_count(), 3);
    }

    #[test]
    fn pen_pop_restores_previous_state() {
        let mut b = PenBuilder::new();
        b.corner(DVec2::new(0.0, 0.0));
        b.corner(DVec2::new(10.0, 0.0));
        assert_eq!(b.segs.len(), 1);
        assert!(b.pop());
        assert_eq!(b.segs.len(), 0);
        assert_eq!(b.start, Some(DVec2::new(0.0, 0.0)));
        assert!(b.pop());
        assert_eq!(b.start, None);
        assert!(!b.pop());
    }

    #[test]
    fn pen_finish_with_one_node_is_none() {
        let mut b = PenBuilder::new();
        assert!(b.clone().finish(false).is_none());
        assert!(b.clone().finish(true).is_none());

        b.corner(DVec2::new(5.0, 5.0));
        assert!(b.clone().finish(false).is_none());
        assert!(b.finish(true).is_none());
    }

    fn eval_cubic(p0: DVec2, p1: DVec2, p2: DVec2, p3: DVec2, t: f64) -> DVec2 {
        let u = 1.0 - t;
        p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t)
    }

    #[test]
    fn insert_node_preserves_shape() {
        let p0 = DVec2::new(0.0, 0.0);
        let c1 = DVec2::new(30.0, 60.0);
        let c2 = DVec2::new(70.0, -40.0);
        let p3 = DVec2::new(100.0, 20.0);
        let sub = Subpath {
            start: p0,
            segs: vec![PathSeg::Cubic { c1, c2, end: p3 }],
            closed: false,
        };
        let split_t = 0.42;
        let split = insert_node_at(&sub, 0, split_t);
        assert_eq!(split.segs.len(), 2);

        let mut max_err: f64 = 0.0;
        for i in 0..=100 {
            let u = i as f64 / 100.0;
            let pt_orig = eval_cubic(p0, c1, c2, p3, u);
            let pt_split = if u <= split_t {
                let local_t = u / split_t;
                match split.segs[0] {
                    PathSeg::Cubic { c1: s_c1, c2: s_c2, end: s_end } => {
                        eval_cubic(split.start, s_c1, s_c2, s_end, local_t)
                    }
                    _ => panic!("Expected cubic"),
                }
            } else {
                let local_t = (u - split_t) / (1.0 - split_t);
                let p_mid = split.segs[0].end();
                match split.segs[1] {
                    PathSeg::Cubic { c1: s_c1, c2: s_c2, end: s_end } => {
                        eval_cubic(p_mid, s_c1, s_c2, s_end, local_t)
                    }
                    _ => panic!("Expected cubic"),
                }
            };
            let err = (pt_orig - pt_split).length();
            if err > max_err {
                max_err = err;
            }
        }
        assert!(max_err <= 1e-9, "max error {} exceeded 1e-9", max_err);
    }

    #[test]
    fn delete_node_error_bounded() {
        let p0 = DVec2::new(0.0, 0.0);
        let c1 = DVec2::new(30.0, 50.0);
        let c2 = DVec2::new(70.0, 50.0);
        let p3 = DVec2::new(100.0, 0.0);
        let sub = Subpath {
            start: p0,
            segs: vec![PathSeg::Cubic { c1, c2, end: p3 }],
            closed: false,
        };
        let split = insert_node_at(&sub, 0, 0.5);
        let restored = delete_node(&split, 1).expect("delete_node should succeed");
        assert_eq!(restored.segs.len(), 1);

        let mut max_err: f64 = 0.0;
        for i in 0..=100 {
            let u = i as f64 / 100.0;
            let pt_orig = eval_cubic(p0, c1, c2, p3, u);
            let pt_rest = match restored.segs[0] {
                PathSeg::Cubic { c1: rc1, c2: rc2, end: rend } => {
                    eval_cubic(restored.start, rc1, rc2, rend, u)
                }
                PathSeg::Line { end } => restored.start.lerp(end, u),
            };
            let err = (pt_orig - pt_rest).length();
            if err > max_err {
                max_err = err;
            }
        }
        assert!(max_err <= 0.1, "max error {} exceeded 0.1 mm", max_err);
    }

    #[test]
    fn set_smooth_makes_handles_collinear() {
        let mut sub = Subpath {
            start: DVec2::new(0.0, 0.0),
            segs: vec![
                PathSeg::Cubic {
                    c1: DVec2::new(5.0, 10.0),
                    c2: DVec2::new(10.0, 10.0),
                    end: DVec2::new(15.0, 0.0),
                },
                PathSeg::Cubic {
                    c1: DVec2::new(20.0, 15.0),
                    c2: DVec2::new(25.0, 5.0),
                    end: DVec2::new(30.0, 0.0),
                },
            ],
            closed: false,
        };
        assert_eq!(node_kind(&sub, 1), NodeKind::Corner);

        sub = set_node_kind(&sub, 1, NodeKind::Smooth);
        let kind = node_kind(&sub, 1);
        assert!(matches!(kind, NodeKind::Smooth | NodeKind::Symmetric));

        let p = sub.node(1);
        let (h_in, h_out) = get_node_handles(&sub, 1);
        let vi = (h_in.unwrap() - p).normalize();
        let vo = (h_out.unwrap() - p).normalize();
        let dot = vi.dot(vo);
        assert!(dot < -0.999, "Handles must be collinear and opposite (dot = {})", dot);
    }

    #[test]
    fn break_then_join_roundtrip() {
        let sub = Subpath {
            start: DVec2::new(0.0, 0.0),
            segs: vec![
                PathSeg::Line { end: DVec2::new(10.0, 5.0) },
                PathSeg::Cubic {
                    c1: DVec2::new(15.0, 10.0),
                    c2: DVec2::new(20.0, 0.0),
                    end: DVec2::new(30.0, 5.0),
                },
                PathSeg::Line { end: DVec2::new(40.0, 20.0) },
            ],
            closed: false,
        };
        let (a, b) = break_at_node(&sub, 1);
        assert_eq!(a.segs.len(), 1);
        assert_eq!(b.segs.len(), 2);
        let joined = join(&a, &b, 1e-4).expect("should join successfully");
        assert_eq!(joined, sub);
    }

    #[test]
    fn closest_point_on_line_seg_is_projection() {
        let sub = Subpath {
            start: DVec2::new(0.0, 0.0),
            segs: vec![PathSeg::Line { end: DVec2::new(10.0, 0.0) }],
            closed: false,
        };
        let p = DVec2::new(6.0, 4.0);
        let (seg, t, pt, dist) = closest_point(&sub, p);
        assert_eq!(seg, 0);
        assert!((t - 0.6).abs() < 1e-9);
        assert!((pt - DVec2::new(6.0, 0.0)).length() < 1e-9);
        assert!((dist - 4.0).abs() < 1e-9);
    }

    #[test]
    fn reverse_twice_is_identity() {
        let sub = Subpath {
            start: DVec2::new(1.0, 2.0),
            segs: vec![
                PathSeg::Line { end: DVec2::new(5.0, 10.0) },
                PathSeg::Cubic {
                    c1: DVec2::new(12.0, 8.0),
                    c2: DVec2::new(15.0, 20.0),
                    end: DVec2::new(25.0, 10.0),
                },
                PathSeg::Line { end: DVec2::new(30.0, 0.0) },
            ],
            closed: false,
        };
        let rev1 = reverse(&sub);
        assert_eq!(rev1.start, DVec2::new(30.0, 0.0));
        let rev2 = reverse(&rev1);
        assert_eq!(rev2, sub);
    }

    proptest::proptest! {
        #[test]
        fn insert_then_delete_is_close_to_original(
            t in 0.2f64..0.8f64,
            c1x in 10.0f64..40.0f64,
            c1y in 10.0f64..40.0f64,
            c2x in 60.0f64..90.0f64,
            c2y in 10.0f64..40.0f64,
        ) {
            let p0 = DVec2::new(0.0, 0.0);
            let c1 = DVec2::new(c1x, c1y);
            let c2 = DVec2::new(c2x, c2y);
            let p3 = DVec2::new(100.0, 0.0);
            let sub = Subpath {
                start: p0,
                segs: vec![PathSeg::Cubic { c1, c2, end: p3 }],
                closed: false,
            };
            let split = insert_node_at(&sub, 0, t);
            let restored = delete_node(&split, 1).expect("delete_node should succeed");
            for i in 0..=20 {
                let u = i as f64 / 20.0;
                let pt_orig = eval_cubic(p0, c1, c2, p3, u);
                let pt_rest = match restored.segs[0] {
                    PathSeg::Cubic { c1: rc1, c2: rc2, end: rend } => {
                        eval_cubic(restored.start, rc1, rc2, rend, u)
                    }
                    PathSeg::Line { end } => restored.start.lerp(end, u),
                };
                let err = (pt_orig - pt_rest).length();
                proptest::prop_assert!(err <= 0.1, "max error {} exceeded 0.1 mm at u={}", err, u);
            }
        }
    }

    #[test]
    fn transform_circle_nonuniform_becomes_path() {
        let circle = Entity::circle(DVec2::new(10.0, 10.0), 5.0);
        let affine = kurbo::Affine::scale_non_uniform(2.0, 3.0);
        let transformed = transform_entity(&circle, affine);
        match transformed {
            Entity::Path { subpaths, .. } => {
                assert_eq!(subpaths.len(), 1);
                assert_eq!(subpaths[0].segs.len(), 4);
                assert!(subpaths[0].closed);
            }
            _ => panic!("Expected Entity::Path for non-uniform circle scaling"),
        }
    }

    #[test]
    fn rotate_path_preserves_lengths() {
        let p0 = DVec2::new(0.0, 0.0);
        let p1 = DVec2::new(10.0, 0.0);
        let p2 = DVec2::new(10.0, 10.0);
        let sub = Subpath {
            start: p0,
            segs: vec![
                PathSeg::Line { end: p1 },
                PathSeg::Line { end: p2 },
            ],
            closed: false,
        };
        let ent = Entity::Path {
            subpaths: vec![sub],
            is_construction: false,
        };
        let affine = kurbo::Affine::rotate(std::f64::consts::FRAC_PI_4);
        let rot_ent = transform_entity(&ent, affine);
        if let Entity::Path { subpaths, .. } = rot_ent {
            let pts = subpaths[0].flatten(0.01);
            let mut total_len: f64 = 0.0;
            for i in 0..pts.len() - 1 {
                total_len += (pts[i + 1] - pts[i]).length();
            }
            assert!(
                (total_len - 20.0).abs() < 1e-3,
                "Length must be preserved, expected 20.0, got {total_len}"
            );
        } else {
            panic!("Expected Entity::Path");
        }
    }
}
