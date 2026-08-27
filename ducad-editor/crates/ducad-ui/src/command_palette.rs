//! Command palette gaya VS Code/Spotlight — Ctrl/Cmd+K, ketik untuk cari
//! aksi (semua tool, undo/redo, dst), Enter untuk eksekusi. Pelengkap
//! toolbar/shortcut huruf tunggal: cocok untuk aksi yang jarang dipakai
//! (tak pantas punya tombol toolbar sendiri) dan untuk device tanpa
//! keyboard fisik penuh (radial menu Fase 4 menutupi kasus sentuh cepat,
//! palette ini untuk pencarian aksi apa saja).
//!
//! Widget ini generik terhadap "apa aksinya" — caller (`ducad-app`)
//! menyediakan daftar `(label, hint)` tiap frame dan menerima index balik
//! ke daftar yang sama saat satu entri dieksekusi, lalu memutuskan sendiri
//! aksi konkretnya lewat match. Pola yang sama dengan `RadialMenu`.

use ducad_i18n::t;
use egui::{Align2, Key, RichText};

#[derive(Default)]
pub struct CommandPalette {
    open: bool,
    query: String,
    highlighted: usize,
    focus_pending: bool,
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

    /// Render overlay bila terbuka. `entries`: `(label, hint)` — hint mis.
    /// shortcut huruf, ditampilkan redup rata-kanan (boleh string kosong).
    /// Return `Some(index)` ke `entries` ASLI (bukan indeks hasil filter)
    /// saat satu entri dieksekusi (klik atau Enter di baris tersorot);
    /// palette otomatis tertutup setelahnya. Pencarian: substring
    /// case-insensitive sederhana, cukup untuk jumlah aksi DUCAD saat ini
    /// (puluhan, bukan ribuan) — fuzzy-match sungguhan bisa menyusul kalau
    /// daftar aksi membengkak.
    pub fn show(&mut self, ctx: &egui::Context, entries: &[(&str, &str)]) -> Option<usize> {
        if !self.open {
            return None;
        }
        if entries.is_empty() {
            self.close();
            return None;
        }

        let query_lower = self.query.to_lowercase();
        let filtered: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, (label, _))| {
                query_lower.is_empty() || label.to_lowercase().contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect();
        if !filtered.is_empty() {
            self.highlighted = self.highlighted.min(filtered.len() - 1);
        }

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.close();
            return None;
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowDown)) && !filtered.is_empty() {
            self.highlighted = (self.highlighted + 1).min(filtered.len() - 1);
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
            self.highlighted = self.highlighted.saturating_sub(1);
        }
        let enter_pressed = ctx.input(|i| i.key_pressed(Key::Enter));

        let mut result = None;

        egui::Area::new(egui::Id::new("ducad-command-palette"))
            .anchor(Align2::CENTER_TOP, egui::vec2(0.0, 72.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.set_min_width(380.0);
                        let resp = ui.text_edit_singleline(&mut self.query);
                        if self.focus_pending {
                            resp.request_focus();
                            self.focus_pending = false;
                        }
                        if resp.changed() {
                            self.highlighted = 0;
                        }

                        ui.separator();
                        egui::ScrollArea::vertical()
                            .max_height(280.0)
                            .show(ui, |ui| {
                                for (row, &orig_idx) in filtered.iter().enumerate() {
                                    let (label, hint) = entries[orig_idx];
                                    let selected = row == self.highlighted;
                                    ui.horizontal(|ui| {
                                        let resp = ui.selectable_label(selected, label);
                                        if !hint.is_empty() {
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.label(RichText::new(hint).weak());
                                                },
                                            );
                                        }
                                        if resp.clicked() || (selected && enter_pressed) {
                                            result = Some(orig_idx);
                                        }
                                    });
                                }
                                if filtered.is_empty() {
                                    ui.weak(t!("cmd-no-match"));
                                }
                            });
                    });
            });

        if result.is_some() {
            self.close();
        }
        result
    }
}
