//! Sheet metal (P19): model data part pelat lipat dan geometri murninya.
//!
//! Sebuah part pelat = pelat dasar (poligon bersisi lurus, tebal `t`) plus
//! flange pada sisi-sisinya. Tiap flange adalah RANTAI segmen "tekuk lalu
//! lurus", sehingga edge flange (1 segmen), hem (1 segmen 180°), dan jog
//! (2 segmen berlawanan arah) memakai jalur yang sama. Dari model ini
//! diturunkan: penampang flange (untuk di-extrude kernel), panjang
//! bentangan (bend allowance), dan pola datar + garis tekuk.
//!
//! Tanpa kernel: semua di sini aritmetika 2D yang bisa diuji murni.

use serde::{Deserialize, Serialize};

/// K-factor bawaan bila material tidak dikenal (baja lunak, tekuk udara).
pub const DEFAULT_K_FACTOR: f64 = 0.44;

/// K-factor tipikal per kunci pustaka material (tekuk udara, R ≈ t). Angka
/// bengkel yang umum dipakai sebagai titik awal; untuk produksi, kalibrasi
/// dengan uji tekuk dan isi tabel bend deduction sendiri.
pub fn default_k_factor(material_key: &str) -> f64 {
    match material_key.to_ascii_lowercase().as_str() {
        "al_6061_t6" | "al_7075_t6" => 0.40,
        "copper" | "brass" => 0.42,
        "aisi_304" => 0.45,
        "s235" | "s355" | "aisi_1045" => 0.44,
        "ti_6al_4v" => 0.46,
        _ => DEFAULT_K_FACTOR,
    }
}

/// Bend allowance `BA = θ·(R + k·t)` (θ radian): panjang busur sumbu netral.
pub fn bend_allowance(angle_deg: f64, radius: f64, k_factor: f64, thickness: f64) -> f64 {
    angle_deg.abs().to_radians() * (radius + k_factor * thickness)
}

/// Bend deduction `BD = 2·OSSB − BA`, OSSB = (R + t)·tan(θ/2). Hanya
/// terdefinisi untuk sudut < 180°.
pub fn bend_deduction(angle_deg: f64, radius: f64, k_factor: f64, thickness: f64) -> Option<f64> {
    let a = angle_deg.abs();
    if a >= 180.0 {
        return None;
    }
    let ossb = (radius + thickness) * (a.to_radians() / 2.0).tan();
    Some(2.0 * ossb - bend_allowance(a, radius, k_factor, thickness))
}

/// Satu baris tabel bend deduction kustom.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BendTableRow {
    pub thickness: f64,
    pub radius: f64,
    pub angle_deg: f64,
    pub deduction: f64,
}

/// Tabel bend deduction kustom (hasil uji tekuk bengkel).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BendTable {
    pub rows: Vec<BendTableRow>,
}

impl BendTable {
    /// CSV berkolom `thickness,radius,angle_deg,deduction` (header wajib).
    pub fn from_csv(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty());
        let (_, header) = lines.next().ok_or("tabel bend deduction kosong")?;
        let header: Vec<&str> = header.split(',').map(str::trim).collect();
        if header != ["thickness", "radius", "angle_deg", "deduction"] {
            return Err("header harus: thickness,radius,angle_deg,deduction".to_string());
        }
        let mut rows = Vec::new();
        for (idx, line) in lines {
            let cells: Vec<f64> = line
                .split(',')
                .map(|c| c.trim().parse::<f64>())
                .collect::<Result<_, _>>()
                .map_err(|e| format!("baris {}: {e}", idx + 1))?;
            let [thickness, radius, angle_deg, deduction] = cells[..] else {
                return Err(format!("baris {}: harus 4 kolom", idx + 1));
            };
            if !(thickness > 0.0 && radius >= 0.0 && angle_deg > 0.0 && angle_deg < 180.0) {
                return Err(format!("baris {}: nilai di luar domain", idx + 1));
            }
            rows.push(BendTableRow {
                thickness,
                radius,
                angle_deg,
                deduction,
            });
        }
        Ok(Self { rows })
    }

    /// Bend allowance dari tabel untuk kombinasi yang cocok (toleransi
    /// 1e-6); `None` → pemanggil memakai rumus k-factor.
    pub fn allowance(&self, thickness: f64, radius: f64, angle_deg: f64) -> Option<f64> {
        let a = angle_deg.abs();
        let near = |x: f64, y: f64| (x - y).abs() < 1e-6;
        let row = self.rows.iter().find(|r| {
            near(r.thickness, thickness) && near(r.radius, radius) && near(r.angle_deg, a)
        })?;
        let ossb = (radius + thickness) * (a.to_radians() / 2.0).tan();
        Some(2.0 * ossb - row.deduction)
    }
}

/// Satu "tekuk lalu lurus" pada rantai flange.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BendSegment {
    /// Sudut tekuk bertanda (derajat): positif = ke arah normal pelat.
    pub angle_deg: f64,
    /// Jari-jari DALAM tekukan.
    pub radius: f64,
    /// Panjang bagian lurus setelah tekukan.
    pub length: f64,
}

/// Bentuk relief di ujung tekukan (dicatat; lihat catatan modul engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReliefKind {
    #[default]
    None,
    Rect,
    Obround,
}

/// Flange pada satu sisi pelat dasar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flange {
    /// Id op pembuatnya.
    pub id: String,
    /// Indeks sisi poligon dasar (`outline[i] → outline[i+1]`).
    pub edge: usize,
    pub segments: Vec<BendSegment>,
    #[serde(default)]
    pub relief: ReliefKind,
}

/// Model lengkap satu part pelat lipat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetMetalModel {
    /// Poligon dasar berlawanan arah jarum jam, koordinat bidang sketsa.
    pub outline: Vec<[f64; 2]>,
    pub thickness: f64,
    pub default_radius: f64,
    pub k_factor: f64,
    pub flanges: Vec<Flange>,
    #[serde(default)]
    pub bend_table: BendTable,
}

/// Segmen batas penampang flange di koordinat `(s, h)`: `s` keluar dari
/// sisi pelat, `h` searah normal pelat (pelat dasar di `0 ≤ h ≤ t`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SectionSegment {
    Line {
        start: [f64; 2],
        end: [f64; 2],
    },
    /// Busur 3 titik (awal, titik di busur, akhir).
    Arc {
        start: [f64; 2],
        via: [f64; 2],
        end: [f64; 2],
    },
}

/// Garis tekuk pada pola datar.
#[derive(Debug, Clone, PartialEq)]
pub struct BendLine {
    pub a: [f64; 2],
    pub b: [f64; 2],
    /// Tekuk ke arah normal pelat (layer `BEND_UP`) atau sebaliknya.
    pub up: bool,
    pub angle_deg: f64,
    pub radius: f64,
    pub flange: String,
}

/// Pola bentangan: pelat dasar + satu persegi panjang per flange.
#[derive(Debug, Clone, PartialEq)]
pub struct FlatPattern {
    /// Poligon tertutup: indeks 0 = pelat dasar, sisanya strip flange.
    pub polygons: Vec<Vec<[f64; 2]>>,
    /// Garis luar (sisi dasar tanpa flange + tiga sisi luar tiap strip).
    pub outline: Vec<([f64; 2], [f64; 2])>,
    pub bend_lines: Vec<BendLine>,
}

fn rot(v: [f64; 2], angle: f64) -> [f64; 2] {
    let (s, c) = angle.sin_cos();
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}

fn add(a: [f64; 2], b: [f64; 2], k: f64) -> [f64; 2] {
    [a[0] + b[0] * k, a[1] + b[1] * k]
}

/// Luas bertanda poligon (positif = berlawanan arah jarum jam).
pub fn signed_area(poly: &[[f64; 2]]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (p, q) = (poly[i], poly[(i + 1) % n]);
            p[0] * q[1] - q[0] * p[1]
        })
        .sum::<f64>()
        / 2.0
}

fn segments_cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let orient = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    let eps = 1e-9;
    let (d1, d2) = (orient(a, b, c), orient(a, b, d));
    let (d3, d4) = (orient(c, d, a), orient(c, d, b));
    // Hanya perpotongan sejati (bukan sekadar bersentuhan di ujung/sisi).
    d1 * d2 < -eps && d3 * d4 < -eps
}

fn point_strictly_inside(p: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let n = poly.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        // Titik di batas tidak dianggap di dalam.
        let cross = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        let dot = (p[0] - a[0]) * (p[0] - b[0]) + (p[1] - a[1]) * (p[1] - b[1]);
        if cross.abs() < 1e-9 && dot <= 1e-9 {
            return false;
        }
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

fn polygons_overlap(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    for i in 0..a.len() {
        for j in 0..b.len() {
            if segments_cross(a[i], a[(i + 1) % a.len()], b[j], b[(j + 1) % b.len()]) {
                return true;
            }
        }
    }
    let centroid = |p: &[[f64; 2]]| {
        let n = p.len() as f64;
        [
            p.iter().map(|q| q[0]).sum::<f64>() / n,
            p.iter().map(|q| q[1]).sum::<f64>() / n,
        ]
    };
    point_strictly_inside(centroid(a), b) || point_strictly_inside(centroid(b), a)
}

impl SheetMetalModel {
    /// Periksa invarian model; pesan menyebut nilai yang salah.
    pub fn validate(&self) -> Result<(), String> {
        if self.outline.len() < 3 {
            return Err("pelat dasar butuh minimal 3 titik".into());
        }
        if !(self.thickness.is_finite() && self.thickness > 0.0) {
            return Err(format!("tebal pelat harus > 0 (dapat {})", self.thickness));
        }
        if !(0.0..=1.0).contains(&self.k_factor) {
            return Err(format!("k-factor harus 0..1 (dapat {})", self.k_factor));
        }
        if signed_area(&self.outline) <= 1e-9 {
            return Err("poligon dasar harus berlawanan arah jarum jam dan berluas".into());
        }
        let mut used = std::collections::BTreeSet::new();
        for f in &self.flanges {
            if f.edge >= self.outline.len() {
                return Err(format!("flange '{}': sisi {} tidak ada", f.id, f.edge));
            }
            if !used.insert(f.edge) {
                return Err(format!(
                    "flange '{}': sisi {} sudah punya flange",
                    f.id, f.edge
                ));
            }
            if f.segments.is_empty() {
                return Err(format!("flange '{}' tanpa segmen", f.id));
            }
            for s in &f.segments {
                let a = s.angle_deg.abs();
                if !(a > 0.0 && a <= 180.0) {
                    return Err(format!(
                        "flange '{}': sudut tekuk harus 0 < |sudut| <= 180 (dapat {})",
                        f.id, s.angle_deg
                    ));
                }
                if !(s.radius.is_finite() && s.radius >= 0.0) {
                    return Err(format!("flange '{}': radius harus >= 0", f.id));
                }
                if !(s.length.is_finite() && s.length >= 0.0) {
                    return Err(format!("flange '{}': panjang harus >= 0", f.id));
                }
            }
        }
        Ok(())
    }

    /// Titik awal/akhir sisi `edge` dan arah satuan keluar poligon.
    pub fn edge_frame(&self, edge: usize) -> Option<([f64; 2], [f64; 2], [f64; 2])> {
        let n = self.outline.len();
        let (a, b) = (*self.outline.get(edge)?, self.outline[(edge + 1) % n]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        (len > 1e-9).then(|| (a, b, [dy / len, -dx / len]))
    }

    /// Bend allowance satu segmen (tabel kustom dulu, lalu rumus k-factor).
    pub fn allowance(&self, seg: &BendSegment) -> f64 {
        self.bend_table
            .allowance(self.thickness, seg.radius, seg.angle_deg)
            .unwrap_or_else(|| {
                bend_allowance(seg.angle_deg, seg.radius, self.k_factor, self.thickness)
            })
    }

    /// Panjang bentangan flange: `Σ (BA + panjang lurus)`.
    pub fn developed_length(&self, flange: &Flange) -> f64 {
        flange
            .segments
            .iter()
            .map(|s| self.allowance(s) + s.length)
            .sum()
    }

    /// Batas tertutup penampang flange di koordinat `(s, h)`, berlawanan
    /// arah jarum jam, mulai dari `(0, 0)` (tepi bawah sisi pelat).
    pub fn cross_section(&self, flange: &Flange) -> Vec<SectionSegment> {
        let t = self.thickness;
        // Jalan di sisi "bawah" dengan arah `phi`; sisi "atas" = kiri + t.
        let mut p = [0.0, 0.0];
        let mut phi = 0.0_f64;
        let normal = |phi: f64| rot([0.0, 1.0], phi);
        let mut bottom: Vec<SectionSegment> = Vec::new();
        let mut top: Vec<SectionSegment> = Vec::new();
        for seg in &flange.segments {
            let theta = seg.angle_deg.to_radians();
            let n0 = normal(phi);
            // Tekuk ke kiri: sisi atas di dalam (radius R); ke kanan: sisi bawah.
            let (center, r_bottom, r_top) = if theta > 0.0 {
                (add(p, n0, t + seg.radius), t + seg.radius, seg.radius)
            } else {
                (add(p, n0, -seg.radius), seg.radius, t + seg.radius)
            };
            let side = if theta > 0.0 { -1.0 } else { 1.0 };
            let on_arc = |r: f64, angle: f64| add(center, normal(phi + angle), side * r);
            let (p1, mid_b, mid_t) = (
                on_arc(r_bottom, theta),
                on_arc(r_bottom, theta / 2.0),
                on_arc(r_top, theta / 2.0),
            );
            let (top0, top1) = (on_arc(r_top, 0.0), on_arc(r_top, theta));
            if r_bottom > 1e-9 {
                bottom.push(SectionSegment::Arc {
                    start: p,
                    via: mid_b,
                    end: p1,
                });
            }
            if r_top > 1e-9 {
                top.push(SectionSegment::Arc {
                    start: top1,
                    via: mid_t,
                    end: top0,
                });
            }
            phi += theta;
            p = p1;
            if seg.length > 1e-9 {
                let dir = rot([1.0, 0.0], phi);
                let p2 = add(p, dir, seg.length);
                bottom.push(SectionSegment::Line { start: p, end: p2 });
                let n1 = normal(phi);
                top.push(SectionSegment::Line {
                    start: add(p2, n1, t),
                    end: add(p, n1, t),
                });
                p = p2;
            }
        }
        let end_top = add(p, normal(phi), t);
        let mut out = bottom;
        out.push(SectionSegment::Line {
            start: p,
            end: end_top,
        });
        out.extend(top.into_iter().rev());
        out.push(SectionSegment::Line {
            start: [0.0, t],
            end: [0.0, 0.0],
        });
        out
    }

    /// Luas penampang flange (untuk memeriksa volume solid terlipat).
    pub fn cross_section_area(&self, flange: &Flange) -> f64 {
        let t = self.thickness;
        flange
            .segments
            .iter()
            .map(|s| {
                let theta = s.angle_deg.abs().to_radians();
                theta / 2.0 * ((s.radius + t).powi(2) - s.radius.powi(2)) + s.length * t
            })
            .sum()
    }

    /// Volume solid terlipat secara analitik (pelat dasar + semua flange).
    pub fn folded_volume(&self) -> f64 {
        let base = signed_area(&self.outline) * self.thickness;
        let flanges: f64 = self
            .flanges
            .iter()
            .filter_map(|f| {
                let (a, b, _) = self.edge_frame(f.edge)?;
                let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
                Some(self.cross_section_area(f) * len)
            })
            .sum();
        base + flanges
    }

    /// Pola bentangan. `Err` bila strip flange saling tumpang-tindih atau
    /// menimpa pelat dasar (pola tidak bisa dipotong dari satu lembar).
    pub fn flat_pattern(&self) -> Result<FlatPattern, String> {
        self.validate()?;
        let n = self.outline.len();
        let mut polygons = vec![self.outline.clone()];
        let mut outline = Vec::new();
        let mut bend_lines = Vec::new();
        for i in 0..n {
            let (a, b) = (self.outline[i], self.outline[(i + 1) % n]);
            let Some(flange) = self.flanges.iter().find(|f| f.edge == i) else {
                outline.push((a, b));
                continue;
            };
            let Some((_, _, out)) = self.edge_frame(i) else {
                return Err(format!("sisi {i} berpanjang nol"));
            };
            let total = self.developed_length(flange);
            let (a2, b2) = (add(a, out, total), add(b, out, total));
            polygons.push(vec![a, a2, b2, b]);
            outline.extend([(a, a2), (a2, b2), (b2, b)]);
            let mut at = 0.0;
            for seg in &flange.segments {
                let ba = self.allowance(seg);
                // Garis tekuk di tengah zona tekuk.
                let s = at + ba / 2.0;
                bend_lines.push(BendLine {
                    a: add(a, out, s),
                    b: add(b, out, s),
                    up: seg.angle_deg > 0.0,
                    angle_deg: seg.angle_deg.abs(),
                    radius: seg.radius,
                    flange: flange.id.clone(),
                });
                at += ba + seg.length;
            }
        }
        for i in 1..polygons.len() {
            for j in (i + 1)..polygons.len() {
                if polygons_overlap(&polygons[i], &polygons[j]) {
                    return Err(format!(
                        "pola datar berpotongan sendiri: strip flange '{}' menimpa '{}'",
                        self.flanges[i - 1].id,
                        self.flanges[j - 1].id
                    ));
                }
            }
            if polygons_overlap(&polygons[i], &polygons[0]) {
                return Err(format!(
                    "pola datar berpotongan sendiri: strip flange '{}' menimpa pelat dasar",
                    self.flanges[i - 1].id
                ));
            }
        }
        Ok(FlatPattern {
            polygons,
            outline,
            bend_lines,
        })
    }

    /// Luas pola datar (jumlah luas pelat dasar + strip flange).
    pub fn flat_area(&self) -> f64 {
        let strips: f64 = self
            .flanges
            .iter()
            .filter_map(|f| {
                let (a, b, _) = self.edge_frame(f.edge)?;
                let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
                Some(len * self.developed_length(f))
            })
            .sum();
        signed_area(&self.outline) + strips
    }

    /// Rasio radius dalam terkecil terhadap tebal (`None` = tanpa tekukan).
    pub fn min_bend_ratio(&self) -> Option<f64> {
        self.flanges
            .iter()
            .flat_map(|f| f.segments.iter())
            .map(|s| s.radius / self.thickness)
            .min_by(f64::total_cmp)
    }

    /// Panjang lurus flange terpendek (segmen terakhir tiap flange).
    pub fn min_flange_length(&self) -> Option<f64> {
        self.flanges
            .iter()
            .filter_map(|f| f.segments.last())
            .map(|s| s.length)
            .min_by(f64::total_cmp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_model(length: f64, angle: f64) -> SheetMetalModel {
        SheetMetalModel {
            outline: vec![[0.0, 0.0], [100.0, 0.0], [100.0, 60.0], [0.0, 60.0]],
            thickness: 2.0,
            default_radius: 2.0,
            k_factor: 0.44,
            flanges: (0..4)
                .map(|edge| Flange {
                    id: format!("f{edge}"),
                    edge,
                    segments: vec![BendSegment {
                        angle_deg: angle,
                        radius: 2.0,
                        length,
                    }],
                    relief: ReliefKind::None,
                })
                .collect(),
            bend_table: BendTable::default(),
        }
    }

    #[test]
    fn sheet_metal_bend_allowance_formula() {
        // 90°, R = 2, t = 2, k = 0.44 → π/2 · (2 + 0.88).
        let ba = bend_allowance(90.0, 2.0, 0.44, 2.0);
        assert!((ba - std::f64::consts::FRAC_PI_2 * 2.88).abs() < 1e-12);
        assert_eq!(bend_allowance(-90.0, 2.0, 0.44, 2.0), ba);
        let bd = bend_deduction(90.0, 2.0, 0.44, 2.0).unwrap();
        assert!((bd - (8.0 - ba)).abs() < 1e-12);
        assert_eq!(bend_deduction(180.0, 1.0, 0.4, 1.0), None);
        assert_eq!(default_k_factor("AL_6061_T6"), 0.40);
        assert_eq!(default_k_factor("unknown"), DEFAULT_K_FACTOR);
    }

    #[test]
    fn sheet_metal_box_flat_pattern_adds_allowances() {
        let m = box_model(20.0, 90.0);
        m.validate().unwrap();
        let ba = bend_allowance(90.0, 2.0, 0.44, 2.0);
        for f in &m.flanges {
            assert!((m.developed_length(f) - (20.0 + ba)).abs() < 1e-12);
        }
        let flat = m.flat_pattern().unwrap();
        assert_eq!(flat.polygons.len(), 5);
        assert_eq!(flat.bend_lines.len(), 4);
        assert!(flat.bend_lines.iter().all(|b| b.up && b.angle_deg == 90.0));
        // Bentangan total di kedua arah = datar + 2·(BA + L).
        let xs: Vec<f64> = flat.polygons.iter().flatten().map(|p| p[0]).collect();
        let ys: Vec<f64> = flat.polygons.iter().flatten().map(|p| p[1]).collect();
        let span = |v: &[f64]| {
            v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min)
        };
        assert!((span(&xs) - (100.0 + 2.0 * (20.0 + ba))).abs() < 0.01);
        assert!((span(&ys) - (60.0 + 2.0 * (20.0 + ba))).abs() < 0.01);
        // Garis tekuk pertama di tengah zona tekuk sisi bawah (y = −BA/2).
        assert!((flat.bend_lines[0].a[1] + ba / 2.0).abs() < 1e-12);
        // 4 strip × 3 sisi luar, tanpa sisi dasar bebas.
        assert_eq!(flat.outline.len(), 12);
        assert!((m.flat_area() - (6000.0 + 2.0 * (100.0 + 60.0) * (20.0 + ba))).abs() < 1e-9);
    }

    #[test]
    fn sheet_metal_cross_section_is_closed_and_matches_area() {
        let m = box_model(20.0, 90.0);
        for angle in [90.0, -90.0, 45.0, 135.0, 180.0] {
            let f = Flange {
                id: "f".into(),
                edge: 0,
                segments: vec![BendSegment {
                    angle_deg: angle,
                    radius: 2.0,
                    length: 15.0,
                }],
                relief: ReliefKind::None,
            };
            let section = m.cross_section(&f);
            let ends = |s: &SectionSegment| match *s {
                SectionSegment::Line { start, end } | SectionSegment::Arc { start, end, .. } => {
                    (start, end)
                }
            };
            // Tertutup: akhir tiap segmen = awal segmen berikutnya.
            for (i, seg) in section.iter().enumerate() {
                let next = ends(&section[(i + 1) % section.len()]).0;
                let end = ends(seg).1;
                assert!(
                    (end[0] - next[0]).abs() < 1e-9 && (end[1] - next[1]).abs() < 1e-9,
                    "sudut {angle}: segmen {i} tidak tersambung ({end:?} → {next:?})"
                );
            }
            // Luas poligon pendekatan (busur → tali via) ≤ luas analitik, selisih kecil.
            let pts: Vec<[f64; 2]> = section
                .iter()
                .flat_map(|s| match *s {
                    SectionSegment::Line { start, .. } => vec![start],
                    SectionSegment::Arc { start, via, .. } => vec![start, via],
                })
                .collect();
            let approx = signed_area(&pts);
            let exact = m.cross_section_area(&f);
            assert!(
                approx > 0.0,
                "sudut {angle}: penampang harus CCW ({approx})"
            );
            // Satu titik-via per busur: makin besar sudut, makin kasar talinya.
            let tol = if angle.abs() > 150.0 { 0.16 } else { 0.12 };
            assert!(
                (approx - exact).abs() / exact < tol,
                "sudut {angle}: {approx} vs {exact}"
            );
        }
        // 90° ke atas: ujung flange berdiri di s = R + t, h dari R + t ke atas.
        let up = m.cross_section(&m.flanges[0]);
        let SectionSegment::Line { start, end } = up[1] else {
            panic!("segmen kedua harus bagian lurus");
        };
        assert!(
            (start[0] - 4.0).abs() < 1e-9 && (start[1] - 4.0).abs() < 1e-9,
            "{start:?}"
        );
        assert!(
            (end[0] - 4.0).abs() < 1e-9 && (end[1] - 24.0).abs() < 1e-9,
            "{end:?}"
        );
    }

    #[test]
    fn sheet_metal_jog_returns_parallel_to_base() {
        let m = box_model(10.0, 90.0);
        let jog = Flange {
            id: "j".into(),
            edge: 0,
            segments: vec![
                BendSegment {
                    angle_deg: 60.0,
                    radius: 2.0,
                    length: 5.0,
                },
                BendSegment {
                    angle_deg: -60.0,
                    radius: 2.0,
                    length: 12.0,
                },
            ],
            relief: ReliefKind::None,
        };
        let section = m.cross_section(&jog);
        // Bagian lurus terakhir sisi bawah sejajar pelat dasar (h tetap).
        let last_bottom = section
            .iter()
            .take_while(|s| !matches!(s, SectionSegment::Line { start, end } if (start[0] - end[0]).abs() < 1e-9 && (end[1] - start[1] - 2.0).abs() < 1e-9))
            .last()
            .unwrap();
        let SectionSegment::Line { start, end } = *last_bottom else {
            panic!("harus garis");
        };
        assert!((start[1] - end[1]).abs() < 1e-9, "{start:?} {end:?}");
        assert!((end[0] - start[0] - 12.0).abs() < 1e-9);
        // Offset = (2R + t)(1 − cos a) + s·sin a.
        let a = 60.0_f64.to_radians();
        let offset = 6.0 * (1.0 - a.cos()) + 5.0 * a.sin();
        assert!((start[1] - offset).abs() < 1e-9, "{} vs {offset}", start[1]);
    }

    #[test]
    fn sheet_metal_flat_pattern_detects_overlap() {
        // Poligon L: flange panjang di dua sisi sudut cekung saling menimpa.
        let m = SheetMetalModel {
            outline: vec![
                [0.0, 0.0],
                [60.0, 0.0],
                [60.0, 20.0],
                [20.0, 20.0],
                [20.0, 60.0],
                [0.0, 60.0],
            ],
            thickness: 1.0,
            default_radius: 1.0,
            k_factor: 0.44,
            flanges: [2usize, 3]
                .iter()
                .map(|&edge| Flange {
                    id: format!("f{edge}"),
                    edge,
                    segments: vec![BendSegment {
                        angle_deg: 90.0,
                        radius: 1.0,
                        length: 30.0,
                    }],
                    relief: ReliefKind::None,
                })
                .collect(),
            bend_table: BendTable::default(),
        };
        let e = m.flat_pattern().unwrap_err();
        assert!(e.contains("berpotongan sendiri"), "{e}");
        // Kotak cembung dengan flange di semua sisi tidak bertabrakan.
        assert!(box_model(500.0, 90.0).flat_pattern().is_ok());
    }

    #[test]
    fn sheet_metal_validate_and_bend_table() {
        let mut m = box_model(20.0, 90.0);
        m.flanges[1].edge = 0;
        assert!(m.validate().unwrap_err().contains("sudah punya flange"));
        let mut m = box_model(20.0, 200.0);
        assert!(m.validate().unwrap_err().contains("sudut tekuk"));
        m = box_model(20.0, 90.0);
        m.outline.reverse();
        assert!(m.validate().is_err());

        let table =
            BendTable::from_csv("thickness,radius,angle_deg,deduction\n2,2,90,3.4\n").unwrap();
        // BA = 2·OSSB − BD = 8 − 3.4.
        assert!((table.allowance(2.0, 2.0, -90.0).unwrap() - 4.6).abs() < 1e-12);
        assert_eq!(table.allowance(2.0, 2.0, 45.0), None);
        let mut m = box_model(20.0, 90.0);
        m.bend_table = table;
        assert!((m.developed_length(&m.flanges[0]) - 24.6).abs() < 1e-12);
        assert!(BendTable::from_csv("a,b\n1,2\n").is_err());
        assert!(BendTable::from_csv("thickness,radius,angle_deg,deduction\n2,2,x,1\n").is_err());
        assert_eq!(box_model(7.0, 90.0).min_flange_length(), Some(7.0));
        assert_eq!(box_model(7.0, 90.0).min_bend_ratio(), Some(1.0));
    }
}
