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

/// Tambah beberapa body baru sekaligus dalam 1 langkah undo/redo (dipakai oleh Pattern 3D).
pub struct AddMultipleSolidsCommand {
    label: String,
    pending: Option<Vec<(String, BodyGeometry)>>,
    ids: Option<Vec<BodyId>>,
}

impl AddMultipleSolidsCommand {
    pub fn new(label: impl Into<String>, bodies: Vec<(String, BodyGeometry)>) -> Self {
        Self {
            label: label.into(),
            pending: Some(bodies),
            ids: None,
        }
    }

    pub fn created_ids(&self) -> &[BodyId] {
        self.ids.as_deref().unwrap_or(&[])
    }
}

impl Command<ModelDoc> for AddMultipleSolidsCommand {
    fn name(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, model: &mut ModelDoc) {
        if let Some(items) = self.pending.take() {
            let mut created = Vec::with_capacity(items.len());
            for (name, geo) in items {
                let id = model.doc.add_body(name);
                model.geometry.insert(id, geo);
                created.push(id);
            }
            self.ids = Some(created);
        }
    }

    fn revert(&mut self, model: &mut ModelDoc) {
        if let Some(ids) = self.ids.take() {
            let mut pending = Vec::with_capacity(ids.len());
            for id in ids {
                if let Some(body) = model.doc.bodies.remove(id) {
                    if let Some(geo) = model.geometry.remove(id) {
                        pending.push((body.name, geo));
                    }
                }
            }
            self.pending = Some(pending);
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

enum SplitBodyState {
    Pending(Vec<(String, BodyGeometry)>),
    Applied {
        orig_body: ducad_core::Body,
        orig_geo: BodyGeometry,
        result_ids: Vec<BodyId>,
        result_names: Vec<String>,
    },
}

/// Split satu body menjadi N body terpisah (biasanya 2 body).
/// Body sumber dihapus, dan N body hasil ditambahkan ke `ModelDoc`.
/// Mendukung penuh Undo / Redo.
pub struct SplitBodyCommand {
    label: &'static str,
    target_id: BodyId,
    state: Option<SplitBodyState>,
}

impl SplitBodyCommand {
    pub fn new(
        target_id: BodyId,
        result_bodies: Vec<(String, BodyGeometry)>,
    ) -> Self {
        Self {
            label: "Split Body",
            target_id,
            state: Some(SplitBodyState::Pending(result_bodies)),
        }
    }

    /// ID body hasil yang baru saja dibuat oleh command ini.
    pub fn result_ids(&self) -> &[BodyId] {
        if let Some(SplitBodyState::Applied { result_ids, .. }) = &self.state {
            result_ids
        } else {
            &[]
        }
    }
}

impl Command<ModelDoc> for SplitBodyCommand {
    fn name(&self) -> &str {
        self.label
    }

    fn apply(&mut self, model: &mut ModelDoc) {
        let Some(SplitBodyState::Pending(_)) = &self.state else {
            return;
        };
        let Some(SplitBodyState::Pending(new_bodies)) = self.state.take() else {
            unreachable!()
        };
        let (Some(orig_body), Some(orig_geo)) = (
            model.doc.bodies.remove(self.target_id),
            model.geometry.remove(self.target_id),
        ) else {
            return;
        };

        let mut result_ids = Vec::with_capacity(new_bodies.len());
        let mut result_names = Vec::with_capacity(new_bodies.len());

        for (name, geo) in new_bodies {
            let id = model.doc.add_body(name.clone());
            model.geometry.insert(id, geo);
            result_ids.push(id);
            result_names.push(name);
        }

        self.state = Some(SplitBodyState::Applied {
            orig_body,
            orig_geo,
            result_ids,
            result_names,
        });
    }

    fn revert(&mut self, model: &mut ModelDoc) {
        let Some(SplitBodyState::Applied { .. }) = &self.state else {
            return;
        };
        let Some(SplitBodyState::Applied {
            orig_body,
            orig_geo,
            result_ids,
            result_names,
        }) = self.state.take()
        else {
            unreachable!()
        };

        let mut pending = Vec::with_capacity(result_ids.len());
        for (id, name) in result_ids.into_iter().zip(result_names.into_iter()) {
            model.doc.bodies.remove(id);
            if let Some(geo) = model.geometry.remove(id) {
                pending.push((name, geo));
            }
        }

        // Kembalikan body awal
        self.target_id = model.doc.bodies.insert(orig_body);
        model.geometry.insert(self.target_id, orig_geo);

        self.state = Some(SplitBodyState::Pending(pending));
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

/// Konversi Spline (Catmull-Rom) menjadi kurva-kurva Arc parametrik analitik halus (Bi-Arc / 3-point Arcs)
/// agar saat diextrude oleh OpenCASCADE menghasilkan permukaan silinder B-Rep yang kontinu dan mulus
/// (bukan jajaran prisma segi banyak terpatah-patah).
pub fn convert_spline_to_smooth_segments(points: &[DVec2]) -> Vec<(DVec2, DVec2, ProfileSegment)> {
    if points.len() < 2 {
        return Vec::new();
    }
    let mut result = Vec::new();
    let n = points.len();

    let get_pt = |idx: isize| -> DVec2 {
        if idx < 0 {
            points[0]
        } else if idx >= n as isize {
            points[n - 1]
        } else {
            points[idx as usize]
        }
    };

    let eval_cr = |p0: DVec2, p1: DVec2, p2: DVec2, p3: DVec2, t: f64| -> DVec2 {
        let t2 = t * t;
        let t3 = t2 * t;
        0.5 * ((2.0 * p1)
            + (-p0 + p2) * t
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
            + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
    };

    for i in 0..n - 1 {
        let p0 = get_pt(i as isize - 1);
        let p1 = get_pt(i as isize);
        let p2 = get_pt(i as isize + 1);
        let p3 = get_pt(i as isize + 2);

        // 2 sub-arcs per rentang fit point untuk aproksimasi kurvatur sangat tinggi dan mulus
        let t_splits = [(0.0, 0.25, 0.5), (0.5, 0.75, 1.0)];
        for (t_start, t_mid, t_end) in t_splits {
            let start = eval_cr(p0, p1, p2, p3, t_start);
            let via = eval_cr(p0, p1, p2, p3, t_mid);
            let end = eval_cr(p0, p1, p2, p3, t_end);

            if (start - end).length() < 1e-5 {
                continue;
            }

            // Cek kelurusan / kolinearitas
            let cross = (via.x - start.x) * (end.y - start.y) - (via.y - start.y) * (end.x - start.x);
            let seg = if cross.abs() < 1e-4 {
                ProfileSegment::Line {
                    start: (start.x, start.y),
                    end: (end.x, end.y),
                }
            } else {
                ProfileSegment::Arc {
                    start: (start.x, start.y),
                    via: (via.x, via.y),
                    end: (end.x, end.y),
                }
            };
            result.push((start, end, seg));
        }
    }

    result
}

/// Bangun `Profile` kernel (siap Extrude) dari seleksi entitas sketch.
///
/// Kasus didukung:
/// - Seleksi tunggal berisi 1 `Circle` → `Profile::Circle` langsung.
/// - Seleksi tunggal berisi 1 `Ellipse` → `Profile::Ellipse` langsung.
/// - Seleksi tunggal berisi 1 `Spline` tertutup mandiri → `Profile::Loop` (smooth arcs).
/// - Seleksi berisi rantai `Line`/`Arc`/`Spline` yang membentuk loop tertutup.
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
        if let Some(Entity::Ellipse {
            center,
            radius_x,
            radius_y,
        }) = sketch.entities.get(id)
        {
            if *radius_x <= 0.0 || *radius_y <= 0.0 {
                return Err("Radius ellips harus bernilai positif".to_string());
            }
            return Ok(Profile::Ellipse {
                center: (center.x, center.y),
                radius_x: *radius_x,
                radius_y: *radius_y,
            });
        }
        if let Some(Entity::Spline { points }) = sketch.entities.get(id) {
            if points.len() >= 3 {
                let first = points[0];
                let last = *points.last().unwrap();
                if (first - last).length() < 0.05 {
                    let smooth_segs: Vec<ProfileSegment> =
                        convert_spline_to_smooth_segments(points)
                            .into_iter()
                            .map(|(_, _, s)| s)
                            .collect();
                    if !smooth_segs.is_empty() {
                        return Ok(Profile::Loop(smooth_segs));
                    }
                }
            }
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
            Some(Entity::Spline { points }) => {
                for (start, end, seg) in convert_spline_to_smooth_segments(points) {
                    segs.push(Seg { start, end, seg });
                }
            }
            Some(Entity::Circle { .. }) => {
                return Err(
                    "Tidak bisa campur Lingkaran dengan entitas lain — pilih Lingkaran sendirian, atau Line/Arc/Spline yang membentuk loop tertutup"
                        .to_string(),
                )
            }
            Some(Entity::Ellipse { .. }) => {
                return Err(
                    "Tidak bisa campur Ellips dengan entitas lain — pilih Ellips sendirian, atau Line/Arc/Spline yang membentuk loop tertutup"
                        .to_string(),
                )
            }
            None => {}
        }
    }

    if segs.len() < 2 {
        return Err("Profil butuh minimal 2 segmen Line/Arc/Spline yang membentuk loop tertutup".to_string());
    }

    // Rantai dirangkai dari DUA ujung (append di ekor, prepend di kepala),
    // bukan cuma ekor — segmen pertama yang diambil dari `HashSet` (urutan
    // tak terjamin) bisa saja segmen di TENGAH rantai terbuka; tumbuh
    // sepihak (cuma ekor) gagal mendeteksi ujung yang tak nyambung kalau
    // kebetulan mulai dari tengah (ditemukan lewat test, bukan teori —
    // lihat `build_profile_open_chain_errors`).
    const EPS: f64 = 0.05;
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

/// Bangun kurva jalur (spine path) untuk Sweep dari seleksi entitas sketch (Line, Arc, Spline, Circle)
/// dengan transformasi ke koordinat 3D dunia berdasarkan `SketchPlane`.
/// Spline Catmull-Rom dipecah menjadi busur-busur analitik kontinu tangensial (smooth arcs)
/// agar tidak terbentuk patahan/miter tajam saat disapu oleh OpenCASCADE.
pub fn build_path_from_selection_on_plane(
    sketch: &Sketch,
    ids: &HashSet<EntityId>,
    plane: &ducad_render::SketchPlane,
) -> Result<Vec<ducad_kernel::PathSegment>, String> {
    if ids.is_empty() {
        return Err("Pilih dulu entitas garis, busur, atau spline sebagai jalur sweep".to_string());
    }

    // Kasus khusus 1 Circle penuh sebagai jalur
    if ids.len() == 1 {
        let id = *ids.iter().next().unwrap();
        if let Some(Entity::Circle { center, radius }) = sketch.entities.get(id) {
            let (cx, cy, r) = (center.x, center.y, *radius);
            let p1 = plane.to_world_f64((cx + r, cy), 0.0);
            let p2 = plane.to_world_f64((cx, cy + r), 0.0);
            let p3 = plane.to_world_f64((cx - r, cy), 0.0);
            let p4 = plane.to_world_f64((cx, cy - r), 0.0);
            return Ok(vec![
                ducad_kernel::PathSegment::Arc {
                    start: p1,
                    via: p2,
                    end: p3,
                },
                ducad_kernel::PathSegment::Arc {
                    start: p3,
                    via: p4,
                    end: p1,
                },
            ]);
        }
    }

    struct PathSeg2D {
        start: DVec2,
        end: DVec2,
        seg: ProfileSegment,
    }

    let mut segs: Vec<PathSeg2D> = Vec::new();
    for id in ids {
        match sketch.entities.get(*id) {
            Some(Entity::Line { start, end }) => segs.push(PathSeg2D {
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
                segs.push(PathSeg2D {
                    start: s,
                    end: e,
                    seg: ProfileSegment::Arc {
                        start: (s.x, s.y),
                        via: (via.x, via.y),
                        end: (e.x, e.y),
                    },
                });
            }
            Some(Entity::Spline { points }) => {
                for (start, end, seg) in convert_spline_to_smooth_segments(points) {
                    segs.push(PathSeg2D { start, end, seg });
                }
            }
            Some(Entity::Circle { center, radius }) => {
                let (cx, cy, r) = (center.x, center.y, *radius);
                let p1 = DVec2::new(cx + r, cy);
                let p2 = DVec2::new(cx, cy + r);
                let p3 = DVec2::new(cx - r, cy);
                let p4 = DVec2::new(cx, cy - r);
                segs.push(PathSeg2D {
                    start: p1,
                    end: p3,
                    seg: ProfileSegment::Arc {
                        start: (p1.x, p1.y),
                        via: (p2.x, p2.y),
                        end: (p3.x, p3.y),
                    },
                });
                segs.push(PathSeg2D {
                    start: p3,
                    end: p1,
                    seg: ProfileSegment::Arc {
                        start: (p3.x, p3.y),
                        via: (p4.x, p4.y),
                        end: (p1.x, p1.y),
                    },
                });
            }
            Some(Entity::Ellipse { .. }) | None => {}
        }
    }

    if segs.is_empty() {
        return Err("Tidak ada kurva jalur yang valid dari seleksi".to_string());
    }

    // Urutkan dan rangkai segmen (head-to-tail chaining)
    const EPS: f64 = 0.05;
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
        break;
    }

    // Konversi ordered segments ke 3D PathSegment
    let mut path_3d = Vec::new();
    for s in ordered {
        match s.seg {
            ProfileSegment::Line { start, end } => {
                path_3d.push(ducad_kernel::PathSegment::Line {
                    start: plane.to_world_f64(start, 0.0),
                    end: plane.to_world_f64(end, 0.0),
                });
            }
            ProfileSegment::Arc { start, via, end } => {
                path_3d.push(ducad_kernel::PathSegment::Arc {
                    start: plane.to_world_f64(start, 0.0),
                    via: plane.to_world_f64(via, 0.0),
                    end: plane.to_world_f64(end, 0.0),
                });
            }
        }
    }

    Ok(path_3d)
}

/// Fallback versi planar XY untuk kompatibilitas fungsi lama / test sederhana.
pub fn build_path_from_selection(
    sketch: &Sketch,
    ids: &HashSet<EntityId>,
) -> Result<Vec<ducad_kernel::PathSegment>, String> {
    build_path_from_selection_on_plane(sketch, ids, &ducad_render::SketchPlane::top())
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
                Entity::Spline { points } => {
                    for p in points {
                        min_x = min_x.min(p.x);
                        max_x = max_x.max(p.x);
                        min_y = min_y.min(p.y);
                        max_y = max_y.max(p.y);
                    }
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

    #[test]
    fn loft_ellipse_profile_build_and_loft_correctly() {
        let mut sketch = Sketch::default();
        // Region 1: Ellipse at (0, 0)
        let mut r1_ids = HashSet::new();
        let e_id = sketch.entities.insert(Entity::Ellipse {
            center: DVec2::new(0.0, 0.0),
            radius_x: 25.0,
            radius_y: 15.0,
        });
        r1_ids.insert(e_id);

        // Region 2: Circle at (0, 0)
        let mut r2_ids = HashSet::new();
        let c_id = sketch.entities.insert(Entity::Circle {
            center: DVec2::new(0.0, 0.0),
            radius: 10.0,
        });
        r2_ids.insert(c_id);

        let p1 = build_profile_from_selection(&sketch, &r1_ids).unwrap();
        let p2 = build_profile_from_selection(&sketch, &r2_ids).unwrap();

        let shape = ducad_kernel::loft_profiles(&p1, &p2, 20.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn extrude_ellipse_horizontal_and_vertical() {
        let mut sketch = Sketch::default();
        let e1 = sketch.entities.insert(Entity::Ellipse {
            center: DVec2::new(0.0, 0.0),
            radius_x: 30.0,
            radius_y: 10.0,
        });
        let mut set1 = HashSet::new();
        set1.insert(e1);
        let p1 = build_profile_from_selection(&sketch, &set1).unwrap();
        let shape1 = ducad_kernel::extrude_profile(&p1, 25.0).unwrap();
        let mesh1 = shape1.tessellate();
        assert!(mesh1.triangle_count() > 0);

        let e2 = sketch.entities.insert(Entity::Ellipse {
            center: DVec2::new(50.0, 0.0),
            radius_x: 10.0,
            radius_y: 30.0,
        });
        let mut set2 = HashSet::new();
        set2.insert(e2);
        let p2 = build_profile_from_selection(&sketch, &set2).unwrap();
        let shape2 = ducad_kernel::extrude_profile(&p2, 25.0).unwrap();
        let mesh2 = shape2.tessellate();
        assert!(mesh2.triangle_count() > 0);
    }

    #[test]
    fn test_extrude_spline_and_arc_profile() {
        let mut sketch = Sketch::default();
        // Spline from (20, 0) to (-20, 0) bending upwards
        let spline_id = sketch.entities.insert(Entity::Spline {
            points: vec![
                DVec2::new(20.0, 0.0),
                DVec2::new(10.0, 15.0),
                DVec2::new(-10.0, 15.0),
                DVec2::new(-20.0, 0.0),
            ],
        });
        // Arc (semicircle) below X axis from (-20, 0) to (20, 0), center at (0, 0), radius 20
        // angle PI (left, (-20,0)) to 0 (right, (20,0)) passing through (0, -20)
        let arc_id = sketch.entities.insert(Entity::Arc {
            center: DVec2::new(0.0, 0.0),
            radius: 20.0,
            start_angle: std::f64::consts::PI,
            end_angle: std::f64::consts::TAU,
        });

        let mut sel = HashSet::new();
        sel.insert(spline_id);
        sel.insert(arc_id);

        let profile = build_profile_from_selection(&sketch, &sel).unwrap();
        let shape = ducad_kernel::extrude_profile(&profile, 20.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_extrude_self_closed_spline() {
        let mut sketch = Sketch::default();
        let spline_id = sketch.entities.insert(Entity::Spline {
            points: vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(10.0, 15.0),
                DVec2::new(20.0, 0.0),
                DVec2::new(10.0, -15.0),
                DVec2::new(0.0, 0.0),
            ],
        });
        let mut sel = HashSet::new();
        sel.insert(spline_id);

        let profile = build_profile_from_selection(&sketch, &sel).unwrap();
        let shape = ducad_kernel::extrude_profile(&profile, 15.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_sweep_from_selection() {
        let mut sketch = Sketch::default();
        let c_id = sketch.entities.insert(Entity::Circle {
            center: DVec2::new(0.0, 0.0),
            radius: 5.0,
        });
        let mut prof_sel = HashSet::new();
        prof_sel.insert(c_id);
        let profile = build_profile_from_selection(&sketch, &prof_sel).unwrap();

        let l_id = sketch.entities.insert(Entity::Line {
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(0.0, 40.0),
        });
        let mut path_sel = HashSet::new();
        path_sel.insert(l_id);
        let path = build_path_from_selection(&sketch, &path_sel).unwrap();

        let shape = ducad_kernel::sweep_profile_along_path(&profile, &path).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_sweep_multi_plane_profile_top_path_front() {
        let mut top_sketch = Sketch::default();
        let c_id = top_sketch.entities.insert(Entity::Circle {
            center: DVec2::new(0.0, 0.0),
            radius: 8.0,
        });
        let mut prof_sel = HashSet::new();
        prof_sel.insert(c_id);
        let profile = build_profile_from_selection(&top_sketch, &prof_sel).unwrap();
        let top_plane = ducad_render::SketchPlane::top();

        let mut front_sketch = Sketch::default();
        // Path on Front plane (XZ): goes from origin (0, 0) up along Z (0, 50)
        let l_id = front_sketch.entities.insert(Entity::Line {
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(0.0, 50.0),
        });
        let mut path_sel = HashSet::new();
        path_sel.insert(l_id);
        let front_plane = ducad_render::SketchPlane::front();
        let path = build_path_from_selection_on_plane(&front_sketch, &path_sel, &front_plane).unwrap();

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
        let c_id = top_sketch.entities.insert(Entity::Circle {
            center: DVec2::new(0.0, 0.0),
            radius: 5.0,
        });
        let mut prof_sel = HashSet::new();
        prof_sel.insert(c_id);
        let profile = build_profile_from_selection(&top_sketch, &prof_sel).unwrap();
        let top_plane = ducad_render::SketchPlane::top();

        let mut front_sketch = Sketch::default();
        // Curving S-path on Front Plane (XZ)
        let s_id = front_sketch.entities.insert(Entity::Spline {
            points: vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(0.0, 30.0),
                DVec2::new(30.0, 60.0),
                DVec2::new(60.0, 60.0),
            ],
        });
        let mut path_sel = HashSet::new();
        path_sel.insert(s_id);
        let front_plane = ducad_render::SketchPlane::front();
        let path = build_path_from_selection_on_plane(&front_sketch, &path_sel, &front_plane).unwrap();

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
    fn test_split_body_command_undo_redo() {
        let mut model = ModelDoc::default();
        let circle_profile = Profile::Circle { center: (0.0, 0.0), radius: 10.0 };
        let shape = ducad_kernel::extrude_profile(&circle_profile, 40.0).unwrap();
        let initial_id = model.doc.add_body("Original Cylinder");
        model.geometry.insert(initial_id, BodyGeometry::from_shape(shape));

        assert_eq!(model.doc.bodies.len(), 1);

        // Split via kernel
        let orig_shape = &model.geometry.get(initial_id).unwrap().shape;
        let mut parts = ducad_kernel::split_body(
            orig_shape,
            glam::DVec3::new(0.0, 0.0, 20.0),
            glam::DVec3::new(0.0, 0.0, 1.0),
        )
        .unwrap();
        assert_eq!(parts.len(), 2);

        let p2 = parts.pop().unwrap();
        let p1 = parts.pop().unwrap();
        let result_bodies = vec![
            ("Original Cylinder (Bagian 1)".to_string(), BodyGeometry::from_shape(p1)),
            ("Original Cylinder (Bagian 2)".to_string(), BodyGeometry::from_shape(p2)),
        ];

        let mut cmd = SplitBodyCommand::new(initial_id, result_bodies);
        cmd.apply(&mut model);

        assert_eq!(model.doc.bodies.len(), 2);
        assert_eq!(model.geometry.len(), 2);

        // Revert (Undo)
        cmd.revert(&mut model);
        assert_eq!(model.doc.bodies.len(), 1);
        assert_eq!(model.geometry.len(), 1);

        // Re-apply (Redo)
        cmd.apply(&mut model);
        assert_eq!(model.doc.bodies.len(), 2);
        assert_eq!(model.geometry.len(), 2);
    }

    #[test]
    fn test_add_multiple_solids_command_undo_redo() {
        let mut model = ModelDoc::default();
        let circle_profile = Profile::Circle { center: (0.0, 0.0), radius: 5.0 };
        let shape1 = ducad_kernel::extrude_profile(&circle_profile, 10.0).unwrap();
        let shape2 = ducad_kernel::extrude_profile(&circle_profile, 20.0).unwrap();

        let items = vec![
            ("Solid 1".to_string(), BodyGeometry::from_shape(shape1)),
            ("Solid 2".to_string(), BodyGeometry::from_shape(shape2)),
        ];

        let mut cmd = AddMultipleSolidsCommand::new("Pattern 3D", items);
        cmd.apply(&mut model);

        assert_eq!(model.doc.bodies.len(), 2);
        assert_eq!(model.geometry.len(), 2);
        assert_eq!(cmd.created_ids().len(), 2);

        // Revert (Undo)
        cmd.revert(&mut model);
        assert_eq!(model.doc.bodies.len(), 0);
        assert_eq!(model.geometry.len(), 0);

        // Re-apply (Redo)
        cmd.apply(&mut model);
        assert_eq!(model.doc.bodies.len(), 2);
        assert_eq!(model.geometry.len(), 2);
    }
}

