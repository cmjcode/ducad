use std::path::{Path, PathBuf};

use ducad_engine::render::{render_photo, render_svg, svg_to_png, RenderOptions, View};
use ducad_engine::Session;

use super::open_part;
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    #[arg(long, value_parser = parse_view, default_value = "iso")]
    view: View,
    /// Output `.svg` or `.png` file.
    #[arg(long)]
    out: PathBuf,
    /// Show hidden edges.
    #[arg(long)]
    hidden: bool,
    /// Image dimensions `WxH` in pixels.
    #[arg(long, value_parser = parse_size, default_value = "800x600")]
    size: (u32, u32),
    /// Limit to specific body names (repeatable).
    #[arg(long = "body")]
    bodies: Vec<String>,
    /// Photorealistic path-traced render instead of line art (`.png` only).
    #[arg(long)]
    photo: bool,
    /// Samples per pixel for `--photo`.
    #[arg(long, requires = "photo")]
    samples: Option<u32>,
    /// Studio lighting environment for `--photo`: studio_soft | studio_contrast |
    /// overcast | sunset | warehouse | night.
    #[arg(long, requires = "photo")]
    env: Option<String>,
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

/// Render foto-realistis lalu tulis PNG.
#[allow(clippy::too_many_arguments)]
pub fn write_photo(
    s: &Session,
    view: View,
    path: &Path,
    width: u32,
    height: u32,
    bodies: Option<Vec<String>>,
    samples: Option<u32>,
    environment: Option<String>,
) -> Result<(), CliError> {
    let r = render_photo(
        s,
        &RenderOptions {
            view,
            width,
            height,
            hidden_lines: false,
            bodies,
        },
        samples,
        environment,
    )?;
    eprintln!("render foto: {} sampel/piksel lewat {}", r.samples, r.backend);
    std::fs::write(path, r.png)
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
    if a.photo && (!png || a.hidden) {
        return Err(CliError::usage(
            "--photo menghasilkan .png dan tidak bisa digabung dengan --hidden",
        ));
    }
    let s = open_part(&a.part)?;
    let bodies = (!a.bodies.is_empty()).then_some(a.bodies);
    if a.photo {
        write_photo(&s, a.view, &a.out, a.size.0, a.size.1, bodies, a.samples, a.env)?;
        return Ok(Exit::Ok);
    }
    write_view(
        &s, a.view, png, &a.out, a.size.0, a.size.1, a.hidden, bodies,
    )?;
    Ok(Exit::Ok)
}
