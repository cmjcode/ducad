//! Saran tindakan berikutnya (Step 5 `PLAN_DESIGNER_UX`, checklist P.5.5):
//! teksnya benar-benar tergambar di bawah titik jangkar, dan tombol tutup
//! melaporkan klik.

use ducad_ui::CanvasHud;
use egui_icons::icons::{ICON_CLOSE, ICON_LIGHTBULB_ON};

const SCREEN: egui::Vec2 = egui::vec2(1280.0, 860.0);
const ANCHOR: egui::Pos2 = egui::pos2(40.0, 80.0);
const TEXT: &str = "Mulai dengan menggambar profil";

fn frame(ctx: &egui::Context, events: Vec<egui::Event>) -> (Vec<(String, egui::Rect)>, bool) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
        events,
        ..Default::default()
    };
    let mut closed = false;
    let mut out = ctx.run_ui(input, |ui| {
        closed = CanvasHud::show_next_action(ui, ANCHOR, TEXT);
    });
    out.textures_delta.clear();
    let mut texts = Vec::new();
    for cs in out.shapes {
        if let egui::epaint::Shape::Text(t) = cs.shape {
            let r = t.galley.rect.translate(t.pos.to_vec2());
            texts.push((t.galley.text().to_string(), r));
        }
    }
    (texts, closed)
}

fn settle(ctx: &egui::Context) -> Vec<(String, egui::Rect)> {
    let mut last = frame(ctx, Vec::new()).0;
    for _ in 0..3 {
        last = frame(ctx, Vec::new()).0;
    }
    last
}

#[test]
fn hint_draws_icon_text_and_close_button_at_the_anchor() {
    let ctx = egui::Context::default();
    let texts = settle(&ctx);
    let find = |needle: &str| {
        texts
            .iter()
            .find(|(s, _)| s.contains(needle))
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("{needle:?} tidak tergambar: {texts:?}"))
    };
    let icon = find(ICON_LIGHTBULB_ON.codepoint);
    let text = find(TEXT);
    let close = find(ICON_CLOSE.codepoint);
    assert!(icon.min.x >= ANCHOR.x - 1.0 && icon.min.y >= ANCHOR.y - 1.0, "mulai di jangkar");
    assert!(icon.max.x <= text.min.x && text.max.x <= close.min.x, "urutan: lampu, teks, tutup");
}

#[test]
fn close_button_reports_the_click() {
    let ctx = egui::Context::default();
    let texts = settle(&ctx);
    let pos = texts
        .iter()
        .find(|(s, _)| s.contains(ICON_CLOSE.codepoint))
        .map(|(_, r)| r.center())
        .expect("tombol tutup tergambar");
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    assert!(!frame(&ctx, vec![egui::Event::PointerMoved(pos)]).1);
    assert!(!frame(&ctx, vec![button(true)]).1);
    assert!(frame(&ctx, vec![button(false)]).1, "klik tutup dilaporkan");
}
