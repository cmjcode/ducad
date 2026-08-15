//! Worker latar belakang untuk Import STEP (Fase 7 — "tessellation di
//! thread terpisah" dari `docs/PLAN.md`). Import STEP adalah satu-satunya
//! operasi kernel yang realistis blocking lama (file besar → `read_step` +
//! `tessellate` bisa makan waktu berarti) yang juga punya jalur murah untuk
//! di-background-kan tanpa memindahkan `KernelShape` lintas thread —
//! `KernelShape` TERBUKTI TIDAK `Send` (lihat komentar `KERNEL_LOCK` di
//! `cadraw-kernel`), jadi worker di sini TIDAK PERNAH mengirim `KernelShape`
//! lewat channel. Yang lewat cuma tipe `Send` murni: `PathBuf` masuk,
//! `String` (teks STEP) + `KernelMesh` keluar — thread utama membangun
//! `KernelShape` MILIKNYA SENDIRI dari string itu (`from_step_string`)
//! untuk disimpan di `ModelDoc`, sesuai pola "raw types at the kernel
//! boundary" yang sama dipakai `KernelMesh`/`Profile` sejak Fase 0/3.
//!
//! Aman dipanggil bersamaan dengan operasi kernel synchronous lain di UI
//! thread (Extrude/Fillet/dst) karena `cadraw-kernel::KERNEL_LOCK`
//! menyerialkan SEMUA panggilan OCCT lintas thread — worker ini cuma
//! membuat UI tidak *beku* menunggu, bukan membuat operasi kernel jalan
//! paralel (OCCT memang tidak mendukung itu).

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use cadraw_kernel::KernelMesh;

/// Satu permintaan import — `name` sudah diturunkan dari nama file di sisi
/// pemanggil (`pick_open_path` sebelumnya), supaya worker tidak perlu tahu
/// apa-apa soal `PathBuf` selain membacanya.
pub struct ImportJob {
    pub name: String,
    pub path: PathBuf,
}

/// Hasil satu `ImportJob` — `outcome` berisi teks STEP (bukan `KernelShape`,
/// lihat komentar modul) + mesh siap render, atau pesan error apa adanya
/// (sama gaya dengan `import_step` synchronous sebelumnya).
pub struct ImportResult {
    pub name: String,
    pub outcome: Result<(String, KernelMesh), String>,
}

/// Handle sisi UI thread: `submit` mengirim job (non-blocking, fire-and-
/// forget), `poll` dipanggil tiap frame dari `update()` (non-blocking,
/// `try_recv` di baliknya) untuk mengambil hasil yang sudah siap.
pub struct ImportWorker {
    sender: Sender<ImportJob>,
    receiver: Receiver<ImportResult>,
}

impl ImportWorker {
    /// Spawn SATU thread worker berumur-panjang (bukan satu thread per
    /// job) — job diproses satu-satu dari channel, urutan submit = urutan
    /// selesai (cukup untuk pola pakai "import lalu tunggu", tidak perlu
    /// paralelisme sungguhan karena `KERNEL_LOCK` menyerialkannya juga).
    pub fn spawn() -> Self {
        let (job_tx, job_rx) = mpsc::channel::<ImportJob>();
        let (result_tx, result_rx) = mpsc::channel::<ImportResult>();
        std::thread::Builder::new()
            .name("cadraw-import".to_string())
            .spawn(move || {
                for job in job_rx {
                    let outcome = import_one(&job.path);
                    // Penerima (UI thread) sudah drop → aplikasi lagi
                    // menutup; abaikan error kirim, jangan panic thread
                    // worker cuma karena UI sudah tidak dengar lagi.
                    let _ = result_tx.send(ImportResult { name: job.name, outcome });
                }
            })
            .expect("gagal spawn thread import STEP");
        Self { sender: job_tx, receiver: result_rx }
    }

    pub fn submit(&self, job: ImportJob) {
        // Sender cuma gagal kalau thread worker sudah mati (panic) — tidak
        // ada yang bisa dilakukan UI selain mengabaikannya; `import_step`
        // pemanggil tidak menjanjikan hasil sinkron lagi pula.
        let _ = self.sender.send(job);
    }

    /// Ambil SEMUA hasil yang sudah siap sejak `poll` terakhir — biasanya
    /// 0 atau 1 per frame, tapi tidak mengasumsikan itu (user bisa submit
    /// beberapa import beruntun sebelum yang pertama selesai).
    pub fn poll(&self) -> Vec<ImportResult> {
        self.receiver.try_iter().collect()
    }
}

fn import_one(path: &std::path::Path) -> Result<(String, KernelMesh), String> {
    let shape = cadraw_kernel::KernelShape::read_step(path).map_err(|e| e.to_string())?;
    let mesh = shape.tessellate();
    let step = shape.to_step_string().map_err(|e| e.to_string())?;
    Ok((step, mesh))
}
