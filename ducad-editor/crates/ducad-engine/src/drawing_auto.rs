//! Gambar kerja otomatis tanpa GUI (P10.1): merangkai HLR, tata letak
//! sheet, dan dimensi otomatis yang sudah ada di `ducad-io/src/drawing.rs`.

use ducad_io::drawing::{format_scale_ratio, DrawingSheet, PaperSize, TextAnnotation};
use ducad_kernel::{HlrExtractor, KernelMesh, KernelShape};

use crate::error::{OpError, OpResult};
use crate::model::ModelDoc;
use crate::ops::{Op, Params};
use crate::session::{DesignDoc, SessionCore};

/// Isi kepala gambar.
#[derive(Debug, Clone, Default)]
pub struct TitleInfo {
    pub title: String,
    pub part_number: String,
    pub author: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub material: String,
    pub revision: String,
}

/// Tinggi teks catatan (mm).
const NOTE_HEIGHT: f32 = 3.5;
/// Jarak antarbaris catatan (mm).
const NOTE_PITCH: f32 = 5.0;
/// Jarak baris catatan terbawah di atas bingkai dalam (mm).
const NOTE_BOTTOM: f32 = 6.0;

/// Sheet 4 tampak + dimensi otomatis + kepala gambar + catatan dari
/// body terlihat di sesi.
pub fn auto_sheet(
    core: &SessionCore,
    paper: PaperSize,
    title: &TitleInfo,
    notes: &[String],
) -> OpResult<DrawingSheet> {
    auto_sheet_model(core.model, paper, title, notes)
}

/// Inti [`auto_sheet`] di atas model baca-saja (dipakai CLI lewat `Session::model`).
pub fn auto_sheet_model(
    model: &ModelDoc,
    paper: PaperSize,
    title: &TitleInfo,
    notes: &[String],
) -> OpResult<DrawingSheet> {
    let mut bodies: Vec<_> = model.doc.bodies.iter().filter(|(_, b)| b.visible).collect();
    bodies.sort_by(|a, b| a.1.name.cmp(&b.1.name));
    let mut shapes: Vec<&KernelShape> = Vec::new();
    let mut meshes: Vec<&KernelMesh> = Vec::new();
    for (id, _) in bodies {
        if let Some(g) = model.geometry.get(id) {
            shapes.push(&g.shape);
            meshes.push(g.mesh.as_ref());
        }
    }
    if shapes.is_empty() {
        return Err(OpError::invalid(
            "tidak ada body terlihat untuk dibuat gambar kerja",
        ));
    }
    let drawing = HlrExtractor::extract_drawing(&shapes, &meshes);
    // `DrawingSheet::new` sudah memanggil `auto_layout`.
    let mut sheet = DrawingSheet::new(drawing, paper);
    sheet.generate_auto_dimensions();

    let scale = format_scale_ratio(sheet.scale);
    let tb = &mut sheet.title_block;
    tb.project_title = title.title.clone();
    tb.drawing_number = title.part_number.clone();
    tb.drawn_by = title.author.clone();
    tb.date = title.date.clone();
    tb.material = title.material.clone();
    tb.revision = title.revision.clone();
    tb.scale = scale;

    // Catatan di kiri bawah, baris pertama paling atas.
    let (_, inner) = sheet.border_rects_mm();
    let n = notes.len();
    for (i, note) in notes.iter().enumerate() {
        let y = inner[1] + NOTE_BOTTOM + (n - 1 - i) as f32 * NOTE_PITCH;
        sheet.custom_texts.push(TextAnnotation {
            position: [inner[0] + 4.0, y],
            text: note.clone(),
            font_size: NOTE_HEIGHT,
        });
    }
    Ok(sheet)
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
