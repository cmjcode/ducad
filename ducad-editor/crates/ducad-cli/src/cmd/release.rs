//! `ducad-cli release` (W5): gerbang persetujuan paket desain.
//!
//! Persetujuan adalah tindakan MANUSIA: perintah ini dijalankan pengguna di
//! terminal, dan tool agent pada sesi live tidak punya padanannya. Status
//! tersimpan di `design.release` berkas `.ducad`; persetujuan terikat pada
//! tanda tangan desain, jadi otomatis tidak berlaku begitu desain berubah.

use std::path::PathBuf;

use ducad_engine::brief::{review_blockers, review_checklist, today, ReleaseState};

use super::{open_part, print_json};
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    /// Part `.ducad`.
    part: PathBuf,
    /// Ajukan untuk direview dengan deskripsi perubahan (masuk tabel revisi).
    #[arg(long, value_name = "DESKRIPSI", conflicts_with_all = ["approve", "reopen"])]
    submit: Option<String>,
    /// Setujui revisi yang sedang berjalan (butuh `--by`).
    #[arg(long, conflicts_with = "reopen")]
    approve: bool,
    /// Cabut persetujuan / kembalikan ke draf.
    #[arg(long)]
    reopen: bool,
    /// Nama pengaju atau penyetuju.
    #[arg(long, default_value = "")]
    by: String,
    /// `YYYY-MM-DD`; default hari ini.
    #[arg(long)]
    date: Option<String>,
    /// Setujui walau ceklis review belum bersih (dicatat di keluaran).
    #[arg(long, requires = "approve")]
    force: bool,
    #[arg(long)]
    json: bool,
}

pub fn exec(a: Args) -> CliResult {
    let mut s = open_part(&a.part)?;
    let (mut release, signature) = s.release();
    let date = a.date.clone().unwrap_or_else(today);
    // Check studi (`max_stress`, `min_safety_factor`, …) butuh hasil segar.
    s.refresh_studies();
    let results = s.run_checks(None).results;
    let review = review_checklist(s.design(), &results);
    let blockers = review_blockers(&review);
    let mut changed = false;

    if let Some(description) = &a.submit {
        if release.effective_state(&signature) == ReleaseState::Approved {
            return Err(CliError::usage(format!(
                "revisi {} sudah disetujui dan desain belum berubah",
                release.revision
            )));
        }
        release.submit(&signature, description, &a.by, &date);
        changed = true;
    } else if a.approve {
        if a.by.trim().is_empty() {
            return Err(CliError::usage("--approve butuh --by NAMA"));
        }
        if !blockers.is_empty() && !a.force {
            for b in &blockers {
                eprintln!("ducad-cli: belum boleh disetujui — {b}");
            }
            return Ok(Exit::NotReleased);
        }
        release.approve(&signature, &a.by, &date)?;
        changed = true;
    } else if a.reopen {
        release.reopen();
        changed = true;
    }

    if changed {
        s.set_release(release.clone());
        s.save(&a.part)?;
    }
    let state = release.effective_state(&signature);
    if a.json {
        print_json(&serde_json::json!({
            "state": state,
            "revision": release.revision,
            "approval": release.approval,
            "approval_stale": release.approval_is_stale(&signature),
            "history": release.history,
            "review": review,
            "blockers": blockers,
            "forced": a.force && !blockers.is_empty(),
        }))?;
    } else {
        println!("status: {} · revisi {}", state.label(), release.revision);
        if release.approval_is_stale(&signature) {
            println!("persetujuan sebelumnya tidak berlaku: desain berubah sejak disetujui");
        }
        if let Some(ap) = release
            .approval
            .as_ref()
            .filter(|_| state == ReleaseState::Approved)
        {
            println!("disetujui {} pada {}", ap.by, ap.date);
        }
        for item in &review {
            println!(
                "{} {}: {}",
                if item.ok { "✓" } else { "✗" },
                item.item,
                item.detail
            );
        }
        for h in &release.history {
            println!(
                "rev {} · {} · {} · {}",
                h.revision, h.date, h.by, h.description
            );
        }
    }
    Ok(Exit::Ok)
}
