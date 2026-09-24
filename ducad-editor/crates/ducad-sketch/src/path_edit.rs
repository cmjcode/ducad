//! Operasi pengeditan path vektor dan konversi bentuk dasar ke path (M2).
//!
//! Logika murni tanpa ketergantungan GUI.

use glam::DVec2;
use crate::entity::{PathSeg, Subpath};

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
}
