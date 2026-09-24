//! Pengelolaan kanvas tak hingga, chunking rendering, batas memori, dan fit tampilan tinta.

use ducad_ink::brush::BrushId;
use ducad_ink::brush::BrushKind;
use ducad_ink::stroke::InkPoint;
use ducad_ink::InkDoc;
use ducad_render::ink::{
    append_stroke_vertices, build_stroke_vertices, InkBrushKind, InkBrushRef, InkLayerBatch,
    InkPointRef, InkVertex,
};
use ducad_render::{CameraMode, SketchPlane};
use ducad_sketch::layer::LayerId;
use ducad_sketch::style::Rgba;
use glam::{DVec2, Vec2};

use crate::app::DuCADApp;

/// Ukuran chunk coretan per batch render untuk efisiensi culling viewport (1.000 coretan).
pub const INK_CHUNK_STROKE_SIZE: usize = 1_000;

/// Ambang batas peringatan jumlah coretan per dokumen (100.000 coretan).
pub const INK_MAX_STROKE_WARNING: usize = 100_000;

/// Menghasilkan batch render layer tinta yang dipartisi per chunk 1.000 coretan
/// dengan viewport culling (`visible_in`).
pub fn build_ink_layer_batches(
    doc: &InkDoc,
    viewport_bounds: Option<(Vec2, Vec2)>,
    active_plane: &SketchPlane,
    layers_visible: &dyn Fn(LayerId) -> bool,
) -> Vec<InkLayerBatch> {
    if doc.strokes.is_empty() {
        return Vec::new();
    }

    // Kelompokkan coretan per layer tinta, urutan layer = kemunculan pertama
    // di z-order (bukan iterasi HashMap — batch harus deterministik).
    let mut layer_strokes: Vec<(LayerId, Vec<usize>)> = Vec::new();
    for (idx, stroke) in doc.strokes.iter().enumerate() {
        if stroke.hidden || !layers_visible(stroke.layer) {
            continue;
        }
        match layer_strokes.iter_mut().find(|(l, _)| *l == stroke.layer) {
            Some((_, v)) => v.push(idx),
            None => layer_strokes.push((stroke.layer, vec![idx])),
        }
    }

    let mut batches = Vec::new();

    for (layer_id, stroke_indices) in layer_strokes {
        let mut layer_vertices = Vec::new();

        // Bagi coretan layer ini menjadi potongan/chunk per 1.000 coretan
        for chunk in stroke_indices.chunks(INK_CHUNK_STROKE_SIZE) {
            // Hitung bounding box keseluruhan chunk untuk viewport culling cepat
            let mut chunk_min = Vec2::splat(f32::MAX);
            let mut chunk_max = Vec2::splat(f32::MIN);

            for &idx in chunk {
                let s = &doc.strokes[idx];
                chunk_min = chunk_min.min(s.bbox.0);
                chunk_max = chunk_max.max(s.bbox.1);
            }

            // Viewport culling: lewati chunk jika sepenuhnya di luar viewport
            if let Some((v_min, v_max)) = viewport_bounds {
                if chunk_max.x < v_min.x
                    || chunk_min.x > v_max.x
                    || chunk_max.y < v_min.y
                    || chunk_min.y > v_max.y
                {
                    continue;
                }
            }

            // Tessellate coretan-coretan dalam chunk yang terlihat
            for &idx in chunk {
                let stroke = &doc.strokes[idx];
                if stroke.points.is_empty() {
                    continue;
                }

                // Cek apakah coretan ini sendiri berada dalam viewport jika batas viewport diberikan
                if let Some((v_min, v_max)) = viewport_bounds {
                    if stroke.bbox.1.x < v_min.x
                        || stroke.bbox.0.x > v_max.x
                        || stroke.bbox.1.y < v_min.y
                        || stroke.bbox.0.y > v_max.y
                    {
                        continue;
                    }
                }

                let brush_ref = brush_ref_for(doc, stroke.brush, stroke.color);
                let points_ref = points_ref_of(&stroke.points);
                let mut stroke_vertices = Vec::new();
                build_stroke_vertices(&points_ref, &brush_ref, active_plane, &mut stroke_vertices);
                append_strip(&mut layer_vertices, &stroke_vertices);
            }
        }

        if !layer_vertices.is_empty() {
            batches.push(InkLayerBatch {
                layer: layer_id,
                vertices: layer_vertices,
            });
        }
    }

    batches
}

/// Sambungkan quad-strip `next` ke `strip` dengan dua vertex degenerate
/// (ulang vertex terakhir & pertama) — tanpa ini satu `TriangleStrip` per
/// layer menarik segitiga penghubung antar-coretan.
pub fn append_strip(strip: &mut Vec<InkVertex>, next: &[InkVertex]) {
    let Some(first) = next.first() else {
        return;
    };
    if let Some(&last) = strip.last() {
        strip.push(last);
        strip.push(*first);
    }
    strip.extend_from_slice(next);
}

/// Parameter kuas render untuk coretan dengan kuas `brush` dan warna `color`.
pub fn brush_ref_for(doc: &InkDoc, brush: BrushId, color: Rgba) -> InkBrushRef {
    let brush = doc
        .brushes
        .get(brush)
        .cloned()
        .unwrap_or_else(|| ducad_ink::brush::Brush::presets().swap_remove(0));
    let kind = match brush.kind {
        BrushKind::Pen => InkBrushKind::Pen,
        BrushKind::Pencil => InkBrushKind::Pencil,
        BrushKind::Marker | BrushKind::Fill => InkBrushKind::Marker,
    };
    InkBrushRef {
        color: color.0,
        width_min: brush.width_min_mm,
        width_max: brush.width_max_mm,
        opacity: brush.opacity,
        kind,
    }
}

/// Titik coretan dalam bentuk ringan untuk `ducad-render`.
pub fn points_ref_of(points: &[InkPoint]) -> Vec<InkPointRef> {
    points
        .iter()
        .map(|p| InkPointRef {
            pos: [p.x, p.y],
            pressure: p.pressure,
        })
        .collect()
}

/// Pembaruan buffer coretan aktif untuk satu frame: GPU memotong ke `keep`
/// vertex lalu menambahkan `tail` (`truncate_active_ink` + `push_active_ink`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ActiveInkUpdate {
    pub keep: usize,
    pub tail: Vec<InkVertex>,
}

/// Cache render tinta milik app (M4.5–M4.6).
#[derive(Debug, Default)]
pub struct InkRenderCache {
    /// (revisi dokumen, bidang) saat batch layer terakhir dibangun.
    built: Option<(u64, SketchPlane)>,
    /// Vertex titik NYATA coretan aktif, dibangun inkremental.
    active_real: Vec<InkVertex>,
    active_real_points: usize,
    /// Jumlah titik nyata saat frame sebelumnya diunggah.
    uploaded_real_points: usize,
}

impl DuCADApp {
    /// Batch layer tinta untuk diunggah frame ini, atau `None` bila dokumen
    /// dan bidang tidak berubah sejak unggahan terakhir (GPU tetap memakai
    /// buffer lama — tidak ada `write_buffer`).
    pub fn ink_layers_for_frame(&mut self) -> Option<Vec<InkLayerBatch>> {
        let key = (self.ink.rev, self.active_plane);
        if self.ink_render.built == Some(key) {
            return None;
        }
        self.ink_render.built = Some(key);
        Some(build_ink_layer_batches(
            &self.ink,
            None,
            &self.active_plane,
            &|_| true,
        ))
    }

    /// Pembaruan coretan aktif frame ini. Titik nyata ditambahkan lewat
    /// `append_stroke_vertices` (O(titik baru)); titik prediksi dibangun di
    /// atas salinan dan dibuang lagi frame berikutnya. Hanya vertex yang
    /// berubah sejak frame sebelumnya yang dikirim ke GPU.
    pub fn active_ink_for_frame(&mut self) -> ActiveInkUpdate {
        let real = &self.ink_state.active_points;
        let cache = &mut self.ink_render;
        if real.is_empty() {
            cache.active_real.clear();
            cache.active_real_points = 0;
            cache.uploaded_real_points = 0;
            return ActiveInkUpdate::default();
        }
        // Coretan baru (titik berkurang): mulai dari nol.
        if real.len() < cache.active_real_points {
            cache.active_real.clear();
            cache.active_real_points = 0;
            cache.uploaded_real_points = 0;
        }

        let brush_id = self.ink_state.active_brush.unwrap_or_default();
        let brush = brush_ref_for(&self.ink, brush_id, self.ink_state.active_color);
        let plane = self.active_plane;
        let real_ref = points_ref_of(real);
        let cache = &mut self.ink_render;
        append_stroke_vertices(
            &real_ref,
            cache.active_real_points,
            &brush,
            &plane,
            &mut cache.active_real,
        );
        cache.active_real_points = real_ref.len();

        let mut frame = cache.active_real.clone();
        if let Some(pred) = self.ink_state.predicted_point {
            let mut with_pred = real_ref.clone();
            with_pred.extend(points_ref_of(&[pred]));
            append_stroke_vertices(&with_pred, real_ref.len(), &brush, &plane, &mut frame);
        }

        // Pasangan vertex titik i bergantung pada titik i−1..i+1, start cap
        // pada titik 0..1: yang stabil sejak frame lalu adalah cap awal +
        // pasangan titik 0..=n−2 (n = titik nyata frame lalu).
        let prev = cache.uploaded_real_points;
        let keep = if prev >= 2 {
            (2 + 2 * (prev - 1)).min(frame.len())
        } else {
            0
        };
        cache.uploaded_real_points = real_ref.len();
        ActiveInkUpdate {
            keep,
            tail: frame[keep..].to_vec(),
        }
    }

    /// Peringatan saat coretan tinta melebihi ambang batas performa 100k coretan.
    pub fn ink_stroke_count_warning(&self) -> Option<&'static str> {
        if self.ink.strokes.len() >= INK_MAX_STROKE_WARNING {
            Some("Peringatan: Dokumen memiliki lebih dari 100.000 coretan tinta. Kinerja rendering mungkin menurun.")
        } else {
            None
        }
    }

    /// Total penggunaan memori dokumen tinta dalam byte.
    pub fn ink_memory_bytes(&self) -> usize {
        self.ink.memory_bytes()
    }

    /// Format ramah baca untuk penggunaan memori dokumen tinta (mis. "124 B", "45.2 KB", "12.8 MB").
    pub fn ink_memory_formatted(&self) -> String {
        let b = self.ink_memory_bytes();
        if b < 1024 {
            format!("{b} B")
        } else if b < 1024 * 1024 {
            format!("{:.1} KB", b as f64 / 1024.0)
        } else {
            format!("{:.1} MB", b as f64 / (1024.0 * 1024.0))
        }
    }

    /// Mengarahkan kamera dan zoom agar pas menampilkan seluruh konten tinta ("Fit ke isi").
    pub fn fit_to_ink_content(&mut self) {
        let visible_strokes: Vec<&ducad_ink::stroke::Stroke> =
            self.ink.strokes.iter().filter(|s| !s.hidden).collect();
        if visible_strokes.is_empty() {
            return;
        }

        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);

        for s in visible_strokes {
            min = min.min(s.bbox.0);
            max = max.max(s.bbox.1);
        }

        let center = (min + max) * 0.5;
        let size = max - min;

        // Orientasikan dulu: `set_mode(Ortho2D)` mereset target ke origin
        // bidang, jadi harus mendahului penempatan target di bawah.
        if self.app_mode.is_ink() {
            self.camera.set_mode(CameraMode::Ortho2D {
                plane: self.active_plane,
            });
        }

        // Pusatkan target kamera ke tengah bidang konten
        let world_center = self
            .active_plane
            .to_world(DVec2::new(center.x as f64, center.y as f64), 0.0);
        self.camera.target = world_center;

        // Hitung jarak kamera agar seluruh luas bidang masuk ke dalam viewport dengan margin 20%
        let extent = size.x.max(size.y).max(10.0) * 1.2;
        let half_fov = (self.camera.fov_y * 0.5).tan();
        self.camera.distance = (extent / (2.0 * half_fov)).clamp(50.0, 50_000.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::AppMode;

    fn pt(i: usize) -> InkPoint {
        let x = i as f32 * 2.0;
        InkPoint::new(
            x,
            (x * 0.3).sin() * 4.0,
            0.4 + 0.02 * i as f32,
            0.0,
            i as u32 * 8,
        )
    }

    /// Regresi REVIEW-2026-09-24 #11: buffer GPU coretan aktif yang diperbarui
    /// inkremental (truncate + push) identik dengan rebuild penuh tiap frame,
    /// termasuk titik prediksi yang dibuang frame berikutnya.
    #[test]
    fn active_ink_incremental_upload_matches_full_rebuild() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);
        let brush = app.get_or_init_active_brush();
        let mut gpu: Vec<InkVertex> = Vec::new();

        for n in 1..=12 {
            app.ink_state.active_points = (0..n).map(pt).collect();
            app.ink_state.predicted_point = (n % 3 != 0).then(|| pt(n + 1));
            let up = app.active_ink_for_frame();
            gpu.truncate(up.keep);
            gpu.extend_from_slice(&up.tail);

            let mut expected_pts = app.ink_state.active_points.clone();
            expected_pts.extend(app.ink_state.predicted_point);
            let mut expected = Vec::new();
            let brush_ref = brush_ref_for(&app.ink, brush, app.ink_state.active_color);
            build_stroke_vertices(
                &points_ref_of(&expected_pts),
                &brush_ref,
                &app.active_plane,
                &mut expected,
            );
            assert_eq!(gpu.len(), expected.len(), "n={n}");
            for (a, b) in gpu.iter().zip(&expected) {
                for k in 0..3 {
                    assert!((a.pos[k] - b.pos[k]).abs() < 1e-4, "n={n}: {a:?} vs {b:?}");
                }
            }
            if n > 3 {
                assert!(up.keep > 0, "n={n}: bagian stabil tidak diunggah ulang");
            }
        }

        app.ink_state.active_points.clear();
        app.ink_state.predicted_point = None;
        assert_eq!(app.active_ink_for_frame(), ActiveInkUpdate::default());
    }

    #[test]
    fn ink_layers_upload_only_when_document_changes() {
        let mut app = DuCADApp::new_for_test();
        let brush = app.get_or_init_active_brush();
        let layer = app.get_or_create_ink_layer();
        app.ink.add_stroke(ducad_ink::Stroke::new(
            0,
            (0..4).map(pt).collect(),
            brush,
            Rgba::BLACK,
            layer,
        ));
        assert!(app.ink_layers_for_frame().is_some());
        assert!(
            app.ink_layers_for_frame().is_none(),
            "tanpa perubahan: tidak ada unggahan"
        );
        app.ink.touch();
        assert!(app.ink_layers_for_frame().is_some());
    }

    #[test]
    fn strokes_in_one_layer_are_joined_with_degenerate_vertices() {
        let mut app = DuCADApp::new_for_test();
        let brush = app.get_or_init_active_brush();
        let layer = app.get_or_create_ink_layer();
        for k in 0..2 {
            let pts = (0..3).map(|i| pt(i + 10 * k)).collect();
            app.ink
                .add_stroke(ducad_ink::Stroke::new(0, pts, brush, Rgba::BLACK, layer));
        }
        let batches = build_ink_layer_batches(&app.ink, None, &app.active_plane, &|_| true);
        assert_eq!(batches.len(), 1);
        let per_stroke = 2 * 3 + 4;
        let v = &batches[0].vertices;
        assert_eq!(v.len(), 2 * per_stroke + 2);
        assert_eq!(
            v[per_stroke],
            v[per_stroke - 1],
            "vertex degenerate: ulang terakhir"
        );
        assert_eq!(
            v[per_stroke + 1],
            v[per_stroke + 2],
            "vertex degenerate: ulang pertama"
        );
    }
}
