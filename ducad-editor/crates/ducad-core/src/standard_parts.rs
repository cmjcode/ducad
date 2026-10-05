//! Toolbox part standar (P20): dimensi baut, mur, ring, pin, dan bearing
//! dari tabel standar. Hanya data + penamaan; geometri dibangun engine.
//!
//! Dimensi = nilai nominal tabel produk standar yang bersangkutan (ISO 4762,
//! ISO 4014/4017, ISO 4032, ISO 7089, ISO 2338, seri dimensi bearing ISO 15).
//! Ulir digambar kosmetik (batang polos berdiameter nominal).

/// Jenis part standar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardKind {
    /// Baut kepala silinder soket heksagon, ISO 4762.
    SocketHeadCapScrew,
    /// Baut kepala heksagon, ISO 4014 (berulir sebagian) / 4017 (penuh).
    HexBolt,
    /// Mur heksagon tipe 1, ISO 4032.
    HexNut,
    /// Ring pelat seri normal, ISO 7089.
    PlainWasher,
    /// Pin silindris, ISO 2338.
    ParallelPin,
    /// Bearing bola alur dalam satu baris (seri 60/62, d×D×B menurut ISO 15).
    DeepGrooveBearing,
}

impl StandardKind {
    pub const ALL: [StandardKind; 6] = [
        StandardKind::SocketHeadCapScrew,
        StandardKind::HexBolt,
        StandardKind::HexNut,
        StandardKind::PlainWasher,
        StandardKind::ParallelPin,
        StandardKind::DeepGrooveBearing,
    ];

    /// Kunci di oplog (`"iso4762"`, …).
    pub fn key(self) -> &'static str {
        match self {
            StandardKind::SocketHeadCapScrew => "iso4762",
            StandardKind::HexBolt => "iso4014",
            StandardKind::HexNut => "iso4032",
            StandardKind::PlainWasher => "iso7089",
            StandardKind::ParallelPin => "iso2338",
            StandardKind::DeepGrooveBearing => "bearing",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        let k = key.to_ascii_lowercase().replace([' ', '-', '_'], "");
        Self::ALL
            .into_iter()
            .find(|s| s.key() == k)
            .or(match k.as_str() {
                "iso4017" => Some(StandardKind::HexBolt),
                "dingroove" | "iso15" => Some(StandardKind::DeepGrooveBearing),
                _ => None,
            })
    }

    /// Nomor standar untuk BOM.
    pub fn standard_number(self) -> &'static str {
        match self {
            StandardKind::SocketHeadCapScrew => "ISO 4762",
            StandardKind::HexBolt => "ISO 4014",
            StandardKind::HexNut => "ISO 4032",
            StandardKind::PlainWasher => "ISO 7089",
            StandardKind::ParallelPin => "ISO 2338",
            StandardKind::DeepGrooveBearing => "ISO 15",
        }
    }

    /// Butuh panjang (baut, pin).
    pub fn needs_length(self) -> bool {
        matches!(
            self,
            StandardKind::SocketHeadCapScrew | StandardKind::HexBolt | StandardKind::ParallelPin
        )
    }

    /// Ukuran yang tersedia (untuk pesan error dan skema).
    pub fn sizes(self) -> Vec<&'static str> {
        match self {
            StandardKind::ParallelPin => PIN_DIAMETERS.iter().map(|p| p.0).collect(),
            StandardKind::DeepGrooveBearing => BEARINGS.iter().map(|b| b.0).collect(),
            _ => METRIC.iter().map(|m| m.size).collect(),
        }
    }
}

/// Satu baris tabel ukuran metrik (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricRow {
    pub size: &'static str,
    /// Diameter nominal ulir.
    pub d: f64,
    /// ISO 4762: diameter kepala `dk` (tinggi kepala = d), kunci soket `s`.
    pub cap_head_d: f64,
    pub cap_socket: f64,
    /// ISO 4014/4032: lebar kunci `s`; tinggi kepala baut `k`; tinggi mur `m`.
    pub hex_across_flats: f64,
    pub hex_head_k: f64,
    pub nut_m: f64,
    /// ISO 7089: diameter dalam `d1`, luar `d2`, tebal `h`.
    pub washer_d1: f64,
    pub washer_d2: f64,
    pub washer_h: f64,
}

const fn row(size: &'static str, v: [f64; 9]) -> MetricRow {
    MetricRow {
        size,
        d: v[0],
        cap_head_d: v[1],
        cap_socket: v[2],
        hex_across_flats: v[3],
        hex_head_k: v[4],
        nut_m: v[5],
        washer_d1: v[6],
        washer_d2: v[7],
        washer_h: v[8],
    }
}

/// d, dk, s(soket), s(heksagon), k, m, d1, d2, h.
static METRIC: [MetricRow; 7] = [
    row("M3", [3.0, 5.5, 2.5, 5.5, 2.0, 2.4, 3.2, 7.0, 0.5]),
    row("M4", [4.0, 7.0, 3.0, 7.0, 2.8, 3.2, 4.3, 9.0, 0.8]),
    row("M5", [5.0, 8.5, 4.0, 8.0, 3.5, 4.7, 5.3, 10.0, 1.0]),
    row("M6", [6.0, 10.0, 5.0, 10.0, 4.0, 5.2, 6.4, 12.0, 1.6]),
    row("M8", [8.0, 13.0, 6.0, 13.0, 5.3, 6.8, 8.4, 16.0, 1.6]),
    row("M10", [10.0, 16.0, 8.0, 16.0, 6.4, 8.4, 10.5, 20.0, 2.0]),
    row("M12", [12.0, 18.0, 10.0, 18.0, 7.5, 10.8, 13.0, 24.0, 2.5]),
];

/// Diameter pin ISO 2338 yang didukung.
static PIN_DIAMETERS: [(&str, f64); 8] = [
    ("2", 2.0),
    ("3", 3.0),
    ("4", 4.0),
    ("5", 5.0),
    ("6", 6.0),
    ("8", 8.0),
    ("10", 10.0),
    ("12", 12.0),
];

/// Bearing bola alur dalam: (kode, d, D, B).
static BEARINGS: [(&str, f64, f64, f64); 9] = [
    ("608", 8.0, 22.0, 7.0),
    ("6000", 10.0, 26.0, 8.0),
    ("6001", 12.0, 28.0, 8.0),
    ("6002", 15.0, 32.0, 9.0),
    ("6200", 10.0, 30.0, 9.0),
    ("6201", 12.0, 32.0, 10.0),
    ("6202", 15.0, 35.0, 11.0),
    ("6204", 20.0, 47.0, 14.0),
    ("6205", 25.0, 52.0, 15.0),
];

/// Baris tabel metrik untuk `size` (`"M6"`, tanpa beda huruf besar/kecil).
pub fn metric_row(size: &str) -> Option<&'static MetricRow> {
    METRIC
        .iter()
        .find(|m| m.size.eq_ignore_ascii_case(size.trim()))
}

/// Bentuk part standar yang sudah diselesaikan menjadi dimensi.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StandardShape {
    /// Batang ke −Z dari bidang bawah kepala (z = 0), kepala silinder ke +Z
    /// dengan soket heksagon.
    CapScrew {
        d: f64,
        length: f64,
        head_d: f64,
        head_h: f64,
        socket_af: f64,
        socket_depth: f64,
    },
    /// Batang ke −Z, kepala heksagon ke +Z.
    HexBolt {
        d: f64,
        length: f64,
        across_flats: f64,
        head_h: f64,
    },
    /// Prisma heksagon berlubang, z = 0..m.
    HexNut {
        d: f64,
        across_flats: f64,
        height: f64,
    },
    /// Cincin, z = 0..h.
    Ring {
        inner_d: f64,
        outer_d: f64,
        height: f64,
    },
    /// Silinder pejal, z = 0..length.
    Pin { d: f64, length: f64 },
}

/// Part standar yang sudah dikenali: bentuk + sebutan BOM.
#[derive(Debug, Clone, PartialEq)]
pub struct StandardPart {
    pub kind: StandardKind,
    pub shape: StandardShape,
    /// Mis. `"ISO 4762 - M6 x 20"`, `"ISO 7089 - 6"`, `"Bearing 6204 (ISO 15)"`.
    pub designation: String,
}

fn fmt_len(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// Selesaikan `(jenis, ukuran, panjang)` menjadi dimensi. Pesan error
/// menyebut ukuran yang tersedia.
pub fn standard_part(
    kind: StandardKind,
    size: &str,
    length: Option<f64>,
) -> Result<StandardPart, String> {
    let unknown = || {
        format!(
            "ukuran '{size}' tidak ada untuk {} (tersedia: {})",
            kind.standard_number(),
            kind.sizes().join(", ")
        )
    };
    let length = match (kind.needs_length(), length) {
        (true, Some(l)) if l.is_finite() && l > 0.0 => l,
        (true, Some(l)) => return Err(format!("panjang harus > 0 (dapat {l})")),
        (true, None) => return Err(format!("{} butuh `length`", kind.standard_number())),
        (false, Some(_)) => {
            return Err(format!("{} tidak memakai `length`", kind.standard_number()))
        }
        (false, None) => 0.0,
    };
    let number = kind.standard_number();
    let (shape, designation) = match kind {
        StandardKind::SocketHeadCapScrew => {
            let m = metric_row(size).ok_or_else(unknown)?;
            (
                StandardShape::CapScrew {
                    d: m.d,
                    length,
                    head_d: m.cap_head_d,
                    head_h: m.d,
                    socket_af: m.cap_socket,
                    socket_depth: m.d / 2.0,
                },
                format!("{number} - {} x {}", m.size, fmt_len(length)),
            )
        }
        StandardKind::HexBolt => {
            let m = metric_row(size).ok_or_else(unknown)?;
            (
                StandardShape::HexBolt {
                    d: m.d,
                    length,
                    across_flats: m.hex_across_flats,
                    head_h: m.hex_head_k,
                },
                format!("{number} - {} x {}", m.size, fmt_len(length)),
            )
        }
        StandardKind::HexNut => {
            let m = metric_row(size).ok_or_else(unknown)?;
            (
                StandardShape::HexNut {
                    d: m.d,
                    across_flats: m.hex_across_flats,
                    height: m.nut_m,
                },
                format!("{number} - {}", m.size),
            )
        }
        StandardKind::PlainWasher => {
            let m = metric_row(size).ok_or_else(unknown)?;
            (
                StandardShape::Ring {
                    inner_d: m.washer_d1,
                    outer_d: m.washer_d2,
                    height: m.washer_h,
                },
                format!("{number} - {}", fmt_len(m.d)),
            )
        }
        StandardKind::ParallelPin => {
            let key = size.trim().trim_start_matches(['d', 'D', 'Ø']);
            let (_, d) = PIN_DIAMETERS
                .iter()
                .find(|p| p.0 == key)
                .ok_or_else(unknown)?;
            (
                StandardShape::Pin { d: *d, length },
                format!("{number} - {} x {}", fmt_len(*d), fmt_len(length)),
            )
        }
        StandardKind::DeepGrooveBearing => {
            let (code, d, big_d, b) = BEARINGS
                .iter()
                .find(|x| x.0 == size.trim())
                .ok_or_else(unknown)?;
            (
                StandardShape::Ring {
                    inner_d: *d,
                    outer_d: *big_d,
                    height: *b,
                },
                format!("Bearing {code} ({number})"),
            )
        }
    };
    Ok(StandardPart {
        kind,
        shape,
        designation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_parts_table_is_consistent() {
        for m in &METRIC {
            // Kepala lebih besar dari batang, soket muat di kepala, ring muat di baut.
            assert!(
                m.cap_head_d > m.d && m.cap_socket < m.cap_head_d,
                "{}",
                m.size
            );
            assert!(m.hex_across_flats > m.d, "{}", m.size);
            assert!(
                m.washer_d1 > m.d && m.washer_d2 > m.hex_across_flats,
                "{}",
                m.size
            );
            assert!(m.nut_m > 0.0 && m.hex_head_k > 0.0 && m.washer_h > 0.0);
            assert_eq!(m.size, format!("M{}", m.d as i64));
        }
        for (code, d, big_d, b) in &BEARINGS {
            assert!(d < big_d && *b > 0.0, "{code}");
        }
        // Sampel yang dicek tangan terhadap tabel standar.
        let m6 = metric_row("m6").unwrap();
        assert_eq!(
            (m6.cap_head_d, m6.cap_socket, m6.hex_across_flats),
            (10.0, 5.0, 10.0)
        );
        assert_eq!((m6.hex_head_k, m6.nut_m), (4.0, 5.2));
        assert_eq!((m6.washer_d1, m6.washer_d2, m6.washer_h), (6.4, 12.0, 1.6));
        let m10 = metric_row("M10").unwrap();
        assert_eq!(
            (m10.cap_head_d, m10.hex_across_flats, m10.nut_m),
            (16.0, 16.0, 8.4)
        );
    }

    #[test]
    fn standard_parts_designations_and_errors() {
        let p = standard_part(StandardKind::SocketHeadCapScrew, "M6", Some(20.0)).unwrap();
        assert_eq!(p.designation, "ISO 4762 - M6 x 20");
        let p = standard_part(StandardKind::HexBolt, "m8", Some(37.5)).unwrap();
        assert_eq!(p.designation, "ISO 4014 - M8 x 37.5");
        assert_eq!(
            standard_part(StandardKind::HexNut, "M10", None)
                .unwrap()
                .designation,
            "ISO 4032 - M10"
        );
        assert_eq!(
            standard_part(StandardKind::PlainWasher, "M6", None)
                .unwrap()
                .designation,
            "ISO 7089 - 6"
        );
        assert_eq!(
            standard_part(StandardKind::ParallelPin, "6", Some(24.0))
                .unwrap()
                .designation,
            "ISO 2338 - 6 x 24"
        );
        let b = standard_part(StandardKind::DeepGrooveBearing, "6204", None).unwrap();
        assert_eq!(b.designation, "Bearing 6204 (ISO 15)");
        assert_eq!(
            b.shape,
            StandardShape::Ring {
                inner_d: 20.0,
                outer_d: 47.0,
                height: 14.0
            }
        );

        let e = standard_part(StandardKind::HexNut, "M7", None).unwrap_err();
        assert!(e.contains("M6") && e.contains("M8"), "{e}");
        assert!(standard_part(StandardKind::HexBolt, "M6", None)
            .unwrap_err()
            .contains("length"));
        assert!(standard_part(StandardKind::HexNut, "M6", Some(5.0)).is_err());
        assert!(standard_part(StandardKind::ParallelPin, "6", Some(-1.0)).is_err());
        assert_eq!(
            StandardKind::from_key("ISO 4762"),
            Some(StandardKind::SocketHeadCapScrew)
        );
        assert_eq!(
            StandardKind::from_key("iso4017"),
            Some(StandardKind::HexBolt)
        );
        assert_eq!(StandardKind::from_key("din912"), None);
    }
}
