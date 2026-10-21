//! Operasi kernel dengan batas waktu (Step 3 `docs/PLAN_DESIGNER_UX.md`,
//! B1-1): boolean truck bisa berputar sangat lama pada konfigurasi tertentu
//! (mis. fillet + lubang berimpit di dua versi part). Daripada membekukan
//! aplikasi, operasi dijalankan di thread pekerja; lewat batas waktu →
//! `kernel_timeout`, model tidak berubah.
//!
//! Thread yang ditinggalkan tidak bisa dihentikan paksa. Agar CPU tidak
//! terkuras, paling banyak [`MAX_ABANDONED`] pekerjaan yang ditinggalkan
//! boleh masih berjalan; lebih dari itu permintaan baru langsung ditolak.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use crate::error::{OpError, OpErrorCode, OpResult};

/// Batas waktu op pengguna (boolean di oplog/GUI).
pub const OP_BUDGET: Duration = Duration::from_secs(45);
/// Batas waktu pratinjau (diff/ghost): boleh dilewati tanpa merugikan.
pub const PREVIEW_BUDGET: Duration = Duration::from_secs(8);

static ABANDONED: AtomicUsize = AtomicUsize::new(0);

/// Pekerjaan yang ditinggalkan boleh berjalan bersamaan paling banyak
/// sebanyak ini; lebih dari itu permintaan baru ditolak agar CPU tidak habis.
pub const MAX_ABANDONED: usize = 2;

/// Jumlah pekerjaan kernel yang ditinggalkan dan masih berjalan.
pub fn abandoned_jobs() -> usize {
    ABANDONED.load(Ordering::SeqCst)
}

fn timeout_error(label: &str, budget: Duration) -> OpError {
    OpError::new(
        OpErrorCode::KernelTimeout,
        format!(
            "{label} melewati batas waktu {} s; model tidak diubah",
            budget.as_secs()
        ),
    )
    .with_hint("kernel geometri tidak menyelesaikan kasus ini; ubah sedikit ukuran/posisi atau urutan fitur lalu coba lagi")
    .with_context(serde_json::json!({ "budget_s": budget.as_secs() }))
}

/// Jalankan `f` paling lama `budget`. Panik di dalam `f` menjadi error
/// `internal` (pagar ADR 0009).
pub fn run<T: Send + 'static>(
    label: &str,
    budget: Duration,
    f: impl FnOnce() -> OpResult<T> + Send + 'static,
) -> OpResult<T> {
    if abandoned_jobs() >= MAX_ABANDONED {
        return Err(OpError::new(
            OpErrorCode::KernelTimeout,
            format!("{label} ditunda: kernel masih menyelesaikan operasi sebelumnya yang melewati batas waktu"),
        )
        .with_hint("tunggu beberapa saat; bila terus terjadi, simpan dokumen lalu buka ulang aplikasi"));
    }
    let (tx, rx) = mpsc::channel();
    let done = Arc::new(AtomicBool::new(false));
    let worker_done = done.clone();
    let what = label.to_string();
    let spawned = std::thread::Builder::new()
        .name("ducad-kernel".to_string())
        .spawn(move || {
            let r = crate::guard::catch_op_panic(&what, f);
            let _ = tx.send(r);
            // `true` = pemanggil sudah menyerah: lepaskan jatah pekerjaan
            // yang ditinggalkan.
            if worker_done.swap(true, Ordering::SeqCst) {
                ABANDONED.fetch_sub(1, Ordering::SeqCst);
            }
        });
    let Ok(handle) = spawned else {
        return Err(OpError::new(
            OpErrorCode::Internal,
            format!("{label}: thread kernel tidak bisa dibuat"),
        ));
    };
    match rx.recv_timeout(budget) {
        Ok(r) => {
            let _ = handle.join();
            r
        }
        Err(_) => {
            ABANDONED.fetch_add(1, Ordering::SeqCst);
            if done.swap(true, Ordering::SeqCst) {
                // Pekerja selesai tepat setelah batas waktu: hasilnya ada.
                ABANDONED.fetch_sub(1, Ordering::SeqCst);
                return rx
                    .recv()
                    .unwrap_or_else(|_| Err(timeout_error(label, budget)));
            }
            Err(timeout_error(label, budget))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_job_returns_result() {
        assert_eq!(run("cepat", Duration::from_secs(5), || Ok(7)).unwrap(), 7);
    }

    #[test]
    fn panic_in_job_becomes_internal_error() {
        let e = run::<()>("panik", Duration::from_secs(5), || panic!("boom")).unwrap_err();
        assert_eq!(e.code, OpErrorCode::Internal);
    }
}
