#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;

use eframe::egui::IconData;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

pub mod app;
#[cfg(target_vendor = "apple")]
pub mod apple;
pub mod document;
pub mod file_io;
pub mod history_db;
pub mod import_worker;
pub mod input;
pub mod model;
pub mod modeling;
pub mod overlay;
pub mod types;
pub mod ui;
pub mod viewport;

use app::DuCADApp;

fn load_icon() -> IconData {
    let svg_path = "images/logo.svg";
    let png_path = "images/logo.png";
    let ico_path = "images/logo_polos.ico";

    const ICON_TARGET_PX: u32 = 256;
    if let Ok(svg_bytes) = fs::read(svg_path) {
        if let Ok(tree) = Tree::from_data(&svg_bytes, &Options::default()) {
            let intrinsic = tree.size();
            let width = ICON_TARGET_PX;
            let height = ICON_TARGET_PX;
            if let Some(mut pixmap) = Pixmap::new(width, height) {
                let scale_x = width as f32 / intrinsic.width();
                let scale_y = height as f32 / intrinsic.height();
                resvg::render(
                    &tree,
                    Transform::from_scale(scale_x, scale_y),
                    &mut pixmap.as_mut(),
                );
                let rgba = pixmap.data().to_vec();
                return IconData {
                    rgba,
                    width,
                    height,
                };
            }
        }
    }
    if let Ok(png_bytes) = fs::read(png_path) {
        if let Ok(img) = image::load_from_memory(&png_bytes) {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            return IconData {
                rgba: rgba.into_raw(),
                width,
                height,
            };
        }
    }
    if let Ok(ico_bytes) = fs::read(ico_path) {
        if let Ok(img) = image::load_from_memory(&ico_bytes) {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            return IconData {
                rgba: rgba.into_raw(),
                width,
                height,
            };
        }
    }
    IconData {
        rgba: Vec::new(),
        width: 0,
        height: 0,
    }
}

fn main() -> eframe::Result {
    env_logger::init();

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 32,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("DUCAD")
            .with_inner_size([1640.0, 900.0])
            .with_icon(load_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "DUCAD",
        options,
        Box::new(|cc| Ok(Box::new(DuCADApp::new(cc)))),
    )
}
