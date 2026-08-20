//! Model dokumen DUCAD: body, penamaan, dan sistem command undo/redo.
//!
//! Crate ini bebas dependensi GUI/kernel supaya bisa diuji murni.

use slotmap::SlotMap;

/// Satuan ukuran panjang yang didukung untuk tampilan dan input dimensi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum LengthUnit {
    #[default]
    Millimeters,
    Centimeters,
    Meters,
    Inches,
}

impl LengthUnit {
    pub fn suffix(self) -> &'static str {
        match self {
            LengthUnit::Millimeters => "mm",
            LengthUnit::Centimeters => "cm",
            LengthUnit::Meters => "m",
            LengthUnit::Inches => "in",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LengthUnit::Millimeters => "Milimeter (mm)",
            LengthUnit::Centimeters => "Sentimeter (cm)",
            LengthUnit::Meters => "Meter (m)",
            LengthUnit::Inches => "Inci (in)",
        }
    }

    /// Skala faktor dari mm internal ke satuan ini (misal: 10 mm -> 1 cm = factor 0.1).
    pub fn from_mm_factor(self) -> f64 {
        match self {
            LengthUnit::Millimeters => 1.0,
            LengthUnit::Centimeters => 0.1,
            LengthUnit::Meters => 0.001,
            LengthUnit::Inches => 1.0 / 25.4,
        }
    }

    /// Skala faktor dari satuan ini ke mm internal (misal: 1 cm -> 10 mm = factor 10.0).
    pub fn to_mm_factor(self) -> f64 {
        match self {
            LengthUnit::Millimeters => 1.0,
            LengthUnit::Centimeters => 10.0,
            LengthUnit::Meters => 1000.0,
            LengthUnit::Inches => 25.4,
        }
    }

    /// Konversi nilai internal (mm) ke nilai satuan tampilan.
    pub fn to_display_val(self, val_in_mm: f64) -> f64 {
        val_in_mm * self.from_mm_factor()
    }

    /// Konversi nilai tampilan ke mm internal.
    pub fn to_internal_mm(self, val_in_unit: f64) -> f64 {
        val_in_unit * self.to_mm_factor()
    }

    /// Format angka (dalam mm internal) menjadi string siap tampil dengan suffix satuan (mis. "496.06 mm" atau "49.61 cm").
    pub fn format(self, val_in_mm: f64) -> String {
        let disp = self.to_display_val(val_in_mm);
        if disp.fract().abs() < 1e-4 {
            format!("{:.0} {}", disp, self.suffix())
        } else if (disp * 10.0).fract().abs() < 1e-3 {
            format!("{:.1} {}", disp, self.suffix())
        } else {
            format!("{:.2} {}", disp, self.suffix())
        }
    }

    /// Format dengan presisi tinggi (mis. 4 desimal seperti pada screenshot: 707.1068 mm).
    pub fn format_precise(self, val_in_mm: f64) -> String {
        let disp = self.to_display_val(val_in_mm);
        format!("{:.4} {}", disp, self.suffix())
    }
}

slotmap::new_key_type! {
    /// Identitas stabil sebuah body di dokumen.
    pub struct BodyId;
}

/// Satu solid/body dalam dokumen. Geometri B-rep hidup di ducad-kernel;
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
    pub unit: LengthUnit,
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
/// atau `Sketch` di ducad-sketch (entitas 2D). Generik sejak awal supaya
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
