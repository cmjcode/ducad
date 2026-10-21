//! Tier lisensi dan fitur yang dibatasinya.
//!
//! Satu sumber kebenaran untuk pertanyaan "boleh tidak?" yang dipakai GUI
//! (`ducad-app`), jembatan agent, dan server MCP. Crate ini tidak tahu dari
//! mana tier berasal — host yang mengisinya (akun login, atau `Free` bila
//! tidak login).
//!
//! Catatan: DUCAD berlisensi AGPL, jadi pembatasan di klien hanya menjaga
//! build resmi. Yang benar-benar ditegakkan adalah kuota Cloud Sync di
//! server.

use serde::{Deserialize, Serialize};

/// Tier lisensi akun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LicenseTier {
    /// Tanpa login, atau akun tanpa langganan.
    #[default]
    Free,
    Pro,
}

impl LicenseTier {
    /// Nilai dari server (`users.license_tier`). Tidak dikenal → `Free`:
    /// pembatasan harus gagal menutup, bukan gagal membuka.
    pub fn parse(s: &str) -> Self {
        if s.trim().eq_ignore_ascii_case("pro") {
            Self::Pro
        } else {
            Self::Free
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Pro => "Pro",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Free => 0,
            Self::Pro => 1,
        }
    }

    pub fn allows(self, feature: Feature) -> bool {
        self.rank() >= feature.required_tier().rank()
    }

    /// `Ok(())` bila diizinkan, selain itu pesan yang siap ditampilkan.
    pub fn check(self, feature: Feature) -> Result<(), String> {
        if self.allows(feature) {
            Ok(())
        } else {
            Err(feature.locked_message())
        }
    }
}

/// Fitur yang dibatasi tier. Fitur yang tidak tercantum di sini bebas untuk
/// semua orang (modeling, FEA, STL/OBJ/GLB/PNG/SVG, simpan `.ducad`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    /// `build_package` dan `submit_for_review`.
    DesignPackage,
    ExportStep,
    ExportIges,
    Export3mf,
    ExportDxf,
    ExportPdfDrawing,
}

impl Feature {
    pub const ALL: [Feature; 6] = [
        Feature::DesignPackage,
        Feature::ExportStep,
        Feature::ExportIges,
        Feature::Export3mf,
        Feature::ExportDxf,
        Feature::ExportPdfDrawing,
    ];

    pub fn required_tier(self) -> LicenseTier {
        LicenseTier::Pro
    }

    /// Nama pendek untuk UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::DesignPackage => "Design Package",
            Self::ExportStep => "Ekspor STEP",
            Self::ExportIges => "Ekspor IGES",
            Self::Export3mf => "Ekspor 3MF",
            Self::ExportDxf => "Ekspor DXF",
            Self::ExportPdfDrawing => "Ekspor PDF gambar teknik",
        }
    }

    /// Pesan saat fitur ditolak. Teksnya juga dibaca agent lewat MCP, jadi
    /// ia menyebut apa yang harus dilakukan pengguna.
    pub fn locked_message(self) -> String {
        let tier = self.required_tier().as_str();
        format!(
            "{} membutuhkan DUCAD {tier}. Masuk dengan akun {tier} lewat menu akun (kanan atas).",
            self.label(),
        )
    }

    /// Fitur yang dibutuhkan untuk mengekspor ke ekstensi berkas `ext`
    /// (dengan/tanpa titik, tak peka huruf besar). `None` = bebas.
    pub fn for_export_extension(ext: &str) -> Option<Feature> {
        match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
            "step" | "stp" => Some(Self::ExportStep),
            "iges" | "igs" => Some(Self::ExportIges),
            "3mf" => Some(Self::Export3mf),
            "dxf" => Some(Self::ExportDxf),
            "pdf" => Some(Self::ExportPdfDrawing),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_is_case_insensitive_and_fails_closed() {
        assert_eq!(LicenseTier::parse("Pro"), LicenseTier::Pro);
        assert_eq!(LicenseTier::parse(" pro "), LicenseTier::Pro);
        assert_eq!(LicenseTier::parse("Free"), LicenseTier::Free);
        assert_eq!(LicenseTier::parse(""), LicenseTier::Free);
        assert_eq!(LicenseTier::parse("Enterprise"), LicenseTier::Free);
    }

    #[test]
    fn free_is_locked_out_of_every_gated_feature_and_pro_is_not() {
        for f in Feature::ALL {
            assert!(!LicenseTier::Free.allows(f), "{f:?}");
            assert!(LicenseTier::Pro.allows(f), "{f:?}");
            assert!(LicenseTier::Free.check(f).is_err());
        }
    }

    #[test]
    fn export_extensions_map_to_features() {
        assert_eq!(Feature::for_export_extension("STEP"), Some(Feature::ExportStep));
        assert_eq!(Feature::for_export_extension(".stp"), Some(Feature::ExportStep));
        assert_eq!(Feature::for_export_extension("igs"), Some(Feature::ExportIges));
        assert_eq!(Feature::for_export_extension("3mf"), Some(Feature::Export3mf));
        assert_eq!(Feature::for_export_extension("dxf"), Some(Feature::ExportDxf));
        assert_eq!(Feature::for_export_extension("pdf"), Some(Feature::ExportPdfDrawing));
        for free in ["stl", "obj", "glb", "png", "svg", "ducad"] {
            assert_eq!(Feature::for_export_extension(free), None, "{free}");
        }
    }

    #[test]
    fn locked_message_names_the_feature_and_tier() {
        let m = Feature::ExportStep.locked_message();
        assert!(m.contains("STEP") && m.contains("Pro"));
    }
}
