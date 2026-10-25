//! Perintah undo untuk penggantian SELURUH model (P5.1).
//!
//! Dipakai jalur agent yang harus mereplay oplog di sesi terpisah
//! (`set_params`, penerimaan proposal berbasis edit): hasil replay adalah
//! `ModelDoc` baru, dan pertukarannya harus tetap menjadi SATU langkah
//! undo GUI. `KernelShape` tidak `Clone`, jadi penggantiannya dilakukan
//! dengan `std::mem::swap` — model lama disimpan di dalam command dan
//! ditukar balik saat undo.

use ducad_core::Command;
use ducad_engine::model::ModelDoc;

pub struct SwapModelCommand {
    /// Model yang akan dipasang saat `apply`, lalu diisi model lama.
    other: ModelDoc,
    label: String,
}

impl SwapModelCommand {
    pub fn new(next: ModelDoc) -> Self {
        Self {
            other: next,
            label: "Agent".to_string(),
        }
    }
}

impl Command<ModelDoc> for SwapModelCommand {
    fn name(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, target: &mut ModelDoc) {
        std::mem::swap(&mut self.other, target);
    }

    fn revert(&mut self, target: &mut ModelDoc) {
        std::mem::swap(&mut self.other, target);
    }
}

/// Terapkan `next` (hasil replay) ke `undo`/`model` sebagai SATU transaksi
/// undo yang mempertahankan `BodyId` (dan UUID) body yang namanya sama:
/// geometri diganti di tempat, material disesuaikan, body baru ditambahkan,
/// body yang hilang dihapus. Assembly dan seleksi merujuk body lewat
/// `BodyId`, jadi menukar seluruh model ([`SwapModelCommand`]) akan memutus
/// referensinya setiap kali replay (ADR 0010).
pub fn apply_model_preserving_ids(
    undo: &mut ducad_core::UndoStack<ModelDoc>,
    model: &mut ModelDoc,
    next: ModelDoc,
    label: &str,
) {
    apply_model_preserving_ids_renamed(undo, model, next, label, &[]);
}

/// Seperti [`apply_model_preserving_ids`], dengan `renames` `(lama, baru)`:
/// body lama bernama `lama` dicocokkan ke body replay bernama `baru` (op
/// `rename` baru disisipkan) sehingga `BodyId`-nya tetap.
pub fn apply_model_preserving_ids_renamed(
    undo: &mut ducad_core::UndoStack<ModelDoc>,
    model: &mut ModelDoc,
    mut next: ModelDoc,
    label: &str,
    renames: &[(String, String)],
) {
    use ducad_engine::model::{
        AddSolidCommand, DeleteBodyCommand, RenameBodyCommand, ReplaceGeometryCommand,
        SetBodyMechanicalCommand,
    };
    undo.begin(label.to_string());
    let mut unmatched_old: Vec<ducad_core::BodyId> = model.doc.bodies.keys().collect();
    let new_ids: Vec<ducad_core::BodyId> = next.doc.bodies.keys().collect();
    for nid in new_ids {
        let Some(nbody) = next.doc.bodies.get(nid).cloned() else {
            continue;
        };
        let Some(geo) = next.geometry.remove(nid) else {
            continue;
        };
        let matched = unmatched_old.iter().position(|oid| {
            model
                .doc
                .bodies
                .get(*oid)
                .is_some_and(|b| b.name == nbody.name)
        });
        let matched = matched.or_else(|| {
            unmatched_old.iter().position(|oid| {
                model.doc.bodies.get(*oid).is_some_and(|b| {
                    renames.iter().any(|(o, n)| *o == b.name && *n == nbody.name)
                })
            })
        });
        match matched {
            Some(pos) => {
                let oid = unmatched_old.remove(pos);
                if model.doc.bodies[oid].name != nbody.name {
                    undo.execute(Box::new(RenameBodyCommand::new(oid, nbody.name.clone())), model);
                }
                undo.execute(
                    Box::new(ReplaceGeometryCommand::new("Replay", oid, geo)),
                    model,
                );
                // Material VISUAL hanya diatur GUI (drawer CMF) — tidak ada
                // op oplog yang menyetelnya, sehingga hasil replay selalu
                // membawa nilai default. Jangan timpa nilai GUI yang ada.
                // Material MEKANIK ikut replay HANYA bila op `set_material`
                // memang menyetelnya (`Some`); `None` berarti oplog tidak
                // berpendapat, jadi pilihan GUI (panel massa) dipertahankan.
                if let Some(mech) = nbody.mechanical.clone() {
                    if model.doc.bodies[oid].mechanical.as_ref() != Some(&mech) {
                        undo.execute(
                            Box::new(SetBodyMechanicalCommand::new("Replay", oid, Some(mech))),
                            model,
                        );
                    }
                }
            }
            None => {
                undo.execute(
                    Box::new(
                        AddSolidCommand::new(label.to_string(), geo).with_body_name(nbody.name),
                    ),
                    model,
                );
            }
        }
    }
    for oid in unmatched_old {
        undo.execute(Box::new(DeleteBodyCommand::new(oid)), model);
    }
    undo.commit();
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_core::UndoStack;

    fn body(m: &mut ModelDoc, name: &str, size: f64) -> ducad_core::BodyId {
        let geo = ducad_engine::compute::primitive(
            &ducad_engine::compute::PrimitiveShape::Box {
                size: [size, size, size],
                centered: false,
            },
            [0.0; 3],
        )
        .unwrap();
        let id = m.doc.add_body(name);
        m.geometry.insert(id, geo);
        id
    }

    #[test]
    fn replay_keeps_body_ids_and_uuids_and_is_one_step() {
        let mut model = ModelDoc::default();
        let a = body(&mut model, "a", 10.0);
        let gone = body(&mut model, "gone", 5.0);
        let uuid_a = model.doc.bodies[a].uuid.clone();
        let mut next = ModelDoc::default();
        body(&mut next, "a", 20.0);
        body(&mut next, "new", 3.0);

        let mut undo: UndoStack<ModelDoc> = UndoStack::default();
        apply_model_preserving_ids(&mut undo, &mut model, next, "Replay");
        assert_eq!(undo.undo_count(), 1);
        assert!(
            (model.geometry[a].shape.volume() - 8000.0).abs() < 1e-6,
            "geometri baru, id lama"
        );
        assert_eq!(model.doc.bodies[a].uuid, uuid_a, "uuid dipertahankan");
        assert!(!model.doc.bodies.contains_key(gone));
        assert!(model.doc.bodies.values().any(|b| b.name == "new"));
        undo.undo(&mut model);
        assert!((model.geometry[a].shape.volume() - 1000.0).abs() < 1e-6);
        assert_eq!(model.doc.bodies.len(), 2);
    }

    #[test]
    fn replay_keeps_gui_material_and_mechanical_unless_op_sets_mechanical() {
        let mut model = ModelDoc::default();
        let a = body(&mut model, "a", 10.0);
        let custom = ducad_core::Material::polished_chrome(Some([1.0, 0.0, 0.0, 1.0]));
        model.doc.bodies[a].material = custom;
        let gui_mech = Some(ducad_core::MaterialSource::Library("AISI 304".to_string()));
        model.doc.bodies[a].mechanical = gui_mech.clone();

        // Replay tanpa info material: nilai GUI dipertahankan.
        let mut next = ModelDoc::default();
        body(&mut next, "a", 20.0);
        let mut undo: UndoStack<ModelDoc> = UndoStack::default();
        apply_model_preserving_ids(&mut undo, &mut model, next, "Replay");
        assert_eq!(model.doc.bodies[a].material, custom, "material visual GUI dipertahankan");
        assert_eq!(model.doc.bodies[a].mechanical, gui_mech, "mekanik GUI dipertahankan");

        // Replay yang membawa `set_material` (Some) menang.
        let mut next = ModelDoc::default();
        let na = body(&mut next, "a", 30.0);
        let op_mech = Some(ducad_core::MaterialSource::Library("Al 6061".to_string()));
        next.doc.bodies[na].mechanical = op_mech.clone();
        apply_model_preserving_ids(&mut undo, &mut model, next, "Replay");
        assert_eq!(model.doc.bodies[a].mechanical, op_mech, "mekanik dari op menang");
        assert_eq!(model.doc.bodies[a].material, custom, "material visual tetap");
        undo.undo(&mut model);
        assert_eq!(model.doc.bodies[a].mechanical, gui_mech, "undo mengembalikan mekanik GUI");
    }

    #[test]
    fn swap_is_one_undo_step() {
        let mut model = ModelDoc::default();
        model.doc.add_body("lama".to_string());
        let mut next = ModelDoc::default();
        next.doc.add_body("baru-1".to_string());
        next.doc.add_body("baru-2".to_string());

        let mut undo: UndoStack<ModelDoc> = UndoStack::default();
        undo.execute(Box::new(SwapModelCommand::new(next)), &mut model);
        assert_eq!(model.doc.bodies.len(), 2);
        undo.undo(&mut model);
        assert_eq!(model.doc.bodies.len(), 1);
        undo.redo(&mut model);
        assert_eq!(model.doc.bodies.len(), 2);
    }
}
