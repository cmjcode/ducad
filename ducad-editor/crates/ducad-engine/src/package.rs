//! Paket desain (W2): satu set dokumen lengkap dari satu part/assembly —
//! sampul + indeks lembar + tabel revisi, gambar part per body, gambar
//! assembly berbalon BOM, pola bentangan sheet metal, lembar BOM, dan lembar
//! kepatuhan (keterlacakan persyaratan + checks bersitasi) — sebagai SATU PDF
//! multi-halaman, plus laporan Markdown.
//!
//! Deterministik: masukan dan tanggal sama → byte sama. `PackageSpec`
//! disimpan di `DesignDoc.package`; tanpa itu susunan lembar dipilih otomatis.

use std::collections::BTreeMap;

use ducad_io::drawing::pages::{document_pages, flat_pattern_page, printable, Block, Table};
use ducad_io::drawing::scene::Group;
use ducad_io::drawing::{DrawingSheet, DrawingSpec, PaperSize, TitleBlockInfo};
use ducad_io::pdf::{generate_pdf_document, PdfPage};
use ducad_kernel::ProjectedViewKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::brief::{traceability, ReleaseInfo, TraceStatus, Traceability};
use crate::check::{CheckResult, CheckStatus, CheckSummary, Severity, RULE_DISCLAIMER};
use crate::drawing_auto::build_sheet;
use crate::error::{OpError, OpErrorCode, OpResult};
use crate::firm::{estimate_cost, material_label, CostReport, FirmProfile};
use crate::model::ModelDoc;
use crate::session::SessionMeta;

/// One sheet of the design package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SheetSpec {
    /// Cover: title, brief, sheet index, revision table, compliance summary.
    Cover,
    /// A drawing sheet stored in the part (tool `drawing` with `save: true`).
    Drawing { name: String },
    /// Automatic part drawing of one body.
    Part { body: String },
    /// Assembly drawing of every visible body with a BOM table and balloons.
    Assembly,
    /// Flat pattern of a sheet metal body.
    FlatPattern { body: String },
    /// Bill of materials as a table page.
    Bom,
    /// Requirement traceability and design checks with their citations.
    Compliance,
}

/// The design package: which sheets go into the document set. Stored in the part.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackageSpec {
    /// Sheets in order. Empty = automatic: cover, stored drawings (or an assembly sheet plus
    /// one part sheet per body), flat patterns, BOM, compliance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sheets: Vec<SheetSpec>,
    /// Paper of the generated sheets: `a4`, `a4-portrait`, `a3` (default), `a3-portrait`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper: Option<String>,
}

/// Isi kepala gambar yang berlaku untuk seluruh paket.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PackageInfo {
    pub title: String,
    pub part_number: String,
    pub author: String,
    /// `YYYY-MM-DD`.
    pub date: String,
}

/// Satu baris BOM: body ber-mesh identik digabung.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BomLine {
    pub item: u32,
    pub part: String,
    pub qty: u32,
    pub material: String,
    pub volume_mm3: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mass_g: Option<f64>,
    /// Sebutan part standar (mis. `ISO 4762 - M6 x 20`); kosong bila bukan.
    pub standard: String,
    /// Nama semua body anggota baris ini.
    pub bodies: Vec<String>,
}

type VisibleBody<'a> = (&'a ducad_core::Body, &'a crate::model::BodyGeometry);

fn visible_bodies(model: &ModelDoc) -> Vec<VisibleBody<'_>> {
    let mut v: Vec<_> = model
        .doc
        .bodies
        .iter()
        .filter(|(_, b)| b.visible)
        .filter_map(|(id, b)| Some((b, model.geometry.get(id)?)))
        .collect();
    v.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    v
}

/// BOM dari body terlihat: satu baris per kelompok `mesh_fingerprint` sama,
/// urut kemunculan nama.
pub fn bom_lines(model: &ModelDoc, meta: &SessionMeta) -> Vec<BomLine> {
    let mut groups: BTreeMap<u64, Vec<VisibleBody<'_>>> = BTreeMap::new();
    let mut order: Vec<u64> = Vec::new();
    for (b, g) in visible_bodies(model) {
        let key = g.mesh_fingerprint;
        if !groups.contains_key(&key) {
            order.push(key);
        }
        groups.entry(key).or_default().push((b, g));
    }
    order
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let members = &groups[key];
            let (first, geo) = members[0];
            let volume = geo.shape.volume();
            BomLine {
                item: i as u32 + 1,
                part: first.name.clone(),
                qty: members.len() as u32,
                material: material_label(first),
                volume_mm3: volume,
                mass_g: model.doc.density_of(first).map(|rho| volume / 1000.0 * rho),
                standard: meta
                    .standard_parts
                    .get(&first.name)
                    .cloned()
                    .unwrap_or_default(),
                bodies: members.iter().map(|(b, _)| b.name.clone()).collect(),
            }
        })
        .collect()
}

/// Susunan lembar otomatis untuk part ini.
pub fn plan_sheets(model: &ModelDoc, meta: &SessionMeta) -> Vec<SheetSpec> {
    let design = &meta.design;
    let bom = bom_lines(model, meta);
    let mut sheets = vec![SheetSpec::Cover];
    if design.drawings.is_empty() {
        let made: Vec<&BomLine> = bom.iter().filter(|l| l.standard.is_empty()).collect();
        if bom.len() > 1 {
            sheets.push(SheetSpec::Assembly);
        }
        for line in made {
            sheets.push(SheetSpec::Part {
                body: line.part.clone(),
            });
        }
    } else {
        for d in &design.drawings {
            sheets.push(SheetSpec::Drawing {
                name: d.name.clone(),
            });
        }
    }
    for body in meta.sheet_metal.keys() {
        if visible_bodies(model).iter().any(|(b, _)| &b.name == body) {
            sheets.push(SheetSpec::FlatPattern { body: body.clone() });
        }
    }
    if bom.len() > 1 {
        sheets.push(SheetSpec::Bom);
    }
    if !design.checks.is_empty() || design.brief.is_some() {
        sheets.push(SheetSpec::Compliance);
    }
    sheets
}

/// Satu halaman paket.
#[derive(Debug, Clone)]
pub struct PackagePage {
    /// Judul di indeks lembar dan bookmark PDF.
    pub title: String,
    /// `cover`, `drawing`, `part`, `assembly`, `flat_pattern`, `bom`, `compliance`.
    pub kind: &'static str,
    pub sheet: DrawingSheet,
    pub groups: Vec<Group>,
}

/// Hasil pembangunan paket.
#[derive(Debug, Clone)]
pub struct PackageOutput {
    pub pages: Vec<PackagePage>,
    pub bom: Vec<BomLine>,
    pub traceability: Option<Traceability>,
    pub cost: CostReport,
    pub release: ReleaseInfo,
    /// Tahap rilis yang berlaku (persetujuan basi = draf).
    pub state: crate::brief::ReleaseState,
    pub warnings: Vec<String>,
}

impl PackageOutput {
    /// PDF multi-halaman dengan bookmark per lembar.
    pub fn pdf_bytes(&self) -> Vec<u8> {
        let pages: Vec<PdfPage<'_>> = self
            .pages
            .iter()
            .map(|p| PdfPage {
                sheet: &p.sheet,
                extra: &p.groups,
                bookmark: &p.title,
            })
            .collect();
        generate_pdf_document(&pages)
    }

    /// Indeks lembar: (nomor, judul, jenis).
    pub fn index(&self) -> Vec<(usize, &str, &'static str)> {
        self.pages
            .iter()
            .enumerate()
            .map(|(i, p)| (i + 1, p.title.as_str(), p.kind))
            .collect()
    }
}

fn status_word(s: CheckStatus, severity: Severity) -> &'static str {
    match (s, severity) {
        (CheckStatus::Pass, _) => "PASS",
        (_, Severity::Warning) => "WARN",
        (CheckStatus::Fail, _) => "FAIL",
        (CheckStatus::Error, _) => "ERROR",
    }
}

fn trace_word(s: TraceStatus) -> &'static str {
    match s {
        TraceStatus::Verified => "VERIFIED",
        TraceStatus::Failed => "FAILED",
        TraceStatus::Unverified => "NO CHECK",
    }
}

fn check_label(r: &CheckResult) -> String {
    r.id.clone().unwrap_or_else(|| r.kind.to_string())
}

fn num(v: f64) -> String {
    let t = format!("{v:.2}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Bangun paket desain.
///
/// `checks` = hasil check desain (pemanggil yang menjalankannya, setelah
/// studi simulasi), dipakai lembar kepatuhan dan ringkasan sampul.
pub fn build_package(
    model: &ModelDoc,
    meta: &SessionMeta,
    spec: &PackageSpec,
    info: &PackageInfo,
    firm: &FirmProfile,
    checks: Option<&CheckSummary>,
) -> OpResult<PackageOutput> {
    let design = &meta.design;
    let paper = match spec.paper.as_deref() {
        Some(p) => ducad_io::drawing::parse_paper(p).ok_or_else(|| {
            OpError::invalid(format!(
                "unknown package paper '{p}' (a4, a4-portrait, a3, a3-portrait)"
            ))
        })?,
        None => firm.paper_size().unwrap_or(PaperSize::A3Landscape),
    };
    let signature = crate::brief::release_signature(model, design);
    let release = design.release.clone().unwrap_or_default();
    let state = release.effective_state(&signature);
    let bom = bom_lines(model, meta);
    let results: &[CheckResult] = checks.map(|c| c.results.as_slice()).unwrap_or(&[]);
    let trace = design
        .brief
        .as_ref()
        .filter(|b| !b.requirements.is_empty())
        .map(|b| traceability(b, results));
    let cost = estimate_cost(model, meta, firm);
    let mut warnings: Vec<String> = Vec::new();

    let plan = if spec.sheets.is_empty() {
        plan_sheets(model, meta)
    } else {
        spec.sheets.clone()
    };
    if plan.is_empty() {
        return Err(OpError::invalid("the package has no sheets"));
    }

    let first_material = visible_bodies(model)
        .first()
        .map(|(b, _)| material_label(b))
        .unwrap_or_default();
    // Kolom judul kepala gambar ±80 mm pada huruf 3,9 mm: judul panjang
    // dipangkas supaya tidak menimpa kolom nomor gambar.
    let base_title = |subtitle: &str| TitleBlockInfo {
        project_title: ducad_io::drawing::pages::fit_text(
            &if subtitle.is_empty() {
                info.title.clone()
            } else {
                format!("{} - {subtitle}", info.title)
            },
            3.9,
            80.0,
        ),
        drawing_number: format!("{}{}", firm.part_number_prefix, info.part_number),
        drawn_by: if info.author.is_empty() {
            firm.drawn_by.clone()
        } else {
            info.author.clone()
        },
        date: info.date.clone(),
        scale: "-".to_string(),
        material: first_material.clone(),
        units: "mm".to_string(),
        sheet_number: String::new(),
        company_name: if firm.company.is_empty() {
            TitleBlockInfo::default().company_name
        } else {
            firm.company.clone()
        },
        revision: release.revision.clone(),
        general_tolerance: firm.general_tolerance.clone(),
    };
    // Lembar gambar memakai kepala paket; tata letak tersimpan pengguna
    // (`layout.title_block`) tetap menang kecuali nomor lembar dan revisi.
    let drawing_sheet =
        |mut spec: DrawingSpec, subtitle: &str| -> OpResult<(DrawingSheet, Vec<String>)> {
            let keep_title = spec.layout.as_ref().and_then(|l| l.title_block.clone());
            for note in &firm.notes {
                if !spec.notes.contains(note) {
                    spec.notes.push(note.clone());
                }
            }
            let out = build_sheet(model, &spec, Some(&design.fingerprint))?;
            let mut sheet = out.sheet;
            let scale = sheet.title_block.scale.clone();
            let mut tb = keep_title.unwrap_or_else(|| {
                let mut tb = base_title(subtitle);
                if !spec.title.title.is_empty() {
                    tb.project_title = spec.title.title.clone();
                }
                if !spec.title.material.is_empty() {
                    tb.material = spec.title.material.clone();
                }
                tb
            });
            tb.scale = scale;
            tb.revision = release.revision.clone();
            tb.general_tolerance = firm.general_tolerance.clone();
            sheet.title_block = tb;
            Ok((sheet, out.warnings))
        };

    // Halaman sampul butuh indeks semua lembar, jadi lembar lain dibangun
    // dulu dan sampul disisipkan di posisinya setelah itu.
    let mut pages: Vec<Option<PackagePage>> = Vec::new();
    let mut cover_at: Vec<usize> = Vec::new();
    for sheet_spec in &plan {
        match sheet_spec {
            SheetSpec::Cover => {
                cover_at.push(pages.len());
                pages.push(None);
            }
            SheetSpec::Drawing { name } => {
                let stored = design.drawing(name).cloned().ok_or_else(|| {
                    let known: Vec<&str> = design.drawings.iter().map(|d| d.name.as_str()).collect();
                    OpError::new(
                        OpErrorCode::UnknownRef,
                        format!("package sheet refers to drawing '{name}' which is not stored in the part (stored: {known:?})"),
                    )
                })?;
                let (sheet, w) = drawing_sheet(stored, name)?;
                warnings.extend(w);
                pages.push(Some(PackagePage {
                    title: format!("Drawing - {name}"),
                    kind: "drawing",
                    sheet,
                    groups: Vec::new(),
                }));
            }
            SheetSpec::Part { body } => {
                let material = visible_bodies(model)
                    .iter()
                    .find(|(b, _)| &b.name == body)
                    .map(|(b, _)| material_label(b))
                    .unwrap_or_default();
                let mut d = DrawingSpec {
                    name: format!("part_{body}"),
                    paper,
                    bodies: vec![body.clone()],
                    ..DrawingSpec::default()
                };
                d.title.material = material;
                d.notes =
                    crate::drawing_auto::hole_notes_for(design, &design.effective_params(), body);
                let (sheet, w) = drawing_sheet(d, body)?;
                warnings.extend(w);
                pages.push(Some(PackagePage {
                    title: format!("Part - {body}"),
                    kind: "part",
                    sheet,
                    groups: Vec::new(),
                }));
            }
            SheetSpec::Assembly => {
                let d = DrawingSpec {
                    name: "assembly".to_string(),
                    paper,
                    sections: Some(Vec::new()),
                    hidden_lines: false,
                    ..DrawingSpec::default()
                };
                let (mut sheet, w) = drawing_sheet(d, "ASSEMBLY")?;
                warnings.extend(w);
                let rows: Vec<ducad_core::assembly::BomRow> = bom
                    .iter()
                    .map(|l| ducad_core::assembly::BomRow {
                        item: l.item,
                        part_number: (!l.standard.is_empty()).then(|| l.standard.clone()),
                        name: l.part.clone(),
                        material: Some(l.material.clone()),
                        quantity: l.qty,
                        sub_assembly: None,
                    })
                    .collect();
                let targets: Vec<[f32; 2]> = bom
                    .iter()
                    .map(|l| {
                        visible_bodies(model)
                            .iter()
                            .find(|(b, _)| b.name == l.part)
                            .and_then(|(_, g)| {
                                let c = g.shape.mass_properties().centroid;
                                sheet.model_point_on_view(
                                    ProjectedViewKind::Isometric,
                                    [c[0] as f32, c[1] as f32, c[2] as f32],
                                )
                            })
                            .unwrap_or([f32::NAN, f32::NAN])
                    })
                    .collect();
                sheet.set_bom_with_balloons(&rows, &targets);
                // Kolom balon dipindah ke kanan tampak isometrik supaya tidak
                // menimpa model (sasaran = pusat massa, biasanya di tengah).
                let (lo, hi) = (sheet.drawing.model_bbox_min, sheet.drawing.model_bbox_max);
                let view_right = (0..8)
                    .filter_map(|i| {
                        sheet.model_point_on_view(
                            ProjectedViewKind::Isometric,
                            [
                                if i & 1 == 0 { lo[0] } else { hi[0] },
                                if i & 2 == 0 { lo[1] } else { hi[1] },
                                if i & 4 == 0 { lo[2] } else { hi[2] },
                            ],
                        )
                    })
                    .map(|p| p[0])
                    .fold(f32::MIN, f32::max);
                let column = sheet
                    .balloons
                    .iter()
                    .map(|b| b.balloon_pos[0])
                    .fold(f32::MIN, f32::max);
                if view_right > f32::MIN && column > f32::MIN {
                    let (_, inner) = sheet.border_rects_mm();
                    let wanted = (view_right + 14.0).min(inner[2] - 10.0);
                    let dx = wanted - column;
                    if dx > 0.0 {
                        for b in &mut sheet.balloons {
                            b.balloon_pos[0] += dx;
                        }
                    }
                }
                sheet.title_block.material = "SEE BOM".to_string();
                pages.push(Some(PackagePage {
                    title: "Assembly".to_string(),
                    kind: "assembly",
                    sheet,
                    groups: Vec::new(),
                }));
            }
            SheetSpec::FlatPattern { body } => {
                let state = meta.sheet_metal.get(body).ok_or_else(|| {
                    OpError::new(
                        OpErrorCode::UnknownRef,
                        format!("package sheet asks for the flat pattern of '{body}', which is not a sheet metal body"),
                    )
                    .with_hint("flat patterns exist only for bodies made with base_flange")
                })?;
                let flat = state
                    .model
                    .flat_pattern()
                    .map_err(|e| OpError::invalid(format!("flat pattern of '{body}': {e}")))?;
                let page = flat_pattern_page(
                    paper,
                    &base_title(&format!("FLAT {body}")),
                    &format!("FLAT PATTERN - {body}"),
                    &flat,
                    state.model.thickness,
                );
                pages.push(Some(PackagePage {
                    title: format!("Flat pattern - {body}"),
                    kind: "flat_pattern",
                    sheet: page.sheet,
                    groups: page.groups,
                }));
            }
            SheetSpec::Bom => {
                let mut t = Table::new(
                    &["ITEM", "PART", "QTY", "MATERIAL", "MASS (g)", "STANDARD"],
                    &[1.0, 4.0, 1.0, 3.0, 1.6, 4.0],
                );
                for l in &bom {
                    t.row([
                        l.item.to_string(),
                        l.part.clone(),
                        l.qty.to_string(),
                        l.material.clone(),
                        l.mass_g.map(num).unwrap_or_else(|| "-".to_string()),
                        l.standard.clone(),
                    ]);
                }
                let total: f64 = bom
                    .iter()
                    .filter_map(|l| l.mass_g.map(|m| m * f64::from(l.qty)))
                    .sum();
                let blocks = vec![
                    Block::Heading("BILL OF MATERIALS".to_string()),
                    Block::Table(t),
                    Block::Text(format!(
                        "{} line(s), {} part(s), total mass {} g.",
                        bom.len(),
                        bom.iter().map(|l| l.qty).sum::<u32>(),
                        num(total)
                    )),
                ];
                for (i, page) in document_pages(paper, &base_title("BOM"), &blocks)
                    .into_iter()
                    .enumerate()
                {
                    pages.push(Some(PackagePage {
                        title: if i == 0 {
                            "Bill of materials".to_string()
                        } else {
                            format!("Bill of materials ({})", i + 1)
                        },
                        kind: "bom",
                        sheet: page.sheet,
                        groups: page.groups,
                    }));
                }
            }
            SheetSpec::Compliance => {
                let blocks = compliance_blocks(trace.as_ref(), checks);
                for (i, page) in document_pages(paper, &base_title("COMPLIANCE"), &blocks)
                    .into_iter()
                    .enumerate()
                {
                    pages.push(Some(PackagePage {
                        title: if i == 0 {
                            "Compliance".to_string()
                        } else {
                            format!("Compliance ({})", i + 1)
                        },
                        kind: "compliance",
                        sheet: page.sheet,
                        groups: page.groups,
                    }));
                }
            }
        }
    }

    // Sampul: indeks menomori lembar akhir (sampul dianggap satu halaman).
    let titles: Vec<String> = pages
        .iter()
        .map(|p| {
            p.as_ref()
                .map(|p| p.title.clone())
                .unwrap_or_else(|| "Cover".to_string())
        })
        .collect();
    for at in cover_at {
        let blocks = cover_blocks(
            meta,
            info,
            &release,
            state,
            &titles,
            trace.as_ref(),
            checks,
            &cost,
        );
        let mut cover = document_pages(paper, &base_title("COVER"), &blocks);
        if cover.len() > 1 {
            warnings.push(format!(
                "PACKAGE_COVER_OVERFLOW: the cover needs {} pages; only the first is kept (shorten the brief or use a larger paper)",
                cover.len()
            ));
        }
        let page = cover.remove(0);
        pages[at] = Some(PackagePage {
            title: "Cover".to_string(),
            kind: "cover",
            sheet: page.sheet,
            groups: page.groups,
        });
    }

    let mut pages: Vec<PackagePage> = pages.into_iter().flatten().collect();
    let total = pages.len();
    for (i, page) in pages.iter_mut().enumerate() {
        page.sheet.title_block.sheet_number = format!("{} / {total}", i + 1);
    }
    Ok(PackageOutput {
        pages,
        bom,
        traceability: trace,
        cost,
        release,
        state,
        warnings,
    })
}

#[allow(clippy::too_many_arguments)]
fn cover_blocks(
    meta: &SessionMeta,
    info: &PackageInfo,
    release: &ReleaseInfo,
    state: crate::brief::ReleaseState,
    titles: &[String],
    trace: Option<&Traceability>,
    checks: Option<&CheckSummary>,
    cost: &CostReport,
) -> Vec<Block> {
    let design = &meta.design;
    let mut blocks = vec![Block::Heading(info.title.to_uppercase())];
    let status = match (state, &release.approval) {
        (crate::brief::ReleaseState::Approved, Some(a)) => {
            format!("APPROVED by {} on {}", a.by, a.date)
        }
        (crate::brief::ReleaseState::InReview, _) => "IN REVIEW - not released".to_string(),
        _ => "DRAFT - not released".to_string(),
    };
    blocks.push(Block::Fields(vec![
        ("Part number".to_string(), info.part_number.clone()),
        ("Revision".to_string(), release.revision.clone()),
        ("Date".to_string(), info.date.clone()),
        ("Status".to_string(), status),
    ]));

    if let Some(b) = &design.brief {
        blocks.push(Block::Section("DESIGN BRIEF".to_string()));
        let mut fields = Vec::new();
        let mut push = |k: &str, v: String| {
            if !v.trim().is_empty() {
                fields.push((k.to_string(), v));
            }
        };
        push("Function", b.function.clone());
        push("Interfaces", b.interfaces.join("; "));
        push("Loads", b.loads.join("; "));
        push("Material", b.material.clone());
        push(
            "Process",
            b.process
                .and_then(|p| serde_json::to_value(p).ok())
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
        );
        push(
            "Quantity",
            b.quantity.map(|q| q.to_string()).unwrap_or_default(),
        );
        push("Standards", b.standards.join("; "));
        push("Cost target", b.cost_target.clone());
        if !fields.is_empty() {
            blocks.push(Block::Fields(fields));
        }
    }

    blocks.push(Block::Section("SHEET INDEX".to_string()));
    let mut index = Table::new(&["SHEET", "CONTENT"], &[1.0, 9.0]);
    for (i, t) in titles.iter().enumerate() {
        index.row([format!("{} / {}", i + 1, titles.len()), t.clone()]);
    }
    blocks.push(Block::Table(index));

    blocks.push(Block::Section("REVISIONS".to_string()));
    let mut rev = Table::new(&["REV", "DESCRIPTION", "DATE", "BY"], &[1.0, 7.0, 2.0, 2.0]);
    if release.history.is_empty() {
        rev.row([
            release.revision.clone(),
            "Not yet submitted for review".to_string(),
            String::new(),
            String::new(),
        ]);
    }
    for h in &release.history {
        rev.row([
            h.revision.clone(),
            h.description.clone(),
            h.date.clone(),
            h.by.clone(),
        ]);
    }
    blocks.push(Block::Table(rev));

    blocks.push(Block::Section("COMPLIANCE SUMMARY".to_string()));
    let mut fields = Vec::new();
    match checks {
        Some(c) => fields.push((
            "Design checks".to_string(),
            format!(
                "{} passed, {} failed, {} error, {} warning",
                c.pass, c.fail, c.error, c.warn
            ),
        )),
        None => fields.push(("Design checks".to_string(), "not evaluated".to_string())),
    }
    if let Some(t) = trace {
        fields.push((
            "Requirements".to_string(),
            format!(
                "{} verified, {} failed, {} without a check",
                t.verified, t.failed, t.unverified
            ),
        ));
    }
    let packs: std::collections::BTreeSet<&str> = design
        .checks
        .iter()
        .filter_map(|c| c.rule.as_deref())
        .filter_map(|r| r.split('/').next())
        .collect();
    if !packs.is_empty() {
        fields.push((
            "Rule packs".to_string(),
            packs.into_iter().collect::<Vec<_>>().join(", "),
        ));
    }
    if let Some(total) = cost.unit_total {
        fields.push((
            "Indicative unit cost".to_string(),
            format!(
                "{} {} at {} pcs (not a quotation)",
                num(total),
                cost.currency,
                cost.quantity
            ),
        ));
    }
    blocks.push(Block::Fields(fields));
    blocks.push(Block::Text(RULE_DISCLAIMER.to_string()));
    blocks
}

fn compliance_blocks(trace: Option<&Traceability>, checks: Option<&CheckSummary>) -> Vec<Block> {
    let mut blocks = vec![Block::Heading("COMPLIANCE".to_string())];
    if let Some(t) = trace {
        blocks.push(Block::Section("REQUIREMENT TRACEABILITY".to_string()));
        let mut table = Table::new(
            &["REQ", "REQUIREMENT", "STATUS", "VERIFIED BY"],
            &[1.0, 6.0, 1.6, 4.0],
        );
        for row in &t.rows {
            table.row([
                row.id.clone(),
                row.text.clone(),
                trace_word(row.status).to_string(),
                row.checks
                    .iter()
                    .map(|c| c.check.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            ]);
        }
        blocks.push(Block::Table(table));
        if !t.unknown_requirements.is_empty() {
            blocks.push(Block::Text(format!(
                "Checks refer to requirement ids that are not in the brief: {}.",
                t.unknown_requirements.join(", ")
            )));
        }
    }
    blocks.push(Block::Section("DESIGN CHECKS".to_string()));
    match checks {
        Some(c) if !c.results.is_empty() => {
            let mut table = Table::new(
                &["STATUS", "CHECK", "RESULT", "BASIS"],
                &[1.1, 3.2, 6.0, 6.0],
            );
            for r in &c.results {
                table.row([
                    status_word(r.status, r.severity).to_string(),
                    check_label(r),
                    r.message.clone(),
                    r.citation.clone().unwrap_or_default(),
                ]);
            }
            blocks.push(Block::Table(table));
        }
        _ => blocks.push(Block::Text("No design checks were evaluated.".to_string())),
    }
    blocks.push(Block::Text(RULE_DISCLAIMER.to_string()));
    blocks
}

// ---------------------------------------------------------------------
// Laporan Markdown.
// ---------------------------------------------------------------------

fn md_cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

/// `traceability.md`: matriks persyaratan → check.
pub fn traceability_md(t: &Traceability) -> String {
    let mut md = format!(
        "# Requirement traceability\n\nVerified {} · failed {} · without a check {}\n\n| Req | Requirement | Status | Verified by |\n|---|---|---|---|\n",
        t.verified, t.failed, t.unverified
    );
    for row in &t.rows {
        let checks: Vec<String> = row
            .checks
            .iter()
            .map(|c| {
                format!(
                    "{} ({})",
                    c.check,
                    status_word(c.status, Severity::Error).to_lowercase()
                )
            })
            .collect();
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            md_cell(&row.id),
            md_cell(&row.text),
            trace_word(row.status).to_lowercase(),
            md_cell(&checks.join(", "))
        ));
    }
    if !t.unknown_requirements.is_empty() {
        md.push_str(&format!(
            "\nChecks refer to requirement ids that are not in the brief: {}.\n",
            t.unknown_requirements.join(", ")
        ));
    }
    md
}

/// `compliance.md`: setiap check dengan hasil dan sitasinya.
pub fn compliance_md(c: &CheckSummary) -> String {
    let mut md = format!(
        "# Compliance\n\nPassed {} · failed {} · error {} · warning {}\n\n| Status | Check | Result | Basis |\n|---|---|---|---|\n",
        c.pass, c.fail, c.error, c.warn
    );
    for r in &c.results {
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            status_word(r.status, r.severity).to_lowercase(),
            md_cell(&check_label(r)),
            md_cell(&r.message),
            md_cell(r.citation.as_deref().unwrap_or(""))
        ));
    }
    md.push_str(&format!("\n> {RULE_DISCLAIMER}\n"));
    md
}

/// `cost.md`: estimasi biaya indikatif.
pub fn cost_md(c: &CostReport) -> String {
    let money = |v: Option<f64>| v.map(num).unwrap_or_else(|| "-".to_string());
    let mut md = format!(
        "# Indicative cost\n\n> Indicative only, not a quotation. Prices and rates come from the firm profile.\n\nQuantity {} · currency {}\n\n| Body | Material | Mass (g) | Stock (cm³) | Material | Hours | Process | Unit |\n|---|---|---:|---:|---:|---:|---:|---:|\n",
        c.quantity, c.currency
    );
    for l in &c.lines {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            md_cell(&l.body),
            md_cell(&l.material),
            money(l.mass_g),
            num(l.stock_cm3),
            money(l.material_cost),
            l.process_hours
                .map(|h| format!("{h:.3}"))
                .unwrap_or_else(|| "-".to_string()),
            money(l.process_cost),
            money(l.unit_cost)
        ));
    }
    if let Some(s) = c.setup_per_unit {
        md.push_str(&format!("\nSetup per unit: {} {}\n", num(s), c.currency));
    }
    match c.unit_total {
        Some(t) => md.push_str(&format!("\n**Unit total: {} {}**\n", num(t), c.currency)),
        None => md.push_str("\nUnit total: not available (see notes).\n"),
    }
    let notes: Vec<String> = c
        .warnings
        .iter()
        .cloned()
        .chain(
            c.lines
                .iter()
                .flat_map(|l| l.notes.iter().map(move |n| format!("{}: {n}", l.body))),
        )
        .collect();
    if !notes.is_empty() {
        md.push_str("\n## Notes\n\n");
        for n in notes {
            md.push_str(&format!("- {n}\n"));
        }
    }
    md
}

/// Catatan proyek untuk vault memori agent (`Projects/<part>.md`): tujuan,
/// persyaratan + statusnya, keputusan revisi, dan angka kunci. Ber-frontmatter
/// sesuai konvensi vault (`tags: [ducad, project]`); tidak memuat geometri.
pub fn project_note_md(
    meta: &SessionMeta,
    info: &PackageInfo,
    source: &str,
    out: &PackageOutput,
    checks: Option<&CheckSummary>,
) -> String {
    let design = &meta.design;
    let mut md = format!(
        "---\ntags: [ducad, project]\npart: {}\nrevision: {}\nstate: {}\nupdated: {}\n---\n# {}\n\nSource: `{source}`\n",
        info.part_number,
        out.release.revision,
        out.state.label(),
        info.date,
        info.title
    );
    if let Some(b) = &design.brief {
        md.push_str("\n## Brief\n\n");
        let mut line = |k: &str, v: String| {
            if !v.trim().is_empty() {
                md.push_str(&format!("- {k}: {v}\n"));
            }
        };
        line("Function", b.function.clone());
        line("Interfaces", b.interfaces.join("; "));
        line("Loads", b.loads.join("; "));
        line("Material", b.material.clone());
        line(
            "Process",
            b.process
                .and_then(|p| serde_json::to_value(p).ok())
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
        );
        line(
            "Quantity",
            b.quantity.map(|q| q.to_string()).unwrap_or_default(),
        );
        line("Standards", b.standards.join("; "));
        line("Cost target", b.cost_target.clone());
        for n in &b.notes {
            md.push_str(&format!("- Note: {n}\n"));
        }
    }
    if let Some(t) = &out.traceability {
        md.push_str("\n## Requirements\n\n");
        for row in &t.rows {
            md.push_str(&format!(
                "- {} [{}] {}\n",
                row.id,
                trace_word(row.status).to_lowercase(),
                row.text
            ));
        }
    }
    if let Some(c) = checks {
        md.push_str(&format!(
            "\n## Checks\n\nPassed {} · failed {} · error {} · warning {}\n",
            c.pass, c.fail, c.error, c.warn
        ));
        for r in c.results.iter().filter(|r| r.status != CheckStatus::Pass) {
            md.push_str(&format!(
                "- {} {}: {}\n",
                status_word(r.status, r.severity).to_lowercase(),
                check_label(r),
                r.message
            ));
        }
    }
    md.push_str("\n## Numbers\n\n");
    for l in &out.bom {
        md.push_str(&format!(
            "- {} x{}: {} mm3, {} g, {}\n",
            l.part,
            l.qty,
            num(l.volume_mm3),
            l.mass_g.map(num).unwrap_or_else(|| "?".to_string()),
            l.material
        ));
    }
    if let Some(total) = out.cost.unit_total {
        md.push_str(&format!(
            "- Indicative unit cost: {} {} at {} pcs\n",
            num(total),
            out.cost.currency,
            out.cost.quantity
        ));
    }
    if !out.release.history.is_empty() {
        md.push_str("\n## Revisions\n\n");
        for h in &out.release.history {
            md.push_str(&format!(
                "- {} ({}, {}): {}\n",
                h.revision, h.date, h.by, h.description
            ));
        }
    }
    md
}

/// Teks sel aman-PDF (dipakai tes dan pemanggil yang menyusun tabel sendiri).
pub fn pdf_text(text: &str) -> String {
    printable(text)
}
