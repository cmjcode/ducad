//! Alat perakitan Fase 4 (PLAN_PARITAS_SOLIDCRAFT §4) di GUI:
//! - simpan/muat perakitan di `.ducad` (body dirujuk lewat UUID);
//! - joint Rigid/Revolute/Slider/Cylindrical/Planar/Ball/Pin-slot/Cam (4.1–4.2),
//!   termasuk joint as-built (4.3) dan asal joint dari datum sumbu (4.4);
//! - ganti nama & aktifkan komponen (4.5), komponen dari body (4.6);
//! - capture/revert posisi (4.7), contact set (4.8), salin occurrence (4.10).

use std::collections::HashMap;

use ducad_core::assembly::{AssemblyInstanceId, JointKind, MateTarget, MateTargetKind};
use ducad_core::joint::{JointFrame, JointState, Pose};
use ducad_core::BodyId;
use glam::DVec3;
use slotmap::Key;

use crate::app::DuCADApp;
use crate::model::BodyGeometry;

/// Pose lokal instance: (translasi, quaternion xyzw).
pub type LocalPose = ((f64, f64, f64), (f64, f64, f64, f64));

fn v3(t: (f64, f64, f64)) -> DVec3 {
    DVec3::new(t.0, t.1, t.2)
}

/// Target mate dari face yang diklik (silinder → sumbu, lainnya → bidang).
pub fn mate_target_from_hit(hit: &ducad_kernel::FaceHit) -> MateTargetKind {
    match (hit.surface_kind, hit.radial_axis) {
        (ducad_kernel::SurfaceKind::Cylinder, Some(ax)) if v3(ax.dir).length_squared() > 1e-12 => {
            MateTargetKind::CylinderAxis {
                origin: ax.origin,
                direction: ax.dir,
                radius: hit.surface_radius.unwrap_or(5.0),
            }
        }
        (ducad_kernel::SurfaceKind::Cylinder, _) => MateTargetKind::CylinderAxis {
            origin: hit.centroid,
            direction: hit.pull_dir,
            radius: hit.surface_radius.unwrap_or(5.0),
        },
        _ => MateTargetKind::PlanarFace {
            origin: hit.centroid,
            normal: hit.normal,
        },
    }
}

/// (asal, arah satuan) target mate.
fn target_axis(t: &MateTargetKind) -> (DVec3, DVec3) {
    match t {
        MateTargetKind::CylinderAxis {
            origin, direction, ..
        } => (v3(*origin), v3(*direction).normalize_or_zero()),
        MateTargetKind::PlanarFace { origin, normal } => {
            (v3(*origin), v3(*normal).normalize_or_zero())
        }
        MateTargetKind::Point { pos } => (v3(*pos), DVec3::Z),
    }
}

/// Bawa titik+arah yang direkam saat pose `from` ke pose `to` instance.
fn carry(o: DVec3, d: DVec3, from: Pose, to: Pose) -> (DVec3, DVec3) {
    let dq = to.1 * from.1.inverse();
    (to.0 + dq * (o - from.0), dq * d)
}

/// Status alat perakitan Fase 4.
pub struct AssemblyToolsState {
    pub open: bool,
    /// Komponen aktif (Paritas 4.5): hanya body komponen ini yang bisa dipilih.
    pub active_instance: Option<AssemblyInstanceId>,
    pub rename_text: String,
    pub component_name: String,
    pub joint_kind: JointKind,
    pub as_built: bool,
    /// Datum sumbu sebagai asal joint (Paritas 4.4).
    pub origin_axis: Option<u32>,
    pub occurrence_offset: [f64; 3],
}

impl Default for AssemblyToolsState {
    fn default() -> Self {
        Self {
            open: false,
            active_instance: None,
            rename_text: String::new(),
            component_name: String::new(),
            joint_kind: JointKind::Revolute,
            as_built: false,
            origin_axis: None,
            occurrence_offset: [20.0, 0.0, 0.0],
        }
    }
}

impl DuCADApp {
    // ------------------------------------------------------------------
    // Persistensi.
    // ------------------------------------------------------------------

    /// Perakitan sebagai JSON (instance merujuk UUID body). `None` bila kosong.
    pub fn assembly_to_json(&self) -> Option<serde_json::Value> {
        let tree = &self.assembly_tree;
        if tree.instances.is_empty() && tree.mates.is_empty() {
            return None;
        }
        let uuid_of: HashMap<u64, String> = self
            .model
            .doc
            .bodies
            .iter()
            .map(|(id, b)| (id.data().as_ffi(), b.uuid.clone()))
            .collect();
        let body_uuids: HashMap<String, String> = tree
            .instances
            .values()
            .filter_map(|i| {
                uuid_of
                    .get(&i.body_id_raw)
                    .filter(|u| !u.is_empty())
                    .map(|u| (i.id.to_string(), u.clone()))
            })
            .collect();
        Some(serde_json::json!({
            "tree": serde_json::to_value(tree).ok()?,
            "body_uuids": body_uuids,
        }))
    }

    /// Pasang perakitan dari JSON berkas (setelah body dimuat). Instance
    /// yang body-nya tidak ada lagi dibuang.
    pub fn assembly_from_json(&mut self, json: Option<serde_json::Value>) {
        self.assembly_tools.active_instance = None;
        let Some(json) = json else {
            self.assembly_tree = Default::default();
            return;
        };
        let Ok(mut tree) =
            serde_json::from_value::<ducad_core::assembly::AssemblyTree>(json["tree"].clone())
        else {
            log::warn!("perakitan di berkas rusak; diabaikan");
            self.assembly_tree = Default::default();
            return;
        };
        let raw_of: HashMap<String, u64> = self
            .model
            .doc
            .bodies
            .iter()
            .filter(|(_, b)| !b.uuid.is_empty())
            .map(|(id, b)| (b.uuid.clone(), id.data().as_ffi()))
            .collect();
        let uuids = json["body_uuids"].as_object().cloned().unwrap_or_default();
        let mut gone = Vec::new();
        for inst in tree.instances.values_mut() {
            match uuids
                .get(&inst.id.to_string())
                .and_then(|u| u.as_str())
                .and_then(|u| raw_of.get(u))
            {
                Some(raw) => inst.body_id_raw = *raw,
                None => gone.push(inst.id),
            }
        }
        for id in gone {
            tree.remove_instance(id);
            tree.mates
                .retain(|_, m| m.target_a.instance_id != id && m.target_b.instance_id != id);
        }
        self.assembly_tree = tree;
    }

    // ------------------------------------------------------------------
    // Pose & geometri.
    // ------------------------------------------------------------------

    fn body_of_raw(&self, raw: u64) -> Option<BodyId> {
        self.model
            .doc
            .bodies
            .iter()
            .find(|(id, _)| id.data().as_ffi() == raw)
            .map(|(id, _)| id)
    }

    pub(crate) fn instance_body(&self, id: AssemblyInstanceId) -> Option<BodyId> {
        let raw = self.assembly_tree.instances.get(&id)?.body_id_raw;
        self.body_of_raw(raw)
    }

    /// Pose lokal semua instance.
    pub fn instance_poses(&self) -> HashMap<AssemblyInstanceId, LocalPose> {
        self.assembly_tree
            .instances
            .iter()
            .map(|(id, i)| (*id, (i.translation, i.rotation_quat)))
            .collect()
    }

    /// Geser geometri body instance dari pose dunia `from` ke `to`
    /// (konvensi sama dengan solver: putar di sekitar `from.t`, lalu geser).
    pub(crate) fn move_instance_geometry(&mut self, id: AssemblyInstanceId, from: Pose, to: Pose) {
        let Some(body) = self.instance_body(id) else {
            return;
        };
        let dq = to.1 * from.1.inverse();
        let (axis, angle) = dq.to_axis_angle();
        let dt = to.0 - from.0;
        if dt.length() < 1e-12 && angle.abs() < 1e-12 {
            return;
        }
        let tf = ducad_kernel::MateTransformResult {
            translation: (dt.x, dt.y, dt.z),
            pivot: (from.0.x, from.0.y, from.0.z),
            axis: (axis.x, axis.y, axis.z),
            angle_rad: angle,
        };
        let Some(geo) = self.model.geometry.get(body) else {
            return;
        };
        match ducad_kernel::apply_mate_transform_to_shape(&geo.shape, &tf) {
            Ok(shape) => {
                self.model.geometry.insert(body, BodyGeometry::from_shape(shape));
                self.model.doc.dirty = true;
            }
            Err(e) => log::warn!("gagal memindah geometri instance {id}: {e}"),
        }
    }

    /// Pasang pose lokal instance (geometri ikut dipindah).
    pub fn restore_instance_poses(&mut self, poses: &HashMap<AssemblyInstanceId, LocalPose>) {
        let mut ids: Vec<_> = poses.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let Some(from) = self.assembly_tree.instance_world_transform(id) else {
                continue;
            };
            let Some(&(t, q)) = poses.get(&id) else {
                continue;
            };
            match self.assembly_tree.instances.get_mut(&id) {
                Some(inst) if inst.translation != t || inst.rotation_quat != q => {
                    inst.translation = t;
                    inst.rotation_quat = q;
                }
                _ => continue,
            }
            if let Some(to) = self.assembly_tree.instance_world_transform(id) {
                self.move_instance_geometry(id, from, to);
            }
        }
    }

    /// Paritas 4.8: nama pasangan contact set yang saling menembus.
    pub fn contact_violation(&self) -> Option<String> {
        for (a, b) in self.assembly_tree.contact_pairs() {
            let (Some(ba), Some(bb)) = (self.instance_body(a), self.instance_body(b)) else {
                continue;
            };
            let (Some(ga), Some(gb)) = (self.model.geometry.get(ba), self.model.geometry.get(bb))
            else {
                continue;
            };
            if let Ok(Some((_, vol))) =
                ducad_kernel::compute_pair_interference(&ga.shape, &gb.shape, 1e-3)
            {
                if vol > 1e-3 {
                    let name = |i: AssemblyInstanceId| {
                        self.assembly_tree
                            .instances
                            .get(&i)
                            .map(|x| x.name.clone())
                            .unwrap_or_default()
                    };
                    return Some(format!("{} - {}", name(a), name(b)));
                }
            }
        }
        None
    }

    /// Dipanggil setelah solve: bila contact set dilanggar, kembalikan pose
    /// `before` (gerakan ditolak). `true` bila ditolak.
    pub fn enforce_contact_sets(&mut self, before: &HashMap<AssemblyInstanceId, LocalPose>) -> bool {
        if self.assembly_tree.contact_sets.iter().all(|s| !s.enabled) {
            return false;
        }
        let Some(pair) = self.contact_violation() else {
            return false;
        };
        self.restore_instance_poses(before);
        self.model_status = Some(ducad_i18n::t!("asm-contact-blocked", pair = pair.as_str()));
        true
    }

    // ------------------------------------------------------------------
    // Joint (4.1–4.4).
    // ------------------------------------------------------------------

    /// Buat joint `kind` dari dua face terpilih (A = acuan, B = yang
    /// bergerak). `as_built`: B tidak dipindah. Asal joint dari datum sumbu
    /// `assembly_tools.origin_axis` bila dipilih.
    pub fn create_joint(&mut self, kind: JointKind, as_built: bool) -> bool {
        if self.staged_mate_targets.len() < 2 {
            self.model_status = Some(ducad_i18n::t!("asm-joint-need-two"));
            return false;
        }
        let (body_a, _, hit_a) = self.staged_mate_targets[0].clone();
        let (body_b, _, hit_b) = self.staged_mate_targets[1].clone();
        if body_a == body_b {
            self.model_status = Some(ducad_i18n::t!("asm-joint-need-two"));
            return false;
        }
        self.sync_assembly_instances();
        let (Some(id_a), Some(id_b)) = (self.instance_of_body(body_a), self.instance_of_body(body_b))
        else {
            self.model_status = Some(ducad_i18n::t!("asm-joint-need-two"));
            return false;
        };
        let (Some(pa0), Some(pb0)) = (
            self.assembly_tree.instance_world_transform(id_a),
            self.assembly_tree.instance_world_transform(id_b),
        ) else {
            return false;
        };
        let ta = mate_target_from_hit(&hit_a);
        let tb = mate_target_from_hit(&hit_b);
        let n = self.assembly_tree.mates.len() + 1;
        let name = format!("Joint {n}: {}", kind.label());
        let mate_id = self.assembly_tree.add_mate(
            &name,
            kind.underlying_mate(),
            MateTarget {
                instance_id: id_a,
                kind: ta.clone(),
            },
            MateTarget {
                instance_id: id_b,
                kind: tb.clone(),
            },
        );
        if !as_built {
            match kind {
                JointKind::Ball => {
                    // Titik asal B ke titik asal A (tanpa rotasi).
                    let (oa, _) = target_axis(&ta);
                    let (ob, _) = target_axis(&tb);
                    let (pt, pq) = self.assembly_tree.instance_parent_transform(id_b);
                    let local_t = pq.inverse() * (pb0.0 + (oa - ob) - pt);
                    let mut poses = self.instance_poses();
                    if let Some(p) = poses.get_mut(&id_b) {
                        p.0 = (local_t.x, local_t.y, local_t.z);
                    }
                    self.restore_instance_poses(&poses);
                }
                JointKind::Cam => {}
                _ => self.solve_and_apply_assembly(),
            }
        }
        let (Some(pa), Some(pb)) = (
            self.assembly_tree.instance_world_transform(id_a),
            self.assembly_tree.instance_world_transform(id_b),
        ) else {
            return false;
        };
        let (oa, da) = {
            let (o, d) = target_axis(&ta);
            carry(o, d, pa0, pa)
        };
        let (ob, db) = {
            let (o, d) = target_axis(&tb);
            carry(o, d, pb0, pb)
        };
        let (origin, axis, u) = match kind {
            JointKind::PinSlot => (ob, db, Some(da.cross(db))),
            JointKind::Cam => (ob, db, None),
            _ => (oa, da, None),
        };
        let (origin, axis) = match self
            .assembly_tools
            .origin_axis
            .and_then(|i| self.datums.axis(i))
        {
            Some(ax) => (DVec3::from(ax.axis.origin), DVec3::from(ax.axis.dir)),
            None => (origin, axis),
        };
        let Some(frame) = JointFrame::from_world(pa, origin, axis, u) else {
            self.assembly_tree.remove_mate(mate_id);
            self.model_status = Some(ducad_i18n::t!("asm-joint-bad-axis"));
            return false;
        };
        let state = JointState::capture(kind, frame, pa, pb, [0.0; 3]);
        if let Some(m) = self.assembly_tree.mates.get_mut(&mate_id) {
            m.joint = Some(kind);
            m.joint_state = Some(state);
        }
        self.staged_mate_targets.clear();
        self.selected_faces.clear();
        self.active_face = None;
        self.model.doc.dirty = true;
        self.model_status = Some(ducad_i18n::t!("asm-joint-created", name = name.as_str()));
        true
    }

    /// Atur nilai joint ke-`k` lalu selesaikan ulang.
    pub fn set_joint_value(&mut self, mate: u32, k: usize, value: f64) {
        if !value.is_finite() || k > 2 {
            return;
        }
        if let Some(js) = self
            .assembly_tree
            .mates
            .get_mut(&mate)
            .and_then(|m| m.joint_state.as_mut())
        {
            js.values[k] = value;
        }
        self.solve_and_apply_assembly();
    }

    /// Paritas 4.2: follower Cam digeser sepanjang sumbunya sampai menyentuh
    /// cam (jarak minimum ≈ 0, tanpa tembus). `true` bila nilai joint berubah.
    pub fn update_cam_joints(&mut self) -> bool {
        let cams: Vec<(u32, AssemblyInstanceId, AssemblyInstanceId)> = self
            .assembly_tree
            .mates
            .values()
            .filter(|m| m.joint == Some(JointKind::Cam) && !m.suppressed && m.joint_state.is_some())
            .map(|m| (m.id, m.target_a.instance_id, m.target_b.instance_id))
            .collect();
        let mut changed = false;
        for (mid, ia, ib) in cams {
            let (Some(ba), Some(bb)) = (self.instance_body(ia), self.instance_body(ib)) else {
                continue;
            };
            let (Some(ga), Some(gb), Some(pa)) = (
                self.model.geometry.get(ba),
                self.model.geometry.get(bb),
                self.assembly_tree.instance_world_transform(ia),
            ) else {
                continue;
            };
            let Some(js) = self.assembly_tree.mates.get(&mid).and_then(|m| m.joint_state) else {
                continue;
            };
            let (_, axis) = js.world_axis(pa);
            if axis.length_squared() < 1e-12 {
                continue;
            }
            let (Ok(cam), Ok(follower)) = (ducad_kernel::clone_shape(&ga.shape), ducad_kernel::clone_shape(&gb.shape)) else {
                continue;
            };
            // Jarak bertanda: > 0 terpisah, < 0 menembus.
            let dist_at = |d: f64| -> Option<f64> {
                let moved =
                    ducad_kernel::translate_shape(&follower, axis.x * d, axis.y * d, axis.z * d)
                        .ok()?;
                let pen = ducad_kernel::compute_pair_interference(&cam, &moved, 1e-6)
                    .ok()
                    .flatten()
                    .map(|(_, v)| v)
                    .unwrap_or(0.0);
                if pen > 1e-6 {
                    return Some(-pen.cbrt());
                }
                ducad_kernel::check_clearance(&cam, &moved, 0.0)
                    .ok()
                    .map(|c| c.distance)
            };
            let Some(d0) = dist_at(0.0) else { continue };
            if d0.abs() < 1e-3 {
                continue;
            }
            // Arah yang mendekatkan ke cam (terpisah) / menjauhkan (tembus).
            let toward = match (dist_at(0.25), dist_at(-0.25)) {
                (Some(p), Some(m)) if p < m => 1.0,
                _ => -1.0,
            };
            let (mut lo, mut hi) = if d0 > 0.0 {
                // lo terpisah, cari hi yang menembus.
                let mut lo = 0.0f64;
                let mut hi = None;
                let mut step = d0.max(0.05);
                for _ in 0..40 {
                    let probe = lo + toward * step;
                    match dist_at(probe) {
                        Some(d) if d > 1e-3 => {
                            lo = probe;
                            step = d.max(0.05);
                        }
                        Some(_) => {
                            hi = Some(probe);
                            break;
                        }
                        None => break,
                    }
                }
                match hi {
                    Some(h) => (lo, h),
                    None => continue,
                }
            } else {
                // hi menembus, cari lo yang terpisah (mundur).
                let mut step = d0.abs().max(0.5);
                let mut lo = None;
                for _ in 0..40 {
                    let probe = -toward * step;
                    match dist_at(probe) {
                        Some(d) if d > 1e-3 => {
                            lo = Some(probe);
                            break;
                        }
                        Some(_) => step *= 2.0,
                        None => break,
                    }
                }
                match lo {
                    Some(l) => (l, 0.0),
                    None => continue,
                }
            };
            for _ in 0..40 {
                let mid = 0.5 * (lo + hi);
                match dist_at(mid) {
                    Some(d) if d > 1e-3 => lo = mid,
                    _ => hi = mid,
                }
                if (hi - lo).abs() < 1e-4 {
                    break;
                }
            }
            if let Some(js) = self
                .assembly_tree
                .mates
                .get_mut(&mid)
                .and_then(|m| m.joint_state.as_mut())
            {
                js.values[0] += lo;
                changed = true;
            }
        }
        changed
    }

    // ------------------------------------------------------------------
    // Komponen (4.5, 4.6, 4.10), posisi (4.7), contact set (4.8).
    // ------------------------------------------------------------------

    /// Instance terpilih (lewat body terpilih / instance terpilih di drawer).
    pub fn selected_instances(&mut self) -> Vec<AssemblyInstanceId> {
        self.sync_assembly_instances();
        let bodies: Vec<BodyId> = self.selected_bodies.iter().copied().collect();
        let mut ids: Vec<_> = bodies
            .into_iter()
            .filter_map(|b| self.instance_of_body(b))
            .chain(self.selected_assembly_instance)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub fn rename_component(&mut self, id: AssemblyInstanceId, name: &str) -> bool {
        if !self.assembly_tree.rename_instance(id, name) {
            return false;
        }
        if let Some(body) = self.instance_body(id) {
            if let Some(b) = self.model.doc.bodies.get_mut(body) {
                b.name = name.trim().to_string();
            }
        }
        self.model.doc.dirty = true;
        true
    }

    /// Paritas 4.5: aktifkan komponen (atau `None` = semua).
    pub fn activate_component(&mut self, id: Option<AssemblyInstanceId>) {
        self.assembly_tools.active_instance = id;
        self.enforce_active_component();
        let name = id
            .and_then(|i| self.assembly_tree.instances.get(&i))
            .map(|i| i.name.clone());
        self.model_status = Some(match name {
            Some(n) => ducad_i18n::t!("asm-active", name = n.as_str()),
            None => ducad_i18n::t!("asm-active-none"),
        });
    }

    /// Body di luar komponen aktif tidak boleh terpilih (dipanggil tiap frame).
    pub fn enforce_active_component(&mut self) {
        let Some(active) = self.assembly_tools.active_instance else {
            return;
        };
        let Some(raw) = self.assembly_tree.instances.get(&active).map(|i| i.body_id_raw) else {
            self.assembly_tools.active_instance = None;
            return;
        };
        self.selected_bodies.retain(|b| b.data().as_ffi() == raw);
        if self
            .active_face
            .as_ref()
            .is_some_and(|(b, _, _)| b.data().as_ffi() != raw)
        {
            self.active_face = None;
        }
    }

    /// Paritas 4.6: kelompokkan instance terpilih ke komponen baru
    /// (sub-assembly) bernama `name`.
    pub fn create_component_from_selection(&mut self, name: &str) -> Option<u32> {
        let ids = self.selected_instances();
        if ids.is_empty() {
            self.model_status = Some(ducad_i18n::t!("body-tool-need-body"));
            return None;
        }
        let n = self.assembly_tree.sub_assemblies.len() + 1;
        let name = if name.trim().is_empty() {
            ducad_i18n::t!("asm-component-name", n = n)
        } else {
            name.trim().to_string()
        };
        let sub = self.assembly_tree.add_sub_assembly(name.clone(), None);
        for id in &ids {
            self.assembly_tree.set_instance_parent(*id, Some(sub));
        }
        self.model.doc.dirty = true;
        self.model_status = Some(ducad_i18n::t!(
            "asm-component-created",
            name = name.as_str(),
            count = ids.len()
        ));
        Some(sub)
    }

    /// Paritas 4.6: pindahkan instance terpilih ke komponen `sub` (atau
    /// keluar bila `None`).
    pub fn move_selection_to_component(&mut self, sub: Option<u32>) {
        for id in self.selected_instances() {
            self.assembly_tree.set_instance_parent(id, sub);
        }
        self.model.doc.dirty = true;
    }

    /// Paritas 4.7.
    pub fn capture_positions(&mut self) {
        self.sync_assembly_instances();
        self.assembly_tree.capture_positions();
        self.model.doc.dirty = true;
        self.model_status = Some(ducad_i18n::t!(
            "asm-captured",
            count = self.assembly_tree.captured_poses.len()
        ));
    }

    /// Paritas 4.7: kembalikan posisi tersimpan (geometri ikut).
    pub fn revert_positions(&mut self) {
        if self.assembly_tree.captured_poses.is_empty() {
            self.model_status = Some(ducad_i18n::t!("asm-nothing-captured"));
            return;
        }
        let poses: HashMap<_, _> = self
            .assembly_tree
            .captured_poses
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();
        self.restore_instance_poses(&poses);
        self.model_status = Some(ducad_i18n::t!("asm-reverted"));
    }

    /// Paritas 4.8: contact set dari instance terpilih.
    pub fn add_contact_set_from_selection(&mut self) {
        let ids = self.selected_instances();
        let ok = self.assembly_tree.add_contact_set("", &ids);
        self.model_status = Some(if ok {
            ducad_i18n::t!("asm-contact-added", count = ids.len())
        } else {
            ducad_i18n::t!("asm-contact-need-two")
        });
    }

    /// Paritas 4.10: salin occurrence body terpilih (part sama, digeser
    /// `offset`); metadata part ikut tersalin.
    pub fn copy_occurrence(&mut self, offset: [f64; 3]) -> Option<BodyId> {
        let src = self
            .selected_bodies
            .iter()
            .copied()
            .find(|b| self.model.geometry.contains_key(*b));
        let Some(src) = src else {
            self.model_status = Some(ducad_i18n::t!("body-tool-need-body"));
            return None;
        };
        self.sync_assembly_instances();
        let shape = ducad_kernel::translate_shape(
            &self.model.geometry.get(src)?.shape,
            offset[0],
            offset[1],
            offset[2],
        )
        .ok()?;
        let meta = self.model.doc.bodies.get(src)?.clone();
        let count = self
            .model
            .doc
            .bodies
            .iter()
            .filter(|(_, b)| b.name.starts_with(&meta.name))
            .count();
        let name = format!("{} ({})", meta.name, count + 1);
        let msg = ducad_i18n::t!("asm-occurrence-copied", name = name.as_str());
        let cmd = crate::model::AddSolidCommand::new(name.clone(), BodyGeometry::from_shape(shape))
            .with_material(meta.material);
        self.execute_model_command(Box::new(cmd), &msg);
        let new_body = self
            .model
            .doc
            .bodies
            .iter()
            .find(|(_, b)| b.name == name)
            .map(|(id, _)| id)?;
        let src_inst = self
            .instance_of_body(src)
            .and_then(|i| self.assembly_tree.instances.get(&i).cloned());
        self.sync_assembly_instances();
        if let (Some(si), Some(ni)) = (src_inst, self.instance_of_body(new_body)) {
            if let Some(inst) = self.assembly_tree.instances.get_mut(&ni) {
                inst.part_number = si.part_number.clone();
                inst.material = si.material.clone();
                inst.source = si.source.clone();
                inst.parent_sub_assembly = si.parent_sub_assembly;
            }
        }
        self.model_status = Some(msg);
        Some(new_body)
    }
}

/// Jenis joint yang ditawarkan di jendela.
pub const JOINT_KINDS: [JointKind; 8] = [
    JointKind::Rigid,
    JointKind::Revolute,
    JointKind::Slider,
    JointKind::Cylindrical,
    JointKind::Planar,
    JointKind::Ball,
    JointKind::PinSlot,
    JointKind::Cam,
];

/// Banyak nilai joint yang bisa diatur (= DOF total joint).
fn value_count(kind: JointKind) -> usize {
    match kind {
        JointKind::Rigid => 0,
        JointKind::Revolute | JointKind::Slider | JointKind::Cam => 1,
        JointKind::Cylindrical | JointKind::PinSlot => 2,
        JointKind::Planar | JointKind::Ball => 3,
    }
}

impl DuCADApp {
    /// Jendela "Alat Perakitan" (Fase 4).
    pub fn show_assembly_tools(&mut self, ctx: &egui::Context) {
        if !self.assembly_tools.open {
            return;
        }
        enum Act {
            CreateJoint,
            SetValue(u32, usize, f64),
            Rename(AssemblyInstanceId, String),
            Activate(Option<AssemblyInstanceId>),
            NewComponent(String),
            MoveTo(Option<u32>),
            Capture,
            Revert,
            AddContact,
            ToggleContact(usize),
            DeleteContact(usize),
            CopyOccurrence([f64; 3]),
            OpenMotion,
        }
        let mut acts: Vec<Act> = Vec::new();
        let staged = self.staged_mate_targets.len();
        let axes: Vec<(u32, String)> = self.datums.axes.iter().map(|a| (a.id, a.name.clone())).collect();
        let mut joints: Vec<(u32, String, JointKind, [f64; 3])> = self
            .assembly_tree
            .mates
            .values()
            .filter_map(|m| Some((m.id, m.name.clone(), m.joint?, m.joint_state?.values)))
            .collect();
        joints.sort_by_key(|j| j.0);
        let sel = self.selected_assembly_instance.or_else(|| {
            self.selected_bodies
                .iter()
                .next()
                .and_then(|b| self.instance_of_body(*b))
        });
        let sel_name = sel
            .and_then(|i| self.assembly_tree.instances.get(&i))
            .map(|i| i.name.clone());
        let mut subs: Vec<(u32, String)> = self
            .assembly_tree
            .sub_assemblies
            .values()
            .map(|s| (s.id, s.name.clone()))
            .collect();
        subs.sort();
        let contacts: Vec<(String, usize, bool)> = self
            .assembly_tree
            .contact_sets
            .iter()
            .map(|c| (c.name.clone(), c.instances.len(), c.enabled))
            .collect();
        let active = self.assembly_tools.active_instance;
        let captured = !self.assembly_tree.captured_poses.is_empty();
        let mut open = true;
        let st = &mut self.assembly_tools;
        egui::Window::new(ducad_i18n::t!("asm-tools-title"))
            .open(&mut open)
            .collapsible(true)
            .resizable(true)
            .default_width(380.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                    // --- Joint ---
                    ui.strong(ducad_i18n::t!("asm-joint-section"));
                    ui.label(egui::RichText::new(ducad_i18n::t!("asm-joint-help")).small());
                    egui::Grid::new("asm-joint-grid").num_columns(2).show(ui, |ui| {
                        ui.label(ducad_i18n::t!("asm-joint-kind"));
                        egui::ComboBox::from_id_salt("asm-joint-kind")
                            .selected_text(st.joint_kind.label())
                            .show_ui(ui, |ui| {
                                for k in JOINT_KINDS {
                                    ui.selectable_value(&mut st.joint_kind, k, k.label());
                                }
                            });
                        ui.end_row();
                        ui.label(ducad_i18n::t!("asm-joint-as-built"));
                        ui.checkbox(&mut st.as_built, "");
                        ui.end_row();
                        ui.label(ducad_i18n::t!("asm-joint-origin"));
                        let cur = st
                            .origin_axis
                            .and_then(|i| axes.iter().find(|(a, _)| *a == i))
                            .map(|(_, n)| n.clone())
                            .unwrap_or_else(|| ducad_i18n::t!("asm-joint-origin-face"));
                        egui::ComboBox::from_id_salt("asm-joint-origin")
                            .selected_text(cur)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut st.origin_axis, None, ducad_i18n::t!("asm-joint-origin-face"));
                                for (id, n) in &axes {
                                    ui.selectable_value(&mut st.origin_axis, Some(*id), n);
                                }
                            });
                        ui.end_row();
                    });
                    if ui
                        .add_enabled(staged >= 2, egui::Button::new(ducad_i18n::t!("asm-joint-create", count = staged)))
                        .clicked()
                    {
                        acts.push(Act::CreateJoint);
                    }
                    for (id, name, kind, values) in &joints {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(name).small());
                            for (k, v) in values.iter().enumerate().take(value_count(*kind)) {
                                let mut x = *v;
                                if ui.add(egui::DragValue::new(&mut x).speed(0.5)).changed() {
                                    acts.push(Act::SetValue(*id, k, x));
                                }
                            }
                        });
                    }
                    ui.separator();
                    // --- Komponen ---
                    ui.strong(ducad_i18n::t!("asm-component-section"));
                    match (sel, &sel_name) {
                        (Some(id), Some(name)) => {
                            ui.horizontal(|ui| {
                                if st.rename_text.is_empty() {
                                    st.rename_text = name.clone();
                                }
                                ui.add(egui::TextEdit::singleline(&mut st.rename_text).desired_width(150.0));
                                if ui.button(ducad_i18n::t!("asm-rename")).clicked() {
                                    acts.push(Act::Rename(id, st.rename_text.clone()));
                                }
                            });
                            ui.horizontal(|ui| {
                                if active == Some(id) {
                                    if ui.button(ducad_i18n::t!("asm-deactivate")).clicked() {
                                        acts.push(Act::Activate(None));
                                    }
                                } else if ui.button(ducad_i18n::t!("asm-activate")).clicked() {
                                    acts.push(Act::Activate(Some(id)));
                                }
                            });
                        }
                        _ => {
                            ui.label(egui::RichText::new(ducad_i18n::t!("asm-select-body")).small());
                            if active.is_some() && ui.button(ducad_i18n::t!("asm-deactivate")).clicked() {
                                acts.push(Act::Activate(None));
                            }
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut st.component_name)
                                .hint_text(ducad_i18n::t!("asm-component-hint"))
                                .desired_width(150.0),
                        );
                        if ui.button(ducad_i18n::t!("asm-component-new")).clicked() {
                            acts.push(Act::NewComponent(std::mem::take(&mut st.component_name)));
                        }
                    });
                    if !subs.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(ducad_i18n::t!("asm-move-to"));
                            for (id, n) in &subs {
                                if ui.small_button(n).clicked() {
                                    acts.push(Act::MoveTo(Some(*id)));
                                }
                            }
                            if ui.small_button(ducad_i18n::t!("asm-move-out")).clicked() {
                                acts.push(Act::MoveTo(None));
                            }
                        });
                    }
                    ui.separator();
                    // --- Posisi & contact set & occurrence ---
                    ui.horizontal(|ui| {
                        if ui.button(ducad_i18n::t!("asm-capture")).clicked() {
                            acts.push(Act::Capture);
                        }
                        if ui.add_enabled(captured, egui::Button::new(ducad_i18n::t!("asm-revert"))).clicked() {
                            acts.push(Act::Revert);
                        }
                    });
                    ui.strong(ducad_i18n::t!("asm-contact-section"));
                    for (i, (name, n, enabled)) in contacts.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let mut on = *enabled;
                            if ui.checkbox(&mut on, format!("{name} ({n})")).changed() {
                                acts.push(Act::ToggleContact(i));
                            }
                            if ui.small_button(egui_icons::icons::ICON_DELETE.codepoint).clicked() {
                                acts.push(Act::DeleteContact(i));
                            }
                        });
                    }
                    if ui.button(ducad_i18n::t!("asm-contact-add")).clicked() {
                        acts.push(Act::AddContact);
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        for v in st.occurrence_offset.iter_mut() {
                            ui.add(egui::DragValue::new(v).speed(0.5).suffix(" mm"));
                        }
                        if ui.button(ducad_i18n::t!("asm-occurrence-copy")).clicked() {
                            acts.push(Act::CopyOccurrence(st.occurrence_offset));
                        }
                    });
                    ui.separator();
                    if ui.button(ducad_i18n::t!("motion-title")).clicked() {
                        acts.push(Act::OpenMotion);
                    }
                });
            });
        for a in acts {
            match a {
                Act::CreateJoint => {
                    let (k, b) = (self.assembly_tools.joint_kind, self.assembly_tools.as_built);
                    self.create_joint(k, b);
                }
                Act::SetValue(m, k, v) => self.set_joint_value(m, k, v),
                Act::Rename(id, name) => {
                    if !self.rename_component(id, &name) {
                        self.model_status = Some(ducad_i18n::t!("timeline-rename-empty"));
                    }
                    self.assembly_tools.rename_text.clear();
                }
                Act::Activate(id) => self.activate_component(id),
                Act::NewComponent(name) => {
                    self.create_component_from_selection(&name);
                }
                Act::MoveTo(sub) => self.move_selection_to_component(sub),
                Act::Capture => self.capture_positions(),
                Act::Revert => self.revert_positions(),
                Act::AddContact => self.add_contact_set_from_selection(),
                Act::ToggleContact(i) => {
                    if let Some(c) = self.assembly_tree.contact_sets.get_mut(i) {
                        c.enabled = !c.enabled;
                    }
                }
                Act::DeleteContact(i) => {
                    if i < self.assembly_tree.contact_sets.len() {
                        self.assembly_tree.contact_sets.remove(i);
                    }
                }
                Act::CopyOccurrence(o) => {
                    self.copy_occurrence(o);
                }
                Act::OpenMotion => self.motion_tools.open = true,
            }
        }
        if !open {
            self.assembly_tools.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_boxes() -> (DuCADApp, BodyId, BodyId) {
        let mut app = DuCADApp::new_for_test();
        app.create_primitive(ducad_ui::PrimitiveKind::Box, [10.0; 3], [0.0; 3], false);
        app.create_primitive(ducad_ui::PrimitiveKind::Box, [10.0; 3], [30.0, 0.0, 0.0], false);
        let ids: Vec<BodyId> = app.model.doc.bodies.iter().map(|(id, _)| id).collect();
        app.sync_assembly_instances();
        (app, ids[0], ids[1])
    }

    fn hit(origin: (f64, f64, f64), normal: (f64, f64, f64)) -> ducad_kernel::FaceHit {
        ducad_kernel::FaceHit {
            hit_point: origin,
            centroid: origin,
            normal,
            surface_kind: ducad_kernel::SurfaceKind::Plane,
            pull_dir: normal,
            boundary_points: Vec::new(),
            face_index: Some(0),
            surface_radius: None,
            radial_axis: None,
        }
    }

    fn ray() -> ducad_kernel::PickRay {
        ducad_kernel::PickRay {
            origin: (0.0, 0.0, 0.0),
            dir: (0.0, 0.0, -1.0),
        }
    }

    fn min_x(app: &DuCADApp, b: BodyId) -> f64 {
        app.model.geometry.get(b).unwrap().shape.bounding_box().unwrap().0[0]
    }

    #[test]
    fn as_built_revolute_keeps_position_then_drives() {
        let (mut app, a, b) = two_boxes();
        app.staged_mate_targets = vec![
            (a, ray(), hit((5.0, 5.0, 10.0), (0.0, 0.0, 1.0))),
            (b, ray(), hit((35.0, 5.0, 10.0), (0.0, 0.0, 1.0))),
        ];
        let before = min_x(&app, b);
        assert!(app.create_joint(JointKind::Revolute, true), "{:?}", app.model_status);
        app.solve_and_apply_assembly();
        assert!((min_x(&app, b) - before).abs() < 1e-6, "as-built tidak memindah");
        let mid = *app.assembly_tree.mates.keys().next().unwrap();
        app.set_joint_value(mid, 0, 180.0);
        // Diputar 180° mengelilingi sumbu Z lewat (5,5): x 30..40 → -30..-20.
        assert!((min_x(&app, b) + 30.0).abs() < 1e-3, "{}", min_x(&app, b));
    }

    #[test]
    fn cam_follower_moves_until_contact() {
        let (mut app, a, b) = two_boxes();
        // Follower B (x 30..40) bergerak sepanjang X menuju cam A (x 0..10).
        app.staged_mate_targets = vec![
            (a, ray(), hit((10.0, 5.0, 5.0), (1.0, 0.0, 0.0))),
            (b, ray(), hit((30.0, 5.0, 5.0), (-1.0, 0.0, 0.0))),
        ];
        assert!(app.create_joint(JointKind::Cam, false), "{:?}", app.model_status);
        app.solve_and_apply_assembly();
        let x = min_x(&app, b);
        assert!((x - 10.0).abs() < 0.05, "follower menempel cam: {x}");
    }

    #[test]
    fn persistence_round_trip_by_uuid() {
        let (mut app, a, b) = two_boxes();
        app.staged_mate_targets = vec![
            (a, ray(), hit((5.0, 5.0, 10.0), (0.0, 0.0, 1.0))),
            (b, ray(), hit((35.0, 5.0, 10.0), (0.0, 0.0, 1.0))),
        ];
        assert!(app.create_joint(JointKind::Slider, true));
        for (_, body) in app.model.doc.bodies.iter_mut() {
            if body.uuid.is_empty() {
                body.uuid = format!("u-{}", body.name);
            }
        }
        let json = app.assembly_to_json();
        assert!(json.is_some());
        let n_inst = app.assembly_tree.instances.len();
        app.assembly_tree = Default::default();
        app.assembly_from_json(json);
        assert_eq!(app.assembly_tree.instances.len(), n_inst);
        assert_eq!(app.assembly_tree.mates.len(), 1);
        app.assembly_from_json(Some(serde_json::json!({"tree": "rusak"})));
        assert!(app.assembly_tree.instances.is_empty());
    }

    #[test]
    fn capture_revert_rename_contact_copy_active() {
        let (mut app, a, b) = two_boxes();
        app.capture_positions();
        let ib = app.instance_of_body(b).unwrap();
        let mut poses = app.instance_poses();
        poses.get_mut(&ib).unwrap().0 = (0.0, 50.0, 0.0);
        app.restore_instance_poses(&poses);
        let y = app.model.geometry.get(b).unwrap().shape.bounding_box().unwrap().0[1];
        assert!((y - 50.0).abs() < 1e-6, "{y}");
        app.revert_positions();
        let y = app.model.geometry.get(b).unwrap().shape.bounding_box().unwrap().0[1];
        assert!(y.abs() < 1e-6, "{y}");
        assert!(app.rename_component(ib, "Tutup"));
        assert_eq!(app.model.doc.bodies.get(b).unwrap().name, "Tutup");
        // Contact set: B didorong menembus A → ditolak.
        app.selected_bodies = [a, b].into_iter().collect();
        app.add_contact_set_from_selection();
        assert_eq!(app.assembly_tree.contact_sets.len(), 1);
        let before = app.instance_poses();
        let mut p = before.clone();
        p.get_mut(&ib).unwrap().0 = (-25.0, 0.0, 0.0);
        app.restore_instance_poses(&p);
        assert!(app.enforce_contact_sets(&before));
        assert!((min_x(&app, b) - 30.0).abs() < 1e-6, "dikembalikan: {}", min_x(&app, b));
        // Salin occurrence.
        app.selected_bodies = [b].into_iter().collect();
        let n = app.model.doc.bodies.len();
        assert!(app.copy_occurrence([0.0, 20.0, 0.0]).is_some(), "{:?}", app.model_status);
        assert_eq!(app.model.doc.bodies.len(), n + 1);
        // Komponen aktif membatasi seleksi.
        app.activate_component(Some(ib));
        app.selected_bodies = [a, b].into_iter().collect();
        app.enforce_active_component();
        assert_eq!(app.selected_bodies.len(), 1);
        app.activate_component(None);
        app.selected_bodies = [a].into_iter().collect();
        assert!(app.create_component_from_selection("Rangka").is_some());
    }
}
