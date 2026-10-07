//! Gambar kerja tanpa GUI (P10.1, diperluas P21): merangkai HLR eksak,
//! potongan, tata letak, dimensi asosiatif, dan render berbayang dari satu
//! [`DrawingSpec`]. Dipakai tool `drawing`, `ducad-cli build`, dan GUI.

use std::sync::Mutex;

use ducad_io::drawing::{
    render_shaded, DrawingSheet, DrawingSpec, PaperSize, ShadedBody, TitleSpec,
};
use ducad_kernel::{
    DrawingOptions, HlrDrawing, HlrExtractor, KernelMesh, KernelShape, SectionRequest,
};

use crate::error::{OpError, OpErrorCode, OpResult};
use crate::model::ModelDoc;
use crate::ops::{Op, Params};
use crate::session::{DesignDoc, SessionCore};

/// Isi kepala gambar (alias lama untuk [`TitleSpec`]).
pub type TitleInfo = TitleSpec;

/// Lembar jadi + peringatan berkode (`HLR_EXACT_FALLBACK: …`, `DRAWING_…`).
#[derive(Debug, Clone)]
pub struct SheetOutput {
    pub sheet: DrawingSheet,
    pub warnings: Vec<String>,
}

/// Cache HLR: `set_params`/render ulang yang tidak mengubah geometri maupun
/// potongan tidak menghitung HLR lagi. Kunci = (sidik jari geometri,
/// potongan). Kecil dan global-proses karena HLR tidak bergantung sesi.
const HLR_CACHE_CAPACITY: usize = 8;
static HLR_CACHE: Mutex<Vec<(String, HlrDrawing)>> = Mutex::new(Vec::new());

fn cached_drawing(key: &str) -> Option<HlrDrawing> {
    let cache = HLR_CACHE.lock().unwrap_or_else(|p| p.into_inner());
    cache.iter().find(|(k, _)| k == key).map(|(_, d)| d.clone())
}

fn store_drawing(key: String, drawing: &HlrDrawing) {
    let mut cache = HLR_CACHE.lock().unwrap_or_else(|p| p.into_inner());
    cache.retain(|(k, _)| *k != key);
    if cache.len() >= HLR_CACHE_CAPACITY {
        cache.remove(0);
    }
    cache.push((key, drawing.clone()));
}

/// Jumlah entri cache HLR (untuk tes).
pub fn hlr_cache_len() -> usize {
    HLR_CACHE.lock().unwrap_or_else(|p| p.into_inner()).len()
}

fn coded(code: OpErrorCode, message: String, hint: &str) -> OpError {
    let mut e = OpError::new(code, message);
    e.hint = Some(hint.to_string());
    e
}

/// Ubah pesan berkode dari kernel/io menjadi `OpError` bertipe.
fn section_error(message: &str) -> OpError {
    if message.starts_with("DRAWING_SECTION_EMPTY") {
        coded(
            OpErrorCode::DrawingSectionEmpty,
            message.to_string(),
            "move the section plane (offset/path) so that it passes through a visible body",
        )
    } else if message.starts_with("DRAWING_SECTION_LABEL_DUP") {
        coded(
            OpErrorCode::DrawingSectionLabelDup,
            message.to_string(),
            "give every section a different single-letter label",
        )
    } else {
        OpError::invalid(message.to_string())
    }
}

/// Sheet dari body terlihat di sesi (API lama: 4 tampak + A-A bawaan).
pub fn auto_sheet(
    core: &SessionCore,
    paper: PaperSize,
    title: &TitleInfo,
    notes: &[String],
) -> OpResult<DrawingSheet> {
    auto_sheet_model(core.model, paper, title, notes)
}

/// Inti [`auto_sheet`] di atas model baca-saja.
pub fn auto_sheet_model(
    model: &ModelDoc,
    paper: PaperSize,
    title: &TitleInfo,
    notes: &[String],
) -> OpResult<DrawingSheet> {
    let spec = DrawingSpec {
        paper,
        title: title.clone(),
        notes: notes.to_vec(),
        ..DrawingSpec::default()
    };
    build_sheet(model, &spec, None).map(|o| o.sheet)
}

/// Bangun lembar gambar dari `spec`.
///
/// `geometry_key` = sidik jari geometri (mis. `design.fingerprint`); bila
/// ada dan tidak kosong, hasil HLR di-cache per (geometri, potongan).
pub fn build_sheet(
    model: &ModelDoc,
    spec: &DrawingSpec,
    geometry_key: Option<&str>,
) -> OpResult<SheetOutput> {
    let mut bodies: Vec<_> = model.doc.bodies.iter().filter(|(_, b)| b.visible).collect();
    bodies.sort_by(|a, b| a.1.name.cmp(&b.1.name));
    let mut shapes: Vec<&KernelShape> = Vec::new();
    let mut meshes: Vec<&KernelMesh> = Vec::new();
    let mut colors: Vec<[f32; 3]> = Vec::new();
    for (id, body) in bodies {
        if let Some(g) = model.geometry.get(id) {
            shapes.push(&g.shape);
            meshes.push(g.mesh.as_ref());
            let c = body.material.base_color;
            colors.push([c[0], c[1], c[2]]);
        }
    }
    if shapes.is_empty() {
        return Err(OpError::invalid(
            "tidak ada body terlihat untuk dibuat gambar kerja",
        ));
    }

    spec.section_labels().map_err(|e| section_error(&e))?;
    let unknown = spec.views.unknown_keys();
    if !unknown.is_empty() {
        return Err(OpError::invalid(format!(
            "unknown view name(s) {unknown:?} in 'views' (front, top, right, isometric, section_<letter>, detail_<letter>)"
        )));
    }

    // Kotak pembatas model, sama dengan yang dipakai `HlrExtractor`.
    let merged = KernelMesh::merge(&meshes);
    let bbox = merged
        .bounding_box()
        .unwrap_or(([0.0; 3], [100.0, 100.0, 100.0]));
    let mut requests: Vec<SectionRequest> = Vec::new();
    for s in spec.sections.as_deref().unwrap_or(&[]) {
        requests.push(s.to_request(bbox).map_err(OpError::invalid)?);
    }
    let options = DrawingOptions {
        sections: requests,
        default_section: spec.sections.is_none(),
        exact: true,
    };

    let key = geometry_key.filter(|k| !k.is_empty()).map(|k| {
        format!(
            "{k}|{}|{}",
            serde_json::to_string(&options.sections).unwrap_or_default(),
            options.default_section
        )
    });
    let drawing = match key.as_deref().and_then(cached_drawing) {
        Some(d) => d,
        None => {
            let d = HlrExtractor::extract_drawing_with(&shapes, &meshes, &[], &options);
            if let Some(k) = key {
                store_drawing(k, &d);
            }
            d
        }
    };

    let mut warnings = Vec::new();
    for w in &drawing.warnings {
        // Potongan yang diminta eksplisit tetapi gagal/kosong = error keras;
        // A-A bawaan yang kebetulan tidak memotong apa pun hanya peringatan.
        let hard = spec.sections.is_some()
            && (w.starts_with("DRAWING_SECTION_EMPTY") || w.starts_with("DRAWING_SECTION_PATH"));
        if hard {
            return Err(section_error(w));
        }
        warnings.push(w.clone());
    }

    let mut sheet = DrawingSheet::from_spec(drawing, spec);
    sheet.title_block.scale = ducad_io::drawing::format_scale_ratio(sheet.scale);

    // Render berbayang: CPU, deterministik.
    let shaded_bodies: Vec<ShadedBody<'_>> = meshes
        .iter()
        .zip(&colors)
        .map(|(mesh, color)| ShadedBody {
            mesh,
            color: *color,
        })
        .collect();
    let mut image_bytes = 0usize;
    for view in &mut sheet.shaded {
        let (w, h) = view.pixel_size();
        view.image = render_shaded(&shaded_bodies, view.spec.camera, w, h);
        image_bytes += view.image.rgb.len();
    }
    if image_bytes > 12 * 1024 * 1024 {
        warnings.push(format!(
            "DRAWING_SHADED_LARGE: shaded renders hold {} MB of pixels; lower px_per_mm to keep the PDF small",
            image_bytes / (1024 * 1024)
        ));
    }
    Ok(SheetOutput { sheet, warnings })
}

/// Tulis `sheet` ke `path` dalam format `pdf`/`svg`/`dxf`. Mengembalikan
/// peringatan tambahan (mis. render berbayang dilewati di DXF).
pub fn write_sheet(
    sheet: &DrawingSheet,
    format: &str,
    path: &std::path::Path,
) -> OpResult<Vec<String>> {
    let io = |e: anyhow::Error| {
        OpError::new(
            OpErrorCode::Io,
            format!("failed to write {}: {e:#}", path.display()),
        )
    };
    let mut warnings = Vec::new();
    match format.to_ascii_lowercase().as_str() {
        "pdf" => ducad_io::pdf::export_pdf(sheet, path).map_err(io)?,
        "svg" => ducad_io::svg::export_drawing_sheet_svg(sheet, path).map_err(io)?,
        "dxf" => {
            ducad_io::dxf::export_drawing_sheet(sheet, path).map_err(io)?;
            if sheet.shaded.iter().any(|s| s.visible) {
                warnings.push(
                    "DRAWING_DXF_NO_RASTER: DXF cannot embed shaded renders; they were skipped"
                        .to_string(),
                );
            }
        }
        other => {
            return Err(OpError::invalid(format!(
                "unknown drawing format '{other}' (pdf, svg, dxf)"
            )))
        }
    }
    Ok(warnings)
}

/// Satu baris `"<n>× <callout>"` per `Op::Hole` di oplog.
pub fn hole_notes(design: &DesignDoc, params: &Params) -> Vec<String> {
    design
        .oplog
        .iter()
        .filter_map(|op| {
            let Op::Hole {
                at, at_world, spec, ..
            } = op
            else {
                return None;
            };
            let n = at.len() + at_world.len();
            let spec = crate::session::hole_spec(spec, params).ok()?;
            Some(format!("{n}× {}", spec.technical_callout()))
        })
        .collect()
}

/// Ringkasan satu lembar tersimpan untuk `inspect`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DrawingSummary {
    pub name: String,
    pub paper: String,
    pub views: Vec<String>,
    pub sections: Vec<String>,
    /// `"auto"`, `"none"`, atau jumlah dimensi eksplisit.
    pub dimension_count: serde_json::Value,
    pub shaded: usize,
}

pub fn summarize(spec: &DrawingSpec) -> DrawingSummary {
    use ducad_io::drawing::DimensionPolicy;
    use ducad_kernel::ProjectedViewKind as K;
    let mut views = Vec::new();
    for kind in [K::Front, K::Top, K::Right, K::Isometric] {
        if spec.views.get(kind).visible.unwrap_or(true) {
            views.push(ducad_io::drawing::view_key(kind));
        }
    }
    let sections: Vec<String> = match &spec.sections {
        None => vec!["A".to_string()],
        Some(list) => list.iter().map(|s| s.label.to_ascii_uppercase()).collect(),
    };
    let manual = spec
        .layout
        .as_ref()
        .map(|l| l.manual_dimensions.len())
        .unwrap_or(0);
    DrawingSummary {
        name: spec.name.clone(),
        paper: spec.paper.short_name().to_string(),
        views,
        sections,
        dimension_count: match &spec.dimensions {
            DimensionPolicy::Auto if manual == 0 => serde_json::json!("auto"),
            DimensionPolicy::Auto => serde_json::json!(format!("auto+{manual}")),
            DimensionPolicy::None => serde_json::json!(manual),
            DimensionPolicy::Only(list) => serde_json::json!(list.len() + manual),
        },
        shaded: spec.shaded.len(),
    }
}
