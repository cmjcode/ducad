//! Command palette gaya VS Code/Spotlight — Cmd+K / Cmd+Shift+P, ketik untuk
//! cari aksi (semua tool, undo/redo, dst), Enter untuk eksekusi. Pelengkap
//! toolbar/shortcut huruf tunggal: cocok untuk aksi yang jarang dipakai
//! (tak pantas punya tombol toolbar sendiri) dan untuk device tanpa
//! keyboard fisik penuh (radial menu Fase 4 menutupi kasus sentuh cepat,
//! palette ini untuk pencarian aksi apa saja).
//!
//! Widget ini generik terhadap "apa aksinya" — caller (`ducad-app`)
//! menyediakan daftar [`PaletteEntry`] tiap frame dan menerima index balik
//! ke daftar yang sama saat satu entri dieksekusi, lalu memutuskan sendiri
//! aksi konkretnya lewat match. Pola yang sama dengan `RadialMenu`.
//!
//! Daftar yang sama juga dirender sebagai submenu di burger menu `TopBar`,
//! jadi setiap perintah palette selalu punya jalur klik di GUI.

use crate::theme::ACCENT_BLUE;
use ducad_i18n::t;
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Key, Margin, Painter, Pos2, Rect, RichText,
    Sense, Stroke, Ui,
};
use egui_icons::icons::{
    ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_UP, ICON_KEYBOARD_RETURN, ICON_SEARCH,
};

const PALETTE_WIDTH: f32 = 580.0;
const LIST_MAX_HEIGHT: f32 = 372.0;
const ROW_HEIGHT: f32 = 32.0;
const HEADER_HEIGHT: f32 = 26.0;
const ROW_GAP: f32 = 1.0;

/// Satu perintah di palette. Entri diharapkan sudah terurut per `group`
/// supaya judul grup hanya muncul sekali saat kolom cari kosong.
#[derive(Debug, Clone, Copy)]
pub struct PaletteEntry<'a> {
    pub label: &'a str,
    /// Pintasan keyboard (mis. `"⌘+Shift+A"`), boleh kosong.
    pub hint: &'a str,
    /// Judul grup, mis. "Berkas".
    pub group: &'a str,
    /// Codepoint ikon (`egui_icons::icons::ICON_*`).
    pub icon: &'a str,
}

#[derive(Default)]
pub struct CommandPalette {
    open: bool,
    query: String,
    highlighted: usize,
    focus_pending: bool,
    /// Gulir ke baris tersorot setelah navigasi keyboard / query berubah.
    scroll_pending: bool,
}

enum Row {
    Header(usize),
    Item(usize),
}

impl CommandPalette {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.highlighted = 0;
        self.focus_pending = true;
        self.scroll_pending = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Render overlay bila terbuka. Return `Some(index)` ke `entries` ASLI
    /// (bukan indeks hasil filter) saat satu entri dieksekusi (klik atau
    /// Enter di baris tersorot); palette otomatis tertutup setelahnya.
    /// Klik di luar palette atau Esc menutupnya.
    pub fn show(&mut self, ctx: &egui::Context, entries: &[PaletteEntry<'_>]) -> Option<usize> {
        if !self.open {
            return None;
        }
        if entries.is_empty() {
            self.close();
            return None;
        }

        let searching = !self.query.trim().is_empty();
        let filtered = filter_entries(&self.query, entries);
        if filtered.is_empty() {
            self.highlighted = 0;
        } else {
            let last = filtered.len() - 1;
            self.highlighted = self.highlighted.min(last);
            // Navigasi melingkar: dari baris terakhir kembali ke atas.
            if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
                self.highlighted = if self.highlighted == last { 0 } else { self.highlighted + 1 };
                self.scroll_pending = true;
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
                self.highlighted = if self.highlighted == 0 { last } else { self.highlighted - 1 };
                self.scroll_pending = true;
            }
        }
        let enter_pressed = ctx.input(|i| i.key_pressed(Key::Enter));

        // Judul grup hanya saat menjelajah; hasil pencarian berupa daftar
        // datar terurut skor dengan nama grup redup di sisi kanan.
        let mut rows = Vec::with_capacity(filtered.len() + 8);
        let mut prev_group: Option<&str> = None;
        for (pos, &idx) in filtered.iter().enumerate() {
            if !searching && prev_group != Some(entries[idx].group) {
                rows.push(Row::Header(idx));
                prev_group = Some(entries[idx].group);
            }
            rows.push(Row::Item(pos));
        }
        let content_h: f32 = rows
            .iter()
            .map(|r| match r {
                Row::Header(_) => HEADER_HEIGHT + ROW_GAP,
                Row::Item(_) => ROW_HEIGHT + ROW_GAP,
            })
            .sum();
        let list_h = content_h.clamp(ROW_HEIGHT * 2.0, LIST_MAX_HEIGHT);

        let dark = ctx.global_style().visuals.dark_mode;
        let colors = Colors::new(dark);
        let width = PALETTE_WIDTH.min(ctx.content_rect().width() - 32.0).max(260.0);
        let id = egui::Id::new("ducad-command-palette");

        let mut result = None;
        let modal = egui::Modal::new(id)
            .area(
                egui::Modal::default_area(id).anchor(Align2::CENTER_TOP, vec2(0.0, 84.0)),
            )
            .backdrop_color(Color32::from_black_alpha(if dark { 120 } else { 70 }))
            .frame(egui::Frame {
                inner_margin: Margin::ZERO,
                outer_margin: Margin::ZERO,
                corner_radius: CornerRadius::same(12),
                shadow: egui::Shadow {
                    offset: [0, 12],
                    blur: 36,
                    spread: 0,
                    color: Color32::from_black_alpha(120),
                },
                fill: colors.bg,
                stroke: Stroke::new(1.0, colors.border),
            })
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

                // ── Kolom cari ─────────────────────────────────────────
                egui::Frame::NONE
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            ui.label(
                                RichText::new(ICON_SEARCH.codepoint).size(18.0).color(colors.muted),
                            );
                            let count = format!("{}", filtered.len());
                            let count_w = 44.0;
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut self.query)
                                    .frame(egui::Frame::NONE)
                                    .hint_text(
                                        RichText::new(t!("cmd-search-hint")).color(colors.muted),
                                    )
                                    .font(FontId::proportional(15.0))
                                    .text_color(colors.text)
                                    .desired_width(ui.available_width() - count_w),
                            );
                            if self.focus_pending {
                                resp.request_focus();
                                self.focus_pending = false;
                            }
                            if resp.changed() {
                                self.highlighted = 0;
                                self.scroll_pending = true;
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(RichText::new(count).size(11.0).color(colors.muted));
                                },
                            );
                        });
                    });
                divider(ui, colors.border);

                // ── Daftar perintah ────────────────────────────────────
                // Tinggi dikunci eksplisit: ScrollArea di dalam Area yang
                // menyusut ke `min_scrolled_height` bawaan (64 px) hanya
                // menampilkan dua baris.
                egui::ScrollArea::vertical()
                    .max_height(list_h)
                    .min_scrolled_height(list_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, ROW_GAP);
                        let pointer_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
                        let row_w = ui.available_width();
                        if filtered.is_empty() {
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(row_w, list_h), Sense::hover());
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                t!("cmd-no-match"),
                                FontId::proportional(13.0),
                                colors.muted,
                            );
                        }
                        for row in &rows {
                            match *row {
                                Row::Header(idx) => {
                                    let (rect, _) = ui.allocate_exact_size(
                                        vec2(row_w, HEADER_HEIGHT),
                                        Sense::hover(),
                                    );
                                    ui.painter().text(
                                        pos2(rect.left() + 16.0, rect.bottom() - 6.0),
                                        Align2::LEFT_BOTTOM,
                                        entries[idx].group.to_uppercase(),
                                        FontId::proportional(10.0),
                                        colors.muted,
                                    );
                                }
                                Row::Item(pos) => {
                                    let idx = filtered[pos];
                                    let (rect, resp) = ui.allocate_exact_size(
                                        vec2(row_w, ROW_HEIGHT),
                                        Sense::click(),
                                    );
                                    if resp.hovered() && pointer_moved {
                                        self.highlighted = pos;
                                    }
                                    let selected = pos == self.highlighted;
                                    if selected && self.scroll_pending {
                                        ui.scroll_to_rect(rect.expand2(vec2(0.0, 6.0)), None);
                                        self.scroll_pending = false;
                                    }
                                    paint_row(
                                        ui.painter(),
                                        rect,
                                        &entries[idx],
                                        selected,
                                        searching,
                                        &colors,
                                    );
                                    if resp.clicked() || (selected && enter_pressed) {
                                        result = Some(idx);
                                    }
                                }
                            }
                        }
                    });

                // ── Petunjuk tombol ────────────────────────────────────
                divider(ui, colors.border);
                egui::Frame::NONE
                    .inner_margin(Margin::symmetric(14, 7))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            keycap(ui, ICON_KEYBOARD_ARROW_UP.codepoint, &colors);
                            keycap(ui, ICON_KEYBOARD_ARROW_DOWN.codepoint, &colors);
                            footer_label(ui, &t!("cmd-hint-navigate"), &colors);
                            keycap(ui, ICON_KEYBOARD_RETURN.codepoint, &colors);
                            footer_label(ui, &t!("cmd-hint-run"), &colors);
                            keycap(ui, "Esc", &colors);
                            footer_label(ui, &t!("cmd-hint-close"), &colors);
                        });
                    });
            });

        if result.is_some() || modal.should_close() {
            self.close();
        }
        result
    }
}

struct Colors {
    bg: Color32,
    border: Color32,
    text: Color32,
    muted: Color32,
    selection: Color32,
    key_bg: Color32,
}

impl Colors {
    /// Latar nyaris solid: palette menimpa viewport 3D yang ramai, jadi kaca
    /// tembus pandang ala panel lain membuat teks sulit dibaca.
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                bg: Color32::from_rgba_unmultiplied(22, 24, 29, 248),
                border: Color32::from_rgb(52, 57, 68),
                text: Color32::from_rgb(236, 237, 240),
                muted: Color32::from_rgb(134, 138, 148),
                selection: Color32::from_rgba_unmultiplied(10, 132, 255, 56),
                key_bg: Color32::from_rgb(40, 44, 53),
            }
        } else {
            Self {
                bg: Color32::from_rgba_unmultiplied(252, 252, 253, 250),
                border: Color32::from_rgb(214, 217, 224),
                text: Color32::from_rgb(28, 30, 36),
                muted: Color32::from_rgb(110, 114, 124),
                selection: Color32::from_rgba_unmultiplied(10, 132, 255, 38),
                key_bg: Color32::from_rgb(236, 238, 242),
            }
        }
    }
}

fn divider(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
}

fn footer_label(ui: &mut Ui, text: &str, colors: &Colors) {
    ui.label(RichText::new(text).size(10.5).color(colors.muted));
    ui.add_space(8.0);
}

fn paint_row(
    painter: &Painter,
    rect: Rect,
    entry: &PaletteEntry<'_>,
    selected: bool,
    show_group: bool,
    colors: &Colors,
) {
    let inner = rect.shrink2(vec2(6.0, 0.0));
    if selected {
        painter.rect_filled(inner, CornerRadius::same(7), colors.selection);
        let bar = Rect::from_min_max(
            pos2(inner.left(), inner.top() + 8.0),
            pos2(inner.left() + 3.0, inner.bottom() - 8.0),
        );
        painter.rect_filled(bar, CornerRadius::same(2), ACCENT_BLUE);
    }

    painter.text(
        pos2(inner.left() + 22.0, inner.center().y),
        Align2::CENTER_CENTER,
        entry.icon,
        FontId::proportional(16.0),
        if selected { ACCENT_BLUE } else { colors.muted },
    );

    // Sisi kanan dulu (keycap, lalu nama grup) supaya label tahu batasnya.
    let mut right = inner.right() - 10.0;
    for key in split_keys(entry.hint).iter().rev() {
        right = paint_keycap(painter, pos2(right, inner.center().y), key, colors) - 4.0;
    }
    if show_group {
        let galley =
            painter.layout_no_wrap(entry.group.to_owned(), FontId::proportional(10.5), colors.muted);
        right -= galley.size().x + 6.0;
        painter.galley(
            pos2(right, inner.center().y - galley.size().y / 2.0),
            galley,
            colors.muted,
        );
    }

    let label_left = inner.left() + 42.0;
    let galley =
        painter.layout_no_wrap(entry.label.to_owned(), FontId::proportional(13.0), colors.text);
    let clip = Rect::from_min_max(
        pos2(label_left, inner.top()),
        pos2((right - 10.0).max(label_left), inner.bottom()),
    );
    painter.with_clip_rect(clip).galley(
        pos2(label_left, inner.center().y - galley.size().y / 2.0),
        galley,
        colors.text,
    );
}

/// Gambar satu keycap rata-kanan di `right_center`; kembalikan tepi kirinya.
fn paint_keycap(painter: &Painter, right_center: Pos2, text: &str, colors: &Colors) -> f32 {
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(10.5), colors.text);
    let size = vec2((galley.size().x + 10.0).max(20.0), 18.0);
    let rect = Rect::from_min_size(
        pos2(right_center.x - size.x, right_center.y - size.y / 2.0),
        size,
    );
    paint_keycap_in(painter, rect, galley, colors);
    rect.left()
}

fn keycap(ui: &mut Ui, text: &str, colors: &Colors) {
    let galley =
        ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(10.5), colors.text);
    let size = vec2((galley.size().x + 10.0).max(20.0), 18.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    paint_keycap_in(ui.painter(), rect, galley, colors);
}

fn paint_keycap_in(
    painter: &Painter,
    rect: Rect,
    galley: std::sync::Arc<egui::Galley>,
    colors: &Colors,
) {
    painter.rect(
        rect,
        CornerRadius::same(4),
        colors.key_bg,
        Stroke::new(1.0, colors.border),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.center() - galley.size() / 2.0, galley, colors.text);
}

/// Pecah teks pintasan menjadi keycap: `"⌘+Shift+A"` → `[⌘, Shift, A]`,
/// `"⌘O"` → `[⌘, O]`.
fn split_keys(hint: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for part in hint.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.strip_prefix('⌘') {
            Some(rest) => {
                keys.push("⌘".to_string());
                if !rest.is_empty() {
                    keys.push(rest.to_string());
                }
            }
            None => keys.push(part.to_string()),
        }
    }
    keys
}

/// Indeks entri yang cocok dengan `query`. Query kosong → semua entri dalam
/// urutan asli; selain itu terurut skor (tertinggi dulu, seri = urutan asli).
fn filter_entries(query: &str, entries: &[PaletteEntry<'_>]) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return (0..entries.len()).collect();
    }
    let tokens: Vec<&str> = query.split_whitespace().collect();
    let mut scored: Vec<(i32, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let label = e.label.to_lowercase();
            let group = e.group.to_lowercase();
            let mut total = 0;
            for token in &tokens {
                total += token_score(token, &label, &group)?;
            }
            Some((total, i))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

/// Skor satu kata kunci: awal label > awal kata > di tengah label > nama
/// grup > huruf berurutan (fuzzy). `None` = tidak cocok.
fn token_score(token: &str, label: &str, group: &str) -> Option<i32> {
    if let Some(pos) = label.find(token) {
        let at_word_start = label[..pos]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        return Some(if pos == 0 {
            100
        } else if at_word_start {
            80
        } else {
            60
        });
    }
    if group.contains(token) {
        return Some(30);
    }
    let mut chars = label.chars();
    token
        .chars()
        .all(|t| chars.any(|c| c == t))
        .then_some(10)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry<'a>(label: &'a str, group: &'a str) -> PaletteEntry<'a> {
        PaletteEntry { label, hint: "", group, icon: "" }
    }

    #[test]
    fn empty_query_keeps_original_order() {
        let entries = [entry("Simpan", "Berkas"), entry("Garis", "Sketsa")];
        assert_eq!(filter_entries("  ", &entries), vec![0, 1]);
    }

    #[test]
    fn prefix_match_ranks_above_inner_and_fuzzy_match() {
        let entries = [
            entry("Import STEP…", "Berkas"),
            entry("Export STEP…", "Berkas"),
            entry("Step ulang", "Edit"),
            entry("Garis", "Sketsa"),
        ];
        assert_eq!(filter_entries("step", &entries), vec![2, 0, 1]);
    }

    #[test]
    fn every_token_must_match_and_group_name_counts() {
        let entries = [
            entry("Export STL…", "Berkas"),
            entry("Export STEP…", "Berkas"),
            entry("Garis", "Sketsa"),
        ];
        assert_eq!(filter_entries("export stl", &entries), vec![0]);
        assert_eq!(filter_entries("sketsa", &entries), vec![2]);
        assert_eq!(filter_entries("grs", &entries), vec![2]);
        assert!(filter_entries("zzz", &entries).is_empty());
    }

    /// Regresi: daftar dulu menyusut jadi dua baris walau ada puluhan perintah.
    #[test]
    fn list_fills_its_height_and_enter_runs_highlighted_entry() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx, crate::theme::ThemeMode::Dark);
        let labels: Vec<String> = (0..40).map(|i| format!("Perintah {i}")).collect();
        let entries: Vec<PaletteEntry<'_>> =
            labels.iter().map(|l| entry(l, "Grup")).collect();
        let mut palette = CommandPalette::default();
        palette.open();

        let input = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
            ..Default::default()
        };
        let mut frame = |raw: egui::RawInput| {
            let mut picked = None;
            let mut out = ctx.run_ui(raw, |ui| picked = palette.show(ui.ctx(), &entries));
            out.textures_delta.clear();
            picked
        };
        for _ in 0..3 {
            assert_eq!(frame(input()), None);
        }
        let rect = ctx
            .memory(|m| m.area_rect(egui::Id::new("ducad-command-palette")))
            .expect("area palette terdaftar");
        assert!(rect.height() > LIST_MAX_HEIGHT, "tinggi palette {}", rect.height());
        assert!((rect.width() - PALETTE_WIDTH).abs() < 4.0, "lebar palette {}", rect.width());

        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let mut with_down = input();
        with_down.events.push(key(Key::ArrowDown));
        assert_eq!(frame(with_down), None);
        let mut with_enter = input();
        with_enter.events.push(key(Key::Enter));
        let picked = frame(with_enter);
        assert_eq!(picked, Some(1));
        assert!(!palette.is_open());
    }

    #[test]
    fn shortcut_hint_splits_into_keycaps() {
        assert_eq!(split_keys("⌘+Shift+A"), ["⌘", "Shift", "A"]);
        assert_eq!(split_keys("⌘O"), ["⌘", "O"]);
        assert_eq!(split_keys("Shift+E"), ["Shift", "E"]);
        assert!(split_keys("").is_empty());
    }
}
