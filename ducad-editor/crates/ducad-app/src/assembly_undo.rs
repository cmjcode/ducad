//! Undo/redo edit perakitan (hide, ground, mate/joint, hapus instance).
//!
//! `assembly_tree` hidup di `DuCADApp`, bukan di `ModelDoc`, jadi tidak
//! tersentuh `model_undo`; solver juga menulis geometri body langsung.
//! Satu langkah perakitan dicatat sebagai [`AssemblyStepCommand`]: pohon
//! sebelum/sesudah (dipulihkan lewat slot, pola `OplogStateCommand`),
//! visibilitas body, dan geometri yang digerakkan solver (ditukar seperti
//! `ReplaceGeometryCommand`). Op `transform` `place<n>` yang ditulis
//! `note_rigid_move` ikut langkah yang sama lewat `begin_oplog_step`.

use std::cell::RefCell;
use std::rc::Rc;

use ducad_core::{AssemblyTree, BodyId, Command};
use ducad_engine::model::{BodyGeometry, ModelDoc};

use crate::app::DuCADApp;
use crate::oplog_undo::{OplogMark, OplogStateCommand};

/// Slot tempat command menaruh pohon perakitan yang harus dipulihkan.
pub type AssemblySlot = Rc<RefCell<Option<AssemblyTree>>>;

/// Satu langkah undo edit perakitan.
pub struct AssemblyStepCommand {
    label: String,
    before: AssemblyTree,
    after: AssemblyTree,
    /// `(body, terlihat sebelum, terlihat sesudah)`.
    visible: Vec<(BodyId, bool, bool)>,
    /// Geometri yang ditukar saat undo/redo (awalnya: keadaan SEBELUM).
    geometry: Vec<(BodyId, Option<BodyGeometry>)>,
    slot: AssemblySlot,
    /// Keadaan oplog (op `place` dari solver) yang menyertai langkah ini.
    /// Dibawa di sini, bukan ditempel ke puncak undo, supaya drag beruntun
    /// tetap tergabung (`attach_to_top` memutus penggabungan).
    oplog: Option<Box<OplogStateCommand>>,
    /// Keadaan sudah berlaku saat dicatat: `apply` pertama dilewati.
    skip_apply: bool,
    coalesce: Option<u64>,
}

impl AssemblyStepCommand {
    fn swap_geometry(&mut self, model: &mut ModelDoc) {
        for (id, slot) in self.geometry.iter_mut() {
            if !model.doc.bodies.contains_key(*id) {
                continue;
            }
            if let Some(incoming) = slot.take() {
                *slot = model.geometry.insert(*id, incoming);
            }
        }
    }

    fn set_visible(&self, model: &mut ModelDoc, after: bool) {
        for (id, b, a) in &self.visible {
            if let Some(body) = model.doc.bodies.get_mut(*id) {
                body.visible = if after { *a } else { *b };
            }
        }
    }
}

impl Command<ModelDoc> for AssemblyStepCommand {
    fn name(&self) -> &str {
        &self.label
    }
    fn apply(&mut self, model: &mut ModelDoc) {
        if std::mem::take(&mut self.skip_apply) {
            return;
        }
        self.swap_geometry(model);
        self.set_visible(model, true);
        *self.slot.borrow_mut() = Some(self.after.clone());
        if let Some(o) = self.oplog.as_mut() {
            o.apply(model);
        }
    }
    fn revert(&mut self, model: &mut ModelDoc) {
        self.swap_geometry(model);
        self.set_visible(model, false);
        *self.slot.borrow_mut() = Some(self.before.clone());
        if let Some(o) = self.oplog.as_mut() {
            o.revert(model);
        }
    }
    fn coalesce_key(&self) -> Option<(&'static str, u64)> {
        self.coalesce.map(|k| ("assembly_step", k))
    }
}

/// Penanda awal langkah perakitan.
pub(crate) struct AssemblyMark {
    tree: AssemblyTree,
    visible: Vec<(BodyId, bool)>,
    oplog: Option<OplogMark>,
    /// Ada langkah lain yang masih terbuka: langkah ini menumpang padanya.
    nested: bool,
    coalesce: Option<u64>,
}

impl DuCADApp {
    /// Mulai langkah perakitan. Langkah bersarang menumpang langkah luar.
    pub(crate) fn begin_assembly_step(&mut self) -> AssemblyMark {
        let nested = self.assembly_geo_log.is_some();
        if !nested {
            self.assembly_geo_log = Some(Vec::new());
        }
        AssemblyMark {
            tree: self.assembly_tree.clone(),
            visible: self.model.doc.bodies.iter().map(|(id, b)| (id, b.visible)).collect(),
            oplog: (!nested).then(|| self.begin_oplog_step()),
            nested,
            coalesce: None,
        }
    }

    /// Seperti [`Self::begin_assembly_step`], tetapi langkah berurutan
    /// dengan `key` sama (drag nilai joint/mate) digabung jadi satu undo.
    pub(crate) fn begin_assembly_step_coalesced(&mut self, key: u64) -> AssemblyMark {
        let mut m = self.begin_assembly_step();
        m.coalesce = Some(key);
        m
    }

    /// Ganti geometri body oleh solver perakitan; geometri sebelumnya
    /// dicatat untuk undo bila ada langkah terbuka.
    pub(crate) fn set_assembly_geometry(&mut self, id: BodyId, geo: BodyGeometry) {
        let prev = self.model.geometry.insert(id, geo);
        if let (Some(prev), Some(log)) = (prev, self.assembly_geo_log.as_mut()) {
            if !log.iter().any(|(b, _)| *b == id) {
                log.push((id, prev));
            }
        }
    }

    /// Selesaikan langkah: catat satu langkah undo bila pohon, visibilitas,
    /// atau geometri berubah.
    pub(crate) fn end_assembly_step(&mut self, mark: AssemblyMark, label: &str) {
        if mark.nested {
            return;
        }
        let geometry = self.assembly_geo_log.take().unwrap_or_default();
        let visible: Vec<(BodyId, bool, bool)> = mark
            .visible
            .iter()
            .filter_map(|(id, before)| {
                let now = self.model.doc.bodies.get(*id)?.visible;
                (now != *before).then_some((*id, *before, now))
            })
            .collect();
        let oplog = mark.oplog.and_then(|m| self.oplog_step_command(m, label));
        let changed = self.assembly_tree != mark.tree
            || !visible.is_empty()
            || !geometry.is_empty()
            || oplog.is_some();
        if changed {
            let cmd = AssemblyStepCommand {
                label: label.to_string(),
                before: mark.tree,
                after: self.assembly_tree.clone(),
                visible,
                geometry: geometry.into_iter().map(|(id, g)| (id, Some(g))).collect(),
                slot: self.assembly_slot.clone(),
                oplog,
                skip_apply: true,
                coalesce: mark.coalesce,
            };
            self.model_undo.execute(Box::new(cmd), &mut self.model);
            self.model.doc.dirty = true;
        }
    }

    /// Pulihkan pohon perakitan yang ditaruh command saat undo/redo.
    pub(crate) fn take_restored_assembly(&mut self) {
        if let Some(tree) = self.assembly_slot.borrow_mut().take() {
            self.assembly_tree = tree;
        }
    }
}
