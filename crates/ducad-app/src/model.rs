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

use std::collections::HashSet;

use ducad_core::{BodyId, Command, Document};
use ducad_kernel::{self, KernelMesh, KernelShape, Profile, ProfileSegment};
use ducad_sketch::{Entity, EntityId, Sketch};
use glam::DVec2;
use slotmap::SecondaryMap;

/// Geometri kernel satu body — pasangan shape B-rep + mesh hasil
/// tessellation-nya (mesh di-cache di sini, bukan dihitung ulang tiap
/// frame render). `edge_dims` (fitur "Tampilkan Semua Ukuran", checkbox
/// ruler properties) di-cache dengan pola yang sama: dihitung SEKALI saat
/// geometri body dibuat/berubah, bukan tiap frame render viewport.
pub struct BodyGeometry {
    pub shape: KernelShape,
    pub mesh: KernelMesh,
    pub edge_dims: Vec<ducad_kernel::EdgeDimension>,
}

impl BodyGeometry {
    pub fn from_shape(shape: KernelShape) -> Self {
        let mesh = shape.tessellate();
        Self::from_shape_with_mesh(shape, mesh)
    }

    /// Sama seperti `from_shape`, tapi `mesh` SUDAH dihitung sebelumnya
    /// (dipakai `import_worker` di ducad-app: mesh dihitung di thread
    /// latar belakang supaya UI tidak beku, lalu shape dibangun ulang di
    /// UI thread dari teks STEP — lihat pemanggilnya). `edge_dims` tetap
    /// dihitung di sini karena hanya `shape` yang dikirim balik dari
    /// worker, bukan dimensi rusuknya.
    pub fn from_shape_with_mesh(shape: KernelShape, mesh: KernelMesh) -> Self {
        let edge_dims = ducad_kernel::edge_dimensions(&shape);
        Self { shape, mesh, edge_dims }
    }
}

/// Dokumen 3D lengkap: metadata body (`ducad-core::Document`) + geometri
/// kernel-nya, dikunci `BodyId` yang sama. Lihat catatan modul.
#[derive(Default)]
pub struct ModelDoc {
    pub doc: Document,
    pub geometry: SecondaryMap<BodyId, BodyGeometry>,
}

/// Tambah satu body baru dari geometri yang SUDAH dihitung (dry-run sudah
/// selesai di pemanggil — lihat `apply_constraint` di `ducad-app` untuk
/// pola yang sama: hitung dulu, baru masuk undo stack kalau sukses).
pub struct AddSolidCommand {
    label: String,
    pending: Option<BodyGeometry>,
    id: Option<BodyId>,
}

impl AddSolidCommand {
    pub fn new(label: impl Into<String>, geometry: BodyGeometry) -> Self {
        Self {
            label: label.into(),
            pending: Some(geometry),
            id: None,
        }
    }
}

impl Command<ModelDoc> for AddSolidCommand {
    fn name(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, model: &mut ModelDoc) {
        if let Some(geo) = self.pending.take() {
            let id = model.doc.add_body(self.label.clone());
            model.geometry.insert(id, geo);
            self.id = Some(id);
        }
    }

    fn revert(&mut self, model: &mut ModelDoc) {
        if let Some(id) = self.id.take() {
            model.doc.bodies.remove(id);
            if let Some(geo) = model.geometry.remove(id) {
                self.pending = Some(geo);
            }
        }
    }
}

/// Ganti geometri SATU body yang sudah ada dengan hasil baru (dipakai
/// Fillet/Chamfer semua tepi, Shell/Hollow) — `BodyId` tetap sama, cuma
/// isinya ditukar. `apply`/`revert` sengaja identik: keduanya cuma
/// menukar geometri yang tersimpan di `pending` dengan yang ada di peta
/// (`SecondaryMap::insert` mengembalikan nilai lama), jadi memanggilnya
/// dua kali berturut-turut kembali ke keadaan semula — pas untuk
/// apply/revert/redo yang simetris.
pub struct ReplaceGeometryCommand {
    label: &'static str,
    id: BodyId,
    pending: Option<BodyGeometry>,
}

impl ReplaceGeometryCommand {
    pub fn new(label: &'static str, id: BodyId, new_geometry: BodyGeometry) -> Self {
        Self {
            label,
            id,
            pending: Some(new_geometry),
        }
    }

    fn swap(&mut self, model: &mut ModelDoc) {
        if let Some(incoming) = self.pending.take() {
            if let Some(previous) = model.geometry.insert(self.id, incoming) {
                self.pending = Some(previous);
            }
        }
    }
}

impl Command<ModelDoc> for ReplaceGeometryCommand {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, model: &mut ModelDoc) {
        self.swap(model);
    }
    fn revert(&mut self, model: &mut ModelDoc) {
        self.swap(model);
    }
}

pub enum BooleanKind {
    Union,
    Subtract,
    /// Irisan (cuma volume yang tumpang tindih) — Fase 8, lewat
    /// `ducad_kernel::intersect`.
    Intersect,
}

/// Sebelum di-apply: hasil sudah dihitung (dry-run), siap dipasang.
/// Setelah di-apply: body A & B sudah lenyap dari `ModelDoc`, datanya
/// (untuk revert) disimpan di sini.
enum BooleanState {
    Pending(BodyGeometry),
    Applied {
        result_id: BodyId,
        a_body: ducad_core::Body,
        a_geo: BodyGeometry,
        b_body: ducad_core::Body,
        b_geo: BodyGeometry,
    },
}

/// Union/Subtract/Intersect dua body jadi satu body hasil — A & B dihapus,
/// hasil masuk sebagai body baru.
pub struct BooleanCommand {
    label: &'static str,
    result_name: String,
    a: BodyId,
    b: BodyId,
    state: Option<BooleanState>,
}

impl BooleanCommand {
    /// Hitung hasil boolean SEKARANG (dry-run) — mengembalikan `Err` kalau
    /// salah satu body tak ada geometrinya atau operasi kernel gagal,
    /// tanpa menyentuh `model` sama sekali.
    pub fn try_new(
        model: &ModelDoc,
        kind: BooleanKind,
        label: &'static str,
        result_name: impl Into<String>,
        a: BodyId,
        b: BodyId,
    ) -> Result<Self, String> {
        let geo_a = model.geometry.get(a).ok_or("Body A tidak ditemukan")?;
        let geo_b = model.geometry.get(b).ok_or("Body B tidak ditemukan")?;
        let result_shape = match kind {
            BooleanKind::Union => ducad_kernel::union(&geo_a.shape, &geo_b.shape),
            BooleanKind::Subtract => ducad_kernel::subtract(&geo_a.shape, &geo_b.shape),
            BooleanKind::Intersect => ducad_kernel::intersect(&geo_a.shape, &geo_b.shape),
        }
        .map_err(|e| format!("{label} gagal: {e}"))?;
        Ok(Self {
            label,
            result_name: result_name.into(),
            a,
            b,
            state: Some(BooleanState::Pending(BodyGeometry::from_shape(result_shape))),
        })
    }
}

impl Command<ModelDoc> for BooleanCommand {
    fn name(&self) -> &str {
        self.label
    }

    fn apply(&mut self, model: &mut ModelDoc) {
        let Some(BooleanState::Pending(_)) = &self.state else {
            return;
        };
        let Some(BooleanState::Pending(result_geo)) = self.state.take() else {
            unreachable!()
        };
        let (Some(a_body), Some(a_geo), Some(b_body), Some(b_geo)) = (
            model.doc.bodies.remove(self.a),
            model.geometry.remove(self.a),
            model.doc.bodies.remove(self.b),
            model.geometry.remove(self.b),
        ) else {
            return;
        };
        let result_id = model.doc.add_body(self.result_name.clone());
        model.geometry.insert(result_id, result_geo);
        self.state = Some(BooleanState::Applied {
            result_id,
            a_body,
            a_geo,
            b_body,
            b_geo,
        });
    }

    fn revert(&mut self, model: &mut ModelDoc) {
        let Some(BooleanState::Applied { .. }) = &self.state else {
            return;
        };
        let Some(BooleanState::Applied {
            result_id,
            a_body,
            a_geo,
            b_body,
            b_geo,
        }) = self.state.take()
        else {
            unreachable!()
        };
        model.doc.bodies.remove(result_id);
        let result_geo = model.geometry.remove(result_id);

        // BodyId lama tidak bisa dipakai lagi (konvensi slotmap yang sama
        // dengan `ducad_sketch::DeleteEntities`) — perbarui `self.a`/`b`
        // supaya `apply` (redo) berikutnya menghapus id yang BENAR.
        self.a = model.doc.bodies.insert(a_body);
        model.geometry.insert(self.a, a_geo);
        self.b = model.doc.bodies.insert(b_body);
        model.geometry.insert(self.b, b_geo);

        if let Some(result_geo) = result_geo {
            self.state = Some(BooleanState::Pending(result_geo));
        }
    }
}

/// Hapus satu body (undo-able).
pub struct DeleteBodyCommand {
    id: BodyId,
    stash: Option<(ducad_core::Body, BodyGeometry)>,
}

impl DeleteBodyCommand {
    pub fn new(id: BodyId) -> Self {
        Self { id, stash: None }
    }
}

impl Command<ModelDoc> for DeleteBodyCommand {
    fn name(&self) -> &str {
        "Hapus Body"
    }
    fn apply(&mut self, model: &mut ModelDoc) {
        let (Some(body), Some(geo)) = (model.doc.bodies.remove(self.id), model.geometry.remove(self.id)) else {
            return;
        };
        self.stash = Some((body, geo));
    }
    fn revert(&mut self, model: &mut ModelDoc) {
        if let Some((body, geo)) = self.stash.take() {
            let new_id = model.doc.bodies.insert(body);
            model.geometry.insert(new_id, geo);
            self.id = new_id;
        }
    }
}

/// Titik awal, titik-di-busur (untuk Arc), dan titik akhir dari
/// `Entity::Arc` — konversi CCW yang sama dengan yang dipakai render
/// (`push_arc` di `ducad-render::sketch`): span dinormalisasi ke (0, TAU]
/// dari `start_angle` ke `end_angle` searah CCW.
fn arc_endpoints_and_via(center: DVec2, radius: f64, start_angle: f64, end_angle: f64) -> (DVec2, DVec2, DVec2) {
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
    let pt = |a: f64| center + DVec2::new(radius * a.cos(), radius * a.sin());
    (pt(start_angle), pt(mid_angle), pt(end_angle))
}

fn reverse_segment(seg: ProfileSegment) -> ProfileSegment {
    match seg {
        ProfileSegment::Line { start, end } => ProfileSegment::Line { start: end, end: start },
        ProfileSegment::Arc { start, via, end } => ProfileSegment::Arc {
            start: end,
            via,
            end: start,
        },
    }
}

/// Bangun `Profile` kernel (siap Extrude) dari seleksi entitas sketch.
///
/// Dua kasus didukung:
/// - Seleksi tunggal berisi 1 `Circle` → `Profile::Circle` langsung.
/// - Seleksi berisi ≥3 `Line`/`Arc` yang, kalau dirangkai lewat
///   titik-ujungnya (toleransi 1e-6), membentuk SATU loop tertutup —
///   urutan pemilihan tidak masalah, chain-builder mencari sambungan
///   sendiri lalu membalik arah segmen kalau perlu.
///
/// `Ellipse` dan campuran Circle+entitas lain sengaja ditolak dengan
/// pesan error (bukan didiamkan) — lihat docs/PLAN.md untuk kenapa belum
/// didukung.
pub fn build_profile_from_selection(sketch: &Sketch, ids: &HashSet<EntityId>) -> Result<Profile, String> {
    if ids.is_empty() {
        return Err("Pilih dulu entitas sketch yang membentuk profil tertutup".to_string());
    }

    if ids.len() == 1 {
        let id = *ids.iter().next().unwrap();
        if let Some(Entity::Circle { center, radius }) = sketch.entities.get(id) {
            return Ok(Profile::Circle {
                center: (center.x, center.y),
                radius: *radius,
            });
        }
    }

    struct Seg {
        start: DVec2,
        end: DVec2,
        seg: ProfileSegment,
    }

    let mut segs: Vec<Seg> = Vec::new();
    for id in ids {
        match sketch.entities.get(*id) {
            Some(Entity::Line { start, end }) => segs.push(Seg {
                start: *start,
                end: *end,
                seg: ProfileSegment::Line {
                    start: (start.x, start.y),
                    end: (end.x, end.y),
                },
            }),
            Some(Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            }) => {
                let (s, via, e) = arc_endpoints_and_via(*center, *radius, *start_angle, *end_angle);
                segs.push(Seg {
                    start: s,
                    end: e,
                    seg: ProfileSegment::Arc {
                        start: (s.x, s.y),
                        via: (via.x, via.y),
                        end: (e.x, e.y),
                    },
                });
            }
            Some(Entity::Circle { .. }) => {
                return Err(
                    "Tidak bisa campur Lingkaran dengan entitas lain — pilih Lingkaran sendirian, atau Line/Arc yang membentuk loop tertutup"
                        .to_string(),
                )
            }
            Some(Entity::Ellipse { .. }) => {
                return Err("Ellips belum didukung untuk profil 3D".to_string())
            }
            None => {}
        }
    }

    if segs.len() < 3 {
        return Err("Profil butuh minimal 3 segmen Line/Arc yang membentuk loop tertutup".to_string());
    }

    // Rantai dirangkai dari DUA ujung (append di ekor, prepend di kepala),
    // bukan cuma ekor — segmen pertama yang diambil dari `HashSet` (urutan
    // tak terjamin) bisa saja segmen di TENGAH rantai terbuka; tumbuh
    // sepihak (cuma ekor) gagal mendeteksi ujung yang tak nyambung kalau
    // kebetulan mulai dari tengah (ditemukan lewat test, bukan teori —
    // lihat `build_profile_open_chain_errors`).
    const EPS: f64 = 1e-6;
    let mut remaining = segs;
    let mut ordered = vec![remaining.remove(0)];

    while !remaining.is_empty() {
        let tail = ordered.last().unwrap().end;
        if let Some(i) = remaining.iter().position(|s| (s.start - tail).length() < EPS) {
            ordered.push(remaining.remove(i));
            continue;
        }
        if let Some(i) = remaining.iter().position(|s| (s.end - tail).length() < EPS) {
            let mut s = remaining.remove(i);
            std::mem::swap(&mut s.start, &mut s.end);
            s.seg = reverse_segment(s.seg);
            ordered.push(s);
            continue;
        }
        let head = ordered.first().unwrap().start;
        if let Some(i) = remaining.iter().position(|s| (s.end - head).length() < EPS) {
            ordered.insert(0, remaining.remove(i));
            continue;
        }
        if let Some(i) = remaining.iter().position(|s| (s.start - head).length() < EPS) {
            let mut s = remaining.remove(i);
            std::mem::swap(&mut s.start, &mut s.end);
            s.seg = reverse_segment(s.seg);
            ordered.insert(0, s);
            continue;
        }
        return Err(
            "Entitas terpilih tidak membentuk rantai tersambung — pastikan setiap ujung bertemu ujung entitas lain"
                .to_string(),
        );
    }

    let head = ordered.first().unwrap().start;
    let tail = ordered.last().unwrap().end;
    if (tail - head).length() > EPS {
        return Err("Rantai entitas terpilih tidak tertutup (ujung terakhir tidak kembali ke titik awal)".to_string());
    }

    Ok(Profile::Loop(ordered.into_iter().map(|s| s.seg).collect()))
}

/// Hitung bounding box 2D `[min_x, min_y, max_x, max_y]` dari seleksi entitas sketch.
pub fn compute_profile_bbox(sketch: &Sketch, ids: &HashSet<EntityId>) -> Option<[f64; 4]> {
    if ids.is_empty() {
        return None;
    }
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    let mut count = 0;

    for id in ids {
        if let Some(e) = sketch.entities.get(*id) {
            count += 1;
            match e {
                Entity::Line { start, end } => {
                    min_x = min_x.min(start.x.min(end.x));
                    max_x = max_x.max(start.x.max(end.x));
                    min_y = min_y.min(start.y.min(end.y));
                    max_y = max_y.max(start.y.max(end.y));
                }
                Entity::Circle { center, radius } => {
                    min_x = min_x.min(center.x - radius);
                    max_x = max_x.max(center.x + radius);
                    min_y = min_y.min(center.y - radius);
                    max_y = max_y.max(center.y + radius);
                }
                Entity::Arc { center, radius, .. } => {
                    min_x = min_x.min(center.x - radius);
                    max_x = max_x.max(center.x + radius);
                    min_y = min_y.min(center.y - radius);
                    max_y = max_y.max(center.y + radius);
                }
                Entity::Ellipse {
                    center,
                    radius_x,
                    radius_y,
                } => {
                    min_x = min_x.min(center.x - radius_x);
                    max_x = max_x.max(center.x + radius_x);
                    min_y = min_y.min(center.y - radius_y);
                    max_y = max_y.max(center.y + radius_y);
                }
            }
        }
    }

    if count == 0 {
        None
    } else {
        Some([min_x, min_y, max_x, max_y])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_sketch::Sketch;

    #[test]
    fn build_profile_single_circle() {
        let mut sketch = Sketch::default();
        let id = sketch.entities.insert(Entity::Circle {
            center: DVec2::new(1.0, 2.0),
            radius: 5.0,
        });
        let ids: HashSet<_> = [id].into_iter().collect();
        let profile = build_profile_from_selection(&sketch, &ids).unwrap();
        assert!(matches!(profile, Profile::Circle { radius, .. } if radius == 5.0));
    }

    #[test]
    fn build_profile_rectangle_any_order() {
        let mut sketch = Sketch::default();
        let corners = [
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 5.0),
            DVec2::new(0.0, 5.0),
        ];
        // Sisipkan sisi dalam urutan yang SENGAJA diacak & sebagian
        // dibalik arahnya, supaya chain-builder benar-benar diuji.
        let mut ids = HashSet::new();
        ids.insert(sketch.entities.insert(Entity::Line {
            start: corners[2],
            end: corners[1],
        }));
        ids.insert(sketch.entities.insert(Entity::Line {
            start: corners[0],
            end: corners[1],
        }));
        ids.insert(sketch.entities.insert(Entity::Line {
            start: corners[3],
            end: corners[0],
        }));
        ids.insert(sketch.entities.insert(Entity::Line {
            start: corners[2],
            end: corners[3],
        }));

        let profile = build_profile_from_selection(&sketch, &ids).unwrap();
        match profile {
            Profile::Loop(segs) => assert_eq!(segs.len(), 4),
            _ => panic!("expected Loop"),
        }
    }

    #[test]
    fn build_profile_open_chain_errors() {
        let mut sketch = Sketch::default();
        let mut ids = HashSet::new();
        ids.insert(sketch.entities.insert(Entity::Line {
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(10.0, 0.0),
        }));
        ids.insert(sketch.entities.insert(Entity::Line {
            start: DVec2::new(10.0, 0.0),
            end: DVec2::new(10.0, 5.0),
        }));
        ids.insert(sketch.entities.insert(Entity::Line {
            start: DVec2::new(10.0, 5.0),
            end: DVec2::new(0.0, 5.0),
        }));
        // Sengaja tidak ditutup (tidak ada segmen balik ke (0,0)).
        let err = build_profile_from_selection(&sketch, &ids).unwrap_err();
        assert!(err.contains("tertutup"));
    }

    #[test]
    fn build_profile_empty_selection_errors() {
        let sketch = Sketch::default();
        assert!(build_profile_from_selection(&sketch, &HashSet::new()).is_err());
    }

    #[test]
    fn compute_profile_bbox_rect_and_circle() {
        let mut sketch = Sketch::default();
        let id1 = sketch.entities.insert(Entity::Line {
            start: DVec2::new(5.0, 10.0),
            end: DVec2::new(15.0, 20.0),
        });
        let id2 = sketch.entities.insert(Entity::Circle {
            center: DVec2::new(0.0, 0.0),
            radius: 3.0,
        });
        let ids: HashSet<_> = [id1, id2].into_iter().collect();
        let bbox = compute_profile_bbox(&sketch, &ids).unwrap();
        assert_eq!(bbox, [-3.0, -3.0, 15.0, 20.0]);
    }

    #[test]
    fn loft_two_closed_regions_profiles_build_correctly() {
        let mut sketch = Sketch::default();
        // Region 1: Rectangle 40x40 at (0, 0)
        let min = DVec2::new(-20.0, -20.0);
        let max = DVec2::new(20.0, 20.0);
        let corners = [
            DVec2::new(min.x, min.y),
            DVec2::new(max.x, min.y),
            DVec2::new(max.x, max.y),
            DVec2::new(min.x, max.y),
        ];
        let mut r1_ids = HashSet::new();
        for i in 0..4 {
            let id = sketch.entities.insert(Entity::Line {
                start: corners[i],
                end: corners[(i + 1) % 4],
            });
            r1_ids.insert(id);
        }

        // Region 2: Circle radius 10 at (50, 0)
        let mut r2_ids = HashSet::new();
        let c_id = sketch.entities.insert(Entity::Circle {
            center: DVec2::new(50.0, 0.0),
            radius: 10.0,
        });
        r2_ids.insert(c_id);

        let p1 = build_profile_from_selection(&sketch, &r1_ids).unwrap();
        let p2 = build_profile_from_selection(&sketch, &r2_ids).unwrap();

        let shape = ducad_kernel::loft_profiles(&p1, &p2, 30.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }
}
