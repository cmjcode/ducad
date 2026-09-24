use glam::DVec2;
use serde::{Deserialize, Serialize};

/// Warna sRGB dengan alpha tidak premultiplied, komponen 0.0..=1.0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rgba(pub [f32; 4]);

impl Rgba {
    pub const WHITE: Self = Self([1.0, 1.0, 1.0, 1.0]);
    pub const BLACK: Self = Self([0.0, 0.0, 0.0, 1.0]);
    pub const TRANSPARENT: Self = Self([0.0, 0.0, 0.0, 0.0]);

    /// Parse warna dari hex string (`#rrggbb`, `rrggbb`, `#rrggbbaa`, `rrggbbaa`).
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim().strip_prefix('#').unwrap_or(s.trim());
        if s.len() == 6 {
            let r = u8::from_str_radix(&s[0..2], 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&s[2..4], 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&s[4..6], 16).ok()? as f32 / 255.0;
            Some(Self([r, g, b, 1.0]))
        } else if s.len() == 8 {
            let r = u8::from_str_radix(&s[0..2], 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&s[2..4], 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&s[4..6], 16).ok()? as f32 / 255.0;
            let a = u8::from_str_radix(&s[6..8], 16).ok()? as f32 / 255.0;
            Some(Self([r, g, b, a]))
        } else {
            None
        }
    }

    /// Konversi ke string hex (`#rrggbb` jika alpha == 1.0, `#rrggbbaa` jika ada transparansi).
    pub fn to_hex(self) -> String {
        let r = (self.0[0].clamp(0.0, 1.0) * 255.0).round() as u8;
        let g = (self.0[1].clamp(0.0, 1.0) * 255.0).round() as u8;
        let b = (self.0[2].clamp(0.0, 1.0) * 255.0).round() as u8;
        let a = (self.0[3].clamp(0.0, 1.0) * 255.0).round() as u8;

        if a == 255 {
            format!("#{r:02x}{g:02x}{b:02x}")
        } else {
            format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
    }
}

/// Cat gambar (fill atau stroke): solid atau gradient linier/radial.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Paint {
    Solid(Rgba),
    /// Gradien linier dalam koordinat sketch (mm), stops terurut 0..=1.
    Linear {
        from: DVec2,
        to: DVec2,
        stops: Vec<(f64, Rgba)>,
    },
    /// Gradien radial dalam koordinat sketch (mm), stops terurut 0..=1.
    Radial {
        center: DVec2,
        radius: f64,
        stops: Vec<(f64, Rgba)>,
    },
}

impl Paint {
    /// Rata-rata warna (dipakai untuk material 3D M3).
    pub fn average_color(&self) -> Rgba {
        match self {
            Paint::Solid(rgba) => *rgba,
            Paint::Linear { stops, .. } | Paint::Radial { stops, .. } => {
                if stops.is_empty() {
                    return Rgba::BLACK;
                }
                if stops.len() == 1 {
                    return stops[0].1;
                }
                if stops.len() == 2 {
                    let c0 = stops[0].1 .0;
                    let c1 = stops[1].1 .0;
                    return Rgba([
                        (c0[0] + c1[0]) * 0.5,
                        (c0[1] + c1[1]) * 0.5,
                        (c0[2] + c1[2]) * 0.5,
                        (c0[3] + c1[3]) * 0.5,
                    ]);
                }
                // Integrasi trapesium berbobot panjang stop
                let mut sum = [0.0f32; 4];
                let mut total_w = 0.0f64;
                for i in 0..(stops.len() - 1) {
                    let (t0, c0) = stops[i];
                    let (t1, c1) = stops[i + 1];
                    let dt = (t1 - t0).max(0.0);
                    if dt > 0.0 {
                        total_w += dt;
                        for (ch, sum_ch) in sum.iter_mut().enumerate() {
                            *sum_ch += (c0.0[ch] + c1.0[ch]) * 0.5 * (dt as f32);
                        }
                    }
                }
                if total_w > 0.0 {
                    Rgba([
                        sum[0] / total_w as f32,
                        sum[1] / total_w as f32,
                        sum[2] / total_w as f32,
                        sum[3] / total_w as f32,
                    ])
                } else {
                    let count = stops.len() as f32;
                    let mut s = [0.0f32; 4];
                    for (_, c) in stops {
                        for (ch, s_ch) in s.iter_mut().enumerate() {
                            *s_ch += c.0[ch];
                        }
                    }
                    Rgba([s[0] / count, s[1] / count, s[2] / count, s[3] / count])
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
}

/// Konfigurasi stroke (outline) suatu entitas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrokeStyle {
    pub paint: Paint,
    pub width_mm: f64,
    #[serde(default)]
    pub dash: Vec<f64>,
    #[serde(default)]
    pub cap: LineCap,
    #[serde(default)]
    pub join: LineJoin,
}

fn one() -> f32 {
    1.0
}

/// Gaya visual entitas: fill, fill_rule, stroke, opacity, blend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Style {
    #[serde(default)]
    pub fill: Option<Paint>,
    #[serde(default)]
    pub fill_rule: FillRule,
    #[serde(default)]
    pub stroke: Option<StrokeStyle>,
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default)]
    pub blend: BlendMode,
}

impl Default for Style {
    fn default() -> Self {
        Self::cad_default()
    }
}

impl Style {
    /// Gaya "garis CAD": tanpa fill, stroke hairline warna tema. Dipakai bila
    /// entitas tidak punya entri di `Sketch::styles` — menjaga tampilan lama.
    pub fn cad_default() -> Self {
        Self {
            fill: None,
            fill_rule: FillRule::NonZero,
            stroke: Some(StrokeStyle {
                paint: Paint::Solid(Rgba([1.0, 1.0, 1.0, 1.0])),
                width_mm: 0.25,
                dash: Vec::new(),
                cap: LineCap::Butt,
                join: LineJoin::Miter,
            }),
            opacity: 1.0,
            blend: BlendMode::Normal,
        }
    }

    /// Validasi konsistensi gaya visual.
    pub fn validate(&self) -> Result<(), String> {
        if self.opacity < 0.0 || self.opacity > 1.0 || self.opacity.is_nan() {
            return Err(format!("opacity must be in 0..=1, got {}", self.opacity));
        }
        if let Some(ref stroke) = self.stroke {
            if stroke.width_mm <= 0.0 || stroke.width_mm.is_nan() {
                return Err(format!("stroke width_mm must be > 0, got {}", stroke.width_mm));
            }
            if stroke.dash.len() % 2 != 0 {
                return Err("stroke dash pattern length must be even".into());
            }
            for d in &stroke.dash {
                if *d <= 0.0 || d.is_nan() {
                    return Err(format!("dash length must be > 0, got {}", d));
                }
            }
            Self::validate_paint(&stroke.paint)?;
        }
        if let Some(ref fill) = self.fill {
            Self::validate_paint(fill)?;
        }
        Ok(())
    }

    fn validate_paint(paint: &Paint) -> Result<(), String> {
        match paint {
            Paint::Solid(_) => Ok(()),
            Paint::Linear { stops, .. } | Paint::Radial { stops, .. } => {
                let mut prev = -1.0;
                for (t, _) in stops {
                    if *t < 0.0 || *t > 1.0 || t.is_nan() {
                        return Err(format!("gradient stop must be in 0..=1, got {}", t));
                    }
                    if *t < prev {
                        return Err("gradient stops must be ordered 0..=1".into());
                    }
                    prev = *t;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Entity;
    use crate::sketch::Sketch;

    #[test]
    fn rgba_hex_roundtrip() {
        // 6 digit hex
        let c6 = Rgba::from_hex("#ff8800").expect("valid 6-digit hex");
        assert!((c6.0[0] - 1.0).abs() < 1e-3);
        assert!((c6.0[1] - 0.5333).abs() < 1e-3);
        assert!((c6.0[2] - 0.0).abs() < 1e-3);
        assert!((c6.0[3] - 1.0).abs() < 1e-3);
        assert_eq!(c6.to_hex(), "#ff8800");

        // 8 digit hex
        let c8 = Rgba::from_hex("#ff8800aa").expect("valid 8-digit hex");
        assert!((c8.0[0] - 1.0).abs() < 1e-3);
        assert!((c8.0[1] - 0.5333).abs() < 1e-3);
        assert!((c8.0[2] - 0.0).abs() < 1e-3);
        assert!((c8.0[3] - 0.6666).abs() < 1e-3);
        assert_eq!(c8.to_hex(), "#ff8800aa");

        // Hex without #
        let c_nohash = Rgba::from_hex("00ff00").expect("valid hex without #");
        assert_eq!(c_nohash.to_hex(), "#00ff00");

        // Invalid hex
        assert_eq!(Rgba::from_hex("zzz"), None);
        assert_eq!(Rgba::from_hex("#123"), None);
        assert_eq!(Rgba::from_hex(""), None);
        assert_eq!(Rgba::from_hex("#1234567"), None);
        assert_eq!(Rgba::from_hex("#123456789"), None);
    }

    #[test]
    fn paint_average_color_of_gradient_is_midpoint() {
        let c0 = Rgba([0.0, 0.2, 0.4, 1.0]);
        let c1 = Rgba([1.0, 0.8, 0.6, 1.0]);

        let grad = Paint::Linear {
            from: DVec2::ZERO,
            to: DVec2::new(10.0, 0.0),
            stops: vec![(0.0, c0), (1.0, c1)],
        };

        let avg = grad.average_color();
        assert!((avg.0[0] - 0.5).abs() < 1e-5);
        assert!((avg.0[1] - 0.5).abs() < 1e-5);
        assert!((avg.0[2] - 0.5).abs() < 1e-5);
        assert!((avg.0[3] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn style_default_is_cad_default_for_unstyled_entity() {
        let mut sketch = Sketch::default();
        let id = sketch
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));

        // Entitas belum diberi entri styles
        let style = sketch.style_of(id);
        assert_eq!(style, Style::cad_default());
        assert_eq!(style.fill, None);
        assert!(style.stroke.is_some());
    }

    #[test]
    fn style_validation() {
        let valid_style = Style::cad_default();
        assert!(valid_style.validate().is_ok());

        let mut invalid_opacity = valid_style.clone();
        invalid_opacity.opacity = 1.5;
        assert!(invalid_opacity.validate().is_err());

        let mut invalid_width = valid_style.clone();
        if let Some(ref mut st) = invalid_width.stroke {
            st.width_mm = 0.0;
        }
        assert!(invalid_width.validate().is_err());

        let mut invalid_dash = valid_style.clone();
        if let Some(ref mut st) = invalid_dash.stroke {
            st.dash = vec![2.0]; // odd length
        }
        assert!(invalid_dash.validate().is_err());
    }
}

