//! `ducad-cli` — antarmuka baris perintah headless DUCAD untuk agent dan CI.
//!
//! stdout hanya berisi hasil (JSON untuk laporan / `--json`); semua log ke
//! stderr. Kode keluar: 0 sukses · 1 operasi/replay gagal (laporan tetap
//! ditulis) · 2 salah pakai argumen atau I/O · 3 ada check yang gagal (P7) ·
//! 4 `build --release` pada paket yang belum disetujui (W5).

// Prinsip "jangan pernah crash" (ADR 0009): tidak ada unwrap/expect/panic di
// kode produksi; modul tes dikecualikan.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
mod cmd;

use std::process::ExitCode;
use std::sync::atomic::{AtomicU8, Ordering};

use clap::{Parser, Subcommand};

/// Active CLI human-output language: `0` = English (`en`, default), `1` = Indonesian (`id`).
static CLI_LANG: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliLang {
    En = 0,
    Id = 1,
}

fn parse_lang(s: &str) -> Result<CliLang, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "en" | "en-us" | "english" => Ok(CliLang::En),
        "id" | "id-id" | "indonesian" | "bahasa" => Ok(CliLang::Id),
        other => Err(format!("unsupported language '{other}' (expected 'en' or 'id')")),
    }
}

pub fn set_lang(lang: CliLang) {
    CLI_LANG.store(lang as u8, Ordering::Relaxed);
}

pub fn is_id() -> bool {
    CLI_LANG.load(Ordering::Relaxed) == CliLang::Id as u8
}

pub fn tr<'a>(en: &'a str, id: &'a str) -> &'a str {
    if is_id() {
        id
    } else {
        en
    }
}

fn init_lang_early() {
    if let Ok(env_lang) = std::env::var("DUCAD_LANG") {
        if let Ok(parsed) = parse_lang(&env_lang) {
            set_lang(parsed);
        }
    }
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--lang" {
            if let Some(val) = args.next() {
                if let Ok(parsed) = parse_lang(&val) {
                    set_lang(parsed);
                }
            }
        } else if let Some(val) = arg.strip_prefix("--lang=") {
            if let Ok(parsed) = parse_lang(val) {
                set_lang(parsed);
            }
        }
    }
}

/// Kode keluar CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Ok = 0,
    OpFailed = 1,
    Usage = 2,
    ChecksFailed = 3,
    /// `build --release` pada paket yang belum disetujui / ceklis belum bersih.
    NotReleased = 4,
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
    about = "Headless parametric B-rep CAD for agents and CI (mm units, degree angles)"
)]
struct Cli {
    /// Human-readable output language (`en` or `id`; overrides `DUCAD_LANG`, default `en`).
    #[arg(long, global = true, value_parser = parse_lang)]
    lang: Option<CliLang>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Execute an operations file (OpFile JSON) and print a BatchReport.
    Run(cmd::run::Args),
    /// Replay a `.ducad` part (optionally overriding parameters).
    Replay(cmd::replay::Args),
    /// Summarize a part: bodies, sketches, parameters, and mass properties.
    Inspect(cmd::inspect::Args),
    /// Evaluate design checks (exits with code 3 when blocking checks fail).
    Check(cmd::check::Args),
    /// Print a git-friendly oplog (one operation per line).
    Oplog(cmd::oplog::Args),
    /// Compare two parts (exits with code 1 when differences exist).
    Diff(cmd::diff::Args),
    /// Test face or edge selectors on a body.
    Select(cmd::select::Args),
    /// Render part views to SVG or PNG.
    Render(cmd::render::Args),
    /// List, export, or import variant configurations (CSV design table).
    Config(cmd::config::Args),
    /// Run a static, thermal, frequency, or buckling simulation study.
    Sim(cmd::sim::Args),
    /// Export a part to STEP, STL, OBJ, or GLB.
    Export(cmd::export::Args),
    /// Build manufacturing artifacts and CI reports (exits with code 3 if checks fail).
    Build(cmd::build::Args),
    /// Manage design package release state: submit for review, approve, or reopen.
    Release(cmd::release::Args),
    /// Ask the on-device AI assistant to propose design edits.
    Assist(cmd::assist::Args),
    /// Run an AI modeling agent turn (Claude/OpenAI/Ollama) via in-process MCP tools.
    Chat(cmd::chat::Args),
    /// Kernel migration golden corpus: capture baselines or compare (code 1 on mismatch).
    Golden(cmd::golden::Args),
    /// Migrate `.ducad` files to v4 format (oplog-derived bodies without embedded STEP).
    MigrateFile(cmd::migrate::Args),
    /// Print the OpFile JSON Schema.
    Schema,
}

fn main() -> ExitCode {
    init_lang_early();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .target(env_logger::Target::Stderr)
        .init();
    // Pelaksana `render --photo` (engine sendiri bebas wgpu).
    ducad_photoreal::install();
    let cli = Cli::parse();
    if let Some(lang) = cli.lang {
        set_lang(lang);
    }
    let result = match cli.command {
        Command::Run(a) => cmd::run::exec(a),
        Command::Replay(a) => cmd::replay::exec(a),
        Command::Inspect(a) => cmd::inspect::exec(a),
        Command::Check(a) => cmd::check::exec(a),
        Command::Oplog(a) => cmd::oplog::exec(a),
        Command::Diff(a) => cmd::diff::exec(a),
        Command::Select(a) => cmd::select::exec(a),
        Command::Render(a) => cmd::render::exec(a),
        Command::Sim(a) => cmd::sim::exec(a),
        Command::Config(a) => cmd::config::exec(a),
        Command::Export(a) => cmd::export::exec(a),
        Command::Build(a) => cmd::build::exec(a),
        Command::Release(a) => cmd::release::exec(a),
        Command::Assist(a) => cmd::assist::exec(a),
        Command::Chat(a) => cmd::chat::exec(a),
        Command::Golden(a) => cmd::golden::exec(a),
        Command::MigrateFile(a) => cmd::migrate::exec(a),
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
