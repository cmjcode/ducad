//! Model dokumen DUCAD: body, penamaan, dan sistem command undo/redo.
//!
//! Crate ini bebas dependensi GUI/kernel supaya bisa diuji murni.

use serde::{Deserialize, Serialize};
use slotmap::SlotMap;

pub mod hole;
pub use hole::{HoleKind, HoleSpec, IsoMetricThread};

pub mod parametric;
pub use parametric::{
    FeatureId, FeatureNode, FeaturePayload, FeatureStatus, ParametricDag, SketchPlaneRef,
};

pub mod assembly;
pub use assembly::{
    AssemblyInstance, AssemblyInstanceId, AssemblyTree, ClashItem, ClashReport, DegreesOfFreedom,
    MateConstraint, MateConstraintId, MateKind, MateStatus, MateTarget, MateTargetKind,
    SubAssembly, SubAssemblyId,
};

/// Satuan ukuran panjang yang didukung untuk tampilan dan input dimensi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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

/// Preset material standar untuk desain industri dan presentasi CMF (Color, Material, Finish).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialPreset {
    /// Plastik matte bertekstur (ABS / PC) — sebaran difus lembut bebas silau.
    MattePlastic,
    /// Plastik licin berkilau tinggi — pantulan specular tajam dengan lapisan clearcoat mengkilap.
    GlossyPlastic,
    /// Aluminium anodisasi sikat / satin — karakter metalik tinggi dengan pantulan satin elegan.
    AnodizedAluminum,
    /// Krom poles cermin / stainless steel — refleksi metalik penuh mengkilap dan kontras tinggi.
    PolishedChrome,
    /// Kaca tembus pandang / akrilik jernih — transparansi alpha blending dengan efek pendaran tepi Fresnel.
    TranslucentGlass,
    /// Nilai parameter kustom dari pengguna.
    Custom,
}

impl MaterialPreset {
    pub fn all() -> &'static [MaterialPreset] {
        &[
            MaterialPreset::MattePlastic,
            MaterialPreset::GlossyPlastic,
            MaterialPreset::AnodizedAluminum,
            MaterialPreset::PolishedChrome,
            MaterialPreset::TranslucentGlass,
            MaterialPreset::Custom,
        ]
    }
}

/// Definisi material fisik (PBR - Physically-Based Rendering) untuk sebuah solid body 3D.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub preset: MaterialPreset,
    /// Warna dasar Albedo RGBA (termasuk alpha / opacity untuk material tembus pandang seperti kaca).
    pub base_color: [f32; 4],
    /// Kekasaran permukaan (0.0 = cermin licin / glossy, 1.0 = difus kasar / matte).
    pub roughness: f32,
    /// Tingkat metalisitas (0.0 = dielektrik / plastik / kaca, 1.0 = metal / chrome / aluminium).
    pub metallic: f32,
    /// Lapisan kilau bening tambahan (clearcoat layer, 0.0 s/d 1.0).
    pub clearcoat: f32,
}

impl Default for Material {
    fn default() -> Self {
        Self::matte_plastic(Some([0.62, 0.68, 0.76, 1.0]))
    }
}

impl Material {
    /// Buat material Plastik Matte (ABS/PC).
    pub fn matte_plastic(color: Option<[f32; 4]>) -> Self {
        Self {
            preset: MaterialPreset::MattePlastic,
            base_color: color.unwrap_or([0.22, 0.24, 0.27, 1.0]), // Charcoal ABS
            roughness: 0.75,
            metallic: 0.0,
            clearcoat: 0.0,
        }
    }

    /// Buat material Plastik Glossy / Licin.
    pub fn glossy_plastic(color: Option<[f32; 4]>) -> Self {
        Self {
            preset: MaterialPreset::GlossyPlastic,
            base_color: color.unwrap_or([0.96, 0.38, 0.12, 1.0]), // Vibrant Industrial Orange
            roughness: 0.10,
            metallic: 0.0,
            clearcoat: 0.90,
        }
    }

    /// Buat material Aluminium Anodisasi Satin / Brushed.
    pub fn anodized_aluminum(color: Option<[f32; 4]>) -> Self {
        Self {
            preset: MaterialPreset::AnodizedAluminum,
            base_color: color.unwrap_or([0.72, 0.75, 0.80, 1.0]), // Space Gray Aluminum
            roughness: 0.32,
            metallic: 0.95,
            clearcoat: 0.10,
        }
    }

    /// Buat material Polished Chrome / Stainless Steel.
    pub fn polished_chrome(color: Option<[f32; 4]>) -> Self {
        Self {
            preset: MaterialPreset::PolishedChrome,
            base_color: color.unwrap_or([0.92, 0.94, 0.96, 1.0]), // Mirror Chrome
            roughness: 0.03,
            metallic: 1.0,
            clearcoat: 0.0,
        }
    }

    /// Buat material Kaca Transparan / Clear Acrylic.
    pub fn translucent_glass(color: Option<[f32; 4]>) -> Self {
        Self {
            preset: MaterialPreset::TranslucentGlass,
            base_color: color.unwrap_or([0.75, 0.88, 0.96, 0.38]), // Clear Ice Blue
            roughness: 0.08,
            metallic: 0.0,
            clearcoat: 1.0,
        }
    }

    /// Apakah material ini tembus pandang (alpha < 0.99).
    pub fn is_translucent(&self) -> bool {
        self.base_color[3] < 0.99
    }
}

slotmap::new_key_type! {
    /// Identitas stabil sebuah body di dokumen.
    pub struct BodyId;
}

/// Satu solid/body dalam dokumen. Geometri B-rep hidup di ducad-kernel;
/// di sini hanya metadata + handle + material PBR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub name: String,
    pub visible: bool,
    #[serde(default)]
    pub material: Material,
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
        self.add_body_with_material(name, Material::default())
    }

    pub fn add_body_with_material(&mut self, name: impl Into<String>, material: Material) -> BodyId {
        self.dirty = true;
        self.bodies.insert(Body {
            name: name.into(),
            visible: true,
            material,
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

    pub fn undo_count(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo.len()
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

    #[test]
    fn material_presets_and_properties() {
        let matte = Material::matte_plastic(None);
        assert_eq!(matte.preset, MaterialPreset::MattePlastic);
        assert!(!matte.is_translucent());
        assert!(matte.roughness > 0.5);

        let glossy = Material::glossy_plastic(None);
        assert_eq!(glossy.preset, MaterialPreset::GlossyPlastic);
        assert!(glossy.roughness < 0.2);
        assert!(glossy.clearcoat > 0.5);

        let alu = Material::anodized_aluminum(None);
        assert_eq!(alu.preset, MaterialPreset::AnodizedAluminum);
        assert!(alu.metallic > 0.9);

        let chrome = Material::polished_chrome(None);
        assert_eq!(chrome.preset, MaterialPreset::PolishedChrome);
        assert_eq!(chrome.metallic, 1.0);
        assert!(chrome.roughness < 0.05);

        let glass = Material::translucent_glass(None);
        assert_eq!(glass.preset, MaterialPreset::TranslucentGlass);
        assert!(glass.is_translucent());
        assert!(glass.base_color[3] < 1.0);
    }
}
