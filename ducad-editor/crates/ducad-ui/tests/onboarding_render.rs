//! Tutorial selamat datang: kartu benar-benar tergambar dan tombol "Lanjut"
//! terkunci sampai aksi pelajaran dicoba.

use ducad_i18n::t;
use ducad_ui::{Onboarding, OnboardingEvent, OnboardingState, ONBOARDING_STEPS};

const SCREEN: egui::Vec2 = egui::vec2(1280.0, 860.0);

struct Frame {
    texts: Vec<(String, egui::Rect)>,
    event: Option<OnboardingEvent>,
}

fn frame(ctx: &egui::Context, state: &mut OnboardingState, events: Vec<egui::Event>) -> Frame {
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN);
    let input = egui::RawInput {
        screen_rect: Some(bounds),
        events,
        ..Default::default()
    };
    let mut event = None;
    let mut out = ctx.run_ui(input, |ui| {
        event = Onboarding::show(ui.ctx(), bounds, state);
    });
    out.textures_delta.clear();
    let mut texts = Vec::new();
    for cs in out.shapes {
        if let egui::epaint::Shape::Text(t) = cs.shape {
            let r = t.galley.rect.translate(t.pos.to_vec2());
            if cs.clip_rect.intersects(r) {
                texts.push((t.galley.text().to_string(), r));
            }
        }
    }
    Frame { texts, event }
}

fn settle(ctx: &egui::Context, state: &mut OnboardingState) -> Frame {
    let mut last = frame(ctx, state, Vec::new());
    for _ in 0..3 {
        last = frame(ctx, state, Vec::new());
    }
    last
}

/// Klik di tengah teks yang memuat `needle`; kembalikan event tutorialnya.
fn click(
    ctx: &egui::Context,
    state: &mut OnboardingState,
    needle: &str,
) -> Option<OnboardingEvent> {
    let shown = settle(ctx, state);
    let pos = shown
        .texts
        .iter()
        .find(|(s, _)| s.contains(needle))
        .map(|(_, r)| r.center())
        .unwrap_or_else(|| panic!("teks {needle:?} tidak tergambar"));
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(ctx, state, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, state, vec![button(true)]);
    let released = frame(ctx, state, vec![button(false)]);
    released.event
}

fn has(frame: &Frame, needle: &str) -> bool {
    frame.texts.iter().any(|(s, _)| s.contains(needle))
}

#[test]
fn closed_tutorial_draws_nothing() {
    let ctx = egui::Context::default();
    let mut st = OnboardingState::default();
    let shown = settle(&ctx, &mut st);
    assert!(shown.texts.is_empty());
    assert_eq!(shown.event, None);
}

#[test]
fn welcome_card_is_visible_and_starts_the_first_lesson() {
    let ctx = egui::Context::default();
    let mut st = OnboardingState::default();
    st.restart();
    let shown = settle(&ctx, &mut st);
    assert!(has(&shown, &t!("onboard-welcome-title")));

    let event = click(&ctx, &mut st, &t!("onboard-start"));
    assert_eq!(event, Some(OnboardingEvent::StepChanged));
    assert_eq!(st.step, 1);
    let lesson = settle(&ctx, &mut st);
    assert!(has(&lesson, &t!("onboard-mode-title")));
    assert!(has(&lesson, &t!("onboard-mode-try")));
}

#[test]
fn next_is_locked_until_the_action_was_tried() {
    let ctx = egui::Context::default();
    let mut st = OnboardingState {
        open: true,
        step: 1,
        done: false,
    };
    assert_eq!(click(&ctx, &mut st, &t!("onboard-next")), None);
    assert_eq!(st.step, 1, "belum dicoba: tidak boleh maju");

    st.done = true;
    let shown = settle(&ctx, &mut st);
    assert!(has(&shown, &t!("onboard-done")));
    assert_eq!(
        click(&ctx, &mut st, &t!("onboard-next")),
        Some(OnboardingEvent::StepChanged)
    );
    assert_eq!(st.step, 2);
    assert!(!st.done, "langkah baru terkunci lagi");
}

#[test]
fn skip_step_advances_and_close_dismisses() {
    let ctx = egui::Context::default();
    let mut st = OnboardingState {
        open: true,
        step: 3,
        done: false,
    };
    assert_eq!(
        click(&ctx, &mut st, &t!("onboard-skip-step")),
        Some(OnboardingEvent::StepChanged)
    );
    assert_eq!(st.step, 4);

    st.step = 0;
    assert_eq!(
        click(&ctx, &mut st, &t!("onboard-skip-all")),
        Some(OnboardingEvent::Dismissed)
    );
    assert!(!st.open);
}

#[test]
fn every_step_renders_and_the_last_one_finishes() {
    let ctx = egui::Context::default();
    let mut st = OnboardingState {
        open: true,
        ..Default::default()
    };
    for (i, step) in ONBOARDING_STEPS.iter().enumerate() {
        st.go_to(i);
        let shown = settle(&ctx, &mut st);
        let title = t!(&format!("onboard-{}-title", step.key));
        assert!(has(&shown, &title), "langkah {} tidak tergambar", step.key);
        // Kartu tidak boleh keluar dari layar.
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN);
        for (text, rect) in &shown.texts {
            assert!(
                screen.contains_rect(*rect),
                "{}: {text:?} keluar layar",
                step.key
            );
        }
    }
    assert_eq!(
        click(&ctx, &mut st, &t!("onboard-finish")),
        Some(OnboardingEvent::Finished)
    );
    assert!(!st.open);
}
