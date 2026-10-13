//! Panel Review & Rilis: benar-benar tergambar, tombol "Setujui" terkunci
//! selama ada penghalang atau nama belum diisi, dan klik menghasilkan event.

use ducad_i18n::t;
use ducad_ui::{
    ReviewItemUi, ReviewPanel, ReviewPanelData, ReviewPanelEvent, ReviewRedlineUi,
    ReviewRequirementUi, ReviewRevisionUi, ReviewStage, ReviewTrace,
};

const SCREEN: egui::Vec2 = egui::vec2(1280.0, 900.0);

struct Frame {
    texts: Vec<(String, egui::Rect)>,
    event: Option<ReviewPanelEvent>,
}

fn frame(
    ctx: &egui::Context,
    panel: &mut ReviewPanel,
    data: &ReviewPanelData,
    events: Vec<egui::Event>,
) -> Frame {
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN);
    let input = egui::RawInput {
        screen_rect: Some(bounds),
        events,
        ..Default::default()
    };
    let mut event = None;
    let mut out = ctx.run_ui(input, |ui| {
        event = egui::Area::new(egui::Id::new("review-test"))
            .fixed_pos(egui::pos2(20.0, 20.0))
            .show(ui.ctx(), |ui| panel.show(ui, data))
            .inner;
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

fn settle(ctx: &egui::Context, panel: &mut ReviewPanel, data: &ReviewPanelData) -> Frame {
    let mut last = frame(ctx, panel, data, Vec::new());
    for _ in 0..3 {
        last = frame(ctx, panel, data, Vec::new());
    }
    last
}

fn click(
    ctx: &egui::Context,
    panel: &mut ReviewPanel,
    data: &ReviewPanelData,
    needle: &str,
) -> Option<ReviewPanelEvent> {
    let shown = settle(ctx, panel, data);
    let pos = shown
        .texts
        .iter()
        .find(|(s, _)| s == needle)
        .map(|(_, r)| r.center())
        .unwrap_or_else(|| {
            panic!(
                "teks {needle:?} tidak tergambar: {:?}",
                shown.texts.iter().map(|t| &t.0).collect::<Vec<_>>()
            )
        });
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(ctx, panel, data, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, panel, data, vec![button(true)]);
    frame(ctx, panel, data, vec![button(false)]).event
}

fn data() -> ReviewPanelData {
    ReviewPanelData {
        has_brief: true,
        title: "Sensor bracket".into(),
        function: "Holds a sensor".into(),
        process: "cnc".into(),
        requirements: vec![
            ReviewRequirementUi {
                id: "R1".into(),
                text: "Mass below 80 g".into(),
                status: ReviewTrace::Verified,
                checks: "mass".into(),
            },
            ReviewRequirementUi {
                id: "R2".into(),
                text: "Millable".into(),
                status: ReviewTrace::Unverified,
                checks: String::new(),
            },
        ],
        stage: ReviewStage::InReview,
        revision: "B".into(),
        checklist: vec![ReviewItemUi {
            label: "Checks".into(),
            ok: true,
            detail: "10 checks".into(),
        }],
        redlines: vec![ReviewRedlineUi {
            sheet: "sheet1".into(),
            id: "RL1".into(),
            text: "Make it 8 thick".into(),
            author: "budi".into(),
        }],
        history: vec![ReviewRevisionUi {
            revision: "A".into(),
            date: "2026-10-13".into(),
            by: "ana".into(),
            description: "Initial release".into(),
        }],
        ..ReviewPanelData::default()
    }
}

#[test]
fn panel_draws_every_section() {
    let ctx = egui::Context::default();
    ducad_ui::theme::apply(&ctx, ducad_ui::theme::ThemeMode::Dark);
    let mut panel = ReviewPanel::default();
    let shown = settle(&ctx, &mut panel, &data());
    let has = |needle: &str| shown.texts.iter().any(|(s, _)| s.contains(needle));
    for needle in [
        t!("review-title"),
        t!("review-brief"),
        t!("review-requirements"),
        t!("review-checklist"),
        t!("review-release"),
        t!("review-submit"),
        t!("review-approve"),
        t!("review-build-package"),
        t!("review-history"),
        "R1  Mass below 80 g".to_string(),
        "RL1".to_string(),
        "Make it 8 thick".to_string(),
        "A  2026-10-13  ana  Initial release".to_string(),
    ] {
        assert!(has(&needle), "{needle:?} tidak tergambar");
    }
    // Di layar pendek bagian Rilis tetap terlihat (disematkan di bawah gulir).
    let short = egui::vec2(1180.0, 620.0);
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, short);
    let mut seen = Vec::new();
    for _ in 0..4 {
        let out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                ..Default::default()
            },
            |ui| {
                egui::Area::new(egui::Id::new("review-test"))
                    .fixed_pos(egui::pos2(20.0, 20.0))
                    .show(ui.ctx(), |ui| panel.show(ui, &data()));
            },
        );
        seen = out
            .shapes
            .into_iter()
            .filter_map(|cs| match cs.shape {
                egui::epaint::Shape::Text(t) => {
                    let r = t.galley.rect.translate(t.pos.to_vec2());
                    (cs.clip_rect.contains_rect(r) && r.max.y <= short.y)
                        .then(|| t.galley.text().to_string())
                }
                _ => None,
            })
            .collect();
    }
    for needle in [t!("review-submit"), t!("review-approve")] {
        assert!(
            seen.contains(&needle),
            "{needle:?} tersembunyi di layar pendek"
        );
    }
    // Semua teks berada di dalam layar.
    for (text, rect) in &shown.texts {
        assert!(
            rect.max.x <= SCREEN.x && rect.min.x >= 0.0,
            "{text:?} keluar layar: {rect:?}"
        );
    }
}

#[test]
fn approve_is_locked_until_named_and_unblocked() {
    let ctx = egui::Context::default();
    ducad_ui::theme::apply(&ctx, ducad_ui::theme::ThemeMode::Dark);
    let mut panel = ReviewPanel::default();
    let approve = t!("review-approve");

    // Tanpa nama: klik tidak menghasilkan apa-apa.
    assert_eq!(click(&ctx, &mut panel, &data(), &approve), None);
    panel.by = "budi".into();
    // Ada penghalang: tetap terkunci.
    let mut blocked = data();
    blocked.blockers = vec!["Open redlines remain".into()];
    assert_eq!(click(&ctx, &mut panel, &blocked, &approve), None);
    // Bersih + bernama: event Approve.
    assert_eq!(
        click(&ctx, &mut panel, &data(), &approve),
        Some(ReviewPanelEvent::Approve { by: "budi".into() })
    );
    // Sudah disetujui: tombol Ajukan terkunci, muncul "Buka kembali".
    let mut approved = data();
    approved.stage = ReviewStage::Approved;
    assert_eq!(
        click(&ctx, &mut panel, &approved, &t!("review-submit")),
        None
    );
    assert_eq!(
        click(&ctx, &mut panel, &approved, &t!("review-reopen")),
        Some(ReviewPanelEvent::Reopen)
    );
}

#[test]
fn pending_redline_asks_for_text() {
    let ctx = egui::Context::default();
    ducad_ui::theme::apply(&ctx, ducad_ui::theme::ThemeMode::Dark);
    let mut panel = ReviewPanel::default();
    let mut pending = data();
    pending.pending_redline = true;
    let shown = settle(&ctx, &mut panel, &pending);
    assert!(shown
        .texts
        .iter()
        .any(|(s, _)| *s == t!("review-redline-pending")));
    // Teks kosong: tombol pasang terkunci.
    assert_eq!(
        click(&ctx, &mut panel, &pending, &t!("review-redline-add")),
        None
    );
    panel.redline_text = "Tebalkan jadi 8".into();
    assert_eq!(
        click(&ctx, &mut panel, &pending, &t!("review-redline-add")),
        Some(ReviewPanelEvent::AddRedline {
            text: "Tebalkan jadi 8".into()
        })
    );
    assert_eq!(
        click(&ctx, &mut panel, &pending, &t!("review-cancel")),
        Some(ReviewPanelEvent::CancelRedline)
    );
}
