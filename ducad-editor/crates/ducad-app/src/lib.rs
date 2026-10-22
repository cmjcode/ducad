//! DUCAD GUI sebagai pustaka: dipakai oleh binary desktop (`main.rs`) dan
//! oleh `ducad-android` (`cdylib` dengan `android_main`). Semua modul
//! aplikasi hidup di sini supaya kedua entry point berbagi kode yang sama.

// Prinsip "jangan pernah crash" (ADR 0009): tidak ada unwrap/expect/panic di
// kode produksi; modul tes dikecualikan.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
pub mod agent_bridge;
pub mod app;
#[cfg(target_vendor = "apple")]
pub mod apple;
#[cfg(target_os = "ios")]
pub mod apple_ios;
pub mod assist_ui;
pub mod assembly_tools;
pub mod avatar;
pub mod body_tools;
pub mod chat_cli;
pub mod chat_history;
pub mod brief_studio_ui;
pub mod chat_ui;
pub mod checks_ui;
pub mod clipboard;
pub mod closed_objects;
pub mod cloud_ui;
pub mod constraint_glyphs;
pub mod crash_reports;
pub mod datum_tools;
pub mod design_tools;
pub mod direct_tools;
pub mod document;
pub mod error_card_ui;
pub mod file_io;
pub mod freehand;
pub mod history_db;
pub mod import_worker;
pub mod industry_ui;
pub mod ink;
pub mod input;
pub mod inspect_tools;
pub mod interop;
pub mod live_tools;
pub mod logging;
pub mod loose_ends;
pub mod mass_ui;
/// Memori MNEMONIC tertaut langsung (P11.5); lihat fitur `memory`.
#[cfg(feature = "memory")]
pub mod memory;
pub mod mobile;
pub mod mode;
pub mod motion_tools;
pub mod named_views;
pub mod model;
pub mod modeling;
pub mod next_action;
pub mod onboarding_ui;
pub mod oplog_direct;
pub mod oplog_gui;
pub mod oplog_undo;
pub mod overlay;
pub mod plastic_tools;
pub mod platform;
pub mod proposal_ui;
pub mod recovery;
pub mod review_ui;
pub mod sim_ui;
pub mod surface_tools;
pub mod svg_insert;
pub mod sketch_break;
pub mod slicers;
pub mod sketch_constraints;
pub mod sketch_dims;
pub mod sketch_points;
pub mod sketch_project;
pub mod sketch_status_ui;
pub mod sketch_modes;
pub mod sketch_modes_fase1;
pub mod text_edit;
pub mod timeline;
pub mod types;
pub mod ui;
pub mod update_ui;
pub mod vector;
pub mod view_tools;
pub mod viewport;

pub use app::DuCADApp;

/// Opsi eframe bersama desktop/mobile. `icon` hanya relevan di desktop;
/// di iPadOS/Android ikon berasal dari bundel aplikasi.
pub fn native_options(icon: Option<eframe::egui::IconData>) -> eframe::NativeOptions {
    // Kelas ObjC delegate scene harus terdaftar SEBELUM `UIApplicationMain`
    // (dipanggil `eframe::run_native`) mencari `UISceneDelegateClassName`.
    #[cfg(target_os = "ios")]
    apple_ios::register_scene_delegate();

    let mut viewport = eframe::egui::ViewportBuilder::default().with_title("DUCAD");
    if platform::is_mobile() {
        // Jendela selalu memenuhi layar di tablet; ukuran awal desktop tidak
        // relevan dan hanya memicu satu resize tambahan saat start.
        viewport = viewport.with_maximized(true);
    } else {
        viewport = viewport.with_inner_size([1640.0, 900.0]);
    }
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 32,
        viewport,
        // Posisi/ukuran jendela desktop tidak dipulihkan dari sesi
        // sebelumnya (perilaku lama); persistence dipakai hanya untuk
        // `App::save` (autosave saat aplikasi ditidurkan di tablet).
        persist_window: false,
        ..Default::default()
    }
}
