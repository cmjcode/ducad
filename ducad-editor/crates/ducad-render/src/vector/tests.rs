use ducad_sketch::{
    Entity, FillRule, Layer, LineCap, LineJoin, Paint, PathSeg, Rgba, Sketch, StrokeStyle, Style,
    Subpath,
};
use glam::DVec2;

use super::cache::VectorCache;
use super::tessellate::{
    subpaths_to_stroked_polylines, tessellate_entity, tessellate_fill, tessellate_stroke,
    TessError, TessOptions, Tessellated,
};
use crate::plane::SketchPlane;

fn total_area(tess: &Tessellated) -> f64 {
    let mut area = 0.0;
    for chunk in tess.indices.chunks_exact(3) {
        let v0 = tess.vertices[chunk[0] as usize].uv;
        let v1 = tess.vertices[chunk[1] as usize].uv;
        let v2 = tess.vertices[chunk[2] as usize].uv;
        let tri_area = 0.5
            * ((v1[0] - v0[0]) * (v2[1] - v0[1]) - (v2[0] - v0[0]) * (v1[1] - v0[1])).abs()
                as f64;
        area += tri_area;
    }
    area
}

#[test]
fn unit_square_fill_area_is_one() {
    let sub = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(1.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(1.0, 1.0),
            },
            PathSeg::Line {
                end: DVec2::new(0.0, 1.0),
            },
        ],
        closed: true,
    };
    let fill = Paint::Solid(Rgba::BLACK);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    let tess = tessellate_fill(&[sub], &fill, FillRule::NonZero, &plane, &opts).unwrap();
    let area = total_area(&tess);
    assert!(
        (area - 1.0).abs() < 1e-4,
        "Luas unit square ({area}) harus mendekati 1.0"
    );
}

#[test]
fn even_odd_ring_has_hole() {
    // Persegi luar 10×10 (luas 100)
    let outer = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(10.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(10.0, 10.0),
            },
            PathSeg::Line {
                end: DVec2::new(0.0, 10.0),
            },
        ],
        closed: true,
    };
    // Persegi dalam 4×4 di posisi (3,3) (luas 16)
    let inner = Subpath {
        start: DVec2::new(3.0, 3.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(7.0, 3.0),
            },
            PathSeg::Line {
                end: DVec2::new(7.0, 7.0),
            },
            PathSeg::Line {
                end: DVec2::new(3.0, 7.0),
            },
        ],
        closed: true,
    };
    let fill = Paint::Solid(Rgba::BLACK);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    let tess =
        tessellate_fill(&[outer, inner], &fill, FillRule::EvenOdd, &plane, &opts).unwrap();
    let area = total_area(&tess);
    assert!(
        (area - 84.0).abs() < 1e-3,
        "Luas ring even-odd ({area}) harus mendekati 84.0 (100 - 16)"
    );
}

#[test]
fn nonzero_same_winding_ring_is_filled() {
    // Persegi luar 10×10 searah jarum jam / CCW
    let outer = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(10.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(10.0, 10.0),
            },
            PathSeg::Line {
                end: DVec2::new(0.0, 10.0),
            },
        ],
        closed: true,
    };
    // Persegi dalam 4×4 dengan winding yang SAMA (keduanya CCW)
    let inner = Subpath {
        start: DVec2::new(3.0, 3.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(7.0, 3.0),
            },
            PathSeg::Line {
                end: DVec2::new(7.0, 7.0),
            },
            PathSeg::Line {
                end: DVec2::new(3.0, 7.0),
            },
        ],
        closed: true,
    };
    let fill = Paint::Solid(Rgba::BLACK);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    let tess =
        tessellate_fill(&[outer, inner], &fill, FillRule::NonZero, &plane, &opts).unwrap();
    let area = total_area(&tess);
    assert!(
        (area - 100.0).abs() < 1e-3,
        "Luas non-zero same winding ({area}) harus 100.0 (seluruh area terisi)"
    );
}

#[test]
fn stroke_width_produces_expected_area() {
    // Garis lurus sepanjang 10 mm
    let poly = vec![DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)];
    let stroke = StrokeStyle {
        paint: Paint::Solid(Rgba::BLACK),
        width_mm: 2.0,
        dash: vec![],
        cap: LineCap::Butt,
        join: LineJoin::Miter,
    };
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    let tess = tessellate_stroke(&[poly], &stroke, &plane, &opts);
    let area = total_area(&tess);
    assert!(
        (area - 20.0).abs() < 0.05,
        "Luas stroke butt cap ({area}) harus mendekati 20.0 mm² (10 mm × 2 mm)"
    );
}

#[test]
fn dash_pattern_splits_into_segments() {
    let sub = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![PathSeg::Line {
            end: DVec2::new(10.0, 0.0),
        }],
        closed: false,
    };
    // Garis 10 mm dengan pola [2.0, 1.0] -> segmen [0..2], [3..5], [6..8], [9..10] = 4 segmen
    let segments = subpaths_to_stroked_polylines(&[sub], &[2.0, 1.0], 0.01);
    assert_eq!(
        segments.len(),
        4,
        "Pola dash [2.0, 1.0] sepanjang 10 mm harus menghasilkan 4 segmen terpisah"
    );
}

#[test]
fn tessellate_is_deterministic() {
    let sub = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            PathSeg::Cubic {
                c1: DVec2::new(20.0, 0.0),
                c2: DVec2::new(20.0, 30.0),
                end: DVec2::new(0.0, 30.0),
            },
            PathSeg::Line {
                end: DVec2::new(0.0, 0.0),
            },
        ],
        closed: true,
    };
    let entity = Entity::path(vec![sub]);
    let style = Style {
        fill: Some(Paint::Solid(Rgba([1.0, 0.5, 0.2, 1.0]))),
        fill_rule: FillRule::NonZero,
        stroke: Some(StrokeStyle {
            paint: Paint::Solid(Rgba::BLACK),
            width_mm: 1.0,
            dash: vec![2.0, 2.0],
            cap: LineCap::Round,
            join: LineJoin::Round,
        }),
        opacity: 0.8,
        blend: ducad_sketch::BlendMode::Normal,
    };
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    let t1 = tessellate_entity(&entity, &style, &plane, &opts).unwrap();
    let t2 = tessellate_entity(&entity, &style, &plane, &opts).unwrap();

    assert_eq!(
        t1.vertices, t2.vertices,
        "Hasil vertex tesselasi harus deterministik dan identik"
    );
    assert_eq!(
        t1.indices, t2.indices,
        "Hasil indeks tesselasi harus deterministik dan identik"
    );
}

#[test]
fn degenerate_path_does_not_panic() {
    let fill = Paint::Solid(Rgba::BLACK);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    // 1. Path kosong (0 subpaths)
    let res0 = tessellate_fill(&[], &fill, FillRule::NonZero, &plane, &opts);
    assert!(
        matches!(res0, Err(TessError::Degenerate(_))),
        "0 titik harus mengembalikan TessError::Degenerate"
    );

    // 2. 1 titik (hanya start, 0 segmen)
    let sub1 = Subpath {
        start: DVec2::new(5.0, 5.0),
        segs: vec![],
        closed: false,
    };
    let res1 = tessellate_fill(&[sub1], &fill, FillRule::NonZero, &plane, &opts);
    assert!(
        matches!(res1, Err(TessError::Degenerate(_))),
        "1 titik harus mengembalikan TessError::Degenerate"
    );

    // 3. 2 titik (1 segmen garis)
    let sub2 = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![PathSeg::Line {
            end: DVec2::new(5.0, 5.0),
        }],
        closed: false,
    };
    let res2 = tessellate_fill(&[sub2], &fill, FillRule::NonZero, &plane, &opts);
    assert!(
        matches!(res2, Err(TessError::Degenerate(_))),
        "2 titik harus mengembalikan TessError::Degenerate"
    );

    // 4. Koordinat NaN
    let sub_nan = Subpath {
        start: DVec2::new(f64::NAN, 0.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(10.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(10.0, 10.0),
            },
        ],
        closed: true,
    };
    let res_nan = tessellate_fill(&[sub_nan], &fill, FillRule::NonZero, &plane, &opts);
    assert!(
        matches!(res_nan, Err(TessError::NonFiniteCoordinate)),
        "Koordinat NaN harus mengembalikan TessError::NonFiniteCoordinate"
    );
}

#[test]
fn sync_skips_unchanged_entities() {
    let mut sketch = Sketch::default();
    let sub1 = Subpath {
        start: DVec2::ZERO,
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(10.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(10.0, 10.0),
            },
            PathSeg::Line {
                end: DVec2::new(0.0, 10.0),
            },
        ],
        closed: true,
    };
    let id1 = sketch.entities.insert(Entity::path(vec![sub1]));
    sketch.styles.insert(
        id1,
        Style {
            fill: Some(Paint::Solid(Rgba::BLACK)),
            ..Style::default()
        },
    );
    sketch.touch(id1);

    let sub2 = Subpath {
        start: DVec2::new(20.0, 0.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(30.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(30.0, 10.0),
            },
            PathSeg::Line {
                end: DVec2::new(20.0, 10.0),
            },
        ],
        closed: true,
    };
    let id2 = sketch.entities.insert(Entity::path(vec![sub2]));
    sketch.styles.insert(
        id2,
        Style {
            fill: Some(Paint::Solid(Rgba::WHITE)),
            ..Style::default()
        },
    );
    sketch.touch(id2);

    let mut cache = VectorCache::new(64 * 1024 * 1024);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    // First sync: both are tessellated
    let changed = cache.sync(&sketch, &plane, &opts, &[id1, id2]);
    assert!(changed, "Sync pertama harus mengembalikan true");
    assert_eq!(
        cache.tessellate_count, 2,
        "Dua entitas baru harus ditesselasi"
    );

    // Second sync without changes: nothing tessellated, returns false
    let changed2 = cache.sync(&sketch, &plane, &opts, &[id1, id2]);
    assert!(
        !changed2,
        "Sync kedua tanpa perubahan harus mengembalikan false"
    );
    assert_eq!(
        cache.tessellate_count, 2,
        "Jumlah panggilan tesselasi tidak boleh bertambah"
    );
}

#[test]
fn sync_retessellates_when_rev_changes() {
    let mut sketch = Sketch::default();
    let sub1 = Subpath {
        start: DVec2::ZERO,
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(10.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(10.0, 10.0),
            },
        ],
        closed: true,
    };
    let id1 = sketch.entities.insert(Entity::path(vec![sub1]));
    sketch.styles.insert(
        id1,
        Style {
            fill: Some(Paint::Solid(Rgba::BLACK)),
            ..Style::default()
        },
    );
    sketch.touch(id1);

    let sub2 = Subpath {
        start: DVec2::new(20.0, 0.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(30.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(30.0, 10.0),
            },
        ],
        closed: true,
    };
    let id2 = sketch.entities.insert(Entity::path(vec![sub2]));
    sketch.styles.insert(
        id2,
        Style {
            fill: Some(Paint::Solid(Rgba::WHITE)),
            ..Style::default()
        },
    );
    sketch.touch(id2);

    let mut cache = VectorCache::new(64 * 1024 * 1024);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    cache.sync(&sketch, &plane, &opts, &[id1, id2]);
    assert_eq!(cache.tessellate_count, 2);

    // Touch id1 so rev increases
    sketch.touch(id1);

    let changed = cache.sync(&sketch, &plane, &opts, &[id1, id2]);
    assert!(changed, "Sync harus melaporkan perubahan ketika rev naik");
    assert_eq!(
        cache.tessellate_count, 3,
        "Hanya id1 yang ditesselasi ulang, id2 harus dilewati"
    );
}

#[test]
fn cache_evicts_over_budget() {
    let mut sketch = Sketch::default();
    let sub1 = Subpath {
        start: DVec2::ZERO,
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(10.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(10.0, 10.0),
            },
        ],
        closed: true,
    };
    let id1 = sketch.entities.insert(Entity::path(vec![sub1]));
    sketch.styles.insert(
        id1,
        Style {
            fill: Some(Paint::Solid(Rgba::BLACK)),
            ..Style::default()
        },
    );
    sketch.touch(id1);

    let sub2 = Subpath {
        start: DVec2::new(20.0, 0.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(30.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(30.0, 10.0),
            },
        ],
        closed: true,
    };
    let id2 = sketch.entities.insert(Entity::path(vec![sub2]));
    sketch.styles.insert(
        id2,
        Style {
            fill: Some(Paint::Solid(Rgba::WHITE)),
            ..Style::default()
        },
    );
    sketch.touch(id2);

    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    // Hitung ukuran satu entri
    let mut probe_cache = VectorCache::new(1024 * 1024);
    probe_cache.sync(&sketch, &plane, &opts, &[id1]);
    let single_entry_bytes = probe_cache.current_bytes();
    assert!(single_entry_bytes > 0);

    // Atur budget tepat untuk memuat hanya 1 entri
    let mut tight_cache = VectorCache::new(single_entry_bytes + 10);
    tight_cache.sync(&sketch, &plane, &opts, &[id1]);
    assert!(tight_cache.entries.contains_key(&id1));

    // Sync id2: id1 harus digusur karena LRU
    tight_cache.sync(&sketch, &plane, &opts, &[id2]);
    assert!(tight_cache.current_bytes() <= tight_cache.budget_bytes());
    assert!(tight_cache.entries.contains_key(&id2));
    assert!(
        !tight_cache.entries.contains_key(&id1),
        "id1 yang lebih lama harus digusur"
    );
}

#[test]
fn batches_follow_draw_order() {
    let mut sketch = Sketch::default();
    let l1 = sketch.layers.insert(Layer::new("Layer 1", Rgba::BLACK));
    let l2 = sketch.layers.insert(Layer::new("Layer 2", Rgba::WHITE));
    sketch.layer_order = vec![l1, l2];

    let id1 = sketch.entities.insert(Entity::circle(DVec2::ZERO, 5.0));
    sketch.styles.insert(
        id1,
        Style {
            fill: Some(Paint::Solid(Rgba::BLACK)),
            ..Style::default()
        },
    );
    sketch.entity_layer.insert(id1, l1);

    let id2 = sketch
        .entities
        .insert(Entity::circle(DVec2::new(10.0, 0.0), 5.0));
    sketch.styles.insert(
        id2,
        Style {
            fill: Some(Paint::Solid(Rgba::WHITE)),
            ..Style::default()
        },
    );
    sketch.entity_layer.insert(id2, l2);

    let mut cache = VectorCache::new(64 * 1024 * 1024);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();
    cache.sync(&sketch, &plane, &opts, &[id1, id2]);

    let batches = cache.batches(&sketch);
    assert_eq!(batches.len(), 2, "Harus menghasilkan 2 layer batch");
    assert_eq!(batches[0].layer, l1, "Batch pertama harus Layer 1");
    assert_eq!(batches[1].layer, l2, "Batch kedua harus Layer 2");
}

#[test]
fn hidden_entity_not_in_batch() {
    let mut sketch = Sketch::default();
    let id1 = sketch.entities.insert(Entity::circle(DVec2::ZERO, 5.0));
    sketch.styles.insert(
        id1,
        Style {
            fill: Some(Paint::Solid(Rgba::BLACK)),
            ..Style::default()
        },
    );

    let id2 = sketch
        .entities
        .insert(Entity::circle(DVec2::new(10.0, 0.0), 5.0));
    sketch.styles.insert(
        id2,
        Style {
            fill: Some(Paint::Solid(Rgba::WHITE)),
            ..Style::default()
        },
    );

    // Sembunyikan id1
    sketch.hidden_entities.insert(id1);

    let mut cache = VectorCache::new(64 * 1024 * 1024);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();
    cache.sync(&sketch, &plane, &opts, &[id1, id2]);

    let batches = cache.batches(&sketch);
    assert_eq!(batches.len(), 1);
    let expected_count = cache.entries[&id2].tess.vertices.len();
    assert_eq!(
        batches[0].vertices.len(),
        expected_count,
        "Entitas tersembunyi tidak boleh masuk ke dalam batch"
    );
}

#[test]
fn vertex_layout_matches_shader() {
    use std::mem::{offset_of, size_of};
    use super::cache::{GradientStop, GradientUniform};
    use super::tessellate::VectorVertex;

    // Ukuran VectorVertex harus tepat 48 byte
    assert_eq!(size_of::<VectorVertex>(), 48);
    assert_eq!(offset_of!(VectorVertex, pos), 0);
    assert_eq!(offset_of!(VectorVertex, color), 12);
    assert_eq!(offset_of!(VectorVertex, paint), 28);
    assert_eq!(offset_of!(VectorVertex, uv), 32);
    assert_eq!(offset_of!(VectorVertex, _pad), 40);

    // Gradient uniform layout untuk shader WGSL std140
    assert_eq!(size_of::<GradientStop>(), 32);
    assert_eq!(size_of::<GradientUniform>(), 288);
    assert_eq!(offset_of!(GradientUniform, kind), 0);
    assert_eq!(offset_of!(GradientUniform, count), 4);
    assert_eq!(offset_of!(GradientUniform, p0), 8);
    assert_eq!(offset_of!(GradientUniform, p1), 16);
    assert_eq!(offset_of!(GradientUniform, _pad), 24);
    assert_eq!(offset_of!(GradientUniform, stops), 32);
}

#[test]
fn wgsl_vector_shader_compiles() {
    let shader_str = include_str!("shader_vector.wgsl");
    let module = egui_wgpu::wgpu::naga::front::wgsl::parse_str(shader_str);
    assert!(module.is_ok(), "WGSL parse error: {:?}", module.err());
}

