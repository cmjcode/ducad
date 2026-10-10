//! DUCAD GUI sebagai pustaka: dipakai oleh binary desktop (`main.rs`) dan
//! oleh `ducad-android` (`cdylib` dengan `android_main`). Semua modul
//! aplikasi hidup di sini supaya kedua entry point berbagi kode yang sama.

pub mod agent_bridge;
pub mod app;
#[cfg(target_vendor = "apple")]
pub mod apple;
#[cfg(target_os = "ios")]
pub mod apple_ios;
pub mod assist_ui;
pub mod chat_cli;
pub mod chat_history;
pub mod chat_ui;
pub mod checks_ui;
pub mod closed_objects;
pub mod document;
pub mod error_card_ui;
pub mod file_io;
pub mod freehand;
pub mod history_db;
pub mod import_worker;
pub mod industry_ui;
pub mod ink;
pub mod input;
pub mod live_tools;
pub mod mass_ui;
/// Memori MNEMONIC tertaut langsung (P11.5); lihat fitur `memory`.
#[cfg(feature = "memory")]
pub mod memory;
pub mod mobile;
pub mod mode;
pub mod model;
pub mod modeling;
pub mod onboarding_ui;
pub mod overlay;
pub mod platform;
pub mod proposal_ui;
pub mod sim_ui;
pub mod types;
pub mod ui;
pub mod vector;
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
