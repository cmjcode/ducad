//! Brief desain, persyaratan, dan keterlacakan (W0), serta status rilis paket
//! (W5). Semuanya tinggal di `DesignDoc` sebagai data, bukan op: mengubah
//! brief tidak mengubah geometri (pola yang sama dengan `drawings`, ADR 0007).
//!
//! Doc comment `///` pada tipe di berkas ini ikut menjadi kontrak yang dibaca
//! agent (skema tool `set_brief`), jadi ditulis dalam bahasa Inggris.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::check::{CheckResult, CheckStatus, Process, Severity};
use crate::error::{OpError, OpResult};

/// One requirement the design must meet. Checks point at it through their `requirement` field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    /// Short unique id, e.g. `"R1"`.
    pub id: String,
    /// The requirement in the user's words, with numbers and units.
    pub text: String,
    /// Where it comes from: `"user"`, a standard, a mating part, a firm rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// The design brief: what is being designed and why. Written before modeling.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignBrief {
    /// Name of the part or assembly.
    #[serde(default)]
    pub title: String,
    /// What the part has to do, in one or two sentences.
    #[serde(default)]
    pub function: String,
    /// Mating parts and interfaces (bolt patterns, shafts, envelopes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<String>,
    /// Loads and operating conditions (forces, torques, temperature).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loads: Vec<String>,
    /// Material, e.g. `"Al 6061-T6"`.
    #[serde(default)]
    pub material: String,
    /// Manufacturing process; selects the matching DFM rule pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<Process>,
    /// Production quantity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,
    /// Standards the design must follow, e.g. `"ISO 2768-m"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub standards: Vec<String>,
    /// Cost target as free text, e.g. `"< 15 USD at 100 pcs"`.
    #[serde(default)]
    pub cost_target: String,
    /// Anything else worth remembering (decisions, open questions).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Numbered requirements; every one should be verified by at least one check.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<Requirement>,
}

impl DesignBrief {
    /// Validasi bentuk: id persyaratan unik dan tidak kosong.
    pub fn validate(&self) -> OpResult<()> {
        let mut seen: Vec<&str> = Vec::new();
        for r in &self.requirements {
            let id = r.id.trim();
            if id.is_empty() || id.len() > 32 || id.chars().any(char::is_whitespace) {
                return Err(OpError::invalid(format!(
                    "requirement id '{}' must be 1-32 characters without spaces (e.g. \"R1\")",
                    r.id
                )));
            }
            if seen.contains(&id) {
                return Err(OpError::invalid(format!(
                    "requirement id '{id}' is used twice"
                )));
            }
            if r.text.trim().is_empty() {
                return Err(OpError::invalid(format!(
                    "requirement '{id}' has an empty text"
                )));
            }
            seen.push(id);
        }
        Ok(())
    }

    /// Nama paket aturan DFM yang cocok dengan proses di brief.
    pub fn suggested_rule_pack(&self) -> Option<&'static str> {
        Some(match self.process? {
            Process::Cnc => "dfm-cnc",
            Process::SheetMetal => "dfm-sheet-metal",
            Process::Fdm | Process::Sla | Process::Sls => "dfm-fdm",
            Process::Injection | Process::Casting => "dfm-injection",
        })
    }
}

/// Status satu persyaratan di matriks keterlacakan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceStatus {
    /// Semua check tertautnya lulus.
    Verified,
    /// Ada check tertaut yang tidak lulus.
    Failed,
    /// Belum ada check yang menautnya.
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraceCheck {
    /// Id check, atau jenisnya bila tanpa id.
    pub check: String,
    pub status: CheckStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraceRow {
    pub id: String,
    pub text: String,
    pub status: TraceStatus,
    pub checks: Vec<TraceCheck>,
}

/// Matriks keterlacakan persyaratan → check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Traceability {
    pub verified: usize,
    pub failed: usize,
    pub unverified: usize,
    pub rows: Vec<TraceRow>,
    /// Check yang menaut id persyaratan yang tidak ada di brief.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unknown_requirements: Vec<String>,
}

impl Traceability {
    /// Semua persyaratan terverifikasi (dan ada setidaknya satu).
    pub fn complete(&self) -> bool {
        self.failed == 0 && self.unverified == 0
    }
}

fn check_label(r: &CheckResult) -> String {
    r.id.clone().unwrap_or_else(|| r.kind.to_string())
}

/// Bangun matriks dari brief dan hasil check. Check `warning` yang gagal
/// tidak menggagalkan persyaratannya (konsisten dengan build).
pub fn traceability(brief: &DesignBrief, results: &[CheckResult]) -> Traceability {
    let rows: Vec<TraceRow> = brief
        .requirements
        .iter()
        .map(|req| {
            let linked: Vec<&CheckResult> = results
                .iter()
                .filter(|r| r.requirement.as_deref() == Some(req.id.as_str()))
                .collect();
            let status = if linked.is_empty() {
                TraceStatus::Unverified
            } else if linked
                .iter()
                .any(|r| r.status != CheckStatus::Pass && r.severity == Severity::Error)
            {
                TraceStatus::Failed
            } else {
                TraceStatus::Verified
            };
            TraceRow {
                id: req.id.clone(),
                text: req.text.clone(),
                status,
                checks: linked
                    .iter()
                    .map(|r| TraceCheck {
                        check: check_label(r),
                        status: r.status,
                        citation: r.citation.clone(),
                    })
                    .collect(),
            }
        })
        .collect();
    let mut unknown: Vec<String> = results
        .iter()
        .filter_map(|r| r.requirement.clone())
        .filter(|id| !brief.requirements.iter().any(|q| &q.id == id))
        .collect();
    unknown.sort();
    unknown.dedup();
    let count = |s: TraceStatus| rows.iter().filter(|r| r.status == s).count();
    Traceability {
        verified: count(TraceStatus::Verified),
        failed: count(TraceStatus::Failed),
        unverified: count(TraceStatus::Unverified),
        rows,
        unknown_requirements: unknown,
    }
}

// ---------------------------------------------------------------------
// Status rilis paket (W5).
// ---------------------------------------------------------------------

/// Tahap persetujuan paket desain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseState {
    #[default]
    Draft,
    InReview,
    Approved,
}

impl ReleaseState {
    pub fn label(self) -> &'static str {
        match self {
            ReleaseState::Draft => "draft",
            ReleaseState::InReview => "in_review",
            ReleaseState::Approved => "approved",
        }
    }
}

/// Satu baris tabel revisi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevisionEntry {
    pub revision: String,
    /// `YYYY-MM-DD`.
    #[serde(default)]
    pub date: String,
    pub description: String,
    #[serde(default)]
    pub by: String,
}

/// Tanda tangan persetujuan: siapa, kapan, dan atas desain yang mana.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Approval {
    pub by: String,
    #[serde(default)]
    pub date: String,
    /// [`design_signature`] saat disetujui.
    pub signature: String,
}

/// Status rilis + riwayat revisi paket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReleaseInfo {
    #[serde(default)]
    pub state: ReleaseState,
    /// Revisi yang sedang dikerjakan/disetujui (`"A"`, `"B"`, …).
    pub revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<Approval>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<RevisionEntry>,
}

impl Default for ReleaseInfo {
    fn default() -> Self {
        Self {
            state: ReleaseState::Draft,
            revision: "A".to_string(),
            approval: None,
            history: Vec::new(),
        }
    }
}

/// Revisi berikutnya: `A`→`B`, …, `Z`→`AA`, `AZ`→`BA`. Bukan huruf → `A`.
pub fn next_revision(current: &str) -> String {
    let mut letters: Vec<u8> = current.trim().bytes().collect();
    if letters.is_empty() || !letters.iter().all(u8::is_ascii_uppercase) {
        return "A".to_string();
    }
    let mut i = letters.len();
    loop {
        if i == 0 {
            letters.insert(0, b'A');
            break;
        }
        i -= 1;
        if letters[i] == b'Z' {
            letters[i] = b'A';
        } else {
            letters[i] += 1;
            break;
        }
    }
    String::from_utf8(letters).unwrap_or_else(|_| "A".to_string())
}

impl ReleaseInfo {
    /// Status yang berlaku untuk desain bertanda tangan `signature`:
    /// persetujuan atas desain yang sudah berubah tidak berlaku lagi.
    pub fn effective_state(&self, signature: &str) -> ReleaseState {
        match (&self.state, &self.approval) {
            (ReleaseState::Approved, Some(a)) if a.signature == signature => ReleaseState::Approved,
            (ReleaseState::Approved, _) => ReleaseState::Draft,
            (other, _) => *other,
        }
    }

    /// Persetujuan tercatat tetapi desain sudah berubah sejak itu.
    pub fn approval_is_stale(&self, signature: &str) -> bool {
        self.state == ReleaseState::Approved
            && self.effective_state(signature) != ReleaseState::Approved
    }

    /// Ajukan untuk direview. Bila revisi sebelumnya sudah disetujui dan
    /// desain berubah, revisi naik dan `description` dicatat di riwayat.
    pub fn submit(&mut self, signature: &str, description: &str, by: &str, date: &str) {
        if self.approval_is_stale(signature) {
            self.revision = next_revision(&self.revision);
            self.approval = None;
        }
        if self.effective_state(signature) == ReleaseState::Approved {
            return;
        }
        self.state = ReleaseState::InReview;
        self.note(description, by, date);
    }

    /// Setujui desain bertanda tangan `signature`.
    pub fn approve(&mut self, signature: &str, by: &str, date: &str) -> OpResult<()> {
        if by.trim().is_empty() {
            return Err(OpError::invalid("approval needs the approver's name"));
        }
        if self.approval_is_stale(signature) {
            self.revision = next_revision(&self.revision);
        }
        self.state = ReleaseState::Approved;
        self.approval = Some(Approval {
            by: by.trim().to_string(),
            date: date.to_string(),
            signature: signature.to_string(),
        });
        if !self.history.iter().any(|h| h.revision == self.revision) {
            self.note("Released", by, date);
        }
        Ok(())
    }

    /// Kembalikan ke draf (persetujuan dicabut, revisi tetap).
    pub fn reopen(&mut self) {
        self.state = ReleaseState::Draft;
        self.approval = None;
    }

    /// Catat/ganti deskripsi revisi yang sedang berjalan.
    pub fn note(&mut self, description: &str, by: &str, date: &str) {
        let description = description.trim();
        match self
            .history
            .iter_mut()
            .find(|h| h.revision == self.revision)
        {
            Some(entry) => {
                if !description.is_empty() {
                    entry.description = description.to_string();
                }
                if !by.trim().is_empty() {
                    entry.by = by.trim().to_string();
                }
                if !date.is_empty() {
                    entry.date = date.to_string();
                }
            }
            None => self.history.push(RevisionEntry {
                revision: self.revision.clone(),
                date: date.to_string(),
                description: if description.is_empty() {
                    "Initial release".to_string()
                } else {
                    description.to_string()
                },
                by: by.trim().to_string(),
            }),
        }
    }
}

/// Satu butir ceklis review paket.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReviewItem {
    pub item: &'static str,
    pub ok: bool,
    pub detail: String,
}

/// Ceklis review sebelum paket disetujui. Butir `ok: false` tidak melarang
/// persetujuan secara teknis kecuali check yang menghalangi (lihat
/// [`review_blockers`]); sisanya adalah pengingat bagi reviewer.
pub fn review_checklist(
    design: &crate::session::DesignDoc,
    results: &[CheckResult],
) -> Vec<ReviewItem> {
    let blocking = results.iter().filter(|r| r.is_blocking()).count();
    let warn = results
        .iter()
        .filter(|r| r.status != CheckStatus::Pass && r.severity == Severity::Warning)
        .count();
    let mut items = vec![ReviewItem {
        item: "checks",
        ok: blocking == 0 && !results.is_empty(),
        detail: if results.is_empty() {
            "no design checks are installed".to_string()
        } else {
            format!(
                "{} checks, {blocking} blocking failure(s), {warn} warning(s)",
                results.len()
            )
        },
    }];
    match design.brief.as_ref() {
        Some(brief) => {
            let t = traceability(brief, results);
            items.push(ReviewItem {
                item: "requirements",
                ok: !brief.requirements.is_empty() && t.complete(),
                detail: format!(
                    "{} verified, {} failed, {} without a check",
                    t.verified, t.failed, t.unverified
                ),
            });
        }
        None => items.push(ReviewItem {
            item: "requirements",
            ok: false,
            detail: "no design brief".to_string(),
        }),
    }
    let cited = results.iter().filter(|r| r.citation.is_some()).count();
    items.push(ReviewItem {
        item: "standards",
        ok: cited > 0,
        detail: format!("{cited} check(s) cite a standard or guideline"),
    });
    items.push(ReviewItem {
        item: "drawings",
        ok: !design.drawings.is_empty(),
        detail: format!("{} stored sheet(s)", design.drawings.len()),
    });
    let open = design
        .drawings
        .iter()
        .map(ducad_io::drawing::open_redlines)
        .sum::<usize>();
    items.push(ReviewItem {
        item: "redlines",
        ok: open == 0,
        detail: format!("{open} open redline(s)"),
    });
    items
}

/// Alasan paket belum boleh disetujui/dirilis; kosong = boleh.
pub fn review_blockers(review: &[ReviewItem]) -> Vec<String> {
    review
        .iter()
        .filter(|i| !i.ok && matches!(i.item, "checks" | "redlines"))
        .map(|i| format!("{}: {}", i.item, i.detail))
        .collect()
}

/// Tanggal hari ini `YYYY-MM-DD` (UTC). Pemanggil yang butuh keluaran
/// deterministik memberi tanggalnya sendiri.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    date_from_epoch(secs)
}

/// Detik sejak 1970-01-01 UTC → `YYYY-MM-DD` (kalender Gregorian).
pub fn date_from_epoch(secs: i64) -> String {
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Tanda tangan desain untuk status rilis.
///
/// Part yang seluruh geometrinya berasal dari oplog cukup ditandai oleh
/// [`design_signature`] (tidak bergantung platform). Part dengan body bawaan
/// (`base_bodies`: dimodelkan di GUI atau hasil impor STEP) tidak bisa
/// direproduksi dari oplog saja, jadi sidik jari geometri model ikut masuk.
pub fn release_signature(
    model: &crate::model::ModelDoc,
    design: &crate::session::DesignDoc,
) -> String {
    let base = design_signature(design);
    if design.base_bodies.is_empty() {
        base
    } else {
        format!("{base}-{}", crate::session::fingerprint(model))
    }
}

/// Tanda tangan isi desain yang menentukan geometri dan kepatuhannya:
/// params, oplog, konfigurasi aktif, dan checks. Lembar gambar, brief, dan
/// status rilis sendiri tidak ikut.
pub fn design_signature(design: &crate::session::DesignDoc) -> String {
    let value = serde_json::json!({
        "params": design.params,
        "oplog": design.oplog,
        "checks": design.checks,
        "configurations": design.configurations,
        "active_configuration": design.active_configuration,
    });
    format!(
        "{:016x}",
        ducad_core::stable_hash(value.to_string().as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{Check, CheckItem};

    fn result(id: &str, req: Option<&str>, status: CheckStatus, severity: Severity) -> CheckResult {
        let _ = CheckItem::plain(Check::BodyCount { expect: 1 });
        CheckResult {
            index: 0,
            id: Some(id.to_string()),
            kind: "body_count",
            status,
            measured: serde_json::Value::Null,
            expected: serde_json::Value::Null,
            message: String::new(),
            location: None,
            body: None,
            requirement: req.map(str::to_string),
            citation: None,
            severity,
            rule: None,
        }
    }

    fn brief(ids: &[&str]) -> DesignBrief {
        DesignBrief {
            requirements: ids
                .iter()
                .map(|id| Requirement {
                    id: id.to_string(),
                    text: format!("persyaratan {id}"),
                    source: None,
                })
                .collect(),
            ..DesignBrief::default()
        }
    }

    #[test]
    fn traceability_groups_checks_per_requirement() {
        let t = traceability(
            &brief(&["R1", "R2", "R3", "R4"]),
            &[
                result("a", Some("R1"), CheckStatus::Pass, Severity::Error),
                result("b", Some("R1"), CheckStatus::Pass, Severity::Error),
                result("c", Some("R2"), CheckStatus::Fail, Severity::Error),
                result("d", Some("R4"), CheckStatus::Fail, Severity::Warning),
                result("e", Some("R9"), CheckStatus::Pass, Severity::Error),
                result("f", None, CheckStatus::Pass, Severity::Error),
            ],
        );
        assert_eq!((t.verified, t.failed, t.unverified), (2, 1, 1));
        assert_eq!(t.rows[0].checks.len(), 2);
        assert_eq!(t.rows[2].status, TraceStatus::Unverified);
        assert_eq!(
            t.rows[3].status,
            TraceStatus::Verified,
            "warning tidak menggagalkan"
        );
        assert_eq!(t.unknown_requirements, vec!["R9".to_string()]);
        assert!(!t.complete());
    }

    #[test]
    fn brief_rejects_duplicate_and_empty_requirement_ids() {
        assert!(brief(&["R1", "R2"]).validate().is_ok());
        assert!(brief(&["R1", "R1"]).validate().is_err());
        assert!(brief(&["R 1"]).validate().is_err());
        assert!(brief(&[""]).validate().is_err());
    }

    #[test]
    fn revisions_advance_like_drawing_letters() {
        assert_eq!(next_revision("A"), "B");
        assert_eq!(next_revision("Z"), "AA");
        assert_eq!(next_revision("AZ"), "BA");
        assert_eq!(next_revision("1"), "A");
    }

    #[test]
    fn approval_goes_stale_when_the_design_changes() {
        let mut r = ReleaseInfo::default();
        r.submit("sig1", "", "ana", "2026-10-13");
        assert_eq!(r.effective_state("sig1"), ReleaseState::InReview);
        assert!(r.approve("sig1", "", "2026-10-13").is_err());
        r.approve("sig1", "budi", "2026-10-13").unwrap();
        assert_eq!(r.effective_state("sig1"), ReleaseState::Approved);
        assert_eq!(r.history.len(), 1);

        // Desain berubah: persetujuan tidak berlaku, revisi naik saat diajukan lagi.
        assert_eq!(r.effective_state("sig2"), ReleaseState::Draft);
        r.submit("sig2", "Plat ditebalkan jadi 8 mm", "ana", "2026-10-14");
        assert_eq!(r.revision, "B");
        assert_eq!(r.effective_state("sig2"), ReleaseState::InReview);
        r.approve("sig2", "budi", "2026-10-14").unwrap();
        assert_eq!(r.history.len(), 2);
        assert_eq!(r.history[1].description, "Plat ditebalkan jadi 8 mm");
    }
}
