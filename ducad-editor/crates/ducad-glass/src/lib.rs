//! Material Liquid Glass untuk UI DUCAD.
//!
//! Panel tetap transparan, tetapi konten di baliknya dibelokkan di tepi
//! (lensa), diburamkan, diberi tint, dan diberi sorotan spekular di rim —
//! sehingga teks di atasnya tetap terbaca tanpa harus memekatkan panel.
pub mod backdrop;
pub mod material;
pub mod sdf;
pub mod widget;

pub use backdrop::{GlassBackdrop, PanelUniform};
pub use material::{GlassMaterial, GlassMode, GlassPreset};
pub use widget::{runtime, set_runtime, GlassFrame, GlassRuntime};

#[cfg(test)]
mod tests {
    /// `ducad-glass` berada DI BAWAH `ducad-ui`/`ducad-app`: ia hanya boleh
    /// mengenal egui + egui_wgpu, supaya bisa dipakai keduanya tanpa siklus.
    #[test]
    fn glass_has_no_app_or_ui_dependency() {
        let manifest = include_str!("../Cargo.toml");
        let deps = manifest
            .split("[dependencies]")
            .nth(1)
            .and_then(|s| s.split("[dev-dependencies]").next())
            .unwrap_or_default();
        // Hanya baris dependensi; komentar boleh menyebut crate lain.
        let deps: String = deps
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(deps.contains("egui"), "bagian [dependencies] tidak terbaca");
        for banned in [
            "ducad-ui",
            "ducad-app",
            "ducad-render",
            "ducad-kernel",
            "eframe",
        ] {
            assert!(
                !deps.contains(banned),
                "ducad-glass tidak boleh bergantung pada {banned}"
            );
        }
    }
}
