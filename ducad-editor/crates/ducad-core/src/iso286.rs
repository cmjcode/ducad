//! Sistem batas dan suaian ISO 286-1 sebagai data statis teruji.
//!
//! Cakupan: toleransi standar IT01..IT18 untuk ukuran nominal sampai 500 mm,
//! deviasi fundamental lubang `D, E, F, G, H, JS, K, M, N, P` dan poros
//! `d, e, f, g, h, js, k, m, n, p, r, s` (termasuk aturan delta ISO untuk
//! lubang K/M/N/P). Huruf lain sengaja tidak disertakan — lihat
//! [`SUPPORTED_HOLE_LETTERS`]/[`SUPPORTED_SHAFT_LETTERS`].
//!
//! Semua fungsi murni, tanpa I/O, dan tidak pernah panic pada masukan dari
//! luar: huruf tak dikenal, ukuran di luar rentang, atau kombinasi yang
//! tidak didefinisikan standar menghasilkan `Err`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Huruf deviasi fundamental lubang yang didukung.
pub const SUPPORTED_HOLE_LETTERS: [&str; 10] = ["D", "E", "F", "G", "H", "JS", "K", "M", "N", "P"];
/// Huruf deviasi fundamental poros yang didukung.
pub const SUPPORTED_SHAFT_LETTERS: [&str; 12] =
    ["d", "e", "f", "g", "h", "js", "k", "m", "n", "p", "r", "s"];

/// Ukuran nominal terbesar yang dicakup tabel (mm).
pub const MAX_NOMINAL_MM: f64 = 500.0;

/// Batas atas (inklusif) langkah ukuran utama ISO 286-1, mm. Langkah ke-`i`
/// mencakup `(STEP_UPPER[i-1], STEP_UPPER[i]]`; langkah pertama `(0, 3]`.
const STEP_UPPER: [f64; 13] = [
    3.0, 6.0, 10.0, 18.0, 30.0, 50.0, 80.0, 120.0, 180.0, 250.0, 315.0, 400.0, 500.0,
];

/// Batas atas (inklusif) langkah ukuran antara, dipakai deviasi poros `r`
/// dan `s` yang di atas 50 mm berubah di tengah langkah utama.
const SUB_STEP_UPPER: [f64; 22] = [
    3.0, 6.0, 10.0, 18.0, 30.0, 50.0, 65.0, 80.0, 100.0, 120.0, 140.0, 160.0, 180.0, 200.0, 225.0,
    250.0, 280.0, 315.0, 355.0, 400.0, 450.0, 500.0,
];

/// Toleransi standar ISO 286-1 Tabel 1, mikrometer. Baris = langkah ukuran
/// utama; kolom = IT01, IT0, IT1, …, IT18.
#[rustfmt::skip]
const IT_UM: [[f64; 20]; 13] = [
    // IT01 IT0  IT1  IT2  IT3   IT4   IT5   IT6   IT7   IT8   IT9    IT10   IT11   IT12   IT13   IT14    IT15    IT16    IT17    IT18
    [0.3, 0.5, 0.8, 1.2, 2.0,  3.0,  4.0,  6.0,  10.0, 14.0, 25.0,  40.0,  60.0,  100.0, 140.0, 250.0,  400.0,  600.0,  1000.0, 1400.0],
    [0.4, 0.6, 1.0, 1.5, 2.5,  4.0,  5.0,  8.0,  12.0, 18.0, 30.0,  48.0,  75.0,  120.0, 180.0, 300.0,  480.0,  750.0,  1200.0, 1800.0],
    [0.4, 0.6, 1.0, 1.5, 2.5,  4.0,  6.0,  9.0,  15.0, 22.0, 36.0,  58.0,  90.0,  150.0, 220.0, 360.0,  580.0,  900.0,  1500.0, 2200.0],
    [0.5, 0.8, 1.2, 2.0, 3.0,  5.0,  8.0,  11.0, 18.0, 27.0, 43.0,  70.0,  110.0, 180.0, 270.0, 430.0,  700.0,  1100.0, 1800.0, 2700.0],
    [0.6, 1.0, 1.5, 2.5, 4.0,  6.0,  9.0,  13.0, 21.0, 33.0, 52.0,  84.0,  130.0, 210.0, 330.0, 520.0,  840.0,  1300.0, 2100.0, 3300.0],
    [0.6, 1.0, 1.5, 2.5, 4.0,  7.0,  11.0, 16.0, 25.0, 39.0, 62.0,  100.0, 160.0, 250.0, 390.0, 620.0,  1000.0, 1600.0, 2500.0, 3900.0],
    [0.8, 1.2, 2.0, 3.0, 5.0,  8.0,  13.0, 19.0, 30.0, 46.0, 74.0,  120.0, 190.0, 300.0, 460.0, 740.0,  1200.0, 1900.0, 3000.0, 4600.0],
    [1.0, 1.5, 2.5, 4.0, 6.0,  10.0, 15.0, 22.0, 35.0, 54.0, 87.0,  140.0, 220.0, 350.0, 540.0, 870.0,  1400.0, 2200.0, 3500.0, 5400.0],
    [1.2, 2.0, 3.5, 5.0, 8.0,  12.0, 18.0, 25.0, 40.0, 63.0, 100.0, 160.0, 250.0, 400.0, 630.0, 1000.0, 1600.0, 2500.0, 4000.0, 6300.0],
    [2.0, 3.0, 4.5, 7.0, 10.0, 14.0, 20.0, 29.0, 46.0, 72.0, 115.0, 185.0, 290.0, 460.0, 720.0, 1150.0, 1850.0, 2900.0, 4600.0, 7200.0],
    [2.5, 4.0, 6.0, 8.0, 12.0, 16.0, 23.0, 32.0, 52.0, 81.0, 130.0, 210.0, 320.0, 520.0, 810.0, 1300.0, 2100.0, 3200.0, 5200.0, 8100.0],
    [3.0, 5.0, 7.0, 9.0, 13.0, 18.0, 25.0, 36.0, 57.0, 89.0, 140.0, 230.0, 360.0, 570.0, 890.0, 1400.0, 2300.0, 3600.0, 5700.0, 8900.0],
    [4.0, 6.0, 8.0, 10.0, 15.0, 20.0, 27.0, 40.0, 63.0, 97.0, 155.0, 250.0, 400.0, 630.0, 970.0, 1550.0, 2500.0, 4000.0, 6300.0, 9700.0],
];

// Deviasi fundamental poros per langkah ukuran utama, mikrometer.
// `d..g`: deviasi atas `es` (negatif). `k..p`: deviasi bawah `ei` (positif).
// Lubang huruf yang sama memakai nilai berlawanan tanda (aturan umum ISO).
const ES_D: [f64; 13] = [
    -20.0, -30.0, -40.0, -50.0, -65.0, -80.0, -100.0, -120.0, -145.0, -170.0, -190.0, -210.0,
    -230.0,
];
const ES_E: [f64; 13] = [
    -14.0, -20.0, -25.0, -32.0, -40.0, -50.0, -60.0, -72.0, -85.0, -100.0, -110.0, -125.0, -135.0,
];
const ES_F: [f64; 13] = [
    -6.0, -10.0, -13.0, -16.0, -20.0, -25.0, -30.0, -36.0, -43.0, -50.0, -56.0, -62.0, -68.0,
];
const ES_G: [f64; 13] = [
    -2.0, -4.0, -5.0, -6.0, -7.0, -9.0, -10.0, -12.0, -14.0, -15.0, -17.0, -18.0, -20.0,
];
/// `ei` poros `k` untuk tingkat IT4..IT7 (tingkat lain: 0).
const EI_K: [f64; 13] = [
    0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0, 5.0,
];
const EI_M: [f64; 13] = [
    2.0, 4.0, 6.0, 7.0, 8.0, 9.0, 11.0, 13.0, 15.0, 17.0, 20.0, 21.0, 23.0,
];
const EI_N: [f64; 13] = [
    4.0, 8.0, 10.0, 12.0, 15.0, 17.0, 20.0, 23.0, 27.0, 31.0, 34.0, 37.0, 40.0,
];
const EI_P: [f64; 13] = [
    6.0, 12.0, 15.0, 18.0, 22.0, 26.0, 32.0, 37.0, 43.0, 50.0, 56.0, 62.0, 68.0,
];
/// `ei` poros `r` per langkah ukuran antara ([`SUB_STEP_UPPER`]).
const EI_R: [f64; 22] = [
    10.0, 15.0, 19.0, 23.0, 28.0, 34.0, 41.0, 43.0, 51.0, 54.0, 63.0, 65.0, 68.0, 77.0, 80.0, 84.0,
    94.0, 98.0, 108.0, 114.0, 126.0, 132.0,
];
/// `ei` poros `s` per langkah ukuran antara ([`SUB_STEP_UPPER`]).
const EI_S: [f64; 22] = [
    14.0, 19.0, 23.0, 28.0, 35.0, 43.0, 53.0, 59.0, 71.0, 79.0, 92.0, 100.0, 108.0, 122.0, 130.0,
    140.0, 158.0, 170.0, 190.0, 208.0, 232.0, 252.0,
];

/// Tingkat toleransi standar. `It(n)` berlaku untuk `n` = 1..=18.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItGrade {
    /// IT01.
    It01,
    /// IT0.
    It0,
    /// IT1..IT18.
    It(u8),
}

impl ItGrade {
    fn column(self) -> Result<usize, String> {
        match self {
            ItGrade::It01 => Ok(0),
            ItGrade::It0 => Ok(1),
            ItGrade::It(n) if (1..=18).contains(&n) => Ok(usize::from(n) + 1),
            ItGrade::It(n) => Err(format!(
                "tingkat toleransi IT{n} tidak didefinisikan (IT01, IT0, IT1..IT18)"
            )),
        }
    }
}

impl fmt::Display for ItGrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ItGrade::It01 => write!(f, "IT01"),
            ItGrade::It0 => write!(f, "IT0"),
            ItGrade::It(n) => write!(f, "IT{n}"),
        }
    }
}

/// Kelas toleransi ISO 286, mis. `H7` (lubang) atau `g6` (poros).
///
/// Huruf besar = lubang, huruf kecil = poros. Di JSON ditulis sebagai string
/// `"H7"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IsoFit {
    /// Huruf deviasi fundamental apa adanya (`"H"`, `"js"`, …).
    pub letter: String,
    /// Tingkat toleransi IT 1..=18.
    pub grade: u8,
}

impl IsoFit {
    /// Mengurai kelas toleransi seperti `"H7"`, `"g6"`, `"JS9"`.
    ///
    /// Yang ditolak: huruf di luar daftar yang didukung, tingkat di luar
    /// 1..=18 (termasuk `01` dan `0`, yang tidak dipakai pada kelas suaian
    /// di implementasi ini), dan huruf campuran besar-kecil.
    pub fn parse(text: &str) -> Result<IsoFit, String> {
        let trimmed = text.trim();
        let split = trimmed
            .char_indices()
            .find(|(_, c)| c.is_ascii_digit())
            .map(|(i, _)| i)
            .ok_or_else(|| {
                format!("kelas toleransi '{trimmed}' tidak memuat tingkat IT (contoh: H7, g6)")
            })?;
        let (letter, digits) = trimmed.split_at(split);
        if letter.is_empty() {
            return Err(format!(
                "kelas toleransi '{trimmed}' tidak memuat huruf deviasi (contoh: H7, g6)"
            ));
        }
        if !SUPPORTED_HOLE_LETTERS.contains(&letter) && !SUPPORTED_SHAFT_LETTERS.contains(&letter) {
            return Err(format!(
                "huruf deviasi '{letter}' tidak didukung; lubang: {}; poros: {}",
                SUPPORTED_HOLE_LETTERS.join(" "),
                SUPPORTED_SHAFT_LETTERS.join(" ")
            ));
        }
        if digits.starts_with('0') || !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "tingkat toleransi '{digits}' pada '{trimmed}' tidak sah (pakai 1..18)"
            ));
        }
        let grade: u8 = digits
            .parse()
            .map_err(|_| format!("tingkat toleransi '{digits}' pada '{trimmed}' tidak sah"))?;
        if !(1..=18).contains(&grade) {
            return Err(format!(
                "tingkat toleransi IT{grade} tidak didefinisikan (pakai 1..18)"
            ));
        }
        Ok(IsoFit {
            letter: letter.to_string(),
            grade,
        })
    }

    /// `true` bila kelas ini milik lubang (huruf besar).
    pub fn is_hole(&self) -> bool {
        self.letter.chars().all(|c| c.is_ascii_uppercase()) && !self.letter.is_empty()
    }

    /// `true` bila kelas ini milik poros (huruf kecil).
    pub fn is_shaft(&self) -> bool {
        self.letter.chars().all(|c| c.is_ascii_lowercase()) && !self.letter.is_empty()
    }
}

impl fmt::Display for IsoFit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.letter, self.grade)
    }
}

impl TryFrom<String> for IsoFit {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        IsoFit::parse(&value)
    }
}

impl From<IsoFit> for String {
    fn from(value: IsoFit) -> Self {
        value.to_string()
    }
}

fn check_nominal(nominal_mm: f64) -> Result<(), String> {
    if !nominal_mm.is_finite() || nominal_mm <= 0.0 {
        return Err(format!(
            "ukuran nominal {nominal_mm} mm tidak sah (harus > 0)"
        ));
    }
    if nominal_mm > MAX_NOMINAL_MM {
        return Err(format!(
            "ukuran nominal {nominal_mm} mm di luar tabel ISO 286 yang dicakup (maksimum {MAX_NOMINAL_MM} mm)"
        ));
    }
    Ok(())
}

fn step_index(nominal_mm: f64, uppers: &[f64]) -> Result<usize, String> {
    check_nominal(nominal_mm)?;
    uppers
        .iter()
        .position(|upper| nominal_mm <= *upper)
        .ok_or_else(|| format!("ukuran nominal {nominal_mm} mm di luar tabel ISO 286"))
}

fn it_um(step: usize, column: usize) -> Result<f64, String> {
    IT_UM
        .get(step)
        .and_then(|row| row.get(column))
        .copied()
        .ok_or_else(|| "indeks tabel IT di luar rentang".to_string())
}

fn table(values: &[f64], index: usize) -> Result<f64, String> {
    values
        .get(index)
        .copied()
        .ok_or_else(|| "indeks tabel deviasi di luar rentang".to_string())
}

/// Toleransi standar (lebar zona) dalam mikrometer untuk ukuran nominal dan
/// tingkat IT tertentu.
///
/// IT14..IT18 tidak berlaku untuk ukuran nominal sampai 1 mm (ISO 286-1).
pub fn it_tolerance_um(nominal_mm: f64, grade: ItGrade) -> Result<f64, String> {
    let step = step_index(nominal_mm, &STEP_UPPER)?;
    let column = grade.column()?;
    if nominal_mm <= 1.0 && matches!(grade, ItGrade::It(n) if n >= 14) {
        return Err(format!(
            "{grade} tidak berlaku untuk ukuran nominal sampai 1 mm"
        ));
    }
    it_um(step, column)
}

/// Delta ISO untuk lubang K/M/N/P: `IT(n) - IT(n-1)`; nol untuk ukuran
/// sampai 3 mm (aturan khusus tidak berlaku di sana).
fn delta_um(step: usize, grade: u8) -> Result<f64, String> {
    if step == 0 {
        return Ok(0.0);
    }
    let column = usize::from(grade) + 1;
    Ok(it_um(step, column)? - it_um(step, column - 1)?)
}

/// Deviasi (atas, bawah) dalam mikrometer.
fn deviations_um(nominal_mm: f64, fit: &IsoFit) -> Result<(f64, f64), String> {
    let step = step_index(nominal_mm, &STEP_UPPER)?;
    let it = it_tolerance_um(nominal_mm, ItGrade::It(fit.grade))?;
    let grade = fit.grade;
    let undefined = || {
        format!("kelas {fit} tidak didefinisikan untuk ukuran nominal {nominal_mm} mm di tabel ini")
    };

    // Deviasi fundamental di sisi dekat garis nol; sisi lain = ± IT.
    let from_upper = |upper: f64| (upper, upper - it);
    let from_lower = |lower: f64| (lower + it, lower);

    let pair = match fit.letter.as_str() {
        // --- Poros ---
        "d" => from_upper(table(&ES_D, step)?),
        "e" => from_upper(table(&ES_E, step)?),
        "f" => from_upper(table(&ES_F, step)?),
        "g" => from_upper(table(&ES_G, step)?),
        "h" => from_upper(0.0),
        "js" | "JS" => (it / 2.0, -it / 2.0),
        "k" => {
            let ei = if (4..=7).contains(&grade) {
                table(&EI_K, step)?
            } else {
                0.0
            };
            from_lower(ei)
        }
        "m" => from_lower(table(&EI_M, step)?),
        "n" => from_lower(table(&EI_N, step)?),
        "p" => from_lower(table(&EI_P, step)?),
        "r" => from_lower(table(&EI_R, step_index(nominal_mm, &SUB_STEP_UPPER)?)?),
        "s" => from_lower(table(&EI_S, step_index(nominal_mm, &SUB_STEP_UPPER)?)?),
        // --- Lubang: EI = -es poros huruf yang sama ---
        "D" => from_lower(-table(&ES_D, step)?),
        "E" => from_lower(-table(&ES_E, step)?),
        "F" => from_lower(-table(&ES_F, step)?),
        "G" => from_lower(-table(&ES_G, step)?),
        "H" => from_lower(0.0),
        // --- Lubang K/M/N/P: ES = -ei + delta (aturan khusus) ---
        "K" | "M" | "N" | "P" => {
            // Delta tidak ditabulasikan di bawah IT3 untuk ukuran > 3 mm.
            if step > 0 && grade < 3 {
                return Err(undefined());
            }
            let es = match fit.letter.as_str() {
                "K" => {
                    if grade <= 8 {
                        -table(&EI_K, step)? + delta_um(step, grade)?
                    } else if step == 0 {
                        0.0
                    } else {
                        // Di atas IT8, K hanya ditabulasikan sampai 3 mm.
                        return Err(undefined());
                    }
                }
                "M" => {
                    if grade <= 8 {
                        // Kasus khusus ISO 286-1: M6 pada 250..315 mm = -9 µm.
                        if grade == 6 && step == 10 {
                            -9.0
                        } else {
                            -table(&EI_M, step)? + delta_um(step, grade)?
                        }
                    } else {
                        -table(&EI_M, step)?
                    }
                }
                "N" => {
                    if grade <= 8 {
                        -table(&EI_N, step)? + delta_um(step, grade)?
                    } else if nominal_mm <= 1.0 {
                        // N di atas IT8 tidak dipakai untuk ukuran sampai 1 mm.
                        return Err(undefined());
                    } else if step == 0 {
                        -table(&EI_N, step)?
                    } else {
                        0.0
                    }
                }
                _ => {
                    if grade <= 7 {
                        -table(&EI_P, step)? + delta_um(step, grade)?
                    } else {
                        -table(&EI_P, step)?
                    }
                }
            };
            from_upper(es)
        }
        other => {
            return Err(format!(
                "huruf deviasi '{other}' tidak didukung; lubang: {}; poros: {}",
                SUPPORTED_HOLE_LETTERS.join(" "),
                SUPPORTED_SHAFT_LETTERS.join(" ")
            ))
        }
    };
    Ok(pair)
}

/// Deviasi batas `(atas, bawah)` dalam mm terhadap ukuran nominal.
///
/// Contoh: `limits(25.0, &IsoFit::parse("H7")?)` = `(0.021, 0.0)`.
/// Untuk `js`/`JS` hasilnya tepat ±IT/2 (tanpa pembulatan ke bilangan genap).
pub fn limits(nominal_mm: f64, fit: &IsoFit) -> Result<(f64, f64), String> {
    let (upper, lower) = deviations_um(nominal_mm, fit)?;
    // `+ 0.0` menormalkan nol negatif supaya tampil sebagai "0".
    Ok((upper / 1000.0 + 0.0, lower / 1000.0 + 0.0))
}

/// Kelonggaran suaian `(minimum, maksimum)` dalam mm untuk pasangan lubang
/// dan poros. Nilai negatif berarti sesak (interferensi).
pub fn fit_clearance(nominal_mm: f64, hole: &IsoFit, shaft: &IsoFit) -> Result<(f64, f64), String> {
    if !hole.is_hole() {
        return Err(format!(
            "'{hole}' bukan kelas toleransi lubang (huruf besar)"
        ));
    }
    if !shaft.is_shaft() {
        return Err(format!(
            "'{shaft}' bukan kelas toleransi poros (huruf kecil)"
        ));
    }
    let (hole_upper, hole_lower) = deviations_um(nominal_mm, hole)?;
    let (shaft_upper, shaft_lower) = deviations_um(nominal_mm, shaft)?;
    Ok((
        (hole_lower - shaft_upper) / 1000.0 + 0.0,
        (hole_upper - shaft_lower) / 1000.0 + 0.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_limits(nominal: f64, class: &str, upper_um: f64, lower_um: f64) {
        let fit = match IsoFit::parse(class) {
            Ok(fit) => fit,
            Err(e) => panic!("{class}: {e}"),
        };
        match limits(nominal, &fit) {
            Ok((upper, lower)) => {
                assert!(
                    (upper * 1000.0 - upper_um).abs() < 1e-6
                        && (lower * 1000.0 - lower_um).abs() < 1e-6,
                    "{nominal}{class}: dapat {:+.1}/{:+.1} um, seharusnya {upper_um:+.1}/{lower_um:+.1} um",
                    upper * 1000.0,
                    lower * 1000.0
                );
            }
            Err(e) => panic!("{nominal}{class}: {e}"),
        }
    }

    /// Gate P19: sampel yang dicocokkan manual dengan tabel ISO 286-2.
    #[test]
    fn iso286_published_limits() {
        // Dua sampel wajib dari rencana.
        assert_limits(25.0, "H7", 21.0, 0.0);
        assert_limits(25.0, "g6", -7.0, -20.0);
        // Lubang.
        assert_limits(6.0, "H7", 12.0, 0.0);
        assert_limits(50.0, "H8", 39.0, 0.0);
        assert_limits(25.0, "G7", 28.0, 7.0);
        assert_limits(25.0, "F8", 53.0, 20.0);
        assert_limits(25.0, "E9", 92.0, 40.0);
        assert_limits(25.0, "D10", 149.0, 65.0);
        // Poros.
        assert_limits(10.0, "h6", 0.0, -9.0);
        assert_limits(25.0, "h7", 0.0, -21.0);
        assert_limits(25.0, "f7", -20.0, -41.0);
        assert_limits(60.0, "e8", -60.0, -106.0);
        assert_limits(80.0, "d9", -100.0, -174.0);
        assert_limits(25.0, "k6", 15.0, 2.0);
        assert_limits(25.0, "m6", 21.0, 8.0);
        assert_limits(25.0, "n6", 28.0, 15.0);
        assert_limits(25.0, "p6", 35.0, 22.0);
        assert_limits(50.0, "r6", 50.0, 34.0);
        assert_limits(100.0, "s6", 93.0, 71.0);
        assert_limits(40.0, "js6", 8.0, -8.0);
    }

    #[test]
    fn iso286_delta_rule_for_kmnp_holes() {
        assert_limits(25.0, "K7", 6.0, -15.0);
        assert_limits(25.0, "M7", 0.0, -21.0);
        assert_limits(25.0, "N7", -7.0, -28.0);
        assert_limits(25.0, "P7", -14.0, -35.0);
        // Di atas batas tingkat aturan delta: tanpa delta.
        assert_limits(25.0, "N9", 0.0, -52.0);
        assert_limits(25.0, "P9", -22.0, -74.0);
        assert_limits(5.0, "M8", 2.0, -16.0);
        // Kasus khusus M6 pada 250..315 mm.
        assert_limits(300.0, "M6", -9.0, -41.0);
        // Sampai 3 mm delta = 0.
        assert_limits(3.0, "K7", 0.0, -10.0);
    }

    #[test]
    fn iso286_size_step_boundaries() {
        // Batas atas langkah termasuk langkah itu: 3 -> (0,3], 30 -> (18,30].
        assert_limits(3.0, "H7", 10.0, 0.0);
        assert_limits(3.001, "H7", 12.0, 0.0);
        assert_limits(30.0, "H7", 21.0, 0.0);
        assert_limits(30.5, "H7", 25.0, 0.0);
        assert_limits(500.0, "H7", 63.0, 0.0);
        // r/s memakai langkah antara di atas 50 mm.
        assert_limits(65.0, "r6", 60.0, 41.0);
        assert_limits(66.0, "r6", 62.0, 43.0);
    }

    #[test]
    fn iso286_it_grades() {
        assert_eq!(it_tolerance_um(25.0, ItGrade::It01), Ok(0.6));
        assert_eq!(it_tolerance_um(25.0, ItGrade::It0), Ok(1.0));
        assert_eq!(it_tolerance_um(25.0, ItGrade::It(7)), Ok(21.0));
        assert_eq!(it_tolerance_um(500.0, ItGrade::It(18)), Ok(9700.0));
        assert_eq!(it_tolerance_um(2.0, ItGrade::It(14)), Ok(250.0));
        assert!(it_tolerance_um(1.0, ItGrade::It(14)).is_err());
        assert!(it_tolerance_um(25.0, ItGrade::It(19)).is_err());
        assert!(it_tolerance_um(25.0, ItGrade::It(0)).is_err());
    }

    /// Tabel IT harus naik monoton ke kanan (tingkat) dan tidak turun ke
    /// bawah (ukuran) — menangkap salah ketik baris/kolom.
    #[test]
    fn iso286_it_table_is_monotonic() {
        for (r, row) in IT_UM.iter().enumerate() {
            for c in 1..row.len() {
                assert!(row[c] > row[c - 1], "baris {r} kolom {c} tidak naik");
            }
            if r > 0 {
                for (c, value) in row.iter().enumerate() {
                    assert!(*value >= IT_UM[r - 1][c], "baris {r} kolom {c} turun");
                }
            }
        }
    }

    #[test]
    fn iso286_fit_clearance() {
        let parse = |s: &str| IsoFit::parse(s).unwrap_or_else(|e| panic!("{e}"));
        // H7/g6 pada 25 mm: longgar 0.007..0.041.
        let (min, max) =
            fit_clearance(25.0, &parse("H7"), &parse("g6")).unwrap_or_else(|e| panic!("{e}"));
        assert!((min - 0.007).abs() < 1e-9 && (max - 0.041).abs() < 1e-9);
        // H7/p6 pada 25 mm: sesak -0.035..-0.001.
        let (min, max) =
            fit_clearance(25.0, &parse("H7"), &parse("p6")).unwrap_or_else(|e| panic!("{e}"));
        assert!((min + 0.035).abs() < 1e-9 && (max + 0.001).abs() < 1e-9);
        // Urutan argumen tertukar ditolak.
        assert!(fit_clearance(25.0, &parse("g6"), &parse("H7")).is_err());
    }

    #[test]
    fn iso286_parse_and_errors() {
        let fit = IsoFit::parse(" JS9 ").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((fit.letter.as_str(), fit.grade), ("JS", 9));
        assert!(fit.is_hole() && !fit.is_shaft());
        assert_eq!(fit.to_string(), "JS9");

        for bad in [
            "", "7", "H", "H0", "H01", "H19", "H7x", "Q7", "Js7", "zc7", "H-7", "H 7",
        ] {
            assert!(IsoFit::parse(bad).is_err(), "'{bad}' seharusnya ditolak");
        }

        let h7 = IsoFit::parse("H7").unwrap_or_else(|e| panic!("{e}"));
        for bad in [0.0, -5.0, 500.1, f64::NAN, f64::INFINITY] {
            assert!(limits(bad, &h7).is_err(), "{bad} seharusnya ditolak");
        }
        // Kombinasi yang tidak ditabulasikan.
        let k9 = IsoFit::parse("K9").unwrap_or_else(|e| panic!("{e}"));
        assert!(limits(25.0, &k9).is_err());
        assert!(limits(2.0, &k9).is_ok());
        let n2 = IsoFit::parse("N2").unwrap_or_else(|e| panic!("{e}"));
        assert!(limits(25.0, &n2).is_err());
        // Struct yang dirakit manual dengan huruf tak dikenal tidak panic.
        let odd = IsoFit {
            letter: "zz".to_string(),
            grade: 7,
        };
        assert!(limits(25.0, &odd).is_err());
        let odd_grade = IsoFit {
            letter: "H".to_string(),
            grade: 0,
        };
        assert!(limits(25.0, &odd_grade).is_err());
    }

    #[test]
    fn iso286_serde_as_string() {
        let fit = IsoFit::parse("g6").unwrap_or_else(|e| panic!("{e}"));
        let json = serde_json::to_string(&fit).unwrap_or_default();
        assert_eq!(json, "\"g6\"");
        let back: IsoFit = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back, fit);
        assert!(serde_json::from_str::<IsoFit>("\"Q7\"").is_err());
    }
}
