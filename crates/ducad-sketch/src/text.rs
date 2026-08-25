//! Vektorisasi Font 2D (TTF / OTF) ke Kurva & Entitas Sketsa DuCAD.
//!
//! Mengurai outline glyph font TrueType/OpenType menggunakan `ttf-parser`,
//! mendekomposisi kurva Bézier kuadratik/kubik menjadi segmen poliline halus,
//! dan menghasilkan `Entity::Line` yang membentuk loop tertutup untuk pembuatan profil & region.

use glam::DVec2;
use serde::{Deserialize, Serialize};
use crate::entity::Entity;

/// Berkas font default bawaan yang disematkan ke dalam biner.
pub const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../fonts/default.ttf");

/// Perataan teks horizontal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Opsi konfigurasi pemformatan teks 2D.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextOptions {
    /// Tinggi huruf dalam milimeter (mm).
    pub font_height_mm: f64,
    /// Jarak ekstra antar karakter (multiplier, default 1.0).
    pub letter_spacing: f64,
    /// Jarak antar baris teks (multiplier, default 1.2).
    pub line_spacing: f64,
    /// Perataan horizontal teks.
    pub align: TextAlign,
    /// Apakah entitas hasil vektorisasi dijadikan garis konstruksi.
    pub is_construction: bool,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            font_height_mm: 10.0,
            letter_spacing: 1.0,
            line_spacing: 1.2,
            align: TextAlign::Left,
            is_construction: false,
        }
    }
}

/// Builder untuk menangkap perintah kurva dari `ttf_parser::OutlineBuilder`.
struct GlyphOutlineBuilder {
    offset: DVec2,
    scale: f64,
    current_pt: DVec2,
    contour_start: DVec2,
    entities: Vec<Entity>,
    is_construction: bool,
}

impl GlyphOutlineBuilder {
    fn new(offset: DVec2, scale: f64, is_construction: bool) -> Self {
        Self {
            offset,
            scale,
            current_pt: offset,
            contour_start: offset,
            entities: Vec::new(),
            is_construction,
        }
    }

    fn to_world(&self, x: f32, y: f32) -> DVec2 {
        DVec2::new(
            self.offset.x + (x as f64) * self.scale,
            self.offset.y + (y as f64) * self.scale,
        )
    }

    fn add_line(&mut self, start: DVec2, end: DVec2) {
        if (start - end).length_squared() > 1e-10 {
            self.entities.push(Entity::Line {
                start,
                end,
                is_construction: self.is_construction,
            });
        }
    }
}

impl ttf_parser::OutlineBuilder for GlyphOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let pt = self.to_world(x, y);
        self.current_pt = pt;
        self.contour_start = pt;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let pt = self.to_world(x, y);
        let start = self.current_pt;
        self.add_line(start, pt);
        self.current_pt = pt;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let p0 = self.current_pt;
        let p1 = self.to_world(x1, y1);
        let p2 = self.to_world(x, y);

        // Subdivisi adaptif kurva Bézier kuadratik
        let chord_len = (p2 - p0).length();
        let num_segments = if chord_len < 0.25 {
            1
        } else if chord_len < 0.8 {
            2
        } else if chord_len < 2.5 {
            4
        } else {
            ((chord_len * 2.0).ceil() as usize).clamp(4, 10)
        };

        let mut prev = p0;
        for step in 1..=num_segments {
            let t = (step as f64) / (num_segments as f64);
            let pt = if step == num_segments {
                p2
            } else {
                let one_minus_t = 1.0 - t;
                one_minus_t * one_minus_t * p0 + 2.0 * one_minus_t * t * p1 + t * t * p2
            };
            self.add_line(prev, pt);
            prev = pt;
        }
        self.current_pt = p2;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p0 = self.current_pt;
        let p1 = self.to_world(x1, y1);
        let p2 = self.to_world(x2, y2);
        let p3 = self.to_world(x, y);

        // Subdivisi adaptif kurva Bézier kubik
        let chord_len = (p3 - p0).length();
        let num_segments = if chord_len < 0.25 {
            1
        } else if chord_len < 0.8 {
            2
        } else if chord_len < 2.5 {
            5
        } else {
            ((chord_len * 2.5).ceil() as usize).clamp(5, 14)
        };

        let mut prev = p0;
        for step in 1..=num_segments {
            let t = (step as f64) / (num_segments as f64);
            let pt = if step == num_segments {
                p3
            } else {
                let one_minus_t = 1.0 - t;
                one_minus_t * one_minus_t * one_minus_t * p0
                    + 3.0 * one_minus_t * one_minus_t * t * p1
                    + 3.0 * one_minus_t * t * t * p2
                    + t * t * t * p3
            };
            self.add_line(prev, pt);
            prev = pt;
        }
        self.current_pt = p3;
    }

    fn close(&mut self) {
        let start = self.current_pt;
        let end = self.contour_start;
        self.add_line(start, end);
        self.current_pt = end;
    }
}

/// Vektorisasi string teks menjadi daftar `Entity` sketsa.
pub fn text_to_entities(
    text: &str,
    origin: DVec2,
    options: &TextOptions,
    custom_font_bytes: Option<&[u8]>,
) -> Result<Vec<Entity>, String> {
    if text.is_empty() {
        return Ok(Vec::new());
    }

    let font_data = custom_font_bytes.unwrap_or(DEFAULT_FONT_BYTES);
    let face = ttf_parser::Face::parse(font_data, 0)
        .map_err(|e| format!("Gagal mem-parsing berkas font TTF/OTF: {e}"))?;

    let upem = face.units_per_em() as f64;
    let ascender = face.ascender() as f64;
    let descender = face.descender() as f64;
    let em_height = (ascender - descender).max(upem);
    let scale = (options.font_height_mm / em_height).max(1e-6);

    let lines: Vec<&str> = text.lines().collect();
    let line_height_mm = options.font_height_mm * options.line_spacing;

    let mut all_entities = Vec::new();

    for (line_idx, line_str) in lines.iter().enumerate() {
        // Hitung total lebar baris untuk perataan horizontal (Alignment)
        let mut line_width = 0.0;
        for c in line_str.chars() {
            if let Some(glyph_id) = face.glyph_index(c) {
                let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f64 * scale;
                line_width += advance * options.letter_spacing;
            }
        }

        let align_offset_x = match options.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => -line_width * 0.5,
            TextAlign::Right => -line_width,
        };

        let mut current_x = origin.x + align_offset_x;
        let current_y = origin.y - (line_idx as f64) * line_height_mm;

        for c in line_str.chars() {
            if let Some(glyph_id) = face.glyph_index(c) {
                let mut builder = GlyphOutlineBuilder::new(
                    DVec2::new(current_x, current_y),
                    scale,
                    options.is_construction,
                );

                if let Some(_bbox) = face.outline_glyph(glyph_id, &mut builder) {
                    all_entities.extend(builder.entities);
                }

                let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f64 * scale;
                current_x += advance * options.letter_spacing;
            }
        }
    }

    Ok(all_entities)
}
