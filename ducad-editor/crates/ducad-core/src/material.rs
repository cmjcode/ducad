//! Material body: tampilan PBR (`Material`) dan sifat mekanik
//! (`MechanicalProperties`, P16). Keduanya terpisah — satu body menyimpan
//! keduanya — karena warna/kilap tidak menentukan kekuatan bahan.

use serde::{Deserialize, Serialize};

/// Sifat mekanik dan termal bahan isotropik; masukan properti massa (P16)
/// dan FEA (P17–P18).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MechanicalProperties {
    pub density_g_cm3: f64,
    pub young_modulus_gpa: f64,
    pub poisson_ratio: f64,
    pub yield_strength_mpa: f64,
    pub ultimate_strength_mpa: f64,
    pub thermal_expansion_per_k: f64,
    pub thermal_conductivity_w_mk: f64,
}

impl MechanicalProperties {
    /// Tolak nilai yang secara fisik tidak mungkin (masukan pengguna/agent).
    pub fn validate(&self) -> Result<(), String> {
        let positive = [
            ("density_g_cm3", self.density_g_cm3),
            ("young_modulus_gpa", self.young_modulus_gpa),
            ("yield_strength_mpa", self.yield_strength_mpa),
            ("ultimate_strength_mpa", self.ultimate_strength_mpa),
        ];
        for (name, v) in positive {
            if !v.is_finite() || v <= 0.0 {
                return Err(format!("{name} harus > 0 (dapat {v})"));
            }
        }
        if !(self.poisson_ratio > 0.0 && self.poisson_ratio < 0.5) {
            return Err(format!(
                "poisson_ratio harus di antara 0 dan 0.5 (dapat {})",
                self.poisson_ratio
            ));
        }
        if self.yield_strength_mpa > self.ultimate_strength_mpa {
            return Err(format!(
                "yield_strength_mpa ({}) tidak boleh melebihi ultimate_strength_mpa ({})",
                self.yield_strength_mpa, self.ultimate_strength_mpa
            ));
        }
        for (name, v) in [
            ("thermal_expansion_per_k", self.thermal_expansion_per_k),
            ("thermal_conductivity_w_mk", self.thermal_conductivity_w_mk),
        ] {
            if !v.is_finite() || v < 0.0 {
                return Err(format!("{name} harus >= 0 (dapat {v})"));
            }
        }
        Ok(())
    }
}

/// Satu entri pustaka material bawaan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LibraryMaterial {
    /// Kunci stabil yang ditulis ke oplog (mis. `"al_6061_t6"`).
    pub key: &'static str,
    /// Nama tampilan.
    pub name: &'static str,
    pub props: MechanicalProperties,
}

const fn entry(key: &'static str, name: &'static str, v: [f64; 7]) -> LibraryMaterial {
    LibraryMaterial {
        key,
        name,
        props: MechanicalProperties {
            density_g_cm3: v[0],
            young_modulus_gpa: v[1],
            poisson_ratio: v[2],
            yield_strength_mpa: v[3],
            ultimate_strength_mpa: v[4],
            thermal_expansion_per_k: v[5],
            thermal_conductivity_w_mk: v[6],
        },
    }
}

/// Pustaka bawaan. Nilai = angka referensi umum pada suhu ruang (tipikal
/// lembar data pemasok / ASM Handbook vol. 1–2 / EN 10025-2 untuk baja
/// struktural / EN 573 & ASTM B209 untuk aluminium), BUKAN nilai minimum
/// terjamin sebuah sertifikat bahan — untuk perhitungan yang mengikat,
/// pakai material kustom dari sertifikat pemasok.
/// Kolom: ρ g/cm³, E GPa, ν, σ_y MPa, σ_ult MPa, α 1/K, k W/(m·K).
/// Kaca bersifat getas: σ_y diisi sama dengan kuat tarik desainnya.
static LIBRARY: [LibraryMaterial; 13] = [
    entry("abs", "ABS", [1.05, 2.3, 0.35, 40.0, 45.0, 90e-6, 0.17]),
    entry(
        "pa6",
        "PA6 (Nilon 6)",
        [1.14, 2.8, 0.39, 70.0, 80.0, 80e-6, 0.25],
    ),
    entry(
        "pc",
        "Polikarbonat",
        [1.20, 2.4, 0.37, 62.0, 68.0, 65e-6, 0.20],
    ),
    entry(
        "al_6061_t6",
        "Aluminium 6061-T6",
        [2.70, 68.9, 0.33, 276.0, 310.0, 23.6e-6, 167.0],
    ),
    entry(
        "al_7075_t6",
        "Aluminium 7075-T6",
        [2.81, 71.7, 0.33, 503.0, 572.0, 23.4e-6, 130.0],
    ),
    entry(
        "s235",
        "Baja S235",
        [7.85, 210.0, 0.30, 235.0, 360.0, 12e-6, 50.0],
    ),
    entry(
        "s355",
        "Baja S355",
        [7.85, 210.0, 0.30, 355.0, 510.0, 12e-6, 50.0],
    ),
    entry(
        "aisi_304",
        "Baja tahan karat AISI 304",
        [8.00, 193.0, 0.29, 215.0, 505.0, 17.3e-6, 16.2],
    ),
    entry(
        "aisi_1045",
        "Baja AISI 1045",
        [7.85, 205.0, 0.29, 310.0, 565.0, 11.5e-6, 49.8],
    ),
    entry(
        "ti_6al_4v",
        "Titanium Ti-6Al-4V",
        [4.43, 113.8, 0.342, 880.0, 950.0, 8.6e-6, 6.7],
    ),
    entry(
        "brass",
        "Kuningan CuZn37",
        [8.44, 110.0, 0.34, 200.0, 400.0, 20e-6, 120.0],
    ),
    entry(
        "copper",
        "Tembaga C11000",
        [8.94, 117.0, 0.34, 70.0, 220.0, 17e-6, 391.0],
    ),
    entry(
        "soda_lime_glass",
        "Kaca soda-lime",
        [2.50, 70.0, 0.22, 40.0, 40.0, 9e-6, 1.0],
    ),
];

/// Seluruh pustaka material bawaan, urut tetap.
pub fn material_library() -> &'static [LibraryMaterial] {
    &LIBRARY
}

/// Cari entri pustaka bawaan lewat kuncinya (tanpa beda huruf besar/kecil).
pub fn library_material(key: &str) -> Option<&'static LibraryMaterial> {
    LIBRARY.iter().find(|m| m.key.eq_ignore_ascii_case(key))
}

/// Dari mana sifat mekanik sebuah body berasal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialSource {
    /// Hanya densitas perkiraan dari preset visual (jalur kompatibel lama);
    /// tanpa E/ν, jadi tidak bisa disimulasikan.
    Preset(MaterialPreset),
    /// Kunci pustaka: pustaka kustom berkas dulu, lalu pustaka bawaan.
    Library(String),
    /// Nilai langsung dari pengguna.
    Custom(MechanicalProperties),
}

impl MaterialSource {
    /// Sifat mekanik lengkap; `None` untuk `Preset` dan kunci tak dikenal.
    pub fn resolve(
        &self,
        custom: &[(String, MechanicalProperties)],
    ) -> Option<MechanicalProperties> {
        match self {
            MaterialSource::Preset(_) => None,
            MaterialSource::Library(key) => custom
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, p)| *p)
                .or_else(|| library_material(key).map(|m| m.props)),
            MaterialSource::Custom(p) => Some(*p),
        }
    }

    pub fn density_g_cm3(&self, custom: &[(String, MechanicalProperties)]) -> Option<f64> {
        match self {
            MaterialSource::Preset(p) => p.density_g_cm3(),
            other => other.resolve(custom).map(|p| p.density_g_cm3),
        }
    }

    /// Label ringkas untuk laporan (`"al_6061_t6"`, `"custom"`, …).
    pub fn label(&self) -> String {
        match self {
            MaterialSource::Preset(p) => format!("preset:{p:?}"),
            MaterialSource::Library(key) => key.clone(),
            MaterialSource::Custom(_) => "custom".to_string(),
        }
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

    /// Densitas perkiraan (g/cm³) untuk check massa: plastik 1,20;
    /// aluminium 2,70; krom/baja 7,85; kaca 2,50. `Custom` tidak diketahui.
    pub fn density_g_cm3(self) -> Option<f64> {
        match self {
            MaterialPreset::MattePlastic | MaterialPreset::GlossyPlastic => Some(1.20),
            MaterialPreset::AnodizedAluminum => Some(2.70),
            MaterialPreset::PolishedChrome => Some(7.85),
            MaterialPreset::TranslucentGlass => Some(2.50),
            MaterialPreset::Custom => None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_library_entries_are_physical() {
        assert_eq!(material_library().len(), 13);
        for m in material_library() {
            let p = m.props;
            assert!(p.young_modulus_gpa > 0.0, "{}", m.key);
            assert!(p.poisson_ratio > 0.0 && p.poisson_ratio < 0.5, "{}", m.key);
            assert!(p.yield_strength_mpa <= p.ultimate_strength_mpa, "{}", m.key);
            p.validate().unwrap_or_else(|e| panic!("{}: {e}", m.key));
            assert!(m.key.is_ascii() && !m.key.contains(' '), "{}", m.key);
        }
        let mut keys: Vec<_> = material_library().iter().map(|m| m.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 13, "kunci pustaka harus unik");
    }

    #[test]
    fn material_source_resolves_custom_before_builtin() {
        let mut steel = library_material("S235").unwrap().props;
        assert_eq!(steel.density_g_cm3, 7.85);
        steel.density_g_cm3 = 7.7;
        let custom = vec![("s235".to_string(), steel)];
        let src = MaterialSource::Library("s235".into());
        assert_eq!(src.density_g_cm3(&custom), Some(7.7));
        assert_eq!(src.density_g_cm3(&[]), Some(7.85));
        assert_eq!(
            MaterialSource::Library("unobtainium".into()).resolve(&[]),
            None
        );
        let preset = MaterialSource::Preset(MaterialPreset::AnodizedAluminum);
        assert_eq!(preset.resolve(&[]), None);
        assert_eq!(preset.density_g_cm3(&[]), Some(2.70));
    }

    #[test]
    fn material_validate_rejects_unphysical_values() {
        let ok = library_material("abs").unwrap().props;
        assert!(MechanicalProperties {
            poisson_ratio: 0.5,
            ..ok
        }
        .validate()
        .is_err());
        assert!(MechanicalProperties {
            young_modulus_gpa: 0.0,
            ..ok
        }
        .validate()
        .is_err());
        assert!(MechanicalProperties {
            yield_strength_mpa: 99.0,
            ..ok
        }
        .validate()
        .is_err());
        assert!(MechanicalProperties {
            density_g_cm3: f64::NAN,
            ..ok
        }
        .validate()
        .is_err());
    }

    #[test]
    fn material_body_without_mechanical_field_still_loads() {
        // `.ducad` lama: body tanpa field `mechanical`.
        let old = r#"{"name":"B","visible":true,"uuid":"u-1"}"#;
        let body: crate::Body = serde_json::from_str(old).unwrap();
        assert_eq!(body.mechanical, None);
        let doc = crate::Document::default();
        assert_eq!(doc.density_of(&body), Some(1.20));
        assert_eq!(doc.mechanical_of(&body), None);

        let with = crate::Body {
            mechanical: Some(MaterialSource::Library("al_6061_t6".into())),
            ..body
        };
        let json = serde_json::to_string(&with).unwrap();
        assert!(
            json.contains(r#""mechanical":{"library":"al_6061_t6"}"#),
            "{json}"
        );
        let back: crate::Body = serde_json::from_str(&json).unwrap();
        assert_eq!(doc.density_of(&back), Some(2.70));
        assert_eq!(doc.mechanical_of(&back).unwrap().young_modulus_gpa, 68.9);
    }
}
