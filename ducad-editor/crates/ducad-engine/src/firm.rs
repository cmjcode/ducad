//! Profil firma (W3): standar perusahaan yang terbaca mesin — kepala gambar,
//! kelas toleransi umum, harga material, tarif proses, stok pelat, paket
//! aturan wajib. Dipakai `ducad-cli build`, tool `build_package`, dan
//! diberikan ke agent lewat resource `ducad://firm`.
//!
//! Estimasi biaya di sini INDIKATIF: massa × harga material + waktu proses
//! kasar × tarif. Semua angka harga berasal dari profil pengguna; tanpa
//! profil, laporan hanya memuat massa dan volume stok.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::check::{Process, RulePackRef};
use crate::error::{OpError, OpErrorCode, OpResult};
use crate::model::ModelDoc;
use crate::session::SessionMeta;

fn default_tolerance() -> String {
    "ISO 2768-m".to_string()
}

fn default_currency() -> String {
    "USD".to_string()
}

/// Harga satu material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirmMaterial {
    /// Nama tampil, mis. `"Al 6061-T6"`.
    pub name: String,
    /// Nama lain yang dicocokkan dengan material body (`"al_6061_t6"`, `"aluminum"`).
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Harga per kilogram dalam `currency`.
    pub price_per_kg: f64,
}

/// Tarif satu proses manufaktur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirmProcess {
    pub process: Process,
    /// Biaya setup per batch (dibagi kuantitas).
    #[serde(default)]
    pub setup_cost: f64,
    /// Tarif mesin + operator per jam.
    pub rate_per_hour: f64,
    /// Laju volume: material TERBUANG per jam (CNC) atau TERBENTUK per jam
    /// (proses aditif/cetak), cm³/jam.
    pub rate_cm3_per_hour: f64,
}

/// Profil firma. Semua field opsional; berkas kosong `{}` sah.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirmProfile {
    /// Nama perusahaan di kepala gambar.
    #[serde(default)]
    pub company: String,
    /// Nama penggambar bawaan.
    #[serde(default)]
    pub drawn_by: String,
    /// Teks toleransi umum di kepala gambar.
    #[serde(default = "default_tolerance")]
    pub general_tolerance: String,
    /// Kertas bawaan: `a4`, `a4-portrait`, `a3`, `a3-portrait`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper: Option<String>,
    /// Awalan nomor part, mis. `"ACME-"`.
    #[serde(default)]
    pub part_number_prefix: String,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub materials: Vec<FirmMaterial>,
    #[serde(default)]
    pub processes: Vec<FirmProcess>,
    /// Tebal pelat yang distok (mm).
    #[serde(default)]
    pub stock_thicknesses_mm: Vec<f64>,
    /// Fastener pilihan, mis. `"ISO 4762 M6"`.
    #[serde(default)]
    pub preferred_fasteners: Vec<String>,
    /// Catatan umum yang selalu dicetak di lembar gambar.
    #[serde(default)]
    pub notes: Vec<String>,
    /// Paket aturan yang selalu diterapkan firma ini.
    #[serde(default)]
    pub rule_packs: Vec<RulePackRef>,
}

impl Default for FirmProfile {
    fn default() -> Self {
        Self {
            company: String::new(),
            drawn_by: String::new(),
            general_tolerance: default_tolerance(),
            paper: None,
            part_number_prefix: String::new(),
            currency: default_currency(),
            materials: Vec::new(),
            processes: Vec::new(),
            stock_thicknesses_mm: Vec::new(),
            preferred_fasteners: Vec::new(),
            notes: Vec::new(),
            rule_packs: Vec::new(),
        }
    }
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

impl FirmProfile {
    pub fn from_json(text: &str) -> OpResult<Self> {
        let profile: FirmProfile = serde_json::from_str(text)
            .map_err(|e| OpError::invalid(format!("firm profile is not valid: {e}")))?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn load(path: &Path) -> OpResult<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            OpError::new(
                OpErrorCode::Io,
                format!("cannot read firm profile {}: {e}", path.display()),
            )
        })?;
        Self::from_json(&text)
    }

    pub fn validate(&self) -> OpResult<()> {
        if let Some(p) = &self.paper {
            if ducad_io::drawing::parse_paper(p).is_none() {
                return Err(OpError::invalid(format!(
                    "firm profile: unknown paper '{p}' (a4, a4-portrait, a3, a3-portrait)"
                )));
            }
        }
        for m in &self.materials {
            if m.name.trim().is_empty() || !(m.price_per_kg.is_finite() && m.price_per_kg >= 0.0) {
                return Err(OpError::invalid(format!(
                    "firm profile: material '{}' needs a name and a price_per_kg >= 0",
                    m.name
                )));
            }
        }
        for p in &self.processes {
            let ok = |v: f64| v.is_finite() && v >= 0.0;
            let valid = ok(p.setup_cost)
                && ok(p.rate_per_hour)
                && p.rate_cm3_per_hour.is_finite()
                && p.rate_cm3_per_hour > 0.0;
            if !valid {
                return Err(OpError::invalid(format!(
                    "firm profile: process {:?} needs setup_cost >= 0, rate_per_hour >= 0 and rate_cm3_per_hour > 0",
                    p.process
                )));
            }
        }
        for pack in &self.rule_packs {
            crate::check::expand_rule_pack(pack)?;
        }
        Ok(())
    }

    /// Material firma yang cocok dengan label material body.
    pub fn material(&self, label: &str) -> Option<&FirmMaterial> {
        let key = norm(label);
        if key.is_empty() {
            return None;
        }
        self.materials.iter().find(|m| {
            std::iter::once(&m.name)
                .chain(&m.aliases)
                .map(|n| norm(n))
                .any(|n| !n.is_empty() && (n == key || key.contains(&n) || n.contains(&key)))
        })
    }

    pub fn process(&self, process: Process) -> Option<&FirmProcess> {
        self.processes.iter().find(|p| p.process == process)
    }

    pub fn paper_size(&self) -> Option<ducad_io::drawing::PaperSize> {
        self.paper
            .as_deref()
            .and_then(ducad_io::drawing::parse_paper)
    }
}

/// Cari profil firma: `explicit`, lalu env `DUCAD_FIRM`, lalu `firm.json` di
/// samping part, lalu `$HOME/DUCAD-Memory/Standards/firm.json`.
pub fn find_firm_profile(explicit: Option<&Path>, part_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os("DUCAD_FIRM").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let beside = part_dir
        .map(|d| d.join("firm.json"))
        .filter(|p| p.is_file());
    if beside.is_some() {
        return beside;
    }
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("DUCAD-Memory/Standards/firm.json"))
        .filter(|p| p.is_file())
}

/// Biaya satu body.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CostLine {
    pub body: String,
    pub material: String,
    pub mass_g: Option<f64>,
    /// Volume bahan baku yang dibeli (cm³).
    pub stock_cm3: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material_cost: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_hours: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_cost: Option<f64>,
    /// Material + proses per unit; `None` bila salah satu tidak diketahui.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_cost: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Estimasi biaya indikatif seluruh part.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CostReport {
    /// Selalu `true`: angka ini bukan penawaran harga.
    pub indicative: bool,
    pub currency: String,
    pub quantity: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<Process>,
    pub lines: Vec<CostLine>,
    /// Jumlah `unit_cost` semua body + setup dibagi kuantitas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_total: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_per_unit: Option<f64>,
    pub warnings: Vec<String>,
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Label material body: material mekanik bila ada, selain itu preset visual.
pub fn material_label(body: &ducad_core::Body) -> String {
    match &body.mechanical {
        Some(m) => m.label(),
        None => serde_json::to_value(body.material.preset)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default(),
    }
}

/// Estimasi biaya indikatif dari geometri + profil firma.
pub fn estimate_cost(model: &ModelDoc, meta: &SessionMeta, firm: &FirmProfile) -> CostReport {
    let brief = meta.design.brief.as_ref();
    let process = brief.and_then(|b| b.process);
    let quantity = brief.and_then(|b| b.quantity).unwrap_or(1).max(1);
    let rate = process.and_then(|p| firm.process(p));
    let mut warnings = Vec::new();
    if firm.materials.is_empty() {
        warnings
            .push("firm profile has no material prices: material cost not estimated".to_string());
    }
    match (process, rate) {
        (None, _) => warnings.push(
            "the design brief names no manufacturing process: process cost not estimated"
                .to_string(),
        ),
        (Some(p), None) => warnings.push(format!(
            "firm profile has no rate for process {}: process cost not estimated",
            serde_json::to_value(p)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default()
        )),
        _ => {}
    }

    let mut bodies: Vec<_> = model
        .doc
        .bodies
        .iter()
        .filter(|(_, b)| b.visible)
        .filter_map(|(id, b)| Some((b, model.geometry.get(id)?)))
        .collect();
    bodies.sort_by(|a, b| a.0.name.cmp(&b.0.name));

    let mut lines = Vec::with_capacity(bodies.len());
    for (body, geo) in bodies {
        let volume_cm3 = geo.shape.volume().abs() / 1000.0;
        let density = model.doc.density_of(body);
        let label = material_label(body);
        let mut notes = Vec::new();
        if let Some(standard) = meta.standard_parts.get(&body.name) {
            // Part standar dibeli, bukan dibuat.
            lines.push(CostLine {
                body: body.name.clone(),
                material: label,
                mass_g: density.map(|d| round2(volume_cm3 * d)),
                stock_cm3: 0.0,
                material_cost: None,
                process_hours: None,
                process_cost: None,
                unit_cost: None,
                notes: vec![format!("purchased standard part: {standard}")],
            });
            continue;
        }
        let bbox_cm3 = geo
            .mesh
            .bounding_box()
            .map(|(a, b)| (0..3).map(|k| (b[k] - a[k]) as f64).product::<f64>() / 1000.0)
            .unwrap_or(volume_cm3);
        // CNC membeli balok seukuran kotak pembatas; proses lain ~ volume part.
        let stock_cm3 = match process {
            Some(Process::Cnc) => bbox_cm3.max(volume_cm3),
            _ => volume_cm3,
        };
        let material_cost = match (firm.material(&label), density) {
            (Some(m), Some(d)) => Some(stock_cm3 * d / 1000.0 * m.price_per_kg),
            (None, _) if !firm.materials.is_empty() => {
                notes.push(format!(
                    "no price for material '{label}' in the firm profile"
                ));
                None
            }
            (Some(_), None) => {
                notes.push("material density unknown: set a mechanical material".to_string());
                None
            }
            _ => None,
        };
        let process_hours = rate.map(|r| {
            let worked = match process {
                Some(Process::Cnc) => (stock_cm3 - volume_cm3).max(0.0),
                _ => volume_cm3,
            };
            worked / r.rate_cm3_per_hour
        });
        let process_cost = rate.zip(process_hours).map(|(r, h)| h * r.rate_per_hour);
        if let (Some(state), false) = (
            meta.sheet_metal.get(&body.name),
            firm.stock_thicknesses_mm.is_empty(),
        ) {
            let t = state.model.thickness;
            if !firm
                .stock_thicknesses_mm
                .iter()
                .any(|s| (s - t).abs() < 1e-6)
            {
                notes.push(format!(
                    "sheet thickness {t} mm is not a stocked thickness ({:?})",
                    firm.stock_thicknesses_mm
                ));
            }
        }
        lines.push(CostLine {
            body: body.name.clone(),
            material: label,
            mass_g: density.map(|d| round2(volume_cm3 * d)),
            stock_cm3: round2(stock_cm3),
            material_cost: material_cost.map(round2),
            process_hours: process_hours.map(|h| (h * 1000.0).round() / 1000.0),
            process_cost: process_cost.map(round2),
            unit_cost: material_cost.zip(process_cost).map(|(m, p)| round2(m + p)),
            notes,
        });
    }

    let made: Vec<&CostLine> = lines.iter().filter(|l| l.stock_cm3 > 0.0).collect();
    let setup_per_unit = rate.map(|r| round2(r.setup_cost / f64::from(quantity)));
    let unit_total = if !made.is_empty() && made.iter().all(|l| l.unit_cost.is_some()) {
        let sum: f64 = made.iter().filter_map(|l| l.unit_cost).sum();
        Some(round2(sum + setup_per_unit.unwrap_or(0.0)))
    } else {
        None
    };
    CostReport {
        indicative: true,
        currency: firm.currency.clone(),
        quantity,
        process,
        lines,
        unit_total,
        setup_per_unit,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_profile_is_valid_and_prices_are_checked() {
        let p = FirmProfile::from_json("{}").unwrap();
        assert_eq!(p.general_tolerance, "ISO 2768-m");
        assert!(FirmProfile::from_json(r#"{"paper":"a2"}"#).is_err());
        assert!(
            FirmProfile::from_json(r#"{"materials":[{"name":"x","price_per_kg":-1}]}"#).is_err()
        );
        assert!(FirmProfile::from_json(r#"{"rule_packs":["nope"]}"#).is_err());
        assert!(FirmProfile::from_json(r#"{"compnay":"typo"}"#).is_err());
    }

    #[test]
    fn material_lookup_ignores_case_and_punctuation() {
        let p = FirmProfile::from_json(
            r#"{"materials":[{"name":"Al 6061-T6","aliases":["aluminum"],"price_per_kg":6.5},
                             {"name":"Steel S235","price_per_kg":1.2}]}"#,
        )
        .unwrap();
        assert_eq!(p.material("al_6061_t6").unwrap().price_per_kg, 6.5);
        assert_eq!(p.material("aluminum").unwrap().price_per_kg, 6.5);
        assert_eq!(p.material("steel_s235").unwrap().price_per_kg, 1.2);
        assert!(p.material("abs").is_none());
        assert!(p.material("").is_none());
    }
}
