//! `ducad-cli` — antarmuka baris perintah headless DUCAD untuk agent dan CI.
//!
//! stdout hanya berisi hasil (JSON untuk laporan / `--json`); semua log ke
//! stderr. Kode keluar: 0 sukses · 1 operasi/replay gagal (laporan tetap
//! ditulis) · 2 salah pakai argumen atau I/O · 3 ada check yang gagal (P7).

mod cmd;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Kode keluar CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Ok = 0,
    OpFailed = 1,
    Usage = 2,
    ChecksFailed = 3,
}

/// Error CLI beserta kode keluarnya.
#[derive(Debug)]
pub struct CliError {
    pub exit: Exit,
    pub message: String,
}

impl CliError {
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            exit: Exit::Usage,
            message: message.into(),
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            exit: Exit::OpFailed,
            message: message.into(),
        }
    }
}

impl From<ducad_engine::OpError> for CliError {
    fn from(e: ducad_engine::OpError) -> Self {
        let exit = match e.code {
            ducad_engine::OpErrorCode::Io | ducad_engine::OpErrorCode::InvalidParam => Exit::Usage,
            _ => Exit::OpFailed,
        };
        let json = serde_json::to_string(&e).unwrap_or_else(|_| e.message.clone());
        Self {
            exit,
            message: json,
        }
    }
}

pub type CliResult = Result<Exit, CliError>;

#[derive(Parser)]
#[command(
    name = "ducad-cli",
    version,
    about = "CAD B-rep parametrik DUCAD tanpa GUI (satuan mm, sudut derajat)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Jalankan file ops (OpFile JSON) dan cetak BatchReport.
    Run(cmd::run::Args),
    /// Replay part `.ducad` (opsional dengan param baru).
    Replay(cmd::replay::Args),
    /// Ringkasan part: body, sketch, params.
    Inspect(cmd::inspect::Args),
    /// Evaluasi checks desain (kode 3 bila ada yang gagal).
    Check(cmd::check::Args),
    /// Tulis oplog ramah git (satu op per baris).
    Oplog(cmd::oplog::Args),
    /// Bandingkan dua part (kode 1 bila berbeda).
    Diff(cmd::diff::Args),
    /// Uji selector face/tepi pada satu body.
    Select(cmd::select::Args),
    /// Render tampak part ke SVG/PNG.
    Render(cmd::render::Args),
    /// Ekspor part ke STEP/STL/OBJ/GLB.
    Export(cmd::export::Args),
    /// Bangun artefak manufaktur + laporan untuk CI (kode 3 bila check gagal).
    Build(cmd::build::Args),
    /// Cetak JSON Schema OpFile.
    Schema,
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .target(env_logger::Target::Stderr)
        .init();
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run(a) => cmd::run::exec(a),
        Command::Replay(a) => cmd::replay::exec(a),
        Command::Inspect(a) => cmd::inspect::exec(a),
        Command::Check(a) => cmd::check::exec(a),
        Command::Oplog(a) => cmd::oplog::exec(a),
        Command::Diff(a) => cmd::diff::exec(a),
        Command::Select(a) => cmd::select::exec(a),
        Command::Render(a) => cmd::render::exec(a),
        Command::Export(a) => cmd::export::exec(a),
        Command::Build(a) => cmd::build::exec(a),
        Command::Schema => cmd::print_json(&ducad_engine::ops::op_schema()).map(|_| Exit::Ok),
    };
    match result {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("ducad-cli: {}", e.message);
            ExitCode::from(e.exit as u8)
        }
    }
}
