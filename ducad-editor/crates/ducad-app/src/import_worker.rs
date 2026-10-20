//! Worker latar belakang untuk Import STEP (Fase 7 — "tessellation di
//! thread terpisah" dari `docs/PLAN.md`). Import STEP adalah satu-satunya
//! operasi kernel yang realistis blocking lama (file besar → `read_step` +
//! `tessellate` bisa makan waktu berarti). Worker membaca file lalu
//! mengirim `KernelShape` + `KernelMesh` hasilnya lewat channel; UI thread
//! mengambilnya tiap frame lewat [`ImportWorker::poll`].
//!
//! Kernel tanpa kunci global (`ducad_kernel::THREAD_SAFE`, PLAN_MIGRASI_TRUCK
//! 5.4): import berjalan sungguh paralel dengan operasi di UI thread (diuji di
//! `ducad-kernel/tests/truck_concurrency.rs`).

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
    /// Diisi bila thread worker gagal dibuat: impor lalu dijalankan langsung
    /// di thread pemanggil (lambat, tetapi tidak crash; ADR 0009).
    inline: Option<Sender<ImportResult>>,
}

impl ImportWorker {
    /// Spawn thread worker berumur panjang — import STEP dan tessellation
    /// berjalan asinkron di thread ini sehingga UI thread tidak pernah lag/freeze.
    pub fn spawn() -> Self {
        let (job_tx, job_rx) = mpsc::channel::<ImportJob>();
        let (result_tx, result_rx) = mpsc::channel::<ImportResult>();
        let inline_tx = result_tx.clone();
        let spawned = std::thread::Builder::new()
            .name("ducad-import".to_string())
            .spawn(move || {
                for job in job_rx {
                    let outcome = import_one(&job.path);
                    let _ = result_tx.send(ImportResult { name: job.name, outcome });
                }
            });
        let inline = match spawned {
            Ok(_) => None,
            Err(e) => {
                log::error!("thread import STEP gagal dibuat ({e}); impor berjalan di UI thread");
                Some(inline_tx)
            }
        };
        Self { sender: job_tx, receiver: result_rx, inline }
    }

    pub fn submit(&self, job: ImportJob) {
        if let Some(tx) = &self.inline {
            let outcome = import_one(&job.path);
            let _ = tx.send(ImportResult { name: job.name, outcome });
            return;
        }
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
    } else if matches!(ext.as_str(), "3mf" | "obj" | "gltf" | "glb") {
        // Paritas 3.10: mesh (tanpa B-rep); beberapa objek digabung jadi satu.
        let meshes = ducad_io::mesh_formats::read_mesh_file(path).map_err(|e| e.to_string())?;
        let refs: Vec<&KernelMesh> = meshes.iter().map(|m| &m.mesh).collect();
        Ok((ducad_kernel::KernelShape::empty(), KernelMesh::merge(&refs)))
    } else if matches!(ext.as_str(), "igs" | "iges") {
        // Paritas 3.11: IGES → B-rep lewat penerjemah SolidCraft.
        let (shapes, warnings) = ducad_kernel::iges::read_iges(path).map_err(|e| e.to_string())?;
        for w in warnings {
            log::warn!("IGES: {w}");
        }
        let shape = if shapes.len() == 1 {
            shapes.into_iter().next().map(|(_, s)| s).ok_or("IGES kosong")?
        } else {
            let refs: Vec<&ducad_kernel::KernelShape> = shapes.iter().map(|(_, s)| s).collect();
            ducad_kernel::make_compound(&refs).map_err(|e| e.to_string())?
        };
        let mesh = shape.tessellate();
        Ok((shape, mesh))
    } else {
        let shape = ducad_kernel::KernelShape::read_step(path).map_err(|e| e.to_string())?;
        let mesh = shape.tessellate();
        Ok((shape, mesh))
    }
}
