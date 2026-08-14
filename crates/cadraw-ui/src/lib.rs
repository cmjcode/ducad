//! Komponen egui bersama CADRAW (Fase 4): command palette, radial menu
//! untuk sentuh, tema + target sentuh minimum 44 pt. Toolbar kontekstual
//! itu sendiri tetap tinggal di `cadraw-app` (susunannya bergantung state
//! app: tool aktif, seleksi, dst) — crate ini menyediakan komponen
//! generik yang platform/app-agnostic (cuma bergantung `egui`), supaya
//! nanti shell iPad (Fase 6) bisa memakainya lagi tanpa mengulang.

pub mod command_palette;
pub mod radial_menu;
pub mod theme;

pub use command_palette::CommandPalette;
pub use radial_menu::RadialMenu;
pub use theme::{apply as apply_theme, ThemeMode, MIN_TOUCH_TARGET};
