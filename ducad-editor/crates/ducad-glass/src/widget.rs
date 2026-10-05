//! Widget egui: [`GlassFrame`], pengganti `egui::Frame` untuk panel kaca.
//!
//! API-nya sengaja meniru `egui::Frame` (field dan builder bernama sama)
//! supaya pemanggil lama `glass_frame().show(ui, …)` tidak perlu berubah.
//! Bila backdrop GPU tidak tersedia — tes headless, GPU belum siap, atau
//! "Kurangi transparansi" — frame jatuh ke isian datar biasa.

use std::sync::atomic::{AtomicU32, Ordering};

use egui::{Color32, CornerRadius, Frame, InnerResponse, Margin, Shadow, Shape, Stroke, Ui};
use egui_wgpu::wgpu;

use crate::{GlassBackdrop, GlassMaterial, GlassMode, GlassPreset, PanelUniform};

/// Keadaan Liquid Glass untuk satu `egui::Context`, diisi aplikasi.
///
/// Bawaan (belum pernah diisi) = GPU mati, sehingga semua [`GlassFrame`]
/// memakai isian datar. Itulah yang membuat tes UI headless tetap aman.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlassRuntime {
    /// Preferensi pengguna: tema Liquid Glass dinyalakan.
    pub enabled: bool,
    /// Backdrop GPU terpasang dan scene dirender ke sana frame ini.
    pub gpu: bool,
    /// Preferensi "Kurangi transparansi": paksa isian datar.
    pub reduce_transparency: bool,
}

impl GlassRuntime {
    /// Apakah panel digambar sebagai kaca GPU (bukan isian datar).
    pub fn uses_gpu(self) -> bool {
        self.enabled && self.gpu && !self.reduce_transparency
    }
}

fn runtime_id() -> egui::Id {
    egui::Id::new("ducad-glass-runtime")
}

/// Simpan keadaan Liquid Glass di context (panggil saat berubah / tiap frame).
pub fn set_runtime(ctx: &egui::Context, runtime: GlassRuntime) {
    ctx.data_mut(|d| d.insert_temp(runtime_id(), runtime));
}

/// Keadaan Liquid Glass saat ini; bawaan bila belum pernah diisi.
pub fn runtime(ctx: &egui::Context) -> GlassRuntime {
    ctx.data(|d| d.get_temp(runtime_id())).unwrap_or_default()
}

/// Frame panel Liquid Glass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassFrame {
    pub inner_margin: Margin,
    pub outer_margin: Margin,
    pub corner_radius: CornerRadius,
    pub shadow: Shadow,
    /// Isian datar pengganti saat kaca GPU tidak dipakai.
    pub fill: Color32,
    /// Garis tepi isian datar. Lebarnya tetap ikut tata letak di mode kaca
    /// (rim spekular menggantikan garisnya), jadi ukuran panel tidak berubah.
    pub stroke: Stroke,
    pub material: GlassMaterial,
}

impl GlassFrame {
    /// Frame dengan material tertentu; isian datar diturunkan dari material.
    pub fn new(material: GlassMaterial) -> Self {
        Self {
            inner_margin: Margin::ZERO,
            outer_margin: Margin::ZERO,
            corner_radius: CornerRadius::same(10),
            shadow: Shadow::NONE,
            fill: material.fallback_fill(),
            stroke: Stroke::NONE,
            material,
        }
    }

    /// Frame dengan preset bawaan.
    pub fn preset(preset: GlassPreset, mode: GlassMode) -> Self {
        Self::new(GlassMaterial::preset(preset, mode))
    }

    #[inline]
    pub fn inner_margin(mut self, inner_margin: impl Into<Margin>) -> Self {
        self.inner_margin = inner_margin.into();
        self
    }

    #[inline]
    pub fn outer_margin(mut self, outer_margin: impl Into<Margin>) -> Self {
        self.outer_margin = outer_margin.into();
        self
    }

    #[inline]
    pub fn corner_radius(mut self, corner_radius: impl Into<CornerRadius>) -> Self {
        self.corner_radius = corner_radius.into();
        self
    }

    #[inline]
    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.shadow = shadow;
        self
    }

    #[inline]
    pub fn fill(mut self, fill: Color32) -> Self {
        self.fill = fill;
        self
    }

    #[inline]
    pub fn stroke(mut self, stroke: impl Into<Stroke>) -> Self {
        self.stroke = stroke.into();
        self
    }

    #[inline]
    pub fn material(mut self, material: GlassMaterial) -> Self {
        self.material = material;
        self
    }

    /// `egui::Frame` datar yang setara — untuk tempat yang menuntut `Frame`
    /// (mis. `egui::Window::frame`) dan untuk jalur tanpa GPU.
    pub fn flat(&self) -> Frame {
        Frame {
            inner_margin: self.inner_margin,
            outer_margin: self.outer_margin,
            corner_radius: self.corner_radius,
            shadow: self.shadow,
            fill: self.fill,
            stroke: self.stroke,
        }
    }

    /// `egui::Frame` transparan dengan margin, radius, dan bayangan yang sama —
    /// untuk kontainer egui yang menuntut `Frame` (`Window`, `Popup`,
    /// `Modal`, menu). Kaca lalu dilukis di dalamnya lewat [`Self::paint_behind`].
    pub fn transparent_flat(&self) -> Frame {
        Frame {
            fill: Color32::TRANSPARENT,
            stroke: Stroke::new(self.stroke.width, Color32::TRANSPARENT),
            ..self.flat()
        }
    }

    /// Lukis kaca DI BAWAH konten `add_contents` di dalam kontainer yang
    /// frame-nya sudah dibuka egui dengan [`Self::transparent_flat`].
    ///
    /// Rect kaca = rect konten + `inner_margin` frame (persis rect isian
    /// `egui::Frame`), ditambah `extend` (mis. tinggi title bar `Window`, yang
    /// berada di luar `Ui` konten). Tanpa GPU, isian datar dilukis di tempat
    /// yang sama sehingga kontainer tidak pernah tembus pandang.
    pub fn paint_behind<R>(
        self,
        ui: &mut Ui,
        extend: Margin,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let background = ui.painter().add(Shape::Noop);
        let inner = add_contents(ui);
        let rect = (ui.min_rect() + self.inner_margin) + extend;
        let shape = if runtime(ui.ctx()).uses_gpu() {
            self.glass_shape(ui, rect)
        } else {
            Shape::Vec(vec![
                Shape::from(self.shadow.as_shape(rect, self.corner_radius)),
                Shape::rect_filled(rect, self.corner_radius, self.fill),
                Shape::rect_stroke(rect, self.corner_radius, self.stroke, egui::StrokeKind::Inside),
            ])
        };
        ui.painter().set(background, shape);
        inner
    }

    /// Seperti [`Self::paint_behind`] untuk konten `egui::Window`/`Area`:
    /// kaca diperluas ke atas sampai tepi area (menutup title bar, yang berada
    /// di luar `Ui` konten). `area_id` = id `Area` kontainer; untuk
    /// `egui::Window::new(title)` itu `egui::Id::new(title)` kecuali diganti
    /// lewat `.id(..)`.
    pub fn paint_behind_window<R>(
        self,
        ui: &mut Ui,
        area_id: egui::Id,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let area_rect = ui.ctx().memory(|m| m.area_rect(area_id));
        let content_top = ui.min_rect().top();
        let extend = match area_rect {
            Some(area) if area.top().is_finite() => {
                let fill_top = content_top - f32::from(self.inner_margin.top);
                Margin {
                    top: (fill_top - area.top() - self.stroke.width).max(0.0).round() as i8,
                    ..Margin::ZERO
                }
            }
            _ => Margin::ZERO,
        };
        self.paint_behind(ui, extend, add_contents)
    }

    /// Lukis kaca (atau isian datar tanpa GPU) pada `rect` lewat painter `ui`,
    /// untuk kartu yang digambar manual dengan `Painter` (mis. panduan tool
    /// beranimasi). Panggil SEBELUM menggambar isinya.
    pub fn paint(&self, ui: &Ui, rect: egui::Rect) {
        let shape = if runtime(ui.ctx()).uses_gpu() {
            self.glass_shape(ui, rect)
        } else {
            Shape::Vec(vec![
                Shape::from(self.shadow.as_shape(rect, self.corner_radius)),
                Shape::rect_filled(rect, self.corner_radius, self.fill),
                Shape::rect_stroke(rect, self.corner_radius, self.stroke, egui::StrokeKind::Inside),
            ])
        };
        ui.painter().add(shape);
    }

    /// Bentuk kaca (bayangan + callback GPU) untuk `rect`.
    fn glass_shape(&self, ui: &Ui, rect: egui::Rect) -> Shape {
        let ppp = ui.ctx().pixels_per_point();
        let mut uniform = PanelUniform::new(&self.material, rect, self.radius(), ppp)
            .with_opacity(ui.opacity());
        if self.material.interactive > 0.0 {
            let anim_id = ui.id().with(("ducad-glass", rect.min.x as i32, rect.min.y as i32));
            let (pos, down) = ui.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down()));
            let hovered = pos.is_some_and(|p| rect.contains(p)) && ui.rect_contains_pointer(rect);
            let target = match (hovered, down) {
                (true, true) => 1.0,
                (true, false) => 0.35,
                _ => 0.0,
            };
            let strength = ui.ctx().animate_value_with_time(anim_id, target, 0.18);
            if let Some(pos) = pos {
                uniform = uniform.with_press(pos, strength * self.material.interactive);
            }
        }
        let mut shapes = Vec::with_capacity(2);
        if self.shadow != Shadow::NONE {
            shapes.push(Shape::from(self.shadow.as_shape(rect, self.corner_radius)));
        }
        // Satu point ekstra agar tepi anti-alias tidak terpotong viewport.
        shapes.push(Shape::Callback(egui_wgpu::Callback::new_paint_callback(
            rect.expand(1.0),
            GlassCallback {
                uniform,
                slot: AtomicU32::new(u32::MAX),
            },
        )));
        Shape::Vec(shapes)
    }

    /// Jari-jari sudut tunggal yang dipakai shader (sudut terbesar).
    fn radius(&self) -> f32 {
        let r = self.corner_radius;
        f32::from(r.nw.max(r.ne).max(r.sw).max(r.se))
    }

    /// Tampilkan konten di dalam frame; padanan `egui::Frame::show`.
    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        if !runtime(ui.ctx()).uses_gpu() {
            return self.flat().show(ui, add_contents);
        }

        // Tempat latar kaca dipesan lebih dulu supaya berada di bawah konten.
        let background = ui.painter().add(Shape::Noop);

        // Tata letak tetap dikerjakan `egui::Frame` (margin + lebar stroke),
        // tetapi tanpa cat: isian, bayangan, dan tepi digambar di bawah ini.
        let layout = Frame {
            fill: Color32::TRANSPARENT,
            shadow: Shadow::NONE,
            stroke: Stroke::new(self.stroke.width, Color32::TRANSPARENT),
            ..self.flat()
        };
        let mut prepared = layout.begin(ui);
        let inner = add_contents(&mut prepared.content_ui);
        let rect = layout.widget_rect(prepared.content_ui.min_rect());
        let response = prepared.end(ui);

        if ui.is_rect_visible(rect) {
            let shape = self.glass_shape(ui, rect);
            ui.painter().set(background, shape);
        }

        InnerResponse::new(inner, response)
    }
}

impl From<GlassFrame> for Frame {
    fn from(frame: GlassFrame) -> Self {
        frame.flat()
    }
}

/// Callback wgpu satu panel kaca.
struct GlassCallback {
    uniform: PanelUniform,
    /// Slot uniform frame ini; diisi di `prepare`, dibaca di `paint`.
    slot: AtomicU32,
}

impl egui_wgpu::CallbackTrait for GlassCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(backdrop) = resources.get_mut::<GlassBackdrop>() {
            self.slot
                .store(backdrop.stage_panel(self.uniform), Ordering::Relaxed);
        }
        Vec::new()
    }

    fn finish_prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(backdrop) = resources.get_mut::<GlassBackdrop>() {
            backdrop.finish(device, queue, encoder);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        rpass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let slot = self.slot.load(Ordering::Relaxed);
        if slot == u32::MAX {
            return;
        }
        if let Some(backdrop) = resources.get::<GlassBackdrop>() {
            backdrop.paint_panel(rpass, slot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Jalankan satu frame; mengembalikan rect panel dan apakah ada callback GPU.
    fn run(ctx: &egui::Context, frame: GlassFrame) -> (egui::Rect, bool) {
        let mut rect = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            rect = frame
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(120.0, 40.0));
                    ui.label("kaca");
                })
                .response
                .rect;
        });
        // egui panik bila delta tekstur di-drop tanpa ditangani.
        output.textures_delta.clear();
        (rect, output.shapes.iter().any(|s| has_callback(&s.shape)))
    }

    fn has_callback(shape: &Shape) -> bool {
        match shape {
            Shape::Callback(_) => true,
            Shape::Vec(v) => v.iter().any(has_callback),
            _ => false,
        }
    }

    fn frame() -> GlassFrame {
        GlassFrame::preset(GlassPreset::Panel, GlassMode::Dark)
            .inner_margin(Margin::symmetric(10, 5))
            .stroke(Stroke::new(1.0, Color32::GRAY))
    }

    #[test]
    fn runtime_defaults_to_flat() {
        let ctx = egui::Context::default();
        assert!(!runtime(&ctx).uses_gpu());
        let on = GlassRuntime {
            enabled: true,
            gpu: true,
            reduce_transparency: false,
        };
        set_runtime(
            &ctx,
            GlassRuntime {
                reduce_transparency: true,
                ..on
            },
        );
        assert!(
            !runtime(&ctx).uses_gpu(),
            "kurangi transparansi harus menang"
        );
        set_runtime(
            &ctx,
            GlassRuntime {
                enabled: false,
                ..on
            },
        );
        assert!(!runtime(&ctx).uses_gpu(), "preferensi mati harus menang");
        set_runtime(&ctx, GlassRuntime { gpu: false, ..on });
        assert!(
            !runtime(&ctx).uses_gpu(),
            "tanpa backdrop GPU tidak ada kaca"
        );
        set_runtime(&ctx, on);
        assert!(runtime(&ctx).uses_gpu());
    }

    #[test]
    fn flat_path_emits_no_gpu_callback() {
        let ctx = egui::Context::default();
        let (_, callback) = run(&ctx, frame());
        assert!(!callback);
    }

    #[test]
    fn glass_path_emits_callback_and_keeps_layout() {
        let flat_ctx = egui::Context::default();
        let (flat_rect, _) = run(&flat_ctx, frame());

        let ctx = egui::Context::default();
        set_runtime(
            &ctx,
            GlassRuntime {
                enabled: true,
                gpu: true,
                reduce_transparency: false,
            },
        );
        let (glass_rect, callback) = run(&ctx, frame());

        assert!(callback);
        // Beralih kaca ↔ datar tidak boleh menggeser tata letak panel.
        assert_eq!(flat_rect, glass_rect);
    }

    #[test]
    fn paint_behind_covers_content_plus_margin_in_both_modes() {
        for gpu in [false, true] {
            let ctx = egui::Context::default();
            set_runtime(
                &ctx,
                GlassRuntime { enabled: gpu, gpu, reduce_transparency: false },
            );
            let mut content = egui::Rect::NOTHING;
            let mut output = ctx.run_ui(Default::default(), |ui| {
                frame().transparent_flat().show(ui, |ui| {
                    frame().paint_behind(ui, Margin::ZERO, |ui| {
                        content = ui.label("isi").rect;
                    });
                });
            });
            output.textures_delta.clear();
            let callback = output.shapes.iter().any(|s| has_callback(&s.shape));
            assert_eq!(callback, gpu);
            if !gpu {
                // Tanpa GPU: isian datar pekat seukuran konten + margin.
                let expected = content + Margin::symmetric(10, 5);
                let found = output.shapes.iter().any(|s| match &s.shape {
                    Shape::Vec(v) => v.iter().any(|sh| {
                        matches!(sh, Shape::Rect(r) if r.fill.a() >= 200 && (r.rect.min - expected.min).length() < 0.5)
                    }),
                    _ => false,
                });
                assert!(found, "isian datar fallback tidak ditemukan");
            }
        }
    }

    #[test]
    fn flat_frame_carries_all_fields() {
        let f = frame()
            .corner_radius(CornerRadius::same(14))
            .fill(Color32::RED);
        let flat: Frame = f.into();
        assert_eq!(flat.inner_margin, Margin::symmetric(10, 5));
        assert_eq!(flat.corner_radius, CornerRadius::same(14));
        assert_eq!(flat.fill, Color32::RED);
        assert_eq!(flat.stroke.width, 1.0);
    }
}
