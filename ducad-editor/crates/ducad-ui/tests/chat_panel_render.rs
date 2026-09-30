//! Reproduksi: transkrip panel chat harus benar-benar tergambar di layar.

use ducad_ui::{ChatItem, ChatPanel, ChatPanelState, ChatRole};

fn frame(ctx: &egui::Context, state: &mut ChatPanelState) -> Vec<(String, egui::Rect)> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 900.0),
        )),
        ..Default::default()
    };
    let out = ctx.run_ui(input, |ui| {
        ChatPanel::show(ui, state);
    });
    let mut out = out;
    out.textures_delta.clear();
    let mut texts = Vec::new();
    for cs in out.shapes {
        if let egui::epaint::Shape::Text(t) = cs.shape {
            let s = t.galley.text().to_string();
            let r = t.galley.rect.translate(t.pos.to_vec2());
            if cs.clip_rect.intersects(r) {
                texts.push((s, r));
            }
        }
    }
    texts
}

#[test]
fn user_message_is_visible_after_send() {
    let ctx = egui::Context::default();
    let mut st = ChatPanelState {
        open: true,
        ..Default::default()
    };
    for _ in 0..3 {
        frame(&ctx, &mut st);
    }
    st.items.push(ChatItem::text(
        ChatRole::User,
        "PESAN-UJI buat blok".to_string(),
    ));
    st.scroll_to_bottom = true;
    st.busy = true;
    let mut seen = false;
    for i in 0..5 {
        let texts = frame(&ctx, &mut st);
        let hit = texts.iter().any(|(s, _)| s.contains("PESAN-UJI"));
        eprintln!(
            "frame {i}: pesan terlihat={hit}; teks={:?}",
            texts
                .iter()
                .map(|(s, r)| (s.chars().take(24).collect::<String>(), r.min.y as i32))
                .collect::<Vec<_>>()
        );
        seen |= hit;
    }
    assert!(seen, "pesan pengguna tidak pernah tergambar");
}

/// Agent mengalirkan kejadian tiap frame: transkrip harus tetap terlihat.
#[test]
fn transcript_stays_visible_while_streaming() {
    let ctx = egui::Context::default();
    let mut st = ChatPanelState {
        open: true,
        busy: true,
        ..Default::default()
    };
    st.items
        .push(ChatItem::text(ChatRole::User, "PESAN-UJI".to_string()));
    for _ in 0..3 {
        frame(&ctx, &mut st);
    }
    for i in 0usize..30 {
        st.items
            .push(ChatItem::text(ChatRole::Assistant, format!("baris {i}")));
        st.scroll_to_bottom = true;
        let texts = frame(&ctx, &mut st);
        let lines: Vec<&str> = texts
            .iter()
            .map(|(s, _)| s.as_str())
            .filter(|s| s.starts_with("baris"))
            .collect();
        assert!(
            !lines.is_empty(),
            "frame {i}: transkrip kosong saat agent mengalirkan kejadian"
        );
        // Gulir diterapkan egui di frame berikutnya: toleransi satu frame.
        let latest = [
            format!("baris {i}"),
            format!("baris {}", i.saturating_sub(1)),
        ];
        assert!(
            lines.iter().any(|l| latest.iter().any(|x| x == l)),
            "frame {i}: baris terbaru tidak terlihat ({lines:?})"
        );
        assert!(
            texts
                .iter()
                .any(|(s, _)| s.contains("Agent is working") || s.contains("bekerja")),
            "frame {i}: status hilang"
        );
    }
}

/// Sidebar menempel di kanan dan menyempitkan area sisa (bukan mengambang).
#[test]
fn sidebar_docks_right_and_shrinks_remaining_area() {
    let ctx = egui::Context::default();
    let mut t = 0.0;
    let mut remaining = |st: &mut ChatPanelState| {
        let mut rect = egui::Rect::NOTHING;
        // Beberapa frame (0,1 s) agar animasi buka/tutup selesai.
        for _ in 0..30 {
            t += 0.1;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 900.0),
                )),
                time: Some(t),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                ChatPanel::show(ui, st);
                rect = ui.available_rect_before_wrap();
            });
            out.textures_delta.clear();
        }
        rect
    };

    let mut closed = ChatPanelState::default();
    let full = remaining(&mut closed);
    assert!(full.width() > 1190.0, "panel tertutup tidak boleh memakan ruang: {full:?}");

    let mut open = ChatPanelState {
        open: true,
        ..Default::default()
    };
    let rest = remaining(&mut open);
    assert!(
        rest.width() < full.width() - 250.0,
        "sidebar terbuka harus menyempitkan area sisa: {rest:?}"
    );
    assert!(rest.min.x < 1.0, "sidebar harus di kanan, bukan kiri: {rest:?}");
}
