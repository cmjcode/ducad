//! Fase 3 — jembatan dokumen 3D (`ducad-core::Document`) ke geometri
//! kernel (`ducad-kernel::KernelShape`), plus command undo-able untuk
//! operasi modeling (Extrude, Union/Subtract, Fillet/Chamfer semua tepi,
//! Shell/Hollow, Hapus Body).
//!
//! `ducad-core::Document` sengaja bebas dependensi kernel (lihat komentar
//! di crate itu) — jadi geometri B-rep sungguhan hidup DI LUAR `Document`,
//! di `ModelDoc::geometry`, sebuah `SecondaryMap` yang dikunci dengan
//! `BodyId` yang SAMA dengan yang dipakai `Document::bodies`. `ModelDoc`
//! itulah target generik `ducad_core::Command<T>` untuk seluruh command
//! di modul ini — bukan `Document` langsung — karena command butuh
//! memutasi keduanya (metadata + geometri) sebagai satu langkah undo.
//!
//! Konsisten dengan `ducad_sketch::DeleteEntities`: `BodyId` TIDAK stabil
//! lintas undo/redo (slotmap tidak menjamin key lama bisa dipakai lagi) —
//! body yang dihapus lalu di-undo muncul kembali dengan id baru. Pemanggil
//! (UI) diharapkan mengosongkan seleksi body setelah operasi destruktif.
//!
//! P0.3: seluruh isi modul ini pindah ke `ducad-engine` (`model` +
//! `profile`); yang tersisa di sini hanya jembatan ke crate render.

pub use ducad_engine::model::*;
pub use ducad_engine::profile::*;

use ducad_kernel::KernelMesh;

/// Pembungkus `Arc<KernelMesh>` yang mengimplementasikan `MeshSource` milik
/// crate render — aturan orphan melarang impl langsung pada `KernelMesh`.
pub struct BodyMeshRef(pub std::sync::Arc<KernelMesh>);

impl ducad_render::MeshSource for BodyMeshRef {
    fn positions(&self) -> &[[f32; 3]] {
        &self.0.positions
    }
    fn normals(&self) -> &[[f32; 3]] {
        &self.0.normals
    }
    fn indices(&self) -> &[u32] {
        &self.0.indices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::plane_frame_from;
    use ducad_sketch::{Entity, Sketch};
    use glam::DVec2;
    use std::collections::HashSet;
    #[test]
    fn test_sweep_multi_plane_profile_top_path_front() {
        let mut top_sketch = Sketch::default();
        let c_id = top_sketch.entities.insert(Entity::circle(
            DVec2::new(0.0, 0.0),
            8.0,
        ));
        let mut prof_sel = HashSet::new();
        prof_sel.insert(c_id);
        let profile = build_profile_from_selection(&top_sketch, &prof_sel).unwrap();
        let top_plane = ducad_render::SketchPlane::top();

        let mut front_sketch = Sketch::default();
        // Path on Front plane (XZ): goes from origin (0, 0) up along Z (0, 50)
        let l_id = front_sketch.entities.insert(Entity::line(
            DVec2::new(0.0, 0.0),
            DVec2::new(0.0, 50.0),
        ));
        let mut path_sel = HashSet::new();
        path_sel.insert(l_id);
        let front_plane = ducad_render::SketchPlane::front();
        let path = build_path_from_selection_on_plane(&front_sketch, &path_sel, &plane_frame_from(&front_plane)).unwrap();

        let origin = [top_plane.origin.x as f64, top_plane.origin.y as f64, top_plane.origin.z as f64];
        let u_axis = [top_plane.u_axis.x as f64, top_plane.u_axis.y as f64, top_plane.u_axis.z as f64];
        let v_axis = [top_plane.v_axis.x as f64, top_plane.v_axis.y as f64, top_plane.v_axis.z as f64];
        let normal = [top_plane.normal.x as f64, top_plane.normal.y as f64, top_plane.normal.z as f64];

        let shape = ducad_kernel::sweep_profile_on_plane_along_path(
            &profile,
            origin,
            u_axis,
            v_axis,
            normal,
            &path,
        )
        .unwrap();

        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_sweep_along_smooth_spline_path() {
        let mut top_sketch = Sketch::default();
        let c_id = top_sketch.entities.insert(Entity::circle(
            DVec2::new(0.0, 0.0),
            5.0,
        ));
        let mut prof_sel = HashSet::new();
        prof_sel.insert(c_id);
        let profile = build_profile_from_selection(&top_sketch, &prof_sel).unwrap();
        let top_plane = ducad_render::SketchPlane::top();

        let mut front_sketch = Sketch::default();
        // Curving S-path on Front Plane (XZ)
        let s_id = front_sketch.entities.insert(Entity::spline(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.0, 30.0),
            DVec2::new(30.0, 60.0),
            DVec2::new(60.0, 60.0),
        ]));
        let mut path_sel = HashSet::new();
        path_sel.insert(s_id);
        let front_plane = ducad_render::SketchPlane::front();
        let path = build_path_from_selection_on_plane(&front_sketch, &path_sel, &plane_frame_from(&front_plane)).unwrap();

        let origin = [top_plane.origin.x as f64, top_plane.origin.y as f64, top_plane.origin.z as f64];
        let u_axis = [top_plane.u_axis.x as f64, top_plane.u_axis.y as f64, top_plane.u_axis.z as f64];
        let v_axis = [top_plane.v_axis.x as f64, top_plane.v_axis.y as f64, top_plane.v_axis.z as f64];
        let normal = [top_plane.normal.x as f64, top_plane.normal.y as f64, top_plane.normal.z as f64];

        let shape = ducad_kernel::sweep_profile_on_plane_along_path(
            &profile,
            origin,
            u_axis,
            v_axis,
            normal,
            &path,
        )
        .unwrap();

        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_extrude_text_with_holes_and_boolean_merge() {
        use ducad_sketch::{text_to_entities, FontPreset, TextAlign, TextOptions};

        let options = TextOptions {
            font_height_mm: 20.0,
            letter_spacing: 1.0,
            line_spacing: 1.2,
            align: TextAlign::Left,
            font_preset: FontPreset::DefaultSans,
            is_construction: false,
        };

        let entities = text_to_entities("DUCAD", DVec2::new(0.0, 0.0), &options, None).unwrap();
        let mut sketch = Sketch::default();
        let mut all_ids = HashSet::new();
        for e in entities {
            all_ids.insert(sketch.entities.insert(e));
        }

        let plane = ducad_render::SketchPlane::top();
        let solids = extrude_selection_with_holes_on_plane(&sketch, &all_ids, &plane_frame_from(&plane), 10.0)
            .expect("Extrude DUCAD dengan lubang & boolean merge harus berhasil");

        assert!(!solids.is_empty(), "Harus menghasilkan minimal 1 bodi solid teks");
        for (name, geo) in &solids {
            assert!(geo.mesh.triangle_count() > 0);
            assert!(name.contains("Teks 3D") || name.contains("Solid"));
        }
    }

    #[test]
    fn test_extrude_circle_smooth_analytic_cylinder() {
        let mut sketch = Sketch::default();
        let c_id = sketch.entities.insert(Entity::circle(DVec2::new(0.0, 0.0), 25.0));
        let mut sel = HashSet::new();
        sel.insert(c_id);

        let plane = ducad_render::SketchPlane::top();
        let solids = extrude_selection_with_holes_on_plane(&sketch, &sel, &plane_frame_from(&plane), 50.0)
            .expect("Extrude circle harus berhasil");

        assert_eq!(solids.len(), 1);
        let (name, geo) = &solids[0];
        assert_eq!(name, "Solid");
        // OpenCASCADE true analytical cylinder has exact faces & non-empty mesh
        assert!(geo.mesh.triangle_count() > 0);
    }

    #[test]
    fn test_extrude_ellipse_smooth_analytic_cylinder() {
        let mut sketch = Sketch::default();
        let e_id = sketch.entities.insert(Entity::ellipse(DVec2::new(0.0, 0.0), 30.0, 15.0));
        let mut sel = HashSet::new();
        sel.insert(e_id);

        let plane = ducad_render::SketchPlane::top();
        let solids = extrude_selection_with_holes_on_plane(&sketch, &sel, &plane_frame_from(&plane), 40.0)
            .expect("Extrude ellipse harus berhasil");

        assert_eq!(solids.len(), 1);
        let (name, geo) = &solids[0];
        assert_eq!(name, "Solid");
        assert!(geo.mesh.triangle_count() > 0);
    }

    #[test]
    fn test_extrude_concentric_circles_pipe_hole() {
        let mut sketch = Sketch::default();
        let outer_id = sketch.entities.insert(Entity::circle(DVec2::new(0.0, 0.0), 30.0));
        let inner_id = sketch.entities.insert(Entity::circle(DVec2::new(0.0, 0.0), 15.0));
        let mut sel = HashSet::new();
        sel.insert(outer_id);
        sel.insert(inner_id);

        let plane = ducad_render::SketchPlane::top();
        let solids = extrude_selection_with_holes_on_plane(&sketch, &sel, &plane_frame_from(&plane), 20.0)
            .expect("Extrude pipe dengan lubang tengah harus berhasil");

        assert_eq!(solids.len(), 1);
        let (name, geo) = &solids[0];
        assert_eq!(name, "Solid");
        assert!(geo.mesh.triangle_count() > 0);
    }
}
