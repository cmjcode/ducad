//! Model dokumen DUCAD: body, penamaan, dan sistem command undo/redo.
//!
//! Crate ini bebas dependensi GUI/kernel supaya bisa diuji murni.

use serde::{Deserialize, Serialize};
use slotmap::SlotMap;

pub mod hole;
pub use hole::{HoleKind, HoleSpec, IsoMetricThread};

pub mod material;
pub use material::{
    library_material, material_library, LibraryMaterial, Material, MaterialPreset, MaterialSource,
    MechanicalProperties,
};

pub mod configuration;
pub use configuration::{
    design_table_from_csv, design_table_to_csv, valid_configuration_name, Configuration,
    DEFAULT_CONFIGURATION,
};

pub mod sheet_metal;
pub use sheet_metal::{
    bend_allowance, bend_deduction, default_k_factor, BendLine, BendSegment, BendTable, Flange,
    FlatPattern, ReliefKind, SectionSegment, SheetMetalModel, DEFAULT_K_FACTOR,
};

pub mod standard_parts;
pub use standard_parts::{standard_part, StandardKind, StandardPart, StandardShape};

pub mod coupling;
pub use coupling::{AxisLine, Coupling, CouplingKind, ExplodeStep};

pub mod undo;
// `Command`/`UndoStack` dipindah ke modul `undo` (lihat catatan modulnya)
// dan di-re-export di sini supaya seluruh pemanggil lama tidak berubah.
pub use undo::{Command, Transaction, UndoStack};

pub mod parametric;
pub use parametric::{
    FeatureId, FeatureNode, FeaturePayload, FeatureStatus, ParametricDag, SketchPlaneRef,
};

pub mod external;
pub use external::{
    check_source_state, new_part_uuid, resolve_external, resolve_external_with, stable_hash,
    ExternalPartRef, FallbackReason, PartSource, ResolveOutcome, SourceStamp, SourceState,
};

pub mod assembly;
pub use assembly::{
    AssemblyInstance, AssemblyInstanceId, AssemblyTree, ClashItem, ClashReport, DegreesOfFreedom,
    MateConstraint, MateConstraintId, MateKind, MateStatus, MateTarget, MateTargetKind,
    MotionStudy, SubAssembly, SubAssemblyId,
};

pub mod iso286;
pub use iso286::IsoFit;

pub mod drawing_annot;

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
    /// Material mekanik (P16): sumber densitas/E/ν untuk properti massa dan
    /// simulasi. `None` = belum dipilih; densitas jatuh ke preset visual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mechanical: Option<MaterialSource>,
    /// Identitas STABIL body lintas simpan/muat (dirujuk berkas perakitan
    /// lain). Diisi saat body dibuat dan disalin apa adanya dari/ke file
    /// native — dulu app menulis `uuid: None` sehingga nilainya berganti
    /// tiap kali disimpan.
    #[serde(default = "crate::new_part_uuid")]
    pub uuid: String,
}

/// Dokumen aktif: kumpulan body + status modifikasi.
#[derive(Debug, Default)]
pub struct Document {
    pub bodies: SlotMap<BodyId, Body>,
    pub dirty: bool,
    pub unit: LengthUnit,
    /// Material mekanik kustom milik berkas ini (kunci → sifat), dirujuk
    /// `MaterialSource::Library` sebelum pustaka bawaan.
    pub material_library: Vec<(String, MechanicalProperties)>,
}

impl Document {
    pub fn add_body(&mut self, name: impl Into<String>) -> BodyId {
        self.add_body_with_material(name, Material::default())
    }

    pub fn add_body_with_material(
        &mut self,
        name: impl Into<String>,
        material: Material,
    ) -> BodyId {
        self.dirty = true;
        self.bodies.insert(Body {
            name: name.into(),
            visible: true,
            material,
            mechanical: None,
            uuid: crate::new_part_uuid(),
        })
    }

    /// Sifat mekanik body (pustaka kustom berkas didahulukan).
    pub fn mechanical_of(&self, body: &Body) -> Option<MechanicalProperties> {
        body.mechanical
            .as_ref()
            .and_then(|m| m.resolve(&self.material_library))
    }

    /// Densitas body (g/cm³): material mekanik bila ada, selain itu
    /// perkiraan dari preset visual.
    pub fn density_of(&self, body: &Body) -> Option<f64> {
        match &body.mechanical {
            Some(m) => m.density_g_cm3(&self.material_library),
            None => body.material.preset.density_g_cm3(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_density_table() {
        assert_eq!(MaterialPreset::MattePlastic.density_g_cm3(), Some(1.20));
        assert_eq!(MaterialPreset::GlossyPlastic.density_g_cm3(), Some(1.20));
        assert_eq!(MaterialPreset::AnodizedAluminum.density_g_cm3(), Some(2.70));
        assert_eq!(MaterialPreset::PolishedChrome.density_g_cm3(), Some(7.85));
        assert_eq!(MaterialPreset::TranslucentGlass.density_g_cm3(), Some(2.50));
        assert_eq!(MaterialPreset::Custom.density_g_cm3(), None);
    }

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
