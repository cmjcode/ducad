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

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_core::UndoStack;

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
