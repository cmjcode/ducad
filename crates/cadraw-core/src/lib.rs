//! Model dokumen CADRAW: body, penamaan, dan sistem command undo/redo.
//!
//! Crate ini bebas dependensi GUI/kernel supaya bisa diuji murni.

use slotmap::SlotMap;

slotmap::new_key_type! {
    /// Identitas stabil sebuah body di dokumen.
    pub struct BodyId;
}

/// Satu solid/body dalam dokumen. Geometri B-rep hidup di cadraw-kernel;
/// di sini hanya metadata + handle.
#[derive(Debug, Clone)]
pub struct Body {
    pub name: String,
    pub visible: bool,
}

/// Dokumen aktif: kumpulan body + status modifikasi.
#[derive(Debug, Default)]
pub struct Document {
    pub bodies: SlotMap<BodyId, Body>,
    pub dirty: bool,
}

impl Document {
    pub fn add_body(&mut self, name: impl Into<String>) -> BodyId {
        self.dirty = true;
        self.bodies.insert(Body {
            name: name.into(),
            visible: true,
        })
    }
}

/// Operasi yang bisa di-undo terhadap target `T` — mis. `Document` (body 3D)
/// atau `Sketch` di cadraw-sketch (entitas 2D). Generik sejak awal supaya
/// setiap lapisan dokumen (sketch, body, nanti assembly) dapat undo/redo
/// yang sama tanpa retrofit; semua mutasi WAJIB lewat trait ini.
pub trait Command<T> {
    fn name(&self) -> &str;
    fn apply(&mut self, target: &mut T);
    fn revert(&mut self, target: &mut T);
}

/// Tumpukan undo/redo klasik, generik atas target `T`.
pub struct UndoStack<T> {
    undo: Vec<Box<dyn Command<T>>>,
    redo: Vec<Box<dyn Command<T>>>,
}

// Impl manual (bukan #[derive(Default)]) agar tidak menambahkan bound
// keliru `T: Default` — Vec::new() tidak butuh itu.
impl<T> Default for UndoStack<T> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl<T> UndoStack<T> {
    pub fn execute(&mut self, mut cmd: Box<dyn Command<T>>, target: &mut T) {
        cmd.apply(target);
        self.undo.push(cmd);
        self.redo.clear();
    }

    pub fn undo(&mut self, target: &mut T) -> Option<&str> {
        let mut cmd = self.undo.pop()?;
        cmd.revert(target);
        self.redo.push(cmd);
        self.redo.last().map(|c| c.name())
    }

    pub fn redo(&mut self, target: &mut T) -> Option<&str> {
        let mut cmd = self.redo.pop()?;
        cmd.apply(target);
        self.undo.push(cmd);
        self.undo.last().map(|c| c.name())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AddBox {
        id: Option<BodyId>,
    }

    impl Command<Document> for AddBox {
        fn name(&self) -> &str {
            "Add Box"
        }
        fn apply(&mut self, doc: &mut Document) {
            self.id = Some(doc.add_body("Box"));
        }
        fn revert(&mut self, doc: &mut Document) {
            if let Some(id) = self.id.take() {
                doc.bodies.remove(id);
            }
        }
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut doc = Document::default();
        let mut stack = UndoStack::default();
        stack.execute(Box::new(AddBox { id: None }), &mut doc);
        assert_eq!(doc.bodies.len(), 1);
        stack.undo(&mut doc);
        assert_eq!(doc.bodies.len(), 0);
        stack.redo(&mut doc);
        assert_eq!(doc.bodies.len(), 1);
    }
}
