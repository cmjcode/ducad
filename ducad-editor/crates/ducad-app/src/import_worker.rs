//! Worker latar belakang untuk Import STEP (Fase 7 — "tessellation di
//! thread terpisah" dari `docs/PLAN.md`). Import STEP adalah satu-satunya
//! operasi kernel yang realistis blocking lama (file besar → `read_step` +
//! `tessellate` bisa makan waktu berarti) yang juga punya jalur murah untuk
//! di-background-kan tanpa memindahkan `KernelShape` lintas thread —
//! `KernelShape` TERBUKTI TIDAK `Send` (lihat komentar `KERNEL_LOCK` di
//! `ducad-kernel`), jadi worker di sini TIDAK PERNAH mengirim `KernelShape`
//! lewat channel. Yang lewat cuma tipe `Send` murni: `PathBuf` masuk,
//! `String` (teks STEP) + `KernelMesh` keluar — thread utama membangun
//! `KernelShape` MILIKNYA SENDIRI dari string itu (`from_step_string`)
//! untuk disimpan di `ModelDoc`, sesuai pola "raw types at the kernel
//! boundary" yang sama dipakai `KernelMesh`/`Profile` sejak Fase 0/3.
//!
//! Aman dipanggil bersamaan dengan operasi kernel synchronous lain di UI
//! thread (Extrude/Fillet/dst) karena `ducad-kernel::KERNEL_LOCK`
//! menyerialkan SEMUA panggilan OCCT lintas thread — worker ini cuma
//! membuat UI tidak *beku* menunggu, bukan membuat operasi kernel jalan
//! paralel (OCCT memang tidak mendukung itu).

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use ducad_kernel::{KernelMesh, KernelShape};

/// Satu permintaan import — `name` sudah diturunkan dari nama file di sisi
/// pemanggil (`pick_open_path` sebelumnya), supaya worker tidak perlu tahu
/// apa-apa soal `PathBuf` selain membacanya.
pub struct ImportJob {
    pub name: String,
    pub path: PathBuf,
}

/// Hasil satu `ImportJob` — `outcome` berisi `KernelShape` dan `KernelMesh`
/// yang di-tessellate di background thread, atau pesan error.
pub struct ImportResult {
    pub name: String,
    pub outcome: Result<(KernelShape, KernelMesh), String>,
}

/// Handle sisi UI thread: `submit` mengirim job (non-blocking, fire-and-
/// forget), `poll` dipanggil tiap frame dari `update()` (non-blocking,
/// `try_recv` di baliknya) untuk mengambil hasil yang sudah siap.
pub struct ImportWorker {
    sender: Sender<ImportJob>,
    receiver: Receiver<ImportResult>,
}

impl ImportWorker {
    /// Spawn thread worker berumur panjang — import STEP dan tessellation
    /// berjalan asinkron di thread ini sehingga UI thread tidak pernah lag/freeze.
    pub fn spawn() -> Self {
        let (job_tx, job_rx) = mpsc::channel::<ImportJob>();
        let (result_tx, result_rx) = mpsc::channel::<ImportResult>();
        std::thread::Builder::new()
            .name("ducad-import".to_string())
            .spawn(move || {
                for job in job_rx {
                    let outcome = import_one(&job.path);
                    let _ = result_tx.send(ImportResult { name: job.name, outcome });
                }
            })
            .expect("gagal spawn thread import STEP");
        Self { sender: job_tx, receiver: result_rx }
    }

    pub fn submit(&self, job: ImportJob) {
        let _ = self.sender.send(job);
    }

    /// Ambil SEMUA hasil yang sudah siap sejak `poll` terakhir.
    pub fn poll(&self) -> Vec<ImportResult> {
        self.receiver.try_iter().collect()
    }
}

fn import_one(path: &std::path::Path) -> Result<(KernelShape, KernelMesh), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "stl" {
        let mesh = ducad_io::read_stl(path).map_err(|e| e.to_string())?;
        let shape = ducad_kernel::KernelShape::empty();
        Ok((shape, mesh))
    } else {
        let shape = ducad_kernel::KernelShape::read_step(path).map_err(|e| e.to_string())?;
        let mesh = shape.tessellate();
        Ok((shape, mesh))
    }
}
