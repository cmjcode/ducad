//! Keyboard layar ringkas untuk tablet (iPad).
//!
//! Keyboard sistem iPad memenuhi separuh bawah layar dan sering menutupi
//! kotak teks yang sedang diisi. Modul ini menggambar keypad kecil yang
//! mengambang tepat di samping kotak teks yang fokus (di bawah, atau di atas
//! bila ruang di bawah tidak cukup), sehingga kotak teks selalu terlihat.
//!
//! Interaksi tidak memakai widget egui: tombol egui akan merebut fokus dari
//! `TextEdit` begitu ditekan. Sebagai gantinya [`CompactKeyboard::filter_input`]
//! dipanggil dari `eframe::App::raw_input_hook`, memakan event pointer/sentuh
//! yang jatuh di atas keypad, lalu menyuntikkan `Event::Text` / `Event::Key`
//! — persis seperti ketikan keyboard sungguhan.
//!
//! Dua tata letak: angka (default untuk kotak sempit, mis. dimensi) dan huruf
//! (kotak lebar, mis. chat). Tombol keyboard sistem tetap tersedia untuk
//! bahasa lain, emoji, atau dikte.

use std::collections::HashMap;

use egui::{
    Align2, Color32, Context, CornerRadius, Event, FontId, Id, Key, LayerId, Modifiers, Order,
    Pos2, RawInput, Rect, Stroke, StrokeKind, TouchId, TouchPhase, Vec2,
};
use egui_icons::icons::{
    ICON_ARROW_UPWARD, ICON_KEYBOARD, ICON_KEYBOARD_BACKSPACE, ICON_KEYBOARD_HIDE,
    ICON_KEYBOARD_RETURN, ICON_SPACE_BAR,
};

/// Jarak keypad dari kotak teks dan dari tepi layar (poin).
const GAP_TO_FIELD: f32 = 8.0;
const SCREEN_MARGIN: f32 = 8.0;
const PADDING: f32 = 8.0;
const HEADER_H: f32 = 28.0;
/// Tahan backspace: jeda sebelum mengulang, lalu interval ulang (detik).
const REPEAT_DELAY: f64 = 0.45;
const REPEAT_INTERVAL: f64 = 0.06;
/// Kotak teks lebih lebar dari ini dianggap teks bebas → tata letak huruf.
const WIDE_FIELD: f32 = 260.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardLayout {
    Numeric,
    Letters,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum KeyAction {
    Char(char),
    Backspace,
    Enter,
    Space,
    Shift,
    Layout(KeyboardLayout),
    SystemKeyboard,
    Hide,
}

#[derive(Clone, Copy, Debug)]
enum KeyLabel {
    Text(&'static str),
    Icon(&'static str),
}

#[derive(Clone, Copy, Debug)]
struct KeyCap {
    action: KeyAction,
    label: KeyLabel,
    rect: Rect,
    /// Tombol fungsi (warna latar berbeda).
    function: bool,
}

/// Preferensi per kotak teks (berdasarkan `Id` widget yang fokus).
#[derive(Clone, Copy, Debug)]
struct FieldPrefs {
    layout: KeyboardLayout,
    system: bool,
}

/// Hasil [`CompactKeyboard::show`] untuk lapisan platform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompactKeyboardOutput {
    /// `true` bila keyboard sistem harus dipakai (keypad ringkas tidak
    /// tampil); `false` bila keyboard sistem harus disembunyikan.
    pub system_keyboard: bool,
}

#[derive(Default)]
pub struct CompactKeyboard {
    /// Hanya aktif di tablet; di desktop keypad tidak pernah tampil.
    pub enabled: bool,
    /// Keypad yang tergambar pada frame terakhir (koordinat poin).
    rect: Option<Rect>,
    keys: Vec<KeyCap>,
    focus: Option<Id>,
    prefs: HashMap<Id, FieldPrefs>,
    /// Disembunyikan pengguna untuk sesi fokus ini.
    hidden: bool,
    /// Ada ketikan keyboard fisik pada sesi fokus ini → keypad tidak perlu.
    hardware_typing: bool,
    shift: bool,
    capturing: bool,
    captured_touch: Option<TouchId>,
    pressed: Option<usize>,
    repeat_at: Option<f64>,
}

impl CompactKeyboard {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Default::default()
        }
    }

    /// Area keypad pada frame terakhir, bila tampil.
    pub fn rect(&self) -> Option<Rect> {
        self.rect
    }

    /// Panggil dari `raw_input_hook`: event pointer di atas keypad dimakan
    /// dan diganti event ketikan.
    pub fn filter_input(&mut self, raw: &mut RawInput) {
        if !self.enabled {
            return;
        }
        let Some(rect) = self.rect else {
            self.release();
            return;
        };
        let now = raw.time.unwrap_or(0.0);
        let mut out = Vec::new();
        let events = std::mem::take(&mut raw.events);
        let mut kept = Vec::with_capacity(events.len());
        for ev in events {
            match &ev {
                // Ketikan dari keyboard fisik: keypad tidak dibutuhkan lagi.
                Event::Text(_) | Event::Key { pressed: true, .. } => {
                    self.hardware_typing = true;
                    kept.push(ev);
                }
                Event::Touch { id, phase, pos, .. } => {
                    let ours = self.captured_touch == Some(*id)
                        || (*phase == TouchPhase::Start && rect.contains(*pos));
                    if ours {
                        match phase {
                            TouchPhase::Start => self.captured_touch = Some(*id),
                            TouchPhase::End | TouchPhase::Cancel => self.captured_touch = None,
                            TouchPhase::Move => {}
                        }
                    } else {
                        kept.push(ev);
                    }
                }
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } => {
                    if *pressed && rect.contains(*pos) {
                        self.capturing = true;
                        self.pressed = self.key_at(*pos);
                        self.repeat_at = self
                            .pressed
                            .filter(|&i| self.keys[i].action == KeyAction::Backspace)
                            .map(|_| now + REPEAT_DELAY);
                        // Backspace langsung menghapus saat ditekan (seperti iOS).
                        if self.repeat_at.is_some() {
                            out.push(KeyAction::Backspace);
                        }
                    } else if !*pressed && self.capturing {
                        if let Some(i) = self.key_at(*pos).or(self.pressed) {
                            let action = self.keys[i].action;
                            // Backspace sudah dikirim saat ditekan.
                            if action != KeyAction::Backspace {
                                out.push(action);
                            }
                        }
                        self.release();
                    } else {
                        kept.push(ev);
                    }
                }
                Event::PointerMoved(pos) => {
                    if self.capturing {
                        let now_key = self.key_at(*pos);
                        if now_key != self.pressed {
                            self.pressed = now_key;
                            self.repeat_at = None;
                        }
                    } else if !rect.contains(*pos) {
                        kept.push(ev);
                    }
                }
                Event::PointerGone => {
                    if self.capturing && self.captured_touch.is_none() {
                        self.release();
                    }
                    kept.push(ev);
                }
                _ => kept.push(ev),
            }
        }
        if let (Some(at), Some(i)) = (self.repeat_at, self.pressed) {
            if self.keys[i].action == KeyAction::Backspace && now >= at {
                out.push(KeyAction::Backspace);
                self.repeat_at = Some(now + REPEAT_INTERVAL);
            }
        }
        raw.events = kept;
        for action in out {
            self.apply(action, &mut raw.events);
        }
    }

    fn release(&mut self) {
        self.capturing = false;
        self.pressed = None;
        self.repeat_at = None;
    }

    fn key_at(&self, pos: Pos2) -> Option<usize> {
        self.keys.iter().position(|k| k.rect.contains(pos))
    }

    fn prefs_mut(&mut self) -> Option<&mut FieldPrefs> {
        let id = self.focus?;
        self.prefs.get_mut(&id)
    }

    fn apply(&mut self, action: KeyAction, events: &mut Vec<Event>) {
        let key = |events: &mut Vec<Event>, key: Key| {
            for pressed in [true, false] {
                events.push(Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                });
            }
        };
        match action {
            KeyAction::Char(c) => {
                let c = if self.shift { c.to_ascii_uppercase() } else { c };
                self.shift = false;
                events.push(Event::Text(c.to_string()));
            }
            KeyAction::Space => events.push(Event::Text(" ".into())),
            KeyAction::Backspace => key(events, Key::Backspace),
            KeyAction::Enter => key(events, Key::Enter),
            KeyAction::Shift => self.shift = !self.shift,
            KeyAction::Layout(layout) => {
                if let Some(p) = self.prefs_mut() {
                    p.layout = layout;
                }
            }
            KeyAction::SystemKeyboard => {
                if let Some(p) = self.prefs_mut() {
                    p.system = true;
                }
            }
            KeyAction::Hide => self.hidden = true,
        }
    }

    /// Panggil di AKHIR frame (setelah semua `TextEdit` tergambar): gambar
    /// keypad di samping kotak teks yang fokus.
    pub fn show(&mut self, ctx: &Context) -> CompactKeyboardOutput {
        self.rect = None;
        self.keys.clear();
        if !self.enabled {
            return CompactKeyboardOutput::default();
        }
        let ime = ctx.output(|o| o.ime);
        let focused = ctx.memory(|m| m.focused());
        let (Some(ime), Some(id)) = (ime, focused) else {
            self.focus = None;
            self.release();
            return CompactKeyboardOutput::default();
        };
        if self.focus != Some(id) {
            self.focus = Some(id);
            self.hidden = false;
            self.hardware_typing = false;
            self.shift = false;
            self.release();
        }
        let field = ime.rect;
        let prefs = *self.prefs.entry(id).or_insert_with(|| FieldPrefs {
            layout: default_layout(field),
            system: false,
        });
        if prefs.system {
            return CompactKeyboardOutput {
                system_keyboard: true,
            };
        }
        // Ketuk kotak teks lagi untuk memunculkan keypad yang disembunyikan.
        let tapped_field = ctx.input(|i| {
            i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| field.contains(p))
        });
        if tapped_field {
            self.hidden = false;
            self.hardware_typing = false;
        }
        if self.hidden || self.hardware_typing {
            return CompactKeyboardOutput::default();
        }

        let size = panel_size(prefs.layout);
        let rect = place(field, size, ctx.content_rect());
        self.keys = layout_keys(prefs.layout, rect);
        self.rect = Some(rect);
        self.paint(ctx, rect);
        if self.repeat_at.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        CompactKeyboardOutput::default()
    }

    fn paint(&self, ctx: &Context, rect: Rect) {
        let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("compact_keyboard")));
        let visuals = ctx.global_style().visuals.clone();
        let radius = CornerRadius::same(12);
        painter.add(
            egui::epaint::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: Color32::from_black_alpha(70),
            }
            .as_shape(rect, radius),
        );
        painter.rect(
            rect,
            radius,
            visuals.window_fill,
            visuals.window_stroke,
            StrokeKind::Inside,
        );
        let text_color = visuals.text_color();
        for (i, key) in self.keys.iter().enumerate() {
            let pressed = self.capturing && self.pressed == Some(i);
            let active = key.action == KeyAction::Shift && self.shift;
            let header = matches!(key.action, KeyAction::SystemKeyboard | KeyAction::Hide);
            let fill = if pressed || active {
                visuals.selection.bg_fill
            } else if key.function {
                visuals.widgets.inactive.weak_bg_fill
            } else {
                visuals.widgets.inactive.bg_fill
            };
            if !header || pressed {
                painter.rect(
                    key.rect,
                    CornerRadius::same(6),
                    fill,
                    Stroke::NONE,
                    StrokeKind::Inside,
                );
            }
            let (text, size) = match (key.label, key.action) {
                (_, KeyAction::Char(c)) if self.shift => (c.to_ascii_uppercase().to_string(), 17.0),
                (KeyLabel::Text(t), _) => (t.to_string(), 17.0),
                (KeyLabel::Icon(c), _) => (c.to_string(), 20.0),
            };
            painter.text(
                key.rect.center(),
                Align2::CENTER_CENTER,
                text,
                FontId::proportional(size),
                text_color,
            );
        }
    }
}

fn default_layout(field: Rect) -> KeyboardLayout {
    if field.width() > WIDE_FIELD || field.height() > 40.0 {
        KeyboardLayout::Letters
    } else {
        KeyboardLayout::Numeric
    }
}

/// Lebar sel tombol, tinggi, jarak antar tombol, dan jumlah kolom.
fn metrics(layout: KeyboardLayout) -> (f32, f32, f32, usize) {
    match layout {
        KeyboardLayout::Numeric => (48.0, 40.0, 6.0, 4),
        KeyboardLayout::Letters => (32.0, 38.0, 4.0, 10),
    }
}

fn panel_size(layout: KeyboardLayout) -> Vec2 {
    let (w, h, gap, cols) = metrics(layout);
    let cols = cols as f32;
    let rows = 4.0;
    Vec2::new(
        PADDING * 2.0 + cols * w + (cols - 1.0) * gap,
        PADDING * 2.0 + HEADER_H + gap + rows * h + (rows - 1.0) * gap,
    )
}

/// Letakkan keypad di dekat `field` tanpa menutupinya: di bawah, lalu di
/// atas, lalu di kanan/kiri; terakhir pojok kanan bawah layar.
fn place(field: Rect, size: Vec2, screen: Rect) -> Rect {
    let area = screen.shrink(SCREEN_MARGIN);
    let clamp_x = |x: f32| x.clamp(area.min.x, (area.max.x - size.x).max(area.min.x));
    let clamp_y = |y: f32| y.clamp(area.min.y, (area.max.y - size.y).max(area.min.y));
    let x = clamp_x(field.min.x);
    let below = field.max.y + GAP_TO_FIELD;
    if below + size.y <= area.max.y {
        return Rect::from_min_size(Pos2::new(x, below), size);
    }
    let above = field.min.y - GAP_TO_FIELD - size.y;
    if above >= area.min.y {
        return Rect::from_min_size(Pos2::new(x, above), size);
    }
    let y = clamp_y(field.center().y - size.y / 2.0);
    let right = field.max.x + GAP_TO_FIELD;
    if right + size.x <= area.max.x {
        return Rect::from_min_size(Pos2::new(right, y), size);
    }
    let left = field.min.x - GAP_TO_FIELD - size.x;
    if left >= area.min.x {
        return Rect::from_min_size(Pos2::new(left, y), size);
    }
    Rect::from_min_size(area.max - size, size)
}

type RowItem = (f32, KeyAction, KeyLabel, bool);

fn layout_keys(layout: KeyboardLayout, rect: Rect) -> Vec<KeyCap> {
    use KeyAction as A;
    let (w, h, gap, _) = metrics(layout);
    let mut keys = Vec::new();
    let inner = rect.shrink(PADDING);

    // Baris kepala: keyboard sistem (kiri) dan sembunyikan (kanan).
    let head_size = Vec2::new(44.0, HEADER_H);
    keys.push(KeyCap {
        action: A::SystemKeyboard,
        label: KeyLabel::Icon(ICON_KEYBOARD.codepoint),
        rect: Rect::from_min_size(inner.min, head_size),
        function: true,
    });
    keys.push(KeyCap {
        action: A::Hide,
        label: KeyLabel::Icon(ICON_KEYBOARD_HIDE.codepoint),
        rect: Rect::from_min_size(Pos2::new(inner.max.x - head_size.x, inner.min.y), head_size),
        function: true,
    });

    // Satu baris tombol; lebar dalam "unit" (1 unit = 1 tombol biasa).
    let top = inner.min.y + HEADER_H + gap;
    let mut row = |r: usize, items: &[RowItem]| {
        let y = top + r as f32 * (h + gap);
        let mut x = inner.min.x;
        for &(units, action, label, function) in items {
            let width = units * w + (units - 1.0) * gap;
            keys.push(KeyCap {
                action,
                label,
                rect: Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, h)),
                function,
            });
            x += width + gap;
        }
    };
    let ch = |c: char| -> RowItem { (1.0, A::Char(c), KeyLabel::Text(char_label(c)), false) };
    let backspace: RowItem = (1.0, A::Backspace, KeyLabel::Icon(ICON_KEYBOARD_BACKSPACE.codepoint), true);
    let enter = |units: f32| -> RowItem { (units, A::Enter, KeyLabel::Icon(ICON_KEYBOARD_RETURN.codepoint), true) };

    match layout {
        KeyboardLayout::Numeric => {
            row(0, &[ch('7'), ch('8'), ch('9'), backspace]);
            row(1, &[ch('4'), ch('5'), ch('6'), ch('-')]);
            row(2, &[ch('1'), ch('2'), ch('3'), ch('+')]);
            row(3, &[
                (1.0, A::Layout(KeyboardLayout::Letters), KeyLabel::Text("ABC"), true),
                ch('0'),
                ch('.'),
                enter(1.0),
            ]);
        }
        KeyboardLayout::Letters => {
            let letters = |s: &str| s.chars().map(ch).collect::<Vec<_>>();
            row(0, &letters("qwertyuiop"));
            let mut r1 = letters("asdfghjkl");
            r1.push(backspace);
            row(1, &r1);
            let mut r2 = vec![(1.0, A::Shift, KeyLabel::Icon(ICON_ARROW_UPWARD.codepoint), true)];
            r2.extend(letters("zxcvbnm,."));
            row(2, &r2);
            row(3, &[
                (1.5, A::Layout(KeyboardLayout::Numeric), KeyLabel::Text("123"), true),
                (4.5, A::Space, KeyLabel::Icon(ICON_SPACE_BAR.codepoint), false),
                ch('-'),
                ch('_'),
                enter(2.0),
            ]);
        }
    }
    keys
}

/// Label statis untuk satu karakter tombol.
fn char_label(c: char) -> &'static str {
    const CHARS: &str = "0123456789abcdefghijklmnopqrstuvwxyz-+.,_";
    match CHARS.find(c) {
        Some(i) => &CHARS[i..i + 1],
        None => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1024.0, 768.0))
    }

    #[test]
    fn keypad_never_covers_the_field() {
        for layout in [KeyboardLayout::Numeric, KeyboardLayout::Letters] {
            let size = panel_size(layout);
            for y in [10.0, 300.0, 600.0, 730.0] {
                for x in [0.0, 500.0, 980.0] {
                    let field = Rect::from_min_size(Pos2::new(x, y), Vec2::new(40.0, 24.0));
                    let kb = place(field, size, screen());
                    assert!(!kb.intersects(field), "{layout:?} menutupi kotak di {x},{y}");
                    assert!(screen().contains_rect(kb), "{layout:?} keluar layar di {x},{y}");
                }
            }
        }
    }

    #[test]
    fn keypad_is_small() {
        let n = panel_size(KeyboardLayout::Numeric);
        let l = panel_size(KeyboardLayout::Letters);
        assert!(n.x < 260.0 && n.y < 260.0, "{n:?}");
        assert!(l.x < 400.0 && l.y < 260.0, "{l:?}");
    }

    #[test]
    fn keys_stay_inside_panel() {
        for layout in [KeyboardLayout::Numeric, KeyboardLayout::Letters] {
            let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), panel_size(layout));
            for k in layout_keys(layout, rect) {
                assert!(rect.shrink(PADDING - 0.5).contains_rect(k.rect), "{layout:?} {:?}", k.action);
            }
        }
    }

    #[test]
    fn narrow_field_gets_numbers_wide_field_gets_letters() {
        let narrow = Rect::from_min_size(Pos2::ZERO, Vec2::new(80.0, 22.0));
        let wide = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 22.0));
        assert_eq!(default_layout(narrow), KeyboardLayout::Numeric);
        assert_eq!(default_layout(wide), KeyboardLayout::Letters);
    }

    fn kb_with(layout: KeyboardLayout) -> CompactKeyboard {
        let mut kb = CompactKeyboard::new(true);
        let field = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(80.0, 22.0));
        let rect = place(field, panel_size(layout), screen());
        kb.keys = layout_keys(layout, rect);
        kb.rect = Some(rect);
        kb.focus = Some(Id::new("field"));
        kb.prefs.insert(Id::new("field"), FieldPrefs { layout, system: false });
        kb
    }

    fn center_of(kb: &CompactKeyboard, action: KeyAction) -> Pos2 {
        kb.keys.iter().find(|k| k.action == action).unwrap().rect.center()
    }

    /// Urutan event egui-winit untuk satu ketukan sentuh (jari/Pencil).
    fn tap(pos: Pos2) -> Vec<Event> {
        let button = |pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let touch = |phase| Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: TouchId(7),
            phase,
            pos,
            force: Some(0.3),
        };
        vec![
            touch(TouchPhase::Start),
            Event::PointerMoved(pos),
            button(true),
            touch(TouchPhase::End),
            button(false),
            Event::PointerGone,
        ]
    }

    fn typed(raw: &RawInput) -> String {
        raw.events
            .iter()
            .filter_map(|e| match e {
                Event::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn tap_on_key_types_text_and_is_swallowed() {
        let mut kb = kb_with(KeyboardLayout::Numeric);
        let mut raw = RawInput {
            events: tap(center_of(&kb, KeyAction::Char('7'))),
            ..Default::default()
        };
        kb.filter_input(&mut raw);
        assert_eq!(typed(&raw), "7");
        assert!(
            !raw.events
                .iter()
                .any(|e| matches!(e, Event::PointerButton { .. } | Event::Touch { .. })),
            "tekanan pada keypad tidak boleh sampai ke widget di bawahnya: {:?}",
            raw.events
        );
        assert!(!kb.hardware_typing, "ketikan keypad sendiri bukan keyboard fisik");
    }

    #[test]
    fn backspace_and_enter_send_keys() {
        let mut kb = kb_with(KeyboardLayout::Numeric);
        let mut events = tap(center_of(&kb, KeyAction::Backspace));
        events.extend(tap(center_of(&kb, KeyAction::Enter)));
        let mut raw = RawInput { events, ..Default::default() };
        kb.filter_input(&mut raw);
        let keys: Vec<_> = raw
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Key { key, pressed: true, .. } => Some(*key),
                _ => None,
            })
            .collect();
        assert_eq!(keys, [Key::Backspace, Key::Enter]);
    }

    #[test]
    fn shift_uppercases_next_letter_only() {
        let mut kb = kb_with(KeyboardLayout::Letters);
        let mut events = tap(center_of(&kb, KeyAction::Shift));
        events.extend(tap(center_of(&kb, KeyAction::Char('a'))));
        events.extend(tap(center_of(&kb, KeyAction::Char('b'))));
        let mut raw = RawInput { events, ..Default::default() };
        kb.filter_input(&mut raw);
        assert_eq!(typed(&raw), "Ab");
    }

    #[test]
    fn taps_outside_keypad_pass_through() {
        let mut kb = kb_with(KeyboardLayout::Numeric);
        let events = tap(Pos2::new(900.0, 700.0));
        let mut raw = RawInput { events: events.clone(), ..Default::default() };
        kb.filter_input(&mut raw);
        assert_eq!(raw.events, events);
    }

    #[test]
    fn hardware_typing_hides_keypad() {
        let mut kb = kb_with(KeyboardLayout::Numeric);
        let mut raw = RawInput {
            events: vec![Event::Text("x".into())],
            ..Default::default()
        };
        kb.filter_input(&mut raw);
        assert!(kb.hardware_typing);
    }

    #[test]
    fn system_key_switches_field_to_system_keyboard() {
        let mut kb = kb_with(KeyboardLayout::Numeric);
        let mut raw = RawInput {
            events: tap(center_of(&kb, KeyAction::SystemKeyboard)),
            ..Default::default()
        };
        kb.filter_input(&mut raw);
        assert!(kb.prefs[&Id::new("field")].system);
    }

    #[test]
    fn disabled_keyboard_never_touches_input() {
        let mut kb = kb_with(KeyboardLayout::Numeric);
        kb.enabled = false;
        let events = tap(center_of(&kb, KeyAction::Char('7')));
        let mut raw = RawInput { events: events.clone(), ..Default::default() };
        kb.filter_input(&mut raw);
        assert_eq!(raw.events, events);
    }

    /// Ketikan sungguhan lewat egui: keypad tidak merebut fokus `TextEdit`.
    #[test]
    fn typing_reaches_focused_text_edit() {
        let ctx = Context::default();
        let mut kb = CompactKeyboard::new(true);
        let mut text = String::new();
        // Frame pertama: fokuskan TextEdit.
        ctx.run_ui(RawInput { screen_rect: Some(screen()), ..Default::default() }, |ui| {
            ui.add(egui::TextEdit::singleline(&mut text).desired_width(80.0)).request_focus();
        })
        .textures_delta
        .clear();
        // Frame kedua: TextEdit fokus → keypad muncul.
        let mut raw = RawInput { screen_rect: Some(screen()), ..Default::default() };
        kb.filter_input(&mut raw);
        ctx.run_ui(raw, |ui| {
            ui.add(egui::TextEdit::singleline(&mut text).desired_width(80.0));
            kb.show(ui.ctx());
        })
        .textures_delta
        .clear();
        assert!(kb.rect().is_some(), "keypad harus tampil saat TextEdit fokus");
        for c in ['4', '2'] {
            let pos = center_of(&kb, KeyAction::Char(c));
            let mut raw = RawInput {
                screen_rect: Some(screen()),
                events: tap(pos),
                ..Default::default()
            };
            kb.filter_input(&mut raw);
            ctx.run_ui(raw, |ui| {
                ui.add(egui::TextEdit::singleline(&mut text).desired_width(80.0));
                kb.show(ui.ctx());
            })
        .textures_delta
        .clear();
            assert!(kb.rect().is_some(), "TextEdit kehilangan fokus setelah menekan {c}");
        }
        assert_eq!(text, "42");
    }
}
