//! `DrawingSpec` (P21.1/P21.6): deskripsi deklaratif satu lembar gambar —
//! kertas, tampak, potongan, skala, render berbayang, kebijakan dimensi, dan
//! tata letak hasil suntingan pengguna. Dipakai tool `drawing`, `ducad-cli
//! build`, dan editor lembar GUI; disimpan di `DesignDoc.drawings`.
//!
//! Doc comment `///` pada tipe di berkas ini ikut menjadi kontrak yang dibaca
//! agent, jadi ditulis dalam bahasa Inggris.

use std::collections::BTreeMap;

use ducad_core::drawing_annot::{Annotation, DimensionRef};
use ducad_kernel::{
    ProjectedViewKind, SectionAxis, SectionPath, SectionPlaneConfig, SectionRequest,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    BomTable, CalloutBalloon, DimensionAnnotation, PaperSize, TextAnnotation, TitleBlockInfo,
};

/// Title block content.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TitleSpec {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub part_number: String,
    #[serde(default)]
    pub author: String,
    /// `YYYY-MM-DD`.
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub material: String,
    #[serde(default)]
    pub revision: String,
}

/// Parent view of a section cutting line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParentView {
    Front,
    #[default]
    Top,
    Right,
}

impl ParentView {
    pub fn kind(self) -> ProjectedViewKind {
        match self {
            ParentView::Front => ProjectedViewKind::Front,
            ParentView::Top => ProjectedViewKind::Top,
            ParentView::Right => ProjectedViewKind::Right,
        }
    }
}

/// One section view. Give either `axis` (+ `offset`, `flip`) for a plane
/// perpendicular to a world axis, or `path` for a cutting line drawn on the
/// parent view (2 points = straight cut, more = stepped/offset section).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionSpec {
    /// Single letter, e.g. "A" → "SECTION A-A".
    pub label: String,
    /// View on which the cutting line is drawn.
    #[serde(default)]
    pub parent: ParentView,
    /// World axis the cutting plane is perpendicular to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis: Option<SectionAxis>,
    /// Plane offset from the model bounding-box centre along `axis` (mm).
    #[serde(default)]
    pub offset: f32,
    /// Reverse the viewing direction.
    #[serde(default)]
    pub flip: bool,
    /// Cutting line in parent-view coordinates relative to the bounding-box
    /// centre (mm). The view looks towards the LEFT of the travel direction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<Vec<[f32; 2]>>,
}

impl SectionSpec {
    /// Ubah menjadi permintaan kernel. `bbox` = kotak pembatas model.
    pub fn to_request(&self, bbox: ([f32; 3], [f32; 3])) -> Result<SectionRequest, String> {
        let parent = self.parent.kind();
        match (&self.path, self.axis) {
            (Some(path), None) => {
                if path.len() < 2 {
                    return Err(format!(
                        "section {}: path needs at least 2 points",
                        self.label
                    ));
                }
                let (_, right, up) = parent.camera_vectors();
                let c = (glam::Vec3::from_array(bbox.0) + glam::Vec3::from_array(bbox.1)) * 0.5;
                let (cu, cv) = (c.dot(right), c.dot(up));
                let mut points: Vec<[f32; 2]> =
                    path.iter().map(|p| [p[0] + cu, p[1] + cv]).collect();
                if self.flip {
                    points.reverse();
                }
                Ok(SectionRequest::from_path(
                    &self.label,
                    SectionPath { points, parent },
                ))
            }
            (None, Some(axis)) => {
                let cfg = SectionPlaneConfig::from_axis(axis, self.offset, self.flip, bbox);
                SectionRequest::from_plane_on(&self.label, &cfg, bbox, Some(parent))
                    .map_err(|e| format!("section {}: {e}", self.label))
            }
            (Some(_), Some(_)) => Err(format!(
                "section {}: give either 'axis' or 'path', not both",
                self.label
            )),
            (None, None) => Err(format!(
                "section {}: 'axis' or 'path' is required",
                self.label
            )),
        }
    }
}

/// Per-view override.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewOptions {
    /// Show or hide the view. Default: front/top/right/isometric and every
    /// section are visible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// View scale (0.5 = 1:2). The view title then states "SCALE 1:2".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
}

/// Per-view overrides keyed by `front`, `top`, `right`, `isometric`,
/// `section_a`, `section_b`, …, `detail_b`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ViewSet(pub BTreeMap<String, ViewOptions>);

/// Kunci `ViewSet` untuk sebuah tampak.
pub fn view_key(kind: ProjectedViewKind) -> String {
    match kind {
        ProjectedViewKind::Front => "front".to_string(),
        ProjectedViewKind::Top => "top".to_string(),
        ProjectedViewKind::Right => "right".to_string(),
        ProjectedViewKind::Isometric => "isometric".to_string(),
        ProjectedViewKind::Section(c) => format!("section_{}", c.to_ascii_lowercase()),
        ProjectedViewKind::Detail(c) => format!("detail_{}", c.to_ascii_lowercase()),
    }
}

/// Kebalikan [`view_key`].
pub fn parse_view_key(key: &str) -> Option<ProjectedViewKind> {
    let key = key.to_ascii_lowercase();
    let letter = |rest: &str| {
        let mut chars = rest.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) if c.is_ascii_alphabetic() => Some(c.to_ascii_uppercase()),
            _ => None,
        }
    };
    match key.as_str() {
        "front" => Some(ProjectedViewKind::Front),
        "top" => Some(ProjectedViewKind::Top),
        "right" => Some(ProjectedViewKind::Right),
        "isometric" | "iso" => Some(ProjectedViewKind::Isometric),
        other => {
            if let Some(rest) = other.strip_prefix("section_") {
                letter(rest).map(ProjectedViewKind::Section)
            } else if let Some(rest) = other.strip_prefix("detail_") {
                letter(rest).map(ProjectedViewKind::Detail)
            } else {
                None
            }
        }
    }
}

impl ViewSet {
    pub fn get(&self, kind: ProjectedViewKind) -> ViewOptions {
        self.0.get(&view_key(kind)).copied().unwrap_or_default()
    }

    pub fn entry(&mut self, kind: ProjectedViewKind) -> &mut ViewOptions {
        self.0.entry(view_key(kind)).or_default()
    }

    /// Kunci yang bukan nama tampak yang sah.
    pub fn unknown_keys(&self) -> Vec<String> {
        self.0
            .keys()
            .filter(|k| parse_view_key(k).is_none())
            .cloned()
            .collect()
    }
}

/// Sheet scale: `"auto"` picks the largest standard scale that fits; a
/// number (0.5) or ratio string ("1:2") fixes it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum ScaleSpec {
    #[default]
    Auto,
    Fixed(f32),
}

/// Parse "1:2", "2:1", "0.5".
pub fn parse_scale(text: &str) -> Option<f32> {
    let t = text.trim();
    let value = match t.split_once(':') {
        Some((a, b)) => {
            let (a, b): (f32, f32) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
            if b.abs() < 1e-9 {
                return None;
            }
            a / b
        }
        None => t.parse().ok()?,
    };
    (value.is_finite() && value > 1e-4 && value < 1e4).then_some(value)
}

impl Serialize for ScaleSpec {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ScaleSpec::Auto => s.serialize_str("auto"),
            ScaleSpec::Fixed(v) => s.serialize_f32(*v),
        }
    }
}

impl<'de> Deserialize<'de> for ScaleSpec {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Num(f32),
            Text(String),
        }
        match Raw::deserialize(d)? {
            Raw::Num(v) if v.is_finite() && v > 1e-4 => Ok(ScaleSpec::Fixed(v)),
            Raw::Num(v) => Err(serde::de::Error::custom(format!("invalid scale {v}"))),
            Raw::Text(t) if t.eq_ignore_ascii_case("auto") => Ok(ScaleSpec::Auto),
            Raw::Text(t) => parse_scale(&t).map(ScaleSpec::Fixed).ok_or_else(|| {
                serde::de::Error::custom(format!("invalid scale '{t}' (auto, 0.5, \"1:2\")"))
            }),
        }
    }
}

/// Camera of a shaded render placed on the sheet.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShadedCamera {
    /// Seen from +X, −Y, +Z.
    #[default]
    Iso,
    /// Seen from −X, +Y, +Z.
    IsoBack,
    /// Yaw about +Z from the −Y direction and pitch above the XY plane (degrees).
    Custom { yaw: f32, pitch: f32 },
}

impl ShadedCamera {
    /// (kanan, atas, arah pandang) kamera ortografik.
    pub fn axes(self) -> (glam::Vec3, glam::Vec3, glam::Vec3) {
        let (yaw, pitch) = match self {
            ShadedCamera::Iso => (45.0f32, 35.264f32),
            ShadedCamera::IsoBack => (225.0, 35.264),
            ShadedCamera::Custom { yaw, pitch } => (yaw, pitch.clamp(-89.0, 89.0)),
        };
        let (y, p) = (yaw.to_radians(), pitch.to_radians());
        // Posisi mata relatif pusat: yaw 0 = dari −Y, berputar ke +X.
        let eye = glam::vec3(y.sin() * p.cos(), -y.cos() * p.cos(), p.sin());
        let dir = -eye.normalize();
        let right = dir.cross(glam::Vec3::Z).normalize_or_zero();
        let up = right.cross(dir).normalize_or_zero();
        (right, up, dir)
    }
}

/// A shaded 3D render embedded in the sheet. Accepts the short forms
/// `"iso"` / `"iso_back"`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ShadedSpec {
    pub camera: ShadedCamera,
    /// Raster resolution in pixels per paper millimetre (default 8, max 16).
    pub px_per_mm: u32,
}

impl Default for ShadedSpec {
    fn default() -> Self {
        Self {
            camera: ShadedCamera::Iso,
            px_per_mm: 8,
        }
    }
}

impl<'de> Deserialize<'de> for ShadedSpec {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        fn default_px() -> u32 {
            8
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Full {
            camera: ShadedCamera,
            #[serde(default = "default_px")]
            px_per_mm: u32,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Camera(ShadedCamera),
            Full(Full),
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Camera(camera) => ShadedSpec {
                camera,
                px_per_mm: 8,
            },
            Raw::Full(f) => ShadedSpec {
                camera: f.camera,
                px_per_mm: f.px_per_mm.clamp(1, 16),
            },
        })
    }
}

/// Which dimensions the sheet carries: `"auto"`, `"none"`, or an explicit
/// list of dimension references.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum DimensionPolicy {
    #[default]
    Auto,
    None,
    Only(Vec<DimensionRef>),
}

impl Serialize for DimensionPolicy {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            DimensionPolicy::Auto => s.serialize_str("auto"),
            DimensionPolicy::None => s.serialize_str("none"),
            DimensionPolicy::Only(list) => list.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for DimensionPolicy {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Text(String),
            List(Vec<DimensionRef>),
        }
        match Raw::deserialize(d)? {
            Raw::Text(t) if t.eq_ignore_ascii_case("auto") => Ok(DimensionPolicy::Auto),
            Raw::Text(t) if t.eq_ignore_ascii_case("none") => Ok(DimensionPolicy::None),
            Raw::Text(t) => Err(serde::de::Error::custom(format!(
                "invalid dimensions '{t}' (auto, none, or a list)"
            ))),
            Raw::List(list) => Ok(DimensionPolicy::Only(list)),
        }
    }
}

/// Posisi/skala satu tampak hasil suntingan pengguna.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewLayout {
    pub view: String,
    pub center_mm: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
    #[serde(default = "super::default_true")]
    pub visible: bool,
}

/// Penempatan satu dimensi asosiatif (dikenali lewat `source`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionLayout {
    pub source: DimensionRef,
    pub offset_mm: f32,
    #[serde(default)]
    pub angle_deg: f32,
}

/// User-edited sheet layout: view positions, dimension offsets, free texts,
/// balloons, BOM and GD&T annotations. Written by the GUI sheet editor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SheetLayout {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<ViewLayout>,
    /// Pusat tiap render berbayang (urut seperti `DrawingSpec.shaded`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shaded_centers: Vec<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<DimensionLayout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manual_dimensions: Vec<DimensionAnnotation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub texts: Vec<TextAnnotation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub balloons: Vec<CalloutBalloon>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bom: Option<BomTable>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_block: Option<TitleBlockInfo>,
}

fn default_sheet_name() -> String {
    "sheet1".to_string()
}

/// Declarative description of one drawing sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingSpec {
    /// Sheet name, unique within the part (default "sheet1").
    #[serde(default = "default_sheet_name")]
    pub name: String,
    #[serde(default)]
    pub paper: PaperSize,
    #[serde(default)]
    pub title: TitleSpec,
    /// General notes, one per line.
    #[serde(default)]
    pub notes: Vec<String>,
    /// Section views. Omitted = one section A-A through the model centre;
    /// `[]` = no section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sections: Option<Vec<SectionSpec>>,
    #[serde(default)]
    pub views: ViewSet,
    #[serde(default)]
    pub scale: ScaleSpec,
    /// Shaded 3D renders embedded in the sheet.
    #[serde(default)]
    pub shaded: Vec<ShadedSpec>,
    #[serde(default)]
    pub dimensions: DimensionPolicy,
    /// Draw hidden (dashed) lines in the orthographic views.
    #[serde(default = "super::default_true")]
    pub hidden_lines: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<SheetLayout>,
}

impl Default for DrawingSpec {
    fn default() -> Self {
        Self {
            name: default_sheet_name(),
            paper: PaperSize::A3Landscape,
            title: TitleSpec::default(),
            notes: Vec::new(),
            sections: None,
            views: ViewSet::default(),
            scale: ScaleSpec::Auto,
            shaded: Vec::new(),
            dimensions: DimensionPolicy::Auto,
            hidden_lines: true,
            layout: None,
        }
    }
}

impl DrawingSpec {
    /// Label potongan (huruf besar) dalam urutan spec; `Err` bila ada yang
    /// kembar atau tidak sah.
    pub fn section_labels(&self) -> Result<Vec<char>, String> {
        let mut labels = Vec::new();
        for s in self.sections.as_deref().unwrap_or(&[]) {
            let mut chars = s.label.chars();
            let c = match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphabetic() => c.to_ascii_uppercase(),
                _ => {
                    return Err(format!(
                        "DRAWING_SECTION_LABEL: section label '{}' must be a single letter",
                        s.label
                    ))
                }
            };
            if labels.contains(&c) {
                return Err(format!(
                    "DRAWING_SECTION_LABEL_DUP: section label '{c}' is used twice"
                ));
            }
            labels.push(c);
        }
        Ok(labels)
    }
}

/// Nama kertas pendek yang diterima tool/CLI: a4, a4-portrait, a3, a3-portrait.
pub fn parse_paper(text: &str) -> Option<PaperSize> {
    match text.to_ascii_lowercase().as_str() {
        "a4" | "a4-landscape" => Some(PaperSize::A4Landscape),
        "a4-portrait" => Some(PaperSize::A4Portrait),
        "a3" | "a3-landscape" => Some(PaperSize::A3Landscape),
        "a3-portrait" => Some(PaperSize::A3Portrait),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_spec_parses_tool_shaped_json() {
        let json = r#"{
            "name": "sheet1",
            "paper": "A3Landscape",
            "scale": "1:2",
            "sections": [
                { "label": "A", "parent": "top", "axis": "x", "offset": 0, "flip": false },
                { "label": "B", "parent": "top", "path": [[-80, 20], [80, 20]] }
            ],
            "views": { "section_b": { "scale": 0.5 }, "right": { "visible": false } },
            "shaded": ["iso", "iso_back", { "camera": { "custom": { "yaw": 30, "pitch": 20 } }, "px_per_mm": 4 }],
            "dimensions": "auto",
            "notes": ["ALL DIMENSIONS ARE IN MILLIMETERS."]
        }"#;
        let spec: DrawingSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.scale, ScaleSpec::Fixed(0.5));
        assert_eq!(spec.section_labels().unwrap(), vec!['A', 'B']);
        assert_eq!(
            spec.views.get(ProjectedViewKind::Section('B')).scale,
            Some(0.5)
        );
        assert_eq!(
            spec.views.get(ProjectedViewKind::Right).visible,
            Some(false)
        );
        assert_eq!(spec.shaded.len(), 3);
        assert_eq!(spec.shaded[1].camera, ShadedCamera::IsoBack);
        assert_eq!(spec.shaded[2].px_per_mm, 4);
        assert!(spec.views.unknown_keys().is_empty());

        // Round-trip stabil.
        let back: DrawingSpec =
            serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
        assert_eq!(back, spec);

        let dup = r#"{ "sections": [ {"label":"A","axis":"x"}, {"label":"a","axis":"y"} ] }"#;
        let spec: DrawingSpec = serde_json::from_str(dup).unwrap();
        assert!(spec
            .section_labels()
            .unwrap_err()
            .starts_with("DRAWING_SECTION_LABEL_DUP"));
    }

    #[test]
    fn section_spec_path_is_relative_to_bbox_centre() {
        let bbox = ([0.0, 0.0, 0.0], [100.0, 40.0, 20.0]);
        let spec = SectionSpec {
            label: "B".into(),
            parent: ParentView::Top,
            axis: None,
            offset: 0.0,
            flip: false,
            path: Some(vec![[-60.0, 5.0], [60.0, 5.0]]),
        };
        let req = spec.to_request(bbox).unwrap();
        assert_eq!(req.path.points, vec![[-10.0, 25.0], [110.0, 25.0]]);
        let both = SectionSpec {
            axis: Some(SectionAxis::X),
            ..spec.clone()
        };
        assert!(both.to_request(bbox).is_err());
    }

    #[test]
    fn scale_and_view_keys_parse() {
        assert_eq!(parse_scale("1:2"), Some(0.5));
        assert_eq!(parse_scale("2:1"), Some(2.0));
        assert_eq!(parse_scale("0.25"), Some(0.25));
        assert_eq!(parse_scale("1:0"), None);
        assert_eq!(
            parse_view_key("section_b"),
            Some(ProjectedViewKind::Section('B'))
        );
        assert_eq!(view_key(ProjectedViewKind::Detail('C')), "detail_c");
        assert_eq!(parse_view_key("back"), None);
        let (right, up, dir) = ShadedCamera::Iso.axes();
        assert!(dir.x < 0.0 && dir.y > 0.0 && dir.z < 0.0, "{dir:?}");
        assert!(right.dot(up).abs() < 1e-5 && up.z > 0.0);
    }
}
