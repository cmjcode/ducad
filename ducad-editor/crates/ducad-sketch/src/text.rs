//! Vektorisasi Font 2D (TTF / OTF) ke Kurva & Entitas Sketsa DuCAD.
//!
//! Mengurai outline glyph font TrueType/OpenType menggunakan `ttf-parser`,
//! lalu menghasilkan `Entity::Spline` tertutup per kontur glyph.
//!
//! Tiap kontur disimpan DUA KALI, sengaja:
//!
//! - `points` — hasil pencacahan (flatten) kurva jadi poliline halus. Inilah
//!   yang dibaca tampilan layar, snap, deteksi region, dan ekspor 2D.
//! - `exact` — kurva Bézier ASLI dari font, tanpa dicacah. Inilah yang dipakai
//!   saat teks dijadikan profil lalu di-extrude, sehingga dindingnya menjadi
//!   permukaan melengkung B-rep sungguhan. Sebelum ini hanya `points` yang ada,
//!   dan tiap ruas lurusnya jadi satu face datar — itulah penyebab hasil
//!   extrude teks terlihat patah-patah.

use glam::DVec2;
use serde::{Deserialize, Serialize};
use crate::entity::{Entity, EntityId, PathSeg, Subpath};
use crate::layer::{LayerId, TextId};
use crate::sketch::Sketch;
use crate::style::{BlendMode, FillRule, Paint, Rgba, Style};

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

/// Pilihan keluarga font standar atau kustom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FontPreset {
    #[default]
    Arial,
    ArialBold,
    ArialRounded,
    TimesNewRoman,
    CourierNew,
    Georgia,
    Impact,
    Trebuchet,
    Verdana,
    DefaultSans,
}

impl FontPreset {
    pub fn all() -> &'static [FontPreset] {
        &[
            FontPreset::Arial,
            FontPreset::ArialBold,
            FontPreset::ArialRounded,
            FontPreset::TimesNewRoman,
            FontPreset::CourierNew,
            FontPreset::Georgia,
            FontPreset::Impact,
            FontPreset::Trebuchet,
            FontPreset::Verdana,
            FontPreset::DefaultSans,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            FontPreset::Arial => "Arial",
            FontPreset::ArialBold => "Arial Bold",
            FontPreset::ArialRounded => "Arial Rounded",
            FontPreset::TimesNewRoman => "Times New Roman (Serif)",
            FontPreset::CourierNew => "Courier New (Monospace)",
            FontPreset::Georgia => "Georgia",
            FontPreset::Impact => "Impact (Heavy)",
            FontPreset::Trebuchet => "Trebuchet MS",
            FontPreset::Verdana => "Verdana",
            FontPreset::DefaultSans => "Standard Sans (Embedded)",
        }
    }

    /// Ambil bytes berkas font sistem atau fallback ke default bawaan.
    pub fn load_font_bytes(&self) -> Vec<u8> {
        let candidates: &[&str] = match self {
            FontPreset::Arial => &[
                "/System/Library/Fonts/Supplemental/Arial.ttf",
                "/Library/Fonts/Arial.ttf",
                "C:\\Windows\\Fonts\\arial.ttf",
                "/usr/share/fonts/truetype/msttcorefonts/Arial.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            ],
            FontPreset::ArialBold => &[
                "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
                "/Library/Fonts/Arial Bold.ttf",
                "C:\\Windows\\Fonts\\arialbd.ttf",
                "/usr/share/fonts/truetype/msttcorefonts/Arial_Bold.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            ],
            FontPreset::ArialRounded => &[
                "/System/Library/Fonts/Supplemental/Arial Rounded Bold.ttf",
                "/Library/Fonts/Arial Rounded Bold.ttf",
                "C:\\Windows\\Fonts\\ARLRDBD.TTF",
            ],
            FontPreset::TimesNewRoman => &[
                "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
                "/Library/Fonts/Times New Roman.ttf",
                "C:\\Windows\\Fonts\\times.ttf",
                "/usr/share/fonts/truetype/msttcorefonts/Times_New_Roman.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
            ],
            FontPreset::CourierNew => &[
                "/System/Library/Fonts/Supplemental/Courier New.ttf",
                "/Library/Fonts/Courier New.ttf",
                "C:\\Windows\\Fonts\\cour.ttf",
                "/usr/share/fonts/truetype/msttcorefonts/Courier_New.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            ],
            FontPreset::Georgia => &[
                "/System/Library/Fonts/Supplemental/Georgia.ttf",
                "/Library/Fonts/Georgia.ttf",
                "C:\\Windows\\Fonts\\georgia.ttf",
                "/usr/share/fonts/truetype/msttcorefonts/Georgia.ttf",
            ],
            FontPreset::Impact => &[
                "/System/Library/Fonts/Supplemental/Impact.ttf",
                "/Library/Fonts/Impact.ttf",
                "C:\\Windows\\Fonts\\impact.ttf",
                "/usr/share/fonts/truetype/msttcorefonts/Impact.ttf",
            ],
            FontPreset::Trebuchet => &[
                "/System/Library/Fonts/Supplemental/Trebuchet MS.ttf",
                "/Library/Fonts/Trebuchet MS.ttf",
                "C:\\Windows\\Fonts\\trebuc.ttf",
                "/usr/share/fonts/truetype/msttcorefonts/Trebuchet_MS.ttf",
            ],
            FontPreset::Verdana => &[
                "/System/Library/Fonts/Supplemental/Verdana.ttf",
                "/Library/Fonts/Verdana.ttf",
                "C:\\Windows\\Fonts\\verdana.ttf",
                "/usr/share/fonts/truetype/msttcorefonts/Verdana.ttf",
            ],
            FontPreset::DefaultSans => &[],
        };

        for path in candidates {
            if let Ok(bytes) = std::fs::read(path) {
                return bytes;
            }
        }

        DEFAULT_FONT_BYTES.to_vec()
    }
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
    /// Keluarga font yang dipilih.
    pub font_preset: FontPreset,
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
            font_preset: FontPreset::Arial,
            is_construction: false,
        }
    }
}

/// Builder untuk menangkap perintah kurva dari `ttf_parser::OutlineBuilder`.
struct GlyphOutlineBuilder {
    offset: DVec2,
    scale: f64,
    current_pt: DVec2,
    /// Titik awal kontur yang sedang digambar — tujuan garis penutupnya.
    contour_start: DVec2,
    contour_points: Vec<DVec2>,
    /// Kurva asli kontur yang sedang digambar, sejalan dengan `contour_points`.
    contour_segs: Vec<PathSeg>,
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
            contour_points: Vec::new(),
            contour_segs: Vec::new(),
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

    fn add_point(&mut self, pt: DVec2) {
        if self.contour_points.last().is_none_or(|last| (*last - pt).length_squared() > 1e-8) {
            self.contour_points.push(pt);
        }
    }

    /// Catat satu langkah kurva asli, melewati langkah yang tak punya panjang.
    fn add_seg(&mut self, seg: PathSeg) {
        if (seg.end() - self.current_pt).length_squared() > 1e-12 {
            self.contour_segs.push(seg);
        }
    }

    fn finish_contour(&mut self) {
        if self.contour_points.len() >= 3 {
            let first = self.contour_points[0];
            let last = *self.contour_points.last().unwrap();
            if (first - last).length_squared() > 1e-8 {
                self.contour_points.push(first);
            }
            // Sebagian font menutup kontur dengan `line_to` eksplisit ke titik
            // awal, sebagian lagi mengandalkan `close()` saja. Garis penutup
            // hanya ditambahkan pada kasus kedua, supaya tidak ada segmen
            // kembar tanpa panjang.
            if (self.current_pt - self.contour_start).length_squared() > 1e-12 {
                self.contour_segs.push(PathSeg::Line {
                    end: self.contour_start,
                });
            }
            let pts = std::mem::take(&mut self.contour_points);
            let segs = std::mem::take(&mut self.contour_segs);
            self.entities.push(Entity::Spline {
                points: pts,
                exact: (!segs.is_empty()).then_some(segs),
                is_construction: self.is_construction,
            });
        } else {
            self.contour_points.clear();
            self.contour_segs.clear();
        }
    }
}

impl ttf_parser::OutlineBuilder for GlyphOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish_contour();
        let pt = self.to_world(x, y);
        self.current_pt = pt;
        self.contour_start = pt;
        self.contour_points.push(pt);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let pt = self.to_world(x, y);
        self.add_point(pt);
        self.add_seg(PathSeg::Line { end: pt });
        self.current_pt = pt;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let p0 = self.current_pt;
        let p1 = self.to_world(x1, y1);
        let p2 = self.to_world(x, y);

        // Kurva ASLI direkam lebih dulu, sebelum dicacah — dinaikkan derajatnya
        // ke kubik supaya sama bentuknya dengan `curve_to`, tanpa galat.
        self.add_seg(PathSeg::cubic_from_quadratic(p0, p1, p2));

        // Subdivisi adaptif poliline kurva Bézier kuadratik
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

        for step in 1..=num_segments {
            let t = (step as f64) / (num_segments as f64);
            let pt = if step == num_segments {
                p2
            } else {
                let one_minus_t = 1.0 - t;
                one_minus_t * one_minus_t * p0 + 2.0 * one_minus_t * t * p1 + t * t * p2
            };
            self.add_point(pt);
        }
        self.current_pt = p2;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p0 = self.current_pt;
        let p1 = self.to_world(x1, y1);
        let p2 = self.to_world(x2, y2);
        let p3 = self.to_world(x, y);

        // Kurva ASLI direkam lebih dulu, sebelum dicacah.
        self.add_seg(PathSeg::Cubic {
            c1: p1,
            c2: p2,
            end: p3,
        });

        // Subdivisi adaptif poliline kurva Bézier kubik
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
            self.add_point(pt);
        }
        self.current_pt = p3;
    }

    fn close(&mut self) {
        self.finish_contour();
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

    let font_buf;
    let font_data: &[u8] = if let Some(custom) = custom_font_bytes {
        custom
    } else {
        font_buf = options.font_preset.load_font_bytes();
        &font_buf
    };

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
                    builder.finish_contour();
                    all_entities.extend(builder.entities);
                }

                let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f64 * scale;
                current_x += advance * options.letter_spacing;
            }
        }
    }

    Ok(all_entities)
}

fn default_text_size() -> f64 {
    10.0
}
fn one_f64() -> f64 {
    1.0
}
fn default_line_spacing() -> f64 {
    1.2
}

/// Spesifikasi teks parametrik (konten, font, ukuran, tata letak, rotasi).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSpec {
    pub content: String,
    #[serde(default)]
    pub font_preset: FontPreset,
    #[serde(default)]
    pub custom_font_name: Option<String>,
    #[serde(default = "default_text_size")]
    pub size_mm: f64,
    #[serde(default)]
    pub align: TextAlign,
    #[serde(default)]
    pub origin: DVec2,
    #[serde(default)]
    pub rotation_rad: f64,
    #[serde(default = "one_f64")]
    pub letter_spacing: f64,
    #[serde(default = "default_line_spacing")]
    pub line_spacing: f64,
    #[serde(default)]
    pub is_construction: bool,
}

impl Default for TextSpec {
    fn default() -> Self {
        Self {
            content: String::new(),
            font_preset: FontPreset::Arial,
            custom_font_name: None,
            size_mm: 10.0,
            align: TextAlign::Left,
            origin: DVec2::ZERO,
            rotation_rad: 0.0,
            letter_spacing: 1.0,
            line_spacing: 1.2,
            is_construction: false,
        }
    }
}

/// Objek teks parametrik pada sketch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextObject {
    pub spec: TextSpec,
    pub glyph_entities: Vec<EntityId>,
    #[serde(default = "TextObject::default_style")]
    pub style: Style,
    #[serde(default)]
    pub layer: Option<LayerId>,
}

impl TextObject {
    pub fn default_style() -> Style {
        Style {
            fill: Some(Paint::Solid(Rgba([0.0, 0.0, 0.0, 1.0]))),
            fill_rule: FillRule::NonZero,
            stroke: None,
            opacity: 1.0,
            blend: BlendMode::Normal,
        }
    }

    pub fn new(spec: TextSpec) -> Self {
        Self {
            spec,
            glyph_entities: Vec::new(),
            style: Self::default_style(),
            layer: None,
        }
    }
}

impl Default for TextObject {
    fn default() -> Self {
        Self::new(TextSpec::default())
    }
}

struct KurboOutlineBuilder {
    path: kurbo::BezPath,
    offset: DVec2,
    scale: f64,
}

impl ttf_parser::OutlineBuilder for KurboOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(kurbo::Point::new(
            self.offset.x + (x as f64) * self.scale,
            self.offset.y + (y as f64) * self.scale,
        ));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(kurbo::Point::new(
            self.offset.x + (x as f64) * self.scale,
            self.offset.y + (y as f64) * self.scale,
        ));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.quad_to(
            kurbo::Point::new(
                self.offset.x + (x1 as f64) * self.scale,
                self.offset.y + (y1 as f64) * self.scale,
            ),
            kurbo::Point::new(
                self.offset.x + (x as f64) * self.scale,
                self.offset.y + (y as f64) * self.scale,
            ),
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path.curve_to(
            kurbo::Point::new(
                self.offset.x + (x1 as f64) * self.scale,
                self.offset.y + (y1 as f64) * self.scale,
            ),
            kurbo::Point::new(
                self.offset.x + (x2 as f64) * self.scale,
                self.offset.y + (y2 as f64) * self.scale,
            ),
            kurbo::Point::new(
                self.offset.x + (x as f64) * self.scale,
                self.offset.y + (y as f64) * self.scale,
            ),
        );
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}

/// Vektorisasi `TextSpec` menjadi daftar `Entity::Path` tertutup.
pub fn spec_to_path_entities(
    spec: &TextSpec,
    custom_font_bytes: Option<&[u8]>,
) -> Result<Vec<Entity>, String> {
    if spec.content.is_empty() {
        return Ok(Vec::new());
    }

    let font_buf;
    let font_data: &[u8] = if let Some(custom) = custom_font_bytes {
        custom
    } else {
        font_buf = spec.font_preset.load_font_bytes();
        &font_buf
    };

    let face = ttf_parser::Face::parse(font_data, 0)
        .map_err(|e| format!("Gagal mem-parsing berkas font TTF/OTF: {e}"))?;

    let upem = face.units_per_em() as f64;
    let ascender = face.ascender() as f64;
    let descender = face.descender() as f64;
    let em_height = (ascender - descender).max(upem);
    let scale = (spec.size_mm / em_height).max(1e-6);

    let lines: Vec<&str> = spec.content.lines().collect();
    let line_height_mm = spec.size_mm * spec.line_spacing;

    let rot = glam::DMat2::from_angle(spec.rotation_rad);
    let has_rot = spec.rotation_rad.abs() > 1e-9;

    let mut entities = Vec::new();

    for (line_idx, line_str) in lines.iter().enumerate() {
        let mut line_width = 0.0;
        for c in line_str.chars() {
            if let Some(glyph_id) = face.glyph_index(c) {
                let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f64 * scale;
                line_width += advance * spec.letter_spacing;
            }
        }

        let align_offset_x = match spec.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => -line_width * 0.5,
            TextAlign::Right => -line_width,
        };

        let mut current_x = spec.origin.x + align_offset_x;
        let current_y = spec.origin.y - (line_idx as f64) * line_height_mm;

        for c in line_str.chars() {
            if let Some(glyph_id) = face.glyph_index(c) {
                let mut builder = KurboOutlineBuilder {
                    path: kurbo::BezPath::new(),
                    offset: DVec2::new(current_x, current_y),
                    scale,
                };

                if let Some(_bbox) = face.outline_glyph(glyph_id, &mut builder) {
                    let mut subpaths = Subpath::from_kurbo(&builder.path);
                    if has_rot {
                        subpaths = subpaths
                            .into_iter()
                            .map(|sp| {
                                sp.map_points(|p| spec.origin + rot.mul_vec2(p - spec.origin))
                            })
                            .collect();
                    }
                    if !subpaths.is_empty() {
                        entities.push(Entity::Path {
                            subpaths,
                            is_construction: spec.is_construction,
                        });
                    }
                }

                let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f64 * scale;
                current_x += advance * spec.letter_spacing;
            }
        }
    }

    Ok(entities)
}

/// Regenerasi: hapus glyph lama, buat glyph baru dari spec, pertahankan
/// layer/grup/style; kembalikan id baru. Deterministik.
pub fn regenerate_text(sketch: &mut Sketch, id: TextId) -> Vec<EntityId> {
    let Some(text_obj) = sketch.texts.get(id).cloned() else {
        return Vec::new();
    };

    let inherited_layer = text_obj.layer.or_else(|| {
        text_obj
            .glyph_entities
            .iter()
            .find_map(|&gid| sketch.entity_layer.get(gid).copied())
    });

    let inherited_group = text_obj
        .glyph_entities
        .iter()
        .find_map(|&gid| sketch.entity_group.get(gid).copied());

    // Hapus glyph lama
    for &gid in &text_obj.glyph_entities {
        sketch.entities.remove(gid);
        sketch.entity_names.remove(&gid);
        sketch.styles.remove(gid);
        sketch.entity_layer.remove(gid);
        sketch.entity_group.remove(gid);
        sketch.origin.remove(gid);
        sketch.z_order.retain(|&z| z != gid);
        if let Some(grp_id) = inherited_group {
            if let Some(grp) = sketch.groups.get_mut(grp_id) {
                grp.members.retain(|&m| m != gid);
            }
        }
        sketch.touch(gid);
    }

    let new_entities = spec_to_path_entities(&text_obj.spec, None).unwrap_or_default();
    let mut new_ids = Vec::with_capacity(new_entities.len());

    for ent in new_entities {
        let new_id = sketch.entities.insert(ent);
        sketch.origin.insert(new_id, crate::layer::Origin::Text { text: id });
        sketch.styles.insert(new_id, text_obj.style.clone());
        if let Some(lay) = inherited_layer {
            sketch.entity_layer.insert(new_id, lay);
        }
        if let Some(grp_id) = inherited_group {
            sketch.entity_group.insert(new_id, grp_id);
            if let Some(grp) = sketch.groups.get_mut(grp_id) {
                grp.members.push(new_id);
            }
        }
        sketch.z_order.push(new_id);
        sketch.touch(new_id);
        new_ids.push(new_id);
    }

    if let Some(obj) = sketch.texts.get_mut(id) {
        obj.glyph_entities = new_ids.clone();
        if let Some(lay) = inherited_layer {
            obj.layer = Some(lay);
        }
    }

    new_ids
}
