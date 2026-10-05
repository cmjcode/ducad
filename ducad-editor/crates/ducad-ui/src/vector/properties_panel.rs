//! Panel Properti Vektor (M2.7).
//!
//! Menampilkan dan mengedit properti gaya grafis entitas terpilih:
//! Fill, Stroke, Opacity, Blend Mode, dan Fill Rule.

use ducad_sketch::commands::StyleField;
use ducad_sketch::style::{BlendMode, FillRule, LineCap, LineJoin, Paint, Rgba, StrokeStyle, Style};
use egui::{ComboBox, RichText, Slider, Ui};
use super::color_picker::{ColorPickerAction, ColorPickerState};
use super::swatches::SwatchManager;

/// Representasi perbandingan properti style dari sekumpulan objek terpilih.
/// Nilai `Mixed` menunjukkan objek terpilih memiliki nilai properti yang berbeda ("—").
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyVal<T> {
    None,
    Uniform(T),
    Mixed,
}

impl<T: PartialEq + Clone> PropertyVal<T> {
    pub fn is_mixed(&self) -> bool {
        matches!(self, PropertyVal::Mixed)
    }

    pub fn uniform(&self) -> Option<&T> {
        match self {
            PropertyVal::Uniform(v) => Some(v),
            _ => None,
        }
    }
}

/// Selisih / agregasi properti dari beberapa objek terpilih.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleDiff {
    pub fill: PropertyVal<Option<Paint>>,
    pub fill_rule: PropertyVal<FillRule>,
    pub stroke: PropertyVal<Option<StrokeStyle>>,
    pub stroke_width: PropertyVal<f64>,
    pub stroke_cap: PropertyVal<LineCap>,
    pub stroke_join: PropertyVal<LineJoin>,
    pub opacity: PropertyVal<f32>,
    pub blend: PropertyVal<BlendMode>,
}

impl StyleDiff {
    /// Menghitung perbandingan style dari daftar entitas yang dipilih.
    pub fn from_styles(styles: &[Style]) -> Self {
        if styles.is_empty() {
            return Self {
                fill: PropertyVal::None,
                fill_rule: PropertyVal::None,
                stroke: PropertyVal::None,
                stroke_width: PropertyVal::None,
                stroke_cap: PropertyVal::None,
                stroke_join: PropertyVal::None,
                opacity: PropertyVal::None,
                blend: PropertyVal::None,
            };
        }

        let first = &styles[0];

        // Fill
        let mut fill_mixed = false;
        for s in &styles[1..] {
            if s.fill != first.fill {
                fill_mixed = true;
                break;
            }
        }
        let fill = if fill_mixed {
            PropertyVal::Mixed
        } else {
            PropertyVal::Uniform(first.fill.clone())
        };

        // Fill Rule
        let mut fr_mixed = false;
        for s in &styles[1..] {
            if s.fill_rule != first.fill_rule {
                fr_mixed = true;
                break;
            }
        }
        let fill_rule = if fr_mixed {
            PropertyVal::Mixed
        } else {
            PropertyVal::Uniform(first.fill_rule)
        };

        // Stroke
        let mut stroke_mixed = false;
        let mut width_mixed = false;
        let mut cap_mixed = false;
        let mut join_mixed = false;
        for s in &styles[1..] {
            if s.stroke != first.stroke {
                stroke_mixed = true;
            }
            let w1 = first.stroke.as_ref().map(|st| st.width_mm);
            let w2 = s.stroke.as_ref().map(|st| st.width_mm);
            if w1 != w2 {
                width_mixed = true;
            }
            let c1 = first.stroke.as_ref().map(|st| st.cap);
            let c2 = s.stroke.as_ref().map(|st| st.cap);
            if c1 != c2 {
                cap_mixed = true;
            }
            let j1 = first.stroke.as_ref().map(|st| st.join);
            let j2 = s.stroke.as_ref().map(|st| st.join);
            if j1 != j2 {
                join_mixed = true;
            }
        }

        let stroke = if stroke_mixed {
            PropertyVal::Mixed
        } else {
            PropertyVal::Uniform(first.stroke.clone())
        };

        let stroke_width = if width_mixed {
            PropertyVal::Mixed
        } else if let Some(st) = &first.stroke {
            PropertyVal::Uniform(st.width_mm)
        } else {
            PropertyVal::None
        };

        let stroke_cap = if cap_mixed {
            PropertyVal::Mixed
        } else if let Some(st) = &first.stroke {
            PropertyVal::Uniform(st.cap)
        } else {
            PropertyVal::None
        };

        let stroke_join = if join_mixed {
            PropertyVal::Mixed
        } else if let Some(st) = &first.stroke {
            PropertyVal::Uniform(st.join)
        } else {
            PropertyVal::None
        };

        // Opacity
        let mut op_mixed = false;
        for s in &styles[1..] {
            if (s.opacity - first.opacity).abs() > 1e-4 {
                op_mixed = true;
                break;
            }
        }
        let opacity = if op_mixed {
            PropertyVal::Mixed
        } else {
            PropertyVal::Uniform(first.opacity)
        };

        // Blend
        let mut blend_mixed = false;
        for s in &styles[1..] {
            if s.blend != first.blend {
                blend_mixed = true;
                break;
            }
        }
        let blend = if blend_mixed {
            PropertyVal::Mixed
        } else {
            PropertyVal::Uniform(first.blend)
        };

        Self {
            fill,
            fill_rule,
            stroke,
            stroke_width,
            stroke_cap,
            stroke_join,
            opacity,
            blend,
        }
    }
}

/// Event perubahan style dari panel properti.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertiesPanelEvent {
    SetField(StyleField),
    AddDocumentSwatch(Rgba),
    StartEyedropper,
}

/// State widget Panel Properti Vektor.
#[derive(Debug, Clone)]
pub struct PropertiesPanelState {
    pub is_visible: bool,
    pub fill_picker: ColorPickerState,
    pub stroke_picker: ColorPickerState,
}

impl Default for PropertiesPanelState {
    fn default() -> Self {
        Self {
            is_visible: true,
            fill_picker: ColorPickerState::default(),
            stroke_picker: ColorPickerState::default(),
        }
    }
}

impl PropertiesPanelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render isi panel properti.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        diff: &StyleDiff,
        recent: &mut SwatchManager,
        doc_swatches: &[Rgba],
    ) -> Option<PropertiesPanelEvent> {
        if !self.is_visible {
            return None;
        }

        let mut event = None;

        ui.vertical(|ui| {
            ui.heading(RichText::new("Properti Vektor").size(14.0));
            ui.separator();

            // 1. FILL
            ui.label(RichText::new("Isian (Fill)").strong());
            ui.horizontal(|ui| {
                if diff.fill.is_mixed() {
                    ui.label("— (Berbeda)");
                } else if let Some(Some(paint)) = diff.fill.uniform() {
                    match paint {
                        Paint::Solid(rgba) => {
                            if ui.button(format!("Solid {}", rgba.to_hex())).clicked() {
                                self.fill_picker.set_color(*rgba);
                                self.fill_picker.is_open = !self.fill_picker.is_open;
                            }
                        }
                        Paint::Linear { .. } => {
                            ui.label("Gradien Linier");
                        }
                        Paint::Radial { .. } => {
                            ui.label("Gradien Radial");
                        }
                    }
                    if ui.button("Hapus").clicked() {
                        event = Some(PropertiesPanelEvent::SetField(StyleField::Fill(None)));
                    }
                } else {
                    ui.label("Tidak ada");
                    if ui.button("Tambah").clicked() {
                        let col = Rgba([0.2, 0.5, 0.9, 1.0]);
                        event = Some(PropertiesPanelEvent::SetField(StyleField::Fill(Some(Paint::Solid(col)))));
                    }
                }
            });

            if self.fill_picker.is_open {
                ui.group(|ui| {
                    if let Some(act) = self.fill_picker.show(ui, recent, doc_swatches) {
                        match act {
                            ColorPickerAction::Changed(rgba) => {
                                event = Some(PropertiesPanelEvent::SetField(StyleField::Fill(Some(Paint::Solid(rgba)))));
                            }
                            ColorPickerAction::AddDocumentSwatch(rgba) => {
                                event = Some(PropertiesPanelEvent::AddDocumentSwatch(rgba));
                            }
                            ColorPickerAction::StartEyedropper => {
                                event = Some(PropertiesPanelEvent::StartEyedropper);
                            }
                        }
                    }
                });
            }

            // Fill Rule
            ui.horizontal(|ui| {
                ui.label("Aturan Isian:");
                let cur_rule = if diff.fill_rule.is_mixed() {
                    "Mixed"
                } else if let Some(rule) = diff.fill_rule.uniform() {
                    match rule {
                        FillRule::NonZero => "Non-Zero",
                        FillRule::EvenOdd => "Even-Odd",
                    }
                } else {
                    "Non-Zero"
                };

                ComboBox::from_id_salt("prop_fill_rule")
                    .selected_text(cur_rule)
                    .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                        if ui.selectable_label(cur_rule == "Non-Zero", "Non-Zero (Biasa)").clicked() {
                            event = Some(PropertiesPanelEvent::SetField(StyleField::FillRule(FillRule::NonZero)));
                        }
                        if ui.selectable_label(cur_rule == "Even-Odd", "Even-Odd (Alternatif)").clicked() {
                            event = Some(PropertiesPanelEvent::SetField(StyleField::FillRule(FillRule::EvenOdd)));
                        }
                    }));
            });

            ui.separator();

            // 2. STROKE
            ui.label(RichText::new("Garis Tepi (Stroke)").strong());
            ui.horizontal(|ui| {
                if diff.stroke.is_mixed() {
                    ui.label("— (Berbeda)");
                } else if let Some(Some(stroke)) = diff.stroke.uniform() {
                    let col = stroke.paint.average_color();
                    if ui.button(format!("Garis {}", col.to_hex())).clicked() {
                        self.stroke_picker.set_color(col);
                        self.stroke_picker.is_open = !self.stroke_picker.is_open;
                    }
                    if ui.button("Hapus").clicked() {
                        event = Some(PropertiesPanelEvent::SetField(StyleField::Stroke(None)));
                    }
                } else {
                    ui.label("Tidak ada");
                    if ui.button("Tambah").clicked() {
                        let stroke = StrokeStyle {
                            paint: Paint::Solid(Rgba::BLACK),
                            width_mm: 1.0,
                            dash: vec![],
                            cap: LineCap::Round,
                            join: LineJoin::Round,
                        };
                        event = Some(PropertiesPanelEvent::SetField(StyleField::Stroke(Some(stroke))));
                    }
                }
            });

            if self.stroke_picker.is_open {
                ui.group(|ui| {
                    if let Some(act) = self.stroke_picker.show(ui, recent, doc_swatches) {
                        match act {
                            ColorPickerAction::Changed(rgba) => {
                                let w = diff.stroke_width.uniform().copied().unwrap_or(1.0);
                                let cap = diff.stroke_cap.uniform().copied().unwrap_or(LineCap::Round);
                                let join = diff.stroke_join.uniform().copied().unwrap_or(LineJoin::Round);
                                let stroke = StrokeStyle {
                                    paint: Paint::Solid(rgba),
                                    width_mm: w,
                                    dash: vec![],
                                    cap,
                                    join,
                                };
                                event = Some(PropertiesPanelEvent::SetField(StyleField::Stroke(Some(stroke))));
                            }
                            ColorPickerAction::AddDocumentSwatch(rgba) => {
                                event = Some(PropertiesPanelEvent::AddDocumentSwatch(rgba));
                            }
                            ColorPickerAction::StartEyedropper => {
                                event = Some(PropertiesPanelEvent::StartEyedropper);
                            }
                        }
                    }
                });
            }

            // Tebal Stroke
            if let Some(Some(stroke)) = diff.stroke.uniform() {
                let mut w = stroke.width_mm;
                ui.horizontal(|ui| {
                    ui.label("Tebal:");
                    if ui.add(Slider::new(&mut w, 0.1..=20.0).suffix(" mm")).changed() {
                        let mut st = stroke.clone();
                        st.width_mm = w;
                        event = Some(PropertiesPanelEvent::SetField(StyleField::Stroke(Some(st))));
                    }
                });

                // Cap & Join
                ui.horizontal(|ui| {
                    ui.label("Ujung:");
                    let cap_text = match stroke.cap {
                        LineCap::Butt => "Butt",
                        LineCap::Round => "Round",
                        LineCap::Square => "Square",
                    };
                    ComboBox::from_id_salt("prop_stroke_cap")
                        .selected_text(cap_text)
                        .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                            if ui.selectable_label(stroke.cap == LineCap::Butt, "Butt").clicked() {
                                let mut st = stroke.clone();
                                st.cap = LineCap::Butt;
                                event = Some(PropertiesPanelEvent::SetField(StyleField::Stroke(Some(st))));
                            }
                            if ui.selectable_label(stroke.cap == LineCap::Round, "Round").clicked() {
                                let mut st = stroke.clone();
                                st.cap = LineCap::Round;
                                event = Some(PropertiesPanelEvent::SetField(StyleField::Stroke(Some(st))));
                            }
                            if ui.selectable_label(stroke.cap == LineCap::Square, "Square").clicked() {
                                let mut st = stroke.clone();
                                st.cap = LineCap::Square;
                                event = Some(PropertiesPanelEvent::SetField(StyleField::Stroke(Some(st))));
                            }
                        }));
                });
            }

            ui.separator();

            // 3. OPACITY
            ui.label(RichText::new("Opasitas & Blend").strong());
            ui.horizontal(|ui| {
                ui.label("Opasitas:");
                let mut op = diff.opacity.uniform().copied().unwrap_or(1.0);
                if diff.opacity.is_mixed() {
                    ui.label("—");
                }
                if ui.add(Slider::new(&mut op, 0.0..=1.0).show_value(true)).changed() {
                    event = Some(PropertiesPanelEvent::SetField(StyleField::Opacity(op)));
                }
            });

            // 4. BLEND MODE
            ui.horizontal(|ui| {
                ui.label("Blend:");
                let blend_text = if diff.blend.is_mixed() {
                    "—"
                } else if let Some(b) = diff.blend.uniform() {
                    match b {
                        BlendMode::Normal => "Normal",
                        BlendMode::Multiply => "Multiply",
                        BlendMode::Screen => "Screen",
                    }
                } else {
                    "Normal"
                };

                ComboBox::from_id_salt("prop_blend_mode")
                    .selected_text(blend_text)
                    .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                        if ui.selectable_label(blend_text == "Normal", "Normal").clicked() {
                            event = Some(PropertiesPanelEvent::SetField(StyleField::Blend(BlendMode::Normal)));
                        }
                        if ui.selectable_label(blend_text == "Multiply", "Multiply").clicked() {
                            event = Some(PropertiesPanelEvent::SetField(StyleField::Blend(BlendMode::Multiply)));
                        }
                        if ui.selectable_label(blend_text == "Screen", "Screen").clicked() {
                            event = Some(PropertiesPanelEvent::SetField(StyleField::Blend(BlendMode::Screen)));
                        }
                    }));
            });
        });

        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_diff_for_multi_selection_marks_mixed_fields() {
        let style_a = Style {
            fill: Some(Paint::Solid(Rgba([1.0, 0.0, 0.0, 1.0]))),
            fill_rule: FillRule::NonZero,
            stroke: Some(StrokeStyle {
                paint: Paint::Solid(Rgba::BLACK),
                width_mm: 2.0,
                dash: vec![],
                cap: LineCap::Round,
                join: LineJoin::Round,
            }),
            opacity: 1.0,
            blend: BlendMode::Normal,
        };

        let style_b = Style {
            fill: Some(Paint::Solid(Rgba([0.0, 0.0, 1.0, 1.0]))), // beda warna fill
            fill_rule: FillRule::NonZero,                          // sama
            stroke: Some(StrokeStyle {
                paint: Paint::Solid(Rgba::BLACK),
                width_mm: 2.0,                                     // sama
                dash: vec![],
                cap: LineCap::Round,
                join: LineJoin::Round,
            }),
            opacity: 0.5,                                          // beda opacity
            blend: BlendMode::Normal,                              // sama
        };

        let diff = StyleDiff::from_styles(&[style_a, style_b]);

        // Fill dan Opacity harus Mixed ("—")
        assert!(diff.fill.is_mixed(), "Fill harus teridentifikasi sebagai mixed");
        assert!(diff.opacity.is_mixed(), "Opacity harus teridentifikasi sebagai mixed");

        // Fill rule, stroke width, dan blend mode harus Uniform
        assert!(!diff.fill_rule.is_mixed());
        assert_eq!(diff.fill_rule.uniform(), Some(&FillRule::NonZero));

        assert!(!diff.stroke_width.is_mixed());
        assert_eq!(diff.stroke_width.uniform(), Some(&2.0));

        assert!(!diff.blend.is_mixed());
        assert_eq!(diff.blend.uniform(), Some(&BlendMode::Normal));
    }
}
