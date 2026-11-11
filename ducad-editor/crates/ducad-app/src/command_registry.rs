//! Registri perintah & pintasan papan ketik tunggal (Tahap 3).
//!
//! Menggantikan tabel statis dan pemeriksaan `key_pressed` yang tersebar dengan
//! satu sumber kebenaran yang:
//! 1. menyelaraskan label pintasan di Command Palette, Burger Menu, dan handler;
//! 2. mendukung penyesuaian ulang (*rebinding*) lewat `prefs.json` (`keybindings`);
//! 3. memblokir seluruh pintasan huruf tunggal / `Shift+Huruf` saat fokus berada
//!    di kotak teks agar mengetik `Z`, `T`, `S`, atau `P` tidak pernah memicu alat.

use std::collections::BTreeMap;

use eframe::egui::{self, Key};

use crate::mode::AppMode;

/// Cakupan konteks tempat pintasan aktif.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShortcutScope {
    /// Berlaku di seluruh mode editor.
    Global,
    /// Berlaku saat kanvas dalam mode sketsa 2D standar (bukan Vektor).
    SketchCad,
    /// Berlaku khusus di Mode Vektor 2D.
    Vector2d,
    /// Berlaku di semua mode 2D (`is_sketching == true`).
    Any2d,
    /// Berlaku hanya di mode Solid 3D (`!is_sketching`).
    Solid3d,
}

impl ShortcutScope {
    pub fn matches(self, is_sketching: bool, mode: AppMode) -> bool {
        match self {
            ShortcutScope::Global => true,
            ShortcutScope::SketchCad => is_sketching && mode != AppMode::Vector,
            ShortcutScope::Vector2d => is_sketching && mode == AppMode::Vector,
            ShortcutScope::Any2d => is_sketching,
            ShortcutScope::Solid3d => !is_sketching,
        }
    }

    pub fn overlaps(self, other: Self) -> bool {
        for &(sk, mode) in &[
            (true, AppMode::Sketch),
            (true, AppMode::Vector),
            (true, AppMode::Ink),
            (false, AppMode::Solid),
        ] {
            if self.matches(sk, mode) && other.matches(sk, mode) {
                return true;
            }
        }
        false
    }
}

/// Kombinasi tombol pintasan yang sudah dinormalisasi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortcutChord {
    pub key: Key,
    pub cmd: bool,
    pub alt: bool,
    pub shift: bool,
}

impl ShortcutChord {
    pub const fn plain(key: Key) -> Self {
        Self {
            key,
            cmd: false,
            alt: false,
            shift: false,
        }
    }

    pub const fn shift(key: Key) -> Self {
        Self {
            key,
            cmd: false,
            alt: false,
            shift: true,
        }
    }

    pub const fn cmd(key: Key) -> Self {
        Self {
            key,
            cmd: true,
            alt: false,
            shift: false,
        }
    }

    pub const fn cmd_shift(key: Key) -> Self {
        Self {
            key,
            cmd: true,
            alt: false,
            shift: true,
        }
    }

    pub const fn cmd_alt(key: Key) -> Self {
        Self {
            key,
            cmd: true,
            alt: true,
            shift: false,
        }
    }

    /// Format ringkas untuk badge UI (mis. `⌘D`, `⌘+Shift+S`, `Shift+T`, `L`).
    pub fn format_badge(self) -> String {
        let key_s = key_display_name(self.key);
        if self.cmd && !self.alt && !self.shift && key_s.len() == 1 {
            return format!("⌘{key_s}");
        }
        let mut parts: Vec<&str> = Vec::new();
        if self.cmd {
            parts.push("⌘");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.push(key_s);
        parts.join("+")
    }

    /// Format kanonik ASCII untuk serialisasi `prefs.json` (mis. `Cmd+Shift+S`).
    pub fn to_config_string(self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.cmd {
            parts.push("Cmd");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.push(key_display_name(self.key));
        parts.join("+")
    }

    /// Urai string konfigurasi seperti `"L"`, `"Shift+T"`, `"Cmd+D"`, `"⌘+Alt+2"`.
    pub fn parse(spec: &str) -> Option<Self> {
        let trimmed = spec.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut cmd = false;
        let mut alt = false;
        let mut shift = false;
        let mut key: Option<Key> = None;

        // Dukung bentuk singkat `"⌘S"` / `"⌘D"`.
        if let Some(rest) = trimmed.strip_prefix('⌘') {
            cmd = true;
            let rest = rest.strip_prefix('+').unwrap_or(rest);
            if !rest.is_empty() && !rest.contains('+') {
                let k = parse_key_token(rest)?;
                return Some(Self {
                    key: k,
                    cmd,
                    alt,
                    shift,
                });
            }
        }

        for raw in trimmed.split('+') {
            let tok = raw.trim();
            if tok.is_empty() {
                continue;
            }
            match tok.to_ascii_lowercase().as_str() {
                "cmd" | "ctrl" | "command" | "meta" | "⌘" => cmd = true,
                "alt" | "opt" | "option" => alt = true,
                "shift" | "⇧" => shift = true,
                _ => {
                    if key.is_some() {
                        return None;
                    }
                    key = Some(parse_key_token(tok)?);
                }
            }
        }

        Some(Self {
            key: key?,
            cmd,
            alt,
            shift,
        })
    }

    pub fn matches_input(self, input: &egui::InputState, text_focus: bool) -> bool {
        // Pintasan tanpa modifier Command/Ctrl tidak boleh bocor saat mengetik teks.
        if !self.cmd && text_focus {
            return false;
        }
        let cmd_down = input.modifiers.command || input.modifiers.ctrl;
        if cmd_down != self.cmd {
            return false;
        }
        if input.modifiers.alt != self.alt {
            return false;
        }
        if input.modifiers.shift != self.shift {
            return false;
        }
        input.key_pressed(self.key)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CommandBindingSpec {
    pub id: &'static str,
    pub scope: ShortcutScope,
    pub default_chord: Option<ShortcutChord>,
}

/// Tabel seluruh ID perintah standar beserta pintasan bawaannya.
pub const DEFAULT_BINDINGS: &[CommandBindingSpec] = &[
    // Global — File, AI, Undo/Redo, Mode
    CommandBindingSpec {
        id: "ai.chat",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::cmd_shift(Key::A)),
    },
    CommandBindingSpec {
        id: "file.open",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::cmd(Key::O)),
    },
    CommandBindingSpec {
        id: "file.save",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::cmd(Key::S)),
    },
    CommandBindingSpec {
        id: "file.save_as",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::cmd_shift(Key::S)),
    },
    CommandBindingSpec {
        id: "file.drawing_sheet",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::cmd(Key::D)),
    },
    CommandBindingSpec {
        id: "edit.undo",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::cmd(Key::Z)),
    },
    CommandBindingSpec {
        id: "edit.redo",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::cmd_shift(Key::Z)),
    },
    CommandBindingSpec {
        id: "mode.sketch",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::cmd_alt(Key::Num2)),
    },
    CommandBindingSpec {
        id: "mode.solid",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::cmd_alt(Key::Num3)),
    },
    CommandBindingSpec {
        id: "mode.vector",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::cmd_alt(Key::Num4)),
    },
    CommandBindingSpec {
        id: "mode.ink",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::cmd_alt(Key::Num5)),
    },
    CommandBindingSpec {
        id: "tool.datum_plane",
        scope: ShortcutScope::Global,
        default_chord: Some(ShortcutChord::shift(Key::P)),
    },
    // 2D Sketch & Vector shared (`Any2d`)
    CommandBindingSpec {
        id: "tool.line",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::L)),
    },
    CommandBindingSpec {
        id: "tool.rectangle",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::R)),
    },
    CommandBindingSpec {
        id: "tool.circle",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::C)),
    },
    CommandBindingSpec {
        id: "tool.ellipse",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::E)),
    },
    CommandBindingSpec {
        id: "tool.extend",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::shift(Key::E)),
    },
    CommandBindingSpec {
        id: "tool.polygon",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::Y)),
    },
    CommandBindingSpec {
        id: "tool.arc",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::A)),
    },
    CommandBindingSpec {
        id: "tool.offset",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::O)),
    },
    CommandBindingSpec {
        id: "tool.mirror",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::M)),
    },
    CommandBindingSpec {
        id: "tool.fillet_2d",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::F)),
    },
    CommandBindingSpec {
        id: "tool.freehand",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::shift(Key::F)),
    },
    CommandBindingSpec {
        id: "tool.spline",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::S)),
    },
    CommandBindingSpec {
        id: "tool.revolve_2d",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::V)),
    },
    CommandBindingSpec {
        id: "sketch.toggle_construction",
        scope: ShortcutScope::Any2d,
        default_chord: Some(ShortcutChord::plain(Key::X)),
    },
    // Sketch CAD spesifik (`SketchCad`) — memisahkan T/Shift+T dan P
    CommandBindingSpec {
        id: "tool.trim",
        scope: ShortcutScope::SketchCad,
        default_chord: Some(ShortcutChord::plain(Key::T)),
    },
    CommandBindingSpec {
        id: "tool.text",
        scope: ShortcutScope::SketchCad,
        default_chord: Some(ShortcutChord::shift(Key::T)),
    },
    CommandBindingSpec {
        id: "sketch.dimension",
        scope: ShortcutScope::SketchCad,
        default_chord: Some(ShortcutChord::plain(Key::D)),
    },
    CommandBindingSpec {
        id: "tool.pattern_2d",
        scope: ShortcutScope::SketchCad,
        default_chord: Some(ShortcutChord::plain(Key::P)),
    },
    // Vector 2D spesifik (`Vector2d`)
    CommandBindingSpec {
        id: "vector.pen_bezier",
        scope: ShortcutScope::Vector2d,
        default_chord: Some(ShortcutChord::plain(Key::P)),
    },
    CommandBindingSpec {
        id: "vector.node_edit",
        scope: ShortcutScope::Vector2d,
        default_chord: Some(ShortcutChord::plain(Key::N)),
    },
    CommandBindingSpec {
        id: "vector.eyedropper",
        scope: ShortcutScope::Vector2d,
        default_chord: Some(ShortcutChord::plain(Key::I)),
    },
    CommandBindingSpec {
        id: "vector.gradient",
        scope: ShortcutScope::Vector2d,
        default_chord: Some(ShortcutChord::plain(Key::G)),
    },
    CommandBindingSpec {
        id: "vector.text",
        scope: ShortcutScope::Vector2d,
        default_chord: Some(ShortcutChord::plain(Key::T)),
    },
    // Solid 3D spesifik (`Solid3d`)
    CommandBindingSpec {
        id: "solid.enter_sketch",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::S)),
    },
    CommandBindingSpec {
        id: "tool.extrude",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::E)),
    },
    CommandBindingSpec {
        id: "tool.revolve",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::V)),
    },
    CommandBindingSpec {
        id: "tool.boolean",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::B)),
    },
    CommandBindingSpec {
        id: "tool.hole_wizard",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::W)),
    },
    CommandBindingSpec {
        id: "tool.shell",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::H)),
    },
    CommandBindingSpec {
        id: "tool.pattern",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::P)),
    },
    CommandBindingSpec {
        id: "tool.draft_angle",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::D)),
    },
    CommandBindingSpec {
        id: "tool.helix",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::G)),
    },
    CommandBindingSpec {
        id: "tool.rib",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::R)),
    },
    CommandBindingSpec {
        id: "view.zebra",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::Z)),
    },
    CommandBindingSpec {
        id: "view.studio_lighting",
        scope: ShortcutScope::Solid3d,
        default_chord: Some(ShortcutChord::plain(Key::L)),
    },
];

/// Registri perintah aktif beserta override dari preferensi pengguna.
#[derive(Debug, Clone, Default)]
pub struct CommandRegistry {
    /// `Some(None)` = pintasan dimatikan pengguna (`""`); `Some(Some(c))` = diubah.
    overrides: BTreeMap<String, Option<ShortcutChord>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Terapkan peta `id -> spec` dari `UserPrefs.keybindings`.
    pub fn apply_user_map(&mut self, map: &BTreeMap<String, String>) {
        self.overrides.clear();
        for (id, raw) in map {
            if DEFAULT_BINDINGS.iter().any(|b| b.id == id.as_str()) {
                if raw.trim().is_empty() {
                    self.overrides.insert(id.clone(), None);
                } else if let Some(chord) = ShortcutChord::parse(raw) {
                    self.overrides.insert(id.clone(), Some(chord));
                }
            }
        }
    }

    /// Ekspor override aktif ke bentuk `BTreeMap<String, String>` untuk `prefs.json`.
    pub fn export_user_map(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        for (id, val) in &self.overrides {
            match val {
                Some(chord) => {
                    out.insert(id.clone(), chord.to_config_string());
                }
                None => {
                    out.insert(id.clone(), String::new());
                }
            }
        }
        out
    }

    /// Ubah pintasan untuk satu `id` (`""` untuk menghapus pintasan).
    pub fn rebind(&mut self, id: &str, spec: &str) -> Result<(), String> {
        let Some(def) = DEFAULT_BINDINGS.iter().find(|b| b.id == id) else {
            return Err(format!("Unknown command id: {id}"));
        };
        if spec.trim().is_empty() {
            self.overrides.insert(id.to_string(), None);
            return Ok(());
        }
        let Some(chord) = ShortcutChord::parse(spec) else {
            return Err(format!("Invalid shortcut chord: {spec}"));
        };
        if Some(chord) == def.default_chord {
            self.overrides.remove(id);
        } else {
            self.overrides.insert(id.to_string(), Some(chord));
        }
        Ok(())
    }

    pub fn chord_for(&self, id: &str) -> Option<(ShortcutScope, ShortcutChord)> {
        let spec = DEFAULT_BINDINGS.iter().find(|b| b.id == id)?;
        let chord = match self.overrides.get(id) {
            Some(override_opt) => *override_opt,
            None => spec.default_chord,
        }?;
        Some((spec.scope, chord))
    }

    /// String badge pintasan untuk UI (kosong bila tidak terikat).
    pub fn hint_for(&self, id: &str) -> String {
        self.chord_for(id)
            .map(|(_, c)| c.format_badge())
            .unwrap_or_default()
    }

    /// Periksa apakah perintah `id` baru saja ditekan pada frame ini.
    pub fn is_pressed(
        &self,
        id: &str,
        input: &egui::InputState,
        text_focus: bool,
        is_sketching: bool,
        app_mode: AppMode,
    ) -> bool {
        let Some((scope, chord)) = self.chord_for(id) else {
            return false;
        };
        if !scope.matches(is_sketching, app_mode) {
            return false;
        }
        chord.matches_input(input, text_focus)
    }
}

fn key_display_name(key: Key) -> &'static str {
    match key {
        Key::A => "A",
        Key::B => "B",
        Key::C => "C",
        Key::D => "D",
        Key::E => "E",
        Key::F => "F",
        Key::G => "G",
        Key::H => "H",
        Key::I => "I",
        Key::J => "J",
        Key::K => "K",
        Key::L => "L",
        Key::M => "M",
        Key::N => "N",
        Key::O => "O",
        Key::P => "P",
        Key::Q => "Q",
        Key::R => "R",
        Key::S => "S",
        Key::T => "T",
        Key::U => "U",
        Key::V => "V",
        Key::W => "W",
        Key::X => "X",
        Key::Y => "Y",
        Key::Z => "Z",
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        Key::Escape => "Esc",
        Key::Enter => "Enter",
        Key::Tab => "Tab",
        Key::Space => "Space",
        Key::Delete => "Del",
        Key::Backspace => "Backspace",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Minus => "-",
        Key::Equals => "=",
        Key::Plus => "+",
        _ => "Key",
    }
}

fn parse_key_token(tok: &str) -> Option<Key> {
    match tok.to_ascii_uppercase().as_str() {
        "A" => Some(Key::A),
        "B" => Some(Key::B),
        "C" => Some(Key::C),
        "D" => Some(Key::D),
        "E" => Some(Key::E),
        "F" => Some(Key::F),
        "G" => Some(Key::G),
        "H" => Some(Key::H),
        "I" => Some(Key::I),
        "J" => Some(Key::J),
        "K" => Some(Key::K),
        "L" => Some(Key::L),
        "M" => Some(Key::M),
        "N" => Some(Key::N),
        "O" => Some(Key::O),
        "P" => Some(Key::P),
        "Q" => Some(Key::Q),
        "R" => Some(Key::R),
        "S" => Some(Key::S),
        "T" => Some(Key::T),
        "U" => Some(Key::U),
        "V" => Some(Key::V),
        "W" => Some(Key::W),
        "X" => Some(Key::X),
        "Y" => Some(Key::Y),
        "Z" => Some(Key::Z),
        "0" => Some(Key::Num0),
        "1" => Some(Key::Num1),
        "2" => Some(Key::Num2),
        "3" => Some(Key::Num3),
        "4" => Some(Key::Num4),
        "5" => Some(Key::Num5),
        "6" => Some(Key::Num6),
        "7" => Some(Key::Num7),
        "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),
        "ESC" | "ESCAPE" => Some(Key::Escape),
        "ENTER" | "RETURN" => Some(Key::Enter),
        "TAB" => Some(Key::Tab),
        "SPACE" => Some(Key::Space),
        "DEL" | "DELETE" => Some(Key::Delete),
        "BACKSPACE" => Some(Key::Backspace),
        "[" => Some(Key::OpenBracket),
        "]" => Some(Key::CloseBracket),
        "-" => Some(Key::Minus),
        "=" => Some(Key::Equals),
        "+" => Some(Key::Plus),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings_have_no_overlapping_collisions() {
        for (i, a) in DEFAULT_BINDINGS.iter().enumerate() {
            let Some(ca) = a.default_chord else { continue };
            for b in &DEFAULT_BINDINGS[i + 1..] {
                let Some(cb) = b.default_chord else { continue };
                if ca == cb {
                    assert!(
                        !a.scope.overlaps(b.scope),
                        "Shortcut collision {:?} between {} ({:?}) and {} ({:?})",
                        ca,
                        a.id,
                        a.scope,
                        b.id,
                        b.scope
                    );
                }
            }
        }
    }

    #[test]
    fn text_focus_blocks_plain_and_shift_shortcuts_but_allows_cmd() {
        let reg = CommandRegistry::new();
        let mut input = egui::InputState::default();
        input.events.push(egui::Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });

        // Di Solid 3D tanpa fokus teks: Z menyalakan Zebra.
        assert!(reg.is_pressed("view.zebra", &input, false, false, AppMode::Solid));
        // Saat fokus di kotak teks: Z diblokir!
        assert!(!reg.is_pressed("view.zebra", &input, true, false, AppMode::Solid));
    }

    #[test]
    fn rebind_updates_badge_and_roundtrips_prefs_map() {
        let mut reg = CommandRegistry::new();
        assert_eq!(reg.hint_for("file.drawing_sheet"), "⌘D");
        assert_eq!(reg.hint_for("tool.text"), "Shift+T");
        assert_eq!(reg.hint_for("tool.datum_plane"), "Shift+P");

        reg.rebind("view.zebra", "Shift+Z").unwrap();
        assert_eq!(reg.hint_for("view.zebra"), "Shift+Z");

        let exported = reg.export_user_map();
        let mut reg2 = CommandRegistry::new();
        reg2.apply_user_map(&exported);
        assert_eq!(reg2.hint_for("view.zebra"), "Shift+Z");
    }
}
