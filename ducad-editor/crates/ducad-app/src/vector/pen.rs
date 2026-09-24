//! Tool Pen Bézier interaktif (M2.2).
//!
//! Logika interaksi untuk menggambar path Bézier seperti CorelDraw / Illustrator:
//! - klik = node sudut (segmen Line)
//! - klik-drag = node halus (handle simetris, Cubic)
//! - Alt saat drag = patahkan handle (smooth_broken)
//! - klik node awal = tutup subpath
//! - Enter/Esc = akhiri terbuka
//! - Backspace = batalkan node terakhir
//! - Shift = kunci sudut 45°
//! - Snap ke node/titik entitas lain

use ducad_render::LineVertex;
use ducad_sketch::path_edit::{snap_angle_45, PenBuilder};
use ducad_sketch::{EntityId, PathSeg, Subpath};
use glam::DVec2;

/// Handler murni untuk tool Pen Bézier.
pub struct PenTool;

impl PenTool {
    /// Menangani penekanan pointer (mouse down / touch tap).
    ///
    /// `close_tol` = jarak (mm) ke node awal yang dianggap "klik node awal".
    pub fn handle_pointer_down(
        builder: &mut PenBuilder,
        pos: DVec2,
        shift: bool,
        snap_pos: Option<DVec2>,
        close_tol: f64,
    ) -> Option<Subpath> {
        let raw_pos = snap_pos.unwrap_or(pos);
        let final_pos = if shift {
            if let Some(prev) = builder.last_point() {
                snap_angle_45(prev, raw_pos)
            } else {
                raw_pos
            }
        } else {
            raw_pos
        };

        // Jika kursor mengklik kembali titik awal (node penutup):
        if let Some(start) = builder.start {
            if builder.segs.len() >= 2 && (final_pos - start).length() <= close_tol.max(1e-4) {
                return std::mem::take(builder).finish(true);
            }
        }

        builder.corner(final_pos);
        None
    }

    /// Menangani drag pointer sesudah `handle_pointer_down`: node yang baru
    /// ditekan (`node`) diganti menjadi node halus dengan handle keluar di
    /// `current_pos`. `before` = keadaan builder SEBELUM node itu ditambahkan,
    /// sehingga setiap frame drag menghitung ulang node yang sama — bukan
    /// menambah node baru per frame. Alt = handle patah (masuk tetap di node).
    pub fn handle_pointer_drag(
        builder: &mut PenBuilder,
        before: &PenBuilder,
        node: DVec2,
        current_pos: DVec2,
        alt: bool,
    ) {
        *builder = before.clone();
        if alt {
            builder.smooth_broken(node, node, current_pos);
        } else {
            builder.smooth(node, current_pos);
        }
    }

    /// Menghasilkan vertex segmen pratinjau (preview overlay) untuk dirender di kanvas.
    pub fn preview_overlay_lines(
        builder: &PenBuilder,
        cursor: DVec2,
        shift: bool,
        color: [f32; 4],
    ) -> Vec<LineVertex> {
        let mut lines = Vec::new();
        let target = if shift {
            if let Some(prev) = builder.last_point() {
                snap_angle_45(prev, cursor)
            } else {
                cursor
            }
        } else {
            cursor
        };

        if let Some(seg) = builder.preview(target) {
            let prev = builder.last_point().unwrap_or(target);
            match seg {
                PathSeg::Line { end } => {
                    lines.push(LineVertex {
                        position: [prev.x as f32, prev.y as f32, 0.0],
                        color,
                    });
                    lines.push(LineVertex {
                        position: [end.x as f32, end.y as f32, 0.0],
                        color,
                    });
                }
                PathSeg::Cubic { c1, c2, end } => {
                    let steps = 16;
                    let mut last_pt = prev;
                    for i in 1..=steps {
                        let t = i as f64 / steps as f64;
                        let u = 1.0 - t;
                        let pt = prev * (u * u * u)
                            + c1 * (3.0 * u * u * t)
                            + c2 * (3.0 * u * t * t)
                            + end * (t * t * t);
                        lines.push(LineVertex {
                            position: [last_pt.x as f32, last_pt.y as f32, 0.0],
                            color,
                        });
                        lines.push(LineVertex {
                            position: [pt.x as f32, pt.y as f32, 0.0],
                            color,
                        });
                        last_pt = pt;
                    }
                }
            }
        }
        lines
    }
}

/// Skenario commit path ke sketch.
pub struct PenCommitOutcome {
    pub entity_id: EntityId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pen_tool_close_on_start_point() {
        let mut builder = PenBuilder::new();
        let p0 = DVec2::new(0.0, 0.0);
        let p1 = DVec2::new(10.0, 0.0);
        let p2 = DVec2::new(10.0, 10.0);

        assert!(PenTool::handle_pointer_down(&mut builder, p0, false, None, 1e-4).is_none());
        assert!(PenTool::handle_pointer_down(&mut builder, p1, false, None, 1e-4).is_none());
        assert!(PenTool::handle_pointer_down(&mut builder, p2, false, None, 1e-4).is_none());

        // Klik titik awal p0 untuk menutup
        let finished = PenTool::handle_pointer_down(&mut builder, p0, false, None, 1e-4);
        assert!(finished.is_some());
        let sub = finished.unwrap();
        assert!(sub.closed);
        assert_eq!(sub.node_count(), 3);
    }

    #[test]
    fn test_pen_preview_overlay_generates_vertices() {
        let mut builder = PenBuilder::new();
        builder.corner(DVec2::new(0.0, 0.0));
        let cursor = DVec2::new(50.0, 50.0);
        let lines = PenTool::preview_overlay_lines(&builder, cursor, false, [1.0, 1.0, 0.0, 1.0]);
        assert_eq!(lines.len(), 2);
    }
}
