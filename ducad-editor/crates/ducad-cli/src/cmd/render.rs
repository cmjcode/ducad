use std::path::{Path, PathBuf};

use ducad_engine::render::{render_svg, svg_to_png, RenderOptions, View};
use ducad_engine::Session;

use super::open_part;
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    #[arg(long, value_parser = parse_view, default_value = "iso")]
    view: View,
    /// Berkas keluaran `.svg` atau `.png`.
    #[arg(long)]
    out: PathBuf,
    /// Tampilkan garis tersembunyi.
    #[arg(long)]
    hidden: bool,
    /// Ukuran `WxH` piksel.
    #[arg(long, value_parser = parse_size, default_value = "800x600")]
    size: (u32, u32),
    /// Batasi ke body tertentu (boleh diulang).
    #[arg(long = "body")]
    bodies: Vec<String>,
}

fn parse_view(s: &str) -> Result<View, String> {
    serde_json::from_value(serde_json::Value::String(s.to_ascii_lowercase()))
        .map_err(|_| format!("view '{s}' tidak dikenal (iso|front|back|left|right|top|bottom)"))
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("ukuran '{s}' harus WxH"))?;
    let p = |v: &str| {
        v.trim()
            .parse::<u32>()
            .map_err(|_| format!("ukuran '{s}' tidak valid"))
    };
    Ok((p(w)?, p(h)?))
}

/// Render tampak lalu tulis SVG atau PNG.
#[allow(clippy::too_many_arguments)]
pub fn write_view(
    s: &Session,
    view: View,
    png: bool,
    path: &Path,
    width: u32,
    height: u32,
    hidden: bool,
    bodies: Option<Vec<String>>,
) -> Result<(), CliError> {
    let r = render_svg(
        s,
        &RenderOptions {
            view,
            width,
            height,
            hidden_lines: hidden,
            bodies,
        },
    )?;
    let bytes = if png {
        svg_to_png(&r.svg, width, height)?
    } else {
        r.svg.into_bytes()
    };
    std::fs::write(path, bytes)
        .map_err(|e| CliError::usage(format!("gagal menulis {}: {e}", path.display())))
}

pub fn exec(a: Args) -> CliResult {
    let png = match a
        .out
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => true,
        Some("svg") => false,
        _ => return Err(CliError::usage("--out harus berakhiran .svg atau .png")),
    };
    let s = open_part(&a.part)?;
    let bodies = (!a.bodies.is_empty()).then_some(a.bodies);
    write_view(
        &s, a.view, png, &a.out, a.size.0, a.size.1, a.hidden, bodies,
    )?;
    Ok(Exit::Ok)
}
