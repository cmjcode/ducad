//! `ducad-engine` — lapisan modeling headless DUCAD.
//!
//! Crate ini menyatukan sketch (`ducad-sketch`), kernel (`ducad-kernel`),
//! dan I/O (`ducad-io`) TANPA egui/wgpu, sehingga operasi modeling yang
//! sama bisa dipanggil dari GUI, CLI, maupun server MCP.

// `OpError` sengaja "gemuk" (pesan, hint, id op, context JSON): bentuknya
// adalah kontrak JSON CLI/MCP (P0.6) dan jalur error jarang dilalui, jadi
// biaya memindahkannya lewat `Result` tidak berarti dibanding operasi OCCT
// di baliknya. Membungkusnya dalam `Box` hanya demi lint ini akan mengubah
// tipe publik kontrak.
#![allow(clippy::result_large_err)]

pub mod check;
pub mod compute;
pub mod diagnose;
pub mod diff;
pub mod drawing_auto;
pub mod error;
pub mod export;
pub mod inspect;
pub mod model;
pub mod oplog;
pub mod ops;
pub mod plane;
pub mod profile;
pub mod render;
pub mod select;
pub mod session;
pub mod tooling;

pub use error::{apply_patch, OpError, OpErrorCode, OpPatch, OpResult, SuggestedFix};
pub use plane::PlaneFrame;
pub use session::{
    BatchReport, DesignDoc, OpOutcome, Proposal, ReplaceOp, Session, SessionCore, SessionMeta,
};

#[test]
fn engine_has_no_gui_dependency() {
    let manifest = include_str!("../Cargo.toml");
    for banned in ["egui", "eframe", "wgpu", "ducad-render", "ducad-ui", "rfd"] {
        assert!(!manifest.contains(banned), "ducad-engine tidak boleh bergantung pada {banned}");
    }
}
