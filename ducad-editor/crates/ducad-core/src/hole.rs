//! Definisi spesifikasi lubang (Hole Wizard) dan tabel baut standar metrik ISO.
//!
//! Mendukung pembuatan 4 tipe lubang mekanikal standar industri:
//! 1. **Simple Hole**: Lubang silinder lurus (tembus atau berkedalaman).
//! 2. **Counterbore Hole**: Lubang bertingkat untuk kepala baut L (*Socket Head Cap Screw* - ISO 4762).
//! 3. **Countersink Hole**: Lubang tirus 90° untuk baut kepala rata (*Flat Head Screw* - ISO 10642).
//! 4. **Tapped Hole**: Lubang ulir standar metrik (ISO 261 / DIN 13) dengan tap drill diameter.

use serde::{Deserialize, Serialize};

/// Tipe lubang mekanikal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HoleKind {
    #[default]
    Simple,
    Counterbore,
    Countersink,
    Tapped,
}

impl HoleKind {
    pub fn all() -> &'static [HoleKind] {
        &[
            HoleKind::Simple,
            HoleKind::Counterbore,
            HoleKind::Countersink,
            HoleKind::Tapped,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            HoleKind::Simple => "Simple Hole",
            HoleKind::Counterbore => "Counterbore",
            HoleKind::Countersink => "Countersink",
            HoleKind::Tapped => "Tapped Thread",
        }
    }
}

/// Standar ukuran ulir baut metrik ISO (Coarse pitch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum IsoMetricThread {
    M2,
    M2_5,
    #[default]
    M3,
    M4,
    M5,
    M6,
    M8,
    M10,
    M12,
    Custom,
}

impl IsoMetricThread {
    pub fn all() -> &'static [IsoMetricThread] {
        &[
            IsoMetricThread::M2,
            IsoMetricThread::M2_5,
            IsoMetricThread::M3,
            IsoMetricThread::M4,
            IsoMetricThread::M5,
            IsoMetricThread::M6,
            IsoMetricThread::M8,
            IsoMetricThread::M10,
            IsoMetricThread::M12,
            IsoMetricThread::Custom,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            IsoMetricThread::M2 => "M2",
            IsoMetricThread::M2_5 => "M2.5",
            IsoMetricThread::M3 => "M3",
            IsoMetricThread::M4 => "M4",
            IsoMetricThread::M5 => "M5",
            IsoMetricThread::M6 => "M6",
            IsoMetricThread::M8 => "M8",
            IsoMetricThread::M10 => "M10",
            IsoMetricThread::M12 => "M12",
            IsoMetricThread::Custom => "Custom",
        }
    }

    /// Parameter standar dimensi ISO:
    /// - `d_nom`: Diameter nominal ulir (mm)
    /// - `pitch`: Kisar ulir standar kasar (*coarse pitch*, mm)
    /// - `tap_drill_dia`: Diameter mata bor tap ulir (mm)
    /// - `clearance_dia`: Diameter lubang pas baut (*clearance normal*, mm)
    /// - `cbore_dia`: Diameter kepala baut L (*Counterbore*, ISO 4762, mm)
    /// - `cbore_depth`: Kedalaman kepala baut L (*Counterbore depth*, ISO 4762, mm)
    /// - `csink_dia`: Diameter kepala baut rata (*Countersink*, ISO 10642, mm)
    pub fn standard_params(self) -> (f64, f64, f64, f64, f64, f64, f64) {
        match self {
            IsoMetricThread::M2 => (2.0, 0.40, 1.60, 2.4, 4.4, 2.4, 4.4),
            IsoMetricThread::M2_5 => (2.5, 0.45, 2.05, 2.9, 5.0, 2.9, 5.5),
            IsoMetricThread::M3 => (3.0, 0.50, 2.50, 3.4, 6.5, 3.4, 6.7),
            IsoMetricThread::M4 => (4.0, 0.70, 3.30, 4.5, 8.0, 4.4, 8.9),
            IsoMetricThread::M5 => (5.0, 0.80, 4.20, 5.5, 10.0, 5.4, 11.2),
            IsoMetricThread::M6 => (6.0, 1.00, 5.00, 6.6, 11.5, 6.5, 13.4),
            IsoMetricThread::M8 => (8.0, 1.25, 6.80, 9.0, 15.0, 8.6, 17.9),
            IsoMetricThread::M10 => (10.0, 1.50, 8.50, 11.0, 18.0, 10.6, 22.4),
            IsoMetricThread::M12 => (12.0, 1.75, 10.20, 13.5, 20.0, 12.6, 26.8),
            IsoMetricThread::Custom => (6.0, 1.00, 5.00, 6.6, 11.5, 6.5, 13.4),
        }
    }
}

/// Spesifikasi konfigurasi pembuatan lubang (*Hole Wizard Specification*).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HoleSpec {
    pub kind: HoleKind,
    pub thread_size: IsoMetricThread,
    /// Diameter lubang utama (atau tap drill diameter pada tapped hole, atau clearance hole) dalam mm.
    pub diameter: f64,
    /// Kedalaman lubang utama (mm).
    pub depth: f64,
    /// Apakah lubang tembus seluruhnya (*Through All*).
    pub is_through: bool,
    /// Diameter Counterbore ($d_2$ / $D_{cb}$) dalam mm.
    pub counterbore_diameter: f64,
    /// Kedalaman Counterbore ($t$ / $T_{cb}$) dalam mm.
    pub counterbore_depth: f64,
    /// Diameter Countersink ($d_k$ / $D_{cs}$) dalam mm.
    pub countersink_diameter: f64,
    /// Sudut tirus Countersink dalam derajat (standar ISO 90.0°).
    pub countersink_angle_deg: f64,
    /// Pitch ulir metrik untuk Tapped Hole (mm).
    pub thread_pitch: f64,
    /// Kedalaman ulir efektif untuk Tapped Hole (mm).
    pub thread_depth: f64,
    /// Apakah ujung bor berbentuk kerucut 118° standar (false = datar / flat bottom).
    pub has_drill_tip: bool,
}

impl Default for HoleSpec {
    fn default() -> Self {
        Self::for_iso(IsoMetricThread::M6, HoleKind::Counterbore, 20.0)
    }
}

impl HoleSpec {
    /// Buat konfigurasi lubang otomatis terisi parameter standar ISO.
    pub fn for_iso(thread_size: IsoMetricThread, kind: HoleKind, default_depth: f64) -> Self {
        let (_d_nom, pitch, tap_drill, clearance_dia, cbore_dia, cbore_depth, csink_dia) =
            thread_size.standard_params();

        let diameter = match kind {
            HoleKind::Simple => clearance_dia,
            HoleKind::Counterbore => clearance_dia,
            HoleKind::Countersink => clearance_dia,
            HoleKind::Tapped => tap_drill,
        };

        let depth = if default_depth > 0.0 { default_depth } else { 20.0 };
        let thread_depth = (depth - 2.0 * pitch).max(pitch * 2.0);

        Self {
            kind,
            thread_size,
            diameter,
            depth,
            is_through: false,
            counterbore_diameter: cbore_dia,
            counterbore_depth: cbore_depth,
            countersink_diameter: csink_dia,
            countersink_angle_deg: 90.0,
            thread_pitch: pitch,
            thread_depth,
            has_drill_tip: true,
        }
    }

    /// Format deskripsi teknis standar (Callout) untuk gambar kerja atau info UI.
    /// Contoh: `ISO 4762 M6 (Ø6.6 ⌴ Ø11.5 ↧ 6.5 ↧ 20.0)`
    pub fn technical_callout(&self) -> String {
        match self.kind {
            HoleKind::Simple => {
                if self.is_through {
                    format!("Ø{:.1} THRU", self.diameter)
                } else {
                    format!("Ø{:.1} ↧ {:.1}", self.diameter, self.depth)
                }
            }
            HoleKind::Counterbore => {
                if self.is_through {
                    format!(
                        "ISO 4762 {} (Ø{:.1} ⌴ Ø{:.1} ↧ {:.1} THRU)",
                        self.thread_size.label(),
                        self.diameter,
                        self.counterbore_diameter,
                        self.counterbore_depth
                    )
                } else {
                    format!(
                        "ISO 4762 {} (Ø{:.1} ⌴ Ø{:.1} ↧ {:.1} ↧ {:.1})",
                        self.thread_size.label(),
                        self.diameter,
                        self.counterbore_diameter,
                        self.counterbore_depth,
                        self.depth
                    )
                }
            }
            HoleKind::Countersink => {
                if self.is_through {
                    format!(
                        "ISO 10642 {} (Ø{:.1} ⌵ Ø{:.1} x {:.0}° THRU)",
                        self.thread_size.label(),
                        self.diameter,
                        self.countersink_diameter,
                        self.countersink_angle_deg
                    )
                } else {
                    format!(
                        "ISO 10642 {} (Ø{:.1} ⌵ Ø{:.1} x {:.0}° ↧ {:.1})",
                        self.thread_size.label(),
                        self.diameter,
                        self.countersink_diameter,
                        self.countersink_angle_deg,
                        self.depth
                    )
                }
            }
            HoleKind::Tapped => {
                let nom = self.thread_size.standard_params().0;
                if self.is_through {
                    format!(
                        "M{:.0}x{:.2} THRU (Tap Drill Ø{:.2})",
                        nom, self.thread_pitch, self.diameter
                    )
                } else {
                    format!(
                        "M{:.0}x{:.2} ↧ {:.1} (Drill Ø{:.2} ↧ {:.1})",
                        nom, self.thread_pitch, self.thread_depth, self.diameter, self.depth
                    )
                }
            }
        }
    }
}
