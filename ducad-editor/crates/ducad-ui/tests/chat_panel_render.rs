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

/// Sidebar terkunci di tepi kanan, memesan lebar untuk chrome lain, dan
/// tidak memakan area `ui` (kanvas tetap selebar layar di balik kaca).
#[test]
fn sidebar_locks_right_and_reserves_width() {
    let ctx = egui::Context::default();
    let mut t = 0.0;
    let mut run = |st: &mut ChatPanelState| {
        let mut rect = egui::Rect::NOTHING;
        for _ in 0..10 {
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
    let full = run(&mut closed);
    assert_eq!(ChatPanel::reserved_width(&ctx, &closed), 0.0);

    let mut open = ChatPanelState {
        open: true,
        ..Default::default()
    };
    let rest = run(&mut open);
    assert!(
        (rest.width() - full.width()).abs() < 1.0,
        "kanvas harus tetap selebar layar: {rest:?} vs {full:?}"
    );
    let reserved = ChatPanel::reserved_width(&ctx, &open);
    assert!(reserved >= 280.0, "lebar yang dipesan: {reserved}");
    let win = ctx
        .memory(|m| m.area_rect(egui::Id::new("ducad-chat-sidebar")))
        .expect("sidebar terbuka harus punya area");
    assert!(win.right() > 1180.0, "sidebar harus menempel di kanan: {win:?}");
    assert!(win.height() > 800.0, "sidebar harus setinggi layar: {win:?}");
}

/// Regresi: input panjang pernah mendorong tombol kirim keluar panel, dan
/// kartu composer pernah terpotong tepi bawah panel. Tombol kirim (yang
/// mengambang di dalam kartu) harus selalu berada di dalam rect jendela.
#[test]
fn send_button_stays_inside_panel() {
    for (name, input, settings, history, busy) in [
        ("kosong", String::new(), false, false, false),
        ("30 baris", "baris\n".repeat(30), false, false, false),
        ("semua terbuka", "a\nb\nc".to_string(), true, true, false),
        ("sibuk", "x".to_string(), false, true, true),
    ] {
        let ctx = egui::Context::default();
        let mut st = ChatPanelState {
            open: true,
            input,
            settings_open: settings,
            history_open: history,
            busy,
            ..Default::default()
        };
        let icon = if busy {
            egui_icons::icons::ICON_STOP.codepoint
        } else {
            egui_icons::icons::ICON_SEND.codepoint
        };
        let mut btn = None;
        for _ in 0..5 {
            let texts = frame(&ctx, &mut st);
            btn = texts.iter().find(|(s, _)| s == icon).map(|(_, r)| *r);
        }
        let r = btn.unwrap_or_else(|| panic!("{name}: ikon tombol harus tergambar"));
        let win = ctx
            .memory(|m| m.area_rect(egui::Id::new("ducad-chat-sidebar")))
            .expect("jendela chat harus punya area");
        assert!(
            r.max.y <= win.max.y - 4.0 && r.min.y >= win.min.y,
            "{name}: tombol harus di dalam jendela: tombol {r:?}, jendela {win:?}"
        );
    }
}
