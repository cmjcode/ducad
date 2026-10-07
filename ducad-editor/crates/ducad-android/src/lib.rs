//! Entry point Android tablet: `android_main` dipanggil `GameActivity`
//! (lihat `android/app/src/main/java/id/ducad/studio/MainActivity.kt`).
//!
//! Yang dilakukan sebelum eframe berjalan:
//! 1. `HOME` → folder data internal aplikasi, supaya seluruh jalur
//!    `$HOME/.ducad` (riwayat, konfigurasi chat, kunci API) bekerja.
//! 2. `DUCAD_DOCUMENTS_DIR` → `Android/data/<paket>/files`, folder yang
//!    terlihat pengguna lewat aplikasi Files; dipakai `platform::documents_dir`.
//! 3. Log ke logcat (tag `ducad`).

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("ducad"),
    );

    if let Some(internal) = app.internal_data_path() {
        // Edisi 2021: `set_var` aman; dipanggil sebelum ada thread lain.
        std::env::set_var("HOME", &internal);
        let _ = std::fs::create_dir_all(internal.join(".ducad"));
    }
    if let Some(external) = app.external_data_path() {
        let _ = std::fs::create_dir_all(&external);
        std::env::set_var(ducad_app::platform::DOCUMENTS_DIR_ENV, &external);
    }

    let mut options = ducad_app::native_options(None);
    options.android_app = Some(app);
    if let Err(err) = eframe::run_native(
        "DUCAD",
        options,
        Box::new(|cc| Ok(Box::new(ducad_app::DuCADApp::new(cc)))),
    ) {
        log::error!("DUCAD berhenti: {err}");
    }
}
