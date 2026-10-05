//! Konfigurasi varian (P19): satu desain, banyak ukuran. Sebuah konfigurasi
//! menimpa sebagian parameter, menonaktifkan (suppress) sebagian op, dan
//! boleh mengganti material mekanik body. Juga impor/ekspor "design table"
//! CSV (baris = konfigurasi, kolom = parameter).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::MaterialSource;

/// Nama konfigurasi bawaan (tanpa penimpaan apa pun).
pub const DEFAULT_CONFIGURATION: &str = "Default";

/// Satu varian desain.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Configuration {
    pub name: String,
    /// Parameter yang ditimpa (nama → nilai); sisanya memakai nilai dasar.
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
    /// Id op yang dilewati saat replay.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppressed_ops: Vec<String>,
    /// Nama body → material mekanik pengganti.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub material_overrides: BTreeMap<String, MaterialSource>,
}

impl Configuration {
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
}

/// Nama konfigurasi sah: 1–48 karakter, huruf/angka/`_`/`-`/spasi/titik,
/// tidak diawali/diakhiri spasi (dipakai sebagai nama direktori build).
pub fn valid_configuration_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && name.trim() == name
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.'))
}

const NAME_COLUMN: &str = "configuration";
const SUPPRESSED_COLUMN: &str = "suppressed_ops";

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// Design table CSV: kolom `configuration`, satu kolom per parameter (urut
/// abjad, gabungan semua konfigurasi; sel kosong = pakai nilai dasar), lalu
/// `suppressed_ops` (id dipisah `;`). Penimpaan material tidak ikut.
pub fn design_table_to_csv(configs: &[Configuration]) -> String {
    let mut columns: Vec<&str> = configs
        .iter()
        .flat_map(|c| c.params.keys().map(String::as_str))
        .collect();
    columns.sort_unstable();
    columns.dedup();
    let mut out = String::from(NAME_COLUMN);
    for c in &columns {
        out.push(',');
        out.push_str(&csv_field(c));
    }
    out.push(',');
    out.push_str(SUPPRESSED_COLUMN);
    out.push('\n');
    for cfg in configs {
        out.push_str(&csv_field(&cfg.name));
        for c in &columns {
            out.push(',');
            if let Some(v) = cfg.params.get(*c) {
                out.push_str(&v.to_string());
            }
        }
        out.push(',');
        out.push_str(&csv_field(&cfg.suppressed_ops.join(";")));
        out.push('\n');
    }
    out
}

/// Pecah satu baris CSV (kutip ganda, `""` = kutip literal).
fn split_csv_line(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                chars.next();
                cur.push('"');
            }
            ('"', _) => quoted = !quoted,
            (',', false) => fields.push(std::mem::take(&mut cur)),
            (other, _) => cur.push(other),
        }
    }
    if quoted {
        return Err("tanda kutip tidak ditutup".to_string());
    }
    fields.push(cur);
    Ok(fields)
}

/// Kebalikan [`design_table_to_csv`]. Baris kosong dilewati; kolom
/// `suppressed_ops` opsional. Menolak nama duplikat/tidak sah dan angka
/// yang tidak bisa dibaca, dengan nomor baris.
pub fn design_table_from_csv(text: &str) -> Result<Vec<Configuration>, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty());
    let (_, header) = lines.next().ok_or("design table kosong")?;
    let header = split_csv_line(header).map_err(|e| format!("baris 1: {e}"))?;
    let header: Vec<String> = header.iter().map(|h| h.trim().to_string()).collect();
    if header.first().map(String::as_str) != Some(NAME_COLUMN) {
        return Err(format!("kolom pertama harus '{NAME_COLUMN}'"));
    }
    let mut out: Vec<Configuration> = Vec::new();
    for (idx, line) in lines {
        let row = idx + 1;
        let fields = split_csv_line(line).map_err(|e| format!("baris {row}: {e}"))?;
        if fields.len() > header.len() {
            return Err(format!(
                "baris {row}: {} kolom, header hanya {}",
                fields.len(),
                header.len()
            ));
        }
        let name = fields[0].trim().to_string();
        if !valid_configuration_name(&name) {
            return Err(format!("baris {row}: nama konfigurasi '{name}' tidak sah"));
        }
        if out.iter().any(|c| c.name == name) {
            return Err(format!("baris {row}: konfigurasi '{name}' muncul dua kali"));
        }
        let mut cfg = Configuration::named(name);
        for (column, value) in header.iter().zip(&fields).skip(1) {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            if column == SUPPRESSED_COLUMN {
                cfg.suppressed_ops = value
                    .split(';')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                continue;
            }
            let number: f64 = value
                .parse()
                .ok()
                .filter(|v: &f64| v.is_finite())
                .ok_or_else(|| format!("baris {row}: '{value}' bukan angka (kolom {column})"))?;
            cfg.params.insert(column.clone(), number);
        }
        out.push(cfg);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Configuration> {
        let mut small = Configuration::named("small");
        small.params.insert("len".into(), 40.0);
        let mut long = Configuration::named("long, no holes");
        long.params.insert("len".into(), 120.5);
        long.params.insert("t".into(), 8.0);
        long.suppressed_ops = vec!["holes".into(), "fold".into()];
        vec![small, long]
    }

    #[test]
    fn configuration_design_table_roundtrip() {
        let csv = design_table_to_csv(&sample());
        assert_eq!(
            csv,
            "configuration,len,t,suppressed_ops\nsmall,40,,\n\"long, no holes\",120.5,8,holes;fold\n"
        );
        // Nama berkoma dikutip saat ditulis; tetapi nama itu sendiri tidak sah
        // sebagai nama konfigurasi (dipakai sebagai nama direktori).
        assert!(design_table_from_csv(&csv)
            .unwrap_err()
            .contains("tidak sah"));

        let mut cfgs = sample();
        cfgs[1].name = "long".into();
        let back = design_table_from_csv(&design_table_to_csv(&cfgs)).unwrap();
        assert_eq!(back, cfgs);
    }

    #[test]
    fn configuration_design_table_reports_bad_rows() {
        let e = design_table_from_csv("configuration,len\na,10\na,20\n").unwrap_err();
        assert!(e.contains("baris 3") && e.contains("dua kali"), "{e}");
        let e = design_table_from_csv("configuration,len\na,sepuluh\n").unwrap_err();
        assert!(e.contains("bukan angka"), "{e}");
        assert!(design_table_from_csv("name,len\na,1\n").is_err());
        assert!(design_table_from_csv("").is_err());
        assert!(design_table_from_csv("configuration,len\na,1,2,3\n").is_err());
        // Kolom suppressed opsional, baris kosong dilewati.
        let ok = design_table_from_csv("configuration,len\n\nbig,90\n").unwrap();
        assert_eq!(ok[0].params["len"], 90.0);
    }

    #[test]
    fn configuration_names_are_safe_directory_names() {
        for good in ["Default", "M8 x 40", "rev-2.1", "a_b"] {
            assert!(valid_configuration_name(good), "{good}");
        }
        for bad in ["", " lead", "trail ", "a/b", "..", "x\\y", "é"] {
            assert!(!valid_configuration_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn configuration_serde_defaults_keep_old_files_readable() {
        let c: Configuration = serde_json::from_str(r#"{"name":"x"}"#).unwrap();
        assert_eq!(c, Configuration::named("x"));
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"name":"x","params":{}}"#
        );
    }
}
