use ducad_sketch::{SnapHit, SnapKind};
use glam::DVec2;

use crate::grid::LineVertex;
use crate::plane::SketchPlane;
use crate::sketch::{push_arc, to3, COLOR_PICKED, COLOR_REMOVAL, COLOR_SNAP};

/// Marker silang ungu di titik yang sudah diklik untuk tool pemilihan titik pada bidang aktif.
pub fn picked_point_glyph(point: DVec2, plane: &SketchPlane) -> Vec<LineVertex> {
    const S: f64 = 3.0;
    vec![
        LineVertex {
            position: to3(plane, point + DVec2::new(-S, -S)),
            color: COLOR_PICKED,
        },
        LineVertex {
            position: to3(plane, point + DVec2::new(S, S)),
            color: COLOR_PICKED,
        },
        LineVertex {
            position: to3(plane, point + DVec2::new(-S, S)),
            color: COLOR_PICKED,
        },
        LineVertex {
            position: to3(plane, point + DVec2::new(S, -S)),
            color: COLOR_PICKED,
        },
    ]
}

/// Garis peringatan untuk sub-segmen yang akan dihapus tool Trim.
pub fn removal_preview_lines(start: DVec2, end: DVec2, plane: &SketchPlane) -> Vec<LineVertex> {
    vec![
        LineVertex {
            position: to3(plane, start),
            color: COLOR_REMOVAL,
        },
        LineVertex {
            position: to3(plane, end),
            color: COLOR_REMOVAL,
        },
    ]
}

/// Glyph indikator snap pada bidang aktif.
pub fn snap_glyph(hit: &SnapHit, plane: &SketchPlane) -> Vec<LineVertex> {
    const S: f64 = 3.0;
    let c = hit.point;
    let mut verts = Vec::new();
    let mut push_loop = |pts: &[DVec2]| {
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            verts.push(LineVertex {
                position: to3(plane, a),
                color: COLOR_SNAP,
            });
            verts.push(LineVertex {
                position: to3(plane, b),
                color: COLOR_SNAP,
            });
        }
    };

    match hit.kind {
        SnapKind::Endpoint => push_loop(&[
            c + DVec2::new(-S, -S),
            c + DVec2::new(S, -S),
            c + DVec2::new(S, S),
            c + DVec2::new(-S, S),
        ]),
        SnapKind::Midpoint => push_loop(&[
            c + DVec2::new(0.0, S),
            c + DVec2::new(S, -S),
            c + DVec2::new(-S, -S),
        ]),
        SnapKind::Center => push_arc(&mut verts, c, S, 0.0, std::f64::consts::TAU, COLOR_SNAP, plane),
        SnapKind::Intersection => push_loop(&[
            c + DVec2::new(-S, 0.0),
            c + DVec2::new(0.0, -S),
            c + DVec2::new(S, 0.0),
            c + DVec2::new(0.0, S),
        ]),
        SnapKind::Grid => {
            let cross = [
                (c + DVec2::new(-S, 0.0), c + DVec2::new(S, 0.0)),
                (c + DVec2::new(0.0, -S), c + DVec2::new(0.0, S)),
            ];
            for (a, b) in cross {
                verts.push(LineVertex {
                    position: to3(plane, a),
                    color: COLOR_SNAP,
                });
                verts.push(LineVertex {
                    position: to3(plane, b),
                    color: COLOR_SNAP,
                });
            }
        }
    }
    verts
}

const COLOR_CANDIDATE_CENTER: [f32; 4] = [0.15, 0.75, 1.0, 0.85];
const COLOR_CANDIDATE_MIDPOINT: [f32; 4] = [0.35, 0.90, 0.45, 0.75];
const COLOR_CANDIDATE_ENDPOINT: [f32; 4] = [0.80, 0.85, 0.95, 0.70];
const COLOR_CANDIDATE_INTERSECT: [f32; 4] = [1.0, 0.70, 0.20, 0.75];

/// Render visual marker untuk semua titik kandidat snap (khususnya saat operasi drag berlangsung).
pub fn candidate_snap_points_glyphs(
    points: &[(DVec2, SnapKind)],
    plane: &SketchPlane,
) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    const S_CENTER: f64 = 3.5;
    const S_SMALL: f64 = 2.5;

    for &(pt, kind) in points {
        match kind {
            SnapKind::Center => {
                // Circle kecil + crosshair di tengahnya
                push_arc(
                    &mut verts,
                    pt,
                    S_CENTER,
                    0.0,
                    std::f64::consts::TAU,
                    COLOR_CANDIDATE_CENTER,
                    plane,
                );
                let cross = [
                    (
                        pt + DVec2::new(-S_CENTER * 0.7, 0.0),
                        pt + DVec2::new(S_CENTER * 0.7, 0.0),
                    ),
                    (
                        pt + DVec2::new(0.0, -S_CENTER * 0.7),
                        pt + DVec2::new(0.0, S_CENTER * 0.7),
                    ),
                ];
                for (a, b) in cross {
                    verts.push(LineVertex {
                        position: to3(plane, a),
                        color: COLOR_CANDIDATE_CENTER,
                    });
                    verts.push(LineVertex {
                        position: to3(plane, b),
                        color: COLOR_CANDIDATE_CENTER,
                    });
                }
            }
            SnapKind::Midpoint => {
                let pts = [
                    pt + DVec2::new(0.0, S_SMALL),
                    pt + DVec2::new(S_SMALL, -S_SMALL),
                    pt + DVec2::new(-S_SMALL, -S_SMALL),
                ];
                for i in 0..pts.len() {
                    let a = pts[i];
                    let b = pts[(i + 1) % pts.len()];
                    verts.push(LineVertex {
                        position: to3(plane, a),
                        color: COLOR_CANDIDATE_MIDPOINT,
                    });
                    verts.push(LineVertex {
                        position: to3(plane, b),
                        color: COLOR_CANDIDATE_MIDPOINT,
                    });
                }
            }
            SnapKind::Endpoint => {
                let pts = [
                    pt + DVec2::new(-S_SMALL, -S_SMALL),
                    pt + DVec2::new(S_SMALL, -S_SMALL),
                    pt + DVec2::new(S_SMALL, S_SMALL),
                    pt + DVec2::new(-S_SMALL, S_SMALL),
                ];
                for i in 0..pts.len() {
                    let a = pts[i];
                    let b = pts[(i + 1) % pts.len()];
                    verts.push(LineVertex {
                        position: to3(plane, a),
                        color: COLOR_CANDIDATE_ENDPOINT,
                    });
                    verts.push(LineVertex {
                        position: to3(plane, b),
                        color: COLOR_CANDIDATE_ENDPOINT,
                    });
                }
            }
            SnapKind::Intersection => {
                let pts = [
                    pt + DVec2::new(-S_SMALL, 0.0),
                    pt + DVec2::new(0.0, -S_SMALL),
                    pt + DVec2::new(S_SMALL, 0.0),
                    pt + DVec2::new(0.0, S_SMALL),
                ];
                for i in 0..pts.len() {
                    let a = pts[i];
                    let b = pts[(i + 1) % pts.len()];
                    verts.push(LineVertex {
                        position: to3(plane, a),
                        color: COLOR_CANDIDATE_INTERSECT,
                    });
                    verts.push(LineVertex {
                        position: to3(plane, b),
                        color: COLOR_CANDIDATE_INTERSECT,
                    });
                }
            }
            SnapKind::Grid => {}
        }
    }
    verts
}

