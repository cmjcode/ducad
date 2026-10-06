//! Tutorial selamat datang interaktif (gaya Shapr3D).
//!
//! Saat aplikasi dibuka pertama kali, kartu sambutan muncul lalu berlanjut ke
//! rangkaian pelajaran. Tiap pelajaran memutar animasi demonstrasi, menyorot
//! tombol yang dimaksud, dan baru bisa dilanjutkan setelah pengguna benar-benar
//! mencoba aksinya. Widget ini murni egui: deteksi aksi dilakukan pemanggil
//! (`ducad-app`) yang menyetel [`OnboardingState::done`].

use ducad_i18n::{t, Language};
use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_AUTO_AWESOME, ICON_CELEBRATION, ICON_CHECK_CIRCLE,
    ICON_CLOSE, ICON_DESCRIPTION, ICON_DRAW, ICON_FOUNTAIN_PEN_TIP, ICON_HUB,
    ICON_PRECISION_MANUFACTURING, ICON_RADIO_BUTTON_UNCHECKED, ICON_SAVE, ICON_SCHOOL,
    ICON_SCIENCE, ICON_SEARCH, ICON_TOUCH_APP, ICON_VIEW_IN_AR, ICON_WAVING_HAND,
};

use crate::left_toolbar::ToolbarTool;
use crate::theme::{
    ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use crate::tool_guides::ToolGuides;

/// Aksi yang harus dicoba pengguna supaya sebuah pelajaran lulus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingGoal {
    /// Tidak ada syarat (kartu sambutan, tur fitur lanjutan).
    None,
    DrawRectangle,
    /// Memilih sesuatu di kanvas dengan tool Pilih.
    UseSelect,
    DrawCircle,
    Extrude,
    Navigate,
    PushPull,
    Fillet,
    /// Pindah mode 2D/3D dan berakhir di mode Sketsa.
    ToggleMode,
    OpenPalette,
    OpenChat,
    Save,
}

/// Elemen UI yang disorot selama sebuah pelajaran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OnboardingTarget {
    Tool(ToolbarTool),
    ModeButton,
    PaletteButton,
    ChatButton,
    ContextBar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingStepKind {
    /// Kartu sambutan di tengah layar.
    Welcome,
    /// Pelajaran dengan animasi dan syarat mencoba.
    Lesson,
    /// Ringkasan fitur lanjutan + penutup.
    Tour,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Demo {
    None,
    Tool(ToolbarTool),
    Select,
    Navigate,
    PushPull,
    Fillet,
    Mode,
    Palette,
    Chat,
    Save,
}

/// Satu langkah tutorial. Teksnya diambil dari kunci i18n
/// `onboard-<key>-title`, `onboard-<key>-body`, dan `onboard-<key>-try`.
#[derive(Debug, Clone, Copy)]
pub struct OnboardingStep {
    pub key: &'static str,
    pub kind: OnboardingStepKind,
    pub goal: OnboardingGoal,
    pub target: Option<OnboardingTarget>,
    demo: Demo,
}

const fn lesson(
    key: &'static str,
    goal: OnboardingGoal,
    target: Option<OnboardingTarget>,
    demo: Demo,
) -> OnboardingStep {
    OnboardingStep {
        key,
        kind: OnboardingStepKind::Lesson,
        goal,
        target,
        demo,
    }
}

/// Urutan tutorial: kenali mode, gambar persegi, pilih dengan tool Pilih,
/// bulatkan sudutnya selagi masih sketsa, lalu jadikan solid. Navigasi 3D diajarkan
/// setelah ada solid untuk dilihat.
pub const STEPS: &[OnboardingStep] = &[
    OnboardingStep {
        key: "welcome",
        kind: OnboardingStepKind::Welcome,
        goal: OnboardingGoal::None,
        target: None,
        demo: Demo::None,
    },
    // Tombol mode 2D/3D dipakai di semua langkah berikutnya, jadi dijelaskan
    // lebih dulu.
    lesson(
        "mode",
        OnboardingGoal::ToggleMode,
        Some(OnboardingTarget::ModeButton),
        Demo::Mode,
    ),
    lesson(
        "rect",
        OnboardingGoal::DrawRectangle,
        Some(OnboardingTarget::Tool(ToolbarTool::Rectangle)),
        Demo::Tool(ToolbarTool::Rectangle),
    ),
    // Tool Pilih dijelaskan begitu ada objek untuk dipilih. Fillet dan extrude
    // tidak punya ikon di bilah kiri; keduanya muncul setelah sesuatu dipilih.
    lesson(
        "select",
        OnboardingGoal::UseSelect,
        Some(OnboardingTarget::Tool(ToolbarTool::Select)),
        Demo::Select,
    ),
    lesson(
        "fillet",
        OnboardingGoal::Fillet,
        Some(OnboardingTarget::Tool(ToolbarTool::Select)),
        Demo::Fillet,
    ),
    lesson(
        "circle",
        OnboardingGoal::DrawCircle,
        Some(OnboardingTarget::Tool(ToolbarTool::Circle)),
        Demo::Tool(ToolbarTool::Circle),
    ),
    lesson(
        "extrude",
        OnboardingGoal::Extrude,
        Some(OnboardingTarget::ContextBar),
        Demo::Tool(ToolbarTool::Extrude),
    ),
    lesson("navigate", OnboardingGoal::Navigate, None, Demo::Navigate),
    lesson(
        "pushpull",
        OnboardingGoal::PushPull,
        Some(OnboardingTarget::ContextBar),
        Demo::PushPull,
    ),
    lesson(
        "palette",
        OnboardingGoal::OpenPalette,
        Some(OnboardingTarget::PaletteButton),
        Demo::Palette,
    ),
    lesson("save", OnboardingGoal::Save, None, Demo::Save),
    // Chat AI paling akhir: butuh penyiapan agent, jadi tidak boleh
    // menghalangi pelajaran memodelkan.
    lesson(
        "chat",
        OnboardingGoal::OpenChat,
        Some(OnboardingTarget::ChatButton),
        Demo::Chat,
    ),
    OnboardingStep {
        key: "tour",
        kind: OnboardingStepKind::Tour,
        goal: OnboardingGoal::None,
        target: None,
        demo: Demo::None,
    },
];

/// State tutorial. `done` disetel pemanggil saat aksi pelajaran terdeteksi.
#[derive(Debug, Clone, Default)]
pub struct OnboardingState {
    pub open: bool,
    pub step: usize,
    pub done: bool,
}

impl OnboardingState {
    pub fn current(&self) -> &'static OnboardingStep {
        &STEPS[self.step.min(STEPS.len() - 1)]
    }

    /// Pelajaran boleh dilanjutkan: tanpa syarat, atau syaratnya sudah dicoba.
    pub fn can_continue(&self) -> bool {
        self.done || self.current().goal == OnboardingGoal::None
    }

    /// Mulai dari kartu sambutan.
    pub fn restart(&mut self) {
        self.open = true;
        self.step = 0;
        self.done = false;
    }

    /// Pindah ke `step` (dijepit ke rentang langkah) dan kunci lagi syaratnya.
    pub fn go_to(&mut self, step: usize) {
        self.step = step.min(STEPS.len() - 1);
        self.done = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingEvent {
    /// Pindah ke langkah lain (maju, mundur, atau melewati satu langkah).
    StepChanged,
    /// Tutorial ditutup sebelum selesai.
    Dismissed,
    /// Tutorial diselesaikan sampai akhir.
    Finished,
    SetLanguage(Language),
}

/// Nama tombol pengubah pintasan di platform ini; mengisi `{ $modkey }` pada
/// teks pelajaran supaya pintasan yang ditulis sama dengan yang ditekan.
pub const MOD_KEY: &str = if cfg!(any(target_os = "macos", target_os = "ios")) {
    "Cmd"
} else {
    "Ctrl"
};

/// Perintah yang diketik pada animasi pelajaran palet. `ducad-app` memakai
/// konstanta ini sebagai label perintahnya, jadi demo dan palet selalu sama.
pub const PALETTE_DEMO_COMMAND: &str = "Extrude (Ekstrusi Profil / Sisi)";

const CARD_W: f32 = 320.0;
const DEMO_H: f32 = 132.0;
const WELCOME_W: f32 = 440.0;

pub struct Onboarding;

impl Onboarding {
    fn target_id(target: OnboardingTarget) -> egui::Id {
        egui::Id::new(("ducad-onboarding-target", target))
    }

    /// Dipanggil widget pemilik tombol (top bar, toolbar, bilah konteks) tiap
    /// frame supaya tutorial tahu letak tombol yang harus disorot.
    pub fn publish_target(ctx: &egui::Context, target: OnboardingTarget, rect: Rect) {
        let frame = ctx.cumulative_frame_nr();
        ctx.data_mut(|d| d.insert_temp(Self::target_id(target), (frame, rect)));
    }

    /// Rect tombol sasaran bila dirender frame ini atau frame sebelumnya
    /// (tutorial bisa digambar sebelum maupun sesudah pemilik tombolnya).
    pub fn target_rect(ctx: &egui::Context, target: OnboardingTarget) -> Option<Rect> {
        let frame = ctx.cumulative_frame_nr();
        ctx.data(|d| d.get_temp::<(u64, Rect)>(Self::target_id(target)))
            .filter(|(f, r)| frame.saturating_sub(*f) <= 1 && r.is_positive())
            .map(|(_, r)| r)
    }

    /// Render tutorial di dalam `bounds` (area kanvas yang tidak tertutup
    /// sidebar). Navigasi langkah diterapkan langsung pada `state`; event
    /// dikembalikan supaya pemanggil bisa menyimpan progres.
    pub fn show(
        ctx: &egui::Context,
        bounds: Rect,
        state: &mut OnboardingState,
    ) -> Option<OnboardingEvent> {
        if !state.open {
            return None;
        }
        // Animasi demo dan denyut sorotan berjalan terus selama tutorial tampil.
        ctx.request_repaint();
        let step = *state.current();
        match step.kind {
            OnboardingStepKind::Welcome => Self::show_welcome(ctx, state),
            OnboardingStepKind::Tour => Self::show_tour(ctx, state),
            OnboardingStepKind::Lesson => {
                if let Some(rect) = step.target.and_then(|t| Self::target_rect(ctx, t)) {
                    if !state.done {
                        paint_spotlight(ctx, rect);
                    }
                }
                Self::show_lesson(ctx, bounds, state, &step)
            }
        }
    }

    fn show_welcome(ctx: &egui::Context, state: &mut OnboardingState) -> Option<OnboardingEvent> {
        let mut event = None;
        egui::Modal::new(egui::Id::new("ducad-onboarding-welcome"))
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                card_frame().show(ui, |ui| {
                    ui.set_width(WELCOME_W.min(ui.ctx().content_rect().width() - 48.0));
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(ICON_WAVING_HAND.codepoint)
                                .size(34.0)
                                .color(ACCENT_BLUE),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(t!("onboard-welcome-title"))
                                .size(20.0)
                                .strong()
                                .color(TEXT_PRIMARY),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(t!("onboard-welcome-body"))
                                .size(12.5)
                                .color(TEXT_SECONDARY),
                        );
                    });
                    ui.add_space(12.0);
                    bullet_list(
                        ui,
                        &[
                            (ICON_DRAW.codepoint, "onboard-welcome-point-sketch"),
                            (ICON_VIEW_IN_AR.codepoint, "onboard-welcome-point-solid"),
                            (ICON_AUTO_AWESOME.codepoint, "onboard-welcome-point-ai"),
                        ],
                    );
                    ui.add_space(10.0);
                    // Bahasa bawaan Inggris; pengguna baru bisa langsung menggantinya di sini.
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(t!("lang-current"))
                                .size(11.0)
                                .color(TEXT_MUTED),
                        );
                        let current = ducad_i18n::current_language();
                        for (lang, key) in [(Language::En, "lang-en"), (Language::Id, "lang-id")] {
                            if ui
                                .selectable_label(
                                    current == lang,
                                    RichText::new(t!(key)).size(11.0),
                                )
                                .clicked()
                                && current != lang
                            {
                                event = Some(OnboardingEvent::SetLanguage(lang));
                            }
                        }
                    });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.add(secondary_button(&t!("onboard-skip-all"))).clicked() {
                            state.open = false;
                            event = Some(OnboardingEvent::Dismissed);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(primary_button(&format!(
                                    "{}  {}",
                                    t!("onboard-start"),
                                    ICON_ARROW_FORWARD.codepoint
                                )))
                                .clicked()
                            {
                                state.go_to(1);
                                event = Some(OnboardingEvent::StepChanged);
                            }
                        });
                    });
                });
            });
        event
    }

    fn show_tour(ctx: &egui::Context, state: &mut OnboardingState) -> Option<OnboardingEvent> {
        let mut event = None;
        egui::Modal::new(egui::Id::new("ducad-onboarding-tour"))
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                card_frame().show(ui, |ui| {
                    ui.set_width(WELCOME_W.min(ui.ctx().content_rect().width() - 48.0));
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(ICON_CELEBRATION.codepoint)
                                .size(30.0)
                                .color(ACCENT_GREEN),
                        );
                        ui.label(
                            RichText::new(t!("onboard-tour-title"))
                                .size(18.0)
                                .strong()
                                .color(TEXT_PRIMARY),
                        );
                        ui.label(
                            RichText::new(t!("onboard-tour-body"))
                                .size(12.0)
                                .color(TEXT_SECONDARY),
                        );
                    });
                    ui.add_space(10.0);
                    bullet_list(
                        ui,
                        &[
                            (ICON_FOUNTAIN_PEN_TIP.codepoint, "onboard-tour-vector"),
                            (ICON_SCIENCE.codepoint, "onboard-tour-sim"),
                            (ICON_DESCRIPTION.codepoint, "onboard-tour-drawing"),
                            (
                                ICON_PRECISION_MANUFACTURING.codepoint,
                                "onboard-tour-industry",
                            ),
                            (ICON_HUB.codepoint, "onboard-tour-agent"),
                        ],
                    );
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(t!("onboard-tour-reopen"))
                                .size(11.0)
                                .color(TEXT_MUTED),
                        )
                        .wrap(),
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(secondary_button(&format!(
                                "{}  {}",
                                ICON_ARROW_BACK.codepoint,
                                t!("onboard-back")
                            )))
                            .clicked()
                        {
                            state.go_to(state.step.saturating_sub(1));
                            event = Some(OnboardingEvent::StepChanged);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(primary_button(&t!("onboard-finish"))).clicked() {
                                state.open = false;
                                event = Some(OnboardingEvent::Finished);
                            }
                        });
                    });
                });
            });
        event
    }

    fn show_lesson(
        ctx: &egui::Context,
        bounds: Rect,
        state: &mut OnboardingState,
        step: &OnboardingStep,
    ) -> Option<OnboardingEvent> {
        let mut event = None;
        let time = ctx.input(|i| i.time);
        // Pojok kanan atas kanvas, di bawah top bar: pojok kiri bawah dipakai
        // kartu panduan tool, tengah bawah dipakai bilah konteks. Kartu bisa
        // digeser bila menutupi bagian yang sedang dikerjakan.
        let default_pos = Pos2::new(bounds.right() - CARD_W - 44.0, bounds.top() + 64.0);
        egui::Area::new(egui::Id::new("ducad-onboarding-lesson"))
            .default_pos(default_pos)
            .movable(true)
            .constrain_to(bounds)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                card_frame().show(ui, |ui| {
                    ui.set_width(CARD_W);
                    let lessons = STEPS.len() - 2;
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(ICON_SCHOOL.codepoint)
                                .size(15.0)
                                .color(ACCENT_BLUE),
                        );
                        ui.label(
                            RichText::new(t!(
                                "onboard-progress",
                                current = state.step as i64,
                                total = lessons as i64
                            ))
                            .size(11.0)
                            .color(TEXT_SECONDARY),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(ICON_CLOSE.codepoint)
                                            .size(13.0)
                                            .color(TEXT_SECONDARY),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text(t!("onboard-skip-all"))
                                .clicked()
                            {
                                state.open = false;
                                event = Some(OnboardingEvent::Dismissed);
                            }
                        });
                    });
                    paint_progress(ui, state.step as f32 / lessons as f32);
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(t!(&format!("onboard-{}-title", step.key)))
                            .size(15.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.add_space(4.0);

                    let (demo_rect, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), DEMO_H),
                        egui::Sense::hover(),
                    );
                    paint_demo(ui, demo_rect, step.demo, time);

                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(t!(
                                &format!("onboard-{}-body", step.key),
                                modkey = MOD_KEY
                            ))
                                .size(12.0)
                                .color(TEXT_SECONDARY),
                        )
                        .wrap(),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if state.done {
                            ui.label(
                                RichText::new(ICON_CHECK_CIRCLE.codepoint)
                                    .size(15.0)
                                    .color(ACCENT_GREEN),
                            );
                            ui.label(
                                RichText::new(t!("onboard-done"))
                                    .size(12.0)
                                    .strong()
                                    .color(ACCENT_GREEN),
                            );
                        } else {
                            ui.label(
                                RichText::new(ICON_RADIO_BUTTON_UNCHECKED.codepoint)
                                    .size(15.0)
                                    .color(ACCENT_ORANGE),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(t!(&format!("onboard-{}-try", step.key)))
                                        .size(12.0)
                                        .color(TEXT_PRIMARY),
                                )
                                .wrap(),
                            );
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(secondary_button(&format!(
                                "{}  {}",
                                ICON_ARROW_BACK.codepoint,
                                t!("onboard-back")
                            )))
                            .clicked()
                        {
                            state.go_to(state.step.saturating_sub(1));
                            event = Some(OnboardingEvent::StepChanged);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let next = ui
                                .add_enabled(
                                    state.can_continue(),
                                    primary_button(&format!(
                                        "{}  {}",
                                        t!("onboard-next"),
                                        ICON_ARROW_FORWARD.codepoint
                                    )),
                                )
                                .on_disabled_hover_text(t!("onboard-next-locked"));
                            if next.clicked() {
                                state.go_to(state.step + 1);
                                event = Some(OnboardingEvent::StepChanged);
                            }
                            if !state.done
                                && ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t!("onboard-skip-step"))
                                                .size(11.0)
                                                .color(TEXT_MUTED),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                            {
                                state.go_to(state.step + 1);
                                event = Some(OnboardingEvent::StepChanged);
                            }
                        });
                    });
                });
            });
        event
    }
}

fn card_frame() -> ducad_glass::GlassFrame {
    crate::theme::popup_frame()
        .inner_margin(egui::Margin::same(14))
        .corner_radius(CornerRadius::same(12))
        .shadow(egui::Shadow {
            offset: [0, 6],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(110),
        })
}

/// Daftar butir `(ikon, kunci i18n)`.
fn bullet_list(ui: &mut Ui, items: &[(&str, &str)]) {
    for (icon, key) in items {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(*icon).size(16.0).color(ACCENT_BLUE));
            ui.add(egui::Label::new(RichText::new(t!(*key)).size(12.0).color(TEXT_PRIMARY)).wrap());
        });
        ui.add_space(3.0);
    }
}

fn primary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(text.to_owned())
            .size(12.5)
            .color(Color32::WHITE),
    )
    .fill(ACCENT_BLUE)
    .corner_radius(CornerRadius::same(7))
    .min_size(Vec2::new(0.0, crate::theme::MIN_TOUCH_TARGET))
}

fn secondary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(text.to_owned())
            .size(12.0)
            .color(TEXT_SECONDARY),
    )
    .corner_radius(CornerRadius::same(7))
    .min_size(Vec2::new(0.0, crate::theme::MIN_TOUCH_TARGET))
}

fn paint_progress(ui: &mut Ui, fraction: f32) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 3.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 1.5, BORDER_SUBTLE);
    let mut filled = rect;
    filled.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    painter.rect_filled(filled, 1.5, ACCENT_BLUE);
}

/// Cincin berdenyut di sekitar tombol sasaran. Hanya dilukis (tanpa
/// `interact`), jadi tombol di bawahnya tetap bisa diklik.
fn paint_spotlight(ctx: &egui::Context, target: Rect) {
    let time = ctx.input(|i| i.time);
    let pulse = ((time * 3.0).sin() * 0.5 + 0.5) as f32;
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("ducad-onboarding-spotlight"),
    ));
    let ring = target.expand(3.0 + pulse * 3.0);
    painter.rect_stroke(
        ring,
        CornerRadius::same(9),
        Stroke::new(2.0, ACCENT_ORANGE),
        StrokeKind::Outside,
    );
    painter.rect_stroke(
        ring.expand(4.0 + pulse * 4.0),
        CornerRadius::same(12),
        Stroke::new(
            1.5,
            ACCENT_ORANGE.gamma_multiply(0.35 * (1.0 - pulse) + 0.1),
        ),
        StrokeKind::Outside,
    );
}

fn paint_demo(ui: &Ui, rect: Rect, demo: Demo, time: f64) {
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        CornerRadius::same(8),
        Color32::from_rgba_premultiplied(10, 12, 16, 200),
        Stroke::new(1.0, BORDER_SUBTLE),
        StrokeKind::Inside,
    );
    match demo {
        Demo::None => {}
        Demo::Tool(tool) => ToolGuides::paint_demo(&painter, rect, tool, time),
        Demo::Select => demo_select(&painter, rect, time),
        Demo::Navigate => demo_navigate(&painter, rect, time),
        Demo::PushPull => demo_push_pull(&painter, rect, time),
        Demo::Fillet => demo_fillet(&painter, rect, time),
        Demo::Mode => demo_mode(&painter, rect, time),
        Demo::Palette => demo_palette(&painter, rect, time),
        Demo::Chat => demo_chat(&painter, rect, time),
        Demo::Save => demo_save(&painter, rect, time),
    }
}

/// Fase 0..1 dari siklus animasi sepanjang `cycle` detik.
fn phase(time: f64, cycle: f64) -> f32 {
    ((time % cycle) / cycle) as f32
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Proyeksi isometrik sederhana titik `(x, y, z)` yang diputar `yaw` radian.
fn iso(center: Pos2, yaw: f32, x: f32, y: f32, z: f32) -> Pos2 {
    let (s, c) = yaw.sin_cos();
    let rx = x * c - y * s;
    let ry = x * s + y * c;
    Pos2::new(center.x + rx, center.y + ry * 0.5 - z)
}

/// Balok kawat berukuran setengah-lebar `w`, setengah-dalam `d`, tinggi `h`
/// dengan alas di `z = 0`.
fn paint_box(painter: &egui::Painter, center: Pos2, yaw: f32, size: Vec2, h: f32, color: Color32) {
    let (w, d) = (size.x, size.y);
    let corners = [(-w, -d), (w, -d), (w, d), (-w, d)];
    let stroke = Stroke::new(1.5, color);
    for i in 0..4 {
        let (x0, y0) = corners[i];
        let (x1, y1) = corners[(i + 1) % 4];
        painter.line_segment(
            [iso(center, yaw, x0, y0, 0.0), iso(center, yaw, x1, y1, 0.0)],
            stroke,
        );
        painter.line_segment(
            [iso(center, yaw, x0, y0, h), iso(center, yaw, x1, y1, h)],
            stroke,
        );
        painter.line_segment(
            [iso(center, yaw, x0, y0, 0.0), iso(center, yaw, x0, y0, h)],
            stroke,
        );
    }
    let top: Vec<Pos2> = corners
        .iter()
        .map(|(x, y)| iso(center, yaw, *x, *y, h))
        .collect();
    painter.add(egui::Shape::convex_polygon(
        top,
        color.gamma_multiply(0.18),
        Stroke::NONE,
    ));
}

/// Kursor mengeklik satu garis persegi; garis tersorot dan gagang muncul.
fn demo_select(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 3.4);
    let shape = Rect::from_center_size(rect.center(), Vec2::new(130.0, 76.0));
    let picked = p > 0.45;
    painter.rect_stroke(
        shape,
        CornerRadius::ZERO,
        Stroke::new(1.5, ACCENT_BLUE),
        StrokeKind::Inside,
    );
    let hit = Pos2::new(shape.center().x + 20.0, shape.top());
    if picked {
        painter.line_segment(
            [shape.left_top(), shape.right_top()],
            Stroke::new(2.5, ACCENT_ORANGE),
        );
        for corner in [
            shape.left_top(),
            shape.right_top(),
            shape.left_bottom(),
            shape.right_bottom(),
        ] {
            painter.circle_stroke(corner, 4.5, Stroke::new(1.5, ACCENT_GREEN));
        }
    }
    let travel = smooth(p / 0.4);
    let start = Pos2::new(rect.left() + 24.0, rect.bottom() - 30.0);
    let cursor = start + (hit - start) * travel;
    ToolGuides::paint_cursor(painter, cursor, p > 0.38 && p < 0.6, time);
}

fn demo_navigate(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 4.0);
    let swing = (p * std::f32::consts::TAU).sin();
    let yaw = 0.6 + swing * 0.9;
    let center = Pos2::new(rect.center().x, rect.center().y + 34.0);
    paint_box(painter, center, yaw, Vec2::splat(34.0), 38.0, ACCENT_BLUE);
    let cursor = Pos2::new(rect.center().x + swing * 60.0, rect.top() + 16.0);
    painter.line_segment(
        [
            Pos2::new(rect.center().x - 60.0, cursor.y),
            Pos2::new(rect.center().x + 60.0, cursor.y),
        ],
        Stroke::new(1.0, TEXT_MUTED),
    );
    ToolGuides::paint_cursor(painter, cursor, true, time);
}

fn demo_push_pull(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 3.2);
    let pull = smooth((p - 0.25) / 0.5);
    let h = 14.0 + pull * 30.0;
    let center = Pos2::new(rect.center().x, rect.bottom() - 30.0);
    paint_box(painter, center, 0.75, Vec2::new(36.0, 30.0), h, ACCENT_BLUE);
    let top = iso(center, 0.75, 0.0, 0.0, h);
    // Panah tarik pada face atas.
    let tip = Pos2::new(top.x, top.y - 18.0);
    painter.line_segment([top, tip], Stroke::new(2.0, ACCENT_ORANGE));
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(tip.x, tip.y - 6.0),
            Pos2::new(tip.x - 5.0, tip.y + 2.0),
            Pos2::new(tip.x + 5.0, tip.y + 2.0),
        ],
        ACCENT_ORANGE,
        Stroke::NONE,
    ));
    painter.text(
        Pos2::new(rect.right() - 12.0, rect.top() + 12.0),
        Align2::RIGHT_TOP,
        format!("{:.0} mm", 10.0 + pull * 20.0),
        FontId::proportional(11.0),
        TEXT_PRIMARY,
    );
    ToolGuides::paint_cursor(
        painter,
        Pos2::new(top.x + 4.0, top.y + 2.0),
        p > 0.2 && p < 0.8,
        time,
    );
}

fn demo_fillet(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 3.0);
    let radius = smooth((p - 0.2) / 0.5) * 26.0;
    let shape = Rect::from_center_size(rect.center(), Vec2::new(130.0, 76.0));
    // Hanya sudut kanan atas yang dibulatkan: tepi terpilih.
    let corner = CornerRadius {
        nw: 0,
        ne: radius as u8,
        sw: 0,
        se: 0,
    };
    painter.rect(
        shape,
        corner,
        ACCENT_BLUE.gamma_multiply(0.18),
        Stroke::new(1.5, ACCENT_BLUE),
        StrokeKind::Inside,
    );
    let edge = shape.right_top();
    painter.circle_filled(
        Pos2::new(edge.x - radius * 0.3, edge.y + radius * 0.3),
        3.5,
        ACCENT_ORANGE,
    );
    painter.text(
        Pos2::new(rect.right() - 12.0, rect.top() + 8.0),
        Align2::RIGHT_TOP,
        format!("R {:.0}", radius / 2.6),
        FontId::proportional(11.0),
        TEXT_PRIMARY,
    );
    ToolGuides::paint_cursor(
        painter,
        Pos2::new(edge.x - radius * 0.6, edge.y + radius * 0.6),
        p > 0.15 && p < 0.75,
        time,
    );
}

fn demo_mode(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 3.0);
    let is_3d = p > 0.5;
    let center = Pos2::new(rect.center().x, rect.center().y + 14.0);
    if is_3d {
        paint_box(
            painter,
            Pos2::new(center.x, center.y + 26.0),
            0.75,
            Vec2::splat(30.0),
            30.0,
            ACCENT_BLUE,
        );
    } else {
        let flat = Rect::from_center_size(center, Vec2::new(70.0, 50.0));
        painter.rect(
            flat,
            CornerRadius::ZERO,
            ACCENT_BLUE.gamma_multiply(0.18),
            Stroke::new(1.5, ACCENT_BLUE),
            StrokeKind::Inside,
        );
    }
    let pill = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + 18.0),
        Vec2::new(96.0, 20.0),
    );
    painter.rect_filled(pill, 10.0, BORDER_SUBTLE);
    let half = Vec2::new(48.0, 20.0);
    let active = if is_3d {
        Rect::from_min_size(Pos2::new(pill.center().x, pill.top()), half)
    } else {
        Rect::from_min_size(pill.min, half)
    };
    painter.rect_filled(active, 10.0, ACCENT_BLUE);
    for (label, x) in [("2D", pill.left() + 24.0), ("3D", pill.right() - 24.0)] {
        painter.text(
            Pos2::new(x, pill.center().y),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
    ToolGuides::paint_cursor(
        painter,
        Pos2::new(active.center().x + 6.0, pill.bottom() - 4.0),
        (p % 0.5) < 0.12,
        time,
    );
}

fn demo_palette(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 4.0);
    let query = "extrude";
    let typed = ((p / 0.5).clamp(0.0, 1.0) * query.len() as f32) as usize;
    let field = Rect::from_min_size(
        Pos2::new(rect.left() + 26.0, rect.top() + 14.0),
        Vec2::new(rect.width() - 52.0, 24.0),
    );
    painter.rect(
        field,
        CornerRadius::same(6),
        Color32::from_rgba_premultiplied(30, 33, 40, 220),
        Stroke::new(1.0, ACCENT_BLUE),
        StrokeKind::Inside,
    );
    painter.text(
        Pos2::new(field.left() + 8.0, field.center().y),
        Align2::LEFT_CENTER,
        ICON_SEARCH.codepoint,
        FontId::proportional(13.0),
        TEXT_SECONDARY,
    );
    painter.text(
        Pos2::new(field.left() + 28.0, field.center().y),
        Align2::LEFT_CENTER,
        &query[..typed.min(query.len())],
        FontId::proportional(12.0),
        TEXT_PRIMARY,
    );
    // Sama dengan palet sungguhan: "extr" masih cocok dengan beberapa
    // perintah, "extrude" tinggal satu dan itulah yang dijalankan Enter.
    let narrowed = typed >= 5;
    let rows: &[&str] = if narrowed {
        &[PALETTE_DEMO_COMMAND]
    } else {
        &["Export SVG Vektor 2D", "Extend (Perpanjang Garis)", PALETTE_DEMO_COMMAND]
    };
    for (i, label) in rows.iter().enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(field.left(), field.bottom() + 6.0 + i as f32 * 22.0),
            Vec2::new(field.width(), 20.0),
        );
        if narrowed {
            painter.rect_filled(row, 5.0, ACCENT_BLUE.gamma_multiply(0.35));
        }
        painter.text(
            Pos2::new(row.left() + 8.0, row.center().y),
            Align2::LEFT_CENTER,
            *label,
            FontId::proportional(11.5),
            TEXT_PRIMARY,
        );
    }
}

fn demo_chat(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 4.5);
    let user = Rect::from_min_size(
        Pos2::new(rect.right() - 196.0, rect.top() + 12.0),
        Vec2::new(184.0, 26.0),
    );
    painter.rect_filled(user, 9.0, ACCENT_BLUE.gamma_multiply(0.55));
    painter.text(
        user.center(),
        Align2::CENTER_CENTER,
        t!("onboard-chat-demo-ask"),
        FontId::proportional(11.0),
        Color32::WHITE,
    );
    if p > 0.3 {
        let reply = Rect::from_min_size(
            Pos2::new(rect.left() + 12.0, user.bottom() + 10.0),
            Vec2::new(170.0, 26.0),
        );
        painter.rect_filled(
            reply,
            9.0,
            Color32::from_rgba_premultiplied(40, 45, 56, 220),
        );
        painter.text(
            Pos2::new(reply.left() + 10.0, reply.center().y),
            Align2::LEFT_CENTER,
            ICON_AUTO_AWESOME.codepoint,
            FontId::proportional(13.0),
            ACCENT_BLUE,
        );
        painter.text(
            Pos2::new(reply.left() + 30.0, reply.center().y),
            Align2::LEFT_CENTER,
            t!("onboard-chat-demo-reply"),
            FontId::proportional(11.0),
            TEXT_PRIMARY,
        );
    }
    if p > 0.55 {
        let grow = smooth((p - 0.55) / 0.3);
        paint_box(
            painter,
            Pos2::new(rect.right() - 56.0, rect.bottom() - 16.0),
            0.75,
            Vec2::splat(20.0),
            6.0 + grow * 20.0,
            ACCENT_GREEN,
        );
    }
}

fn demo_save(painter: &egui::Painter, rect: Rect, time: f64) {
    let saved = phase(time, 3.0) > 0.5;
    let center = rect.center();
    painter.text(
        Pos2::new(center.x, center.y - 8.0),
        Align2::CENTER_CENTER,
        ICON_SAVE.codepoint,
        FontId::proportional(40.0),
        if saved { ACCENT_GREEN } else { ACCENT_BLUE },
    );
    painter.text(
        Pos2::new(center.x, rect.bottom() - 22.0),
        Align2::CENTER_CENTER,
        if saved {
            "part.ducad"
        } else {
            "Untitled.ducad"
        },
        FontId::proportional(11.5),
        TEXT_PRIMARY,
    );
    let (badge, color) = if saved {
        (ICON_CHECK_CIRCLE.codepoint, ACCENT_GREEN)
    } else {
        (ICON_TOUCH_APP.codepoint, ACCENT_ORANGE)
    };
    painter.text(
        Pos2::new(center.x + 36.0, center.y - 24.0),
        Align2::CENTER_CENTER,
        badge,
        FontId::proportional(18.0),
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_start_with_welcome_and_end_with_tour() {
        assert_eq!(STEPS[0].kind, OnboardingStepKind::Welcome);
        assert_eq!(STEPS[STEPS.len() - 1].kind, OnboardingStepKind::Tour);
        assert!(STEPS[1..STEPS.len() - 1]
            .iter()
            .all(|s| s.kind == OnboardingStepKind::Lesson && s.goal != OnboardingGoal::None));
    }

    #[test]
    fn lesson_is_locked_until_tried() {
        let mut st = OnboardingState::default();
        st.restart();
        assert!(st.can_continue(), "kartu sambutan tanpa syarat");
        st.go_to(1);
        assert!(!st.can_continue());
        st.done = true;
        assert!(st.can_continue());
        st.go_to(2);
        assert!(!st.done, "pindah langkah mengunci lagi");
    }

    #[test]
    fn every_step_has_translations() {
        // Kunci teks dibentuk saat runtime, jadi tidak tertangkap pencarian
        // literal. `translate_lang` mengembalikan kuncinya bila tidak ada;
        // paritas en/id dijaga tes `test_key_parity` di ducad-i18n.
        let has = |key: String| {
            let mut args = ducad_i18n::fluent_bundle::FluentArgs::new();
            args.set("modkey", MOD_KEY);
            let text = ducad_i18n::translate_lang(Language::En, &key, Some(&args));
            // Pintasan harus memakai `{ $modkey }`, bukan "Cmd/Ctrl" generik.
            text != key && !text.contains("Cmd/Ctrl")
        };
        for step in STEPS {
            assert!(has(format!("onboard-{}-title", step.key)), "{}", step.key);
            assert!(has(format!("onboard-{}-body", step.key)), "{}", step.key);
            if step.kind == OnboardingStepKind::Lesson {
                assert!(has(format!("onboard-{}-try", step.key)), "{}", step.key);
            }
        }
    }

    #[test]
    fn target_rect_expires_after_one_frame() {
        let ctx = egui::Context::default();
        let run = |f: &mut dyn FnMut(&mut egui::Ui)| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| f(ui));
            out.textures_delta.clear();
        };
        let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::splat(30.0));
        run(&mut |ui| {
            Onboarding::publish_target(ui.ctx(), OnboardingTarget::ChatButton, rect);
            assert_eq!(
                Onboarding::target_rect(ui.ctx(), OnboardingTarget::ChatButton),
                Some(rect)
            );
        });
        for _ in 0..3 {
            run(&mut |_| {});
        }
        run(&mut |ui| {
            assert_eq!(
                Onboarding::target_rect(ui.ctx(), OnboardingTarget::ChatButton),
                None
            );
        });
    }
}
