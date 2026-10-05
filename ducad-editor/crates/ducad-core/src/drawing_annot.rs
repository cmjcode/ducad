//! Anotasi gambar kerja: toleransi dimensi, GD&T ISO 1101, datum, kekasaran
//! permukaan, tabel lubang, dan tabel revisi.
//!
//! Hanya tipe data + validasi + hitungan murni; penggambaran ke PDF/SVG ada
//! di `ducad-io`. Semua field opsional memakai `#[serde(default)]` supaya
//! berkas lama (tanpa anotasi) tetap terbaca.

use crate::iso286::{self, IsoFit};
use serde::{Deserialize, Serialize};

/// Empat belas karakteristik geometris ISO 1101.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GdtSymbol {
    Straightness,
    Flatness,
    Circularity,
    Cylindricity,
    ProfileOfLine,
    ProfileOfSurface,
    Perpendicularity,
    Angularity,
    Parallelism,
    Position,
    Concentricity,
    Symmetry,
    CircularRunout,
    TotalRunout,
}

/// Golongan karakteristik (menentukan aturan datum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GdtCategory {
    Form,
    Profile,
    Orientation,
    Location,
    Runout,
}

impl GdtSymbol {
    /// Seluruh simbol dalam urutan ISO 1101.
    pub const ALL: [GdtSymbol; 14] = [
        GdtSymbol::Straightness,
        GdtSymbol::Flatness,
        GdtSymbol::Circularity,
        GdtSymbol::Cylindricity,
        GdtSymbol::ProfileOfLine,
        GdtSymbol::ProfileOfSurface,
        GdtSymbol::Perpendicularity,
        GdtSymbol::Angularity,
        GdtSymbol::Parallelism,
        GdtSymbol::Position,
        GdtSymbol::Concentricity,
        GdtSymbol::Symmetry,
        GdtSymbol::CircularRunout,
        GdtSymbol::TotalRunout,
    ];

    /// Nama stabil (sama dengan bentuk serde `snake_case`).
    pub fn name(self) -> &'static str {
        match self {
            GdtSymbol::Straightness => "straightness",
            GdtSymbol::Flatness => "flatness",
            GdtSymbol::Circularity => "circularity",
            GdtSymbol::Cylindricity => "cylindricity",
            GdtSymbol::ProfileOfLine => "profile_of_line",
            GdtSymbol::ProfileOfSurface => "profile_of_surface",
            GdtSymbol::Perpendicularity => "perpendicularity",
            GdtSymbol::Angularity => "angularity",
            GdtSymbol::Parallelism => "parallelism",
            GdtSymbol::Position => "position",
            GdtSymbol::Concentricity => "concentricity",
            GdtSymbol::Symmetry => "symmetry",
            GdtSymbol::CircularRunout => "circular_runout",
            GdtSymbol::TotalRunout => "total_runout",
        }
    }

    pub fn category(self) -> GdtCategory {
        match self {
            GdtSymbol::Straightness
            | GdtSymbol::Flatness
            | GdtSymbol::Circularity
            | GdtSymbol::Cylindricity => GdtCategory::Form,
            GdtSymbol::ProfileOfLine | GdtSymbol::ProfileOfSurface => GdtCategory::Profile,
            GdtSymbol::Perpendicularity | GdtSymbol::Angularity | GdtSymbol::Parallelism => {
                GdtCategory::Orientation
            }
            GdtSymbol::Position | GdtSymbol::Concentricity | GdtSymbol::Symmetry => {
                GdtCategory::Location
            }
            GdtSymbol::CircularRunout | GdtSymbol::TotalRunout => GdtCategory::Runout,
        }
    }

    /// Toleransi bentuk tidak pernah merujuk datum.
    pub fn forbids_datum(self) -> bool {
        self.category() == GdtCategory::Form
    }

    /// Orientasi, konsentrisitas, simetri, dan run-out wajib punya datum.
    /// Profil dan posisi boleh tanpa datum.
    pub fn requires_datum(self) -> bool {
        match self.category() {
            GdtCategory::Orientation | GdtCategory::Runout => true,
            GdtCategory::Location => self != GdtSymbol::Position,
            GdtCategory::Form | GdtCategory::Profile => false,
        }
    }

    /// Zona toleransi silindris (tanda diameter) hanya bermakna untuk
    /// karakteristik yang bisa mengendalikan sumbu.
    pub fn allows_diameter_zone(self) -> bool {
        matches!(
            self,
            GdtSymbol::Straightness
                | GdtSymbol::Perpendicularity
                | GdtSymbol::Angularity
                | GdtSymbol::Parallelism
                | GdtSymbol::Position
                | GdtSymbol::Concentricity
        )
    }

    /// Pengubah kondisi material (M/L) tidak berlaku untuk kebulatan,
    /// kesilindrisan, profil, dan run-out.
    pub fn allows_material_modifier(self) -> bool {
        !matches!(
            self,
            GdtSymbol::Circularity
                | GdtSymbol::Cylindricity
                | GdtSymbol::ProfileOfLine
                | GdtSymbol::ProfileOfSurface
                | GdtSymbol::CircularRunout
                | GdtSymbol::TotalRunout
        )
    }
}

/// Pengubah kondisi material pada nilai toleransi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialModifier {
    /// Kondisi material maksimum (huruf M dalam lingkaran).
    Mmc,
    /// Kondisi material minimum (huruf L dalam lingkaran).
    Lmc,
}

impl MaterialModifier {
    /// Huruf yang ditulis di dalam lingkaran.
    pub fn letter(self) -> &'static str {
        match self {
            MaterialModifier::Mmc => "M",
            MaterialModifier::Lmc => "L",
        }
    }
}

/// Cara toleransi sebuah dimensi dinyatakan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToleranceSpec {
    /// Deviasi eksplisit dalam mm: batas atas = `nominal + plus`, batas
    /// bawah = `nominal - minus`. Keduanya biasanya >= 0; nilai negatif
    /// dipakai bila kedua batas di sisi yang sama (mis. +0.035/+0.022 ->
    /// `plus: 0.035, minus: -0.022`).
    Limits { plus: f64, minus: f64 },
    /// Kelas toleransi ISO 286, mis. `{"fit": "H7"}`.
    Fit(IsoFit),
}

/// Dimensi bertoleransi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionTolerance {
    /// Ukuran nominal, mm.
    pub nominal: f64,
    pub tolerance: ToleranceSpec,
    /// Dimensi diameter: tanda diameter digambar di depan nilai.
    #[serde(default)]
    pub diameter: bool,
}

impl DimensionTolerance {
    /// Dimensi dengan deviasi eksplisit (lihat [`ToleranceSpec::Limits`]).
    pub fn with_limits(nominal: f64, plus: f64, minus: f64) -> Self {
        Self {
            nominal,
            tolerance: ToleranceSpec::Limits { plus, minus },
            diameter: false,
        }
    }

    /// Dimensi dengan kelas ISO 286.
    pub fn with_fit(nominal: f64, fit: IsoFit) -> Self {
        Self {
            nominal,
            tolerance: ToleranceSpec::Fit(fit),
            diameter: false,
        }
    }

    /// Deviasi `(atas, bawah)` bertanda dalam mm terhadap nominal, mis.
    /// `(0.021, 0.0)` untuk 25 H7.
    pub fn deviations(&self) -> Result<(f64, f64), String> {
        if !self.nominal.is_finite() {
            return Err("ukuran nominal dimensi tidak sah".to_string());
        }
        match &self.tolerance {
            ToleranceSpec::Limits { plus, minus } => {
                if !plus.is_finite() || !minus.is_finite() {
                    return Err("deviasi toleransi tidak sah".to_string());
                }
                if plus + minus < 0.0 {
                    return Err(format!(
                        "batas atas (nominal + {plus}) lebih kecil dari batas bawah (nominal - {minus})"
                    ));
                }
                Ok((*plus, -*minus + 0.0))
            }
            ToleranceSpec::Fit(fit) => iso286::limits(self.nominal, fit),
        }
    }

    /// Ukuran batas `(atas, bawah)` dalam mm.
    pub fn limits(&self) -> Result<(f64, f64), String> {
        let (upper, lower) = self.deviations()?;
        Ok((self.nominal + upper, self.nominal + lower))
    }

    /// Mata rantai `(nominal, plus, minus)` untuk [`tolerance_stackup`].
    pub fn stackup_link(&self) -> Result<(f64, f64, f64), String> {
        let (upper, lower) = self.deviations()?;
        Ok((self.nominal, upper, -lower + 0.0))
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.nominal.is_finite() || self.nominal <= 0.0 {
            return Err(format!(
                "ukuran nominal dimensi harus > 0 (dapat {})",
                self.nominal
            ));
        }
        self.deviations().map(|_| ())
    }
}

/// Bingkai kontrol fitur (feature control frame) ISO 1101.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureControlFrame {
    pub symbol: GdtSymbol,
    /// Lebar zona toleransi, mm.
    pub value: f64,
    /// Zona silindris: tanda diameter di depan nilai.
    #[serde(default)]
    pub diameter_zone: bool,
    /// Pengubah kondisi material pada nilai toleransi.
    #[serde(default)]
    pub modifiers: Vec<MaterialModifier>,
    /// Datum acuan berurutan (primer, sekunder, tersier), mis. `["A", "B"]`.
    #[serde(default)]
    pub datums: Vec<String>,
}

impl FeatureControlFrame {
    pub fn validate(&self) -> Result<(), String> {
        let name = self.symbol.name();
        if !self.value.is_finite() || self.value <= 0.0 {
            return Err(format!(
                "nilai toleransi {name} harus > 0 (dapat {})",
                self.value
            ));
        }
        if self.symbol.forbids_datum() && !self.datums.is_empty() {
            return Err(format!("toleransi bentuk {name} tidak boleh merujuk datum"));
        }
        if self.symbol.requires_datum() && self.datums.is_empty() {
            return Err(format!("toleransi {name} butuh minimal satu datum"));
        }
        if self.datums.len() > 3 {
            return Err(format!(
                "bingkai kontrol memuat paling banyak 3 datum (dapat {})",
                self.datums.len()
            ));
        }
        for (i, datum) in self.datums.iter().enumerate() {
            validate_datum_label(datum, true)?;
            if self.datums[..i].contains(datum) {
                return Err(format!("datum '{datum}' dirujuk lebih dari sekali"));
            }
        }
        if self.diameter_zone && !self.symbol.allows_diameter_zone() {
            return Err(format!(
                "zona diameter tidak berlaku untuk toleransi {name}"
            ));
        }
        if !self.modifiers.is_empty() && !self.symbol.allows_material_modifier() {
            return Err(format!(
                "pengubah kondisi material (M/L) tidak berlaku untuk toleransi {name}"
            ));
        }
        if self.modifiers.len() > 1 {
            return Err(
                "hanya satu pengubah kondisi material (M atau L) per nilai toleransi".to_string(),
            );
        }
        Ok(())
    }
}

/// Label datum: 1-2 huruf kapital ASCII. `allow_common` mengizinkan datum
/// bersama berbentuk `A-B` (hanya di dalam bingkai kontrol).
fn validate_datum_label(label: &str, allow_common: bool) -> Result<(), String> {
    let part_ok =
        |part: &str| (1..=2).contains(&part.len()) && part.chars().all(|c| c.is_ascii_uppercase());
    let ok = if allow_common {
        let mut parts = label.split('-');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(a), None, _) => part_ok(a),
            (Some(a), Some(b), None) => part_ok(a) && part_ok(b) && a != b,
            _ => false,
        }
    } else {
        part_ok(label)
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "label datum '{label}' tidak sah (pakai 1-2 huruf kapital, mis. A atau AB)"
        ))
    }
}

/// Penanda fitur datum (segitiga + kotak berhuruf).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatumFeature {
    pub label: String,
}

impl DatumFeature {
    pub fn validate(&self) -> Result<(), String> {
        validate_datum_label(&self.label, false)
    }
}

/// Simbol kekasaran permukaan dengan nilai Ra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceFinish {
    /// Kekasaran rata-rata aritmetis Ra, mikrometer.
    pub ra_um: f64,
}

impl SurfaceFinish {
    pub fn validate(&self) -> Result<(), String> {
        if !self.ra_um.is_finite() || self.ra_um <= 0.0 {
            return Err(format!("kekasaran Ra harus > 0 um (dapat {})", self.ra_um));
        }
        Ok(())
    }
}

/// Satu baris tabel lubang.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoleTableRow {
    /// Tanda lubang di gambar, mis. `A1`.
    pub tag: String,
    /// Posisi pusat terhadap titik acuan tabel, mm.
    pub x: f64,
    pub y: f64,
    /// Diameter, mm.
    pub diameter: f64,
    /// Kedalaman, mm; `None` = tembus.
    #[serde(default)]
    pub depth: Option<f64>,
    /// Keterangan bebas (ulir, counterbore, …).
    #[serde(default)]
    pub note: String,
}

/// Tabel lubang (tag, koordinat, diameter, kedalaman).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HoleTable {
    /// Judul; kosong = judul bawaan.
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub rows: Vec<HoleTableRow>,
}

impl HoleTable {
    pub fn validate(&self) -> Result<(), String> {
        for (i, row) in self.rows.iter().enumerate() {
            if row.tag.trim().is_empty() {
                return Err(format!("baris tabel lubang #{} tidak punya tag", i + 1));
            }
            if self.rows[..i].iter().any(|r| r.tag == row.tag) {
                return Err(format!("tag lubang '{}' dipakai dua kali", row.tag));
            }
            if !row.x.is_finite() || !row.y.is_finite() {
                return Err(format!("posisi lubang '{}' tidak sah", row.tag));
            }
            if !row.diameter.is_finite() || row.diameter <= 0.0 {
                return Err(format!("diameter lubang '{}' harus > 0", row.tag));
            }
            if let Some(depth) = row.depth {
                if !depth.is_finite() || depth <= 0.0 {
                    return Err(format!("kedalaman lubang '{}' harus > 0", row.tag));
                }
            }
        }
        Ok(())
    }
}

/// Satu baris tabel revisi.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RevisionRow {
    /// Kode revisi, mis. `A`, `B`, `01`.
    pub rev: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub by: String,
}

/// Tabel riwayat revisi gambar.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RevisionTable {
    #[serde(default)]
    pub rows: Vec<RevisionRow>,
}

impl RevisionTable {
    pub fn validate(&self) -> Result<(), String> {
        for (i, row) in self.rows.iter().enumerate() {
            if row.rev.trim().is_empty() {
                return Err(format!("baris tabel revisi #{} tidak punya kode", i + 1));
            }
            if self.rows[..i].iter().any(|r| r.rev == row.rev) {
                return Err(format!("kode revisi '{}' dipakai dua kali", row.rev));
            }
        }
        Ok(())
    }
}

/// Satu anotasi pada lembar gambar. `position` = titik jangkar di kertas
/// dalam mm dari pojok kiri-bawah (sama dengan entitas lembar lainnya):
///
/// - toleransi dimensi: awal garis dasar teks;
/// - bingkai kontrol dan tabel: pojok kiri-bawah;
/// - datum: tengah alas segitiga (menempel pada fitur), simbol tumbuh ke atas;
/// - kekasaran: ujung bawah tanda centang (menempel pada permukaan).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Annotation {
    DimensionTolerance {
        position: [f64; 2],
        dimension: DimensionTolerance,
    },
    FeatureControlFrame {
        position: [f64; 2],
        frame: FeatureControlFrame,
    },
    DatumFeature {
        position: [f64; 2],
        datum: DatumFeature,
    },
    SurfaceFinish {
        position: [f64; 2],
        finish: SurfaceFinish,
    },
    HoleTable {
        position: [f64; 2],
        table: HoleTable,
    },
    RevisionTable {
        position: [f64; 2],
        table: RevisionTable,
    },
}

impl Annotation {
    /// Titik jangkar di kertas, mm.
    pub fn position(&self) -> [f64; 2] {
        match self {
            Annotation::DimensionTolerance { position, .. }
            | Annotation::FeatureControlFrame { position, .. }
            | Annotation::DatumFeature { position, .. }
            | Annotation::SurfaceFinish { position, .. }
            | Annotation::HoleTable { position, .. }
            | Annotation::RevisionTable { position, .. } => *position,
        }
    }

    /// Nama jenis (sama dengan tag serde `type`).
    pub fn kind_name(&self) -> &'static str {
        match self {
            Annotation::DimensionTolerance { .. } => "dimension_tolerance",
            Annotation::FeatureControlFrame { .. } => "feature_control_frame",
            Annotation::DatumFeature { .. } => "datum_feature",
            Annotation::SurfaceFinish { .. } => "surface_finish",
            Annotation::HoleTable { .. } => "hole_table",
            Annotation::RevisionTable { .. } => "revision_table",
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let position = self.position();
        if !position[0].is_finite() || !position[1].is_finite() {
            return Err(format!("posisi anotasi {} tidak sah", self.kind_name()));
        }
        match self {
            Annotation::DimensionTolerance { dimension, .. } => dimension.validate(),
            Annotation::FeatureControlFrame { frame, .. } => frame.validate(),
            Annotation::DatumFeature { datum, .. } => datum.validate(),
            Annotation::SurfaceFinish { finish, .. } => finish.validate(),
            Annotation::HoleTable { table, .. } => table.validate(),
            Annotation::RevisionTable { table, .. } => table.validate(),
        }
    }
}

/// Memvalidasi sekumpulan anotasi; pesan error menyebut indeks (mulai 0)
/// anotasi pertama yang salah. Juga menolak label datum ganda.
pub fn validate_annotations(annotations: &[Annotation]) -> Result<(), String> {
    let mut datum_labels: Vec<&str> = Vec::new();
    for (i, annotation) in annotations.iter().enumerate() {
        annotation
            .validate()
            .map_err(|e| format!("anotasi #{i} ({}): {e}", annotation.kind_name()))?;
        if let Annotation::DatumFeature { datum, .. } = annotation {
            if datum_labels.contains(&datum.label.as_str()) {
                return Err(format!(
                    "anotasi #{i} (datum_feature): datum '{}' didefinisikan dua kali",
                    datum.label
                ));
            }
            datum_labels.push(&datum.label);
        }
    }
    Ok(())
}

/// Hasil tumpukan toleransi sebuah rantai dimensi, mm.
///
/// Batas rantai: `nominal + *_plus` (atas) dan `nominal - *_minus` (bawah).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StackupResult {
    /// Jumlah nominal seluruh mata rantai.
    pub nominal: f64,
    /// Kasus terburuk: jumlah seluruh deviasi plus.
    pub worst_case_plus: f64,
    /// Kasus terburuk: jumlah seluruh deviasi minus.
    pub worst_case_minus: f64,
    /// Statistik (root-sum-square) di sisi plus.
    pub rss_plus: f64,
    /// Statistik (root-sum-square) di sisi minus.
    pub rss_minus: f64,
}

impl StackupResult {
    /// Lebar pita kasus terburuk (atas - bawah).
    pub fn worst_case_total(&self) -> f64 {
        self.worst_case_plus + self.worst_case_minus
    }

    /// Lebar pita RSS (atas - bawah).
    pub fn rss_total(&self) -> f64 {
        self.rss_plus + self.rss_minus
    }
}

/// Tumpukan toleransi rantai dimensi: kasus terburuk dan root-sum-square.
///
/// Tiap mata rantai `(nominal, plus, minus)`: batas atas `nominal + plus`,
/// batas bawah `nominal - minus` (konvensi [`ToleranceSpec::Limits`]). Mata
/// rantai yang berlawanan arah ditulis `(-nominal, minus, plus)`.
///
/// RSS memperlakukan toleransi tak simetris dengan menggeser nilai tengah:
/// tiap mata rantai menjadi `tengah ± setengah-lebar`, setengah-lebar
/// dijumlah kuadrat, lalu hasilnya dinyatakan kembali terhadap jumlah
/// nominal. Untuk toleransi simetris ini sama dengan `sqrt(sum(t^2))`.
/// Rantai kosong menghasilkan nol semua.
pub fn tolerance_stackup(chain: &[(f64, f64, f64)]) -> StackupResult {
    let mut nominal = 0.0;
    let mut plus_sum = 0.0;
    let mut minus_sum = 0.0;
    let mut mean_shift = 0.0;
    let mut half_sq = 0.0;
    for (link_nominal, plus, minus) in chain {
        nominal += link_nominal;
        plus_sum += plus;
        minus_sum += minus;
        mean_shift += (plus - minus) / 2.0;
        let half = (plus + minus) / 2.0;
        half_sq += half * half;
    }
    let rss_half = half_sq.sqrt();
    StackupResult {
        nominal,
        worst_case_plus: plus_sum,
        worst_case_minus: minus_sum,
        rss_plus: mean_shift + rss_half,
        rss_minus: rss_half - mean_shift,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fcf(symbol: GdtSymbol, value: f64, datums: &[&str]) -> FeatureControlFrame {
        FeatureControlFrame {
            symbol,
            value,
            diameter_zone: false,
            modifiers: Vec::new(),
            datums: datums.iter().map(|d| d.to_string()).collect(),
        }
    }

    fn parse_fit(class: &str) -> IsoFit {
        IsoFit::parse(class).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn drawing_annot_dimension_resolves_fit_and_limits() {
        let hole = DimensionTolerance::with_fit(25.0, parse_fit("H7"));
        let (upper, lower) = hole.limits().unwrap_or_else(|e| panic!("{e}"));
        assert!((upper - 25.021).abs() < 1e-9 && (lower - 25.0).abs() < 1e-9);

        let shaft = DimensionTolerance::with_fit(25.0, parse_fit("g6"));
        let (upper, lower) = shaft.deviations().unwrap_or_else(|e| panic!("{e}"));
        assert!((upper + 0.007).abs() < 1e-9 && (lower + 0.020).abs() < 1e-9);

        let plain = DimensionTolerance::with_limits(10.0, 0.1, 0.2);
        assert_eq!(plain.deviations(), Ok((0.1, -0.2)));
        let (upper, lower) = plain.limits().unwrap_or_else(|e| panic!("{e}"));
        assert!((upper - 10.1).abs() < 1e-12 && (lower - 9.8).abs() < 1e-12);
        assert_eq!(plain.stackup_link(), Ok((10.0, 0.1, 0.2)));
        assert!(plain.validate().is_ok());

        // Kedua batas di atas nominal (seperti p6).
        let same_side = DimensionTolerance::with_limits(25.0, 0.035, -0.022);
        assert_eq!(same_side.deviations(), Ok((0.035, 0.022)));

        // Batas atas di bawah batas bawah.
        assert!(DimensionTolerance::with_limits(10.0, -0.2, 0.1)
            .validate()
            .is_err());
        assert!(DimensionTolerance::with_limits(0.0, 0.1, 0.1)
            .validate()
            .is_err());
        assert!(DimensionTolerance::with_limits(10.0, f64::NAN, 0.1)
            .validate()
            .is_err());
        // Ukuran di luar tabel ISO 286.
        assert!(DimensionTolerance::with_fit(800.0, parse_fit("H7"))
            .validate()
            .is_err());
    }

    #[test]
    fn drawing_annot_fcf_validation() {
        assert!(fcf(GdtSymbol::Flatness, 0.05, &[]).validate().is_ok());
        assert!(fcf(GdtSymbol::Position, 0.1, &["A", "B", "C"])
            .validate()
            .is_ok());
        assert!(fcf(GdtSymbol::Position, 0.1, &[]).validate().is_ok());
        assert!(fcf(GdtSymbol::TotalRunout, 0.02, &["A-B"])
            .validate()
            .is_ok());

        // Toleransi bentuk tidak menerima datum.
        for symbol in [
            GdtSymbol::Straightness,
            GdtSymbol::Flatness,
            GdtSymbol::Circularity,
            GdtSymbol::Cylindricity,
        ] {
            let err = fcf(symbol, 0.05, &["A"]).validate();
            assert!(err.is_err(), "{symbol:?} dengan datum seharusnya ditolak");
        }
        // Orientasi/run-out/konsentrisitas/simetri wajib datum.
        for symbol in [
            GdtSymbol::Perpendicularity,
            GdtSymbol::Angularity,
            GdtSymbol::Parallelism,
            GdtSymbol::Concentricity,
            GdtSymbol::Symmetry,
            GdtSymbol::CircularRunout,
            GdtSymbol::TotalRunout,
        ] {
            assert!(fcf(symbol, 0.05, &[]).validate().is_err(), "{symbol:?}");
        }
        // Nilai harus > 0 dan hingga.
        assert!(fcf(GdtSymbol::Flatness, 0.0, &[]).validate().is_err());
        assert!(fcf(GdtSymbol::Flatness, -0.1, &[]).validate().is_err());
        assert!(fcf(GdtSymbol::Flatness, f64::NAN, &[]).validate().is_err());
        // Datum: terlalu banyak, ganda, atau label tak sah.
        assert!(fcf(GdtSymbol::Position, 0.1, &["A", "B", "C", "D"])
            .validate()
            .is_err());
        assert!(fcf(GdtSymbol::Position, 0.1, &["A", "A"])
            .validate()
            .is_err());
        for bad in ["", "a", "ABC", "A1", "A-", "A-A", "A-B-C"] {
            assert!(
                fcf(GdtSymbol::Position, 0.1, &[bad]).validate().is_err(),
                "datum '{bad}' seharusnya ditolak"
            );
        }

        // Zona diameter dan pengubah material.
        let mut position = fcf(GdtSymbol::Position, 0.1, &["A"]);
        position.diameter_zone = true;
        position.modifiers = vec![MaterialModifier::Mmc];
        assert!(position.validate().is_ok());
        position.modifiers = vec![MaterialModifier::Mmc, MaterialModifier::Lmc];
        assert!(position.validate().is_err());

        let mut flat = fcf(GdtSymbol::Flatness, 0.05, &[]);
        flat.diameter_zone = true;
        assert!(flat.validate().is_err());

        let mut runout = fcf(GdtSymbol::CircularRunout, 0.05, &["A"]);
        runout.modifiers = vec![MaterialModifier::Lmc];
        assert!(runout.validate().is_err());
    }

    #[test]
    fn drawing_annot_symbol_table_is_complete() {
        assert_eq!(GdtSymbol::ALL.len(), 14);
        for (i, symbol) in GdtSymbol::ALL.iter().enumerate() {
            assert!(!GdtSymbol::ALL[..i].contains(symbol));
            // Nama = bentuk serde.
            let json = serde_json::to_string(symbol).unwrap_or_default();
            assert_eq!(json, format!("\"{}\"", symbol.name()));
            // Tidak ada simbol yang sekaligus melarang dan mewajibkan datum.
            assert!(!(symbol.forbids_datum() && symbol.requires_datum()));
        }
    }

    #[test]
    fn drawing_annot_other_validation() {
        assert!(DatumFeature {
            label: "A".to_string()
        }
        .validate()
        .is_ok());
        assert!(DatumFeature {
            label: "A-B".to_string()
        }
        .validate()
        .is_err());
        assert!(SurfaceFinish { ra_um: 1.6 }.validate().is_ok());
        assert!(SurfaceFinish { ra_um: 0.0 }.validate().is_err());

        let row = |tag: &str, diameter: f64, depth: Option<f64>| HoleTableRow {
            tag: tag.to_string(),
            x: 10.0,
            y: 20.0,
            diameter,
            depth,
            note: String::new(),
        };
        let mut holes = HoleTable {
            title: String::new(),
            rows: vec![row("A1", 5.5, None), row("A2", 5.5, Some(12.0))],
        };
        assert!(holes.validate().is_ok());
        holes.rows.push(row("A1", 3.0, None));
        assert!(holes.validate().is_err());
        holes.rows.pop();
        holes.rows.push(row("B1", 0.0, None));
        assert!(holes.validate().is_err());
        holes.rows.pop();
        holes.rows.push(row("B1", 3.0, Some(-1.0)));
        assert!(holes.validate().is_err());

        let rev = |code: &str| RevisionRow {
            rev: code.to_string(),
            ..RevisionRow::default()
        };
        assert!(RevisionTable {
            rows: vec![rev("A"), rev("B")]
        }
        .validate()
        .is_ok());
        assert!(RevisionTable {
            rows: vec![rev("A"), rev("A")]
        }
        .validate()
        .is_err());
        assert!(RevisionTable {
            rows: vec![rev("")]
        }
        .validate()
        .is_err());

        // Kumpulan: indeks anotasi yang salah disebut; datum ganda ditolak.
        let datum = |label: &str| Annotation::DatumFeature {
            position: [10.0, 10.0],
            datum: DatumFeature {
                label: label.to_string(),
            },
        };
        assert!(validate_annotations(&[datum("A"), datum("B")]).is_ok());
        assert!(validate_annotations(&[datum("A"), datum("A")]).is_err());
        let bad = Annotation::SurfaceFinish {
            position: [f64::NAN, 0.0],
            finish: SurfaceFinish { ra_um: 1.6 },
        };
        let err = validate_annotations(&[datum("A"), bad]).unwrap_err();
        assert!(err.contains("#1"), "{err}");
    }

    #[test]
    fn drawing_annot_serde_roundtrip_and_defaults() {
        let annotations = vec![
            Annotation::DimensionTolerance {
                position: [40.0, 50.0],
                dimension: DimensionTolerance::with_fit(25.0, parse_fit("H7")),
            },
            Annotation::DimensionTolerance {
                position: [40.0, 60.0],
                dimension: DimensionTolerance::with_limits(10.0, 0.1, 0.2),
            },
            Annotation::FeatureControlFrame {
                position: [80.0, 50.0],
                frame: FeatureControlFrame {
                    symbol: GdtSymbol::Position,
                    value: 0.1,
                    diameter_zone: true,
                    modifiers: vec![MaterialModifier::Mmc],
                    datums: vec!["A".to_string(), "B".to_string()],
                },
            },
            Annotation::DatumFeature {
                position: [20.0, 20.0],
                datum: DatumFeature {
                    label: "A".to_string(),
                },
            },
            Annotation::SurfaceFinish {
                position: [30.0, 30.0],
                finish: SurfaceFinish { ra_um: 3.2 },
            },
            Annotation::HoleTable {
                position: [100.0, 100.0],
                table: HoleTable::default(),
            },
            Annotation::RevisionTable {
                position: [100.0, 150.0],
                table: RevisionTable::default(),
            },
        ];
        let json = serde_json::to_string(&annotations).unwrap_or_else(|e| panic!("{e}"));
        let back: Vec<Annotation> = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back, annotations);
        assert!(validate_annotations(&back).is_ok());

        // Field opsional boleh tidak ada.
        let minimal = r#"[
            {"type":"feature_control_frame","position":[1,2],
             "frame":{"symbol":"flatness","value":0.05}},
            {"type":"dimension_tolerance","position":[1,2],
             "dimension":{"nominal":25,"tolerance":{"fit":"g6"}}},
            {"type":"dimension_tolerance","position":[1,2],
             "dimension":{"nominal":10,"tolerance":{"limits":{"plus":0.1,"minus":0.1}}}},
            {"type":"hole_table","position":[1,2],
             "table":{"rows":[{"tag":"A1","x":1,"y":2,"diameter":5}]}},
            {"type":"revision_table","position":[1,2],"table":{"rows":[{"rev":"A"}]}}
        ]"#;
        let parsed: Vec<Annotation> =
            serde_json::from_str(minimal).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(parsed.len(), 5);
        assert!(validate_annotations(&parsed).is_ok());
        // Kelas ISO tak dikenal ditolak saat memuat, bukan panic.
        let bad = r#"{"type":"dimension_tolerance","position":[1,2],
            "dimension":{"nominal":25,"tolerance":{"fit":"Q7"}}}"#;
        assert!(serde_json::from_str::<Annotation>(bad).is_err());
    }

    #[test]
    fn drawing_annot_tolerance_stackup() {
        // Simetris: kasus terburuk = jumlah, RSS = akar jumlah kuadrat.
        let r = tolerance_stackup(&[(10.0, 0.1, 0.1), (20.0, 0.2, 0.2), (5.0, 0.2, 0.2)]);
        assert!((r.nominal - 35.0).abs() < 1e-12);
        assert!((r.worst_case_plus - 0.5).abs() < 1e-12);
        assert!((r.worst_case_minus - 0.5).abs() < 1e-12);
        assert!((r.rss_plus - 0.3).abs() < 1e-12);
        assert!((r.rss_minus - 0.3).abs() < 1e-12);
        assert!((r.worst_case_total() - 1.0).abs() < 1e-12);
        assert!((r.rss_total() - 0.6).abs() < 1e-12);

        // Tak simetris: (10 +0.2/-0) = 10.1 ± 0.1 dan (5 +0/-0.1) = 4.95 ± 0.05.
        let r = tolerance_stackup(&[(10.0, 0.2, 0.0), (5.0, 0.0, 0.1)]);
        let rss_half = (0.1f64 * 0.1 + 0.05 * 0.05).sqrt();
        assert!((r.worst_case_plus - 0.2).abs() < 1e-12);
        assert!((r.worst_case_minus - 0.1).abs() < 1e-12);
        assert!((r.rss_plus - (0.05 + rss_half)).abs() < 1e-12);
        assert!((r.rss_minus - (rss_half - 0.05)).abs() < 1e-12);
        // RSS tidak pernah melebihi kasus terburuk.
        assert!(r.rss_total() <= r.worst_case_total() + 1e-12);

        // Mata rantai berlawanan arah: celah = 30 - 10 - 19.5.
        let r = tolerance_stackup(&[(30.0, 0.1, 0.1), (-10.0, 0.05, 0.05), (-19.5, 0.0, 0.1)]);
        assert!((r.nominal - 0.5).abs() < 1e-12);
        assert!((r.worst_case_plus - 0.15).abs() < 1e-12);
        assert!((r.worst_case_minus - 0.25).abs() < 1e-12);

        // Satu mata rantai: RSS = kasus terburuk. Rantai kosong: nol.
        let r = tolerance_stackup(&[(12.0, 0.3, 0.1)]);
        assert!((r.rss_plus - 0.3).abs() < 1e-12 && (r.rss_minus - 0.1).abs() < 1e-12);
        let r = tolerance_stackup(&[]);
        assert_eq!(
            (r.nominal, r.worst_case_plus, r.rss_plus, r.rss_minus),
            (0.0, 0.0, 0.0, 0.0)
        );

        // Dari kelas ISO: 25 H7 -> (25, 0.021, 0).
        let link = DimensionTolerance::with_fit(25.0, parse_fit("H7"))
            .stackup_link()
            .unwrap_or_else(|e| panic!("{e}"));
        assert!((link.1 - 0.021).abs() < 1e-12 && link.2.abs() < 1e-12);
    }
}
