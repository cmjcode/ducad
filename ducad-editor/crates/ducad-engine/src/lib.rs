//! `ducad-engine` — lapisan modeling headless DUCAD.
//!
//! Crate ini menyatukan sketch (`ducad-sketch`), kernel (`ducad-kernel`),
//! dan I/O (`ducad-io`) TANPA egui/wgpu, sehingga operasi modeling yang
//! sama bisa dipanggil dari GUI, CLI, maupun server MCP.

// `OpError` sengaja "gemuk" (pesan, hint, id op, context JSON): bentuknya
// adalah kontrak JSON CLI/MCP (P0.6) dan jalur error jarang dilalui, jadi
// biaya memindahkannya lewat `Result` tidak berarti dibanding operasi kernel
// di baliknya. Membungkusnya dalam `Box` hanya demi lint ini akan mengubah
// tipe publik kontrak.
#![allow(clippy::result_large_err)]
// Prinsip "jangan pernah crash" (ADR 0009): kode produksi engine tidak boleh
// panik lewat unwrap/expect — input agent (JSON, berkas, argumen tool) bersifat
// hostil. Modul tes boleh (assert lewat unwrap wajar di sana).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod brief;
pub mod check;
pub mod compute;
pub mod datum;
pub mod diagnose;
pub mod diff;
pub mod drawing_auto;
pub mod error;
pub mod export;
pub mod firm;
pub mod guard;
pub mod inspect;
pub mod migrate;
pub mod model;
pub mod oplog;
pub mod package;
pub mod ops;
pub mod plane;
pub mod profile;
pub mod render;
pub mod select;
pub mod session;
pub mod sim;
pub mod tooling;

pub use ducad_core::{Feature, LicenseTier};
pub use error::{apply_patch, OpError, OpErrorCode, OpPatch, OpResult, SuggestedFix};
pub use ops::MaterialSel;
pub use plane::PlaneFrame;
pub use session::{
    edit_design, preview_configuration, preview_edit, resolve_material, BatchReport, DesignDoc, EditPreview, OpOutcome,
    Proposal, ReplaceOp, Session, SessionCore, SessionMeta,
};

#[test]
fn engine_has_no_gui_dependency() {
    let manifest = include_str!("../Cargo.toml");
    for banned in ["egui", "eframe", "wgpu", "ducad-render", "ducad-ui", "rfd"] {
        assert!(!manifest.contains(banned), "ducad-engine tidak boleh bergantung pada {banned}");
    }
}
