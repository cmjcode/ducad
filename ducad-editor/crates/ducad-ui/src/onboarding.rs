//! Tutorial selamat datang interaktif (gaya Shapr3D): satu proyek berkelanjutan,
//! sebuah rem cakram, dikerjakan dari sketsa pertama sampai siap produksi dalam
//! tiga bab. Tiap langkah memakai hasil langkah sebelumnya dan baru bisa
//! dilanjutkan setelah pengguna benar-benar mengerjakannya; deteksinya
//! dilakukan pemanggil (`ducad-app`) berdasarkan perubahan geometri, lalu
//! menyetel [`OnboardingState::done`].

use ducad_i18n::{t, Language};
use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_AUTO_AWESOME, ICON_BALANCE, ICON_CELEBRATION,
    ICON_CHECK_CIRCLE, ICON_CLOSE, ICON_CONTENT_CUT, ICON_DESCRIPTION, ICON_FOUNTAIN_PEN_TIP,
    ICON_HUB, ICON_LOOKS_3, ICON_LOOKS_ONE, ICON_LOOKS_TWO, ICON_RADIO_BUTTON_UNCHECKED, ICON_SAVE,
    ICON_SCHOOL, ICON_SCIENCE, ICON_SEARCH, ICON_TOUCH_APP, ICON_UPLOAD, ICON_VIEW_IN_AR,
    ICON_WAVING_HAND,
};

use crate::left_toolbar::ToolbarTool;
use crate::theme::{
    ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use crate::tool_guides::ToolGuides;

/// Perubahan geometri/keadaan yang harus terjadi supaya sebuah langkah lulus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingGoal {
    /// Tidak ada syarat (kartu sambutan, tur layar, kartu akhir bab).
    None,
    /// Lingkaran baru di bidang sketsa aktif.
    Circle,
    /// Body baru dari extrude profil.
    ExtrudeBody,
    /// Kamera diorbit/digeser/di-zoom oleh pengguna.
    Navigate,
    /// Sketsa pindah ke sisi solid DAN lingkaran baru digambar di sana.
    CircleOnFace,
    /// Volume body berkurang (potong tembus).
    Cut,
    /// Tepi solid di-chamfer.
    Chamfer,
    Save,
    /// Lingkaran bertambah minimal `n` (pattern sirkular).
    MoreCircles(usize),
    /// Slot baru (dua busur) di bidang sketsa.
    Slot,
    /// Busur bertambah minimal `n` (pattern slot).
    MoreArcs(usize),
    /// Tepi solid di-fillet.
    Fillet,
    Measure,
    /// Nama sebuah body diganti.
    Rename,
    /// Volume body bertambah (hub di-extrude di atas cakram).
    AddVolume,
    Shell,
    Hole,
    Section,
    /// Sebuah body diberi material mekanik.
    Material,
    /// Studi simulasi punya hasil.
    SimResult,
    ExportPdf,
    ExportStep,
    OpenChat,
}

/// Elemen UI yang disorot selama sebuah langkah.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OnboardingTarget {
    Tool(ToolbarTool),
    TopBar,
    LeftToolbar,
    ViewCube,
    ModeButton,
    PaletteButton,
    ChatButton,
    HelpButton,
    ItemsButton,
    ShareButton,
    ContextBar,
}

/// Tiga bab tutorial. Ketiganya mengerjakan rem cakram yang sama.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OnboardingChapter {
    Beginner,
    Intermediate,
    Advanced,
}

impl OnboardingChapter {
    pub const ALL: [OnboardingChapter; 3] = [
        OnboardingChapter::Beginner,
        OnboardingChapter::Intermediate,
        OnboardingChapter::Advanced,
    ];

    /// Bagian kunci i18n: `onboard-chapter-<key>`, `-desc`, `-start`.
    pub const fn key(self) -> &'static str {
        match self {
            OnboardingChapter::Beginner => "beginner",
            OnboardingChapter::Intermediate => "intermediate",
            OnboardingChapter::Advanced => "advanced",
        }
    }

    pub fn title(self) -> String {
        t!(&format!("onboard-chapter-{}", self.key()))
    }

    fn icon(self) -> &'static str {
        match self {
            OnboardingChapter::Beginner => ICON_LOOKS_ONE.codepoint,
            OnboardingChapter::Intermediate => ICON_LOOKS_TWO.codepoint,
            OnboardingChapter::Advanced => ICON_LOOKS_3.codepoint,
        }
    }

    /// Indeks langkah pertama bab ini di [`STEPS`].
    pub fn first_step(self) -> usize {
        STEPS
            .iter()
            .position(|s| s.chapter == self && s.kind != OnboardingStepKind::Welcome)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingStepKind {
    /// Kartu sambutan di tengah layar, berisi pilihan bab.
    Welcome,
    /// Tur layar: beberapa halaman, tiap halaman menyorot satu bagian UI.
    Overview,
    /// Pelajaran dengan animasi dan syarat geometri.
    Lesson,
    /// Kartu penutup bab; yang terakhir menutup tutorial.
    ChapterEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Demo {
    None,
    Tool(ToolbarTool),
    Select,
    Navigate,
    PushPull,
    Cut,
    EdgeFillet,
    SketchFace,
    Hole,
    Measure,
    Pattern,
    Shell,
    Palette,
    Chat,
    Save,
    /// Ikon besar berdenyut.
    Icon(&'static str),
}

/// Satu halaman tur layar: kunci i18n `onboard-<key>-title`/`-body`.
#[derive(Debug, Clone, Copy)]
pub struct OverviewPage {
    pub key: &'static str,
    pub target: Option<OnboardingTarget>,
    demo: Demo,
}

/// Satu langkah tutorial. Teksnya diambil dari kunci i18n
/// `onboard-<key>-title`, `onboard-<key>-body`, dan `onboard-<key>-try`.
#[derive(Debug, Clone, Copy)]
pub struct OnboardingStep {
    pub key: &'static str,
    pub chapter: OnboardingChapter,
    pub kind: OnboardingStepKind,
    pub goal: OnboardingGoal,
    pub target: Option<OnboardingTarget>,
    demo: Demo,
    /// Hanya untuk `Overview`.
    pub pages: &'static [OverviewPage],
}

const fn lesson(
    chapter: OnboardingChapter,
    key: &'static str,
    goal: OnboardingGoal,
    target: Option<OnboardingTarget>,
    demo: Demo,
) -> OnboardingStep {
    OnboardingStep {
        key,
        chapter,
        kind: OnboardingStepKind::Lesson,
        goal,
        target,
        demo,
        pages: &[],
    }
}

const fn chapter_end(chapter: OnboardingChapter, key: &'static str) -> OnboardingStep {
    OnboardingStep {
        key,
        chapter,
        kind: OnboardingStepKind::ChapterEnd,
        goal: OnboardingGoal::None,
        target: None,
        demo: Demo::None,
        pages: &[],
    }
}

const fn page(key: &'static str, target: Option<OnboardingTarget>, demo: Demo) -> OverviewPage {
    OverviewPage { key, target, demo }
}

const BEGINNER: OnboardingChapter = OnboardingChapter::Beginner;
const INTERMEDIATE: OnboardingChapter = OnboardingChapter::Intermediate;
const ADVANCED: OnboardingChapter = OnboardingChapter::Advanced;
const SELECT_TOOL: Option<OnboardingTarget> = Some(OnboardingTarget::Tool(ToolbarTool::Select));
const CIRCLE_TOOL: Option<OnboardingTarget> = Some(OnboardingTarget::Tool(ToolbarTool::Circle));
const CONTEXT_BAR: Option<OnboardingTarget> = Some(OnboardingTarget::ContextBar);
const PALETTE: Option<OnboardingTarget> = Some(OnboardingTarget::PaletteButton);
const SHARE: Option<OnboardingTarget> = Some(OnboardingTarget::ShareButton);

/// Langkah 0 Bab 1: tur layar.
pub const OVERVIEW_PAGES: &[OverviewPage] = &[
    page("ov-topbar", Some(OnboardingTarget::TopBar), Demo::None),
    page(
        "ov-toolbar",
        Some(OnboardingTarget::LeftToolbar),
        Demo::None,
    ),
    page("ov-mode", Some(OnboardingTarget::ModeButton), Demo::None),
    page(
        "ov-viewcube",
        Some(OnboardingTarget::ViewCube),
        Demo::Navigate,
    ),
    page("ov-context", None, Demo::Select),
    page("ov-palette", PALETTE, Demo::Palette),
    page("ov-help", Some(OnboardingTarget::HelpButton), Demo::Chat),
];

/// Proyek tutorial: rem cakram Ø240 mm.
///
/// - Pemula: piringan pejal dengan lubang poros dan chamfer.
/// - Menengah: lima lubang baut dan dua belas slot ventilasi.
/// - Mahir: hub berongga dengan lubang ulir, lalu verifikasi dan ekspor.
///
/// Tiap langkah memakai hasil langkah sebelumnya; urutannya tidak boleh diacak.
pub const STEPS: &[OnboardingStep] = &[
    OnboardingStep {
        key: "welcome",
        chapter: BEGINNER,
        kind: OnboardingStepKind::Welcome,
        goal: OnboardingGoal::None,
        target: None,
        demo: Demo::None,
        pages: &[],
    },
    // ---- Bab 1: Pemula — piringan dasar ----
    OnboardingStep {
        key: "overview",
        chapter: BEGINNER,
        kind: OnboardingStepKind::Overview,
        goal: OnboardingGoal::None,
        target: None,
        demo: Demo::None,
        pages: OVERVIEW_PAGES,
    },
    lesson(
        BEGINNER,
        "disc-circle",
        OnboardingGoal::Circle,
        CIRCLE_TOOL,
        Demo::Tool(ToolbarTool::Circle),
    ),
    lesson(
        BEGINNER,
        "disc-extrude",
        OnboardingGoal::ExtrudeBody,
        SELECT_TOOL,
        Demo::Tool(ToolbarTool::Extrude),
    ),
    lesson(
        BEGINNER,
        "navigate",
        OnboardingGoal::Navigate,
        Some(OnboardingTarget::ViewCube),
        Demo::Navigate,
    ),
    lesson(
        BEGINNER,
        "bore-circle",
        OnboardingGoal::CircleOnFace,
        CONTEXT_BAR,
        Demo::SketchFace,
    ),
    lesson(
        BEGINNER,
        "bore-cut",
        OnboardingGoal::Cut,
        SELECT_TOOL,
        Demo::Cut,
    ),
    lesson(
        BEGINNER,
        "rim-chamfer",
        OnboardingGoal::Chamfer,
        SELECT_TOOL,
        Demo::EdgeFillet,
    ),
    lesson(BEGINNER, "save", OnboardingGoal::Save, None, Demo::Save),
    chapter_end(BEGINNER, "end-beginner"),
    // ---- Bab 2: Menengah — lubang baut dan ventilasi ----
    lesson(
        INTERMEDIATE,
        "bolt-circle",
        OnboardingGoal::CircleOnFace,
        CONTEXT_BAR,
        Demo::SketchFace,
    ),
    lesson(
        INTERMEDIATE,
        "bolt-pattern",
        OnboardingGoal::MoreCircles(4),
        CONTEXT_BAR,
        Demo::Pattern,
    ),
    lesson(
        INTERMEDIATE,
        "bolt-cut",
        OnboardingGoal::Cut,
        SELECT_TOOL,
        Demo::Cut,
    ),
    lesson(
        INTERMEDIATE,
        "vent-slot",
        OnboardingGoal::Slot,
        Some(OnboardingTarget::Tool(ToolbarTool::Slot)),
        Demo::Tool(ToolbarTool::Slot),
    ),
    lesson(
        INTERMEDIATE,
        "vent-pattern",
        OnboardingGoal::MoreArcs(8),
        CONTEXT_BAR,
        Demo::Pattern,
    ),
    lesson(
        INTERMEDIATE,
        "vent-cut",
        OnboardingGoal::Cut,
        SELECT_TOOL,
        Demo::Cut,
    ),
    lesson(
        INTERMEDIATE,
        "bore-fillet",
        OnboardingGoal::Fillet,
        SELECT_TOOL,
        Demo::EdgeFillet,
    ),
    lesson(
        INTERMEDIATE,
        "measure",
        OnboardingGoal::Measure,
        PALETTE,
        Demo::Measure,
    ),
    lesson(
        INTERMEDIATE,
        "rename",
        OnboardingGoal::Rename,
        Some(OnboardingTarget::ItemsButton),
        Demo::Icon(ICON_VIEW_IN_AR.codepoint),
    ),
    chapter_end(INTERMEDIATE, "end-intermediate"),
    // ---- Bab 3: Mahir — hub, verifikasi, produksi ----
    lesson(
        ADVANCED,
        "hub-extrude",
        OnboardingGoal::AddVolume,
        CONTEXT_BAR,
        Demo::PushPull,
    ),
    lesson(
        ADVANCED,
        "hub-shell",
        OnboardingGoal::Shell,
        CONTEXT_BAR,
        Demo::Shell,
    ),
    lesson(
        ADVANCED,
        "hub-hole",
        OnboardingGoal::Hole,
        CONTEXT_BAR,
        Demo::Hole,
    ),
    lesson(
        ADVANCED,
        "section",
        OnboardingGoal::Section,
        Some(OnboardingTarget::Tool(ToolbarTool::SectionView)),
        Demo::Icon(ICON_CONTENT_CUT.codepoint),
    ),
    lesson(
        ADVANCED,
        "material",
        OnboardingGoal::Material,
        PALETTE,
        Demo::Icon(ICON_BALANCE.codepoint),
    ),
    lesson(
        ADVANCED,
        "sim",
        OnboardingGoal::SimResult,
        PALETTE,
        Demo::Icon(ICON_SCIENCE.codepoint),
    ),
    lesson(
        ADVANCED,
        "drawing",
        OnboardingGoal::ExportPdf,
        SHARE,
        Demo::Icon(ICON_DESCRIPTION.codepoint),
    ),
    lesson(
        ADVANCED,
        "step",
        OnboardingGoal::ExportStep,
        SHARE,
        Demo::Icon(ICON_UPLOAD.codepoint),
    ),
    // Chat AI paling akhir: butuh penyiapan agent.
    lesson(
        ADVANCED,
        "chat",
        OnboardingGoal::OpenChat,
        Some(OnboardingTarget::ChatButton),
        Demo::Chat,
    ),
    chapter_end(ADVANCED, "tour"),
];

/// Posisi `(urutan, jumlah)` langkah `step` di dalam babnya; tur layar = 0.
fn chapter_progress(step: usize) -> (usize, usize) {
    let step = step.min(STEPS.len() - 1);
    let chapter = STEPS[step].chapter;
    let lessons =
        |s: &&OnboardingStep| s.chapter == chapter && s.kind == OnboardingStepKind::Lesson;
    let total = STEPS.iter().filter(lessons).count();
    let position = STEPS[..=step].iter().filter(lessons).count();
    (position, total)
}

/// State tutorial. `done` disetel pemanggil saat syarat langkah terpenuhi.
#[derive(Debug, Clone, Default)]
pub struct OnboardingState {
    pub open: bool,
    pub step: usize,
    pub done: bool,
    /// Halaman tur layar yang sedang tampil.
    pub page: usize,
}

impl OnboardingState {
    pub fn current(&self) -> &'static OnboardingStep {
        &STEPS[self.step.min(STEPS.len() - 1)]
    }

    /// Langkah boleh dilanjutkan: tanpa syarat, atau syaratnya sudah dikerjakan.
    pub fn can_continue(&self) -> bool {
        self.done || self.current().goal == OnboardingGoal::None
    }

    /// Mulai dari kartu sambutan.
    pub fn restart(&mut self) {
        self.open = true;
        self.step = 0;
        self.done = false;
        self.page = 0;
    }

    /// Pindah ke `step` (dijepit ke rentang langkah) dan kunci lagi syaratnya.
    pub fn go_to(&mut self, step: usize) {
        self.step = step.min(STEPS.len() - 1);
        self.done = false;
        self.page = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingEvent {
    /// Pindah ke langkah lain (maju, mundur, atau melewati satu langkah).
    StepChanged,
    /// Tutorial ditutup sebelum selesai.
    Dismissed,
    /// Tutorial diselesaikan (akhir bab mana pun).
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

/// Perintah yang diketik pada animasi palet. `ducad-app` memakai konstanta
/// ini sebagai label perintahnya, jadi demo dan palet selalu sama.
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
    /// frame supaya tutorial tahu letak bagian yang harus disorot.
    pub fn publish_target(ctx: &egui::Context, target: OnboardingTarget, rect: Rect) {
        let frame = ctx.cumulative_frame_nr();
        ctx.data_mut(|d| d.insert_temp(Self::target_id(target), (frame, rect)));
    }

    /// Rect sasaran bila dirender frame ini atau frame sebelumnya (tutorial
    /// bisa digambar sebelum maupun sesudah pemilik tombolnya).
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
            OnboardingStepKind::ChapterEnd => Self::show_chapter_end(ctx, state, &step),
            OnboardingStepKind::Overview => {
                let page = step.pages[state.page.min(step.pages.len() - 1)];
                if let Some(rect) = page.target.and_then(|t| Self::target_rect(ctx, t)) {
                    paint_spotlight(ctx, rect);
                }
                Self::show_card(ctx, bounds, state, &step, Some(page))
            }
            OnboardingStepKind::Lesson => {
                if !state.done {
                    if let Some(rect) = step.target.and_then(|t| Self::target_rect(ctx, t)) {
                        paint_spotlight(ctx, rect);
                    }
                }
                Self::show_card(ctx, bounds, state, &step, None)
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
                    for chapter in OnboardingChapter::ALL {
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(RichText::new(chapter.icon()).size(20.0).color(ACCENT_BLUE));
                            let text_w = (ui.available_width() - 96.0).max(120.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(text_w, 0.0),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.set_width(text_w);
                                    ui.label(
                                        RichText::new(chapter.title())
                                            .size(13.0)
                                            .strong()
                                            .color(TEXT_PRIMARY),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(t!(&format!(
                                                "onboard-chapter-{}-desc",
                                                chapter.key()
                                            )))
                                            .size(11.5)
                                            .color(TEXT_SECONDARY),
                                        )
                                        .wrap(),
                                    );
                                },
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let label =
                                        t!(&format!("onboard-chapter-{}-start", chapter.key()));
                                    let button = if chapter == OnboardingChapter::Beginner {
                                        primary_button(&label)
                                    } else {
                                        secondary_button(&label)
                                    };
                                    if ui.add(button).clicked() {
                                        state.go_to(chapter.first_step());
                                        event = Some(OnboardingEvent::StepChanged);
                                    }
                                },
                            );
                        });
                        ui.add_space(6.0);
                    }
                    ui.add_space(6.0);
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
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(secondary_button(&t!("onboard-skip-all"))).clicked() {
                                state.open = false;
                                event = Some(OnboardingEvent::Dismissed);
                            }
                        });
                    });
                });
            });
        event
    }

    fn show_chapter_end(
        ctx: &egui::Context,
        state: &mut OnboardingState,
        step: &OnboardingStep,
    ) -> Option<OnboardingEvent> {
        let mut event = None;
        let is_last = state.step >= STEPS.len() - 1;
        egui::Modal::new(egui::Id::new(("ducad-onboarding-chapter-end", step.key)))
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
                            RichText::new(t!(&format!("onboard-{}-title", step.key)))
                                .size(18.0)
                                .strong()
                                .color(TEXT_PRIMARY),
                        );
                        ui.add(
                            egui::Label::new(
                                RichText::new(t!(&format!("onboard-{}-body", step.key)))
                                    .size(12.0)
                                    .color(TEXT_SECONDARY),
                            )
                            .wrap(),
                        );
                    });
                    ui.add_space(10.0);
                    if is_last {
                        bullet_list(
                            ui,
                            &[
                                (ICON_VIEW_IN_AR.codepoint, "onboard-tour-shapes"),
                                (ICON_FOUNTAIN_PEN_TIP.codepoint, "onboard-tour-vector"),
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
                    }
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
                            if is_last {
                                if ui.add(primary_button(&t!("onboard-finish"))).clicked() {
                                    state.open = false;
                                    event = Some(OnboardingEvent::Finished);
                                }
                                return;
                            }
                            if ui
                                .add(primary_button(&format!(
                                    "{}  {}",
                                    t!("onboard-next-chapter"),
                                    ICON_ARROW_FORWARD.codepoint
                                )))
                                .clicked()
                            {
                                state.go_to(state.step + 1);
                                event = Some(OnboardingEvent::StepChanged);
                            }
                            if ui.add(secondary_button(&t!("onboard-stop-here"))).clicked() {
                                state.open = false;
                                event = Some(OnboardingEvent::Finished);
                            }
                        });
                    });
                });
            });
        event
    }

    /// Kartu pelajaran (atau satu halaman tur layar bila `page` ada).
    fn show_card(
        ctx: &egui::Context,
        bounds: Rect,
        state: &mut OnboardingState,
        step: &OnboardingStep,
        page: Option<OverviewPage>,
    ) -> Option<OnboardingEvent> {
        let mut event = None;
        let time = ctx.input(|i| i.time);
        let (text_key, demo) = match page {
            Some(p) => (p.key, p.demo),
            None => (step.key, step.demo),
        };
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
                    let (position, lessons) = chapter_progress(state.step);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(ICON_SCHOOL.codepoint)
                                .size(15.0)
                                .color(ACCENT_BLUE),
                        );
                        let label = match page {
                            Some(_) => t!(
                                "onboard-overview-progress",
                                chapter = step.chapter.title(),
                                current = (state.page + 1) as i64,
                                total = step.pages.len() as i64
                            ),
                            None => t!(
                                "onboard-progress",
                                chapter = step.chapter.title(),
                                current = position as i64,
                                total = lessons as i64
                            ),
                        };
                        ui.label(RichText::new(label).size(11.0).color(TEXT_SECONDARY));
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
                    paint_progress(ui, position as f32 / lessons.max(1) as f32);
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(t!(&format!("onboard-{text_key}-title")))
                            .size(15.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.add_space(4.0);
                    let (demo_rect, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), DEMO_H),
                        egui::Sense::hover(),
                    );
                    paint_demo(ui, demo_rect, demo, time);
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(t!(
                                &format!("onboard-{text_key}-body"),
                                modkey = MOD_KEY
                            ))
                            .size(12.0)
                            .color(TEXT_SECONDARY),
                        )
                        .wrap(),
                    );
                    ui.add_space(6.0);
                    if page.is_none() {
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
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add(secondary_button(&format!(
                                "{}  {}",
                                ICON_ARROW_BACK.codepoint,
                                t!("onboard-back")
                            )))
                            .clicked()
                        {
                            if page.is_some() && state.page > 0 {
                                state.page -= 1;
                            } else {
                                state.go_to(state.step.saturating_sub(1));
                                event = Some(OnboardingEvent::StepChanged);
                            }
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
                                if page.is_some() && state.page + 1 < step.pages.len() {
                                    state.page += 1;
                                } else {
                                    state.go_to(state.step + 1);
                                    event = Some(OnboardingEvent::StepChanged);
                                }
                            }
                            if page.is_none()
                                && !state.done
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
        Demo::Palette => demo_palette(&painter, rect, time),
        Demo::Chat => demo_chat(&painter, rect, time),
        Demo::Save => demo_save(&painter, rect, time),
        Demo::SketchFace => demo_sketch_face(&painter, rect, time),
        Demo::Cut => demo_cut(&painter, rect, time),
        Demo::EdgeFillet => demo_edge_fillet(&painter, rect, time),
        Demo::Pattern => demo_pattern(&painter, rect, time),
        Demo::Shell => demo_shell(&painter, rect, time),
        Demo::Hole => demo_hole(&painter, rect, time),
        Demo::Measure => demo_measure(&painter, rect, time),
        Demo::Icon(icon) => demo_icon(&painter, rect, icon, time),
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

/// Sisi atas balok diklik lalu berubah menjadi bidang sketsa.
fn demo_sketch_face(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 3.4);
    let center = Pos2::new(rect.center().x, rect.bottom() - 30.0);
    let (size, h, yaw) = (Vec2::new(44.0, 30.0), 22.0, 0.75);
    paint_box(painter, center, yaw, size, h, ACCENT_BLUE);
    let top = iso(center, yaw, 0.0, 0.0, h);
    if p > 0.35 {
        let face: Vec<Pos2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .iter()
            .map(|(x, y)| iso(center, yaw, x * size.x, y * size.y, h))
            .collect();
        painter.add(egui::Shape::convex_polygon(
            face,
            ACCENT_ORANGE.gamma_multiply(0.35),
            Stroke::new(1.5, ACCENT_ORANGE),
        ));
    }
    if p > 0.6 {
        // Profil baru digambar di atas sisi itu.
        let grow = smooth((p - 0.6) / 0.25);
        let profile: Vec<Pos2> = [(-0.8, -0.7), (-0.2, -0.7), (-0.2, 0.7), (-0.8, 0.7)]
            .iter()
            .map(|(x, y)| iso(center, yaw, x * size.x, y * size.y * grow, h))
            .collect();
        painter.add(egui::Shape::closed_line(
            profile,
            Stroke::new(1.5, ACCENT_GREEN),
        ));
    }
    ToolGuides::paint_cursor(
        painter,
        Pos2::new(top.x + 6.0, top.y),
        p > 0.25 && p < 0.5,
        time,
    );
}

/// Lubang muncul di sisi atas balok.
fn demo_hole(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 3.2);
    let center = Pos2::new(rect.center().x, rect.bottom() - 30.0);
    let (size, h, yaw) = (Vec2::new(44.0, 30.0), 22.0, 0.75);
    paint_box(painter, center, yaw, size, h, ACCENT_BLUE);
    let top = iso(center, yaw, 0.0, 0.0, h);
    let grow = smooth((p - 0.3) / 0.3);
    if grow > 0.0 {
        let r = 4.0 + grow * 10.0;
        painter.add(egui::Shape::ellipse_filled(
            top,
            Vec2::new(r, r * 0.5),
            Color32::from_rgba_premultiplied(4, 5, 8, 240),
        ));
        painter.add(egui::Shape::ellipse_stroke(
            top,
            Vec2::new(r, r * 0.5),
            Stroke::new(1.5, ACCENT_ORANGE),
        ));
    }
    painter.text(
        Pos2::new(rect.right() - 12.0, rect.top() + 10.0),
        Align2::RIGHT_TOP,
        "M5",
        FontId::proportional(11.0),
        TEXT_PRIMARY,
    );
    ToolGuides::paint_cursor(
        painter,
        Pos2::new(top.x + 3.0, top.y + 1.0),
        p > 0.2 && p < 0.45,
        time,
    );
}

/// Dua titik diklik lalu garis ukur muncul di antaranya.
fn demo_measure(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 3.4);
    let shape = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.center().y + 12.0),
        Vec2::new(150.0, 56.0),
    );
    painter.rect_stroke(
        shape,
        CornerRadius::ZERO,
        Stroke::new(1.5, ACCENT_BLUE),
        StrokeKind::Inside,
    );
    let (a, b) = (shape.left_top(), shape.right_top());
    let travel = smooth((p - 0.2) / 0.4);
    let cursor = a + (b - a) * travel;
    if p > 0.15 {
        painter.circle_filled(a, 3.5, ACCENT_ORANGE);
    }
    if travel > 0.0 {
        let lift = Vec2::new(0.0, -14.0);
        painter.line_segment([a + lift, cursor + lift], Stroke::new(1.5, ACCENT_ORANGE));
        painter.line_segment([a, a + lift], Stroke::new(1.0, TEXT_MUTED));
        painter.line_segment([cursor, cursor + lift], Stroke::new(1.0, TEXT_MUTED));
    }
    if p > 0.62 {
        painter.circle_filled(b, 3.5, ACCENT_ORANGE);
        painter.text(
            Pos2::new(shape.center().x, a.y - 24.0),
            Align2::CENTER_CENTER,
            "80 mm",
            FontId::proportional(11.5),
            TEXT_PRIMARY,
        );
    }
    ToolGuides::paint_cursor(
        painter,
        cursor,
        (p > 0.12 && p < 0.22) || (p > 0.58 && p < 0.7),
        time,
    );
}

/// Ikon panel yang dibuka pelajaran ini, dengan cincin berdenyut.
fn demo_icon(painter: &egui::Painter, rect: Rect, icon: &str, time: f64) {
    let pulse = ((time * 2.4).sin() * 0.5 + 0.5) as f32;
    let center = rect.center();
    painter.circle_stroke(
        center,
        34.0 + pulse * 8.0,
        Stroke::new(1.5, ACCENT_BLUE.gamma_multiply(0.6 - pulse * 0.45)),
    );
    painter.circle_filled(center, 30.0, ACCENT_BLUE.gamma_multiply(0.16));
    painter.text(
        center,
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(34.0),
        ACCENT_BLUE,
    );
}

/// Profil lingkaran di sisi atas ditekan ke bawah sampai menembus balok.
fn demo_cut(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 3.4);
    let center = Pos2::new(rect.center().x, rect.bottom() - 26.0);
    let (size, h, yaw) = (Vec2::new(46.0, 30.0), 26.0, 0.75);
    paint_box(painter, center, yaw, size, h, ACCENT_BLUE);
    let depth = smooth((p - 0.3) / 0.45) * h;
    let outline = |z: f32| -> Vec<Pos2> {
        [(-0.6, -0.25), (0.6, -0.25), (0.6, 0.25), (-0.6, 0.25)]
            .iter()
            .map(|(x, y)| iso(center, yaw, x * size.x, y * size.y, z))
            .collect()
    };
    let (top, bottom) = (outline(h), outline(h - depth));
    for (a, b) in top.iter().zip(&bottom) {
        painter.line_segment([*a, *b], Stroke::new(1.0, ACCENT_ORANGE));
    }
    painter.add(egui::Shape::convex_polygon(
        top,
        Color32::from_rgba_premultiplied(4, 5, 8, 230),
        Stroke::new(1.5, ACCENT_ORANGE),
    ));
    painter.add(egui::Shape::closed_line(
        bottom,
        Stroke::new(1.0, ACCENT_ORANGE),
    ));
    let tip = iso(center, yaw, 0.0, 0.0, h - depth);
    let tail = iso(center, yaw, 0.0, 0.0, h + 22.0 - depth);
    painter.line_segment([tail, tip], Stroke::new(2.0, ACCENT_ORANGE));
    painter.text(
        Pos2::new(rect.right() - 12.0, rect.top() + 10.0),
        Align2::RIGHT_TOP,
        format!("-{:.0} mm", depth / h * 6.0),
        FontId::proportional(11.0),
        TEXT_PRIMARY,
    );
    ToolGuides::paint_cursor(painter, tail, p > 0.25 && p < 0.8, time);
}

/// Tepi atas balok 3D diklik lalu membulat (fillet) atau terpotong (chamfer).
fn demo_edge_fillet(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 3.2);
    let center = Pos2::new(rect.center().x, rect.bottom() - 28.0);
    let (size, h, yaw) = (Vec2::new(44.0, 30.0), 30.0, 0.75);
    paint_box(painter, center, yaw, size, h, ACCENT_BLUE);
    let radius = smooth((p - 0.3) / 0.4) * 12.0;
    let a = iso(center, yaw, size.x, size.y, h);
    let b = iso(center, yaw, -size.x, size.y, h);
    let drop = Vec2::new(0.0, radius);
    painter.line_segment(
        [a + drop * 0.5, b + drop * 0.5],
        Stroke::new(2.5, ACCENT_ORANGE),
    );
    if radius > 0.5 {
        painter.line_segment([a + drop, b + drop], Stroke::new(1.0, ACCENT_ORANGE));
    }
    painter.text(
        Pos2::new(rect.right() - 12.0, rect.top() + 10.0),
        Align2::RIGHT_TOP,
        format!("{:.0} mm", radius / 6.0),
        FontId::proportional(11.0),
        TEXT_PRIMARY,
    );
    let mid = a + (b - a) * 0.5 + drop * 0.5;
    ToolGuides::paint_cursor(painter, mid, p > 0.2 && p < 0.75, time);
}

/// Satu lingkaran disalin melingkar menjadi lima.
fn demo_pattern(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 3.6);
    let center = Pos2::new(rect.center().x, rect.center().y + 6.0);
    painter.circle_stroke(center, 54.0, Stroke::new(1.0, TEXT_MUTED));
    painter.circle_stroke(center, 14.0, Stroke::new(1.0, TEXT_MUTED));
    let shown = 1 + (smooth((p - 0.3) / 0.5) * 4.0).round() as usize;
    for i in 0..shown.min(5) {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 5.0;
        let c = center + Vec2::new(a.cos(), a.sin()) * 36.0;
        let color = if i == 0 { ACCENT_ORANGE } else { ACCENT_BLUE };
        painter.circle_filled(c, 6.0, color.gamma_multiply(0.25));
        painter.circle_stroke(c, 6.0, Stroke::new(1.5, color));
    }
    painter.text(
        Pos2::new(rect.right() - 12.0, rect.top() + 10.0),
        Align2::RIGHT_TOP,
        format!("{}x", shown.min(5)),
        FontId::proportional(11.0),
        TEXT_PRIMARY,
    );
    ToolGuides::paint_cursor(painter, center + Vec2::new(4.0, -32.0), p < 0.25, time);
}

/// Sisi atas silinder diklik lalu rongga berdinding tipis muncul.
fn demo_shell(painter: &egui::Painter, rect: Rect, time: f64) {
    let p = phase(time, 3.4);
    let center = Pos2::new(rect.center().x, rect.bottom() - 26.0);
    let (rx, ry, h) = (46.0, 20.0, 46.0);
    let top = Pos2::new(center.x, center.y - h);
    painter.line_segment(
        [center + Vec2::new(-rx, 0.0), top + Vec2::new(-rx, 0.0)],
        Stroke::new(1.5, ACCENT_BLUE),
    );
    painter.line_segment(
        [center + Vec2::new(rx, 0.0), top + Vec2::new(rx, 0.0)],
        Stroke::new(1.5, ACCENT_BLUE),
    );
    painter.add(egui::Shape::ellipse_stroke(
        center,
        Vec2::new(rx, ry),
        Stroke::new(1.5, ACCENT_BLUE),
    ));
    painter.add(egui::Shape::ellipse_filled(
        top,
        Vec2::new(rx, ry),
        ACCENT_BLUE.gamma_multiply(0.18),
    ));
    painter.add(egui::Shape::ellipse_stroke(
        top,
        Vec2::new(rx, ry),
        Stroke::new(1.5, ACCENT_BLUE),
    ));
    let open = smooth((p - 0.35) / 0.4);
    if open > 0.0 {
        let inner = Vec2::new((rx - 6.0) * open, (ry - 3.0) * open);
        painter.add(egui::Shape::ellipse_filled(
            top,
            inner,
            Color32::from_rgba_premultiplied(4, 5, 8, 235),
        ));
        painter.add(egui::Shape::ellipse_stroke(
            top,
            inner,
            Stroke::new(1.5, ACCENT_ORANGE),
        ));
    }
    painter.text(
        Pos2::new(rect.right() - 12.0, rect.top() + 10.0),
        Align2::RIGHT_TOP,
        format!("t = {:.0} mm", 4.0),
        FontId::proportional(11.0),
        TEXT_PRIMARY,
    );
    ToolGuides::paint_cursor(painter, top + Vec2::new(6.0, 2.0), p > 0.2 && p < 0.4, time);
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
        &[
            "Export SVG Vektor 2D",
            "Extend (Perpanjang Garis)",
            PALETTE_DEMO_COMMAND,
        ]
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
    fn steps_form_three_chapters_each_closed_by_an_end_card() {
        assert_eq!(STEPS[0].kind, OnboardingStepKind::Welcome);
        assert_eq!(
            STEPS[1].kind,
            OnboardingStepKind::Overview,
            "langkah 0 = tur layar"
        );
        assert!(!STEPS[1].pages.is_empty());
        let mut order = Vec::new();
        for chapter in OnboardingChapter::ALL {
            let steps: Vec<_> = STEPS[1..].iter().filter(|s| s.chapter == chapter).collect();
            let (end, lessons) = steps.split_last().expect("bab punya langkah");
            assert_eq!(end.kind, OnboardingStepKind::ChapterEnd);
            assert!(
                lessons
                    .iter()
                    .filter(|s| s.kind == OnboardingStepKind::Lesson)
                    .count()
                    >= 5,
                "bab {} terlalu pendek",
                chapter.key()
            );
            assert!(lessons
                .iter()
                .filter(|s| s.kind == OnboardingStepKind::Lesson)
                .all(|s| s.goal != OnboardingGoal::None));
            order.push(chapter.first_step());
        }
        assert!(order.windows(2).all(|w| w[0] < w[1]), "bab berurutan");
        assert_eq!(STEPS[STEPS.len() - 1].kind, OnboardingStepKind::ChapterEnd);
        let mut keys: Vec<_> = STEPS.iter().map(|s| s.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), STEPS.len(), "kunci langkah unik");
    }

    #[test]
    fn progress_counts_within_the_chapter() {
        let first = OnboardingChapter::Intermediate.first_step();
        let (position, total) = chapter_progress(first);
        assert_eq!(position, 1);
        assert_eq!(chapter_progress(first + total - 1), (total, total));
        // Tur layar belum dihitung sebagai pelajaran.
        assert_eq!(chapter_progress(1).0, 0);
    }

    #[test]
    fn lesson_is_locked_until_tried() {
        let mut st = OnboardingState::default();
        st.restart();
        assert!(st.can_continue(), "kartu sambutan tanpa syarat");
        st.go_to(2);
        assert!(!st.can_continue());
        st.done = true;
        assert!(st.can_continue());
        st.go_to(3);
        assert!(!st.done, "pindah langkah mengunci lagi");
        assert_eq!(st.page, 0);
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
            if step.kind != OnboardingStepKind::Overview {
                assert!(has(format!("onboard-{}-title", step.key)), "{}", step.key);
                assert!(has(format!("onboard-{}-body", step.key)), "{}", step.key);
            }
            if step.kind == OnboardingStepKind::Lesson {
                assert!(has(format!("onboard-{}-try", step.key)), "{}", step.key);
            }
            for page in step.pages {
                assert!(has(format!("onboard-{}-title", page.key)), "{}", page.key);
                assert!(has(format!("onboard-{}-body", page.key)), "{}", page.key);
            }
        }
        for chapter in OnboardingChapter::ALL {
            for suffix in ["", "-desc", "-start"] {
                assert!(has(format!("onboard-chapter-{}{suffix}", chapter.key())));
            }
        }
    }

    #[test]
    fn target_rect_expires_after_one_frame() {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::splat(30.0));
        let run = |f: &mut dyn FnMut(&mut egui::Ui)| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| f(ui));
            out.textures_delta.clear();
        };
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
